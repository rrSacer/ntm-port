# Bedrock etapas 3 e 4: conteúdo de processamento

Entrega estática de pirólise, eletrólise e forno elétrico a arco grande, mais as identidades comuns que bloqueavam receitas. O catálogo passa de **2.580 para 2.637 IDs**: 373 blocos, 2.264 itens e 40 máquinas. São 57 IDs novos: três máquinas, oito eletrodos novos/gastos, 23 sucatas plásticas, 16 corantes e sete materiais/intermediários. `concrete_rebar` já existia e foi preservado.

## Blocos e construção

| ID público | Identidade Java | Conteúdo |
|---|---|---|
| `ntm:machine_pyrooven` | MachinePyroOven | Forno de Pirólise / Pyrolysis Oven |
| `ntm:machine_electrolyser` | MachineElectrolyser | Máquina de Eletrólise / Electrolysis Machine |
| `ntm:machine_arc_furnace_large` | **ModBlocks.machine_arc_furnace**, MachineArcFurnaceLarge | Forno Elétrico a Arco / Electric Arc Furnace |

As três máquinas constam em `ids.machines`, usam beehive, facing e honey_level 0..5, loot próprio e textura original de carcaça de aço como cubo de fallback. O manifesto visual mantém precedência; OBJ, telas e multiblocos pertencem ao visual-gen. O agente de lógica registrou as três como **Inert**: esta entrega não habilita processamento nem pressupõe aprovação da lógica de cada etapa.

`ass.pyrooven` (300 ticks, 100 HE/t) e `ass.electrolyzer` (200 ticks, 100 HE/t) estão na montadora com os ingredientes originais. **`ass.arcfurnace` permanece em skipped**: requer `machine_transformer`, ausente. Não se criou ponte ou receita substituta. A montadora totaliza 416 receitas; também destravou `ass.explosivelenses2` pelo semtex.

Os eletrodos `ntm:arc_electrode__{graphite,lanthanium,desh,saturnite}` têm metadata 0..3, pilha 1 e limites de **10/100/500/1500 lotes**, extraídos de ItemArcElectrode. Cada um aponta para seu `arc_electrode_burnt__*`. O desgaste original usa NBT `durability` e ocorre uma vez por lote; aplicação e troca do item pertencem ao servidor. Não se usa durabilidade vanilla como substituto.

## Formato dos arquivos

Todos são schema 1, com origem, índice fonte, labels pt_br/en_us, `recipes` e `skipped` com motivo. `pyrooven.json` tem uma tabela; `electrolyser.json` separa `fluid` e `metal`; `arc_furnace.json` separa `solid` e `fluid`. Cada seção registra a seleção de receita feita pelo NTM.

- Itens: `inputItems:[{ids:[alternativas OreDict],count:N}]`, `outputItems:[[id,N]]`.
- Fluidos: `inputFluids/outputFluids:[[string_id,mB]]`.
- Metal fundido: **`outputMaterials:[[material,quanta]]`**, nomes internos do foundry, **72 quanta por lingote**. Quanta não são mB e não geram recipientes artificiais.
- `machine_config` conserva os valores básicos sem upgrades e uma referência tile:linha por valor.

| Tabela | Receitas exportadas | Omissões |
|---|---:|---:|
| Pirólise | 71 | 0 |
| Eletrólise de fluido | 8 | 0 |
| Eletrólise de metal | 36 | 0 |
| Arco sólido | 143 | 2 registros de lacuna |
| Arco fundido | 491 | 0 |

### Pirólise

As 71 instâncias incluem 27 `registerSFAuto` e 30 receitas bedrock. O calor vem dos valores **finais** calculados em Fluids.java, que sobrescrevem as declarações iniciais de FT_Flammable. O exportador aplica as expressões aritméticas na ordem fonte, a truncagem e o arredondamento do calor, depois o cálculo/arredondamento de mB de PyroOvenRecipes. Por exemplo, o calor final de kerosene é 2.560.000 TU/balde, e balefire exige 46 mB para um solid_fuel_bf.

Os setters `in`/`out` têm um campo de item e um de fluido; uma segunda atribuição do mesmo tipo sobrescreve a anterior. A receita de alcatrão chama primeiro `.out(HYDROGEN,250)` e depois `.out(CARBONDIOXIDE,1000)`: somente CO2 é produzido. `overwritten_source_fields` conserva esse achado. As cinco torrefações por tipo de bedrock produzem o estágio torrado e 50 mB de vitriol em 10 ticks.

Seleção: item + tipo de fluido, com cache da última combinação; sem cache, primeira receita correspondente na ordem da lista. Receita sem item requer slot vazio. Consumo básico 10.000 HE/t, buffer 10.000.000 HE e dois tanques de 24.000 mB.

### Eletrólise

Fluido é selecionado pelo tipo do tanque; metal por item exato, depois OreDict. As duas linhas podem trabalhar ao mesmo tempo, consumindo 10.000 HE/t **cada**. Buffer de 20.000.000 HE, quatro tanques de 16.000 mB e duas saídas fundidas de 10.368 quanta cada. Metal consome 100 mB de ácido nítrico por receita. Duração padrão de receita de fluido é 20 ticks, de metal 600; os overrides explícitos foram preservados.

As 36 linhas de metal incluem 18 de bedrock. O helper mantém no máximo dois produtos fundidos; material não fundível vira fragmento com o teto original de 64. Sem produto fundido, acrescenta dois lingotes de slag. Água 2.000 mB → hidrogênio 200 + oxigênio 200 leva 10 ticks; alumina usa 40 ticks. `outputFluidSlots` preserva posições NONE/0 sem introduzir um fluido de saída inexistente.

### Arco: precedência e limites do snapshot

Foram expandidas 85 chamadas explícitas (13 estáticas e 12 × seis tipos), mais autogen de material/forma, distribuições personalizadas e as receitas de fornalha NTM já resolvidas. A rejeição do Java é integral: se **qualquer** entrada colidir em um dos modos que o candidato declara, todo o candidato é recusado. Sete colisões estão registradas, com a receita anterior preservada. Seis receitas `fluidNull` não têm produto fundível e ficam em `empty_source_recipes`, sem fabricar uma saída.

Autogen usa conversões `convIn/convOut` e apenas destinos SMELTABLE. Distribuições customizadas conservam materiais fundíveis originais. As HashMaps fonte não declaram ordem; o snapshot ordena suas chaves para reprodução e documenta essa diferença. As colisões customizadas atuais são aliases de tório; não se promete reproduzir a ordem de HashMap de uma JVM específica.

As duas lacunas sólidas são: a linha de areia de quartzo, cujas identidades `sand_mix`/`glass_quartz` não estão integralmente no catálogo, e o universo dinâmico de `FurnaceRecipes` fora do snapshot resolvido. Este último é **um registro de lacuna, não uma contagem de receitas ausentes**. O exportador usa as receitas NTM de smelting resolvidas, filtradas pelo conjunto arcSmeltable; não inclui as pontes de blasting. Receitas vanilla legadas/external mods não disponíveis nesse snapshot continuam explicitamente fora. Isso impede confundir a tabela finita com uma execução integral do registro Java.

Sucata dinâmica também exige lógica: `scrap_default_materials` documenta 72 quanta para os itens padrão, mas o NTM lê a quantidade real de custom_data. O forno básico usa 400 ticks e 1.000 HE/t **por lote**, não por item: 20 slots de processamento, um item por slot sem upgrades, limitados pela capacidade fundida de 82.944 quanta. O descarte dos três eletrodos ocorre uma vez por lote.

## Fechamento dos buracos do catálogo

Os 23 `scrap_plastic__*` conservam ordinal de ScrapType, nome/ícone compartilhados e identidade distinta; são intermediários escondidos no NTM. Os **16** `chemical_dye__*` conservam ordinal de EnumChemDye, nomes originais e overlay colorido como no renderizador. As 18 receitas antes omitidas eram seis cores processadas com três óleos.

Entraram powder_cement, block_graphite, ingot_semtex, rebar, ingot_schraranium e meteorite_sword_treated/etched. **SRN é Schraranium**, confirmado em OreDictManager; não é estrôncio. As duas espadas são apenas identidades de inventário/processamento: habilidades e indestrutibilidade continuam no backlog, sem herdar comportamento de espada vanilla. Comida/uso especial de semtex e calor/fogo de grafite não são implementados por estes assets. Rebar usa template iron_bars e cubo JSON de fallback; a geometria NTM não foi portada nesta frente.

Comparação com o snapshot anterior a esta frente:

| Máquina | Skipped antes → depois | Receitas antes → depois |
|---|---:|---:|
| Cristalizador | **52 → 5** | 357 → 404 |
| Planta química | 30 → 25 | 47 → 52 |
| Montadora | 218 → 215 | 413 → 416 |
| PUREX | 37 → 36 | 22 → 23 |
| Triturador | 196 → 198 | 722 → 723 |
| Centrífuga | 25 → 25 | 54 → 54 |
| Alto-forno | 3 → 3 | 12 → 12 |
| Bigorna (duas seções) | 176 → 176 | 139 → 139 |
| Prensa | 1 → 1 | 47 → 47 |

No triturador, surgiram dois candidatos OreDict sem pó resolvível; por isso skipped aumentou apesar de haver uma receita nova. As cinco omissões restantes do cristalizador são pill_red, sand_mix e três guards OreDict vazios (Certus Quartz, fósforo branco e cinábrio). Nenhuma linha bedrock ficou de fora. Não se reivindica que as contagens de skipped entre máquinas sejam a mesma unidade: cada arquivo declara seu nível de expansão.

## Validação

Os **78 testes Python** passaram em PYTHONHASHSEED 1 e 42, incluindo oito testes das etapas 3/4 e quatro de fechamento do catálogo. Passaram validate-bulk e recipe-gen/validate: 2.637 IDs, 891 receitas vanilla do datapack, 97 faixas/27 bits de radiação e oito abas criativas. Foi necessário atualizar a expectativa do teste PUREX porque ingot_schraranium destravou uma receita original; o teste também verifica seus ingredientes e fluidos.

Hazards, radiation_buckets e furnace_recipes são saídas regeneradas do pipeline; o agente de lógica autorizou expressamente seu commit nesta frente. Nenhum arquivo Rust foi alterado. As três máquinas novas foram combinadas com ele para manter o registro Inert consistente.

Build/check API 0.2 passou em `/tmp/ntm-processing-content-2jhyhal9/target/horizon-mod/ntm.hmod`, **5.229.126 bytes**, SHA-256 `9f5876934ca86310042f2ab79174b96639e17c74436b8e78ea20c6fd1025b457`. O snapshot inclui as sobreposições visuais presentes na árvore; a propriedade desses arquivos continua com o visual-gen. A exportação limpa de `3de83fb` em `/tmp/ntm-processing-head-jsca67zr` passou nos **225 testes de ntm-core --lib**; regenerar hazards e buckets não mudou seus hashes. A prova adicional de assets foi concluída no cliente base mudo: 69 chamadas de controle, seis estados de máquina (níveis 0/5), três blocos decorativos e 12 itens conferidos, sem conteúdo recusado; ela não é aceitação funcional das máquinas. Nenhuma instalação ou mundo real foi alterado.


Reprodução da amostra: `agent-run --user sacer xvfb-run -a python3 tools/content-gen/verify-bedrock-processing.py --package <hmod em disco> --horizon-client <binário>`. O script inicia seu próprio cliente offscreen, com cwd/HOME/dados/opções temporários em disco em `../.agent-tmp`, HORIZON_NO_AUDIO e mute.py, verifica estados das três máquinas e /give de 12 itens, captura dois PNGs e encerra o processo. Evidências locais de todas as rodadas: `content-gen-evidence/bedrock-stages34/`.


Medição final em cliente: pacote de 5.405.752 bytes, SHA-256 `7bfcac051deca568e0393ed365876ab8c9da9c546b1d7d8c7558907e29896026`, em `../.agent-tmp/ntm-processing-content-9h0es1uj/target/horizon-mod/ntm.hmod` (snapshot posterior com conteúdo adicional, conservando os mesmos IDs de prova). Os replies de estado e inventário estão em `game.json`/`game-checks.json`, os logs em `game-client.log`, sob `content-gen-evidence/bedrock-stages34/`. `machines.png` mostra os seis cubos opacos e os medidores dos três níveis 5; `items.png` mostra ícones no inventário. As capturas foram inspecionadas. Não houve magenta, erro de carregamento ou divergência de ID/estado na amostra. Os scripts aguardam a atualização do cliente após os comandos; a plataforma de câmera foi ampliada para evitar queda durante a captura. Mundos temporários antigos podem ser removidos, pois as evidências estão copiadas no repositório.
