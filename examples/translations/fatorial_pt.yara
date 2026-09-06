# Recursive factorial, fully in Portuguese vocabulary (`funcao`, `fim`, `se`,
# `senao`). Imported by kitchen_sink_pt.yara to exercise `import`/`importar`
# under a translated Vocabulary — no other PT example demonstrates it. Also
# runs clean standalone: a pure function definition, no top-level side effects.
funcao fatorial(n: Inteiro): Inteiro
  se n < 2
    1
  senao
    n * fatorial(n - 1)
  fim
fim