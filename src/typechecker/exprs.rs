use super::*;

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
pub(super) fn check_expr(checker: &mut TypeChecker, expr: &Expr) -> Result<Type, TypeError> {
    match expr {
        Expr::IntLit { .. } => Ok(Type::Integer),
        Expr::FloatLit { .. } => Ok(Type::Float),
        Expr::StringLit { .. } => Ok(Type::String),
        Expr::BoolLit { .. } => Ok(Type::Boolean),
        Expr::NilLit { .. } => Ok(Type::Nil),
        Expr::Ident { name, line, column } => {
            checker.lookup_var(name).cloned().ok_or_else(|| TypeError {
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
            let left_ty = check_expr(checker, left)?;
            let right_ty = check_expr(checker, right)?;
            check_binary_op(*op, &left_ty, &right_ty, *line, *column)
        }
        Expr::Unary {
            op: UnOp::Neg,
            expr,
            line,
            column,
        } => {
            let ty = check_expr(checker, expr)?;
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
        } => super::calls::check_call(checker, callee, args, *line, *column),
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
            let first_ty = check_expr(checker, &elements[0])?;
            for elem in &elements[1..] {
                let ty = check_expr(checker, elem)?;
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
            let array_ty = check_expr(checker, array)?;
            let index_ty = check_expr(checker, index)?;
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
        } => super::classes::check_field_access(checker, object, field, *line, *column),
        Expr::MethodCall {
            object,
            method,
            args,
            line,
            column,
        } => super::classes::check_method_call(checker, object, method, args, *line, *column),
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
