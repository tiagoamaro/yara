//! Tree-walk evaluator executing a typechecked AST.

use crate::ast::{BinOp, Expr, Stmt, UnOp};
use crate::builtins;
use crate::env::Environment;
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

/// One entry in a `RuntimeError`'s call-stack trace: which function/method
/// call was active, and the source position of the call site (not the
/// position inside the callee). `call_function`/`run_method`/`construct`
/// push one of these before running a body and pop it after, so at the
/// moment an error is actually raised `self.call_stack` holds the full chain
/// of calls that led there, outermost first.
#[derive(Debug, Clone)]
pub struct StackFrame {
    pub function_name: String,
    pub line: usize,
    pub column: usize,
}

/// A runtime failure (e.g. division by zero, undefined variable, wrong
/// argument type slipping past the typechecker). Carries the position of the
/// failing operation plus a snapshot of `Interpreter::call_stack` at the
/// moment it was constructed, so the top-level error reporter can print a
/// full trace back through every enclosing function/method call.
#[derive(Debug, Clone)]
pub struct RuntimeError {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub call_stack: Vec<StackFrame>,
}

impl fmt::Display for RuntimeError {
    /// Renders a rustc-style multi-line trace: the error message, then the
    /// exact `line:column` where it occurred, then each `StackFrame` in
    /// `call_stack` reversed (innermost call first) so the trace reads
    /// top-to-bottom as "here's where it broke, here's who called that,
    /// here's who called that, ..." — matching how the frames were pushed
    /// (outermost first) but printed in the opposite order.
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

impl crate::diagnostics::Diagnostic for RuntimeError {
    fn kind(&self) -> &str {
        "runtime error"
    }
    fn message(&self) -> &str {
        &self.message
    }
    fn span(&self) -> crate::diagnostics::Span {
        crate::diagnostics::Span::new(self.line, self.column)
    }
    /// The call stack is pushed outermost-first; the trace prints innermost
    /// first, so reverse it here (same order the CLI produced before this trait
    /// existed).
    fn frames(&self) -> Vec<crate::diagnostics::Frame> {
        self.call_stack
            .iter()
            .rev()
            .map(|f| crate::diagnostics::Frame {
                name: f.function_name.clone(),
                span: crate::diagnostics::Span::new(f.line, f.column),
            })
            .collect()
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
pub struct Interpreter {
    /// Lexical scope stack mapping in-scope names to their runtime `Value`
    /// (see [`Environment`]); the typechecker uses the same structure over
    /// `Type` at check time.
    env: Environment<Value>,
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
            env: Environment::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
            call_stack: Vec::new(),
        }
    }

    /// Runs a whole program in two passes, mirroring how the typechecker
    /// resolves forward references. Pass 1: walk every top-level statement
    /// and register `Stmt::FunctionDef`/`Stmt::ClassDef` into `self.functions`
    /// / `self.classes` without executing anything else — this is why a
    /// function can call another function defined later in the same file, or
    /// a class can reference itself. Pass 2: execute each top-level statement
    /// in source order (function/class defs are no-ops the second time
    /// around, see their `exec_stmt` arms).
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

    /// Unconditionally inserts `name` into the *innermost* (last) scope,
    /// creating a brand-new binding there even if an outer scope already has
    /// a variable with the same name (which would then be shadowed for the
    /// rest of this scope's lifetime). Used for parameter binding, loop
    /// variables, and anywhere the language semantics say "this is a fresh
    /// local," as opposed to `set_var`'s "find and mutate" behavior.
    fn declare_var(&mut self, name: &str, value: Value) {
        self.env.declare(name, value);
    }

    /// Implements assignment (`x = value`, incl. `x = x + 1`). Walks the
    /// scope stack from innermost to outermost looking for an *existing*
    /// binding named `name`; if found, mutates it in place. This is what lets
    /// `while x < 5 { x = x + 1 }` actually increment the `x` declared
    /// outside the loop body's scope, rather than each loop iteration
    /// silently creating a new `x` local to that iteration and leaving the
    /// outer `x` untouched forever. Only if no existing binding is found
    /// anywhere on the stack does it fall back to `declare_var`, creating a
    /// brand-new variable in the current (innermost) scope — this is how a
    /// plain `x = 1` first introduces `x`.
    fn set_var(&mut self, name: &str, value: Value) {
        self.env.set_or_declare(name, value);
    }

    /// Reads a variable by walking the scope stack innermost-first, so a
    /// local shadows an outer variable of the same name. Returns `None` if
    /// no scope on the stack has bound `name` (the caller turns that into an
    /// "undefined variable" `RuntimeError`).
    fn lookup_var(&self, name: &str) -> Option<&Value> {
        self.env.lookup(name)
    }

    /// Pushes a fresh, empty scope onto the stack. Called around function
    /// bodies and `for` loop bodies to give them their own local namespace;
    /// `if`/`while`/`elsif`/`else` deliberately do *not* push a scope, so
    /// variables assigned inside them are visible (and, via `set_var`,
    /// mutate outer bindings) after the block ends — matching typical
    /// Ruby-like block scoping rather than C-style brace scoping. Delegates to
    /// [`Environment::push_scope`].
    fn push_scope(&mut self) {
        self.env.push_scope();
    }

    /// Discards the innermost scope and everything declared in it. Must be
    /// paired with every `push_scope` — callers are responsible for calling
    /// this on every exit path (including early `return`/error propagation),
    /// which is why sites like `Stmt::For` and `call_function` explicitly
    /// pop in each branch of a `match` rather than relying on RAII.
    fn pop_scope(&mut self) {
        self.env.pop_scope();
    }

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

    /// Runs a sequence of statements (an `if`/`while`/`for` body) one after
    /// another, stopping early and propagating `Flow::Return` up to the
    /// caller the moment any statement produces one — this is the
    /// "threading" half of the `Flow` mechanism described on `exec_stmt`:
    /// a nested block never swallows a `return`, it just forwards it.
    fn exec_block(&mut self, body: &[Stmt]) -> Result<Flow, RuntimeError> {
        for stmt in body {
            match self.exec_stmt(stmt)? {
                Flow::Normal => {}
                flow @ Flow::Return(_) => return Ok(flow),
            }
        }
        Ok(Flow::Normal)
    }

    /// Evaluates `expr` and requires the result to be a `Value::Boolean`
    /// (used for `if`/`elsif`/`while` conditions); any other runtime value
    /// is a `RuntimeError` — the typechecker should already have rejected a
    /// non-Boolean condition, so this is a defense-in-depth check.
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

    /// Evaluates `expr` and requires the result to be a `Value::Integer`
    /// (used for `for` loop range bounds and array indices); any other value
    /// is a `RuntimeError` at the given `line`/`column` (the call site's
    /// position, since `expr` itself may be a sub-expression without its own
    /// obviously-relevant position for the error message).
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

    /// The other half of the recursive-evaluation loop alongside
    /// `exec_stmt`: recursively evaluates an `Expr` down to a runtime
    /// `Value`. Every compound expression (`Binary`, `Call`, `Index`,
    /// `FieldAccess`, `MethodCall`, ...) works by recursively calling
    /// `eval_expr` on its sub-expressions first, then combining the
    /// resulting `Value`s — there's no separate "compile" step, each
    /// expression is evaluated directly against the tree on every visit.
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

    /// Dispatches `object_val.method(args)` to the method declared on
    /// `object_val`'s class (looked up by the class name stored alongside
    /// the instance's fields in `Value::Instance`), then delegates to
    /// `run_method` to actually execute it. Errors if `object_val` isn't an
    /// instance at all, or if its class has no method of that name — no
    /// inheritance, so only the exact class name recorded at construction
    /// time is consulted.
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

    /// Bounds-checked read of `array_val[idx]`, shared by `Expr::Index`
    /// (`arr[i]`) and the `get` builtin. Negative or out-of-range indices
    /// produce a `RuntimeError` naming the offending index and the array's
    /// actual length, rather than panicking — `usize::try_from` rejects
    /// negative `idx` up front, and `Vec::get` handles the too-large case.
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

    /// Runs `src` through the *typechecker and then* the interpreter. The
    /// typechecker's tail-expression logic (`check_tail_stmt`) and the
    /// interpreter's (`exec_tail_stmt`) must stay in lockstep — the docs warn
    /// that if they diverge, return-value bugs slip past type checking. This
    /// helper exercises both, so a divergence surfaces either as a type error
    /// here or as a wrong computed value in the assertions below.
    fn run_checked(src: &str) -> Interpreter {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let program = Parser::new(tokens).parse_program().unwrap();
        crate::typechecker::TypeChecker::new()
            .check_program(&program)
            .expect("program should type-check");
        let mut interp = Interpreter::new();
        interp.run_program(&program).expect("program should run");
        interp
    }

    /// A trailing `if`/`elsif`/`else` used as a function's implicit return value
    /// must type-check *and* evaluate to the same thing across every branch
    /// shape: plain if/else, an `elsif` chain, a nested tail `if`, and recursion
    /// through the tail. Guards the typechecker↔interpreter tail-expr agreement.
    #[test]
    fn tail_if_return_value_agrees_across_stages() {
        let interp = run_checked(
            "def pick(n: Int): Int\n  if n < 0\n    100\n  else\n    1\n  end\nend\nr = pick(-5)\n",
        );
        assert_eq!(interp.lookup_var("r"), Some(&Value::Integer(100)));

        let interp = run_checked(
            "def grade(n: Int): Int\n  if n < 1\n    0\n  elsif n < 2\n    1\n  else\n    2\n  end\nend\ng = grade(1)\n",
        );
        assert_eq!(interp.lookup_var("g"), Some(&Value::Integer(1)));

        let interp = run_checked(
            "def f(n: Int): Int\n  if n < 0\n    0\n  else\n    if n < 10\n      1\n    else\n      2\n    end\n  end\nend\nx = f(5)\n",
        );
        assert_eq!(interp.lookup_var("x"), Some(&Value::Integer(1)));

        let interp = run_checked(
            "def fact(n: Int): Int\n  if n <= 1\n    1\n  else\n    n * fact(n - 1)\n  end\nend\nf = fact(6)\n",
        );
        assert_eq!(interp.lookup_var("f"), Some(&Value::Integer(720)));
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
