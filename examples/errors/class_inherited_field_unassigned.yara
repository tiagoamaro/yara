# A child class must assign every inherited field itself in its own
# `initializer` — there is no `super`, so the parent's field is never
# assigned unless the child does it.
class Animal
  name: String

  def initializer(animal_name: Str)
    name = animal_name
  end
end

class Dog < Animal
  breed: String

  def initializer(dog_breed: Str)
    breed = dog_breed
  end
end

d: Dog = Dog.new("Labrador")
print(d.breed)
