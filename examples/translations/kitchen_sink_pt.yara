# Kitchen sink in Portuguese: every translatable Yara language feature in a
# single program, all spelled with the bundled `translations/pt.vocab`.
#
# Run with:
#   cargo run -- run examples/translations/kitchen_sink_pt.yara \
#     --vocabulary translations/pt.vocab
# ...or just `cargo test` — every example under examples/translations/ is
# collected and run with the pt Vocabulary automatically.
#
# Two deliberate exceptions to full translation:
#   - `initializer` is a fixed constructor-method name, not a vocab entry, so
#     it stays English even here.
#   - `import` paths are filesystem paths — identifiers by design, not localized.
#
# Together with the imported fatorial_pt.yara this exercises all 15 keywords,
# every type name, the builtin + method registries, classes with inheritance,
# and the pointer / garbage-collection surface.

# --- Import: `importar`; path is relative to this file's own directory.
#     fatorial_pt defines a recursive function we call below. ---
importar "fatorial_pt"

# --- Const + primitives: Inteiro / Flutuante / Booleano / Texto / Nulo ---
constante PI: Flutuante = 3.14159
inteiro: Inteiro = 10
flutuante: Flutuante = 3.14
ativa: Booleano = verdadeiro
inativa: Booleano = falso
nome: Texto = "Yara"
nada: Nulo = nulo
escreva(inteiro)
escreva(flutuante)
escreva(ativa)
escreva(nome)
escreva(PI)

# --- Control flow: if / elsif / else = `se` / `senaose` / `senao`,
#     used as a function's tail expression ---
funcao classificar(n: Inteiro): Texto
  se n > 0
    "positivo"
  senaose n < 0
    "negativo"
  senao
    "zero"
  fim
fim
escreva(classificar(5))
escreva(classificar(-3))
escreva(classificar(0))

# --- Functions: explicit `retorne`, plus recursion via the imported fatorial ---
funcao com_retorne(x: Inteiro): Inteiro
  se x > 100
    retorne 999
  fim
  x * 2
fim
escreva(com_retorne(1))
escreva(com_retorne(500))
escreva(fatorial(5))
escreva(fatorial(6))

# --- Loops: `for` = `para ... em`, `while` = `enquanto` ---
total: Inteiro = 0
para i em 0..10
  total = total + i
fim
escreva(total)
count: Inteiro = 3
enquanto count > 0
  escreva(count)
  count = count - 1
fim

# --- Arrays: literal + `len` (tamanho) + index, then every array method ---
xs: ArrayDeInteiros = [10, 20, 30]
escreva(tamanho(xs))
escreva(xs[0])
escreva(xs.tamanho())
xs.empilhar(40)
escreva(xs.obter(3))
xs.definir(1, 25)
escreva(xs.obter(1))
y: Inteiro = xs.desempilhar()
escreva(y)
escreva(xs.esta_vazio())
vazio: ArrayDeInteiros = []
escreva(vazio.esta_vazio())

# --- Strings: every string method + numeric conversions ---
escreva(nome.tamanho())
escreva(nome.maiusculas())
escreva(nome.minusculas())
com_espaco: Texto = "  mundo  "
escreva(com_espaco.aparar())
escreva(nome.esta_vazio())
num_str: Texto = "42"
n: Inteiro = num_str.para_inteiro()
escreva(n)
f: Flutuante = "3.5".para_flutuante()
escreva(f)
escreva("7".para_inteiro().para_texto())

# --- Integer / Float / Boolean methods ---
x: Inteiro = 5
escreva(x.para_texto())
escreva(x.para_flutuante())
neg: Inteiro = -7
escreva(neg.absoluto())
r: Flutuante = -2.5
escreva(r.absoluto())
escreva(r.para_inteiro())
escreva(ativa.para_texto())
escreva(inativa.para_texto())

# --- Pointers: alocar / desreferenciar / definir_desreferenciado / liberar,
#     as both free builtins and methods on Ponteiro<T>. `p` is freed here so
#     its slot is absent from the sweep below. ---
p: Ponteiro<Inteiro> = alocar(99)
escreva(desreferenciar(p))
definir_desreferenciado(p, 111)
escreva(p.desreferenciar())
p.liberar()

# --- Garbage collection: `coletar` reclaims unreachable allocations. The three
#     slots allocated inside vazamentos(3) become unreachable when the call's
#     scope pops, so coletar() reclaims exactly those three and leaves the
#     still-reachable `mantido` slot intact. ---
funcao vazamentos(n: Inteiro)
  i: Inteiro = 0
  enquanto i < n
    s: Ponteiro<Inteiro> = alocar(i)
    i = i + 1
  fim
fim
vazamentos(3)
mantido: Ponteiro<Inteiro> = alocar(7)
liberadas: Inteiro = coletar()
escreva(liberadas)
escreva(desreferenciar(mantido))

# --- Classes: `classe`, class const, instance field, method, `.novo` ---
classe Circulo
  constante PI_C: Flutuante = 3.14159
  raio: Flutuante
  funcao initializer(r: Flutuante)
    raio = r
  fim
  funcao area(): Flutuante
    PI_C * raio * raio
  fim
fim
c: Circulo = Circulo.novo(2.0)
escreva(c.raio)
c.raio = 3.0
escreva(c.raio)
escreva(c.area())

# --- Inheritance: `class Child < Parent`; fields + methods inherited, and a
#     same-name child member implicitly overrides the parent's ---
classe Animal
  nome: Texto
  funcao initializer(an: Texto)
    nome = an
  fim
  funcao falar(): Texto
    "..."
  fim
fim
classe Cao < Animal
  raca: Texto
  funcao initializer(an: Texto, rr: Texto)
    nome = an
    raca = rr
  fim
  funcao falar(): Texto
    "Au!"
  fim
fim
cao: Cao = Cao.novo("Rex", "Labrador")
escreva(cao.nome)
escreva(cao.raca)
escreva(cao.falar())