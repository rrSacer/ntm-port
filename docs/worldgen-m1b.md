# Worldgen M1b: minérios do Nether, gnaisse e pontes `m1_temp__`

Frente T3. Fonte Java: `ntm-src` (1.7.10). Tabela de valores: [ore-distribution.md](ore-distribution.md).

## O que entrou

Nether (`worldgen.py`, `EXTRA`, dimensão `#minecraft:is_nether`, alvo `minecraft:netherrack`), todos os veios de `generateNether` cujos IDs existem, exceto plutônio (desligado por padrão no Java):

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

Cada veio usa a mesma cadeia já provada para o overworld: `structure_set → jigsaw → feature_pool_element → placed_feature → ore`. Os salts são únicos e vão de 2167187 a 2167408 (veios do Nether e do gnaisse, incluindo coal, iron, gold e rare).

## Pontes `m1_temp__` (10 → 4)

Substituídas pela receita Java, com os mesmos IDs de entrada (todos já no catálogo):

- `machine_wood_burner`: aço (`plate_steel`) e bobina de cobre (`coil_copper`), em vez de titânio e fio.
- `machine_electric_furnace_off`: placa de cobre fundido (`plate_cast__copper`) e bobina de tungstênio (`coil_tungsten`).
- `red_cable`: polímero (`plate_polymer`) e fio de liga vermelha, sem couro.
- `wiring_red_copper`: aço, sem titânio.
- Liga de cobre vermelho: a rota padrão é o alto-forno (`BlastFurnaceRecipes.java:54`, cobre + redstone → 2 lingotes), já presente no conteúdo. A receita `powder_red_copper` (`PowderRecipes.java:79`) foi **removida**: só existe com `enableLBSM && enableLBSMSimpleCrafting`, falso por padrão.
- Minério de alumínio: a fundição para `chunk_ore__cryolite` já vem da `bulk` da `main` (`bulk__6def5b8b4c15ff0227.json`). A ponte antiga `m1_temp/aluminium_from_ore` (minério → lingote) foi **removida**; a entrada correspondente em `furnace_recipes.json` também saiu, o que é a única diferença desse arquivo em relação à `main` (aviso à T4).

Continuam temporárias, cada uma com o motivo em `tools/recipe-gen/coverage.json`:

| ID | Motivo |
| --- | --- |
| `machine_epress` | Receita de máquina de montagem (`AssemblyMachineRecipes.java:354`); a máquina não é portada e as entradas `part_generic`/`item_expensive` não têm IDs no catálogo. |
| `machine_battery` | Não há receita de fabricação no Java 1.7.10 (`MachineBattery` só é registrado). |
| `stamp_iron_plate`, `stamp_iron_wire` | Construção na bigorna (`AnvilRecipes.java:588-589`); a bigorna não é portada (T4). |

## Estrato de gnaisse: bloqueado fora do conteúdo

O `SchistStratum` troca pedra por `stone_gneiss` onde um Perlin de 4 oitavas (`NoiseGeneratorPerlin`, escala 0,01) passa de 5. A espessura é `int((n-5)*3)`, dobrada para `8 - range` acima de 4, em torno de y=30. Verificado no fonte decompilado de 1.7.10: a soma de oitavas tem amplitude 1, 2, 4, 8, então o limiar é alcançável.

Tentativa feita e desfeita: uma material rule nova em `data/minecraft/worldgen/material_rule/overworld.json`, com nove faixas de ruído, inserida antes de `overworld/underground`. O empacotador recusa:

- `crates/horizon-mods/src/content.rs:289`: "a mod adds files in its own namespaces ... or to vanilla and convention (c) tags". Só tags de `minecraft` e `c` são aceitas.
- `docs/mods.md`, "The fence": "Tags are the only `minecraft` files a mod may hold".
- Não há hook de conteúdo para material rule ou noise. O único gancho é `horizon:add_to`, para sets e pools.

Logo, o estrato só sai com mudança de API de conteúdo (um hook de material rule ou permissão de override) ou de motor. Decisão de escopo para o coordenador.

Por isso os veios de gnaisse estão corretos e registrados, mas geram 0 blocos em jogo. Comportamento de Java sem o estrato: também geram 0, porque o substrato é o estrato.

## Excesso de rendimento em jogo: investigação (lacuna de motor, aberta)

Medido com o cliente e o empacotador do fork (`/home/sacer/.cache/ntm-native-main`), pacote completo, 81 chunks centrais, janela 9×9 (`tools/content-gen/yield_probe.py`).

O port do algoritmo de veio de 26.3 (`horizon-world/src/worldgen/ore_feature.rs`) e o `WorldGenMinable` de 1.7.10 dão rendimentos por tentativa iguais, dentro de 15% (`tools/content-gen/vein_yield_oracle.py`). Em jogo, todos os veios saem acima de Java: Nether de 1,4× a 2,2×, overworld de 1,2× a 2,0×. **Atenção:** o oráculo compara dois modelos reimplementados em Python por esta frente (um do algoritmo de 26.3, outro do `WorldGenMinable`). Não roda o Java nem o motor, então não é paridade validada com o Java; é evidência de investigação. O próprio fork declara que a geometria dos veios não tem paridade (`docs/feature-placement.md:306–311`).

| Hipótese | Resultado |
| --- | --- |
| Duplicação por bioma (feature rodando uma vez por bioma no chunk) | **Refutada.** No Nether, os 81 chunks têm 1 bioma distinto e o excesso persiste (carvão: 514,7 por chunk contra 238 de Java). No overworld, o rendimento dos veios do ntm não sobe com o número de biomas: alumínio 19,2 / 16,5 / 19,6 para 2 / 3 / 4 biomas; urânio 14,9 / 12,2 / 10,6. |
| Duplicação no código de decoração | Refutada por leitura. Features de bioma são deduplicadas por índice em `worldgen/decoration.rs:153–160` (`BTreeSet`). Peças de feature de jigsaw só rodam no chunk que contém o ponto da peça (`structure/start.rs:1391`, `structure/jigsaw.rs:274`). |
| Método de medição | Não explica: o oráculo de Java medido com a mesma janela dá o mesmo rendimento. |
| Controle vanilla no Nether (quartzo: 16 tentativas × 6,0 = 96 por chunk, alvo netherrack) | **Inflado**: 150,6 por chunk, 1,57× acima do teto. Como o quartzo vanilla também sai acima do teto com 1 bioma por chunk, o excesso não é do conteúdo ntm. |
| Controle vanilla no overworld (carvão) | Não conclusivo: a previsão ignorou `ore_coal_upper` (30 tentativas). |

Conclusão atual: o excesso não vem do conteúdo (forma do veio igual a Java, sem duplicação por bioma) e aparece também em veio vanilla no Nether. A causa provável está no motor, em como as features de decoração ou de estrutura são executadas por chunk. **Lacuna de motor, não corrigida.** Não há contorno no conteúdo.

Os valores declarados de Java estão nos veios (`attempts_per_chunk` = tentativas de Java). A calibração de tentativas foi revertida, como pedido.

## Experimento decisivo: execuções por chunk e blocos por execução (pacote descartável)

Pacote de teste sem conteúdo ntm (namespace `probe`), construído com o empacotador do fork e apagado ao final. Mesma cadeia de estrutura dos veios (`structure_set → jigsaw → feature_pool_element → placed_feature`), origem `in_square` + `offset 8`, altura 60, 81 chunks centrais, cliente do fork.

1. **Execuções por chunk.** Marcador `simple_block` (`emerald_block`, `count 1`):
   - Overworld: 77 marcadores em 81 chunks.
   - Nether: 80 marcadores em 81 chunks.
   - Por chunk, 0 a 3 marcadores: a origem cai em faixa de 16 blocos começando em +8, então parte dos marcadores cai no chunk vizinho. O total é ~81, uma execução por start. **O motor não roda a feature mais de uma vez por chunk.**

2. **Blocos por execução**, `ore` com `count 1` sobre netherrack, origens nas mesmas posições, mesma janela. Modelo com o port de 26.3 (`vein_yield_oracle.py`) e com o `WorldGenMinable` de 1.7.10:

| Tamanho | Medido em jogo | Modelo 26.3 | Modelo Java | Medido / 26.3 |
| ---: | ---: | ---: | ---: | ---: |
| 6 | 2,75 | 1,81 | 1,94 | 1,52× |
| 12 | 8,26 | 4,89 | 4,69 | 1,69× |
| 32 | 77,4 | 30,5 | 29,9 | 2,54× |

   O excesso cresce com o tamanho. Isso indica diferença na geometria do veio, não contagem de execuções.

**Conclusão.** A causa é o **rendimento por execução do motor** para veios de minério, não o número de execuções, nem a duplicação por bioma, nem o modelo do 26.3 (que bate com o Java dentro de 15%). Não calibrei tentativas: a conversão seria mascarar esse defeito.

**Motor, arquivo:linha:**
- Caminho de produção: `crates/horizon-world/src/worldgen/feature_graph.rs:562` chama `OreFeature` (`ore_feature.rs`).
- Geometria do veio: `ore_feature.rs:148–300` (`place`, `do_place`). Suspeito: cálculo do raio e dos passos em `do_place`, ou a máscara `tested`, a ser confirmado com um teste de cruzamento contra o `WorldGenMinable` de 1.7.10.
- **Não é paridade validada:** `docs/feature-placement.md:306–311` declara que veios de minério comuns usam um caminho de seção em massa "pending" e que "no vein geometry parity is claimed". Portanto a premissa de paridade por harness não vale para veios.

**Para o coordenador:** o defeito está no motor. Antes de qualquer contorno no conteúdo, decidir: corrigir o `ore_feature.rs` (com teste de paridade contra o `WorldGenMinable`) ou aceitar a lacuna. Os veios do M1 permanecem com as tentativas de Java.

## Lacunas

- **Estrato de gnaisse:** ver seção acima.
- **Nether sem nome no doc (revisão):** `ore_nether_smoldering` (`HbmWorldGen.generateNether`, 30 tentativas por chunk, na superfície do netherrack com ar acima) não tem id em `ids.json`. E o minério de bedrock do Nether (`BedrockOre.generate`, 1 em 10 chunks), que pertence ao sistema de bedrock.
- **Alumínio sem rota de lingote:** o minério de alumínio agora rende só `chunk_ore__cryolite`. O lingote vem de cristal ou pó de alumínio (sete produtores no conteúdo), e essas receitas dependem do criolito, que não está portado. Impacto de progressão: sem o criolito, o alumínio não sai do minério em sobrevivência. `plate_aluminium` não tem produtor no conteúdo.
- **Consumo de placa de alumínio:** a bobina da fornalha elétrica agora usa cobre fundido, como no Java; a ponte antiga consumia `plate_aluminium`.
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
