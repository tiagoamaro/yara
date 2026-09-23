//! Registry of Yara's methods on **primitive** receivers (`Integer`, `Float`,
//! `Boolean`, `String`, arrays, pointers) — the "everything is an object"
//! surface, e.g. `xs.size()`, `2.to_s()`, `"3".to_i()`, `p.deref()`.
//!
//! Mirrors `src/builtins.rs`'s shape (name + arity + two function pointers,
//! compile-time-enforced dual implementation) but keyed by `(ReceiverKind,
//! name)` instead of name alone, since the same method name can exist on
//! multiple receiver kinds (`to_s` on every primitive) with different
//! implementations. Unlike `builtins.rs`, the receiver has *already been
//! resolved* by the time a `CheckFn`/`EvalFn` here runs (a `Type` in the
//! typechecker, a `Value` in the interpreter) — both hook sites
//! (`typechecker/classes.rs::check_method_call`,
//! `interpreter/classes.rs::call_method`) reach the non-`Instance` branch
//! only after `check_expr`/`eval_expr` has already produced it.
//!
//! Adding a method means: (1) add a `Method` entry here with function
//! pointers, (2) implement the `MethodCheckFn` in
//! `src/typechecker/methods.rs`, and (3) implement the `MethodEvalFn` in
//! `src/interpreter/methods.rs`. The compile-time entry of each field
//! enforces that both implementations exist.
//!
//! Instance methods (user-defined classes) are untouched by this module —
//! they still go through `checker.classes`/`self.classes` method tables, not
//! this registry. `Type::Instance`/`Value::Instance` (and `Nil`) map to
//! `ReceiverKind::of_type`/`of_value` returning `None`, so `nil.foo()` and
//! ordinary instance method calls keep falling through to the existing
//! class-method-table / "no method" error paths untouched.
//!
//! Free-function builtins (`len`, `push`, `deref`, ...) in `src/builtins.rs`
//! are intentionally kept alongside these methods — `len(xs)` and
//! `xs.size()` both work, calling into shared logic where practical.

/// Which kind of receiver a method attaches to. Bridges typechecker `Type`
/// and interpreter `Value` without either stage depending on the other's
/// types directly (both convert their own value into this common key).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReceiverKind {
    Integer,
    Float,
    Boolean,
    String,
    Array,
    Pointer,
}

impl ReceiverKind {
    /// Maps a typechecker `Type` to its `ReceiverKind`, or `None` if the
    /// type isn't a primitive-method receiver (`Instance` goes through the
    /// ordinary class method table instead; `Nil` has no methods).
    pub fn of_type(t: &crate::typechecker::Type) -> Option<ReceiverKind> {
        match t {
            crate::typechecker::Type::Integer => Some(ReceiverKind::Integer),
            crate::typechecker::Type::Float => Some(ReceiverKind::Float),
            crate::typechecker::Type::Boolean => Some(ReceiverKind::Boolean),
            crate::typechecker::Type::String => Some(ReceiverKind::String),
            crate::typechecker::Type::Array(_) => Some(ReceiverKind::Array),
            crate::typechecker::Type::Pointer(_) => Some(ReceiverKind::Pointer),
            crate::typechecker::Type::Nil | crate::typechecker::Type::Instance(_) => None,
        }
    }

    /// Maps an interpreter `Value` to its `ReceiverKind`, or `None` if the
    /// value isn't a primitive-method receiver (`Instance` goes through the
    /// ordinary class method table instead; `Nil` has no methods).
    pub fn of_value(v: &crate::interpreter::Value) -> Option<ReceiverKind> {
        match v {
            crate::interpreter::Value::Integer(_) => Some(ReceiverKind::Integer),
            crate::interpreter::Value::Float(_) => Some(ReceiverKind::Float),
            crate::interpreter::Value::Boolean(_) => Some(ReceiverKind::Boolean),
            crate::interpreter::Value::String(_) => Some(ReceiverKind::String),
            crate::interpreter::Value::Array(_) => Some(ReceiverKind::Array),
            crate::interpreter::Value::Pointer(_) => Some(ReceiverKind::Pointer),
            crate::interpreter::Value::Nil | crate::interpreter::Value::Instance(_, _) => None,
        }
    }
}

pub type MethodCheckFn = fn(
    &mut crate::typechecker::TypeChecker,
    &crate::typechecker::Type,
    &[crate::ast::Expr],
    usize,
    usize,
) -> Result<crate::typechecker::Type, crate::typechecker::TypeError>;

pub type MethodEvalFn = fn(
    &mut crate::interpreter::Interpreter,
    crate::interpreter::Value,
    &[crate::ast::Expr],
    usize,
    usize,
) -> Result<crate::interpreter::Value, crate::interpreter::RuntimeError>;

/// One method's receiver kind, name, argument count, and dispatch function
/// pointers.
#[derive(Clone, Copy)]
pub struct Method {
    /// Which receiver kind this method attaches to.
    pub receiver: ReceiverKind,
    /// The name it's called by in source (e.g. `size` or `to_s`).
    pub name: &'static str,
    /// Exact number of arguments (the typechecker enforces this; the
    /// interpreter then trusts it and indexes the args directly).
    pub arity: usize,
    /// The typechecker's type-checking function for this method.
    pub check: MethodCheckFn,
    /// The interpreter's execution function for this method.
    pub eval: MethodEvalFn,
}

/// Every primitive method. Adding one here: (1) add a `Method` entry with
/// function pointers, (2) implement the `MethodCheckFn` in
/// `src/typechecker/methods.rs`, and (3) implement the `MethodEvalFn` in
/// `src/interpreter/methods.rs`. The compile-time field requirements
/// enforce that both implementations exist.
pub const METHODS: &[Method] = &[
    // Array
    Method {
        receiver: ReceiverKind::Array,
        name: "size",
        arity: 0,
        check: crate::typechecker::check_array_size,
        eval: crate::interpreter::eval_array_size,
    },
    Method {
        receiver: ReceiverKind::Array,
        name: "push",
        arity: 1,
        check: crate::typechecker::check_array_push,
        eval: crate::interpreter::eval_array_push,
    },
    Method {
        receiver: ReceiverKind::Array,
        name: "get",
        arity: 1,
        check: crate::typechecker::check_array_get,
        eval: crate::interpreter::eval_array_get,
    },
    Method {
        receiver: ReceiverKind::Array,
        name: "set",
        arity: 2,
        check: crate::typechecker::check_array_set,
        eval: crate::interpreter::eval_array_set,
    },
    Method {
        receiver: ReceiverKind::Array,
        name: "pop",
        arity: 0,
        check: crate::typechecker::check_array_pop,
        eval: crate::interpreter::eval_array_pop,
    },
    Method {
        receiver: ReceiverKind::Array,
        name: "is_empty",
        arity: 0,
        check: crate::typechecker::check_array_is_empty,
        eval: crate::interpreter::eval_array_is_empty,
    },
    // String
    Method {
        receiver: ReceiverKind::String,
        name: "size",
        arity: 0,
        check: crate::typechecker::check_string_size,
        eval: crate::interpreter::eval_string_size,
    },
    Method {
        receiver: ReceiverKind::String,
        name: "upper",
        arity: 0,
        check: crate::typechecker::check_string_upper,
        eval: crate::interpreter::eval_string_upper,
    },
    Method {
        receiver: ReceiverKind::String,
        name: "lower",
        arity: 0,
        check: crate::typechecker::check_string_lower,
        eval: crate::interpreter::eval_string_lower,
    },
    Method {
        receiver: ReceiverKind::String,
        name: "trim",
        arity: 0,
        check: crate::typechecker::check_string_trim,
        eval: crate::interpreter::eval_string_trim,
    },
    Method {
        receiver: ReceiverKind::String,
        name: "is_empty",
        arity: 0,
        check: crate::typechecker::check_string_is_empty,
        eval: crate::interpreter::eval_string_is_empty,
    },
    Method {
        receiver: ReceiverKind::String,
        name: "to_i",
        arity: 0,
        check: crate::typechecker::check_string_to_i,
        eval: crate::interpreter::eval_string_to_i,
    },
    Method {
        receiver: ReceiverKind::String,
        name: "to_f",
        arity: 0,
        check: crate::typechecker::check_string_to_f,
        eval: crate::interpreter::eval_string_to_f,
    },
    Method {
        receiver: ReceiverKind::String,
        name: "to_s",
        arity: 0,
        check: crate::typechecker::check_string_to_s,
        eval: crate::interpreter::eval_string_to_s,
    },
    // Integer
    Method {
        receiver: ReceiverKind::Integer,
        name: "to_s",
        arity: 0,
        check: crate::typechecker::check_int_to_s,
        eval: crate::interpreter::eval_int_to_s,
    },
    Method {
        receiver: ReceiverKind::Integer,
        name: "to_f",
        arity: 0,
        check: crate::typechecker::check_int_to_f,
        eval: crate::interpreter::eval_int_to_f,
    },
    Method {
        receiver: ReceiverKind::Integer,
        name: "abs",
        arity: 0,
        check: crate::typechecker::check_int_abs,
        eval: crate::interpreter::eval_int_abs,
    },
    // Float
    Method {
        receiver: ReceiverKind::Float,
        name: "to_s",
        arity: 0,
        check: crate::typechecker::check_float_to_s,
        eval: crate::interpreter::eval_float_to_s,
    },
    Method {
        receiver: ReceiverKind::Float,
        name: "to_i",
        arity: 0,
        check: crate::typechecker::check_float_to_i,
        eval: crate::interpreter::eval_float_to_i,
    },
    Method {
        receiver: ReceiverKind::Float,
        name: "abs",
        arity: 0,
        check: crate::typechecker::check_float_abs,
        eval: crate::interpreter::eval_float_abs,
    },
    // Boolean
    Method {
        receiver: ReceiverKind::Boolean,
        name: "to_s",
        arity: 0,
        check: crate::typechecker::check_bool_to_s,
        eval: crate::interpreter::eval_bool_to_s,
    },
    // Pointer
    Method {
        receiver: ReceiverKind::Pointer,
        name: "deref",
        arity: 0,
        check: crate::typechecker::check_ptr_deref,
        eval: crate::interpreter::eval_ptr_deref,
    },
    Method {
        receiver: ReceiverKind::Pointer,
        name: "set_deref",
        arity: 1,
        check: crate::typechecker::check_ptr_set_deref,
        eval: crate::interpreter::eval_ptr_set_deref,
    },
    Method {
        receiver: ReceiverKind::Pointer,
        name: "free",
        arity: 0,
        check: crate::typechecker::check_ptr_free,
        eval: crate::interpreter::eval_ptr_free,
    },
];

/// Looks up a method by receiver kind and name, returning its registry
/// entry (with check/eval function pointers), or `None` if no such method
/// exists on that receiver kind (in which case the caller renders a "no
/// method" error, ideally using `names_for` to suggest what does exist).
pub fn lookup(receiver: ReceiverKind, name: &str) -> Option<&'static Method> {
    METHODS
        .iter()
        .find(|m| m.receiver == receiver && m.name == name)
}

/// All method names available on a given receiver kind, in registry order.
/// Used to build "no method `x` on `Integer` (available: ...)" errors.
pub fn names_for(receiver: ReceiverKind) -> Vec<&'static str> {
    METHODS
        .iter()
        .filter(|m| m.receiver == receiver)
        .map(|m| m.name)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_duplicate_receiver_name_pairs() {
        for (i, a) in METHODS.iter().enumerate() {
            for b in &METHODS[i + 1..] {
                assert!(
                    !(a.receiver == b.receiver && a.name == b.name),
                    "duplicate method `{}` on {:?}",
                    a.name,
                    a.receiver
                );
            }
        }
    }

    #[test]
    fn of_type_and_of_value_agree_on_every_kind() {
        use crate::interpreter::Value;
        use crate::typechecker::Type;

        let pairs: &[(Type, Value)] = &[
            (Type::Integer, Value::Integer(0)),
            (Type::Float, Value::Float(0.0)),
            (Type::Boolean, Value::Boolean(false)),
            (Type::String, Value::String(String::new())),
        ];
        for (ty, val) in pairs {
            assert_eq!(
                ReceiverKind::of_type(ty),
                ReceiverKind::of_value(val),
                "mismatch for {ty:?}"
            );
        }
    }

    #[test]
    fn instance_and_nil_have_no_receiver_kind() {
        use crate::typechecker::Type;
        assert_eq!(ReceiverKind::of_type(&Type::Nil), None);
        assert_eq!(
            ReceiverKind::of_type(&Type::Instance("Foo".to_string())),
            None
        );
    }

    #[test]
    fn lookup_finds_registered_methods() {
        assert!(lookup(ReceiverKind::Array, "size").is_some());
        assert!(lookup(ReceiverKind::Integer, "to_s").is_some());
        assert!(lookup(ReceiverKind::Integer, "nope").is_none());
        assert!(lookup(ReceiverKind::Array, "to_s").is_none());
    }

    #[test]
    fn names_for_lists_only_that_receivers_methods() {
        let names = names_for(ReceiverKind::Boolean);
        assert_eq!(names, vec!["to_s"]);
    }
}
