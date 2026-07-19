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

Milestones 1-6 done: lexer, AST, parser, typechecker, interpreter all implemented; `yara run <file>` works end to end; all three examples run correctly. 35 unit tests passing, `cargo fmt` clean. Plan: `~/.claude/plans/cosmic-purring-stream.md`. Native codegen (LLVM/Cranelift or C transpile) deferred, not started.
