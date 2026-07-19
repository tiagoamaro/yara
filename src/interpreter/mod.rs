//! Tree-walk evaluator executing a typechecked AST.

use crate::ast::{BinOp, Expr, Stmt, UnOp};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    String(String),
    Nil,
    /// Reference semantics (like Python lists): cloning a `Value::Array` shares
    /// the same backing storage, so passing an array into a function and
    /// mutating it there (`push`/`set`) is visible to the caller — needed for
    /// arena-style linked lists/trees/graphs built out of arrays of indices.
    Array(Rc<RefCell<Vec<Value>>>),
    /// A `class` instance: field name -> value (consts and instance vars
    /// share this one map), plus the class name for method dispatch.
    /// Reference semantics, same rationale as `Array`.
    Instance(Rc<RefCell<HashMap<String, Value>>>, String),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Integer(v) => write!(f, "{v}"),
            Value::Float(v) => write!(f, "{v}"),
            Value::Boolean(v) => write!(f, "{v}"),
            Value::String(v) => write!(f, "{v}"),
            Value::Nil => write!(f, "nil"),
            Value::Array(items) => {
                write!(f, "[")?;
                for (i, item) in items.borrow().iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            Value::Instance(_, class_name) => write!(f, "#<{class_name}>"),
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

#[derive(Debug, Clone)]
struct ClassDecl {
    /// `(name, value_expr)` for each class const, evaluated once per `new`.
    const_inits: Vec<(String, Expr)>,
    /// Instance-var names declared with no value (start out `Nil`).
    field_names: Vec<String>,
    methods: HashMap<String, FunctionDecl>,
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
    classes: HashMap<String, ClassDecl>,
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
            classes: HashMap::new(),
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
            if let Stmt::ClassDef {
                name,
                consts,
                fields,
                methods,
                ..
            } = stmt
            {
                let const_inits = consts
                    .iter()
                    .filter_map(|c| match c {
                        Stmt::ConstDecl { name, value, .. } => Some((name.clone(), value.clone())),
                        _ => None,
                    })
                    .collect();
                let field_names = fields.iter().map(|f| f.name.clone()).collect();
                let method_decls = methods
                    .iter()
                    .filter_map(|m| match m {
                        Stmt::FunctionDef {
                            name, params, body, ..
                        } => Some((
                            name.clone(),
                            FunctionDecl {
                                params: params.iter().map(|p| p.name.clone()).collect(),
                                body: body.clone(),
                            },
                        )),
                        _ => None,
                    })
                    .collect();
                self.classes.insert(
                    name.clone(),
                    ClassDecl {
                        const_inits,
                        field_names,
                        methods: method_decls,
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

    /// `ClassName.new(args)`: builds a fresh instance, evaluates each class
    /// const in declaration order (so a later const may reference an earlier
    /// one), zero-initializes declared fields to `Nil`, then runs
    /// `initializer` (if the class has one) with `args` bound to its params.
    fn construct(
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

    fn call_method(
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
    fn run_method(
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
            let current_scope = &self.scopes.last().unwrap().vars;
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

    fn array_get(
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

    /// Evaluates `len`/`push`/`get`/`set` if `callee` names one of them,
    /// returning `Ok(None)` for any other callee so `call_function` falls
    /// through to a user-defined function lookup.
    fn call_array_builtin(
        &mut self,
        callee: &str,
        args: &[Expr],
        line: usize,
        column: usize,
    ) -> Result<Option<Value>, RuntimeError> {
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
            _ => Ok(None),
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

    /// Executes a function body with Ruby-style implicit last-expression return:
    /// if the body doesn't hit an explicit `return`, the value of a trailing
    /// `ExprStmt` (or trailing `if`/`elsif`/`else`, recursively) becomes the
    /// call's result. Mirrors `typechecker::check_body_return_type`.
    fn exec_function_body(&mut self, body: &[Stmt]) -> Result<Value, RuntimeError> {
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

    fn exec_tail_stmt(&mut self, stmt: &Stmt) -> Result<Value, RuntimeError> {
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

    #[test]
    fn unary_negation() {
        let interp = run("x = -5\ny = -1.5").unwrap();
        assert_eq!(interp.lookup_var("x"), Some(&Value::Integer(-5)));
        assert_eq!(interp.lookup_var("y"), Some(&Value::Float(-1.5)));
    }

    #[test]
    fn recursive_function_with_if_else_tail() {
        let interp =
            run("def fact(n: Int): Int\n  if n <= 1\n    1\n  else\n    n * fact(n - 1)\n  end\nend\nx = fact(5)")
                .unwrap();
        assert_eq!(interp.lookup_var("x"), Some(&Value::Integer(120)));
    }

    #[test]
    fn array_index_and_builtins() {
        let interp = run(
            "xs: IntArray = [1, 2, 3]\npush(xs, 4)\nn = len(xs)\nfirst = get(xs, 0)\nset(xs, 0, 99)\nsecond = xs[1]",
        )
        .unwrap();
        assert_eq!(interp.lookup_var("n"), Some(&Value::Integer(4)));
        assert_eq!(interp.lookup_var("first"), Some(&Value::Integer(1)));
        assert_eq!(interp.lookup_var("second"), Some(&Value::Integer(2)));
        match interp.lookup_var("xs") {
            Some(Value::Array(items)) => {
                assert_eq!(
                    *items.borrow(),
                    vec![
                        Value::Integer(99),
                        Value::Integer(2),
                        Value::Integer(3),
                        Value::Integer(4)
                    ]
                );
            }
            other => panic!("expected array, got {other:?}"),
        }
    }

    #[test]
    fn array_mutation_visible_through_function_call() {
        // Arrays have reference semantics: a function that `push`es onto an
        // array parameter mutates the caller's array too.
        let interp = run(
            "def fill(xs: IntArray): Nil\n  push(xs, 1)\n  push(xs, 2)\nend\nxs: IntArray = []\nfill(xs)",
        )
        .unwrap();
        match interp.lookup_var("xs") {
            Some(Value::Array(items)) => {
                assert_eq!(*items.borrow(), vec![Value::Integer(1), Value::Integer(2)]);
            }
            other => panic!("expected array, got {other:?}"),
        }
    }

    #[test]
    fn array_index_out_of_bounds_reports_position() {
        let err = run("xs: IntArray = [1]\ny = xs[5]").unwrap_err();
        assert!(err.message.contains("out of bounds"));
    }

    #[test]
    fn pop_removes_and_returns_last_element() {
        let interp = run("xs: IntArray = [1, 2, 3]\ny = pop(xs)").unwrap();
        assert_eq!(interp.lookup_var("y"), Some(&Value::Integer(3)));
        match interp.lookup_var("xs") {
            Some(Value::Array(items)) => {
                assert_eq!(*items.borrow(), vec![Value::Integer(1), Value::Integer(2)]);
            }
            other => panic!("expected array, got {other:?}"),
        }
    }

    #[test]
    fn pop_from_empty_array_is_runtime_error() {
        let err = run("xs: IntArray = []\ny = pop(xs)").unwrap_err();
        assert!(err.message.contains("empty array"));
    }

    const HELLO_CLASS: &str = "class Hello\n  const PI: Float = 3.14159\n  count: Integer\n\n  def initializer(number: Int)\n    count = number\n  end\n\n  def area(radius: Float): Float\n    PI * radius * radius\n  end\nend\n";

    #[test]
    fn class_construction_and_field_read() {
        let interp = run(&format!(
            "{HELLO_CLASS}h = Hello.new(5)\nx = h.count\ny = h.PI"
        ))
        .unwrap();
        assert_eq!(interp.lookup_var("x"), Some(&Value::Integer(5)));
        assert_eq!(interp.lookup_var("y"), Some(&Value::Float(3.14159)));
    }

    #[test]
    fn class_field_assignment_and_method_call() {
        let interp = run(&format!(
            "{HELLO_CLASS}h = Hello.new(5)\nh.count = 10\nx = h.count\ny = h.area(2.0)"
        ))
        .unwrap();
        assert_eq!(interp.lookup_var("x"), Some(&Value::Integer(10)));
        assert_eq!(interp.lookup_var("y"), Some(&Value::Float(12.56636)));
    }

    #[test]
    fn class_instances_have_reference_semantics() {
        // Mutating a field through one binding is visible through another
        // binding of the same instance (like arrays; see Value::Instance).
        let interp = run(&format!(
            "{HELLO_CLASS}h = Hello.new(1)\nalias = h\nalias.count = 42\nx = h.count"
        ))
        .unwrap();
        assert_eq!(interp.lookup_var("x"), Some(&Value::Integer(42)));
    }
}
