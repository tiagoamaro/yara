module Yara
  # A lexical scope stack shared by the typechecker (binding names to types)
  # and the interpreter (binding names to values), mirroring `rust/src/env.rs`.
  # The innermost scope is last and is searched first, so inner declarations
  # shadow outer ones.
  class Environment
    def initialize
      @scopes = [{}]
    end

    # Opens a scope for a function body or `for` loop; pair with `pop_scope`.
    #
    # @return [void]
    def push_scope
      @scopes << {}
    end

    # @return [void]
    def pop_scope
      @scopes.pop
    end

    # Binds `name` in the innermost scope, shadowing any outer binding.
    #
    # @param name [String]
    # @param value [Object]
    # @return [void]
    def declare(name, value)
      @scopes.last[name] = value
    end

    # @param name [String]
    # @return [Boolean] whether `name` is bound in any scope
    def bound?(name)
      @scopes.any? { |scope| scope.key?(name) }
    end

    # The innermost binding of `name`, or nil when unbound (use `bound?` when
    # a bound value can itself be nil).
    #
    # @param name [String]
    # @return [Object, nil]
    def lookup(name)
      scope = @scopes.reverse_each.find { |candidate| candidate.key?(name) }
      scope && scope[name]
    end

    # Assignment: mutates the nearest existing binding, or declares `name` in
    # the innermost scope when none exists. This is what makes `x = x + 1` in
    # a loop body update the outer `x` instead of shadowing it.
    #
    # @param name [String]
    # @param value [Object]
    # @return [void]
    def set_or_declare(name, value)
      scope = @scopes.reverse_each.find { |candidate| candidate.key?(name) } || @scopes.last
      scope[name] = value
    end

    # The innermost scope, for the interpreter's implicit-`self` copy-back.
    #
    # @return [Hash{String => Object}]
    def current
      @scopes.last
    end

    # Every bound value in every scope, the garbage collector's roots.
    #
    # @return [Array<Object>]
    def values
      @scopes.flat_map(&:values)
    end
  end
end
