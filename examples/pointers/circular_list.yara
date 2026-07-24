# Circular linked list: the tail's next points back at the head, so no node
# ever holds a nil next — the classic structure for round-robin scheduling.
# Walking it needs a step counter (or a start-pointer comparison), because
# a `while p != nil` walk would never terminate.

class Node
  value: Integer
  next: Ptr<Node>

  def initializer(v: Integer)
    value = v
    next = nil
  end
end

# Build a three-node ring: 1 -> 2 -> 3 -> back to 1.
head: Ptr<Node> = alloc(Node.new(1))
second: Ptr<Node> = alloc(Node.new(2))
third: Ptr<Node> = alloc(Node.new(3))

n1: Node = deref(head)
n1.next = second
n2: Node = deref(second)
n2.next = third
n3: Node = deref(third)
n3.next = head

# Walk seven steps around the ring — the values cycle 1 2 3 1 2 3 1.
print("Seven steps around the ring:")
p: Ptr<Node> = head
steps: Integer = 0
while steps < 7
  n: Node = deref(p)
  print(n.value)
  p = n.next
  steps = steps + 1
end
