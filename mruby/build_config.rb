# The mruby that the standalone `yara` executable links against.
MRuby::Build.new do |conf|
  conf.toolchain
  conf.gembox "default"
  # Without it mruby strings are byte arrays and columns in diagnostics for
  # non-ASCII source would count bytes.
  conf.cc.defines << "MRB_UTF8_STRING"
end
