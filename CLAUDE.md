# Yara — Project Map

Yara: learning-focused, strongly typed, compiled language. Ruby+Pascal hybrid syntax. Rust compiler/interpreter.

**Agents: when you modify a folder below, update that folder's CLAUDE.md before finishing your turn.**

## Layout

- `src/lexer/` — tokenizer. See `src/lexer/CLAUDE.md`.
- `src/ast/` — AST node definitions. See `src/ast/CLAUDE.md`.
- `src/diagnostics/` — shared error presentation: the `Diagnostic` trait every stage's error type implements, a `Span`/`Frame` position type (with a `file` hook for the future imported-file fix), and `render`/`render_snippet` (rustc-style header + source line + `^` caret). See `src/diagnostics/CLAUDE.md`.
- `src/env.rs` — `Environment<T>`, the generic lexical-scope stack (a `Vec<HashMap<String, T>>` searched innermost-first) shared by the typechecker (`Environment<Type>`) and interpreter (`Environment<Value>`). Same *container*, different binding type; each stage still walks the AST with its own separate logic. Replaces the two hand-duplicated `Scope` structs.
- `src/types.rs` — small type-name utility module (currently just `normalize_type_alias`, `Int`->`Integer` etc.), kept out of any single stage so the parser can canonicalize type names without importing from the lexer.
- `src/parser/` — recursive-descent parser, tokens to AST. See `src/parser/CLAUDE.md`.
- `src/typechecker/` — static type checking pass. See `src/typechecker/CLAUDE.md`.
- `src/interpreter/` — tree-walk evaluator. See `src/interpreter/CLAUDE.md`.
- `src/resolver/` — resolves `import "path"` statements before typechecking. See `src/resolver/CLAUDE.md`.
- `src/translations/` — parses keyword-translation files (`if = se`) into the keyword table `lexer::Lexer::with_keywords` uses, powering `yara run <file> --keywords <path>`. See `src/translations/CLAUDE.md`.
- `src/lib.rs` — the library crate root. Declares every compiler module (`pub mod ast/lexer/parser/resolver/typechecker/interpreter/translations`) so the whole pipeline is usable as a library and testable end to end. The `yara` binary is a thin wrapper over it. (Crate is both `[lib]` and `[[bin]]` in `Cargo.toml`.)
- `src/main.rs` — thin CLI over the library crate (`use yara::…`): argument parsing (`yara run <file> [--keywords <path>]`) plus a `stage` helper that wraps each pipeline stage and, on error, defers to `diagnostics::render`. Every lex/parse/import/type/runtime/translation error is printed rustc-style (file:line:col header, source line, `^` caret), with a snippet per call-stack frame for runtime errors. No compilation logic lives here — it only drives the library stages and renders their errors.
- `tests/` — end-to-end integration tests (`run_examples.rs`) driving the full public pipeline over every bundled example; the non-error examples must run clean and each `examples/errors/*` must fail at its expected stage. Only possible because the compiler is a library crate.
- `translations/` — bundled keyword-translation files (currently `pt.keywords`, Portuguese). See `translations/CLAUDE.md`.
- `examples/` — sample `.yara` programs. See `examples/CLAUDE.md`.
- `docs/syntax.md` — grammar notes, updated as syntax stabilizes.
- `docs/architecture.md` — Mermaid-diagrammed walkthrough of the real pipeline and each stage's internal control flow, for anyone studying how this compiler/interpreter is built. Every function in `src/` also carries a `///` doc comment explaining its mechanics (not just its name) — keep both in sync when changing a stage's algorithm, not just its behavior.
- `editors/vscode-yara/` — VS Code syntax highlighting (TextMate grammar), no LSP/diagnostics. Not part of the Rust build; keep its keyword/type lists in sync with `src/lexer/mod.rs` if either changes. See its own `CLAUDE.md`.

## Conventions

- Every token/AST node carries `(line, column)` position — required for diagnostics, not optional.
- Errors (lexer/parser/resolver/typechecker/runtime/translation) must report exact line:column with a source excerpt + caret, rustc-style. Each stage keeps its own error type (locality) but implements `diagnostics::Diagnostic`; rendering is centralized in `diagnostics::render` (invoked by `main.rs`'s `stage` helper), not in the individual stages — they still only carry line/column + message (runtime additionally carries a call-stack trace).
- Type aliases are interchangeable and normalized at lex/parse time: `Int`=`Integer`, `Bool`=`Boolean`, `Str`=`String`.
- No implicit numeric coercion (Int vs Float stays strict).
- Rust version pinned via `.tool-versions` (asdf).
- Always run `cargo fmt` before finishing a change; code must pass `cargo fmt --check`.
- Cover new logic with unit tests wherever feasible (colocated `#[cfg(test)] mod tests` per module); run `cargo test` and confirm green before finishing a change.

## Status

Milestones 1-6 done: lexer, AST, parser, typechecker, interpreter all implemented; `yara run <file>` works end to end. Since then: unary negation, `if`/`elsif`/`else` as a function's tail expression, file `import`, a minimal `Array` type (`IntArray`/`FloatArray`/`BoolArray`/`StringArray` with `[]` literals, `arr[i]` indexing, and `len`/`push`/`pop`/`get`/`set` builtins), a `class` feature (const/instance-var/initializer/method, `.new` construction, `.field` read/write, implicit-`self` name resolution, no inheritance), and configurable keyword translation (`yara run <file> --keywords <path>`, e.g. `translations/pt.keywords`) have all landed. 78 unit tests + 2 end-to-end integration tests (`tests/run_examples.rs`) passing, `cargo fmt` clean. `examples/data_structures/` demonstrates list/stack/queue/linked-list/binary-tree/graph built on arrays; `examples/objects/` demonstrates classes; `examples/translations/` demonstrates keyword translation.

**In-progress: modularization/isolation refactor** (behavior-preserving; plan at `~/.claude/plans/let-s-do-a-major-sharded-wadler.md`). Done so far: Phase 0 — library crate (`src/lib.rs`) + thin CLI binary + `tests/` integration net; Phase 1 — shared diagnostics (`src/diagnostics/`: `Diagnostic` trait, `Span`/`Frame`, one `render`); Phase 2 — shared `Environment<T>` scope stack (`src/env.rs`) replacing both stages' duplicated `Scope`. Remaining phases: single-source-of-truth tables (type names / keywords / builtin registry), god-function splitting, and a docs sync. Earlier plan: `~/.claude/plans/cosmic-purring-stream.md`.

## TODO

- Native codegen (LLVM/Cranelift or C transpile) — deferred, not started.
- **Pointers + memory management (teaching GC)**: opt-in, not a silent footgun forced on everyone. Sketch:
  - Syntax: a `Ptr<T>` type (or `T*`, TBD) plus `alloc`/`deref`/`free`-style builtins (mirroring `Array`'s `push`/`get`/`set` ad-hoc-builtin pattern in `typechecker`/`interpreter`), giving explicit manual allocation on a heap the interpreter actually models (not just Rust's own heap under the hood) — the point is to make allocation *visible*, not to be a fast/real allocator.
  - Default mode stays exactly as today: `Value`/`Type` untouched, arrays/classes keep their current `Rc<RefCell<..>>` reference semantics, existing programs unaffected.
  - Opt-in mode: a new `Value::Pointer(usize)` (a heap index/handle, not a Rust reference) plus an `Interpreter`-owned heap (`Vec<Option<Value>>` or similar) that `alloc`/`deref`/`free` operate on; `free`-ing and then `deref`-ing an already-freed slot is a `RuntimeError` (use-after-free made *visible and diagnosable* — the pedagogical point), not UB.
  - GC teaching angle: once manual `alloc`/`free` exists, add a second opt-in mode with a toy mark-and-sweep collector over that same heap (a `collect()` builtin or automatic-between-statements sweep), so a learner can compare manual memory management vs. GC side by side in the same language. This is the actual teaching payload — a real compiler course topic made runnable.
  - Needs: new `Type`/`Value` variants, typechecker rules (a `Ptr<T>` derefs to `T`), interpreter heap + builtins, examples under `examples/pointers/` demonstrating leaks, use-after-free-as-error, and manual-vs-GC. Not started — bigger than the `Array`/`class` cuts, budget accordingly.
- **Class inheritance**, class-level/static methods/fields (beyond the special `.new`), and visibility modifiers — all deliberately out of scope for the first `class` cut; not started.
- Known soundness gap: an instance field declared with no value (`count: Integer`) is `Nil` at runtime until some method assigns it — the typechecker doesn't track "definitely assigned before use," so reading a field before `initializer` sets it type-checks fine but is a `Nil` where e.g. `Integer` was expected. Not fixed yet.
- Known gap: error rendering (`main.rs::print_error`) always reads the source snippet from the entry file the user ran `yara run` on, never from whichever file a position actually originated in. Since `import` splices statements from other files in before typechecking/interpretation runs (see `src/resolver/CLAUDE.md`), an error whose line:column belongs to an imported file renders the wrong (or out-of-range) snippet. Fixing it means threading a file path alongside every error's line/column, not just the entry path — not done yet.
