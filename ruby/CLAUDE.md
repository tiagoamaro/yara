# ruby/

Ruby implementation of Yara (Phase 6 in `docs/plan-next-milestones.md`), meant to replace `rust/` once it reaches parity and to ship as a standalone mruby executable.

## Status
Step 0 done: `bin/yara` (usage error only, matching `rust/src/main.rs`), `lib/yara.rb` load-order manifest, `lib/yara/cli.rb`, `Rakefile`, `test/cli_test.rb`. Step 1 done: `test/parity_test.rb` compares every example with Rust's output (`tests/stdout/`, `tests/golden/`), skipping examples that reach unported stages (`PORTED_STAGES`); `script/capture_rust_stdout.rb` regenerates `tests/stdout/`. Run `rake test` from `ruby/`. Follow `PLAN.md` (steps, gates, parity traps, progress checklist). Ruby version comes from the repo-root `.tool-versions`. `rust/` stays the executable specification until the parity gate passes.

## Parity target
Same shared fixtures as `rust/`, read from the repo root: `examples/` must run clean, stdout must match `tests/stdout/<path under examples>.stdout`, each `examples/errors/*` must render byte-identical to `tests/golden/<name>.stderr`, and `translations/pt.vocab` must load.
