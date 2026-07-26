# Primitive methods: the "everything is an object" surface.
# Every basic value type (Integer, Float, Boolean, String, Array, Pointer)
# has methods (e.g., `xs.size()`, `2.to_s()`, `"3".to_i()`).

# Array methods
xs: IntArray = [10, 20, 30]
print(xs.size())
xs.push(40)
print(xs.get(0))
xs.set(1, 25)
print(xs.get(1))
y: Integer = xs.pop()
print(y)
print(xs.is_empty())
empty_array: IntArray = []
print(empty_array.is_empty())

# String methods
name: Str = "hello"
print(name.size())
print(name.upper())
print(name.lower())
trimmed: Str = "  world  "
print(trimmed.trim())
print(name.is_empty())
empty_str: Str = ""
print(empty_str.is_empty())

# String conversions
num_str: Str = "42"
n: Integer = num_str.to_i()
print(n)
float_str: Str = "3.14"
f: Float = float_str.to_f()
print(f)
print(name.to_s())

# Integer methods
x: Integer = 5
print(x.to_s())
x_as_float: Float = x.to_f()
print(x_as_float)
neg_int: Integer = -7
print(neg_int.abs())

# Float methods
ratio: Float = 2.5
print(ratio.to_s())
int_from_float: Integer = ratio.to_i()
print(int_from_float)
neg_float: Float = -1.5
print(neg_float.abs())

# Boolean methods
flag: Boolean = true
print(flag.to_s())
flag2: Boolean = false
print(flag2.to_s())

# Pointer methods
ptr: Ptr<Integer> = alloc(99)
deref_val: Integer = ptr.deref()
print(deref_val)
ptr.set_deref(111)
deref_val2: Integer = ptr.deref()
print(deref_val2)
ptr.free()
