# Mesmo programa que examples/objects/hello.yara, agora com vocabulario
# completo em portugues (palavras-chave, tipos, e builtins/metodos) —
# veja translations/pt.vocab. Rode com:
#   cargo run -- run examples/translations/hello_pt.yara --vocabulary translations/pt.vocab
classe Ola
  constante PI: Flutuante = 3.14159 # constante no escopo da classe
  contagem: Inteiro                 # variavel de instancia

  funcao initializer(numero: Inteiro)
    contagem = numero
  fim

  funcao area(raio: Flutuante): Flutuante
    PI * raio * raio
  fim
fim

h: Ola = Ola.novo(5)
escreva(h.contagem)

h.contagem = 10
escreva(h.contagem)

escreva(h.PI)
escreva(h.area(2.0))
