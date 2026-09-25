module Yara
  class TypeChecker
    private

    # @param expr [AST::Node]
    # @return [Type]
    # @raise [TypeError]
    def check_expr(expr)
      case expr
      when AST::IntLit then Type::INTEGER
      when AST::FloatLit then Type::FLOAT
      when AST::StringLit then Type::STRING
      when AST::BoolLit then Type::BOOLEAN
      when AST::NilLit then Type::NIL
      when AST::Ident
        @environment.lookup(expr.name) ||
          type_error("type/undefined-variable", [expr.name], expr.line, expr.column)
      when AST::Binary
        check_binary(expr.op, check_expr(expr.left), check_expr(expr.right), expr.line, expr.column)
      when AST::Unary
        type = check_expr(expr.expr)
        return type if type == Type::INTEGER || type == Type::FLOAT

        type_error("type/cannot-negate", [name_of(type)], expr.line, expr.column)
      when AST::Call then check_call(expr.callee, expr.args, expr.line, expr.column)
      when AST::ArrayLit then check_array_literal(expr)
      when AST::Index then check_index(expr)
      when AST::FieldAccess then check_field_access(expr.object, expr.field, expr.line, expr.column)
      when AST::MethodCall then check_method_call(expr.object, expr.method, expr.args, expr.line, expr.column)
      end
    end

    # An empty literal checks as `Array<Nil>`, which a declaration's
    # annotation then replaces; otherwise every element must share the first
    # one's type.
    #
    # @param expr [AST::ArrayLit]
    # @return [Type]
    def check_array_literal(expr)
      return Type.array(Type::NIL) if expr.elements.empty?

      first = check_expr(expr.elements.first)
      expr.elements.drop(1).each do |element|
        type = check_expr(element)
        next if type == first

        type_error("type/array-elements-must-share-type", [name_of(first), name_of(type)], expr.line, expr.column)
      end
      Type.array(first)
    end

    # @param expr [AST::Index]
    # @return [Type] the element type
    def check_index(expr)
      array = check_expr(expr.array)
      index = check_expr(expr.index)
      if index != Type::INTEGER
        type_error("type/array-index-must-be-integer", [name_of(index)], expr.line, expr.column)
      end
      return array.inner if array.kind == "Array"

      type_error("type/cannot-index-into", [name_of(array)], expr.line, expr.column)
    end

    # No implicit numeric coercion: arithmetic and ordering need both sides
    # the same numeric type, `String + String` concatenates, and `==`/`!=`
    # need matching types except a pointer compared with `nil`.
    #
    # @param op [Symbol]
    # @param left [Type]
    # @param right [Type]
    # @param line [Integer]
    # @param column [Integer]
    # @return [Type]
    def check_binary(op, left, right, line, column)
      numeric = left == right && (left == Type::INTEGER || left == Type::FLOAT)
      result =
        case op
        when :add, :sub, :mul, :div
          if op == :add && left == Type::STRING && right == Type::STRING
            Type::STRING
          elsif numeric
            left
          end
        when :lt, :gt, :lt_eq, :gt_eq
          Type::BOOLEAN if numeric
        when :eq, :not_eq
          pointer_nil = [left.kind, right.kind].sort == ["Nil", "Pointer"]
          Type::BOOLEAN if left == right || pointer_nil
        end
      result || type_error("type/cannot-apply-binop", [AST::OPERATOR_SYMBOLS[op], name_of(left), name_of(right)], line, column)
    end
  end
end
