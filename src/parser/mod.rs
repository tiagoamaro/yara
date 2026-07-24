//! Recursive-descent parser: tokens -> AST.

use crate::ast::{BinOp, Expr, FieldDecl, Param, Stmt, TypeAnnotation, UnOp};
use crate::lexer::{Token, TokenKind};
use crate::types::normalize_type_alias;
use std::fmt;

/// A parse-time failure. Mirrors `lexer::LexError`'s shape (a `message` plus
/// `line`/`column` of the offending token) so both stages of the pipeline
/// report errors the same way and callers can format them uniformly.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl crate::diagnostics::Diagnostic for ParseError {
    fn kind(&self) -> &str {
        "parse error"
    }
    fn message(&self) -> &str {
        &self.message
    }
    fn span(&self) -> crate::diagnostics::Span {
        crate::diagnostics::Span::new(self.line, self.column)
    }
}

/// Holds the full token stream produced by the lexer plus a cursor (`pos`)
/// into it. There is no separate "current token" field: every method reads
/// through `peek`/`advance`, so `pos` is the single source of truth for where
/// parsing is. Because it's just an integer, it can be saved and restored
/// (see `parse_ident_stmt`'s checkpoint/rewind) to support limited backtracking.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    /// Entry point: repeatedly parses top-level statements via `parse_stmt`
    /// until the `Eof` token is reached. There's no enclosing block here (no
    /// terminator other than `Eof`), since the whole file is one implicit block.
    pub fn parse_program(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut stmts = Vec::new();
        while !self.check(&TokenKind::Eof) {
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    /// Returns the token at the current cursor position without consuming it.
    /// Every dispatch decision in the parser (which statement/expression
    /// variant to build) is made by inspecting this token first.
    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    /// Reports whether the current token has the same *kind* as `kind`,
    /// ignoring any payload the variant carries (e.g. the wrapped `String` in
    /// `Ident`/`Str`, or the number in `Int`/`Float`). It compares
    /// `std::mem::discriminant`s rather than the values themselves, so a
    /// placeholder like `TokenKind::Ident(String::new())` can be used purely
    /// to mean "any identifier" when building a terminator list, without the
    /// placeholder's payload ever being compared.
    fn check(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(kind)
    }

    /// Consumes and returns the current token, moving `pos` one step forward.
    /// Clamps at the last index instead of running past the end of the
    /// vector, relying on the lexer's invariant that it always appends a
    /// trailing `Eof` token — so once positioned on `Eof`, further calls just
    /// keep returning it rather than panicking on out-of-bounds access.
    fn advance(&mut self) -> Token {
        let tok = self.tokens[self.pos].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        tok
    }

    /// Consumes the current token if it matches `kind`, otherwise produces a
    /// `ParseError` naming what was expected (the `what` string, e.g. "`)`"
    /// or "`=`") versus what was actually found. This is the parser's main
    /// building block for "this token must be here or the input is invalid"
    /// checks (closing parens/brackets, `end`, operators, etc.).
    fn expect(&mut self, kind: &TokenKind, what: &str) -> Result<Token, ParseError> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            let tok = self.peek();
            Err(ParseError {
                message: format!("expected {what}, found {:?}", tok.kind),
                line: tok.line,
                column: tok.column,
            })
        }
    }

    /// Like `expect`, specialized for `TokenKind::Ident`: consumes the
    /// current token only if it's an identifier, unwraps its `String` payload,
    /// and returns it along with the token's source position (needed by
    /// callers to stamp AST nodes with a location). Used everywhere a name is
    /// required — variable/function/class/param names, field/method names.
    fn expect_ident(&mut self) -> Result<(String, usize, usize), ParseError> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::Ident(name) => {
                self.advance();
                Ok((name, tok.line, tok.column))
            }
            _ => Err(ParseError {
                message: format!("expected identifier, found {:?}", tok.kind),
                line: tok.line,
                column: tok.column,
            }),
        }
    }

    /// Parses a type name (after a `:` in a param/field/var/const/return-type
    /// position) as a bare identifier, then normalizes short aliases (`Int`,
    /// `Str`, `Bool`, ...) to their canonical names (`Integer`, `String`,
    /// `Boolean`, ...) via `types::normalize_type_alias`. Alias resolution
    /// happens here in the parser rather than the lexer so the lexer only
    /// ever emits raw identifier tokens.
    ///
    /// Also handles `Ptr<T>` syntax: if the type name is `Ptr`, expects a `<`,
    /// recursively parses the inner type annotation, expects `>`, and returns
    /// a `TypeAnnotation` with name `Ptr<{inner.name}>`. The inner type is
    /// alias-normalized by the recursion, so `Ptr<Int>` and `Ptr<Ptr<Str>>`
    /// both work.
    pub(super) fn parse_type_annotation(&mut self) -> Result<TypeAnnotation, ParseError> {
        let (name, line, column) = self.expect_ident()?;

        if name == "Ptr" {
            self.expect(&TokenKind::Lt, "`<`")?;
            let inner = self.parse_type_annotation()?;
            self.expect(&TokenKind::Gt, "`>`")?;
            return Ok(TypeAnnotation {
                name: format!("Ptr<{}>", inner.name),
                line,
                column,
            });
        }

        Ok(TypeAnnotation {
            name: normalize_type_alias(&name).to_string(),
            line,
            column,
        })
    }

    /// Parses statements (via `parse_stmt`) into a `Vec` until the current
    /// token matches one of the given `terminators` (e.g. `[End]` for a
    /// `while`/`def` body, or `[Elsif, Else, End]` for an `if`-branch body).
    /// The terminator token itself is left unconsumed — the caller (e.g.
    /// `parse_if`, `parse_while`) is responsible for advancing past it, since
    /// which terminator matched affects what happens next (another `elsif`
    /// branch vs. the final `end`). Hitting `Eof` before any terminator is an
    /// error, since it means the block was never closed.
    pub(super) fn parse_block(
        &mut self,
        terminators: &[TokenKind],
    ) -> Result<Vec<Stmt>, ParseError> {
        let mut stmts = Vec::new();
        while !terminators.iter().any(|t| self.check(t)) {
            if self.check(&TokenKind::Eof) {
                let tok = self.peek();
                return Err(ParseError {
                    message: "unexpected end of input, expected `end`".to_string(),
                    line: tok.line,
                    column: tok.column,
                });
            }
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    /// Parses a comma-separated list of items — each produced by `parse_item`
    /// — up to and including `terminator` (named by `terminator_desc` for the
    /// "expected X" error message). Handles both the empty list (an immediate
    /// terminator) and a final item with no trailing comma; a trailing comma is
    /// rejected, since the next iteration would try to parse another item and
    /// hit the terminator. Shared by every bracketed list in the grammar —
    /// call args, method-call args, array literals, and function params — which
    /// differ only in the element parser and the closing bracket.
    pub(super) fn parse_comma_separated<T>(
        &mut self,
        terminator: &TokenKind,
        terminator_desc: &str,
        parse_item: fn(&mut Self) -> Result<T, ParseError>,
    ) -> Result<Vec<T>, ParseError> {
        let mut items = Vec::new();
        if !self.check(terminator) {
            loop {
                items.push(parse_item(self)?);
                if self.check(&TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect(terminator, terminator_desc)?;
        Ok(items)
    }
}

mod expressions;
mod statements;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse(src: &str) -> Vec<Stmt> {
        let tokens = Lexer::new(src).tokenize().unwrap();
        Parser::new(tokens).parse_program().unwrap()
    }

    #[test]
    fn parses_function_def() {
        let stmts = parse("def add(a: Int, b: Int): Int\n  a + b\nend");
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::FunctionDef {
                name,
                params,
                return_type,
                body,
                ..
            } => {
                assert_eq!(name, "add");
                assert_eq!(params.len(), 2);
                assert_eq!(params[0].type_ann.name, "Integer");
                assert_eq!(return_type.as_ref().unwrap().name, "Integer");
                assert_eq!(body.len(), 1);
            }
            other => panic!("expected FunctionDef, got {other:?}"),
        }
    }

    #[test]
    fn parses_var_decl_inferred_and_explicit() {
        let stmts = parse("x = 5\ny: Float = 5.0");
        match &stmts[0] {
            Stmt::VarDecl {
                name,
                type_ann: None,
                ..
            } => assert_eq!(name, "x"),
            other => panic!("expected VarDecl, got {other:?}"),
        }
        match &stmts[1] {
            Stmt::VarDecl {
                name,
                type_ann: Some(t),
                ..
            } => {
                assert_eq!(name, "y");
                assert_eq!(t.name, "Float");
            }
            other => panic!("expected VarDecl, got {other:?}"),
        }
    }

    #[test]
    fn parses_type_alias_in_var_decl() {
        let stmts = parse("x: Int = 5\ny: Bool = true\nz: Str = \"hi\"");
        for (stmt, expected) in stmts.iter().zip(["Integer", "Boolean", "String"]) {
            match stmt {
                Stmt::VarDecl {
                    type_ann: Some(t), ..
                } => assert_eq!(t.name, expected),
                other => panic!("expected VarDecl, got {other:?}"),
            }
        }
    }

    #[test]
    fn parses_if_elsif_else() {
        let stmts = parse("if x > 0\n  1\nelsif x < 0\n  2\nelse\n  3\nend");
        match &stmts[0] {
            Stmt::If {
                elsif_branches,
                else_body,
                ..
            } => {
                assert_eq!(elsif_branches.len(), 1);
                assert!(else_body.is_some());
            }
            other => panic!("expected If, got {other:?}"),
        }
    }

    #[test]
    fn parses_while() {
        let stmts = parse("while x > 0\n  x = x - 1\nend");
        assert!(matches!(stmts[0], Stmt::While { .. }));
    }

    #[test]
    fn parses_for_range() {
        let stmts = parse("for i in 0..10\n  print(i)\nend");
        match &stmts[0] {
            Stmt::For { var_name, .. } => assert_eq!(var_name, "i"),
            other => panic!("expected For, got {other:?}"),
        }
    }

    #[test]
    fn parses_call_expr_stmt() {
        let stmts = parse("print(\"hi\")");
        match &stmts[0] {
            Stmt::ExprStmt(Expr::Call { callee, args, .. }) => {
                assert_eq!(callee, "print");
                assert_eq!(args.len(), 1);
            }
            other => panic!("expected call ExprStmt, got {other:?}"),
        }
    }

    #[test]
    fn operator_precedence() {
        // 1 + 2 * 3 should parse as 1 + (2 * 3)
        let stmts = parse("1 + 2 * 3");
        match &stmts[0] {
            Stmt::ExprStmt(Expr::Binary {
                op, left, right, ..
            }) => {
                assert_eq!(*op, BinOp::Add);
                assert!(matches!(**left, Expr::IntLit { value: 1, .. }));
                assert!(matches!(**right, Expr::Binary { op: BinOp::Mul, .. }));
            }
            other => panic!("expected Binary, got {other:?}"),
        }
    }

    #[test]
    fn parse_error_reports_position() {
        let tokens = Lexer::new("def foo(\n  1\nend").tokenize().unwrap();
        let err = Parser::new(tokens).parse_program().unwrap_err();
        assert_eq!(err.line, 2);
    }

    #[test]
    fn const_decl() {
        let stmts = parse("const PI: Float = 3.14");
        match &stmts[0] {
            Stmt::ConstDecl { name, type_ann, .. } => {
                assert_eq!(name, "PI");
                assert_eq!(type_ann.as_ref().unwrap().name, "Float");
            }
            other => panic!("expected ConstDecl, got {other:?}"),
        }
    }

    #[test]
    fn parses_unary_negation() {
        let stmts = parse("x = -5");
        match &stmts[0] {
            Stmt::VarDecl { value, .. } => {
                assert!(matches!(
                    value,
                    Expr::Unary {
                        op: crate::ast::UnOp::Neg,
                        ..
                    }
                ));
            }
            other => panic!("expected VarDecl, got {other:?}"),
        }
    }

    #[test]
    fn parses_array_literal_and_index() {
        let stmts = parse("xs = [1, 2, 3]\ny = xs[0]");
        match &stmts[0] {
            Stmt::VarDecl {
                value: Expr::ArrayLit { elements, .. },
                ..
            } => assert_eq!(elements.len(), 3),
            other => panic!("expected ArrayLit VarDecl, got {other:?}"),
        }
        match &stmts[1] {
            Stmt::VarDecl {
                value: Expr::Index { .. },
                ..
            } => {}
            other => panic!("expected Index VarDecl, got {other:?}"),
        }
    }

    #[test]
    fn parses_import() {
        let stmts = parse("import \"helper\"");
        match &stmts[0] {
            Stmt::Import { path, .. } => assert_eq!(path, "helper"),
            other => panic!("expected Import, got {other:?}"),
        }
    }

    #[test]
    fn parses_class_with_const_field_and_method() {
        let stmts = parse(
            "class Hello\n  const PI: Float = 3.14159\n  count: Integer\n\n  def initializer(number: Int)\n    count = number\n  end\nend",
        );
        match &stmts[0] {
            Stmt::ClassDef {
                name,
                consts,
                fields,
                methods,
                ..
            } => {
                assert_eq!(name, "Hello");
                assert_eq!(consts.len(), 1);
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "count");
                assert_eq!(fields[0].type_ann.name, "Integer");
                assert_eq!(methods.len(), 1);
                match &methods[0] {
                    Stmt::FunctionDef {
                        name: mname,
                        params,
                        ..
                    } => {
                        assert_eq!(mname, "initializer");
                        assert_eq!(params.len(), 1);
                    }
                    other => panic!("expected FunctionDef, got {other:?}"),
                }
            }
            other => panic!("expected ClassDef, got {other:?}"),
        }
    }

    #[test]
    fn parses_new_field_access_and_method_call() {
        let stmts = parse("h = Hello.new(5)\nx = h.count\nh.greet(\"hi\")\nh.count = 9");
        match &stmts[0] {
            Stmt::VarDecl {
                value:
                    Expr::MethodCall {
                        method,
                        args,
                        object,
                        ..
                    },
                ..
            } => {
                assert_eq!(method, "new");
                assert_eq!(args.len(), 1);
                assert!(matches!(**object, Expr::Ident { .. }));
            }
            other => panic!("expected MethodCall VarDecl, got {other:?}"),
        }
        match &stmts[1] {
            Stmt::VarDecl {
                value: Expr::FieldAccess { field, .. },
                ..
            } => assert_eq!(field, "count"),
            other => panic!("expected FieldAccess VarDecl, got {other:?}"),
        }
        match &stmts[2] {
            Stmt::ExprStmt(Expr::MethodCall { method, args, .. }) => {
                assert_eq!(method, "greet");
                assert_eq!(args.len(), 1);
            }
            other => panic!("expected MethodCall ExprStmt, got {other:?}"),
        }
        match &stmts[3] {
            Stmt::FieldAssign { field, .. } => assert_eq!(field, "count"),
            other => panic!("expected FieldAssign, got {other:?}"),
        }
    }

    #[test]
    fn parses_ptr_type_annotation() {
        let stmts = parse("p: Ptr<Integer> = alloc(5)");
        match &stmts[0] {
            Stmt::VarDecl {
                name,
                type_ann: Some(t),
                ..
            } => {
                assert_eq!(name, "p");
                assert_eq!(t.name, "Ptr<Integer>");
            }
            other => panic!("expected VarDecl with Ptr type, got {other:?}"),
        }
    }

    #[test]
    fn parses_ptr_with_alias_normalization() {
        let stmts = parse("p: Ptr<Int> = alloc(5)");
        match &stmts[0] {
            Stmt::VarDecl {
                type_ann: Some(t), ..
            } => assert_eq!(t.name, "Ptr<Integer>"),
            other => panic!("expected VarDecl with Ptr type, got {other:?}"),
        }
    }

    #[test]
    fn parses_nested_ptr_type_annotation() {
        let stmts = parse("p: Ptr<Ptr<Integer>> = alloc(q)");
        match &stmts[0] {
            Stmt::VarDecl {
                type_ann: Some(t), ..
            } => assert_eq!(t.name, "Ptr<Ptr<Integer>>"),
            other => panic!("expected VarDecl with nested Ptr type, got {other:?}"),
        }
    }

    #[test]
    fn parses_ptr_missing_closing_bracket() {
        let tokens = Lexer::new("p: Ptr<Integer = alloc(5)").tokenize().unwrap();
        let err = Parser::new(tokens).parse_program().unwrap_err();
        assert!(err.message.contains("expected `>`"));
    }
}
