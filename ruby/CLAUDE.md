# ruby/

Ruby implementation of Yara (Phase 6 in `docs/plan-next-milestones.md`), meant to replace `rust/` once it reaches parity and to ship as a standalone mruby executable.

## Status
Step 0 done: `bin/yara` (usage error only, matching `rust/src/main.rs`), `lib/yara.rb` load-order manifest, `lib/yara/cli.rb`, `Rakefile`, `test/cli_test.rb`. Run `rake test` from `ruby/`. Follow `PLAN.md` (steps, gates, parity traps, progress checklist). Ruby version comes from the repo-root `.tool-versions`. `rust/` stays the executable specification until the parity gate passes.

## Parity target
Same shared fixtures as `rust/`, read from the repo root: `examples/` must run clean, each `examples/errors/*` must render byte-identical to `tests/golden/<name>.stderr`, and `translations/pt.vocab` must load.
