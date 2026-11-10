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

`HazmatRegistry.getResistance(EntityPlayer)` (`:201-217`) soma num `float` sobre
`armorInventory[0..3]`, **das botas ao capacete** (`:209-211`): `res += d` estreita para `float`
depois de cada peça (`double`), então a ordem muda o resultado; o Rad-X soma `0.2F` no fim.
`calculateRadiationMod` (`ContaminationUtil.java:34-41`) devolve `(float) Math.pow(10.0F, -res)`.
O port faz o mesmo (`item_hooks::player_resistance`, com as peças das botas ao capacete, e
`radiation_mod`). O conjunto de aço soma 0,044999998 no Java; o Geiger mostra `9.84% (0.04)` e a
dose cai para `int(bruta × 0,90157115)`. Num conjunto misto a ordem aparece no texto: capacete de
ferro, peitoral de aço, calça de cobalto e botas hazmat vermelhas dão `30.81% (0.16)` no Java
(somando do capacete às botas seriam `(0.15)`).

A revisão independente (artifact `revisao-onda1-fatia1`) achou a ordem invertida na primeira
entrega. Desde então o esperado vem de um oráculo Java executável, `tools/hazmat-oracle/run.py`: os
corpos de `getResistance(EntityPlayer)`, `getResistance(ItemStack)`, `getCladding`,
`calculateRadiationMod` e as duas linhas de número do `printGeigerData` são recortados do fonte, sem
tradução, e compilados com stubs mínimos. O fixture (`fixture.json`) tem 1503 conjuntos (os do
revisor e 1500 sorteados, com revestimento em tags de todo tipo numérico, mods e Rad-X) e 500
expoentes do `pow`, 150 deles onde um `powf` em `float` erraria; o teste de unidade
`the_java_oracle_agrees_bit_for_bit` compara bits e texto. `--check` refaz o fixture.

`ShadyUtil.Pu_238` (+0,4 para um UUID específico de desenvolvedor) não foi portado: depende da
identidade de um jogador real do Java.

### Revestimento

`getCladding` (`:183-198`): se a pilha tem `hfr_cladding` maior que 0, vale ele (`getFloat`: qualquer
tag numérica, convertida para `float`); senão,
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

### Tooltips e indicador da mesa

`ModEventHandlerClient.java:709-712`: toda pilha com resistência positiva mostra
`Radiation resistance: X` (truncado em três casas, com o revestimento). `ItemModCladding` e
`ItemArmorMod` acrescentam `+X rad-resistance`, `Applicable To: All` e `Slot: Cladding`.
`:713-735`: uma armadura com mods mostra "Hold <LSHIFT>..." ou, com Shift, `Mods:` e o `addDesc` de
cada mod dos slots 0 a 7 (`Lead Cladding (+0.1 radiation resistance)`, `Gas Mask Attachment (gas
protection)`). Desvio: dentro da mesa o Java lista os mods sem Shift; o gancho de tooltip não sabe
qual tela está aberta, e o port pede o Shift também ali. As linhas do filtro do `ItemModGasmask`
(`addGasMaskTooltip`) não foram portadas. O texto é o en_US do Java (como o relatório do Geiger);
o pt_BR (`trait.radResistance`, `armorMod.*`) é lacuna nominal.

`GUIArmorTable.drawIndicator` (`:99-111`): o quadrado de cada mod fica verde (176,34) quando
`ArmorModHandler.isApplicable(armor, mod)`, vermelho (176,16) caso contrário; o quadro da peça é
176,74 com uma armadura, 176,52 com outra coisa e 176,52 piscando com a mesa vazia. Antes da revisão
o verde só valia para o slot 0, e o revestimento aparecia vermelho.

### Lacunas nominais da fatia

- Aba criativa: os revestimentos estão em `parts`, como o `attachment_mask` da `main`; no Java os
  `ItemArmorMod` ficam na `consumableTab`.
- Durabilidade: o `max_damage` é o valor do Java (480 no peitoral de aço); no 1.7.10
  `attemptDamageItem` só quebra com `itemDamage > maxDamage`, um uso a mais (481). Regra global do
  projeto, não desta fatia.
- `ShadyUtil.Pu_238` (+0,4 para o UUID de um jogador real do Java): não portado; decisão do usuário.

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

Depois da revisão (commits `1e2e4e6f`, `872dfd7f`, `9791fbd6`): a família `protection` ganhou os
dois conjuntos mistos do oráculo Java e o `hfr_cladding` em `double` (22 checagens), e passou junto da
família `masks` no binário `fd1a0482…` (ver a prova da fatia 2). Teste de unidade
`the_java_oracle_agrees_bit_for_bit` (1503 conjuntos, 500 expoentes); mutações novas mortas: ordem
invertida, `pow` em `float`, sem Rad-X, `hfr_cladding` alterado; `test_hazmat.py` 9/9 (inclui o
oráculo com `--check`); Clippy `-D warnings` do `ntm-core` e da cópia transformada do motor.

Verificação estática da primeira entrega: 517 testes de unidade do `ntm-core`, os 9 arquivos de integração em debug e
`save_model` (11) em release; Clippy `-D warnings` do `ntm-core` e da cópia transformada do motor
(`horizon-client --lib`); `test_hazmat.py` (6) e `hazmat.py --check`. Mutações: 10 de 10 mortas
(soma em `double`, Rad-X 0,25, precedência e limite do `hfr_cladding`, slot 4, máscara no peitoral,
tooltip com duas casas, fator alterado na sétima casa, `koeff` arredondado, valor do peitoral de aço
na tabela); a de controle (equivalente) passa.

Atomicidade da mesa num corte de gravação: cada clique (`armor_table::clicked`) monta numa cópia do
snapshot do jogador o inventário, a armadura vestida, o cursor e a peça da mesa (attachment
`ntm:armor_table`) e publica tudo num único `native_commit_player` (`armor_table.rs`, no fim de
`clicked` e em `closed`); a troca armadura/revestimento não tem publicação parcial. Esse estado vai
para o arquivo do jogador, salvo inteiro pelo motor. A revisão tentou um corte de gravação da mesa e
viu o controle vanilla perder tudo nos mesmos cortes, o que torna o resultado inconclusivo. O corte
de gravação da fatia 2 (`run-filter-copy-proof.py`) passa pelo mesmo `native_commit_player` e conta
o arquivo do dono (`singleplayer_uuid` do `level.dat`; o mundo de teste guarda um segundo arquivo de
jogador, que somado dobra a conta e pode explicar o resultado da revisão), contra um controle
vanilla no mesmo arquivo: os resultados estão na seção da fatia 2. Não há prova de corte específica
da mesa com cliques de menu.

A regressão inteira do roteiro `items` (todas as famílias) não fechou nesta máquina: quatro
tentativas no mesmo binário pararam em quatro checagens de tempo diferentes e alheias à fatia
(`syringe_count_and_container`, `thrown_container_waits_its_pickup_delay`,
`coffee_radium_adds_five_hundred_rad`, `cancel_does_not_consume`), com carga entre 37 e 66. Três delas passaram em outra tentativa; `coffee_radium` (um gole de 32
ticks segurado por 1,8 s reais) não chegou a passar em nenhuma, porque a quarta parou antes dele.
Fica pendente repetir com a máquina livre.

## Fatia 2: máscaras, trajes e a proteção do `ArmorRegistry`

### Itens novos

| Itens | Classe Java | Pontos e durabilidade | Resistência | Classes próprias (`ArmorUtil`) |
|---|---|---|---|---|
| `gas_mask`, `gas_mask_olde`, `gas_mask_mono` | `ArmorGasMask` (`IGasMask`) | IRON: 2, 165 | `gas_mask` 0,07 | `gas_mask`: SAND, LIGHT |
| `goggles`, `ashglasses` | `ArmorModel`, `ArmorAshGlasses` | IRON: 2, 165 | — | LIGHT, SAND |
| `mask_rag`, `mask_piss` | `ModArmor` (`aMatRags`) | 1, 1650 | — | PARTICLE_COARSE (+ GAS_LUNG no `mask_piss`) |
| `hazmat_paa_*` | `ArmorHazmat(Mask)` (`aMatPaa`) | 3/8/6/3, 825/1200/1125/975 | 1,7 × fração | capuz: LIGHT, SAND |
| `liquidator_*` | `ArmorLiquidator(Mask)` | 3/8/6/3, 8250/12000/11250/9750; +100 de resistência a empurrão (limitada a 1) e −10% de velocidade por peça (`ArmorLiquidator.java:55-56`) | 2,4 × fração | capuz: LIGHT, SAND |
| `asbestos_*` | `ArmorFSB` (`aMatAsbestos`) | 1/4/3/1, 220/320/300/260 | — | capacete: SAND, LIGHT |
| `jackt`, `jackt2` | `ModArmor` (`aMatSteel`, peito) | 8, 480 | 0,1 | — |

Todos têm a receita do Java (`ArmorRecipes.java:134-161`, `CraftingManager.java:614`); as que pedem
`ANY_RUBBER` ou `ANY_PLASTIC` passam pelo expansor de `DictGroup`.

### Regra de proteção

`hazmat.py` também gera as classes de perigo: as linhas de `ArmorUtil.register` (`:39-64`) e, depois,
os `setHazardClass` das armaduras FSB (`ArmorUtil.external`, que o Java registra por último e que
substituem). `item_hooks::protection` é `ArmorRegistry.getProtectionFromItem`: as classes do próprio
item, as do filtro menos a lista negra quando o item é `IGasMask`, e o mesmo, recursivamente, para
cada mod. `has_protection` e `has_any_protection` são `hasProtection`/`hasAllProtection` de uma
classe e `hasAnyProtection` (só o capacete, slot 3). O enum ganhou `GAS_INERT`, `SAND` e `LIGHT`.

Listas negras (`getBlacklist`): `gas_mask_mono` recusa gás pulmonar, vesicante e bactérias; as outras
`ArmorGasMask` e o `ItemModGasmask`, gás vesicante; os capuzes `ArmorHazmatMask` e
`ArmorLiquidatorMask` não têm restrição.

`filter_exposure` segue o padrão dos consumidores Java (`BlockGasClorine`, `HazardTypeCoal`...):
`if(hasProtection) damageGasMaskFilter(entity, wear)`. Antes, o port só protegia pelo filtro; agora
uma máscara de pano protege de partículas grossas sem filtro (nada a gastar), e um capuz com filtro
gasta o filtro também quando a proteção vem da classe própria do capuz, como no Java.

### O bug de cópia do filtro (divergência de integridade intencional)

`ArmorUtil.damageGasMaskFilter(ItemStack mask, int)` (`ArmorUtil.java:332-352`): quando o capacete é
`IGasMask` mas não tem filtro próprio, o Java lê o filtro da máscara acoplada (`helmet_only`), gasta e
grava a cópia gasta no **capacete** com `installGasMaskFilter(mask, filter)`, enquanto o mod continua
com o filtro. Agachar e usar o capacete tira a cópia: um filtro vira dois. Por decisão do coordenador
(10/11), o port reproduz o bug (fidelidade ao Java, inclusive bugs), atrás de uma única constante,
`ntm_core::item_hooks::JAVA_FILTER_COPY_BUG` (`false` gasta o filtro do próprio mod). É uma
divergência intencional da regra geral "sem duplicação" desta base; **o usuário decide se mantém**.
A cópia nasce de uma ação do jogador e passa pelo commit transacional normal do jogador; a prova de
corte de gravação (`tools/test-game/run-filter-copy-proof.py`) mostra que o save não acrescenta perda
nem uma segunda duplicação.

`item_hooks::filter_wear` escolhe o que o Java gasta: o filtro do capacete; o do mod quando o
capacete não é `IGasMask` (`:317-326`); a cópia no capacete (o bug); ou nada.

### Prova da fatia 2

Em jogo, depois das correções da revisão da fatia 1 (binário `fd1a0482b1abfdc0…`, código igual ao
commit `9791fbd6`; pacote `42525a6b6a5e1d1e…`), famílias `protection` e `masks`: `setup` 51/51,
`capture-state` 12/12, `check-state` 8/8 (`docs/evidence/onda1-f2/items-protection-masks.json`). Os
conjuntos mistos da revisão dão o texto do oráculo Java: `30.81% (0.16)` e `65.32% (0.45)`; um
`hfr_cladding` em tag `double` conta como o `float`. A rodada anterior (binário
`41424521bf6974cd…`, sem as correções) deu 48/48, 12/12 e 8/8. Linhas do Geiger iguais ao Java: PAA
`98.0% (1.69)`, liquidator `99.6% (2.39)`, `gas_mask` `14.88% (0.07)`, `jackt` `20.56% (0.1)`;
liquidator com resistência a empurrão 1.0 e velocidade 0,06; filtro instalado nas cinco máscaras
novas; `gas_mask_mono` recusa gás pulmonar (nada gasto) e gasta 40 com monóxido; máscara de pano,
óculos, cinzas, amianto e M65 protegem pelas próprias classes sem gastar nada; o bug de cópia
reproduzido inteiro (protege pelo mod, cópia gasta de 40 no capuz, agachar devolve a cópia e o mod
continua com o filtro).

Corte de gravação do bug contra o controle vanilla (`run-filter-copy-proof.py`, binário `41424521…`,
`docs/evidence/onda1-f2/filter-copy-cuts.json`):
8 cortes, mod e controle iguais em todos, as duas reaberturas iguais ao arquivo do dono e entre si.

| Corte | Mod (filtros) | Controle (filtros) |
|---|---|---|
| `none` | 2 | 2 |
| `kill`, `save_begin`, `chunks_written`, `level_saved` | 1 (último save) | 1 |
| `players_saved`, `world_saved`, `kv_committed` | 2 | 2 |

O controle recebe o segundo filtro por `/give` (mesmo arquivo do jogador). Nenhum corte cria um
terceiro filtro nem perde o primeiro. Essa prova rodou antes das correções da revisão da fatia 1,
que mudam só a ordem da soma da resistência e um `unwrap_or_else(Compound::new)` equivalente no
leitor do capacete; o caminho do filtro e do commit é o mesmo.

Testes: 12 de `item_hooks` (listas negras, classes, recursão nos mods, seleção do desgaste); mutações
10/10 mortas (lista negra do mono, `olde` e capuzes novos fora, sem recursão, sem lista negra, `any`
como `all`, sem o bug, mod sem filtro gasta, tag de filtro em capacete comum, `SAND` como `LIGHT`);
`test_hazmat.py` 8/8; `hazmat.py --check`.

### Lacunas desta fatia

- Visual: as camadas de tela das máscaras (`renderHelmetOverlay`: embaçado por desgaste, escuro do
  liquidator, amianto) e os modelos vestidos (`ModelM65`, `ModelGasMask`, `ModelGoggles`,
  `ModelGlasses`) usam o modelo vanilla.
- Som: passos, pulo e queda do conjunto liquidator (`ArmorFSB.handleTick/Jump/Fall`,
  `hbm:step.metal`, `step.iron_jump`, `step.iron_land`) dependem de ganchos de passo e queda no motor.
- Tooltips das máscaras (`ArmorUtil.addGasMaskTooltip`, `hazard.neverProtects`).
- Nenhuma fonte ambiental do port chama ainda `filter_exposure` (gases, poeiras, cinzas): entram com
  as famílias de poluição e gases.
