require "minitest/autorun"
require_relative "../lib/yara"

# Ported from the tests in `rust/src/translations/messages.rs` and `rust/src/types.rs`.
class MessagesTest < Minitest::Test
  def test_substitutes_positional_placeholders
    assert_equal "expected `)`, found `if`", Yara::Messages.substitute("expected {0}, found {1}", ["`)`", "`if`"])
  end

  def test_leaves_template_without_placeholders_untouched
    assert_equal "division by zero", Yara::Messages.substitute("division by zero", [])
  end

  def test_catalog_has_every_rust_key
    assert_equal 128, Yara::Messages::CATALOG.size
  end

  def test_type_aliases_map_to_canonical_and_others_pass_through
    assert_equal %w[Integer Boolean String Float MyClass],
                 %w[Int Bool Str Float MyClass].map { |name| Yara::Types.normalize_alias(name) }
  end
end
