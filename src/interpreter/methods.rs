use super::*;

/// Dispatches a primitive method call by looking it up in the methods registry,
/// then invoking its eval function. Returns a RuntimeError if the method name
/// doesn't exist (defense-in-depth; the typechecker should already have rejected it).
pub(crate) fn eval_primitive_method(
    interp: &mut Interpreter,
    kind: crate::methods::ReceiverKind,
    receiver: Value,
    method: &str,
    args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Value, RuntimeError> {
    let canonical_method = interp.vocab.canonical_method(method);
    let m = crate::methods::lookup(kind, &canonical_method).ok_or_else(|| RuntimeError {
        message: format!("no method `{method}` for this value"),
        line,
        column,
        call_stack: interp.call_stack.clone(),
    })?;
    (m.eval)(interp, receiver, args, line, column)
}

// Array methods

pub(crate) fn eval_array_size(
    _interp: &mut Interpreter,
    receiver: Value,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Value, RuntimeError> {
    if let Value::Array(items) = receiver {
        let len = items.borrow().len() as i64;
        Ok(Value::Integer(len))
    } else {
        unreachable!("eval_array_size only called for arrays")
    }
}

pub(crate) fn eval_array_push(
    interp: &mut Interpreter,
    receiver: Value,
    args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Value, RuntimeError> {
    let value = interp.eval_expr(&args[0])?;
    if let Value::Array(items) = receiver {
        items.borrow_mut().push(value);
        Ok(Value::Nil)
    } else {
        unreachable!("eval_array_push only called for arrays")
    }
}

pub(crate) fn eval_array_get(
    interp: &mut Interpreter,
    receiver: Value,
    args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Value, RuntimeError> {
    let idx = interp.eval_int(&args[0], line, column)?;
    interp.array_get(&receiver, idx, line, column)
}

pub(crate) fn eval_array_set(
    interp: &mut Interpreter,
    receiver: Value,
    args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Value, RuntimeError> {
    let idx = interp.eval_int(&args[0], line, column)?;
    let value = interp.eval_expr(&args[1])?;
    if let Value::Array(items) = receiver {
        let mut items_mut = items.borrow_mut();
        let Some(slot) = usize::try_from(idx).ok().and_then(|i| items_mut.get_mut(i)) else {
            return Err(RuntimeError {
                message: format!(
                    "array index {idx} out of bounds (length {})",
                    items_mut.len()
                ),
                line,
                column,
                call_stack: interp.call_stack.clone(),
            });
        };
        *slot = value;
        Ok(Value::Nil)
    } else {
        unreachable!("eval_array_set only called for arrays")
    }
}

pub(crate) fn eval_array_pop(
    interp: &mut Interpreter,
    receiver: Value,
    _args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Value, RuntimeError> {
    if let Value::Array(items) = receiver {
        items.borrow_mut().pop().ok_or_else(|| RuntimeError {
            message: "cannot `pop` from an empty array".to_string(),
            line,
            column,
            call_stack: interp.call_stack.clone(),
        })
    } else {
        unreachable!("eval_array_pop only called for arrays")
    }
}

pub(crate) fn eval_array_is_empty(
    _interp: &mut Interpreter,
    receiver: Value,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Value, RuntimeError> {
    if let Value::Array(items) = receiver {
        Ok(Value::Boolean(items.borrow().is_empty()))
    } else {
        unreachable!("eval_array_is_empty only called for arrays")
    }
}

// String methods

pub(crate) fn eval_string_size(
    _interp: &mut Interpreter,
    receiver: Value,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Value, RuntimeError> {
    if let Value::String(s) = receiver {
        Ok(Value::Integer(s.chars().count() as i64))
    } else {
        unreachable!("eval_string_size only called for strings")
    }
}

pub(crate) fn eval_string_upper(
    _interp: &mut Interpreter,
    receiver: Value,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Value, RuntimeError> {
    if let Value::String(s) = receiver {
        Ok(Value::String(s.to_uppercase()))
    } else {
        unreachable!("eval_string_upper only called for strings")
    }
}

pub(crate) fn eval_string_lower(
    _interp: &mut Interpreter,
    receiver: Value,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Value, RuntimeError> {
    if let Value::String(s) = receiver {
        Ok(Value::String(s.to_lowercase()))
    } else {
        unreachable!("eval_string_lower only called for strings")
    }
}

pub(crate) fn eval_string_trim(
    _interp: &mut Interpreter,
    receiver: Value,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Value, RuntimeError> {
    if let Value::String(s) = receiver {
        Ok(Value::String(s.trim().to_string()))
    } else {
        unreachable!("eval_string_trim only called for strings")
    }
}

pub(crate) fn eval_string_is_empty(
    _interp: &mut Interpreter,
    receiver: Value,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Value, RuntimeError> {
    if let Value::String(s) = receiver {
        Ok(Value::Boolean(s.is_empty()))
    } else {
        unreachable!("eval_string_is_empty only called for strings")
    }
}

pub(crate) fn eval_string_to_i(
    interp: &mut Interpreter,
    receiver: Value,
    _args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Value, RuntimeError> {
    if let Value::String(s) = receiver {
        s.trim()
            .parse::<i64>()
            .map(Value::Integer)
            .map_err(|_| RuntimeError {
                message: format!("cannot parse `{s}` as an Integer"),
                line,
                column,
                call_stack: interp.call_stack.clone(),
            })
    } else {
        unreachable!("eval_string_to_i only called for strings")
    }
}

pub(crate) fn eval_string_to_f(
    interp: &mut Interpreter,
    receiver: Value,
    _args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Value, RuntimeError> {
    if let Value::String(s) = receiver {
        s.trim()
            .parse::<f64>()
            .map(Value::Float)
            .map_err(|_| RuntimeError {
                message: format!("cannot parse `{s}` as a Float"),
                line,
                column,
                call_stack: interp.call_stack.clone(),
            })
    } else {
        unreachable!("eval_string_to_f only called for strings")
    }
}

pub(crate) fn eval_string_to_s(
    _interp: &mut Interpreter,
    receiver: Value,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Value, RuntimeError> {
    if let Value::String(s) = receiver {
        Ok(Value::String(s))
    } else {
        unreachable!("eval_string_to_s only called for strings")
    }
}

// Integer methods

pub(crate) fn eval_int_to_s(
    _interp: &mut Interpreter,
    receiver: Value,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Value, RuntimeError> {
    if let Value::Integer(n) = receiver {
        Ok(Value::String(n.to_string()))
    } else {
        unreachable!("eval_int_to_s only called for integers")
    }
}

pub(crate) fn eval_int_to_f(
    _interp: &mut Interpreter,
    receiver: Value,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Value, RuntimeError> {
    if let Value::Integer(n) = receiver {
        Ok(Value::Float(n as f64))
    } else {
        unreachable!("eval_int_to_f only called for integers")
    }
}

pub(crate) fn eval_int_abs(
    _interp: &mut Interpreter,
    receiver: Value,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Value, RuntimeError> {
    if let Value::Integer(n) = receiver {
        Ok(Value::Integer(n.abs()))
    } else {
        unreachable!("eval_int_abs only called for integers")
    }
}

// Float methods

pub(crate) fn eval_float_to_s(
    _interp: &mut Interpreter,
    receiver: Value,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Value, RuntimeError> {
    if let Value::Float(f) = receiver {
        Ok(Value::String(f.to_string()))
    } else {
        unreachable!("eval_float_to_s only called for floats")
    }
}

pub(crate) fn eval_float_to_i(
    _interp: &mut Interpreter,
    receiver: Value,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Value, RuntimeError> {
    if let Value::Float(f) = receiver {
        Ok(Value::Integer(f.trunc() as i64))
    } else {
        unreachable!("eval_float_to_i only called for floats")
    }
}

pub(crate) fn eval_float_abs(
    _interp: &mut Interpreter,
    receiver: Value,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Value, RuntimeError> {
    if let Value::Float(f) = receiver {
        Ok(Value::Float(f.abs()))
    } else {
        unreachable!("eval_float_abs only called for floats")
    }
}

// Boolean methods

pub(crate) fn eval_bool_to_s(
    _interp: &mut Interpreter,
    receiver: Value,
    _args: &[Expr],
    _line: usize,
    _column: usize,
) -> Result<Value, RuntimeError> {
    if let Value::Boolean(b) = receiver {
        let s = if b { "true" } else { "false" }.to_string();
        Ok(Value::String(s))
    } else {
        unreachable!("eval_bool_to_s only called for booleans")
    }
}

// Pointer methods

pub(crate) fn eval_ptr_deref(
    interp: &mut Interpreter,
    receiver: Value,
    _args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Value, RuntimeError> {
    super::calls::heap_read(interp, &receiver, line, column)
}

pub(crate) fn eval_ptr_set_deref(
    interp: &mut Interpreter,
    receiver: Value,
    args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Value, RuntimeError> {
    let value = interp.eval_expr(&args[0])?;
    super::calls::heap_write(interp, &receiver, value, line, column)?;
    Ok(Value::Nil)
}

pub(crate) fn eval_ptr_free(
    interp: &mut Interpreter,
    receiver: Value,
    _args: &[Expr],
    line: usize,
    column: usize,
) -> Result<Value, RuntimeError> {
    super::calls::heap_free(interp, &receiver, line, column)?;
    Ok(Value::Nil)
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

    #[test]
    fn array_size() {
        let interp = run("xs: IntArray = [1, 2, 3]\nn = xs.size()").unwrap();
        assert_eq!(interp.lookup_var("n"), Some(&Value::Integer(3)));
    }

    #[test]
    fn array_push_mutates_original() {
        let interp = run("xs: IntArray = [1, 2]\nxs.push(3)\nn = xs.size()").unwrap();
        assert_eq!(interp.lookup_var("n"), Some(&Value::Integer(3)));
    }

    #[test]
    fn array_pop_from_empty_errors() {
        let err = run("xs: IntArray = []\nxs.pop()").unwrap_err();
        assert_eq!(err.message, "cannot `pop` from an empty array");
    }

    #[test]
    fn array_get() {
        let interp = run("xs: IntArray = [10, 20, 30]\nv = xs.get(1)").unwrap();
        assert_eq!(interp.lookup_var("v"), Some(&Value::Integer(20)));
    }

    #[test]
    fn array_set() {
        let interp = run("xs: IntArray = [1, 2, 3]\nxs.set(1, 99)\nv = xs.get(1)").unwrap();
        assert_eq!(interp.lookup_var("v"), Some(&Value::Integer(99)));
    }

    #[test]
    fn array_is_empty() {
        let interp =
            run("xs: IntArray = [1]\nys: IntArray = []\na = xs.is_empty()\nb = ys.is_empty()")
                .unwrap();
        assert_eq!(interp.lookup_var("a"), Some(&Value::Boolean(false)));
        assert_eq!(interp.lookup_var("b"), Some(&Value::Boolean(true)));
    }

    #[test]
    fn string_size() {
        let interp = run("s = \"hello\"\nn = s.size()").unwrap();
        assert_eq!(interp.lookup_var("n"), Some(&Value::Integer(5)));
    }

    #[test]
    fn string_upper() {
        let interp = run("s = \"hello\"\nu = s.upper()").unwrap();
        assert_eq!(
            interp.lookup_var("u"),
            Some(&Value::String("HELLO".to_string()))
        );
    }

    #[test]
    fn string_lower() {
        let interp = run("s = \"HELLO\"\nl = s.lower()").unwrap();
        assert_eq!(
            interp.lookup_var("l"),
            Some(&Value::String("hello".to_string()))
        );
    }

    #[test]
    fn string_trim() {
        let interp = run("s = \"  hello  \"\nt = s.trim()").unwrap();
        assert_eq!(
            interp.lookup_var("t"),
            Some(&Value::String("hello".to_string()))
        );
    }

    #[test]
    fn string_is_empty() {
        let interp = run("s = \"\"\nt = \"x\"\na = s.is_empty()\nb = t.is_empty()").unwrap();
        assert_eq!(interp.lookup_var("a"), Some(&Value::Boolean(true)));
        assert_eq!(interp.lookup_var("b"), Some(&Value::Boolean(false)));
    }

    #[test]
    fn string_to_i() {
        let interp = run("s = \"42\"\nn = s.to_i()").unwrap();
        assert_eq!(interp.lookup_var("n"), Some(&Value::Integer(42)));
    }

    #[test]
    fn string_to_i_parse_error() {
        let err = run("s = \"not_a_number\"\nn = s.to_i()").unwrap_err();
        assert!(err.message.contains("cannot parse"));
        assert!(err.message.contains("as an Integer"));
    }

    #[test]
    fn string_to_f() {
        let interp = run("s = \"3.14\"\nf = s.to_f()").unwrap();
        assert_eq!(interp.lookup_var("f"), Some(&Value::Float(3.14)));
    }

    #[test]
    fn string_to_s() {
        let interp = run("s = \"hello\"\nt = s.to_s()").unwrap();
        assert_eq!(
            interp.lookup_var("t"),
            Some(&Value::String("hello".to_string()))
        );
    }

    #[test]
    fn int_to_s() {
        let interp = run("n = 42\ns = n.to_s()").unwrap();
        assert_eq!(
            interp.lookup_var("s"),
            Some(&Value::String("42".to_string()))
        );
    }

    #[test]
    fn int_to_f() {
        let interp = run("n = 5\nf = n.to_f()").unwrap();
        assert_eq!(interp.lookup_var("f"), Some(&Value::Float(5.0)));
    }

    #[test]
    fn int_abs() {
        let interp = run("a = -10\nb = 5\nx = a.abs()\ny = b.abs()").unwrap();
        assert_eq!(interp.lookup_var("x"), Some(&Value::Integer(10)));
        assert_eq!(interp.lookup_var("y"), Some(&Value::Integer(5)));
    }

    #[test]
    fn float_to_s() {
        let interp = run("f = 2.5\ns = f.to_s()").unwrap();
        assert_eq!(
            interp.lookup_var("s"),
            Some(&Value::String("2.5".to_string()))
        );
    }

    #[test]
    fn float_to_i() {
        let interp = run("f = 3.7\nn = f.to_i()").unwrap();
        assert_eq!(interp.lookup_var("n"), Some(&Value::Integer(3)));
    }

    #[test]
    fn float_abs() {
        let interp = run("a = -2.5\nb = 3.0\nx = a.abs()\ny = b.abs()").unwrap();
        assert_eq!(interp.lookup_var("x"), Some(&Value::Float(2.5)));
        assert_eq!(interp.lookup_var("y"), Some(&Value::Float(3.0)));
    }

    #[test]
    fn bool_to_s() {
        let interp = run("a = true\nb = false\ns = a.to_s()\nt = b.to_s()").unwrap();
        assert_eq!(
            interp.lookup_var("s"),
            Some(&Value::String("true".to_string()))
        );
        assert_eq!(
            interp.lookup_var("t"),
            Some(&Value::String("false".to_string()))
        );
    }

    #[test]
    fn pointer_deref() {
        let interp = run("p: Ptr<Integer> = alloc(42)\nv = p.deref()").unwrap();
        assert_eq!(interp.lookup_var("v"), Some(&Value::Integer(42)));
    }

    #[test]
    fn pointer_set_deref() {
        let interp = run("p: Ptr<Integer> = alloc(10)\np.set_deref(99)\nv = p.deref()").unwrap();
        assert_eq!(interp.lookup_var("v"), Some(&Value::Integer(99)));
    }

    #[test]
    fn pointer_free_then_deref_errors() {
        let err = run("p: Ptr<Integer> = alloc(42)\np.free()\nv = p.deref()").unwrap_err();
        assert!(err.message.contains("use after free"));
    }
}
