# Yara

Yara is a learning-focused programming language: strongly typed, with syntax blending Ruby (low punctuation, `def`/`end` blocks, expression-oriented) and Pascal (explicit, declarative feel). It is implemented in Ruby, and ships as a standalone executable built with mruby that needs no Ruby installed to run.

## Goals

- Learn how a real language pipeline works: lexer, parser, typechecker, interpreter (native codegen later).
- Strong typing, no implicit coercion between numeric types.
- Friendly, explicit diagnostics: every error traceable to an exact line and column, at check time and at runtime.

## Quick start

You need Ruby 4.0.5, pinned in `.tool-versions`. With [asdf](https://asdf-vm.com): `asdf install`. No gems to install; `rake` and `minitest` ship with Ruby. Every command below runs from the repo root.

Run a program:

```sh
bin/yara run examples/hello.yara
```

Run the tests:

```sh
make test
```

Build the standalone executable and run a program with it (needs a C compiler and `curl`):

```sh
make build
build/yara run examples/hello.yara
```

`make` on its own lists every task:

| Command | What it does |
|---|---|
| `make test` | Run every test, including the per-example comparison |
| `make parity` | Run only the per-example comparison with the expected output |
| `make run FILE=examples/hello.yara` | Run a program with `bin/yara` (add `ARGS="--vocabulary translations/pt.vocab"` for a translated one) |
| `make build` | Build the standalone executable, `build/yara` |
| `make parity-mruby` | Build, then run the per-example comparison through `build/yara` |
| `make capture` | Record `bin/yara`'s output as every example's expected output |

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

Type names have short and long aliases: `Int`/`Integer`, `Bool`/`Boolean`, `Str`/`String` are interchangeable. `docs/syntax.md` documents the whole language.

## Status

Lexer, parser, import resolver, typechecker and tree-walk interpreter all work. The language has functions with implicit returns, `if`/`elsif`/`else`, `while` and `for` loops, file `import`, arrays (`IntArray`/`FloatArray`/`BoolArray`/`StringArray`), classes with single inheritance, methods on primitive values (`xs.size()`, `2.to_s()`), opt-in pointers with manual `free` and a teaching garbage collector (`collect()`), and full-vocabulary translation. `docs/plan-next-milestones.md` tracks what is next.

## Examples

`examples/` has runnable `.yara` programs, organized by theme:
- Top-level: language-feature smoke tests (`hello.yara`, `functions.yara`, `types.yara`, `control_flow.yara`, `loops.yara`, `recursion.yara`, `constants.yara`, `kitchen_sink.yara`).
- `data_structures/`: list, stack, queue, linked list, binary tree, graph (arena-style, built on arrays).
- `objects/`: classes and inheritance.
- `errors/`: deliberately failing programs showing rendered lex/parse/type/runtime error output, including a recursive call-stack trace.
- `pointers/`: manual `alloc`/`free` and the `collect()` garbage collector.
- `translations/`: programs written entirely in Portuguese.

## Translation

`--vocabulary <path>` lets a program be written in another language: keywords, type names, builtins, primitive methods and error messages all come from the vocabulary file.

```sh
bin/yara run examples/translations/hello_pt.yara --vocabulary translations/pt.vocab
```

See `translations/pt.vocab` for the format. Identifiers and string contents are never translated. `--keywords <path>` still works as an older alias.

## The standalone executable

`make build` compiles the implementation into one native executable, `build/yara`. The first run downloads mruby 4.0.0 into `build/` and compiles it, which takes about a minute; later runs only relink. The steps, all in the `Rakefile`:

1. Build mruby (`libmruby.a`, `mrbc`, `mruby-config`) from `mruby/build_config.rb`.
2. Compile every file `lib/yara.rb` requires, in that order, plus `mruby/main.rb`, into one bytecode blob with `mrbc`.
3. Compile the launcher, `mruby/yara.c`, and link it with the bytecode and `libmruby.a` into `build/yara`.

`make build` rebuilds when a source file, the launcher or the build configuration changes. To start from scratch, delete `build/`.

**Using it.** `build/yara` takes the same arguments as `bin/yara` and exits 0 on success, 1 on any error. It carries the whole implementation and reads no Ruby files, so it can be copied anywhere and run on a machine without Ruby. It links the C library dynamically, so run it on the operating system and CPU architecture it was built on.

**Checking it.** `make parity-mruby` runs every example through it. `script/parity.sh build/yara` does the same in plain shell (needs only `sh`, `find` and `cmp`); CI uses it in an Ubuntu container with no Ruby installed.

**Platforms.** CI builds `build/yara` on Linux x64 and macOS arm64 and uploads each as an artifact named `yara-<OS>-<arch>`. For another platform, run `make build` there; Ruby and a C compiler are needed only to build.

**Adding an mruby gem.** The executable includes mruby's `default` gembox, which covers everything the implementation uses. To add a gem, list it in `mruby/build_config.rb`:

```ruby
conf.gem core: "mruby-time"                     # a gem that ships with mruby
conf.gem github: "owner/mruby-some-gem"         # a third-party gem
```

Then `make build` rebuilds mruby with it; commit the updated `mruby/build_config.rb.lock`. `bin/yara` runs the same code under CRuby, so a new dependency needs an equivalent there too.

## How it is checked

- `examples/`: the Yara programs.
- `test/stdout/`: what each program prints.
- `test/golden/`: the exact error output of each program in `examples/errors/`.

`test/parity_test.rb` runs every example and compares stdout, stderr and exit status with those files. After adding an example, run `make capture` and review the new files before committing them.

## Project layout

- `bin/yara`: command-line entry point under CRuby.
- `lib/yara.rb`: loads the other files in order. It is the only file that uses `require`, because mruby has none, and `make build` compiles the same list.
- `lib/yara/`: the pipeline (lexer, parser, resolver, typechecker, interpreter, plus AST, diagnostics, environment and vocabularies).
- `test/`: minitest tests and the expected output of every example.
- `mruby/`: the mruby build configuration, the entry script and the C launcher.
- `script/`: `capture_output.rb` (behind `make capture`) and `parity.sh`.
- `examples/`, `translations/`: Yara programs and vocabulary files.
- `docs/`: `architecture.md` (how the pipeline works, with diagrams), `syntax.md`, `plan-next-milestones.md`, and `ruby-rewrite-plan.md` (including the Ruby subset that must work under mruby).
- `editors/vscode-yara/`: syntax highlighting for VS Code.

The code sticks to the Ruby subset that both CRuby and mruby support; `docs/ruby-rewrite-plan.md` lists it.

## Architecture

`docs/architecture.md` walks through the real pipeline (`Lexer` -> `Parser` -> `Resolver` -> `TypeChecker` -> `Interpreter`) with Mermaid diagrams and the actual method names involved, written for anyone studying how a small compiler/interpreter is put together. Every method in `lib/yara/` has a YARD comment explaining what it does.

## Editor support

`editors/vscode-yara/` is a minimal VS Code extension providing syntax highlighting for `.yara` files (TextMate grammar only, no language server). See its `README.md` for install instructions.
