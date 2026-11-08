# Worldgen M1b: minérios do Nether, gnaisse e pontes `m1_temp__`

Frente T3. Fonte Java: `ntm-src` (1.7.10). Tabela de valores: [ore-distribution.md](ore-distribution.md).

## O que entrou

Nether (`worldgen.py`, `EXTRA`, dimensão `#minecraft:is_nether`, alvo `minecraft:netherrack`):

| Minério | Tentativas/chunk | Tamanho | Y (inclusivo) | Origem Java |
| --- | ---: | ---: | --- | --- |
| `ore_nether_uranium` | 8 | 6 | 0–126 | `netherUraniumuSpawn`, `generateNether` |
| `ore_nether_tungsten` | 10 | 10 | 0–126 | `netherTungstenSpawn` |
| `ore_nether_sulfur` | 26 | 12 | 0–126 | `netherSulfurSpawn` |
| `ore_nether_fire` | 24 | 6 | 0–126 | `netherPhosphorusSpawn` |
| `ore_nether_cobalt` | 2 | 6 | 100–125 | `netherCobaltSpawn`, `minHeight 100 + nextInt(26)` |

Gnaisse (dimensão overworld, alvo `ntm:stone_gneiss`, Y 30–39, contagens ×3 como no Java):

| Minério | Tentativas/chunk | Tamanho | Origem Java |
| --- | ---: | ---: | --- |
| `ore_gneiss_uranium` | 21 (7×3) | 6 | `uraniumSpawn * 3` |
| `ore_gneiss_copper` | 36 (12×3) | 6 | `copperSpawn * 3` |
| `ore_gneiss_asbestos` | 6 (2×3) | 6 | `asbestosSpawn * 3` |
| `ore_gneiss_lithium` | 6 | 6 | `lithiumSpawn` |

Cada veio usa a mesma cadeia já provada para o overworld: `structure_set → jigsaw → feature_pool_element → placed_feature → ore`. Os salts são únicos (2167187–2167323).

## Pontes `m1_temp__` (10 → 4)

Substituídas pela receita Java, com os mesmos IDs de entrada (todos já no catálogo):

- `machine_wood_burner`: aço (`plate_steel`) e bobina de cobre (`coil_copper`), em vez de titânio e fio.
- `machine_electric_furnace_off`: placa de cobre fundido (`plate_cast__copper`) e bobina de tungstênio (`coil_tungsten`).
- `red_cable`: polímero (`plate_polymer`) e fio de liga vermelha, sem couro.
- `wiring_red_copper`: aço, sem titânio.
- Liga de cobre vermelho: `powder_red_copper` (redstone + pó de cobre) e fundição já extraída de `SmeltingRecipes.java:88`.
- Minério de alumínio: fundição para `chunk_ore__cryolite`, como no Java, em vez de lingote direto.

Continuam temporárias, cada uma com o motivo em `tools/recipe-gen/coverage.json`:

| ID | Motivo |
| --- | --- |
| `machine_epress` | Receita de máquina de montagem (`AssemblyMachineRecipes.java:354`); a máquina não é portada e as entradas `part_generic`/`item_expensive` não têm IDs no catálogo. |
| `machine_battery` | Não há receita de fabricação no Java 1.7.10 (`MachineBattery` só é registrado). |
| `stamp_iron_plate`, `stamp_iron_wire` | Construção na bigorna (`AnvilRecipes.java:588-589`); a bigorna não é portada (T4). |

## Lacunas

- **Estrato de gnaisse não portado.** No Java, `SchistStratum` troca pedra por `stone_gneiss` em uma faixa em torno de y=30, com largura dada por ruído Perlin (escala 0,01). Uma feature de JSON não reproduz esse ruído. Sem o estrato, os veios de gnaisse não encontram substrato e não geram blocos. Porta exige material rule em `noise_settings` ou sistema de terreno: decisão de escopo, não feita aqui.
- **`ore_nether_schrabidium` sem geração natural.** No 1.7.10 ele só aparece em `ExplosionNukeGeneric` e no mapa de fundição. Não há veio para portar.
- **Nether fora da lista da T3:** `ore_nether_plutonium` e `ore_nether_coal` (veios próprios no Java), além dos veios de `HbmWorldGen.generateNether` (scorched uranium, depth deposits, geysers) ficam para outras frentes.
- **Gnaisse fora da lista da T3:** `ore_gneiss_iron`, `ore_gneiss_gold`, `ore_gneiss_rare` e `ore_gneiss_gas` existem no Java e não foram geradas.
- **`m1.json` `worldgen_gaps.lithium`** ainda diz que o lítio não tem substituição. O texto continua válido, mas é um arquivo do catálogo fora da posse desta frente.

## Provas

- `python3 -m unittest test_m1_worldgen` em `tools/content-gen/`: 6 testes, OK. Confere contagens, tamanhos, alturas, biomas, alvos, salts únicos e a emissão de 5 arquivos por veio.
- `python3 -m unittest discover -s tools/recipe-gen -p test_m1_bridges.py`: 3 testes, OK. Confere que só as 4 pontes restantes são temporárias e que cada uma cita o Java.
- `tools/recipe-gen/generate.py` com `--source ntm-src`: `temporary_count` 10 → 4, `furnace_count` 255, `vanilla_recipe_count` 245.
- Arquivos de worldgen do overworld: regenerados e idênticos aos commitados.
- Não rodado nesta frente: `test_ownership.py` (2 testes com erro de caminho: `ntm-src` precisa estar ao lado da worktree) e a conferência em chunks reais do Nether/overworld com `verify-worldgen.py` (requer pacote e servidores dedicados; pendente).
