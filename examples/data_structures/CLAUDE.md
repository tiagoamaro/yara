# examples/data_structures/

Data structure demos built on Yara's `Array` type (see `src/typechecker/CLAUDE.md`, `src/interpreter/CLAUDE.md`). Yara has no pointers or records yet, so linked-list/tree/graph structures are built arena-style: nodes live in parallel arrays, and integer array-indices stand in for pointers/references. See root `CLAUDE.md` TODO for the (deferred) idea of an opt-in real pointer type.

## Status
All six run end-to-end via `cargo run -- run examples/data_structures/<file>.yara` (verified 2026-07-18).

## Files
- `list.yara` — dynamic array basics: `push`, `[i]` indexing, `len`, iteration, `set`.
- `stack.yara` — LIFO via `push`/`pop` on an `IntArray`.
- `queue.yara` — FIFO via `push` (enqueue) plus a hand-tracked `front` index (dequeue); no built-in "shift"/remove-from-front.
- `linked_list.yara` — singly linked list: `values`/`nexts` parallel `IntArray`s, `-1` as the null-next sentinel, recursion-free `prepend`/`print_list`.
- `binary_tree.yara` — BST: `values`/`lefts`/`rights` parallel `IntArray`s, `-1` as the null-child sentinel, recursive `insert`/`inorder` (exercises a function calling itself while mutating shared arrays through parameters).
- `graph.yara` — edge list: two parallel `IntArray`s (`from_nodes`/`to_nodes`) rather than an adjacency list, since Yara has no array-of-array type; `is_adjacent` scans linearly.

## Gotchas
- No adjacency-list-style graph is possible yet — `Type::Array` isn't parametrized generically, only `IntArray`/`FloatArray`/`BoolArray`/`StringArray` exist as concrete annotations, so there's no `IntArray` *of* `IntArray`. `graph.yara`'s edge-list is the workaround.
- Arrays passed as function parameters share storage with the caller (reference semantics — see `src/interpreter/CLAUDE.md`), which is exactly what lets `insert`/`prepend`/`add_edge` mutate the caller's arena arrays; a function that wanted a private copy would have no way to get one (no `clone`/`dup` builtin yet).
