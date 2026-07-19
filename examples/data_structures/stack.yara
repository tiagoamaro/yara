# LIFO stack built on an IntArray using push/pop.
stack: IntArray = []
push(stack, 1)
push(stack, 2)
push(stack, 3)

print(len(stack))
print(pop(stack))
print(pop(stack))
print(len(stack))
