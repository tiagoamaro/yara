require "rake/testtask"

Rake::TestTask.new(:test) do |t|
  t.libs << "lib"
  t.pattern = "test/**/*_test.rb"
end

task default: :test

MRUBY_VERSION = "4.0.0"
MRUBY_DIR = File.expand_path("build/mruby-#{MRUBY_VERSION}", __dir__)
MRUBY_CONFIG = File.expand_path("build/mruby-#{MRUBY_VERSION}/build/host/bin/mruby-config", __dir__)

# The files `lib/yara.rb` requires, in its order, then the entry script.
SOURCES = File.read("lib/yara.rb").scan(/^require_relative "(.+)"$/).map { |(path)| "lib/#{path}.rb" } +
          ["mruby/main.rb"]

desc "Build the standalone mruby executable, build/yara"
task build: "build/yara"

file MRUBY_DIR do
  mkdir_p "build"
  sh "curl -sSL https://github.com/mruby/mruby/archive/refs/tags/#{MRUBY_VERSION}.tar.gz | tar xz -C build"
end

file MRUBY_CONFIG => [MRUBY_DIR, "mruby/build_config.rb"] do
  sh({ "MRUBY_CONFIG" => File.expand_path("mruby/build_config.rb", __dir__) }, "rake", "-C", MRUBY_DIR, "all")
end

file "build/yara" => [MRUBY_CONFIG, "mruby/yara.c", *SOURCES] do
  sh "#{MRUBY_DIR}/build/host/bin/mrbc", "-Byara_bytecode", "-o", "build/yara_bytecode.c", *SOURCES
  cflags = `#{MRUBY_CONFIG} --cflags`.split.join(" ")
  libs = `#{MRUBY_CONFIG} --ldflags --libs`.split.join(" ")
  sh "cc #{cflags} -o build/yara mruby/yara.c build/yara_bytecode.c #{libs}"
end
