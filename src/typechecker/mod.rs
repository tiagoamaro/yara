//! Static type checking pass over the AST.

use crate::ast::{BinOp, Expr, Stmt, UnOp};
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
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Integer => write!(f, "Integer"),
            Type::Float => write!(f, "Float"),
            Type::Boolean => write!(f, "Boolean"),
            Type::String => write!(f, "String"),
            Type::Nil => write!(f, "Nil"),
            Type::Array(elem) => write!(f, "Array<{elem}>"),
        }
    }
}

impl Type {
    /// `IntArray`/`FloatArray`/`BoolArray`/`StringArray` are the only array
    /// type annotations — there's no generic `Array<T>` syntax, so each
    /// element type gets its own concrete annotation name (Pascal-array style).
    fn from_annotation_name(name: &str) -> Option<Type> {
        match name {
            "Integer" => Some(Type::Integer),
            "Float" => Some(Type::Float),
            "Boolean" => Some(Type::Boolean),
            "String" => Some(Type::String),
            "Nil" => Some(Type::Nil),
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

#[derive(Clone)]
struct FunctionSig {
    param_types: Vec<Type>,
    return_type: Option<Type>,
}

struct Scope {
    vars: HashMap<String, Type>,
}

pub struct TypeChecker {
    scopes: Vec<Scope>,
    functions: HashMap<String, FunctionSig>,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        TypeChecker {
            scopes: vec![Scope {
                vars: HashMap::new(),
            }],
            functions: HashMap::new(),
        }
    }

    pub fn check_program(&mut self, program: &[Stmt]) -> Result<(), TypeError> {
        self.collect_function_signatures(program)?;
        for stmt in program {
            self.check_stmt(stmt)?;
        }
        Ok(())
    }

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

    fn resolve_type(&self, name: &str, line: usize, column: usize) -> Result<Type, TypeError> {
        Type::from_annotation_name(name).ok_or_else(|| TypeError {
            message: format!("unknown type `{name}`"),
            line,
            column,
        })
    }

    fn declare_var(&mut self, name: &str, ty: Type) {
        self.scopes
            .last_mut()
            .unwrap()
            .vars
            .insert(name.to_string(), ty);
    }

    fn lookup_var(&self, name: &str) -> Option<&Type> {
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
                            line: stmt_line(stmt),
                            column: stmt_column(stmt),
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
        }
    }

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
            } => {
                if callee == "print" {
                    for a in args {
                        self.check_expr(a)?;
                    }
                    return Ok(Type::Nil);
                }
                if let Some(ty) = self.check_array_builtin(callee, args, *line, *column)? {
                    return Ok(ty);
                }
                let sig = self
                    .functions
                    .get(callee)
                    .cloned()
                    .ok_or_else(|| TypeError {
                        message: format!("undefined function `{callee}`"),
                        line: *line,
                        column: *column,
                    })?;
                if args.len() != sig.param_types.len() {
                    return Err(TypeError {
                        message: format!(
                            "function `{callee}` expects {} argument(s), found {}",
                            sig.param_types.len(),
                            args.len()
                        ),
                        line: *line,
                        column: *column,
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
        }
    }

    /// Type-checks `len`/`push`/`get`/`set` if `callee` names one of them, returning
    /// `Ok(None)` for any other callee so the caller falls through to user-defined
    /// function lookup.
    fn check_array_builtin(
        &mut self,
        callee: &str,
        args: &[Expr],
        line: usize,
        column: usize,
    ) -> Result<Option<Type>, TypeError> {
        let arity_err = |expected: usize| TypeError {
            message: format!(
                "`{callee}` expects {expected} argument(s), found {}",
                args.len()
            ),
            line,
            column,
        };
        match callee {
            "len" => {
                if args.len() != 1 {
                    return Err(arity_err(1));
                }
                match self.check_expr(&args[0])? {
                    Type::Array(_) => Ok(Some(Type::Integer)),
                    other => Err(TypeError {
                        message: format!("`len` expects an array, found `{other}`"),
                        line,
                        column,
                    }),
                }
            }
            "push" => {
                if args.len() != 2 {
                    return Err(arity_err(2));
                }
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
                if args.len() != 2 {
                    return Err(arity_err(2));
                }
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
                if args.len() != 3 {
                    return Err(arity_err(3));
                }
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
            "pop" => {
                if args.len() != 1 {
                    return Err(arity_err(1));
                }
                match self.check_expr(&args[0])? {
                    Type::Array(elem) => Ok(Some(*elem)),
                    other => Err(TypeError {
                        message: format!("`pop` expects an array, found `{other}`"),
                        line,
                        column,
                    }),
                }
            }
            _ => Ok(None),
        }
    }

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

fn stmt_line(stmt: &Stmt) -> usize {
    match stmt {
        Stmt::VarDecl { line, .. }
        | Stmt::ConstDecl { line, .. }
        | Stmt::FunctionDef { line, .. }
        | Stmt::Return { line, .. }
        | Stmt::If { line, .. }
        | Stmt::While { line, .. }
        | Stmt::For { line, .. }
        | Stmt::Import { line, .. } => *line,
        Stmt::ExprStmt(expr) => expr.line(),
    }
}

fn stmt_column(stmt: &Stmt) -> usize {
    match stmt {
        Stmt::VarDecl { column, .. }
        | Stmt::ConstDecl { column, .. }
        | Stmt::FunctionDef { column, .. }
        | Stmt::Return { column, .. }
        | Stmt::If { column, .. }
        | Stmt::While { column, .. }
        | Stmt::For { column, .. }
        | Stmt::Import { column, .. } => *column,
        Stmt::ExprStmt(expr) => expr.column(),
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
}
