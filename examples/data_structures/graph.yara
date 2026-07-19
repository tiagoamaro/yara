# Graph as an edge list: two parallel arrays hold each edge's endpoints.
# No adjacency-list nesting (Yara has no array-of-array type yet), so
# adjacency is answered by scanning the edge list.

def add_edge(from_nodes: IntArray, to_nodes: IntArray, a: Int, b: Int): Nil
  push(from_nodes, a)
  push(to_nodes, b)
end

def is_adjacent(from_nodes: IntArray, to_nodes: IntArray, a: Int, b: Int): Boolean
  found = false
  for i in 0..len(from_nodes)
    if get(from_nodes, i) == a
      if get(to_nodes, i) == b
        found = true
      end
    end
  end
  found
end

from_nodes: IntArray = []
to_nodes: IntArray = []

add_edge(from_nodes, to_nodes, 1, 2)
add_edge(from_nodes, to_nodes, 2, 3)
add_edge(from_nodes, to_nodes, 1, 3)

print(is_adjacent(from_nodes, to_nodes, 1, 2))
print(is_adjacent(from_nodes, to_nodes, 2, 1))
print(is_adjacent(from_nodes, to_nodes, 1, 3))
