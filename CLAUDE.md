# Yara — Project Map

Yara: learning-focused, strongly typed, compiled language. Ruby+Pascal hybrid syntax. Rust compiler/interpreter.

**Agents: when you modify a folder below, update that folder's CLAUDE.md before finishing your turn.** This includes every mermaid diagram touched by the change — `docs/architecture.md` and the `///` flowcharts in stage modules (e.g. `src/lexer/mod.rs`) — not just prose. CLAUDE.md files and diagrams are living documentation, kept current as an ongoing part of this project's progress, not a one-off pass.

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
- **Full-vocabulary translation**: done. Extends the old keyword-only translation system to *all* user-facing terms via a `Vocabulary` struct (`src/translations/mod.rs`) parsed from a sectioned `[keywords]/[types]/[builtins]/[methods]/[messages]` file format; `Rc<Vocabulary>` is threaded through `Lexer`/`Parser`/`Resolver`/`TypeChecker`/`Interpreter` (`with_vocabulary` constructors alongside the older English-only ones). CLI flag is `--vocabulary <path>` (`--keywords <path>` kept working as a backward-compat alias). `translations/pt.vocab` is the bundled Portuguese reference, covering all 5 sections; `examples/translations/hello_pt.yara` is written in fully-Portuguese vocabulary (keywords, types, builtins/methods, `.new`/`.novo`), and `examples/errors/runtime_error_pt.yara` demonstrates a Portuguese-language runtime error message. The `[messages]` catalog (`src/translations/messages.rs`) routes the large majority of user-facing error strings across lexer/parser/resolver/interpreter through `Vocabulary::msg`, but is not exhaustive — `typechecker/` and `diagnostics/` message conversion, and stage-label/call-stack-frame-word localization beyond what's already converted, are not fully swept; untranslated keys fall back to English by design. Docs updated: `docs/syntax.md` Translation section, `docs/architecture.md` pipeline diagrams. Out of scope, not started: translating identifiers/string contents (never translatable by design), and full coverage of every remaining `format!`-built message.
- **Everything-is-an-object (Ruby-style)**: done (Phase 4 complete, 2026-07-25). Primitive methods on Array/String/Integer/Float/Boolean/Pointer via `Expr::MethodCall` reusing the existing instance-method-call AST path; `src/methods.rs` registry keys entries by `(ReceiverKind, name)` and mirrors `src/builtins.rs` structure (arity, typecheck/eval function pointers per method); parens always required (`.size()` not `.size` — paren-less stays `Expr::FieldAccess`, untouched). 25 methods total: Array 6 (`size`/`push`/`get`/`set`/`pop`/`is_empty`), String 8 (`size`/`upper`/`lower`/`trim`/`is_empty`/`to_i`/`to_f`/`to_s`), Integer 3 (`to_s`/`to_f`/`abs`), Float 3 (`to_s`/`to_i`/`abs`), Boolean 1 (`to_s`), Pointer 3 (`deref`/`set_deref`/`free`), plus shared conversion methods (`to_s`/`to_i`/`to_f`). Free-function builtins kept unchanged and side-by-side (`len(xs)` and `xs.size()` both work). See `src/methods.rs`, `src/typechecker/methods.rs`, `src/interpreter/methods.rs`, `examples/methods.yara`. Out of scope: user-defined methods on primitives, class-level/static methods, method overloading on arity.
- **Class inheritance**: done (single parent, `class Child < Parent`, fields+methods inherit via flattening, no `super`, no override keyword — see `docs/syntax.md` Inheritance section, `src/typechecker/CLAUDE.md`, `src/interpreter/CLAUDE.md`). Class-level/static methods/fields (beyond the special `.new`), visibility modifiers, `super`, multiple inheritance — still deliberately out of scope; not started.
