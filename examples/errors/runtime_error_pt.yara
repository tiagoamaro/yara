# Erro em portugues: divisao por zero, detectado em tempo de execucao.
# Rode com:
#   cargo run -- run examples/errors/runtime_error_pt.yara --vocabulary translations/pt.vocab
n: Inteiro = 0
escreva(1 / n)
