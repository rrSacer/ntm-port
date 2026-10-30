# Conteúdo e receitas da prensa

`content/machine-recipes/press.json` (schema 1) exporta a tabela completa de `PressRecipes.java` e as matrizes (`ItemStamp.java` / `ModItems.java:2724-2757`) no contrato compartilhado com o agente de lógica (`tools/machine-gen/build.py` → `crates/ntm-core/data/press_recipes.json`), sem rede nem execução de Java.

A escrita direta de `crates/ntm-core/data/press_recipes.json` que existia desde o M1 em `tools/recipe-gen/generate.py` (3 chapas, 9 fios e 3 matrizes de ferro) foi transferida para `tools/machine-gen/build.py`; `tools/recipe-gen/generate.py` agora lê `press_count` de `content/machine-recipes/press.json` para `tools/recipe-gen/coverage.json` e remove `crates/ntm-core/data/press_recipes.json` de `tools/recipe-gen/generated-files.json`.

## Cobertura e ordem da fonte

| Métrica | Valor |
|---|---:|
| Chamadas `makeRecipe` em `PressRecipes.registerDefaults()` | 38 |
| Receitas expandidas (`Mats.orderedList` para `WIRE`) | 48 |
| Receitas emitidas em `recipes` | 47 |
| Receitas omitidas em `skipped` | 1 |
| Matrizes emitidas em `stamps` (`ItemStamp`) | 32 |
| Matrizes omitidas em `skipped_stamps` (`ItemStampBook`) | 1 |

`source_index` (`0..47`) preserva a ordem exata de `PressRecipes.registerDefaults()`, expandindo o laço `for (NTMMaterial mat : Mats.orderedList)` na ordem da lista de materiais — relevante porque `PressRecipes.getOutput` retorna a primeira entrada que casa com `(StampType, AStack)`.

### Contagem por tipo de matriz (`StampType`)

| `stamp` | Emitidas | `skipped` | Resumo das receitas |
|---|---:|---:|---|
| `flat` | 10 | 1 | Pós de quartzo/lápis/diamante/esmeralda → gemas vanilla, `biomass` → `biomass_compressed`, `ANY_COKE.gem()` (`coke__coal`, `coke__lignite`, `coke__petroleum`) → `ingot_graphite`, `minecraft:jungle_log` (`Blocks.log` meta 3) → `ball_resin`, pós de carvão/linhito/serragem → `briquette__{coal,lignite,wood}`. Omitida: `press.6` (`meteorite_sword_reforged` → `meteorite_sword_hardened`). |
| `plate` | 13 | 0 | Lingotes → `plate_{iron,gold,titanium,aluminium,steel,lead,copper,schrabidium,combine_steel,gunmetal,weaponsteel,saturnite,dura_steel}` (usando o snapshot OreDict conservador do Forge 1.7.10, sem alias moderno `minecraft:copper_ingot`). |
| `c9` | 2 | 0 | `plate_gunmetal` → `casing__small ×4`; `plate_weaponsteel` → `casing__small_steel ×4`. |
| `c50` | 2 | 0 | `plate_gunmetal` → `casing__large ×2`; `plate_weaponsteel` → `casing__large_steel ×2`. |
| `wire` | 11 | 0 | Laço `Mats.orderedList` com `autogen.contains(WIRE)` e `ingot` registrado no OreDict (`×8`): `carbon` (`ingot_graphite` → `wire_fine__carbon`), `gold` (`minecraft:gold_ingot` → `wire_fine__gold`), `schrabidium`, `copper`, `tungsten`, `aluminium`, `lead`, `zirconium`, `steel`, `mingrade` (`ingot_red_copper` → `wire_fine__red_copper`) e `magtung` (`ntm:ingot_magnetized_tungsten` → `wire_fine__magnetized_tungsten`). |
| `circuit` | 1 | 0 | `billet_silicon` → `circuit__silicon`. |
| `printing1..8` | 8 (1 cada) | 0 | `minecraft:paper` → `ntm:page_of___page1..8` (`1` por tipo `printing1`..`printing8`). |
| `c357`, `c44` | 0 | 0 | As 4 matrizes existem em `ModItems.java`, mas `PressRecipes.registerDefaults()` não registra receitas para `.357`/`.44` no NTM 1.7.10. |

## As 32 matrizes (`stamps`), `max_damage` e lacunas documentadas

Todas as **32 instâncias de `ItemStamp`** declaradas em `ModItems.java:2724-2757` já existem em `content/ids.json` com `max_stack_size: 1`. Assim como nas lâminas do triturador (`ItemBlades`), `m3_catalog.py` registra `max_damage` em `content/ids.json` para as **24 matrizes desgastáveis** e omite `max_damage` nas **8 matrizes eternas de desh** (`durability == 0`): o conteúdo em `content/data/ntm/horizon/item/stamp_*.json` já declara o componente, e `tools/build-visual.sh` conserva `components: {"minecraft:max_damage": N}` nas declarações para preservar o dano real do slot no inventário. O componente é aceito pelo upstream atual e pelo fork com o patch de compatibilidade da [migração C1](content-migration-c1.md); o campo solto não é mais emitido.

| Família | Durabilidade (`maxDamage`) | `max_damage` em `ids.json` | Tipos (`type`) | Quantidade |
|---|---:|---:|---|---:|
| Pedra (`ntm:stamp_stone_*`) | 32 | `32` | `flat`, `plate`, `wire`, `circuit` | 4 |
| Ferro (`ntm:stamp_iron_*`) | 64 | `64` | `flat`, `plate`, `wire`, `circuit` | 4 |
| Aço (`ntm:stamp_steel_*`) | 192 | `192` | `flat`, `plate`, `wire`, `circuit` | 4 |
| Titânio (`ntm:stamp_titanium_*`) | 256 | `256` | `flat`, `plate`, `wire`, `circuit` | 4 |
| Obsidiana (`ntm:stamp_obsidian_*`) | 512 | `512` | `flat`, `plate`, `wire`, `circuit` | 4 |
| Desh (`ntm:stamp_desh_{flat,plate,wire,circuit}`) | 0 (inquebrável) | omitido | `flat`, `plate`, `wire`, `circuit` | 4 |
| Munição padrão (`ntm:stamp_{357,44,9,50}`) | 1000 | `1000` | `c357`, `c44`, `c9`, `c50` | 4 |
| Munição de desh (`ntm:stamp_desh_{357,44,9,50}`) | 0 (inquebrável) | omitido | `c357`, `c44`, `c9`, `c50` | 4 |

### Lacunas documentadas (sem IDs nem receitas inventados)

1. **`press.6` (`PressRecipes.java:67`, `flat`):** `ModItems.meteorite_sword_reforged` → `ModItems.meteorite_sword_hardened` permanece em `skipped` porque a cadeia de espadas de meteorito ainda não faz parte de `content/ids.json`.
2. **`ModItems.stamp_book` (`ModItems.java:2752`, `ItemStampBook`):** item secreto (`setCreativeTab(null)`, subtipos `0..7` correspondentes a `printing1..printing8`, durabilidade `0`) ausente de `content/ids.json` e registrado em `skipped_stamps`. As 8 receitas `printing1..printing8` (`minecraft:paper` → `ntm:page_of___page1..8`) são emitidas fielmente porque tanto o papel vanilla quanto as 8 páginas já estão catalogados, permanecendo inalcançáveis em jogo enquanto `stamp_book` não for portado.
3. **`c357` e `c44`:** fidelidade direta à fonte — o próprio NTM 1.7.10 declara `stamp_357`, `stamp_44`, `stamp_desh_357` e `stamp_desh_44`, mas não registra entradas `StampType.C357`/`C44` em `PressRecipes`.

## Reprodução e validação

```sh
/usr/local/bin/agent-run python3 tools/content-gen/generate.py --ids-only
/usr/local/bin/agent-run python3 tools/content-gen/press_recipes.py
/usr/local/bin/agent-run python3 tools/recipe-gen/generate.py
/usr/local/bin/agent-run python3 tools/recipe-gen/validate.py
/usr/local/bin/agent-run python3 tools/content-gen/test_press.py
/usr/local/bin/agent-run python3 tools/content-gen/test_anvil.py
/usr/local/bin/agent-run python3 tools/content-gen/test_blast_furnace.py
/usr/local/bin/agent-run python3 tools/content-gen/test_shredder.py
/usr/local/bin/agent-run python3 tools/content-gen/test_m3.py
/usr/local/bin/agent-run python3 tools/content-gen/validate-bulk.py
```
