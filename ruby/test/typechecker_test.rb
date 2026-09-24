require "minitest/autorun"
require_relative "../lib/yara"

# Ported from the tests in `rust/src/typechecker/` (`mod.rs`, `classes.rs`,
# `methods.rs`).
class TypeCheckerTest < Minitest::Test
  HELLO_CLASS = "class Hello\n  const PI: Float = 3.14159\n  count: Integer\n\n  def initializer(number: Int)\n    count = number\n  end\nend\n".freeze
  NODE_CLASS = "class Node\n  value: Integer\n\n  def initializer(v: Integer)\n    value = v\n  end\nend\n".freeze
  PARENT_CLASS = "class Parent\n  value: Integer\n\n  def initializer(v: Int)\n    value = v\n  end\n\n  def get_value(): Int\n    value\n  end\nend\n\n".freeze

  ACCEPTED = {
    "valid function" => "def add(a: Int, b: Int): Int\n  a + b\nend",
    "string concatenation" => "x = \"a\" + \"b\"",
    "function call" => "def add(a: Int, b: Int): Int\n  a + b\nend\nprint(add(1, 2))",
    "negation" => "x: Int = -5\ny: Float = -1.5",
    "if-else tail" => "def fact(n: Int): Int\n  if n <= 1\n    1\n  else\n    n * fact(n - 1)\n  end\nend",
    "array literal and index" => "xs: IntArray = [1, 2, 3]\ny: Int = xs[0]",
    "empty array literal" => "xs: IntArray = []",
    "array builtins" => "xs: IntArray = [1, 2]\npush(xs, 3)\ny: Int = get(xs, 0)\nset(xs, 0, 9)\nz: Int = len(xs)\nw: Int = pop(xs)",
    "conditional field assignment" => "class Counter\n  count: Integer\n\n  def initializer(big: Bool)\n    if big\n      count = 100\n    else\n      count = 0\n    end\n  end\nend\n",
    "construction, field and const" => "#{HELLO_CLASS}h: Hello = Hello.new(5)\nx: Int = h.count\ny: Float = h.PI",
    "field assignment" => "#{HELLO_CLASS}h = Hello.new(5)\nh.count = 9",
    "pointer builtins" => "p: Ptr<Integer> = alloc(5)\nx: Integer = deref(p)\nset_deref(p, 9)\nfree(p)",
    "pointer to class" => "#{NODE_CLASS}n: Node = Node.new(7)\np: Ptr<Node> = alloc(n)\nm: Node = deref(p)",
    "nil pointers" => "p: Ptr<Integer> = nil\nq: Ptr<Integer> = alloc(5)\na: Boolean = p == nil\nb: Boolean = nil != q",
    "collect" => "n: Integer = collect()",
    "inherited field" => "#{PARENT_CLASS}class Child < Parent\n  def initializer(v: Int)\n    value = v\n  end\nend\n\nc: Child = Child.new(42)\nx: Int = c.value\ny: Int = c.get_value()\n",
    "overridden method" => "class Parent\n  def greet(): String\n    \"Parent\"\n  end\nend\n\nclass Child < Parent\n  def greet(): String\n    \"Child\"\n  end\nend\n\nc: Child = Child.new()\nx: String = c.greet()\n",
    "multi-level inheritance" => "class GrandParent\n  a: Integer\n\n  def initializer(x: Int)\n    a = x\n  end\n\n  def get_a(): Int\n    a\n  end\nend\n\nclass Parent < GrandParent\n  b: Integer\n\n  def initializer(x: Int, y: Int)\n    a = x\n    b = y\n  end\nend\n\nclass Child < Parent\n  c: Integer\n\n  def initializer(x: Int, y: Int, z: Int)\n    a = x\n    b = y\n    c = z\n  end\nend\n\nch: Child = Child.new(1, 2, 3)\nv1: Int = ch.a\nv2: Int = ch.b\nv3: Int = ch.c\nv4: Int = ch.get_a()\n",
    "array methods" => "xs: IntArray = []\nxs.push(5)\nn: Int = xs.size()\nm: Int = xs.get(0)\nxs.set(0, 99)\no: Int = xs.pop()\nb: Boolean = xs.is_empty()",
    "string methods" => "s: String = \" hi \"\nn: Int = s.size()\na: String = s.upper()\nb: String = s.lower()\nc: String = s.trim()\nd: Boolean = s.is_empty()\ne: Int = s.to_i()\nf: Float = s.to_f()\ng: String = s.to_s()",
    "number and boolean methods" => "n: Int = -42\na: String = n.to_s()\nb: Float = n.to_f()\nc: Int = n.abs()\nf: Float = -3.14\nd: String = f.to_s()\ne: Int = f.to_i()\ng: Float = f.abs()\nt: Boolean = true\nh: String = t.to_s()",
    "pointer methods" => "p: Ptr<Int> = alloc(5)\nn: Int = p.deref()\np.set_deref(10)\np.free()"
  }.freeze

  REJECTED = {
    "Int + Float" => ["x: Int = 5\ny: Float = 1.0\nz = x + y", "cannot apply"],
    "return type" => ["def bad(): Int\n  \"oops\"\nend", "declared to return"],
    "declaration type" => ["x: Int = \"hi\"", "type mismatch"],
    "if condition" => ["if 5\n  1\nend", "must be Boolean"],
    "undefined variable" => ["print(missing)", "undefined variable"],
    "call arity" => ["def add(a: Int, b: Int): Int\n  a + b\nend\nadd(1)", "expects 2 argument"],
    "for range" => ["for i in 0..\"a\"\n  print(i)\nend", "`for` range bounds must be Integer"],
    "negating a string" => ["x = -\"hi\"", "cannot negate"],
    "if branch types" => ["def f(): Int\n  if true\n    1\n  else\n    \"oops\"\n  end\nend", "different types"],
    "mixed array" => ["xs: IntArray = [1, \"two\"]", "must share one type"],
    "push mismatch" => ["xs: IntArray = [1, 2]\npush(xs, \"oops\")", "push"],
    "pop mismatch" => ["xs: IntArray = [1]\ny: String = pop(xs)", "type mismatch"],
    "string index" => ["xs: IntArray = [1]\ny = xs[\"zero\"]", "must be Integer"],
    "unassigned outside initializer" => ["class Counter\n  count: Integer\n\n  def bump()\n    count = 1\n  end\nend\n", "never assigned in `initializer`"],
    "field assignment type" => ["#{HELLO_CLASS}h = Hello.new(5)\nh.count = \"oops\"", "cannot assign"],
    "unknown field" => ["#{HELLO_CLASS}h = Hello.new(5)\nx = h.missing", "has no field"],
    "unknown method" => ["#{HELLO_CLASS}h = Hello.new(5)\nh.missing_method()", "has no method"],
    "construction arity" => ["#{HELLO_CLASS}h = Hello.new(5, 6)", "expects 1 argument"],
    "set_deref mismatch" => ["p: Ptr<Integer> = alloc(5)\nset_deref(p, 1.5)", "`set_deref` into `Ptr<Integer>` expects `Integer`, found `Float`"],
    "deref non-pointer" => ["deref(3)", "pointer"],
    "pointer type" => ["p: Ptr<Float> = alloc(5)", "type mismatch"],
    "unknown pointee" => ["p: Ptr<Mystery> = alloc(1)", "unknown type `Mystery`"],
    "nil into Integer" => ["x: Integer = nil", "type mismatch"],
    "Integer == nil" => ["x: Integer = 5\nb: Boolean = x == nil", "cannot apply"],
    "collect type" => ["n: String = collect()", "type mismatch"],
    "unknown parent" => ["class Child < NonExistent\nend\n", "unknown class `NonExistent`"],
    "direct cycle" => ["class A < B\nend\n\nclass B < A\nend\n", "inheritance cycle"],
    "indirect cycle" => ["class A < B\nend\n\nclass B < C\nend\n\nclass C < A\nend\n", "inheritance cycle"],
    "inherited field unassigned" => ["#{PARENT_CLASS}class Child < Parent\n  def initializer()\n  end\nend\n", "never assigned in `initializer`"],
    "array push method" => ["xs: IntArray = []\nxs.push(\"bad\")", "push"],
    "unknown primitive method" => ["n: Int = 42\nresult = n.nope()", "has no method `nope` (available: to_s, to_f, abs)"],
    "primitive method arity" => ["n: Int = 42\nresult = n.to_s(1)", "expects 0 argument"]
  }.freeze

  def check(source, vocabulary = Yara::Vocabulary.english)
    program = Yara::Parser.new(Yara::Lexer.new(source).tokenize).parse_program
    Yara::TypeChecker.new(vocabulary).check_program(program)
  end

  ACCEPTED.each do |name, source|
    define_method("test_accepts_#{name.tr(" ,-", "_")}") { check(source) }
  end

  REJECTED.each do |name, (source, fragment)|
    define_method("test_rejects_#{name.tr(" ,+=-", "_")}") do
      error = assert_raises(Yara::TypeError) { check(source) }
      assert_includes error.message, fragment
    end
  end

  def test_unassigned_field_points_at_the_field
    error = assert_raises(Yara::TypeError) { check("class Counter\n  count: Integer\nend\n") }

    assert_equal [2, 3], [error.line, error.column]
  end

  def test_inheritance_cycle_names_the_first_declared_class
    error = assert_raises(Yara::TypeError) { check("class A < B\nend\n\nclass B < A\nend\n") }

    assert_equal ["inheritance cycle: class `A` has a circular parent chain", 1], [error.message, error.line]
  end

  def test_undefined_variable_points_at_its_line
    assert_equal 1, assert_raises(Yara::TypeError) { check("print(missing)") }.line
  end

  def test_unknown_type_is_localized_including_inside_ptr
    vocabulary = Yara::Vocabulary.new(Yara::Lexer::KEYWORDS, {}, { "type/unknown-type" => "tipo desconhecido `{0}`" })

    ["x: Bogus = 5", "x: Ptr<Bogus> = nil"].each do |source|
      assert_equal "tipo desconhecido `Bogus`", assert_raises(Yara::TypeError) { check(source, vocabulary) }.message
    end
  end

  def test_untranslated_message_falls_back_to_english
    vocabulary = Yara::Vocabulary.new(Yara::Lexer::KEYWORDS, {}, { "runtime/division-by-zero" => "divisao por zero" })

    assert_equal "unknown type `Bogus`", assert_raises(Yara::TypeError) { check("x: Bogus = 5", vocabulary) }.message
  end

  def test_primitive_method_table_matches_the_registry
    fixed = Yara::TypeChecker::FIXED_RESULTS.keys
    checked = Yara::Methods::ARITIES.keys - fixed

    assert_empty fixed - Yara::Methods::ARITIES.keys
    checked.each do |kind, name|
      assert Yara::TypeChecker.private_method_defined?("check_#{kind}_#{name}"), "no check for #{kind}##{name}"
    end
  end
end
