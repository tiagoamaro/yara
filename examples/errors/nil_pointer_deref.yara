# Pointers are nullable: nil is a valid Ptr<Integer> value.
p: Ptr<Integer> = nil
# But dereferencing nil is a runtime error, caught and named — not UB.
print(deref(p))
