module Yara
  # AST node definitions. Pure data: the parser builds these, the typechecker
  # and interpreter walk them.
  #
  # Every node is a `Struct` whose members end in `line, column` (1-indexed
  # position of the construct's first token). Binary operators are the symbols
  # in `BINARY_OPERATORS`; the only unary operator is `:neg`.
  module AST
    BINARY_OPERATORS = [:add, :sub, :mul, :div, :eq, :not_eq, :lt, :gt, :lt_eq, :gt_eq].freeze

    # Each binary operator's source spelling, for error messages.
    OPERATOR_SYMBOLS = {
      add: "+", sub: "-", mul: "*", div: "/", eq: "==", not_eq: "!=", lt: "<", gt: ">", lt_eq: "<=", gt_eq: ">="
    }.freeze

    # Behavior shared by every node.
    module Node
      # Adds `offset` to this node's line and every nested node's line,
      # leaving columns untouched. The resolver uses it to move an imported
      # file's AST into its own range of the virtual line space (see
      # `Diagnostics::SourceMap`).
      #
      # @param offset [Integer]
      # @return [void]
      def shift_lines(offset)
        self.line += offset if respond_to?(:line=)
        members.each { |member| Node.shift_value(self[member], offset) }
      end

      # Shifts a member value if it holds nodes: a node, or an array of nodes
      # (possibly nested, as in `If#elsif_branches`' `[condition, body]` pairs).
      #
      # @param value [Object]
      # @param offset [Integer]
      # @return [void]
      def self.shift_value(value, offset)
        if value.is_a?(Node)
          value.shift_lines(offset)
        elsif value.is_a?(Array)
          value.each { |element| shift_value(element, offset) }
        end
      end
    end

    # A canonical (alias-normalized) type name, e.g. `Integer` or a class name.
    class TypeAnnotation < Struct.new(:name, :line, :column)
      include Node
    end

    # A function or method parameter, `x: Integer`.
    class Param < Struct.new(:name, :type_ann, :line, :column)
      include Node
    end

    # A class instance-variable declaration with no value, `count: Integer`.
    class FieldDecl < Struct.new(:name, :type_ann, :line, :column)
      include Node
    end

    class IntLit < Struct.new(:value, :line, :column)
      include Node
    end

    class FloatLit < Struct.new(:value, :line, :column)
      include Node
    end

    # `value` is the unescaped string contents, without quotes.
    class StringLit < Struct.new(:value, :line, :column)
      include Node
    end

    class BoolLit < Struct.new(:value, :line, :column)
      include Node
    end

    class NilLit < Struct.new(:line, :column)
      include Node
    end

    class Ident < Struct.new(:name, :line, :column)
      include Node
    end

    # `op` is one of `BINARY_OPERATORS`.
    class Binary < Struct.new(:op, :left, :right, :line, :column)
      include Node
    end

    # A free function call; `callee` is a bare name, since Yara has no
    # first-class functions.
    class Call < Struct.new(:callee, :args, :line, :column)
      include Node
    end

    # `op` is `:neg`.
    class Unary < Struct.new(:op, :expr, :line, :column)
      include Node
    end

    class ArrayLit < Struct.new(:elements, :line, :column)
      include Node
    end

    class Index < Struct.new(:array, :index, :line, :column)
      include Node
    end

    class FieldAccess < Struct.new(:object, :field, :line, :column)
      include Node
    end

    # Also `ClassName.new(args)`: later stages tell construction apart, since
    # the parser has no class table.
    class MethodCall < Struct.new(:object, :method, :args, :line, :column)
      include Node
    end

    # `type_ann` is nil when the type is inferred.
    class VarDecl < Struct.new(:name, :type_ann, :value, :line, :column)
      include Node
    end

    class ConstDecl < Struct.new(:name, :type_ann, :value, :line, :column)
      include Node
    end

    # Also used for class methods, including `initializer`.
    class FunctionDef < Struct.new(:name, :params, :return_type, :body, :line, :column)
      include Node
    end

    # `value` is nil for a bare `return`.
    class Return < Struct.new(:value, :line, :column)
      include Node
    end

    # `elsif_branches` is an array of `[condition, body]` pairs; `else_body`
    # is nil when there is no `else`.
    class If < Struct.new(:condition, :then_body, :elsif_branches, :else_body, :line, :column)
      include Node
    end

    class While < Struct.new(:condition, :body, :line, :column)
      include Node
    end

    class For < Struct.new(:var_name, :range_start, :range_end, :body, :line, :column)
      include Node
    end

    # An expression statement; its position is the expression's.
    class ExprStmt < Struct.new(:expr)
      include Node

      # @return [Integer]
      def line
        expr.line
      end

      # @return [Integer]
      def column
        expr.column
      end
    end

    # Spliced away by the resolver before typechecking.
    class Import < Struct.new(:path, :line, :column)
      include Node
    end

    # `parent` is nil without `< Parent`; `consts` holds only `ConstDecl`s and
    # `methods` only `FunctionDef`s.
    class ClassDef < Struct.new(:name, :parent, :consts, :fields, :methods, :line, :column)
      include Node
    end

    class FieldAssign < Struct.new(:object, :field, :value, :line, :column)
      include Node
    end
  end
end
