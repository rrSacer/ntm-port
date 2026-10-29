# Conteúdo e receitas do PUREX

`ntm:machine_purex` (`ModBlocks.java:2218,3259`) e `content/machine-recipes/purex.json` (schema 1) exportam o bloco e a tabela completa de `PUREXRecipes.java` no contrato compartilhado com o agente de lógica (`tools/machine-gen/build.py` → `crates/ntm-core/data/machines/purex.json`) e com o agente visual (`tools/visual-gen/generate.py`), sem rede nem execução de Java.

## Bloco `ntm:machine_purex` e montagem (`ass.purex`)

O bloco `ntm:machine_purex` entra em `content/ids.json` (e em `ids.json.machines`, elevando o catálogo de 2.563 para **2.564 IDs**: 364 blocos, 2.200 itens, 34 máquinas) com o mesmo contrato de estado da planta química (`ntm:machine_chemical_plant`):

- **Declaração (`content/data/ntm/horizon/block/machine_purex.json`):** `template: "minecraft:beehive"`, `item: true`, `hardness: 5.0`, `explosion_resistance: 10.0`, `requires_tool: true`, `sounds: "minecraft:iron_block"`, `light: 0`.
- **Barramento lógico (`ids.json`):** `{"kind": "machine", "facing": "facing", "active": null, "level": {"prop": "honey_level", "max": 5}}` (24 variantes em `assets/ntm/blockstates/machine_purex.json`, cubo opaco de reserva `machine_purex_0..5` sobre `block_steel` com frente `models/machines/purex`, item model e auto-drop em `data/ntm/loot_table/blocks/machine_purex.json`).
- **Idiomas e aba criativa:** `block.ntm.machine_purex = "PUREX"` em `en_us` e `pt_br` (`en_US.lang:5891`, `pt_BR.lang:5843`), listado em `data/ntm/creative_tab/machines.json` (`itemGroup.ntm.machines`).
- **Propriedade visual respeitada:** caminhos reivindicados em `content/.visual-files.json` (`horizon_machine`, `horizon_screen`, `horizon_model`, `gui_purex.png`, `dummy_machine`) permanecem intocados pelo `content-gen`.
- **Receita na montadora (`content/machine-recipes/assembly.json`):** com o bloco catalogado e todos os ingredientes já presentes no catálogo, `ass.purex` (`AssemblyMachineRecipes.java:270`, 300 ticks, 100 HE/t) passa automaticamente de `skipped` para `recipes` (elevando a montadora de 399 para **400 receitas emitidas** e reduzindo `skipped` de 232 para 231):
  - `4× ntm:shell__steel` (`STEEL.shell()`), `8× ntm:pipe__rubber` (`RUBBER.pipe()`), `4× ntm:plate_cast__lead` (`PB.plateCast()`), `1× ntm:motor_desh` e `4× ntm:circuit__basic` → `1× ntm:machine_purex`.

## Cobertura e ordem da fonte (`content/machine-recipes/purex.json`)

| Métrica | Valor |
|---|---:|
| Chamadas `this.register(...)` em `PUREXRecipes.registerDefaults()` | 59 |
| Receitas emitidas em `recipes` | 22 |
| Receitas omitidas em `skipped` | 37 |

`source_index` (`0..58`) preserva a ordem exata de `PUREXRecipes.registerDefaults()`. Cada receita emitida segue o mesmo schema 1 de `chemical_plant.json`, acrescido de `source_index` e `autoSwitchGroup` (string ou `null`), respeitando os limites do `PUREXRecipes` (`<= 3` `inputItems` na ordem dos slots, `<= 3` `inputFluids` na ordem dos tanques, `<= 6` `outputItems` `[id, count, chance]` com `1.0` para 100%, e `0..1` `outputFluids`).

Todos os 13 fluidos referenciados em `PUREXRecipes.java` correspondem aos identificadores exatos de `crates/ntm-core/data/fluids.json`: `amat`, `helium4`, `hydrogen`, `kerosene`, `nitric_acid`, `schrabidic`, `solvent`, `sulfuric_acid`, `thorium_salt`, `thorium_salt_depleted`, `wastefluid`, `wastegas` e `watz`.

### Contagem por família

| Família | `autoSwitchGroup` | Emitidas | `skipped` | Resumo e motivo das omissões |
|---|---|---:|---:|---|
| Síntese (`synthesis`) | `null` | 3 | 0 | `purex.uzh` (`billet_uranium_fuel` + `3× billet_zirconium` + `1000 mB nitric_acid` + `4000 mB hydrogen` → `4× billet_uzh`), `purex.flashgold` (`billet_au198` + `pellet_charged` + `1000 mB amat` → `2× billet_balefire_gold`) e `purex.flashlead` (`billet_pb209` + `billet_balefire_gold` + `1000 mB amat` → `1× billet_flashlead`), todas 600 ticks a 1.000 HE/t. |
| Chicago Pile (`pile`) | `autoswitch.pile` | 0 | 4 | `ModItems.pile_rod` (`ItemPileRodMK2`) ausente do catálogo. |
| ZIRNOX (`zirnox`) | `autoswitch.zirnox` | 0 | 9 | Combustíveis gastos ZIRNOX (`ModItems.waste_*`, `ItemDepletedFuel`) ausentes do catálogo. O extrator preserva saídas duplicadas separadas (como as duas entradas `nuclear_waste_tiny` de `purex.zirnoxles` nas linhas 158–159, verificadas em `test_purex.py`). |
| Placas de pesquisa (`plate`) | `autoswitch.plate` | 0 | 7 | Placas gastas (`ModItems.waste_plate_*`, `ItemDepletedFuel`) ausentes do catálogo. |
| Reator PWR (`pwr`) | `autoswitch.pwr` | 15 | 0 | Todas as 15 hastes `ntm:pwr_fuel_depleted__*{meu,heu233,heu235,men,hen237,mox,mep,hep239,hep241,mea,hea242,hes326,hes327,bfb_am_mix,bfb_pu241}` + `500 mB kerosene` + `250 mB nitric_acid` (100 ticks, 2.500 HE/t), com rótulos `Reprocessamento de ...` / `Reprocessing of ...`. |
| Sal fundido (`molten_salt`) | `null` | 1 | 0 | `purex.thoriumsalt`: `2× ntm:nugget_th232` (`TH232.nugget()`) + `16000 mB thorium_salt_depleted` → `16000 mB thorium_salt` + `ChanceOutput` independentes `['ntm:nugget_u233', 1, 0.5]` e `['ntm:nuclear_waste_tiny', 1, 0.25]` (20 ticks, 10.000 HE/t). |
| Reator Watz (`watz`) | `autoswitch.watz` | 0 | 12 | 10 pastilhas `ModItems.watz_pellet_depleted` (`ItemWatzPellet`) ausentes do catálogo + 2 receitas condicionais (`purex.watznaqadah`, `purex.watznaqadria`, linhas 474–497) bloqueadas pela guarda `if(naquadriaNuggets.size() != 0)` porque `nuggetNaquadria` não existe no OreDict do perfil padrão (sem mods externos). |
| Fusão ICF (`icf`) | `null` | 1 | 0 | `purex.icf`: `ntm:icf_pellet_depleted` → `ntm:icf_pellet_empty` + `ntm:pellet_charged` + `ntm:powder_iron` + `1250 mB helium4` (300 ticks, 10.000 HE/t). |
| Vitrificação (`vitrification`) | `null` | 0 | 3 | `purex.vitliquid`, `purex.vitgaseous` e `purex.vitsolid` requerem `ModBlocks.sand_mix` (`BlockNTMSand`, `EnumSandType.LEAD`), ausente do catálogo. |
| Extração de Schrabídio (`schrabidium`) | `autoswitch.schrab` / `null` | 2 | 2 | Emitidas `purex.schrabpwr` (`pwr_fuel_depleted__mep`) e `purex.schrabmen` (`pwr_fuel_depleted__men`) com `4000 mB solvent` + `250 mB schrabidic` → `powder_schrabidium` + `3× nugget_technetium` + `4× nuclear_waste_tiny` (200 ticks, 50.000 HE/t, grupo `autoswitch.schrab`). Omitidas: `purex.schraranium` (`ModItems.ingot_schraranium` ausente) e `purex.schrabzirnox` (`ModItems.waste_plutonium` ausente). |

## Reprodução e validação

```sh
/usr/local/bin/agent-run python3 tools/content-gen/generate.py
/usr/local/bin/agent-run python3 tools/content-gen/test_purex.py
/usr/local/bin/agent-run python3 tools/content-gen/test_press.py
/usr/local/bin/agent-run python3 tools/content-gen/test_anvil.py
/usr/local/bin/agent-run python3 tools/content-gen/test_foundry.py
/usr/local/bin/agent-run python3 tools/content-gen/test_blast_furnace.py
/usr/local/bin/agent-run python3 tools/content-gen/test_shredder.py
/usr/local/bin/agent-run python3 tools/content-gen/test_m3.py
/usr/local/bin/agent-run python3 tools/content-gen/test_ownership.py
/usr/local/bin/agent-run python3 tools/content-gen/validate-bulk.py
/usr/local/bin/agent-run python3 tools/recipe-gen/validate.py
```
