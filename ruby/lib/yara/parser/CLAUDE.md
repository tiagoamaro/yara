# ruby/lib/yara/parser/

Recursive-descent parser turning tokens into `AST` nodes. `../parser.rb` holds `Parser`, `ParseError` and the token helpers (`peek`, `check`, `advance`, `expect`, `expect_ident`, `parse_type_annotation`, `parse_block`, `parse_comma_separated`); `statements.rb` and `expressions.rb` reopen `Parser`.

## Design
- `parse_statement` dispatches on the leading token (`def`, `const`, `class`, `import`, `return`, `if`, `while`, `for`, else `parse_ident_statement` or an expression statement).
- Expressions use precedence climbing: `parse_expression` → `parse_comparison` → `parse_additive` → `parse_multiplicative` → `parse_unary` → `parse_postfix(parse_primary)`. The binary levels share `parse_left_associative`, which builds left-associative trees. `parse_unary` is right-recursive, so `--x` parses.
- `parse_postfix` loops over `[index]`, `.field` and `.method(args)`, so they chain freely. A method call always has parentheses; `.name` without them is a field access. `ClassName.new(args)` parses as an ordinary `MethodCall`; the typechecker and interpreter decide it is construction.
- `parse_ident_statement`: `x: Type = value` is recognized by the colon; anything else is parsed as a full expression first, then a following `=` makes an `Ident` a `VarDecl` and a `FieldAccess` a `FieldAssign`. Any other target is "invalid assignment target".
- `parse_type_annotation` normalizes aliases (and localized type names, through the vocabulary) and parses `Ptr<T>` recursively, recording it as the single name `"Ptr<Integer>"`.
- `parse_class` runs a restricted loop: only `const`, a bare `name: Type` field, or `def` may appear in a class body. `< Parent` after the name reuses the `<` token.
- Every comma-separated list (call arguments, parameters, array literals) goes through `parse_comma_separated`; trailing commas are rejected.
- Messages come from the vocabulary's catalog (`parse/...` keys), with tokens printed by `Lexer.describe`.

## Gotchas
- Newlines are not statement terminators, so a line starting with unary `-` joins the previous line's expression as a binary minus: `if n < 0` followed by `-1` parses as `n < 0 - 1` with an empty body. Write `0 - 1` or parenthesize.
- `import` is accepted anywhere a statement is, though only top-level imports are meaningful.
