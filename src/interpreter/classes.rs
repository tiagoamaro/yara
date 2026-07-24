use super::*;

impl Interpreter {
    /// `ClassName.new(args)`: builds a fresh instance, evaluates each class
    /// const in declaration order (so a later const may reference an earlier
    /// one), zero-initializes declared fields to `Nil`, then runs
    /// `initializer` (if the class has one) with `args` bound to its params.
    pub(super) fn construct(
        &mut self,
        class_name: &str,
        args: &[Expr],
        line: usize,
        column: usize,
    ) -> Result<Value, RuntimeError> {
        let decl = self.classes[class_name].clone();
        let fields: Rc<RefCell<HashMap<String, Value>>> = Rc::new(RefCell::new(HashMap::new()));

        self.call_stack.push(StackFrame {
            function_name: format!("{class_name}.new"),
            line,
            column,
        });
        self.push_scope();
        for (const_name, expr) in &decl.const_inits {
            for (k, v) in fields.borrow().iter() {
                self.declare_var(k, v.clone());
            }
            let value = self.eval_expr(expr)?;
            fields.borrow_mut().insert(const_name.clone(), value);
        }
        self.pop_scope();

        for field_name in &decl.field_names {
            fields.borrow_mut().insert(field_name.clone(), Value::Nil);
        }
        self.call_stack.pop();

        let instance = Value::Instance(fields, class_name.to_string());
        if let Some(initializer) = decl.methods.get("initializer") {
            self.run_method(
                &instance,
                "initializer",
                initializer.clone(),
                args,
                line,
                column,
            )?;
        }
        Ok(instance)
    }

    /// Dispatches `object_val.method(args)` to the method declared on
    /// `object_val`'s class (looked up by the class name stored alongside
    /// the instance's fields in `Value::Instance`), then delegates to
    /// `run_method` to actually execute it. Errors if `object_val` isn't an
    /// instance at all, or if its class has no method of that name — no
    /// inheritance, so only the exact class name recorded at construction
    /// time is consulted.
    pub(super) fn call_method(
        &mut self,
        object_val: Value,
        method: &str,
        args: &[Expr],
        line: usize,
        column: usize,
    ) -> Result<Value, RuntimeError> {
        let Value::Instance(_, class_name) = &object_val else {
            return Err(RuntimeError {
                message: format!("cannot call method `{method}` on a non-object value"),
                line,
                column,
                call_stack: self.call_stack.clone(),
            });
        };
        let decl = self.classes[class_name]
            .methods
            .get(method)
            .cloned()
            .ok_or_else(|| RuntimeError {
                message: format!("class `{class_name}` has no method `{method}`"),
                line,
                column,
                call_stack: self.call_stack.clone(),
            })?;
        self.run_method(&object_val, method, decl, args, line, column)
    }

    /// Runs a method body with the instance's fields copied into the method's
    /// scope (so bare names resolve as implicit `self.field`), then copies
    /// any updated field values back into the instance afterward.
    pub(super) fn run_method(
        &mut self,
        instance: &Value,
        method_name: &str,
        decl: FunctionDecl,
        args: &[Expr],
        line: usize,
        column: usize,
    ) -> Result<Value, RuntimeError> {
        let Value::Instance(fields, class_name) = instance else {
            unreachable!("run_method always called with a Value::Instance")
        };
        let arg_values = args
            .iter()
            .map(|a| self.eval_expr(a))
            .collect::<Result<Vec<_>, _>>()?;

        self.call_stack.push(StackFrame {
            function_name: format!("{class_name}#{method_name}"),
            line,
            column,
        });
        self.push_scope();
        let field_snapshot: Vec<(String, Value)> = fields
            .borrow()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (k, v) in &field_snapshot {
            self.declare_var(k, v.clone());
        }
        for (pname, pval) in decl.params.iter().zip(arg_values) {
            self.declare_var(pname, pval);
        }

        let result = self.exec_function_body(&decl.body);

        {
            let mut fields_mut = fields.borrow_mut();
            let current_scope = self.env.current();
            for (k, _) in &field_snapshot {
                if let Some(updated) = current_scope.get(k) {
                    fields_mut.insert(k.clone(), updated.clone());
                }
            }
        }
        self.pop_scope();
        self.call_stack.pop();
        result
    }
}
