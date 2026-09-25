# ruby/lib/yara/

The pipeline, `lexer` → `parser` → `resolver` → `typechecker` → `interpreter`, plus the files every stage shares. `parser/`, `typechecker/` and `interpreter/` have their own `CLAUDE.md`. `docs/architecture.md` walks the control flow with diagrams; keep both in sync when a stage's algorithm changes.

Everything here must also compile with `mrbc`: no `require` outside `../yara.rb`, and only the Ruby subset listed in `ruby/PLAN.md`.

## Shared files

- `ast.rb`: one `Struct` per node, members ending in `line, column` (1-indexed position of the construct's first token). Binary operators are the symbols in `BINARY_OPERATORS` (spelled via `OPERATOR_SYMBOLS` in messages); the only unary operator is `:neg`. `ExprStmt` takes its position from its expression. `ClassDef#consts` holds only `ConstDecl`s and `#methods` only `FunctionDef`s; `fields` holds `FieldDecl`s (a declaration with no value). `If#elsif_branches` is an array of `[condition, body]` pairs. `Node#shift_lines` walks every member generically, so a new node type needs no shifting code.
- `diagnostics.rb`: `Error` (the base of every stage's error; subclasses define `kind`, `RuntimeError` also `frames`), `Span`, `Frame`, `SourceMap`, and `render`/`render_with_map`/`render_snippet`. Output is `kind: message`, `  --> path:line:column`, a gutter-aligned snippet with a caret, then one `in `name` at ...` block per call-stack frame, innermost first. `source_lines` reproduces Rust's `str::lines` (no empty last line, trailing `\r` dropped), which the line counts and snippets depend on. `render_with_map` localizes the stage label and the frame words `in`/`at` when given a vocabulary; lex and parse errors go through plain `render`, so their labels stay English.
- `environment.rb`: the scope stack used by the typechecker (over types) and the interpreter (over values). `lookup` returns nil when unbound, so use `bound?` where a bound value can be nil. `set_or_declare` mutates the nearest existing binding, which is what makes `x = x + 1` in a loop update the outer `x`.
- `types.rb`: alias normalization, `Int`→`Integer`, `Bool`→`Boolean`, `Str`→`String`, applied by the parser so later stages see only canonical names.
- `builtins.rb` / `methods.rb`: the arity registries for free builtins and primitive methods. Adding one means an entry here plus `check_builtin_<name>`/`eval_builtin_<name>` (or `check_<kind>_<name>`/`eval_<kind>_<name>`) in the typechecker and interpreter; `test/typechecker_test.rb` and `test/interpreter_test.rb` fail when an implementation is missing. `print` is variadic and special-cased in both stages instead.
- `messages.rb`: the English message catalog, `stage/kebab-name` keys with `{0}`, `{1}` placeholders, and `Messages.substitute`.
- `rust_format.rb`: Yara's number and string formatting, which follows Rust's `{}`/`{:?}` conventions: floats print without `.0` or exponent (`5`, `100000000000000000000`), debug form keeps them (`5.0`, `1e16`). `parse_float` and `shortest_digits` convert between decimal text and floats in exact integer arithmetic, because mruby's own `String#to_f` and `%e` are not exact. The lexer, `String#to_f` and every float printed go through here; never use `Float#to_s`.
- `cli.rb`: `yara run <file> [--vocabulary <path>]` (`--keywords` is an older alias). Reads the source, then the vocabulary, then runs the stages, rendering the first error and returning 1.

## Lexer (`lexer.rb`)

- Walks characters by hand (mruby has no `Regexp`): `tokenize` skips whitespace and `#` comments, then dispatches on the character to `read_number`, `read_string`, `read_ident_or_keyword` or `read_operator`. Only `peek`/`advance` move the position, so line and column tracking lives in one place.
- A `Token` is a `kind` symbol, a `value` and its position. Keywords come from the vocabulary's `keywords` map (spelling to kind), so a translated vocabulary changes which words are keywords.
- `Lexer.describe(token)` prints a token as parse errors show it (`Ident("x")`, `Float(5.0)`, `Plus`).
- String escapes: `\n \t \" \\`. Errors (unterminated string, bad escape, unexpected character) are `LexError`s with catalog messages.
- Non-ASCII letters count as identifier characters, a simplification of the full Unicode alphabetic table marked `ponytail:`.
- Gotchas: `!` alone is a lex error (there is no boolean negation); `..` is the only range operator.

## Resolver (`resolver.rb`)

- Replaces each `import "path"` in place with the imported file's statements, recursively, so imported functions and classes land in the importer's flat global namespace. Name collisions across files silently overwrite.
- Paths resolve relative to the importing file's directory, with `.yara` appended when there is no extension. `import_path` spells the result the way Rust's `Path::join` does (no `./` for a bare entry file name), since it appears in messages and `-->` lines.
- Each imported file is registered in the `SourceMap`, which gives it a disjoint range of virtual line numbers; its AST is shifted into that range with `shift_lines`, and later diagnostics map back to the right file and snippet.
- A shared list of canonical paths (`File.realpath`), seeded with the entry file, detects cycles. Importing any file twice anywhere in a run counts as a cycle.
- Errors (`ResolveError`, "import error") point at the importing `import` statement; lex and parse errors inside an imported file are wrapped with its path.
- Only top-level `import` is meaningful; the parser accepts it anywhere, but nothing exercises that.

## Vocabularies (`translations.rb`)

- `Vocabulary.parse` reads a `.vocab` file: `#` comments, `canonical = localized` lines split on the first `=`, under `[keywords]` (the default before any header), `[types]`, `[builtins]`, `[methods]` or `[messages]`. A file overrides only what it mentions.
- Every name map starts with English identity entries (`Vocabulary.english`), and translating a name removes its old spelling, so a spelling means one name per section. The "already used" check reads those identity entries, so `Float = Integer` is rejected.
- `TranslationError` ("keyword translation error") renders against the vocabulary file, line-numbered within it.
- Lookups: `canonical_type`/`canonical_builtin`/`canonical_method` map a source spelling to English; `type_name`, `localized_method_names` and `bool_word` map back for messages and `Boolean#to_s`. `msg(key, args)` uses the `[messages]` override, else the English catalog.
- Identifiers, string contents, `initializer` and import paths never translate.
- Still hardcoded English, not in the catalog: the lexer's unexpected `!`; the parser's invalid assignment target, missing import path, unexpected end of input and bad class-body statement; the typechecker's `for` range bounds; the interpreter's nil-pointer, `free` of nil, empty `pop` and integer-overflow errors; and the vocabulary file's own errors.
