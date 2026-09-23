# The shared example fixtures at the repo root, and how each one is run.
# Used by the parity test and by `script/capture_rust_stdout.rb`, so both
# pick the same examples and the same vocabulary.
module Examples
  ROOT = File.expand_path("../../..", __dir__)

  # Pipeline stages in order, spelled as the first word pair of a rendered
  # error (`type error: ...`).
  STAGES = ["lex error", "parse error", "import error", "type error", "runtime error"].freeze

  # Every example, as a repo-root-relative path.
  #
  # @return [Array<String>]
  def self.all
    Dir.chdir(ROOT) { Dir.glob("examples/**/*.yara").sort }
  end

  # The `yara run` arguments for an example. Portuguese-vocabulary examples
  # need `pt.vocab`, the same rule `rust/tests/run_examples.rs` uses.
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

  # Expected stdout, captured from the Rust binary.
  #
  # @param example [String] repo-root-relative path
  # @return [String] repo-root-relative path
  def self.stdout_path(example)
    "tests/stdout/#{example.delete_prefix("examples/").delete_suffix(".yara")}.stdout"
  end

  # Expected stderr; only error examples have one.
  #
  # @param example [String] repo-root-relative path
  # @return [String, nil] repo-root-relative path, or nil for a clean example
  def self.stderr_path(example)
    return nil unless example.start_with?("examples/errors/")

    "tests/golden/#{File.basename(example, ".yara")}.stderr"
  end
end
