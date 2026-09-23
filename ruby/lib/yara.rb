# Load-order manifest: the only file allowed to `require`. mruby has no
# `require`, so the mruby build compiles these same files in this same order.
require_relative "yara/ast"
require_relative "yara/diagnostics"
require_relative "yara/environment"
require_relative "yara/cli"
