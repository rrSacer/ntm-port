# Dimensões e block entities no host nativo

## O defeito (S1, pré-existente, global)

No `ServerTickEnd` o motor tem em lugar o **último nível que ticou** (ordem Overworld, Nether, End; só os que têm jogador, chunk forçado, ticket de portal, cama ou pregen). O `Host::entity` do glue devolvia `None` quando a dimensão da máquina não era essa, e tudo que passa por ele virava um no-op silencioso: `save_machine`, `machine_slots`, `set_machine_slot`, `machine_data`, `set_machine_data`. Pior, `native_save` marcava o estado como gravado de qualquer jeito.

Aconteceu com: jogador no Nether, chunk forçado no Nether (fazendas), ticket de portal, jogadores em dimensões diferentes em LAN/dedicado.

Consequências: máquina **nova** nunca recebia `STATE` no block entity (bloco morto depois de reabrir o mundo); máquina antiga perdia o estado final; slots paravam de reagir.

## A correção

1. **Acesso preguiçoso à dimensão pedida** (`native/ntm/mixins/ntm/glue.rs`):
   - `Host::entity` lê/escreve o block entity no `World` da dimensão pedida, diretamente, quando ela está estacionada (`levels.parked_world_mut`), e no nível em lugar quando é a dele. Não troca de nível, então não zera a ociosidade nem limpa partículas. Com isso `close_idle_levels` continua fechando dimensões sem jogador e sem chunk forçado, mesmo quando têm máquinas e leitores de redstone (teste em jogo: `native_dims.py idle`). Nunca abre um nível: uma dimensão fechada não tem máquinas carregadas. O chunk é marcado `modified` como o `setChanged` faria.
   - O que só o nível em lugar sabe fazer (largar item, explodir, avisar clientes do block entity, partículas e sons) usa `in_place(game, dim, f)`: troca de nível só quando a dimensão pedida não é a em lugar e só para essa operação, com guarda (`Back`, `Drop`) que devolve o nível original também se `f` entrar em pânico, e sem `static`. A sincronização com clientes e as partículas só ocorrem se há jogador na dimensão.
2. **Cinto de segurança** (`ntm-core`): `ServerHost::save_machine` devolve `bool`; `native_save` só marca `native_saved` quando gravou. Um block entity inalcançável é regravado depois (teste `an_unreachable_block_entity_is_written_again_later`). `set_machine_slot` e `set_machine_data` já devolviam `bool`.
3. **Pile**: o passo da pile não precisa do nível em lugar e não entra em nenhum. `ServerHost::pile_reachable(dim)` decide se a dimensão está aberta (fechada: congela); `pile_drop` e `pile_explode` usam `in_place` e só trocam de nível quando há algo a ejetar ou explodir. `pile_enter`/`pile_restore` e o `static PILE_HOME` foram removidos (R1, R2 da rechecagem).

## Caminhos com o mesmo padrão `dim == game.dim`

| Caminho | Situação |
| --- | --- |
| `Host::entity` e derivados (`machine_slots`, `set_machine_slot`, `machine_data`, `set_machine_data`, `save_machine`) | corrigido |
| `pile_drop`, `pile_explode` | migrados para `in_place` |
| `Host::show` (partículas e sons de explosões/bombas) | só descartava em outra dimensão; agora usa `in_place` quando há jogador nela |
| sincronização com clientes em `set_machine_data` | idem |
| `redstone_many` (loader e controle da pile, baterias e outros leitores) | o `native_redstone_many` do motor colocava o nível em lugar e de volta a cada chamada (zerando a ociosidade e limpando as partículas do nível em lugar); para um nível estacionado o glue agora lê o sinal sem trocar de nível (`parked_redstone`: só o `World` ocupa o lugar do mundo durante a leitura, com guarda `WorldBack`). A dimensão que o motor passa a `with_level` continua a em lugar, o que não afeta a leitura de sinais por blocos |
| `native_set_blocks` (escrita em outro nível) | ainda entra no nível estacionado a cada escrita; só ocorre quando algo escreve blocos nele |
| `solar_sample` | já lia o mundo estacionado; segue sem troca de nível (e `native_loaded_block`/`native_set_blocks`/`native_redstone_many` já aceitam dimensão) |
| `use_block`, `use_item`, `menu_button`, `tick` do block entity | contexto do jogador/do nível que está ticando: a dimensão em lugar é a certa |

Limitações: (1) ao mudar itens de uma máquina de um nível estacionado, o `container_changed` do motor (que precisa do nível em lugar) não roda. Os **funis** leem os slots atuais e não dependem dele; os **comparadores** que leem a máquina perdem a notificação dessa mudança, e voltar a ser o nível em lugar não a repõe (só o próximo evento do comparador a atualiza). Não enfileirei a notificação: exigiria guardar posições por nível e entrar nele depois, e o ganho é só para comparadores em níveis sem jogador. (2) A persistência de um nível estacionado depende do motor salvá-lo (`modified` marcado; `save_parked_levels` e `close_idle_levels` o fazem). (3) `native_set_blocks` do motor ainda entra no nível estacionado a cada escrita (zera a ociosidade e as partículas daquela troca): só ocorre quando algo escreve blocos nele (a pile ao desmontar, escavadoras), não por tick.

## Verificação

- `an_unreachable_block_entity_is_written_again_later`, `native_core_break_in_a_frozen_dimension_*` e os testes da pile.
- Aceitação em jogo descartável (`tools/test-game/native_dims.py`): jogador no Nether com Nether forçado (último a ticar); uma bateria antiga no Overworld é alimentada nesse período e uma bateria nova é colocada; os dois block entities mostram o estado (`execute in minecraft:overworld run data get block … ntm`) antes de fechar; depois de reabrir e voltar ao Overworld, ambas continuam com o estado e a nova foi adotada.

Resultados (pin `2f35078b3`, motor `2ca67d71e`, `SKIP_UPSTREAM_CHECK=1`):

- `cargo test --workspace`: 300 unitários + 26 de integração; `cargo clippy --workspace -- -D warnings` limpo. O Clippy nativo da stage só aponta os 3 `collapsible_if` do motor (`game.rs`), nenhum no glue.
- `native_dims.py`: com o binário da `main` anterior (`a2be171b`) o roteiro **falha** (`old_machine_state_follows_while_not_last: 0`, o estado do Overworld não é gravado com o Nether por último); com o binário corrigido passa: 4 verificações na montagem e 3 depois de reabrir, incluindo `reopened_new_machine_adopted`.
- Regressões no binário corrigido: `native_pile.py` (23 + 25 + 31 verificações) e `native_solar_stirling.py` (iron, steel, creative, solar, captura e reabertura) verdes.
