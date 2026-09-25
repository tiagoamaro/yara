# Compiled last, after every file in `lib/yara.rb`'s load order; the launcher
# reads the exit status back from this global.
$yara_status = Yara::CLI.run(ARGV)
