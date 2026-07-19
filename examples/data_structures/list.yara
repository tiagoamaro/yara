# A dynamic list: push, index, len, and iteration over an IntArray.
xs: IntArray = [10, 20, 30]
push(xs, 40)

print(len(xs))

for i in 0..len(xs)
  print(xs[i])
end

set(xs, 0, 99)
print(xs)
