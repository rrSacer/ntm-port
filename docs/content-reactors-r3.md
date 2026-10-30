# Reatores R3: PWR

Entram 22 IDs, levando o catálogo a 2.962 (504 blocos e 2.458 itens). Só conteúdo: o controlador e o bloco interno entram como Inert no registro nativo. O R3 estende o R2 (mesmos geradores, extrator de hazards e perfil de prova).

Já existiam, vindos do import em massa, os seis blocos de estrutura (`pwr_casing`, `heatex`, `heatsink`, `neutron_source`, `port`, `reflector`) e os 30 itens `pwr_fuel_hot__*`/`pwr_fuel_depleted__*`. O R3 acrescenta o que o import em massa havia pulado:

| Grupo | IDs (prefixo `ntm:`) |
| --- | --- |
| Colunas (3) | `pwr_fuel`, `pwr_control`, `pwr_channel` (`BlockPillarPWR`; cubos com lado/topo originais, como os demais `BlockPillar`) |
| Controlador (1) | `pwr_controller` (beehive, facing, honey_level 0..5, `auto: false`) |
| Bloco interno (2) | `pwr_block__internal` (meta 0) e `pwr_block__port` (meta 1): beehive, ocultos no criativo, sem loot, como no fonte (`getItemDropped` nulo, `setCreativeTab(null)`) |
| Combustível (15) | `pwr_fuel__<meu, heu233, heu235, men, hen237, mox, mep, hep239, hep241, mea, hea242, hes326, hes327, bfb_am_mix, bfb_pu241>` |
| Impressora (1) | `pwr_printer` |

**Homônimo `pwr_fuel`.** No fonte, `ModBlocks.pwr_fuel` (coluna) e `ModItems.pwr_fuel` (barra, 15 metas) usam o mesmo nome. Resolvido por id distinto: o **bloco** continua `ntm:pwr_fuel`; as **barras** são `ntm:pwr_fuel__<enum>`, o mesmo esquema de sufixo dos `pwr_fuel_hot__*` e `pwr_fuel_depleted__*` já existentes. Os IDs não colidem (o item de bloco de `pwr_fuel` é o único `ntm:pwr_fuel` sem sufixo). O contrato está em `identities` de `content/machine-recipes/reactors_pwr.json`, e as linhas do catálogo compartilham `source_variable: pwr_fuel` com o `type` (bloco/item) distinguindo-as.

**Tabela de combustível** (`reactors_pwr.json`): para cada um dos 15, calor por fluxo (`heat_emission`), função de reação (`FunctionLogarithmic` com nível e divisor, ou `FunctionSqrt`), as três formas (`fresh`, `hot`, `depleted`) e os dois tanques do controlador (COOLANT e COOLANT_HOT, 128.000 mB) e as capacidades-base de calor do núcleo e do casco (10.000.000). **Peculiaridade do fonte:** `EnumPWRFuel` tem um construtor com rendimento, mas nunca atribui o argumento: todos, inclusive `BFB_AM_MIX` e `BFB_PU241`, ficam com rendimento 1.000.000.000. A tabela registra `yield` 1.000.000.000 e o `declared_yield` (250.000.000) ignorado, sem corrigir o fonte.

**Receitas originais** (+15 arquivos): as 14 cargas `RodRecipes` do PWR (uma delas, a BFB, em duas formas) viram receitas vanilla; montadora 461 (+`ass.pwrfuel`, `ass.pwrcontrol`, `ass.pwrchannel`, `ass.pwrcontroller`). Sem mudança no PUREX (as 15 reciclagens de PWR já existiam) e na bigorna. O `pwr_printer` não tem receita no fonte.

**Hazards.** O extrator lê `registerPWRFuel`: barra fresca = taxa base, quente = 10× mais HOT 5, esgotada = 10×. Isso também preenche os 30 itens `hot`/`depleted` que antes estavam sem registro resolvido (zero), então as taxas de 30 identidades antigas deixam de ser zero. Nenhuma taxa não nula antiga mudou.

**Lacunas.** Validação de estrutura e conversão em `pwr_block`, fluxo/calor/refrigerante, barras de controle, portas, falha e a GUI da impressora ficam como trabalho nativo; `FT_Heatable`/`FT_PWRModerator` continuam nos contratos de fluido.

**Prova e validação.** `verify-bedrock-processing.py --profile reactors-r3`: 22 identidades, 80 conferências, três colunas e três máquinas em dois níveis, sem `reject` nem `panic` no log do cliente nativo. 196 testes por seed 1/42, `validate-bulk` e `recipe-gen/validate` verdes. Pacote `ntm.hmod` SHA-256 `8af1be6222e44ba6dad2a1ef496cb7fa4be8b643d45cc04095e80fb178825916` (sobre o R2), aprovado no check do upstream e do fork.
