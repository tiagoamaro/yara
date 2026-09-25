module Yara
  # Methods on primitive receivers, keyed by receiver kind and name. The order
  # matters: an unknown method's error lists the available ones in it.
  module Methods
    ARITIES = {
      [:array, "size"] => 0, [:array, "push"] => 1, [:array, "get"] => 1,
      [:array, "set"] => 2, [:array, "pop"] => 0, [:array, "is_empty"] => 0,
      [:string, "size"] => 0, [:string, "upper"] => 0, [:string, "lower"] => 0,
      [:string, "trim"] => 0, [:string, "is_empty"] => 0, [:string, "to_i"] => 0,
      [:string, "to_f"] => 0, [:string, "to_s"] => 0,
      [:integer, "to_s"] => 0, [:integer, "to_f"] => 0, [:integer, "abs"] => 0,
      [:float, "to_s"] => 0, [:float, "to_i"] => 0, [:float, "abs"] => 0,
      [:boolean, "to_s"] => 0,
      [:pointer, "deref"] => 0, [:pointer, "set_deref"] => 1, [:pointer, "free"] => 0
    }.freeze

    # @param kind [Symbol]
    # @return [Array<String>] the method names `kind` receivers have
    def self.names_for(kind)
      ARITIES.keys.select { |receiver, _| receiver == kind }.map(&:last)
    end
  end
end
