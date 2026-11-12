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
