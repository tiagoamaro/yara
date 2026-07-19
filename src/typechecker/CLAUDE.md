# typechecker/

Static type-checking pass over the parsed AST, before interpretation.

## Status
TODO — not yet implemented.

## Requirements
- Enforce strong typing: no implicit Int/Float coercion, no implicit nil coercion.
- Infer types for un-annotated `x = expr` declarations; verify explicit annotations match inferred/assigned type.
- Type errors must report exact line:column, expected-vs-found types, source excerpt + caret (rustc-style, see root CLAUDE.md / docs/syntax.md).

## Gotchas
(none yet — update as discovered)
