module Yara
  # The vocabulary a run uses: source spellings of keywords (and, from step 7,
  # types, builtins and methods) plus message-catalog overrides. Mirrors
  # `rust/src/translations/mod.rs`; only the English default exists so far.
  class Vocabulary
    # @return [Hash{String => Symbol}] source spelling to keyword token
    attr_reader :keywords

    # @return [Vocabulary] every name spelled in English, no message overrides
    def self.english
      new(Lexer::KEYWORDS.dup, {}, {})
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
