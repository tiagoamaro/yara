module Yara
  # A class instance: its class name and a field map shared by every binding
  # of the instance. Equal when the class and every field are.
  Instance = Struct.new(:class_name, :fields) do
    # @return [String]
    def to_s
      "#<#{class_name}>"
    end
  end

  # An index into the interpreter's heap, not a reference.
  Pointer = Struct.new(:index) do
    # @return [String]
    def to_s
      "ptr##{index}"
    end
  end

  class RuntimeError < Diagnostics::Error
    # @param message [String]
    # @param line [Integer]
    # @param column [Integer]
    # @param call_stack [Array<Diagnostics::Frame>] outermost first
    def initialize(message, line, column, call_stack)
      super(message, line, column)
      @call_stack = call_stack
    end

    # @return [String]
    def kind
      "runtime error"
    end

    # @return [Array<Diagnostics::Frame>] innermost first
    def frames
      @call_stack.reverse
    end
  end

  # Tree-walk evaluator over a typechecked program. Yara values are Ruby values:
  # `Integer`, `Float`, `true`/`false`, `String`, `nil`, `Array` (shared by
  # reference), `Instance` and `Pointer`. Integers stay within i64: overflow is
  # a runtime error rather than a promotion to Bignum.
  class Interpreter
    # A function or method: parameter names and body.
    Function = Struct.new(:params, :body)

    # A class flattened with its ancestors: `[name, value expression]` const
    # initializers and instance-variable names (parents' first), and methods.
    ClassDecl = Struct.new(:const_inits, :field_names, :methods)

    # Unwinds a function body on `return`.
    Return = Struct.new(:value)

    INTEGER_MIN = -(2**63)
    INTEGER_MAX = (2**63) - 1

    # @param vocabulary [Vocabulary]
    # @param stdout [IO] where `print` writes
    def initialize(vocabulary = Vocabulary.english, stdout = $stdout)
      @vocabulary = vocabulary
      @stdout = stdout
      @environment = Environment.new
      @functions = {}
      @classes = {}
      @call_stack = []
      @heap = []
    end

    # Registers every function and class, flattens inheritance, then runs the
    # top-level statements in order.
    #
    # @param program [Array<AST::Node>]
    # @return [void]
    # @raise [RuntimeError]
    def run_program(program)
      definitions = program.grep(AST::ClassDef)
      program.grep(AST::FunctionDef).each { |function| @functions[function.name] = function_of(function) }
      definitions.each do |klass|
        methods = {}
        klass.methods.each { |method| methods[method.name] = function_of(method) }
        const_inits = klass.consts.map { |const| [const.name, const.value] }
        @classes[klass.name] = ClassDecl.new(const_inits, klass.fields.map(&:name), methods)
      end
      flatten_classes(definitions)
      program.each { |statement| exec_statement(statement) }
    end

    # The text `print` and error messages show for a value.
    #
    # @param value [Object]
    # @return [String]
    def self.display(value)
      case value
      when Float then RustFormat.float_display(value)
      when nil then "nil"
      when Array then "[#{value.map { |element| display(element) }.join(", ")}]"
      else value.to_s
      end
    end

    private

    # @param function [AST::FunctionDef]
    # @return [Function]
    def function_of(function)
      Function.new(function.params.map(&:name), function.body)
    end

    # Merges each parent's declarations into its children, parents first; the
    # typechecker already rejected unknown parents and cycles.
    #
    # @param definitions [Array<AST::ClassDef>]
    # @return [void]
    def flatten_classes(definitions)
      parents = {}
      definitions.each { |klass| parents[klass.name] = klass.parent }
      flattened = {}
      flatten = lambda do |name|
        return flattened[name] if flattened.key?(name)

        own = @classes[name]
        parent = parents[name] && flatten.call(parents[name])
        flattened[name] =
          if parent
            ClassDecl.new(parent.const_inits + own.const_inits, parent.field_names + own.field_names,
                          parent.methods.merge(own.methods))
          else
            own
          end
      end
      parents.each_key { |name| flatten.call(name) }
      @classes = flattened
    end

    # @param message [String]
    # @param line [Integer]
    # @param column [Integer]
    # @raise [RuntimeError] carrying the current call stack
    def runtime_error(message, line, column)
      raise RuntimeError.new(message, line, column, @call_stack.dup)
    end

    # @param key [String] message catalog key
    # @param args [Array<String>]
    # @param line [Integer]
    # @param column [Integer]
    # @raise [RuntimeError]
    def runtime_error_msg(key, args, line, column)
      runtime_error(@vocabulary.msg(key, args), line, column)
    end

    # @param value [Object]
    # @return [String]
    def display(value)
      Interpreter.display(value)
    end

    # Pushes a call-stack frame and a scope around `block`, popping both even
    # when it raises.
    #
    # @param name [String] the frame's label
    # @param line [Integer]
    # @param column [Integer]
    # @return [Object] the block's result
    def with_frame(name, line, column)
      @call_stack << Diagnostics::Frame.new(name, Diagnostics::Span.new(line, column))
      @environment.push_scope
      yield
    ensure
      @environment.pop_scope
      @call_stack.pop
    end
  end
end
