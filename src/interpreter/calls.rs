use super::*;

pub(crate) fn eval_len(
    interp: &mut Interpreter,
    args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Value, RuntimeError> {
    let arr = interp.eval_expr(&args[0])?;
    match arr {
        Value::Array(items) => {
            let len = items.borrow().len() as i64;
            Ok(Value::Integer(len))
        }
        other => Err(RuntimeError {
            message: format!("`len` expects an array, found `{other}`"),
            line,
            column,
            call_stack: interp.call_stack.clone(),
        }),
    }
}

pub(crate) fn eval_push(
    interp: &mut Interpreter,
    args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Value, RuntimeError> {
    let arr = interp.eval_expr(&args[0])?;
    let value = interp.eval_expr(&args[1])?;
    match arr {
        Value::Array(items) => {
            items.borrow_mut().push(value);
            Ok(Value::Nil)
        }
        other => Err(RuntimeError {
            message: format!("`push` expects an array, found `{other}`"),
            line,
            column,
            call_stack: interp.call_stack.clone(),
        }),
    }
}

pub(crate) fn eval_get(
    interp: &mut Interpreter,
    args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Value, RuntimeError> {
    let arr = interp.eval_expr(&args[0])?;
    let idx = interp.eval_int(&args[1], line, column)?;
    interp.array_get(&arr, idx, line, column)
}

pub(crate) fn eval_set(
    interp: &mut Interpreter,
    args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Value, RuntimeError> {
    let arr = interp.eval_expr(&args[0])?;
    let idx = interp.eval_int(&args[1], line, column)?;
    let value = interp.eval_expr(&args[2])?;
    match arr {
        Value::Array(items) => {
            let mut items = items.borrow_mut();
            let Some(slot) = usize::try_from(idx).ok().and_then(|i| items.get_mut(i)) else {
                return Err(RuntimeError {
                    message: format!("array index {idx} out of bounds (length {})", items.len()),
                    line,
                    column,
                    call_stack: interp.call_stack.clone(),
                });
            };
            *slot = value;
            Ok(Value::Nil)
        }
        other => Err(RuntimeError {
            message: format!("`set` expects an array, found `{other}`"),
            line,
            column,
            call_stack: interp.call_stack.clone(),
        }),
    }
}

pub(crate) fn eval_pop(
    interp: &mut Interpreter,
    args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Value, RuntimeError> {
    let arr = interp.eval_expr(&args[0])?;
    match arr {
        Value::Array(items) => {
            let popped = items.borrow_mut().pop().ok_or_else(|| RuntimeError {
                message: "cannot `pop` from an empty array".to_string(),
                line,
                column,
                call_stack: interp.call_stack.clone(),
            })?;
            Ok(popped)
        }
        other => Err(RuntimeError {
            message: format!("`pop` expects an array, found `{other}`"),
            line,
            column,
            call_stack: interp.call_stack.clone(),
        }),
    }
}

pub(crate) fn eval_alloc(
    interp: &mut Interpreter,
    args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Value, RuntimeError> {
    let value = interp.eval_expr(&args[0])?;
    interp.heap.push(Some(value));
    Ok(Value::Pointer(interp.heap.len() - 1))
}

pub(crate) fn eval_deref(
    interp: &mut Interpreter,
    args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Value, RuntimeError> {
    let ptr = interp.eval_expr(&args[0])?;
    let idx = match ptr {
        Value::Pointer(i) => i,
        Value::Nil => {
            return Err(RuntimeError {
                message: "nil pointer dereference: `deref` on `nil`".to_string(),
                line,
                column,
                call_stack: interp.call_stack.clone(),
            });
        }
        other => {
            return Err(RuntimeError {
                message: format!("`deref` expects a pointer, found `{other}`"),
                line,
                column,
                call_stack: interp.call_stack.clone(),
            });
        }
    };
    match interp.heap.get(idx) {
        Some(Some(v)) => Ok(v.clone()),
        Some(None) => Err(RuntimeError {
            message: format!("use after free: pointer ptr#{idx} was already freed"),
            line,
            column,
            call_stack: interp.call_stack.clone(),
        }),
        None => Err(RuntimeError {
            message: format!("invalid pointer ptr#{idx}"),
            line,
            column,
            call_stack: interp.call_stack.clone(),
        }),
    }
}

pub(crate) fn eval_set_deref(
    interp: &mut Interpreter,
    args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Value, RuntimeError> {
    let ptr = interp.eval_expr(&args[0])?;
    let new_value = interp.eval_expr(&args[1])?;
    let idx = match ptr {
        Value::Pointer(i) => i,
        Value::Nil => {
            return Err(RuntimeError {
                message: "nil pointer dereference: `set_deref` on `nil`".to_string(),
                line,
                column,
                call_stack: interp.call_stack.clone(),
            });
        }
        other => {
            return Err(RuntimeError {
                message: format!("`set_deref` expects a pointer, found `{other}`"),
                line,
                column,
                call_stack: interp.call_stack.clone(),
            });
        }
    };
    match interp.heap.get_mut(idx) {
        Some(Some(slot)) => {
            *slot = new_value;
            Ok(Value::Nil)
        }
        Some(None) => Err(RuntimeError {
            message: format!("use after free: pointer ptr#{idx} was already freed"),
            line,
            column,
            call_stack: interp.call_stack.clone(),
        }),
        None => Err(RuntimeError {
            message: format!("invalid pointer ptr#{idx}"),
            line,
            column,
            call_stack: interp.call_stack.clone(),
        }),
    }
}

pub(crate) fn eval_free(
    interp: &mut Interpreter,
    args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Value, RuntimeError> {
    let ptr = interp.eval_expr(&args[0])?;
    let idx = match ptr {
        Value::Pointer(i) => i,
        Value::Nil => {
            return Err(RuntimeError {
                message: "cannot `free` a nil pointer".to_string(),
                line,
                column,
                call_stack: interp.call_stack.clone(),
            });
        }
        other => {
            return Err(RuntimeError {
                message: format!("`free` expects a pointer, found `{other}`"),
                line,
                column,
                call_stack: interp.call_stack.clone(),
            });
        }
    };
    match interp.heap.get_mut(idx) {
        Some(Some(_)) => {
            interp.heap[idx] = None;
            Ok(Value::Nil)
        }
        Some(None) => Err(RuntimeError {
            message: format!("double free: pointer ptr#{idx} was already freed"),
            line,
            column,
            call_stack: interp.call_stack.clone(),
        }),
        None => Err(RuntimeError {
            message: format!("invalid pointer ptr#{idx}"),
            line,
            column,
            call_stack: interp.call_stack.clone(),
        }),
    }
}

pub(crate) fn eval_collect(
    interp: &mut Interpreter,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Value, RuntimeError> {
    Ok(Value::Integer(interp.collect_garbage()))
}

impl Interpreter {
    /// Evaluates array and pointer builtins if `callee` names one of them,
    /// returning `Ok(None)` for any other callee so `call_function` falls
    /// through to a user-defined function lookup.
    pub(super) fn call_array_builtin(
        &mut self,
        callee: &str,
        args: &[Expr],
        line: usize,
        column: usize,
    ) -> Result<Option<Value>, RuntimeError> {
        let Some(builtin) = builtins::lookup(callee) else {
            return Ok(None);
        };
        (builtin.eval)(self, args, line, column).map(Some)
    }

    /// The teaching mark-and-sweep collector behind the `collect()` builtin.
    ///
    /// **Mark**: every binding in every live scope ([`Environment::iter_values`])
    /// is a GC root; `mark_value` chases each root recursively — through array
    /// elements, instance fields, and the heap slots that marked pointers refer
    /// to (a heap slot can itself hold a pointer/array/instance, so reachability
    /// cascades). **Sweep**: every still-allocated heap slot whose index was
    /// never marked is freed exactly as `free` would (`None`), and the freed-slot
    /// count is returned so programs can print what a collection reclaimed.
    ///
    /// Container values are tracked by `Rc` identity during the walk so a
    /// self-referencing array/instance can't recurse forever.
    fn collect_garbage(&mut self) -> i64 {
        let mut marked: std::collections::HashSet<usize> = std::collections::HashSet::new();
        let mut seen_containers: std::collections::HashSet<*const ()> =
            std::collections::HashSet::new();
        let roots: Vec<Value> = self.env.iter_values().cloned().collect();
        for root in &roots {
            self.mark_value(root, &mut marked, &mut seen_containers);
        }
        let mut freed = 0;
        for (idx, slot) in self.heap.iter_mut().enumerate() {
            if slot.is_some() && !marked.contains(&idx) {
                *slot = None;
                freed += 1;
            }
        }
        freed
    }

    /// Marks every heap slot reachable from `value` (see [`Self::collect_garbage`]).
    /// `marked` is the set of reachable heap indices; `seen` records visited
    /// array/instance `Rc`s (by pointer identity) so cyclic structures terminate.
    fn mark_value(
        &self,
        value: &Value,
        marked: &mut std::collections::HashSet<usize>,
        seen: &mut std::collections::HashSet<*const ()>,
    ) {
        match value {
            Value::Pointer(idx) => {
                if marked.insert(*idx) {
                    if let Some(Some(pointee)) = self.heap.get(*idx) {
                        self.mark_value(&pointee.clone(), marked, seen);
                    }
                }
            }
            Value::Array(elements) => {
                if seen.insert(std::rc::Rc::as_ptr(elements) as *const ()) {
                    for elem in elements.borrow().iter() {
                        self.mark_value(elem, marked, seen);
                    }
                }
            }
            Value::Instance(fields, _) => {
                if seen.insert(std::rc::Rc::as_ptr(fields) as *const ()) {
                    for field in fields.borrow().values() {
                        self.mark_value(field, marked, seen);
                    }
                }
            }
            _ => {}
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
