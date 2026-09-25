#!/usr/bin/env ruby
# Records what `bin/yara` prints for every example as its expected output:
# `test/stdout/` for each example and `test/golden/` for each error
# example's stderr. Run it after adding an example, then review the diff
# before committing. Run from anywhere: `ruby script/capture_output.rb`.
require "fileutils"
require "open3"
require_relative "../test/support/examples"

binary = File.expand_path("../bin/yara", __dir__)

Dir.chdir(Examples::ROOT) do
  Examples.all.each do |example|
    stdout, stderr, status = Open3.capture3(binary, *Examples.run_arguments(example))
    stderr_path = Examples.stderr_path(example)
    if stderr_path
      File.write(stderr_path, stderr)
    elsif !stderr.empty?
      abort("#{example}: wrote to stderr but is not in examples/errors/:\n#{stderr}")
    end

    path = Examples.stdout_path(example)
    FileUtils.mkdir_p(File.dirname(path))
    File.write(path, stdout)
    puts("#{path} (exit #{status.exitstatus})")
  end
end
