require "minitest/autorun"
require "open3"
require_relative "support/examples"

# Runs every shared example through `bin/yara` and compares stdout, stderr and
# exit status with what the Rust implementation produces (`tests/stdout/`,
# `tests/golden/`). One test per example, so progress shows per example.
class ParityTest < Minitest::Test
  # Stages the Ruby pipeline implements so far, in `Examples::STAGES` order.
  # An example is only checked once every stage it reaches is ported.
  PORTED_STAGES = ["lex error", "parse error", "import error", "type error", "runtime error"].freeze

  BINARY = File.expand_path("../bin/yara", __dir__)

  Examples.all.each do |example|
    define_method("test_#{example.delete_prefix("examples/").delete_suffix(".yara")}") do
      assert_parity(example)
    end
  end

  private

  # @param example [String] repo-root-relative path
  def assert_parity(example)
    stderr_path = Examples.stderr_path(example)
    expected_stderr = stderr_path ? read_fixture(stderr_path) : ""
    skip("needs #{last_stage(expected_stderr)}, not ported yet") unless ported?(expected_stderr)
    skip("needs the vocabulary file parser (step 7)") if Examples.run_arguments(example).include?("--vocabulary")

    stdout, stderr, status = Open3.capture3(BINARY, *Examples.run_arguments(example), chdir: Examples::ROOT)

    assert_equal read_fixture(Examples.stdout_path(example)), stdout, "stdout of #{example}"
    assert_equal expected_stderr, stderr, "stderr of #{example}"
    assert_equal (stderr_path ? 1 : 0), status.exitstatus, "exit status of #{example}"
  end

  # The last stage an example runs: the stage its error names, or the
  # interpreter for an example that runs clean.
  #
  # @param expected_stderr [String]
  # @return [String] one of `Examples::STAGES`
  def last_stage(expected_stderr)
    return Examples::STAGES.last if expected_stderr.empty?

    Examples::STAGES.find { |stage| expected_stderr.start_with?("#{stage}:") } ||
      flunk("unrecognized stage in #{expected_stderr.lines.first.inspect}")
  end

  # @param expected_stderr [String]
  # @return [Boolean]
  def ported?(expected_stderr)
    needed = Examples::STAGES.index(last_stage(expected_stderr)) + 1
    PORTED_STAGES.size >= needed
  end

  # @param path [String] repo-root-relative path
  # @return [String]
  def read_fixture(path)
    File.read(File.join(Examples::ROOT, path))
  end
end
