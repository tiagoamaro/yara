module Yara
  class Interpreter
    private

    # @param expr [AST::Node]
    # @return [Object] the value
    # @raise [RuntimeError]
    def eval_expr(expr)
      case expr
      when AST::IntLit, AST::FloatLit, AST::StringLit, AST::BoolLit then expr.value
      when AST::NilLit then nil
      when AST::Ident
        return @environment.lookup(expr.name) if @environment.bound?(expr.name)

        runtime_error_msg("runtime/undefined-variable", [expr.name], expr.line, expr.column)
      when AST::Binary
        left = eval_expr(expr.left)
        eval_binary(expr.op, left, eval_expr(expr.right), expr.line, expr.column)
      when AST::Unary then eval_negation(expr)
      when AST::Call then call_function(expr.callee, expr.args, expr.line, expr.column)
      when AST::ArrayLit then expr.elements.map { |element| eval_expr(element) }
      when AST::Index
        array = eval_expr(expr.array)
        array_get(array, eval_int(expr.index, expr.line, expr.column), expr.line, expr.column)
      when AST::FieldAccess then eval_field_access(expr)
      when AST::MethodCall
        object = expr.object
        if object.is_a?(AST::Ident) && !@environment.bound?(object.name) && @classes.key?(object.name)
          return construct(object.name, expr.args, expr.line, expr.column)
        end

        call_method(eval_expr(object), expr.method, expr.args, expr.line, expr.column)
      end
    end

    # @param expr [AST::Node]
    # @return [Boolean]
    def eval_bool(expr)
      value = eval_expr(expr)
      return value if value == true || value == false

      runtime_error_msg("runtime/expected-boolean-condition", [display(value)], expr.line, expr.column)
    end

    # @param expr [AST::Node]
    # @param line [Integer]
    # @param column [Integer]
    # @return [Integer]
    def eval_int(expr, line, column)
      value = eval_expr(expr)
      return value if value.is_a?(Integer)

      runtime_error_msg("runtime/expected-integer", [display(value)], line, column)
    end

    # @param expr [AST::Unary]
    # @return [Integer, Float]
    def eval_negation(expr)
      value = eval_expr(expr.expr)
      return checked_integer(-value, "-", expr.line, expr.column) if value.is_a?(Integer)
      return -value if value.is_a?(Float)

      runtime_error_msg("runtime/cannot-negate", [display(value)], expr.line, expr.column)
    end

    # @param expr [AST::FieldAccess]
    # @return [Object]
    def eval_field_access(expr)
      object = eval_expr(expr.object)
      unless object.is_a?(Instance)
        runtime_error_msg("runtime/field-on-non-object", [expr.field], expr.line, expr.column)
      end
      return object.fields[expr.field] if object.fields.key?(expr.field)

      runtime_error_msg("runtime/class-has-no-field", [object.class_name, expr.field], expr.line, expr.column)
    end

    # Shared by `xs[i]`, `get` and `Array#get`.
    #
    # @param array [Object]
    # @param index [Integer]
    # @param line [Integer]
    # @param column [Integer]
    # @return [Object]
    def array_get(array, index, line, column)
      runtime_error_msg("runtime/cannot-index-into", [display(array)], line, column) unless array.is_a?(Array)
      check_bounds(array, index, line, column)
      array[index]
    end

    # @param array [Array]
    # @param index [Integer]
    # @param line [Integer]
    # @param column [Integer]
    # @return [void]
    def check_bounds(array, index, line, column)
      return if index >= 0 && index < array.size

      runtime_error_msg("runtime/array-index-out-of-bounds", [index.to_s, array.size.to_s], line, column)
    end

    # @param op [Symbol]
    # @param left [Object]
    # @param right [Object]
    # @param line [Integer]
    # @param column [Integer]
    # @return [Object]
    def eval_binary(op, left, right, line, column)
      case op
      when :eq then left == right
      when :not_eq then left != right
      when :lt, :gt, :lt_eq, :gt_eq then eval_comparison(op, left, right, line, column)
      else eval_arithmetic(op, left, right, line, column)
      end
    end

    ARITHMETIC_ERRORS = {
      add: "runtime/cannot-add", sub: "runtime/cannot-subtract",
      mul: "runtime/cannot-multiply", div: "runtime/cannot-divide"
    }.freeze

    # Integer division truncates toward zero, as in Rust.
    #
    # @param op [Symbol] `:add`, `:sub`, `:mul` or `:div`
    # @param left [Object]
    # @param right [Object]
    # @param line [Integer]
    # @param column [Integer]
    # @return [Object]
    def eval_arithmetic(op, left, right, line, column)
      integers = left.is_a?(Integer) && right.is_a?(Integer)
      if integers || (left.is_a?(Float) && right.is_a?(Float))
        runtime_error_msg("runtime/division-by-zero", [], line, column) if integers && op == :div && right.zero?
        result =
          case op
          when :add then left + right
          when :sub then left - right
          when :mul then left * right
          when :div then integers ? (left.abs / right.abs) * ((left < 0) == (right < 0) ? 1 : -1) : left / right
          end
        return integers ? checked_integer(result, AST::OPERATOR_SYMBOLS[op], line, column) : result
      end
      return left + right if op == :add && left.is_a?(String) && right.is_a?(String)

      # Rust's subtraction message names the right operand first.
      args = op == :sub ? [display(right), display(left)] : [display(left), display(right)]
      runtime_error_msg(ARITHMETIC_ERRORS[op], args, line, column)
    end

    # Ordering needs two Integers or two Floats, neither NaN.
    #
    # @param op [Symbol]
    # @param left [Object]
    # @param right [Object]
    # @param line [Integer]
    # @param column [Integer]
    # @return [Boolean]
    def eval_comparison(op, left, right, line, column)
      comparable = (left.is_a?(Integer) && right.is_a?(Integer)) ||
                   (left.is_a?(Float) && right.is_a?(Float) && !left.nan? && !right.nan?)
      runtime_error_msg("runtime/cannot-compare", [display(left), display(right)], line, column) unless comparable

      case op
      when :lt then left < right
      when :gt then left > right
      when :lt_eq then left <= right
      when :gt_eq then left >= right
      end
    end

    # @param value [Integer]
    # @param operation [String] named in the overflow error
    # @param line [Integer]
    # @param column [Integer]
    # @return [Integer] `value`, when it fits in an i64
    def checked_integer(value, operation, line, column)
      return value if value >= INTEGER_MIN && value <= INTEGER_MAX

      runtime_error("integer overflow in `#{operation}`", line, column)
    end
  end
end
