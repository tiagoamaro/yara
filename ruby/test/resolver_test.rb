require "minitest/autorun"
require "tmpdir"
require_relative "../lib/yara"

class ResolverTest < Minitest::Test
  include Yara::AST

  def setup
    @dir = Dir.mktmpdir("yara_resolver")
  end

  def teardown
    FileUtils.remove_entry(@dir)
  end

  def write(name, contents)
    File.join(@dir, name).tap { |path| File.write(path, contents) }
  end

  def resolve(main_path)
    source = File.read(main_path)
    program = Yara::Parser.new(Yara::Lexer.new(source).tokenize).parse_program
    @map = Yara::Diagnostics::SourceMap.new(main_path, source)
    Yara::Resolver.resolve_imports(program, main_path, @map, Yara::Vocabulary.english)
  end

  def test_splices_imported_statements
    write("helper.yara", "def add(a: Int, b: Int): Int\n  a + b\nend")
    resolved = resolve(write("main.yara", "import \"helper\"\nx = add(1, 2)"))

    assert_equal [FunctionDef, VarDecl], resolved.map(&:class)
    assert_equal [3, 2], resolved.map(&:line)
    assert @map.lookup(3).path.end_with?("helper.yara")
  end

  def test_detects_import_cycle
    write("a.yara", "import \"b\"")
    write("b.yara", "import \"a\"")
    error = assert_raises(Yara::ResolveError) { resolve(write("main.yara", "import \"a\"")) }

    assert_equal "import cycle detected at `#{File.join(@dir, "a.yara")}`", error.message
    assert_equal [3, 1], [error.line, error.column]
    assert @map.lookup(3).path.end_with?("b.yara")
  end

  def test_missing_import_reports_position
    error = assert_raises(Yara::ResolveError) { resolve(write("main.yara", "x = 1\n  import \"does_not_exist\"")) }

    assert_equal "cannot resolve import `does_not_exist`: No such file or directory (os error 2)", error.message
    assert_equal [2, 3], [error.line, error.column]
  end

  def test_wraps_parse_error_in_imported_file
    write("broken.yara", "x = (1")
    error = assert_raises(Yara::ResolveError) { resolve(write("main.yara", "import \"broken\"")) }

    assert error.message.start_with?("parse error in `#{File.join(@dir, "broken.yara")}`: 1:")
  end

  def test_import_path_mirrors_rust_path_join
    assert_equal "helper.yara", Yara::Resolver.import_path("main.yara", "helper")
    assert_equal "examples/lib/x.yara", Yara::Resolver.import_path("examples/main.yara", "lib/x.yara")
    assert_equal "/abs/x.yara", Yara::Resolver.import_path("examples/main.yara", "/abs/x")
  end
end
