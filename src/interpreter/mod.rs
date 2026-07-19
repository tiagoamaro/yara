//! Tree-walk evaluator executing a typechecked AST.

use crate::ast::{BinOp, Expr, Stmt};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    String(String),
    Nil,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Integer(v) => write!(f, "{v}"),
            Value::Float(v) => write!(f, "{v}"),
            Value::Boolean(v) => write!(f, "{v}"),
            Value::String(v) => write!(f, "{v}"),
            Value::Nil => write!(f, "nil"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StackFrame {
    pub function_name: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct RuntimeError {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub call_stack: Vec<StackFrame>,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "error: {}", self.message)?;
        writeln!(f, "  at {}:{}", self.line, self.column)?;
        for frame in self.call_stack.iter().rev() {
            writeln!(
                f,
                "  in `{}` at {}:{}",
                frame.function_name, frame.line, frame.column
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct FunctionDecl {
    params: Vec<String>,
    body: Vec<Stmt>,
}

enum Flow {
    Normal,
    Return(Value),
}

#[derive(Debug)]
struct Scope {
    vars: HashMap<String, Value>,
}

#[derive(Debug)]
pub struct Interpreter {
    scopes: Vec<Scope>,
    functions: HashMap<String, FunctionDecl>,
    call_stack: Vec<StackFrame>,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            scopes: vec![Scope {
                vars: HashMap::new(),
            }],
            functions: HashMap::new(),
            call_stack: Vec::new(),
        }
    }

    pub fn run_program(&mut self, program: &[Stmt]) -> Result<(), RuntimeError> {
        for stmt in program {
            if let Stmt::FunctionDef {
                name, params, body, ..
            } = stmt
            {
                self.functions.insert(
                    name.clone(),
                    FunctionDecl {
                        params: params.iter().map(|p| p.name.clone()).collect(),
                        body: body.clone(),
                    },
                );
            }
        }
        for stmt in program {
            self.exec_stmt(stmt)?;
        }
        Ok(())
    }

    fn declare_var(&mut self, name: &str, value: Value) {
        self.scopes
            .last_mut()
            .unwrap()
            .vars
            .insert(name.to_string(), value);
    }

    fn set_var(&mut self, name: &str, value: Value) {
        for scope in self.scopes.iter_mut().rev() {
            if scope.vars.contains_key(name) {
                scope.vars.insert(name.to_string(), value);
                return;
            }
        }
        self.declare_var(name, value);
    }

    fn lookup_var(&self, name: &str) -> Option<&Value> {
        self.scopes.iter().rev().find_map(|s| s.vars.get(name))
    }

    fn push_scope(&mut self) {
        self.scopes.push(Scope {
            vars: HashMap::new(),
        });
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn exec_stmt(&mut self, stmt: &Stmt) -> Result<Flow, RuntimeError> {
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
        }
    }

    fn exec_block(&mut self, body: &[Stmt]) -> Result<Flow, RuntimeError> {
        for stmt in body {
            match self.exec_stmt(stmt)? {
                Flow::Normal => {}
                flow @ Flow::Return(_) => return Ok(flow),
            }
        }
        Ok(Flow::Normal)
    }

    fn eval_bool(&mut self, expr: &Expr) -> Result<bool, RuntimeError> {
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

    fn eval_int(&mut self, expr: &Expr, line: usize, column: usize) -> Result<i64, RuntimeError> {
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

    fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
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
        }
    }

    fn eval_binary_op(
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

    fn call_function(
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

    /// Executes a function body with Ruby-style implicit last-expression return:
    /// if the body doesn't hit an explicit `return`, the value of a trailing
    /// `ExprStmt` becomes the call's result.
    fn exec_function_body(&mut self, body: &[Stmt]) -> Result<Value, RuntimeError> {
        for (i, stmt) in body.iter().enumerate() {
            let is_last = i == body.len() - 1;
            if is_last {
                if let Stmt::ExprStmt(expr) = stmt {
                    return self.eval_expr(expr);
                }
            }
            if let Flow::Return(v) = self.exec_stmt(stmt)? {
                return Ok(v);
            }
        }
        Ok(Value::Nil)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn run(src: &str) -> Result<Interpreter, RuntimeError> {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let program = Parser::new(tokens).parse_program().unwrap();
        let mut interp = Interpreter::new();
        interp.run_program(&program)?;
        Ok(interp)
    }

    #[test]
    fn evaluates_arithmetic() {
        let interp = run("x = 1 + 2 * 3").unwrap();
        assert_eq!(interp.lookup_var("x"), Some(&Value::Integer(7)));
    }

    #[test]
    fn calls_function_with_return_value() {
        let interp = run("def add(a: Int, b: Int): Int\n  a + b\nend\nresult = add(2, 3)").unwrap();
        assert_eq!(interp.lookup_var("result"), Some(&Value::Integer(5)));
    }

    #[test]
    fn runs_if_else() {
        let interp = run("if 1 > 2\n  x = 1\nelse\n  x = 2\nend").unwrap();
        assert_eq!(interp.lookup_var("x"), Some(&Value::Integer(2)));
    }

    #[test]
    fn runs_while_loop() {
        let interp = run("x = 0\nwhile x < 5\n  x = x + 1\nend").unwrap();
        assert_eq!(interp.lookup_var("x"), Some(&Value::Integer(5)));
    }

    #[test]
    fn runs_for_range() {
        let interp = run("total = 0\nfor i in 0..5\n  total = total + i\nend").unwrap();
        assert_eq!(interp.lookup_var("total"), Some(&Value::Integer(10)));
    }

    #[test]
    fn division_by_zero_reports_position() {
        let err = run("x = 1 / 0").unwrap_err();
        assert!(err.message.contains("division by zero"));
        assert_eq!(err.line, 1);
    }

    #[test]
    fn runtime_error_includes_call_stack() {
        let err = run("def boom(): Int\n  1 / 0\nend\nboom()").unwrap_err();
        assert_eq!(err.call_stack.len(), 1);
        assert_eq!(err.call_stack[0].function_name, "boom");
    }

    #[test]
    fn string_concatenation() {
        let interp = run("x = \"a\" + \"b\"").unwrap();
        assert_eq!(
            interp.lookup_var("x"),
            Some(&Value::String("ab".to_string()))
        );
    }

    #[test]
    fn explicit_return_short_circuits() {
        let interp = run("def f(): Int\n  return 1\n  2\nend\nx = f()").unwrap();
        assert_eq!(interp.lookup_var("x"), Some(&Value::Integer(1)));
    }
}
