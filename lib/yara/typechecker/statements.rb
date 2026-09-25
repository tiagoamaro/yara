module Yara
  class TypeChecker
    private

    # @param statement [AST::Node]
    # @return [void]
    # @raise [TypeError]
    def check_statement(statement)
      case statement
      when AST::VarDecl, AST::ConstDecl then check_declaration(statement)
      when AST::FunctionDef then check_function(statement)
      when AST::Return then check_expr(statement.value) if statement.value
      when AST::If
        check_condition(statement.condition, "type/if-condition-must-be-boolean", statement)
        check_block(statement.then_body)
        statement.elsif_branches.each do |condition, body|
          check_condition(condition, "type/elsif-condition-must-be-boolean", condition)
          check_block(body)
        end
        check_block(statement.else_body) if statement.else_body
      when AST::While
        check_condition(statement.condition, "type/while-condition-must-be-boolean", statement)
        check_block(statement.body)
      when AST::For then check_for(statement)
      when AST::ExprStmt then check_expr(statement.expr)
      when AST::FieldAssign then check_field_assign(statement)
      end
    end

    # A declared annotation must accept the value's type and becomes the
    # variable's type, which is how `xs: IntArray = []` learns its element
    # type.
    #
    # @param statement [AST::VarDecl, AST::ConstDecl]
    # @return [void]
    def check_declaration(statement)
      value = check_expr(statement.value)
      annotation = statement.type_ann
      if annotation
        declared = resolve_type(annotation.name, annotation.line, annotation.column)
        unless declared.accepts?(value)
          type_error("type/var-decl-type-mismatch", [statement.name, name_of(declared), name_of(value)],
                     statement.line, statement.column)
        end
        value = declared
      end
      @environment.declare(statement.name, value)
    end

    # @param function [AST::FunctionDef]
    # @return [void]
    def check_function(function)
      @environment.push_scope
      function.params.each do |param|
        @environment.declare(param.name, resolve_type(param.type_ann.name, param.line, param.column))
      end
      check_return_type(function, "type/function-return-type-mismatch", [function.name])
      @environment.pop_scope
    end

    # Compares a body's tail type with its declared return type, skipped when
    # either is unknown.
    #
    # @param function [AST::FunctionDef]
    # @param key [String] message catalog key for a mismatch
    # @param names [Array<String>] the leading message arguments naming the function
    # @return [void]
    def check_return_type(function, key, names)
      annotation = function.return_type
      declared = annotation && resolve_type(annotation.name, annotation.line, annotation.column)
      actual = check_body_return_type(function.body)
      return if declared.nil? || actual.nil? || declared == actual

      type_error(key, names + [name_of(declared), name_of(actual)], function.line, function.column)
    end

    # @param condition [AST::Node]
    # @param key [String] message catalog key for a non-Boolean condition
    # @param position [AST::Node] where the error points
    # @return [void]
    def check_condition(condition, key, position)
      type = check_expr(condition)
      type_error(key, [name_of(type)], position.line, position.column) if type != Type::BOOLEAN
    end

    # @param statement [AST::For]
    # @return [void]
    def check_for(statement)
      bounds = [check_expr(statement.range_start), check_expr(statement.range_end)]
      if bounds.any? { |type| type != Type::INTEGER }
        raise TypeError.new("`for` range bounds must be Integer", statement.line, statement.column)
      end

      @environment.push_scope
      @environment.declare(statement.var_name, Type::INTEGER)
      check_block(statement.body)
      @environment.pop_scope
    end

    # @param statement [AST::FieldAssign]
    # @return [void]
    def check_field_assign(statement)
      field = check_field_access(statement.object, statement.field, statement.line, statement.column)
      value = check_expr(statement.value)
      return if field.accepts?(value)

      type_error("type/cannot-assign-field", [name_of(value), statement.field, name_of(field)],
                 statement.line, statement.column)
    end

    # @param body [Array<AST::Node>]
    # @return [void]
    def check_block(body)
      body.each { |statement| check_statement(statement) }
    end

    # The type of a body's last statement, its implicit return value.
    #
    # @param body [Array<AST::Node>]
    # @return [Type, nil] nil when the tail yields no known value
    def check_body_return_type(body)
      body[0...-1].each { |statement| check_statement(statement) }
      body.empty? ? nil : check_tail(body.last)
    end

    # A trailing `if` is a tail expression when it has an `else`: every
    # branch's own tail type must agree.
    #
    # @param statement [AST::Node]
    # @return [Type, nil]
    def check_tail(statement)
      case statement
      when AST::ExprStmt then check_expr(statement.expr)
      when AST::Return then statement.value ? check_expr(statement.value) : Type::NIL
      when AST::If
        check_condition(statement.condition, "type/if-condition-must-be-boolean", statement)
        result = check_body_return_type(statement.then_body)
        statement.elsif_branches.each do |condition, body|
          check_condition(condition, "type/elsif-condition-must-be-boolean", condition)
          result = combine_tail_types(result, check_body_return_type(body), statement)
        end
        statement.else_body && combine_tail_types(result, check_body_return_type(statement.else_body), statement)
      else
        check_statement(statement)
        nil
      end
    end

    # @param first [Type, nil]
    # @param second [Type, nil]
    # @param statement [AST::If] where a mismatch points
    # @return [Type, nil] nil when either branch yields no known value
    def combine_tail_types(first, second, statement)
      return nil if first.nil? || second.nil?
      return first if first == second

      type_error("type/branches-return-different-types", [name_of(first), name_of(second)],
                 statement.line, statement.column)
    end
  end
end
