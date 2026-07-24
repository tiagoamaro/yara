use super::*;

impl Interpreter {
    /// Executes one statement and reports how control should continue via
    /// `Flow`: `Flow::Normal` means "fall through to the next statement,"
    /// `Flow::Return(value)` means "an explicit or implicit `return` fired
    /// somewhere in here." Rather than unwinding through Rust panics or a
    /// custom exception type, `return` is threaded *up* the call tree as an
    /// ordinary value: nested constructs (`Stmt::If`'s branches,
    /// `Stmt::While`'s/`Stmt::For`'s loop body via `exec_block`) check the
    /// `Flow` their sub-block produced and, on `Flow::Return`, immediately
    /// re-return it themselves instead of continuing — so a `return` inside
    /// a deeply nested `if` inside a `while` propagates all the way out to
    /// `exec_function_body` without any special control-flow machinery.
    pub(super) fn exec_stmt(&mut self, stmt: &Stmt) -> Result<Flow, RuntimeError> {
        match stmt {
            Stmt::VarDecl { name, value, .. } | Stmt::ConstDecl { name, value, .. } => {
                let v = self.eval_expr(value)?;
                self.set_var(name, v);
                Ok(Flow::Normal)
            }
            Stmt::FunctionDef { .. } => Ok(Flow::Normal),
            Stmt::Return { value, .. } => {
                let v = match value {
                    Some(e) => self.eval_expr(e)?,
                    None => Value::Nil,
                };
                Ok(Flow::Return(v))
            }
            Stmt::If {
                condition,
                then_body,
                elsif_branches,
                else_body,
                ..
            } => {
                if self.eval_bool(condition)? {
                    return self.exec_block(then_body);
                }
                for (cond, body) in elsif_branches {
                    if self.eval_bool(cond)? {
                        return self.exec_block(body);
                    }
                }
                if let Some(body) = else_body {
                    return self.exec_block(body);
                }
                Ok(Flow::Normal)
            }
            Stmt::While {
                condition, body, ..
            } => {
                while self.eval_bool(condition)? {
                    match self.exec_block(body)? {
                        Flow::Normal => {}
                        flow @ Flow::Return(_) => return Ok(flow),
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::For {
                var_name,
                range_start,
                range_end,
                body,
                line,
                column,
            } => {
                let start = self.eval_int(range_start, *line, *column)?;
                let end = self.eval_int(range_end, *line, *column)?;
                self.push_scope();
                for i in start..end {
                    self.declare_var(var_name, Value::Integer(i));
                    match self.exec_block(body) {
                        Ok(Flow::Normal) => {}
                        Ok(flow @ Flow::Return(_)) => {
                            self.pop_scope();
                            return Ok(flow);
                        }
                        Err(e) => {
                            self.pop_scope();
                            return Err(e);
                        }
                    }
                }
                self.pop_scope();
                Ok(Flow::Normal)
            }
            Stmt::ExprStmt(expr) => {
                self.eval_expr(expr)?;
                Ok(Flow::Normal)
            }
            // Resolved away by `resolver` before the interpreter ever sees the program.
            Stmt::Import { .. } => Ok(Flow::Normal),
            // Registered into `self.classes` up front in `run_program`.
            Stmt::ClassDef { .. } => Ok(Flow::Normal),
            Stmt::FieldAssign {
                object,
                field,
                value,
                line,
                column,
            } => {
                let object_val = self.eval_expr(object)?;
                let value_val = self.eval_expr(value)?;
                let Value::Instance(fields, _) = object_val else {
                    return Err(RuntimeError {
                        message: format!("cannot assign field `{field}` on a non-object value"),
                        line: *line,
                        column: *column,
                        call_stack: self.call_stack.clone(),
                    });
                };
                fields.borrow_mut().insert(field.clone(), value_val);
                Ok(Flow::Normal)
            }
        }
    }

    /// Runs a sequence of statements (an `if`/`while`/`for` body) one after
    /// another, stopping early and propagating `Flow::Return` up to the
    /// caller the moment any statement produces one — this is the
    /// "threading" half of the `Flow` mechanism described on `exec_stmt`:
    /// a nested block never swallows a `return`, it just forwards it.
    pub(super) fn exec_block(&mut self, body: &[Stmt]) -> Result<Flow, RuntimeError> {
        for stmt in body {
            match self.exec_stmt(stmt)? {
                Flow::Normal => {}
                flow @ Flow::Return(_) => return Ok(flow),
            }
        }
        Ok(Flow::Normal)
    }

    /// Executes a function body with Ruby-style implicit last-expression return:
    /// if the body doesn't hit an explicit `return`, the value of a trailing
    /// `ExprStmt` (or trailing `if`/`elsif`/`else`, recursively) becomes the
    /// call's result. Mirrors `typechecker::check_body_return_type`.
    pub(super) fn exec_function_body(&mut self, body: &[Stmt]) -> Result<Value, RuntimeError> {
        for (i, stmt) in body.iter().enumerate() {
            let is_last = i == body.len() - 1;
            if is_last {
                return self.exec_tail_stmt(stmt);
            }
            if let Flow::Return(v) = self.exec_stmt(stmt)? {
                return Ok(v);
            }
        }
        Ok(Value::Nil)
    }

    /// Produces the `Value` a function body's *last* statement contributes
    /// as the call's implicit return value, called only for that final
    /// statement by `exec_function_body`. A trailing `Stmt::ExprStmt` simply
    /// evaluates to its expression's value. A trailing `Stmt::If` recurses:
    /// whichever branch's body actually runs (`then`/an `elsif`/`else`) is
    /// itself handed to `exec_function_body` again — not `exec_tail_stmt` —
    /// so that branch's own trailing statement (including another nested
    /// `if`) is resolved the same way, all the way down. Any other kind of
    /// trailing statement (e.g. a `while`/`for`/assignment) falls through to
    /// ordinary `exec_stmt`: an explicit `Flow::Return` still yields its
    /// value, but `Flow::Normal` yields `Value::Nil` (there's no expression
    /// value to take). This logic must stay in lockstep with
    /// `typechecker::check_body_return_type`/`check_tail_stmt`, which
    /// statically predicts the same result.
    pub(super) fn exec_tail_stmt(&mut self, stmt: &Stmt) -> Result<Value, RuntimeError> {
        match stmt {
            Stmt::ExprStmt(expr) => self.eval_expr(expr),
            Stmt::If {
                condition,
                then_body,
                elsif_branches,
                else_body,
                ..
            } => {
                if self.eval_bool(condition)? {
                    return self.exec_function_body(then_body);
                }
                for (cond, body) in elsif_branches {
                    if self.eval_bool(cond)? {
                        return self.exec_function_body(body);
                    }
                }
                match else_body {
                    Some(body) => self.exec_function_body(body),
                    None => Ok(Value::Nil),
                }
            }
            _ => match self.exec_stmt(stmt)? {
                Flow::Return(v) => Ok(v),
                Flow::Normal => Ok(Value::Nil),
            },
        }
    }
}
