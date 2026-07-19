# Binary search tree, arena-style: nodes live in parallel arrays
# (values[i], lefts[i], rights[i]); -1 plays the role of a null child.

def insert(values: IntArray, lefts: IntArray, rights: IntArray, node: Int, value: Int): Int
  if node == -1
    push(values, value)
    push(lefts, -1)
    push(rights, -1)
    len(values) - 1
  else
    if value < get(values, node)
      set(lefts, node, insert(values, lefts, rights, get(lefts, node), value))
    else
      set(rights, node, insert(values, lefts, rights, get(rights, node), value))
    end
    node
  end
end

def inorder(values: IntArray, lefts: IntArray, rights: IntArray, node: Int): Nil
  if node != -1
    inorder(values, lefts, rights, get(lefts, node))
    print(get(values, node))
    inorder(values, lefts, rights, get(rights, node))
  end
end

values: IntArray = []
lefts: IntArray = []
rights: IntArray = []
root: Int = -1

root = insert(values, lefts, rights, root, 5)
root = insert(values, lefts, rights, root, 3)
root = insert(values, lefts, rights, root, 8)
root = insert(values, lefts, rights, root, 1)
root = insert(values, lefts, rights, root, 4)

inorder(values, lefts, rights, root)
