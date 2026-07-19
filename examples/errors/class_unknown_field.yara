# Type error: accessing a field the class never declared.
class Hello
  count: Integer

  def initializer(number: Int)
    count = number
  end
end

h = Hello.new(5)
print(h.missing)
