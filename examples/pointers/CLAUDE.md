# examples/pointers/

Manual memory management and garbage collection demos — the teaching payload
of the `Ptr<T>` feature (see root `CLAUDE.md` and `src/interpreter/CLAUDE.md`
for the heap/GC mechanics). All run clean via
`cargo run -- run examples/pointers/<file>.yara`; the deliberately-failing
pointer programs (use-after-free, double-free) live in `examples/errors/`
instead, since everything here must pass `tests/run_examples.rs`.

## Files

- `basic.yara` — the manual lifecycle: `alloc`, `deref`, `set_deref`, `free`.
- `leak.yara` — allocations never freed; allowed in manual mode (the language
  doesn't force cleanup), setting up the problem `gc.yara` solves.
- `gc.yara` — manual-vs-GC side by side: one allocation freed by hand, three
  leaked from inside a function call (unreachable once it returns), one still
  reachable. `collect()` reclaims exactly the three leaks and the survivor
  still derefs. Actual output:
  ```
  Manual: allocated one slot, freed it ourselves
  GC: slots reclaimed by collect():
  3
  Still-reachable allocation survives the sweep:
  99
  ```

## Gotchas

- `while` bodies don't push a scope, so a pointer bound in a loop *stays
  reachable after the loop* (only the last rebinding, though — earlier
  iterations' allocations become garbage). To leak something on purpose for a
  GC demo, allocate inside a function: the call's scope pops on return
  (`gc.yara`'s `leak_some`).
- `collect()` counts only slots it freed itself — manually-freed slots are
  already `None` and don't inflate the count.
