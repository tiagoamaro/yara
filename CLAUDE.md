# Yara — Project Map

Yara: learning-focused, strongly typed language. Ruby+Pascal hybrid syntax. Implemented in Ruby; runs under CRuby (`bin/yara`) for development and ships as a standalone mruby executable (`build/yara`, built by `make build`). Every command runs from the repo root; `README.md` covers usage.

**Agents: when you modify a folder below, update that folder's CLAUDE.md before finishing your turn.** This includes every mermaid diagram touched by the change in `docs/architecture.md`, not just prose. CLAUDE.md files and diagrams are living documentation, kept current as an ongoing part of this project's progress, not a one-off pass.

## Layout

- `bin/yara`: CRuby entry point, `exit(Yara::CLI.run(ARGV))`.
- `lib/yara.rb`: the load-order manifest, the only file allowed to `require`. `rake build` compiles the same list with `mrbc`, so a new file must be added here in dependency order.
- `lib/yara/`: the pipeline, `lexer` → `parser` → `resolver` → `typechecker` → `interpreter`. Stage notes live in `lib/yara/CLAUDE.md` (lexer, resolver, vocabularies, shared files) and in `lib/yara/parser/`, `typechecker/` and `interpreter/`.
- `mruby/`: `build_config.rb` (the default gembox plus `MRB_UTF8_STRING`, so strings are characters, not bytes), `main.rb` (compiled last; stores the exit status in `$yara_status`) and `yara.c` (the launcher: sets `ARGV`, loads the embedded bytecode, returns the status). `build_config.rb.lock` is written by the mruby build and committed.
- `Rakefile`: `rake test`, and `rake build` (downloads mruby 4.0.0 into `build/`, builds it, compiles and links `build/yara`). `Makefile`: the task list `README.md` documents.
- `test/`: minitest, one file per stage. `test/stdout/` (expected stdout per example) and `test/golden/` (expected stderr per error example, with root-relative paths) are the expected output; `support/examples.rb` decides which examples get `--vocabulary translations/pt.vocab`.
- `script/capture_output.rb`: records `bin/yara`'s output as the expected output (`make capture`). `script/parity.sh`: the per-example comparison in plain shell, used by CI on a machine without Ruby.
- `examples/` (the Yara programs), `translations/` (vocabulary files), `docs/`, `editors/`: each with its own CLAUDE.md where relevant. Read the folder's file before working in it.

Non-obvious bits that aren't in a folder doc:

- `test/parity_test.rb` runs every example from the repo root and compares stdout, stderr and exit status with `test/stdout/` and `test/golden/`, through `bin/yara` or through `YARA_BINARY` (`make parity-mruby`). Contract: non-error examples run clean, and each `examples/errors/*` fails with its golden output. After adding an example, `make capture` records its expected output; review it before committing.
- `docs/architecture.md`: walkthrough of each stage's control flow with diagrams. Every method in `lib/yara/` carries a YARD comment; keep both in sync when changing a stage's algorithm, not just its behavior.
- `docs/syntax.md`: grammar notes, updated as syntax stabilizes.
- `docs/plan-next-milestones.md`: remaining work and its order; its Progress section at the top is the live status of this project. `docs/ruby-rewrite-plan.md`: the finished Ruby rewrite, plus the mruby-compatible Ruby subset and the parity traps (places where plain Ruby differs from Yara's defined behavior), both still in force.
- `editors/vscode-yara/`: not part of the build, so nothing fails if it drifts: keep its keyword/type lists in sync with `lib/yara/lexer.rb` by hand.

## Conventions

- Source file names are spelled-out full words (`statements.rb`, `expressions.rb`), never abbreviations: explicitness over brevity, this is a teaching codebase.
- Every token and AST node carries `(line, column)`, required for diagnostics. After `import` resolution, line numbers are shifted into disjoint virtual ranges per imported file (`Node#shift_lines`), then mapped back to their file and line when rendering (`Diagnostics::SourceMap`).
- Errors (lexer/parser/resolver/typechecker/runtime/vocabulary) report exact line:column with a source excerpt and caret, rustc-style. Each stage keeps its own error class (subclassing `Diagnostics::Error`); rendering is centralized in `Diagnostics.render`/`render_with_map`, called from `cli.rb`.
- Type aliases are interchangeable and normalized at parse time: `Int`=`Integer`, `Bool`=`Boolean`, `Str`=`String`.
- No implicit numeric coercion (Int vs Float stays strict).
- Ruby version pinned via `.tool-versions` (asdf). The code must also compile with mruby: stay within the Ruby subset in `docs/ruby-rewrite-plan.md` (no `Regexp`, `Set`, `StringScanner` and so on), and no `require` outside `lib/yara.rb`.
- Never use `Float#to_s` or `String#to_f` for Yara values; go through `RustFormat`, since mruby's float conversions are inexact.
- Every method gets a YARD comment; no endless methods.
- Cover new logic with minitest tests in `test/`; run `make test` and confirm green before finishing a change. For changes that could behave differently under mruby, also run `make parity-mruby`.
- CI (`.github/workflows/ci.yml`): tests, build and parity through the executable on Linux and macOS, then `script/parity.sh` in a Ruby-free Ubuntu container.

## TODO

- Native codegen (LLVM/Cranelift or C transpile): deferred, not started.
- Message-catalog leftovers: a few fixed-string messages are still hardcoded English (listed in `lib/yara/CLAUDE.md`), lex/parse-stage labels aren't localized, and `translations/pt.vocab`'s `[messages]` translates only 4 keys.
- Out of scope, not started: translating identifiers or string contents (never translatable by design); user-defined methods on primitives; class-level/static methods beyond `.new`; visibility modifiers; `super`; multiple inheritance; method overloading on arity.
