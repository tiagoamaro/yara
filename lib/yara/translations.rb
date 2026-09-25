module Yara
  # A vocabulary file that fails to parse; it renders against the file itself.
  class TranslationError < Diagnostics::Error
    # @param message [String]
    # @param line [Integer]
    def initialize(message, line)
      super(message, line, 1)
    end

    # @return [String]
    def kind
      "keyword translation error"
    end
  end

  # The vocabulary a run uses: source spellings of keywords, types, builtins and
  # primitive methods, plus message-catalog overrides. Every name map starts
  # with English identity entries, which the duplicate-spelling check reads.
  class Vocabulary
    TYPE_NAMES = %w[
      Integer Float Boolean String Nil Int Bool Str IntArray FloatArray BoolArray StringArray Array Ptr
    ].freeze
    SECTIONS = %w[keywords types builtins methods messages].freeze

    # @return [Hash{String => Symbol}] source spelling to keyword token
    attr_reader :keywords

    # @return [Vocabulary] every name spelled in English, no message overrides
    def self.english
      name_maps = identity_maps(canonical_names)
      new(Lexer::KEYWORDS.dup, name_maps["types"], {}, name_maps["builtins"], name_maps["methods"])
    end

    # Parses a vocabulary file: `canonical = localized` lines under
    # `[keywords]` (the default), `[types]`, `[builtins]`, `[methods]` or
    # `[messages]` headers, with `#` comments. Translating a name replaces its
    # English spelling; a spelling may mean only one name per section.
    #
    # @param text [String]
    # @return [Vocabulary]
    # @raise [TranslationError]
    def self.parse(text)
      keywords = Lexer::KEYWORDS.dup
      names = canonical_names
      name_maps = identity_maps(names)
      messages = {}
      section = "keywords"
      Diagnostics.source_lines(text).each_with_index do |raw_line, index|
        number = index + 1
        line = raw_line.split("#", 2).first.to_s.strip
        next if line.empty?

        if line.start_with?("[") && line.end_with?("]")
          header = line[1...-1]
          section = header.strip
          raise TranslationError.new("unknown section `[#{header}]`", number) unless SECTIONS.include?(section)

          next
        end

        canonical, localized = line.split("=", 2)
        raise TranslationError.new("expected `canonical = localized`, found `#{line}`", number) if localized.nil?

        canonical = canonical.strip
        localized = localized.strip
        raise TranslationError.new("`#{canonical}` has no translated spelling after `=`", number) if localized.empty?

        case section
        when "keywords" then translate_keyword(keywords, canonical, localized, number)
        when "messages"
          raise TranslationError.new("unknown message key `#{canonical}`", number) unless Messages::CATALOG.key?(canonical)

          messages[canonical] = localized
        else translate_name(name_maps[section], names[section], canonical, localized, number)
        end
      end
      new(keywords, name_maps["types"], messages, name_maps["builtins"], name_maps["methods"])
    end

    # @return [Hash{String => Array<String>}] each name section's translatable names
    def self.canonical_names
      methods = Methods::ARITIES.keys.map(&:last).uniq + ["new"]
      { "types" => TYPE_NAMES, "builtins" => Builtins::ARITIES.keys + ["print"], "methods" => methods }
    end

    # @param names [Hash{String => Array<String>}] section to names
    # @return [Hash{String => Hash{String => String}}] section to each name mapped to itself
    def self.identity_maps(names)
      maps = {}
      names.each do |section, section_names|
        maps[section] = {}
        section_names.each { |name| maps[section][name] = name }
      end
      maps
    end

    # @param keywords [Hash{String => Symbol}] spelling to token, updated in place
    # @param canonical [String] the English keyword
    # @param localized [String]
    # @param number [Integer] the file line, for errors
    # @return [void]
    def self.translate_keyword(keywords, canonical, localized, number)
      token = Lexer::KEYWORDS[canonical] || raise(TranslationError.new("unknown keyword `#{canonical}`", number))
      owner = keywords[localized]
      if owner && owner != token
        message = "`#{localized}` is already used for `#{Lexer::KEYWORDS.key(owner)}`, cannot also mean `#{canonical}`"
        raise TranslationError.new(message, number)
      end
      keywords.delete(keywords.key(token))
      keywords[localized] = token
    end

    # @param map [Hash{String => String}] localized to canonical, updated in place
    # @param names [Array<String>] the section's translatable names
    # @param canonical [String]
    # @param localized [String]
    # @param number [Integer] the file line, for errors
    # @return [void]
    def self.translate_name(map, names, canonical, localized, number)
      raise TranslationError.new("unknown name `#{canonical}`", number) unless names.include?(canonical)

      existing = map[localized]
      if existing && existing != canonical
        raise TranslationError.new("`#{localized}` is already used for `#{existing}`, cannot also mean `#{canonical}`", number)
      end
      map.delete(map.key(canonical))
      map[localized] = canonical
    end

    # @param keywords [Hash{String => Symbol}]
    # @param types [Hash{String => String}] localized type name to canonical
    # @param messages [Hash{String => String}] catalog key to localized template
    # @param builtins [Hash{String => String}] localized builtin name to canonical
    # @param methods [Hash{String => String}] localized primitive-method name to canonical
    def initialize(keywords, types, messages, builtins = {}, methods = {})
      @keywords = keywords
      @types = types
      @messages = messages
      @builtins = builtins
      @methods = methods
    end

    # The canonical English spelling of a possibly localized type name; an
    # unknown name (a class, or a typo) passes through unchanged.
    #
    # @param name [String]
    # @return [String]
    def canonical_type(name)
      @types.fetch(name, name)
    end

    # @param name [String] a possibly localized builtin name
    # @return [String] its canonical English spelling, else `name`
    def canonical_builtin(name)
      @builtins.fetch(name, name)
    end

    # @param name [String] a possibly localized primitive-method name
    # @return [String] its canonical English spelling, else `name`
    def canonical_method(name)
      @methods.fetch(name, name)
    end

    # A type spelled in this vocabulary, for error messages (`Array<Integer>`
    # in English). Class names are user identifiers and never translate.
    #
    # @param type [Type]
    # @return [String]
    def type_name(type)
      case type.kind
      when "Array" then "#{localized(@types, "Array")}<#{type_name(type.inner)}>"
      when "Pointer" then "#{localized(@types, "Ptr")}<#{type_name(type.inner)}>"
      when "Instance" then type.inner
      else localized(@types, type.kind)
      end
    end

    # How `Boolean#to_s` spells a boolean: the keyword the lexer reads for it.
    #
    # @param value [Boolean]
    # @return [String]
    def bool_word(value)
      @keywords.key(value ? :true : :false) || value.to_s
    end

    # @param names [Array<String>] canonical primitive-method names
    # @return [Array<String>] each spelled in this vocabulary
    def localized_method_names(names)
      names.map { |name| localized(@methods, name) }
    end

    # The message for catalog `key` with `args` substituted: the localized
    # template if this vocabulary has one, else the English catalog entry,
    # else the key itself.
    #
    # @param key [String]
    # @param args [Array<String>]
    # @return [String]
    def msg(key, args)
      template = @messages[key] || Messages::CATALOG[key] || key
      Messages.substitute(template, args)
    end

    private

    # The localized spelling of a canonical name, reading a localized-to-canonical
    # map backwards.
    #
    # @param map [Hash{String => String}]
    # @param canonical [String]
    # @return [String]
    def localized(map, canonical)
      map.key(canonical) || canonical
    end
  end
end
