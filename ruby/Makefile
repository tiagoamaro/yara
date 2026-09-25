.PHONY: help test parity capture run build parity-mruby

help: ## List the available tasks
	@awk 'BEGIN { FS = ":.*## " } /^[a-z]+:.*## / { printf "  make %-8s %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

test: ## Run every Ruby test, parity included
	rake test

parity: ## Run only the per-example comparison with the Rust output
	rake test TEST=test/parity_test.rb

build: ## Build the standalone mruby executable, build/yara
	rake build

parity-mruby: build ## Run the per-example comparison through build/yara
	YARA_BINARY=build/yara rake test TEST=test/parity_test.rb

capture: ## Regenerate tests/stdout/ from the Rust binary
	ruby script/capture_rust_stdout.rb

run: ## Run a program from the repo root: make run FILE=examples/hello.yara [ARGS="--vocabulary ..."]
	cd .. && ruby/bin/yara run $(FILE) $(ARGS)
