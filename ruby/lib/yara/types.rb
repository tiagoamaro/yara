module Yara
  # Type-name helpers shared across stages, mirroring `rust/src/types.rs`.
  module Types
    ALIASES = { "Int" => "Integer", "Bool" => "Boolean", "Str" => "String" }.freeze

    # @param name [String]
    # @return [String] the canonical name for a short alias, else `name`
    def self.normalize_alias(name)
      ALIASES.fetch(name, name)
    end
  end
end
