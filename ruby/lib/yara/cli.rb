module Yara
  # Command-line entry point, mirroring `rust/src/main.rs`: only
  # `yara run <file> [--vocabulary <path>]` is supported.
  module CLI
    USAGE = "usage: yara run <file> [--vocabulary <path>]"

    # Runs the command line and returns the process exit status.
    #
    # @param argv [Array<String>] arguments after the program name
    # @param stderr [IO] where usage and error output goes
    # @return [Integer] exit status
    def self.run(argv, stderr = $stderr)
      if argv[0] != "run" || argv[1].nil?
        stderr.puts(USAGE)
        return 1
      end

      run_file(argv[1], stderr)
    end

    # Runs the ported pipeline stages over one file, rendering the first
    # error rustc-style.
    #
    # @param path [String]
    # @param stderr [IO]
    # @return [Integer] exit status
    def self.run_file(path, stderr)
      begin
        source = File.read(path)
      rescue SystemCallError => e
        stderr.puts("error: cannot read `#{path}`: #{os_error(e)}")
        return 1
      end

      begin
        tokens = Lexer.new(source).tokenize
        Parser.new(tokens).parse_program
      rescue Diagnostics::Error => e
        stderr.print(Diagnostics.render(e, path, source))
        return 1
      end

      stderr.puts("yara: the Ruby pipeline stops after parsing for now")
      1
    end

    # A system error spelled the way Rust's `io::Error` prints it:
    # `No such file or directory (os error 2)`.
    #
    # @param error [SystemCallError]
    # @return [String]
    def self.os_error(error)
      description = error.message.split(" @ ").first.split(" - ").first
      "#{description} (os error #{error.errno})"
    end
  end
end
