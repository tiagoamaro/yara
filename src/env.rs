//! A generic lexical-scope stack shared by the typechecker and interpreter.
//!
//! Both stages need the exact same machinery — a stack of name→binding maps,
//! searched innermost-first so inner declarations shadow outer ones — differing
//! only in *what* they bind a name to: the typechecker maps names to `Type`, the
//! interpreter to `Value`. That difference is the single type parameter `T`
//! here, so the mechanism lives in one place instead of being reimplemented
//! (identically) in each stage.
//!
//! This shares only the *container*: each stage still walks the AST with its own
//! separate logic (`check_*` vs `eval_*`). The scope stack was pure duplication
//! with no teaching value, so unifying it costs nothing and removes a
//! keep-in-sync hazard.

use std::collections::HashMap;

/// A stack of scopes, each a `name -> T` map. Innermost scope is last. Never
/// empty in normal use: [`Environment::new`] seeds one global scope, and
/// [`Environment::pop_scope`] is only ever called to undo a matching
/// [`Environment::push_scope`].
#[derive(Debug)]
pub struct Environment<T> {
    scopes: Vec<HashMap<String, T>>,
}

impl<T> Default for Environment<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Environment<T> {
    /// A fresh environment with a single empty (global) scope on the stack.
    pub fn new() -> Self {
        Environment {
            scopes: vec![HashMap::new()],
        }
    }

    /// Opens a new empty scope on top of the stack — entry to a function/method
    /// body or a `for` loop body, so bindings inside don't leak out. Must be
    /// paired with a later [`Environment::pop_scope`].
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Discards the innermost scope and every binding in it, restoring
    /// resolution to whatever scope was active before the matching
    /// [`Environment::push_scope`].
    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// Unconditionally binds `name` in the *innermost* scope, creating a fresh
    /// binding even if an outer scope already has that name (which is then
    /// shadowed for this scope's lifetime). Used for fresh locals, parameters,
    /// and loop variables.
    pub fn declare(&mut self, name: &str, value: T) {
        self.scopes
            .last_mut()
            .unwrap()
            .insert(name.to_string(), value);
    }

    /// Resolves `name` to its binding, searching innermost-first so an inner
    /// declaration shadows an enclosing one. `None` if unbound in every scope
    /// currently on the stack.
    pub fn lookup(&self, name: &str) -> Option<&T> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }

    /// Assignment semantics: walk innermost→outermost for an *existing* binding
    /// and mutate it in place; only if none exists anywhere, declare a fresh one
    /// in the innermost scope. This is what lets `x = x + 1` inside a loop body
    /// mutate the outer `x` rather than shadow it, while a first `x = 1` still
    /// introduces `x`.
    pub fn set_or_declare(&mut self, name: &str, value: T) {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), value);
                return;
            }
        }
        self.declare(name, value);
    }

    /// The innermost scope's raw map. Exposed for the interpreter's implicit-
    /// `self` copy-back in `run_method`, which reads whichever field-named
    /// bindings the method body left in its own (innermost) scope.
    pub fn current(&self) -> &HashMap<String, T> {
        self.scopes.last().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An inner-scope binding shadows an outer one, and popping the inner scope
    /// restores the outer binding.
    #[test]
    fn inner_scope_shadows_then_restores() {
        let mut env: Environment<i32> = Environment::new();
        env.declare("x", 1);
        env.push_scope();
        env.declare("x", 2);
        assert_eq!(env.lookup("x"), Some(&2));
        env.pop_scope();
        assert_eq!(env.lookup("x"), Some(&1));
    }

    /// `set_or_declare` mutates the nearest existing binding (even in an outer
    /// scope) rather than creating a shadow in the current scope.
    #[test]
    fn set_or_declare_mutates_outer_binding_in_place() {
        let mut env: Environment<i32> = Environment::new();
        env.declare("x", 1);
        env.push_scope();
        env.set_or_declare("x", 5);
        env.pop_scope();
        assert_eq!(env.lookup("x"), Some(&5));
    }

    /// `set_or_declare` on an unknown name introduces it in the innermost scope.
    #[test]
    fn set_or_declare_introduces_new_binding() {
        let mut env: Environment<i32> = Environment::new();
        env.set_or_declare("y", 9);
        assert_eq!(env.lookup("y"), Some(&9));
        assert!(env.current().contains_key("y"));
    }

    /// An unbound name resolves to `None`.
    #[test]
    fn unbound_name_is_none() {
        let env: Environment<i32> = Environment::new();
        assert_eq!(env.lookup("nope"), None);
    }
}
