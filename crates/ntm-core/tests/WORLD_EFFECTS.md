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

## Fatia 2

No replay, um efeito posterior `Items`, `EncodedItems` ou `Entities` já visto no chunk de destino prova a aplicação
ordenada dos anteriores. O cursor avança sem nova aplicação/recibo. Não se aplica a bateladas novas nem a IDs alheios.
O teste inclui cursor inicial 3, dois efeitos anteriores e as três variantes de destino.

Correção correlata autorizada pelo coordenador: `native_obligations_loaded` rastreia uma casa que chega com dívida,
mesmo sem marca de migração. Sem isso, pagar tudo no primeiro `native_settle` deixa a casa fora de `sync_obligations`
e o anexo antigo continua no chunk. A sonda exige uma gravação vazia e uma segunda reabertura sem nova explosão.

Extensão `common/world_effects_order_model.rs` do `save_model`: dois registros independentes, captura nos dois sentidos,
writer FIFO com coalescência, morte, recusa e destino descarregado. Cada tick chama o `Server` real; o controle do
recorte R3 é a saída vanilla já salva (um item). O verificador anterior continua cobrindo conservação vanilla geral;
a extensão prova especificamente que o replay não destrói essa saída. Não pretende modelar o algoritmo de dano nativo.
Na primeira rodada, profundidade 9: 17.544 estados distintos, 22.053.400 sequências, zero violações. O padrão da extensão
foi elevado a 10 para acompanhar o máximo da Onda 0. Reproduzir:

```sh
cargo test --release --test save_model saved_outputs_survive_an_older_homes_explosion_replay -- --nocapture
python3 crates/ntm-core/tests/mutate_world_effects.py 2
```

A entrega só é pronta após finalizar o mutante `ReplayWithoutOrderGuard` nos dois oráculos, o mutante
`LoadedHomeNotTracked`, os testes finais e clippy; os números finais são enviados com o hash ao coordenador.

### Limite bloqueante da integração (revisão P1 do coordenador)

**R3 não é seguro no host nativo atual.** O contrato do núcleo exige que o destino que publica itens ou entidades
registre o ID de efeito, junto de seu chunk; o host sintético do verificador cumpre esse contrato. A cola atual não:
`glue.rs:620-623` e `648-650` fazem spawn direto em destino carregado sem marcar o ID, enquanto `effect_seen`
(`1248-1252`) só consulta `ledger_knows`. O mutante `HostWithoutLoadedMarker` reproduz essa implementação e deve
ser morto pelo verificador: replay sobre a saída já salva (dano repetido, mesmo se o drop recriado disfarçar a contagem)
ou duplicação de saída sem novo dano. O oráculo verifica também esses dois casos, além da perda do item marcado.

Marcar os IDs nos destinos carregados para `Items`, `EncodedItems` e `Entities` é **requisito bloqueante da fatia 4**,
antes de qualquer consumidor de explosão + drop. Conferir também o dreno de dívidas em `ledger_tick`: `take_owed`
e `take_owed_entities` marcam o registro da origem que devia, não necessariamente o chunk de destino que recebe.
Esta rodada não altera a cola nem valida o algoritmo nativo de explosão em jogo. A fatia 2 é infraestrutura sob contrato.
