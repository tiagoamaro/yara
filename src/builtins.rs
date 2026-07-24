//! Registry of Yara's builtins: array operations (`len`, `push`, `get`, `set`, `pop`),
//! pointer operations (`alloc`, `deref`, `set_deref`, `free`), and GC (`collect`).
//!
//! These aren't user-defined functions and aren't in any `functions` table;
//! they're recognized ad hoc by name in both the typechecker (which type-checks
//! the call) and the interpreter (which executes it). Before this registry, the
//! name set and per-builtin arity lived duplicated inside both stages' dispatch
//! `match`es, so adding a builtin meant editing arity numbers in two places and
//! risking them drifting apart.
//!
//! This module is now the single source of truth for *which* names are builtins,
//! *how many* arguments each takes, *and* the wiring of both the typechecker's
//! type-checking logic and the interpreter's execution logic via function pointers.
//! Adding a builtin means: (1) add a `Builtin` entry here with function pointers,
//! (2) implement the `CheckFn` in `src/typechecker/calls.rs`, and (3) implement
//! the `EvalFn` in `src/interpreter/calls.rs`. The compile-time entry of each
//! `CheckFn` and `EvalFn` field enforces that both implementations exist.
//!
//! `print` is intentionally not here: it's a variadic I/O builtin handled on a
//! different code path in each stage, not a fixed-arity builtin operation.

pub type CheckFn = fn(
    &mut crate::typechecker::TypeChecker,
    &[crate::ast::Expr],
    usize,
    usize,
) -> Result<crate::typechecker::Type, crate::typechecker::TypeError>;

pub type EvalFn = fn(
    &mut crate::interpreter::Interpreter,
    &[crate::ast::Expr],
    usize,
    usize,
) -> Result<crate::interpreter::Value, crate::interpreter::RuntimeError>;

/// One builtin's name, argument count, and dispatch function pointers.
#[derive(Clone, Copy)]
pub struct Builtin {
    /// The name it's called by in source (e.g. `push` or `alloc`).
    pub name: &'static str,
    /// Exact number of arguments (the typechecker enforces this; the
    /// interpreter then trusts it and indexes the args directly).
    pub arity: usize,
    /// The typechecker's type-checking function for this builtin.
    pub check: CheckFn,
    /// The interpreter's execution function for this builtin.
    pub eval: EvalFn,
}

/// Every builtin. Adding one here: (1) add a `Builtin` entry with function
/// pointers, (2) implement the `CheckFn` in `src/typechecker/calls.rs`, and
/// (3) implement the `EvalFn` in `src/interpreter/calls.rs`. The compile-time
/// field requirements enforce that both implementations exist.
pub const BUILTINS: &[Builtin] = &[
    Builtin {
        name: "len",
        arity: 1,
        check: crate::typechecker::check_len,
        eval: crate::interpreter::eval_len,
    },
    Builtin {
        name: "push",
        arity: 2,
        check: crate::typechecker::check_push,
        eval: crate::interpreter::eval_push,
    },
    Builtin {
        name: "get",
        arity: 2,
        check: crate::typechecker::check_get,
        eval: crate::interpreter::eval_get,
    },
    Builtin {
        name: "set",
        arity: 3,
        check: crate::typechecker::check_set,
        eval: crate::interpreter::eval_set,
    },
    Builtin {
        name: "pop",
        arity: 1,
        check: crate::typechecker::check_pop,
        eval: crate::interpreter::eval_pop,
    },
    Builtin {
        name: "alloc",
        arity: 1,
        check: crate::typechecker::check_alloc,
        eval: crate::interpreter::eval_alloc,
    },
    Builtin {
        name: "deref",
        arity: 1,
        check: crate::typechecker::check_deref,
        eval: crate::interpreter::eval_deref,
    },
    Builtin {
        name: "set_deref",
        arity: 2,
        check: crate::typechecker::check_set_deref,
        eval: crate::interpreter::eval_set_deref,
    },
    Builtin {
        name: "free",
        arity: 1,
        check: crate::typechecker::check_free,
        eval: crate::interpreter::eval_free,
    },
    Builtin {
        name: "collect",
        arity: 0,
        check: crate::typechecker::check_collect,
        eval: crate::interpreter::eval_collect,
    },
];

/// Looks up a builtin by name, returning its registry entry (with check/eval
/// function pointers), or `None` if `name` isn't one (in which case the caller
/// falls through to user-defined function resolution).
pub fn lookup(name: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|b| b.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Names must be unique, or `lookup` would shadow later entries.
    #[test]
    fn builtin_names_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for b in BUILTINS {
            assert!(seen.insert(b.name), "duplicate builtin name: {}", b.name);
        }
    }

    #[test]
    fn lookup_finds_known_and_misses_unknown() {
        assert_eq!(lookup("push").map(|b| b.arity), Some(2));
        assert_eq!(lookup("set_deref").map(|b| b.arity), Some(2));
        assert!(lookup("print").is_none());
        assert!(lookup("nope").is_none());
    }
}
