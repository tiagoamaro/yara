use super::*;

impl Interpreter {
    /// Evaluates `len`/`push`/`get`/`set`/`pop` array builtins and
    /// `alloc`/`deref`/`set_deref`/`free` pointer builtins if `callee` names one of them,
    /// returning `Ok(None)` for any other callee so `call_function` falls
    /// through to a user-defined function lookup.
    pub(super) fn call_array_builtin(
        &mut self,
        callee: &str,
        args: &[Expr],
        line: usize,
        column: usize,
    ) -> Result<Option<Value>, RuntimeError> {
        if builtins::lookup(callee).is_none() {
            return Ok(None);
        }
        let expect_array = |v: Value,
                            call_stack: &[StackFrame]|
         -> Result<Rc<RefCell<Vec<Value>>>, RuntimeError> {
            match v {
                Value::Array(items) => Ok(items),
                other => Err(RuntimeError {
                    message: format!("`{callee}` expects an array, found `{other}`"),
                    line,
                    column,
                    call_stack: call_stack.to_vec(),
                }),
            }
        };
        match callee {
            "len" => {
                let arr = self.eval_expr(&args[0])?;
                let items = expect_array(arr, &self.call_stack)?;
                let len = items.borrow().len() as i64;
                Ok(Some(Value::Integer(len)))
            }
            "push" => {
                let arr = self.eval_expr(&args[0])?;
                let value = self.eval_expr(&args[1])?;
                let items = expect_array(arr, &self.call_stack)?;
                items.borrow_mut().push(value);
                Ok(Some(Value::Nil))
            }
            "get" => {
                let arr = self.eval_expr(&args[0])?;
                let idx = self.eval_int(&args[1], line, column)?;
                Ok(Some(self.array_get(&arr, idx, line, column)?))
            }
            "set" => {
                let arr = self.eval_expr(&args[0])?;
                let idx = self.eval_int(&args[1], line, column)?;
                let value = self.eval_expr(&args[2])?;
                let items = expect_array(arr, &self.call_stack)?;
                let mut items = items.borrow_mut();
                let Some(slot) = usize::try_from(idx).ok().and_then(|i| items.get_mut(i)) else {
                    return Err(RuntimeError {
                        message: format!(
                            "array index {idx} out of bounds (length {})",
                            items.len()
                        ),
                        line,
                        column,
                        call_stack: self.call_stack.clone(),
                    });
                };
                *slot = value;
                Ok(Some(Value::Nil))
            }
            "pop" => {
                let arr = self.eval_expr(&args[0])?;
                let items = expect_array(arr, &self.call_stack)?;
                let popped = items.borrow_mut().pop().ok_or_else(|| RuntimeError {
                    message: "cannot `pop` from an empty array".to_string(),
                    line,
                    column,
                    call_stack: self.call_stack.clone(),
                })?;
                Ok(Some(popped))
            }
            "alloc" => {
                let value = self.eval_expr(&args[0])?;
                self.heap.push(Some(value));
                Ok(Some(Value::Pointer(self.heap.len() - 1)))
            }
            "deref" => {
                let ptr = self.eval_expr(&args[0])?;
                let idx = match ptr {
                    Value::Pointer(i) => i,
                    other => {
                        return Err(RuntimeError {
                            message: format!("`deref` expects a pointer, found `{other}`"),
                            line,
                            column,
                            call_stack: self.call_stack.clone(),
                        });
                    }
                };
                match self.heap.get(idx) {
                    Some(Some(v)) => Ok(Some(v.clone())),
                    Some(None) => Err(RuntimeError {
                        message: format!("use after free: pointer ptr#{idx} was already freed"),
                        line,
                        column,
                        call_stack: self.call_stack.clone(),
                    }),
                    None => Err(RuntimeError {
                        message: format!("invalid pointer ptr#{idx}"),
                        line,
                        column,
                        call_stack: self.call_stack.clone(),
                    }),
                }
            }
            "set_deref" => {
                let ptr = self.eval_expr(&args[0])?;
                let new_value = self.eval_expr(&args[1])?;
                let idx = match ptr {
                    Value::Pointer(i) => i,
                    other => {
                        return Err(RuntimeError {
                            message: format!("`set_deref` expects a pointer, found `{other}`"),
                            line,
                            column,
                            call_stack: self.call_stack.clone(),
                        });
                    }
                };
                match self.heap.get_mut(idx) {
                    Some(Some(slot)) => {
                        *slot = new_value;
                        Ok(Some(Value::Nil))
                    }
                    Some(None) => Err(RuntimeError {
                        message: format!("use after free: pointer ptr#{idx} was already freed"),
                        line,
                        column,
                        call_stack: self.call_stack.clone(),
                    }),
                    None => Err(RuntimeError {
                        message: format!("invalid pointer ptr#{idx}"),
                        line,
                        column,
                        call_stack: self.call_stack.clone(),
                    }),
                }
            }
            "free" => {
                let ptr = self.eval_expr(&args[0])?;
                let idx = match ptr {
                    Value::Pointer(i) => i,
                    other => {
                        return Err(RuntimeError {
                            message: format!("`free` expects a pointer, found `{other}`"),
                            line,
                            column,
                            call_stack: self.call_stack.clone(),
                        });
                    }
                };
                match self.heap.get_mut(idx) {
                    Some(Some(_)) => {
                        self.heap[idx] = None;
                        Ok(Some(Value::Nil))
                    }
                    Some(None) => Err(RuntimeError {
                        message: format!("double free: pointer ptr#{idx} was already freed"),
                        line,
                        column,
                        call_stack: self.call_stack.clone(),
                    }),
                    None => Err(RuntimeError {
                        message: format!("invalid pointer ptr#{idx}"),
                        line,
                        column,
                        call_stack: self.call_stack.clone(),
                    }),
                }
            }
            _ => unreachable!("builtin `{callee}` is in the registry but has no interpreter arm"),
        }
    }

    /// Resolves and invokes a call to a bare name: first the `print`
    /// built-in (special-cased directly, joins evaluated args with a space
    /// and prints them), then the array builtins via `call_array_builtin`,
    /// and only then a user-defined top-level function looked up in
    /// `self.functions` (populated by `run_program`'s first pass). For a
    /// user function: evaluates all argument expressions in the *caller's*
    /// scope first, pushes a `StackFrame` (for error traces) and a fresh
    /// `Scope`, binds each param name to its evaluated argument via
    /// `declare_var`, runs the body through `exec_function_body`, then pops
    /// the scope and stack frame before returning the body's result — the
    /// pop happens unconditionally after `exec_function_body` regardless of
    /// whether it returned `Ok` or `Err`, so a mid-body error doesn't leak
    /// the callee's scope.
    pub(super) fn call_function(
        &mut self,
        callee: &str,
        args: &[Expr],
        line: usize,
        column: usize,
    ) -> Result<Value, RuntimeError> {
        if callee == "print" {
            let mut parts = Vec::new();
            for a in args {
                parts.push(self.eval_expr(a)?.to_string());
            }
            println!("{}", parts.join(" "));
            return Ok(Value::Nil);
        }
        if let Some(result) = self.call_array_builtin(callee, args, line, column)? {
            return Ok(result);
        }

        let decl = self
            .functions
            .get(callee)
            .cloned()
            .ok_or_else(|| RuntimeError {
                message: format!("undefined function `{callee}`"),
                line,
                column,
                call_stack: self.call_stack.clone(),
            })?;

        let mut arg_values = Vec::new();
        for a in args {
            arg_values.push(self.eval_expr(a)?);
        }

        self.call_stack.push(StackFrame {
            function_name: callee.to_string(),
            line,
            column,
        });
        self.push_scope();
        for (name, value) in decl.params.iter().zip(arg_values.into_iter()) {
            self.declare_var(name, value);
        }

        let result = self.exec_function_body(&decl.body);

        self.pop_scope();
        self.call_stack.pop();

        result
    }
}
