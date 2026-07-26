//! Tree-walk evaluator executing a typechecked AST.

use crate::ast::{BinOp, Expr, Stmt, UnOp};
use crate::builtins;
use crate::env::Environment;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

mod calls;
mod classes;
mod expressions;
mod methods;
mod statements;

pub(crate) use calls::{
    eval_alloc, eval_collect, eval_deref, eval_free, eval_get, eval_len, eval_pop, eval_push,
    eval_set, eval_set_deref,
};
pub(crate) use methods::{
    eval_array_get, eval_array_is_empty, eval_array_pop, eval_array_push, eval_array_set,
    eval_array_size, eval_bool_to_s, eval_float_abs, eval_float_to_i, eval_float_to_s,
    eval_int_abs, eval_int_to_f, eval_int_to_s, eval_ptr_deref, eval_ptr_free, eval_ptr_set_deref,
    eval_string_is_empty, eval_string_lower, eval_string_size, eval_string_to_f, eval_string_to_i,
    eval_string_to_s, eval_string_trim, eval_string_upper,
};

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
    /// An opt-in manual-memory pointer: an index into `Interpreter::heap`, not a Rust reference.
    /// `free` empties the slot; `deref`/`set_deref` on a freed slot is a `RuntimeError` (visible use-after-free — the teaching point).
    Pointer(usize),
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
            Value::Pointer(idx) => write!(f, "ptr#{idx}"),
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
    /// The modeled heap for `alloc`/`deref`/`set_deref`/`free`: one slot per allocation; `None` marks a freed slot (slots are never reused, so a stale pointer reliably reports use-after-free instead of aliasing a new allocation).
    heap: Vec<Option<Value>>,
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
            heap: Vec::new(),
        }
    }

    /// Runs a whole program in three passes, mirroring how the typechecker
    /// resolves forward references. Pass 1: walk every top-level statement
    /// and register `Stmt::FunctionDef`/`Stmt::ClassDef` into `self.functions`
    /// / `self.classes` without executing anything else — this is why a
    /// function can call another function defined later in the same file, or
    /// a class can reference itself. Pass 2: flatten class inheritance by
    /// merging parent fields, consts, and methods into each child (the
    /// typechecker validates unknown parents and cycles before the interpreter
    /// runs, so we can safely assume all parent references are valid here).
    /// Pass 3: execute each top-level statement in source order (function/class
    /// defs are no-ops the second time around, see their `exec_stmt` arms).
    pub fn run_program(&mut self, program: &[Stmt]) -> Result<(), RuntimeError> {
        // Pass 1: register functions and unflattened classes
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

        // Pass 2: flatten class inheritance
        self.flatten_classes(program)?;

        // Pass 3: execute statements
        for stmt in program {
            self.exec_stmt(stmt)?;
        }
        Ok(())
    }

    /// Flattens class inheritance by merging parent fields, consts, and methods
    /// into each child class. Assumes the typechecker has already validated that
    /// all parent references are valid and no cycles exist (typechecker runs
    /// before interpreter in the main pipeline).
    fn flatten_classes(&mut self, program: &[Stmt]) -> Result<(), RuntimeError> {
        // Build a map of class name -> parent name (or None if no parent)
        let parent_map: std::collections::HashMap<String, Option<String>> = program
            .iter()
            .filter_map(|stmt| {
                if let Stmt::ClassDef { name, parent, .. } = stmt {
                    Some((name.clone(), parent.clone()))
                } else {
                    None
                }
            })
            .collect();

        // Flatten each class by recursively merging its parent's fields/consts/methods
        let mut flattened = std::collections::HashMap::new();
        let mut visited = std::collections::HashSet::new();

        for class_name in parent_map.keys() {
            self.flatten_class_recursive(class_name, &parent_map, &mut flattened, &mut visited);
        }

        self.classes = flattened;
        Ok(())
    }

    /// Recursively flattens a single class by visiting its parent first,
    /// then merging the parent's flattened fields/consts/methods into the child.
    fn flatten_class_recursive(
        &self,
        class_name: &str,
        parent_map: &std::collections::HashMap<String, Option<String>>,
        flattened: &mut std::collections::HashMap<String, ClassDecl>,
        visited: &mut std::collections::HashSet<String>,
    ) {
        // Avoid infinite recursion on cycles (though typechecker should have
        // already ruled these out)
        if visited.contains(class_name) {
            return;
        }
        visited.insert(class_name.to_string());

        // If this class has a parent, flatten the parent first
        if let Some(Some(parent_name)) = parent_map.get(class_name) {
            self.flatten_class_recursive(parent_name, parent_map, flattened, visited);
        }

        // Now flatten this class by merging parent (if any) into it
        let mut class_decl = self.classes[class_name].clone();

        if let Some(Some(parent_name)) = parent_map.get(class_name) {
            // Parent has already been flattened, grab it from flattened map
            if let Some(parent_decl) = flattened.get(parent_name) {
                // Prepend parent's field_names to child's field_names
                // (parent fields should come first so they're initialized first)
                let mut merged_field_names = parent_decl.field_names.clone();
                merged_field_names.extend(class_decl.field_names);
                class_decl.field_names = merged_field_names;

                // Prepend parent's const_inits to child's const_inits
                // (parent consts evaluate first, child wins if names clash)
                let mut merged_const_inits = parent_decl.const_inits.clone();
                merged_const_inits.extend(class_decl.const_inits);
                class_decl.const_inits = merged_const_inits;

                // Merge parent's methods into child's methods
                // (child methods override parent methods of the same name)
                let mut merged_methods = parent_decl.methods.clone();
                for (method_name, method_decl) in class_decl.methods {
                    merged_methods.insert(method_name, method_decl);
                }
                class_decl.methods = merged_methods;
            }
        }

        flattened.insert(class_name.to_string(), class_decl);
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

    /// Dereferencing a `nil` pointer is a runtime error naming the mistake,
    /// and pointer-vs-nil equality evaluates sanely at runtime.
    #[test]
    fn nil_pointer_deref_is_runtime_error() {
        let err = run("p: Ptr<Integer> = nil\nderef(p)").unwrap_err();
        assert!(err.message.contains("nil pointer dereference"));
        let interp =
            run("p: Ptr<Integer> = nil\nq: Ptr<Integer> = alloc(1)\na = p == nil\nb = q == nil")
                .unwrap();
        assert_eq!(interp.lookup_var("a"), Some(&Value::Boolean(true)));
        assert_eq!(interp.lookup_var("b"), Some(&Value::Boolean(false)));
    }

    /// `collect()` frees an allocation whose pointer went out of scope (a
    /// leak) but keeps one still bound in a live scope, and reports exactly
    /// one freed slot.
    #[test]
    fn collect_frees_unreachable_keeps_reachable() {
        let interp = run(
            "def leak()\n  p: Ptr<Integer> = alloc(1)\nend\nleak()\nkept: Ptr<Integer> = alloc(2)\nn: Integer = collect()\nv: Integer = deref(kept)",
        )
        .unwrap();
        assert_eq!(interp.lookup_var("n"), Some(&Value::Integer(1)));
        assert_eq!(interp.lookup_var("v"), Some(&Value::Integer(2)));
    }

    /// A pointer reachable only *through* a container (an array element) is
    /// still a live root — `collect()` must not free its pointee.
    #[test]
    fn collect_traces_pointers_inside_arrays() {
        let interp = run(
            "def make(): Ptr<Integer>\n  alloc(7)\nend\nps: IntArray = []\nq: Ptr<Integer> = make()\nn: Integer = collect()\nv: Integer = deref(q)",
        )
        .unwrap();
        assert_eq!(interp.lookup_var("n"), Some(&Value::Integer(0)));
        assert_eq!(interp.lookup_var("v"), Some(&Value::Integer(7)));
    }

    /// A heap slot can itself hold a pointer: marking must cascade through
    /// pointees, so a chain reachable only via its head stays fully alive.
    #[test]
    fn collect_cascades_through_heap_slots() {
        let interp = run(
            "inner: Ptr<Integer> = alloc(9)\nouter: Ptr<Ptr<Integer>> = alloc(inner)\ninner = alloc(0)\nfree(inner)\nn: Integer = collect()\nv: Integer = deref(deref(outer))",
        )
        .unwrap();
        assert_eq!(interp.lookup_var("n"), Some(&Value::Integer(0)));
        assert_eq!(interp.lookup_var("v"), Some(&Value::Integer(9)));
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

    #[test]
    fn pointer_alloc_deref_roundtrip() {
        let interp = run(
            "p: Ptr<Integer> = alloc(5)\nx: Integer = deref(p)\nset_deref(p, 9)\ny: Integer = deref(p)",
        )
        .unwrap();
        assert_eq!(interp.lookup_var("x"), Some(&Value::Integer(5)));
        assert_eq!(interp.lookup_var("y"), Some(&Value::Integer(9)));
    }

    #[test]
    fn pointer_use_after_free_errors() {
        let err = run("p: Ptr<Integer> = alloc(5)\nfree(p)\nderef(p)").unwrap_err();
        assert!(err.message.contains("use after free"));
    }

    #[test]
    fn pointer_double_free_errors() {
        let err = run("p: Ptr<Integer> = alloc(5)\nfree(p)\nfree(p)").unwrap_err();
        assert!(err.message.contains("double free"));
    }
}
