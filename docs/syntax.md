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

## Unary operators

`-x` (negation, `Integer`/`Float` only). No `!`/`not` yet.

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
