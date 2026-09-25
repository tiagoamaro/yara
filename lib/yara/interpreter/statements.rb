module Yara
  class Interpreter
    private

    # @param statement [AST::Node]
    # @return [Return, nil] a `Return` when the statement returned
    # @raise [RuntimeError]
    def exec_statement(statement)
      case statement
      when AST::VarDecl, AST::ConstDecl
        @environment.set_or_declare(statement.name, eval_expr(statement.value))
        nil
      when AST::Return then Return.new(statement.value && eval_expr(statement.value))
      when AST::If
        body = chosen_branch(statement)
        body && exec_block(body)
      when AST::While
        while eval_bool(statement.condition)
          flow = exec_block(statement.body)
          return flow if flow
        end
        nil
      when AST::For then exec_for(statement)
      when AST::ExprStmt
        eval_expr(statement.expr)
        nil
      when AST::FieldAssign then exec_field_assign(statement)
      end
    end

    # The body of the first `if`/`elsif` branch whose condition holds, else
    # the `else` body (nil when there is none).
    #
    # @param statement [AST::If]
    # @return [Array<AST::Node>, nil]
    def chosen_branch(statement)
      return statement.then_body if eval_bool(statement.condition)

      statement.elsif_branches.each { |condition, body| return body if eval_bool(condition) }
      statement.else_body
    end

    # @param statement [AST::For]
    # @return [Return, nil]
    def exec_for(statement)
      first = eval_int(statement.range_start, statement.line, statement.column)
      last = eval_int(statement.range_end, statement.line, statement.column)
      @environment.push_scope
      begin
        (first...last).each do |index|
          @environment.declare(statement.var_name, index)
          flow = exec_block(statement.body)
          return flow if flow
        end
        nil
      ensure
        @environment.pop_scope
      end
    end

    # @param statement [AST::FieldAssign]
    # @return [nil]
    def exec_field_assign(statement)
      object = eval_expr(statement.object)
      value = eval_expr(statement.value)
      unless object.is_a?(Instance)
        runtime_error_msg("runtime/field-assign-on-non-object", [statement.field], statement.line, statement.column)
      end
      object.fields[statement.field] = value
      nil
    end

    # @param body [Array<AST::Node>]
    # @return [Return, nil]
    def exec_block(body)
      body.each do |statement|
        flow = exec_statement(statement)
        return flow if flow
      end
      nil
    end

    # Runs a function body; the last statement is its implicit return value.
    #
    # @param body [Array<AST::Node>]
    # @return [Object]
    def exec_function_body(body)
      body[0...-1].each do |statement|
        flow = exec_statement(statement)
        return flow.value if flow
      end
      body.empty? ? nil : exec_tail(body.last)
    end

    # @param statement [AST::Node]
    # @return [Object]
    def exec_tail(statement)
      case statement
      when AST::ExprStmt then eval_expr(statement.expr)
      when AST::If
        body = chosen_branch(statement)
        body && exec_function_body(body)
      else exec_statement(statement)&.value
      end
    end
  end
end
