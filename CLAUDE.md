# Yara — Project Map

Yara: learning-focused, strongly typed language. Ruby+Pascal hybrid syntax. Implemented in Ruby in `ruby/`, run under CRuby (`ruby/bin/yara`) or as a standalone mruby executable (`ruby/build/yara`).

**Agents: when you modify a folder below, update that folder's CLAUDE.md before finishing your turn.** This includes every mermaid diagram touched by the change in `docs/architecture.md`, not just prose. CLAUDE.md files and diagrams are living documentation, kept current as an ongoing part of this project's progress, not a one-off pass.

## Layout

- `ruby/`: the implementation (see `ruby/CLAUDE.md`, `ruby/README.md`). Run `make`/`rake` commands from there.
- Shared fixtures at the repo root: `examples/` (the Yara programs), `translations/` (vocabulary files), `tests/stdout/` (expected stdout per example), `tests/golden/` (expected stderr per error example, with root-relative paths), `docs/`, `editors/`.

Pipeline order: `lexer` → `parser` → `resolver` → `typechecker` → `interpreter`. Stage notes live in `ruby/lib/yara/CLAUDE.md` (lexer, resolver, vocabularies, shared files) and in `ruby/lib/yara/parser/`, `typechecker/` and `interpreter/`; so do `translations/`, `examples/` and `editors/vscode-yara/`. Read the folder's file before working in it.

Non-obvious bits that aren't in a folder doc:

- `ruby/test/parity_test.rb` runs every example from the repo root and compares stdout, stderr and exit status with `tests/stdout/` and `tests/golden/`. Contract: non-error examples run clean, and each `examples/errors/*` fails with its golden output. After adding an example, `make capture` in `ruby/` records its expected output; review it before committing.
- `docs/architecture.md`: walkthrough of each stage's control flow with diagrams. Every method in `ruby/lib/yara/` carries a YARD comment; keep both in sync when changing a stage's algorithm, not just its behavior.
- `docs/syntax.md`: grammar notes, updated as syntax stabilizes.
- `docs/plan-next-milestones.md`: remaining work and its order; its Progress section at the top is the live status of this project.
- `editors/vscode-yara/`: not part of the build, so nothing fails if it drifts: keep its keyword/type lists in sync with `ruby/lib/yara/lexer.rb` by hand.

## Conventions

- Source file names are spelled-out full words (`statements.rb`, `expressions.rb`), never abbreviations: explicitness over brevity, this is a teaching codebase.
- Every token and AST node carries `(line, column)`, required for diagnostics. After `import` resolution, line numbers are shifted into disjoint virtual ranges per imported file (`Node#shift_lines`), then mapped back to their file and line when rendering (`Diagnostics::SourceMap`).
- Errors (lexer/parser/resolver/typechecker/runtime/vocabulary) report exact line:column with a source excerpt and caret, rustc-style. Each stage keeps its own error class (subclassing `Diagnostics::Error`); rendering is centralized in `Diagnostics.render`/`render_with_map`, called from `cli.rb`.
- Type aliases are interchangeable and normalized at parse time: `Int`=`Integer`, `Bool`=`Boolean`, `Str`=`String`.
- No implicit numeric coercion (Int vs Float stays strict).
- Ruby version pinned via `.tool-versions` (asdf). The code must also compile with mruby: only the Ruby subset listed in `ruby/PLAN.md`, and no `require` outside `ruby/lib/yara.rb`.
- Cover new logic with minitest tests in `ruby/test/`; run `make test` (from `ruby/`) and confirm green before finishing a change. For changes that could behave differently under mruby, also run `make parity-mruby`.

## TODO

- Native codegen (LLVM/Cranelift or C transpile): deferred, not started.
- Message-catalog leftovers: a few fixed-string messages are still hardcoded English (listed in `ruby/lib/yara/CLAUDE.md`), lex/parse-stage labels aren't localized, and `translations/pt.vocab`'s `[messages]` translates only 4 keys.
- Out of scope, not started: translating identifiers or string contents (never translatable by design); user-defined methods on primitives; class-level/static methods beyond `.new`; visibility modifiers; `super`; multiple inheritance; method overloading on arity.
