# Conteúdo e receitas da bigorna

`content/machine-recipes/anvil.json` (schema 1) exporta a tabela da bigorna NTM (`AnvilRecipes.java` e `NTMAnvil.java`) no formato combinado com o agente de máquinas (`tools/machine-gen/build.py` → `crates/ntm-core/data/machines/anvil.json`), sem rede nem execução de Java.

## Cobertura e ordem da fonte

| Seção | Chamadas na fonte | Expandidas | Emitidas | `skipped` |
|---|---:|---:|---:|---:|
| `smithing` | 41 | 58 | 36 | 22 |
| `construction` | 177 | 257 | 103 | 154 |

`source_index` preserva a ordem global base zero da lista Java após a expansão finita dos laços (`Block anvil : anvils`, intervalos inteiros e `NTMMaterial mat : Mats.orderedList`), incluindo as entradas omitidas em `skipped`. O perfil usa a configuração padrão do port (`enableLBSM = false`, `enableLBSMUnlockAnvil = false`, `enableExpensiveMode = false`, `enable528 = false`, `external_mods = []`), de modo que os limites `tier`, `tier_lower` e `tier_upper` permanecem ativos.

### Forja (`smithing`)

Cada entrada emitida contém `name`, `source_index`, `source`, `tier`, `left`, `right`, `out`, `shapeless`, `consume: [L, R]` e `kind` (`"plain"` ou `"mold"`):

- **18 melhorias de bigorna (`anvil.smithing.0..17`, nível 1):** `ntm:anvil_iron` (`0..8`) ou `ntm:anvil_lead` (`9..17`) na esquerda (gasto 1) + 10 lingotes na direita (gasto 10) → `anvil_steel`, `anvil_desh`, `anvil_saturnite`, `anvil_ferrouranium`, `anvil_bismuth_bronze`, `anvil_arsenic_bronze`, `anvil_schrabidate`, `anvil_dnt` e `anvil_osmiridium`. Na fonte original não há receita 3×3 de bancada para esses nove níveis (1 + 10 = 11 itens).
- **Gunmetal (`anvil.smithing.38`, nível 1):** lingote de cobre + lingote de alumínio → `ntm:ingot_gunmetal` (`consume: [1, 1]`).
- **17 gravações de molde (`anvil.smithing.39..55`, `AnvilSmithingMold`, nível 1):** `ntm:mold_base` na direita (`consume: [0, 1]`), slot esquerdo não consumido e com quantidade exata (`"exact": true`). Nas 14 gravações por prefixo OreDict (`nugget`, `billet`, `ingot` ×1/×9, `plate` ×1/×9, `plateTriple` ×1/×3, `wireFine`, `shell`, `ntmpipe`, `block`, `wireDense` ×1/×9), a resolução replica a ordem de curto-circuito de `AnvilSmithingMold.matches`: qualquer chave OreDict do item que comece com um prefixo mais longo de `MaterialShapes` (`plateTriple`, `plateSextuple`, `wireDense` etc.) rejeita o item antes do `startsWith`. As outras três gravações (`mold__blade`, `mold__blades`, `mold__stamp`) usam lista explícita de itens. Somadas aos 9 moldes de `registerConstructionAmmo` (`c9`, `c50`, canos, receptores, mecanismo, coronha e empunhadura), cobrem todos os **26 moldes** (`ntm:mold__*`) de `foundry.json`.
- **22 omissões documentadas em `skipped`:** 18 `AnvilSmithingHotRecipe` (dependem de calor por item em NBT), `AnvilSmithingCyanideRecipe` e `AnvilSmithingRenameRecipe` (estado/nome em NBT), além de duas receitas com itens fora do catálogo (`wings_limp` e `bottle_nuka`).

### Construção (`construction`)

Cada entrada emitida contém `name`, `source_index`, `source`, `in: [{ids, count}]`, `out: [[id, count, chance]]`, `tier_lower`, `tier_upper` (`-1` quando sem teto) e `overlay` (`NONE`, `CONSTRUCTION`, `RECYCLING` ou `SMITHING`, determinado pela sobrecarga do construtor e por `.setOverlay(...)` explícito).

As **103 receitas emitidas** cobrem chapas de nível 3, fios finos autogerados de nível 4 (`×8`), carcaças (`shell__*`) e tubos (`pipe__*`, preferindo chapa sobre lingote no snapshot OreDict), motores, bobinas toroidais, matrizes de prensa e munição (`stamp_*`), os 9 moldes de armas/munição, máquinas já catalogadas (`ntm:machine_crucible`, `ntm:heater_firebox`, `ntm:machine_assembly_machine`, `ntm:machine_chemical_plant`, `ntm:machine_centrifuge` etc.) e reciclagens com probabilidades independentes (como `ntm:chunk_ore__rare` e `ntm:heater_firebox`).

As **154 omissões documentadas em `skipped`** correspondem exclusivamente a subsistemas e blocos/itens ainda fora do catálogo: máquinas não portadas, blocos RBMK, varetas da Chicago Pile (`pile_rod`), caixas de dutos e cabos (`fluid_duct_box`, `fluid_duct_exhaust`, `red_cable_box`), trilhas de sirene (`siren_track`), meteorito/starmetal e blocos decorativos com subclasses próprias (`platemetal`, `steel_grate`, `steel_scaffold`, `deco_titanium`, `deco_asbestos`, `gear_large` etc.). Nenhum ID ou receita ponte foi inventado.

## Os 12 blocos de bigorna e receitas de bancada

São **12 IDs novos** em `ids.json` (todos em `ids.json.machines`), elevando o catálogo de 2.551 para **2.563 IDs** (363 blocos, 2.200 itens, 33 máquinas):

| ID | Nível (`tier`) | Nome (`pt_br` / `en_us`) | Obtenção |
|---|---:|---|---|
| `ntm:anvil_iron` | 1 | Bigorna de Ferro / Iron Anvil | Bancada (`CraftingManager.java:294`) |
| `ntm:anvil_lead` | 1 | Bigorna de Chumbo / Lead Anvil | Bancada (`CraftingManager.java:295`) |
| `ntm:anvil_steel` | 2 | Bigorna de Aço / Steel Anvil | Forja nível 1 (`anvil_{iron,lead}` + 10 `ingot_steel`) |
| `ntm:anvil_desh` | 3 | Bigorna de Desh / Desh Anvil | Forja nível 1 (`anvil_{iron,lead}` + 10 `ingot_desh`) |
| `ntm:anvil_ferrouranium` | 4 | Bigorna de Ferrourânio / Ferrouranium Anvil | Forja nível 1 (`anvil_{iron,lead}` + 10 `ingot_ferrouranium`) |
| `ntm:anvil_saturnite` | 5 | Bigorna de Saturnite / Saturnite Anvil | Forja nível 1 (`anvil_{iron,lead}` + 10 `ingot_saturnite`) |
| `ntm:anvil_bismuth_bronze` | 5 | Bigorna de Bronze de Bismuto / Bismuth Bronze Anvil | Forja nível 1 (`anvil_{iron,lead}` + 10 `ingot_bismuth_bronze`) |
| `ntm:anvil_arsenic_bronze` | 5 | Bigorna de Bronze de Arsênio / Arsenic Bronze Anvil | Forja nível 1 (`anvil_{iron,lead}` + 10 `ingot_arsenic_bronze`) |
| `ntm:anvil_schrabidate` | 6 | Bigorna de Schrabidato Férrico / Ferric Schrabidate Anvil | Forja nível 1 (`anvil_{iron,lead}` + 10 `ingot_schrabidate`) |
| `ntm:anvil_dnt` | 7 | Bigorna de Dineutrônio / Dineutronium Anvil | Forja nível 1 (`anvil_{iron,lead}` + 10 `ingot_dineutronium`) |
| `ntm:anvil_osmiridium` | 8 | Bigorna de Osmirídio / Osmiridium Anvil | Forja nível 1 (`anvil_{iron,lead}` + 10 `ingot_osmiridium`) |
| `ntm:anvil_murky` | 1916169 | Bigorna Sombria / Murky Anvil | Bancada (`CraftingManager.java:296`, `undefined` + `anvil_steel`) |

Todos os 12 blocos usam `template: "minecraft:beehive"`, `logic: {kind: "machine", facing: "facing", active: null, level: {prop: "honey_level", max: 5}}` (24 variantes de estado), dureza `5.0`, resistência a explosão `100.0`, som `minecraft:anvil`, loot próprio e texturas `blocks/anvil_*` (`anvil_murky` usa `anvil_murky` apenas no topo e `anvil_steel` nas laterais e na base, como `NTMAnvil.getIcon`). `ntm:mold_base` e sua receita de bancada (`CraftingManager.java:928`) já constam no catálogo e permanecem ativos.

**Lacunas explícitas de bloco:** o template `minecraft:beehive` foi adotado para cumprir o contrato uniforme de estado/orientação das máquinas (`facing` + `honey_level`), não incluindo queda por gravidade nem dano de impacto de `BlockFallingNT`. O modelo OBJ estreito (`ResourceManager.anvil` e caixa de colisão `0.75` de altura) e a tela `GUIAnvil` pertencem à frente visual (`tools/visual-gen`).

## Reprodução e validação

```sh
/usr/local/bin/agent-run python3 tools/content-gen/generate.py
/usr/local/bin/agent-run python3 tools/visual-gen/generate.py
/usr/local/bin/agent-run python3 tools/content-gen/test_anvil.py
/usr/local/bin/agent-run python3 tools/content-gen/test_foundry.py
/usr/local/bin/agent-run python3 tools/content-gen/test_blast_furnace.py
/usr/local/bin/agent-run python3 tools/content-gen/test_shredder.py
/usr/local/bin/agent-run python3 tools/content-gen/test_m3.py
/usr/local/bin/agent-run python3 tools/content-gen/validate-bulk.py
```

`test_anvil.py` valida os 12 blocos (níveis, propriedades, texturas opacas, variantes, loot e receitas vanilla), partição exata e ordem da fonte, melhorias de bigorna, regras de prefixo exato dos 17 moldes de forja + 9 de construção, probabilidades/overlays da construção, grafo de IDs e determinismo byte a byte. Build e `cargo-horizon-mod check` do pacote de conteúdo são validados em cópia temporária sob `/tmp`, sem tocar em `~/Games/horizon*`.
