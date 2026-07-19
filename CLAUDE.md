# Yara — Project Map

Yara: learning-focused, strongly typed, compiled language. Ruby+Pascal hybrid syntax. Rust compiler/interpreter.

**Agents: when you modify a folder below, update that folder's CLAUDE.md before finishing your turn.**

## Layout

- `src/lexer/` — tokenizer. See `src/lexer/CLAUDE.md`.
- `src/ast/` — AST node definitions. See `src/ast/CLAUDE.md`.
- `src/parser/` — recursive-descent parser, tokens to AST. See `src/parser/CLAUDE.md`.
- `src/typechecker/` — static type checking pass. See `src/typechecker/CLAUDE.md`.
- `src/interpreter/` — tree-walk evaluator. See `src/interpreter/CLAUDE.md`.
- `src/resolver/` — resolves `import "path"` statements before typechecking. See `src/resolver/CLAUDE.md`.
- `examples/` — sample `.yara` programs. See `examples/CLAUDE.md`.
- `docs/syntax.md` — grammar notes, updated as syntax stabilizes.

## Conventions

- Every token/AST node carries `(line, column)` position — required for diagnostics, not optional.
- Errors (lexer/parser/typechecker/runtime) must report exact line:column with a source excerpt + caret, rustc-style.
- Type aliases are interchangeable and normalized at lex/parse time: `Int`=`Integer`, `Bool`=`Boolean`, `Str`=`String`.
- No implicit numeric coercion (Int vs Float stays strict).
- Rust version pinned via `.tool-versions` (asdf).
- Always run `cargo fmt` before finishing a change; code must pass `cargo fmt --check`.
- Cover new logic with unit tests wherever feasible (colocated `#[cfg(test)] mod tests` per module); run `cargo test` and confirm green before finishing a change.

## Status

Milestones 1-6 done: lexer, AST, parser, typechecker, interpreter all implemented; `yara run <file>` works end to end. Since then: unary negation, `if`/`elsif`/`else` as a function's tail expression, file `import`, a minimal `Array` type (`IntArray`/`FloatArray`/`BoolArray`/`StringArray` with `[]` literals, `arr[i]` indexing, and `len`/`push`/`pop`/`get`/`set` builtins), and a `class` feature (const/instance-var/initializer/method, `.new` construction, `.field` read/write, implicit-`self` name resolution, no inheritance) have all landed. 67 unit tests passing, `cargo fmt` clean. `examples/data_structures/` demonstrates list/stack/queue/linked-list/binary-tree/graph built on arrays; `examples/objects/` demonstrates classes. Plan: `~/.claude/plans/cosmic-purring-stream.md`.

## TODO

- Native codegen (LLVM/Cranelift or C transpile) — deferred, not started.
- **Pointers**: allow users who want to study pointers to opt into them, but handle their absence gracefully for users who don't want to deal with them (i.e. not a silent footgun forced on everyone — likely an explicit opt-in type/syntax, deferred design decision, not started). Current data-structure examples avoid pointers entirely by using arena-style parallel arrays with integer indices instead (see `src/interpreter/CLAUDE.md`, `examples/data_structures/`).
- **Class inheritance**, class-level/static methods/fields (beyond the special `.new`), and visibility modifiers — all deliberately out of scope for the first `class` cut; not started.
- Known soundness gap: an instance field declared with no value (`count: Integer`) is `Nil` at runtime until some method assigns it — the typechecker doesn't track "definitely assigned before use," so reading a field before `initializer` sets it type-checks fine but is a `Nil` where e.g. `Integer` was expected. Not fixed yet.
