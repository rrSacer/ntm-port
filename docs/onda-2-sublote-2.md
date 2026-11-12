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
