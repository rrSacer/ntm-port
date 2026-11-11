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

Ajustes da revisão da 5a: o gerador mora numa chave própria do KV (`SICKNESS_KEY`), com carimbo próprio, como as
camadas do RBMK; antes ele entrava no carimbo da chave do mundo, que um tick com alguém doente regravava inteira
(medido em release com a camada 15×15: 0,52 ms por tick com 5 mil gerações e 5,68 ms com 50 mil; agora 0,0001 ms nos
três casos, `keep_world_cost_with_sickness`). Testes novos: só a chave da doença é escrita num tick em que só ela mudou;
a semente é a do mundo (semente 0 quebra); o ×20 das bombas; criativo pulado e sem carência. Mutações mortas: as quatro. O formato com o gerador dentro da chave `world` (o build `afe065dd`) nunca saiu da branch: nenhum save o tem, e a
leitura não o procura lá. Não pôr o gerador de volta na chave do mundo nem nos seus carimbos.

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

### 5b: a morte por radiação e o tipo de dano (D2)

`EntityEffectHandler.handleRadiationEffect:223-226`: de 1000 RAD, `attackEntityFrom(ModDamageSource.radiation,
1000F)` e a dose volta a 0, seja qual for o efeito do ataque; nada é sorteado nesse tick. Agora isso é checado a cada
tick, como no Java (antes, uma vez por segundo e só depois da carência do port, que o Java não tem). A D2 ficou na opção
A: o motor aceita tipos de dano de conteúdo sem mudança nele (`damage_source::Catalog::with_packs` acrescenta ao
registro os `damage_type` do mod e resolve as tags depois), então `ModDamageSource.radiation` é o tipo `ntm:radiation`,
gerado por `tools/content-gen/damage_types.py`: `message_id` `radiation` (a mensagem `death.attack.radiation` do
idioma do Java, "%1$s died from radiation poisoning." / "%1$s morreu por envenenamento por radiação."), exaustão 0
(`setDamageBypassesArmor` zera a fome), escala `never` (`difficultyScaled` é falso), e a tag `minecraft:bypasses_armor`:
a armadura não segura, a Resistência segura. Saiu o `generic_kill`.

A ordem no tick também segue o Java: o `EntityEffectHandler` do jogador corre em
`NetHandlerPlayServer.processPlayer`, no tick de rede que `MinecraftServer.updateTimeLightAndEntities` abre depois dos
tile entities de todos os mundos. Então a doença e a morte (`sickness_tick`) vêm depois das máquinas, dos
descontaminadores, da ruína do ZIRNOX e da dose do inventário do tick (o `PlayerTickEvent` vem antes), e antes da dose
da poção de radiação (`updatePotionEffects`, mais adiante na atualização do jogador). Consequências testadas: com
1000,2 RAD em cima de um descontaminador, o jogador perde 0,5 RAD antes da checagem e vive com 999,7; na ruína do
ZIRNOX, o jogador que passa de 1000 RAD morre de radiação no mesmo tick, com a dose zerada; na Little Boy, a dose da
bomba mata no tick da explosão e a dose volta a 0. Há um limiar só (`radiation::death`): a 999,999 RAD sorteia a faixa
de 800 e vive; a 1000 morre sem sortear.

Prova: o oráculo da 5a também compara os ataques (tick, fonte e 1000), 20 nos quatro casos. Mutações mortas: tipo
`minecraft:magic`, dose mantida, limiar 999 RAD, dano 100, sorteio na faixa da morte. Em jogo (família
`radiation_death`): sem armadura e com armadura de diamante completa, o jogador morre com "ItemsTest died from
radiation poisoning." e volta com dose 0; com Resistência V sobrevive e a dose é 0; no criativo, nada.

Correções da revisão da 5b. A ordem no tick (acima) e um limiar só para a morte. A mensagem com matador: o jogo pede
`death.attack.radiation.player` quando um mob feriu o jogador nos últimos 100 ticks; o Java cai para o texto base
(`DamageSource.func_151519_b`, o idioma do Java não tem `.player`), o motor mostraria a chave crua, então o gerador emite
o texto base nas duas chaves. `test_damage_types.py` relê `ModDamageSource.java:15` e os idiomas do Java. Em jogo, duas
checagens novas na família `radiation_death`: com Resistência V e peitoral de diamante, o peitoral não perde
durabilidade (sem a tag `minecraft:bypasses_armor`, o pacote mutante perde 250, isto é 1000 / 4); e, depois de um golpe
de zumbi, a morte mostra "died from radiation poisoning." (o pacote mutante sem `.player` mostra a chave). Mutações
mortas a mais: limiar 999 RAD na morte (quatro testes), a doença antes do descontaminador (o teste de 1000,2 RAD).
Rodada final (`docs/evidence/onda1-f5/death-5b-review.json`): binário `b9ab6c92…`, pacote `87f5e937…`, motor no pin
sem mudança; `items` 276/276, ZIRNOX 136/136 em três rodadas, pilha 85/85, fusão da pilha 32/32, `save_model` 11/11.

Re-revisão da 5b. R-1 (P1, anterior à 5b): a dose da ruína do ZIRNOX não chegava ao jogador. A ruína doseia dentro do
passo da máquina, e o host adiado da transação (`Deferred`, `transaction.rs`) não repassava `player_dose`,
`commit_player_dose` nem `radiation_factor`: a dose ia só para o espelho do mundo, que a leitura seguinte sobrescreve
com o anexo do jogador, e a armadura não contava. Agora o passo guarda as doses que escreveu (uma leitura no mesmo passo
as vê) e as grava no anexo depois do commit da máquina, cada uma esperando a anterior; um passo que não faz commit não
grava nada. É a única dose dentro de um passo de máquina (as bombas doseiam fora, em `bombs_tick`). Testes: o
`Deferred` com um anexo (ordem, leitura em estágio, fator, passo sem commit) e os do ZIRNOX com um host de anexo (a
ruína grava no anexo, a morte no mesmo tick lê essa dose e deixa 0, o fator da armadura vale, a dose sobrevive a
reabrir). Mutantes mortos: sem repasse da escrita (4 testes), sem a leitura em estágio, sem o fator. Em jogo, no
roteiro do ZIRNOX (fases `ruin-dose` e `ruin-dose-check`): atrás da obsidiana, a dose do anexo sobe; depois de reabrir o
mundo, é a mesma; sem blindagem, a seis blocos, a ruína mata com "died from radiation poisoning." e o anexo fica em 0. O
cliente mutante (o `transaction.rs` anterior, binário idêntico ao da rodada anterior) deixa o anexo em 0 e falha.

R-2: a poção de radiação vem depois da checagem de morte (`updatePotionEffects` depois do `LivingUpdateEvent`); o teste
`the_radiation_potion_doses_after_the_death_check` (999,9 RAD com a poção: vive no tick, morre no seguinte) mata o
mutante que inverte a ordem. R-3: a exaustão padrão de um `DamageSource` do 1.7.10 é 0,3 (`ldc 0.3f` no construtor);
o gerador e `test_damage_types.py` usam 0,3, e só `setDamageBypassesArmor` zera.
Rodada (`docs/evidence/onda1-f5/ruin-dose-r1.json`): binário `de5a93cf…`, pacote `87f5e937…`, motor no pin sem
mudança; ZIRNOX 140/140 em três rodadas (as 136 de antes e as 4 da ruína), pilha 85/85, fusão da pilha 32/32; `cargo
test` 539 + 118 de integração, `save_model` 11/11, clippy limpo.

Re-revisão 2, R2-1: a ruína (`TileEntityZirnoxDestroyed.radiate:89`) e a poção de radiação (`HbmPotion:123`) doseiam
por `ContaminationUtil.contaminate`, que pula os primeiros 200 ticks do jogador (`:287`, `ticksExisted < 200`). Agora
as duas usam a carência que o port já tinha (`blasts.entered`, `GRACE_TICKS`): quem entra ou renasce perto da ruína não
recebe dose por 200 ticks. A poção continua contando o tempo e o Geiger continua ouvindo as duas. O renascimento
recomeça a carência porque o host só lista os vivos, e o morto sai de `entered`. Teste
`the_ruin_and_the_potion_spare_a_players_first_200_ticks` (entrada e renascimento); mutantes mortos: sem a carência na
ruína, sem a carência na poção. Em jogo (`ruin-dose-check`): depois de renascer ao lado da ruína em chamas, o jogador
está vivo com dose 0 aos 60 ticks e morre 204 ticks depois; o cliente anterior (sem a carência) morre antes dos 60.
Rodada: binário `88837ca0…`, pacote `87f5e937…`, ZIRNOX 142/142, `cargo test` 540 + 118.

### 5c: a poção de radiação no anexo do jogador (D3)

`HbmPotion.radiation` saiu do mapa do mundo (`zirnox.irradiated`) para o anexo do jogador (`ntm:item_state`, chaves
`irradiated` e `irradiated_level`) e aparece como o efeito `ntm:radiation` ("Contaminado", nocivo, cor 0x84C128, o
ícone (1, 0) do `registerPotion`). Decisão do coordenador: opção (A), o anexo guarda o estado e o efeito é a bandeira.

- Núcleo: `irradiate_player` aplica com a regra do `PotionEffect.combine` (nível maior assume com a sua duração, nível
  igual fica com a maior duração, nível menor não muda nada); `zirnox_irradiation` doseia `(n + 1) × 0,05 RAD` por
  tick (com a carência, o fator da armadura e a ordem da 5b), desconta o tempo e repõe o efeito com o tempo restante.
  O host lê e grava o anexo por CAS (`player_irradiation`/`commit_player_irradiation`), como a dose; o `Deferred`
  repassa em estágio (hoje nenhuma aplicação corre dentro de um passo de máquina: os detritos aplicam em
  `zirnox_world_effects`, fora dele).
- Cura: o efeito sumido com tempo restante encerra a poção, como `removePotionEffect`: leite (curativos padrão do
  Forge), antídoto (`clearActivePotions`), pílula de iodo (`ItemPill:45`), bolsa médica (`ItemSyringe:127`),
  `/effect clear`. O iodo e a bolsa ganharam `ntm:radiation` na lista de cura (`consumables.json`). O descontaminador
  (`TileEntityDecon:27`) zera o anexo e tira o efeito. Não conta como cura a falta do efeito no tick em que a poção foi
  dada (o motor aplica o efeito no fim do tick) nem no primeiro tick do jogador no mundo; e o leite bebido antes do passo
  no micélio, no mesmo tick, faz a poção nova começar do zero, sem somar com a velha.
- Migração única: uma entrada de `zirnox.irradiated` de um mundo anterior à 5c entra no anexo (pela mesma regra de
  soma) quando o jogador está no mundo, e sai do mapa só depois da gravação no anexo. Jogador fora do mundo: a entrada
  espera.

Provas. Testes de unidade (anexo, efeito enfileirado como no motor, curas, confiança no tick dado e na entrada, leite e
micélio no mesmo tick, descontaminador, migração com recusa e ida e volta por `WorldState::load`, criativo, `Deferred`,
`combine`); 13 mutantes mortos (sem cura, sem confiança na entrada, sem confiança no tick dado, aplicação ignorando a
cura, aplicação só pelo mapa, recusa sem desconto, migração mantendo a entrada, sem repor o efeito, descontaminador sem
efeito, `Deferred` sem estágio, duas regras de soma). Conteúdo: `test_mob_effects.py` lê o `registerPotion` e o
idioma; `test_consumables.py` passou a ler também as remoções de `HbmPotion`. Em jogo (família `irradiation`): o
micélio dá o efeito nível 3 com o tempo do anexo, a dose sobe 0,2 RAD por tick, e leite, iodo, bolsa, antídoto e
`/effect clear` encerram a poção (sem mais dose); o binário anterior à 5c falha (sem efeito, anexo vazio).

Corte de save (`run-player-cut-proof.py`, sete pontos do save, duas reaberturas):

| Cenário | Antes da 5c (binário `88837ca0…`) | Com a 5c (`59f23f19…`) |
|---|---|---|
| `irradiation`, com o controle do baú do Ender | `players_saved`: **a dose ficou (310,8 RAD), a poção sumiu** (estava no KV do mundo, não gravado); os outros seis coerentes | Os sete cortes: a poção e a dose andam juntas, como o controle |
| `migration` (mundo preparado pelo binário anterior) | — | Os sete cortes: a poção está no anexo, nível 3, com o efeito (a rodada da entrega; ver a correção P2-1 abaixo) |

Rodada: binário `59f23f19…`, pacote `36eb93fa…`, motor no pin sem mudança; `items` 283/283 (as 276 de antes e 7 da
`irradiation`), ZIRNOX 142/142. Evidência em `docs/evidence/onda1-f5/irradiation-5c.json`.

Correções da revisão da 5c (APROVADO COM RESSALVAS):

- P2-1, dose dupla no corte da migração. O save grava o arquivo do jogador antes do KV do mundo: num corte entre os
  dois, o disco ficava com o anexo da sessão (a poção já descontada e a dose que ela deu) e com a entrada antiga do KV
  (a poção cheia), e a reabertura as somava de novo: os ticks já doseados doseavam outra vez (na sonda do revisor, 156
  ticks viraram 452, cerca de 72 RAD a mais). Agora o anexo guarda a entrada que absorveu (`irradiated_migrated`,
  `irradiated_migrated_level`, chaves `Long` do mesmo formato), na mesma escrita por CAS da poção
  (`zirnox::absorb`): a mesma entrada achada de novo é descartada, não somada. O descontaminador que encerra uma
  entrada ainda não absorvida também a marca. Testes: `a_save_cut_during_the_migration_doses_nothing_twice` (o corte em
  três momentos, sempre 600 doses), `a_decontaminated_worlds_potion_does_not_come_back_after_a_cut`,
  `a_worlds_potion_is_taken_in_once`. Em jogo: o corte `migration`, com o ato de 16 s fora da carência e o controle do
  baú do Ender, exige que as doses dadas mais o tempo restante não passem da poção.
- P3-2, a poção migrada perdia o último tick. O motor desconta os efeitos antes do passo do NTM: o efeito agora é
  reposto com um tick de folga (`tempo + 1`), em qualquer caminho. O host de teste passou a imitar o motor (desconto
  antes do passo, fila aplicada pelo `update`). Teste: `a_migrated_potion_doses_all_its_ticks` (exatos 120 000).
- P3-3, `/effect give @s ntm:radiation` não doseava. Um efeito mais forte ou mais longo que o do anexo (lido por
  `effect_state`) entra como uma aplicação, pela regra de soma. Teste: `a_given_radiation_effect_doses_as_the_potion`.
- P3-4, a gravação recusada. A poção é gravada antes da dose; recusada, o tick não doseia nem desconta nada, e o
  próximo tenta de novo. Teste: `refused_writes_give_no_dose_and_lose_no_tick`.
- P3-5, divergências anteriores à fatia, registradas para depois: o micélio aplica a poção a cada tick e pula o
  criativo, mas no Java `onEntityWalking` só vem num passo (`Entity.moveEntity`, depois de `nextStepDistance`) e
  `WasteEarth:83-91` e `EntityZirnoxDebris:118-124` não excluem o criativo; e o descontaminador roda antes dos detritos,
  mas no Java as entidades vêm antes das tile entities, e ele tira a poção que o detrito acabou de dar. Para a 5d: os
  itens de cura por instância do Forge (`BlockFallout:58` esvazia a lista: o leite não cura essa poção) pedem um campo no
  anexo, como o `herbal`; e o `update` do motor guarda efeitos ocultos (amplificador maior e mais curto), que o 1.7.10
  não tem.

Rodada das correções (`docs/evidence/onda1-f5/irradiation-5c-review.json`): binário `d0c35f1e…`, pacote `36eb93fa…`,
motor no pin sem mudança. Família `irradiation` 8/8 (o `/effect give` nível 9 doseia 0,5 RAD por tick e entra no
anexo). Corte `migration` com controle: os sete coerentes; em `players_saved`, com a entrada antiga `[539, 3]` ainda no
KV, a poção fica em 125 ticks (antes da correção, o cliente da entrega `59f23f19…` volta a 468 ticks com 234 doses já
dadas e falha). Corte `irradiation` com controle: sete de sete. `items` 284/284, ZIRNOX 142/142, `cargo test` 555 +
118, `save_model` 11/11, clippy limpo; 15 mutantes do núcleo mortos (os 13 da entrega, mais a marca ignorada e o
descontaminador sem marca).

Correções da re-revisão da 5c:

- RR-1: um convidado que o motor ainda não carregou fica com os efeitos parados (o motor só passa os efeitos de quem
  está carregado), um tick acima do anexo, e o NTM já o vê: a correção do P3-3 tomava isso por aplicação a cada tick, e
  a poção não descontava. Agora a leitura do efeito é guardada a cada tick (e só é usada depois do primeiro), e só conta
  como aplicação um efeito que cresceu desde a leitura anterior. Teste: `an_effect_held_still_is_no_application` (30
  ticks parados dão exatos 30 × 200 millirad). Mutantes mortos: sem a condição de crescimento, efeito dado ignorado,
  fim sem normalizar.
- RR-2 (nominal): a marca da migração mora no anexo, e o anexo não passa da morte (`copy_on_death: false`). Se o
  jogador morre depois da migração e o save cai entre a gravação do jogador e o primeiro commit do KV, a entrada antiga
  ainda está no KV e o anexo novo a absorve de novo: o jogador renascido volta com a poção, que no Java ele não tem.
  Exige migração, morte e queda no save, tudo antes do primeiro commit do KV; fica registrado.
- RR-3: a regra do corte `migration` usa o orçamento medido no fim do prepare, `G0` = doses dadas + ticks restantes,
  lido de uma só gravação do binário anterior (o KV tem a poção e o espelho da dose do mesmo instante). Em todo ponto,
  `dadas + restantes ≤ G0`; a carência das reaberturas só faz a soma cair, e uma poção que acabou no caminho (0 ticks,
  sem efeito) também cabe.

Rodada da re-revisão (`docs/evidence/onda1-f5/irradiation-5c-rereview.json`): binário `589d2efe…`, pacote `36eb93fa…`,
motor no pin sem mudança. Família `irradiation` 8/8. Corte `migration` com controle, `G0` = 626 (`[559, 3]` e 313 400
millirad numa gravação): sete de sete na primeira rodada; o pior ponto, `players_saved`, soma 312 + 43 = 355. `cargo
test` 556 + 118, clippy limpo.

Re-revisão 3: RR3-1 (regressão do RR-1): num tick não assentado (a poção sendo dada, no micélio ou pelos detritos), a
leitura do motor virava a referência, e um `/effect give` dado ali só descia depois e se perdia. Agora, num tick não
assentado em que o NTM põe a poção, a referência é o efeito que ele pôs; e crescer exige mais que o tick de folga da
repostagem. Testes: `an_effect_given_on_the_mycelium_is_taken_in`, `an_effect_given_in_the_tick_the_potion_is_given_is_taken_in`,
`a_guest_held_still_then_loaded_doses_the_same_total` (os três do revisor); mutantes mortos: sem a folga no
crescimento, referência pela leitura num tick não assentado, sem a condição de crescimento. Em jogo, na família
`irradiation`: dado no micélio, o anexo fica com o nível 9 (efeito `[9, 346]`, anexo `[343, 9]`). RR3-2: no corte
`migration`, uma poção que acabou só é aceita na segunda reabertura, depois de a primeira tê-la mostrado. Rodada
(`docs/evidence/onda1-f5/irradiation-5c-rereview3.json`): binário `5358d52f…`, pacote `36eb93fa…`; família 9/9; corte
`migration` com controle 7/7, `G0` = 626; `cargo test` 559 + 118, clippy limpo.

Ordem combinada: depois da 5c, a 5d (fontes da poção com bloco no port: `BlockFallout:57`, `BlockSellafield:49`, e a
regra de dose do fallout), com plano e revisão próprios.

### 5d: as fontes da poção com bloco no port (fallout e sellafield)

Decisões do coordenador: D1 (A) os níveis 0..5 do sellafield; D2 (A) a poção do fallout que o leite não cura
(`irradiated_milkproof`); D3 (A) a poção dada num passo, como o `Entity.moveEntity` do 1.7.10.

- Conteúdo (bloco 1): o conteúdo não tem estados de bloco próprios, então cada nível do `BlockSellafield` é um bloco,
  como todo metadado do port: `ntm:sellafield` (nível 0, o id que os mundos já têm) e `ntm:sellafield__1`..`__5`. Nomes
  `tile.sellafield.<n>.name`; textura pela tinta do Java em tempo de execução (`RGBMutatorInterpolatedComponentRemap
  (0x858384, 0x434343, clara, escura)` sobre `sellafield_slaked`), conferida pixel a pixel; perigos de inventário 0,5 /
  1 / 2,5 / 4 / 5 / 10 RAD/s (`HazardRegistry:222-227`); aba "Minérios e Blocos" (o `blockTab` do construtor); drop de
  si mesmo; as receitas de triturador dos níveis 1-5 (`ShredderRecipes:295-299`) passaram a existir; tick aleatório
  (`needsRandomTick`).
- O passo (D3, `crate::walking`), conferido no bytecode de `sa.d(DDD)V` (offsets 1498-1886):
  1. só sem agachar no chão (`onGround && isSneaking()`, a variável 20, calculada no começo do método) e sem montar
     (`ridingEntity == null`);
  2. o bloco é `(floor x, floor(posY − 0,2 − yOffset), floor z)` (o `EntityPlayerMP` zera o `yOffset`: o pé menos
     0,2), ou o de baixo quando este tem tipo de render 11, 32 ou 21 (cerca, muro, portão);
  3. `dy = 0` fora de escada (só `Blocks.ladder`, `ajn.ap`; trepadeira e andaime não);
  4. `Q += (float) sqrt(dx² + dy² + dz²) × 0,6`, somado em double e guardado em float;
  5. passo quando `Q > (float) nextStepDistance` e o material do bloco não é ar; então `nextStepDistance = (int) Q + 1`
     e `onEntityWalking`.
  Os contadores começam em 0 com a entidade (entrada, renascimento) e não são salvos. `tools/step-oracle` compila a
  conta transcrita e o port dá os mesmos passos e o mesmo float, bit a bit, em 2000 movimentos de 4 caminhadas.
- Teleporte: `setPositionAndUpdate` (comando, pérola, portal) não passa por `moveEntity`: nenhum teleporte dá passo,
  de qualquer distância (revisão P2-1). Andar é preciso, inclusive nas provas em jogo.
- Agachar, montar e teleportar vêm de campos do motor que a API nativa (`NativePlayerInfo`) não traz; a cola os lê num
  único ponto (`glue.rs`, `step_blocked` e `teleported`), com tipos explícitos, para que um pin que os mude quebre a
  compilação: o convidado de LAN (`lan::guest::Guest::{on_ground, sneaking, vehicle, teleport_counter,
  awaiting_teleport, landing}`) e o jogador local (`Player::ground.on_ground`, a tecla `Action::Sneak`, `Game::riding`,
  e `Player::steps`, o `moveDist`/`flyDist` que só a física faz crescer). **Pedido ao motor para depois (B):** expor
  em `NativePlayerInfo` os três campos `on_ground`, `sneaking` e `vehicle` (e um sinal de teleporte).
- As poções ao pisar: micélio 600 ticks nível 3 (criativo incluso, `WasteEarth:83-91`); fallout 12000 ticks nível 0,
  sem itens de cura, fora do criativo (`BlockFallout:51-59`); sellafield 600 ticks nível `meta < 5 ? meta : meta × 2`
  (criativo incluso, `BlockSellafield:28-33`). Parado em cima não há passo, e a poção desconta, como no Java.
- O fallout tem 0,125 de altura no Java e não empilha; o do port é uma camada de neve (1 a 8). No passo, sobre uma
  célula de fallout, o pé é tomado na altura do Java (revisão P3-4): andar no plano, com qualquer número de camadas, pisa
  no bloco de baixo; só um passo no ar mais de 0,075 acima da camada (um pulo) pisa no fallout. No 1.7.10 é igual:
  andando no plano, `floor(pé − 0,2)` é o bloco de baixo.
- `milkproof` (D2): ligado quando a poção nasce do fallout; o `combine` fica com a lista da primeira poção (a do micélio
  continua curável sob o fallout; a do fallout continua imune quando o nível 3 do micélio assume). O leite do motor tira
  todos os efeitos: o efeito sumido de uma poção `milkproof` é reposto, como no herbal. O que no Java a encerra por id ou
  por `clearActivePotions` (iodo, bolsa médica, antídoto, descontaminador) zera o anexo direto. O `milkproof` é limpo
  quando a poção acaba (revisão P3-3).
- Decadência: no tick aleatório, com 1/25 no nível 0 (vira `sellafield_slaked`) e 1/15 nos outros (desce um nível),
  `BlockSellafield.updateTick:36-46`. O sorteio é por hash da célula e do `Level.randValue` (os mods não têm aleatório
  próprio): a chance é a do Java, a sequência não.

Provas. Unidade: `crate::walking` (passo, cerca, escada, trepadeira e andaime, ar, teleporte, fallout com camadas, o
oráculo, as poções dos blocos, a chance da decadência em 4 desvios-padrão) e o servidor (o micélio dá a poção nos passos
e não parado; agachar e montar; o fallout sobrevive ao leite e o descontaminador o encerra; a lista da primeira poção;
leite e passo no mesmo tick; o `milkproof` que acaba não sobra; o teleporte curto sobre o micélio). Mutantes mortos no
núcleo e no passo, inclusive os dois do revisor (trepadeira como escada; `milkproof` velho) — o segundo ficou
equivalente depois da correção, porque a bandeira não sobra mais. Em jogo, famílias `sellafield_levels` e `fallout`
(14/14): no plano sobre o fallout nada; pulando, a poção `milkproof` (nível 0, ~12000 ticks); o leite não cura e a dose
segue; o iodo cura; o criativo é poupado; sellafield 3 e 5 dão os níveis 3 e 10; parado em cima a poção desconta;
agachado (devagar) e montado (um porco movido pela faixa) não há passo, andando há; teleportando sobre o micélio nada,
andando a poção; o tick aleatório desce os níveis até `slaked`. O cliente anterior ao sinal de teleporte falha na
checagem do teleporte (a poção vem).

Rodada (`docs/evidence/onda1-f5/fallout-sellafield-5d.json`): binário `b456388f…`, pacote `dca24969…`, motor no pin sem
mudança. `items` 301/301 (as 284 de antes e 17 da 5d), ZIRNOX 142/142, cortes de save `irradiation` 7/7 (com o controle
do baú do Ender; a poção dada andando) e `migration` 7/7 (`G0` = 614), `cargo test` 578 + 118, conteúdo 312,
`save_model` 11/11, clippy limpo.

Lacunas da 5d:

- `/effect clear` não encerra a poção do fallout (revisão P3-2): no Java, `clearActivePotions` tira tudo; no port, o
  comando tira o efeito do motor e a poção `milkproof` o repõe (como o herbal). A cola não vê o comando do jogador local
  (o gancho de comandos do motor é só de convidados, e o `clear` usa seletores).
- O fallout do port é uma camada de neve: ao perder o apoio ela se solta e **dropa o item** (60 RAD/s no inventário);
  o `BlockFallout` do Java vira ar sem drop (`func_150155_m`, `setBlockToAir`). Também empilha camadas (o Java não). O
  passo já usa a altura do Java; o drop e o empilhamento são do conteúdo, para uma frente de blocos.
- O `incrementRad` do `updateTick` do sellafield espera a radiação por chunk (fatia 4); o clique no fallout
  (`addCont`) espera a lista de contaminação do jogador; as quatro variantes de textura por posição do sellafield ficam
  na primeira; a masmorra `Barrel` (onde o Java põe sellafield) não existe no port.

### Lacunas nominais da fatia 5

- As outras fontes de `HbmPotion.radiation` (a 5c tem o micélio e os detritos do ZIRNOX; a 5d, o fallout e o
  sellafield). Sem bloco ou entidade no port ainda: `BlockOre:156-165`, `BarbedWire:75`, `Balefire:159`,
  `BlockGasRadonDense:42`, `BlockGasMeltdown:40`, `EntityRBMKDebris:105`, `EntityTaintCrab:53`, `ModEventHandler:1015`.
- Dose em mobs e as transformações do `handleRadiationEffect` (creeper → creeper nuclear, vaca → mooshroom, aldeão →
  zumbi, pato → quackos): o port só guarda dose de jogadores. Sugerido para a Onda 4 ou a frente de mobs.
- As conquistas `achRadPoison` e `achRadDeath`: o port não tem conquistas.
- A imunidade `ContaminationUtil.isRadImmune` (`EntityEffectHandler:219`): para um jogador ela só vale com o efeito
  `HbmPotion.mutation` ativo, que o port não tem; os mobs imunes da lista dependem da dose em mobs (acima).
- Os outros usos de `ModDamageSource.radiation` no Java. A 5b só faz a morte a 1000 RAD (`EntityEffectHandler:225`);
  o tipo `ntm:radiation` já existe para eles, mas as fontes ainda não ferem: o sellafield que fere metade da vida
  (`TileEntitySellafield:29`); o cório, 2 de dano (`CoriumBlock:81`, `CoriumFinite:54`); os fluidos tóxico e
  schrabídico (`ModBlocks:2329`, `:2333`); o ataque da besta radioativa, 16 (`EntityRADBeast:145`); a nuvem química, 1
  sem pausa de invulnerabilidade (`EntityChemical:153`); o item glitch, 1000 (`ItemGlitch:49`); os 100 de dano na
  transformação do creeper (`EntityEffectHandler:196`, com a dose em mobs); e a imunidade do creeper nuclear a esse
  dano (`EntityCreeperNuclear:53`).
