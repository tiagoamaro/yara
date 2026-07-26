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
    /// instance at all, or if its class has no method of that name (including
    /// inherited methods — the class's flattened `ClassDecl` includes all
    /// inherited methods, so method lookup covers the full inheritance chain).
    pub(super) fn call_method(
        &mut self,
        object_val: Value,
        method: &str,
        args: &[Expr],
        line: usize,
        column: usize,
    ) -> Result<Value, RuntimeError> {
        let Value::Instance(_, class_name) = &object_val else {
            if let Some(kind) = crate::methods::ReceiverKind::of_value(&object_val) {
                return super::methods::eval_primitive_method(
                    self, kind, object_val, method, args, line, column,
                );
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{FieldDecl, Param, TypeAnnotation};

    /// Helper to create a simple integer literal expression.
    fn int_expr(value: i64) -> Expr {
        Expr::IntLit {
            value,
            line: 1,
            column: 1,
        }
    }

    /// Helper to create a simple identifier expression.
    fn ident_expr(name: &str) -> Expr {
        Expr::Ident {
            name: name.to_string(),
            line: 1,
            column: 1,
        }
    }

    /// Helper to create a simple variable declaration statement.
    fn var_decl(name: &str, value: Expr) -> Stmt {
        Stmt::VarDecl {
            name: name.to_string(),
            type_ann: None,
            value,
            line: 1,
            column: 1,
        }
    }

    /// Test: child instance correctly initializes inherited fields.
    /// Parent has field `x`, child has field `y`.
    /// After instantiation, both should exist with correct values.
    #[test]
    fn child_inherits_parent_fields() {
        let program = vec![
            // Parent class
            Stmt::ClassDef {
                name: "Parent".to_string(),
                parent: None,
                consts: vec![],
                fields: vec![FieldDecl {
                    name: "x".to_string(),
                    type_ann: TypeAnnotation {
                        name: "Integer".to_string(),
                        line: 1,
                        column: 1,
                    },
                    line: 1,
                    column: 1,
                }],
                methods: vec![Stmt::FunctionDef {
                    name: "initializer".to_string(),
                    params: vec![Param {
                        name: "val_x".to_string(),
                        type_ann: TypeAnnotation {
                            name: "Integer".to_string(),
                            line: 1,
                            column: 1,
                        },
                        line: 1,
                        column: 1,
                    }],
                    return_type: None,
                    body: vec![var_decl("x", ident_expr("val_x"))],
                    line: 1,
                    column: 1,
                }],
                line: 1,
                column: 1,
            },
            // Child class inheriting from Parent
            Stmt::ClassDef {
                name: "Child".to_string(),
                parent: Some("Parent".to_string()),
                consts: vec![],
                fields: vec![FieldDecl {
                    name: "y".to_string(),
                    type_ann: TypeAnnotation {
                        name: "Integer".to_string(),
                        line: 1,
                        column: 1,
                    },
                    line: 1,
                    column: 1,
                }],
                methods: vec![Stmt::FunctionDef {
                    name: "initializer".to_string(),
                    params: vec![
                        Param {
                            name: "val_x".to_string(),
                            type_ann: TypeAnnotation {
                                name: "Integer".to_string(),
                                line: 1,
                                column: 1,
                            },
                            line: 1,
                            column: 1,
                        },
                        Param {
                            name: "val_y".to_string(),
                            type_ann: TypeAnnotation {
                                name: "Integer".to_string(),
                                line: 1,
                                column: 1,
                            },
                            line: 1,
                            column: 1,
                        },
                    ],
                    return_type: None,
                    body: vec![
                        var_decl("x", ident_expr("val_x")),
                        var_decl("y", ident_expr("val_y")),
                    ],
                    line: 1,
                    column: 1,
                }],
                line: 1,
                column: 1,
            },
        ];

        let mut interp = Interpreter::new();
        let result = interp.run_program(&program);
        assert!(result.is_ok(), "program should run successfully");

        // Check that Child class has both parent's and child's fields
        let child_decl = &interp.classes["Child"];
        assert_eq!(child_decl.field_names.len(), 2);
        // Parent's field should come first
        assert_eq!(child_decl.field_names[0], "x");
        assert_eq!(child_decl.field_names[1], "y");
    }

    /// Test: calling an inherited method on a child instance works.
    /// Parent has method `get_x()`, child calls it.
    #[test]
    fn child_can_call_inherited_method() {
        let program = vec![
            // Parent class with a method
            Stmt::ClassDef {
                name: "Parent".to_string(),
                parent: None,
                consts: vec![],
                fields: vec![FieldDecl {
                    name: "x".to_string(),
                    type_ann: TypeAnnotation {
                        name: "Integer".to_string(),
                        line: 1,
                        column: 1,
                    },
                    line: 1,
                    column: 1,
                }],
                methods: vec![
                    Stmt::FunctionDef {
                        name: "initializer".to_string(),
                        params: vec![Param {
                            name: "val".to_string(),
                            type_ann: TypeAnnotation {
                                name: "Integer".to_string(),
                                line: 1,
                                column: 1,
                            },
                            line: 1,
                            column: 1,
                        }],
                        return_type: None,
                        body: vec![var_decl("x", ident_expr("val"))],
                        line: 1,
                        column: 1,
                    },
                    Stmt::FunctionDef {
                        name: "get_x".to_string(),
                        params: vec![],
                        return_type: Some(TypeAnnotation {
                            name: "Integer".to_string(),
                            line: 1,
                            column: 1,
                        }),
                        body: vec![Stmt::ExprStmt(ident_expr("x"))],
                        line: 1,
                        column: 1,
                    },
                ],
                line: 1,
                column: 1,
            },
            // Child class inheriting from Parent
            Stmt::ClassDef {
                name: "Child".to_string(),
                parent: Some("Parent".to_string()),
                consts: vec![],
                fields: vec![],
                methods: vec![],
                line: 1,
                column: 1,
            },
        ];

        let mut interp = Interpreter::new();
        let result = interp.run_program(&program);
        assert!(result.is_ok(), "program should run successfully");

        // Check that Child class has inherited the method
        assert!(
            interp.classes["Child"].methods.contains_key("get_x"),
            "Child should have inherited get_x method"
        );
    }

    /// Test: child's overridden method runs instead of parent's.
    /// Both parent and child define method `compute()`.
    /// Calling on child instance should execute child's version.
    #[test]
    fn child_overrides_parent_method() {
        let program = vec![
            // Parent class
            Stmt::ClassDef {
                name: "Parent".to_string(),
                parent: None,
                consts: vec![],
                fields: vec![],
                methods: vec![Stmt::FunctionDef {
                    name: "compute".to_string(),
                    params: vec![],
                    return_type: Some(TypeAnnotation {
                        name: "Integer".to_string(),
                        line: 1,
                        column: 1,
                    }),
                    body: vec![Stmt::ExprStmt(int_expr(1))],
                    line: 1,
                    column: 1,
                }],
                line: 1,
                column: 1,
            },
            // Child class with overridden method
            Stmt::ClassDef {
                name: "Child".to_string(),
                parent: Some("Parent".to_string()),
                consts: vec![],
                fields: vec![],
                methods: vec![Stmt::FunctionDef {
                    name: "compute".to_string(),
                    params: vec![],
                    return_type: Some(TypeAnnotation {
                        name: "Integer".to_string(),
                        line: 1,
                        column: 1,
                    }),
                    body: vec![Stmt::ExprStmt(int_expr(2))],
                    line: 1,
                    column: 1,
                }],
                line: 1,
                column: 1,
            },
        ];

        let mut interp = Interpreter::new();
        let result = interp.run_program(&program);
        assert!(result.is_ok(), "program should run successfully");

        // Check that Child's method overwrites Parent's
        // The flattened Child should have the child's version (returns 2)
        let child_method = &interp.classes["Child"].methods["compute"];
        assert_eq!(
            child_method.body.len(),
            1,
            "Child method should have one statement"
        );
        // The body should be the child's return 2, not parent's return 1
        if let Stmt::ExprStmt(Expr::IntLit { value, .. }) = &child_method.body[0] {
            assert_eq!(*value, 2, "Child method should return 2");
        } else {
            panic!("Child method body should be int literal");
        }
    }

    /// Test: parent fields initialize in correct order (parent first, then child).
    /// This is important so parent consts can be evaluated before child consts
    /// that might reference them.
    #[test]
    fn parent_fields_come_before_child_fields() {
        let program = vec![
            Stmt::ClassDef {
                name: "Parent".to_string(),
                parent: None,
                consts: vec![],
                fields: vec![
                    FieldDecl {
                        name: "p1".to_string(),
                        type_ann: TypeAnnotation {
                            name: "Integer".to_string(),
                            line: 1,
                            column: 1,
                        },
                        line: 1,
                        column: 1,
                    },
                    FieldDecl {
                        name: "p2".to_string(),
                        type_ann: TypeAnnotation {
                            name: "Integer".to_string(),
                            line: 1,
                            column: 1,
                        },
                        line: 1,
                        column: 1,
                    },
                ],
                methods: vec![],
                line: 1,
                column: 1,
            },
            Stmt::ClassDef {
                name: "Child".to_string(),
                parent: Some("Parent".to_string()),
                consts: vec![],
                fields: vec![
                    FieldDecl {
                        name: "c1".to_string(),
                        type_ann: TypeAnnotation {
                            name: "Integer".to_string(),
                            line: 1,
                            column: 1,
                        },
                        line: 1,
                        column: 1,
                    },
                    FieldDecl {
                        name: "c2".to_string(),
                        type_ann: TypeAnnotation {
                            name: "Integer".to_string(),
                            line: 1,
                            column: 1,
                        },
                        line: 1,
                        column: 1,
                    },
                ],
                methods: vec![],
                line: 1,
                column: 1,
            },
        ];

        let mut interp = Interpreter::new();
        let result = interp.run_program(&program);
        assert!(result.is_ok(), "program should run successfully");

        let child_fields = &interp.classes["Child"].field_names;
        assert_eq!(child_fields.len(), 4);
        // Parent fields come first
        assert_eq!(child_fields[0], "p1");
        assert_eq!(child_fields[1], "p2");
        // Then child fields
        assert_eq!(child_fields[2], "c1");
        assert_eq!(child_fields[3], "c2");
    }

    /// Test: parent consts evaluate first, then child consts.
    /// If child const shadows parent const, child's version is used.
    #[test]
    fn parent_consts_before_child_consts() {
        let program = vec![
            Stmt::ClassDef {
                name: "Parent".to_string(),
                parent: None,
                consts: vec![Stmt::ConstDecl {
                    name: "val".to_string(),
                    type_ann: Some(TypeAnnotation {
                        name: "Integer".to_string(),
                        line: 1,
                        column: 1,
                    }),
                    value: int_expr(10),
                    line: 1,
                    column: 1,
                }],
                fields: vec![],
                methods: vec![],
                line: 1,
                column: 1,
            },
            Stmt::ClassDef {
                name: "Child".to_string(),
                parent: Some("Parent".to_string()),
                consts: vec![Stmt::ConstDecl {
                    name: "val".to_string(),
                    type_ann: Some(TypeAnnotation {
                        name: "Integer".to_string(),
                        line: 1,
                        column: 1,
                    }),
                    value: int_expr(20),
                    line: 1,
                    column: 1,
                }],
                fields: vec![],
                methods: vec![],
                line: 1,
                column: 1,
            },
        ];

        let mut interp = Interpreter::new();
        let result = interp.run_program(&program);
        assert!(result.is_ok(), "program should run successfully");

        let child_consts = &interp.classes["Child"].const_inits;
        // Both const_inits should be present (parent first, child second to override)
        assert_eq!(child_consts.len(), 2);
        assert_eq!(child_consts[0].0, "val"); // Parent's val
        assert_eq!(child_consts[1].0, "val"); // Child's val (overrides parent)
    }
}
