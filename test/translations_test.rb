require "minitest/autorun"
require_relative "../lib/yara"
require_relative "support/examples"

class TranslationsTest < Minitest::Test
  # File text, and the error message fragment and line it must fail with.
  REJECTED = {
    "unknown keyword" => ["iff = se\n", "unknown keyword `iff`", 1],
    "duplicate keyword spelling" => ["if = pal\nwhile = pal\n", "`pal` is already used for `if`, cannot also mean `while`", 2],
    "malformed line" => ["this is not valid\n", "expected `canonical = localized`, found `this is not valid`", 1],
    "missing spelling" => ["if =\n", "`if` has no translated spelling after `=`", 1],
    "unknown type" => ["[types]\nBogus = Fake\n", "unknown name `Bogus`", 2],
    "unknown builtin" => ["[builtins]\nnope = naoexiste\n", "unknown name `nope`", 2],
    "unknown method" => ["[methods]\nnope = naoexiste\n", "unknown name `nope`", 2],
    "duplicate type spelling" => ["[types]\nInteger = X\nFloat = X\n", "`X` is already used for `Integer`, cannot also mean `Float`", 3],
    "English name reused" => ["[types]\nFloat = Integer\n", "`Integer` is already used for `Integer`, cannot also mean `Float`", 2],
    "unknown section" => ["[bogus]\nif = se\n", "unknown section `[bogus]`", 1],
    "unknown message key" => ["[messages]\nnope/nope = oops\n", "unknown message key `nope/nope`", 2]
  }.freeze

  REJECTED.each do |name, (text, message, line)|
    define_method("test_rejects_#{name.tr(" ", "_")}") do
      error = assert_raises(Yara::TranslationError) { Yara::Vocabulary.parse(text) }

      assert_equal [message, line], [error.message, error.line]
    end
  end

  def test_translates_a_keyword_replacing_its_english_spelling
    keywords = Yara::Vocabulary.parse("# a comment\n\nif = se # trailing comment\n").keywords

    assert_equal [:if, nil, :end], [keywords["se"], keywords["if"], keywords["end"]]
  end

  def test_parses_every_section
    vocabulary = Yara::Vocabulary.parse("[keywords]\nif = se\n[types]\nInteger = Inteiro\n[builtins]\nprint = escreva\n[methods]\nsize = tamanho\n")

    assert_equal [:if, "Integer", "print", "size"],
                 [vocabulary.keywords["se"], vocabulary.canonical_type("Inteiro"),
                  vocabulary.canonical_builtin("escreva"), vocabulary.canonical_method("tamanho")]
  end

  def test_bundled_portuguese_file_parses
    vocabulary = Yara::Vocabulary.parse(File.read(File.join(Examples::ROOT, "translations/pt.vocab")))

    assert_equal [:class, "verdadeiro", "Array<Inteiro>", "new"],
                 [vocabulary.keywords["classe"], vocabulary.bool_word(true),
                  vocabulary.type_name(Yara::Type.array(Yara::Type::INTEGER)), vocabulary.canonical_method("novo")]
  end

  def test_message_override_falls_back_to_english
    vocabulary = Yara::Vocabulary.parse("[messages]\nruntime/division-by-zero = divisao por zero\n")

    assert_equal ["divisao por zero", "undefined variable `x`"],
                 [vocabulary.msg("runtime/division-by-zero", []), vocabulary.msg("type/undefined-variable", ["x"])]
  end

  def test_english_round_trips_every_name
    vocabulary = Yara::Vocabulary.english
    names = Yara::Vocabulary.canonical_names

    assert_equal names["types"], names["types"].map { |name| vocabulary.canonical_type(name) }
    assert_equal names["builtins"], names["builtins"].map { |name| vocabulary.canonical_builtin(name) }
    assert_equal names["methods"], names["methods"].map { |name| vocabulary.canonical_method(name) }
  end
end
