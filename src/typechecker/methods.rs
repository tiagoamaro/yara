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
        return Err(TypeError {
            message: format!(
                "`{object_ty}` has no method `{method}` (available: {})",
                crate::methods::names_for(kind).join(", ")
            ),
            line,
            column,
        });
    };

    if args.len() != m.arity {
        return Err(TypeError {
            message: format!(
                "`{object_ty}#{method}` expects {} argument(s), found {}",
                m.arity,
                args.len()
            ),
            line,
            column,
        });
    }

    (m.check)(checker, object_ty, args, line, column)
}

// Array methods

pub(crate) fn check_array_size(
    _checker: &mut TypeChecker,
    array_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match array_ty {
        Type::Array(_) => Ok(Type::Integer),
        other => Err(TypeError {
            message: format!("`Array#size` expects an array, found `{other}`"),
            line: _line,
            column: _column,
        }),
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
        Type::Array(elem) => Err(TypeError {
            message: format!("`Array<{elem}>#push` expects `{elem}`, found `{value_ty}`"),
            line,
            column,
        }),
        other => Err(TypeError {
            message: format!("`Array#push` expects an array, found `{other}`"),
            line,
            column,
        }),
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
        return Err(TypeError {
            message: format!("`Array#get` index must be `Integer`, found `{index_ty}`"),
            line,
            column,
        });
    }
    match array_ty {
        Type::Array(elem) => Ok(*elem.clone()),
        other => Err(TypeError {
            message: format!("`Array#get` expects an array, found `{other}`"),
            line,
            column,
        }),
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
        return Err(TypeError {
            message: format!("`Array#set` index must be `Integer`, found `{index_ty}`"),
            line,
            column,
        });
    }
    match array_ty {
        Type::Array(elem) if super::assignable(elem, &value_ty) => Ok(Type::Nil),
        Type::Array(elem) => Err(TypeError {
            message: format!("`Array<{elem}>#set` expects `{elem}`, found `{value_ty}`"),
            line,
            column,
        }),
        other => Err(TypeError {
            message: format!("`Array#set` expects an array, found `{other}`"),
            line,
            column,
        }),
    }
}

pub(crate) fn check_array_pop(
    _checker: &mut TypeChecker,
    array_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match array_ty {
        Type::Array(elem) => Ok(*elem.clone()),
        other => Err(TypeError {
            message: format!("`Array#pop` expects an array, found `{other}`"),
            line: _line,
            column: _column,
        }),
    }
}

pub(crate) fn check_array_is_empty(
    _checker: &mut TypeChecker,
    array_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match array_ty {
        Type::Array(_) => Ok(Type::Boolean),
        other => Err(TypeError {
            message: format!("`Array#is_empty` expects an array, found `{other}`"),
            line: _line,
            column: _column,
        }),
    }
}

// String methods

pub(crate) fn check_string_size(
    _checker: &mut TypeChecker,
    string_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match string_ty {
        Type::String => Ok(Type::Integer),
        other => Err(TypeError {
            message: format!("`String#size` expects a string, found `{other}`"),
            line: _line,
            column: _column,
        }),
    }
}

pub(crate) fn check_string_upper(
    _checker: &mut TypeChecker,
    string_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match string_ty {
        Type::String => Ok(Type::String),
        other => Err(TypeError {
            message: format!("`String#upper` expects a string, found `{other}`"),
            line: _line,
            column: _column,
        }),
    }
}

pub(crate) fn check_string_lower(
    _checker: &mut TypeChecker,
    string_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match string_ty {
        Type::String => Ok(Type::String),
        other => Err(TypeError {
            message: format!("`String#lower` expects a string, found `{other}`"),
            line: _line,
            column: _column,
        }),
    }
}

pub(crate) fn check_string_trim(
    _checker: &mut TypeChecker,
    string_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match string_ty {
        Type::String => Ok(Type::String),
        other => Err(TypeError {
            message: format!("`String#trim` expects a string, found `{other}`"),
            line: _line,
            column: _column,
        }),
    }
}

pub(crate) fn check_string_is_empty(
    _checker: &mut TypeChecker,
    string_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match string_ty {
        Type::String => Ok(Type::Boolean),
        other => Err(TypeError {
            message: format!("`String#is_empty` expects a string, found `{other}`"),
            line: _line,
            column: _column,
        }),
    }
}

pub(crate) fn check_string_to_i(
    _checker: &mut TypeChecker,
    string_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match string_ty {
        Type::String => Ok(Type::Integer),
        other => Err(TypeError {
            message: format!("`String#to_i` expects a string, found `{other}`"),
            line: _line,
            column: _column,
        }),
    }
}

pub(crate) fn check_string_to_f(
    _checker: &mut TypeChecker,
    string_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match string_ty {
        Type::String => Ok(Type::Float),
        other => Err(TypeError {
            message: format!("`String#to_f` expects a string, found `{other}`"),
            line: _line,
            column: _column,
        }),
    }
}

pub(crate) fn check_string_to_s(
    _checker: &mut TypeChecker,
    string_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match string_ty {
        Type::String => Ok(Type::String),
        other => Err(TypeError {
            message: format!("`String#to_s` expects a string, found `{other}`"),
            line: _line,
            column: _column,
        }),
    }
}

// Integer methods

pub(crate) fn check_int_to_s(
    _checker: &mut TypeChecker,
    int_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match int_ty {
        Type::Integer => Ok(Type::String),
        other => Err(TypeError {
            message: format!("`Integer#to_s` expects an integer, found `{other}`"),
            line: _line,
            column: _column,
        }),
    }
}

pub(crate) fn check_int_to_f(
    _checker: &mut TypeChecker,
    int_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match int_ty {
        Type::Integer => Ok(Type::Float),
        other => Err(TypeError {
            message: format!("`Integer#to_f` expects an integer, found `{other}`"),
            line: _line,
            column: _column,
        }),
    }
}

pub(crate) fn check_int_abs(
    _checker: &mut TypeChecker,
    int_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match int_ty {
        Type::Integer => Ok(Type::Integer),
        other => Err(TypeError {
            message: format!("`Integer#abs` expects an integer, found `{other}`"),
            line: _line,
            column: _column,
        }),
    }
}

// Float methods

pub(crate) fn check_float_to_s(
    _checker: &mut TypeChecker,
    float_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match float_ty {
        Type::Float => Ok(Type::String),
        other => Err(TypeError {
            message: format!("`Float#to_s` expects a float, found `{other}`"),
            line: _line,
            column: _column,
        }),
    }
}

pub(crate) fn check_float_to_i(
    _checker: &mut TypeChecker,
    float_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match float_ty {
        Type::Float => Ok(Type::Integer),
        other => Err(TypeError {
            message: format!("`Float#to_i` expects a float, found `{other}`"),
            line: _line,
            column: _column,
        }),
    }
}

pub(crate) fn check_float_abs(
    _checker: &mut TypeChecker,
    float_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match float_ty {
        Type::Float => Ok(Type::Float),
        other => Err(TypeError {
            message: format!("`Float#abs` expects a float, found `{other}`"),
            line: _line,
            column: _column,
        }),
    }
}

// Boolean methods

pub(crate) fn check_bool_to_s(
    _checker: &mut TypeChecker,
    bool_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match bool_ty {
        Type::Boolean => Ok(Type::String),
        other => Err(TypeError {
            message: format!("`Boolean#to_s` expects a boolean, found `{other}`"),
            line: _line,
            column: _column,
        }),
    }
}

// Pointer methods

pub(crate) fn check_ptr_deref(
    _checker: &mut TypeChecker,
    ptr_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match ptr_ty {
        Type::Pointer(elem) => Ok(*elem.clone()),
        other => Err(TypeError {
            message: format!("`Ptr#deref` expects a pointer, found `{other}`"),
            line: _line,
            column: _column,
        }),
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
        Type::Pointer(elem) => Err(TypeError {
            message: format!("`Ptr<{elem}>#set_deref` expects `{elem}`, found `{value_ty}`"),
            line,
            column,
        }),
        other => Err(TypeError {
            message: format!("`Ptr#set_deref` expects a pointer, found `{other}`"),
            line,
            column,
        }),
    }
}

pub(crate) fn check_ptr_free(
    _checker: &mut TypeChecker,
    ptr_ty: &Type,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Type, TypeError> {
    match ptr_ty {
        Type::Pointer(_) => Ok(Type::Nil),
        other => Err(TypeError {
            message: format!("`Ptr#free` expects a pointer, found `{other}`"),
            line: _line,
            column: _column,
        }),
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
