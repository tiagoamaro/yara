# src/

Rust source for the Yara compiler/interpreter. Each pipeline **stage** lives in
its own subfolder with its own `CLAUDE.md` (`lexer/`, `parser/`, `ast/`,
`resolver/`, `typechecker/`, `interpreter/`, `translations/`, `diagnostics/`).
This file documents the **flat `.rs` files** that sit directly in `src/` — the
crate roots and the small cross-stage shared modules — since they have no
subfolder doc of their own.

## Crate shape

The package builds **both** a library and a binary (see `Cargo.toml`'s `[lib]`
and `[[bin]]`), so the compiler is usable and testable as a library while the
`yara` command is a thin wrapper over it. This split is what makes the
end-to-end `tests/` possible (a binary-only crate can't be `use`d from tests).

## Flat files

- **`lib.rs`** — the library crate root. Declares every module (`pub mod ast;`
  `pub mod lexer;` …) and carries the crate-level docs describing the
  lexer→parser→resolver→typechecker→interpreter pipeline. Contains no logic
  itself; it's the module manifest + overview. Everything reusable is reached as
  `yara::<module>` from both the binary and the integration tests.

- **`main.rs`** — the binary crate root: a thin CLI over the library. Parses
  `yara run <file> [--keywords <path>]`, reads the source (and optional keyword
  file), and runs the stages via a `stage(...)` helper (lex/parse/translation errors
  only) and `stage_mapped(...)` helper (resolver/typechecker/interpreter errors with
  `diagnostics::render_with_map`). The `SourceMap` is built after parsing, seeded
  with the entry file path and source, then passed to the resolver; the resolver
  registers each imported file in the map and shifts their AST line numbers into a
  virtual line space, so error positions in imported code map back to the correct file
  and local line. No compilation logic lives here — it only drives library stages and
  prints their errors rustc-style. Carries no unit tests of its own — snippet rendering
  (and its tests) now live in `diagnostics/`.

- **`env.rs`** — `Environment<T>`, the generic lexical-scope stack
  (`Vec<HashMap<String, T>>`, searched innermost-first for shadowing). Shared by
  the typechecker as `Environment<Type>` and the interpreter as
  `Environment<Value>`: identical scoping *machinery*, different binding type.
  Only the container is shared — each stage still walks the AST with its own
  logic. Replaced two hand-duplicated `Scope` structs.

- **`types.rs`** — small type-name utilities kept out of any single stage.
  Currently just `normalize_type_alias` (`Int`→`Integer`, `Bool`→`Boolean`,
  `Str`→`String`). Lives here rather than in the lexer so the *parser* can
  canonicalize a type annotation without importing from the lexer (a
  name-spelling concern, not a lexing one). The canonical primitive
  name↔`Type` table (`PRIMITIVE_TYPES`) lives in `typechecker/` instead, since it
  needs the `Type` enum.

- **`builtins.rs`** — registry of the array builtins (`len`/`push`/`get`/`set`/
  `pop`) with each one's name + arity. Single source of truth for *which* names
  are builtins and their arity; the typechecker and interpreter each still
  implement the builtin's *behavior* in their own parallel `match` (shared
  metadata, separate logic — the deliberate teaching split). `print` is a
  separate variadic path and is intentionally not in the registry. An integration
  test (`tests/run_examples.rs`) asserts every registry entry is wired into both
  stages.

## Conventions

- Adding a cross-stage shared module: prefer a flat `src/<name>.rs` (like
  `env`/`types`/`builtins`) for a leaf utility, a `src/<name>/` folder (like
  `diagnostics`) once it grows submodules — and give a folder its own
  `CLAUDE.md`.
- Keep this file's flat-file list in sync when adding/removing a `src/*.rs`.
