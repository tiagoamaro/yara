# Basic pointer operations: alloc, deref, set_deref, free

# Allocate memory for an integer value
p: Ptr<Integer> = alloc(5)
print("After alloc(5):")
print(deref(p))

# Modify the value at the pointer
set_deref(p, 10)
print("After set_deref(p, 10):")
print(deref(p))

# Modify again
set_deref(p, 42)
print("After set_deref(p, 42):")
print(deref(p))

# Free the memory
free(p)
print("Memory freed")
