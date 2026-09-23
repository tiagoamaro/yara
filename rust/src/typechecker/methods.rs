use super::*;

/// Dispatcher for primitive method calls. Routes to the appropriate per-method
/// `MethodCheckFn` based on receiver kind and method name. Arity is checked once
/// here from the registry; the function pointer dispatch can then trust `args`
/// has exactly the right number of elements.
pub(super) fn check_primitive_method(
    checker: &mut TypeChecker,
    kind: crate::methods::ReceiverKind,
    object_ty: &Type,
    method: &str,
    args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Type, TypeError> {
    let canonical_method = checker.vocab.canonical_method(method);
    let Some(m) = crate::methods::lookup(kind, &canonical_method) else {
        let object_name = checker.vocab.type_name(object_ty);
        let available = checker
            .vocab
            .localized_method_names(&crate::methods::names_for(kind))
            .join(", ");
        return Err(TypeError {
            message: checker.vocab.msg(
                "type/no-method-available",
                &[&object_name, method, &available],
            ),
            line,
            column,
        });
    };

    if args.len() != m.arity {
        let object_name = checker.vocab.type_name(object_ty);
        let expected = m.arity.to_string();
        let found = args.len().to_string();
        return Err(TypeError {
            message: checker.vocab.msg(
                "type/method-arity-mismatch",
                &[&object_name, method, &expected, &found],
            ),
            line,
            column,
        });
    }

    (m.check)(checker, object_ty, args, line, column)
}

// Array methods

pub(crate) fn check_array_size(
    checker: &mut TypeChecker,
    array_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match array_ty {
        Type::Array(_) => Ok(Type::Integer),
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker.vocab.msg("type/array-size-expects-array", &[&name]),
                line: _line,
                column: _column,
            })
        }
    }
}

pub(crate) fn check_array_push(
    checker: &mut TypeChecker,
    array_ty: &Type,
    args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Type, TypeError> {
    let value_ty = super::expressions::check_expr(checker, &args[0])?;
    match array_ty {
        Type::Array(elem) if super::assignable(elem, &value_ty) => Ok(Type::Nil),
        Type::Array(elem) => {
            let elem_name = checker.vocab.type_name(elem);
            let value_name = checker.vocab.type_name(&value_ty);
            Err(TypeError {
                message: checker
                    .vocab
                    .msg("type/array-push-mismatch", &[&elem_name, &value_name]),
                line,
                column,
            })
        }
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker.vocab.msg("type/array-push-expects-array", &[&name]),
                line,
                column,
            })
        }
    }
}

pub(crate) fn check_array_get(
    checker: &mut TypeChecker,
    array_ty: &Type,
    args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Type, TypeError> {
    let index_ty = super::expressions::check_expr(checker, &args[0])?;
    if index_ty != Type::Integer {
        let index_name = checker.vocab.type_name(&index_ty);
        return Err(TypeError {
            message: checker
                .vocab
                .msg("type/array-get-index-not-integer", &[&index_name]),
            line,
            column,
        });
    }
    match array_ty {
        Type::Array(elem) => Ok(*elem.clone()),
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker.vocab.msg("type/array-get-expects-array", &[&name]),
                line,
                column,
            })
        }
    }
}

pub(crate) fn check_array_set(
    checker: &mut TypeChecker,
    array_ty: &Type,
    args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Type, TypeError> {
    let index_ty = super::expressions::check_expr(checker, &args[0])?;
    let value_ty = super::expressions::check_expr(checker, &args[1])?;
    if index_ty != Type::Integer {
        let index_name = checker.vocab.type_name(&index_ty);
        return Err(TypeError {
            message: checker
                .vocab
                .msg("type/array-set-index-not-integer", &[&index_name]),
            line,
            column,
        });
    }
    match array_ty {
        Type::Array(elem) if super::assignable(elem, &value_ty) => Ok(Type::Nil),
        Type::Array(elem) => {
            let elem_name = checker.vocab.type_name(elem);
            let value_name = checker.vocab.type_name(&value_ty);
            Err(TypeError {
                message: checker
                    .vocab
                    .msg("type/array-set-mismatch", &[&elem_name, &value_name]),
                line,
                column,
            })
        }
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker.vocab.msg("type/array-set-expects-array", &[&name]),
                line,
                column,
            })
        }
    }
}

pub(crate) fn check_array_pop(
    checker: &mut TypeChecker,
    array_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match array_ty {
        Type::Array(elem) => Ok(*elem.clone()),
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker.vocab.msg("type/array-pop-expects-array", &[&name]),
                line: _line,
                column: _column,
            })
        }
    }
}

pub(crate) fn check_array_is_empty(
    checker: &mut TypeChecker,
    array_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match array_ty {
        Type::Array(_) => Ok(Type::Boolean),
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker
                    .vocab
                    .msg("type/array-is-empty-expects-array", &[&name]),
                line: _line,
                column: _column,
            })
        }
    }
}

// String methods

pub(crate) fn check_string_size(
    checker: &mut TypeChecker,
    string_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match string_ty {
        Type::String => Ok(Type::Integer),
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker
                    .vocab
                    .msg("type/string-size-expects-string", &[&name]),
                line: _line,
                column: _column,
            })
        }
    }
}

pub(crate) fn check_string_upper(
    checker: &mut TypeChecker,
    string_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match string_ty {
        Type::String => Ok(Type::String),
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker
                    .vocab
                    .msg("type/string-upper-expects-string", &[&name]),
                line: _line,
                column: _column,
            })
        }
    }
}

pub(crate) fn check_string_lower(
    checker: &mut TypeChecker,
    string_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match string_ty {
        Type::String => Ok(Type::String),
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker
                    .vocab
                    .msg("type/string-lower-expects-string", &[&name]),
                line: _line,
                column: _column,
            })
        }
    }
}

pub(crate) fn check_string_trim(
    checker: &mut TypeChecker,
    string_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match string_ty {
        Type::String => Ok(Type::String),
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker
                    .vocab
                    .msg("type/string-trim-expects-string", &[&name]),
                line: _line,
                column: _column,
            })
        }
    }
}

pub(crate) fn check_string_is_empty(
    checker: &mut TypeChecker,
    string_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match string_ty {
        Type::String => Ok(Type::Boolean),
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker
                    .vocab
                    .msg("type/string-is-empty-expects-string", &[&name]),
                line: _line,
                column: _column,
            })
        }
    }
}

pub(crate) fn check_string_to_i(
    checker: &mut TypeChecker,
    string_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match string_ty {
        Type::String => Ok(Type::Integer),
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker
                    .vocab
                    .msg("type/string-to-i-expects-string", &[&name]),
                line: _line,
                column: _column,
            })
        }
    }
}

pub(crate) fn check_string_to_f(
    checker: &mut TypeChecker,
    string_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match string_ty {
        Type::String => Ok(Type::Float),
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker
                    .vocab
                    .msg("type/string-to-f-expects-string", &[&name]),
                line: _line,
                column: _column,
            })
        }
    }
}

pub(crate) fn check_string_to_s(
    checker: &mut TypeChecker,
    string_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match string_ty {
        Type::String => Ok(Type::String),
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker
                    .vocab
                    .msg("type/string-to-s-expects-string", &[&name]),
                line: _line,
                column: _column,
            })
        }
    }
}

// Integer methods

pub(crate) fn check_int_to_s(
    checker: &mut TypeChecker,
    int_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match int_ty {
        Type::Integer => Ok(Type::String),
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker.vocab.msg("type/int-to-s-expects-int", &[&name]),
                line: _line,
                column: _column,
            })
        }
    }
}

pub(crate) fn check_int_to_f(
    checker: &mut TypeChecker,
    int_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match int_ty {
        Type::Integer => Ok(Type::Float),
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker.vocab.msg("type/int-to-f-expects-int", &[&name]),
                line: _line,
                column: _column,
            })
        }
    }
}

pub(crate) fn check_int_abs(
    checker: &mut TypeChecker,
    int_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match int_ty {
        Type::Integer => Ok(Type::Integer),
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker.vocab.msg("type/int-abs-expects-int", &[&name]),
                line: _line,
                column: _column,
            })
        }
    }
}

// Float methods

pub(crate) fn check_float_to_s(
    checker: &mut TypeChecker,
    float_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match float_ty {
        Type::Float => Ok(Type::String),
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker.vocab.msg("type/float-to-s-expects-float", &[&name]),
                line: _line,
                column: _column,
            })
        }
    }
}

pub(crate) fn check_float_to_i(
    checker: &mut TypeChecker,
    float_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match float_ty {
        Type::Float => Ok(Type::Integer),
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker.vocab.msg("type/float-to-i-expects-float", &[&name]),
                line: _line,
                column: _column,
            })
        }
    }
}

pub(crate) fn check_float_abs(
    checker: &mut TypeChecker,
    float_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match float_ty {
        Type::Float => Ok(Type::Float),
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker.vocab.msg("type/float-abs-expects-float", &[&name]),
                line: _line,
                column: _column,
            })
        }
    }
}

// Boolean methods

pub(crate) fn check_bool_to_s(
    checker: &mut TypeChecker,
    bool_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match bool_ty {
        Type::Boolean => Ok(Type::String),
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker.vocab.msg("type/bool-to-s-expects-bool", &[&name]),
                line: _line,
                column: _column,
            })
        }
    }
}

// Pointer methods

pub(crate) fn check_ptr_deref(
    checker: &mut TypeChecker,
    ptr_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match ptr_ty {
        Type::Pointer(elem) => Ok(*elem.clone()),
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker.vocab.msg("type/ptr-deref-expects-ptr", &[&name]),
                line: _line,
                column: _column,
            })
        }
    }
}

pub(crate) fn check_ptr_set_deref(
    checker: &mut TypeChecker,
    ptr_ty: &Type,
    args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Type, TypeError> {
    let value_ty = super::expressions::check_expr(checker, &args[0])?;
    match ptr_ty {
        Type::Pointer(elem) if super::assignable(elem, &value_ty) => Ok(Type::Nil),
        Type::Pointer(elem) => {
            let elem_name = checker.vocab.type_name(elem);
            let value_name = checker.vocab.type_name(&value_ty);
            Err(TypeError {
                message: checker
                    .vocab
                    .msg("type/ptr-set-deref-mismatch", &[&elem_name, &value_name]),
                line,
                column,
            })
        }
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker
                    .vocab
                    .msg("type/ptr-set-deref-expects-ptr", &[&name]),
                line,
                column,
            })
        }
    }
}

pub(crate) fn check_ptr_free(
    checker: &mut TypeChecker,
    ptr_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match ptr_ty {
        Type::Pointer(_) => Ok(Type::Nil),
        other => {
            let name = checker.vocab.type_name(other);
            Err(TypeError {
                message: checker.vocab.msg("type/ptr-free-expects-ptr", &[&name]),
                line: _line,
                column: _column,
            })
        }
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
    fn array_size_method() {
        assert!(check("xs: IntArray = [1, 2, 3]\nn: Int = xs.size()").is_ok());
    }

    #[test]
    fn array_push_method() {
        assert!(check("xs: IntArray = []\nxs.push(5)").is_ok());
    }

    #[test]
    fn array_push_wrong_type() {
        let err = check("xs: IntArray = []\nxs.push(\"bad\")").unwrap_err();
        assert!(err.message.contains("push"));
    }

    #[test]
    fn array_get_method() {
        assert!(check("xs: IntArray = [1, 2]\nn: Int = xs.get(0)").is_ok());
    }

    #[test]
    fn array_set_method() {
        assert!(check("xs: IntArray = [1, 2]\nxs.set(0, 99)").is_ok());
    }

    #[test]
    fn array_pop_method() {
        assert!(check("xs: IntArray = [1, 2]\nn: Int = xs.pop()").is_ok());
    }

    #[test]
    fn array_is_empty_method() {
        assert!(check("xs: IntArray = []\nb: Boolean = xs.is_empty()").is_ok());
    }

    #[test]
    fn string_size_method() {
        assert!(check("s: String = \"hello\"\nn: Int = s.size()").is_ok());
    }

    #[test]
    fn string_upper_method() {
        assert!(check("s: String = \"hello\"\ns2: String = s.upper()").is_ok());
    }

    #[test]
    fn string_lower_method() {
        assert!(check("s: String = \"HELLO\"\ns2: String = s.lower()").is_ok());
    }

    #[test]
    fn string_trim_method() {
        assert!(check("s: String = \"  hi  \"\ns2: String = s.trim()").is_ok());
    }

    #[test]
    fn string_is_empty_method() {
        assert!(check("s: String = \"\"\nb: Boolean = s.is_empty()").is_ok());
    }

    #[test]
    fn string_to_i_method() {
        assert!(check("s: String = \"42\"\nn: Int = s.to_i()").is_ok());
    }

    #[test]
    fn string_to_f_method() {
        assert!(check("s: String = \"3.14\"\nf: Float = s.to_f()").is_ok());
    }

    #[test]
    fn string_to_s_method() {
        assert!(check("s: String = \"hello\"\ns2: String = s.to_s()").is_ok());
    }

    #[test]
    fn int_to_s_method() {
        assert!(check("n: Int = 42\ns: String = n.to_s()").is_ok());
    }

    #[test]
    fn int_to_f_method() {
        assert!(check("n: Int = 42\nf: Float = n.to_f()").is_ok());
    }

    #[test]
    fn int_abs_method() {
        assert!(check("n: Int = -42\nm: Int = n.abs()").is_ok());
    }

    #[test]
    fn float_to_s_method() {
        assert!(check("f: Float = 3.14\ns: String = f.to_s()").is_ok());
    }

    #[test]
    fn float_to_i_method() {
        assert!(check("f: Float = 3.14\nn: Int = f.to_i()").is_ok());
    }

    #[test]
    fn float_abs_method() {
        assert!(check("f: Float = -3.14\ng: Float = f.abs()").is_ok());
    }

    #[test]
    fn bool_to_s_method() {
        assert!(check("b: Boolean = true\ns: String = b.to_s()").is_ok());
    }

    #[test]
    fn ptr_deref_method() {
        assert!(check("p: Ptr<Int> = alloc(5)\nn: Int = p.deref()").is_ok());
    }

    #[test]
    fn ptr_set_deref_method() {
        assert!(check("p: Ptr<Int> = alloc(5)\np.set_deref(10)").is_ok());
    }

    #[test]
    fn ptr_free_method() {
        assert!(check("p: Ptr<Int> = alloc(5)\np.free()").is_ok());
    }

    #[test]
    fn unknown_method_error() {
        let err = check("n: Int = 42\nresult = n.nope()").unwrap_err();
        assert!(err.message.contains("no method"));
    }

    #[test]
    fn method_arity_error() {
        let err = check("n: Int = 42\nresult = n.to_s(1)").unwrap_err();
        assert!(err.message.contains("expects 0 argument"));
    }

    #[test]
    fn conversion_result_usable_in_assignment() {
        assert!(check("n: Int = 42\nf: Float = n.to_f()").is_ok());
    }
}
