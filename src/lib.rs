//! Yara — a learning-focused, strongly typed, compiled/interpreted language.
//!
//! This crate exposes the whole compiler/interpreter pipeline as a **library**
//! so it can be driven programmatically and, crucially, tested end to end (see
//! the crate's `tests/` directory). The `yara` binary (`src/main.rs`) is a thin
//! CLI wrapper over this library: it does argument parsing and rustc-style error
//! rendering, and defers every actual compilation stage to the modules here.
//!
//! The pipeline is a straight line, each stage consuming the previous stage's
//! output and shielding the next from lower-level detail:
//!
//! ```text
//! source ──[lexer]──▶ tokens ──[parser]──▶ AST ──[resolver]──▶ AST'
//!        ──[typechecker]──▶ (checked) ──[interpreter]──▶ effects
//! ```
//!
//! - [`lexer`] turns source text into `Token`s (optionally using a translated
//!   keyword table produced by [`translations`]).
//! - [`parser`] turns `Token`s into the [`ast`] node tree — the shared vocabulary
//!   every later stage walks.
//! - [`resolver`] splices `import`ed files into that tree before any checking.
//! - [`typechecker`] statically checks the tree; [`interpreter`] then executes it.

pub mod ast;
pub mod diagnostics;
pub mod interpreter;
pub mod lexer;
pub mod parser;
pub mod resolver;
pub mod translations;
pub mod typechecker;
