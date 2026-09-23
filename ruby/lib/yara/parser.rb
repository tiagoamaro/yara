module Yara
  class ParseError < Diagnostics::Error
    # @return [String]
    def kind
      "parse error"
    end
  end

  # Recursive-descent parser turning tokens into `AST` statements, mirroring
  # `rust/src/parser/`. Statements live in `parser/statements.rb`, the
  # expression precedence chain in `parser/expressions.rb`.
  class Parser
    # @param tokens [Array<Token>] ending in an `:eof` token
    # @param vocabulary [Vocabulary] type spellings and error messages
    def initialize(tokens, vocabulary = Vocabulary.english)
      @tokens = tokens
      @position = 0
      @vocabulary = vocabulary
    end

    # @return [Array<AST::Node>]
    # @raise [ParseError]
    def parse_program
      statements = []
      statements << parse_statement until check(:eof)
      statements
    end

    private

    # @return [Token]
    def peek
      @tokens[@position]
    end

    # @param kind [Symbol]
    # @return [Boolean]
    def check(kind)
      peek.kind == kind
    end

    # Consumes the current token; stays on the final `:eof` token.
    #
    # @return [Token]
    def advance
      token = peek
      @position += 1 if @position < @tokens.size - 1
      token
    end

    # @param kind [Symbol]
    # @param what [String] how the expected token reads in the error
    # @return [Token]
    def expect(kind, what)
      return advance if check(kind)

      raise error_at(peek, @vocabulary.msg("parse/expected-found", [what, Lexer.describe(peek)]))
    end

    # @return [Token] an `:ident` token
    def expect_ident
      return advance if check(:ident)

      raise error_at(peek, @vocabulary.msg("parse/expected-identifier-found", [Lexer.describe(peek)]))
    end

    # `Name`, an alias such as `Int` (normalized to `Integer`), a localized
    # spelling (normalized to English), or `Ptr<T>`, recorded as the single
    # name `"Ptr<T>"`.
    #
    # @return [AST::TypeAnnotation]
    def parse_type_annotation
      token = expect_ident
      name = @vocabulary.canonical_type(token.value)
      if name == "Ptr"
        expect(:lt, "`<`")
        inner = parse_type_annotation
        expect(:gt, "`>`")
        return AST::TypeAnnotation.new("Ptr<#{inner.name}>", token.line, token.column)
      end

      AST::TypeAnnotation.new(Types.normalize_alias(name), token.line, token.column)
    end

    # Statements up to (not including) one of `terminators`.
    #
    # @param terminators [Array<Symbol>]
    # @return [Array<AST::Node>]
    def parse_block(terminators)
      statements = []
      until terminators.include?(peek.kind)
        raise error_at(peek, "unexpected end of input, expected `end`") if check(:eof)

        statements << parse_statement
      end
      statements
    end

    # Items separated by commas, then `terminator`, which is consumed.
    #
    # @param terminator [Symbol]
    # @param terminator_description [String]
    # @yieldreturn [Object] one parsed item
    # @return [Array<Object>]
    def parse_comma_separated(terminator, terminator_description)
      items = []
      unless check(terminator)
        loop do
          items << yield
          break unless check(:comma)

          advance
        end
      end
      expect(terminator, terminator_description)
      items
    end

    # @param token [Token]
    # @param message [String]
    # @return [ParseError]
    def error_at(token, message)
      ParseError.new(message, token.line, token.column)
    end
  end
end
