//! AST node definitions for Yara.
//!
//! These types are the shared vocabulary between the parser (which builds
//! them), the typechecker (which walks them to check types without running
//! anything), and the interpreter (which walks them again to actually
//! execute the program). Nothing in this module executes or type-checks
//! anything itself — it is pure data.
//!
//! Every node that represents a piece of source syntax carries a `line`
//! and `column` pair: the 1-indexed source position (line/column of the
//! first token of that construct) used purely for diagnostics (error
//! messages, panics with location info). These fields are not used for
//! anything semantic — two otherwise-identical nodes at different source
//! positions are still "the same" node as far as typechecking/evaluation
//! are concerned.

/// A parsed type name such as `Integer`, `Float`, `Boolean`, `String`, or a
/// class name used as a type. Appears anywhere a variable/parameter/field/
/// return value declares its type (`x: Integer`, `def f(x: Integer): Boolean`).
#[derive(Debug, Clone, PartialEq)]
pub struct TypeAnnotation {
    /// Canonical (alias-normalized) type name, e.g. `Integer`, `Float`, `Boolean`, `String`.
    /// The parser calls `types::normalize_type_alias` before constructing this, so
    /// short-hand aliases like `Int`/`Bool`/`Str` are already resolved here — the
    /// typechecker and interpreter never have to know about aliases at all.
    pub name: String,
    /// Source line of the type annotation token.
    pub line: usize,
    /// Source column of the type annotation token.
    pub column: usize,
}

/// A single function/method parameter declaration, e.g. `x: Integer` inside
/// `def add(x: Integer, y: Integer)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// The parameter's name, used to bind the argument value in the callee's scope.
    pub name: String,
    /// The parameter's declared type, checked against the argument expression's
    /// inferred type at each call site.
    pub type_ann: TypeAnnotation,
    /// Source line of the parameter declaration.
    pub line: usize,
    /// Source column of the parameter declaration.
    pub column: usize,
}

/// A class instance-variable declaration (`count: Integer`, no value —
/// instance vars start out unset and are given a value in `initializer`).
/// This is the only place in the AST where a "declaration with no value"
/// exists; every other declaration (`VarDecl`, `ConstDecl`, `Param`) is
/// always paired with either a value expression or a call-site argument.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl {
    /// The instance variable's name.
    pub name: String,
    /// The instance variable's declared type.
    pub type_ann: TypeAnnotation,
    /// Source line of the field declaration.
    pub line: usize,
    /// Source column of the field declaration.
    pub column: usize,
}

/// An expression node — anything that produces a value. Every variant
/// carries its own `line`/`column` pair (rather than, say, a single
/// wrapper struct around a variant-less enum) so that pattern-matching on
/// the kind of expression and reading its position can both be done
/// directly on the same value without an extra layer of indirection.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// An integer literal, e.g. `42`.
    IntLit {
        /// The parsed integer value.
        value: i64,
        /// Source line of the literal.
        line: usize,
        /// Source column of the literal.
        column: usize,
    },
    /// A floating-point literal, e.g. `3.14`.
    FloatLit {
        /// The parsed floating-point value.
        value: f64,
        /// Source line of the literal.
        line: usize,
        /// Source column of the literal.
        column: usize,
    },
    /// A string literal, e.g. `"hello"`. Stored already unescaped/unquoted
    /// by the lexer — this is the literal string content, not the raw
    /// source text including quotes.
    StringLit {
        /// The literal's string contents.
        value: String,
        /// Source line of the literal.
        line: usize,
        /// Source column of the literal.
        column: usize,
    },
    /// A boolean literal, `true` or `false`.
    BoolLit {
        /// The parsed boolean value.
        value: bool,
        /// Source line of the literal.
        line: usize,
        /// Source column of the literal.
        column: usize,
    },
    /// The `nil` literal. Carries no payload beyond its position, since
    /// there is exactly one nil value.
    NilLit {
        /// Source line of the literal.
        line: usize,
        /// Source column of the literal.
        column: usize,
    },
    /// A bare identifier reference, e.g. `x` — looked up in the current
    /// scope (local variable, parameter, or constant) at evaluation/typecheck time.
    Ident {
        /// The identifier's name.
        name: String,
        /// Source line of the identifier token.
        line: usize,
        /// Source column of the identifier token.
        column: usize,
    },
    /// A binary operator expression, e.g. `a + b`, `x == y`. The specific
    /// operator is `op`; both operands are boxed since `Expr` is recursive
    /// and would otherwise have infinite size.
    Binary {
        /// The binary operator being applied.
        op: BinOp,
        /// The left-hand operand.
        left: Box<Expr>,
        /// The right-hand operand.
        right: Box<Expr>,
        /// Source line of the operator token.
        line: usize,
        /// Source column of the operator token.
        column: usize,
    },
    /// A free function call, e.g. `add(1, 2)`. `callee` is the bare
    /// function name rather than a boxed `Expr` — Yara has no first-class
    /// function values, so the callee position can only ever be an
    /// identifier naming a top-level function, and storing the `String`
    /// directly avoids an unnecessary `Expr::Ident` wrapper.
    Call {
        /// Name of the function being called.
        callee: String,
        /// The call's argument expressions, in order.
        args: Vec<Expr>,
        /// Source line of the call.
        line: usize,
        /// Source column of the call.
        column: usize,
    },
    /// A unary operator expression, e.g. `-x`.
    Unary {
        /// The unary operator being applied.
        op: UnOp,
        /// The operand.
        expr: Box<Expr>,
        /// Source line of the operator token.
        line: usize,
        /// Source column of the operator token.
        column: usize,
    },
    /// An array literal, e.g. `[1, 2, 3]`.
    ArrayLit {
        /// The literal's element expressions, in order.
        elements: Vec<Expr>,
        /// Source line of the literal.
        line: usize,
        /// Source column of the literal.
        column: usize,
    },
    /// An array index expression, e.g. `arr[i]`. `array` is boxed because
    /// it is itself an arbitrary expression (not just an identifier), which
    /// means indexing chains like `arr[i][j]` are syntactically representable
    /// even though no nested-array type annotation exists to type-check them
    /// (see the typechecker's design notes for that gap).
    Index {
        /// The expression being indexed.
        array: Box<Expr>,
        /// The index expression.
        index: Box<Expr>,
        /// Source line of the indexing `[`.
        line: usize,
        /// Source column of the indexing `[`.
        column: usize,
    },
    /// `object.field` — reading an instance variable or class constant.
    FieldAccess {
        /// The expression whose field is being read.
        object: Box<Expr>,
        /// Name of the field being read.
        field: String,
        /// Source line of the field-access expression.
        line: usize,
        /// Source column of the field-access expression.
        column: usize,
    },
    /// `object.method(args)`. Also covers `ClassName.new(args)` construction:
    /// the typechecker/interpreter special-case a bare `Ident` `object` that
    /// names a known class rather than a variable, with `method == "new"`.
    /// This reuses one node for both "call a method on an instance" and
    /// "construct a new instance" because the parser has no semantic/class-table
    /// information available to distinguish them at parse time — that
    /// distinction can only be made once a symbol table exists.
    MethodCall {
        /// The receiver expression (an instance, or a class name for `.new`).
        object: Box<Expr>,
        /// Name of the method being called (or `"new"` for construction).
        method: String,
        /// The call's argument expressions, in order.
        args: Vec<Expr>,
        /// Source line of the call.
        line: usize,
        /// Source column of the call.
        column: usize,
    },
}

/// Unary (prefix) operators. Currently only numeric negation exists — there
/// is no logical-not (`!`/`not`) operator in Yara yet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnOp {
    /// Arithmetic negation, `-x`.
    Neg,
}

impl Expr {
    /// Returns the source line this expression started on.
    ///
    /// Every `Expr` variant stores its own `line` field independently (there
    /// is no shared base struct an enum can borrow fields from in Rust), so
    /// getting "the line of this expression" from a value of unknown variant
    /// requires matching over every variant to pull out the field. This
    /// method centralizes that match so every caller that needs a position
    /// for error reporting (parser, typechecker, interpreter) can call
    /// `expr.line()` instead of re-deriving the same exhaustive match.
    pub fn line(&self) -> usize {
        match self {
            Expr::IntLit { line, .. }
            | Expr::FloatLit { line, .. }
            | Expr::StringLit { line, .. }
            | Expr::BoolLit { line, .. }
            | Expr::NilLit { line, .. }
            | Expr::Ident { line, .. }
            | Expr::Binary { line, .. }
            | Expr::Call { line, .. }
            | Expr::Unary { line, .. }
            | Expr::ArrayLit { line, .. }
            | Expr::Index { line, .. }
            | Expr::FieldAccess { line, .. }
            | Expr::MethodCall { line, .. } => *line,
        }
    }

    /// Returns the source column this expression started on.
    ///
    /// Same rationale as [`Expr::line`]: dispatches across every variant to
    /// extract the shared `column` field so callers get a uniform way to
    /// ask "where did this expression come from" without matching on the
    /// variant themselves.
    pub fn column(&self) -> usize {
        match self {
            Expr::IntLit { column, .. }
            | Expr::FloatLit { column, .. }
            | Expr::StringLit { column, .. }
            | Expr::BoolLit { column, .. }
            | Expr::NilLit { column, .. }
            | Expr::Ident { column, .. }
            | Expr::Binary { column, .. }
            | Expr::Call { column, .. }
            | Expr::Unary { column, .. }
            | Expr::ArrayLit { column, .. }
            | Expr::Index { column, .. }
            | Expr::FieldAccess { column, .. }
            | Expr::MethodCall { column, .. } => *column,
        }
    }
}

/// Binary operators. Carries no precedence information — precedence is
/// entirely a property of the parser's grammar (comparison binds looser
/// than additive, which binds looser than multiplicative), not of this enum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `==`
    Eq,
    /// `!=`
    NotEq,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `<=`
    LtEq,
    /// `>=`
    GtEq,
}

/// A statement node — anything executed for its effect (or, per Yara's
/// Ruby-style implicit-last-expression-return rule, potentially also used
/// as a function body's tail value; see `typechecker::check_tail_stmt` /
/// `interpreter::exec_tail_stmt` for how `If` in tail position is handled
/// as a value-producing construct despite being a `Stmt`).
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `x: Integer = value` or `x = value` — declares (or, depending on
    /// scope rules enforced elsewhere, reassigns) a local variable.
    VarDecl {
        /// The variable's name.
        name: String,
        /// The variable's declared type, if one was written explicitly;
        /// `None` when the type is to be inferred from `value`.
        type_ann: Option<TypeAnnotation>,
        /// The initializer expression.
        value: Expr,
        /// Source line of the declaration.
        line: usize,
        /// Source column of the declaration.
        column: usize,
    },
    /// A constant declaration — same shape as `VarDecl` but semantically
    /// immutable after initialization (enforced outside this module, by
    /// the typechecker/interpreter, not by this data type itself). Also
    /// reused, unchanged, as the element type of `ClassDef.consts`.
    ConstDecl {
        /// The constant's name.
        name: String,
        /// The constant's declared type, if written explicitly.
        type_ann: Option<TypeAnnotation>,
        /// The initializer expression.
        value: Expr,
        /// Source line of the declaration.
        line: usize,
        /// Source column of the declaration.
        column: usize,
    },
    /// `def name(params): return_type ... end` — a top-level function
    /// definition, or (reused unchanged) a class method/`initializer` when
    /// stored in `ClassDef.methods`.
    FunctionDef {
        /// The function's name.
        name: String,
        /// The function's parameter list, in declaration order.
        params: Vec<Param>,
        /// The function's declared return type, if any (`None` means the
        /// function returns nothing meaningful, e.g. it's used only for
        /// side effects).
        return_type: Option<TypeAnnotation>,
        /// The function body, as a sequence of statements. Per Yara's
        /// Ruby-style semantics, the last statement (if it's an expression
        /// or a tail-position `If`) is also the function's implicit return
        /// value when no explicit `Return` is hit first.
        body: Vec<Stmt>,
        /// Source line of the `def`.
        line: usize,
        /// Source column of the `def`.
        column: usize,
    },
    /// `return value` or bare `return` — an explicit early return from a
    /// function. `value` is `None` for a bare `return` with no expression.
    Return {
        /// The returned expression, if any.
        value: Option<Expr>,
        /// Source line of the `return`.
        line: usize,
        /// Source column of the `return`.
        column: usize,
    },
    /// `if condition ... elsif ... elsif ... else ... end`.
    ///
    /// `elsif_branches` is `Vec<(Expr, Vec<Stmt>)>` — a flat list of
    /// (condition, body) pairs — rather than modeling each `elsif` as a
    /// nested `Stmt::If` inside `else_body`. This keeps the whole
    /// if/elsif-chain as one flat node instead of a right-leaning tree of
    /// nested `If`s, which makes both typechecking (check every branch's
    /// body against the same expected type) and evaluation (walk the list
    /// looking for the first true condition, falling through to
    /// `else_body`) a simple loop instead of recursive descent through
    /// synthetic nested statements that don't actually appear in the
    /// source.
    If {
        /// The primary `if` condition.
        condition: Expr,
        /// The body executed when `condition` is true.
        then_body: Vec<Stmt>,
        /// Zero or more `elsif` branches, each an (condition, body) pair,
        /// tried in source order after `condition` is false.
        elsif_branches: Vec<(Expr, Vec<Stmt>)>,
        /// The `else` body, if present; `None` if there is no `else` clause.
        else_body: Option<Vec<Stmt>>,
        /// Source line of the `if`.
        line: usize,
        /// Source column of the `if`.
        column: usize,
    },
    /// `while condition ... end`.
    While {
        /// The loop condition, re-evaluated before each iteration.
        condition: Expr,
        /// The loop body.
        body: Vec<Stmt>,
        /// Source line of the `while`.
        line: usize,
        /// Source column of the `while`.
        column: usize,
    },
    /// `for var_name in range_start..range_end ... end` — a bounded
    /// integer-range loop; there is no generic iterator protocol, only
    /// this fixed start/end range form.
    For {
        /// The loop variable's name, bound to each value in the range in turn.
        var_name: String,
        /// The (inclusive/exclusive semantics defined by the interpreter)
        /// start-of-range expression.
        range_start: Expr,
        /// The end-of-range expression.
        range_end: Expr,
        /// The loop body.
        body: Vec<Stmt>,
        /// Source line of the `for`.
        line: usize,
        /// Source column of the `for`.
        column: usize,
    },
    /// An expression evaluated purely for its side effects (or as a tail
    /// value in implicit-return position). Wraps an `Expr` directly rather
    /// than duplicating `line`/`column` fields, since `Expr` already carries
    /// its own position via [`Expr::line`]/[`Expr::column`].
    ExprStmt(Expr),
    /// `import "path"` — a module import. This is parsed as an ordinary
    /// statement but has no runtime or typecheck meaning of its own:
    /// `resolver::resolve_imports` splices the imported module's contents
    /// in and removes the `Import` node before typechecking ever runs, so
    /// both `typechecker` and `interpreter` only ever handle this variant
    /// as an unreachable no-op kept for match-exhaustiveness.
    Import {
        /// The imported module's path, as written in source.
        path: String,
        /// Source line of the `import`.
        line: usize,
        /// Source column of the `import`.
        column: usize,
    },
    /// `class Name ... end`. `consts` holds only `Stmt::ConstDecl` entries,
    /// `methods` only `Stmt::FunctionDef` entries (including `initializer`)
    /// — reusing those variants rather than inventing near-duplicates.
    /// Both fields are typed as the general `Vec<Stmt>` (rather than, say,
    /// a dedicated struct per const/method) purely to avoid adding new AST
    /// types whose shape would be identical to the existing top-level
    /// `ConstDecl`/`FunctionDef` variants; the invariant that they only
    /// ever contain those specific variants is enforced by the parser, not
    /// by the type system.
    ClassDef {
        /// The class's name.
        name: String,
        /// The class's constant declarations; every element is a
        /// `Stmt::ConstDecl`.
        consts: Vec<Stmt>,
        /// The class's instance-variable declarations (no value; see
        /// [`FieldDecl`]).
        fields: Vec<FieldDecl>,
        /// The class's methods, including `initializer` if defined; every
        /// element is a `Stmt::FunctionDef`.
        methods: Vec<Stmt>,
        /// Source line of the `class`.
        line: usize,
        /// Source column of the `class`.
        column: usize,
    },
    /// `object.field = value` — assigns to an instance variable.
    FieldAssign {
        /// The expression whose field is being assigned (typically an
        /// `Expr::Ident` referring to `self` or an instance variable).
        object: Expr,
        /// Name of the field being assigned.
        field: String,
        /// The value expression being assigned.
        value: Expr,
        /// Source line of the assignment.
        line: usize,
        /// Source column of the assignment.
        column: usize,
    },
}
