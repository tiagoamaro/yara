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
- Allowed: classes, modules, `Struct`, `Array`, `Hash`, `String`, `Integer`, `Float`, `Comparable`, exceptions, `File.read`, `File.exist?`, `$stdout`/`$stderr`, `exit`.
- Unverified until the step 0 spike: `Data.define`, `case`/`in`, `String#%`, `File.dirname`/`File.join`, keyword arguments with defaults. Don't use them until the spike confirms they work under mruby 4.0.0.

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
| Printing a whole Float | `5` | `5.0` | Port Rust's `{}` formatting in `Value#to_s`, including large and small exponents |
| Integer overflow | panics in debug builds, wraps in release builds; no example covers it | becomes a Bignum (CRuby) or a Float (mruby) | Raise a runtime error ("integer overflow in `+`", and so on) at the i64 bounds in `+ - * /`, unary `-`, `abs()` and `Float#to_i()`; unit-tested in Ruby only, since there is no Rust behavior to match |
| Integer `/` with negative operands | truncates (`-7 / 2 = -3`) | floors (`-4`) | Truncate by hand: `(a.abs / b.abs) * sign` |
| `"12abc".to_i()` | runtime error "cannot parse" | `12` | Validate digits by hand, raise the same message |
| `trim`, `upper`, `lower` | Unicode whitespace and case rules | ASCII-leaning in mruby | Match Rust for ASCII, note the gap for non-ASCII |

## Steps

### 0. Toolchain and mruby spike
- Create `bin/yara`, `lib/yara.rb`, `Rakefile`, and one minitest smoke test.
- Build stock mruby 4.0.0 once in a scratch directory and run a probe script that uses every item in the "Unverified" list. Record the results in the section above: move each item to "Allowed" or to the "No" list.
**Gate:** `rake test` green; the spike results are written down.

### 1. Stdout goldens and the parity harness
- Run the Rust binary over every non-error example and save its stdout as `tests/golden/<path under examples>.stdout` (e.g. `tests/golden/data_structures/list.stdout`). A small Ruby script under `ruby/` does the capture, so regenerating never needs a Rust change.
- Write `ruby/test/parity_test.rb`. It runs `bin/yara run <example>` as a subprocess from the repo root, passes `--vocabulary translations/pt.vocab` under the same rule the Rust tests use, and compares stdout, stderr, and exit status with the goldens.
- An example whose golden fails at a stage Ruby hasn't ported yet is skipped, not failed. That lets the harness go green stage by stage.
**Gate:** the stdout goldens are committed; the Ruby harness runs, with everything skipped.

### 2. AST, diagnostics, source map
Port `rust/src/ast/`, `diagnostics/` (`Span`, `SourceMap`, `render`, the snippet and caret layout), and `environment.rb`. The error base class carries `line`, `column`, `message`, and `kind`.
**Gate:** the ported renderer unit tests pass byte-identically.

### 3. Lexer, then parser
Port each stage along with its unit tests. Keyword and type-alias normalization happens here, as in Rust.
**Gate:** `lex_error` and `parse_error` goldens pass in the harness; every other example lexes and parses without error.

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
- `translations.rb` and `messages.rb` (the 128-key catalog), with `--vocabulary` and `--keywords` in the CLI. The PT examples must pass.
- `mruby/build_config.rb` with the needed core gems, plus a small C launcher that embeds the `mrbc`-compiled bytecode of the `lib/yara.rb` load order and passes `ARGV`. `rake build` produces `ruby/build/yara`.
- The harness gains a mode that runs the mruby binary instead of CRuby.
**Gate:** the harness is green through the mruby binary on a machine with no Ruby installed (CI job).

### 8. Retire Rust
- CI switches to Ruby: `rake test` plus a harness run through the mruby binary.
- Delete `rust/`, and move `docs/architecture.md` and the stage `CLAUDE.md` files over to the Ruby layout.
**Gate:** Phase 6 acceptance criteria in `docs/plan-next-milestones.md`.

## Progress

- [ ] 0. Toolchain and mruby spike
- [ ] 1. Stdout goldens and harness
- [ ] 2. AST, diagnostics
- [ ] 3. Lexer, parser
- [ ] 4. Resolver
- [ ] 5. Typechecker
- [ ] 6. Interpreter
- [ ] 7. Vocabulary, CLI, mruby build
- [ ] 8. Retire Rust
