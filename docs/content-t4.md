# T4 — catálogo estático restante e receitas `skipped`

Branch `front-catalog`, a partir de `main @ 85d06d9c`.

## O que entrou

- **16 itens de combustível gasto** (`ItemDepletedFuel`) como identidades estáticas (`tools/content-gen/catalog_t4.py`): `waste_natural_uranium`, `waste_uranium`, `waste_thorium`, `waste_mox`, `waste_plutonium`, `waste_u233`, `waste_u235`, `waste_schrabidium`, `waste_zfb_mox` e as 7 `waste_plate_*`. Consumo e decaimento ficam como `behavior_gap`.
- **43 blocos de forma não cúbica** com o template vanilla mais próximo, cada um com `behavior_gap` dizendo o que não é reproduzido:
  - 24 `deco_pipe*` (`minecraft:iron_bars`);
  - 5 `ladder_*` (`minecraft:ladder`, escalada vanilla);
  - 6 `barbed_wire*` (`minecraft:iron_bars`, textura do ícone, sem dano por contato);
  - 6 barris `red_barrel`, `pink_barrel`, `yellow_barrel`, `vitrified_barrel`, `lox_barrel`, `taint_barrel` (cubo de ferro);
  - `cm_flux`, `cm_heat` (cubo de ferro);
  - não entram: `cm_block`, `cm_sheet`, `cm_engine`, `cm_circuit`, `cm_tank` (variantes `EnumCMMaterials`), `foam_layer`, `sand_boron_layer`, `leaves_layer`, `oil_spill` (camadas, ver lacunas).
- **Metadata 0 em blocos sem subtipo** (`bulk_recipes.py`, `Resolver.row`): um bloco com identidade única é `ItemStack(x, n, 0)` no Java.
- **Shredder: pó sem registro de origem** (`shredder_recipes.py`): `getDustByName` retorna scrap, e `putIfValid` não roda para scrap. Esses 147 casos viram `no_output` com o motivo, em vez de `skipped`.
- **Auditoria do bulk** (`bulk.py`): o conjunto `claimed` agora tem só `m3.json` e `bombs.json`. `item_hooks.json` só sobrescreve linhas-base. Variáveis cobertas por essas declarações saem da lista de lacunas e entram em `claimed_elsewhere`.

## Contagens (main `85d06d9c` → esta branch)

| Medida | Antes | Depois |
|---|---|---|
| `ids.json` entradas | 3333 | 3392 (+59: 16 combustíveis e 43 blocos) |
| `anvil` construções: emitidas / puladas | 161 / 96 (fonte: 118 pulados no total) | 167 / 90 (total pulado 112) |
| `purex` fontes puladas | 22 | 5 |
| `purex` receitas | 37 | 54 |
| `assembly` receitas / puladas | 500 / 131 | 502 / 129 |
| `shredder` receitas / puladas / `no_output` | 734 / 193 / 0 | 759 / 21 / 147 |
| `bulk-recipe-coverage` receitas / gaps | 931 / 1025 | 1151 / 988 |
| `bulk-coverage` gaps / `claimed_elsewhere` | 944 / — | 689 / 255 |

A queda de `bulk-coverage` tem duas causas. Uma é a regra de `claimed_elsewhere`, que não é conteúdo novo. A outra são os 59 itens declarados nesta frente.

Notas sobre `bulk-recipe-coverage`: as 220 receitas novas (`data/ntm/recipe/bulk__*.json`) estão em `content/data/ntm/recipe/`, território da T3, e não foram copiadas. O `generate.py` do merge cria esses arquivos.

## Provas

- `tools/content-gen`: 275 testes rodaram e passaram. Contratos de `anvil`, `purex` e `shredder` atualizados. `test_catalog_t4.py` (4 testes) cobre os 16 itens, os 43 blocos e o lookup de metadata 0. `test_rbmk_oracle` passou a exigir só que as entradas do T00 continuem idênticas e na mesma ordem relativa.
- Estabilidade: `bulk.py` rodado duas vezes dá o mesmo resultado; `bulk.py` seguido de `generate.py --ids-only` deixa `ids.json` e `machines_backlog.json` iguais ao HEAD.
- Pacote: `cargo-horizon-mod build` e `check` com o build compartilhado geram `ntm.hmod`. O cliente compartilhado carrega o pacote, entra no mundo, e o log não tem `reject` nem `panic`. Antes do ajuste final, o log acusou modelos ausentes nas camadas (removidas) e texturas ausentes nos barris e no arame farpado (corrigidas).

## Revisão independente (2026-11)

Revisão aprovada com ressalvas (sem P0/P1). Correções aplicadas:

- **P2-1:** `block_properties` vem do `ModBlocks.java` por bloco (`catalog_t4.java_block_properties`): dureza, resistência, som (`ModSoundTypes.grate` vira `minecraft:iron_bars`; escadas `minecraft:ladder`; o resto `minecraft:stone`) e `requires_tool` (escadas `false`, por `BlockLadder` usar `Material.circuits`; o resto `true`, por `Material.iron`).
- **P2-2:** `test_bulk_idempotence.py` roda `bulk.py` em cópia, duas vezes, e exige bytes iguais aos commitados. Com `item_hooks.json` de volta em `claimed`, o teste falha (verificado).
- **P3-1:** `test_catalog_t4.ShapedBlockIdentities` compara com o `ModBlocks.java` lido de novo pelo teste, sem usar `catalog_t4`.
- **P3-2:** os 7 `waste_plate_*` têm `creative_hidden` por `setCreativeTab(null)` no Java.
- **P3-3:** `cm_port` e `custom_machine` registrados como lacunas acima.
- **P3-4:** captura dos 43 blocos no jogo: pendente, com a máquina em janela de medição.

## Lacunas

- **Falha da T1 resolvida:** o contrato de `test_rbmk_oracle` foi relaxado, como acima.
- `stamp_book` (`ItemStampBook`): páginas em NBT. Requer lógica de item.
- Camadas (`foam_layer`, `sand_boron_layer`, `leaves_layer`, `oil_spill`): a altura vem do metadata. Precisa de um conjunto de modelos por camada, que o gerador ainda não produz. Sem isso, o cliente cai no modelo ausente.
- `cm_block`, `cm_sheet`, `cm_engine`, `cm_circuit`, `cm_tank`: subtipos `EnumCMMaterials`, um nome de idioma por material. Precisam de subtipos de metadata.
- `cm_anchor` (`custom_machine_anchor`): textura e modelo próprios, sem declaração direta.
- `cm_port` (`BlockCMPort`, com `EnumCMMaterials` como as outras variantes CM) e `custom_machine` (`BlockCustomMachine`, sem textura própria declarada): não declarados.
- `sand_mix` (purex, 3 fontes): a fonte não declara textura, e não existe `sand_mix.png` no `ntm-src`.
- `anvil`: `siren_track:1..15`, `machine_stirling:1`, `machine_stirling_steel:1` (subtipos de metadata); receitas com NBT por item (`AnvilSmithingHotRecipe` etc.).
- `shredder`: 21 pulados restantes, de `EnumDriveType` dinâmico, `boxcar`/`steel_*`/`crate_*` ambíguos e `sellafield` com subtipo.
- `centrifuge` (23): laços dinâmicos sobre `BedrockOreType`/`EnumBedrockOre`.
- `chemical_plant` (15): inteiros não literais (`GeneralConfig`) e fluidos condicionais.

## Como reproduzir

```sh
cd tools/content-gen
python3 bulk.py                       # auditoria (idempotente)
python3 generate.py --ids-only        # ids, machines_backlog (m3 anota)
HORIZON_RESOURCES=/home/sacer/Games/horizon/.local/horizon-client/26.3 python3 generate.py --output <pasta_scratch>
HORIZON_RESOURCES=... python3 -m unittest discover -s . -p 'test_*.py'
```
