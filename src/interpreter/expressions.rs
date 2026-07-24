use super::*;

impl Interpreter {
    /// Evaluates `expr` and requires the result to be a `Value::Boolean`
    /// (used for `if`/`elsif`/`while` conditions); any other runtime value
    /// is a `RuntimeError` — the typechecker should already have rejected a
    /// non-Boolean condition, so this is a defense-in-depth check.
    pub(super) fn eval_bool(&mut self, expr: &Expr) -> Result<bool, RuntimeError> {
        match self.eval_expr(expr)? {
            Value::Boolean(b) => Ok(b),
            other => Err(RuntimeError {
                message: format!("expected Boolean condition, found `{other}`"),
                line: expr.line(),
                column: expr.column(),
                call_stack: self.call_stack.clone(),
            }),
        }
    }

    /// Evaluates `expr` and requires the result to be a `Value::Integer`
    /// (used for `for` loop range bounds and array indices); any other value
    /// is a `RuntimeError` at the given `line`/`column` (the call site's
    /// position, since `expr` itself may be a sub-expression without its own
    /// obviously-relevant position for the error message).
    pub(super) fn eval_int(
        &mut self,
        expr: &Expr,
        line: usize,
        column: usize,
    ) -> Result<i64, RuntimeError> {
        match self.eval_expr(expr)? {
            Value::Integer(i) => Ok(i),
            other => Err(RuntimeError {
                message: format!("expected Integer, found `{other}`"),
                line,
                column,
                call_stack: self.call_stack.clone(),
            }),
        }
    }

    /// The other half of the recursive-evaluation loop alongside
    /// `exec_stmt`: recursively evaluates an `Expr` down to a runtime
    /// `Value`. Every compound expression (`Binary`, `Call`, `Index`,
    /// `FieldAccess`, `MethodCall`, ...) works by recursively calling
    /// `eval_expr` on its sub-expressions first, then combining the
    /// resulting `Value`s — there's no separate "compile" step, each
    /// expression is evaluated directly against the tree on every visit.
    pub(super) fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            Expr::IntLit { value, .. } => Ok(Value::Integer(*value)),
            Expr::FloatLit { value, .. } => Ok(Value::Float(*value)),
            Expr::StringLit { value, .. } => Ok(Value::String(value.clone())),
            Expr::BoolLit { value, .. } => Ok(Value::Boolean(*value)),
            Expr::NilLit { .. } => Ok(Value::Nil),
            Expr::Ident { name, line, column } => {
                self.lookup_var(name).cloned().ok_or_else(|| RuntimeError {
                    message: format!("undefined variable `{name}`"),
                    line: *line,
                    column: *column,
                    call_stack: self.call_stack.clone(),
                })
            }
            Expr::Binary {
                op,
                left,
                right,
                line,
                column,
            } => {
                let l = self.eval_expr(left)?;
                let r = self.eval_expr(right)?;
                self.eval_binary_op(*op, l, r, *line, *column)
            }
            Expr::Call {
                callee,
                args,
                line,
                column,
            } => self.call_function(callee, args, *line, *column),
            // Unary negation (`-x`): only `Integer`/`Float` can be negated.
            // Anything else is a `RuntimeError` — the typechecker should
            // already reject a non-numeric operand, so this arm is
            // defense-in-depth rather than the primary check.
            Expr::Unary {
                op: UnOp::Neg,
                expr,
                line,
                column,
            } => match self.eval_expr(expr)? {
                Value::Integer(i) => Ok(Value::Integer(-i)),
                Value::Float(f) => Ok(Value::Float(-f)),
                other => Err(RuntimeError {
                    message: format!("cannot negate `{other}`"),
                    line: *line,
                    column: *column,
                    call_stack: self.call_stack.clone(),
                }),
            },
            Expr::ArrayLit { elements, .. } => {
                let mut values = Vec::with_capacity(elements.len());
                for e in elements {
                    values.push(self.eval_expr(e)?);
                }
                Ok(Value::Array(Rc::new(RefCell::new(values))))
            }
            Expr::Index {
                array,
                index,
                line,
                column,
            } => {
                let array_val = self.eval_expr(array)?;
                let idx = self.eval_int(index, *line, *column)?;
                self.array_get(&array_val, idx, *line, *column)
            }
            Expr::FieldAccess {
                object,
                field,
                line,
                column,
            } => {
                let object_val = self.eval_expr(object)?;
                let Value::Instance(fields, class_name) = &object_val else {
                    return Err(RuntimeError {
                        message: format!("cannot access field `{field}` on a non-object value"),
                        line: *line,
                        column: *column,
                        call_stack: self.call_stack.clone(),
                    });
                };
                let found = fields.borrow().get(field).cloned();
                found.ok_or_else(|| RuntimeError {
                    message: format!("class `{class_name}` has no field `{field}`"),
                    line: *line,
                    column: *column,
                    call_stack: self.call_stack.clone(),
                })
            }
            Expr::MethodCall {
                object,
                method,
                args,
                line,
                column,
            } => {
                if let Expr::Ident { name, .. } = &**object {
                    if self.lookup_var(name).is_none() && self.classes.contains_key(name) {
                        return self.construct(name, args, *line, *column);
                    }
                }
                let object_val = self.eval_expr(object)?;
                self.call_method(object_val, method, args, *line, *column)
            }
        }
    }

    /// Bounds-checked read of `array_val[idx]`, shared by `Expr::Index`
    /// (`arr[i]`) and the `get` builtin. Negative or out-of-range indices
    /// produce a `RuntimeError` naming the offending index and the array's
    /// actual length, rather than panicking — `usize::try_from` rejects
    /// negative `idx` up front, and `Vec::get` handles the too-large case.
    pub(super) fn array_get(
        &self,
        array_val: &Value,
        idx: i64,
        line: usize,
        column: usize,
    ) -> Result<Value, RuntimeError> {
        match array_val {
            Value::Array(items) => {
                let items = items.borrow();
                usize::try_from(idx)
                    .ok()
                    .and_then(|i| items.get(i).cloned())
                    .ok_or_else(|| RuntimeError {
                        message: format!(
                            "array index {idx} out of bounds (length {})",
                            items.len()
                        ),
                        line,
                        column,
                        call_stack: self.call_stack.clone(),
                    })
            }
            other => Err(RuntimeError {
                message: format!("cannot index into `{other}`"),
                line,
                column,
                call_stack: self.call_stack.clone(),
            }),
        }
    }

    /// Implements the actual runtime semantics of each `BinOp` once both
    /// operands have already been evaluated to `Value`s. Notable cases:
    /// - `Add` also handles `String + String` as concatenation (`a + &b`),
    ///   not just numeric addition.
    /// - `Div` on two `Integer`s explicitly checks for a zero divisor first
    ///   and raises "division by zero" as a `RuntimeError` instead of
    ///   letting Rust's integer division panic; float division by zero is
    ///   *not* checked here and silently produces IEEE 754 `inf`/`NaN`.
    /// - `Eq`/`NotEq` just defer to `Value`'s derived `PartialEq` — works
    ///   for any pair of values, including comparing across different
    ///   variants (always `false`/`true` respectively).
    /// - `Lt`/`Gt`/`LtEq`/`GtEq` require both operands to be the same
    ///   numeric type (`Integer`/`Integer` or `Float`/`Float`) and use
    ///   `partial_cmp`, erroring on anything else (including mixed
    ///   Integer/Float, which the typechecker should already reject).
    pub(super) fn eval_binary_op(
        &self,
        op: BinOp,
        left: Value,
        right: Value,
        line: usize,
        column: usize,
    ) -> Result<Value, RuntimeError> {
        let err = |message: String| RuntimeError {
            message,
            line,
            column,
            call_stack: self.call_stack.clone(),
        };
        match op {
            BinOp::Add => match (left, right) {
                (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a + b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
                (Value::String(a), Value::String(b)) => Ok(Value::String(a + &b)),
                (a, b) => Err(err(format!("cannot add `{a}` and `{b}`"))),
            },
            BinOp::Sub => match (left, right) {
                (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a - b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
                (a, b) => Err(err(format!("cannot subtract `{b}` from `{a}`"))),
            },
            BinOp::Mul => match (left, right) {
                (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a * b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
                (a, b) => Err(err(format!("cannot multiply `{a}` and `{b}`"))),
            },
            BinOp::Div => match (left, right) {
                (Value::Integer(_), Value::Integer(0)) => Err(err("division by zero".to_string())),
                (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a / b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
                (a, b) => Err(err(format!("cannot divide `{a}` by `{b}`"))),
            },
            BinOp::Eq => Ok(Value::Boolean(left == right)),
            BinOp::NotEq => Ok(Value::Boolean(left != right)),
            BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => {
                let ord = match (&left, &right) {
                    (Value::Integer(a), Value::Integer(b)) => a.partial_cmp(b),
                    (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
                    _ => return Err(err(format!("cannot compare `{left}` and `{right}`"))),
                };
                let Some(ord) = ord else {
                    return Err(err(format!("cannot compare `{left}` and `{right}`")));
                };
                let result = match op {
                    BinOp::Lt => ord.is_lt(),
                    BinOp::Gt => ord.is_gt(),
                    BinOp::LtEq => ord.is_le(),
                    BinOp::GtEq => ord.is_ge(),
                    _ => unreachable!(),
                };
                Ok(Value::Boolean(result))
            }
        }
    }
}
