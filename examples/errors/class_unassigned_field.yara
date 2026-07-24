class Counter
  count: Integer

  def bump()
    count = count + 1
  end
end

c: Counter = Counter.new()
print(c.count)
