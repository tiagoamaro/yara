# Singly linked list built from real pointers: each node owns a Ptr<Node>
# to the next node, and `nil` marks the end of the list — compare with
# examples/data_structures/linked_list.yara, which fakes pointers with
# parallel arrays and integer indices (arena style).

class Node
  value: Integer
  next: Ptr<Node>

  def initializer(v: Integer)
    value = v
    next = nil
  end
end

# Append by walking to the last node (the one whose next is nil) and
# pointing its next at a freshly allocated node.
def append(head: Ptr<Node>, v: Integer)
  p: Ptr<Node> = head
  n: Node = deref(p)
  while n.next != nil
    p = n.next
    n = deref(p)
  end
  n.next = alloc(Node.new(v))
end

def sum_list(head: Ptr<Node>): Integer
  total: Integer = 0
  p: Ptr<Node> = head
  while p != nil
    n: Node = deref(p)
    total = total + n.value
    p = n.next
  end
  total
end

def print_list(head: Ptr<Node>)
  p: Ptr<Node> = head
  while p != nil
    n: Node = deref(p)
    print(n.value)
    p = n.next
  end
end

head: Ptr<Node> = alloc(Node.new(10))
append(head, 20)
append(head, 30)

print("List contents:")
print_list(head)
print("Sum:")
print(sum_list(head))
