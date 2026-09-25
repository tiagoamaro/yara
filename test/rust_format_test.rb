require "minitest/autorun"
require_relative "../lib/yara"

# Expected strings come from running the same values through Rust's `{}` and
# `{:?}`.
class RustFormatTest < Minitest::Test
  CASES = {
    5.0 => ["5", "5.0"],
    0.1 + 0.2 => ["0.30000000000000004", "0.30000000000000004"],
    1e15 => ["1000000000000000", "1000000000000000.0"],
    1e16 => ["10000000000000000", "1e16"],
    1.5e16 => ["15000000000000000", "1.5e16"],
    0.0001 => ["0.0001", "0.0001"],
    0.00001 => ["0.00001", "1e-5"],
    1.5e-5 => ["0.000015", "1.5e-5"],
    -0.0 => ["-0", "-0.0"],
    123_456.789 => ["123456.789", "123456.789"],
    -2.5 => ["-2.5", "-2.5"],
    Float::INFINITY => ["inf", "inf"],
    -Float::INFINITY => ["-inf", "-inf"]
  }.freeze

  def test_floats_match_rust
    CASES.each do |value, (display, debug)|
      assert_equal display, Yara::RustFormat.float_display(value), "display of #{value}"
      assert_equal debug, Yara::RustFormat.float_debug(value), "debug of #{value}"
    end
    assert_equal "NaN", Yara::RustFormat.float_display(Float::NAN)
  end

  def test_string_debug_matches_rust
    assert_equal "\"a\\\"b\\\\c\\td\\ne\\r\\0\\u{7}é\\u{7f}\"",
                 Yara::RustFormat.string_debug("a\"b\\c\td\ne\r\u0000\u0007é\u007f")
  end
end
