module Yara
  class TypeChecker
    # The primitive methods whose result type depends only on the receiver
    # kind; the others have a `check_<kind>_<name>` method below.
    FIXED_RESULTS = {
      [:array, "size"] => Type::INTEGER, [:array, "is_empty"] => Type::BOOLEAN,
      [:string, "size"] => Type::INTEGER, [:string, "upper"] => Type::STRING,
      [:string, "lower"] => Type::STRING, [:string, "trim"] => Type::STRING,
      [:string, "is_empty"] => Type::BOOLEAN, [:string, "to_i"] => Type::INTEGER,
      [:string, "to_f"] => Type::FLOAT, [:string, "to_s"] => Type::STRING,
      [:integer, "to_s"] => Type::STRING, [:integer, "to_f"] => Type::FLOAT, [:integer, "abs"] => Type::INTEGER,
      [:float, "to_s"] => Type::STRING, [:float, "to_i"] => Type::INTEGER, [:float, "abs"] => Type::FLOAT,
      [:boolean, "to_s"] => Type::STRING,
      [:pointer, "free"] => Type::NIL
    }.freeze

    private

    # @param receiver [Type] a type with a `receiver_kind`
    # @param method [String] as written, possibly localized
    # @param args [Array<AST::Node>]
    # @param line [Integer]
    # @param column [Integer]
    # @return [Type]
    def check_primitive_method(receiver, method, args, line, column)
      key = [receiver.receiver_kind, @vocabulary.canonical_method(method)]
      arity = Methods::ARITIES[key]
      unless arity
        available = @vocabulary.localized_method_names(Methods.names_for(key[0])).join(", ")
        type_error("type/no-method-available", [name_of(receiver), method, available], line, column)
      end
      if args.size != arity
        type_error("type/method-arity-mismatch", [name_of(receiver), method, arity.to_s, args.size.to_s], line, column)
      end
      FIXED_RESULTS[key] || send("check_#{key[0]}_#{key[1]}", receiver, args, line, column)
    end

    # @return [Type]
    def check_array_push(array, args, line, column)
      value = check_expr(args[0])
      return Type::NIL if array.inner.accepts?(value)

      type_error("type/array-push-mismatch", [name_of(array.inner), name_of(value)], line, column)
    end

    # @return [Type]
    def check_array_get(array, args, line, column)
      index = check_expr(args[0])
      type_error("type/array-get-index-not-integer", [name_of(index)], line, column) if index != Type::INTEGER
      array.inner
    end

    # @return [Type]
    def check_array_set(array, args, line, column)
      index = check_expr(args[0])
      value = check_expr(args[1])
      type_error("type/array-set-index-not-integer", [name_of(index)], line, column) if index != Type::INTEGER
      return Type::NIL if array.inner.accepts?(value)

      type_error("type/array-set-mismatch", [name_of(array.inner), name_of(value)], line, column)
    end

    # @return [Type]
    def check_array_pop(array, _args, _line, _column)
      array.inner
    end

    # @return [Type]
    def check_pointer_deref(pointer, _args, _line, _column)
      pointer.inner
    end

    # @return [Type]
    def check_pointer_set_deref(pointer, args, line, column)
      value = check_expr(args[0])
      return Type::NIL if pointer.inner.accepts?(value)

      type_error("type/ptr-set-deref-mismatch", [name_of(pointer.inner), name_of(value)], line, column)
    end
  end
end
