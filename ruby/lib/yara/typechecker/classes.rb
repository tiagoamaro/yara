module Yara
  class TypeChecker
    private

    # Builds the class table in three passes: register every name (so any
    # annotation can name any class, itself included), fill each class's own
    # members, then flatten inheritance.
    #
    # @param program [Array<AST::Node>]
    # @return [void]
    def collect_classes(program)
      classes = program.grep(AST::ClassDef)
      classes.each { |klass| @classes[klass.name] ||= ClassInfo.new({}, {}) }
      classes.each do |klass|
        fields = {}
        klass.consts.each do |const|
          annotation = const.type_ann ||
                       type_error("type/class-const-requires-annotation", [], const.line, const.column)
          fields[const.name] = resolve_type(annotation.name, annotation.line, annotation.column)
        end
        klass.fields.each do |field|
          fields[field.name] = resolve_type(field.type_ann.name, field.line, field.column)
        end
        methods = {}
        klass.methods.each { |method| methods[method.name] = signature_of(method) }
        @classes[klass.name] = ClassInfo.new(fields, methods)
      end
      flatten_inheritance(classes)
    end

    # Merges each parent's (already flattened) members into its children,
    # parents first; a child's own member wins over an inherited one. Unknown
    # parents and cycles are reported at the child's `class`.
    #
    # @param classes [Array<AST::ClassDef>]
    # @return [void]
    def flatten_inheritance(classes)
      definitions = {}
      parents = {}
      classes.each do |klass|
        definitions[klass.name] = klass
        parents[klass.name] = klass.parent if klass.parent
      end
      parents.each do |child, parent|
        next if @classes.key?(parent)

        type_error("type/unknown-parent-class", [child, parent], definitions[child].line, definitions[child].column)
      end

      order = []
      @classes.each_key { |name| visit_for_cycle(name, parents, definitions, [], order) }
      order.each do |name|
        parent = parents[name]
        next unless parent

        child = @classes[name]
        child.fields = @classes[parent].fields.merge(child.fields)
        child.methods = @classes[parent].methods.merge(child.methods)
      end
    end

    # Depth-first walk up the parent chain, appending each class to `order`
    # after its ancestors. A cycle is reported at the first of its classes
    # the walk reached, where Rust picks one by `HashMap` order.
    #
    # @param name [String]
    # @param parents [Hash{String => String}]
    # @param definitions [Hash{String => AST::ClassDef}]
    # @param path [Array<String>] classes on the current chain
    # @param order [Array<String>]
    # @return [void]
    def visit_for_cycle(name, parents, definitions, path, order)
      return if order.include?(name)

      parent = parents[name]
      if parent
        if path.include?(parent) || parent == name
          type_error("type/inheritance-cycle", [parent], definitions[parent].line, definitions[parent].column)
        end
        visit_for_cycle(parent, parents, definitions, path + [name], order)
      end
      order << name
    end

    # Checks every method body with the class's fields pre-declared, since
    # methods read them unqualified.
    #
    # @param program [Array<AST::Node>]
    # @return [void]
    def check_classes(program)
      program.grep(AST::ClassDef).each do |klass|
        check_fields_assigned(program, klass)
        fields = @classes[klass.name].fields
        klass.methods.each do |method|
          @environment.push_scope
          fields.each { |name, type| @environment.declare(name, type) }
          method.params.each do |param|
            @environment.declare(param.name, resolve_type(param.type_ann.name, param.line, param.column))
          end
          check_return_type(method, "type/method-return-type-mismatch", [klass.name, method.name])
          @environment.pop_scope
        end
      end
    end

    # Every instance variable, own or inherited, must be assigned somewhere
    # in `initializer`. Flow-insensitive: an assignment inside an `if` counts.
    #
    # @param program [Array<AST::Node>]
    # @param klass [AST::ClassDef]
    # @return [void]
    def check_fields_assigned(program, klass)
      fields = klass.fields.dup
      parent = klass.parent
      while parent
        definition = program.find { |statement| statement.is_a?(AST::ClassDef) && statement.name == parent }
        break unless definition

        fields.concat(definition.fields)
        parent = definition.parent
      end
      return if fields.empty?

      initializer = klass.methods.find { |method| method.name == "initializer" }
      assigned = initializer ? assigned_names(initializer.body) : []
      fields.each do |field|
        next if assigned.include?(field.name)

        type_error("type/field-never-assigned", [field.name, klass.name, field.type_ann.name], field.line, field.column)
      end
    end

    # Every name assigned anywhere in `body`, nested blocks included: a bare
    # `name = value` (how implicit-self assignment parses) or `object.name = value`.
    #
    # @param body [Array<AST::Node>]
    # @return [Array<String>]
    def assigned_names(body)
      body.flat_map do |statement|
        case statement
        when AST::VarDecl then [statement.name]
        when AST::FieldAssign then [statement.field]
        when AST::If
          assigned_names(statement.then_body) +
            statement.elsif_branches.flat_map { |_, branch| assigned_names(branch) } +
            assigned_names(statement.else_body || [])
        when AST::While, AST::For then assigned_names(statement.body)
        else []
        end
      end
    end

    # @param object [AST::Node]
    # @param field [String]
    # @param line [Integer]
    # @param column [Integer]
    # @return [Type] the field's type
    def check_field_access(object, field, line, column)
      type = check_expr(object)
      type_error("type/cannot-access-field", [field, name_of(type)], line, column) if type.kind != "Instance"

      @classes[type.inner].fields[field] ||
        type_error("type/class-has-no-field", [type.inner, field], line, column)
    end

    # `ClassName.new(args)` when `object` names a class and no variable, an
    # instance method call on an instance, or a primitive method otherwise.
    #
    # @param object [AST::Node]
    # @param method [String]
    # @param args [Array<AST::Node>]
    # @param line [Integer]
    # @param column [Integer]
    # @return [Type]
    def check_method_call(object, method, args, line, column)
      if object.is_a?(AST::Ident) && !@environment.bound?(object.name) && @classes.key?(object.name)
        return check_construction(object.name, method, args, line, column)
      end

      type = check_expr(object)
      if type.kind != "Instance"
        return check_primitive_method(type, method, args, line, column) if type.receiver_kind

        type_error("type/cannot-call-method", [method, name_of(type)], line, column)
      end
      signature = @classes[type.inner].methods[method] ||
                  type_error("type/class-has-no-method", [type.inner, method], line, column)
      check_arguments("#{type.inner}##{method}", signature, args, line, column)
      signature.return_type || Type::NIL
    end

    # @param class_name [String]
    # @param method [String] must be `new`, possibly localized
    # @param args [Array<AST::Node>]
    # @param line [Integer]
    # @param column [Integer]
    # @return [Type]
    def check_construction(class_name, method, args, line, column)
      if @vocabulary.canonical_method(method) != "new"
        type_error("type/class-has-no-static-method", [class_name, method], line, column)
      end
      initializer = @classes[class_name].methods["initializer"]
      if initializer
        check_arguments("#{class_name}.new", initializer, args, line, column)
      elsif !args.empty?
        type_error("type/no-initializer-takes-no-args", [class_name], line, column)
      end
      Type.instance(class_name)
    end
  end
end
