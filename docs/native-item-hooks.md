# Hooks nativos de item, stack e equipamento

Implementação na branch `item-hooks`, sobre upstream congelado `fc0210381`.
A API e o lote de prova estão aceitos em jogo (seção Aceitação); a existência
desta API não conclui os 116 itens K2.

## Contrato de publicação

O motor oferece `native_player_snapshot(key)` e
`native_commit_player(key, expected, desired, benefits)` em
`game/native_player_transactions.rs`. A chave é um `AttachmentType<Compound>`
com codec persistente. A leitura recusa jogador ausente, cliente e dados
persistidos inválidos. O snapshot carrega UUID, época do mundo, cursor e todos os slots
com todos os componentes, armadura, offhand e o attachment da família.

O commit compara o snapshot completo, valida identidades, limites de pilha,
benefícios e codec antes de publicar. Uma recusa não consome nada. Inventário,
armadura, offhand, estado, fome/saturação, cura, efeitos e cooldown são publicados
na mesma passagem do servidor, sem fila de efeitos separada. Os eventos oficiais
de equipamento são emitidos após a publicação. Armadura segue a ordem do motor:
cabeça, peito, pernas e pés.

A família monta a operação numa cópia do snapshot. Exemplo conceitual:

```rust
let expected = game.native_player_snapshot(&STATE)?;
let mut desired = expected.clone();
// Retirar uma unidade do slot observado, inserir recipiente e atualizar estado.
// O que não cabe vira `drops` do mesmo commit, como no Java.
game.native_commit_player(&STATE, &expected, desired, benefits)
```

O planejador conta toda unidade antes de publicar. Inventário cheio segue o Java
item a item, e o que não cabe sai como `drops` do mesmo commit (publicados depois
da validação, sem duplicar nem perder):

| Item | Java | Port |
| --- | --- | --- |
| `cotton_candy` | sem recipiente | só consome |
| `syringe_metal_medx` | `stack--`; vazio vira `syringe_metal_empty` no próprio slot; senão `addItemStackToInventory` ou `dropPlayerItemWithRandomChoice` | idem |
| `radaway` | `giveSoundAndDecrement` + `tryAddItem`: o slot esvaziado ainda está ocupado, então até o último RadAway derruba o `iv_empty` sem espaço | idem (`reserved`) |
| `hazmat_kit` | `giveHaz` nunca recusa; a armadura trocada vira entidade no mundo | a armadura trocada vira drop |
| `gas_mask_filter` | `ItemFilter` troca a pilha da mão pelo filtro antigo | idem, sem inventário |
| `gas_mask_m65`, `attachment_mask` | com sneak, tira o filtro: inventário ou drop | idem |
| `hazmat_plate` | `ItemArmor` equipa pelo clique, sem recipiente | motor |

Em criativo, `addItemStackToInventory` descarta o que não cabe (retorna `true`) e
nada cai no chão; o port reproduz isso.

### Modos de drop

`NativePlayerBenefits.drops` é uma lista de `NativeDrop { stack, mode }`, publicada
pelo motor depois da validação (`Game::native_drop_item`). Cada fonte Java usa o
seu modo:

| Modo | Java 1.7.10 | Posição, velocidade, delay | Quem usa |
| --- | --- | --- | --- |
| `Hand` | `dropPlayerItem` (`func_146097_a(stack, false, true)`): tecla de drop e clique THROW | o `player_drop` do motor: thrower, estatística de drop e balanço da mão | clique THROW na mesa |
| `Thrown` | `dropPlayerItemWithRandomChoice(stack, false)` | olhos − 0,3; `-sin(yaw)·cos(pitch)·0,3`, `-sin(pitch)·0,3 + 0,1`, mais ruído `0,02·rand` num ângulo aleatório e `(rand−rand)·0,1` em Y; delay 40; sem thrower nem estatística de mão | `tryAddItem` (seringa, RadAway) e o fechamento da mesa (`onContainerClosed`) |
| `Random` | `dropPlayerItemWithRandomChoice(stack, true)` | olhos − 0,3; `f = rand·0,5`, `a = rand·2π`, `(−sin a·f, 0,2, cos a·f)`; delay 40 | filtro tirado da máscara |
| `World { delay }` | `new EntityItem(world, x, y, z, stack)` | no jogador com `y + eyeHeight`; o construtor dá `(float)(rand·0,2 − 0,1)` em X e Z e `0,2` em Y; delay é o do chamador | `giveHaz` não define delay, então vale o padrão do campo, 0 |

`Thrown` é o `throw_from_eyes(.., false)` que o motor já usava nos demais
"não coube": a equivalência com `Player.drop(stack, false)` está na fórmula acima.
As fórmulas de `Random` e `World` estão em `random_toss_velocity` e
`world_item_velocity`, com teste unitário. O padrão 0 do delay do `EntityItem` e o
salto do construtor vêm do 1.7.10 de memória: o fonte do Minecraft não está no
repositório (só o do HBM), então ficam marcados para conferir no decompilado.

## Uso e previsão

Hooks oficiais utilizados: `use_item`, `use_item_on`, `use_duration`,
`use_animation`, `living_entity_use_item_start`,
`living_entity_use_item_finish`, `player_tick` e `on_equip`.
`interact_entity` e `EntityRef` oficiais estão disponíveis para famílias com
alvo; não se deve converter um id de rede em referência persistente sem época.
Hooks de tick, parada e release oficiais permitem duração variável e cancelamento.
A troca de mão/slot ou a liberação antes do término não chama o commit de consumo.

`ntm_core::item_hooks::use_spec` é a tabela das famílias deste lote; itens sem
entrada mantêm seu comportamento anterior. O algodão-doce usa 32 ticks e a pose
oficial de comer. A animação é prevista no cliente; só o servidor altera itens e
aplica efeitos. Seringas, RadAway e kit são instantâneos como no Java. A API aceita
poses de comer/beber e as famílias podem usar as poses oficiais para seus usos.
Não foi inventada duração de injeção para uma seringa instantânea do fonte.

## Estado por stack e equipamento

`ntm_items::patch_stack` compara a pilha completa e faz uma alteração atômica no
compound `minecraft:custom_data`; preserva todos os campos e componentes alheios.
Dados não-compound são recusados. Durabilidade vanilla usa `ItemStack.damage`;
carga pode usar `ntm_charge` no compound. Filtro instalado usa `hfrFilter` com o
codec oficial de ItemStack, retendo os componentes do filtro devolvido na troca.

`hazard_rate(base, target, fraction)` compartilha a interpolação Java: diferença
dos extremos em float, interpolação em double e quantização final em float.
RTG e ZIRNOX usam esse caminho preservando suas curvas distintas de vida.
`interpolate` oferece também interpolação limitada de carga/vida entre extremos.

Armadura usa os slots, atributos, desgaste e eventos de equipar/desequipar do
motor. O NTM não tem slots próprios de acessórios. Seus nove slots de mods ficam
em `ntm_armor_mods/mod_slot_N` dentro da própria armadura. A mesa de modificação
usa o layout de 198×222, 14 slots e textura originais do Java; os nove slots
de mods e os quatro slots de equipamento são vistas, nunca cópias físicas.
Inserir/retirar e shift-click atualizam armadura, inventário e cursor juntos.
Ao fechar, os mods e a peça são derrubados como no Java. Baubles é uma
integração externa excluída nominalmente desta API. `player_tick` é o ponto oficial para efeitos enquanto equipado.

Dano recebido continua pelos hooks oficiais `living_incoming_damage` e pelos
atributos de armadura; proteção de dose passa por `ServerHost::radiation_factor`.
A máscara M65 tem resistência gama 0,095; cada peça do hazmat amarelo tem
0,6 vezes a fração corporal (cabeça 0,2; peito 0,4; pernas 0,3; pés 0,1).
O multiplicador executado no Java é `10^(-soma das resistências)`, apesar de um
comentário antigo citar 5. O filtro não modifica esse fator gama.

`filter_exposure(game, hazard, wear)` aplica proteção de partículas/gás e desgaste
num único commit. A M65 bloqueia as classes do filtro comum, exceto gás vesicante
(o blacklist da máscara prevalece). Depois de dano > 20000, o filtro é removido,
como `ArmorUtil`. Sem filtro válido não há proteção. A integração das demais
fontes ambientais/gases permanece para suas famílias na onda 1.

## Persistência e dose

O attachment privado `ntm:item_state` tem schema 1 e contém dose, ticks restantes
de RadAway e estado de cooldown. Ele é salvo no arquivo vanilla do
jogador junto com inventário, efeitos, fome e armadura, também para hóspedes LAN.
Dados desconhecidos/inválidos não são substituídos por defaults. O attachment não
é copiado na morte; respeita a nova vida do jogador.

A dose antiga do `WorldState` serve somente como semente na primeira leitura.
Depois, todas as fontes nativas (inventário, fallout, bombas e ZIRNOX) leem e
publicam a dose do jogador. O espelho legado não sobrescreve um attachment já
presente. Consumo do RadAway, recipiente e tratamento são gravados juntos;
cada tick confirmado reduz dose e contador juntos. Fechar/reabrir não reaplica
o consumo nem reinicia o tratamento.

## Lote e proveniência

| Item | Fonte Java e comportamento |
| --- | --- |
| `cotton_candy` | `ItemCottonCandy` + `ModItems`: 5 fome, modificador 0,6 de saturação; poison 300t I, wither 100t I, weakness 500t III, speed 500t III e resistance 600t V. |
| `syringe_metal_medx` | `ItemSyringe`: resistance 4800t III e `syringe_metal_empty`. PotionConfig padrão é OFF; não acrescenta potion sickness arbitrária. |
| `radaway` | Inicialização real em `ItemSimpleConsumable`, `doRadaway` e `HbmPotion`: 140t acumuláveis de −1 RAD/t, devolvendo `iv_empty`. O ramo antigo de ItemSyringe não é o registro ativo. |
| `hazmat_kit` | `ItemStarterKit.giveHaz`: veste as quatro peças amarelas e conserva equipamento substituído. |
| `gas_mask_m65` / `gas_mask_filter` | `ArmorGasMask`, `ItemFilter`, `ArmorUtil` e `HazmatRegistry`: instalação/troca, resistências e desgaste acima. |
| Peças hazmat amarelas | `ArmorHazmat` e `MainRegistry.aMatHaz`: armadura 2/5/4/1 e durabilidade 660/960/900/780. |

`iv_empty` tem identidade/recipiente neste lote; seu uso para coleta de sangue
continua explicitamente como lacuna. Os demais consumíveis não foram ativados.

## Diagnóstico e aceitação

Comandos de operador nível 2 usam a mesma API do jogo:

- `/ntmitem inspect`: mostra o attachment autoritativo.
- `/ntmitem dose 200000`: estabelece dose em milliRAD para ensaio descartável.
- `/ntmitem expose lung 100`: oferece uma exposição ao filtro e gasta 100 se protegido.
- `/ntmitem charge 123`: altera carga da pilha observada, preservando seus demais dados.

`NTM_TRACE_ITEMS=1` registra commits de uso e eventos de equipamento no log.
Estado da validação e evidências ficam no artifact `item-hooks`.


O clique direito na mesa roda dentro do turno do jogador, onde o hub está emprestado
ao turno e `game.lan` está vazio; por isso `open` só registra o pedido e
`drain`, no fim do tick do servidor (`ServerTickEnd`), abre o menu pela mesma
visita ao jogador que usam os cliques e o fechamento. O servidor responde a um
clique em menu nativo depois de reenviar o menu antigo; o cursor publicado pelo
commit chega ao cliente pela extensão do motor `sent_carried`
(`docs/horizon-current.md`). A limpeza no tick do jogador lê a sessão da mesa, não
o menu do hóspede, que o turno não enxerga.

A mesa conserva sua peça física no attachment separado `ntm:armor_table`,
salvo junto com o jogador. As posições de mods são vistas desse NBT, os slots
de armadura são vistas do equipamento real. Após reabrir sem o menu, a limpeza
usa o mesmo commit de fechamento e libera os itens uma vez. A transação do
motor inclui também o cursor e os drops pós-publicação necessários aos menus.

## Aceitação

Binário `777dffefdaf6…` e pacote `3fbcad5823f7…`, motor `57e52927c`, no mesmo par para
todos os cenários (hashes iguais antes e depois). Cada fase grava a própria
evidência (`native-items-setup.json`, `-capture-state.json`, `-check-state.json`).

| Roteiro `items` | Checks |
| --- | --- |
| `setup` (41): CAS nativo com recusa e repetição, comer com animação real, cancelar sem consumir, fome/saturação, cinco efeitos, seringa com espaço, kit hazmat, filtro instalado uma vez, desgaste por exposição, blacklist da máscara, gama com máscara/hazmat, RadAway (total exato e `iv_empty`), mesa real (14 slots, inserir, shift-click, remover, fechar conservando) e 18 checks de inventário cheio, um caso por item do lote (abaixo) | 41 |
| `capture-state`: mesa preparando a peça com mod, RadAway acumulado | 6 |
| `check-state` depois de fechar e reabrir o mundo: dose sem repetir tratamento, desgaste do filtro e carga, mods da armadura | 3 |

Inventário cheio (survival, 36 slots de pedra, uma execução por caso): comida só
consome; última `syringe_metal_medx` vira seringa vazia no próprio slot; medx
empilhada derruba a seringa vazia; último RadAway derruba o `iv_empty` (o slot
esvaziado ainda está ocupado) e RadAway empilhado mantém a pilha e derruba o
recipiente; com um slot livre o `iv_empty` vai para ele; em criativo é descartado;
o kit veste as quatro peças e derruba o capacete e o peitoral trocados; a troca
de filtro devolve o antigo na mão; sneak com a máscara na mão derruba o filtro
(sem espaço) ou o guarda no slot livre; a placa equipa. Todos conservam o total.

Regressão no mesmo binário: `mining` 18 checks, `bombs` 49, `zirnox` 127, `pile` 83,
`dims` 10 (com `dims-idle` 2 e `dims-idle-clean` 1), `inventory-proxy` 12,
`radioisotopes` 67, `solar` 36, `combustion` 15 e `catalog` 251.
O `pile` falhou uma vez numa repetição (`interact`: nenhuma face visível do
bloco, sem relação com itens) e passou em duas execuções seguidas, no mesmo binário.

Verificação estática no HEAD final: NTM 417 testes passaram (2 doctests herdados
ignorados), Clippy `-D warnings` do workspace e da cópia transformada do motor sem
isenções; motor 1.560 testes de `horizon-client-lib` (inclui
`a_native_menus_cursor_published_after_the_resend_is_synced`) e 764 de
`horizon-client` passaram, Clippy do motor e `cargo fmt --check` limpos.

Limites desta aceitação:

- Nenhuma fonte ambiental do port chama `filter_exposure` ainda; o desgaste foi
  provado pelo comando de operador `/ntmitem expose`, que usa a mesma API. A
  dose inicial do RadAway e a carga da pilha também vêm de comandos de operador
  (`/ntmitem dose` e `charge`); a dose gama do inventário é real.
- Não há roteiro de dois clientes: o hóspede LAN é coberto pelo motor (o jogador
  local já entra como hóspede da instância integrada) e pelo teste do hub, não por
  um segundo cliente.
- O delay 0 e o salto do `EntityItem` (kit) são do 1.7.10 de memória, a conferir no
  decompilado. Um jogador com espaço recolhe de volta a peça trocada do kit na hora,
  como no Java.
- `/ntmitem charge` prova o `patch_stack` num `minecraft:stick`. Nenhum item elétrico
  real está portado (as baterias de item são só identidades), então o teste com
  bateria real fica como pendência da família de baterias de item.
