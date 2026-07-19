# FIFO queue: enqueue appends, dequeue reads from a moving front index
# (no built-in "shift", so the front position is tracked by hand).
items: IntArray = []
front: Int = 0

push(items, 1)
push(items, 2)
push(items, 3)

print(items[front])
front = front + 1
print(items[front])
front = front + 1
print(len(items) - front)
