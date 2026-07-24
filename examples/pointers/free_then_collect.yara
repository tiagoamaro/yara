# Manual `free` and `collect()` in the same program: how the two interact.
# Rule of thumb: freeing by hand is always safe to mix with the collector —
# the sweep only reclaims slots that are still live *and* unreachable, so a
# slot you already freed is never counted twice.

# Every allocation here becomes garbage as soon as the call returns:
# the call's scope pops at `end`, so nothing can reach these slots again.
def make_garbage(n: Integer)
  i: Integer = 0
  while i < n
    g: Ptr<Integer> = alloc(i)
    i = i + 1
  end
end

kept: Ptr<Integer> = alloc(7)
by_hand: Ptr<Integer> = alloc(8)

# Free one slot manually. `by_hand` still names the slot, but the slot is empty.
free(by_hand)
print("Freed one slot by hand")

# Now make two slots unreachable without freeing them.
make_garbage(2)

# collect() reclaims the two leaks only: the hand-freed slot is already empty
# and does not inflate the count, and `kept` is still reachable.
first: Integer = collect()
print("collect() reclaimed (expect 2):")
print(first)
print("Reachable allocation survived:")
print(deref(kept))

# Free the survivor by hand, then collect again: nothing left to reclaim.
free(kept)
second: Integer = collect()
print("collect() after freeing everything (expect 0):")
print(second)
