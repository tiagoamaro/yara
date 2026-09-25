module Yara
  class Interpreter
    private

    # `ClassName.new(args)`: evaluates consts in order (each can read the
    # earlier ones by bare name), sets instance variables to nil, then runs
    # `initializer` if the class has one.
    #
    # @param class_name [String]
    # @param args [Array<AST::Node>]
    # @param line [Integer]
    # @param column [Integer]
    # @return [Instance]
    def construct(class_name, args, line, column)
      decl = @classes[class_name]
      fields = {}
      with_frame("#{class_name}.new", line, column) do
        decl.const_inits.each do |name, expr|
          fields.each { |field, value| @environment.declare(field, value) }
          fields[name] = eval_expr(expr)
        end
      end
      decl.field_names.each { |name| fields[name] = nil }
      instance = Instance.new(class_name, fields)
      initializer = decl.methods["initializer"]
      run_method(instance, "initializer", initializer, args, line, column) if initializer
      instance
    end

    # An instance method, or a primitive method for any other receiver with
    # a receiver kind.
    #
    # @param object [Object]
    # @param method [String]
    # @param args [Array<AST::Node>]
    # @param line [Integer]
    # @param column [Integer]
    # @return [Object]
    def call_method(object, method, args, line, column)
      unless object.is_a?(Instance)
        kind = receiver_kind(object)
        return eval_primitive_method(kind, object, method, args, line, column) if kind

        runtime_error_msg("runtime/method-on-non-object", [method], line, column)
      end
      function = @classes[object.class_name].methods[method] ||
                 runtime_error_msg("runtime/class-has-no-method", [object.class_name, method], line, column)
      run_method(object, method, function, args, line, column)
    end

    # Implicit `self`: the instance's fields are copied into the method's
    # scope as locals, and whatever those names hold afterwards is copied
    # back, even when the body raises.
    #
    # @param instance [Instance]
    # @param method_name [String]
    # @param function [Function]
    # @param args [Array<AST::Node>]
    # @param line [Integer]
    # @param column [Integer]
    # @return [Object]
    def run_method(instance, method_name, function, args, line, column)
      values = args.map { |arg| eval_expr(arg) }
      with_frame("#{instance.class_name}##{method_name}", line, column) do
        field_names = instance.fields.keys
        instance.fields.each { |name, value| @environment.declare(name, value) }
        function.params.zip(values).each { |name, value| @environment.declare(name, value) }
        begin
          exec_function_body(function.body)
        ensure
          scope = @environment.current
          field_names.each { |name| instance.fields[name] = scope[name] if scope.key?(name) }
        end
      end
    end
  end
end
