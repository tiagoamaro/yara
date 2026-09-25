module Yara
  # One token: `kind` is a symbol (`:int`, `:ident`, `:plus`, ...); `value`
  # carries the payload for literal and identifier kinds and is nil otherwise.
  Token = Struct.new(:kind, :value, :line, :column)

  class LexError < Diagnostics::Error
    # @return [String]
    def kind
      "lex error"
    end
  end

  # Tokenizer. Walks the source one character at a time (no `Regexp`, which
  # mruby lacks), tracking the 1-indexed line and column of each token's first
  # character.
  class Lexer
    # English source spelling of every reserved word to its keyword token.
    # `true`/`false` are keywords that lex to `:bool` tokens.
    KEYWORDS = {
      "def" => :def, "end" => :end, "if" => :if, "elsif" => :elsif, "else" => :else,
      "while" => :while, "for" => :for, "in" => :in, "const" => :const, "return" => :return,
      "nil" => :nil, "import" => :import, "class" => :class, "true" => :true, "false" => :false
    }.freeze

    # How each token kind prints inside error messages, matching the Rust
    # `TokenKind` variant names.
    KIND_NAMES = {
      int: "Int", float: "Float", str: "Str", bool: "Bool", ident: "Ident",
      def: "Def", end: "End", if: "If", elsif: "Elsif", else: "Else", while: "While",
      for: "For", in: "In", const: "Const", return: "Return", nil: "Nil", import: "Import",
      class: "Class", plus: "Plus", minus: "Minus", star: "Star", slash: "Slash",
      eq_eq: "EqEq", not_eq: "NotEq", lt: "Lt", gt: "Gt", lt_eq: "LtEq", gt_eq: "GtEq",
      eq: "Eq", colon: "Colon", colon_eq: "ColonEq", dot_dot: "DotDot", dot: "Dot",
      l_paren: "LParen", r_paren: "RParen", l_bracket: "LBracket", r_bracket: "RBracket",
      comma: "Comma", eof: "Eof"
    }.freeze

    SINGLE_CHARACTER_KINDS = {
      "+" => :plus, "-" => :minus, "*" => :star, "/" => :slash, "(" => :l_paren,
      ")" => :r_paren, "[" => :l_bracket, "]" => :r_bracket, "," => :comma
    }.freeze

    # A character followed by `=` (or `.` by `.`) forms a two-character token.
    PAIRED_KINDS = {
      "=" => [:eq, :eq_eq], "<" => [:lt, :lt_eq], ">" => [:gt, :gt_eq], ":" => [:colon, :colon_eq]
    }.freeze

    ESCAPES = { "n" => "\n", "t" => "\t", "\"" => "\"", "\\" => "\\" }.freeze

    # Every character Rust's `char::is_whitespace` accepts.
    WHITESPACE = [
      "\t", "\n", "\v", "\f", "\r", " ", "\u0085", " ", " ", " ", " ",
      " ", " ", " ", " ", " ", " ", " ", " ",
      " ", " ", " ", " ", " ", "　"
    ].freeze

    I64_MAX = 9_223_372_036_854_775_807

    # How a token prints inside error messages, byte-identical to Rust's
    # `TokenKind` display (`Ident("foo")`, `Int(5)`, `Float(5.0)`, `Plus`).
    #
    # @param token [Token]
    # @return [String]
    def self.describe(token)
      name = KIND_NAMES.fetch(token.kind)
      case token.kind
      when :int, :bool then "#{name}(#{token.value})"
      when :float then "#{name}(#{RustFormat.float_debug(token.value)})"
      when :str, :ident then "#{name}(#{RustFormat.string_debug(token.value)})"
      else name
      end
    end

    # @param source [String]
    # @param vocabulary [Vocabulary] keyword spellings and error messages
    def initialize(source, vocabulary = Vocabulary.english)
      @chars = source.chars
      @position = 0
      @line = 1
      @column = 1
      @vocabulary = vocabulary
    end

    # @return [Array<Token>] ending in an `:eof` token
    # @raise [LexError]
    def tokenize
      tokens = []
      loop do
        skip_whitespace_and_comments
        line = @line
        column = @column
        char = peek
        if char.nil?
          tokens << Token.new(:eof, nil, line, column)
          return tokens
        end

        kind, value =
          if ascii_digit?(char)
            read_number
          elsif char == "\""
            read_string
          elsif alphabetic?(char) || char == "_"
            read_ident_or_keyword
          else
            read_operator
          end
        tokens << Token.new(kind, value, line, column)
      end
    end

    private

    # @return [String, nil]
    def peek(offset = 0)
      @chars[@position + offset]
    end

    # Consumes one character, moving to the next line after a newline.
    #
    # @return [String, nil]
    def advance
      char = peek
      return nil if char.nil?

      @position += 1
      if char == "\n"
        @line += 1
        @column = 1
      else
        @column += 1
      end
      char
    end

    # @return [void]
    def skip_whitespace_and_comments
      loop do
        char = peek
        if !char.nil? && WHITESPACE.include?(char)
          advance
        elsif char == "#"
          advance until peek.nil? || peek == "\n"
        else
          return
        end
      end
    end

    # Digits, optionally followed by `.` and more digits (`1.` alone stays an
    # integer followed by `.`, so `0..10` lexes as a range).
    #
    # @return [Array(Symbol, Numeric)]
    def read_number
      line = @line
      column = @column
      start = @position
      advance while ascii_digit?(peek)
      is_float = peek == "." && ascii_digit?(peek(1))
      if is_float
        advance
        advance while ascii_digit?(peek)
      end
      text = @chars[start...@position].join
      return [:float, RustFormat.parse_float(text)] if is_float

      value = text.to_i
      raise error("lex/invalid-integer-literal", [text], line, column) if value > I64_MAX

      [:int, value]
    end

    # @return [Array(Symbol, String)]
    def read_string
      line = @line
      column = @column
      advance
      value = +""
      loop do
        char = peek
        raise error("lex/unterminated-string", [], line, column) if char.nil? || char == "\n"

        advance
        return [:str, value] if char == "\""

        if char == "\\"
          escaped = advance
          raise error("lex/unterminated-string", [], line, column) if escaped.nil?
          raise error("lex/invalid-escape-sequence", [escaped], line, column) unless ESCAPES.key?(escaped)

          value << ESCAPES[escaped]
        else
          value << char
        end
      end
    end

    # @return [Array(Symbol, Object)]
    def read_ident_or_keyword
      start = @position
      advance while !peek.nil? && (alphanumeric?(peek) || peek == "_")
      text = @chars[start...@position].join
      keyword = @vocabulary.keywords[text]
      case keyword
      when nil then [:ident, text]
      when :true then [:bool, true]
      when :false then [:bool, false]
      else [keyword, nil]
      end
    end

    # @return [Array(Symbol, nil)]
    def read_operator
      line = @line
      column = @column
      char = advance
      return [SINGLE_CHARACTER_KINDS[char], nil] if SINGLE_CHARACTER_KINDS.key?(char)

      if PAIRED_KINDS.key?(char)
        single, double = PAIRED_KINDS[char]
        return [double, nil] if peek == "=" && advance

        return [single, nil]
      end
      return (peek == "." && advance ? [:dot_dot, nil] : [:dot, nil]) if char == "."

      if char == "!"
        return [:not_eq, nil] if peek == "=" && advance

        raise LexError.new("unexpected character `!`", line, column)
      end
      raise error("lex/unexpected-character", [char], line, column)
    end

    # @param key [String] message catalog key
    # @param args [Array<String>]
    # @param line [Integer]
    # @param column [Integer]
    # @return [LexError]
    def error(key, args, line = @line, column = @column)
      LexError.new(@vocabulary.msg(key, args), line, column)
    end

    # @param char [String, nil]
    # @return [Boolean]
    def ascii_digit?(char)
      !char.nil? && char >= "0" && char <= "9"
    end

    # ponytail: treats every non-ASCII, non-whitespace character as a letter;
    # Rust uses the full Unicode alphabetic table, add it if a symbol such as
    # `§` ever needs to be rejected.
    #
    # @param char [String]
    # @return [Boolean]
    def alphabetic?(char)
      (char >= "a" && char <= "z") || (char >= "A" && char <= "Z") ||
        (char.ord > 127 && !WHITESPACE.include?(char))
    end

    # @param char [String]
    # @return [Boolean]
    def alphanumeric?(char)
      alphabetic?(char) || ascii_digit?(char)
    end
  end
end
