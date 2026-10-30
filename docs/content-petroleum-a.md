# Petróleo, lote A: liquefação e solidificação

Este lote fornece o conteúdo estático de `ntm:machine_liquefactor` e `ntm:machine_solidifier`. Os blocos usam beehive, facing e honey_level 0..5, loot próprio, nomes en_us/pt_br e textura NTM opaca de fallback. O visual-gen conserva a propriedade de seus modelos, telas e multiblocos. A entrega de conteúdo não implementa nem autoriza a lógica dessas máquinas.

O catálogo passa de 2.637 para **2.639 IDs**: 375 blocos, 2.264 itens e 42 máquinas. Os 157 fluidos já existentes bastam; nenhum foi acrescentado. Hazards, tags de radiação e as oito abas criativas foram regenerados. Há 891 receitas vanilla, 97 taxas de radiação e 27 tags por bit.

## Tabelas e precedência

`tools/content-gen/petroleum_recipes.py`, chamado pelo pipeline de máquinas, escreve schema 1 em `content/machine-recipes/{liquefaction,solidification}.json`. As entradas conservam alternativas OreDict, quantidades e fonte; saídas e fluidos usam o formato generic, com labels pt_br/en_us. `source_index` conserva a posição da declaração na fonte.

| Tabela | Chamadas fonte | Exportadas | Skipped | Outros registros |
| --- | ---: | ---: | ---: | --- |
| LiquefactionRecipes | 34 | 31 | 3 | 1 lacuna dinâmica ItemFood |
| SolidificationRecipes | 48 | 45 | 2 | 1 entrada substituída |

Ambas têm parâmetros-base extraídos dos tiles: **250 HE/t, 60 ticks, buffer de 100.000 HE e tanque de 24.000 mB**, com fonte individual em `machine_config.sources`. Valores com upgrades não foram simulados.

Liquefação escolhe por identidade exata, depois wildcard do item, depois as chaves OreDict na ordem de registro do item e, por fim, ItemFood. A ordem do array é a das declarações; `selection_order` e `match_kind` expressam a precedência. O handler original ignora o tamanho da pilha de entrada e consome uma unidade. O peixe cru legado vira uma alternativa com cod, salmon, tropical_fish e pufferfish; snow_block, melon_slice, sunflower, short_grass e fern conservam a identidade da metadata 1.7.10, sem transformar melon_slice em bloco de melancia ou snow_block em camada de neve.

Solidificação escolhe pelo fluido do tanque. O último `HashMap.put` substitui o anterior: a declaração final de BALEFIRE exige **110 mB**, produz um `solid_fuel_bf` e substitui a de 250 mB. O registro antigo fica em `superseded`, com fonte e referência à substituta. O cálculo usa o calor final de Fluids.java, penalidade **1,25** e a mesma truncagem/arredondamento Java. Isso dá, por exemplo, 700 mB para querosene; o bônus 0,5 da pirólise não se aplica aqui.

## Construção e lacunas

As receitas originais `ass.liquefactor` e `ass.solidifier` estão na montadora: ambas duram 200 ticks a 100 HE/t. O snapshot passou a resolver os membros KEY_* dos DictGroup depois dos registros OreDict; isso recupera as quatro variantes de ANY_TAR (crude, crack, coal e wood). Além do liquefator, a correção destrava `chem.tel`: a planta química passa de 52/25 para **53 exportadas / 24 skipped**, e a montadora de 416/215 para **418/213**.

Os skipped são lacunas de lotes posteriores: liquefação de `ore_oil_sand` e das duas metadata plant_flower; solidificação para `ore_oil_sand` e `bio_wafer`. Cada uma conserva a expressão e o motivo. Não foram criadas identidades ou receitas substitutas.

A regra dinâmica ItemFood → salient é registrada em `runtime_gaps`: `int(food_value * saturation_modifier * 20)`. Não há snapshot completo da nutrição vanilla 1.7.10; usar os valores modernos mudaria rendimentos. As receitas explícitas foram exportadas, mas essa regra geral permanece pendente de dados originais. Nenhuma geração de mundo pertence ao lote A.

## Verificação

Passaram **86 testes Python em cada seed, PYTHONHASHSEED=1 e 42**, incluindo oito testes do lote A; `validate-bulk.py` e `tools/recipe-gen/validate.py` também passaram. Há testes de precedência, identidades legadas, alternativas de alcatrão, parâmetros, conservação das chamadas e regeneração em diretório novo. Evidências locais ficam em `content-gen-evidence/petroleum-a/`.

Build/check API 0.2 passou em cópia temporária: **5.300.024 bytes**, SHA-256 `9b38e07522f0ff3b125338dcd33887b2d84038bc32997522c82c39b77cea3761`. O pacote foi removido pela limpeza de temporários; próximos exports, targets, mundos e pacotes grandes usam disco em `Workspace/.agent-tmp`, pois /tmp neste host é tmpfs. O agente de lógica confirmou o HEAD isolado com os dois registros Inert: 225/225 testes e hazard-gen sem mudança; o push conjunto já foi feito. Nenhum pacote foi instalado no jogo real.
