# Levantamento de conteúdo: geração de energia

Leitura do NTM 1.7.10 e do catálogo após `0cedfd3`, sem gerar blocos, receitas ou dados. Reatores RBMK, ZIRNOX, PWR, Watz e fusão ficam para outra frente. A ordem abaixo foi proposta pelo agente de lógica; depende da confirmação do coordenador.

## Máquinas e fontes das tabelas

Os caminhos de Java abaixo são relativos a `ntm-src/src/main/java/com/hbm/`. Não existe uma classe única de receitas de geração: grande parte das regras vive nos tiles e nos traits de fluidos.

| Lote proposto | Conteúdo ausente | Fonte e mecanismo a extrair |
|---|---|---|
| 1. Aquecedores | `heater_oven`, `heater_oilburner`, `heater_electric`, `heater_heatex` | `tileentity/machine/TileEntityHeater{Oven,Oilburner,Electric,Heatex}.java`. Oven queima sólidos e recebe/transfere calor; oilburner usa **FT_Flammable**; electric converte HE em TU conforme setting; heatex usa **FT_Coolable/HEATEXCHANGER**, não FT_Heatable. |
| 2. Ciclo de vapor | `machine_steam_engine`, `machine_industrial_turbine`, `machine_chungus`, `machine_condenser`, `machine_condenser_powered` | `TileEntitySteamEngine`, `TileEntityTurbineBase`, `TileEntityMachineIndustrialTurbine`, `TileEntityChungus`, `TileEntityCondenser{,Powered}`. Turbinas usam **FT_Coolable/TURBINE**; condensadores convertem spentsteam→water. |
| 3. Combustão | `machine_diesel`, `machine_combustion_engine`, `machine_turbofan`, `machine_turbinegas` | Tiles homônimos; **FT_Combustible** com FuelGrade. Diesel tem eficiências por categoria; combustion exige piston_set e sua matriz; turbofan exige AERO; turbinegas exige GAS e tem partida/RPM/lubrificante/água/vapor. |
| 4. Alternativas | `machine_solar_boiler`, `solar_mirror`, `machine_stirling`, `machine_stirling_steel`, `machine_stirling_creative` | `TileEntitySolarBoiler`, `TileEntitySolarMirror`, `TileEntityStirling`. Espelhos e céu alimentam calor solar; Stirling recebe calor de IHeatSource. |
| 5. Radioisótopos | **`machine_rtg_grey`**, `machine_radgen` | `TileEntityMachineRTG`, `util/RTGUtil`, `items/machine/ItemRTGPellet`; `TileEntityMachineRadGen.fuels`. O campo ModBlocks.machine_rtg tem registro efetivo **machine_rtg_grey** (`blocks/ModBlocks.java:1855,3086`). |
| 6. Infraestrutura | `machine_battery_socket`, `machine_battery_redd`; battery_pack; cabos/conectores/pylons abaixo | `tileentity/machine/storage/TileEntityBattery{Base,Socket,REDD}`, `items/machine/ItemBatteryPack`. Capacidade, carga/descarga, prioridade e regras de rede são dados/algoritmos próprios. REDD usa BigInteger. |

Já existem `machine_wood_burner`, `machine_battery`, `red_cable`, `heater_firebox`, `machine_boiler`, `machine_industrial_boiler`, `machine_transformer`, `machine_hephaestus` e `machine_flare`. Existir no conteúdo não implica lógica funcional: vários foram registrados como Inert durante os lotes anteriores.

`machine_large_turbine` tem tile funcional antigo, mas o campo está marcado Deprecated (`ModBlocks.java:951`); o agente de lógica propõe deixá-lo fora. `machine_industrial_generator` é registrado, porém **TileEntityMachineIGenerator.updateEntity() é vazio** (`:23`): não há algoritmo de geração para transcrever. Se entrar, deve ser Inert com essa nota, sem inventar combustível/produção. Radiolysis também produz HE a partir de pellets (`TileEntityMachineRadiolysis.java:116`), mas é uma máquina de processamento/irradiação adicional; não é necessária para o primeiro ciclo elétrico. Conversores RF/HE e receptor de energia do DFC são integração/outro sistema, não fontes independentes desta frente.

## Combustíveis, calor e fluidos

**Nenhum tipo de fluido falta:** as 157 declarações de Fluids.java estão em `crates/ntm-core/data/fluids.json` (incluindo NONE). O que falta são propriedades energéticas completas, não novos IDs. O arquivo atual conserva identificação/flags/temperatura, mas não a tabela final de combustão/resfriamento.

O agente de lógica propôs **`fluid_traits.json` separado**, com fonte por linha, em vez de ampliar fluids.json. Ainda é contrato proposto; nenhum arquivo foi criado. Conteúdo necessário:

- **FT_Combustible:** 43 fluidos com trait, categorias HIGH 16, LOW 6, MEDIUM 12, AERO 2, GAS 7. Extrair HE/balde e FuelGrade, aplicando as atualizações de `registerCalculatedFuel` na ordem do Java. `Fluids.java:935–946` arredonda flammable e combustible separadamente; não se deve usar max(TU,HE) como combustível de um motor. `pyrooven_recipes.fuel_heat` já resolve expressões/overrides, mas sua opção include_combustion retorna um máximo e perde a categoria: precisa de extração própria para motores.
- **FT_Flammable:** reutilizar os 60 traits finais já conservados em `machine-recipes/flare.json`, para oilburner. TU e HE são unidades distintas.
- **FT_Coolable:** 16 declarações (`Fluids.java:637–677`), com coolsTo, amountReq, amountProduced, heatEnergy e eficiências TURBINE/HEATEXCHANGER. As quatro linhas de vapor são steam 100→spentsteam 1/200 TU; hotsteam 1→steam 10/2 TU; superhotsteam 1→hotsteam 10/18 TU; ultrahotsteam 1→superhotsteam 10/120 TU. Eficiências fonte 1,0 em turbina e 0,5 em heatex para vapor. Outros coolants têm as próprias eficiências.
- **FT_Heatable:** as 18 heating_steps já estão em boiler.json, inclusive elegibilidade BOILER/HEATEXCHANGER. Não substituem a tabela Coolable do aquecedor heatex.
- **Combustíveis sólidos:** FuelHandler/vanilla e modificadores de calor de ModuleBurnTime; firebox/foundry já têm extração. Não aplicar a filtragem restrita do alto-forno a todos os aquecedores.
- **RTG:** calor, decaimento, material depleted, meia-vida e flags VersatileConfig.rtgDecay/scaleRTGPower. ModItems.java:2647–2656 define os pellets; RTGUtil atualiza inventário e decaimento. Radgen tem sua própria tabela discreta em TileEntityMachineRadGen.java:235–253.

Exemplos que delimitam o contrato: diesel consome 1 mB/t e usa eficiência MEDIUM 0,5/HIGH 0,75/AERO 0,1; combustion usa décimos de mB e a matriz dos pistões. Steam engine tem eficiência 0,85; industrial 1,0 com volante/inércia; chungus 0,85. Condensador simples é 1:1 sem HE; powered cobra 10 HE/mB com tolerância na condição inicial. Electric heater produz setting×100 TU, cobrando floor(setting^1,4×200) HE. RTG gera heat×5 HE e possui teto de calor dependente da configuração. São regras algorítmicas, não receitas generic com power/duration fabricados.

## Itens e infraestrutura ausentes

- **4 piston_set:** STEEL, DURA, DESH, STARMETAL (`ItemPistons.java:55–59`). Nenhum está no catálogo. Também falta `ingot_starmetal`, exigido pelo pistão STARMETAL e pela construção do radgen; não inventar esse ID sem importar o material original.
- **2 gear_large**, metas 0/1, faltam e bloqueiam as construções de Stirling/steam_engine na bigorna. Bobinas, flywheel_beryllium, turbine_titanium/tungsten e reactor_core já existem.
- **10 pellets RTG vivos** (radium, weak, padrão, strontium, cobalt, actinium, americium, polonium, gold, lead) e **6 depleted** (bismuth, mercury, neptunium, lead, zirconium, nickel) estão ausentes. rtg_unit existe.
- **Resíduos do radgen:** oito variantes curtas e cinco longas, cada família em normal/tiny/vivo/depleted: **52 identidades ausentes**. A tabela energética usa os 26 vivos e gera o depleted correspondente. scrap_nuclear e gem_rad já existem.
- **12 battery_pack:** seis baterias redstone/lead/lithium/sodium/schrabidium/quantum e seis capacitores copper/gold/niobium/tantalum/bismuth/spark, todos ausentes. EnumBatteryPack (`ItemBatteryPack.java:32–68`) define descarga e deriva capacidade/carga; quantum tem duração própria. battery_creative existe; battery_spark e battery_trixite do catálogo são componentes comuns de bombas, **não** baterias IBatteryItem.
- Também ausentes cube_power, battery_sc, battery_potato e battery_potatos; são acessórios separados. Os blocos antigos machine_battery_potato/lithium/schrabidium/dineutronium e capacitores legados estão Deprecated; não substituí-los silenciosamente pela nova família de packs.
- Rede: faltam red_cable_classic/paintable/gauge/box, red_connector/super, red_pylon/steel/medium_wood/medium_wood_transformer/medium_steel/medium_steel_transformer/large, cable_switch/detector/diode. Somente red_cable existe dessa lista. cable_diode é Deprecated. Pintura, medidor, fios aéreos, colisão e orientação exigem lógica/modelo; importar cubos não os implementa.

## Construções e limites do levantamento

- **Montadora:** `AssemblyMachineRecipes.java:411–447`: dieselgen, combustiongen, quatro pistonsets, turbofan, gasturbine, iturbine, leviturbine, radgen e hpcondenser; RTG em :295; capacitores em :450–470. A expansão de EnumPistonType/EnumBatteryPack.ordinal precisa ser suportada pelo resolver.
- **Bigorna:** `inventory/recipes/anvil/AnvilRecipes.java:267–306` para aquecedores e :333–358 para Stirling/steel/steam_engine. Não adicionar receitas de bancada temporárias para essas construções.
- **Bancada:** `main/CraftingManager.java:257–260` para socket/capacitor copper, :292–293 para solar boiler/mirror e :844 para condenser. O mirror usa steel_beam já presente.
- **REDD:** construção em **PlasmaForgeRecipes.java:226**, não montadora: a forja de plasma ainda não foi portada. Deve permanecer como lacuna de progressão, sem receita improvisada.

Não foi feita ainda uma prova exaustiva de alcançabilidade de todos os ingredientes dessas construções. O levantamento identifica os bloqueios diretos e as tabelas; cada lote deve resolver as receitas originais e listar os demais skipped com motivo. Novas máquinas continuam exigindo coordenação do registro Inert e dados de hazards no mesmo push. Nenhum ID, recipe ou arquivo em crates/ foi alterado nesta etapa de pesquisa.
