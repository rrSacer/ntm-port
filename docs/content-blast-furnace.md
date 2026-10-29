# Conteúdo e receitas do alto-forno

`content/machine-recipes/blast_furnace.json` (schema 1) exporta **12 receitas e 20 combustíveis**. O checkout de `BlastFurnaceRecipesNT.registerDefaults` contém **15**, e não 16, declarações: três ficam em `skipped`, com expressão e motivo. A ordem original é preservada por `source_index` (base zero) e pela lista completa `source_recipe_order`. A busca do Java percorre essa ordem; o consumidor não deve ordenar por nome.

Cada receita tem `name`, `source`, `source_index`, `duration` em ticks, `inputItems:[{ids:[id,...],count:n}]`, `outputItems:[[id,n],...]` e `label`/`label_en`. As alternativas vêm do OreDict já usado no triturador, incluindo os minérios vanilla legados. `sand` admite areia e areia vermelha. Não são adicionadas equivalências modernas sem registro legado, nem receitas elétricas ou pontes de crafting.

| Índice | Receita | Ticks | Saída |
|---|---|---:|---|
| 0 | blast.steelFromIngot | 800 | 2 aço + 1 escória |
| 1 | blast.steelFromDust | 800 | 2 aço + 1 escória |
| 2 | blast.steelFromOre | 800 | 2 aço + 2 escórias |
| 3 | blast.steelWithFlux | 1200 | 3 aço + 2 escórias |
| 4–7 | blast.mingrade / Dust / Ingot / Cursed | 400 | 2 cobre vermelho |
| 8 | blast.mingradeOre | 1200 | 6 cobre vermelho + 1 escória |
| 12 | blast.paa | 600 | 1 placa de liga PaA |
| 13–14 | blast.firebrick / firebrickLimestone | 800 | 8 tijolos refratários |

A escória é o item existente `ntm:ingot_raw__slag`, resolvido por `Mats.MAT_SLAG.id`; não se cria um segundo item. As lacunas de identidade são:

| Índice | Receita omitida | Conteúdo ausente |
|---|---|---|
| 9 | blast.meteorSword | meteorite_sword_hardened, meteorite_sword_alloyed |
| 10 | blast.meteor | ingot_meteorite |
| 11 | blast.starmetal | ingot_meteorite, ingot_starmetal |

## Combustíveis

`fuels` é um objeto `{id:ticks}`. `fuel_details` registra tempo base, multiplicador de calor, categoria, chaves OreDict e arquivo:linha; `fuel_rejected` explica exclusões e `fuel_gaps` registra expressões ou classes não verificadas. Na fonte atual não há lacunas de extração entre os 20 combustíveis aceitos.

Os tempos vêm de `FuelHandler.java` (incluindo metadata de briquetes e coque) e dos dois combustíveis vanilla elegíveis: carvão=1600 e bloco de carvão=16000. O perfil solicitado admite somente nomes OreDict contendo `Coal`, `Coke` ou `Lignite`, além de `solid_fuel*` e `rocket_fuel`. O multiplicador de calor dessas categorias é 1; as variantes `_bf` usam a categoria balefire do módulo. Itens com recipiente são recusados antes de considerar o calor. Classes desconhecidas e expressões de tempo não resolvidas são omitidas e documentadas, sem valores estimados.

Madeira, toras e itens sem categoria permitida ficam em zero. Isso é o **perfil restrito da tarefa**, não uma afirmação de equivalência integral ao Java: `setWoodHeatMod(0)` não zera a categoria separada Log e `getBurnHeat` ignora seu argumento `def`, usando o padrão 1 do módulo. Toras continuam excluídas por decisão explícita de escopo. Carvão vegetal não recebe uma categoria Coal por inferência do nome do item; lava e outros recipientes não são combustíveis desta tabela.

Os valores não são cortados na capacidade térmica de 38.400. Por exemplo, `solid_fuel_presto_triplet_bf` conserva 400.000 e `solid_fuel_presto_bf` conserva 80.000; a máquina decide se `burnValue <= capacity`, como no original. Isso também preserva valores de 32.000 para quando houver espaço suficiente.

## Bloco e reprodução

O único ID novo é **`ntm:machine_blast_furnace`**, em `ids.json.machines`, com template `minecraft:beehive`, `facing` e `honey_level=0..5` (24 variantes). Tem loot próprio, dureza 5, resistência 10, nomes fonte `Blast Furnace`/`Alto-Forno` e textura NTM `blocks/brick_fire` em todas as faces do cubo de fallback. O medidor usa os seis modelos gerados. A sobreposição OBJ, tela e multibloco pertence ao agente de visual; o funcionamento pertence ao agente de máquinas.

Arquivos principais: `content/data/ntm/horizon/block/machine_blast_furnace.json`, `content/assets/ntm/blockstates/machine_blast_furnace.json`, `content/assets/ntm/models/block/machine_blast_furnace_{0..5}.json` e `content/data/ntm/loot_table/blocks/machine_blast_furnace.json`.

```sh
/usr/local/bin/agent-run python3 tools/content-gen/generate.py
/usr/local/bin/agent-run python3 tools/visual-gen/generate.py
/usr/local/bin/agent-run python3 tools/content-gen/test_blast_furnace.py
/usr/local/bin/agent-run python3 tools/content-gen/test_shredder.py
/usr/local/bin/agent-run python3 tools/content-gen/test_m3.py
/usr/local/bin/agent-run python3 tools/content-gen/validate-bulk.py
# Executar na pasta content:
/usr/local/bin/agent-run /home/sacer/Games/horizon/target/release/cargo-horizon-mod build
/usr/local/bin/agent-run /home/sacer/Games/horizon/target/release/cargo-horizon-mod check target/horizon-mod/ntm.hmod
```

O pipeline integra o exportador ao final de `machine_recipes.export`, depois do triturador, e regenera hazards e abas. O catálogo passa a **2.517 IDs** (343 blocos, 2.174 itens; 13 máquinas). As receitas vanilla e temporárias existentes permanecem em 235 e 10. Logs de validação desta unidade ficam em `content-gen-evidence/blast-furnace/`. Os seis testes novos cobrem ordem/lacunas, escória/quantidades/alternativas, todos os IDs, combustíveis/metadados, rejeição de recipiente/expressão desconhecida e determinismo/contrato do bloco. A aceitação funcional em jogo é feita pelo agente de máquinas com a tela e o multibloco do agente de visual.

O build/check API 0.2 base aceitou `dist/ntm-content-blast-furnace.hmod` (3.985.255 bytes), SHA-256 `4a4d8ddd1945e748d7d62f59d30261e131eb22f6de0feee02cf2afa3f05328db`. O arquivo é um artefato local ignorado pelo git; nenhuma instalação real foi alterada.

Verificação concluída: seis testes do alto-forno, sete do triturador, sete M3 e validação integral verdes; `cargo test --workspace -j 3` passou (187 testes, 2 ignorados). Os testes Rust rodaram sobre o checkout compartilhado durante a integração do agente de máquinas; os commits desta frente incluem somente conteúdo e geradores.
