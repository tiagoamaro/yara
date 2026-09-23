require "minitest/autorun"
require_relative "../lib/yara"

# Ported from the tests in `rust/src/lexer/mod.rs`.
class LexerTest < Minitest::Test
  def tokens(source, vocabulary = Yara::Vocabulary.english)
    Yara::Lexer.new(source, vocabulary).tokenize
  end

  def described(source)
    tokens(source).map { |token| Yara::Lexer.describe(token) }
  end

  def lex_error(source)
    assert_raises(Yara::LexError) { tokens(source) }
  end

  def test_tokenizes_function_def
    assert_equal %w[Def Ident("add") LParen Ident("a") Colon Ident("Int") Comma Ident("b") Colon
                    Ident("Int") RParen Colon Ident("Int") Ident("a") Plus Ident("b") End Eof],
                 described("def add(a: Int, b: Int): Int\n  a + b\nend")
  end

  def test_tracks_line_and_column
    first, second = tokens("x\ny")

    assert_equal [1, 1, 2, 1], [first.line, first.column, second.line, second.column]
  end

  def test_reads_literals
    assert_equal [:int, 5], tokens("5").first.to_a[0, 2]
    assert_equal [:float, 5.5], tokens("5.5").first.to_a[0, 2]
    assert_equal [:bool, true], tokens("true").first.to_a[0, 2]
    assert_equal [:str, "hi"], tokens("\"hi\"").first.to_a[0, 2]
  end

  def test_reads_string_escapes
    assert_equal "a\n\t\"\\", tokens("\"a\\n\\t\\\"\\\\\"").first.value
  end

  def test_skips_comments
    assert_equal %w[Ident("x") Ident("y") Eof], described("x # a comment\ny")
  end

  def test_range_operator
    assert_equal %w[Int(0) DotDot Int(10) Eof], described("0..10")
  end

  def test_two_character_operators
    assert_equal %w[EqEq NotEq LtEq GtEq ColonEq Eq Lt Gt Colon Dot Eof], described("== != <= >= := = < > : .")
  end

  def test_brackets
    assert_equal %w[LBracket Int(1) Comma Int(2) RBracket Eof], described("[1, 2]")
  end

  def test_class_keyword_and_dot
    assert_equal %w[Class Ident("Foo") End Ident("x") Dot Ident("field") Eof], described("class Foo\nend\nx.field")
  end

  def test_non_ascii_identifiers_count_columns_in_characters
    identifier, equals = tokens("coração = 1")

    assert_equal ["coração", 9], [identifier.value, equals.column]
  end

  def test_unterminated_string_reports_its_start
    error = lex_error("x = \"abc")

    assert_equal ["unterminated string literal", 1, 5], [error.message, error.line, error.column]
  end

  def test_invalid_escape_sequence
    assert_equal "invalid escape sequence `\\q`", lex_error("\"a\\q\"").message
  end

  def test_integer_literal_beyond_i64_reports_its_start
    error = lex_error("x = 9223372036854775808")

    assert_equal ["invalid integer literal `9223372036854775808`", 5], [error.message, error.column]
  end

  def test_unexpected_characters
    assert_equal "unexpected character `@`", lex_error("x @ 3").message
    assert_equal "unexpected character `!`", lex_error("!x").message
  end

  def test_translated_keyword_spellings
    keywords = Yara::Lexer::KEYWORDS.dup
    keywords.delete("if")
    keywords["se"] = :if
    vocabulary = Yara::Vocabulary.new(keywords, {}, {})

    assert_equal [:if, :ident, :eof], tokens("se if", vocabulary).map(&:kind)
    assert_equal :if, tokens("if").first.kind
  end

  def test_localized_vocabulary_translates_lex_errors
    vocabulary = Yara::Vocabulary.new(Yara::Lexer::KEYWORDS, {}, { "lex/unterminated-string" => "string nao terminada" })
    error = assert_raises(Yara::LexError) { tokens("\"abc", vocabulary) }

    assert_equal "string nao terminada", error.message
  end

  def test_describe_matches_rust_token_display
    assert_equal ["Float(5.0)", "Str(\"a\\\"b\")", "Bool(false)"],
                 tokens("5.0 \"a\\\"b\" false")[0, 3].map { |token| Yara::Lexer.describe(token) }
  end
end
