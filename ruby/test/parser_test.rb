require "minitest/autorun"
require_relative "../lib/yara"
require_relative "support/examples"

class ParserTest < Minitest::Test
  include Yara::AST

  def parse(source)
    Yara::Parser.new(Yara::Lexer.new(source).tokenize).parse_program
  end

  def parse_error(source)
    assert_raises(Yara::ParseError) { parse(source) }
  end

  def test_parses_function_def
    function = parse("def add(a: Int, b: Int): Int\n  a + b\nend").first

    assert_instance_of FunctionDef, function
    assert_equal ["add", 2, "Integer", "Integer", 1],
                 [function.name, function.params.size, function.params.first.type_ann.name,
                  function.return_type.name, function.body.size]
  end

  def test_parses_var_decl_inferred_and_explicit
    inferred, explicit = parse("x = 5\ny: Float = 5.0")

    assert_equal ["x", nil], [inferred.name, inferred.type_ann]
    assert_equal ["y", "Float"], [explicit.name, explicit.type_ann.name]
  end

  def test_parses_type_alias_in_var_decl
    assert_equal %w[Integer Boolean String],
                 parse("x: Int = 5\ny: Bool = true\nz: Str = \"hi\"").map { |statement| statement.type_ann.name }
  end

  def test_parses_if_elsif_else
    statement = parse("if x > 0\n  1\nelsif x < 0\n  2\nelse\n  3\nend").first

    assert_equal [1, 1], [statement.elsif_branches.size, statement.else_body.size]
  end

  def test_parses_while
    assert_instance_of While, parse("while x > 0\n  x = x - 1\nend").first
  end

  def test_parses_for_range
    assert_equal "i", parse("for i in 0..10\n  print(i)\nend").first.var_name
  end

  def test_parses_call_expression_statement
    call = parse("print(\"hi\")").first.expr

    assert_equal ["print", 1], [call.callee, call.args.size]
  end

  def test_operator_precedence
    expression = parse("1 + 2 * 3").first.expr

    assert_equal :add, expression.op
    assert_equal IntLit.new(1, 1, 1), expression.left
    assert_equal :mul, expression.right.op
  end

  def test_parse_error_reports_position
    error = parse_error("def foo(\n  1\nend")

    assert_equal [2, "expected identifier, found Int(1)"], [error.line, error.message]
  end

  def test_const_decl
    constant = parse("const PI: Float = 3.14").first

    assert_equal ["PI", "Float"], [constant.name, constant.type_ann.name]
  end

  def test_parses_unary_negation
    assert_equal Unary.new(:neg, IntLit.new(5, 1, 6), 1, 5), parse("x = -5").first.value
  end

  def test_parses_array_literal_and_index
    literal, index = parse("xs = [1, 2, 3]\ny = xs[0]")

    assert_equal 3, literal.value.elements.size
    assert_instance_of Index, index.value
  end

  def test_parses_import
    assert_equal "helper", parse("import \"helper\"").first.path
  end

  def test_parses_class_with_const_field_and_method
    klass = parse("class Hello\n  const PI: Float = 3.14159\n  count: Integer\n\n" \
                  "  def initializer(number: Int)\n    count = number\n  end\nend").first

    assert_equal ["Hello", 1, "count", "Integer", "initializer", 1],
                 [klass.name, klass.consts.size, klass.fields.first.name, klass.fields.first.type_ann.name,
                  klass.methods.first.name, klass.methods.first.params.size]
  end

  def test_parses_class_with_and_without_parent
    child, simple = parse("class Child < Parent\n  x: Integer\nend\nclass Simple\n  y: Boolean\nend")

    assert_equal ["Parent", "x"], [child.parent, child.fields.first.name]
    assert_equal [nil, "y"], [simple.parent, simple.fields.first.name]
  end

  def test_parses_new_field_access_method_call_and_field_assign
    construct, read, call, assign = parse("h = Hello.new(5)\nx = h.count\nh.greet(\"hi\")\nh.count = 9")

    assert_equal ["new", 1], [construct.value.method, construct.value.args.size]
    assert_instance_of Ident, construct.value.object
    assert_equal "count", read.value.field
    assert_equal ["greet", 1], [call.expr.method, call.expr.args.size]
    assert_instance_of FieldAssign, assign
    assert_equal "count", assign.field
  end

  def test_parses_ptr_type_annotations
    assert_equal ["Ptr<Integer>", "Ptr<Integer>", "Ptr<Ptr<Integer>>"],
                 parse("p: Ptr<Integer> = alloc(5)\nq: Ptr<Int> = alloc(5)\nr: Ptr<Ptr<Integer>> = alloc(q)")
                   .map { |statement| statement.type_ann.name }
  end

  def test_ptr_missing_closing_bracket
    assert_equal "expected `>`, found Eq", parse_error("p: Ptr<Integer = alloc(5)").message
  end

  def test_invalid_assignment_target
    error = parse_error("f(1) = 2")

    assert_equal ["invalid assignment target", 1, 1], [error.message, error.line, error.column]
  end

  def test_unterminated_block_and_class
    assert_equal "unexpected end of input, expected `end`", parse_error("while true\n  1\n").message
    assert_equal "unexpected end of input, expected `end`", parse_error("class A\n  x: Int\n").message
  end

  def test_import_needs_a_string
    assert_equal "expected string literal after `import`, found Ident(\"helper\")", parse_error("import helper").message
  end

  def test_class_body_rejects_other_statements
    assert_equal "expected a const, field, or method declaration inside `class`, found Return",
                 parse_error("class A\n  return\nend").message
  end

  def test_unexpected_token
    assert_equal "unexpected token RParen", parse_error("x = )").message
  end

  # Portuguese-vocabulary examples wait for the vocabulary loader (step 7).
  def test_every_english_example_lexes_and_parses_unless_meant_to_fail
    Examples.all.each do |example|
      next if Examples.run_arguments(example).include?("--vocabulary")
      next if %w[lex_error parse_error].include?(File.basename(example, ".yara"))

      parse(File.read(File.join(Examples::ROOT, example)))
    end
  end
end
