require "minitest/autorun"
require_relative "../lib/yara"

class ASTTest < Minitest::Test
  include Yara::AST

  def test_shift_lines_reaches_every_nested_position
    annotation = TypeAnnotation.new("Integer", 1, 10)
    function = FunctionDef.new(
      "f", [Param.new("n", TypeAnnotation.new("Integer", 1, 10), 1, 7)], annotation,
      [If.new(Ident.new("c", 2, 6), [ExprStmt.new(IntLit.new(1, 3, 5))],
              [[Ident.new("d", 4, 9), [ExprStmt.new(IntLit.new(2, 5, 5))]]],
              [ExprStmt.new(Binary.new(:add, IntLit.new(3, 7, 5), IntLit.new(4, 7, 9), 7, 7))], 2, 3)],
      1, 1
    )

    function.shift_lines(10)

    body = function.body.first
    assert_equal [11, 11, 11, 11], [function.line, function.params.first.line,
                                     function.params.first.type_ann.line, function.return_type.line]
    assert_equal [12, 12, 13], [body.line, body.condition.line, body.then_body.first.line]
    assert_equal [14, 15], [body.elsif_branches.first[0].line, body.elsif_branches.first[1].first.line]
    assert_equal [17, 17, 17], [body.else_body.first.line, body.else_body.first.expr.left.line,
                                body.else_body.first.expr.right.line]
    assert_equal [1, 7, 3], [function.column, function.params.first.column, body.column]
  end

  def test_expression_statement_takes_its_position_from_the_expression
    statement = ExprStmt.new(Ident.new("x", 4, 2))

    assert_equal [4, 2], [statement.line, statement.column]
  end
end
