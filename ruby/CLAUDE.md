# ruby/

The Yara implementation. Runs under CRuby (`bin/yara`) for development and ships as a standalone mruby executable (`build/yara`, built by `make build`). `README.md` covers usage, the standalone build, adding mruby gems and platforms.

## Layout
- `lib/yara.rb`: the load-order manifest, the only file allowed to `require`. `rake build` compiles the same list with `mrbc`, so a new file must be added here in dependency order.
- `lib/yara/`: the pipeline; see `lib/yara/CLAUDE.md` and the `CLAUDE.md` in `parser/`, `typechecker/` and `interpreter/`.
- `bin/yara`: CRuby entry point, `exit(Yara::CLI.run(ARGV))`.
- `mruby/`: `build_config.rb` (the default gembox plus `MRB_UTF8_STRING`, so strings are characters, not bytes), `main.rb` (compiled last; stores the exit status in `$yara_status`) and `yara.c` (the launcher: sets `ARGV`, loads the embedded bytecode, returns the status). `build_config.rb.lock` is written by the mruby build and committed.
- `Rakefile`: `rake test`, and `rake build` (downloads mruby 4.0.0 into `build/`, builds it, compiles and links `build/yara`).
- `test/`: minitest, one file per stage; `parity_test.rb` runs every example through `bin/yara`, or through `YARA_BINARY` (`make parity-mruby`). `support/examples.rb` decides which examples get `--vocabulary translations/pt.vocab`.
- `script/capture_output.rb`: records `bin/yara`'s output as the expected output (`make capture`). `script/parity.sh`: the per-example comparison in plain shell, used by CI on a machine without Ruby.
- `PLAN.md`: the Ruby rewrite plan (Phase 6), the mruby-compatible Ruby subset, and the parity traps (places where plain Ruby differs from Yara's defined behavior).

## Rules
- Stay within the Ruby subset in `PLAN.md`: no `Regexp`, `Set`, `StringScanner` and so on, and nothing CRuby and mruby disagree on without a test covering both. When in doubt, run `make parity-mruby`.
- Never use `Float#to_s` or `String#to_f` for Yara values; go through `RustFormat`, since mruby's float conversions are inexact.
- Every method gets a YARD comment; no endless methods.

## Checks
- `make test`: every minitest test, parity included.
- `make parity-mruby`: rebuilds `build/yara` and runs the parity test through it.
- CI (`.github/workflows/ci.yml`): tests, build and parity through the executable on Linux and macOS, then `script/parity.sh` in a Ruby-free Ubuntu container.
