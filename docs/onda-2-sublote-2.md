# Onda 2, sublote 2: máquinas K3 industriais que ainda estão Inert

Branch `onda2-k3-s2` (a partir de `f7078c87`, a `main` com o sublote 1 integrado). Mesma regra do sublote 1
(`docs/onda-2-sublote-1.md`): cada ID só deixa o `Kind::Inert` quando operação, receita, portas, GUI, falha e save
estiverem demonstrados contra o Java 1.7.10 (classe e linha citadas), com testes de unidade, mutantes, `clippy -D warnings`,
prova no jogo e corte de gravação contra o controle de baú. Uma máquina por vez, um commit por máquina, "PRONTO PARA REVISÃO"
por grupo. Este arquivo é o inventário e a ordem de trabalho; ele **só planeja**. Nada abaixo foi implementado ainda.

## De onde vem a lista

`content/machines_backlog.json` (entradas "K3 content declared") cruzado com os `Kind::Inert` de `registry.rs` na `main`
`f7078c87`: 62 IDs K3 Inert. Tirando o que não é máquina industrial, sobram **39 máquinas**, em quatro grupos. As colunas de
arquivo e linhas são da classe `TileEntity` do Java; as marcas são o que o código usa (receitas, calor, entidades, explosão,
poluição, mundo), lidas por varredura e **a confirmar máquina a máquina** na leitura do contrato, como no sublote 1.

### Fora do sublote (a confirmar com o coordenador)

| O quê | Por quê |
| --- | --- |
| 5 blocos de teste e decoração: `event_tester`, `obj_tester`, `deco_emitter`, `part_emitter`, `filing_cabinet` | Não são máquinas industriais; sem operação a provar. |
| 16 portas e escotilhas K3 (mais `fan` e `decon`): `vault_door`, `blast_door`, `sliding_blast_door`, `fire_door`, `transition_seal`, `silo_hatch`, `silo_hatch_large`, `secure_access_door`, `large_vehicle_door`, `qe_containment`, `qe_sliding_door`, `round_airlock_door`, `sliding_seal_door`, `water_door`, `cargo_door`, `seal_hatch` (e `fan`, `decon`; `machine_transformer` e os pilones também não são deste sublote) | São a família das portas (`BlockDoorGeneric`, `BlastDoor`, `BlockHatch`) e dos ventiladores; têm frente própria (`front-doors`) e não têm receita nem GUI de máquina. `machine_transformer` e os pilones nem aparecem no backlog. |

## Grupo A: as cinco que o sublote 1 deixou, travadas por infraestrutura

| ID | Classe (linhas) | O que trava | Depende de |
| --- | --- | --- | --- |
| `machine_sawmill` | `TileEntitySawmill` (310) | lâmina como entidade, dano a entidades, explosão de sobrevelocidade | D1+D2 (efeitos de mundo, explosão genérica) e D3 (item sem lâmina ao quebrar); lógica pronta e parada em `onda2-sawmill-wip` (`38c2bf49`) |
| `machine_rotary_furnace` | `TileEntityMachineRotaryFurnace` (451) | vazamento da fundição para o mundo, poluição | F1 / D4 (fundição), poluição (Onda 1/3) |
| `machine_strand_caster` | `TileEntityMachineStrandCaster` (356) | idem: lingotamento contínuo | F1 / D4 |
| `machine_precass` | `TileEntityMachinePrecAss` (501) | o único par do perfil padrão (`crystalcircuit`) produz `ntm:orbital_assembly`, que não está no port | `orbital_assembly` (frente do espaço, Onda 5) ou decisão do usuário de ligar o perfil 528 |
| `machine_chemical_factory` | `TileEntityMachineChemicalFactory` (560) | quatro seletores de receita independentes: o painel nativo serve uma receita por bloco | extensão do painel de receitas e do hook de botão (desenho de infraestrutura do Opus) |

O item `ntm:broken_item` (e o seu NBT) já está pronto; a regra para a montadora (sempre `broken::make`) está no doc do sublote 1.

## Grupo B: máquinas de receita, sem efeito de mundo (as primeiras a fazer)

Têm slots, tanques, energia e uma tabela de receitas do Java para extrair (o mesmo caminho das soldadoras: extrator em
`tools/content-gen`, JSON em `crates/ntm-core/data/machines`, módulo em `crates/ntm-core`).

| ID | Classe (linhas) | Receitas / o que faz | Observações |
| --- | --- | --- | --- |
| `machine_fel` | `TileEntityFEL` (335) | laser (20 M HE); feixe 24 blocos em linha, alimenta o SILEX, acerta jogadores e blocos | **antecipada do 2e para o 2a** (decisão do Maestro, Opção A) para provar o SILEX com laser real em jogo |
| `machine_silex` | `TileEntitySILEX` (385) | `SILEXRecipes`, laser, peróxido (tanque 16 000) | multibloco; fluido; depende dos itens de cristal/laser do catálogo |
| `machine_cyclotron` | `TileEntityMachineCyclotron` (421) | `CyclotronRecipes`; água 32 000, vapor gasto 32 000, antimatéria 8 000; 100 M de energia | multibloco; fluido; consome/produz antimatéria |
| `machine_radiolysis` | `TileEntityMachineRadiolysis` (294) | `RadiolysisRecipes`; 3 tanques de 2 000; calor | multibloco |
| `machine_exposure_chamber` | `TileEntityMachineExposureChamber` (356) | `ExposureChamberRecipes`; 1 M de energia | multibloco; fonte de partículas |
| `machine_ammo_press` | `TileEntityMachineAmmoPress` (234) | `AmmoPressRecipes` | **bloqueada por catálogo / Onda 4**: `ntm:casing__*` existe, mas os itens de munição (`ntm:ammo_standard` / Sedna) ainda não existem em `content/ids.json` |
| `machine_microwave` | `TileEntityMicrowave` (293) | cozinha como o forno vanilla (`FurnaceRecipes`); 50 000 de energia; **explode** com o superaquecimento | tem efeito de mundo (explosão): espera D1+D2 para a falha |
| `machine_rockmill` | `TileEntityMachineRockMill` (282) | 2 tanques de 4 000, energia; mexe no mundo | confirmar o que faz no mundo |
| `machine_autocrafter` | `TileEntityMachineAutocrafter` (372) | grade de fabricação com modelo e filtro | lógica própria, sem tabela de receita do NTM |
| `machine_keyforge` | `TileEntityMachineKeyForge` (211) | 3 slots, grava chaves/trancas | liga com as portas e trancas já entregues (`K3 doors locks`) |
| `machine_ashpit` | `TileEntityAshpit` (224) | cinzeiro: recebe a cinza dos fornos e queimadores | depende da poluição/fuligem (Onda 1) para a parte do ar; a parte de itens é separável |
| `machine_supercomputer` | `TileEntityMachineSuperComputer` (199) | `SuperComputerRecipes` (simula/processa/copia drives, gera `blueprint_folder`) | **bloqueada por catálogo / Onda 4**: faltam os drives (`ItemDrive`) e `ntm:blueprint_folder` em `content/ids.json` |
| `machine_annihilator` | `TileEntityMachineAnnihilator` (280) | tanque de 2,5 M; apaga fluidos/itens | simples |
| `machine_funnel` | `TileEntityMachineFunnel` (179) | recolhe itens | simples |
| `machine_orbus` | `TileEntityMachineOrbus` (75) | armazenamento de fluido (tanque grande) | simples |
| `machine_industrial_generator` | `TileEntityMachineIGenerator` (28, casca) | gerador industrial: a lógica fica em outra classe | ler a hierarquia antes de estimar |
| `machine_lpw2` | `TileEntityMachineLPW2` (23) | peça de multibloco | confirmar se há comportamento |

## Grupo C: fluido e infraestrutura de torre/ar (curtas, dependem de redes que já existem)

| ID | Classe (linhas) | O que faz |
| --- | --- | --- |
| `machine_intake` | `TileEntityMachineIntake` (160) | puxa ar (tanque `AIR` 1 000) |
| `machine_compressor`, `machine_compressor_compact` | `TileEntityMachineCompressor` (89), `...Compact` (31) | compressão de fluido a pressão |
| `machine_tower_small`, `machine_tower_large` | `TileEntityTowerSmall/Large` (99 cada) | torre de resfriamento: vapor gasto → água |
| `machine_deuterium_extractor` | `TileEntityDeuteriumExtractor` (130) | água → água pesada |
| `machine_drain` | `TileEntityMachineDrain` (180) | esgota fluido no mundo; explode; escreve no mundo |
| `chimney_brick`, `chimney_industrial` | `TileEntityChimney*` (61, 65) | chaminés: só dão sentido com poluição/partículas |

## Grupo D: dependem de entidades, mundo ou chunks (esperam a API de entidades e os efeitos de mundo)

| ID | Classe (linhas) | O que trava |
| --- | --- | --- |
| `machine_teleporter` | `TileEntityMachineTeleporter` (243) | move entidades, carrega chunks |
| `machine_forcefield` | `TileEntityForceField` (540) | entidades e campo |
| `machine_siren` | `TileEntityMachineSiren` (245) | som e alcance |
| `tesla` | `TileEntityTesla` (187) | dano a entidades e raio no mundo |
| `machine_autosaw` | `TileEntityMachineAutosaw` (605) | entidades e blocos |
| `machine_thresher` | `TileEntityMachineThresher` (460) | entidades, mundo, calor |
| `machine_mining_laser` | `TileEntityMachineMiningLaser` (670) | laser que minera; usa as receitas de centrífuga, cristalizador, forno e triturador |
| `machine_assembly_factory` | `TileEntityMachineAssemblyFactory` (766) | quatro montadoras em um multibloco: o mesmo problema do painel da fábrica química |

## Ordem proposta

1. **2a (livre de infraestrutura):** `machine_annihilator`, `machine_funnel`, `machine_orbus`, `machine_keyforge`, depois as de receita simples e acopladas ao laser: `machine_radiolysis`, `machine_exposure_chamber`, `machine_fel` (antecipada do 2e, Opção A) → `machine_silex`, `machine_cyclotron`. São as que se provam como as soldadoras.
2. **2b (fluido e ar):** `machine_intake`, `machine_compressor` (e a compacta), `machine_tower_small/large`, `machine_deuterium_extractor`, `machine_industrial_generator`, `machine_lpw2`, `machine_rockmill`, `machine_autocrafter`.
3. **2c (depende dos efeitos de mundo, quando D1+D2 e D3 existirem):** `machine_sawmill`, `machine_microwave` (a falha), `machine_drain`, `machine_ashpit` e as chaminés (com a poluição).
4. **2d (depende de F1/D4 e do painel de múltiplas receitas):** `machine_rotary_furnace`, `machine_strand_caster`, `machine_chemical_factory`, `machine_assembly_factory`; a `machine_precass` espera `orbital_assembly`.
5. **2e (entidades):** `machine_teleporter`, `machine_forcefield`, `machine_siren`, `tesla`, `machine_autosaw`, `machine_thresher`, `machine_mining_laser`.

`machine_ammo_press` e `machine_supercomputer` ficam formalmente bloqueadas por catálogo aguardando a Onda 4 (`ntm:ammo_standard` / Sedna, `ItemDrive` e `ntm:blueprint_folder` ausentes de `content/ids.json`); decisão 4 homologada pelo Maestro (Opção A), junto com a antecipação de `machine_fel` para o 2a antes de `machine_silex`.

## Como cada máquina é entregue (igual ao sublote 1)

1. Ler a classe Java e anotar o contrato (slots e regras de mão e funil, tanques, energia, tempo, falha, save) com classe e linha.
2. Extrair a tabela de receitas do Java com um extrator em `tools/content-gen` (item exato antes do nome do OreDict; o que o catálogo não tem fica `skipped`, nunca inventado) e gerar o JSON em `crates/ntm-core/data/machines`.
3. Módulo em `crates/ntm-core`, ligado a `Kind`, `registry`, ponte de slots, política de slot e, se for multibloco, `inventory_ports` só nas células que o Java expõe.
4. Testes de unidade na ordem real da ponte (máquina nova, `load_state`, slots), mutantes escolhidos à mão, clippy.
5. Cliente release e prova no jogo (sem injetar estado; o tempo passa com `tick sprint`), corte de gravação contra o controle de baú com **estado comparado com itens** (não só contagem), e quebra do multibloco contra o baú.
6. Doc, lacunas honestas e "PRONTO PARA REVISÃO" por grupo; revisor independente.

Lições já pagas no sublote 1 que valem aqui: o identificador de fluido em slot só chega à lógica se o glue (`stack()` em `native/ntm/mixins/ntm/glue.rs`) passar o `custom_data` do item; a máquina reaberta é reconstruída dos próprios dados serializados, então todo estado que o Java grava (como o `lastItems` do forno de aço) tem de ir nesses dados **e** no `save_state`; o diretório de alvo do cargo não pode ser compartilhado entre a cópia de mutantes e os testes reais.

## Decisões pedidas ao coordenador

1. **Escopo:** as 16 portas/escotilhas (com `fan` e `decon`) e os 5 blocos de teste ficam fora? (proposto: sim, são de outra frente).
2. **Ordem:** começo pelo 2a? (proposto: sim, são as que não dependem de ninguém).
3. **Perfil 528 e `orbital_assembly`** (para a precass): segue pendente da decisão do usuário.
4. **`machine_ammo_press`:** confirmar se há itens de munição no catálogo (se não, vira Onda 4).

## Entregas

### `machine_orbus` (grupo 2a)

`TileEntityMachineOrbus` estende `TileEntityBarrel` com 512 000 mB; o bloco `MachineOrbus` é um `BlockDummyable` de dimensões
`{4,0,2,1,2,1}` e offset 1, com `TileEntityProxyCombo(false, false, true)` (só fluido). No port ele é um `FluidStore` (o mesmo
dos barris e do tanque grande, `fluidbox.rs`) de 512 000 mB, com os modos do barril (entrada, buffer, saída, desligado), a tela do
barril (`GUIBarrel`, que o Java não sobrescreve) e as velocidades `max(1 000, sala/100)` para entrar e `max(1 000, conteúdo/100)`
para sair (`getReceiverSpeed`/`getProviderSpeed` da orbus). O multibloco tem 80 células (79 dummies e o núcleo); as oito portas de
`getAllPorts` (as colunas 2x2 de baixo e de cima, em y 0 e 4) ligam as redes de fluido: sete células além do núcleo
(`container_bridge::FLUID_PROXIES`).

**Correção de passagem, no tanque grande também:** o Java recebe pela sala que sobra (`(max - fill) / 100`,
`TileEntityMachineFluidTank.java:49`); o port usava o conteúdo (`fill / 100`) nas duas direções. Agora entra pela sala e sai pelo
conteúdo. Vale para o `machine_fluidtank` (piso 500) e para a orbus (piso 1 000); o barril de aço (sem piso) não muda.

Lacunas declaradas:

| Lacuna | Por quê |
| --- | --- |
| NBT persistente (`IPersistentNBT`): o conteúdo do tanque não vai no item ao quebrar o bloco | O barril e o tanque grande têm a mesma lacuna; o REDD também. A quebra da orbus perde o fluido e devolve só os itens dos slots. |
| A direção de cada porta (as de baixo para baixo, as de cima para cima) | Os dutos ligam em qualquer face das células-porta, como nas demais máquinas de fluido. |
| O clique com o identificador na mão e agachado (`onBlockActivated`, `setTankType`) | O tipo se muda pelos botões da tela e pelo recipiente na mão, como no barril; o clique agachado não é um gancho do port. |
| A corrosão e o vazamento do barril de aço (`checkFluidInteraction`) | A orbus os desliga no Java ("NO!"): nada a reproduzir. |
| O modelo 3D e o nome do item na mão | O visual-gen gerou o modelo da esfera e a textura da fonte; o resto é cosmético. |

Provas (cliente release `306e5738…`, `ntm.hmod` `db5652b2…`, em `content-gen-evidence/onda2-s2/`):
`run-native-reactors.py orbus` (8 verificações, 0 falhas): o bloco vira uma loja de 512 000 mB e o multibloco tem 79 dummies e um
núcleo; três baldes de água entram pela tela de uma célula; em modo de entrada dois dutos nas portas de baixo e de cima não tiram
nada; em modo de saída a água sai pelos dois dutos para dois barris e a orbus esvazia, com a conservação (3 000 mB) entre os três
tanques; depois de salvar e reabrir o mundo o tanque, o fluido e o modo voltam iguais. Corte de gravação `orbus-state` (cinco baldes
saindo por dois dutos enquanto o mundo é salvo e morto em cada fronteira): 7 de 7 pontos, 5 000 mB entre os tanques e o modo
intacto. Corte de quebra `orbus-break` (a orbus com 6 recipientes vazios quebrada por uma célula contra o baú de controle, no modo
estrito sem salvamento adiantado de chunks): 7 de 7 pontos. No modo normal, com o salvamento adiantado de chunks, dois pontos de
morte sem save (`kill`, `kv_committed`) divergem do controle porque o baú de vanilla perde os 6 itens que a orbus conserva; o critério
do harness para esses pontos é "evidência, não prova", e a prova é o modo estrito (evidência em `orbus-break-eager`).

**Correção das portas (depois da aprovação).** As oito portas de fluido da orbus ficaram espelhadas pelo núcleo: a regra do
`docs/visual-fiel.md` (a porta `(x, y, z)` do NTM fica em `(-x, y, -z)` no bridge, porque `rotatePorts` leva o x para a esquerda de quem
coloca e o z para quem coloca) não tinha sido aplicada, e a tabela usava as tuplas do Java direto. Os dois testes de jogo da orbus
passavam porque punham os dutos ao lado das mesmas células erradas. Agora as portas são `[[0,0,1], [-1,0,1], [-1,0,0], [0,4,1],
[0,4,0], [-1,4,1], [-1,4,0]]` (mais o núcleo), o teste unitário confere as células esperadas e que as espelhadas não são portas, e o
teste de jogo põe um duto por baixo da porta de baixo e outro por cima da de cima (as faces livres, e as do Java: `DOWN` e `UP`), cada
um com o seu barril. Refeito: jogo 8/8 e corte `orbus-state` 7/7 no cliente novo.

### `machine_keyforge` (grupo 2a)

`TileEntityMachineKeyForge` (mesa do chaveiro): três slots, sem energia, sem fluido, sem multibloco (`updateEntity`, linhas
187-201). Se o slot 0 e o slot 1 têm um `ItemKeyPin` que pode transferir, o slot 1 recebe os pinos do slot 0 (`setPins`); se o slot
2 tem um, ele recebe `rand.nextInt(900) + 100` **a cada tick**. Os itens são os `ItemKeyPin` do Java: `key`, `key_fake` (que não
transfere: `canTransfer`, não muda nem é copiado) e os quatro cadeados (`padlock_rusty`, `padlock`, `padlock_reinforced`,
`padlock_unbreakable`). Os pinos são o int `pins` do NBT; no port, o int `pins` do `custom_data` do item, e os demais dados do item
ficam (`snbt::with_field`).

| Regra | Java | Port |
| --- | --- | --- |
| Mão | `Slot` comum: aceita qualquer item nos três slots | `slot_allows`: qualquer item, só por clique (`manual`) |
| Funil | `isItemValidForSlot` é sempre falso: nenhum funil põe nada; `canExtractItem` é verdadeiro | `container`: sem entradas, três saídas |
| Lados | `getAccessibleSlotsFromSide`: de cima o slot 0, de baixo o 1, dos lados o 2 | `container().faces` |
| Shift-click | `transferStackInSlot`: do inventário só para o slot 0 | `slot_filters`: os slots 1 e 2 recusam o shift |
| Aleatório | `worldObj.rand` | xorshift64* com o estado guardado na máquina |
| Tela | `ContainerMachineKeyForge`: slots (44,36), (80,36), (116,36); `gui_keyforge.png` 176x186; dois painéis de informação | tela do visual-gen; os painéis são dicas de texto nas posições (12,28) e (12,44), sem os ícones |

O glue (`stack()`) passa o `custom_data` desses seis itens à lógica (como para o identificador de fluido e o item quebrado).

Lacunas: o shift-click que sai dos slots 1 e 2 (no Java vai para o slot 0, uma esquisitice de `mergeItemStack(var5, 0, 1, false)`; no
port vai para o inventário); os ícones dos painéis de informação; o nome do item e o modelo, que são os do bloco da fonte.

Provas (cliente release `a372c919…`, `ntm.hmod` `54e13d99…`, em `content-gen-evidence/onda2-s2/`): 6 testes de unidade e um de
servidor pela ponte (os pinos vão do slot 0 ao 1, o 2 rola três dígitos, a chave falsa fica como estava); no jogo
`run-native-reactors.py keyforge` (8 verificações, 0 falhas): a segunda recebe os pinos da primeira, a terceira muda a cada leitura
com três dígitos, a chave falsa guarda os seus pinos, um funil embaixo tira o item do slot 1 e um funil em cima não põe nada (guarda
as três chaves do baú), e depois de salvar e reabrir o mundo os dois primeiros slots seguem iguais e o terceiro continua rolando.

### `machine_funnel` (grupo 2a)

`TileEntityMachineFunnel` (o funil de compressão): 18 slots, sem energia, sem fluido, sem multibloco. Os nove de cima recebem itens e
os nove de baixo recebem o resultado. A cada tick, para cada coluna `i`, se o slot `i` tem 9 ou mais itens e uma grade 3x3 cheia dele
tem receita, 9 itens viram o resultado; senão, se tem 4 ou mais e uma grade 2x2 cheia dele tem receita, 4 viram o resultado
(`updateEntity`, linhas 38-64; `getFrom9`/`getFrom4`, linhas 120-142, perguntam à lista do `CraftingManager`). O botão da tela troca o
modo (`receiveControl`, linha 166): 3x3 e depois 2x2, só 3x3, só 2x2.

| Regra | Java | Port |
| --- | --- | --- |
| Compressão | 9 antes de 4; a contagem da receita 2x2 sai inteira (4 pedras dão 4 tijolos de pedra) | `Funnel::step`, uma compressão por coluna por tick |
| Saída ocupada | outro item ou sem espaço (`getMaxStackSize` do resultado): nada acontece, e **não** tenta a receita 2x2 | igual; o limite vem de `max_stack_size` do catálogo (64 para o vanilla) |
| Modos | `mode` 0, 1, 2; o botão soma 1 e volta a 0 depois de 2 | `Slotted::button`; o modo é salvo no estado da máquina e na tela (valor 0) |
| Mão | `Slot` comum nos nove de cima; `SlotCraftingOutput` nos de baixo (só tira) | `slot_filters` (os de baixo com `output`) e `slot_allows` |
| Funil/duto | `isItemValidForSlot`: só os nove de cima, e um item sem receita só entra num slot já ocupado (linhas 85-90) | `funnel::allows` |
| Lados | `getAccessibleSlotsFromSide`: de baixo os resultados, dos outros lados as entradas | `container().faces` |
| Receitas | a lista do `CraftingManager` de 1.7.10 | tabela embutida `data/machines/funnel.json`, gerada por `tools/content-gen/funnel_compression.py` |

**A tabela.** O port não tem o `CraftingManager` do 1.7.10, então a tabela junta, na ordem da lista do Java (a primeira receita de um
item vence): (1) as receitas do vanilla 1.7.10 que enchem uma grade com um item só, mapeadas para as identidades da 26.3 e conferidas
em `registries.json` (lingotes/gemas/redstone/carvão em blocos, pepitas de ouro em lingote, trigo em fardo de feno, fatias em melancia,
pó de glowstone, quartzo, bolas de argila, tijolo, tijolo do Nether, bolas de neve, areia, pedra e as seis tábuas em mesa de trabalho);
(2) as receitas do catálogo (`content/data/ntm/recipe`: moldadas 3x3/2x2 de uma chave só e sem forma de 9/4 ingredientes iguais), com
as tags expandidas nos seus itens. Hoje: 207 entradas de 9 e 39 de 4; 17 colisões (todas com o mesmo resultado, a primeira vence e a
outra fica listada); nada pulado. O carvão vegetal **não** comprime (a receita do bloco de carvão do 1.7.10 pede `coal` de dano 0).

Lacunas: (a) o jar do 1.7.10 está ofuscado e sem fonte nesta máquina, então a lista do vanilla foi escrita à mão a partir das classes
`CraftingManager`, `RecipesIngots` e `RecipesCrafting` do 1.7.10 e **não tem linhas citadas**; se aparecer um desvio, é aqui;
(b) o OreDict de 1.7.10 que casa itens de outras mods com as receitas do vanilla (por exemplo `plankWood` com tábuas da mod) vale só
para o que o catálogo e a lista acima já cobrem; (c) a dica do botão lista os três modos (a do Java mostra só o atual: o visual-gen não
tem dica condicional); (d) um duto que tira dos lados (o Java deixa tirar as entradas por `canExtractItem` dos lados) não consegue no
port, só o funil de baixo, que tira os resultados; (e) o modelo do bloco continua sendo a caixa genérica (o renderizador não foi
portado); (f) o shift-click do menu não foi provado em jogo: o Java manda os 18 slots para o inventário (de trás para a frente) e o inventário para os nove de cima (`ContainerFunnel.transferStackInSlot`); no port vale o padrão do motor para os slots sem regra de shift.

Provas: veja a entrega abaixo (hashes e logs em `content-gen-evidence/onda2-s2/`).

**Entrega do funil** (cliente release `559a2afe…`, `ntm.hmod` `c2cf924d…`, em `content-gen-evidence/onda2-s2/`; o build usou
`SKIP_UPSTREAM_CHECK=1` porque o Horizon oficial avançou para `f24ee1ac` depois do pin, e o empacotador do próprio motor, porque o
do `Games/horizon` recusa o menu da mesa de armadura que veio da main):

- unidade: 9 testes em `funnel.rs` mais 2 (regras de slot e faces pela `native`), 1 pela ponte
  (`the_funnel_compresses_a_column_and_its_button_cycles_the_mode`) e 5 do extrator (`tools/content-gen/test_funnel.py`, que também
  confere que o arquivo embutido é o que as fontes dão);
- jogo, `run-native-reactors.py funnel` (18 verificações, 0 falhas): 9 lingotes de ouro dão um bloco por vez e sobram os restantes,
  4 de pó de glowstone dão glowstone e sobra 1, o que não tem receita fica, o botão passa por só 3x3 (4 de pó ficam), só 2x2 (o pó
  vai e 9 de ouro não) e volta ao padrão (o ouro vai), um funil em cima põe o ferro e a terra que se junta a um slot ocupado e não
  põe graveto em slot vazio, um funil embaixo tira os resultados; depois de salvar e reabrir, o modo e os itens seguem iguais e 4 de
  pó ainda comprimem;
- cortes de save no modo estrito (`HORIZON_NO_EAGER_SAVE=1`): `funnel-state` 7/7 (90 lingotes de ouro em duas colunas, comprimindo, o
  total em lingotes e blocos fica 90 e o modo fica) e `funnel-break` 7/7 (quebrar o funil com itens dentro);
- mutantes: 18 escolhidos à mão, 17 mortos. O que sobrevive (`count < 4` trocado por `count < 3` na escolha do modo 3x3) é
  equivalente: o teste seguinte `count < size` já descarta um stack de 3.

### `machine_radiolysis` (grupo 2a)

`TileEntityMachineRadiolysis` (a câmara de radiólise): um RTG que craqueia fluidos com o calor e esteriliza itens. Multibloco 3x3x3 no
centro da base (`getDimensions` `{2,0,1,1,1,1}`, offset 0), com quatro portas `flare` (`makeExtra`) ao lado do núcleo, cada uma aberta
para fora, que levam energia, fluido e inventário. 15 slots (linha 51): 0 a 9 pelotas de RTG, 10 e 11 o identificador de fluido (entra e
sai), 12 e 13 a esterilização (entra e sai), 14 a bateria; três tanques de 2 000 mB (a entrada e duas saídas).

Um tick (`updateEntity`, linhas 107-141), na ordem: a energia vai para o item da bateria (`Library.chargeItemsFromTE`); as pelotas dão
calor (`RTGUtil.updateRTGs`, sem teto, e cada uma decai) e `calor * 10` HE entram no armazém (teto 1 000 000); o identificador do slot 10
tipa o tanque de entrada e vai para o slot 11, que precisa estar vazio (`FluidTank.setType(10, 11, slots)`, linha 157); os tanques de
saída recebem o tipo da receita da entrada (`setupTanks`), e uma entrada sem receita zera os três; com calor acima de 100 craqueia 100 mB
a cada `max(30 - 0.1 * (calor - 100), 5)` ticks, só se as duas saídas têm lugar (`crack`); com calor de 200 ou mais esteriliza a cada 100
ticks (`sterilize`).

| Regra | Java | Port |
| --- | --- | --- |
| Receitas | `RadiolysisRecipes.registerRadiolysis`: água vira 80 mB de peróxido e 20 de hidrogênio (linha 50), mais as 12 receitas de craqueamento, com as saídas como estão | `data/machines/radiolysis.json`, de `tools/content-gen/radiolysis_data.py` (13 entradas; a segunda saída que falta é `NONE`, como no querosene) |
| Calor | `RTGUtil.updateRTGs` sobre os slots 0 a 9 | `radioisotopes::update_rtgs`, compartilhada com o RTG (que mantém o teto de 600 e 200) |
| Energia | provedora; `updatePortPOFIFO` oferece antes de gerar | `EnergyNode` provedor; o que o calor fez neste tick (`pending`) entra no armazém depois da oferta da rede, e o item da bateria leva a sua parte antes dele |
| Bateria | `Library.chargeItemsFromTE`: qualquer bateria carrega | só a bateria criativa (leva tudo); o port não tem carga em outros itens (igual às outras máquinas) |
| Fluido | `getReceivingTanks` a entrada, `getSendingTanks` as duas saídas | `FluidNode`: a entrada demanda o seu tipo, as saídas oferecem |
| Mão | `Slot` comum em 0 a 9, 10, 12 e 14; `SlotTakeOnly` em 11 e 13 | `slot_filters` (11 e 13 com `output`) e `slot_allows` |
| Funil/duto | `isItemValidForSlot`: pelota nos slots de pelota, qualquer coisa no 12; `canExtractItem`: pelota gasta e o slot 13; todas as faces dão `{0..9, 12, 13}` | `radiolysis::allows` e `container()` |
| Esterilização | `sterilize`: um alimento (`ItemFood`, menos a panqueca) no slot 12 é comido (um por vez); depois um item com `ntmContagion` vai para o 13 sem a marca (um por vez, juntando ao igual até o limite) | `Radiolysis::sterilize`; `ntmContagion` é um campo do `custom_data` do item; o glue passa o `custom_data` de qualquer item que o tenha e o tira quando a lógica o remove |
| Relógio | `getTotalWorldTime` | o tick da máquina não recebe o relógio do mundo: um contador próprio (`clock`), então a fase do craqueamento e da esterilização muda, a cadência não |

**Listas.** Os alimentos que a esterilização come vêm de duas fontes: os da NTM, achados pelas classes que estendem `ItemFood` e
`ItemSoup` e pelos itens que `ModItems` faz delas (o que o catálogo não tem, como `crayon`, `apple_lead`, `apple_schrabidium`,
`tem_flakes` e `med_schizophrenia`, fica `skipped`), e os do vanilla 1.7.10, escritos à mão (o jar está ofuscado e sem fonte aqui) e
mapeados para a 26.3. As pelotas gastas que saem pelo funil são os seis `pellet_rtg_depleted__*` do catálogo.

Lacunas: (a) o relógio próprio no lugar de `getTotalWorldTime`; (b) a lista vanilla de alimentos é curada, sem linhas citadas; (c) o
contágio ainda não existe no port (`EntityEffectHandler` e `ModEventHandler` o põem nos itens), então só aparece com um item dado por
comando; (d) o limite de pilha do item de saída é 64 (o port não conhece o de cada item; o contágio vale em itens que no Java não
empilham); (e) baterias que não a criativa; (f) os painéis de informação à esquerda da tela (ícones fora dela) não são desenhados:
o calor vai na dica da barra de energia; (g) o shift-click do menu não foi provado em jogo; (h) as portas, em cada célula, não guardam a
direção do Java (`canConnect` recusa só `DOWN`) além de ficarem abertas para fora; (i) o modelo do bloco continua sendo a caixa genérica.

**Entrega da radiólise** (árvore com a main `318887c9` mesclada, cliente release `1b2d819f…`, `ntm.hmod` `e52ac80b…`, em
`content-gen-evidence/onda2-s2/`; mesmo build do funil: `SKIP_UPSTREAM_CHECK=1` e o empacotador do próprio motor):

- unidade: 20 testes em `radiolysis.rs` (receitas, calor e energia depois da oferta, bateria criativa, identificador, receita ausente,
  craqueamento de 100 em 100, saída cheia, limites de 100 e 200 de calor, período, esterilização, comida, pilha de 64, regras de
  mão e de funil, tela, rede, regras pela `native`), 1 pela ponte, o teste novo de `snbt::without_field`, 4 do extrator
  (`tools/content-gen/test_radiolysis_data.py`); suíte `ntm-core` 771 passando e 0 falhas; clippy `--workspace --all-targets -D warnings`
  limpo; a refatoração de `RTGUtil.updateRTGs` em `radioisotopes::update_rtgs` não mudou nenhum teste do RTG;
- jogo, `run-native-reactors.py radiolysis` (15 verificações, 0 falhas): o identificador de água tipa a entrada e vai para o slot 11, a
  água de um barril leste (4 baldes) vira peróxido e hidrogênio na razão de 4 para 1 (3 200 mB de peróxido no barril oeste, 800 de
  hidrogênio no tanque), os três tanques ficam tipados pela receita, o calor é 200 com quatro pelotas de polônio, o armazém enche, a
  espada contaminada sai do slot 13 sem a marca e as pelotas perdem vida; depois de salvar e reabrir, slots, tipos dos tanques e vida das
  pelotas seguem como estavam e a câmara segue rodando; e `funnel` de novo, 18/18, no mesmo cliente;
- cortes de save no modo estrito (`HORIZON_NO_EAGER_SAVE=1`): `radiolysis-state` 7/7 (quatro pelotas, o identificador e a espada
  contaminada: cada item existe exatamente uma vez, o identificador num dos slots de fluido e a espada num dos de esterilização) e
  `radiolysis-break` 7/7 (quebrar a câmara com itens dentro, contra o baú de controle);
- mutantes: 36 escolhidos à mão, 36 mortos; cinco só morreram depois de testes mais fortes (limite de 100 de calor com o relógio no
  múltiplo certo, exatamente 100 mB, saída com vaga exata, limpeza do contágio ao juntar, tanque sem tipo).


### `machine_exposure_chamber` (grupo 2a)

`TileEntityMachineExposureChamber` (a câmara de exposição): uma cápsula de partícula expõe oito lingotes do ingrediente da sua receita,
um de cada vez. Multibloco irregular (`MachineExposureChamber`): o corpo 5x5x5 (`getDimensions` `{4,0,2,2,2,2}`, offset 2) e cinco
caixas (`getAllDimensions`: o corredor de 6 células à direita, duas pontes e duas pernas), 17 células de comprimento; as cinco portas
(`getPorts`, `makeExtra`) ficam nas pontas das pernas e do corredor, cada uma aberta para fora, e levam energia e inventário. Oito
slots no Java (linha 91): 0 a partícula, 1 a partícula carregada (interna), 2 a cápsula vazia, 3 o ingrediente, 4 a saída, 5 a bateria,
6 e 7 as melhorias; a tela e o port usam os sete que aparecem (o 1 é um fantasma de tamanho 0 no Java; aqui é o id da cápsula em
`loaded`).

Um tick (`updateEntity`, linhas 109-192): a bateria dá a energia; as melhorias fixam o tempo e o consumo (200 ticks e 10 000 HE, em
`int`: `tempo = 200 - 50 * velocidade`, `consumo *= velocidade / 2 + 1`, `tempo *= potência / 2 + 1`, `consumo /= potência + 1`,
`tempo /= overdrive + 1`, `consumo *= 2 * overdrive + 1`); sem partícula carregada e com uma cápsula e um ingrediente que fazem receita,
a cápsula é carregada (a vazia vai para o slot 2, que precisa aceitá-la) e dá 8 usos; com partícula, energia para um tick e lugar na
saída, o progresso corre e, no fim do tempo, um ingrediente vira a saída e um uso se gasta; sem usos a partícula sai.

| Regra | Java | Port |
| --- | --- | --- |
| Receitas | `ExposureChamberRecipes.registerDefaults` (linhas 29-36): higgs + urânio dá schraranium, higgs + U238 dá schrabidium, escuro + plutônio dá euphemium, sparkticle + schrabidate dá dineutronium; o ingrediente é `OreDictStack(X.ingot())` | `data/machines/exposure.json`, de `tools/content-gen/exposure_data.py` (o nome do OreDict é `ingot` + o primeiro nome da `DictFrame`; os itens, do `oredict.json`; a cápsula vazia é o `setContainerItem` da partícula) |
| Modo caro | a receita do dineutronium com a matéria degenerada, em `enableExpensiveMode` | fora (desligado por padrão); fica `skipped` com o motivo |
| Melhorias | `UpgradeManagerNT`, velocidade, potência e overdrive até 3 | `upgrades::level`, as mesmas contas em inteiros |
| Estado fora dos slots | `power`, `progress`, `savedParticles`, o slot 1 | `save_state`: a energia, o progresso, os usos e o código da partícula viajam com os slots: um corte não deixa a cápsula gasta sem usos nem usos sem a cápsula |
| Mão | `Slot` comum nos slots 0, 3, 5, 6, 7; `SlotTakeOnly` (cápsula vazia) e `SlotCraftingOutput` (saída) | `slot_filters` (a cápsula vazia e a saída com `output`) e `slot_allows` |
| Funil/duto | `isItemValidForSlot` (linhas 196-231): só entram partícula e ingrediente que fazem receita (com o outro já presente) ou, sem nada, qualquer um de alguma receita; `canExtractItem`: a cápsula vazia e a saída; todas as faces dão `{0, 2, 3, 4}` | `exposure::allows` e `container()` |
| Tela | `GUIMachineExposureChamber` 176x186 (`gui_exposure_chamber.png`): barra de energia, progresso `progresso * 42 / (tempo + 1)`, partículas `usos * 16 / 8`, lâmpada se `consumo <= energia` | visual-gen: as barras, a lâmpada e as dicas de energia e partículas; sete valores |
| Portas | `rotatePorts`: `(-7, 0, ±1)`, `(-8, 0, ±1)` e `(-8, 0, 0)` | `(7, 0, ∓1)`, `(8, 0, ∓1)` e `(8, 0, 0)` no quadro de quem coloca, cada uma aberta para fora do volume (teste unitário que confere que cada porta é célula do volume e que a face livre não entra nele) |

Lacunas: (a) a receita do modo caro; (b) a regra de `isItemValidForSlot` que usa a partícula interna (linha 205) como referência: com
uma cápsula carregada e o slot de entrada vazio o Java recusa um ingrediente que o funil traz com o slot do ingrediente vazio, e o port
o aceita (a política só vê os slots do motor, e a partícula interna não é um deles: guardá-la num slot soltaria uma cápsula ao quebrar a máquina);
(c) o modelo do bloco continua a caixa genérica e o rotor não gira; (d) os painéis de informação das melhorias não são desenhados; (e) o
shift-click do menu não foi provado em jogo; (f) só a bateria criativa carrega no slot 5 (igual às outras máquinas).

**Entrega da câmara de exposição e da correção da orbus** (cliente release `69342871…`, `ntm.hmod` `d468ea81…`, em
`content-gen-evidence/onda2-s2/`; mesmo build do funil: `SKIP_UPSTREAM_CHECK=1` e o empacotador do próprio motor):

- unidade: 17 testes em `exposure.rs` (receitas, carga e usos, ritmo de 200 ticks, uma partícula faz oito, sem energia, saída cheia ou de
  outro item, a cápsula vazia que não cabe, as melhorias em inteiros, o estado que viaja com os slots, a tela, as regras de mão e de
  funil, as regras pela `native`, o container, a energia), 1 pela ponte (`the_exposure_chamber_runs_a_capsule_through_the_engine_slots`),
  1 das portas (cada porta é célula do volume e abre para fora dele), 5 do extrator (`tools/content-gen/test_exposure_data.py`); e o
  teste das portas da orbus, que agora confere as células esperadas e que as espelhadas não são portas; suíte `ntm-core` 787 passando e
  0 falhas; clippy `--workspace --all-targets -D warnings` limpo;
- jogo, `run-native-reactors.py exposure` (23 verificações, 0 falhas): dois ingredientes saem a cada 200 ticks com a cápsula vazia no
  slot 2 e uma cápsula esperando, a melhoria de velocidade 3 leva o tempo a 50 e o consumo a 20 000, um funil numa perna põe a
  cápsula escura e os lingotes de plutônio (e nada do que nenhuma receita pede: a terra e o graveto ficam no baú e no funil) e um
  funil embaixo da outra perna tira a cápsula vazia e o euphemium; depois de salvar e reabrir, os slots, a partícula carregada e os usos
  são os mesmos e o progresso segue de onde estava (nenhum lingote some nem se faz duas vezes); `orbus` 8/8 com as portas corrigidas;
  `radiolysis` de novo, 15/15, no mesmo cliente;
- cortes de save no modo estrito (`HORIZON_NO_EAGER_SAVE=1`): `exposure-state` 7/7 (duas cápsulas e dez lingotes: as cápsulas, no slot
  ou como a vazia que a carregada deixa, são duas, os lingotes dez, e a partícula carregada está onde está a cápsula vazia),
  `exposure-break` 7/7 (quebrar a câmara com itens dentro, contra o baú de controle) e `orbus-state` 7/7;
- mutantes: 38 escolhidos à mão, 38 mortos (dois só depois de um teste do container: `inputs` e `outputs`).

### `machine_cyclotron` (grupo 2a)

`TileEntityMachineCyclotron` (o cíclotron): três faixas em que uma partícula bombardeia um alvo e o transforma em outro pó, com um
pouco de antimatéria, gastando muita água como refrigerante. Multibloco 5x5x3 (`getDimensions` `{2,0,2,2,2,2}`, offset 2) com doze
portas (`getAllPorts`, `makeExtra`): três em cada lado da base, abertas para fora; levam energia, fluido e inventário. 12 slots
(linha 66): 0 a 2 as partículas, 3 a 5 os alvos, 6 a 8 as saídas, 9 a bateria, 10 e 11 as melhorias; três tanques: água de entrada
(32 000 mB), vapor gasto de saída (32 000 mB) e antimatéria de saída (8 000 mB); 100 000 000 HE.

Um tick (`updateEntity`, linhas 109-137): a bateria dá a energia; se pode processar (energia para o tick, água, lugar no tanque de
vapor e uma faixa cuja receita tem lugar na saída; `canProcess`, linhas 155-171), o progresso corre pela velocidade, a energia sai, a água
vira vapor (`coolant` por tick) e, aos 690, cada faixa que pode faz a sua saída e soma a antimatéria da receita (`process`, linhas
189-224); senão o progresso volta a 0. As melhorias (`UpgradeManagerNT`, até 3 cada): velocidade `+1` por nível no progresso, potência
`-100 000 HE` por nível, efeito `500 / (nível + 1) * velocidade` mB de água por tick (inteiros: `500 / 4 * 4 = 500`, não `500`).

| Regra | Java | Port |
| --- | --- | --- |
| Receitas | `CyclotronRecipes.registerDefaults` (linhas 28-93): 42 receitas de partícula (lítio, berílio, carbono, cobre, plutônio) + alvo (pó de OreDict ou item) dando um pó e uma quantidade de antimatéria | `data/machines/cyclotron.json`, de `tools/content-gen/cyclotron_data.py`: 38 receitas; 4 ficam `skipped` porque o catálogo não tem o item (`dustPhosphorus` duas vezes, `dustLanthanium`, `dustActinium`); o mapa do Java é um `HashMap`, mas nenhum alvo é de duas receitas da mesma partícula (`collisions` vazio), então a ordem não importa |
| Estado fora dos slots | `power`, `progress` e os três tanques | `save_state`: viajam com os slots: um corte não deixa os insumos de uma faixa gastos sem a saída, nem a água gasta sem o vapor |
| Mão | `Slot` comum nos slots 0 a 5, 9 a 11; `SlotCraftingOutput` nas saídas | `slot_filters` (as saídas com `output`) e `slot_allows` |
| Shift-click | partícula (as 5 do Java) para 0..3; bateria para 9; melhoria para 10..12; o resto para os alvos 3..6 | `slot_filters`: partículas das receitas para 0..3, ingredientes das receitas para 3..6, a bateria criativa, as melhorias (item sem receita não entra nos alvos) |
| Funil/duto | `isItemValidForSlot` (linhas 304-323): uma partícula de alguma receita nos slots 0 a 2, um alvo de alguma receita nos 3 a 5; `canExtractItem` as saídas | `cyclotron::allows` e `container()` |
| Faixas por célula | `getAccessibleSlotsFromSide(x, y, z, side)`: cada uma das três células de um lado liga uma faixa (`{0, 3}`, `{1, 4}`, `{2, 5}`) e as saídas | a política de slots do motor vale por máquina, não pela célula (`docs/native-slot-policy.md`): todas as células alcançam os slots 0 a 8 |
| Tela | `GUIMachineCyclotron` 190x215: energia, seta de progresso `progresso * 34 / 690`, lâmpada com a seta, três tanques | visual-gen; treze valores (o 34 é o divisor da seta) |
| Portas | `rotatePorts`: `(2, 0, ±1)`, `(-2, 0, ±1)`, `(±1, 0, 2)`, `(±1, 0, -2)` | os mesmos pontos em `(-x, -z)` no quadro de quem coloca, cada uma aberta para fora (teste unitário) |

**A água.** Um ciclo pede 690 ticks de progresso, e o refrigerante é `500 / (efeito + 1) * velocidade` mB por tick: com velocidade 1 são
345 000 mB por ciclo, e com velocidade 3 e efeito 3 ainda 86 000 (o total quase não depende da velocidade). O tanque de entrada tem
32 000 mB e o de vapor 32 000 mB: sem uma rede que traga a água e leve o vapor, a máquina roda só uns 64 ticks. Os testes de unidade
usam tanques maiores; o de jogo usa um orbus cheio de barris como fonte e outro como destino do vapor.

Lacunas: (a) as faixas por célula (acima); (b) as quatro receitas que o catálogo não tem; (c) os plugues decorativos (`plugs`, itens que
se põem com o botão direito e só mudam o modelo) e o modelo e a animação do bloco; (d) o painel de informação das melhorias é uma dica
de texto; (e) o shift-click não foi provado em jogo; (f) só a bateria criativa carrega no slot 9.

**Entrega do cíclotron** (cliente release `91d64a9c…`, `ntm.hmod` `93a08c7c…`, em `content-gen-evidence/onda2-s2/`; mesmo build do
funil: `SKIP_UPSTREAM_CHECK=1` e o empacotador do próprio motor):

- unidade: 14 testes em `cyclotron.rs` (38 receitas do catálogo, melhorias de velocidade/potência/efeito em inteiros incluindo a ordem
  `500 / (efeito + 1) * velocidade`, ciclo de 690 ticks com 345 000 mB de água e antimatéria, velocidade 3 em 173 ticks, múltiplas
  faixas ao mesmo tempo, esperas e perdas de progresso por falta de energia/água/espaço/receita, pilha até o limite, teto de
  antimatéria, `save_state`/`load_state`, tela em 13 inteiros, regras de mão e de funil, regras pela `native`, container, rede de água
  não ofertando o tanque de entrada e portas), 1 pela ponte (`the_cyclotron_runs_a_lane_through_the_engine_slots`), 1 das portas
  (`the_cyclotron_has_three_ports_on_each_side_of_its_footprint`), 5 do extrator (`tools/content-gen/test_cyclotron_data.py`); suíte
  `ntm-core` 804 passando e 0 falhas no lib (+ 47 nos 9 binários de integração, 5 ignorados pré-existentes); clippy
  `--workspace --all-targets -- -D warnings` limpo;
- jogo, `run-native-reactors.py cyclotron` (27 verificações em 3 fases — `setup` 16/16, `capture-state` 8/8, `check-state` 3/3 —, 0 falhas,
  `backend_warnings: []`): dois orbus ligados por dutos tipados trazem água (12 barris = 192 000 mB) e recolhem vapor gasto, um funil
  alimenta partícula/pó na faixa 0 e recusa terra (que fica fora do cíclotron), a faixa 1 recebe partícula de berílio e pó de tório,
  melhorias `speed_3` + `effect_3` reduzem o consumo para ~86 000 mB no ciclo, as duas saídas chegam ao baú inferior e o tanque de
  antimatéria soma 75 mB com conservação exata da água; depois de salvar e reabrir (`speed_1` + `effect_3`), os slots e o progresso
  seguem de onde estavam e completam a saída esperada;
- cortes de save no modo estrito (`HORIZON_NO_EAGER_SAVE=1`): `cyclotron-state` 7/7 (dois conjuntos de `part_lithium` + `powder_lithium`
  com `speed_1` + `effect_3`, conservando os 4 insumos entre saídas feitas e slots sem perda nem duplicação, com progresso coerente e
  conservação de água/antimatéria) e `cyclotron-break` 7/7 (quebrar o cíclotron com itens dentro, contra o baú de controle);
- mutantes: 42 escolhidos à mão, 42 mortos (dois só depois de reforçar os testes unitários: a ordem inteira `500 / (efeito + 1) * velocidade`
  combinando `speed_1` + `effect_2`, e a oferta de fluido `tanks[1..]` com água no tanque de entrada).

### `machine_annihilator` (grupo 2a)

`TileEntityMachineAnnihilator` (o aniquilador): destrói itens e fluidos em *pools* nomeados persistentes no mundo
(`AnnihilatorSavedData`, chave `ANNIHILATOR_KEY`), ventila radiação de itens pela chaminé e de fluidos pelo núcleo, monitora uma
chave no slot 8 e entrega marcos de receita. Multibloco 3x3x9 (`MachineAnnihilator.getDimensions` `{2,0,4,4,1,1}`, *offset* 4) mais a
chaminé `{8,-2,1,1,1,1}` em `(0, 9, 3)` — 134 dummies + núcleo — com três portas na frente (`getPorts` / `getAllPorts`, linhas
55-67 e 42-48): `[0, 0, -4]`, `[-1, 0, -3]` e `[1, 0, -3]` no quadro de quem coloca, abertas para fora, levando fluido e
inventário. 11 slots (linha 48): `0` entrada (lixo), `1` identificador de fluido (`type_from_identifier`), `2..7` saídas (2x3,
`SlotCraftingOutput`), `8` monitor, `9` pedido de marco (*request*), `10` entrega do marco (`SlotCraftingOutput`); um tanque de
2 500 000 mB com prioridade `LOW` (`getFluidPriority`, linha 235), sem energia.

Um tick (`updateEntity`, linhas 75-141): atualiza o tipo do tanque pelo slot 1; com `pool` não vazio: (a) consome o stack inteiro
do slot 0, emite radiação na chaminé `(rad * 5.0).min(1_000.0)` e soma no *pool* (`Item`, `Comp` e cada nome de OreDict),
colocando o maior marco em `2..7` (`dict > comp > item`, `tryAddPayout`); (b) drena todo o tanque (`tank.mb > 0`), emite radiação
no núcleo `mb * 2 * vent_radiation` e soma no *pool* (`Fluid`), colocando o marco em `2..7`; (c) atualiza `monitor` pelo slot 8
(`Fluid` se for identificador de fluido, senão `Comp`); (d) se o slot 9 tem itens, consome **1** por tick, emite radiação na
chaminé, soma no *pool* com `always_payout = true` e deposita ou empilha o marco no slot 10.

| Regra | Java | Port |
| --- | --- | --- |
| Persistência em cortes | `AnnihilatorSavedData` (`WorldSavedData`, chave `"annihilator"`) em `perWorldStorage` | `WorldState::annihilator` sob `ANNIHILATOR_KEY` + WAL por máquina (`seq` e `pending: Vec<PendingPush>`); na adoção de chunk (`native_server.rs`), `replay_push` reexecuta apenas `seq > applied_seq`, e `pending` é podado após `applied_seq` confirmado (`prune_annihilator_pending`) |
| Portas | `getPorts` / `getAllPorts`: três células frontais abertas para fora (`[0,0,-4]`, `[-1,0,-3]`, `[1,0,-3]`) | `annihilator::port_cells` + `fluid_server` / `container_bridge`; outras células e lados recusam fluido e funis |
| Mão e funil | `isItemValidForSlot`: `0`, `8`, `9` aceitam qualquer item, `1` só `IItemFluidIdentifier`; `canExtractItem`: `2..7`; `getAccessibleSlotsFromSide`: `[0, 2..7]` | `annihilator::allows`, `annihilator::container()` (`inputs: [0]`, `outputs: 2..8`, `faces` `[0, 2..7]`) e `slot_filters` (`quick_reverse` no `0` e `1`) |
| Shift-click | `transferStackInSlot` (`ContainerMachineAnnihilator`, linhas 36-68): identificador de fluido tenta o slot 1, demais itens vão para o slot 0; slots da máquina vão para o inventário | `slot_filters`: `accept` `ntm:fluid_identifier_multi` no slot 1 antes do slot 0; `monitor`, `request` e `payout` bloqueiam entrada por shift (`quick_move: []`) |
| Receitas de marco | `AnnihilatorRecipes` (linhas 65-82): 16 entradas presas em `GeneralConfig.enable528` pagando `ntm:blueprints` | `highest_payout_from_recipe` / `highest_payout_from_key` e precedência `dict > comp > item` portados e testados; `recipes()` devolve `&[]` por padrão enquanto `enable528` e `ntm:blueprints` não existem no port |
| Radiação | `onDestroy`: `(rad * 5).min(1000)` em `(x - dir.x * 3, y + 9, z - dir.z * 3)`; fluidos via `FluidTrait.onRelease(BURN, fill * 2)` | `chimney_cell` (`[0, 9, 3]` no quadro do colocador) para slots `0` e `9`, núcleo para fluidos (`mb * 2 * vent_radiation`), via `chunk_rad_pending` (atômico com `commit_machines`) |
| Tela | `GUIMachineAnnihilator` 176x208 (`gui_annihilator.png`), campo `GuiTextField` para `pool` e dica de `monitorBigInt` | visual-gen (`machine_annihilator.json`): layout, texto estático `"Recycling"`, dica `"Monitored: {0}"` com 1 inteiro |

Lacunas: (a) as 16 receitas padrão de `AnnihilatorRecipes` dependem de `enable528` (desligado) e de `ntm:blueprints` (ausente em
`content/ids.json`), ficando vazia a tabela padrão; (b) sem `GuiTextField` editável na tela para trocar `pool` de `"Recycling"`
em jogo (o campo e `Annihilator::pool` estão prontos no núcleo); (c) da liberação `FluidReleaseType.BURN`, só a emissão radiativa
(`vent_radiation`) está ligada (`FT_Polluting` de fuligem/veneno ainda não portado), e não há partículas de chama na chaminé,
som nem renderizador OBJ animado da correia; (d) o shift-click foi provado em teste unitário (`native::slot_filters`), não
interativamente em jogo.

**Entrega do aniquilador** (cliente release `28fbb9f1…`, `ntm.hmod` `8ff9b5ba…`, em `content-gen-evidence/onda2-s2/`; build com
`SKIP_UPSTREAM_CHECK=1` e empacotador do próprio motor):

- unidade: 6 testes em `annihilator.rs` (incremento em `Item`/`Comp`/`Dict` e monitoramento de `Comp`/`Fluid`, troca e recusa de
  identificador de fluido, precedência de marcos `dict > comp > item` e slot `REQUEST` 1/tick com `always_payout`, emissão de
  radiação na chaminé vs núcleo, idempotência do replay WAL vs `confirmed_seq`, regras de slot e `container()`), 4 em
  `server/tests/annihilator_tests.rs` (cortes de save A–E incluindo `refuse_commits`, portas frontais vs células não-porta,
  radiação de urânio na chaminé e `sas3` no núcleo, filtros `native_slot_filters`), 1 pela ponte em `furnaces_tests.rs`
  (`the_annihilator_runs_items_monitor_and_request_through_the_bridge`); suíte `ntm-core --lib` 815 passando e 0 falhas (5 ignorados
  pré-existentes) + 118 passando e 0 falhas nos 12 binários de integração sem `save_model` (`save_model` atingiu timeout de 600s em
  `--release` após passar os 10 primeiros de 11 testes); clippy `--workspace --all-targets -- -D warnings` limpo;
- jogo, `run-native-reactors.py annihilator` (27 verificações em 3 fases — `setup` 18/18, `capture-state` 5/5, `check-state` 4/4 —,
  0 falhas, `backend_warnings: []`): multibloco de 134 dummies + núcleo com `pool = "Recycling"`, baú + funil alimentando pela
  porta lateral `(-1, 0, -3)` (12 pedregulhos + 8 minérios de ferro) enquanto o slot 9 consome 3 minérios de ferro e o monitor
  (slot 8) troca de `12` para `11`; lado sem porta `(2, 0, 0)` recusa água do barril (`1000` mB ficam no barril, monitor `"0"`) e
  porta frontal `(0, 0, -4)` drena `1000` mB para o monitor (`"1000"`); 6 lingotes de urânio no slot 9 irradiam `2.0 RAD/s` na
  chaminé `(0, 9, 3)` com `0.0 RAD/s` no núcleo, e `sas3` pela porta `(1, 0, -3)` irradia `5020.6 RAD/s` no núcleo; após salvar no
  meio de 4 minérios de ouro em `REQUEST` com `lava` no tanque e 20 granitos destruídos (`capture-state`) e reabrir (`check-state`),
  tanque, monitor e fila continuam sem perda nem duplicação (`gold_ore` termina em `"4"`);
- cortes de save no modo estrito (`HORIZON_NO_EAGER_SAVE=1`): `annihilator-state` 7/7 (`none`, `kill`, `kv_committed`, `save_begin`,
  `chunks_written`, `entities_written`, `players_saved`: 10 pedregulhos destruídos + 16 soltos de um baú trancado, somando sempre
  26 entre `monitor + chest + hopper + input` nos dois reabres, com consulta fresca a `Pools` ao alternar o slot 8) e
  `annihilator-break` 7/7 (`none`, `kill`, `kv_committed`, `save_begin`, `live_chunk:8,0`, `chunks_written`, `entities_written`:
  quebrar o aniquilador com 6 terras + 1 identificador contra o controle de baú vanilla, igual item a item nos dois reabres e
  armazenamento limpo);
- mutantes: 43 escolhidos à mão (`mutate-annihilator.py`), 43 mortos (os 3 últimos mortos em `898235b3` reforçando `prev_cmp` e
  `fluid_server` para células não-porta).

### Digama: a infraestrutura do modo DRX de `machine_fel` (grupo 2a)

O cristal `ntm:laser_crystal_digamma` põe o FEL no modo DRX. No Java (`TileEntityFEL:134,187-190`) ele faz duas coisas: dá 0,1 de digama a cada tick a quem estiver no feixe e transforma o bloco queimado em fogo de digama, ou em matéria de digama quando o `polaroidID` é 11, com cinza de digama logo abaixo. Até aqui o port não tinha nenhuma dessas peças. O passo do FEL ficava congelado no DRX.

**Contrato portado**

- **Dose no jogador** (`HbmLivingProps.digamma`, `:130-199`): um `float` de 0 a 10, somado em `float`. Fica no anexo do próprio jogador (`ntm:item_state`, chave `digamma`, ao lado da dose de radiação) e é salvo com ele. O corpo novo, depois de uma morte, começa em 0, como o `HbmLivingProps` novo do Java (`copy_on_death: false`; o motor também só leva os valores-base dos atributos).
- **Vida máxima** (`setDigamma:144-156`): o modificador `ntm:digamma`, `0,5^d − 1` com a operação 2 (`add_multiplied_total`), fica entre os modificadores permanentes do jogador, e a vida é limitada ao novo máximo. Um corpo que entra no mundo (ao entrar ou renascer) recebe de volta o modificador do anexo.
- **Morte em 10** (`setDigamma:158`): 500 de dano `ntm:digamma`, que é absoluto, atravessa armadura e vale no criativo (`ModDamageSource.digamma`; tags `bypasses_armor`, `bypasses_effects` e `bypasses_invulnerability`). A mensagem de morte é a do Java (`death.attack.digamma`).
- **Bloqueios** (`applyDigammaData:108-129`, `contaminate:265-300`): nada no criativo, nada nos primeiros 200 ticks, nada sob Stability (`ntm:stability`), nada com o conjunto completo FAU ou DNS (`checkForDigamma`). O `checkForDigamma2` (robes) exige Stability e por isso nunca libera sozinho.
- **Gravação transacional**: dentro do passo de uma máquina, a dose fica guardada como a de radiação (`Deferred::digammas`) e só chega ao jogador depois do commit, em ordem, conferindo o valor anterior. Um passo recusado não grava nada.
- **`ntm:fire_digamma`** (`DigammaFlame`): modelo em cruz com textura animada, luz 15, dureza 0, resistência 150, sem colisão mas com a célula inteira para mirar, sem drop e fora da aba criativa. Usa o modelo de vidro, sem colisão e em cruz: um modelo com regra de sobrevivência própria (planta, fogo) imporia a regra dele. Quando um bloco ao lado muda, o fogo some se o bloco de baixo não tiver topo sólido (`onNeighborBlockChange`), lido como cubo opaco. Isso roda no glue: a troca é anotada em `block_changed` e o fogo é removido no fim do tick. Na primeira versão usei o `mayPlaceOn` do modelo de arbusto morto, mas a prova em jogo mostrou que as trocas publicadas pelo mod não passam por ele, e o fogo morria sobre a cinza. Cada célula dele que a caixa do jogador toca (`doBlockCollisions`, a caixa 0,6 × 1,8 recolhida em 0,001) dá 0,05 por tick.
- **`ntm:digamma_matter`** (`DigammaMatter`): os metadados 0..7 viram um bloco cada (`digamma_matter`, `digamma_matter__<n>`). Cada um é inquebrável, tem resistência 18 000 000 e não tem colisão. Ao ser colocado agenda o próprio tick para daqui a 10 a 49 ticks. No tick ele some e, abaixo do metadado 7, coloca o metadado seguinte nas 18 células a até dois passos que ainda não têm matéria (`setBlock` com flag 3, seja qual for o bloco que estava ali).
- **`polaroidID`** (`MainRegistry:250-256`): sorteado quando o servidor inicia, de 1 a 18 sem o 4 e o 9; `NTM_POLAROID` faz o papel do `generalOverride` do config. Com 11, o DRX deixa matéria em vez de fogo.
- **DRX no FEL**: o feixe deixa o bloco de fogo, ou de matéria, com cinza embaixo, e dá 0,1 aos jogadores no feixe. O passo do FEL não congela mais no DRX.
- **Diagnóstico de digama**: `ntm:digamma_diagnostic` passa a imprimir a dose guardada, truncada ao centésimo com o produto em `float` (`printDiagnosticData:235`). Antes imprimia 0.

**Lacunas documentadas**

- Só jogadores recebem digama. As outras entidades vivas do Java (`EntityLivingBase` no feixe e no fogo) dependem da API de entidades do port.
- As armaduras FAU, DNS e robes não estão no catálogo: a imunidade está implementada por id, mas ninguém consegue vesti-las.
- As conquistas `digammaSee`, `digammaFeel` e `digammaKnow` não existem: o port não tem conquistas.
- O suor de soul sand (`handleDigamma:343-364`, partículas) é só visual e não foi portado; `digamma::sweat_chance` guarda a cadência.
- O topo sólido é lido como cubo opaco (a regra geral do `isSideSolid`). Os casos especiais do 1.7.10 ficam de fora: laje superior, escada invertida, funil e neve cheia.
- Os limites aleatórios de seleção da matéria (`setBlockBoundsBasedOnState`, só aparência) são a célula inteira. Um vizinho em chunk não carregado não recebe matéria; no Java, o `setBlock` carregaria o chunk.
- As pílulas que mexem no digama (`xanax`, `fmn`, `five_htp`; K2) ainda fazem só a parte visível, embora o armazenamento agora exista.

**Provas** (refeitas no binário final: cliente `67817408dd75cbc96bb7a7efbd318415b48fdcf356b281c4b5fcb2ff2a8a72cf`, pacote `eb9f78039702e982af956ff534ab3c8bd2a63a8f7e95373455ee2f9f69ec72ad`, HEAD `e40f33a9`; evidência em `content-gen-evidence/onda2-s2/`)

- Testes de unidade: suíte do `ntm-core` com 858 passando, 0 falhas; clippy `--workspace --all-targets -D warnings` limpo; gerador de conteúdo com 346 testes (entre eles `test_digamma_blocks` e `test_damage_types`).
- Em jogo (`run-native-reactors.py digamma`): 20/20, sem avisos do backend (inclui a troca de um vizinho que mantém o fogo sobre pedra):
  - o fogo fica sobre pedra, some sem chão e não fica sobre vidro;
  - no criativo não há dose; em sobrevivência o fogo dá dose (1,94 DRX). O modificador é 0,5^d − 1 e a vida máxima é 20 × (1 + m); a influência impressa é a do Java;
  - fora do fogo a dose não muda; sob Stability também não;
  - o xanax tira 0,5, o fmn limita a 2 (modificador −0,75) e o 5-HTP zera a dose e tira o modificador (vida máxima 20);
  - a matéria no metadado 6, observada com o tempo congelado, tica no 33º tick: deixa as 18 células no metadado 7, que somem nos 50 ticks seguintes;
  - a dose e o modificador continuam iguais depois de reabrir o mundo;
  - em 10 o jogador morre com a mensagem do Java (“stepped into the abyss”), e o corpo novo começa em 0, sem modificador.
- Cortes estritos (`HORIZON_NO_EAGER_SAVE=1`, `run-crash-more.py digamma-state`): 7/7. A dose reaberta é sempre a do último save do arquivo do jogador, com o próprio modificador:
  - `kill`, `save_begin`, `chunks_written` e `entities_written` reabriram na primeira dose (1,44);
  - `kv_committed`, `players_saved` e `none` reabriram na segunda (2,84 a 2,89).
- Mutantes: 44 escolhidos à mão (`mutate-digamma.py`: modelo, servidor, DRX do FEL, gravação transacional, comprimidos e diagnóstico), 44 mortos. Um mutante equivalente ficou de fora: somar em f64 e arredondar para `float` dá o mesmo que somar em `float`.

### `machine_silex` e a ligação com `machine_fel` (grupo 2a)

**Contrato portado** (`crates/ntm-core/src/silex.rs`, `TileEntitySILEX`)

- **Slots:** 11 (entrada; identificador; recipientes cheio e vazio; saída; fila de 6). **Tanque:** 16 000 mB, peróxido no início.
- **Tick** (`updateEntity:69-89`), nesta ordem:
  1. o identificador dá o tipo ao tanque (`setType(1, 1)`, o identificador fica);
  2. um recipiente enche o tanque (`loadTank(2, 3)`);
  3. `loadFluid` carrega a carga;
  4. `process` roda a receita, ou o progresso volta a 0;
  5. `dequeue` passa a saída para a fila;
  6. uma carga vazia esquece o item;
  7. o modo volta a NULL.
- **`loadFluid`:**
  - UF6, PUF6 e DEATH (`fluidConversion`), e qualquer fluido que tenha receita, carregam o próprio ícone, 50 por tick, desde que a carga seja desse ícone.
  - A cada 21 ticks (`loadDelay`), e só com o tanque de peróxido, o item da entrada carrega a sua `fluidProduced` se couber e houver peróxido. Isso só acontece com a carga vazia ou do mesmo item.
- **`process`:**
  - Precisa de laser de força ≥ à da receita, de carga ≥ `fluidConsumed` e da saída vazia.
  - Avança `2^(laser − exigido)` por tick; com 100 sai uma rodada.
  - A saída é sorteada pelo `recipeIndex`, que primeiro faz `%=` da soma dos pesos e depois soma 137 (`PRIME`).
- **`dequeue`:** põe uma unidade numa pilha igual da fila abaixo do limite; se não houver, a saída inteira vai para o primeiro slot vazio.
- **Receitas** (`data/machines/silex.json`, `tools/content-gen/silex_data.py`, conferidas contra o oráculo Java do RBMK):
  - 68 receitas.
  - A busca do `getOutput` em três camadas: tradução de item (4, como UF6 → lingote de urânio e o pó de lápis → `minecraft:lapis_lazuli`), tradução de dicionário (2: pó → lingote) e resíduo miúdo (26, um nono do fluido: `(fluidProduced / 900) * 100`, com o metadado preservado).
  - Os itens vanilla do 1.7.10 (`gold_nugget`, `flint`, `gravel`, `dye:4`) mapeados para os de hoje.
- **Automação** (`getAccessibleSlotsFromSide`, `isItemValidForSlot`, `canExtractItem`): um funil põe na entrada um item que tenha receita e tira da fila. **Menu** (`ContainerSILEX`): a mão põe na entrada, no identificador e no recipiente cheio, e tira de todos. As duas portas combo (inventário e fluido) ficam em (±1, 1, 0).
- **Estado fora dos slots** em `save_state`: carga, item (índice numa lista fixa ordenada), progresso, índice da receita, atraso da carga, tipo do tanque (id do fluido) e mB.

**Ligação com o FEL** (`TileEntityFEL:139-195`)

- Uma célula do feixe que é parte de um SILEX (`b == machine_silex`) procura o núcleo uma célula adiante, na altura do FEL. Sem núcleo ali, nada acontece e o feixe segue.
- Com núcleo, alinhado ao feixe (num sentido ou no outro), a pelo menos 5 células e sem outro SILEX já atendido nesse trecho: o SILEX recebe o comprimento de onda e o feixe segue.
  - O comprimento de onda vai por um mapa transitório em `Ready`, que o passo do SILEX consome antes do seu tick.
  - Se o passo do FEL for recusado, o mapa volta atrás.
  - A ordem entre os dois passos dá no máximo um tick de atraso, como a ordem das tile entities no Java.
- Caso contrário o SILEX quebra (`breakBlock`): todas as suas partes são removidas, os itens caem pelo caminho normal de remoção de máquina, e cai um `machine_silex` na célula atingida.
- Uma parte de multibloco de outra máquina é um obstáculo comum, com a resistência da parte. O passo do FEL não congela mais nela.
- O FEL passa a `Kind::Fel` no registro e roda no mundo.

**Lacunas documentadas**

- As 227 receitas de estados 1..9 das pastilhas RBMK esperam o codec de pastilha (T03/T14; decisão do Maestro).
- O nome e a onda do laser na tela não são desenhados: o modo aparece na dica da carga.
- A parte de multibloco de outra máquina atingida pelo feixe usa a resistência da parte genérica (`ntm:dummy_machine`), não a do bloco da máquina.
- O `mode` salvo no NBT do Java não é guardado (ele volta a NULL a cada tick do SILEX).


**Provas** (cliente `67817408dd75cbc96bb7a7efbd318415b48fdcf356b281c4b5fcb2ff2a8a72cf`, pacote `eb9f78039702e982af956ff534ab3c8bd2a63a8f7e95373455ee2f9f69ec72ad`, HEAD `e40f33a9`; evidência em `content-gen-evidence/onda2-s2/{fel,silex-state}`)

- Testes de unidade: `ntm-core` com 858 passando, 0 falhas; clippy `--workspace --all-targets -D warnings` limpo; gerador de conteúdo com 346 testes, entre eles `test_silex_data` (4) contra o oráculo Java.
- Em jogo (`run-native-reactors.py fel`, com `NTM_POLAROID=1`): 14/14, sem avisos do backend.
  - **Montagem e feixe:** o SILEX fica sobre a linha do feixe, ao longo dele; o FEL, com bateria criativa e cristal de bismuto, liga no visível, acha o SILEX válido e o feixe segue até 24.
  - **SILEX no visível:** os 16 tanques de peróxido o enchem; os dois lingotes carregam 900 cada (tanque com 14 200); o laser roda 3 rodadas em pepitas (1 U-235, 2 U-238), com o índice 147 igual ao simulado.
  - **DRX:** o SILEX roda 8 vezes mais rápido (4 rodadas em 40 ticks); a pedra adiante vira fogo de digama sobre cinza de digama; um jogador em sobrevivência no feixe recebe 2,0 DRX em cerca de 20 ticks.
  - **Persistência:** sem o cristal, o SILEX parado e o FEL ligado reabrem exatamente como salvos; com o cristal de volta, o SILEX continua.
  - **SILEX mal alinhado:** um segundo SILEX perpendicular, 14 células adiante, é derrubado inteiro, solta o lingote que tinha e um `machine_silex`; o alinhado fica intacto.
- Cortes estritos (`HORIZON_NO_EAGER_SAVE=1`, `run-crash-more.py silex-state`): 7/7.
  - Sem o cristal antes de cada save, o SILEX fica parado, então a reabertura tem que ser exatamente o save-base ou o save do passo.
  - O peróxido fecha (tanque + tanques cheios + 900 por lingote carregado = 16 000), e a carga mais 100 por pepita é igual a 900 por lingote carregado.
  - `kill` deixa exatamente o save-base.
- Mutantes: 43 escolhidos à mão (`mutate-silex.py`: o SILEX e o acoplamento no FEL), 41 mortos e 2 equivalentes nos dados.
  - Na primeira rodada, `+= 1` no lugar de `+= 137` sobreviveu; foi morto com o teste do índice guardado e dos três pesos do plutônio.
  - `produced / 9` no lugar de `(produced / 900) * 100` só difere para um resíduo inteiro que não produza múltiplo de 900, e os 26 produzem 900.
  - Sem o ramo `CONVERSION`, UF6, PUF6 e DEATH carregam do mesmo jeito: todos têm receita direta ou por tradução.
- O polaroid 11 (matéria em vez de fogo) não tem prova em jogo: a matéria de metadado 0 se espalharia por cerca de 14 células e alcançaria o SILEX da montagem. A escolha está coberta em teste de unidade, e o espalhamento da matéria tem prova própria na prova `digamma`.
