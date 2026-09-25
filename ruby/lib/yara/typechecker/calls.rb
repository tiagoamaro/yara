module Yara
  class TypeChecker
    private

    # Registers every top-level function's signature before any body is
    # checked, so calls may precede definitions or recurse. A return type
    # error points at the `def`.
    #
    # @param program [Array<AST::Node>]
    # @return [void]
    def collect_function_signatures(program)
      program.grep(AST::FunctionDef).each do |function|
        @functions[function.name] = signature_of(function)
      end
    end

    # @param function [AST::FunctionDef]
    # @return [Signature]
    def signature_of(function)
      params = function.params.map { |param| resolve_type(param.type_ann.name, param.line, param.column) }
      returns = function.return_type && resolve_type(function.return_type.name, function.line, function.column)
      Signature.new(params, returns)
    end

    # A free call resolves to `print` (any arguments), a registry builtin, or
    # a user function, in that order.
    #
    # @param callee [String]
    # @param args [Array<AST::Node>]
    # @param line [Integer]
    # @param column [Integer]
    # @return [Type]
    def check_call(callee, args, line, column)
      canonical = @vocabulary.canonical_builtin(callee)
      if canonical == "print"
        args.each { |arg| check_expr(arg) }
        return Type::NIL
      end
      return check_builtin(canonical, callee, args, line, column) if Builtins::ARITIES.key?(canonical)

      signature = @functions[callee] || type_error("type/undefined-function", [callee], line, column)
      check_arguments(callee, signature, args, line, column, "type/function-arity-mismatch")
      signature.return_type || Type::NIL
    end

    # Arity and exact argument types, shared by function, method and `.new`
    # calls; `what` names the callee in errors (`add`, `Hello#greet`,
    # `Hello.new`).
    #
    # @param what [String]
    # @param signature [Signature]
    # @param args [Array<AST::Node>]
    # @param line [Integer]
    # @param column [Integer]
    # @param arity_key [String] message catalog key for an arity mismatch
    # @return [void]
    def check_arguments(what, signature, args, line, column, arity_key = "type/call-arity-mismatch")
      if args.size != signature.param_types.size
        type_error(arity_key, [what, signature.param_types.size.to_s, args.size.to_s], line, column)
      end
      args.zip(signature.param_types).each do |arg, expected|
        actual = check_expr(arg)
        next if expected.accepts?(actual)

        type_error("type/argument-type-mismatch", [what, name_of(expected), name_of(actual)], arg.line, arg.column)
      end
    end

    # @param canonical [String] the builtin's English name
    # @param callee [String] the name as written, for errors
    # @param args [Array<AST::Node>]
    # @param line [Integer]
    # @param column [Integer]
    # @return [Type]
    def check_builtin(canonical, callee, args, line, column)
      arity = Builtins::ARITIES[canonical]
      if args.size != arity
        type_error("type/builtin-arity-mismatch", [callee, arity.to_s, args.size.to_s], line, column)
      end
      send("check_builtin_#{canonical}", args, line, column)
    end

    # @return [Type]
    def check_builtin_len(args, _line, _column)
      array = check_expr(args[0])
      return Type::INTEGER if array.kind == "Array"

      type_error("type/len-expects-array", [name_of(array)], args[0].line, args[0].column)
    end

    # Unlike `Array#push`, the value must match the element type exactly.
    #
    # @return [Type]
    def check_builtin_push(args, line, column)
      array = check_expr(args[0])
      value = check_expr(args[1])
      type_error("type/push-expects-array", [name_of(array)], line, column) if array.kind != "Array"
      return Type::NIL if array.inner == value

      type_error("type/push-onto-mismatch", [name_of(array.inner), name_of(value)], line, column)
    end

    # @return [Type]
    def check_builtin_get(args, line, column)
      array = check_expr(args[0])
      index = check_expr(args[1])
      type_error("type/get-index-not-integer", [name_of(index)], line, column) if index != Type::INTEGER
      return array.inner if array.kind == "Array"

      type_error("type/get-expects-array", [name_of(array)], line, column)
    end

    # @return [Type]
    def check_builtin_set(args, line, column)
      array = check_expr(args[0])
      index = check_expr(args[1])
      value = check_expr(args[2])
      type_error("type/set-index-not-integer", [name_of(index)], line, column) if index != Type::INTEGER
      type_error("type/set-expects-array", [name_of(array)], line, column) if array.kind != "Array"
      return Type::NIL if array.inner == value

      type_error("type/set-onto-mismatch", [name_of(array.inner), name_of(value)], line, column)
    end

    # @return [Type]
    def check_builtin_pop(args, _line, _column)
      array = check_expr(args[0])
      return array.inner if array.kind == "Array"

      type_error("type/pop-expects-array", [name_of(array)], args[0].line, args[0].column)
    end

    # @return [Type]
    def check_builtin_alloc(args, _line, _column)
      Type.pointer(check_expr(args[0]))
    end

    # @return [Type]
    def check_builtin_deref(args, line, column)
      pointer = check_expr(args[0])
      return pointer.inner if pointer.kind == "Pointer"

      type_error("type/deref-expects-pointer", [name_of(pointer)], line, column)
    end

    # @return [Type]
    def check_builtin_set_deref(args, line, column)
      pointer = check_expr(args[0])
      value = check_expr(args[1])
      type_error("type/set-deref-expects-pointer", [name_of(pointer)], line, column) if pointer.kind != "Pointer"
      return Type::NIL if pointer.inner.accepts?(value)

      type_error("type/set-deref-into-mismatch", [name_of(pointer.inner), name_of(value)], line, column)
    end

    # @return [Type]
    def check_builtin_free(args, line, column)
      pointer = check_expr(args[0])
      return Type::NIL if pointer.kind == "Pointer"

      type_error("type/free-expects-pointer", [name_of(pointer)], line, column)
    end

    # The number of heap slots the collector freed.
    #
    # @return [Type]
    def check_builtin_collect(_args, _line, _column)
      Type::INTEGER
    end
  end
end
