# Onda 2: efeitos comuns de máquina — fatias de núcleo

Base: `40a50730`. Escopo confirmado pelo coordenador: fatias 1–3 da seção 7 do artifact
`onda2-efeitos-mundo`, sem cola, consumidor Stirling ou prova em jogo nesta rodada.

## Fatia 1

`WorldEffect::Explosion` conserva dimensão, centro double, força float e flags Java de fogo/quebra.
O host padrão recusa; `Deferred` guarda em `critical`; uma recusa no flush mantém cursor e ordem.
A variante `PileExplosion` continua desserializando saves antigos sem mudar seu comportamento.

Verificado: 583 testes unitários passaram (3 ignorados herdados), clippy `--all-targets -- -D warnings` limpo.
Três mutantes compilados e mortos por asserções: `ExplosionAcknowledgedOnRefusal`, `ExplosionFlagsLost`,
`ExplosionRepeatedOnItemRetry`. Reproduzir, sempre via `agent-run` com target próprio:

```sh
python3 crates/ntm-core/tests/mutate_world_effects.py 1
cargo test --lib
cargo clippy --all-targets -- -D warnings
```

Formatação: o módulo novo passa em `rustfmt --check`. `cargo fmt --all -- --check` falha na base também
(`transaction.rs` original e `examples/explosion_big.rs`, entre outros); os arquivos existentes mantêm o estilo
local para não misturar uma reformatação de todo o núcleo às fatias.

Contexto Java do consumidor futuro (não ativado aqui): `TileEntityStirling.java:90-105` cria explosão de força 5,
sem fogo/quebra e uma `EntityCog`; `EntityCog.java:60-67` devolve um `ModItems.gear_large` com o metadado da entidade
ao interagir e só mata a entidade quando o inventário aceita. Portanto perder a engrenagem continua defeito nominal;
a explosão, sozinha, não conclui o Stirling nem a jogabilidade da família.
