# Yara Syntax Notes

Living document, updated as the grammar stabilizes during implementation.

## Base types

`Int`/`Integer`, `Float`, `Bool`/`Boolean`, `Str`/`String`, `Nil`. Short/long aliases are equivalent — normalized to one canonical form during lexing/parsing.

## Declarations

```
x = 5            # inferred type
y: Float = 5.0   # explicit type annotation
const PI: Float = 3.14
```

## Functions

```
def add(a: Int, b: Int): Int
  a + b
end
```

Last expression in a function body is its return value (Ruby-style implicit return); explicit `return` also supported.

## Control flow

```
if x > 0
  print("positive")
elsif x < 0
  print("negative")
else
  print("zero")
end

while x > 0
  x = x - 1
end

for i in 0..10
  print(i)
end
```

## Imports

```
import "helper"       # resolves to helper.yara, relative to this file's directory
```

Splices the imported file's top-level declarations into this file at typecheck/interpret time. No namespacing yet — everything lands in one flat global scope.

## Unary operators

`-x` (negation, `Integer`/`Float` only). No `!`/`not` yet.

## Arrays

```
xs: IntArray = [1, 2, 3]   # IntArray, FloatArray, BoolArray, StringArray
push(xs, 4)
print(xs[0])               # indexing
print(len(xs))
set(xs, 0, 99)
print(pop(xs))              # removes and returns the last element
```

No generic `Array<T>` — each element type has its own concrete annotation name. No array-of-array type yet, so nested collections (e.g. adjacency lists) aren't representable; see `examples/data_structures/graph.yara` for a workaround (edge list instead of adjacency list). Arrays have reference semantics: passing one into a function shares the same backing storage, so mutations (`push`/`set`/`pop`) inside the function are visible to the caller.

## Comments

`# line comment`

## Error format (target)

```
error: type mismatch
  --> examples/foo.yara:3:7
  |
3 |   x: Int = "hello"
  |            ^^^^^^^ expected Int, found String
```
