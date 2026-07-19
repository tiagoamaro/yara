# Yara

Yara is a learning-focused programming language: strongly typed, compiled, with syntax blending Ruby (low punctuation, `def`/`end` blocks, expression-oriented) and Pascal (explicit, declarative feel).

## Goals

- Learn how a real compiler pipeline works: lexer, parser, typechecker, interpreter (native codegen later).
- Strong typing, no implicit coercion between numeric types.
- Friendly, explicit diagnostics — every error traceable to an exact line and column, at compile time and at runtime.

## Status

Interpreter-first milestone complete: lexer, parser, typechecker, and tree-walk interpreter all working, `yara run <file>` executes real `.yara` programs. See `CLAUDE.md` for the current project map and each subfolder's `CLAUDE.md` for stage-specific status.

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
```

Type names have short and long aliases: `Int`/`Integer`, `Bool`/`Boolean`, `Str`/`String` are interchangeable.

## Roadmap

1. Lexer
2. AST + parser
3. Typechecker
4. Tree-walk interpreter
5. Examples (functions, base types)
6. Later: native compilation (LLVM/Cranelift) or C transpile

## Running

```
cargo run -- run examples/hello.yara
```
