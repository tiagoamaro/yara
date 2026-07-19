# resolver/

Resolves `import "path"` statements before typechecking/interpretation.

## Status
Implemented. `resolve_imports(program: Vec<Stmt>, current_file: &Path) -> Result<Vec<Stmt>, ResolveError>`.

## Design
- `import "path"` is parsed by `parser` into `Stmt::Import { path, line, column }` — a normal AST node — but `typechecker` and `interpreter` only ever see it as a no-op (`Stmt::Import { .. }` arm); real handling happens here, between parsing and typechecking, wired in `main.rs::run_file`.
- Import paths are resolved relative to the *importing file's* directory, with `.yara` appended if the path has no extension (`resolve_import_path`). No search path / stdlib directory concept yet — everything is relative-to-file.
- Recursion: an imported file's own `import`s are resolved too (`resolve` calls itself), so imports can chain.
- Cycle detection: a `HashSet<PathBuf>` of canonicalized paths, seeded with the entry file, threaded through the recursion; re-visiting a path is a `ResolveError`.
- Splicing: each `Stmt::Import` is replaced in place by the imported file's (already-resolved) statement list — so a function/const defined in an imported file becomes globally visible to the importer, same scope as if it had been pasted in.
- Errors (`ResolveError`) carry the *importing* `import` statement's line:column (not a position inside the imported file) — same shape/Display as `LexError`/`ParseError`/`TypeError`.

## Gotchas
- Only top-level `import` is meaningful — the parser will accept `import "x"` inside a function body or `if` block too (nothing stops it grammatically), but nothing currently exercises or documents that case; treat as unsupported until deliberately designed.
- No re-export / namespacing / qualified access (`module.func()`) — imported names just land in the same flat global namespace as the importer's own top-level defs, so name collisions across files are silent overwrites in whichever table (typechecker's `functions` map, etc.) processes them later. See `examples/kitchen_sink.yara` for a working multi-file example.
