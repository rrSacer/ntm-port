# Catálogo K3: portas, chaves e máquinas restantes

Integração retomada em 02/11/2026, sobre a base atualizada `6e3f3f1e`. O levantamento anterior era heurístico: "Lock" capturava também "Block", portanto seus 68/13/75 não são quantidades de aceitação. A triagem por classe/registro entrega a lista abaixo. Colecionáveis com enums próprios, baterias/tanques/conversores (K4), rádio/radar/satélites (K5), reatores legados, mísseis e fusão/ICF não entram neste lote.

O gerador usa `bulk.declarations` e o catálogo m3, com fronteira explícita em `catalog-k3.json`. Nada de processamento, inventário, energia, lógica de chaves ou multiblocos é inferido de uma classe. Máquinas com tile usam beehive/facing/honey0..5 e auto:false; os 70 blocos com tile agora têm registro Inert e tabelas regeneradas na mesma integração. Blocos sem tile não recebem registro.

As quatro portas simples tentam oak_door, herdando estados/duas metades/abertura como adaptação autorizada, porque `BlockModDoor.onBlockActivated` permite clique manual. Prova mínima em jogo confirmou esse contrato nas quatro portas: 68 conferências, duas metades por item e open=true após clique, detector limpo. O nome original foi preservado. Itens ModItems.door_* resolvem para o item do mesmo bloco; o resolvedor mantém o tipo explícito nos demais casos. Receitas e texturas originais upper/lower são preservadas, com drop apenas na metade inferior. Sons, resistência e transformação de metadados do NTM não se tornam automaticamente iguais aos do vanilla.

Chaves/cadeados são itens inertes. `ItemKeyPin` usa int NBT `pins`; não há código de pareamento/segurança no conteúdo. Chaves ocultas no fonte continuam ocultas. `key_kit` e `pin` já existiam e foram reaproveitados. Sem durabilidade vanilla inventada.

| ID | Classe | Contrato |
| --- | --- | --- |
| `stone_keyhole` | `BlockKeyhole` | Bloco simples inerte |
| `stone_keyhole_meta` | `BlockRedBrickKeyhole` | Bloco simples inerte |
| `brick_forgotten_lock` | `BlockForgottenLock` | Bloco simples inerte |
| `trapdoor_steel` | `BlockNTMTrapdoor` | Bloco simples inerte |
| `seal_controller` | `BlockSeal` | Bloco simples inerte |
| `seal_hatch` | `BlockHatch` | Inert (registrado) |
| `vault_door` | `BlockDoorGeneric` | Inert (registrado) |
| `blast_door` | `BlastDoor` | Inert (registrado) |
| `sliding_blast_door` | `BlockDoorGeneric` | Inert (registrado) |
| `fire_door` | `BlockDoorGeneric` | Inert (registrado) |
| `transition_seal` | `BlockDoorGeneric` | Inert (registrado) |
| `silo_hatch` | `BlockDoorGeneric` | Inert (registrado) |
| `silo_hatch_large` | `BlockDoorGeneric` | Inert (registrado) |
| `secure_access_door` | `BlockDoorGeneric` | Inert (registrado) |
| `large_vehicle_door` | `BlockDoorGeneric` | Inert (registrado) |
| `qe_containment` | `BlockDoorGeneric` | Inert (registrado) |
| `qe_sliding_door` | `BlockDoorGeneric` | Inert (registrado) |
| `round_airlock_door` | `BlockDoorGeneric` | Inert (registrado) |
| `sliding_seal_door` | `BlockDoorGeneric` | Inert (registrado) |
| `water_door` | `BlockDoorGeneric` | Inert (registrado) |
| `cargo_door` | `BlockDoorGeneric` | Inert (registrado) |
| `door_metal` | `BlockModDoor` | Porta vanilla em prova |
| `door_office` | `BlockModDoor` | Porta vanilla em prova |
| `door_bunker` | `BlockModDoor` | Porta vanilla em prova |
| `door_red` | `BlockModDoor` | Porta vanilla em prova |
| `event_tester` | `TestEventTester` | Inert (registrado) |
| `obj_tester` | `TestObjTester` | Inert (registrado) |
| `deco_emitter` | `BlockEmitter` | Inert (registrado) |
| `part_emitter` | `PartEmitter` | Inert (registrado) |
| `filing_cabinet` | `BlockDecoContainer` | Inert (registrado) |
| `machine_ashpit` | `MachineAshpit` | Inert (registrado) |
| `furnace_iron` | `FurnaceIron` | Inert (registrado) |
| `furnace_steel` | `FurnaceSteel` | Inert (registrado) |
| `furnace_combination` | `FurnaceCombination` | Inert (registrado) |
| `machine_sawmill` | `MachineSawmill` | Ativa na onda 2 (`Kind::Sawmill`; bloco único, sem o multibloco 3×3×2) |
| `machine_strand_caster` | `MachineStrandCaster` | Inert (registrado) |
| `machine_fel` | `MachineFEL` | Inert (registrado) |
| `machine_silex` | `MachineSILEX` | Inert (registrado) |
| `machine_rotary_furnace` | `MachineRotaryFurnace` | Inert (registrado) |
| `machine_furnace_brick_off` | `MachineBrickFurnace` | Inert (registrado) |
| `machine_furnace_brick_on` | `MachineBrickFurnace` | Inert (registrado) |
| `machine_industrial_generator` | `MachineIGenerator` | Inert (registrado) |
| `machine_cyclotron` | `MachineCyclotron` | Inert (registrado) |
| `machine_exposure_chamber` | `MachineExposureChamber` | Inert (registrado) |
| `machine_electric_furnace_on` | `MachineElectricFurnace` | Inert (registrado) |
| `machine_microwave` | `MachineMicrowave` | Inert (registrado) |
| `machine_teleporter` | `MachineTeleporter` | Inert (registrado) |
| `teleanchor` | `MachineTeleanchor` | Bloco simples inerte |
| `field_disturber` | `MachineFieldDisturber` | Bloco simples inerte |
| `machine_radiolysis` | `MachineRadiolysis` | Inert (registrado) |
| `machine_drain` | `MachineDrain` | Inert (registrado) |
| `fan` | `MachineFan` | Inert (registrado) |
| `machine_keyforge` | `MachineKeyForge` | Inert (registrado) |
| `machine_armor_table` | `BlockArmorTable` | Bloco simples inerte |
| `machine_weapon_table` | `BlockWeaponTable` | Bloco simples inerte |
| `tesla` | `MachineTesla` | Inert (registrado) |
| `chimney_brick` | `MachineChimneyBrick` | Inert (registrado) |
| `chimney_industrial` | `MachineChimneyIndustrial` | Inert (registrado) |
| `machine_autosaw` | `MachineAutosaw` | Inert (registrado) |
| `machine_thresher` | `MachineThresher` | Inert (registrado) |
| `machine_annihilator` | `MachineAnnihilator` | Inert (registrado) |
| `machine_rockmill` | `MachineRockMill` | Inert (registrado) |
| `machine_mining_laser` | `MachineMiningLaser` | Inert (registrado) |
| `machine_assembly_factory` | `MachineAssemblyFactory` | Inert (registrado) |
| `machine_precass` | `MachinePrecAss` | Inert (registrado) |
| `machine_arc_welder` | `MachineArcWelder` | Inert (registrado) |
| `machine_soldering_station` | `MachineSolderingStation` | Inert (registrado) |
| `machine_chemical_factory` | `MachineChemicalFactory` | Inert (registrado) |
| `machine_supercomputer` | `MachineSuperComputer` | Inert (registrado) |
| `machine_orbus` | `MachineOrbus` | Inert (registrado) |
| `machine_lpw2` | `MachineLPW2` | Inert (registrado) |
| `press_preheater` | `BlockBase` | Bloco simples inerte |
| `machine_ammo_press` | `MachineAmmoPress` | Inert (registrado) |
| `machine_boiler_off` | `MachineBoiler` | Bloco simples inerte |
| `machine_tower_small` | `MachineTowerSmall` | Inert (registrado) |
| `machine_tower_large` | `MachineTowerLarge` | Inert (registrado) |
| `machine_deuterium_extractor` | `MachineDeuteriumExtractor` | Inert (registrado) |
| `machine_intake` | `MachineIntake` | Inert (registrado) |
| `machine_compressor` | `MachineCompressor` | Inert (registrado) |
| `machine_compressor_compact` | `MachineCompressorCompact` | Inert (registrado) |
| `machine_autocrafter` | `MachineAutocrafter` | Inert (registrado) |
| `machine_funnel` | `MachineFunnel` | Inert (registrado) |
| `machine_siren` | `MachineSiren` | Inert (registrado) |
| `machine_forcefield` | `MachineForceField` | Inert (registrado) |
| `decon` | `BlockDecon` | Inert (registrado) |
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

## Aceitação integrada em 02/11/2026

No Horizon oficial `d928d1cd6` com as extensões locais no pin `3eb781ab5`, o probe nativo passou: 85 blocos colocados, 94 itens reconhecidos e quatro portas colocadas pelo item, com duas metades e abertura por clique. Foram 292 comandos/conferências, sem rejeição nem panic. Evidência: `content-gen-evidence/catalog-k3-native-current/game.json`.

As 70 identidades Inert estão registradas e as tabelas Rust agora incluem 500 receitas de montadora, 160 construções e 36 smithing da bigorna. Os modelos curados de PWR/Watz foram preservados. As máquinas K3 catalogadas continuam sem processamento ou segurança inventados.

O pacote desta integração também contém a ferramenta solar do lote de energia: 3.286 IDs no total, 9.093.273 bytes, SHA-256 `06b31d447623a7c88a2cc440f6866cd570d5d2d09b2d3a1cbb12395a0757cac8`. As contagens e o hash do candidato content-only acima são históricos.
