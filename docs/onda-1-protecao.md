# Onda 1: proteção, medicina e radiação

Escopo e ordem das fatias: [onda-1-escopo.md](onda-1-escopo.md). Os hooks de item, a dose no
attachment do jogador e a mesa de armadura estão em [native-item-hooks.md](native-item-hooks.md).

## Fatia 1: regra de proteção do Java

### Tabela gerada do `HazmatRegistry`

`crates/ntm-core/data/hazmat.json` é escrita por `tools/content-gen/hazmat.py` a partir de
`handler/HazmatRegistry.java` (`initDefault`) e das linhas `new ItemModCladding(rad)` de
`items/ModItems.java:2859-2863`. Cada registro é o produto de dois `double` (material × fração do
corpo) avaliado em IEEE double, como na JVM: `0.6 * 0.2` fica `0.12`, `0.07 * 0.2` fica
`0.014000000000000002`. A tabela de antes era escrita à mão e só tinha hazmat e M65.

| Origem | Peças no port | Antes |
|---|---|---|
| hazmat amarelo/vermelho/cinza, M65 (`HazmatRegistry.java:68-81,111`) | 13 | iguais |
| estelar (`:102-105`), aço e titânio (`:113-121`), cobalto (`:123-126`), liga (`:138-141`) | 20 | 0 |
| ferro e ouro vanilla (`:128-136`) | 8 | 0 |
| liquidator, PAA, security, jaquetas, `gas_mask`, CMB, schrabidium, euphemium | 0 (30 em `unported`) | — |

As 30 peças sem item no port ficam em `unported` com o nome Java e entram sozinhas quando o item
existir (o teste falha se o catálogo ganhar uma delas e a tabela não for regenerada).
`python3 tools/content-gen/hazmat.py --check` sai com 1 se a tabela divergir do Java;
`tools/content-gen/test_hazmat.py` relê o Java com outro parser (materiais e frações por nome) e
confere os 67 produtos, os literais (`gas_mask_m65` 0,095; `gas_mask` 0,07; `jackt` 0,1) e os
revestimentos.

### Aritmética do jogador

`HazmatRegistry.getResistance(EntityPlayer)` (`:201-217`) soma num `float`: `res += d` estreita
para `float` depois de cada peça (`double`), e o Rad-X soma `0.2F`. `calculateRadiationMod`
(`ContaminationUtil.java:34-41`) devolve `(float) Math.pow(10.0F, -res)`. O port faz o mesmo
(`item_hooks::player_resistance` e `radiation_mod`). Efeito visível: o conjunto de aço soma
0,045000002 (não 0,045), o Geiger mostra `9.84% (0.04)` e a dose cai para `int(bruta × 0,90157115)`.
O antigo `10^(-soma em double)` diferia na sétima casa.

`ShadyUtil.Pu_238` (+0,4 para um UUID específico de desenvolvedor) não foi portado: depende da
identidade de um jogador real do Java.

### Revestimento

`getCladding` (`:183-198`): se a pilha tem o `float` `hfr_cladding` maior que 0, vale ele; senão,
o item no slot 5 (`ArmorModHandler.cladding`) de `ntm_armor_mods`, se for `ItemModCladding`. O valor
soma ao da peça (`f + cladding`), e uma peça sem registro também recebe o revestimento. O port lê os
dois do `minecraft:custom_data` da peça vestida (`worn_resistance`).

Entram no catálogo os cinco `ItemModCladding`: `cladding_paint` (0,025), `cladding_rubber` (0,005),
`cladding_lead` (0,1), `cladding_desh` (0,2) e `cladding_ghiorsium` (0,5), pilha 1. Receitas de
`ConsumableRecipes.java:152-156`, todas emitidas; a da borracha usa o `DictGroup` `ANY_RUBBER`
(látex e borracha), resolvido pelo mesmo expansor de grupos que as brocas manuais já usavam
(`bulk_recipes.py`). `cladding_iron` e `cladding_obsidian` são `ItemArmorMod` do mesmo slot, mas
mudam atributos (empurrão, dano), não radiação: ficam para a família de mods de armadura.

A mesa de armadura segue `ContainerArmorTable` (`:197`): o slot de mod aceita um `ItemArmorMod`
só com uma armadura na mesa, `ArmorModHandler.isApplicable` (tipo da armadura) e `type == slot`.
`item_hooks::armor_mod` traz o `ItemModGasmask` (slot 0, só capacete) e os revestimentos (slot 5,
os quatro tipos); a mesa, o shift-click e o filtro de slot leem essa tabela.

### Tooltips

`ModEventHandlerClient.java:709-712`: toda pilha com resistência positiva mostra
`Radiation resistance: X` (truncado em três casas, com o revestimento). `ItemModCladding` e
`ItemArmorMod` acrescentam `+X rad-resistance`, `Applicable To: All` e `Slot: Cladding`. O texto é o
en_US do Java (como o relatório do Geiger), sem tradução.

### Armaduras simples do HBM

Aço, titânio, liga, cobalto e estelar passam a ter os pontos e a durabilidade do `ArmorMaterial`
(`MainRegistry.java:135-144`): redução `{3, 8, 6, 3}` e durabilidade `{11, 16, 15, 13}[tipo] ×
fator` (aço 330/480/450/390). A redução vira o atributo `minecraft:armor` da peça e substitui os do
modelo vanilla (o diamante e a netherita davam robustez e resistência a empurrão que o Java não tem).
Continuam diferentes, nominalmente: encantabilidade, material de reparo e a textura vestida (a do
modelo vanilla); a fórmula de dano por armadura é a do motor moderno (a mesma aceita para o hazmat).

### Prova

Binário `horizon-client` `c9388c3c84286cca…` (release, motor no pin de `compatibility.json`, sem
mudança no motor) e pacote `ntm.hmod` `4a28877f908bed71…`, o mesmo par em todas as fases
(`docs/evidence/onda1-f1/summary.json`).

| Fase do roteiro `items` (família `protection`) | Checagens |
|---|---|
| `setup`: linha de resistência do Geiger dos conjuntos de aço, titânio, liga, cobalto, estelar, ferro e ouro vanilla e hazmat cinza, contra a fórmula do Java calculada no roteiro (soma `float` e `(float) pow`); 20 pontos de armadura do aço; durabilidade 480 do peitoral (a 478 sobrevive a um golpe de cacto e quebra no seguinte); dose real com aço `int(350 × 0,90157115) = 315`; `hfr_cladding` 0,25; revestimento recusado sem armadura e no slot da máscara, aceito no slot 5, mantido nos mods e somado no capacete vestido | 19/19 |
| `capture-state`: capacete hazmat com máscara e revestimento de chumbo, conta antes de fechar | 12/12 |
| `check-state`: depois de fechar e reabrir o mundo, os mods (máscara e revestimento) continuam no capacete e o Geiger dá 39,74% (0,21) | 8/8 |

Verificação estática: 517 testes de unidade do `ntm-core`, os 9 arquivos de integração em debug e
`save_model` (11) em release; Clippy `-D warnings` do `ntm-core` e da cópia transformada do motor
(`horizon-client --lib`); `test_hazmat.py` (6) e `hazmat.py --check`. Mutações: 10 de 10 mortas
(soma em `double`, Rad-X 0,25, precedência e limite do `hfr_cladding`, slot 4, máscara no peitoral,
tooltip com duas casas, fator alterado na sétima casa, `koeff` arredondado, valor do peitoral de aço
na tabela); a de controle (equivalente) passa.

A regressão inteira do roteiro `items` (todas as famílias) não fechou nesta máquina: quatro
tentativas no mesmo binário pararam em quatro checagens de tempo diferentes e alheias à fatia
(`syringe_count_and_container`, `thrown_container_waits_its_pickup_delay`,
`coffee_radium_adds_five_hundred_rad`, `cancel_does_not_consume`), com carga entre 37 e 66. Três delas passaram em outra tentativa; `coffee_radium` (um gole de 32
ticks segurado por 1,8 s reais) não chegou a passar em nenhuma, porque a quarta parou antes dele.
Fica pendente repetir com a máquina livre.
