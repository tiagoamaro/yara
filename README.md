# Yara

Yara is a learning-focused programming language: strongly typed, compiled, with syntax blending Ruby (low punctuation, `def`/`end` blocks, expression-oriented) and Pascal (explicit, declarative feel).

## Goals

- Learn how a real compiler pipeline works: lexer, parser, typechecker, interpreter (native codegen later).
- Strong typing, no implicit coercion between numeric types.
- Friendly, explicit diagnostics — every error traceable to an exact line and column, at compile time and at runtime.

## Status

Interpreter-first milestone complete and then some: lexer, parser, typechecker, and tree-walk interpreter all working, `yara run <file>` executes real `.yara` programs. Since the initial milestone: unary negation, `if`/`elsif`/`else` as a function's tail expression, file `import`, an `Array` type (`IntArray`/`FloatArray`/`BoolArray`/`StringArray` with indexing and `len`/`push`/`pop`/`get`/`set`), `class` declarations (no inheritance), and configurable keyword translation (`--keywords <path>`). See `CLAUDE.md` for the current project map and each subfolder's `CLAUDE.md` for stage-specific status.

## Syntax preview

```
def add(a: Int, b: Int): Int
  a + b
end

x = 5          # inferred Int
y: Float = 5.0 # explicit annotation

if x > 0
  print("positive")
end

xs: IntArray = [1, 2, 3]
push(xs, 4)
print(xs[0])

import "helper"   # splices helper.yara's top-level declarations in

class Hello
  const PI: Float = 3.14159
  count: Integer

  def initializer(number: Int)
    count = number
  end
end

h = Hello.new(5)
print(h.count)
```

Type names have short and long aliases: `Int`/`Integer`, `Bool`/`Boolean`, `Str`/`String` are interchangeable.

## Roadmap

1. Lexer
2. AST + parser
3. Typechecker
4. Tree-walk interpreter
5. Arrays, imports, classes, configurable keyword translation
6. Later: native compilation (LLVM/Cranelift) or C transpile; class inheritance/static methods; opt-in pointers + a teaching-focused garbage collector — see root `CLAUDE.md` TODO for design sketches

## Architecture

`docs/architecture.md` walks through the real pipeline (`Lexer` -> `Parser` -> `resolver` -> `TypeChecker` -> `Interpreter`) with Mermaid diagrams and the actual function names involved — written for anyone studying how a small compiler/interpreter is put together. Every function in `src/` also has a `///` doc comment explaining its mechanics, not just its name.

## Examples

`examples/` has runnable `.yara` programs, organized by theme:
- Top-level: language-feature smoke tests (`hello.yara`, `functions.yara`, `types.yara`, `control_flow.yara`, `loops.yara`, `recursion.yara`, `constants.yara`, `kitchen_sink.yara`).
- `data_structures/` — list, stack, queue, linked list, binary tree, graph (arena-style, built on arrays).
- `objects/` — `class` usage.
- `errors/` — deliberately-failing programs showing rendered lex/parse/type/runtime error output, including a recursive call-stack trace.
- `translations/` — the same `class` example, written with Portuguese keywords (`--keywords translations/pt.keywords`).

## Keyword translation

`yara run <file> --keywords <path>` lets `if`/`while`/`class`/etc. be written in another language — see `translations/pt.keywords` and `examples/translations/hello_pt.yara`. Only the fixed set of reserved words translate; type names, identifiers, and error messages stay in English.

## Editor support

`editors/vscode-yara/` is a minimal VS Code extension providing syntax highlighting for `.yara` files (TextMate grammar only — no language server). See its `README.md` for install instructions.

## Running

```
cargo run -- run examples/hello.yara
```
