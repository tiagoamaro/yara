module Yara
  # Rendering shared by every stage's errors: a rustc-style header, the source
  # line, and a caret under the column, plus one such block per call-stack
  # frame.
  module Diagnostics
    # A 1-indexed position. The line is virtual once imports are spliced in;
    # `SourceMap#lookup` recovers the file and local line.
    Span = Struct.new(:line, :column)

    # One call-stack entry: the active function's name and its call site.
    Frame = Struct.new(:name, :span)

    # One file registered in a `SourceMap`.
    SourceFile = Struct.new(:start_line, :path, :source)

    # Base class for every stage's error. Subclasses define `kind` (e.g.
    # `"lex error"`); runtime errors also override `frames`.
    class Error < StandardError
      attr_reader :line, :column

      # @param message [String] without any position
      # @param line [Integer]
      # @param column [Integer]
      def initialize(message, line, column)
        super(message)
        @line = line
        @column = column
      end

      # @return [Span]
      def span
        Span.new(line, column)
      end

      # Call-stack frames, innermost first.
      #
      # @return [Array<Frame>]
      def frames
        []
      end
    end

    # Maps virtual lines back to a file and its local line. The entry file
    # occupies lines 1..N; each imported file is appended after everything
    # registered before it.
    class SourceMap
      # @param entry_path [String]
      # @param entry_source [String]
      def initialize(entry_path, entry_source)
        @files = [SourceFile.new(1, entry_path, entry_source)]
      end

      # Registers an imported file after every file so far and returns the
      # offset its AST lines must be shifted by. An empty file still reserves
      # one line, so ranges stay distinct.
      #
      # @param path [String]
      # @param source [String]
      # @return [Integer]
      def add_file(path, source)
        last = @files.last
        start_line = last.start_line + [Diagnostics.source_lines(last.source).size, 1].max
        @files << SourceFile.new(start_line, path, source)
        start_line - 1
      end

      # The last registered file whose range starts at or before `line`.
      #
      # @param line [Integer]
      # @return [SourceFile]
      def lookup(line)
        @files.reverse_each.find { |file| file.start_line <= line } || @files.first
      end
    end

    FRAME_WORD_KEYS = { "in" => "diag/frame-in", "at" => "diag/frame-at" }.freeze

    KIND_KEYS = {
      "lex error" => "diag/lex-error",
      "parse error" => "diag/parse-error",
      "type error" => "diag/type-error",
      "runtime error" => "diag/runtime-error",
      "import error" => "diag/import-error",
      "keyword translation error" => "diag/keyword-translation-error"
    }.freeze

    # Renders `error` against a single file, for stages that run before
    # imports are spliced in.
    #
    # @param error [Error]
    # @param path [String]
    # @param source [String]
    # @return [String]
    def self.render(error, path, source)
      render_with_map(error, SourceMap.new(path, source))
    end

    # Renders `error`, resolving every position through `map`. With a
    # vocabulary, the stage label and the frame words `in`/`at` are looked up
    # in its message catalog.
    #
    # @param error [Error]
    # @param map [SourceMap]
    # @param vocabulary [#msg, nil]
    # @return [String]
    def self.render_with_map(error, map, vocabulary = nil)
      span = error.span
      file = map.lookup(span.line)
      local = span.line - file.start_line + 1
      out = "#{localize(error.kind, KIND_KEYS, vocabulary)}: #{error.message}\n"
      out += "  --> #{file.path}:#{local}:#{span.column}\n"
      out += render_snippet(file.source, local, span.column)
      in_word = localize("in", FRAME_WORD_KEYS, vocabulary)
      at_word = localize("at", FRAME_WORD_KEYS, vocabulary)
      error.frames.each do |frame|
        file = map.lookup(frame.span.line)
        local = frame.span.line - file.start_line + 1
        out += "  #{in_word} `#{frame.name}` #{at_word} #{file.path}:#{local}:#{frame.span.column}\n"
        out += render_snippet(file.source, local, frame.span.column)
      end
      out
    end

    # One source line with a caret under `column`, gutter-aligned. A line
    # past the end of the source renders as an empty string, so an
    # end-of-file error degrades gracefully.
    #
    # @param source [String]
    # @param line [Integer]
    # @param column [Integer]
    # @return [String]
    def self.render_snippet(source, line, column)
      text = source_lines(source)[[line - 1, 0].max]
      return "" if text.nil?

      gutter = line.to_s
      pad = " " * gutter.size
      caret_pad = " " * [column - 1, 0].max
      "#{pad} |\n#{gutter} | #{text}\n#{pad} | #{caret_pad}^\n"
    end

    # Splits source into lines the way Rust's `str::lines` does: no empty
    # last line for a trailing newline, and a trailing `\r` dropped from each
    # line. Snippets and `SourceMap` line counts must match Rust exactly.
    #
    # @param source [String]
    # @return [Array<String>]
    def self.source_lines(source)
      lines = source.split("\n", -1)
      lines.pop if lines.last == ""
      lines.map { |line| line.chomp("\r") }
    end

    # @param word [String] the English spelling
    # @param keys [Hash{String => String}] English spelling to catalog key
    # @param vocabulary [#msg, nil]
    # @return [String]
    def self.localize(word, keys, vocabulary)
      key = keys[word]
      return word if vocabulary.nil? || key.nil?

      vocabulary.msg(key, [])
    end
  end
end
