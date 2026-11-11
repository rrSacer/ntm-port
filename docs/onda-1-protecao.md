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

Mesa num corte de gravação: cada clique (`armor_table::clicked`) e o fechamento publicam inventário,
armadura vestida, cursor e a peça da mesa (anexo `ntm:armor_table`) num único `native_commit_player`,
sem publicação parcial. Isso não basta para a mesa sobreviver a um corte: a revisão
(`revisao-onda1-fatia1`, re-revisão) mostrou que, depois de um save no meio da sessão, o arquivo do
dono fica com o inventário novo e sem nenhum anexo, e o anexo da mesa só no arquivo do outro UUID do
jogador, que a reabertura não lê: a peça e o revestimento somem. A causa é um defeito do motor
(seção "Defeito do motor: anexos do dono num save no meio da sessão", abaixo). O corte do bug de
cópia (fatia 2) não cobre a mesa: o filtro é componente de item, que vai com o inventário.

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

## Fatia 3: descontaminação (`ntm:decon`)

`TileEntityDecon.updateEntity` (servidor, todo tick): as entidades vivas dentro da caixa
`(x − 0,5, y, z − 0,5) .. (x + 1,5, y + 2, z + 1,5)` perdem 0,5 RAD (`incrementRadiation(e, -0.5F)`,
sem nenhuma condição: criativo também), o efeito `HbmPotion.radiation` e a lista de contaminação
(`HbmLivingProps.getCont(e).clear()`).

No port: `radiation::decon_reaches` é o teste de caixa (`getEntitiesWithinAABB`: a caixa do jogador,
0,6 × 1,8, cruza a do `decon`, estrito nos três eixos) e `decon_cells`, as células que podem alcançar
um jogador; `Server::decon_tick` (todo tick, antes do `radiation_tick`) conta os `decon` que
alcançam cada jogador, tira 500 milliRAD por `decon` (vários somam, como no Java, em que cada bloco
age por si) num commit da dose do jogador, e encerra a irradiação (`zirnox.irradiated`, o equivalente
do port ao `HbmPotion.radiation`, que o micélio radioativo e os destroços aplicam). Sem dose, nada é
publicado. O bloco continua `Kind::Inert` no registro: o `TileEntityDecon` não guarda estado, então
não há entidade de bloco a criar nem a salvar.

Lacunas nominais: mobs não são descontaminados (o port não guarda dose de mobs); a lista de
contaminação (`ContaminationEffect`) não existe no port; a partícula `townaura` que o cliente emite
todo tick em cada `decon` carregado (`TileEntityDecon:31-43`, cor 0,8/0,9/1,0 × [0,5, 1]) não é
emitida: seria preciso conhecer as posições de todos os `decon` carregados, não só dos que estão
perto de um jogador.

### Correção encontrada pela prova: o micélio radioativo só irradiava com destroços do ZIRNOX

`WasteEarth.onEntityWalking` (`WasteEarth.java:81-92`): pisar no `waste_mycelium` dá
`HbmPotion.radiation` (30 s, amplificador 3), sempre. No port, essa regra e o tique do efeito
(`zirnox.irradiated`, `(3 + 1) × 0,05 RAD` por tick) ficavam dentro de `zirnox_debris`, depois do
`return` antecipado de um mundo sem destroços: sem nenhum destroço do ZIRNOX, o micélio não irradiava
ninguém, e um efeito já dado parava de doar. Agora `zirnox_irradiation` roda todo tick, depois dos
efeitos do ZIRNOX (a mesma ordem de antes quando há destroços), e também quando o ZIRNOX tem efeitos
pendentes. Teste: `mycelium_irradiates_without_debris_and_decon_ends_it`.

### Regressão do ZIRNOX (commit `68102b17` e motor corrigido)

Três rodadas completas do cenário `zirnox` com o binário "depois" (motor `130153a4ac`, NTM com `68102b17`), comparadas
checagem por checagem com quatro rodadas de referência de outras frentes. Cada rodada tem 136 checagens (183 nomes
na união das sete, porque alguns nomes levam o número do passo); as 136 passam nas três. Das 85
checagens que dão o mesmo valor nas quatro referências, a rodada 2 é igual em todas, a rodada 1 difere só no número de
barris que o tanque já tinha esvaziado no instante do retrato, e a rodada 3 (26 minutos sob carga, contra 7 a 8) difere em
11, todas consequência de retratar instantes mais tardios (mais barris vazios, 14 barras esgotadas em vez de 11). O harness
espera em tempo real, não em ticks. Destroços: criados `[20, 10, 10, 10, 20, 2]` e sobreviventes não destrutivos
`20, 10, 10, 10` exatos em todas; os tipos 4 e 5 são destrutivos e aleatórios (fora da checagem): 11, 13, 14 e 2, 1, 2,
contra 14, 14, 12, 13 e 0, 0, 0, 2 nas referências. Nenhum número aprovado mudou. Sob carga alta (25 a 48), antes das três
rodadas completas, cinco tentativas pararam num menu que não abriu em 0,7 s (espera fixa do harness; nas fases de
preparação, conversão e automação) e uma parou ao ler a vida da barra do slot 0 do reator ("missing life"), sem log
guardado: causa não determinada (um clique perdido deixaria a barra no cursor). Pendência nominal: se reaparecer,
investigar com o log, que o loop agora guarda. Evidência: `docs/evidence/onda1-f3/zirnox-regression.json`.

Com a espera por estado (decisão do coordenador, `e91d5eb3`: menu aberto em vez de 0,7 s fixos, e a transferência
do funil em ticks), a primeira tentativa fechou completa: 136 de 136. Das 84 checagens constantes nas referências, 6
diferem, todas do instante do retrato (barris esvaziados, barras esgotadas na automação, calor no fechamento), como na
rodada lenta; destroços criados `[20, 10, 10, 10, 20, 2]` e não destrutivos `20, 10, 10, 10` exatos. No harness final (`6dd6f59a`), outra rodada completa na primeira tentativa: 136 de 136; das 84
constantes, 2 diferem (barris esvaziados; as emissões de gás do reator destruído no instante do retrato).

### Famílias em jogo no binário final e regressão completa de `items`

No binário final (motor `130153a4ac`, bundle no `HEAD` da branch), as famílias desta frente passam:
`protection` inteira (27 checagens) e `masks` + `decon` (35 de 35). Numa primeira rodada, a mono não recebeu o
filtro porque os três cliques de instalação se perderam sob carga (filtro ainda na mão, sem erro no log); repetida, passou.
Evidência: `docs/evidence/onda1-f3/items-families-final.json`.

**Regressão completa de `items`: fechou (264 de 264, harness final `6dd6f59a`, binário com o motor `130153a4ac`).** Decisões do coordenador (10/11): as esperas por tempo
de parede que bloqueavam a regressão passam a esperar em ticks do servidor ou num estado, cada conversão com uma
mutação que prova que a checagem continua pegando a falha que pegava; nenhum veredito mudou. A causa comum: sob
carga, o controlador do teste leva vários segundos por comando (uma soltura medida 15 s depois), enquanto o servidor
segue a 20 ticks por segundo. As mutações são ligadas por `NTM_ITEMS_MUTANT` e `NTM_ZIRNOX_MUTANT`, só nas rodadas
de mutação.

| Checagem | Conversão | Mutação, que a checagem pega |
| --- | --- | --- |
| `real_eating_started`, `cancel_does_not_consume` | `cancel_use`: segura 9 ticks com o servidor a `SLOW_RATE` (1) tick por segundo; repete a tentativa cuja soltura chegou depois que o uso podia terminar, sem repor a pilha na última | `late_release:ntm:cotton_candy` (segura além dos 32 ticks) |
| `can_cancel_keeps_it` | `cancel_use` (32 ticks, `ItemEnergy:205`) | `late_release:ntm:can_smart` |
| `cancelled_bdcl_stops_its_gulps` | `cancel_use` (40 ticks, `ItemBDCL:12`; soltura antes do tick 19, no máximo quatro goles) | `late_release:ntm:bdcl` |
| `food_and_saturation_apply`, `only_one_food_consumed` | espera a fome baixar a comida; come segurando `EAT_TICKS` + margem a `SLOW_RATE` | `food_not_eaten` |
| `java_food_effects` | efeitos lidos do servidor (`/data`), ainda a `SLOW_RATE`: o cliente conta a 20 por segundo e perde o wither de 100 ticks | `no_wither` |
| `vegan_schnitzel_java_effects` | come por ticks; duração do Java menos os ticks passados | `schnitzel_short_blindness` |
| `full_food_consumes_one`, `bdcl_is_consumed_without_a_container` | `use_one`: a duração + margem a `SLOW_RATE`, soltura antes do fim da segunda | `second_one_too` |
| `thrown_container_waits_its_pickup_delay` | `tick freeze` desde antes do uso: o atraso de 40 ticks do item caído não corre | `pickup_delay_run_out` |
| `full_last_radaway_drops_container` | relê até o estado assentar (`settle_ticks`, 40 ticks) | `radaway_container_lost` (mata o item caído) |
| `mask_and_hazmat_reduce_gamma` | congela, `tick step 200`, congela: exatamente dez incrementos de dose (`RATE_EVERY` 20; o NTM só anda com o mundo rodando) | `mask_unprotective` (capacete de couro no lugar da M65) |
| todo `use(g, segundos)` | segura segundos × 20 ticks | `use_short` (pega em `can_leaves_ring_pull_and_empty_can`) |
| menus do reator, tanque e funil (ZIRNOX) | espera o menu abrir (prazo de 20 s); a transferência do funil em 240 ticks | `menu_never` |

Tolerâncias explícitas no roteiro: `SYNC_TICKS = 10`, `SLOW_RATE = 1`, `EAT_TICKS = 32`.

Depois da absorção da `main` `e4aa8b2c` (merge `ff8c6049`): a `main` trouxe a sua própria conversão do harness de
`items` para ticks (`fe170242`, `567bee58`, `df088afa`, revisada na frente dela), em que o próprio jogo segura o
botão (`use-item --hold`, `short_hold`) e a idade dos efeitos conta do gesto mais a duração do uso. Ela é a base no
merge e substitui as minhas conversões dos cancelamentos, do "comer um", da comida, dos efeitos do algodão-doce, do
contêiner do RadAway e do `use(segundos)` (os ajudantes `cancel_use`, `use_one`, `SLOW_RATE` e afins saíram). Ficam
da Onda 1, porque a revisão daqui pediu mais que a `main` exige: a gama exata (janela `tick step 200`, 281 e 201, no
lugar dos tetos 0,92 e 0,75), o atraso de coleta congelado, agora lido da idade (`Age`) da seringa lançada pelo fluxo
da `main` (passo até 30 e mais 15), e as diferenças exatas do schnitzel (400 e 3400) sobre o `has_effects` da `main`,
que tolera 80 ticks. No merge: `items` 264 de 264, unidade 527, gerador 299, `test_hazmat` 9; mutações desses três
trechos refeitas no binário do merge (motor `6a79054fa9`).

Ajustes da re-revisão (10/11): a gama exige 281 (M65) e 201 (placa hazmat) de 350, porque a janela é exata; o
expoente pela metade daria 313 (`d2225961`). O schnitzel limita pelos ticks desde o fim do consumo
(`elapsed − EAT_TICKS`) e exige as diferenças do Java entre os efeitos (400 e 3400), dados no mesmo tick; a mutação
encurta a cegueira a 160 ticks, o mais perto de 170 que o `/effect` aceita em segundos inteiros (`d4d04c22`). O
atraso de coleta é provado congelado, `tick step 30` (ainda no chão) e mais 15 (de volta): 40, e não 60 nem 80;
a mutação põe `PickupDelay` 60 no item caído (`4ac63533`).

Lacunas registradas: `settle_ticks` relê até 40 ticks, então uma queda de item que o servidor fizesse mais tarde que
isso também reprovaria a checagem do RadAway (é o mesmo veredito, com prazo); e os nomes das checagens repetidas do
ZIRNOX levavam o número de passos, que as releituras de uma máquina carregada aumentam: agora contam o próprio tipo
na fase (`reactor_menu_n`, `tank_menu_n`, `lever_<on>_n`, `cursor_move_ack_n`). Nas rodadas de
referência antigas esses nomes têm o número de passos e não casam com os novos. A primeira mutação do
algodão-doce não foi pega: o `cancel_use` repunha a pilha também depois da última tentativa e escondia o consumo;
corrigido (não repõe na última) e refeita.

Continuam por tempo de parede, porque não bloquearam a regressão: 100 esperas em `native_items.py` e 28 em
`native_zirnox.py`, entre elas as recusas (`refused`, o algodão-doce no criativo, a garrafa sem abridor, o cantil
esfriando), os 0,8 s depois de cada clique em `use`, as expirações longas (RadAway forte 19,5 s, o cantil 6 s, o
mucho mango, o leite com chocolate 2,4 s, o plan C), as quedas de armadura do kit e, no ZIRNOX, o enchimento do
tanque d'água e as alavancas. Numa rodada de mutação, `chocolate_milk_kills_a_survival_drinker` (2,4 s) falhou por
tempo; se passar a bloquear, converte-se do mesmo jeito.

### Contrato do catálogo do T00 (RBMK) e os stats de armadura

A fatia 1 mudou de propósito 20 entradas do catálogo que já existiam na foto do T00 (`81a210d4`): as peças de
`steel`, `titanium`, `alloy`, `cobalt` e `starmetal`, nos campos `components`, `equipment_gap` e `max_damage` (pontos de
armadura, durabilidade e lacuna de equipamento do Java). O teste
`test_rbmk_oracle.test_catalog_identity_stack_and_previous_content_are_unchanged` exigia o `ids.json` igual à foto.
Decisão do coordenador (10/11, opção A): toda entrada da foto continua presente e na mesma ordem relativa; as identidades
do próprio RBMK (as 105 que `rbmk_catalog.build` gera: colunas, consoles, painéis, destroços, barras e pastilhas) continuam
idênticas. A foto não foi atualizada e o gerador não mudou. Mutações: mudar o nome de `ntm:rbmk_rod` falha, tirar
`ntm:deco_rbmk` falha, mudar a durabilidade de `ntm:steel_plate` passa.

## Defeito do motor: anexos do dono num save no meio da sessão

Achado pela re-revisão da fatia 1 e diagnosticado por sonda (anexo `ntm:item_state`, sem a mesa):
num save no meio da sessão (`ctl save`, autosave, `/save-all`), `save_game_metadata` chama
`store_owner` (`crates/horizon-client/src/game/lan.rs:1634-1655`), que guarda o registro do hóspede
dono com os anexos do registro, e não os vivos: estes moram em `Game.guest_attachments[uuid]` e só
voltam ao registro em `save_guest` (`lan.rs:1427-1440`, `guest_attachments_into`), na saída. O
arquivo do dono (`players/data/<singleplayer_uuid>.dat`, o que `load_owner` lê) fica sem anexos; os
atuais vão só para o arquivo do UUID próprio do jogador (`native_player_save_to_file`).

| Momento | Arquivo do dono | Outro arquivo do jogador |
|---|---|---|
| save no meio da sessão, e depois `kill -9` | inventário atual, sem `horizon:attachments` | anexo `ntm` atual |
| saída limpa | anexo `ntm` atual | anexo `ntm` atual |

Efeito hoje, num crash depois de um save no meio da sessão: a dose volta certa só porque, sem o anexo,
o leitor semeia do espelho legado `WorldState.radiation` (KV); os ticks de tratamento do RadAway, o
tempo do enjoo herbal, o cooldown e a peça da mesa se perdem (o RadAway consumido fica gravado no
inventário: perda de item contra o vanilla). Afeta qualquer anexo de jogador de qualquer mod. A Onda 1
só entra na `main` com a correção do motor (branch `player-attachments-fix`, a partir de `a31497f38`).

### Correção do upstream e prova de corte

O upstream corrigiu o defeito em `61aa03d1a` (#12612). Branch `player-attachments-fix` sobre `a31497f38`:
só esse cherry-pick, commit `130153a4ac` (adaptação só no teste; detalhes e pontos de contato em
`docs/horizon-current.md`). O teste do upstream passa no pin com a correção.

Prova em jogo (`tools/test-game/run-player-cut-proof.py`): o mesmo estado, cortado nos pontos do save
atômico (`none`, `kill`, `save_begin`, `level_saved`, `players_saved`, `world_saved`, `kv_committed`),
reaberto duas vezes. Mesa: as duas peças clicadas para os slots 9 e 5 com a mesa aberta, contra o
controle do baú do Ender (as mesmas pilhas, que moram no arquivo do jogador). RadAway e herbal: o item
consumido tem de vir com o efeito inteiro. A RadAway dá 140 ticks de −1 RAD, então
`dose − 1000 × ticks restantes = 300000 − 140000`. O herbal tira 100 RAD de uma vez e liga o
temporizador, então `dose = 200000` com `herbal > 0`.

| Cenário | Antes da correção (`a31497f38`) | Depois (`130153a4ac`) |
|---|---|---|
| Mesa | `none`..`level_saved`: as peças voltam ao inventário, como o controle. `players_saved`, `world_saved`, `kv_committed`: **0 de 2**, contra 1 de 1 no baú do Ender | Os 7 cortes: as duas peças voltam uma vez cada, como o controle; as duas reaberturas concordam |
| RadAway | Mesmos três cortes: item consumido, efeito visível, **tratamento restante perdido** (dose 300000 ou 270000, contra 160000) | Os 7 cortes: consumida ⇒ dose 160000 (tratamento inteiro) |
| Herbal | Mesmos três cortes: **temporizador da doença perdido**; em `players_saved` a cura de 100 RAD também (dose 300000) | Os 7 cortes: consumida ⇒ dose 200000 e temporizador entre 11189 e 11765 ticks |

Evidência: `docs/evidence/onda1-f3/player-cut-{before,after}-{table,radaway,herbal}.json` (mesmo bundle nas duas;
só o motor muda). As rodadas de RadAway e
herbal usaram uma regra mais fraca (só "efeito presente"), que deixou passar a perda; a regra acima é a do
runner agora e está aplicada aos mesmos dados (`ok_strict`).

## Fatia 5: doença da radiação e `HbmPotion.radiation`

Decisões do coordenador (10/11): D1 (A) um `java.util.Random` por mundo para a doença, semeado pela semente do mundo e
guardado no KV, na forma da D3 do RBMK; D2 (A) um tipo de dano `ntm:radiation` se o motor aceitar um tipo de conteúdo
que ignore armadura, senão `minecraft:magic` (nunca `generic_kill`); D3 (A) o efeito no anexo do jogador, visível como
`ntm:radiation`. Entregue em três partes: 5a (doença e D1), 5b (morte e D2), 5c (anexo e D3).

### 5a: os sorteios da doença

`EntityEffectHandler.handleRadiationEffect:221-258`, a cada tick, para cada jogador fora do criativo entre 200 e
1000 RAD: os efeitos da faixa sorteados com `world.rand.nextInt(n) == 0`, na ordem do Java, todos os sorteios feitos
seja qual for o resultado dos anteriores.

| Faixa (RAD) | Efeito: chance, duração em ticks, nível |
| --- | --- |
| 800 a 999 | náusea 1/300, 150, I; lentidão 1/300, 200, III; fraqueza 1/300, 200, III; veneno 1/500, 60, III; wither 1/700, 60, II |
| 600 a 799 | náusea 1/300, 150, I; lentidão 1/300, 200, III; fraqueza 1/300, 200, III; veneno 1/500, 60, II |
| 400 a 599 | náusea 1/300, 150, I; lentidão 1/500, 100, I; fraqueza 1/300, 100, II |
| 200 a 399 | náusea 1/300, 100, I; fraqueza 1/500, 100, I |

Antes era uma aproximação determinística: uma vez por segundo, todos os efeitos da faixa por 5 s, mais 1 de dano
"magic" por segundo acima de 800 RAD, que não existe no Java (saiu). O sorteio usa o `JavaRandom` do mundo
(`WorldState.sickness`, no KV), semeado pela semente do mundo (`ServerHost::world_seed`) na primeira vez que alguém
fica doente; um mundo sem ninguém doente não o grava. O Java não tem carência aqui (a regra dos 200 ticks é do
`contaminate`, parte da 5c). Os efeitos passam ao host em ticks (`ServerHost::effect`), porque a náusea dura 150 ticks
(7,5 s). A morte a 1000 RAD fica como estava até a 5b.

Prova: oráculo Java (`tools/sickness-oracle`), o `handleRadiationEffect` recortado inteiro, compilado com stubs e
rodado com `java.util.Random` de quatro sementes sobre um cronograma que passa por todas as faixas e bordas: 519
efeitos; `radiation::tests::sickness_rolls_as_the_java_oracle` exige a mesma sequência (tick, efeito, duração, nível).
`test_sickness_oracle.py` confere que a fixture é a do Java. Mutações mortas: chance 700 → 701, duração 150 → 100,
nível trocado, ordem trocada, parar no primeiro acerto, borda 600 → 601. Em jogo (família `sickness`): em cada faixa,
12000 ticks com `tick sprint`; cada efeito com a duração e o nível do Java, e as contagens dentro de quatro desvios
padrão da binomial (por exemplo, 39 náuseas para 40 esperadas a 300 RAD).

### Lacunas nominais da fatia 5

- Dose em mobs e as transformações do `handleRadiationEffect` (creeper → creeper nuclear, vaca → mooshroom, aldeão →
  zumbi, pato → quackos): o port só guarda dose de jogadores. Sugerido para a Onda 4 ou a frente de mobs.
- As conquistas `achRadPoison` e `achRadDeath`: o port não tem conquistas.
