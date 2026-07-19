# Yara for VS Code

Syntax highlighting for `.yara` files. A TextMate grammar only — no language server, no diagnostics, no folding, no autocomplete. It colors the source; the compiler (`cargo run -- run <file>` from the repo root) is what actually catches errors, with source-snippet-and-caret rendering (see `src/main.rs`).

## What's covered

- Comments (`# ...`)
- String literals, with `\n \t \" \\` escapes
- Integer and float literals
- Keywords (`def end if elsif else while for in const return import class`)
- `true`/`false`/`nil`
- Base types and array types (`Int`/`Integer`, `Float`, `Bool`/`Boolean`, `Str`/`String`, `Nil`, `IntArray`/`FloatArray`/`BoolArray`/`StringArray`)
- Function/method definition names (after `def`) and class names (after `class`)
- Function calls (`name(`) and member access/calls (`.name`)
- Operators (`+ - * / == != < > <= >= = := : .. .`)

## What's not covered (known limitations)

TextMate grammars are regex-based pattern matching over text, not a real parser — they don't know Yara's actual grammar the way `src/parser/` does. So:
- A capitalized identifier used as a type annotation (e.g. `h: Hello`) that isn't one of the built-in type names just highlights as a plain identifier, not a type — regex can't know `Hello` is a user-defined class without cross-referencing every `class` declaration in the file.
- `.name` after any expression is highlighted the same way whether it's a field read (`h.count`) or a method call (`h.area(2.0)`) — same limitation.
- No error checking, no go-to-definition, no autocomplete. Those would require an actual language server talking to the Rust compiler, which doesn't exist yet.

## Installing (unpublished, local use only)

This extension isn't published to the VS Code Marketplace. To use it locally:

```sh
ln -s "$(pwd)" ~/.vscode/extensions/yara-language
```

(On Windows, copy the folder instead of symlinking, or use `mklink /D`.) Then reload VS Code — files with a `.yara` extension should pick up highlighting immediately.
