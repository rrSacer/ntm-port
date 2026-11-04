# Catálogo em massa K2: consumíveis, kits e materiais soltos

Entram 116 itens, levando o catálogo a 3.191 (593 blocos e 2.598 itens). Na importação K2, todos entraram como itens inertes: identidade, textura, nome (en_US e pt_BR) e os hooks do fonte listados como lacuna. A fundação de hooks ativa um lote representativo, descrito abaixo.

## Reconhecimento genérico de classes de item (bulk.py)

Além da regra de cubo do K1, o importador agora mapeia classes de item para uma forma, sem casos isolados (`static_item`):

- **Item inerte de uso**: uma classe cuja ancestralidade não toca nenhuma base de equipamento ou estado de máquina e que não sobrescreve nenhum hook de renderização por metadata (`registerIcons`, `getIconFromDamage`, `getColorFromItemStack`, `getSubItems`, passes de render) entra como item. Os hooks de uso que ela tem (`onItemRightClick`, `onEaten`, `onItemUse`, `hitEntity`, `getMaxItemUseDuration`, `getItemUseAction`, `addInformation`, durabilidade...) viram `behavior_gap` explícito, e o NBT por pilha também fica anotado. `setMaxDamage` limita a pilha a 1 (sem `max_damage`, como nos demais casos).
- **Bases proibidas**: `ItemArmor`, `ItemTool`, `ItemSword`, `ItemHoe`, `ItemBucket`, `ItemRecord`, `ItemArmorMod`, `ItemFuelRod`, `ItemNuclearWaste`, `ItemEnumMulti`, `ItemGenericPart`, `ItemBattery`, `ItemKeyPin`, `ItemCraftingDegradation` continuam auditadas.
- **Outras famílias** continuam fora, por classe ou nome: mísseis e partes (`mp_*`, `missile_*`), armas e munição, satélites e chips, designadores, detonadores e granadas, pilha e placas legadas, resíduos, armaduras e modificadores, instrumentos (Geiger, dosímetro, detectores, scanners, linkers, RTTY, sensor do reator), varinhas, sifão e itens de lote próprio.
- Linhas já em `bulk.json` ficam congeladas e nomes já declarados em outros catálogos não são reclamados. A regeneração acrescentou 116 itens e não removeu nem alterou nenhum.

## O que entrou

| Classe do fonte | Itens |
| --- | ---: |
| `ItemEnergy` (latas e garrafas de bebida) | 19 |
| `ItemStarterKit` (kits de bombas, armaduras e custom) | 19 |
| `ItemLemon` e comida (limão, queijo, chocolate, comidas especiais, remédios) | 17 |
| `ItemDrop` (células, singularidades, pellets) | 10 |
| `ItemPill` (pílulas) | 9 |
| `ItemSyringe` (seringas) | 6 |
| `ItemFilter` (filtros de máscara) | 5 |
| `ItemSoup`, `ItemLootCrate`, `ItemCigarette` | 8 |
| Materiais e utilitários soltos (cada um com classe própria: `rag`, `plastic_bag`, `casing_bag`, `bdcl`, `matchstick`, `nugget`, `powder_fertilizer`, `particle_digamma`, `crystal_xen`, `loops`...) | 23 |

Lista completa em `content/ids.json` (as linhas novas deste commit). Os itens de bomba e de míssil de `ItemStarterKit` aparecem nas abas do criativo de bombas e mísseis, como no fonte.

## Uso dos itens (onda 1)

O uso de cada item vem de duas tabelas conferidas contra o Java:

- `crates/ntm-core/data/consumables.json`, escrita à mão a partir das classes (`ItemEnergy`, `ItemCanteen`, `ItemMuchoMango`, `ItemBDCL`, `ItemPill`, `ItemSyringe`, `ItemSimpleConsumable`, `ItemLemon`, `ItemSoup`, `ItemCigarette`...). Cada linha cita a classe e a linha do `ModItems`.
- `crates/ntm-core/data/kits.json`, **gerada** por `tools/content-gen/kits.py` a partir de `ItemStarterKit` e `ItemLootCrate`.

`tools/content-gen/test_consumables.py` relê o Java e compara efeitos, durações, curas, doses, recipientes, tampas, listas dos kits e pesos das caixas com essas tabelas. O glue (`native/ntm/mixins/ntm/item_hooks.rs`) aplica cada uma num único commit atômico do jogador: efeitos, cura, fome, dose, recipientes e quedas.

Regras do Java que o port reproduz e que enganam à primeira leitura:

- Em 1.7.10, `EntityPlayer.canEat` é `(alwaysEdible || fome < 20) && !disableDamage`: **o jogador em criativo não come `ItemFood`** (conferido no `server.jar` decompilado). Bebidas, pílulas, mango e BDCL não perguntam `canEat`.
- `ItemLemon.onFoodEaten` não chama `super`: o `setPotionEffect` de `glyphid_meat_grilled` (Strength) nunca dispara.
- Recipientes devolvidos: `ItemEnergy` e os kits chamam `addItemStackToInventory` **ignorando o retorno** (o que não cabe some, não cai no chão); `ItemSyringe` e `ItemSimpleConsumable` usam `tryAddItem`/`dropPlayerItemWithRandomChoice` (cai lançado). O slot do item gasto ainda conta como ocupado enquanto o item roda.
- `ItemBDCL` anuncia 40 ticks mas `onUsingTick` pula contagens: o gole termina em 34.
- `FoodStats.addStats` satura em 20: `nugget` (cura 200) e `apple_euphemium` (modificador 100) acabam no mesmo estado que o motor aceita (0..20).
- `ItemCanteen` guarda o cooldown como dano da pilha (180, um por segundo).
- `ItemLemon` chama `setAlwaysEdible` atrás de `this == ModItems.med_ipecac || this == ModItems.med_ptsd`, mas dentro do construtor o campo estático ainda é nulo: a condição nunca vale e `med_ipecac` e `med_ptsd` **exigem fome** como qualquer `ItemFood`. Só `cotton_candy` e `apple_euphemium` (e `mucho_mango`, pílulas, `tem_flakes`...) são `alwaysEdible`.
- Tipos de dano: `euthanizedSelf` (`syringe_poison`, `plan_c`) só ignora armadura (`minecraft:magic`; Resistência ainda o absorve) e `overdose` (`chocolate`) é absoluto (`minecraft:generic_kill`: ignora armadura e Resistência). Em criativo nenhum dos dois fere.
- O RadAway é uma poção comum no Java (`HbmPotion.radaway`, curável): o port a espelha com o efeito `ntm:radaway`, e o contador `radaway` do attachment termina junto com ele. O antídoto (`clearActivePotions`), o leite (`ClearAllEffects` do motor) e `/effect clear` encerram o tratamento; a dose para de cair. O enjoo herbal é a exceção incurável: o antídoto o zera, o leite não. O leite não cura o enjoo herbal (`setCurativeItems(empty)`), mas o leite do motor limpa todos os efeitos: o attachment guarda o tempo que falta (`herbal`) e o tick devolve o efeito se ele sumir com tempo sobrando; só o antídoto ou o fim do tempo o encerram (um `/effect clear` também é desfeito).
- `rad_buf` soma a leitura contínua (itens carregados, zonas, ZIRNOX) e os `ContaminationUtil.contaminate` pontuais (garrafas Nuka/Cherry/Quantum/Sparkle/Rad: 5/5/15/5/15 RAD) na janela de um segundo, como `radEnv → radBuf`. `incrementRadiation` direto (`coffee_radium`, `pill_herbal`, `cigarette`) não entra. Rad-X e tratamento não mexem na leitura.
- `powder_fertilizer`: a célula clicada cresce sempre (`force`) e as demais sorteiam o próprio sucesso; o motor só tem `growCrop`, que sorteia por dentro, então a central o repete (máx. 64 vezes) até a planta mudar nos casos em que o sorteio pode falhar. Não há o fallback de plantas aquáticas do bone meal moderno. Fora do criativo a pilha é gasta num commit confirmado antes de qualquer mudança no mundo e devolvida do mesmo modo se nada era crescível, então uma publicação recusada nunca deixa um crescimento de graça.
- Enjoo de poção (`VersatileConfig.hasPotionSickness`) trava seringas, pílulas e bebidas enquanto o efeito `ntm:potion_sickness` (vindo de `pill_herbal`) estiver ativo; `applyPotionSickness` é nulo com a `PotionConfig` padrão (OFF).
- `chocolate_milk` roda o `ExplosionVNT(50)`: alocação de raios com `BlockAllocatorStandard(32)` (`ntm_core::blast::vanilla::allocate_vnt`, raios idênticos aos da explosão vanilla do NTM, resolução 32, limite 64), dano e empurrão do `EntityProcessorCross(5)` (alcance 2×tamanho, 7 nós, `(int)((k²+k)/2·8·alcance+1)`), blocos removidos sem drops. A explosão só é enfileirada no uso e roda no fim do tick do servidor (`ServerTickEnd`), quando o hub está livre: aí todos os jogadores vivos do nível são visitados (`native_players` e `native_visit_player`), cada um mede a exposição com o raycast de colisão do motor (`getBlockDensity` do 1.7.10: grade de `1/(2·extensão+1)` sobre a caixa, um raio por ponto até cada nó; lajes, escadas e cercas bloqueiam pela forma real) com os blocos ainda de pé, e só então os blocos saem. Mobs e itens soltos não são atingidos (sem API de dano).

Efeitos de poção do HBM que o conteúdo registra como efeitos passivos (`data/ntm/horizon/mob_effect`, ícones recortados de `potions.png`): `ntm:radx` (+0,2 de resistência na fórmula `10^(-soma)` da dose, como `HazmatRegistry`), `ntm:radaway` (bandeira do tratamento), `ntm:stability`, `ntm:death`, `ntm:potion_sickness` e `ntm:taint`. O motor não tem tick para efeitos de conteúdo: o dano periódico do taint fica listado abaixo.

Itens novos desta onda: `radaway_strong`, `radaway_flush`, `iv_blood`, `iv_xp_empty`, `iv_xp`, `syringe_antidote`, `syringe_poison`, `syringe_awesome`, `bottle_opener` (só identidade: o `WeaponSpecial` espera a onda de armas) e as doze peças do hazmat vermelho e cinza (`ArmorHazmat`, resistências 1,0× e 2,0× as frações do corpo), que os kits equipam.

## Lacunas nominais

- **Item solto no chão** (`onEntityItemUpdate`): `beta`, células, singularidades, `crystal_xen`, `particle_digamma`, `demon_core_open` e o `rag` na água precisam de um gancho de tick de `ItemEntity` (no chão, remover/trocar a pilha) que o motor fixado não oferece (só `native_item_spawn`, observação, e `native_item_despawn`/`expire`). Os efeitos que ele dispara (`EntityVortex`, `EntityBlackHole`, `EntityRagingVortex`, `EntityQuasar`, `ExplosionVNT.makeAmat`, `EntityNukeExplosionMK3`, `ExplosionChaos`) também não existem.
- **Dano e empurrão em mobs** (explosões VNT, seringas `hitEntity`): sem API do NTM para ferir mobs.
- **Crescimento forçado do bone meal** (`powder_fertilizer`): o `ItemFertilizer` força a célula clicada (`fertilize(..., force)`, sem o sorteio de sucesso) e o motor só expõe `growCrop`, que sorteia por dentro. O port repete a chamada até a planta mudar (máx. 64). Em jogo, a muda clicada mudou nas 6 tentativas (avanço de estágio) com uma pilha gasta cada, contra 5 de 10 do bone meal vanilla. Dependência nominal da próxima frente de motor: API de crescimento forçado, junto com dano em entidades e tick de `ItemEntity`.
- **Sistemas do jogador**: `HbmLivingProps` asbestos, black lung e digamma; conquistas; `HbmPotion.radiation` e o tick do `taint`.
- **Menus de item**: `plastic_bag`, `casing_bag` e `containment_box` abrem um inventário ligado à pilha.
- **Outras famílias**: fechaduras (`key_kit`), `EntityBoat` (`boat_rubber`), loja (`bobmazon`), `ModBlocks.balefire`, armaduras elétricas (`pancake`), `ExplosionNukeSmall` (`bomb_waffle`), `bottle_opener` real.

## Efeito nas tabelas

Receitas que dependiam desses itens e estavam em `skipped` passaram a resolver: bigorna +1 (143), planta química +2 (62), cristalizador +1 (405 emitidas; `pill_red` deixou de ser uma lacuna), e a receita da barra RBMK `rbmk_fuel_drx` (balefire + `particle_digamma`), já documentada no R2. `crates/ntm-core/data/furnace_recipes.json` ganhou três receitas de forno do fonte (`glyphid_meat` → `glyphid_meat_grilled`, `rag_damp` e `rag_piss` → `rag`); o gerador as trata como receita de forno nativa e não emite o equivalente vanilla (a regeneração só estabiliza assim, verificada em duas passadas). Hazards: +116 identidades, com uma radioativa (`demon_core_open`, 5 RAD/s); nenhuma taxa antiga muda. Buckets regenerados.

## Prova e validação

`verify-catalog-batch.py` entrega os 116 itens pelo inventário no binário nativo, mudo, com detector de rejeição: 116 IDs, 144 conferências, sem `reject` nem `panic` no log do cliente. 208 testes por seed 1/42 (inclui as regras de classe de item em `test_bulk_forms.py`), `validate-bulk` e `recipe-gen/validate` verdes. Pacote `ntm.hmod` SHA-256 `353fa287a9cd1b20ae6eceed2775a7aafe94bf47f8d20389ab6c390f09e0ff6c` (sobre o K1), aprovado no check do upstream e do fork.


## Lote representativo de hooks (em validação)

`cotton_candy`, `syringe_metal_medx`, `hazmat_kit` e `gas_mask_filter` usam
agora a API nativa de uso/estado. O lote acrescenta `radaway`, `iv_empty`,
M65, quatro peças hazmat amarelas e `attachment_mask`, com a mesa de
modificação. O uso de coleta de sangue de `iv_empty` continua fora do lote.
Os demais consumíveis K2 não foram ativados. Contratos e evidências atuais
estão em [Hooks nativos de item](native-item-hooks.md) e no artifact `item-hooks`;
as provas históricas acima dizem respeito à importação do catálogo.

## Estado por item (gerado)

<!-- k2-status:start -->

Resumo dos 116 itens: 52 ativo, 19 dependente, 45 parcial.

| Item | Classe Java | Estado | Origem Java | Dependências nominais |
|---|---|---|---|---|
| `ntm:ams_lens` | ItemLens | parcial | `ItemLens.addInformation` | a barra de durabilidade (`showDurabilityBar`) não é desenhada: o gancho `native_item_bar` não está injetado; o tooltip está portado |
| `ntm:apple_euphemium` | ItemAppleEuphemium | ativo | ItemAppleEuphemium(20, 100, false), ModItems.java:3812: alwaysEdible, epic, glint | - |
| `ntm:balefire_and_ham` | ItemSoup | ativo | ItemSoup(6), ModItems.java:3619: vanilla ItemSoup, stack size 1, returns the bowl | - |
| `ntm:balefire_and_steel` | ItemBalefireMatch | dependente | `ItemBalefireMatch`, ModItems.java:4295 | `ModBlocks.balefire` não existe no port |
| `ntm:balefire_scrambled` | ItemSoup | ativo | ItemSoup(6), ModItems.java:3618: vanilla ItemSoup, stack size 1, returns the bowl | - |
| `ntm:bdcl` | ItemBDCL | ativo | ItemBDCL, ModItems.java:2849; getMaxItemUseDuration 40, onUsingTick skips ticks (see consumables.rs) | - |
| `ntm:beta` | ItemDrop | dependente | `ItemDrop`, ModItemsArmor.java:35 | `onEntityItemUpdate` (efeito ao cair no chão): sem gancho de tick de item solto (`ItemEntity.tick`, `onGround`, remover/trocar) no motor fixado; o Java apaga o item no primeiro tick |
| `ntm:black_hole` | ItemDrop | dependente | `ItemDrop`, ModItems.java:2719 | `onEntityItemUpdate` (efeito ao cair no chão): sem gancho de tick de item solto (`ItemEntity.tick`, `onGround`, remover/trocar) no motor fixado; `EntityBlackHole` |
| `ntm:boat_rubber` | ItemBoatRubber | dependente | `ItemBoatRubber`, ModItems.java:2950 | `EntityBoat` do mod (bote de borracha) |
| `ntm:bobmazon` | ItemCatalog | dependente | `ItemCatalog`, ModItems.java:3894 | `GUIScreenBobmazon` e `BobmazonOfferFactory` (loja do cliente) |
| `ntm:bomb_waffle` | ItemWaffle | parcial | ItemWaffle(20, false), ModItems.java:3610: ExplosionNukeSmall.explode(PARAMS_MEDIUM) has no counterpart in the port | `ExplosionNukeSmall.explode(PARAMS_MEDIUM)`, o "muke", não tem contraparte no `ntm-core` |
| `ntm:bottle2_fritz` | ItemEnergy | parcial | ItemEnergy.makeBottle(bottle2_empty, cap_fritz), ModItems.java:2942 | exige `bottle_opener` no inventário (item só com identidade: `WeaponSpecial`) |
| `ntm:bottle2_korl` | ItemEnergy | parcial | ItemEnergy.makeBottle(bottle2_empty, cap_korl), ModItems.java:2941 | exige `bottle_opener` no inventário (item só com identidade: `WeaponSpecial`) |
| `ntm:bottle_cherry` | ItemEnergy | parcial | ItemEnergy.makeBottle(bottle_empty, cap_nuka), ModItems.java:2936 | exige `bottle_opener` no inventário (item só com identidade: `WeaponSpecial`) |
| `ntm:bottle_nuka` | ItemEnergy | parcial | ItemEnergy.makeBottle(bottle_empty, cap_nuka), ModItems.java:2935 | exige `bottle_opener` no inventário (item só com identidade: `WeaponSpecial`) |
| `ntm:bottle_quantum` | ItemEnergy | parcial | ItemEnergy.makeBottle(bottle_empty, cap_quantum), ModItems.java:2937 | exige `bottle_opener` no inventário (item só com identidade: `WeaponSpecial`) |
| `ntm:bottle_rad` | ItemEnergy | parcial | ItemEnergy.makeBottle(bottle_empty, cap_rad), ModItems.java:2939 | exige `bottle_opener` no inventário (item só com identidade: `WeaponSpecial`) |
| `ntm:bottle_sparkle` | ItemEnergy | parcial | ItemEnergy.makeBottle(bottle_empty, cap_sparkle), ModItems.java:2938 | exige `bottle_opener` no inventário (item só com identidade: `WeaponSpecial`) |
| `ntm:boy_kit` | ItemStarterKit | ativo | ItemStarterKit.onItemRightClick boy_kit, ModItems.java:3786 | - |
| `ntm:can_bepis` | ItemEnergy | ativo | ItemEnergy.makeCan, ModItems.java:2931 | - |
| `ntm:can_breen` | ItemEnergy | ativo | ItemEnergy.makeCan, ModItems.java:2932 | - |
| `ntm:can_creature` | ItemEnergy | ativo | ItemEnergy.makeCan, ModItems.java:2926 | - |
| `ntm:can_luna` | ItemEnergy | ativo | ItemEnergy.makeCan, ModItems.java:2930 | - |
| `ntm:can_mrsugar` | ItemEnergy | ativo | ItemEnergy.makeCan, ModItems.java:2928 | - |
| `ntm:can_mug` | ItemEnergy | ativo | ItemEnergy.makeCan, ModItems.java:2933 | - |
| `ntm:can_overcharge` | ItemEnergy | ativo | ItemEnergy.makeCan, ModItems.java:2929 | - |
| `ntm:can_redbomb` | ItemEnergy | ativo | ItemEnergy.makeCan, ModItems.java:2927 | - |
| `ntm:can_smart` | ItemEnergy | ativo | ItemEnergy.makeCan, ModItems.java:2925 | - |
| `ntm:canteen_vodka` | ItemCanteen | ativo | ItemCanteen(3 * 60), ModItems.java:3632 | - |
| `ntm:casing_bag` | ItemCasingBag | dependente | `ItemCasingBag`, ModItems.java:3362 | menu de inventário ligado à pilha (`InventoryCasingBag`, 15 slots, `pushCasing`) |
| `ntm:cell_anti_schrabidium` | ItemDrop | dependente | `ItemDrop`, ModItems.java:2693 | `onEntityItemUpdate` (efeito ao cair no chão): sem gancho de tick de item solto (`ItemEntity.tick`, `onGround`, remover/trocar) no motor fixado; `EntityNukeExplosionMK3.statFacFleija` e `EntityCloudFleija` |
| `ntm:cell_antimatter` | ItemDrop | dependente | `ItemDrop`, ModItems.java:2689 | `onEntityItemUpdate` (efeito ao cair no chão): sem gancho de tick de item solto (`ItemEntity.tick`, `onGround`, remover/trocar) no motor fixado; `ExplosionVNT(3).makeAmat()` |
| `ntm:cheese` | ItemLemon | ativo | ItemLemon(5, 0.75F), ModItems.java:3637 | - |
| `ntm:cheese_quesadilla` | ItemLemon | ativo | ItemLemon(8, 1.0F), ModItems.java:3638 | - |
| `ntm:chocolate` | ItemPill | ativo | ItemPill, ModItems.java:2947: 1 in 25 overdose (1000 absolute damage: armor and Resistance bypassed, minecraft:generic_kill), then speed, haste and jump 60 seconds | - |
| `ntm:chocolate_milk` | ItemEnergy | parcial | ItemEnergy, ModItems.java:2944 | mobs e itens soltos do raio 100 não levam dano nem empurrão (sem API de dano a mobs); `ExplosionCreator.composeEffectStandard` é um show de partículas do cliente, substituído por som e emissor vanilla; os raios usam o acaso por hash de posição do `ntm-core`, não o `world.rand` |
| `ntm:cigarette` | ItemCigarette | parcial | ItemCigarette, ModItems.java:2847: black lung and asbestos (HbmLivingProps) and the no9 achievement are not in the port | asbestos e black lung; conquista `achNo9` |
| `ntm:coffee` | ItemEnergy | ativo | ItemEnergy, ModItems.java:2945 | - |
| `ntm:coffee_radium` | ItemEnergy | parcial | ItemEnergy, ModItems.java:2946 | conquista `achRadium` (sistema de conquistas) |
| `ntm:containment_box` | ItemLeadBox | dependente | `ItemLeadBox`, ModItems.java:3357 | menu de inventário ligado à pilha (`InventoryLeadBox`, 20 slots) |
| `ntm:cotton_candy` | ItemCottonCandy | ativo | ItemCottonCandy(5, false), ModItems.java:3613; alwaysEdible; ItemFood.onEaten | - |
| `ntm:crackpipe` | ItemCigarette | parcial | ItemCigarette, ModItems.java:2848: black lung (HbmLivingProps) is not in the port | black lung |
| `ntm:crystal_xen` | ItemDrop | dependente | `ItemDrop`, ModItems.java:2722 | `onEntityItemUpdate` (efeito ao cair no chão): sem gancho de tick de item solto (`ItemEntity.tick`, `onGround`, remover/trocar) no motor fixado; `ExplosionChaos.floater` e `move` |
| `ntm:custom_kit` | ItemStarterKit | parcial | ItemStarterKit.onItemRightClick custom_kit, ModItems.java:3791 | itens Java ausentes do port: `ModBlocks.nuke_custom x1` |
| `ntm:definitelyfood` | ItemLemon | ativo | ItemLemon(3, 0.5F), ModItems.java:3621 | - |
| `ntm:demon_core_open` | ItemDemonCore | dependente | `ItemDemonCore`, ModItems.java:2695 | `onEntityItemUpdate` (efeito ao cair no chão): sem gancho de tick de item solto (`ItemEntity.tick`, `onGround`, remover/trocar) no motor fixado; troca por `demon_core_closed` e solta `screwdriver` (o item não existe) |
| `ntm:euphemium_kit` | ItemStarterKit | parcial | ItemStarterKit.onItemRightClick euphemium_kit, ModItems.java:3795 | itens Java ausentes do port: `ModItems.euphemium_helmet x1`, `ModItems.euphemium_plate x1`, `ModItems.euphemium_legs x1`, `ModItems.euphemium_boots x1`, `ModBlocks.statue_elb_f x1` |
| `ntm:five_htp` | ItemPill | parcial | ItemPill, ModItems.java:2833: digamma to 0 (not in the port) and Stability 10 minutes | `HbmLivingProps.digamma` (o efeito Stability está portado) |
| `ntm:fleija_kit` | ItemStarterKit | parcial | ItemStarterKit.onItemRightClick fleija_kit, ModItems.java:3792 | itens Java ausentes do port: `ModBlocks.nuke_fleija x1` |
| `ntm:fmn` | ItemPill | parcial | ItemPill, ModItems.java:2832: digamma capped at 2 (not in the port) and blindness 60 ticks | `HbmLivingProps.digamma` (a cegueira de 60 ticks está portada) |
| `ntm:fooditem` | ItemLemon | ativo | ItemLemon(2, 5.0F), ModItems.java:3628 | - |
| `ntm:gadget_kit` | ItemStarterKit | ativo | ItemStarterKit.onItemRightClick gadget_kit, ModItems.java:3785 | - |
| `ntm:gas_mask_filter` | ItemFilter | parcial | ItemFilter + ArmorUtil.register | instalar, retirar (agachado) e o desgaste estão portados nas máscaras `gas_mask_m65`, `attachment_mask` e nos três capuzes `hazmat_helmet*` (`ArmorHazmatMask`, sem lista negra), mas só o comando `/ntmitem expose` chama `filter_exposure`: as fontes ambientes que o Java liga ao filtro (monóxido, gás pulmonar e de bolhas, bactérias, partículas finas e grossas) não existem no port, então nada gasta o filtro durante o jogo |
| `ntm:gas_mask_filter_combo` | ItemFilter | parcial | ItemFilter + ArmorUtil.register | instalar, retirar (agachado) e o desgaste estão portados nas máscaras `gas_mask_m65`, `attachment_mask` e nos três capuzes `hazmat_helmet*` (`ArmorHazmatMask`, sem lista negra), mas só o comando `/ntmitem expose` chama `filter_exposure`: as fontes ambientes que o Java liga ao filtro (monóxido, gás pulmonar e de bolhas, bactérias, partículas finas e grossas) não existem no port, então nada gasta o filtro durante o jogo |
| `ntm:gas_mask_filter_mono` | ItemFilter | parcial | ItemFilter + ArmorUtil.register | instalar, retirar (agachado) e o desgaste estão portados nas máscaras `gas_mask_m65`, `attachment_mask` e nos três capuzes `hazmat_helmet*` (`ArmorHazmatMask`, sem lista negra), mas só o comando `/ntmitem expose` chama `filter_exposure`: as fontes ambientes que o Java liga ao filtro (monóxido, gás pulmonar e de bolhas, bactérias, partículas finas e grossas) não existem no port, então nada gasta o filtro durante o jogo |
| `ntm:gas_mask_filter_piss` | ItemFilter | parcial | ItemFilter + ArmorUtil.register | instalar, retirar (agachado) e o desgaste estão portados nas máscaras `gas_mask_m65`, `attachment_mask` e nos três capuzes `hazmat_helmet*` (`ArmorHazmatMask`, sem lista negra), mas só o comando `/ntmitem expose` chama `filter_exposure`: as fontes ambientes que o Java liga ao filtro (monóxido, gás pulmonar e de bolhas, bactérias, partículas finas e grossas) não existem no port, então nada gasta o filtro durante o jogo |
| `ntm:gas_mask_filter_rag` | ItemFilter | parcial | ItemFilter + ArmorUtil.register | instalar, retirar (agachado) e o desgaste estão portados nas máscaras `gas_mask_m65`, `attachment_mask` e nos três capuzes `hazmat_helmet*` (`ArmorHazmatMask`, sem lista negra), mas só o comando `/ntmitem expose` chama `filter_exposure`: as fontes ambientes que o Java liga ao filtro (monóxido, gás pulmonar e de bolhas, bactérias, partículas finas e grossas) não existem no port, então nada gasta o filtro durante o jogo |
| `ntm:glowing_stew` | ItemSoup | ativo | ItemSoup(6), ModItems.java:3617: vanilla ItemSoup, stack size 1, returns the bowl | - |
| `ntm:glyphid_meat` | ItemLemon | ativo | ItemLemon(3, 0.5F), ModItems.java:3640 | - |
| `ntm:glyphid_meat_grilled` | ItemLemon | ativo | ItemLemon(8, 0.75F), ModItems.java:3641; setPotionEffect(damageBoost 180s) is never applied: ItemLemon.onFoodEaten does not call super | - |
| `ntm:hazmat_grey_kit` | ItemStarterKit | ativo | ItemStarterKit.onItemRightClick hazmat_grey_kit, ModItems.java:3799 | - |
| `ntm:hazmat_kit` | ItemStarterKit | ativo | ItemStarterKit.onItemRightClick hazmat_kit, ModItems.java:3797 | - |
| `ntm:hazmat_red_kit` | ItemStarterKit | ativo | ItemStarterKit.onItemRightClick hazmat_red_kit, ModItems.java:3798 | - |
| `ntm:ingot_c4` | ItemLemon | ativo | ItemLemon(4, 5, true), ModItems.java:2170 | - |
| `ntm:ingot_smore` | ItemFood | ativo | ItemFood(10, 20F, false), ModItems.java:2122 | - |
| `ntm:key_kit` | ItemCounterfeitKeys | dependente | `ItemCounterfeitKeys`, ModItems.java:3852 | `TileEntityLockableBase` (fechaduras e pinos) |
| `ntm:lemon` | ItemLemon | ativo | ItemLemon(3, 0.5F), ModItems.java:3620 | - |
| `ntm:loop_stew` | ItemLemon | ativo | ItemLemon(10, 0.5F), ModItems.java:3626; onEaten returns the bowl | - |
| `ntm:loops` | ItemLemon | ativo | ItemLemon(4, 0.25F), ModItems.java:3625 | - |
| `ntm:loot_10` | ItemLootCrate | parcial | ItemLootCrate.onItemRightClick loot_10, ModItems.java:3803 | a lista `list10` do Java é vazia (nenhuma fuselagem de 10 chama `setRarity`): `rand.nextInt(0)` lança lá e aqui nada acontece |
| `ntm:loot_15` | ItemLootCrate | parcial | ItemLootCrate.onItemRightClick loot_15, ModItems.java:3804 | a lista `list15` do Java é vazia: idem loot_10 |
| `ntm:loot_misc` | ItemLootCrate | parcial | ItemLootCrate.onItemRightClick loot_misc, ModItems.java:3805 | itens Java ausentes do port: `mp_thruster_15_solid_hexdecuple`, `mp_thruster_15_balefire_large_rad`, `mp_stability_10_space`, `mp_stability_15_soyuz` |
| `ntm:man_kit` | ItemStarterKit | ativo | ItemStarterKit.onItemRightClick man_kit, ModItems.java:3787 | - |
| `ntm:matchstick` | ItemMatch | ativo | `ItemMatch.onItemUse` | - |
| `ntm:med_bag` | ItemSyringe | parcial | ItemSyringe.onItemRightClick med_bag, ModItems.java:2827: setHealth(getMaxHealth()), no sound, no container | `HbmPotion.radiation` (efeito que contamina) não existe: a dose vive no attachment do jogador |
| `ntm:med_ipecac` | ItemLemon | ativo | ItemLemon(0, 0, false), ModItems.java:3622: NOT alwaysEdible (the constructor compares ModItems.med_ipecac before the field is assigned, so setAlwaysEdible never runs); the vomit particle packet has no mode and count 0, so only the sound shows | - |
| `ntm:med_ptsd` | ItemLemon | ativo | ItemLemon(0, 0, false), ModItems.java:3623: NOT alwaysEdible (the constructor compares ModItems.med_ptsd before the field is assigned, so setAlwaysEdible never runs); the vomit particle packet has no mode and count 0, so only the sound shows | - |
| `ntm:mike_kit` | ItemStarterKit | ativo | ItemStarterKit.onItemRightClick mike_kit, ModItems.java:3788 | - |
| `ntm:mucho_mango` | ItemMuchoMango | ativo | ItemMuchoMango(10), ModItems.java:3639; alwaysEdible, right click unconditional | - |
| `ntm:multi_kit` | ItemStarterKit | parcial | ItemStarterKit.onItemRightClick multi_kit, ModItems.java:3790 | itens Java ausentes do port: `ModBlocks.bomb_multi x6` |
| `ntm:nugget` | ItemLemon | ativo | ItemLemon(200, 1.0F), ModItems.java:3634 | - |
| `ntm:nuke_advanced_kit` | ItemStarterKit | parcial | ItemStarterKit.onItemRightClick nuke_advanced_kit, ModItems.java:3782 | itens Java ausentes do port: `ModBlocks.machine_uf6_tank x2`, `ModBlocks.machine_puf6_tank x2` |
| `ntm:nuke_commercially_kit` | ItemStarterKit | parcial | ItemStarterKit.onItemRightClick nuke_commercially_kit, ModItems.java:3783 | entrega `ntm:inf_water_mk2` só com identidade: o uso real depende de `FluidLoaderInfinite` |
| `ntm:nuke_electric_kit` | ItemStarterKit | parcial | ItemStarterKit.onItemRightClick nuke_electric_kit, ModItems.java:3784 | itens Java ausentes do port: `ModItems.battery_potato x1`, `ModItems.screwdriver x1`, `ModBlocks.red_wire_coated x16`, `ModBlocks.machine_converter_he_rf x1`, `ModBlocks.machine_converter_rf_he x1` |
| `ntm:nuke_starter_kit` | ItemStarterKit | ativo | ItemStarterKit.onItemRightClick nuke_starter_kit, ModItems.java:3781 | - |
| `ntm:pancake` | ItemPancake | parcial | ItemPancake(20, 20, false), ModItems.java:3633: onItemRightClick needs ArmorFSB with bj_helmet, which the port does not have | `ArmorFSB.hasFSBArmorIgnoreCharge` e `bj_helmet`, e `IBatteryItem` das armaduras (família de armaduras elétricas) |
| `ntm:particle_digamma` | ItemDigamma | dependente | `ItemDigamma`, ModItems.java:2713 | `onEntityItemUpdate` (efeito ao cair no chão): sem gancho de tick de item solto (`ItemEntity.tick`, `onGround`, remover/trocar) no motor fixado; `EntityQuasar`; `onUpdate` contamina com digamma (sem sistema) |
| `ntm:peas` | ItemPeas | parcial | ItemPeas, ModItems.java:3635: despawns EntityQuackos in 50 blocks, which the port does not have | `EntityQuackos` (entidade de mod) |
| `ntm:pellet_antimatter` | ItemDrop | dependente | `ItemDrop`, ModItems.java:2721 | `onEntityItemUpdate` (efeito ao cair no chão): sem gancho de tick de item solto (`ItemEntity.tick`, `onGround`, remover/trocar) no motor fixado; `ExplosionVNT(20).makeAmat()` |
| `ntm:pill_iodine` | ItemPill | parcial | ItemPill, ModItems.java:2834: removes the bad effects; HbmPotion.radiation is not an effect in the port | `HbmPotion.radiation` não existe (idem med_bag) |
| `ntm:pill_red` | ItemPill | parcial | ItemPill, ModItems.java:2836: Astolfization one hour | o `PermaSyncHandler` só desenha uma marca no cliente; o efeito `ntm:death` existe, passivo, 1 hora |
| `ntm:plan_c` | ItemPill | ativo | ItemPill, ModItems.java:2835: ten attackEntityFrom(euthanizedSelf, 1000); armor only is bypassed, Resistance still absorbs: minecraft:magic | - |
| `ntm:plastic_bag` | ItemPlasticBag | dependente | `ItemPlasticBag`, ModItems.java:3358 | menu de inventário ligado à pilha (`InventoryPlasticBag`, 1 slot) |
| `ntm:powder_fertilizer` | ItemFertilizer | ativo | `ItemFertilizer.onItemUse` | a célula clicada cresce sempre e as outras sorteiam o seu próprio sucesso, como `fertilize(..., force)`; o motor só expõe `growCrop` (com o sorteio dentro), então a central o repete até a planta mudar (mudas, azaleias, propágulos, cogumelos e fungos), no máximo 64 vezes; o sorteio do crescimento usa o acaso do nível, não o de `world.rand`; provado em jogo: a muda clicada mudou nas 6 tentativas (avanço de estágio ou árvore) e gastou uma pilha cada, enquanto o bone meal vanilla mudou 5 de 10. Dependência nominal: API de crescimento forçado no motor (próxima frente), que tornaria exato o que hoje é a repetição de `growCrop` |
| `ntm:prototype_kit` | ItemStarterKit | parcial | ItemStarterKit.onItemRightClick prototype_kit, ModItems.java:3793 | itens Java ausentes do port: `ModBlocks.nuke_prototype x1`, `ModItems.rod_quad[URANIUM] x4`, `ModItems.rod_quad[LEAD] x4`, `ModItems.rod_quad[NP237] x2` |
| `ntm:pudding` | ItemLemon | ativo | ItemLemon(6, 1.0F), ModItems.java:3631 | - |
| `ntm:radx` | ItemPill | ativo | ItemPill, ModItems.java:2828: Rad-X potion 3 minutes | - |
| `ntm:rag` | ItemRag | parcial | ItemRag.onItemRightClick, ModItems.java:2398: the rag_piss is added with the result ignored | `onEntityItemUpdate` (rag na água vira `rag_damp`): sem gancho de tick de item solto no motor fixado |
| `ntm:schnitzel_vegan` | ItemSchnitzelVegan | ativo | ItemSchnitzelVegan(0, true), ModItems.java:3612: setFire(5 * 20) is 100 seconds, motionY = 2 | - |
| `ntm:singularity` | ItemDrop | dependente | `ItemDrop`, ModItems.java:2715 | `onEntityItemUpdate` (efeito ao cair no chão): sem gancho de tick de item solto (`ItemEntity.tick`, `onGround`, remover/trocar) no motor fixado; `EntityVortex` |
| `ntm:singularity_counter_resonant` | ItemDrop | dependente | `ItemDrop`, ModItems.java:2717 | `onEntityItemUpdate` (efeito ao cair no chão): sem gancho de tick de item solto (`ItemEntity.tick`, `onGround`, remover/trocar) no motor fixado; `EntityVortex` |
| `ntm:singularity_spark` | ItemDrop | dependente | `ItemDrop`, ModItems.java:2720 | `onEntityItemUpdate` (efeito ao cair no chão): sem gancho de tick de item solto (`ItemEntity.tick`, `onGround`, remover/trocar) no motor fixado; `EntityRagingVortex` |
| `ntm:singularity_super_heated` | ItemDrop | dependente | `ItemDrop`, ModItems.java:2718 | `onEntityItemUpdate` (efeito ao cair no chão): sem gancho de tick de item solto (`ItemEntity.tick`, `onGround`, remover/trocar) no motor fixado; `EntityVortex` |
| `ntm:siox` | ItemPill | parcial | ItemPill, ModItems.java:2829: asbestos and black lung only (HbmLivingProps), not in the port | `HbmLivingProps.asbestos` e `blackLung` (sistema de doenças do pulmão) |
| `ntm:solinium_kit` | ItemStarterKit | parcial | ItemStarterKit.onItemRightClick solinium_kit, ModItems.java:3796 | itens Java ausentes do port: `ModBlocks.nuke_solinium x1` |
| `ntm:spongebob_macaroni` | ItemLemon | ativo | ItemLemon(5, 1.0F), ModItems.java:3627 | - |
| `ntm:static_sandwich` | ItemLemon | ativo | ItemLemon(6, 1.0F), ModItems.java:3630 | - |
| `ntm:stealth_boy` | ItemStarterKit | ativo | ItemStarterKit.onItemRightClick stealth_boy, ModItems.java:2837 | - |
| `ntm:syringe_metal_medx` | ItemSyringe | parcial | ItemSyringe.onItemRightClick syringe_metal_medx, ModItems.java:2821 | sem gancho de ataque de item sobre mobs: só o uso em si mesmo está portado |
| `ntm:syringe_metal_psycho` | ItemSyringe | parcial | ItemSyringe.onItemRightClick syringe_metal_psycho, ModItems.java:2822 | sem gancho de ataque de item sobre mobs: só o uso em si mesmo está portado |
| `ntm:syringe_metal_stimpak` | ItemSyringe | parcial | ItemSyringe.onItemRightClick syringe_metal_stimpak, ModItems.java:2820 | sem gancho de ataque de item sobre mobs: só o uso em si mesmo está portado |
| `ntm:syringe_metal_super` | ItemSyringe | parcial | ItemSyringe.onItemRightClick syringe_metal_super, ModItems.java:2823 | sem gancho de ataque de item sobre mobs: só o uso em si mesmo está portado |
| `ntm:syringe_taint` | ItemSyringe | parcial | ItemSyringe.onItemRightClick syringe_taint, ModItems.java:2824: no sickness check; both containers are added, none replaces the slot | sem gancho de ataque de item sobre mobs: só o uso em si mesmo está portado; `HbmPotion.taint.performEffect` (a cada 2 ticks, com chance 1/40 por tentativa: `(amplificador + 1)` de dano `taint`, exceto em `EntityCreeperTainted` e `EntityTaintCrab`; mais a trilha de blocos taint sob `ENTITY_TAINT_TRAILS`) não tem tick de efeito de conteúdo: o efeito `ntm:taint` existe, passivo |
| `ntm:tsar_kit` | ItemStarterKit | ativo | ItemStarterKit.onItemRightClick tsar_kit, ModItems.java:3789 | - |
| `ntm:twinkie` | ItemLemon | ativo | ItemLemon(3, 0.25F), ModItems.java:3629 | - |
| `ntm:xanax` | ItemPill | parcial | ItemPill, ModItems.java:2831: digamma -0.5 only (HbmLivingProps), not in the port | `HbmLivingProps.digamma` (sistema digamma do jogador) |

### Itens fora dos 116, adicionados nesta onda

| Item | Estado | Origem Java | Dependências nominais |
|---|---|---|---|
| `ntm:geiger_counter` | ativo | `ItemGeigerCounter` | relatório de 5 linhas e cliques a cada 5 ticks pelo `radBuf` do servidor de radiação; "radiação do chunk" é a taxa das zonas de fallout do port (`ChunkRadiationManager` não existe); texto em inglês |
| `ntm:dosimeter` | ativo | `ItemDosimeter` | idem, teto de 3,6 RAD/s |
| `ntm:digamma_diagnostic` | parcial | `ItemDigammaDiagnostic` | `HbmLivingProps.digamma` não existe: imprime 0 |
| `ntm:survey_scanner` | parcial | `ItemSurveyScanner` | marcadores de partícula do minério de bedrock, `TileEntityBedrockOre` e a viagem ao End com `entanglement_kit` |
| `ntm:pipette, ntm:pipette_boron, ntm:pipette_laboratory` | parcial | `ItemPipette` | capacidade por clique direito portada; encher e esvaziar exige o slot de fluido de máquinas (`IFillableItem`) |
| `ntm:inf_water, ntm:inf_water_mk2, ntm:fluid_barrel_infinite, ntm:chlorine_pinwheel` | dependente | `ItemInfiniteFluid` | `FluidLoaderInfinite` (o slot de tanque das máquinas que os usa) |
| `ntm:gun_kit_1, ntm:gun_kit_2` | dependente | `ItemRepairKit` | repara armas: onda de armas (só identidade) |
| `ntm:ingot_meteorite, ntm:ingot_meteorite_forged, ntm:blade_meteorite, ntm:ingot_steel_dusted, ntm:ingot_chainsteel` | parcial | `ItemHot` | esfriamento de 1 por tick e o fim da tag portados; `heatUp` vem das receitas de bigorna e fundição com NBT (não portadas); no Java o `heat` é estático e vale 100 para todos |

### Sons do HBM usados e seus substitutos

| Item(ns) | Som Java | Substituto vanilla | Volume / tom |
|---|---|---|---|
| `ntm:cotton_candy`, `ntm:mucho_mango`, `ntm:radx`, `ntm:siox`, `ntm:pill_herbal`, `ntm:xanax` e mais 33 | `random.burp` | `minecraft:entity.player.burp` | 0.5 / 0.9 + rand*0.1 |
| `ntm:iv_xp` | `random.orb` | `minecraft:entity.experience_orb.pickup` | 1.0 / 1.0 |
| `ntm:syringe_metal_medx`, `ntm:syringe_metal_stimpak`, `ntm:syringe_metal_psycho`, `ntm:syringe_metal_super`, `ntm:syringe_taint`, `ntm:syringe_antidote` e mais 4 | `hbm:item.syringe` | `minecraft:entity.arrow.hit_player` | 1.0 / 1.0 |
| `ntm:radaway`, `ntm:radaway_strong`, `ntm:radaway_flush`, `ntm:iv_blood` | `hbm:item.radaway` | `minecraft:item.bottle.fill` | 1.0 / 1.0 |
| `ntm:nuke_starter_kit`, `ntm:nuke_advanced_kit`, `ntm:nuke_commercially_kit`, `ntm:nuke_electric_kit`, `ntm:gadget_kit`, `ntm:boy_kit` e mais 13 | `hbm:item.unpack` | `minecraft:item.bundle.drop_contents` | 1.0 / 1.0 |
| `ntm:bdcl` (nos ticks 0, 5, 10, 15, 19 e 26 do uso: `count % 5 == 0 && count >= 10`) | `hbm:player.gulp` | `minecraft:entity.generic.drink` | 1.0 / 1.0 |
| `ntm:bdcl` | `hbm:player.groan` | `minecraft:entity.player.breath` | 1.0 / 1.0 |
| `ntm:cigarette`, `ntm:crackpipe` | `hbm:player.cough` | `minecraft:entity.panda.sneeze` | 1.0 / 1.0 |
| `ntm:med_ipecac`, `ntm:med_ptsd` | `hbm:entity.vomit` | `minecraft:entity.fox.spit` | 1.0 / 1.0 |
| - | `hbm:item.techBoop` | `minecraft:block.note_block.bit` | 1.0 / 1.0 |
| - | `hbm:item.geiger1` | `minecraft:block.lever.click` | 1.0 / 1.0 |
| - | `hbm:item.geiger2` | `minecraft:block.lever.click` | 1.0 / 1.1 |
| - | `hbm:item.geiger3` | `minecraft:block.lever.click` | 1.0 / 1.2 |
| - | `hbm:item.geiger4` | `minecraft:block.lever.click` | 1.0 / 1.3 |
| - | `hbm:item.geiger5` | `minecraft:block.lever.click` | 1.0 / 1.4 |
| - | `hbm:item.geiger6` | `minecraft:block.lever.click` | 1.0 / 1.5 |

<!-- k2-status:end -->
