//! Static type checking pass over the AST.

use crate::ast::{BinOp, Expr, Stmt, UnOp};
use crate::builtins;
use crate::env::Environment;
use std::collections::HashMap;
use std::fmt;

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
}

/// The primitive types, each paired with its canonical annotation/display name.
/// This one bijective table is the single source of truth for the primitive
/// name↔`Type` mapping, read in both directions: `from_annotation_name`
/// resolves a name to a `Type`, `Display` renders a `Type` back to its name.
/// Compound types (`Array`, `Instance`) carry data a flat table can't and are
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
    /// gets its own concrete annotation name (Pascal-array style).
    fn from_annotation_name(name: &str) -> Option<Type> {
        if let Some((_, ty)) = PRIMITIVE_TYPES.iter().find(|(n, _)| *n == name) {
            return Some(ty.clone());
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
        self.collect_classes(program)?;
        self.collect_function_signatures(program)?;
        self.check_classes(program)?;
        for stmt in program {
            self.check_stmt(stmt)?;
        }
        Ok(())
    }

    /// Builds `self.classes` in two passes over every `Stmt::ClassDef` in the
    /// program. Pass one just registers each class's *name* with an empty,
    /// placeholder `ClassInfo` — this exists purely so that pass two can
    /// freely resolve a field/param/return type annotation that names *any*
    /// class, including one declared later in the file or the class's own
    /// name (self-referential fields, e.g. a `Node` with a `next: Node`
    /// field). Pass two then does the real work for each class: resolves
    /// every const/field annotation into a `Type` (consts and instance vars
    /// are merged into one `fields` map, since both are read unqualified
    /// inside methods via implicit `self` and the checker doesn't need to
    /// tell them apart afterward), and resolves every method's param/return
    /// types into a `FunctionSig` — but does not yet check any method
    /// *bodies*; that happens later in `check_classes`, once every class's
    /// signatures are known.
    fn collect_classes(&mut self, program: &[Stmt]) -> Result<(), TypeError> {
        // Pre-register every class name (with an empty placeholder) first, so
        // field/param/return type annotations can name any class regardless
        // of declaration order (including a class referencing itself).
        for stmt in program {
            if let Stmt::ClassDef { name, .. } = stmt {
                self.classes.entry(name.clone()).or_insert(ClassInfo {
                    fields: HashMap::new(),
                    methods: HashMap::new(),
                });
            }
        }

        for stmt in program {
            let Stmt::ClassDef {
                name,
                consts,
                fields,
                methods,
                ..
            } = stmt
            else {
                continue;
            };

            let mut field_types = HashMap::new();
            for c in consts {
                let Stmt::ConstDecl {
                    name: cname,
                    type_ann,
                    line,
                    column,
                    ..
                } = c
                else {
                    unreachable!("parser only ever puts ConstDecl in ClassDef.consts")
                };
                let Some(ann) = type_ann else {
                    return Err(TypeError {
                        message: "class constants require an explicit type annotation".to_string(),
                        line: *line,
                        column: *column,
                    });
                };
                let ty = self.resolve_type(&ann.name, ann.line, ann.column)?;
                field_types.insert(cname.clone(), ty);
            }
            for f in fields {
                let ty = self.resolve_type(&f.type_ann.name, f.line, f.column)?;
                field_types.insert(f.name.clone(), ty);
            }

            let mut method_sigs = HashMap::new();
            for m in methods {
                let Stmt::FunctionDef {
                    name: mname,
                    params,
                    return_type,
                    line,
                    column,
                    ..
                } = m
                else {
                    unreachable!("parser only ever puts FunctionDef in ClassDef.methods")
                };
                let mut param_types = Vec::new();
                for p in params {
                    param_types.push(self.resolve_type(&p.type_ann.name, p.line, p.column)?);
                }
                let ret = match return_type {
                    Some(t) => Some(self.resolve_type(&t.name, *line, *column)?),
                    None => None,
                };
                method_sigs.insert(
                    mname.clone(),
                    FunctionSig {
                        param_types,
                        return_type: ret,
                    },
                );
            }

            self.classes.insert(
                name.clone(),
                ClassInfo {
                    fields: field_types,
                    methods: method_sigs,
                },
            );
        }
        Ok(())
    }

    /// Type-checks every class method body (including `initializer`), with
    /// the class's fields/consts pre-declared in the method's scope so bare
    /// names resolve to them (implicit `self`, Ruby-ivar style).
    fn check_classes(&mut self, program: &[Stmt]) -> Result<(), TypeError> {
        for stmt in program {
            let Stmt::ClassDef {
                name,
                fields,
                methods,
                ..
            } = stmt
            else {
                continue;
            };
            self.check_fields_assigned_in_initializer(name, fields, methods)?;
            let field_types = self.classes[name].fields.clone();
            for m in methods {
                let Stmt::FunctionDef {
                    params,
                    body,
                    return_type,
                    ..
                } = m
                else {
                    unreachable!("parser only ever puts FunctionDef in ClassDef.methods")
                };
                self.push_scope();
                for (fname, fty) in &field_types {
                    self.declare_var(fname, fty.clone());
                }
                for p in params {
                    let ty = self.resolve_type(&p.type_ann.name, p.line, p.column)?;
                    self.declare_var(&p.name, ty);
                }
                let declared_return = match return_type {
                    Some(t) => Some(self.resolve_type(&t.name, t.line, t.column)?),
                    None => None,
                };
                let actual_return = self.check_body_return_type(body)?;
                if let (Some(declared), Some(actual)) = (&declared_return, &actual_return) {
                    if declared != actual {
                        self.pop_scope();
                        return Err(TypeError {
                            message: format!(
                                "method `{name}#{}` declared to return `{declared}`, but returns `{actual}`",
                                method_name(m)
                            ),
                            line: m.line(),
                            column: m.column(),
                        });
                    }
                }
                self.pop_scope();
            }
        }
        Ok(())
    }

    /// Requires every instance-variable declaration (a `FieldDecl` — always
    /// valueless by construction) to be assigned somewhere in the class's
    /// `initializer`, closing the soundness gap where reading a never-assigned
    /// field type-checked as its declared type but evaluated to `Nil`.
    ///
    /// The check is deliberately flow-insensitive: an assignment anywhere in
    /// the initializer body (including inside an `if` branch or loop) counts,
    /// so a conditionally-assigned field still passes. That over-approximation
    /// is accepted — the goal is catching the common "declared but never set
    /// anywhere" mistake, not full definite-assignment analysis. Inside
    /// methods a bare `count = number` assignment parses as `Stmt::VarDecl`
    /// (implicit self), so collecting `VarDecl` names is what detects field
    /// assignment.
    fn check_fields_assigned_in_initializer(
        &self,
        class_name: &str,
        fields: &[crate::ast::FieldDecl],
        methods: &[Stmt],
    ) -> Result<(), TypeError> {
        if fields.is_empty() {
            return Ok(());
        }
        let initializer = methods.iter().find_map(|m| match m {
            Stmt::FunctionDef { name, body, .. } if name == "initializer" => Some(body),
            _ => None,
        });
        let mut assigned = std::collections::HashSet::new();
        if let Some(body) = initializer {
            collect_assigned_names(body, &mut assigned);
        }
        for f in fields {
            if !assigned.contains(f.name.as_str()) {
                return Err(TypeError {
                    message: format!(
                        "field `{}` of class `{class_name}` is never assigned in `initializer` \
                         (it would be `Nil` at runtime, not `{}`)",
                        f.name, f.type_ann.name
                    ),
                    line: f.line,
                    column: f.column,
                });
            }
        }
        Ok(())
    }

    /// Pre-registers every top-level function's parameter and return types
    /// into `self.functions`, without checking any body. Run before any
    /// bodies are checked so that a call to a function defined later in the
    /// file (or a recursive call to the function currently being checked)
    /// already has a known signature to check argument types/arity against.
    fn collect_function_signatures(&mut self, program: &[Stmt]) -> Result<(), TypeError> {
        for stmt in program {
            if let Stmt::FunctionDef {
                name,
                params,
                return_type,
                line,
                column,
                ..
            } = stmt
            {
                let mut param_types = Vec::new();
                for p in params {
                    param_types.push(self.resolve_type(&p.type_ann.name, p.line, p.column)?);
                }
                let return_type = match return_type {
                    Some(t) => Some(self.resolve_type(&t.name, *line, *column)?),
                    None => None,
                };
                self.functions.insert(
                    name.clone(),
                    FunctionSig {
                        param_types,
                        return_type,
                    },
                );
            }
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

    /// The main statement-level recursive check: one arm per `Stmt` variant.
    /// Each arm follows the same general shape — recursively check whatever
    /// sub-expressions/sub-blocks the statement contains (via `check_expr`/
    /// `check_block`/`check_body_return_type`), then either enforce a rule
    /// specific to that statement (e.g. an `if`/`while` condition must be
    /// `Boolean`, a `VarDecl`'s declared annotation must match its value's
    /// inferred type) or just thread the result through. Declarations
    /// (`VarDecl`/`ConstDecl`, function/loop parameters) call `declare_var`
    /// so later statements in the same scope can look the name up. Returns
    /// `Ok(())` rather than a `Type` because statements (unlike expressions)
    /// don't themselves have a value — see `check_tail_stmt` for the one
    /// place a statement's "value" (its tail-expression type) does matter.
    fn check_stmt(&mut self, stmt: &Stmt) -> Result<(), TypeError> {
        match stmt {
            Stmt::VarDecl {
                name,
                type_ann,
                value,
                line,
                column,
            }
            | Stmt::ConstDecl {
                name,
                type_ann,
                value,
                line,
                column,
            } => {
                let value_ty = self.check_expr(value)?;
                let mut stored_ty = value_ty.clone();
                if let Some(ann) = type_ann {
                    let declared = self.resolve_type(&ann.name, ann.line, ann.column)?;
                    // `[]` (empty array literal) can't infer an element type on its
                    // own; check_expr reports it as `Array(Nil)` as a sentinel, which
                    // is compatible with any declared array type.
                    let empty_array_ok = matches!(
                        (&declared, &value_ty),
                        (Type::Array(_), Type::Array(elem)) if **elem == Type::Nil
                    );
                    if declared != value_ty && !empty_array_ok {
                        return Err(TypeError {
                            message: format!(
                                "type mismatch for `{name}`: declared `{declared}`, found `{value_ty}`"
                            ),
                            line: *line,
                            column: *column,
                        });
                    }
                    stored_ty = declared;
                }
                self.declare_var(name, stored_ty);
                Ok(())
            }
            Stmt::FunctionDef {
                name,
                params,
                body,
                return_type,
                ..
            } => {
                self.push_scope();
                for p in params {
                    let ty = self.resolve_type(&p.type_ann.name, p.line, p.column)?;
                    self.declare_var(&p.name, ty);
                }
                let declared_return = match return_type {
                    Some(t) => Some(self.resolve_type(&t.name, t.line, t.column)?),
                    None => None,
                };
                let actual_return = self.check_body_return_type(body)?;
                if let (Some(declared), Some(actual)) = (&declared_return, &actual_return) {
                    if declared != actual {
                        self.pop_scope();
                        return Err(TypeError {
                            message: format!(
                                "function `{name}` declared to return `{declared}`, but returns `{actual}`"
                            ),
                            line: stmt.line(),
                            column: stmt.column(),
                        });
                    }
                }
                self.pop_scope();
                Ok(())
            }
            Stmt::Return { value, .. } => {
                if let Some(v) = value {
                    self.check_expr(v)?;
                }
                Ok(())
            }
            Stmt::If {
                condition,
                then_body,
                elsif_branches,
                else_body,
                line,
                column,
            } => {
                let cond_ty = self.check_expr(condition)?;
                if cond_ty != Type::Boolean {
                    return Err(TypeError {
                        message: format!("`if` condition must be Boolean, found `{cond_ty}`"),
                        line: *line,
                        column: *column,
                    });
                }
                self.check_block(then_body)?;
                for (cond, body) in elsif_branches {
                    let ty = self.check_expr(cond)?;
                    if ty != Type::Boolean {
                        return Err(TypeError {
                            message: format!("`elsif` condition must be Boolean, found `{ty}`"),
                            line: cond.line(),
                            column: cond.column(),
                        });
                    }
                    self.check_block(body)?;
                }
                if let Some(body) = else_body {
                    self.check_block(body)?;
                }
                Ok(())
            }
            Stmt::While {
                condition,
                body,
                line,
                column,
            } => {
                let cond_ty = self.check_expr(condition)?;
                if cond_ty != Type::Boolean {
                    return Err(TypeError {
                        message: format!("`while` condition must be Boolean, found `{cond_ty}`"),
                        line: *line,
                        column: *column,
                    });
                }
                self.check_block(body)?;
                Ok(())
            }
            Stmt::For {
                var_name,
                range_start,
                range_end,
                body,
                line,
                column,
            } => {
                let start_ty = self.check_expr(range_start)?;
                let end_ty = self.check_expr(range_end)?;
                if start_ty != Type::Integer || end_ty != Type::Integer {
                    return Err(TypeError {
                        message: "`for` range bounds must be Integer".to_string(),
                        line: *line,
                        column: *column,
                    });
                }
                self.push_scope();
                self.declare_var(var_name, Type::Integer);
                self.check_block(body)?;
                self.pop_scope();
                Ok(())
            }
            Stmt::ExprStmt(expr) => {
                self.check_expr(expr)?;
                Ok(())
            }
            // Resolved away by `resolver` before typechecking ever sees the program.
            Stmt::Import { .. } => Ok(()),
            // Already fully checked by `collect_classes`/`check_classes` in `check_program`.
            Stmt::ClassDef { .. } => Ok(()),
            Stmt::FieldAssign {
                object,
                field,
                value,
                line,
                column,
            } => {
                let field_ty = self.check_field_access(object, field, *line, *column)?;
                let value_ty = self.check_expr(value)?;
                if field_ty != value_ty {
                    return Err(TypeError {
                        message: format!(
                            "cannot assign `{value_ty}` to field `{field}` of type `{field_ty}`"
                        ),
                        line: *line,
                        column: *column,
                    });
                }
                Ok(())
            }
        }
    }

    /// Shared by `Expr::FieldAccess` and `Stmt::FieldAssign`: resolves
    /// `object.field`'s type, erroring if `object` isn't a class instance or
    /// the class has no such field.
    fn check_field_access(
        &mut self,
        object: &Expr,
        field: &str,
        line: usize,
        column: usize,
    ) -> Result<Type, TypeError> {
        let object_ty = self.check_expr(object)?;
        let Type::Instance(class_name) = &object_ty else {
            return Err(TypeError {
                message: format!("cannot access field `{field}` on `{object_ty}`"),
                line,
                column,
            });
        };
        self.classes[class_name]
            .fields
            .get(field)
            .cloned()
            .ok_or_else(|| TypeError {
                message: format!("class `{class_name}` has no field `{field}`"),
                line,
                column,
            })
    }

    /// Type-checks every statement in a block (an `if`/`while`/`for` body)
    /// purely for side effects/errors — unlike `check_body_return_type`, no
    /// statement here is treated as a tail expression, since these blocks
    /// aren't function bodies and their "last value" is never observed.
    fn check_block(&mut self, body: &[Stmt]) -> Result<(), TypeError> {
        for stmt in body {
            self.check_stmt(stmt)?;
        }
        Ok(())
    }

    /// Returns the type of the function body's final expression, used to validate
    /// against a declared return type (Ruby-style implicit last-expression return).
    /// An `if`/`elsif`/`else` as the trailing statement is itself treated as a tail
    /// expression: each branch's own tail type must agree.
    fn check_body_return_type(&mut self, body: &[Stmt]) -> Result<Option<Type>, TypeError> {
        let mut last_ty = None;
        for (i, stmt) in body.iter().enumerate() {
            if i == body.len() - 1 {
                last_ty = self.check_tail_stmt(stmt)?;
            } else {
                self.check_stmt(stmt)?;
            }
        }
        Ok(last_ty)
    }

    /// Checks a single statement *as a tail position* — i.e. as the last
    /// statement of a function/method body (or of one branch of a trailing
    /// `if`), where Ruby-style implicit return means "the type of this
    /// statement is the type the enclosing body returns". An `ExprStmt`
    /// or an explicit `return expr` yields that expression's type; a bare
    /// `return` (no value) yields `Type::Nil`. A trailing `if`/`elsif`/`else`
    /// is the interesting recursive case: it's treated as a tail expression
    /// in its own right, so each branch's body is checked via
    /// `check_body_return_type` (its own tail statement resolved the same
    /// way), and all branch types must be reconciled through
    /// `combine_tail_types`. If there's no `else`, one possible path yields
    /// no value at all, so the whole `if`'s tail type collapses to `None`
    /// ("can't guarantee a return type from this tail position") rather than
    /// erroring — that `None` is later just skipped when comparing against a
    /// declared return type. Any other statement in tail position (e.g. a
    /// `VarDecl` as the last line of a body) is checked normally via
    /// `check_stmt` and contributes no return type (`None`).
    fn check_tail_stmt(&mut self, stmt: &Stmt) -> Result<Option<Type>, TypeError> {
        match stmt {
            Stmt::ExprStmt(expr) => Ok(Some(self.check_expr(expr)?)),
            Stmt::Return {
                value: Some(expr), ..
            } => Ok(Some(self.check_expr(expr)?)),
            Stmt::Return { value: None, .. } => Ok(Some(Type::Nil)),
            Stmt::If {
                condition,
                then_body,
                elsif_branches,
                else_body,
                line,
                column,
            } => {
                let cond_ty = self.check_expr(condition)?;
                if cond_ty != Type::Boolean {
                    return Err(TypeError {
                        message: format!("`if` condition must be Boolean, found `{cond_ty}`"),
                        line: *line,
                        column: *column,
                    });
                }
                let mut result = self.check_body_return_type(then_body)?;
                for (cond, body) in elsif_branches {
                    let ty = self.check_expr(cond)?;
                    if ty != Type::Boolean {
                        return Err(TypeError {
                            message: format!("`elsif` condition must be Boolean, found `{ty}`"),
                            line: cond.line(),
                            column: cond.column(),
                        });
                    }
                    let branch_ty = self.check_body_return_type(body)?;
                    result = Self::combine_tail_types(result, branch_ty, *line, *column)?;
                }
                result = match else_body {
                    Some(body) => {
                        let branch_ty = self.check_body_return_type(body)?;
                        Self::combine_tail_types(result, branch_ty, *line, *column)?
                    }
                    // No `else`: not every path yields a value, so this `if` can't
                    // be relied on as a tail expression.
                    None => None,
                };
                Ok(result)
            }
            _ => {
                self.check_stmt(stmt)?;
                Ok(None)
            }
        }
    }

    /// Reconciles the tail type of two sibling branches of an `if` (e.g. the
    /// running result so far and the next `elsif`/`else` branch). Requires
    /// exact agreement when both branches have a known type — a mismatch is
    /// reported as a `TypeError` ("branches of `if` return different types").
    /// If either side is `None` (that branch's tail wasn't a value, e.g. it
    /// ended in an ordinary statement rather than an expression), the
    /// combined result is `None` too, propagating the "not every path
    /// yields a value" signal up through the whole `if`/`elsif`/`else` chain.
    fn combine_tail_types(
        a: Option<Type>,
        b: Option<Type>,
        line: usize,
        column: usize,
    ) -> Result<Option<Type>, TypeError> {
        match (a, b) {
            (Some(x), Some(y)) if x == y => Ok(Some(x)),
            (Some(x), Some(y)) => Err(TypeError {
                message: format!("branches of `if` return different types: `{x}` vs `{y}`"),
                line,
                column,
            }),
            _ => Ok(None),
        }
    }

    /// The main expression-level recursive check: one arm per `Expr` variant,
    /// returning the expression's inferred `Type` (or a `TypeError` if it
    /// doesn't type-check). Literals (`IntLit`, `StringLit`, etc.) return
    /// their fixed type directly with no recursion. Everything else
    /// recursively checks its sub-expressions first and then combines their
    /// types according to that variant's rule — a binary op delegates to
    /// `check_binary_op`, a call to a function/array-builtin/method
    /// delegates to `check_array_builtin`/function-signature lookup/
    /// `check_method_call`, an index expression requires an `Integer` index
    /// into an `Array` and yields the element type, and so on. This is the
    /// same general shape as `check_stmt`, just producing a `Type` instead
    /// of `()` since every expression (unlike every statement) has a value.
    fn check_expr(&mut self, expr: &Expr) -> Result<Type, TypeError> {
        match expr {
            Expr::IntLit { .. } => Ok(Type::Integer),
            Expr::FloatLit { .. } => Ok(Type::Float),
            Expr::StringLit { .. } => Ok(Type::String),
            Expr::BoolLit { .. } => Ok(Type::Boolean),
            Expr::NilLit { .. } => Ok(Type::Nil),
            Expr::Ident { name, line, column } => {
                self.lookup_var(name).cloned().ok_or_else(|| TypeError {
                    message: format!("undefined variable `{name}`"),
                    line: *line,
                    column: *column,
                })
            }
            Expr::Binary {
                op,
                left,
                right,
                line,
                column,
            } => {
                let left_ty = self.check_expr(left)?;
                let right_ty = self.check_expr(right)?;
                self.check_binary_op(*op, &left_ty, &right_ty, *line, *column)
            }
            Expr::Unary {
                op: UnOp::Neg,
                expr,
                line,
                column,
            } => {
                let ty = self.check_expr(expr)?;
                match ty {
                    Type::Integer | Type::Float => Ok(ty),
                    other => Err(TypeError {
                        message: format!("cannot negate `{other}`"),
                        line: *line,
                        column: *column,
                    }),
                }
            }
            Expr::Call {
                callee,
                args,
                line,
                column,
            } => self.check_call(callee, args, *line, *column),
            Expr::ArrayLit {
                elements,
                line,
                column,
            } => {
                if elements.is_empty() {
                    // Sentinel: element type unknown, resolved by the enclosing
                    // `VarDecl`/`ConstDecl`'s explicit annotation if present.
                    return Ok(Type::Array(Box::new(Type::Nil)));
                }
                let first_ty = self.check_expr(&elements[0])?;
                for elem in &elements[1..] {
                    let ty = self.check_expr(elem)?;
                    if ty != first_ty {
                        return Err(TypeError {
                            message: format!(
                                "array elements must share one type: found `{first_ty}` and `{ty}`"
                            ),
                            line: *line,
                            column: *column,
                        });
                    }
                }
                Ok(Type::Array(Box::new(first_ty)))
            }
            Expr::Index {
                array,
                index,
                line,
                column,
            } => {
                let array_ty = self.check_expr(array)?;
                let index_ty = self.check_expr(index)?;
                if index_ty != Type::Integer {
                    return Err(TypeError {
                        message: format!("array index must be Integer, found `{index_ty}`"),
                        line: *line,
                        column: *column,
                    });
                }
                match array_ty {
                    Type::Array(elem) => Ok(*elem),
                    other => Err(TypeError {
                        message: format!("cannot index into `{other}`"),
                        line: *line,
                        column: *column,
                    }),
                }
            }
            Expr::FieldAccess {
                object,
                field,
                line,
                column,
            } => self.check_field_access(object, field, *line, *column),
            Expr::MethodCall {
                object,
                method,
                args,
                line,
                column,
            } => self.check_method_call(object, method, args, *line, *column),
        }
    }

    /// Handles both `ClassName.new(args)` (when `object` is a bare `Ident`
    /// naming a class rather than a bound variable) and ordinary
    /// `instance.method(args)` calls.
    fn check_method_call(
        &mut self,
        object: &Expr,
        method: &str,
        args: &[Expr],
        line: usize,
        column: usize,
    ) -> Result<Type, TypeError> {
        if let Expr::Ident { name, .. } = object {
            if self.lookup_var(name).is_none() && self.classes.contains_key(name) {
                return self.check_construction(name, method, args, line, column);
            }
        }

        let object_ty = self.check_expr(object)?;
        let Type::Instance(class_name) = &object_ty else {
            return Err(TypeError {
                message: format!("cannot call method `{method}` on `{object_ty}`"),
                line,
                column,
            });
        };
        let sig = self.classes[class_name]
            .methods
            .get(method)
            .cloned()
            .ok_or_else(|| TypeError {
                message: format!("class `{class_name}` has no method `{method}`"),
                line,
                column,
            })?;
        self.check_call_args(&format!("{class_name}#{method}"), &sig, args, line, column)?;
        Ok(sig.return_type.unwrap_or(Type::Nil))
    }

    /// Type-checks `ClassName.new(args)`, dispatched from `check_method_call`
    /// when `method_call`'s object is a bare `Ident` naming a known class
    /// (rather than a variable bound to an instance). Only `new` is a valid
    /// "static method" name — anything else is an error. If the class
    /// declared an `initializer` method, arguments are checked against its
    /// signature via `check_call_args`; if it didn't, `.new` must be called
    /// with zero arguments (a helpful error otherwise). Always yields
    /// `Type::Instance(class_name)` on success, since construction always
    /// produces an instance of the class being constructed.
    fn check_construction(
        &mut self,
        class_name: &str,
        method: &str,
        args: &[Expr],
        line: usize,
        column: usize,
    ) -> Result<Type, TypeError> {
        if method != "new" {
            return Err(TypeError {
                message: format!("class `{class_name}` has no static method `{method}`"),
                line,
                column,
            });
        }
        match self.classes[class_name].methods.get("initializer").cloned() {
            Some(sig) => {
                self.check_call_args(&format!("{class_name}.new"), &sig, args, line, column)?;
            }
            None if !args.is_empty() => {
                return Err(TypeError {
                    message: format!(
                        "class `{class_name}` has no initializer, so `.new` takes no arguments"
                    ),
                    line,
                    column,
                });
            }
            None => {}
        }
        Ok(Type::Instance(class_name.to_string()))
    }

    /// Shared arity/type-checking for any call-like site that already has a
    /// resolved `FunctionSig` — user-defined top-level function calls,
    /// instance method calls, and `.new` construction all funnel through
    /// here. Checks `args.len()` matches `sig.param_types.len()` first (using
    /// `what` — e.g. `"add"`, `"Hello#greet"`, `"Hello.new"` — to name the
    /// callee in the error), then checks each argument expression and
    /// requires its type to exactly equal the corresponding declared
    /// parameter type (no implicit coercion).
    fn check_call_args(
        &mut self,
        what: &str,
        sig: &FunctionSig,
        args: &[Expr],
        line: usize,
        column: usize,
    ) -> Result<(), TypeError> {
        if args.len() != sig.param_types.len() {
            return Err(TypeError {
                message: format!(
                    "`{what}` expects {} argument(s), found {}",
                    sig.param_types.len(),
                    args.len()
                ),
                line,
                column,
            });
        }
        for (arg, expected) in args.iter().zip(sig.param_types.iter()) {
            let arg_ty = self.check_expr(arg)?;
            if arg_ty != *expected {
                return Err(TypeError {
                    message: format!("argument to `{what}` expects `{expected}`, found `{arg_ty}`"),
                    line: arg.line(),
                    column: arg.column(),
                });
            }
        }
        Ok(())
    }

    /// Type-checks `len`/`push`/`get`/`set` if `callee` names one of them, returning
    /// `Ok(None)` for any other callee so the caller falls through to user-defined
    /// function lookup.
    /// Type-checks a free function call `callee(args)`, resolving in order: the
    /// `print` builtin (any args, yields `Nil`), the array builtins (via
    /// `check_array_builtin`), then a user-defined function in `self.functions`
    /// — checking argument count and each argument's type against the signature.
    /// Yields the function's declared return type, or `Nil` if it declares none.
    /// Split out of `check_expr`'s `Call` arm to keep that dispatch readable.
    fn check_call(
        &mut self,
        callee: &str,
        args: &[Expr],
        line: usize,
        column: usize,
    ) -> Result<Type, TypeError> {
        if callee == "print" {
            for a in args {
                self.check_expr(a)?;
            }
            return Ok(Type::Nil);
        }
        if let Some(ty) = self.check_array_builtin(callee, args, line, column)? {
            return Ok(ty);
        }
        let sig = self
            .functions
            .get(callee)
            .cloned()
            .ok_or_else(|| TypeError {
                message: format!("undefined function `{callee}`"),
                line,
                column,
            })?;
        if args.len() != sig.param_types.len() {
            return Err(TypeError {
                message: format!(
                    "function `{callee}` expects {} argument(s), found {}",
                    sig.param_types.len(),
                    args.len()
                ),
                line,
                column,
            });
        }
        for (arg, expected) in args.iter().zip(sig.param_types.iter()) {
            let arg_ty = self.check_expr(arg)?;
            if arg_ty != *expected {
                return Err(TypeError {
                    message: format!(
                        "argument to `{callee}` expects `{expected}`, found `{arg_ty}`"
                    ),
                    line: arg.line(),
                    column: arg.column(),
                });
            }
        }
        Ok(sig.return_type.unwrap_or(Type::Nil))
    }

    fn check_array_builtin(
        &mut self,
        callee: &str,
        args: &[Expr],
        line: usize,
        column: usize,
    ) -> Result<Option<Type>, TypeError> {
        let Some(builtin) = builtins::lookup(callee) else {
            return Ok(None);
        };
        // Arity is checked once here from the registry; the per-builtin arms
        // below can then trust `args` has exactly `builtin.arity` elements.
        if args.len() != builtin.arity {
            return Err(TypeError {
                message: format!(
                    "`{callee}` expects {} argument(s), found {}",
                    builtin.arity,
                    args.len()
                ),
                line,
                column,
            });
        }
        match callee {
            "len" => match self.check_expr(&args[0])? {
                Type::Array(_) => Ok(Some(Type::Integer)),
                other => Err(TypeError {
                    message: format!("`len` expects an array, found `{other}`"),
                    line,
                    column,
                }),
            },
            "push" => {
                let array_ty = self.check_expr(&args[0])?;
                let value_ty = self.check_expr(&args[1])?;
                match array_ty {
                    Type::Array(elem) if *elem == value_ty => Ok(Some(Type::Nil)),
                    Type::Array(elem) => Err(TypeError {
                        message: format!(
                            "`push` onto `Array<{elem}>` expects `{elem}`, found `{value_ty}`"
                        ),
                        line,
                        column,
                    }),
                    other => Err(TypeError {
                        message: format!("`push` expects an array, found `{other}`"),
                        line,
                        column,
                    }),
                }
            }
            "get" => {
                let array_ty = self.check_expr(&args[0])?;
                let index_ty = self.check_expr(&args[1])?;
                if index_ty != Type::Integer {
                    return Err(TypeError {
                        message: format!("`get` index must be Integer, found `{index_ty}`"),
                        line,
                        column,
                    });
                }
                match array_ty {
                    Type::Array(elem) => Ok(Some(*elem)),
                    other => Err(TypeError {
                        message: format!("`get` expects an array, found `{other}`"),
                        line,
                        column,
                    }),
                }
            }
            "set" => {
                let array_ty = self.check_expr(&args[0])?;
                let index_ty = self.check_expr(&args[1])?;
                let value_ty = self.check_expr(&args[2])?;
                if index_ty != Type::Integer {
                    return Err(TypeError {
                        message: format!("`set` index must be Integer, found `{index_ty}`"),
                        line,
                        column,
                    });
                }
                match array_ty {
                    Type::Array(elem) if *elem == value_ty => Ok(Some(Type::Nil)),
                    Type::Array(elem) => Err(TypeError {
                        message: format!(
                            "`set` onto `Array<{elem}>` expects `{elem}`, found `{value_ty}`"
                        ),
                        line,
                        column,
                    }),
                    other => Err(TypeError {
                        message: format!("`set` expects an array, found `{other}`"),
                        line,
                        column,
                    }),
                }
            }
            "pop" => match self.check_expr(&args[0])? {
                Type::Array(elem) => Ok(Some(*elem)),
                other => Err(TypeError {
                    message: format!("`pop` expects an array, found `{other}`"),
                    line,
                    column,
                }),
            },
            _ => unreachable!("builtin `{callee}` is in the registry but has no typecheck arm"),
        }
    }

    /// Enforces binary-operator typing rules given the already-checked types
    /// of both operands: no implicit numeric coercion, so `+ - * /` require
    /// both sides to be the *same* type and that type to be `Integer` or
    /// `Float` (yielding that same type) — except `String + String`, the one
    /// deliberate cross-type special case, which yields `String`
    /// (concatenation). Comparisons (`< > <= >=`) require matching numeric
    /// operands and always yield `Boolean`. Equality (`== !=`) only requires
    /// both sides to share a type (any type), also yielding `Boolean`. Any
    /// other combination is a `TypeError` naming the operator via
    /// `binop_symbol`.
    fn check_binary_op(
        &self,
        op: BinOp,
        left: &Type,
        right: &Type,
        line: usize,
        column: usize,
    ) -> Result<Type, TypeError> {
        let mismatch = || TypeError {
            message: format!(
                "cannot apply `{}` to `{left}` and `{right}`",
                binop_symbol(op)
            ),
            line,
            column,
        };
        match op {
            BinOp::Add if *left == Type::String && *right == Type::String => Ok(Type::String),
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                if left != right {
                    return Err(mismatch());
                }
                match left {
                    Type::Integer => Ok(Type::Integer),
                    Type::Float => Ok(Type::Float),
                    _ => Err(mismatch()),
                }
            }
            BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => {
                if left != right || !matches!(left, Type::Integer | Type::Float) {
                    return Err(mismatch());
                }
                Ok(Type::Boolean)
            }
            BinOp::Eq | BinOp::NotEq => {
                if left != right {
                    return Err(mismatch());
                }
                Ok(Type::Boolean)
            }
        }
    }
}

/// Collects every name assigned anywhere in `body` into `assigned`,
/// recursing through `if`/`while`/`for` bodies (flow-insensitively — a
/// conditional assignment still counts; see
/// `check_fields_assigned_in_initializer`). Both a bare `name = value`
/// (`Stmt::VarDecl`, how implicit-self field assignment parses inside
/// methods) and an explicit `object.field = value` (`Stmt::FieldAssign`)
/// register the assigned name.
fn collect_assigned_names(body: &[Stmt], assigned: &mut std::collections::HashSet<String>) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl { name, .. } => {
                assigned.insert(name.clone());
            }
            Stmt::FieldAssign { field, .. } => {
                assigned.insert(field.clone());
            }
            Stmt::If {
                then_body,
                elsif_branches,
                else_body,
                ..
            } => {
                collect_assigned_names(then_body, assigned);
                for (_, branch) in elsif_branches {
                    collect_assigned_names(branch, assigned);
                }
                if let Some(body) = else_body {
                    collect_assigned_names(body, assigned);
                }
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                collect_assigned_names(body, assigned);
            }
            _ => {}
        }
    }
}

/// Renders a `BinOp` back to its source-syntax symbol (e.g. `BinOp::Add` ->
/// `"+"`), used only to build human-readable `TypeError` messages in
/// `check_binary_op`.
fn binop_symbol(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Eq => "==",
        BinOp::NotEq => "!=",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::LtEq => "<=",
        BinOp::GtEq => ">=",
    }
}

/// Extracts a method's name from its `Stmt::FunctionDef` node, for building
/// the `"ClassName#method_name"` label used in return-type-mismatch error
/// messages in `check_classes`. Returns `"?"` for any other statement kind,
/// which should never actually happen since `ClassDef.methods` only ever
/// contains `FunctionDef`s (see the `unreachable!` guards in `collect_classes`
/// and `check_classes`).
fn method_name(method: &Stmt) -> &str {
    match method {
        Stmt::FunctionDef { name, .. } => name,
        _ => "?",
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
}
