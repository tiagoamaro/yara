module Yara
  class Parser
    private

    # @return [AST::Node]
    def parse_statement
      case peek.kind
      when :def then parse_function_def
      when :const then parse_const_decl
      when :class then parse_class
      when :import then parse_import
      when :return then parse_return
      when :if then parse_if
      when :while then parse_while
      when :for then parse_for
      when :ident then parse_ident_statement
      else AST::ExprStmt.new(parse_expression)
      end
    end

    # `x: Type = value`, `x = value`, `object.field = value`, or an
    # expression statement. Anything but a typed declaration is parsed as a
    # full expression first, then the `=` decides what it was.
    #
    # @return [AST::Node]
    def parse_ident_statement
      checkpoint = @position
      name = expect_ident
      if check(:colon)
        advance
        type_ann = parse_type_annotation
        expect(:eq, "`=`")
        return AST::VarDecl.new(name.value, type_ann, parse_expression, name.line, name.column)
      end

      @position = checkpoint
      expression = parse_expression
      return AST::ExprStmt.new(expression) unless check(:eq)

      advance
      value = parse_expression
      case expression
      when AST::Ident
        AST::VarDecl.new(expression.name, nil, value, expression.line, expression.column)
      when AST::FieldAccess
        AST::FieldAssign.new(expression.object, expression.field, value, expression.line, expression.column)
      else
        raise ParseError.new("invalid assignment target", expression.line, expression.column)
      end
    end

    # @return [AST::Import]
    def parse_import
      import_token = advance
      unless check(:str)
        raise error_at(peek, "expected string literal after `import`, found #{Lexer.describe(peek)}")
      end

      AST::Import.new(advance.value, import_token.line, import_token.column)
    end

    # @return [AST::ConstDecl]
    def parse_const_decl
      const_token = advance
      name = expect_ident.value
      type_ann = nil
      if check(:colon)
        advance
        type_ann = parse_type_annotation
      end
      expect(:eq, "`=`")
      AST::ConstDecl.new(name, type_ann, parse_expression, const_token.line, const_token.column)
    end

    # `class Name [< Parent]`, then consts, fields and methods until `end`.
    #
    # @return [AST::ClassDef]
    def parse_class
      class_token = advance
      name = expect_ident.value
      parent = nil
      if check(:lt)
        advance
        parent = expect_ident.value
      end

      consts = []
      fields = []
      methods = []
      until check(:end)
        case peek.kind
        when :eof
          raise error_at(peek, "unexpected end of input, expected `end`")
        when :const
          consts << parse_const_decl
        when :def
          methods << parse_function_def
        when :ident
          field = expect_ident
          expect(:colon, "`:`")
          fields << AST::FieldDecl.new(field.value, parse_type_annotation, field.line, field.column)
        else
          raise error_at(peek, "expected a const, field, or method declaration inside `class`, " \
                               "found #{Lexer.describe(peek)}")
        end
      end
      expect(:end, "`end`")
      AST::ClassDef.new(name, parent, consts, fields, methods, class_token.line, class_token.column)
    end

    # @return [AST::FunctionDef]
    def parse_function_def
      def_token = advance
      name = expect_ident.value
      expect(:l_paren, "`(`")
      params = parse_comma_separated(:r_paren, "`)`") { parse_param }
      return_type = nil
      if check(:colon)
        advance
        return_type = parse_type_annotation
      end
      body = parse_block([:end])
      expect(:end, "`end`")
      AST::FunctionDef.new(name, params, return_type, body, def_token.line, def_token.column)
    end

    # @return [AST::Return]
    def parse_return
      token = advance
      value = check(:end) || check(:eof) ? nil : parse_expression
      AST::Return.new(value, token.line, token.column)
    end

    # @return [AST::If]
    def parse_if
      if_token = advance
      condition = parse_expression
      then_body = parse_block([:elsif, :else, :end])
      elsif_branches = []
      while check(:elsif)
        advance
        branch_condition = parse_expression
        elsif_branches << [branch_condition, parse_block([:elsif, :else, :end])]
      end
      else_body = nil
      if check(:else)
        advance
        else_body = parse_block([:end])
      end
      expect(:end, "`end`")
      AST::If.new(condition, then_body, elsif_branches, else_body, if_token.line, if_token.column)
    end

    # @return [AST::While]
    def parse_while
      while_token = advance
      condition = parse_expression
      body = parse_block([:end])
      expect(:end, "`end`")
      AST::While.new(condition, body, while_token.line, while_token.column)
    end

    # `for name in start..end`; the bounds are additive expressions, so a
    # comparison can't swallow the `..`.
    #
    # @return [AST::For]
    def parse_for
      for_token = advance
      var_name = expect_ident.value
      expect(:in, "`in`")
      range_start = parse_additive
      expect(:dot_dot, "`..`")
      range_end = parse_additive
      body = parse_block([:end])
      expect(:end, "`end`")
      AST::For.new(var_name, range_start, range_end, body, for_token.line, for_token.column)
    end

    # @return [AST::Param]
    def parse_param
      name = expect_ident
      expect(:colon, "`:`")
      AST::Param.new(name.value, parse_type_annotation, name.line, name.column)
    end
  end
end
