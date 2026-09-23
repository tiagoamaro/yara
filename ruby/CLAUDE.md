# ruby/

Ruby implementation of Yara (Phase 6 in `docs/plan-next-milestones.md`), meant to replace `rust/` once it reaches parity and to ship as a standalone mruby executable.

## Status
Not started. `rust/` stays the executable specification until the parity gate passes.

## Parity target
Same shared fixtures as `rust/`, read from the repo root: `examples/` must run clean, each `examples/errors/*` must render byte-identical to `tests/golden/<name>.stderr`, and `translations/pt.vocab` must load.
