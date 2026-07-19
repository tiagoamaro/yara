# Runtime error with a call stack: countdown recurses until it divides by
# zero, and the error shows every call frame that led there.
def countdown(n: Int): Int
  if n <= 0
    1 / n
  else
    countdown(n - 1)
  end
end

countdown(3)
