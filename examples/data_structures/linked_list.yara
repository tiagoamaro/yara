# Singly linked list, arena-style: since Yara has no pointers/records yet,
# nodes live in two parallel arrays (values[i], nexts[i]), and a node's
# "address" is just its index. -1 plays the role of a null next-pointer.

def prepend(values: IntArray, nexts: IntArray, head: Int, value: Int): Int
  push(values, value)
  push(nexts, head)
  len(values) - 1
end

def print_list(values: IntArray, nexts: IntArray, head: Int): Nil
  current = head
  while current != -1
    print(get(values, current))
    current = get(nexts, current)
  end
end

values: IntArray = []
nexts: IntArray = []
head: Int = -1

head = prepend(values, nexts, head, 30)
head = prepend(values, nexts, head, 20)
head = prepend(values, nexts, head, 10)

print_list(values, nexts, head)
