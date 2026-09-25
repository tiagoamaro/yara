module Yara
  # The fixed-arity builtins. The typechecker implements `check_builtin_<name>`
  # and the interpreter `eval_builtin_<name>` for each; `print` is variadic and
  # handled separately by both.
  module Builtins
    ARITIES = {
      "len" => 1, "push" => 2, "get" => 2, "set" => 3, "pop" => 1,
      "alloc" => 1, "deref" => 1, "set_deref" => 2, "free" => 1, "collect" => 0
    }.freeze
  end
end
