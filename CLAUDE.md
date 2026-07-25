# Yara — Project Map

Yara: learning-focused, strongly typed, compiled language. Ruby+Pascal hybrid syntax. Rust compiler/interpreter.

**Agents: when you modify a folder below, update that folder's CLAUDE.md before finishing your turn.**

## Layout

Pipeline order: `lexer` → `parser` → `resolver` → `typechecker` → `interpreter`.
Each has its own `CLAUDE.md`; so do `src/ast/`, `src/diagnostics/`, `src/translations/`,
`src/` (the flat `env.rs`/`types.rs`/`builtins.rs`/`lib.rs`/`main.rs`), `translations/`,
`examples/`, and `editors/vscode-yara/`. Read the folder's file before working in it.

Non-obvious bits that aren't in a folder doc:

- `tests/` — end-to-end integration tests over every bundled example. Contract: non-error examples must run clean, and each `examples/errors/*` must fail at its *expected stage*. Only possible because the crate is both `[lib]` and `[[bin]]`.
- `docs/architecture.md` — walkthrough of each stage's internal control flow. Every function in `src/` also carries a `///` doc comment explaining its mechanics — keep both in sync when changing a stage's *algorithm*, not just its behavior.
- `docs/syntax.md` — grammar notes, updated as syntax stabilizes.
- `docs/plan-next-milestones.md` — remaining phases and their order; its Progress section at the top is the live status of this project.
- `editors/vscode-yara/` — not part of the Rust build, so nothing fails if it drifts: keep its keyword/type lists in sync with `src/lexer/mod.rs` by hand.

## Conventions

- Source file and module names are spelled-out full words (`statements.rs`, `expressions.rs`), never abbreviations — explicitness over brevity, this is a teaching codebase.
- Every token/AST node carries `(line, column)` position — required for diagnostics, not optional. After `import` resolution, AST line numbers are shifted into disjoint virtual ranges per imported file (via `Stmt::shift_lines`/`Expr::shift_lines`), then mapped back to their original file + line during rendering via `diagnostics::SourceMap`.
- Errors (lexer/parser/resolver/typechecker/runtime/translation) must report exact line:column with a source excerpt + caret, rustc-style. Each stage keeps its own error type (locality) but implements `diagnostics::Diagnostic`; rendering is centralized in `diagnostics::render` (invoked by `main.rs`'s `stage` helper), not in the individual stages — they still only carry line/column + message (runtime additionally carries a call-stack trace).
- Type aliases are interchangeable and normalized at lex/parse time: `Int`=`Integer`, `Bool`=`Boolean`, `Str`=`String`.
- No implicit numeric coercion (Int vs Float stays strict).
- Rust version pinned via `.tool-versions` (asdf).
- Always run `cargo fmt` before finishing a change; code must pass `cargo fmt --check`.
- Cover new logic with unit tests wherever feasible (colocated `#[cfg(test)] mod tests` per module); run `cargo test` and confirm green before finishing a change.

## TODO

- Native codegen (LLVM/Cranelift or C transpile) — deferred, not started.
- **Teaching GC (Phase 3b)**: `collect()` mark-and-sweep builtin landed (roots from every environment scope, chased through arrays/instances/pointee slots, returns freed count; see `src/interpreter/CLAUDE.md`); side-by-side manual-vs-GC examples in `examples/pointers/gc.yara`.
- **Full-vocabulary translation**: extend the keyword-translation system (`--keywords`, `src/translations/`) beyond keywords to *all* user-facing terms — type names (`Integer`, `Boolean`, `Ptr`, ...), boolean literals (`true`/`false`), builtin function names (`len`/`push`/`alloc`/`deref`/..., `print`), so a learner can write fully localized programs (e.g. all-Portuguese including `Inteiro`, `verdadeiro`, `tamanho`). Needs design: translation currently happens at lexing (keyword table only); type names and builtins are resolved later stages by canonical English names, so either the lexer/parser normalizes translated names to canonical ones, or each stage's lookup grows a translation layer. Not started.
- **Everything-is-an-object (Ruby-style)**: make every basic value an object with methods — `[1,2].size`, `2.to_s`, `"3".to_i`, etc. — replacing/complementing the free-function builtins (`len(xs)` becomes `xs.size`). Big design shift: method dispatch on primitive types (typechecker needs per-type method tables, not just `Instance` classes; interpreter needs `MethodCall` on non-`Instance` values), plus a conversion-method family (`to_s`/`to_i`/`to_f`). Could reuse the existing `Expr::MethodCall` path. Keep the old free-function builtins during transition or drop them — decide then. Not started, design needed.
- **Class inheritance**, class-level/static methods/fields (beyond the special `.new`), and visibility modifiers — all deliberately out of scope for the first `class` cut; not started.
