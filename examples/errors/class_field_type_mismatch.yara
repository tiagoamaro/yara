# Type error: assigning a String to an Integer field.
class Hello
  count: Integer

  def initializer(number: Int)
    count = number
  end
end

h = Hello.new(5)
h.count = "oops"
