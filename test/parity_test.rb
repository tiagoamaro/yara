require "minitest/autorun"
require "open3"
require_relative "support/examples"

# Runs every shared example through `bin/yara` and compares stdout, stderr and
# exit status with the committed expected output (`test/stdout/`,
# `test/golden/`). One test per example.
class ParityTest < Minitest::Test
  # `YARA_BINARY` swaps in another executable, such as the mruby build.
  BINARY = File.expand_path(ENV.fetch("YARA_BINARY", "bin/yara"), File.expand_path("..", __dir__))

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

    stdout, stderr, status = Open3.capture3(BINARY, *Examples.run_arguments(example), chdir: Examples::ROOT)

    assert_equal read_fixture(Examples.stdout_path(example)), stdout, "stdout of #{example}"
    assert_equal expected_stderr, stderr, "stderr of #{example}"
    assert_equal (stderr_path ? 1 : 0), status.exitstatus, "exit status of #{example}"
  end

  # @param path [String] repo-root-relative path
  # @return [String]
  def read_fixture(path)
    File.read(File.join(Examples::ROOT, path))
  end
end
