# Yara in Ruby

This folder is the Ruby implementation of Yara, a learning-focused, strongly typed language with Ruby and Pascal flavored syntax. It is a port of the Rust implementation in `../rust/`. Once it reproduces the Rust output exactly, it will ship as a standalone `yara` executable built with mruby, so running Yara programs won't need Ruby or Rust installed.

The port is in progress. `PLAN.md` lists the steps and tracks which ones are done. Until the port is finished, `yara run` only prints a usage error or a "not implemented yet" message, and the Rust binary is the working implementation.

## Requirements

- Ruby 4.0.5, pinned in the repo-root `.tool-versions`. With asdf: `asdf install`.
- Rust, only for `make capture`, which rebuilds the expected output from the Rust binary.

No gems to install: `rake` and `minitest` ship with Ruby.

## Usage

Run these from this folder:

```sh
make                                   # list every task
make test                              # every test
make parity                            # only the comparison with the Rust output
make run FILE=examples/hello.yara      # run a program (paths are relative to the repo root)
make run FILE=examples/translations/hello_pt.yara ARGS="--vocabulary translations/pt.vocab"
make capture                           # regenerate tests/stdout/ from the Rust binary
```

## How the port is checked

Both implementations share the fixtures at the repo root:

- `examples/`: the Yara programs.
- `tests/stdout/`: what each program prints, captured from the Rust binary.
- `tests/golden/`: the exact error output of each program in `examples/errors/`.

`test/parity_test.rb` runs every example through `bin/yara` and compares stdout, stderr and exit status with those files. An example is skipped until every pipeline stage it reaches has been ported (`PORTED_STAGES` in that test), so the suite stays green as the port moves forward stage by stage. The skip count shows what is left.

## Layout

- `bin/yara`: command-line entry point.
- `lib/yara.rb`: loads the other files in order. It is the only file that uses `require`, because mruby has none, and the mruby build will compile the same list.
- `lib/yara/`: the pipeline, mirroring `../rust/src/` (lexer, parser, resolver, typechecker, interpreter, plus AST, diagnostics and environment).
- `test/`: minitest tests, mostly ported from the Rust unit tests.
- `script/capture_rust_stdout.rb`: the script behind `make capture`.

The code sticks to the Ruby subset that both CRuby and mruby support. `PLAN.md` lists what is allowed and the known places where plain Ruby behaves differently from the Rust implementation.
