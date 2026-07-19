# interpreter/

Tree-walk evaluator executing a typechecked AST.

## Status
TODO — not yet implemented.

## Requirements
- Execute function defs, var/const bindings, control flow, arithmetic/comparison expressions.
- Runtime errors (division by zero, etc.) must carry line:column of the failing expression plus a call-frame stack trace (function name + line:column per frame) when feasible.
- Entry point wired from `main.rs` via `yara run <file>`.

## Gotchas
(none yet — update as discovered)
