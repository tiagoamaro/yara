//! Static type checking pass over the AST.

use crate::ast::{BinOp, Expr, Stmt, UnOp};
use crate::env::Environment;
use std::collections::HashMap;
use std::fmt;

mod calls;
mod classes;
mod exprs;
mod stmts;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Integer,
    Float,
    Boolean,
    String,
    Nil,
    Array(Box<Type>),
    /// An instance of a user-defined `class`, identified by class name.
    Instance(String),
    /// An opt-in manual-memory pointer to a heap cell holding a `T` (see `alloc`/`deref`/`set_deref`/`free`).
    Pointer(Box<Type>),
}

/// The primitive types, each paired with its canonical annotation/display name.
/// This one bijective table is the single source of truth for the primitive
/// name↔`Type` mapping, read in both directions: `from_annotation_name`
/// resolves a name to a `Type`, `Display` renders a `Type` back to its name.
/// Compound types (`Array`, `Instance`, `Pointer`) carry data a flat table can't and are
/// handled separately in each direction.
const PRIMITIVE_TYPES: &[(&str, Type)] = &[
    ("Integer", Type::Integer),
    ("Float", Type::Float),
    ("Boolean", Type::Boolean),
    ("String", Type::String),
    ("Nil", Type::Nil),
];

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Array(elem) => write!(f, "Array<{elem}>"),
            Type::Pointer(elem) => write!(f, "Ptr<{elem}>"),
            Type::Instance(name) => write!(f, "{name}"),
            primitive => {
                let name = PRIMITIVE_TYPES
                    .iter()
                    .find(|(_, ty)| ty == primitive)
                    .map(|(name, _)| *name)
                    .expect("every non-Array/Instance Type is listed in PRIMITIVE_TYPES");
                write!(f, "{name}")
            }
        }
    }
}

impl Type {
    /// Resolves a canonical type-annotation name to a `Type`. Primitives come
    /// straight from [`PRIMITIVE_TYPES`]; the array annotations
    /// (`IntArray`/`FloatArray`/`BoolArray`/`StringArray`) are the only array
    /// type names — there's no generic `Array<T>` syntax, so each element type
    /// gets its own concrete annotation name (Pascal-array style). Pointer
    /// annotations (`Ptr<T>`) are decoded recursively.
    fn from_annotation_name(name: &str) -> Option<Type> {
        if let Some((_, ty)) = PRIMITIVE_TYPES.iter().find(|(n, _)| *n == name) {
            return Some(ty.clone());
        }
        if name.starts_with("Ptr<") && name.ends_with('>') {
            let inner_name = &name[4..name.len() - 1];
            return Type::from_annotation_name(inner_name)
                .map(|inner| Type::Pointer(Box::new(inner)));
        }
        match name {
            "IntArray" => Some(Type::Array(Box::new(Type::Integer))),
            "FloatArray" => Some(Type::Array(Box::new(Type::Float))),
            "BoolArray" => Some(Type::Array(Box::new(Type::Boolean))),
            "StringArray" => Some(Type::Array(Box::new(Type::String))),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl crate::diagnostics::Diagnostic for TypeError {
    fn kind(&self) -> &str {
        "type error"
    }
    fn message(&self) -> &str {
        &self.message
    }
    fn span(&self) -> crate::diagnostics::Span {
        crate::diagnostics::Span::new(self.line, self.column)
    }
}

#[derive(Clone)]
struct FunctionSig {
    param_types: Vec<Type>,
    return_type: Option<Type>,
}

/// A class's field types (instance vars + consts, both accessible unqualified
/// inside methods via implicit `self`) and its method signatures.
#[derive(Clone)]
struct ClassInfo {
    fields: HashMap<String, Type>,
    methods: HashMap<String, FunctionSig>,
}

pub struct TypeChecker {
    /// Lexical scope stack mapping in-scope names to their static `Type`
    /// (see [`Environment`]); the runtime interpreter uses the same structure
    /// over `Value` instead.
    env: Environment<Type>,
    functions: HashMap<String, FunctionSig>,
    classes: HashMap<String, ClassInfo>,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        TypeChecker {
            env: Environment::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
        }
    }

    /// Runs the full type-checking pipeline over a parsed program, in an order
    /// designed so nothing has to be declared before it's used:
    /// 1. `collect_classes` — register every class's fields/consts and method
    ///    signatures (itself a two-pass step, see its own doc comment).
    /// 2. `collect_function_signatures` — register every top-level function's
    ///    param/return types up front, so calls can appear before (or recurse
    ///    into) the function they call regardless of textual order.
    /// 3. `check_classes` — now that all signatures are known, actually walk
    ///    every method body and check it.
    /// 4. Walk the top-level statements in order with `check_stmt`, checking
    ///    ordinary code (and, along the way, top-level function bodies too).
    pub fn check_program(&mut self, program: &[Stmt]) -> Result<(), TypeError> {
        classes::collect_classes(self, program)?;
        calls::collect_function_signatures(self, program)?;
        classes::check_classes(self, program)?;
        for stmt in program {
            stmts::check_stmt(self, stmt)?;
        }
        Ok(())
    }

    /// Turns a `TypeAnnotation`'s bare string name (e.g. `"Integer"`,
    /// `"IntArray"`, or a user-defined class name) into a checker-internal
    /// `Type`. Checks `self.classes` first, so any name matching an already
    /// (or not-yet-fully) registered class resolves to `Type::Instance(name)`
    /// — this is what lets a class name be used as a type annotation exactly
    /// like a builtin one. Otherwise falls back to the fixed builtin/array
    /// names in `Type::from_annotation_name`, and errors as "unknown type"
    /// if the name matches neither (this is also how a typo'd class name in
    /// an annotation gets caught — same error either way).
    fn resolve_type(&self, name: &str, line: usize, column: usize) -> Result<Type, TypeError> {
        if self.classes.contains_key(name) {
            return Ok(Type::Instance(name.to_string()));
        }
        Type::from_annotation_name(name).ok_or_else(|| TypeError {
            message: format!("unknown type `{name}`"),
            line,
            column,
        })
    }

    /// Binds `name: ty` in the *innermost* (last) scope on the stack — i.e.
    /// the scope of whatever block/function/loop body is currently being
    /// checked. Used for local variable declarations, function/method
    /// parameters, `for`-loop induction variables, and (in `check_classes`)
    /// pre-declaring a class's field types so method bodies can read them
    /// like ordinary locals.
    fn declare_var(&mut self, name: &str, ty: Type) {
        self.env.declare(name, ty);
    }

    /// Resolves a variable name to its declared `Type`, searching scopes
    /// innermost-first (`.rev()` over the stack) so that a name declared in
    /// an inner block correctly shadows a same-named binding from an
    /// enclosing scope. Returns `None` if the name isn't declared in any
    /// scope currently on the stack, which callers turn into an "undefined
    /// variable" `TypeError`.
    fn lookup_var(&self, name: &str) -> Option<&Type> {
        self.env.lookup(name)
    }

    /// Opens a new, empty scope for a function/method body or a `for`-loop
    /// body, so variables declared inside don't leak into (or clash with) the
    /// enclosing scope. Must be paired with a later `pop_scope`. Delegates to
    /// [`Environment::push_scope`].
    fn push_scope(&mut self) {
        self.env.push_scope();
    }

    /// Discards the innermost scope (and every variable declared in it),
    /// restoring `lookup_var` resolution to whatever scope was active before
    /// the matching `push_scope`. Delegates to [`Environment::pop_scope`].
    fn pop_scope(&mut self) {
        self.env.pop_scope();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn check(src: &str) -> Result<(), TypeError> {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let program = Parser::new(tokens).parse_program().unwrap();
        TypeChecker::new().check_program(&program)
    }

    #[test]
    fn accepts_valid_function() {
        assert!(check("def add(a: Int, b: Int): Int\n  a + b\nend").is_ok());
    }

    #[test]
    fn rejects_int_float_mismatch() {
        let err = check("x: Int = 5\ny: Float = 1.0\nz = x + y").unwrap_err();
        assert!(err.message.contains("cannot apply"));
    }

    #[test]
    fn rejects_return_type_mismatch() {
        let err = check("def bad(): Int\n  \"oops\"\nend").unwrap_err();
        assert!(err.message.contains("declared to return"));
    }

    #[test]
    fn rejects_var_decl_type_mismatch() {
        let err = check("x: Int = \"hi\"").unwrap_err();
        assert!(err.message.contains("type mismatch"));
    }

    #[test]
    fn string_concat_allowed() {
        assert!(check("x = \"a\" + \"b\"").is_ok());
    }

    #[test]
    fn if_condition_must_be_boolean() {
        let err = check("if 5\n  1\nend").unwrap_err();
        assert!(err.message.contains("must be Boolean"));
    }

    #[test]
    fn undefined_variable_errors_with_position() {
        let err = check("print(missing)").unwrap_err();
        assert!(err.message.contains("undefined variable"));
        assert_eq!(err.line, 1);
    }

    #[test]
    fn function_call_arity_and_types_checked() {
        assert!(check("def add(a: Int, b: Int): Int\n  a + b\nend\nprint(add(1, 2))").is_ok());
        let err = check("def add(a: Int, b: Int): Int\n  a + b\nend\nadd(1)").unwrap_err();
        assert!(err.message.contains("expects 2 argument"));
    }

    #[test]
    fn for_loop_range_must_be_integer() {
        let err = check("for i in 0..\"a\"\n  print(i)\nend");
        assert!(err.is_err());
    }

    #[test]
    fn unary_negation_type() {
        assert!(check("x: Int = -5").is_ok());
        assert!(check("x: Float = -1.5").is_ok());
        assert!(check("x = -\"hi\"").is_err());
    }

    #[test]
    fn if_else_as_tail_expr_return_type() {
        assert!(check(
            "def fact(n: Int): Int\n  if n <= 1\n    1\n  else\n    n * fact(n - 1)\n  end\nend"
        )
        .is_ok());
    }

    #[test]
    fn if_tail_branches_must_agree_on_type() {
        let err =
            check("def f(): Int\n  if true\n    1\n  else\n    \"oops\"\n  end\nend").unwrap_err();
        assert!(err.message.contains("different types"));
    }

    #[test]
    fn array_literal_and_index_types() {
        assert!(check("xs: IntArray = [1, 2, 3]\ny: Int = xs[0]").is_ok());
        let err = check("xs: IntArray = [1, \"two\"]").unwrap_err();
        assert!(err.message.contains("must share one type"));
    }

    #[test]
    fn empty_array_literal_needs_annotation_to_infer() {
        assert!(check("xs: IntArray = []").is_ok());
    }

    #[test]
    fn array_builtins_type_checked() {
        assert!(check("xs: IntArray = [1, 2]\npush(xs, 3)\ny: Int = get(xs, 0)\nset(xs, 0, 9)\nz: Int = len(xs)").is_ok());
        let err = check("xs: IntArray = [1, 2]\npush(xs, \"oops\")").unwrap_err();
        assert!(err.message.contains("push"));
    }

    #[test]
    fn pop_type_checked() {
        assert!(check("xs: IntArray = [1, 2]\ny: Int = pop(xs)").is_ok());
        let err = check("xs: IntArray = [1]\ny: String = pop(xs)").unwrap_err();
        assert!(err.message.contains("type mismatch"));
    }

    #[test]
    fn index_requires_integer() {
        let err = check("xs: IntArray = [1]\ny = xs[\"zero\"]").unwrap_err();
        assert!(err.message.contains("must be Integer"));
    }

    const HELLO_CLASS: &str = "class Hello\n  const PI: Float = 3.14159\n  count: Integer\n\n  def initializer(number: Int)\n    count = number\n  end\nend\n";

    /// A field never assigned in `initializer` (here: no initializer at all)
    /// must be rejected at check time — it would read as `Nil` at runtime.
    #[test]
    fn rejects_field_never_assigned_in_initializer() {
        let err = check("class Counter\n  count: Integer\nend\n").unwrap_err();
        assert!(err.message.contains("never assigned in `initializer`"));
        assert_eq!((err.line, err.column), (2, 3));
    }

    /// Assigned in a *non*-initializer method only — still rejected: the
    /// window between construction and that method call reads `Nil`.
    #[test]
    fn rejects_field_assigned_only_outside_initializer() {
        let err =
            check("class Counter\n  count: Integer\n\n  def bump()\n    count = 1\n  end\nend\n")
                .unwrap_err();
        assert!(err.message.contains("never assigned in `initializer`"));
    }

    /// Flow-insensitivity: an assignment inside an `if` branch of the
    /// initializer counts as assigned.
    #[test]
    fn accepts_field_assigned_conditionally_in_initializer() {
        assert!(check(
            "class Counter\n  count: Integer\n\n  def initializer(big: Bool)\n    if big\n      count = 100\n    else\n      count = 0\n    end\n  end\nend\n"
        )
        .is_ok());
    }

    #[test]
    fn class_construction_field_access_and_method_call() {
        let src =
            format!("{HELLO_CLASS}h: Hello = Hello.new(5)\nx: Int = h.count\ny: Float = h.PI");
        assert!(check(&src).is_ok());
    }

    #[test]
    fn class_field_assignment_type_checked() {
        let src = format!("{HELLO_CLASS}h = Hello.new(5)\nh.count = 9");
        assert!(check(&src).is_ok());
        let src_bad = format!("{HELLO_CLASS}h = Hello.new(5)\nh.count = \"oops\"");
        let err = check(&src_bad).unwrap_err();
        assert!(err.message.contains("cannot assign"));
    }

    #[test]
    fn class_unknown_field_and_method_are_errors() {
        let src = format!("{HELLO_CLASS}h = Hello.new(5)\nx = h.missing");
        let err = check(&src).unwrap_err();
        assert!(err.message.contains("has no field"));

        let src2 = format!("{HELLO_CLASS}h = Hello.new(5)\nh.missing_method()");
        let err2 = check(&src2).unwrap_err();
        assert!(err2.message.contains("has no method"));
    }

    #[test]
    fn class_new_arg_count_checked() {
        let src = format!("{HELLO_CLASS}h = Hello.new(5, 6)");
        let err = check(&src).unwrap_err();
        assert!(err.message.contains("expects 1 argument"));
    }

    #[test]
    fn pointer_alloc_deref_free() {
        assert!(check(
            "p: Ptr<Integer> = alloc(5)\nx: Integer = deref(p)\nset_deref(p, 9)\nfree(p)"
        )
        .is_ok());
    }

    #[test]
    fn pointer_set_deref_type_mismatch() {
        let err = check("p: Ptr<Integer> = alloc(5)\nset_deref(p, 1.5)").unwrap_err();
        assert!(
            err.message.contains("set_deref")
                && err.message.contains("Ptr<Integer>")
                && err.message.contains("Float")
        );
    }

    #[test]
    fn pointer_deref_non_pointer() {
        let err = check("deref(3)").unwrap_err();
        assert!(err.message.contains("deref") && err.message.contains("pointer"));
    }

    #[test]
    fn pointer_declared_vs_actual_mismatch() {
        let err = check("p: Ptr<Float> = alloc(5)").unwrap_err();
        assert!(err.message.contains("type mismatch"));
    }

    #[test]
    fn collect_returns_integer() {
        assert!(check("n: Integer = collect()").is_ok());
    }

    #[test]
    fn collect_type_mismatch() {
        let err = check("n: String = collect()").unwrap_err();
        assert!(err.message.contains("type mismatch"));
    }
}
