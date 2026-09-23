#!/usr/bin/env ruby
# Regenerates `tests/stdout/` by running the frozen Rust binary over every
# example. Run from anywhere: `ruby ruby/script/capture_rust_stdout.rb`.
require "fileutils"
require "open3"
require_relative "../test/support/examples"

Dir.chdir(Examples::ROOT) do
  system("cargo", "build", "--release", "--quiet", "--manifest-path", "rust/Cargo.toml", exception: true)

  Examples.all.each do |example|
    stdout, stderr, status = Open3.capture3("rust/target/release/yara", *Examples.run_arguments(example))
    expected_stderr = Examples.stderr_path(example)
    expected_stderr = expected_stderr ? File.read(expected_stderr) : ""
    if stderr != expected_stderr
      abort("#{example}: Rust stderr does not match its golden; fix that before capturing")
    end

    path = Examples.stdout_path(example)
    FileUtils.mkdir_p(File.dirname(path))
    File.write(path, stdout)
    puts("#{path} (exit #{status.exitstatus})")
  end
end
