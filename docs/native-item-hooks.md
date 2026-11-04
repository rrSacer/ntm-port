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
| `gas_mask_m65`, `attachment_mask`, `hazmat_helmet`, `hazmat_helmet_red`, `hazmat_helmet_grey` (`IGasMask`) | com sneak, tira o filtro: inventário ou drop | idem |
| `hazmat_plate` | `ItemArmor` equipa pelo clique, sem recipiente | motor |

Em criativo, `addItemStackToInventory` descarta o que não cabe (retorna `true`) e
nada cai no chão; o port reproduz isso.

### Modos de drop

`NativePlayerBenefits.drops` é uma lista de `NativeDrop { stack, mode }`, publicada
pelo motor depois da validação (`Game::native_drop_item`). Cada fonte Java usa o
seu modo:

| Modo | Java 1.7.10 | Posição, velocidade, delay | Quem usa |
| --- | --- | --- | --- |
| `Hand` | `func_146097_a(stack, false, true)`: tecla de drop e clique THROW | o `player_drop` do motor: thrower, estatística de drop e balanço da mão | clique THROW na mesa |
| `Thrown` | `dropPlayerItemWithRandomChoice(stack, qualquer)`, que **ignora o flag** e chama `func_146097_a(stack, false, false)` | olhos − 0,3; `-sin(yaw)·cos(pitch)·0,3`, `-sin(pitch)·0,3 + 0,1`, mais `cos(a)·0,02·rand` em X, `sin(a)·0,02·rand` em Z (`a = rand·2π`) e `(rand−rand)·0,1` em Y; delay 40; sem thrower; estatística de drop contada uma vez, sem balanço | `tryAddItem` (seringa, RadAway), o filtro tirado da máscara e o fechamento da mesa |
| `World { delay }` | `new EntityItem(world, x, y, z, stack)` | no jogador com `y + eyeHeight`; o construtor dá `(float)(rand·0,2 − 0,1)` em X e Z e `0,2` em Y; delay é o do chamador; sem estatística | `giveHaz` não define delay, então vale o campo, 0 |

`Thrown` é o `throw_from_eyes(.., false)` que o motor já usava nos demais "não coube",
mais a estatística. O `Random` que eu havia previsto (`func_146097_a(.., true, ..)`:
`f = rand·0,5`, `a = rand·2π`, `(−sin a·f, 0,2, cos a·f)`) só é alcançado pelos drops
de morte do vanilla; nenhum fonte do HBM o chama (o `dropPlayerItemWithRandomChoice(filter,
true)` da máscara não o alcança), então o modo não existe no port.

Origem verificada: decompilado (CFR 0.152, lido como dado, nada executado) do
`server.jar` oficial do 1.7.10 (sha1 `952438ac4e01b4d115c5fc38f891710c4941df29`, pelo
manifest da Mojang). Nomes ofuscados do jar: `EntityItem` é `xk` e `EntityPlayer` é `yz`.

| Fato | Onde (decompilado) |
| --- | --- |
| salto do construtor: `motionX = (float)(random·0,2 − 0,1)`, `motionY = 0,2f`, `motionZ` igual, yaw `random·360` | `xk` linhas 17 a 26 (construtor de 3 coordenadas; o de 5 só chama `setEntityItemStack`) |
| `delayBeforeCanPickup` é o campo `public int b` de `xk`, sem inicializador (padrão 0); o tick (`xk.h()`) o decrementa enquanto for maior que 0 | `xk` linha 11 |
| `dropPlayerItemWithRandomChoice(stack, flag)` ignora o flag: `return func_146097_a(stack, false, false)` | `yz` linhas 411 a 413 |
| posição `posY − 0,3 + eyeHeight`, delay 40, thrower só se o 3º argumento, estatística `stat.drop` em todos os modos | `yz` linhas 422 a 426 e 446 |
| fórmula do ramo `false`: 0,3, `+0,1`, ruído `0,02·rand`, `(rand−rand)·0,1` | `yz` linhas 435 a 443 |

Os números do salto e do delay 0 deixaram de ser de memória. O `giveHaz` não define
delay nem thrower, então o `World { delay: 0 }` do kit é fiel.

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
num único commit. Cada `IGasMask` tem a sua lista negra (`getBlacklist`): a M65 e a
`attachment_mask` recusam gás vesicante; os três capuzes `hazmat_helmet*`
(`ArmorHazmatMask`) não têm restrição e deixam o filtro cobri-lo. Depois de dano > 20000, o filtro é removido,
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

## Tabela de uso das famílias (onda 1)

`ntm_core::consumables` generaliza o lote de prova: `use_spec(id)` devolve uma
`UseSpec` carregada de `data/consumables.json` e `data/kits.json` (ver
`docs/content-catalogo-k2.md`). O glue `complete` aplica o spec numa única
transação do jogador e `after_commit` faz o que o commit não cobre (remover
efeitos, XP, perda direta de vida, dano, explosão).

| Campo | Significado no Java |
|---|---|
| `ticks`, `animation`, `gate` | `getMaxItemUseDuration`, `getItemUseAction`, a condição de `onItemRightClick` (`canEat`, cantil esfriado, abridor no inventário, enjoo de poção) |
| `consume`, `set_damage` | `stackSize--` sempre (`ItemFood`, `ItemSyringe`), só fora do criativo (`ItemEnergy`, BDCL), ou o dano-cooldown do cantil |
| `gives` (`replace`, `overflow`, `creative`) | o que o item devolve: no próprio slot se era o último, senão `addItemStackToInventory` com o resultado ignorado (`lost`) ou `tryAddItem` (`thrown`) |
| `food`, `heal`, `effects`, `cure`, `dose`, `radaway`, `hurt`, `hp_cost`, `xp_cost`, `xp_gain` | os efeitos, na ordem em que o Java os aplica |
| `loot` | caixa de loot: peso = chance de aceitação por raridade de `ItemLootCrate.choose` |
| `unported` | itens do Java que a linha entregaria e o port não tem: nome do Java, nada é inventado |

Mudanças de regra que o lote de prova ganhou: o criativo do 1.7.10 não come
`ItemFood`; o Rad-X soma 0,2 à resistência do fator `10^(-soma)`; o enjoo de
poção bloqueia o uso enquanto o efeito estiver ativo.

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

A mesa não abre com o jogador agachado (`BlockArmorTable.onBlockActivated`) e `open_now`
revalida o bloco e a vida do jogador antes de abrir.

A mesa conserva sua peça física no attachment separado `ntm:armor_table`,
salvo junto com o jogador. As posições de mods são vistas desse NBT, os slots
de armadura são vistas do equipamento real. Após reabrir sem o menu, a limpeza
usa o mesmo commit de fechamento e libera os itens uma vez. A transação do
motor inclui também o cursor e os drops pós-publicação necessários aos menus.

## Aceitação

Binário `40b1dffa8cd7…` e pacote `3fbcad5823f7…`, motor `0615f5db0` (sobre `ntm-current` `d5eb69a33`), NTM sobre `main` `05e51050`, no mesmo par para
todos os cenários (hashes iguais antes e depois). Cada fase grava a própria
evidência (`native-items-setup.json`, `-capture-state.json`, `-check-state.json`).

| Roteiro `items` | Checks |
| --- | --- |
| `setup` (49): CAS nativo com recusa e repetição, comer com animação real, cancelar sem consumir, fome/saturação, cinco efeitos, seringa com espaço, kit hazmat, filtro instalado uma vez, desgaste por exposição, blacklist da máscara, gama com máscara/hazmat, RadAway (total exato e `iv_empty`), mesa real (14 slots, inserir, shift-click, remover, fechar conservando) 18 checks de inventário cheio (um caso por item do lote, abaixo), os pickup delays dos modos `Thrown` e `World`, a mesa recusando o clique agachado, o filtro no limite (20000 fica, 20001 destrói) e a morte zerando a dose no próximo `radiation_tick` | 49 |
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

Regressão no mesmo binário (`mining` 18 checks, `bombs` 49, `zirnox` 136, `pile` 85,
`dims` 14 com `dims-idle` 2 e `dims-idle-clean` 1, `inventory-proxy` 12,
`radioisotopes` 67, `solar` 36, `combustion` 15, `catalog` 251, `comparator` 9,
`stations` 5 e `pile-meltdown` 32): todas passaram. `bombs`, `combustion` e `pile-meltdown`
falharam numa rodada por disco cheio (a máquina é compartilhada) e carga alta (`control
socket`, `No space left on device`); repetidos depois, no mesmo binário, passaram. Os roteiros novos da onda 0
rodaram no binário do item-hooks sobre o motor e o NTM rebaseados.

Verificação estática no HEAD final: NTM 421 testes passaram (2 doctests herdados
ignorados), Clippy `-D warnings` do workspace e da cópia transformada do motor sem
isenções; motor 1.565 testes de `horizon-client-lib` e 766 de `horizon-client`, Clippy
e `cargo fmt --check` limpos. Testes novos desta rodada: `world_item_hop_is_the_entity_constructors`, `drops_name_their_java_mode`,
`a_native_menu_click_is_answered_with_the_state_after_the_mod` e
`container_placement_follows_each_java_item`.

### Revisão independente (`artifacts/revisao-item-hooks`): 0 P0/P1/P2, 6 P3

| Achado | Resposta |
| --- | --- |
| P3-1 mesa abria agachado | `use-block` passa `sneaking`; `armor_table::open` não abre (o clique é engolido, pois a mesa não tem BE e a queda para o menu mostraria "not available yet"). Prova: `sneaking_does_not_open_the_table`. |
| P3-2 rollback de 1 tick no hóspede LAN | O clique só marca `Menu::native_resend`; `Hub::flush_native_resends` envia o menu e o cursor depois de `run_native_menu_events`, no mesmo tick, com o resultado. Teste de hub. |
| P3-3 `open_now` sem revalidar | Revalida o bloco e `dead_or_dying` antes de abrir. |
| P3-4 `ShiftLeft` físico | Não procede: `Game::keys` guarda o sneak lógico (o mapeamento de `key.sneak`, com toggle e remap; o físico fica em `physical_keys`), o mesmo que `use_hand` lê. Só acrescentei o comentário. |
| P3-5 `legacy(game)` em itens sem dose | `complete` só exige a semente do espelho para `radaway` e `cooldown`. |
| P3-6 testes | `expose` 20000/20001 (destruição), morte e respawn com dose 0 depois do próximo `radiation_tick`, pickup delays. `charge` num item elétrico real: nenhum está portado (as baterias de item são só identidades), pendência da família de baterias de item. |

Limites desta aceitação:

- Nenhuma fonte ambiental do port chama `filter_exposure` ainda; o desgaste foi
  provado pelo comando de operador `/ntmitem expose`, que usa a mesma API. A
  dose inicial do RadAway e a carga da pilha também vêm de comandos de operador
  (`/ntmitem dose` e `charge`); a dose gama do inventário é real.
- Não há roteiro de dois clientes: o hóspede LAN é coberto pelo motor (o jogador
  local já entra como hóspede da instância integrada) e pelo teste do hub, não por
  um segundo cliente.
- Um jogador com espaço recolhe de volta a peça trocada do kit na hora, como no Java
  (delay 0, conferido no decompilado do 1.7.10).
- `/ntmitem charge` prova o `patch_stack` num `minecraft:stick`. Nenhum item elétrico
  real está portado (as baterias de item são só identidades), então o teste com
  bateria real fica como pendência da família de baterias de item.

## Aceitação da onda 1 (consumíveis, kits e medicina)

Binário `horizon-client` `7eb2ffc12972…` (rebaseado sobre `main` `0e0b0ca8`) e pacote `ntm.hmod` `5a15e08d26cb…` (pin do motor
`0615f5db0`, sem mudança no motor), o mesmo par em todos os cenários, com os hashes conferidos
antes e depois (`evidence/hashes-rebase.txt`). Esta é a rodada que fecha as correções da revisão independente
(`revisao-k2`) e da sua rechecagem (R1 a R3 e a atomicidade do fertilizante).

| Roteiro `items` | Checks |
|---|---|
| `setup` (uma seção por família: bebidas com os goles exatos do BDCL, medicina com o leite e o antídoto sobre o RadAway, comidas, kits, filtros e capuzes hazmat, instrumentos com a janela do `rad_buf`, fósforo, fertilizante com o controle vanilla e lente AMS em jogo, `chocolate_milk` com o jogador morrendo) | 179 |
| `capture-state`: cooldown do cantil, antídoto limpando o RadAway, enjoo herbal sob o leite, o leite zerando um RadAway novo, `radx`, tratamento de RadAway | 10 |
| `check-state` depois de fechar e reabrir o mundo (RadAway zerado sem dose nova no meio e dose igual; o leite de novo sobre o enjoo restaurado) | 7 |

Cada seção usa o item real com animação (segurar o botão direito), mede o efeito pelo
estado do jogador, confere o recipiente devolvido e repete o uso com o inventário
cheio e em criativo. O Geiger e o dosímetro leem o `rad_buf` do servidor de radiação
(a mesma autoridade da dose), a lente AMS e os kits têm tabela própria no doc do catálogo.

Limites desta aceitação:

- Não há segundo cliente: o `chocolate_milk` fere todos os jogadores vivos do nível no
  alcance pelo caminho `native_visit_player`, mas só o jogador local foi exercitado em jogo.
- A regra de dose do Rad-X (+0,2 no expoente) e os cliques do Geiger são provados por teste de unidade;
  em jogo foi provado o efeito de conteúdo e o relatório.
- Sons e partículas são os vanilla listados em `docs/content-catalogo-k2.md`.
- O crescimento forçado da célula clicada do `powder_fertilizer` é a repetição do `growCrop` do motor
  (que sorteia por dentro): em jogo a muda mudou nas 6 tentativas (avanço de estágio, 0 árvores) e
  gastou uma pilha cada, contra 5 de 10 do bone meal vanilla (3 árvores). Dependência nominal: API de
  crescimento forçado no motor, na próxima frente de motor (com dano em entidades e tick de `ItemEntity`).
- Um `/effect clear` também é desfeito para o enjoo herbal (o tick o restaura); `/ntmitem cure` o encerra. O RadAway, ao contrário, é uma poção curável: o leite, o antídoto e o `/effect clear` o encerram.

Regressão completa no mesmo par binário e pacote (os hashes conferem antes e depois), sem falha:
`mining` 18, `bombs` 49, `zirnox` 136, `pile` 85, `pile-meltdown` 32, `dims` 14 com `dims-idle` 2 e
`dims-idle-clean` 1, `inventory-proxy` 12, `radioisotopes` 67, `solar` 36, `combustion` 15,
`catalog` 251, `comparator` 9 e `stations` 5, mais o `items` acima (196). Durante o desenvolvimento, execuções do `items` falharam por temporização do script em máquina
carregada (um clique respondido tarde; o limite do subprocesso subiu de 900 s para 1800 s depois que
o roteiro cresceu) e passaram depois do ajuste do roteiro, sem mudar o binário.
Verificação estática no HEAD final: NTM 421 testes passaram (2 doctests herdados
ignorados), Clippy `-D warnings` do workspace e da cópia transformada do motor sem
isenções; motor 1.565 testes de `horizon-client-lib` e 766 de `horizon-client`, Clippy
e `cargo fmt --check` limpos. Testes novos desta rodada: `world_item_hop_is_the_entity_constructors`, `drops_name_their_java_mode`,
`a_native_menu_click_is_answered_with_the_state_after_the_mod` e
`container_placement_follows_each_java_item`.

### Revisão independente (`artifacts/revisao-item-hooks`): 0 P0/P1/P2, 6 P3

| Achado | Resposta |
| --- | --- |
| P3-1 mesa abria agachado | `use-block` passa `sneaking`; `armor_table::open` não abre (o clique é engolido, pois a mesa não tem BE e a queda para o menu mostraria "not available yet"). Prova: `sneaking_does_not_open_the_table`. |
| P3-2 rollback de 1 tick no hóspede LAN | O clique só marca `Menu::native_resend`; `Hub::flush_native_resends` envia o menu e o cursor depois de `run_native_menu_events`, no mesmo tick, com o resultado. Teste de hub. |
| P3-3 `open_now` sem revalidar | Revalida o bloco e `dead_or_dying` antes de abrir. |
| P3-4 `ShiftLeft` físico | Não procede: `Game::keys` guarda o sneak lógico (o mapeamento de `key.sneak`, com toggle e remap; o físico fica em `physical_keys`), o mesmo que `use_hand` lê. Só acrescentei o comentário. |
| P3-5 `legacy(game)` em itens sem dose | `complete` só exige a semente do espelho para `radaway` e `cooldown`. |
| P3-6 testes | `expose` 20000/20001 (destruição), morte e respawn com dose 0 depois do próximo `radiation_tick`, pickup delays. `charge` num item elétrico real: nenhum está portado (as baterias de item são só identidades), pendência da família de baterias de item. |

Limites desta aceitação:

- Nenhuma fonte ambiental do port chama `filter_exposure` ainda; o desgaste foi
  provado pelo comando de operador `/ntmitem expose`, que usa a mesma API. A
  dose inicial do RadAway e a carga da pilha também vêm de comandos de operador
  (`/ntmitem dose` e `charge`); a dose gama do inventário é real.
- Não há roteiro de dois clientes: o hóspede LAN é coberto pelo motor (o jogador
  local já entra como hóspede da instância integrada) e pelo teste do hub, não por
  um segundo cliente.
- Um jogador com espaço recolhe de volta a peça trocada do kit na hora, como no Java
  (delay 0, conferido no decompilado do 1.7.10).
- `/ntmitem charge` prova o `patch_stack` num `minecraft:stick`. Nenhum item elétrico
  real está portado (as baterias de item são só identidades), então o teste com
  bateria real fica como pendência da família de baterias de item.

## Aceitação da onda 1 (consumíveis, kits e medicina)

Binário `horizon-client` `d7f5a365dfa0…` e pacote `ntm.hmod` `5a15e08d26cb…` (pin do motor
`0615f5db0`, sem mudança no motor), o mesmo par em todos os cenários, com os hashes conferidos
antes e depois (`evidence/hashes-r2.txt`). Esta é a rodada que fecha as correções da revisão independente
(`revisao-k2`) e da sua rechecagem (R1 a R3 e a atomicidade do fertilizante).

| Roteiro `items` | Checks |
|---|---|
| `setup` (uma seção por família: bebidas com os goles exatos do BDCL, medicina com o leite e o antídoto sobre o RadAway, comidas, kits, filtros e capuzes hazmat, instrumentos com a janela do `rad_buf`, fósforo, fertilizante com o controle vanilla e lente AMS em jogo, `chocolate_milk` com o jogador morrendo) | 179 |
| `capture-state`: cooldown do cantil, antídoto limpando o RadAway, enjoo herbal sob o leite, o leite zerando um RadAway novo, `radx`, tratamento de RadAway | 10 |
| `check-state` depois de fechar e reabrir o mundo (RadAway zerado sem dose nova no meio e dose igual; o leite de novo sobre o enjoo restaurado) | 7 |

Cada seção usa o item real com animação (segurar o botão direito), mede o efeito pelo
estado do jogador, confere o recipiente devolvido e repete o uso com o inventário
cheio e em criativo. O Geiger e o dosímetro leem o `rad_buf` do servidor de radiação
(a mesma autoridade da dose), a lente AMS e os kits têm tabela própria no doc do catálogo.

Limites desta aceitação:

- Não há segundo cliente: o `chocolate_milk` fere todos os jogadores vivos do nível no
  alcance pelo caminho `native_visit_player`, mas só o jogador local foi exercitado em jogo.
- A regra de dose do Rad-X (+0,2 no expoente) e os cliques do Geiger são provados por teste de unidade;
  em jogo foi provado o efeito de conteúdo e o relatório.
- Sons e partículas são os vanilla listados em `docs/content-catalogo-k2.md`.
- O crescimento forçado da célula clicada do `powder_fertilizer` é a repetição do `growCrop` do motor
  (que sorteia por dentro): em jogo a muda mudou nas 6 tentativas (avanço de estágio, 0 árvores) e
  gastou uma pilha cada, contra 5 de 10 do bone meal vanilla (3 árvores). Dependência nominal: API de
  crescimento forçado no motor, na próxima frente de motor (com dano em entidades e tick de `ItemEntity`).
- Um `/effect clear` também é desfeito para o enjoo herbal (o tick o restaura); `/ntmitem cure` o encerra. O RadAway, ao contrário, é uma poção curável: o leite, o antídoto e o `/effect clear` o encerram.

Regressão completa no mesmo par binário e pacote (os hashes conferem antes e depois), sem falha:
`mining` 18, `bombs` 49, `zirnox` 136, `pile` 85, `pile-meltdown` 32, `dims` 14 com `dims-idle` 2 e
`dims-idle-clean` 1, `inventory-proxy` 12, `radioisotopes` 67, `solar` 36, `combustion` 15,
`catalog` 251, `comparator` 9 e `stations` 5, mais o `items` acima (196). Uma pausa programada do
computador interrompeu a regressão no meio de `dims-idle`; ele e os seguintes rodaram de novo depois,
no mesmo binário. Durante o desenvolvimento, execuções do `items` falharam por temporização do
script em máquina carregada (um clique que o servidor respondeu tarde) e passaram depois do ajuste
do roteiro, sem mudar o binário.
Depois do rebase sobre `main` `0e0b0ca8` (correção da montadora e do meltdown), no binário novo: `items` 179 + 10 + 7, `recipes` 5 (montadora e bigorna), `pile` 85 e `pile-meltdown` 32, sem falha; a regressão dos demais cenários é a do binário anterior, cujo código não mudou além do rebase.
Verificação estática: 437 testes do NTM e Clippy `-D warnings` do workspace e do stage do motor sem
isenções; 38 testes Python de `test_consumables.py`.

