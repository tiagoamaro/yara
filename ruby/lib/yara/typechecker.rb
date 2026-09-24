module Yara
  # A static type. `kind` is a primitive name (`Integer`, `Float`, `Boolean`,
  # `String`, `Nil`) or `Array`/`Pointer` (with the element type in `inner`)
  # or `Instance` (with the class name in `inner`). Mirrors
  # `rust/src/typechecker/mod.rs`'s `Type`.
  Type = Data.define(:kind, :inner)

  class Type
    INTEGER = new("Integer", nil)
    FLOAT = new("Float", nil)
    BOOLEAN = new("Boolean", nil)
    STRING = new("String", nil)
    NIL = new("Nil", nil)
    PRIMITIVES = {
      "Integer" => INTEGER, "Float" => FLOAT, "Boolean" => BOOLEAN, "String" => STRING, "Nil" => NIL
    }.freeze
    ARRAY_ANNOTATIONS = {
      "IntArray" => INTEGER, "FloatArray" => FLOAT, "BoolArray" => BOOLEAN, "StringArray" => STRING
    }.freeze
    RECEIVER_KINDS = {
      "Integer" => :integer, "Float" => :float, "Boolean" => :boolean,
      "String" => :string, "Array" => :array, "Pointer" => :pointer
    }.freeze

    # @param element [Type]
    # @return [Type]
    def self.array(element)
      new("Array", element)
    end

    # @param element [Type]
    # @return [Type]
    def self.pointer(element)
      new("Pointer", element)
    end

    # @param class_name [String]
    # @return [Type]
    def self.instance(class_name)
      new("Instance", class_name)
    end

    # The primitive-method receiver kind, or nil for `Nil` and instances,
    # which have no primitive methods.
    #
    # @return [Symbol, nil]
    def receiver_kind
      RECEIVER_KINDS[kind]
    end

    # Whether a value of type `actual` may be stored where `self` is declared:
    # equality, plus the empty-array literal (`Array<Nil>`) into any array and
    # `nil` into any pointer, since pointers are nullable.
    #
    # @param actual [Type]
    # @return [Boolean]
    def accepts?(actual)
      self == actual ||
        (kind == "Array" && actual == Type.array(NIL)) ||
        (kind == "Pointer" && actual == NIL)
    end
  end

  class TypeError < Diagnostics::Error
    # @return [String]
    def kind
      "type error"
    end
  end

  # Static checking of a resolved program, mirroring `rust/src/typechecker/`.
  # Expressions live in `typechecker/expressions.rb`, statements in
  # `typechecker/statements.rb`, free calls and builtins in
  # `typechecker/calls.rb`, classes in `typechecker/classes.rb`, primitive
  # methods in `typechecker/methods.rb`.
  class TypeChecker
    # A function or method signature; `return_type` is nil when undeclared.
    Signature = Struct.new(:param_types, :return_type)

    # A class's field types (instance variables and consts together, both
    # read unqualified inside methods) and method signatures, inherited
    # members included once flattened.
    ClassInfo = Struct.new(:fields, :methods)

    # @param vocabulary [Vocabulary] localized names and error messages
    def initialize(vocabulary = Vocabulary.english)
      @vocabulary = vocabulary
      @environment = Environment.new
      @functions = {}
      @classes = {}
    end

    # Checks classes and function signatures before any body, so nothing
    # has to be declared before it is used, then the top-level statements
    # in order.
    #
    # @param program [Array<AST::Node>]
    # @return [void]
    # @raise [TypeError]
    def check_program(program)
      collect_classes(program)
      collect_function_signatures(program)
      check_classes(program)
      program.each { |statement| check_statement(statement) }
    end

    private

    # Resolves an annotation name: a class, `Ptr<T>` (with `T` resolved the
    # same way, so `Ptr<Node>` works), a primitive, or an array annotation.
    #
    # @param name [String]
    # @param line [Integer]
    # @param column [Integer]
    # @return [Type]
    # @raise [TypeError] for an unknown name
    def resolve_type(name, line, column)
      return Type.instance(name) if @classes.key?(name)
      if name.start_with?("Ptr<") && name.end_with?(">")
        return Type.pointer(resolve_type(name[4...-1], line, column))
      end

      element = Type::ARRAY_ANNOTATIONS[name]
      return Type.array(element) if element

      Type::PRIMITIVES[name] || type_error("type/unknown-type", [name], line, column)
    end

    # @param key [String] message catalog key
    # @param args [Array<String>]
    # @param line [Integer]
    # @param column [Integer]
    # @raise [TypeError]
    def type_error(key, args, line, column)
      raise TypeError.new(@vocabulary.msg(key, args), line, column)
    end

    # @param type [Type]
    # @return [String] `type` spelled in the run's vocabulary
    def name_of(type)
      @vocabulary.type_name(type)
    end
  end
end
