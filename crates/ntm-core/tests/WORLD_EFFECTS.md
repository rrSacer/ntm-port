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
(`1248-1252`) só consulta `ledger_knows`. O mutante `HostWithoutLoadedMarker` reproduz o caminho de `Items` em destino carregado e deve
ser morto pelo verificador: replay sobre a saída já salva (dano repetido, mesmo se o drop recriado disfarçar a contagem)
ou duplicação de saída sem novo dano. O oráculo verifica também esses dois casos, além da perda do item marcado.

Marcar e testar os IDs nos destinos carregados para `Items`, `EncodedItems` e `Entities` é **requisito bloqueante da fatia 4**,
antes de qualquer consumidor de explosão + drop. Conferir também o dreno de dívidas em `ledger_tick`: `take_owed`
e `take_owed_entities` marcam o registro da origem que devia, não necessariamente o chunk de destino que recebe.
O modelo não simula o débito e sua drenagem: `DestinationOut` apenas recusa o efeito. `EncodedItems`, `Entities` e o caminho de dívida e drenagem precisam de provas próprias na fatia 4. Esta rodada não altera a cola nem valida o algoritmo nativo de explosão em jogo. A fatia 2 é infraestrutura sob contrato.

## Fatia 3: efeitos do passo e segunda porta

`Logic::world_effects` drena saídas runtime depois do `tick`, dentro do `Deferred`; o commit confirmado publica
modelo e fila crítica juntos. Sem consumidor de produção nesta rodada: o `String` inerte só tem uma sonda em
`cfg(test)` que consome um estado armado e produz uma explosão e um drop.

A convenção aprovada dos offsets é `[direita, cima, frente]`. Frente vem de `facing`, padrão `north`; direita é
leste quando a frente é norte. Drop parte da célula do núcleo. Explosão parte de `[x+0.5,y,z+0.5]`: offset
`[0,1,0]` dá literalmente `[x+0.5,y+1,z+0.5]` do Java. Testes cobrem north/default e east e outra máquina em outro chunk.

### Segunda porta: confirmação explícita da fase

O coordenador aprovou alterar a proposta inicial de Vec direto para:

```rust
server.publish_machine_effects(host, &key, |candidate| {
    // Só altera a cópia candidate e retorna as saídas locais do passo.
    Vec::<ntm_core::machine::MachineEffect>::new()
});
```

Ela recusa origem descarregada, removida, substituída ou de geração antiga, roda a fase numa cópia e confirma
`MachineCommit` completo (slots, modelo e dados). Só com aceitação troca o estado vivo, registra o sobrevivente e
põe a batelada em `machine_effects`, com `owners=[key]` e origem na geração observada. Recusa não altera nada e
permite repetir. A chamada fica no mesmo tick da fase, antes de `native_save`/`sync_obligations`.

O Vec direto não bastava: o finish futuro do Stirling vem depois de `local_steps`, logo `native_save` seria o primeiro
commit de `has_cog=false` e poderia recusar, com a obrigação já publicada. A closure torna esse primeiro commit parte
da porta; depois dele uma recusa de `native_save` é redundante e não pode deixar a batelada com o BE anterior.
O consumidor Stirling permanece desativado; mover seu finish para a closure cabe à fatia 5.

Oráculos preparados: recusa do passo comum sem estado/batelada, aceitação e uma única batelada na casa do núcleo;
recusa do primeiro commit pós-fase, repetição aceita, recusa posterior de save; captura entre este tick e o próximo,
duas reaberturas com a explosão recusada e uma aplicação quando aceita. Mutantes da fatia 3:
`PublishBeforeCommit`, `CommitWithoutBatch`, `PostPhaseWithoutCommit`, `PublishNextTick` (fila real drenada só no tick
seguinte). Reproduzir com `python3 crates/ntm-core/tests/mutate_world_effects.py 3` via `agent-run`.

As provas finais e números são enviados ao coordenador após rodar; ter código e oráculos preparados não é conclusão.
O bloqueio da marca de destino da fatia 4 continua valendo para consumidores que combinarem explosão e item/entidade.
