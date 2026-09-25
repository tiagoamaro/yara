module Yara
  class Parser
    COMPARISON_OPERATORS = { eq_eq: :eq, not_eq: :not_eq, lt: :lt, gt: :gt, lt_eq: :lt_eq, gt_eq: :gt_eq }.freeze
    ADDITIVE_OPERATORS = { plus: :add, minus: :sub }.freeze
    MULTIPLICATIVE_OPERATORS = { star: :mul, slash: :div }.freeze

    private

    # Precedence, loosest first: comparison, additive, multiplicative,
    # unary minus, then postfix `[index]`, `.field` and `.method(args)`.
    #
    # @return [AST::Node]
    def parse_expression
      parse_comparison
    end

    # @return [AST::Node]
    def parse_comparison
      parse_left_associative(COMPARISON_OPERATORS) { parse_additive }
    end

    # @return [AST::Node]
    def parse_additive
      parse_left_associative(ADDITIVE_OPERATORS) { parse_multiplicative }
    end

    # @return [AST::Node]
    def parse_multiplicative
      parse_left_associative(MULTIPLICATIVE_OPERATORS) { parse_unary }
    end

    # One precedence level: operands from the block, joined left to right by
    # any operator in `operators`. Each node takes the operator's position.
    #
    # @param operators [Hash{Symbol => Symbol}] token kind to AST operator
    # @yieldreturn [AST::Node] an operand at the next tighter level
    # @return [AST::Node]
    def parse_left_associative(operators)
      left = yield
      while operators.key?(peek.kind)
        token = advance
        left = AST::Binary.new(operators[token.kind], left, yield, token.line, token.column)
      end
      left
    end

    # @return [AST::Node]
    def parse_unary
      return parse_postfix(parse_primary) unless check(:minus)

      token = advance
      AST::Unary.new(:neg, parse_unary, token.line, token.column)
    end

    # @param expression [AST::Node]
    # @return [AST::Node]
    def parse_postfix(expression)
      loop do
        if check(:l_bracket)
          bracket = advance
          index = parse_expression
          expect(:r_bracket, "`]`")
          expression = AST::Index.new(expression, index, bracket.line, bracket.column)
        elsif check(:dot)
          dot = advance
          name = expect_ident.value
          if check(:l_paren)
            advance
            args = parse_comma_separated(:r_paren, "`)`") { parse_expression }
            expression = AST::MethodCall.new(expression, name, args, dot.line, dot.column)
          else
            expression = AST::FieldAccess.new(expression, name, dot.line, dot.column)
          end
        else
          return expression
        end
      end
    end

    # @return [AST::Node]
    def parse_primary
      token = peek
      case token.kind
      when :int then literal(AST::IntLit)
      when :float then literal(AST::FloatLit)
      when :str then literal(AST::StringLit)
      when :bool then literal(AST::BoolLit)
      when :nil
        advance
        AST::NilLit.new(token.line, token.column)
      when :ident
        advance
        return AST::Ident.new(token.value, token.line, token.column) unless check(:l_paren)

        advance
        args = parse_comma_separated(:r_paren, "`)`") { parse_expression }
        AST::Call.new(token.value, args, token.line, token.column)
      when :l_paren
        advance
        expression = parse_expression
        expect(:r_paren, "`)`")
        expression
      when :l_bracket
        advance
        elements = parse_comma_separated(:r_bracket, "`]`") { parse_expression }
        AST::ArrayLit.new(elements, token.line, token.column)
      else
        raise error_at(token, @vocabulary.msg("parse/unexpected-token", [Lexer.describe(token)]))
      end
    end

    # @param node_class [Class] a literal node taking `(value, line, column)`
    # @return [AST::Node]
    def literal(node_class)
      token = advance
      node_class.new(token.value, token.line, token.column)
    end
  end
end
