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
| `machine_silex` | `TileEntitySILEX` (385) | `SILEXRecipes`, laser, peróxido (tanque 16 000) | multibloco; fluido; depende dos itens de cristal/laser do catálogo |
| `machine_cyclotron` | `TileEntityMachineCyclotron` (421) | `CyclotronRecipes`; água 32 000, vapor gasto 32 000, antimatéria 8 000; 100 M de energia | multibloco; fluido; consome/produz antimatéria |
| `machine_radiolysis` | `TileEntityMachineRadiolysis` (294) | `RadiolysisRecipes`; 3 tanques de 2 000; calor | multibloco |
| `machine_exposure_chamber` | `TileEntityMachineExposureChamber` (356) | `ExposureChamberRecipes`; 1 M de energia | multibloco; fonte de partículas |
| `machine_ammo_press` | `TileEntityMachineAmmoPress` (234) | `AmmoPressRecipes` | só vale se os itens de munição existirem no catálogo (armas são a Onda 4): verificar antes |
| `machine_microwave` | `TileEntityMicrowave` (293) | cozinha como o forno vanilla (`FurnaceRecipes`); 50 000 de energia; **explode** com o superaquecimento | tem efeito de mundo (explosão): espera D1+D2 para a falha |
| `machine_rockmill` | `TileEntityMachineRockMill` (282) | 2 tanques de 4 000, energia; mexe no mundo | confirmar o que faz no mundo |
| `machine_autocrafter` | `TileEntityMachineAutocrafter` (372) | grade de fabricação com modelo e filtro | lógica própria, sem tabela de receita do NTM |
| `machine_keyforge` | `TileEntityMachineKeyForge` (211) | 3 slots, grava chaves/trancas | liga com as portas e trancas já entregues (`K3 doors locks`) |
| `machine_ashpit` | `TileEntityAshpit` (224) | cinzeiro: recebe a cinza dos fornos e queimadores | depende da poluição/fuligem (Onda 1) para a parte do ar; a parte de itens é separável |
| `machine_supercomputer` | `TileEntityMachineSuperComputer` (199) | 2 tanques de 4 000; energia | multibloco; confirmar o efeito |
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
| `machine_fel` | `TileEntityFEL` (335) | laser (20 M); acerta entidades e blocos |
| `machine_autosaw` | `TileEntityMachineAutosaw` (605) | entidades e blocos |
| `machine_thresher` | `TileEntityMachineThresher` (460) | entidades, mundo, calor |
| `machine_mining_laser` | `TileEntityMachineMiningLaser` (670) | laser que minera; usa as receitas de centrífuga, cristalizador, forno e triturador |
| `machine_assembly_factory` | `TileEntityMachineAssemblyFactory` (766) | quatro montadoras em um multibloco: o mesmo problema do painel da fábrica química |

## Ordem proposta

1. **2a (livre de infraestrutura):** `machine_annihilator`, `machine_funnel`, `machine_orbus`, `machine_keyforge`, `machine_supercomputer`, depois as de receita simples: `machine_radiolysis`, `machine_exposure_chamber`, `machine_silex`, `machine_cyclotron`. São as que se provam como as soldadoras.
2. **2b (fluido e ar):** `machine_intake`, `machine_compressor` (e a compacta), `machine_tower_small/large`, `machine_deuterium_extractor`, `machine_industrial_generator`, `machine_lpw2`, `machine_rockmill`, `machine_autocrafter`.
3. **2c (depende dos efeitos de mundo, quando D1+D2 e D3 existirem):** `machine_sawmill`, `machine_microwave` (a falha), `machine_drain`, `machine_ashpit` e as chaminés (com a poluição).
4. **2d (depende de F1/D4 e do painel de múltiplas receitas):** `machine_rotary_furnace`, `machine_strand_caster`, `machine_chemical_factory`, `machine_assembly_factory`; a `machine_precass` espera `orbital_assembly`.
5. **2e (entidades):** `machine_teleporter`, `machine_forcefield`, `machine_siren`, `tesla`, `machine_fel`, `machine_autosaw`, `machine_thresher`, `machine_mining_laser`.

`machine_ammo_press` entra no 2a se os itens de munição já existirem; senão espera a Onda 4.

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
