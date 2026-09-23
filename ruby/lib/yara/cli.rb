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

      stderr.puts("yara: the Ruby pipeline is not implemented yet")
      1
    end
  end
end
