# lib/yara/typechecker/

Static checking of a resolved program. `../typechecker.rb` holds `Type`, `TypeError` and `TypeChecker` (`check_program`, `resolve_type`, `type_error`); `expressions.rb`, `statements.rb`, `calls.rb`, `classes.rb` and `methods.rb` reopen `TypeChecker`.

## Design
- `Type` is a `Data` with `kind` (`Integer`, `Float`, `Boolean`, `String`, `Nil`, `Array`, `Pointer`, `Instance`) and `inner` (element type, or class name for instances). `accepts?` is assignability: equality, plus the empty-array literal (`Array<Nil>`) into any array and `nil` into any pointer.
- `check_program` runs, in order: `collect_classes` (register every class name, fill each class's own fields and method signatures, then `flatten_inheritance`), `collect_function_signatures`, `check_classes` (every method body), then each top-level statement. Collecting first means nothing has to be declared before use, and a class may name itself or a later class in an annotation.
- `resolve_type` resolves a class name to an instance type first, then `Ptr<T>` recursively (so `Ptr<Node>` works), then primitives and the array annotations (`IntArray`, `FloatArray`, `BoolArray`, `StringArray`). Anything else is "unknown type"; annotations are only checked here, not by the parser.
- Inheritance is flattened: parents' fields and methods merge into each child, parents first, and a child's own member wins. Unknown parents and cycles are errors at the child's `class`; a cycle names the first of its classes reached in declaration order.
- Method bodies are checked with the class's fields pre-declared in the scope (implicit `self`), then the parameters, which can shadow fields.
- Every instance variable, own or inherited, must be assigned somewhere in `initializer` (`check_fields_assigned`). Flow-insensitive: an assignment inside an `if` counts.
- A declaration with an annotation checks the value against it and stores the annotated type, which is how `xs: IntArray = []` gets its element type.
- No implicit numeric coercion. Arithmetic and ordering need both sides the same numeric type; `String + String` concatenates; `==`/`!=` need matching types, except a pointer compared with `nil`. Conditions must be `Boolean`, `for` bounds `Integer`.
- Implicit return: `check_body_return_type` types a body by its last statement, and a trailing `if` with an `else` is a tail expression whose branches must agree (`check_tail`). Without an `else` the type is unknown and the declared return type goes unchecked.
- Free calls (`check_call`): `print` takes anything, then registry builtins (`check_builtin_<name>`), then user functions. `check_arguments` checks arity and exact argument types for functions, methods and `.new`.
- `check_method_call`: a bare identifier that names a class and no variable is construction (only `new`, possibly localized, is allowed); an instance uses its class's flattened methods; any other receiver with a kind uses the primitive methods (`methods.rb`: `FIXED_RESULTS` for methods whose result depends only on the receiver, `check_<kind>_<name>` for the rest).
- Messages come from the catalog (`type/...` keys), with types spelled through the vocabulary (`name_of`).

## Gotchas
- There is no array-of-array annotation, so nested collections cannot be typed; `examples/data_structures/graph.yara` uses parallel `IntArray`s instead.
- `push`/`set` as free builtins need the value's type to equal the element type exactly, while `Array#push`/`#set` use `accepts?`.
