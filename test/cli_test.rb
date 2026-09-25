require "minitest/autorun"
require "stringio"
require_relative "../lib/yara"

class CLITest < Minitest::Test
  def test_no_arguments_prints_usage_and_fails
    stderr = StringIO.new

    assert_equal 1, Yara::CLI.run([], stderr)
    assert_equal "usage: yara run <file> [--vocabulary <path>]\n", stderr.string
  end

  def test_run_without_a_file_prints_usage_and_fails
    stderr = StringIO.new

    assert_equal 1, Yara::CLI.run(["run"], stderr)
    assert_equal "#{Yara::CLI::USAGE}\n", stderr.string
  end
end
