# A simple class: instance vars, a class constant, and an initializer.
class Hello
  const PI: Float = 3.14159 # constant within class scope
  count: Integer            # instance variable

  def initializer(number: Int)
    count = number
  end

  def area(radius: Float): Float
    PI * radius * radius
  end
end

h: Hello = Hello.new(5)
print(h.count)

h.count = 10
print(h.count)

print(h.PI)
print(h.area(2.0))
