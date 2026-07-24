use super::*;

/// Pre-registers every top-level function's parameter and return types
/// into `self.functions`, without checking any body. Run before any
/// bodies are checked so that a call to a function defined later in the
/// file (or a recursive call to the function currently being checked)
/// already has a known signature to check argument types/arity against.
pub(super) fn collect_function_signatures(
    checker: &mut TypeChecker,
    program: &[Stmt],
) -> Result<(), TypeError> {
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
                param_types.push(checker.resolve_type(&p.type_ann.name, p.line, p.column)?);
            }
            let return_type = match return_type {
                Some(t) => Some(checker.resolve_type(&t.name, *line, *column)?),
                None => None,
            };
            checker.functions.insert(
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

/// Type-checks array and pointer builtins if `callee` names one of them, returning
/// `Ok(None)` for any other callee so the caller falls through to user-defined
/// function lookup.
/// Type-checks a free function call `callee(args)`, resolving in order: the
/// `print` builtin (any args, yields `Nil`), the array/pointer builtins (via
/// `check_array_builtin`), then a user-defined function in `self.functions`
/// — checking argument count and each argument's type against the signature.
/// Yields the function's declared return type, or `Nil` if it declares none.
/// Split out of `check_expr`'s `Call` arm to keep that dispatch readable.
pub(super) fn check_call(
    checker: &mut TypeChecker,
    callee: &str,
    args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Type, TypeError> {
    if callee == "print" {
        for a in args {
            super::exprs::check_expr(checker, a)?;
        }
        return Ok(Type::Nil);
    }
    if let Some(ty) = check_array_builtin(checker, callee, args, line, column)? {
        return Ok(ty);
    }
    let sig = checker
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
        let arg_ty = super::exprs::check_expr(checker, arg)?;
        if arg_ty != *expected {
            return Err(TypeError {
                message: format!("argument to `{callee}` expects `{expected}`, found `{arg_ty}`"),
                line: arg.line(),
                column: arg.column(),
            });
        }
    }
    Ok(sig.return_type.unwrap_or(Type::Nil))
}

/// Shared arity/type-checking for any call-like site that already has a
/// resolved `FunctionSig` — user-defined top-level function calls,
/// instance method calls, and `.new` construction all funnel through
/// here. Checks `args.len()` matches `sig.param_types.len()` first (using
/// `what` — e.g. `"add"`, `"Hello#greet"`, `"Hello.new"` — to name the
/// callee in the error), then checks each argument expression and
/// requires its type to exactly equal the corresponding declared
/// parameter type (no implicit coercion).
pub(super) fn check_call_args(
    checker: &mut TypeChecker,
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
        let arg_ty = super::exprs::check_expr(checker, arg)?;
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

/// Type-checks array and pointer builtins. Arity is checked once here from the
/// registry; the per-builtin arms below can then trust args have exactly the
/// right number of elements. Handles `len`/`push`/`get`/`set`/`pop` (array
/// builtins), `alloc`/`deref`/`set_deref`/`free` (pointer builtins), and `collect` (GC).
fn check_array_builtin(
    checker: &mut TypeChecker,
    callee: &str,
    args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Option<Type>, TypeError> {
    let Some(builtin) = crate::builtins::lookup(callee) else {
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
        "len" => match super::exprs::check_expr(checker, &args[0])? {
            Type::Array(_) => Ok(Some(Type::Integer)),
            other => Err(TypeError {
                message: format!("`len` expects an array, found `{other}`"),
                line,
                column,
            }),
        },
        "push" => {
            let array_ty = super::exprs::check_expr(checker, &args[0])?;
            let value_ty = super::exprs::check_expr(checker, &args[1])?;
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
            let array_ty = super::exprs::check_expr(checker, &args[0])?;
            let index_ty = super::exprs::check_expr(checker, &args[1])?;
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
            let array_ty = super::exprs::check_expr(checker, &args[0])?;
            let index_ty = super::exprs::check_expr(checker, &args[1])?;
            let value_ty = super::exprs::check_expr(checker, &args[2])?;
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
        "pop" => match super::exprs::check_expr(checker, &args[0])? {
            Type::Array(elem) => Ok(Some(*elem)),
            other => Err(TypeError {
                message: format!("`pop` expects an array, found `{other}`"),
                line,
                column,
            }),
        },
        "alloc" => {
            let value_ty = super::exprs::check_expr(checker, &args[0])?;
            Ok(Some(Type::Pointer(Box::new(value_ty))))
        }
        "deref" => {
            let ptr_ty = super::exprs::check_expr(checker, &args[0])?;
            match ptr_ty {
                Type::Pointer(elem) => Ok(Some(*elem)),
                other => Err(TypeError {
                    message: format!("`deref` expects a pointer, found `{other}`"),
                    line,
                    column,
                }),
            }
        }
        "set_deref" => {
            let ptr_ty = super::exprs::check_expr(checker, &args[0])?;
            let value_ty = super::exprs::check_expr(checker, &args[1])?;
            match ptr_ty {
                Type::Pointer(elem) if *elem == value_ty => Ok(Some(Type::Nil)),
                Type::Pointer(elem) => Err(TypeError {
                    message: format!(
                        "`set_deref` into `Ptr<{elem}>` expects `{elem}`, found `{value_ty}`"
                    ),
                    line,
                    column,
                }),
                other => Err(TypeError {
                    message: format!("`set_deref` expects a pointer, found `{other}`"),
                    line,
                    column,
                }),
            }
        }
        "free" => {
            let ptr_ty = super::exprs::check_expr(checker, &args[0])?;
            match ptr_ty {
                Type::Pointer(_) => Ok(Some(Type::Nil)),
                other => Err(TypeError {
                    message: format!("`free` expects a pointer, found `{other}`"),
                    line,
                    column,
                }),
            }
        }
        // Mark-and-sweep GC over the pointer heap; yields the freed-slot count.
        "collect" => Ok(Some(Type::Integer)),
        _ => unreachable!("builtin `{callee}` is in the registry but has no typecheck arm"),
    }
}
