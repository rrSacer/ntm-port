# Catálogo K3: portas, chaves e máquinas restantes

Em andamento, base `a27f03d8`. O levantamento anterior era heurístico: "Lock" capturava também "Block", portanto seus 68/13/75 não são quantidades de aceitação. A triagem por classe/registro entrega a lista abaixo. Colecionáveis com enums próprios, baterias/tanques/conversores (K4), rádio/radar/satélites (K5), reatores legados, mísseis e fusão/ICF não entram neste lote.

O gerador usa `bulk.declarations` e o catálogo m3, com fronteira explícita em `catalog-k3.json`. Nada de processamento, inventário, energia, lógica de chaves ou multiblocos é inferido de uma classe. Máquinas com tile usam beehive/facing/honey0..5 e auto:false; o registro Inert e o machine-gen ficam com a frente nativa no push conjunto. Blocos sem tile não recebem registro.

As quatro portas simples tentam oak_door, herdando estados/duas metades/abertura como adaptação autorizada, porque `BlockModDoor.onBlockActivated` permite clique manual. Prova mínima em jogo confirmou esse contrato nas quatro portas: 68 conferências, duas metades por item e open=true após clique, detector limpo. O nome original foi preservado. Itens ModItems.door_* resolvem para o item do mesmo bloco; o resolvedor mantém o tipo explícito nos demais casos. Receitas e texturas originais upper/lower são preservadas, com drop apenas na metade inferior. Sons, resistência e transformação de metadados do NTM não se tornam automaticamente iguais aos do vanilla.

Chaves/cadeados são itens inertes. `ItemKeyPin` usa int NBT `pins`; não há código de pareamento/segurança no conteúdo. Chaves ocultas no fonte continuam ocultas. `key_kit` e `pin` já existiam e foram reaproveitados. Sem durabilidade vanilla inventada.

| ID | Classe | Contrato |
| --- | --- | --- |
| `stone_keyhole` | `BlockKeyhole` | Bloco simples inerte |
| `stone_keyhole_meta` | `BlockRedBrickKeyhole` | Bloco simples inerte |
| `brick_forgotten_lock` | `BlockForgottenLock` | Bloco simples inerte |
| `trapdoor_steel` | `BlockNTMTrapdoor` | Bloco simples inerte |
| `seal_controller` | `BlockSeal` | Bloco simples inerte |
| `seal_hatch` | `BlockHatch` | Inert após registro |
| `vault_door` | `BlockDoorGeneric` | Inert após registro |
| `blast_door` | `BlastDoor` | Inert após registro |
| `sliding_blast_door` | `BlockDoorGeneric` | Inert após registro |
| `fire_door` | `BlockDoorGeneric` | Inert após registro |
| `transition_seal` | `BlockDoorGeneric` | Inert após registro |
| `silo_hatch` | `BlockDoorGeneric` | Inert após registro |
| `silo_hatch_large` | `BlockDoorGeneric` | Inert após registro |
| `secure_access_door` | `BlockDoorGeneric` | Inert após registro |
| `large_vehicle_door` | `BlockDoorGeneric` | Inert após registro |
| `qe_containment` | `BlockDoorGeneric` | Inert após registro |
| `qe_sliding_door` | `BlockDoorGeneric` | Inert após registro |
| `round_airlock_door` | `BlockDoorGeneric` | Inert após registro |
| `sliding_seal_door` | `BlockDoorGeneric` | Inert após registro |
| `water_door` | `BlockDoorGeneric` | Inert após registro |
| `cargo_door` | `BlockDoorGeneric` | Inert após registro |
| `door_metal` | `BlockModDoor` | Porta vanilla em prova |
| `door_office` | `BlockModDoor` | Porta vanilla em prova |
| `door_bunker` | `BlockModDoor` | Porta vanilla em prova |
| `door_red` | `BlockModDoor` | Porta vanilla em prova |
| `event_tester` | `TestEventTester` | Inert após registro |
| `obj_tester` | `TestObjTester` | Inert após registro |
| `deco_emitter` | `BlockEmitter` | Inert após registro |
| `part_emitter` | `PartEmitter` | Inert após registro |
| `filing_cabinet` | `BlockDecoContainer` | Inert após registro |
| `machine_ashpit` | `MachineAshpit` | Inert após registro |
| `furnace_iron` | `FurnaceIron` | Inert após registro |
| `furnace_steel` | `FurnaceSteel` | Inert após registro |
| `furnace_combination` | `FurnaceCombination` | Inert após registro |
| `machine_sawmill` | `MachineSawmill` | Inert após registro |
| `machine_strand_caster` | `MachineStrandCaster` | Inert após registro |
| `machine_fel` | `MachineFEL` | Inert após registro |
| `machine_silex` | `MachineSILEX` | Inert após registro |
| `machine_rotary_furnace` | `MachineRotaryFurnace` | Inert após registro |
| `machine_furnace_brick_off` | `MachineBrickFurnace` | Inert após registro |
| `machine_furnace_brick_on` | `MachineBrickFurnace` | Inert após registro |
| `machine_industrial_generator` | `MachineIGenerator` | Inert após registro |
| `machine_cyclotron` | `MachineCyclotron` | Inert após registro |
| `machine_exposure_chamber` | `MachineExposureChamber` | Inert após registro |
| `machine_electric_furnace_on` | `MachineElectricFurnace` | Inert após registro |
| `machine_microwave` | `MachineMicrowave` | Inert após registro |
| `machine_teleporter` | `MachineTeleporter` | Inert após registro |
| `teleanchor` | `MachineTeleanchor` | Bloco simples inerte |
| `field_disturber` | `MachineFieldDisturber` | Bloco simples inerte |
| `machine_radiolysis` | `MachineRadiolysis` | Inert após registro |
| `machine_drain` | `MachineDrain` | Inert após registro |
| `fan` | `MachineFan` | Inert após registro |
| `machine_keyforge` | `MachineKeyForge` | Inert após registro |
| `machine_armor_table` | `BlockArmorTable` | Bloco simples inerte |
| `machine_weapon_table` | `BlockWeaponTable` | Bloco simples inerte |
| `tesla` | `MachineTesla` | Inert após registro |
| `chimney_brick` | `MachineChimneyBrick` | Inert após registro |
| `chimney_industrial` | `MachineChimneyIndustrial` | Inert após registro |
| `machine_autosaw` | `MachineAutosaw` | Inert após registro |
| `machine_thresher` | `MachineThresher` | Inert após registro |
| `machine_annihilator` | `MachineAnnihilator` | Inert após registro |
| `machine_rockmill` | `MachineRockMill` | Inert após registro |
| `machine_mining_laser` | `MachineMiningLaser` | Inert após registro |
| `machine_assembly_factory` | `MachineAssemblyFactory` | Inert após registro |
| `machine_precass` | `MachinePrecAss` | Inert após registro |
| `machine_arc_welder` | `MachineArcWelder` | Inert após registro |
| `machine_soldering_station` | `MachineSolderingStation` | Inert após registro |
| `machine_chemical_factory` | `MachineChemicalFactory` | Inert após registro |
| `machine_supercomputer` | `MachineSuperComputer` | Inert após registro |
| `machine_orbus` | `MachineOrbus` | Inert após registro |
| `machine_lpw2` | `MachineLPW2` | Inert após registro |
| `press_preheater` | `BlockBase` | Bloco simples inerte |
| `machine_ammo_press` | `MachineAmmoPress` | Inert após registro |
| `machine_boiler_off` | `MachineBoiler` | Bloco simples inerte |
| `machine_tower_small` | `MachineTowerSmall` | Inert após registro |
| `machine_tower_large` | `MachineTowerLarge` | Inert após registro |
| `machine_deuterium_extractor` | `MachineDeuteriumExtractor` | Inert após registro |
| `machine_intake` | `MachineIntake` | Inert após registro |
| `machine_compressor` | `MachineCompressor` | Inert após registro |
| `machine_compressor_compact` | `MachineCompressorCompact` | Inert após registro |
| `machine_autocrafter` | `MachineAutocrafter` | Inert após registro |
| `machine_funnel` | `MachineFunnel` | Inert após registro |
| `machine_siren` | `MachineSiren` | Inert após registro |
| `machine_forcefield` | `MachineForceField` | Inert após registro |
| `decon` | `BlockDecon` | Inert após registro |
| `key` | `ItemKey` | Item inerte |
| `key_red` | `ItemCustomLore` | Item inerte |
| `key_red_cracked` | `ItemCustomLore` | Item inerte |
| `key_fake` | `ItemKey` | Item inerte |
| `padlock_rusty` | `ItemLock` | Item inerte |
| `padlock` | `ItemLock` | Item inerte |
| `padlock_reinforced` | `ItemLock` | Item inerte |
| `padlock_unbreakable` | `ItemLock` | Item inerte |
| `mech_key` | `ItemCustomLore` | Item inerte |

Texturas: usa o ícone declarado, uma face literal registrada pela classe ou o casing original; os casos sem ícone resolvido usam block_steel explicitamente como fallback: `machine_ashpit`, `machine_industrial_generator`. Modelos 3D, animações, volumes, inventários, efeitos e danos continuam lacunas.

Durante a leitura de textura, deuterium_extractor_side.png revelou bytes depois do chunk IEND. O decoder PNG agora termina no IEND conforme a estrutura da imagem; teste verifica que bytes finais não alteram o resultado. A imagem-fonte permanece intacta.

Fora do escopo por decisão do coordenador: `machine_difurnace_off`, `machine_difurnace_on`, `machine_difurnace_extension`, `machine_difurnace_rtg_off`, `machine_difurnace_rtg_on`, `machine_rtg_furnace_off`, `machine_rtg_furnace_on`, `machine_large_turbine`. Todos estão @Deprecated no fonte; mantida a ausência da turbina legada nos testes, como no R5.

## Contagens e validação

O lote final acrescenta 94 identidades (85 blocos e nove itens), levando o catálogo a 3.285 IDs (678 blocos e 2.607 itens). 70 blocos precisam de registro Inert; os outros 15 são simples, incluindo quatro portas vanilla. Nenhuma identidade antiga nem seus campos foram alterados.

Receitas originais resolvidas: montadora 469→500 e bigorna 143→160 construções. O total vanilla chega a 1.184 receitas. As tabelas Rust e o teste Rust da bigorna são gerados/ajustados pelo 870f na integração, sem edição desta frente. Hazards: +94 entradas sem registro fonte resolvido/taxa zero; nenhuma taxa nem hazard antigo mudou e radiation_buckets ficou intacto.

A regeneração content-gen+visual-gen em duas passadas terminou sem diferenças. 213 testes passaram em cada seed 1/42 e ambos os validadores passaram. O pacote passou no check do upstream e no do fork. Texturas e grafos de item/tag/receita foram verificados contra o recurso vanilla 26.3. O teste antigo da turbina Deprecated permaneceu intacto; o teste do gerador industrial agora impede lógica sintética, permitindo seu casing Inert.

Pacote candidato: 8.971.323 bytes, SHA-256 `c3ec8c12de8afb7c73c4dbcd2c8164376671ad0e9c4cc8c44a887678861de27a`. Evidências locais em `content-gen-evidence/catalog-k3/`; a prova completa do candidato passou: 94 IDs, 340 conferências, quatro portas colocadas pelo item e abertas por clique, sem rejeição/panic. O mundo descartável foi removido e nenhuma instalação real recebeu o pacote. A criação de block entity para os novos Inert depende do registro/bundle posterior; esta etapa não afirma simulação de máquinas.

O único alerta de whitespace no diff padrão vem do arquivo press_preheater.png.mcmeta, copiado byte a byte do fonte com finais CRLF. O check com cr-at-eol passa; não se alterou a animação para normalizar seu arquivo.

A exportação isolada do conteúdo preparado reproduziu 18.354 arquivos e confirmou os 18.270 assets/data do pacote byte a byte. O único acréscimo após a árvore verificada é a documentação dos resultados finais.
