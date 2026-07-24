# Runtime error: dereferencing after free
p: Ptr<Integer> = alloc(5)
free(p)
print(deref(p))
