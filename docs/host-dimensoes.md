# Dimensões e block entities no host nativo

## O defeito (S1, pré-existente, global)

No `ServerTickEnd` o motor tem em lugar o **último nível que ticou** (ordem Overworld, Nether, End; só os que têm jogador, chunk forçado, ticket de portal, cama ou pregen). O `Host::entity` do glue devolvia `None` quando a dimensão da máquina não era essa, e tudo que passa por ele virava um no-op silencioso: `save_machine`, `machine_slots`, `set_machine_slot`, `machine_data`, `set_machine_data`. Pior, `native_save` marcava o estado como gravado de qualquer jeito.

Aconteceu com: jogador no Nether, chunk forçado no Nether (fazendas), ticket de portal, jogadores em dimensões diferentes em LAN/dedicado.

Consequências: máquina **nova** nunca recebia `STATE` no block entity (bloco morto depois de reabrir o mundo); máquina antiga perdia o estado final; slots paravam de reagir.

## A correção

1. **Acesso preguiçoso à dimensão pedida** (`native/ntm/mixins/ntm/glue.rs`):
   - `Host::entity` lê/escreve o block entity no `World` da dimensão pedida, diretamente, quando ela está estacionada (`levels.parked_world_mut`), e no nível em lugar quando é a dele. Não troca de nível, então não zera a ociosidade nem limpa partículas. Com isso `close_idle_levels` continua fechando dimensões sem jogador e sem chunk forçado, mesmo quando têm máquinas e leitores de redstone (teste em jogo: `native_dims.py idle`). Nunca abre um nível: uma dimensão fechada não tem máquinas carregadas. O chunk é marcado `modified` como o `setChanged` faria.
   - O que só o nível em lugar sabe fazer (largar item, explodir, avisar clientes do block entity, partículas e sons) usa `in_place(game, dim, f)`, que é o `Game::visit_level` do motor: troca de nível só quando a dimensão pedida não é a em lugar e só para essa operação, com guarda `Drop` que devolve o nível original também em pânico, sem `static` e sem deixar rastro (ociosidade, partículas). A sincronização com clientes e as partículas só ocorrem se há jogador na dimensão.
2. **Cinto de segurança** (`ntm-core`): `ServerHost::save_machine` devolve `bool`; `native_save` só marca `native_saved` quando gravou. Um block entity inalcançável é regravado depois (teste `an_unreachable_block_entity_is_written_again_later`). `set_machine_slot` e `set_machine_data` já devolviam `bool`.
3. **Pile**: o passo da pile não precisa do nível em lugar e não entra em nenhum. `ServerHost::pile_reachable(dim)` decide se a dimensão está aberta (fechada: congela); `pile_drop` e `pile_explode` usam `in_place` e só trocam de nível quando há algo a ejetar ou explodir. `pile_enter`/`pile_restore` e o `static PILE_HOME` foram removidos (R1, R2 da rechecagem).

## Caminhos com o mesmo padrão `dim == game.dim`

| Caminho | Situação |
| --- | --- |
| `Host::entity` e derivados (`machine_slots`, `set_machine_slot`, `machine_data`, `set_machine_data`, `save_machine`) | corrigido |
| `pile_drop`, `pile_explode` | migrados para `in_place` |
| `Host::show` (partículas e sons de explosões/bombas) | só descartava em outra dimensão; agora usa `in_place` quando há jogador nela |
| sincronização com clientes em `set_machine_data` | idem |
| `redstone_many` (loader e controle da pile, baterias e outros leitores) | o `native_redstone_many` do motor colocava o nível em lugar e de volta a cada chamada (zerando a ociosidade e limpando as partículas do nível em lugar). Uma tentativa intermediária trocava só o `World`, mas `with_level` também entrega `block_ticks` (saídas de comparador), `lid_openers` (baú com armadilha) e a dimensão ao hook de sinal do nível em lugar: leitura híbrida, descartada. Agora o motor (`ntm-current` `840fef9a0`) tem `Game::visit_level`, que move **tudo** o que `enter_level` move mas não zera a ociosidade nem apaga partículas/pickups do nível em lugar, com guarda `Drop` (pânico incluído); `native_redstone_many` e `native_set_blocks` o usam |
| `native_set_blocks` (escrita em outro nível) | passa a usar `visit_level` (antes entrava no nível a cada escrita, zerando a ociosidade) |
| `solar_sample` | já lia o mundo estacionado; segue sem troca de nível (e `native_loaded_block`/`native_set_blocks`/`native_redstone_many` já aceitam dimensão) |
| `use_block`, `use_item`, `menu_button`, `tick` do block entity | contexto do jogador/do nível que está ticando: a dimensão em lugar é a certa |

Limitações: (1) ~~ao mudar itens de uma máquina de um nível estacionado, o `container_changed` não roda~~ **corrigido (onda 0, item 3)**: `Host::entity` compara os itens antes e depois da escrita no nível estacionado e, se mudaram, chama `container_changed` numa visita passageira (`visit_level`), que não abre nível nem zera a ociosidade; os comparadores daquela máquina são notificados no próprio nível. (2) A persistência de um nível estacionado depende do motor salvá-lo (`modified` marcado; `save_parked_levels` e `close_idle_levels` o fazem). (3) A visita não dispara `LevelEvent` de abrir/fechar nem chegadas por portal, e nunca abre um nível.

## Atualização ZIRNOX — commit atômico

Os nomes dos setters na descrição histórica acima foram substituídos por
`ServerHost::commit_machine` / `commit_machines`. O host valida `expected` e os
limites de todos os slots antes de publicar slots, dados e JSON; grupos de pilha
usam o mesmo acesso exclusivo ao `Game`, sem fallback parcial. `Host::entity`
continua acessando diretamente níveis abertos estacionados. Falha congela o
passo local e o exclui das redes; o tick seguinte tenta a partir do inventário
observado. Os setters separados foram removidos.

Drops e alterações de mundo são liberados após a confirmação. Tombstones e
cursores preservam o conteúdo de remoções confirmadas quando o nível fecha;
recibos só são descartados depois do hook existente `world_saved(wait=true)`.
A janela de crash/I/O entre mundo e KV permite replay/duplicação e permanece
pendência de motor. Ver [contrato completo](content-reactors-zirnox.md).

## Verificação

- `an_unreachable_block_entity_is_written_again_later`, `native_core_break_in_a_frozen_dimension_*` e os testes da pile.
- Aceitação em jogo descartável (`tools/test-game/native_dims.py`): jogador no Nether com Nether forçado (último a ticar); uma bateria antiga no Overworld é alimentada nesse período e uma bateria nova é colocada; os dois block entities mostram o estado (`execute in minecraft:overworld run data get block … ntm`) antes de fechar; depois de reabrir e voltar ao Overworld, ambas continuam com o estado e a nova foi adotada.

Resultados (pin `2f35078b3`, motor `2ca67d71e`, `SKIP_UPSTREAM_CHECK=1`):

- `cargo test --workspace`: 300 unitários + 26 de integração; `cargo clippy --workspace -- -D warnings` limpo. O Clippy nativo da stage só aponta os 3 `collapsible_if` do motor (`game.rs`), nenhum no glue.
- `native_dims.py`: com o binário da `main` anterior (`a2be171b`) o roteiro **falha** (`old_machine_state_follows_while_not_last: 0`, o estado do Overworld não é gravado com o Nether por último); com o binário corrigido passa: 4 verificações na montagem e 3 depois de reabrir, incluindo `reopened_new_machine_adopted`.
- Regressões no binário corrigido: `native_pile.py` (23 + 25 + 31 verificações) e `native_solar_stirling.py` (iron, steel, creative, solar, captura e reabertura) verdes.

## Atualização — visita completa ao nível estacionado

Motor `ntm-current` `840fef9a0` (pins em `compatibility.json`, `horizon-mixin.toml` e `docs/horizon-current.md`): `Game::visit_level` (em `levels.rs`) e o `native_world_access` que o usa. Reavaliei `in_place` com ele: serve (drops, explosão, sincronização, partículas e sons rodam no nível visitado e nada fica para trás; a ociosidade seguir contando é o desejado, pois sem jogador o nível ainda fecha), então o glue o usa e a guarda própria do glue foi removida.

Testes em jogo (`native_dims.py`, binário no pin novo):
- `comparator_feeds_the_loader_with_the_overworld_in_place` e `comparator_still_feeds_the_loader_while_not_last`: um comparador lendo um baú alimenta um loader no Overworld; com o Nether forçado e em lugar a leitura continua verdadeira. **No binário anterior (`6ac4cd00`, troca só do `World`) a segunda falha** (`was_redstone` volta a falso: o comparador lia as saídas do Nether).
- `idle`: Nether com um leitor fecha e grava (17 s). O roteiro também emite 60 partículas de fumaça no Overworld para inspeção visual; a sobrevivência delas não tem verificação automatizada. A antiga asserção `particles_survive_the_parked_reader` foi removida: `status['particles']` é o contador histórico `made()` do cliente, que não diminui com `clear()` e não mede partículas vivas no `Game` do servidor onde a visita ocorre.


## Onda 0, item 3 — comparadores em nível estacionado

`Host::entity` (glue) clona os itens do block entity antes de `f`; se mudaram, `Game::visit_level(dim, |g| g.container_changed(pos))`
roda depois da escrita, a mesma notificação que `native_with_mod_block_entity` faz no nível em lugar. Prova em jogo
(`native_dims.py setup`, `parked_comparator`): com o Nether em lugar e o Overworld estacionado, uma bateria carregada alimenta um
forno elétrico por cabo; um funil insere uma picareta de ferro (item de pilha 1) e o comparador ao lado lê o forno; ao fundir
a picareta em pepita o `OutputSignal` do comparador cai. Sem a notificação ele continua com o valor da picareta.
