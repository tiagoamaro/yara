# Yara in Ruby

This folder is the implementation of Yara, a learning-focused, strongly typed language with Ruby and Pascal flavored syntax. It runs two ways: under CRuby for development (`bin/yara`), and as a standalone `yara` executable built with mruby (`build/yara`), which needs no Ruby installed to run.

## Requirements

- Ruby 4.0.5, pinned in the repo-root `.tool-versions`. With asdf: `asdf install`.
- A C compiler (`cc`) and `curl`, only for `make build`.

No gems to install: `rake` and `minitest` ship with Ruby.

## Usage

Run these from this folder:

```sh
make                                   # list every task
make test                              # every test
make parity                            # only the per-example comparison with the expected output
make run FILE=examples/hello.yara      # run a program under CRuby (paths are relative to the repo root)
make run FILE=examples/translations/hello_pt.yara ARGS="--vocabulary translations/pt.vocab"
make build                             # build the standalone executable, build/yara
make parity-mruby                      # the per-example comparison through build/yara
make capture                           # record bin/yara's output as every example's expected output
```

## The standalone executable

### Building it

```sh
make build
```

The first run downloads mruby 4.0.0 into `build/mruby-4.0.0/` and compiles it with `mruby/build_config.rb`, which takes about a minute. Later runs reuse it and only relink. The steps, all in the `Rakefile`:

1. Build mruby (`libmruby.a`, `mrbc`, `mruby-config`) from `mruby/build_config.rb`.
2. Compile every file `lib/yara.rb` requires, in that order, plus `mruby/main.rb`, into one bytecode blob with `mrbc` (`build/yara_bytecode.c`).
3. Compile `mruby/yara.c`, the launcher, and link it with the bytecode and `libmruby.a` into `build/yara`.

`make build` rebuilds when a source file, the launcher or the build configuration changes. To start from scratch, delete `build/`.

### Using it

`build/yara` takes the same arguments as `bin/yara`:

```sh
build/yara run program.yara
build/yara run program.yara --vocabulary translations/pt.vocab
```

It carries the whole implementation inside it and reads no Ruby files at runtime, so it can be copied anywhere and run on a machine without Ruby. It links the C library dynamically, so run it on the same operating system and CPU architecture it was built on, and on a C library at least as new as the build machine's. Exit status is 0 on success and 1 on any error, as with `bin/yara`.

### Checking it

```sh
make parity-mruby                          # the minitest harness, through build/yara
script/parity.sh ruby/build/yara           # the same check in plain shell, run from the repo root
```

`script/parity.sh` needs only a POSIX shell, `find` and `cmp`. CI uses it to check the executable in an Ubuntu container with no Ruby installed.

### Platforms

CI builds `build/yara` on Linux x64 and macOS arm64 and uploads each as an artifact named `yara-<OS>-<arch>` on every run. For another platform, run `make build` on that platform; mruby needs a C compiler and Ruby only to build.

### Adding an mruby gem

The executable includes mruby's `default` gembox, which covers everything the implementation uses (`mruby-io`, `mruby-errno`, `mruby-bigint`, `mruby-data`, `mruby-struct`, `mruby-sprintf`, `mruby-math` and the core extensions). To add a gem, list it in `mruby/build_config.rb`:

```ruby
conf.gem core: "mruby-time"                     # a gem that ships with mruby
conf.gem github: "owner/mruby-some-gem"         # a third-party gem
```

Then `make build` rebuilds mruby with it. mruby records the resolved gem versions in `mruby/build_config.rb.lock`; commit that file with the change. Keep CRuby in mind: `bin/yara` runs the same code under CRuby, so a new dependency needs an equivalent there too.

## How it is checked

The fixtures live at the repo root:

- `examples/`: the Yara programs.
- `tests/stdout/`: what each program prints.
- `tests/golden/`: the exact error output of each program in `examples/errors/`.

`test/parity_test.rb` runs every example through `bin/yara` (or `YARA_BINARY`) and compares stdout, stderr and exit status with those files. After adding an example, run `make capture` and review the new files before committing them.

## Layout

- `bin/yara`: command-line entry point under CRuby.
- `lib/yara.rb`: loads the other files in order. It is the only file that uses `require`, because mruby has none, and `rake build` compiles the same list with `mrbc`.
- `lib/yara/`: the pipeline (lexer, parser, resolver, typechecker, interpreter, plus AST, diagnostics, environment and vocabularies). Its `CLAUDE.md` files describe each stage.
- `test/`: minitest tests.
- `mruby/`: the mruby build configuration, the entry script and the C launcher that embeds the compiled bytecode.
- `script/capture_output.rb`: the script behind `make capture`.
- `script/parity.sh`: the per-example comparison in plain shell.

The code sticks to the Ruby subset that both CRuby and mruby support. `PLAN.md` lists what is allowed and the places where plain Ruby behaves differently from Yara's defined behavior.
