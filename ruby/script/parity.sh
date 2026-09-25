#!/bin/sh
# Runs every example through a yara executable and compares stdout, stderr
# and exit status with the Rust captures, like test/parity_test.rb but with
# no Ruby needed, so it can check the mruby build on a bare machine.
# Usage, from the repo root: ruby/script/parity.sh ruby/build/yara
set -u
binary=$1
scratch=$(mktemp -d)
failures=0
total=0

for example in $(find examples -name '*.yara' | sort); do
  total=$((total + 1))
  name=${example#examples/}
  name=${name%.yara}

  vocabulary=""
  case $example in
    examples/translations/* | *runtime_error_pt.yara) vocabulary="--vocabulary translations/pt.vocab" ;;
  esac

  expected_status=0
  expected_stderr=/dev/null
  case $example in
    examples/errors/*)
      expected_status=1
      expected_stderr=tests/golden/$(basename "$example" .yara).stderr
      ;;
  esac

  # $vocabulary is deliberately unquoted: it is either empty or two words.
  "$binary" run "$example" $vocabulary >"$scratch/stdout" 2>"$scratch/stderr"
  status=$?

  if [ "$status" -ne "$expected_status" ] ||
    ! cmp -s "$scratch/stdout" "tests/stdout/$name.stdout" ||
    ! cmp -s "$scratch/stderr" "$expected_stderr"; then
    failures=$((failures + 1))
    echo "FAIL $example (exit $status, expected $expected_status)"
  fi
done

rm -rf "$scratch"
echo "$total examples, $failures failures"
[ "$failures" -eq 0 ]
