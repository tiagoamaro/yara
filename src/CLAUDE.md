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
  `yara run <file> [--vocabulary <path>]` (`--keywords <path>` still accepted as an
  alias for backward compatibility — both flags are parsed by the same
  `parse_vocabulary_flag`), reads the source (and optional vocabulary file via
  `translations::parse_vocabulary`, building a full `Rc<Vocabulary>` rather than a bare
  keyword map), and runs the stages via a `stage(...)` helper (lex/parse/translation
  errors only, via plain `diagnostics::render` — not localized) and `stage_mapped(...)`
  helper (resolver/typechecker/interpreter errors, via `diagnostics::render_with_map_and_vocab`
  passed `Some(&vocab)` — localizes the stage label and call-stack `in`/`at` words when
  `vocab` is a translated one). The same `Rc<Vocabulary>` is threaded into
  `Lexer::with_keywords`, `Parser::with_vocabulary`, `resolver::resolve_imports`,
  `TypeChecker::with_vocabulary`, and `Interpreter::with_vocabulary` — one vocabulary
  governs every stage of a given run. The `SourceMap` is built after parsing, seeded
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

- **`builtins.rs`** — unified registry of the array builtins (`len`/`push`/`get`/`set`/
  `pop`), pointer builtins (`alloc`/`deref`/`set_deref`/`free`), and the GC
  builtin (`collect`, arity 0). Each registry entry holds the builtin's name, arity,
  and function pointers to its typecheck and execution logic — single source of
  truth for which builtins exist, how they're typed, and how they execute.
  Adding a builtin requires: (1) a `Builtin` entry here with check/eval function
  pointers, (2) a `CheckFn` in `src/typechecker/calls.rs`, (3) an `EvalFn` in
  `src/interpreter/calls.rs` (compile-time field requirements enforce both exist).
  `print` is a separate variadic path and is intentionally not in the registry.
  An integration test (`tests/run_examples.rs`) asserts every registry entry is
  wired into both stages via the probe-snippet pattern.

- **`methods.rs`** — unified registry of primitive methods callable on Array/String/
  Integer/Float/Boolean/Pointer receiver types (`xs.size()`, `2.to_s()`, `"3".to_i()`,
  `p.deref()`, etc: 25 methods total). Each registry entry is keyed by `(ReceiverKind,
  name)` (unlike `builtins.rs`, a method name like `to_s` exists on multiple receiver
  kinds) and holds name, arity, and function pointers to typecheck and eval logic.
  Adding a primitive method requires: (1) a registry entry here, (2) a `MethodCheckFn`
  in `src/typechecker/methods.rs`, (3) a `MethodEvalFn` in `src/interpreter/methods.rs`.
  Free-function builtins (e.g., `len`, `push`, `deref` in `builtins.rs`) are kept
  unchanged and side-by-side — both `len(xs)` and `xs.size()` work concurrently.

## Conventions

- Adding a cross-stage shared module: prefer a flat `src/<name>.rs` (like
  `env`/`types`/`builtins`) for a leaf utility, a `src/<name>/` folder (like
  `diagnostics`) once it grows submodules — and give a folder its own
  `CLAUDE.md`.
- Keep this file's flat-file list in sync when adding/removing a `src/*.rs`.
