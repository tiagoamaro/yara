require "minitest/autorun"
require "stringio"
require_relative "../lib/yara"

# Includes the Ruby-specific traps listed in `PLAN.md`.
class InterpreterTest < Minitest::Test
  # Source, variable, expected value after running.
  VALUES = [
    ["x = 1 + 2 * 3", "x", 7],
    ["def add(a: Int, b: Int): Int\n  a + b\nend\nresult = add(2, 3)", "result", 5],
    ["if 1 > 2\n  x = 1\nelse\n  x = 2\nend", "x", 2],
    ["x = 0\nwhile x < 5\n  x = x + 1\nend", "x", 5],
    ["total = 0\nfor i in 0..5\n  total = total + i\nend", "total", 10],
    ["x = \"a\" + \"b\"", "x", "ab"],
    ["def f(): Int\n  return 1\n  2\nend\nx = f()", "x", 1],
    ["x = -5", "x", -5],
    ["y = -1.5", "y", -1.5],
    ["def fact(n: Int): Int\n  if n <= 1\n    1\n  else\n    n * fact(n - 1)\n  end\nend\nx = fact(5)", "x", 120],
    ["def pick(n: Int): Int\n  if n < 0\n    100\n  else\n    1\n  end\nend\nr = pick(-5)", "r", 100],
    ["def grade(n: Int): Int\n  if n < 1\n    0\n  elsif n < 2\n    1\n  else\n    2\n  end\nend\ng = grade(1)", "g", 1],
    ["def f(n: Int): Int\n  if n < 0\n    0\n  else\n    if n < 10\n      1\n    else\n      2\n    end\n  end\nend\nx = f(5)", "x", 1],
    ["p: Ptr<Integer> = nil\nq: Ptr<Integer> = alloc(1)\na = p == nil", "a", true],
    ["p: Ptr<Integer> = nil\nq: Ptr<Integer> = alloc(1)\nb = q == nil", "b", false],
    ["def leak()\n  p: Ptr<Integer> = alloc(1)\nend\nleak()\nkept: Ptr<Integer> = alloc(2)\nn: Integer = collect()", "n", 1],
    ["def leak()\n  p: Ptr<Integer> = alloc(1)\nend\nleak()\nkept: Ptr<Integer> = alloc(2)\nn: Integer = collect()\nv: Integer = deref(kept)", "v", 2],
    ["def make(): Ptr<Integer>\n  alloc(7)\nend\nq: Ptr<Integer> = make()\nn: Integer = collect()\nv: Integer = deref(q)", "v", 7],
    ["inner: Ptr<Integer> = alloc(9)\nouter: Ptr<Ptr<Integer>> = alloc(inner)\ninner = alloc(0)\nfree(inner)\nn: Integer = collect()", "n", 0],
    ["inner: Ptr<Integer> = alloc(9)\nouter: Ptr<Ptr<Integer>> = alloc(inner)\ninner = alloc(0)\nn: Integer = collect()\nv: Integer = deref(deref(outer))", "v", 9],
    ["xs: IntArray = [1, 2]\nxs.push(3)\nn = xs.size()", "n", 3],
    ["xs: IntArray = [10, 20, 30]\nv = xs.get(1)", "v", 20],
    ["xs: IntArray = [1, 2, 3]\nxs.set(1, 99)\nv = xs.get(1)", "v", 99],
    ["ys: IntArray = []\nb = ys.is_empty()", "b", true],
    ["s = \"hello\"\nn = s.size()", "n", 5],
    ["s = \"hello\"\nu = s.upper()", "u", "HELLO"],
    ["s = \"HELLO\"\nl = s.lower()", "l", "hello"],
    ["s = \"  hello  \"\nt = s.trim()", "t", "hello"],
    ["s = \"42\"\nn = s.to_i()", "n", 42],
    ["s = \"3.14\"\nf = s.to_f()", "f", 3.14],
    ["n = 42\ns = n.to_s()", "s", "42"],
    ["n = 5\nf = n.to_f()", "f", 5.0],
    ["a = -10\nx = a.abs()", "x", 10],
    ["f = 2.5\ns = f.to_s()", "s", "2.5"],
    ["f = 5.0\ns = f.to_s()", "s", "5"],
    ["f = 3.7\nn = f.to_i()", "n", 3],
    ["a = -2.5\nx = a.abs()", "x", 2.5],
    ["a = false\ns = a.to_s()", "s", "false"],
    # Ruby-only parity traps.
    ["x = -7 / 2", "x", -3],
    ["x = 7 / -2", "x", -3],
    ["s = \" -12 \"\nn = s.to_i()", "n", -12],
    ["s = \"+5\"\nn = s.to_i()", "n", 5],
    ["s = \"1.e2\"\nf = s.to_f()", "f", 100.0],
    ["s = \".5\"\nf = s.to_f()", "f", 0.5],
    ["s = \"-inf\"\nf = s.to_f()", "f", -Float::INFINITY],
    ["f = 1000000000000000000000000000000.0\nn = f.to_i()", "n", (2**63) - 1],
    ["f = -1000000000000000000000000000000.0\nn = f.to_i()", "n", -(2**63)]
  ].freeze

  # Source and the runtime error message it must raise.
  ERRORS = [
    ["x = 1 / 0", "division by zero"],
    ["p: Ptr<Integer> = nil\nderef(p)", "nil pointer dereference: `deref` on `nil`"],
    ["xs: IntArray = []\nxs.pop()", "cannot `pop` from an empty array"],
    ["s = \"not_a_number\"\nn = s.to_i()", "cannot parse `not_a_number` as an Integer"],
    ["s = \"12abc\"\nn = s.to_i()", "cannot parse `12abc` as an Integer"],
    ["s = \"99999999999999999999\"\nn = s.to_i()", "cannot parse `99999999999999999999` as an Integer"],
    ["s = \"e5\"\nf = s.to_f()", "cannot parse `e5` as a Float"],
    ["x = 9223372036854775807 + 1", "integer overflow in `+`"],
    ["x = 3037000500 * 3037000500", "integer overflow in `*`"],
    ["x = 0 - 9223372036854775807 - 2", "integer overflow in `-`"]
  ].freeze

  def interpreter_for(source, vocabulary = Yara::Vocabulary.english)
    program = Yara::Parser.new(Yara::Lexer.new(source, vocabulary).tokenize, vocabulary).parse_program
    Yara::Interpreter.new(vocabulary, StringIO.new).tap { |interpreter| interpreter.run_program(program) }
  end

  def variable(interpreter, name)
    interpreter.instance_variable_get(:@environment).lookup(name)
  end

  VALUES.each_with_index do |(source, name, expected), index|
    define_method("test_value_#{index}_#{name}") do
      actual = variable(interpreter_for(source), name)

      assert_equal [expected, expected.class], [actual, actual.class]
    end
  end

  ERRORS.each_with_index do |(source, message), index|
    define_method("test_error_#{index}") do
      assert_equal message, assert_raises(Yara::RuntimeError) { interpreter_for(source) }.message
    end
  end

  def test_runtime_error_carries_the_call_stack
    error = assert_raises(Yara::RuntimeError) { interpreter_for("def boom(): Int\n  1 / 0\nend\nboom()") }

    assert_equal [["boom", 4]], error.frames.map { |frame| [frame.name, frame.span.line] }
  end

  def test_runtime_messages_are_localized_with_english_fallback
    vocabulary = Yara::Vocabulary.new(Yara::Lexer::KEYWORDS, {}, { "runtime/undefined-function" => "funcao desconhecida `{0}`" })

    assert_equal "funcao desconhecida `mystery`",
                 assert_raises(Yara::RuntimeError) { interpreter_for("mystery()", vocabulary) }.message
    assert_equal "array index 5 out of bounds (length 1)",
                 assert_raises(Yara::RuntimeError) { interpreter_for("x: Integer = [1][5]", vocabulary) }.message
  end

  def test_boolean_to_s_uses_the_localized_keyword
    keywords = Yara::Lexer::KEYWORDS.reject { |_, kind| kind == :true }.merge("verdadeiro" => :true)
    vocabulary = Yara::Vocabulary.new(keywords, {}, {})

    assert_equal "verdadeiro", variable(interpreter_for("a = verdadeiro\ns = a.to_s()", vocabulary), "s")
  end

  def test_child_inherits_parent_fields_and_methods
    source = "class Parent\n  x: Integer\n  def initializer(v: Integer)\n    x = v\n  end\n  def get_x(): Integer\n    x\n  end\nend\n" \
             "class Child < Parent\n  y: Integer\n  def initializer(a: Integer, b: Integer)\n    x = a\n    y = b\n  end\nend\n" \
             "c = Child.new(1, 2)\nv = c.get_x()\nw = c.y"
    interpreter = interpreter_for(source)

    assert_equal [1, 2], [variable(interpreter, "v"), variable(interpreter, "w")]
    assert_equal %w[x y], interpreter.instance_variable_get(:@classes)["Child"].field_names
  end

  def test_print_joins_arguments_like_rust_display
    stdout = StringIO.new
    program = Yara::Parser.new(Yara::Lexer.new("print(1, 2.0, 0.1, \"s\", true, nil, [1.5, 2.0])").tokenize).parse_program
    Yara::Interpreter.new(Yara::Vocabulary.english, stdout).run_program(program)

    assert_equal "1 2 0.1 s true nil [1.5, 2]\n", stdout.string
  end

  def test_primitive_methods_all_have_an_implementation
    Yara::Methods::ARITIES.each_key do |kind, name|
      assert Yara::Interpreter.private_method_defined?("eval_#{kind}_#{name}"), "no eval for #{kind}##{name}"
    end
    Yara::Builtins::ARITIES.each_key do |name|
      assert Yara::Interpreter.private_method_defined?("eval_builtin_#{name}"), "no eval for #{name}"
    end
  end
end
