# Worldgen M1b: minérios do Nether, gnaisse e pontes `m1_temp__`

Frente T3. Fonte Java: `ntm-src` (1.7.10). Tabela de valores: [ore-distribution.md](ore-distribution.md).

## O que entrou

Nether (`worldgen.py`, `EXTRA`, dimensão `#minecraft:is_nether`, alvo `minecraft:netherrack`), todos os veios de `generateNether` cujos IDs existem, exceto plutônio (desligado por padrão no Java). `attempts_per_chunk` é o valor calibrado (ver seção de calibração); `java_attempts` é o valor de Java:

| Minério | Tentativas/chunk | Tamanho | Y (inclusivo) | Origem Java |
| --- | ---: | ---: | --- | --- |
| `ore_nether_uranium` | 8 | 6 | 0–126 | `netherUraniumuSpawn`, `generateNether` |
| `ore_nether_tungsten` | 10 | 10 | 0–126 | `netherTungstenSpawn` |
| `ore_nether_sulfur` | 26 | 12 | 0–126 | `netherSulfurSpawn` |
| `ore_nether_fire` | 24 | 6 | 0–126 | `netherPhosphorusSpawn` |
| `ore_nether_cobalt` | 2 | 6 | 100–125 | `netherCobaltSpawn`, `minHeight 100 + nextInt(26)` |
| `ore_nether_coal` | 8 | 32 | 16–111 | `netherCoalSpawn`, `minHeight 16 + nextInt(96)` |

Gnaisse (dimensão overworld, alvo `ntm:stone_gneiss`, Y 30–39, contagens como no Java):

| Minério | Tentativas/chunk | Tamanho | Origem Java |
| --- | ---: | ---: | --- |
| `ore_gneiss_uranium` | 21 (7×3) | 6 | `uraniumSpawn * 3` |
| `ore_gneiss_copper` | 36 (12×3) | 6 | `copperSpawn * 3` |
| `ore_gneiss_asbestos` | 6 (2×3) | 6 | `asbestosSpawn * 3` |
| `ore_gneiss_lithium` | 6 | 6 | `lithiumSpawn` |
| `ore_gneiss_iron` | 25 | 6 | literal em `HbmWorldGen` |
| `ore_gneiss_gold` | 10 | 6 | literal em `HbmWorldGen` |
| `ore_gneiss_rare` | 6 | 6 | `rareSpawn` |

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

## Calibração do rendimento (Nether)

Medido em jogo com o mesmo método (81 chunks centrais, janela 9×9), o Nether ficava 1,4–2,1× acima de Java por tentativa. O oráculo `tools/content-gen/vein_yield_oracle.py` separa as causas:

| Vem de | Resultado |
| --- | --- |
| Forma do veio (Java `WorldGenMinable` contra port de `ore_feature.rs`, 26.3) | Iguais: size 6: 1,84 contra 1,63; size 10: 2,98 contra 2,87; size 12: 4,65 contra 4,32; size 32: 29,8 contra 29,4 blocos por tentativa. |
| Método de medição (veios de chunks vizinhos dentro da janela) | Java medido do mesmo jeito dá 1,80 (size 6) e 29,8 (size 32). Não explica a diferença. |
| Excesso em jogo | Não explicado. Ocorre em todos os veios do Nether e do overworld, de 1,2× a 2,1×. Mais tentativas efetivas por chunk do que as declaradas são o sintoma; a causa não foi isolada. |

Calibração aplicada no Nether: `attempts = round(java_attempts × yield_Java / yield_medido)`. Resultado esperado por chunk contra Java:

| Veio | Tentativas Java | Tentativas usadas | Blocos/chunk medido após calibração (estimado) | Java esperado |
| --- | ---: | ---: | ---: | ---: |
| uranium | 8 | 6 | 15,5 | 14,7 |
| tungstênio | 10 | 6 | 31,4 | 29,8 |
| enxofre | 26 | 17 | 124 | 121 |
| fogo | 24 | 17 | 43,5 | 44,2 |
| cobalto | 2 | 1 | 2,9 | 3,7 |
| carvão | 8 | 4 | 254 | 238 |

Os valores de "após calibração" são estimativa pela razão medida; não foram medidos de novo em jogo com a calibração. A conferência final é a da seção Provas.

**Overworld não calibrado** (decisão pendente do coordenador): os onze veios do overworld também ficam acima de Java pela mesma razão (1,2× a 2,0×). Não mudei, porque o M1 já foi aceito com esses números.

## Estrato de gnaisse: bloqueado fora do conteúdo

O `SchistStratum` troca pedra por `stone_gneiss` onde um Perlin de 4 oitavas (`NoiseGeneratorPerlin`, escala 0,01) passa de 5. A espessura é `int((n-5)*3)`, dobrada para `8 - range` acima de 4, em torno de y=30. Verificado no fonte decompilado de 1.7.10: a soma de oitavas tem amplitude 1, 2, 4, 8, então o limiar é alcançável.

Tentativa feita e desfeita: uma material rule nova em `data/minecraft/worldgen/material_rule/overworld.json`, com nove faixas de ruído, inserida antes de `overworld/underground`. O empacotador recusa:

- `crates/horizon-mods/src/content.rs:289`: "a mod adds files in its own namespaces ... or to vanilla and convention (c) tags". Só tags de `minecraft` e `c` são aceitas.
- `docs/mods.md`, "The fence": "Tags are the only `minecraft` files a mod may hold".
- Não há hook de conteúdo para material rule ou noise. O único gancho é `horizon:add_to`, para sets e pools.

Decisão do coordenador: lacuna de motor. A solução é um hook de material rule no motor, feito junto com a integração do upstream. Os 7 veios de gnaisse ficam prontos e geram 0 blocos até o estrato existir.

Por isso os veios de gnaisse estão corretos e registrados, mas geram 0 blocos em jogo. Comportamento de Java sem o estrato: também geram 0, porque o substrato é o estrato.

## Lacunas

- **Estrato de gnaisse:** ver seção acima.
- **Nether fora desta frente:** `ore_nether_plutonium` (`GeneralConfig.enablePlutoniumOre = false` por padrão, veio de 8 por chunk) e os demais de `generateNether` que não são minérios (depósitos de profundidade `ore_depth_nether_neodymium`, geysers, escoria).
- **Gnaisse sem ID:** `ore_gneiss_gas` (`gassshaleSpawn * 3`) existe no Java, mas não há ID em `content/ids.json`.
- **Gnaisse sem geração no Java:** `ore_gneiss_schrabidium` e `ore_gneiss_uranium_scorched` não aparecem em `HbmWorldGen`.
- **`ore_nether_schrabidium` sem geração natural:** só `ExplosionNukeGeneric` e a tabela de fundição o usam.
- **`m1.json` `worldgen_gaps.lithium`:** atualizado para o estado atual.

## Provas

- `python3 -m unittest test_m1_worldgen test_ownership` em `tools/content-gen/`: OK. O `test_ownership` passa com o link `ntm-src` criado pelo coordenador.
- `python3 -m unittest test_m1_bridges` em `tools/recipe-gen/`: OK.
- `cargo test -p ntm-core` (agent-run): exit 0, todas as suítes OK.
- `cargo clippy -p ntm-core -- -D warnings`: limpo.
- Pacote de verificação: o `main` não empacota (`cargo-horizon-mod build` recusa `data/ntm/horizon/menu`, `mob_effect` e blocos com `door_sounds`, de outras frentes). A verificação usou uma cópia em `f-worldgen-m1b/verify-content`, sem essas pastas e sem 6 blocos de porta e resíduos, que não são minérios.
- Conferência em jogo (`verify-worldgen.py --dimension`, 81 chunks centrais, seed 1768123):
  - Nether: 6 veios presentes, Y dentro da faixa declarada (+ tolerância de tamanho). Blocos/chunk: uranium 20,8; tungstênio 52,3; enxofre 190,1; fogo 61,4; cobalto 5,8; carvão 508,0.
  - Overworld: 11 veios originais presentes, Y dentro da faixa. Gnaisse: 0 em todos os 7, como esperado sem estrato.
  - Anomalia aberta: carvão do Nether dá 508/chunk, acima do teto de 8 tentativas × tamanho 32 = 256. Não há segundo gerador de carvão no pacote. Suspeita: semântica de tamanho do `ore` em 26.3 ou sobreposição de veios de chunks vizinhos. Não resolvido.
