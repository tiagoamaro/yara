require "minitest/autorun"
require_relative "../lib/yara"

# Ported from the tests in `rust/src/diagnostics/mod.rs`.
class DiagnosticsTest < Minitest::Test
  include Yara::Diagnostics

  class TypeError < Yara::Diagnostics::Error
    def kind
      "type error"
    end
  end

  class TracedError < TypeError
    def frames
      [Yara::Diagnostics::Frame.new("helper", Yara::Diagnostics::Span.new(1, 1))]
    end
  end

  # Stands in for a translated vocabulary until the real one is ported.
  class FakeVocabulary
    def initialize(messages)
      @messages = messages
    end

    def msg(key, _args)
      @messages.fetch(key)
    end
  end

  def test_snippet_points_caret_at_column
    assert_equal "  |\n2 | y = x @ 2\n  |       ^\n", Yara::Diagnostics.render_snippet("x = 5\ny = x @ 2\n", 2, 7)
  end

  def test_snippet_out_of_range_line_is_empty
    assert_equal "", Yara::Diagnostics.render_snippet("only one line\n", 5, 1)
  end

  def test_snippet_gutter_width_matches_line_number_digits
    snippet = Yara::Diagnostics.render_snippet("#{"\n" * 9}tenth line", 10, 3)

    assert snippet.start_with?("   |\n10 | tenth line\n   |   ^\n")
  end

  def test_snippet_drops_carriage_return_like_rust_lines
    assert_equal "  |\n1 | a\n  | ^\n", Yara::Diagnostics.render_snippet("a\r\nb\r\n", 1, 1)
  end

  def test_render_uses_kind_message_and_span
    rendered = Yara::Diagnostics.render(TypeError.new("boom", 2, 7), "prog.yara", "x = 5\ny = x @ 2\n")

    assert_equal "type error: boom\n  --> prog.yara:2:7\n  |\n2 | y = x @ 2\n  |       ^\n", rendered
  end

  def test_source_map_assigns_disjoint_ranges_and_looks_them_up
    map = SourceMap.new("main.yara", "a\nb\nc\n")

    assert_equal 3, map.add_file("one.yara", "x\ny\n")
    assert_equal 5, map.add_file("two.yara", "z\n")
    assert_equal "main.yara", map.lookup(2).path
    assert_equal "one.yara", map.lookup(4).path
    assert_equal "one.yara", map.lookup(5).path
    assert_equal "two.yara", map.lookup(6).path
  end

  def test_source_map_empty_file_reserves_a_line
    map = SourceMap.new("main.yara", "a\n")

    assert_equal 1, map.add_file("empty.yara", "")
    assert_equal 2, map.add_file("next.yara", "x\n")
  end

  def test_render_with_map_uses_imported_file_for_its_range
    map = SourceMap.new("main.yara", "a\nb\n")
    map.add_file("helper.yara", "h1\nbad line\n")

    assert_equal "type error: boom\n  --> helper.yara:2:5\n  |\n2 | bad line\n  |     ^\n",
                 Yara::Diagnostics.render_with_map(TypeError.new("boom", 4, 5), map)
  end

  def test_render_with_vocabulary_localizes_kind_and_frame_words
    vocabulary = FakeVocabulary.new(
      "diag/type-error" => "erro de tipo", "diag/frame-in" => "em", "diag/frame-at" => "as"
    )
    rendered = Yara::Diagnostics.render_with_map(
      TracedError.new("boom", 1, 1), SourceMap.new("main.yara", "x = 1\n"), vocabulary
    )

    assert rendered.start_with?("erro de tipo: boom\n")
    assert_includes rendered, "  em `helper` as main.yara:1:1\n"
  end
end
