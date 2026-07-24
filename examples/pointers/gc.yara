# Garbage collection: collect() reclaims allocations you forgot to free.
# Side by side with the manual style (basic.yara) and the leak it fixes (leak.yara).

# Allocations made here become unreachable as soon as the call returns:
# each loop iteration rebinds p, and the call's scope pops at `end`.
def leak_some(n: Integer)
  i: Integer = 0
  while i < n
    p: Ptr<Integer> = alloc(i)
    i = i + 1
  end
end

# Manual style: you must free exactly what you allocate.
m: Ptr<Integer> = alloc(1)
free(m)
print("Manual: allocated one slot, freed it ourselves")

# GC style: allocate, forget, and let the collector find the garbage.
leak_some(3)
kept: Ptr<Integer> = alloc(99)
freed: Integer = collect()
print("GC: slots reclaimed by collect():")
print(freed)
print("Still-reachable allocation survives the sweep:")
print(deref(kept))
