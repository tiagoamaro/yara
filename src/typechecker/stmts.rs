use super::*;

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
pub(super) fn check_stmt(checker: &mut TypeChecker, stmt: &Stmt) -> Result<(), TypeError> {
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
            let value_ty = super::exprs::check_expr(checker, value)?;
            let mut stored_ty = value_ty.clone();
            if let Some(ann) = type_ann {
                let declared = checker.resolve_type(&ann.name, ann.line, ann.column)?;
                // `assignable` covers the two sanctioned mismatches: the `[]`
                // empty-array sentinel and `nil` into a pointer type.
                if !super::assignable(&declared, &value_ty) {
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
            checker.declare_var(name, stored_ty);
            Ok(())
        }
        Stmt::FunctionDef {
            name,
            params,
            body,
            return_type,
            ..
        } => {
            checker.push_scope();
            for p in params {
                let ty = checker.resolve_type(&p.type_ann.name, p.line, p.column)?;
                checker.declare_var(&p.name, ty);
            }
            let declared_return = match return_type {
                Some(t) => Some(checker.resolve_type(&t.name, t.line, t.column)?),
                None => None,
            };
            let actual_return = check_body_return_type(checker, body)?;
            if let (Some(declared), Some(actual)) = (&declared_return, &actual_return) {
                if declared != actual {
                    checker.pop_scope();
                    return Err(TypeError {
                        message: format!(
                            "function `{name}` declared to return `{declared}`, but returns `{actual}`"
                        ),
                        line: stmt.line(),
                        column: stmt.column(),
                    });
                }
            }
            checker.pop_scope();
            Ok(())
        }
        Stmt::Return { value, .. } => {
            if let Some(v) = value {
                super::exprs::check_expr(checker, v)?;
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
            let cond_ty = super::exprs::check_expr(checker, condition)?;
            if cond_ty != Type::Boolean {
                return Err(TypeError {
                    message: format!("`if` condition must be Boolean, found `{cond_ty}`"),
                    line: *line,
                    column: *column,
                });
            }
            check_block(checker, then_body)?;
            for (cond, body) in elsif_branches {
                let ty = super::exprs::check_expr(checker, cond)?;
                if ty != Type::Boolean {
                    return Err(TypeError {
                        message: format!("`elsif` condition must be Boolean, found `{ty}`"),
                        line: cond.line(),
                        column: cond.column(),
                    });
                }
                check_block(checker, body)?;
            }
            if let Some(body) = else_body {
                check_block(checker, body)?;
            }
            Ok(())
        }
        Stmt::While {
            condition,
            body,
            line,
            column,
        } => {
            let cond_ty = super::exprs::check_expr(checker, condition)?;
            if cond_ty != Type::Boolean {
                return Err(TypeError {
                    message: format!("`while` condition must be Boolean, found `{cond_ty}`"),
                    line: *line,
                    column: *column,
                });
            }
            check_block(checker, body)?;
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
            let start_ty = super::exprs::check_expr(checker, range_start)?;
            let end_ty = super::exprs::check_expr(checker, range_end)?;
            if start_ty != Type::Integer || end_ty != Type::Integer {
                return Err(TypeError {
                    message: "`for` range bounds must be Integer".to_string(),
                    line: *line,
                    column: *column,
                });
            }
            checker.push_scope();
            checker.declare_var(var_name, Type::Integer);
            check_block(checker, body)?;
            checker.pop_scope();
            Ok(())
        }
        Stmt::ExprStmt(expr) => {
            super::exprs::check_expr(checker, expr)?;
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
            let field_ty =
                super::classes::check_field_access(checker, object, field, *line, *column)?;
            let value_ty = super::exprs::check_expr(checker, value)?;
            if !super::assignable(&field_ty, &value_ty) {
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

/// Type-checks every statement in a block (an `if`/`while`/`for` body)
/// purely for side effects/errors — unlike `check_body_return_type`, no
/// statement here is treated as a tail expression, since these blocks
/// aren't function bodies and their "last value" is never observed.
pub(super) fn check_block(checker: &mut TypeChecker, body: &[Stmt]) -> Result<(), TypeError> {
    for stmt in body {
        check_stmt(checker, stmt)?;
    }
    Ok(())
}

/// Returns the type of the function body's final expression, used to validate
/// against a declared return type (Ruby-style implicit last-expression return).
/// An `if`/`elsif`/`else` as the trailing statement is itself treated as a tail
/// expression: each branch's own tail type must agree.
pub(super) fn check_body_return_type(
    checker: &mut TypeChecker,
    body: &[Stmt],
) -> Result<Option<Type>, TypeError> {
    let mut last_ty = None;
    for (i, stmt) in body.iter().enumerate() {
        if i == body.len() - 1 {
            last_ty = check_tail_stmt(checker, stmt)?;
        } else {
            check_stmt(checker, stmt)?;
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
fn check_tail_stmt(checker: &mut TypeChecker, stmt: &Stmt) -> Result<Option<Type>, TypeError> {
    match stmt {
        Stmt::ExprStmt(expr) => Ok(Some(super::exprs::check_expr(checker, expr)?)),
        Stmt::Return {
            value: Some(expr), ..
        } => Ok(Some(super::exprs::check_expr(checker, expr)?)),
        Stmt::Return { value: None, .. } => Ok(Some(Type::Nil)),
        Stmt::If {
            condition,
            then_body,
            elsif_branches,
            else_body,
            line,
            column,
        } => {
            let cond_ty = super::exprs::check_expr(checker, condition)?;
            if cond_ty != Type::Boolean {
                return Err(TypeError {
                    message: format!("`if` condition must be Boolean, found `{cond_ty}`"),
                    line: *line,
                    column: *column,
                });
            }
            let mut result = check_body_return_type(checker, then_body)?;
            for (cond, body) in elsif_branches {
                let ty = super::exprs::check_expr(checker, cond)?;
                if ty != Type::Boolean {
                    return Err(TypeError {
                        message: format!("`elsif` condition must be Boolean, found `{ty}`"),
                        line: cond.line(),
                        column: cond.column(),
                    });
                }
                let branch_ty = check_body_return_type(checker, body)?;
                result = combine_tail_types(result, branch_ty, *line, *column)?;
            }
            result = match else_body {
                Some(body) => {
                    let branch_ty = check_body_return_type(checker, body)?;
                    combine_tail_types(result, branch_ty, *line, *column)?
                }
                // No `else`: not every path yields a value, so this `if` can't
                // be relied on as a tail expression.
                None => None,
            };
            Ok(result)
        }
        _ => {
            check_stmt(checker, stmt)?;
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
