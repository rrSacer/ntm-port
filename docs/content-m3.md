# Conteúdo M3: fluidos e processamento

Pacote: `dist/ntm-content-m3.hmod`, SHA-256 `31d5b1463ffcca52e9cc3a9e40786c3c38633cf1857893569be72a82cb736d3e`.

O catálogo passa de 1881 para **2515 IDs**: 341 blocos e 2174 itens. O acréscimo é de seis máquinas, um duto, 610 recipientes preenchidos e 17 componentes de material (parafusos, carcaças e tubos autogerados). O pacote de conteúdo usa API 0.2; a execução das máquinas pertence aos módulos Rust do M3.

## Contratos de blocos

| ID `ntm:` | Contrato |
|---|---|
| `machine_assembly_machine` | Montadora |
| `machine_chemical_plant` | Planta química |
| `machine_centrifuge` | Centrífuga |
| `machine_gascent` | Centrífuga de gás |
| `barrel_steel` | Barril, 16.000 mB |
| `machine_fluidtank` | Tanque, 256.000 mB |
| `fluid_duct_neo` | Duto, cubo sem propriedades; tipo de fluido no servidor |

As seis máquinas aparecem em `ids.json.machines`, com template `minecraft:beehive`, orientação `facing` e nível `honey_level=0..5`. Cada uma tem 24 variantes, barra de nível e loot próprio. O duto aparece em `ids.json.ducts`, com `logic.kind=duct`. Modelos TESR e multiblocos usam uma aproximação de um cubo com texturas originais; a lacuna está em cada entrada do catálogo.

O backlog histórico de máquinas conserva as entradas originais e marca as seis como contrato de conteúdo M3 declarado; isso não atesta funcionamento da lógica.

## Recipientes e identidade dos fluidos

`content/fluids.json` preserva as **157 definições**, incluindo `NONE=0`. O catálogo de recipientes exclui NONE e segue `FluidContainerRegistry.java:69–91` e o laço de embalagens em `AssemblyMachineRecipes.java:1114–1121`:

| Família | Variantes | Volume |
|---|---:|---:|
| `fluid_tank_full__<stringId>` | 136 | 1.000 mB |
| `fluid_barrel_full__<stringId>` | 136 | 16.000 mB |
| `fluid_tank_lead_full__<stringId>` | 138 | 1.000 mB |
| `fluid_pack_full__<stringId>` | 138 | 32.000 mB |
| `canister_full__<stringId>` | 43 | 1.000 mB |
| `gas_full__<stringId>` | 19 | 1.000 mB |

`stringId` é o nome estável do NTM em minúsculas, como `uf6` e `sulfuric_acid`. Cada item tem `metadata` original e `fluid:{id,amount_mb,empty}`. As texturas seguem as camadas e cores de `ItemFluidTank`, `ItemCanister` e `ItemGasTank`: duas camadas nos tanques/galões e três nos cilindros de gás. Os 18 fluidos úteis NOCON não ganham tanque/barril/pacote; os dois LEADCON não ganham tanque/barril comum. As embalagens seguem o laço próprio da montadora, que aceita LEADCON.

O índice `content/fluids.json` serve ao catálogo/render: `id` e `string_id` são strings, `source_id`/`metadata` são numéricos, `color` é hexadecimal, `temp` usa °C, e há `no_container`, `lead_container`, descritores de cor e referência fonte. O extrator `tools/fluid-gen/extract.py` e `crates/ntm-core/data/fluids.json` continuam donos dos dados de simulação. O teste cruza os 157 IDs, cores, temperaturas e restrições dos dois índices.

As células legadas já existentes permanecem. Saídas disponíveis da centrífuga de gás: `nugget_uranium_fuel`, `nugget_u235`, `nugget_u238`, `nugget_pu238`, `nugget_pu_mix`, `fluorite`, `powder_lead`, `dust`. A fluorita usa o registro fonte `fluorite`.

## Exportação de receitas de máquinas

`tools/content-gen/machine_recipes.py` reaproveita o `Resolver` de receitas vanilla, com resolução adicional de DictGroup, MaterialShapes e metadata de fluido. Exporta `content/machine-recipes/{assembly,chemical_plant,centrifuge}.json`, schema 1.

| Máquina | Receitas emitidas | Chamadas fonte omitidas |
|---|---:|---:|
| Montadora | 398 | 233 |
| Planta química | 47 | 30 |
| Centrífuga | 54 | 25 |

As 398 da montadora incluem 276 de empacotar/desempacotar, expansão explícita de duas chamadas fonte para os 138 fluidos permitidos. A contagem de chamadas omitidas não representa o número de instâncias dentro dos outros laços dinâmicos. Cada omissão tem nome, motivo, expressão e arquivo:linha em `skipped`.

Exemplo de receita exportada:

```json
{
  "name": "ass.plateiron",
  "label": "Placa de Ferro",
  "label_en": "Iron Plate",
  "duration": 60,
  "power": 100,
  "inputItems": [{"ids": ["minecraft:iron_ingot"], "count": 1}],
  "inputFluids": [],
  "outputItems": [["ntm:plate_iron", 1]],
  "outputFluids": [],
  "pools": ["alt.plates"],
  "source": "src/main/java/com/hbm/inventory/recipes/AssemblyMachineRecipes.java:67"
}
```

Na centrífuga, os campos são `input:{ids,count}` e `outputs:[[id,count],…]`. `duration` é em ticks, `power` em HE/t e os pares de fluido usam stringId/mB. Ingredientes OreDict conservam todas as alternativas disponíveis no catálogo; o representante vem primeiro, preferindo IDs `ntm:` e depois ordem lexical. A lógica deve usar a lista `ids`, não somente o representante.

O perfil exportado explicita `enable528=false`, `enableExpensiveMode=false`, `enableLBSM=false` e ausência de mods externos, conforme os defaults de `GeneralConfig`. `inputItemsEx`/`inputFluidsEx` e pools exclusivos de 528 respeitam esse perfil. A expansão não executa Java. Itens ausentes (equipamentos especiais, máquinas de outros marcos e subtipos não catalogados), laços de minérios de bedrock, probabilidades, pressão e expressões dinâmicas sem tradução ficam em `skipped`. Nenhuma receita temporária foi adicionada. As receitas da centrífuga de gás pertencem ao agente de lógica M3.

Os JSONs de exportação na raiz de `content/` são contratos para o build Rust; o `.hmod` de conteúdo empacota `assets/` e `data/`. O índice não é uma nova API de receitas do motor.

## Reprodução e evidências

```sh
/usr/local/bin/agent-run python3 tools/content-gen/generate.py
/usr/local/bin/agent-run python3 tools/content-gen/test_m3.py
/usr/local/bin/agent-run python3 tools/content-gen/validate-bulk.py
cd content
/usr/local/bin/agent-run /home/sacer/Games/horizon/target/release/cargo-horizon-mod build
/usr/local/bin/agent-run /home/sacer/Games/horizon/target/release/cargo-horizon-mod check target/horizon-mod/ntm.hmod
```

O gerador reconstrói `m3.json` da fonte antes de combinar os quatro catálogos, exporta as receitas de máquinas e preserva a integração existente de receitas vanilla, hazards/tags e oito abas creative-plus. A API base ignora as abas; o fork creative-plus é necessário para exibi-las.

Sete testes Python verificam os contratos fonte, restrições de recipientes, estados e opacidade das faces, conservação dos pacotes, receitas conhecidas, alternativas e rejeição de expressões não suportadas. A validação integral cobre os 2515 IDs e 807 receitas vanilla. Duas gerações compararam 11.838 arquivos sem diferença. `cargo test --workspace -j 3` passou com 153 testes e dois ignorados; log em `content-gen-evidence/m3/cargo-test.log`.

O launcher `tools/content-gen/m3/run.sh` usa `agent-run --user sacer`, Xvfb :10, diretório temporário, `HORIZON_NO_AUDIO=1`, master/music zero e `mute.py`. A amostra `m3/verify-game.py` verifica estados das seis máquinas, duto, 12 itens e loot real do barril. A rodada inicial passou com 54 comandos e nenhuma falha; o log registra áudio desligado e nenhuma recusa de conteúdo. No Xvfb houve uma falha ao abrir `/dev/input/event263`; os comandos e a amostra passaram. Seus comandos e screenshots estão em `content-gen-evidence/m3/`, identificados como `before-opacity`.

A inspeção visual dessa rodada detectou alpha nas texturas fonte das máquinas e do duto. O pacote final compõe as faces sobre aço; o teste de opacidade verifica todos os pixels das seis faces dos sete blocos. A **aceitação visual final passou**, com hash conferido pelo agente de lógica M3 em `/tmp/ntm-test-game/m3-visual`, Xvfb :9, cliente mudo e mundo criativo novo ao meio-dia. As vistas próximas, de frente e de costas confirmam faces opacas nas seis máquinas e no duto. As seis máquinas estavam em `honey_level=0`; o desenho dos níveis 1–5 não foi reavaliado nessa cena do pacote final. Os sete testes Python, validação estática, determinismo e build/check também passaram no pacote corrigido.

Capturas: [frentes](img/m3-visual-north-front.png), [frentes próximas à esquerda](img/m3-visual-north-close-left.png), [frentes próximas à direita](img/m3-visual-north-close-right.png), [costas](img/m3-visual-front.png), [costas à esquerda](img/m3-visual-close-left.png), [costas à direita](img/m3-visual-close-right.png). O índice com hashes das imagens está em `content-gen-evidence/m3/verification-status.json`.

Permanece uma melhoria estética sem bloqueio: as frentes da montadora e da planta química mostram o atlas UV dos modelos NTM. Um ícone frontal dedicado pode substituir esse atlas em um trabalho futuro; os modelos atuais conservam a aproximação cúbica documentada.
