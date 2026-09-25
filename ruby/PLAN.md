# Ruby Implementation Plan

Phase 6 of `docs/plan-next-milestones.md`, broken into steps we can follow and tick off: port the Rust implementation to Ruby and ship it as a standalone mruby executable. Every step ends with its gate green.

`rust/` is frozen: it stays exactly as it is and serves as the executable specification. The Ruby port reproduces its observable behavior (stdout, stderr, exit status) and never requires a Rust change. The Rust binary is only run, to capture goldens, until step 8 retires it.

## Toolchain

- **CRuby 4.0.x** via asdf (`.tool-versions` at the repo root), used for development and tests. The latest release is 4.0.7 (2026-09-15); the repo pins 4.0.5, which is what is installed. To bump: `asdf plugin update ruby && asdf install ruby 4.0.7`, then edit `.tool-versions`.
- **mruby 4.0.0** (latest stable; 4.1.0 is at rc2) for the shipped `yara` executable, from step 7 onward.
- No gems beyond what ships with CRuby: `minitest` and `rake` are bundled gems. No `Gemfile`.

## mruby-compatible Ruby, from the first line

The same sources must compile with `mrbc` later, so every file sticks to the subset both runtimes share:

- No `require`/`require_relative` outside `lib/yara.rb`. That file is the load-order manifest; step 7 feeds the same list to `mrbc`.
- No `Regexp`, `StringScanner`, `Set`, `Pathname`, `OptionParser`, `Ractor`, `ObjectSpace`. The lexer walks characters by hand, like the Rust one.
- Allowed, each checked with a probe script under stock mruby 4.0.0 in step 0: classes, modules, `Struct`, `Data.define`, `case`/`in` (array and hash patterns), keyword arguments with defaults, `Comparable`, custom exception classes, `Array`, insertion-ordered `Hash`, `String#chars`/`each_char`/`ord`/`strip`/`upcase`, `String#%` and `format`, `Integer()`, `File.read`/`File.exist?`/`File.dirname`/`File.join`, `ARGV`, `$stdout`/`$stderr`.
- `exit` needs the `mruby-exit` core gem, which the default gembox leaves out.

### mruby build settings (step 7)
- `conf.gembox "default"` plus `conf.gem core: "mruby-exit"`.
- `conf.cc.defines << "MRB_UTF8_STRING"`. Without it strings are byte arrays (`"héllo".chars.size` is 6), which would break column numbers in diagnostics for non-ASCII source such as the PT examples.
- The default gembox already includes `mruby-bigint`, `mruby-io`, `mruby-sprintf`, `mruby-string-ext` and `mruby-data`.

## Layout

```
ruby/
  bin/yara                CRuby entry: loads lib/yara.rb, calls Yara::CLI.run(ARGV)
  lib/yara.rb             the only file with requires, in load order
  lib/yara/ast.rb
  lib/yara/diagnostics.rb
  lib/yara/lexer.rb
  lib/yara/parser/        statements.rb, expressions.rb (mirrors rust/src/parser/)
  lib/yara/resolver.rb
  lib/yara/typechecker/   statements.rb, expressions.rb, calls.rb, classes.rb, methods.rb
  lib/yara/interpreter/   same split
  lib/yara/builtins.rb, methods.rb, environment.rb, translations.rb, messages.rb
  test/                   minitest, one file per stage, ported from the Rust unit tests
  test/parity_test.rb     runs every example through bin/yara against the goldens
  mruby/                  build_config.rb, launcher (step 7)
  Rakefile                `rake test`, later `rake build`
```

File names use full words, matching the Rust side. Every method gets a YARD comment, and there are no endless methods.

## Parity traps (known Rust behavior Ruby does differently)

| Behavior | Rust today | Plain Ruby | Plan |
|---|---|---|---|
| Printing a Float | `5`, `0.30000000000000004`, `100000000000000000000` | CRuby: `5.0`, `0.30000000000000004`, `1.0e+20`; mruby: `5.0`, `0.3` (fewer digits), `1.0e+20` | `RustFormat.float_display`/`float_debug` rebuild Rust's `{}`/`{:?}` from the shortest digits that round-trip; never use `Float#to_s`, since CRuby and mruby disagree too |
| Reading and writing float digits | exact (`str::parse`, shortest round-trip printing) | CRuby is exact; mruby's `String#to_f` can be one bit off and its `%e` prints zeros past about 17 digits | `RustFormat.parse_float` and `shortest_digits` work in exact integer arithmetic (`Math.frexp`/`ldexp` plus bigint), used by the lexer, `String#to_f` and float printing |
| Negating `0.0` | `-0.0`, printed `-0` | mruby's unary minus gives `0.0` | Negate floats as `value * -1.0` |
| Integer overflow | panics in debug builds, wraps in release builds; no example covers it | becomes a Bignum (CRuby and mruby, which bundles `mruby-bigint`) | Raise a runtime error ("integer overflow in `+`", and so on) at the i64 bounds in `+ - * /`, unary `-` and `abs()`; unit-tested in Ruby only, since there is no Rust behavior to match. `Float#to_i()` saturates and maps NaN to 0 instead, because Rust's `as i64` defines that |
| Integer `/` with negative operands | truncates (`-7 / 2 = -3`) | floors (`-4`) in both runtimes | Truncate by hand: `(a.abs / b.abs) * sign` |
| `"12abc".to_i()` | runtime error "cannot parse" | `12` | Validate digits by hand, raise the same message |
| Which class an inheritance cycle names | whichever `HashMap` iteration reaches first, different between runs | Hash order is insertion order | Name the first cycle member reached in declaration order; no golden covers it |
| `trim`, `upper`, `lower` | Unicode whitespace and case rules | CRuby is Unicode-aware; mruby changes ASCII only (`"É".downcase` stays `É`) | Match Rust for ASCII; non-ASCII case mapping differs in the mruby build, and no example covers it |

## Steps

### 0. Toolchain and mruby spike
- Create `bin/yara`, `lib/yara.rb`, `Rakefile`, and one minitest smoke test.
- Build stock mruby 4.0.0 once in a scratch directory, run a probe script for each uncertain feature, and record the results above.
**Gate:** `rake test` green; the spike results are written down.

### 1. Stdout goldens and the parity harness
- Run the Rust binary over every example and save its stdout as `tests/stdout/<path under examples>.stdout` (e.g. `tests/stdout/data_structures/list.stdout`), with `ruby/script/capture_rust_stdout.rb`. The files can't live in `tests/golden/`, because Rust's golden test rejects any file there that isn't an error example's `.stderr`. The script also aborts if Rust's stderr no longer matches a golden.
- Write `ruby/test/parity_test.rb`, one test per example. It runs `bin/yara run <example>` as a subprocess from the repo root, passes `--vocabulary translations/pt.vocab` under the same rule the Rust tests use (`test/support/examples.rb`), and compares stdout, stderr, and exit status (1 for error examples, 0 otherwise).
- An example that reaches a stage Ruby hasn't ported yet is skipped, not failed. `PORTED_STAGES` in the test lists the ported stages; each later step appends to it.
**Gate:** the stdout goldens are committed; the Ruby harness runs, with everything skipped.

### 2. AST, diagnostics, source map
Port `rust/src/ast/`, `diagnostics/` (`Span`, `SourceMap`, `render`, the snippet and caret layout), and `environment.rb`. The error base class carries `line`, `column`, `message`, and `kind`.
**Gate:** the ported renderer unit tests pass byte-identically.

### 3. Lexer, then parser
Port each stage along with its unit tests. Keyword and type-alias normalization happens here, as in Rust.
**Gate:** `lex_error` and `parse_error` goldens pass in the harness; every other example lexes and parses without error (the Portuguese-vocabulary ones wait for step 7).

### 4. Resolver
`import` splicing, cycle detection, virtual line shifting, `SourceMap` registration.
**Gate:** the import-error goldens pass, including snippets rendered from the imported file.

### 5. Typechecker
Ported in the Rust split: classes and inheritance flattening, calls and the builtin registry, primitive methods, statements, expressions, and the definite-assignment check.
**Gate:** every `type error` golden passes.

### 6. Interpreter, heap, GC
Values, environments, calls with the call-stack trace, classes, primitive methods, the pointer heap, and `collect()`.
**Gate:** the harness is fully green (all stdout and stderr goldens), with no skips left.

### 7. Vocabulary, CLI, mruby executable
- The vocabulary file parser in `translations.rb` (the 128-key catalog in `messages.rb` and an English-only `Vocabulary` already exist from step 3), with `--vocabulary` and `--keywords` in the CLI. The PT examples must pass.
- `mruby/build_config.rb` with the needed core gems, plus a small C launcher that embeds the `mrbc`-compiled bytecode of the `lib/yara.rb` load order and passes `ARGV`. `rake build` produces `ruby/build/yara`.
- The harness gains a mode that runs the mruby binary instead of CRuby.
**Gate:** the harness is green through the mruby binary on a machine with no Ruby installed (CI job).

### 8. Retire Rust
- CI switches to Ruby: `rake test` plus a harness run through the mruby binary.
- Delete `rust/`, and move `docs/architecture.md` and the stage `CLAUDE.md` files over to the Ruby layout.
**Gate:** Phase 6 acceptance criteria in `docs/plan-next-milestones.md`.

## Progress

- [x] 0. Toolchain and mruby spike (2026-09-22)
- [x] 1. Stdout goldens and harness (2026-09-22)
- [x] 2. AST, diagnostics (2026-09-22)
- [x] 3. Lexer, parser (2026-09-22)
- [x] 4. Resolver (2026-09-24)
- [x] 5. Typechecker (2026-09-24)
- [x] 6. Interpreter (2026-09-24)
- [x] 7. Vocabulary, CLI, mruby build (2026-09-24)
- [ ] 8. Retire Rust
