# Type error: initializer takes one argument, call passes two.
class Hello
  count: Integer

  def initializer(number: Int)
    count = number
  end
end

h = Hello.new(5, 6)
