module Yara
  # Command-line entry point, mirroring `rust/src/main.rs`: only
  # `yara run <file> [--vocabulary <path>]` is supported, with `--keywords`
  # as an older alias for `--vocabulary`.
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

      flag = argv[2..].index { |arg| arg == "--vocabulary" || arg == "--keywords" }
      run_file(argv[1], flag && argv[flag + 3], stderr)
    end

    # Runs the ported pipeline stages over one file, rendering the first
    # error rustc-style.
    #
    # @param path [String]
    # @param vocabulary_path [String, nil] a vocabulary file, else English
    # @param stderr [IO]
    # @return [Integer] exit status
    def self.run_file(path, vocabulary_path, stderr)
      begin
        source = File.read(path)
      rescue SystemCallError => e
        stderr.puts("error: cannot read `#{path}`: #{os_error(e)}")
        return 1
      end

      vocabulary = load_vocabulary(vocabulary_path, stderr)
      return 1 unless vocabulary

      begin
        tokens = Lexer.new(source, vocabulary).tokenize
        program = Parser.new(tokens, vocabulary).parse_program
      rescue Diagnostics::Error => e
        stderr.print(Diagnostics.render(e, path, source))
        return 1
      end

      map = Diagnostics::SourceMap.new(path, source)
      begin
        program = Resolver.resolve_imports(program, path, map, vocabulary)
        TypeChecker.new(vocabulary).check_program(program)
        Interpreter.new(vocabulary).run_program(program)
      rescue Diagnostics::Error => e
        stderr.print(Diagnostics.render_with_map(e, map, vocabulary))
        return 1
      end
      0
    end

    # Reads and parses the vocabulary file, rendering any error against the
    # file itself.
    #
    # @param path [String, nil]
    # @param stderr [IO]
    # @return [Vocabulary, nil] nil after reporting an error
    def self.load_vocabulary(path, stderr)
      return Vocabulary.english if path.nil?

      begin
        text = File.read(path)
      rescue SystemCallError => e
        stderr.puts("error: cannot read `#{path}`: #{os_error(e)}")
        return nil
      end
      Vocabulary.parse(text)
    rescue TranslationError => e
      stderr.print(Diagnostics.render(e, path, text))
      nil
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
