# ruby/

Ruby implementation of Yara (Phase 6 in `docs/plan-next-milestones.md`), meant to replace `rust/` once it reaches parity and to ship as a standalone mruby executable.

## Status
Step 0 done: `bin/yara` (usage error only, matching `rust/src/main.rs`), `lib/yara.rb` load-order manifest, `lib/yara/cli.rb`, `Rakefile`, `test/cli_test.rb`. Step 1 done: `test/parity_test.rb` compares every example with Rust's output (`tests/stdout/`, `tests/golden/`), skipping examples that reach unported stages (`PORTED_STAGES`); `script/capture_rust_stdout.rb` regenerates `tests/stdout/`. Step 2 done:
- `lib/yara/ast.rb`: one `Struct` per Rust node, members in Rust field order ending in `line, column`; binary operators are symbols (`AST::BINARY_OPERATORS`), unary is `:neg`. `Node#shift_lines` walks every member generically, so a new node type needs no extra shifting code.
- `lib/yara/diagnostics.rb`: `Span`, `Frame`, `SourceMap`, `render`/`render_with_map` (optional vocabulary, anything with `msg(key, args)`), `render_snippet`, and `Error`, the base class every stage's error subclasses with its own `kind`. `source_lines` reproduces Rust's `str::lines` so snippets and line counts stay byte-identical.
- `lib/yara/environment.rb`: the scope stack; `lookup` returns nil when unbound, so use `bound?` where a bound value can be nil.

Step 3 done:
- `lib/yara/lexer.rb`: `Token` (`kind` symbol plus `value`), `LexError`, `Lexer`. `Lexer.describe(token)` prints a token as Rust's `TokenKind` display does (`Ident("x")`, `Float(5.0)`), which parse errors embed. Non-ASCII letters count as identifier characters, a simplification of Rust's Unicode table marked `ponytail:`.
- `lib/yara/parser.rb` plus `parser/statements.rb` and `parser/expressions.rb` (reopening `Parser`), with `ParseError`.
- `lib/yara/messages.rb`: the 128-key English catalog, generated from `rust/src/translations/messages.rs`, and `Messages.substitute`.
- `lib/yara/translations.rb`: `Vocabulary` with `english`, `keywords`, `canonical_type` and `msg`; the file parser comes in step 7.
- `lib/yara/rust_format.rb`: Rust's `{}`/`{:?}` for floats and `{:?}` for strings, checked against real Rust output.
- `lib/yara/cli.rb` runs lex and parse and renders their errors; `PORTED_STAGES` includes both stages.

Step 4 done:
- `lib/yara/resolver.rb`: `ResolveError` and `Resolver.resolve_imports`, mirroring `rust/src/resolver/`. `import_path` reproduces Rust's `Path::join` spelling (no `./` for a bare entry file name), since the path shows up in messages and `-->` lines. `File.realpath` stands in for `canonicalize`; OS errors print via `CLI.os_error`. Like Rust, importing the same file twice anywhere in a run is a cycle error.
- `cli.rb` now resolves imports and renders their errors through the `SourceMap`; `PORTED_STAGES` includes `import error`. No example has an import-error golden, so `test/resolver_test.rb` covers the messages; they were also checked byte-identical against the Rust binary.

Step 5 done:
- `lib/yara/typechecker.rb`: `Type` (a `Data` with `kind` and `inner`; `accepts?` is Rust's `assignable`), `TypeError`, and `TypeChecker` with `check_program`; `typechecker/expressions.rb`, `statements.rb`, `calls.rb`, `classes.rb` and `methods.rb` reopen it, mirroring the Rust split.
- `lib/yara/builtins.rb` and `lib/yara/methods.rb`: the arity registries. Each stage dispatches by name with `send` (`check_builtin_<name>`, `check_<kind>_<name>`); primitive methods whose result depends only on the receiver sit in `TypeChecker::FIXED_RESULTS` instead.
- `Vocabulary` gained `canonical_builtin`, `canonical_method`, `type_name` and `localized_method_names`, identity for English.
- An inheritance cycle is reported at the first of its classes reached in declaration order; Rust's choice depends on `HashMap` order and changes between runs.
- `cli.rb` typechecks after resolving; `PORTED_STAGES` includes `type error`. Error messages were diffed against the Rust binary over the unit-test sources.

Step 6 done:
- `lib/yara/interpreter.rb`: `Instance`, `Pointer`, `RuntimeError` (frames from the call stack) and `Interpreter`; `interpreter/expressions.rb`, `statements.rb`, `calls.rb`, `classes.rb` and `methods.rb` reopen it. Yara values are plain Ruby values (`Array` shares by reference, as Rust's `Rc<RefCell<Vec>>` does); `Interpreter.display` is Rust's `Display`, with floats through `RustFormat`.
- `return` unwinds as a `Return` value handed back through `exec_statement`, like Rust's `Flow`. Heap slots are one-element arrays, nil once freed.
- Integer results are checked against the i64 bounds (`checked_integer`); `/` truncates toward zero. `String#to_i`/`to_f` validate by hand to match Rust's `parse`, and `Float#to_i` saturates like `as i64`.
- `cli.rb` runs the whole pipeline and exits 0 on success; `PORTED_STAGES` lists every stage. The parity test skips the Portuguese-vocabulary examples until step 7 adds the vocabulary file parser.

Run `make test` from `ruby/` (or `make parity`, `make capture`, `make run FILE=examples/hello.yara`; see `Makefile`). Follow `PLAN.md` (steps, gates, parity traps, progress checklist). Ruby version comes from the repo-root `.tool-versions`. `rust/` stays the executable specification until the parity gate passes.

## Parity target
Same shared fixtures as `rust/`, read from the repo root: `examples/` must run clean, stdout must match `tests/stdout/<path under examples>.stdout`, each `examples/errors/*` must render byte-identical to `tests/golden/<name>.stderr`, and `translations/pt.vocab` must load.
