# Mesmo programa que examples/objects/hello.yara, com palavras-chave em
# portugues (veja translations/pt.keywords). Rode com:
#   cargo run -- run examples/translations/hello_pt.yara --keywords translations/pt.keywords
classe Ola
  constante PI: Float = 3.14159
  count: Integer

  funcao initializer(number: Int)
    count = number
  fim

  funcao area(radius: Float): Float
    PI * radius * radius
  fim
fim

h: Ola = Ola.new(5)
print(h.count)

h.count = 10
print(h.count)

print(h.PI)
print(h.area(2.0))
