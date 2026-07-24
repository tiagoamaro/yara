# Memory leak: allocating without freeing
# In manual memory mode, this is allowed — the language doesn't force cleanup.
# See gc.yara for the collected alternative: collect() reclaims exactly this
# kind of forgotten allocation.

i: Integer = 0
while i < 3
  # Allocate memory, then immediately discard the pointer.
  # The allocated memory will never be freed.
  p: Ptr<Integer> = alloc(i)
  print("Leaked allocation:")
  print(i)
  i = i + 1
end

print("Loop complete; memory leaked in manual mode")
