# Runtime error: freeing the same pointer twice
p: Ptr<Integer> = alloc(5)
free(p)
free(p)
