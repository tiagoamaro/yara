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
    def initialize(keywords, types, messages)
      @keywords = keywords
      @types = types
      @messages = messages
    end

    # The canonical English spelling of a possibly localized type name; an
    # unknown name (a class, or a typo) passes through unchanged.
    #
    # @param name [String]
    # @return [String]
    def canonical_type(name)
      @types.fetch(name, name)
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
  end
end
