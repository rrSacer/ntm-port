# Receitas do Marco 1

Fonte: HBM NTM 1.7.10 em `../ntm-src`, GPL-3.0. O gerador
`python3 tools/recipe-gen/generate.py` lê o catálogo atual e grava apenas os
arquivos que possui no manifesto `tools/recipe-gen/generated-files.json`.
Não apaga receitas de outros geradores. O pipeline de conteúdo o chama após
recriar `ids.json`. Nenhum arquivo do motor Horizon foi alterado.

## Contrato

- Conteúdo: `content/data/ntm/recipe/`, formato vanilla 26.3 (ingrediente string,
  tag `#namespace:path`, resultado `{id,count}`). Tags de equivalência locais em
  `content/data/ntm/tags/item/ingredients/`: representam os membros do catálogo
  que satisfazem o ingrediente original; não prometem integração automática com
  o ore dictionary de mods futuros.
- Rust: `crates/ntm-core/data/{press,furnace}_recipes.json`, `schema:1`, acordado
  com a fundação. Cada entrada contém `input:{item,count}`, `output:{item,count}`
  e `id`; `stamp` da prensa é um tipo (`plate`, `wire`), não um id de item.
  `stamps` mapeia os três moldes de ferro, com durabilidade original de 64 usos.
- Sem `ticks` por receita: a velocidade pertence à máquina. Metadados: fornalha
  100 ticks e 50 HE/t; prensa elétrica 100 HE/t; prensa combustível com aceleração
  até 400, progresso máximo 25 e operação base 200. Fontes:
  `TileEntityMachineElectricFurnace.java:42-43,151-152`,
  `TileEntityMachineEPress.java:84-86`, `TileEntityMachinePress.java:26-36`.
- A fornalha elétrica original consulta `FurnaceRecipes.smelting()`. A tabela Rust
  inclui as fusões NTM representáveis e **todas** as fusões vanilla do bundle local
  26.3, expandindo tags de ingredientes. A extração inclui receitas modernas de
  vidro, comida, terracota e carvão, além dos minérios.
- `coverage.json` guarda contagens, origem de cada receita e ids sem saída de
  craft/fusão. Os JSON de conteúdo não recebem campos próprios de proveniência.

## Original versus adaptação

Pares bloco↔material (9), barra↔pepita (9), tarugos U/Be (6 pepitas;
2 barras↔3 tarugos) preservam `MineralRecipes.java`. Pepitas genéricas
Al/Li/Ti/W vêm de `OreDictManager.java:666-670`. Fusões preservam saída,
quantidade e experiência de `SmeltingRecipes.java`. Blasting é uma extensão
explícita do port 26.3, com as mesmas saídas e metade do tempo da fusão vanilla.

A prensa manual preserva o craft de `CraftingManager.java:288`. A tabela de
prensa preserva três placas (`PressRecipes.java:89-97`) e os quatro fios finos
(`PressRecipes.java:114-118`, oito por barra). Não produz placas fundidas,
soldadas ou fios densos na prensa.

O coordenador autorizou pontes temporárias para dependências fora do M1.
Todas estão em **`recipe/m1_temp__*.json`**, para remoção sem ambiguidade quando suas
estações originais chegarem. Cada padrão abaixo usa os símbolos indicados.

| Saída / padrão | Receita original e lacuna | Ponte M1 | Remover quando |
| --- | --- | --- | --- |
| Gerador `PPP/CFC/I I` | `CraftingManager.java:251`: P placa de aço, C bobina de cobre, F fornalha, I ferro | P placa de titânio, C fio de cobre vermelho; F/I preservados | Alto-forno e bigorna NTM/bobinas |
| Cabo ×16 ` W /RRR/ W ` | `CraftingManager.java:239`: W placa de polímero, R fio de cobre vermelho | Couro substitui W | Cadeia de polímeros |
| Fornalha elétrica `BBB/WFW/RRR` | `CraftingManager.java:230`: B berílio, W placa fundida de cobre, R bobina de tungstênio, F fornalha | W placa de alumínio, R fio fino de tungstênio | Fundição e bobinas |
| Prensa elétrica `PPP/RMR/WCW` | `AssemblyMachineRecipes.java:354-356`: 8 placas aço, 4 borrachas, 2 pistões hidráulicos, circuito básico; assembler | 3 placas Ti, 2 pistões vanilla, prensa manual, 2 fios cobre vermelho e redstone | Assembler, circuitos e hidráulica |
| Bateria `PPP/RLR/WWW` | `machine_battery` não tem craft vanilla registrado na fonte atual | 3 placas chumbo, 2 redstones, lítio e 3 fios cobre vermelho | Cadeia original de baterias e definição da variante |
| Carretel `PPP/PIP/PPP` | `CraftingManager.java:539`: P placa aço, I barra aço | P placa Ti, I barra ferro | Alto-forno/aço |
| Molde plate ` I / S / I ` | `AnvilRecipes.java:588`: flat→plate em bigorna NTM | S molde flat + 2 ferros | Bigorna NTM |
| Molde wire `I I/ S /I I` | `AnvilRecipes.java:589`: flat→wire em bigorna NTM | S molde flat + 4 ferros | Bigorna NTM |
| Alumínio do minério | `SmeltingRecipes.java:40`: minério→cryolite, não barra | Fusão direta minério→barra ×1, XP 2.5 | Beneficiamento de criolita |
| Cobre vermelho ×2 (shapeless) | `BlastFurnaceRecipes.java:54`, `BlastFurnaceRecipesNT.java:46-54`: cobre+redstone no alto-forno | 1 barra de cobre vanilla + 1 redstone na bancada | Alto-forno |

Moldes flat preservam os dois crafts originais (3 tijolos normais **ou** 3
nether bricks sobre 3 barras de ferro, `CraftingManager.java:346-354`). O
catálogo foi ampliado apenas com `stamp_iron_flat`, `stamp_iron_plate` e
`stamp_iron_wire`; tamanho da pilha 1, desgaste aplicado pela lógica Rust.
Aço, bobinas e novas máquinas auxiliares não foram incluídos: sua produção
original depende de estações fora do M1.

## Lacunas e limites de cobertura

- Minérios são obtidos no mundo; cristais requerem cristalizador; pós dependem
  de triturador/processamento. Não foram inventados crafts de bancada para eles.
- Placas fundidas, placas soldadas e fios densos continuam no catálogo visual,
  porém dependem de fundição/soldagem/bigorna; estão listados em
  `coverage.json:without_vanilla_recipe`. Apenas placas simples/fios finos
  entram na tabela da prensa do M1.
- `CraftingManager.java:947-976` tem receitas opcionais LBSM: 4 barras→4 placas,
  3 barras→24 fios, e lavagem de urânio queimado. `GeneralConfig.enableLBSM`
  é falso por padrão; este port preserva esse padrão, não habilita os atalhos.
  `PowderRecipes.java:79` contém liga em pó opcional LBSM, mas depende de pó
  de cobre ausente do catálogo. São lacunas explícitas de perfil opcional.
- Receitas alternativas de reciclagem/isótopos que retornam barras/tarugos do
  catálogo mas consomem RTGs, combustíveis nucleares e isótopos ausentes não são
  emitidas com ingredientes inválidos. Exemplos `MineralRecipes.java:203-220`.
- Não há parser Java universal: fusões diretas são extraídas da fonte com uma
  expressão restrita; famílias de craft são curadas. Atualizar a fonte exige
  reavaliar estas famílias e a cobertura.

## Validação

Ver `recipe-gen-evidence/`: cliente isolado em `/tmp/ntm-recipes-*`, opções
master/music 0.0, `HORIZON_NO_AUDIO=1`, monitor PipeWire para mutar qualquer
stream. O log confirma áudio desativado antes de iniciar o mundo. Não se usa
`~/.minecraft` nem o diretório permanente do Horizon.

Resultado em 2026-10-28: **119 receitas** (36 shaped, 26 shapeless,
29 smelting, 28 blasting), das quais 10 pontes temporárias; 7 receitas de
prensa e 192 entradas da fornalha Rust (29 NTM, 163 vanilla).
`validate.py` confirmou geração determinística e referências válidas de
itens/tags/moldes. O log não tem `recipe ... not loaded`. Três crafts reais
passaram com retirada do resultado: 9 barras U→1 bloco, 1 barra U→9 pepitas,
e cobre+redstone→2 barras de cobre vermelho. Evidências completas em
`recipe-gen-evidence/crafts.json` e `probe.json`.

Dois detalhes do motor foram observados sem alterá-lo:

1. Receitas em subpastas não são enumeradas (`crafting.rs:1183`); por isso o
   prefixo plano `m1_temp__` é obrigatório. O validador impede regressão.
2. `/recipe give @s ntm:<id>` respondeu `Unknown recipe` mesmo para os crafts
   que efetivamente funcionaram. Esse comando não serve como prova de carga
   deste conteúdo neste build; a tentativa está preservada em
   `registered-recipes.json`. Ausência de rejeição no log e crafts bem-sucedidos
   verificam os formatos exercitados, mas não constituem teste funcional
   individual de todas as 119 receitas.
