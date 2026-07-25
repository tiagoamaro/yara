# Single-parent inheritance: `class Child < Parent`. Fields and methods are
# inherited; no `super`, so the child initializer must assign every
# inherited field itself. Child members with the same name override the
# parent's (implicit override, no keyword).

class Animal
  name: String

  def initializer(animal_name: Str)
    name = animal_name
  end

  def speak(): Str
    "..."
  end
end

class Dog < Animal
  breed: String

  def initializer(dog_name: Str, dog_breed: Str)
    name = dog_name
    breed = dog_breed
  end

  def speak(): Str
    "Woof"
  end
end

d: Dog = Dog.new("Rex", "Labrador")
print(d.name)
print(d.breed)
print(d.speak())
