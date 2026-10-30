# Catálogo em massa K1: decorativos, blocos de mundo e iluminação

Entram 84 IDs (84 blocos, cada um com seu item de bloco), levando o catálogo a 3.075 (593 blocos e 2.482 itens), já com o R4 publicado.

## Reconhecimento genérico de classes (bulk.py)

O importador em massa só aceitava classes de uma lista fixa (`STATIC_BLOCKS`). Agora, para as demais, aplica um mapeamento classe → forma, sem casos isolados:

- **Cubo cheio** (`static_cube`): uma classe de bloco sem container e sem nenhum override de forma (`isOpaqueCube`, `renderAsNormalBlock`, `getRenderType`, `setBlockBounds`, `getCollisionBoundingBoxFromPool`, `getSelectedBoundingBoxFromPool`, `getBlocksMovement`) em toda a sua ancestralidade entra como cubo. Os hooks de comportamento que ela tem (`updateTick`, `onBlockActivated`, `onNeighborBlockChange`, `getItemDropped`, `isBeaconBase`...) viram `behavior_gap` explícito, em vez de motivo para rejeitar. Os blocos que caem (`BlockFalling` na ancestralidade) usam o template `minecraft:gravel`.
- **Faces por sufixo**: se existem `<textura>_side`, `_top` ou `_bottom`, o bloco usa `surface_textures` (lado/topo/fundo), como o `BlockPillar` já fazia.
- **Enum multi** (`BlockEnumMulti`, `BlockEnumMultiCT`, `BlockCap`, `BlockLightstone`, `BlockNTMSand`): o construtor de enum é lido da classe ou da própria declaração (`new BlockEnumMulti(Material, X.class, a, b)`); a textura aceita o separador `.` ou `_` e a face de topo por `_top`.
- **Degraus**: qualquer subclasse de `BlockGenericStairs` (por exemplo `BlockSpeedyStairs`) vira `minecraft:stone_stairs`.
- **Outras formas continuam auditadas**: portas, trilhos, painéis, cercas, escadas, lajes, fluidos, gases, fogo, plantas, cipós, arame farpado, espinhos, camadas, estruturas, glifídeos, fechaduras, barris explosivos, detonáveis, containers (BE), e as classes `Machine*`, `Bomb*`, `RBMK*`, `PWR*`, `Watz*`. Também ficam de fora, por família, `cm_*`, `fusion_*`, `icf_*`, `hadron_*`, `det_*` e demais famílias dos próximos lotes.

Estabilidade: as linhas que já estavam em `bulk.json` não mudam na regeneração (ficam congeladas), e nomes já declarados em `m3.json` ou `bombs.json` nunca são reclamados de novo. A regeneração acrescentou 84 entradas e nenhuma foi removida nem alterada; `watz_end` e as demais já declaradas pelo R4 saem do alcance do K1.

## O que entrou

| Família | IDs (sem prefixo `ntm:`) |
| --- | --- |
| Mundo e decorativos | `stone_depth`, `stone_depth_nether`, `depth_brick`, `depth_tiles`, `depth_nether_brick`, `depth_nether_tiles`, `depth_dnt`, `stone_porous`, `block_meteor`, `_broken`, `_cobble`, `_molten`, `block_meteor_treasure`, `frozen_dirt`, `frozen_planks`, `waste_planks`, `impact_dirt`, `mush_block`, `mush_block_stem`, `sellafield`, `sellafield_bedrock`, `sellafield_slaked`, `waste_trinitite`, `waste_trinitite_red`, `block_slag`, `ash_digamma`, `asphalt`, `asphalt_light`, `asphalt_stairs`, `deco_titanium`, `deco_asbestos`, `brick_asbestos`, `brick_concrete_marked`, `brick_forgotten`, `brick_jungle_fragile`, `_mystic`, `_ooze`, `tile_lab`, `_broken`, `_cracked`, `ancient_scrap`, `block_corium_cobble` |
| Resíduos e perigos (cubos) | `block_waste`, `block_waste_painted`, `block_waste_vitrified`, `block_fallout`, `block_yellowcake`, `block_red_phosphorus` |
| Caixas | `crate`, `crate_lead`, `crate_metal`, `crate_red`, `crate_weapon`, `crate_jungle` |
| Iluminação | `lamp_tritium_green_on/off`, `lamp_tritium_blue_on/off`, `reinforced_lamp_on/off` |
| Enum multi | `platemetal__<15 cores>`, `lightstone__<5>`, `vinyl_tile__small/large`, `block_cap__<nuka, quantum, sparkle, rad, korl, fritz>` |

Os blocos com subtipo ficam com IDs separados (`<nome>__<subtipo>`), como as demais famílias de metadata.

## Lacunas

- Nada disso tem a lógica de origem: propagação e queda de gás/radiação dos blocos de resíduos e de `sellafield`, acendimento e emissão de luz das lâmpadas e a troca on/off, queda e abertura das caixas, minas, crescimento, saída de itens e o resto dos hooks listados em cada `behavior_gap`.
- As caixas (`BlockCrate`) herdam de `BlockFalling` no fonte; aqui só carregam o template do gravel, sem o inventário.
- `sand_mix` (cinco tipos de areia) fica no audit: o código da classe nomeia as texturas `sand_<tipo>` (`sand_boron.png`...), um terceiro padrão que o resolvedor de sufixos não cobre; entra quando o gerador aprender a ler o prefixo na própria classe.
- Os itens "colecionáveis" com block entity (`BlockBobble`, `BlockSnowglobe`, `BlockPlushie`), os holofotes e as lâmpadas animadas (`Spotlight*`, `Floodlight`, `DemonLamp`) têm forma ou BE e ficam para os lotes de máquinas Inert.

## Efeito nas tabelas

Com os 84 IDs, receitas que estavam em `skipped` por falta de item passaram a resolver: montadora +7 (469; 462 antes), bigorna +6 construções (142), centrífuga +1, planta química +1, triturador +4, e `machine_arc_furnace_large.json` ganhou duas entradas de insumo (`ash_digamma`, `block_slag`). Hazards: +84 identidades, 9 com taxa de radiação ou outro hazard (`block_waste` 150 RAD/s, `ancient_scrap` e `block_corium_cobble` 150, `block_waste_vitrified` 75, `block_fallout` e `block_yellowcake` 10,5; asbesto em `brick_asbestos` e `tile_lab_broken`); nenhuma taxa antiga mudou. Buckets regenerados.

## Prova e validação

`verify-catalog-batch.py` (novo, genérico para os próximos lotes de catálogo): coloca cada bloco numa grade e confere o `block` correspondente, depois entrega cada item pelo inventário, no binário nativo, mudo, com detector de rejeição. Resultado do K1: 84 IDs, 276 conferências, sem `reject` nem `panic` no log do cliente. 205 testes por seed 1/42 (inclui `test_bulk_forms.py`), `validate-bulk` e `recipe-gen/validate` verdes. Pacote `ntm.hmod` SHA-256 `c67456a407f4704216ec3c2cb7190ceace28e5d4ff6aa79e328dc3662c70ece8` (sobre o R4), aprovado no check do upstream e do fork.
