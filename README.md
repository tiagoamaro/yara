# Yara

Yara is a learning-focused programming language: strongly typed, compiled, with syntax blending Ruby (low punctuation, `def`/`end` blocks, expression-oriented) and Pascal (explicit, declarative feel).

## Goals

- Learn how a real compiler pipeline works: lexer, parser, typechecker, interpreter (native codegen later).
- Strong typing, no implicit coercion between numeric types.
- Friendly, explicit diagnostics — every error traceable to an exact line and column, at compile time and at runtime.

## Status

Lexer, parser, import resolver, typechecker and tree-walk interpreter all work, and `yara run <file>` executes real `.yara` programs. The language has functions with implicit returns, `if`/`elsif`/`else`, `while` and `for` loops, file `import`, arrays (`IntArray`/`FloatArray`/`BoolArray`/`StringArray`), classes with single inheritance, methods on primitive values (`xs.size()`, `2.to_s()`), opt-in pointers with manual `free` and a teaching garbage collector (`collect()`), and full-vocabulary translation. `docs/plan-next-milestones.md` tracks what is next.

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

1. Lexer, parser, typechecker, tree-walk interpreter
2. Arrays, imports, classes and inheritance, primitive methods
3. Pointers with manual memory management and a teaching garbage collector
4. Full-vocabulary translation
5. A standalone executable built with mruby
6. Later: native compilation (LLVM/Cranelift or C transpile)

## Architecture

`docs/architecture.md` walks through the real pipeline (`Lexer` -> `Parser` -> `Resolver` -> `TypeChecker` -> `Interpreter`) with Mermaid diagrams and the actual method names involved, written for anyone studying how a small compiler/interpreter is put together. Yara is implemented in Ruby, in `ruby/lib/yara/`, and every method there has a YARD comment explaining what it does.

## Examples

`examples/` has runnable `.yara` programs, organized by theme:
- Top-level: language-feature smoke tests (`hello.yara`, `functions.yara`, `types.yara`, `control_flow.yara`, `loops.yara`, `recursion.yara`, `constants.yara`, `kitchen_sink.yara`).
- `data_structures/` — list, stack, queue, linked list, binary tree, graph (arena-style, built on arrays).
- `objects/` — `class` usage.
- `errors/` — deliberately-failing programs showing rendered lex/parse/type/runtime error output, including a recursive call-stack trace.
- `pointers/` — manual `alloc`/`free` and the `collect()` garbage collector.
- `translations/` — programs written entirely in Portuguese (`--vocabulary translations/pt.vocab`).

## Translation

`yara run <file> --vocabulary <path>` lets a program be written in another language: keywords, type names, builtins, primitive methods and error messages all come from the vocabulary file. See `translations/pt.vocab` and `examples/translations/`. Identifiers and string contents are never translated. `--keywords <path>` still works as an older alias.

## Editor support

`editors/vscode-yara/` is a minimal VS Code extension providing syntax highlighting for `.yara` files (TextMate grammar only — no language server). See its `README.md` for install instructions.

## Running

With Ruby 4.0.5 (pinned in `.tool-versions`), from the repo root:

```
ruby/bin/yara run examples/hello.yara
```

## The standalone executable

`make build` in `ruby/` compiles the implementation with mruby into one native executable, `ruby/build/yara`, which runs without Ruby installed:

```
cd ruby && make build && cd ..
ruby/build/yara run examples/hello.yara
```

It takes the same arguments as `ruby/bin/yara` (e.g. `yara run <file> --vocabulary <path>`) and can be copied anywhere with the same operating system and architecture. `ruby/README.md` covers building it, testing it, adding mruby gems and platform builds.
