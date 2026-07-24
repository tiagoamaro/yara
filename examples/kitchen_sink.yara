# Kitchen sink: imports every other example so all Yara features run together.
import "hello"
import "types"
import "functions"
import "control_flow"
import "loops"
import "recursion"
import "constants"

# Pointers. Only the examples whose output doesn't depend on the shared heap:
# imports splice into one program, so `gc.yara`/`free_then_collect.yara` would
# collect each other's garbage and print different counts than they do alone.
import "pointers/basic"
import "pointers/leak"
import "pointers/linked_list"
