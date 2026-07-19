# ast/

AST node type definitions shared by parser, typechecker, interpreter.

## Status
TODO — not yet implemented.

## Requirements
- Every node carries `(line, column)` from its originating token(s) — needed for typechecker/runtime error reporting.
- Cover: `FunctionDef`, `VarDecl`, `ConstDecl`, expressions (binary/unary ops, literals, calls, identifiers), statements (`If`, `While`, `For`, `Return`, block/body), base type references.

## Gotchas
(none yet — update as discovered)
