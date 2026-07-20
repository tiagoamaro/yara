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

## Classes

```
class Hello
  const PI: Float = 3.14159 # constant within class scope
  count: Integer             # instance variable, no default value

  def initializer(number: Int)
    count = number
  end

  def area(radius: Float): Float
    PI * radius * radius
  end
end

h: Hello = Hello.new(5)   # construction; calls initializer
print(h.count)             # field read
h.count = 10               # field write
print(h.area(2.0))         # method call
```

No inheritance, no class-level/static methods other than `.new`, no visibility modifiers — everything is public. Inside a method body, bare names resolve first to locals/params, then to the instance's own fields/consts (implicit `self`, no `self.`/`@` sigil needed) — this is why `count = number` inside `initializer` sets the instance variable rather than creating a local. Instance vars declared with no value (`count: Integer`) start out effectively unset until a method assigns them; reading one before that happens is a latent gap (see `src/typechecker/CLAUDE.md`). A class name doubles as its own type annotation (`h: Hello = ...`). Class instances have reference semantics like arrays: assigning `a = b` (both `Hello`) makes `a`/`b` alias the same instance.

## Keyword translation

`yara run <file> --keywords <path>` recognizes translated reserved-word spellings instead of the English defaults, e.g. `translations/pt.keywords` maps `if -> se`, `class -> classe`, `def -> funcao`, `end -> fim`, and so on. See `examples/translations/hello_pt.yara` for the same `class` example from above, rewritten in Portuguese. Only the fixed set of reserved words is translatable — type names (`Int`/`Integer`/...), identifiers, string contents, and error messages always stay in English. A translation file only needs to list the keywords it wants to change; anything omitted keeps its English spelling.

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
