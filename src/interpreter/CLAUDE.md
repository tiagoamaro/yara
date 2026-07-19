# interpreter/

Tree-walk evaluator executing a typechecked AST.

## Status
Implemented. `Interpreter::new().run_program(&[Stmt]) -> Result<(), RuntimeError>`. Wired into `main.rs` (`yara run <file>` executes lexer to parser to typechecker to interpreter in sequence, bailing with the first error).

## Design
- `Value` enum: `Integer, Float, Boolean, String, Nil` — runtime counterpart of `typechecker::Type`, but this crate stays decoupled from that module (no shared type).
- Scoping mirrors typechecker: `Vec<Scope>` stack, pushed/popped around function calls and `for` loops.
- `set_var` (used by `Stmt::VarDecl`/`ConstDecl`) walks the scope stack looking for an existing binding to mutate in place (needed for `x = x + 1` inside a `while` body to actually mutate the outer `x` rather than shadow it); falls back to declaring in the current scope if not found.
- Function calls: `call_function` pushes a `StackFrame { function_name, line, column }` onto `call_stack` before executing the body and pops it after — this is what populates `RuntimeError.call_stack` for the trace.
- **Implicit last-expression return** (Ruby-style): `exec_function_body` special-cases a trailing `Stmt::ExprStmt` to become the call's value, mirroring `typechecker::check_body_return_type`'s logic — if these two ever diverge, return-value bugs slip past the typechecker. Explicit `return expr` still works via `Flow::Return` short-circuiting through `exec_block`.
- `print(...)` is a built-in special-cased in `call_function` (joins args with a space via `Value`'s `Display`, `println!`s), same ad-hoc pattern as the typechecker's `print` handling — keep both in sync if a real stdlib/builtin registry replaces this.
- `RuntimeError::Display` prints rustc-adjacent multi-line output: message, `at line:column`, then each call-stack frame reversed (innermost first).

## Gotchas
- Division by zero is checked only for `Integer / Integer` (float division by zero silently yields `inf`/`NaN`, matching IEEE 754 — revisit if that's undesirable for a "friendly errors" language).
- `exec_function_body` duplicates the "is this the last statement" logic from `typechecker::check_body_return_type` — any AST changes to `Stmt` must update both in lockstep.
