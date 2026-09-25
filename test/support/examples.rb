# The shared example fixtures at the repo root, and how each one is run.
# Used by the parity test and by `script/capture_output.rb`, so both
# pick the same examples and the same vocabulary.
module Examples
  ROOT = File.expand_path("../..", __dir__)

  # Every example, as a repo-root-relative path.
  #
  # @return [Array<String>]
  def self.all
    Dir.chdir(ROOT) { Dir.glob("examples/**/*.yara").sort }
  end

  # The `yara run` arguments for an example. Portuguese-vocabulary examples
  # need `pt.vocab`.
  #
  # @param example [String] repo-root-relative path
  # @return [Array<String>]
  def self.run_arguments(example)
    arguments = ["run", example]
    if example.start_with?("examples/translations/") || example.end_with?("runtime_error_pt.yara")
      arguments += ["--vocabulary", "translations/pt.vocab"]
    end
    arguments
  end

  # Expected stdout.
  #
  # @param example [String] repo-root-relative path
  # @return [String] repo-root-relative path
  def self.stdout_path(example)
    "test/stdout/#{example.delete_prefix("examples/").delete_suffix(".yara")}.stdout"
  end

  # Expected stderr; only error examples have one.
  #
  # @param example [String] repo-root-relative path
  # @return [String, nil] repo-root-relative path, or nil for a clean example
  def self.stderr_path(example)
    return nil unless example.start_with?("examples/errors/")

    "test/golden/#{File.basename(example, ".yara")}.stderr"
  end
end
