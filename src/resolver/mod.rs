//! Resolves `import "path"` statements by splicing the imported file's
//! top-level statements in place, before typechecking/interpretation ever run.

use crate::ast::Stmt;
use crate::diagnostics::SourceMap;
use crate::lexer::Lexer;
use crate::parser::Parser;
use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct ResolveError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for ResolveError {
    /// Formats as `line:column: message`, matching the `Display` shape used
    /// by `LexError`/`ParseError`/`TypeError` elsewhere in the pipeline.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl crate::diagnostics::Diagnostic for ResolveError {
    fn kind(&self) -> &str {
        "import error"
    }
    fn message(&self) -> &str {
        &self.message
    }
    fn span(&self) -> crate::diagnostics::Span {
        crate::diagnostics::Span::new(self.line, self.column)
    }
}

/// Public entry point: resolves all `import` statements in `program`
/// (recursively), returning a single flat statement list with each
/// `Stmt::Import` replaced by the imported file's own (recursively resolved)
/// statements.
///
/// `current_file` is used to resolve relative import paths and to detect
/// import cycles. Before delegating to [`resolve`], this seeds the
/// cycle-detection set with `current_file`'s own canonicalized path, so that
/// a chain of imports that eventually re-imports the entry file itself is
/// caught as a cycle, not just cycles among the imported files.
///
/// `map` must be a [`SourceMap`] seeded with the entry file
/// (`SourceMap::new(path, source)`): every imported file is registered in it
/// and that file's spliced statements have their line numbers shifted into
/// the file's assigned virtual range ([`Stmt::shift_lines`]), so any later
/// diagnostic can be resolved back to the right file/line/snippet via
/// `SourceMap::lookup`. Callers that don't render diagnostics (tests) can
/// pass a throwaway map.
pub fn resolve_imports(
    program: Vec<Stmt>,
    current_file: &Path,
    map: &mut SourceMap,
) -> Result<Vec<Stmt>, ResolveError> {
    let mut visited = HashSet::new();
    if let Ok(canonical) = current_file.canonicalize() {
        visited.insert(canonical);
    }
    resolve(program, current_file, &mut visited, map)
}

/// Recursive worker behind [`resolve_imports`]. Walks `program`'s top-level
/// statements in order and, for every `Stmt::Import { path, line, column }`,
/// replaces it in place with the (recursively resolved) statement list of the
/// file `path` names — so a function/const defined in an imported file ends
/// up spliced directly into the importer's own flat statement list, in the
/// same scope, as if it had been pasted in. Non-import statements are passed
/// through unchanged.
///
/// `visited` is the shared cycle-detection set threaded through every level
/// of recursion: each imported file's canonicalized path is inserted before
/// it is read/lexed/parsed, and re-inserting an already-visited path (i.e.
/// `HashSet::insert` returning `false`) is reported as an import cycle rather
/// than recursing forever.
fn resolve(
    program: Vec<Stmt>,
    current_file: &Path,
    visited: &mut HashSet<PathBuf>,
    map: &mut SourceMap,
) -> Result<Vec<Stmt>, ResolveError> {
    let mut resolved = Vec::with_capacity(program.len());
    for stmt in program {
        match stmt {
            Stmt::Import { path, line, column } => {
                let target = resolve_import_path(current_file, &path);
                let canonical = target.canonicalize().map_err(|e| ResolveError {
                    message: format!("cannot resolve import `{path}`: {e}"),
                    line,
                    column,
                })?;
                if !visited.insert(canonical.clone()) {
                    return Err(ResolveError {
                        message: format!("import cycle detected at `{}`", target.display()),
                        line,
                        column,
                    });
                }

                let source = std::fs::read_to_string(&target).map_err(|e| ResolveError {
                    message: format!("cannot read imported file `{}`: {e}", target.display()),
                    line,
                    column,
                })?;
                let tokens = Lexer::new(&source).tokenize().map_err(|e| ResolveError {
                    message: format!("lex error in `{}`: {e}", target.display()),
                    line,
                    column,
                })?;
                let mut imported_program =
                    Parser::new(tokens)
                        .parse_program()
                        .map_err(|e| ResolveError {
                            message: format!("parse error in `{}`: {e}", target.display()),
                            line,
                            column,
                        })?;

                // Register the file in the source map and shift its AST
                // positions into the virtual line range it was assigned, so
                // diagnostics raised later inside these statements point back
                // at this file, not the entry file.
                let offset = map.add_file(target.display().to_string(), source);
                for stmt in &mut imported_program {
                    stmt.shift_lines(offset);
                }

                let nested = resolve(imported_program, &target, visited, map)?;
                resolved.extend(nested);
            }
            other => resolved.push(other),
        }
    }
    Ok(resolved)
}

/// Turns the string literal in an `import "path"` statement into a concrete
/// filesystem path: appends a `.yara` extension if `import_path` doesn't
/// already have one, then resolves it relative to the *importing* file's own
/// parent directory (`current_file.parent()`) — not the process's current
/// working directory — so imports remain correct regardless of where `yara`
/// is invoked from. If `current_file` has no parent (e.g. it's a bare
/// filename with no directory component), the path is used as-is.
fn resolve_import_path(current_file: &Path, import_path: &str) -> PathBuf {
    let mut path = PathBuf::from(import_path);
    if path.extension().is_none() {
        path.set_extension("yara");
    }
    match current_file.parent() {
        Some(dir) => dir.join(path),
        None => path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Writes `contents` to `dir/name`, returning the file's path. Test helper
    /// for building small multi-file import fixtures on disk.
    fn write_temp(dir: &Path, name: &str, contents: &str) -> PathBuf {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        path
    }

    /// Lexes and parses `src` into a top-level statement list, panicking on
    /// any lex/parse error. Test helper to keep fixtures terse.
    fn parse_src(src: &str) -> Vec<Stmt> {
        let tokens = Lexer::new(src).tokenize().unwrap();
        Parser::new(tokens).parse_program().unwrap()
    }

    /// A single `import` in the main file should be replaced by the imported
    /// file's own statements, spliced in place.
    #[test]
    fn splices_imported_statements() {
        let dir = std::env::temp_dir().join(format!("yara_resolver_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        write_temp(
            &dir,
            "helper.yara",
            "def add(a: Int, b: Int): Int\n  a + b\nend",
        );
        let main_path = write_temp(&dir, "main.yara", "import \"helper\"\nx = add(1, 2)");

        let main_src = std::fs::read_to_string(&main_path).unwrap();
        let program = parse_src(&main_src);
        let mut map = SourceMap::new(main_path.to_str().unwrap(), &main_src);
        let resolved = resolve_imports(program, &main_path, &mut map).unwrap();

        assert_eq!(resolved.len(), 2);
        assert!(matches!(resolved[0], Stmt::FunctionDef { .. }));
        assert!(matches!(resolved[1], Stmt::VarDecl { .. }));

        // The imported function's lines must be shifted past the entry file's
        // 2 lines (helper's `def` is local line 1 -> virtual line 3), while
        // the entry file's own statement keeps its natural line.
        assert_eq!(resolved[0].line(), 3);
        assert_eq!(resolved[1].line(), 2);
        // And the map must resolve the shifted line back to the helper file.
        assert!(map.lookup(3).path.ends_with("helper.yara"));

        std::fs::remove_dir_all(&dir).ok();
    }

    /// A chain of imports that loops back on itself (main -> a -> b -> a)
    /// must be reported as a cycle instead of recursing forever.
    #[test]
    fn detects_import_cycle() {
        let dir = std::env::temp_dir().join(format!("yara_resolver_cycle_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        write_temp(&dir, "a.yara", "import \"b\"");
        let a_reimport = "import \"a\"";
        write_temp(&dir, "b.yara", a_reimport);
        let main_path = write_temp(&dir, "main.yara", "import \"a\"");

        let main_src = std::fs::read_to_string(&main_path).unwrap();
        let program = parse_src(&main_src);
        let mut map = SourceMap::new(main_path.to_str().unwrap(), &main_src);
        let err = resolve_imports(program, &main_path, &mut map).unwrap_err();
        assert!(err.message.contains("cycle"));

        std::fs::remove_dir_all(&dir).ok();
    }

    /// An import that names a nonexistent file should fail with the
    /// *importing* statement's own line:column, not some default/zeroed
    /// position.
    #[test]
    fn missing_import_reports_position() {
        let dir =
            std::env::temp_dir().join(format!("yara_resolver_missing_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let main_path = write_temp(&dir, "main.yara", "import \"does_not_exist\"");

        let main_src = std::fs::read_to_string(&main_path).unwrap();
        let program = parse_src(&main_src);
        let mut map = SourceMap::new(main_path.to_str().unwrap(), &main_src);
        let err = resolve_imports(program, &main_path, &mut map).unwrap_err();
        assert_eq!(err.line, 1);
        assert_eq!(err.column, 1);

        std::fs::remove_dir_all(&dir).ok();
    }
}
