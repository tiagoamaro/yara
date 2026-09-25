module Yara
  # Raised at the importing `import` statement's position, never at one inside
  # the imported file.
  class ResolveError < Diagnostics::Error
    # @return [String]
    def kind
      "import error"
    end
  end

  # Splices `import "path"` statements: each import is replaced in place by the
  # imported file's own (recursively resolved) statements, shifted into that
  # file's virtual line range.
  module Resolver
    # @param program [Array<AST::Node>] the entry file's parsed statements
    # @param current_file [String] the entry file's path, as given on the command line
    # @param map [Diagnostics::SourceMap] seeded with the entry file; every imported file is registered in it
    # @param vocabulary [Vocabulary]
    # @return [Array<AST::Node>]
    # @raise [ResolveError]
    def self.resolve_imports(program, current_file, map, vocabulary)
      visited = []
      begin
        visited << File.realpath(current_file)
      rescue SystemCallError
        nil
      end
      resolve(program, current_file, visited, map, vocabulary)
    end

    # Recursive worker behind `resolve_imports`. `visited` holds every
    # canonical path read so far across the whole run, so importing any file
    # twice is reported as a cycle.
    #
    # @param program [Array<AST::Node>]
    # @param current_file [String]
    # @param visited [Array<String>]
    # @param map [Diagnostics::SourceMap]
    # @param vocabulary [Vocabulary]
    # @return [Array<AST::Node>]
    # @raise [ResolveError]
    def self.resolve(program, current_file, visited, map, vocabulary)
      resolved = []
      program.each do |statement|
        unless statement.is_a?(AST::Import)
          resolved << statement
          next
        end

        fail_with = lambda do |key, args|
          raise ResolveError.new(vocabulary.msg(key, args), statement.line, statement.column)
        end
        target = import_path(current_file, statement.path)
        begin
          canonical = File.realpath(target)
        rescue SystemCallError => e
          fail_with.call("resolve/cannot-resolve-import", [statement.path, CLI.os_error(e)])
        end
        fail_with.call("resolve/import-cycle-at", [target]) if visited.include?(canonical)
        visited << canonical

        begin
          source = File.read(target)
        rescue SystemCallError => e
          fail_with.call("resolve/cannot-read-imported-file", [target, CLI.os_error(e)])
        end
        begin
          tokens = Lexer.new(source, vocabulary).tokenize
        rescue LexError => e
          fail_with.call("resolve/lex-error-in", [target, "#{e.line}:#{e.column}: #{e.message}"])
        end
        begin
          imported = Parser.new(tokens, vocabulary).parse_program
        rescue ParseError => e
          fail_with.call("resolve/parse-error-in", [target, "#{e.line}:#{e.column}: #{e.message}"])
        end

        offset = map.add_file(target, source)
        imported.each { |node| node.shift_lines(offset) }
        resolved.concat(resolve(imported, target, visited, map, vocabulary))
      end
      resolved
    end

    # The file an `import` names: `.yara` appended when the path has no
    # extension, joined onto the importing file's directory. A bare file name
    # has an empty parent in Rust, so the import path stays unprefixed rather
    # than gaining Ruby's `./`; an absolute import path replaces the directory,
    # as `Path::join` does.
    #
    # @param current_file [String]
    # @param path [String]
    # @return [String]
    def self.import_path(current_file, path)
      path += ".yara" if File.extname(path).empty?
      return path if path.start_with?("/") || !current_file.include?("/")

      File.join(File.dirname(current_file), path)
    end
  end
end
