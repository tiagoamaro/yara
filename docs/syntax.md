# Yara Syntax Notes

Living document, updated as the grammar stabilizes during implementation.

## Base types

`Int`/`Integer`, `Float`, `Bool`/`Boolean`, `Str`/`String`, `Nil`. Short/long aliases are equivalent — normalized to one canonical form during lexing/parsing. See `## Methods on primitives` below for method calls on these types.

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

No generic `Array<T>` — each element type has its own concrete annotation name. No array-of-array type yet, so nested collections (e.g. adjacency lists) aren't representable; see `examples/data_structures/graph.yara` for a workaround (edge list instead of adjacency list). Arrays have reference semantics: passing one into a function shares the same backing storage, so mutations (`push`/`set`/`pop`) inside the function are visible to the caller. Array elements also support method-call syntax (e.g. `xs.size()`, `xs.push(4)`) as an alternative to the free-function builtins (`len(xs)`, `push(xs, 4)`); both work and call the same underlying logic.

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

No class-level/static methods other than `.new`, no visibility modifiers — everything is public. Inside a method body, bare names resolve first to locals/params, then to the instance's own fields/consts (implicit `self`, no `self.`/`@` sigil needed) — this is why `count = number` inside `initializer` sets the instance variable rather than creating a local. There is no `self` *expression*, though — a method can't call one of its own class's other methods without a receiver, so intra-class method calls aren't possible yet (see `src/interpreter/CLAUDE.md`). Instance vars declared with no value (`count: Integer`) start out effectively unset until a method assigns them; reading one before that happens is a latent gap (see `src/typechecker/CLAUDE.md`). A class name doubles as its own type annotation (`h: Hello = ...`). Class instances have reference semantics like arrays: assigning `a = b` (both `Hello`) makes `a`/`b` alias the same instance.

User-defined instance methods (defined in a `class` body) are distinct from the parallel registry of methods on primitive types (described in `## Methods on primitives` below). Methods cannot currently be user-defined on primitive types; the primitive-method registry is built into the compiler.

### Inheritance

```
class Animal
  name: String

  def initializer(animal_name: Str)
    name = animal_name
  end

  def speak(): Str
    "..."
  end
end

class Dog < Animal
  breed: String

  def initializer(dog_name: Str, dog_breed: Str)
    name = dog_name    # inherited field — must be assigned here, no `super`
    breed = dog_breed
  end

  def speak(): Str      # overrides Animal.speak
    "Woof"
  end
end
```

Single parent only (`class Child < Parent`), fields and methods inherit, no `super`, no override keyword — a child member with the same name as a parent's implicitly overrides it. Implemented by *flattening*: at class-registration time the parent's fields/methods are merged into the child's class table entry (typechecker `ClassInfo`, interpreter `ClassDecl`), so every other class feature (field access, method dispatch, `.new`) works against the child's table unchanged. Because there's no `super`, the child's `initializer` must assign every inherited non-defaulted field itself (the existing definite-assignment check now walks the parent chain too) — see `examples/objects/inheritance.yara` and the error example `examples/errors/class_inherited_field_unassigned.yara`. Unknown parent names and inheritance cycles (`A < B < A`) are typecheck-time errors.

## Methods on primitives

```
xs: IntArray = [1, 2, 3]
print(xs.size())           # 3
xs.push(4)
print(xs.get(0))           # 1
xs.set(0, 99)
print(xs.pop())            # 4

x: Integer = 5
print(x.to_s())            # "5"
print(x.to_f())            # 5.0

s: String = "  hello  "
print(s.trim().upper())    # "HELLO"
print(s.to_i())            # runtime error: cannot parse "  hello  " as an Integer

p: Ptr<Integer> = alloc(42)
print(p.deref())           # 42
p.set_deref(100)
print(p.deref())           # 100
p.free()
```

Every primitive type — `Array`, `String`, `Integer`, `Float`, `Boolean`, `Pointer` — has an associated set of methods callable with postfix syntax (e.g. `value.method(args)`). Parentheses are always required, even for zero-argument methods (`xs.size()` not `xs.size`).

Method reference (receiver kind → method → arity → return type):
- **Array**: `size()->Integer`, `push(T)->Nil`, `get(Integer)->T`, `set(Integer,T)->Nil`, `pop()->T`, `is_empty()->Boolean`
- **String**: `size()->Integer`, `upper()->String`, `lower()->String`, `trim()->String`, `is_empty()->Boolean`, `to_i()->Integer`, `to_f()->Float`, `to_s()->String`
- **Integer**: `to_s()->String`, `to_f()->Float`, `abs()->Integer`
- **Float**: `to_s()->String`, `to_i()->Integer`, `abs()->Float`
- **Boolean**: `to_s()->String`
- **Pointer**: `deref()->T`, `set_deref(T)->Nil`, `free()->Nil`

Method calls on primitive types are type-checked and dispatched via a parallel registry (`src/methods.rs`), similar to how free-function builtins (`len(xs)`, `push(xs, v)`, ...) work. Both syntaxes coexist: `xs.size()` and `len(xs)` call the same underlying logic.

Errors: an unknown method on a primitive type is a type error `` `{Type}` has no method `{method}` (available: ...) ``. Passing the wrong number of arguments to a method is a type error `` `{Type}#{method}` expects N argument(s), found M ``. String-to-number conversion methods (`to_i()`, `to_f()` on invalid input) are runtime errors: `` cannot parse `{s}` as an Integer `` or `` cannot parse `{s}` as a Float ``.

## Pointers

```
p: Ptr<Integer> = alloc(5)   # allocate a pointer to an integer
print(deref(p))               # dereference: prints 5
set_deref(p, 10)              # update the pointed-to value
print(deref(p))               # prints 10
free(p)                        # deallocate the slot

q: Ptr<Integer> = nil         # nil is assignable to any Ptr<T>
if p != nil                    # can compare pointers to nil with == / !=
  print("p is not nil")
end
```

A pointer is declared with the `Ptr<T>` type annotation — `T` can be any base type or class. Nil is a valid value of any `Ptr<T>` type. Five builtins manage pointers:
- `alloc(value: T) -> Ptr<T>` — allocate a new slot on the heap, initialize it with `value`, return a pointer handle to that slot.
- `deref(pointer: Ptr<T>) -> T` — read the value at the pointed-to slot. If the slot has been freed, this is a runtime error: "use after free". If the pointer is `nil`, this is a runtime error: "nil pointer dereference: `deref` on `nil`".
- `set_deref(pointer: Ptr<T>, value: T) -> Nil` — write `value` to the pointed-to slot. If the slot has been freed, this is a runtime error: "use after free". If the pointer is `nil`, this is a runtime error: "nil pointer dereference: `set_deref` on `nil`".
- `free(pointer: Ptr<T>) -> Nil` — deallocate the slot, marking it as no longer valid. If called twice on the same pointer, this is a runtime error: "double free". If the pointer is `nil`, this is a runtime error: "cannot `free` a nil pointer". A freed slot's backing memory is never reused.
- `collect() -> Integer` — run a mark-and-sweep garbage collection pass over the heap. Roots are all reachable pointers in every live scope, chased recursively through arrays, instances, and pointee slots. Unmarked slots are freed; returns the count of reclaimed slots.

Pointers enable both explicit memory management (via `alloc`/`free`) and automatic collection (via `collect()`) — teaching two fundamental memory-management models. Mistakes in manual management are caught as visible runtime errors, not silent undefined behavior.

## Vocabulary translation

`yara run <file> --vocabulary <path>` (older alias `--keywords <path>` still works) loads a *vocabulary* file and uses it instead of the English defaults across every stage — not just the lexer's 15 reserved words, but type names, builtin functions, primitive methods, and a large majority of error-message prose too. See `translations/pt.vocab` for the bundled Portuguese reference and `examples/translations/hello_pt.yara` for a fully-Portuguese rewrite of the `class` example above (`classe`, `constante`, `funcao`, `fim`, `Inteiro`, `Flutuante`, `escreva`, `.novo`, ...).

A vocabulary file is plain text with `#` line comments and sectioned `canonical = localized` mappings:

```
[keywords]
if = se
class = classe

[types]
Integer = Inteiro

[builtins]
print = escreva

[methods]
to_s = para_texto

[messages]
runtime/division-by-zero = divisao por zero
```

Any name omitted from any section keeps its English spelling/message — a vocabulary file only needs to list what it wants to change (a file translating just `if` is valid). Identifiers and string literal contents are never translatable; not every error message is covered by the `[messages]` catalog yet, so some error prose can still surface in English even under a translated vocabulary — untranslated keys fall back to English rather than erroring.

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
