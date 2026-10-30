# Cristalizador: conteúdo da etapa 2 do bedrock

Esta entrega acrescenta `ntm:machine_crystallizer` e suas receitas estáticas. O nome original é **Ore Acidizer / Acidificador de Minério**, preservado nos idiomas en_us e pt_br. A lógica, a tela e o multibloco 3×3×6 pertencem às outras frentes; o conteúdo não implica aprovação ou implementação da lógica da etapa 2.

## Bloco e montagem

O bloco consta em `ids.json.machines`, usa template `minecraft:beehive`, `facing` e `honey_level` 0..5, loot próprio e textura original do NTM como cubo de fallback. O manifesto do visual-gen tem precedência sobre os arquivos que ele reivindicar. O catálogo total passa para **2.580 IDs**, sendo 368 blocos, 2.212 itens e 37 máquinas; a bateria criativa foi uma entrega separada.

A receita original da montadora é `ass.acidizer`: duas placas soldadas de aço, três carcaças de titânio, quatro lingotes de desh, um motor e dois circuitos básicos, em 200 ticks a 100 HE/t. O exportador conserva alternativas OreDict e o modo padrão da fonte (receitas caras desativadas). O arquivo principal da montadora passa para **413 receitas**.

## Extração das receitas

`tools/content-gen/crystallizer_recipes.py` é chamado pelo exportador de máquinas e gera `content/machine-recipes/crystallizer.json`, schema 1. Ele lê `CrystallizerRecipes.registerDefaults` e conserva a ordem de execução, expandindo cada laço na ordem das enumerações. `source_index` identifica a declaração original; `expansion` distingue suas instâncias.

| Cobertura | Contagem |
|---|---:|
| Chamadas na fonte | 118 |
| Instâncias após expandir laços finitos | 409 |
| Receitas exportadas | 357 |
| Cadeia nova de bedrock, incluída nas exportadas | 222 |
| Sistema legado de minério de bedrock | 75 |
| Aliases de tório | 3 |
| Outras receitas estáticas exportadas | 57 |
| Instâncias em `skipped`, com fonte e motivo | 52 |

As entradas usam `inputItems:[{ids:[...],count:N}]`, saídas `outputItems:[[id,N]]`, com `label` pt_br e `label_en`. Cada receita tem **exatamente um fluido explícito** em `inputFluids:[[string_id,mB]]`. O overload sem fluido no Java usa **peroxide, 500 mB**, extraído da fonte. `setReq` define a quantidade de itens: por exemplo, 64 migalhas com 1.000 mB de slop produzem um BASE, e quatro subprodutos com 250 mB de água produzem um lavado.

Todas as receitas atuais declaram duração no construtor. Os tempos e `productivity` são conservados por receita. `machine_config` também documenta os valores de `TileEntityMachineCrystallizer`: demanda de 1.000 HE/t, buffer de 1.000.000 HE, tanque de 8.000 mB e duração inicial de 600 ticks.

**Produtividade não é probabilidade de saída:** no NTM, o upgrade EFFECT dá chance `min(nível × productivity, 0,99)` de preservar o item de entrada. O fluido é sempre consumido. Sem upgrade essa chance é zero; o crafter do port ainda não tem upgrades, portanto o campo é documental.

Lavagem com água é **1 BASE → 1 BASE_WASHED**, usando 250 mB e 100 ticks. As duas unidades de PRIMARY são obtidas na centrífuga posterior. Os ramos de PRIMARY com sulfúrico, hidrogênio, cloro, solvente e radiosolvente usam 250 mB cada e foram exportados para os seis tipos. Nenhuma receita bedrock foi omitida.

## Omissões e limites

As 52 omissões são 23 instâncias do laço de plásticos, 18 de corantes (seis cores com três óleos) e 11 estáticas. Faltam identidades de catálogo como `powder_cement`, `scrap_plastic`, `circuit_star_piece`, `block_graphite`, `ingot_semtex`, `rebar`/`concrete_rebar`, espadas de meteorito, `chemical_dye`, `pill_red` e `sand_mix`; a expressão `SRN.ingot()` não resolve no snapshot. Três integrações condicionais têm OreDict vazio: Certus Quartz, pó de fósforo branco e cinábrio. O JSON registra cada expressão e seus motivos; não foram inventados IDs ou receitas substitutas.

O OreDict é o snapshot conservador NTM + vanilla legado, sem mods externos. A saída de sucata de cobre com NBT é representada pelo item existente `ntm:scraps__copper`: os 72 quanta da fonte coincidem com a quantidade padrão dessa identidade no port, registrada em `source_scrap_amount`. Isso não generaliza sucatas de quantidades arbitrárias.

## Verificação

Passaram os **66 testes Python** do content-gen em `PYTHONHASHSEED=1` e `42`, incluindo oito testes do cristalizador e dois da bateria. Após refinar os motivos de omissão, os oito testes do cristalizador passaram novamente nos dois seeds. Eles cobrem chamadas, ordem dos laços, IDs, chave item/fluido, default de peróxido, duração, produtividade, requisitos, lavagem, montagem e regeneração determinística. `validate-bulk.py` e `tools/recipe-gen/validate.py` também passaram.

Catálogo, hazards, tags de radiação e oito abas criativas foram regenerados. O datapack conserva 861 receitas vanilla; há 97 faixas e 27 bits de radiação. O agente de lógica mantém o commit de `crates/ntm-core/data/hazards.json`; esta frente não altera `crates/`.

Build/check API 0.2 passou numa cópia temporária, via `agent-run`: `/tmp/ntm-crystallizer-content-nsmsqnsy/target/horizon-mod/ntm.hmod`, **4.964.680 bytes**, SHA-256 `2fda6bae5b3b3384ac56a12c4fa19ad6cc58cafe8726ee44437e151f2bd8ecb6`. Evidências locais: `content-gen-evidence/crystallizer/`. Nenhum pacote foi instalado no jogo real; a aceitação funcional da máquina fica para a frente de lógica, após autorização da etapa 2.
