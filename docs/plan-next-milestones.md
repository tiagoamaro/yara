# Yara — Next Milestones Plan

Status baseline: 88 unit + 3 integration tests green, `cargo fmt` clean, modularization refactor done.
Execution policy: implement with parallel Haiku sub-agents (thinking OFF), one agent per file/area; main thread (Sonnet) plans, splits work, reviews, and runs `cargo fmt` + `cargo test` gates between phases.

---

## Phase 1 — Fix imported-file snippet rendering (known gap, small, foundational)

Problem: `Span` has a `file` hook but errors always render snippets from the entry file; positions from imported files show wrong/out-of-range snippets.

Steps:
1. `src/resolver/` — when splicing imported statements, record origin file per statement (populate `Span.file` / attach path to positions). Decide: either rewrite positions to carry file, or keep a side map `(line range -> file)` — prefer carrying the file in each error's span (locality, matches existing `file` hook).
2. Thread file through error types: lexer/parser errors already come from a known file (the one being lexed/parsed) — set it at construction. Typechecker/runtime errors take file from the AST node's span.
   - Requires `Stmt`/`Expr` positions to optionally carry a file (or an interned file id). Cheapest cut: `Option<Rc<str>>` on `Span`, `None` = entry file.
3. `src/main.rs` `stage`/render path — read snippet from the span's file when present, entry file otherwise. Out-of-range guard stays.
4. Tests: new `examples/errors/import_error.yara` (+ imported helper file) asserting the snippet comes from the imported file; golden-compare error output.

Agent split: resolver agent, diagnostics agent, main.rs agent, test-authoring agent — resolver first (defines the data shape), rest parallel after.

Gate: full test suite + golden error output byte-compare for existing errors (must not change for single-file programs).

## Phase 2 — Definite-assignment soundness fix (known gap, medium)

Problem: `count: Integer` field with no initializer reads as `Nil` at runtime; typechecker accepts it.

Approach (simplest sound cut): typechecker requires every non-defaulted field to be assigned in `initializer` (flow-insensitive: "assigned somewhere in initializer body" — accept minor over-approximation; document it). Error at class-check time, rustc-style.

Steps:
1. `src/typechecker/` — when checking a class: collect fields without default values; walk initializer body for `self.field = ...` / implicit-self assignments; report unassigned ones with the field's span.
2. New error example `examples/errors/class_unassigned_field.yara`; unit tests in typechecker.
3. Update `docs/syntax.md` + CLAUDE.md status.

Agent split: one typechecker agent, one test/example agent, one docs agent.

## Phase 3 — Pointers + manual memory + teaching GC (main feature, large)

Per TODO sketch. Sub-phases, each independently green:

### 3a. `Ptr<T>` type + heap + `alloc`/`deref`/`free`
- `src/types.rs`/`src/ast/` — `Type::Pointer(Box<Type>)`, parse `Ptr<T>` (first generic-ish type syntax; parser needs `<`/`>` in type position).
- `src/builtins.rs` — register `alloc` (arity 1), `deref` (1), `free` (1).
- `src/typechecker/` — `alloc(x: T) -> Ptr<T>`, `deref(p: Ptr<T>) -> T`, `free(p: Ptr<T>) -> Nil` (or unit-like).
- `src/interpreter/` — `Value::Pointer(usize)`; `Interpreter`-owned heap `Vec<Option<Value>>`; `free` sets slot `None`; `deref`/`free` on freed slot = `RuntimeError` "use after free" with span (the pedagogical point).
- Also pointer assignment through deref? First cut: `set_deref(p, v)` builtin (mirrors array `set`) — avoids new lvalue syntax in parser. Syntax sugar later.
- Examples `examples/pointers/`: basic alloc/deref, leak demo, use-after-free error example under `examples/errors/`.

### 3b. Toy mark-and-sweep GC
- Roots: all reachable `Value::Pointer`s in every environment scope + inside arrays/objects (recursive scan).
- `collect()` builtin: mark from roots, sweep unmarked heap slots to `None`; returns freed count (Integer) so examples can print it.
- Example: same program run with manual `free` vs `collect()` side by side; leak program showing `collect()` reclaiming.
- Docs: `docs/architecture.md` heap + GC section; syntax doc.

Agent split per sub-phase: ast/parser agent, types agent, builtins agent, typechecker agent, interpreter agent, examples agent, docs agent — parser+types first, then typechecker/interpreter parallel, examples/docs last. Integration test asserts registry wiring both stages (existing test extends automatically).

Gate after each sub-phase: `cargo test`, fmt, integration runs of new examples.

## Phase 4 — Class inheritance (deferred until Phase 3 lands)

Single-parent, methods + fields inherit, no override keyword first cut; `super` deferred. Only start when 1–3 done; re-scope then.

## Phase 5 — Native codegen — explicitly parked. Not in this plan's scope.

---

## Structure improvements (do opportunistically, phase-boundary work)

1. **Split the three big `mod.rs` files** (typechecker 1405, interpreter 1259, parser 1134 lines) into submodules: e.g. `typechecker/{expr,stmt,class}.rs`, `interpreter/{expr,stmt,value,heap}.rs`, `parser/{expr,stmt,types}.rs`. Behavior-preserving; do it *before* Phase 3 bloats them further. Keep dispatchers whole per prior decision — split by node family, not by extracting every arm.
2. **Golden error-output tests in-repo**: byte-compare rendered diagnostics for every `examples/errors/*` inside `tests/` (currently verified manually against golden). Locks the "byte-identical errors" invariant automatically.
3. **Builtins duplication**: registry names arity but behavior lives in two parallel matches. After pointers land (more builtins), consider one `Builtin` trait/struct with `check(args) -> Type` + `eval(args) -> Value` per builtin, killing the parallel matches. Not urgent; do when the matches hurt.
4. **CI**: tiny GitHub Actions workflow — `cargo fmt --check` + `cargo test`. Cheap, catches drift.
5. **Span everywhere instead of bare (line, column)**: Phase 1 pushes this anyway; finish by making all error types hold `Span` uniformly.
6. **`examples/` as test fixtures is good** — keep the rule "every feature ships an example + it runs in `run_examples.rs`".

## Execution order

Phase 1 -> structure item 2 (golden tests, protects Phase 1) -> structure item 1 (splits) -> Phase 2 -> Phase 3a -> 3b -> structure item 3 -> Phase 4. Structure items 4–5 slot anywhere.
