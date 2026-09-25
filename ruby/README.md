# Yara in Ruby

This folder is the Ruby implementation of Yara, a learning-focused, strongly typed language with Ruby and Pascal flavored syntax. It is a port of the Rust implementation in `../rust/`. Once it reproduces the Rust output exactly, it will ship as a standalone `yara` executable built with mruby, so running Yara programs won't need Ruby or Rust installed.

The port runs every example with the same output as the Rust binary, under CRuby (`bin/yara`) and as the standalone mruby executable (`build/yara`). `PLAN.md` lists the steps and tracks which ones are done; retiring `../rust/` is the last one.

## Requirements

- Ruby 4.0.5, pinned in the repo-root `.tool-versions`. With asdf: `asdf install`.
- A C compiler and `curl`, only for `make build`, which downloads and builds mruby 4.0.0 under `build/`.
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
make build                             # build the standalone executable, build/yara
make parity-mruby                      # the parity comparison through build/yara
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
- `lib/yara.rb`: loads the other files in order. It is the only file that uses `require`, because mruby has none, and `rake build` compiles the same list with `mrbc`.
- `lib/yara/`: the pipeline, mirroring `../rust/src/` (lexer, parser, resolver, typechecker, interpreter, plus AST, diagnostics and environment).
- `test/`: minitest tests, mostly ported from the Rust unit tests.
- `mruby/`: the mruby build configuration, the entry script and the C launcher that embeds the compiled bytecode.
- `script/capture_rust_stdout.rb`: the script behind `make capture`.
- `script/parity.sh`: the parity comparison in plain shell, for checking `build/yara` on a machine without Ruby (CI does this).

The code sticks to the Ruby subset that both CRuby and mruby support. `PLAN.md` lists what is allowed and the known places where plain Ruby behaves differently from the Rust implementation.
