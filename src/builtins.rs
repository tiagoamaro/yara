//! Registry of Yara's builtins: array operations (`len`, `push`, `get`, `set`, `pop`)
//! and pointer operations (`alloc`, `deref`, `set_deref`, `free`).
//!
//! These aren't user-defined functions and aren't in any `functions` table;
//! they're recognized ad hoc by name in both the typechecker (which type-checks
//! the call) and the interpreter (which executes it). Before this registry, the
//! name set and per-builtin arity lived duplicated inside both stages' dispatch
//! `match`es, so adding a builtin meant editing arity numbers in two places and
//! risking them drifting apart.
//!
//! This module is the single source of truth for *which* names are builtins
//! and *how many* arguments each takes. Each stage still owns its own
//! *behavior* — the typechecker's type rules and the interpreter's execution
//! are deliberately kept as separate parallel `match`es, not unified, so each
//! stage reads as its own straightforward walk.
//!
//! `print` is intentionally not here: it's a variadic I/O builtin handled on a
//! different code path in each stage, not a fixed-arity builtin operation.

/// One builtin's name and its exact argument count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Builtin {
    /// The name it's called by in source (e.g. `push` or `alloc`).
    pub name: &'static str,
    /// Exact number of arguments (the typechecker enforces this; the
    /// interpreter then trusts it and indexes the args directly).
    pub arity: usize,
}

/// Every builtin. Adding one here is step one; the second and third steps
/// are a type-checking arm in the typechecker and an execution arm in the
/// interpreter (the `every_builtin_is_handled_by_both_stages` integration test
/// enforces both).
pub const BUILTINS: &[Builtin] = &[
    Builtin {
        name: "len",
        arity: 1,
    },
    Builtin {
        name: "push",
        arity: 2,
    },
    Builtin {
        name: "get",
        arity: 2,
    },
    Builtin {
        name: "set",
        arity: 3,
    },
    Builtin {
        name: "pop",
        arity: 1,
    },
    Builtin {
        name: "alloc",
        arity: 1,
    },
    Builtin {
        name: "deref",
        arity: 1,
    },
    Builtin {
        name: "set_deref",
        arity: 2,
    },
    Builtin {
        name: "free",
        arity: 1,
    },
];

/// Looks up an array builtin by name, or `None` if `name` isn't one (in which
/// case the caller falls through to user-defined function resolution).
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
