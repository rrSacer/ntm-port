# Conteúdo da fundição

`content/machine-recipes/foundry.json` (schema 1) é o contrato combinado com o agente de máquinas. A tabela é extraída do checkout local do NTM, sem rede, por `tools/content-gen/foundry_recipes.py`, integrado ao pipeline existente do triturador e alto-forno. O gerador de máquinas converte esse arquivo para os dados do `ntm-core`.

## Materiais e itens

| Tabela | Quantidade | Contrato |
|---|---:|---|
| `materials` | 109 | IDs inteiros do NTM, nomes internos e OreDict, comportamento, cores, traits, conversões |
| Fundíveis / aditivos / outros | 78 / 4 / 27 | sem alterar as conversões dos não fundíveis |
| `items` | 609 | materiais **antes** da conversão, por item singular |
| Scraps já catalogados | 82 | `scraps__<material>`; 72 quanta por unidade |
| Regras `MatDistribution` | 44 | nenhuma chamada omitida; a chamada duplicada do minecart substitui a anterior |
| `alloys` / `disabled_alloys` | 13 / 3 | 11 ligas + hematita/malaquita; variantes GT6 desativadas |
| `molds` | 26 | 21 pequenos, 5 grandes; 452 pares molde × material com saída |
| `firebox_fuels` | 85 | 33 NTM e 52 vanilla; tempo, calor e recipiente |

Os nomes internos seguem a constante Java, sem `MAT_` e em minúsculas (`mingrade`, `dura`, `ferro`); os IDs públicos dos itens conservam as identidades já existentes (`red_copper`, `dura_steel` etc.). `colors` tem ordem **`[solidColorLight, moltenColor, solidColorDark]`**, declarada também em `color_fields`. Materiais construídos com `make` conservam as cores padrão de `NTMMaterial`, sem inventar paleta.

`quanta` contém todas as 31 formas estáticas de `MaterialShapes`, incluindo as técnicas de quantidade zero. Exemplos: quantum=1, nugget=8, billet=48, ingot=72, castplate=216, heavybarrel=432, block=648. O consumidor aplica `quantidade * convOut / convIn` com truncamento inteiro; carvão 72 vira carbono 36, e esmeralda 72 vira berílio 54.

O snapshot reutiliza `dictionary()` e `ShredderResolver`, estendendo apenas os prefixos declarados em `DictFrame` e as formas autogeradas do NTM. Não adiciona cobre vanilla moderno, deepslate ou entradas de outros mods. O JSON conserva o snapshot, suas origens e quatro colisões entre chaves.

A precedência é a de `Mats.getMaterialsFromItem`: percorre as chaves OreDict do snapshot, verifica entrada exata do minério antes de tentar prefixo **para aquela chave**, e para na primeira correspondência. Depois **soma** as entradas fixas do item e o scrap, se houver. Uma entrada fixa não substitui o resultado OreDict. `MatDistribution.registerEntry`, por outro lado, substitui uma entrada fixa anterior para o mesmo item. `item_sources`, `distribution_rules`, `ore_collisions` e `precedence` registram essas decisões; o snapshot exclui a ordem de registro de outros mods.

## Ligas e moldes

A fonte atual tem **16 declarações**, não 14: 13 ativas no perfil sem mods externos e três condicionais de GregTech 6 (`steelWrought`, `steelPig`, `steelMeteoric`). As 13 mantêm a ordem da fonte, frequência em ticks, ícone e entradas/saídas em quanta. Hematita e malaquita também são receitas selecionáveis do cadinho. As três condicionais ficam completas em `disabled_alloys`, com `requires_mod` e `enabled_by_default=false`.

Os 26 moldes têm ID estático, tamanho, custo, quantidade e saídas por material. A seleção por forma prefere o primeiro item NTM que não contenha `fragment`; depois usa o primeiro OreDict, como `ItemMold`. MoldBlock conserva os overrides de pedra e obsidiana. O molde de carimbo custa 288, enquanto a reciclagem do carimbo devolve 216; essa diferença existe na fonte e é preservada.

## Combustíveis

A firebox tem perfil próprio: aceita madeira e recipientes, ao contrário da tabela restrita do alto-forno. Coal/Coke/Lignite usam tempo ×1,25 e calor ×2; sólidos ×1,5/×3; foguete ×1,5/×5; balefire ×0,5/×15; sem categoria ×1/×1. O calor base é 100. Tempos não são cortados na capacidade térmica.

`fuel_details` registra valor base, categoria, modificadores, chaves OreDict e origem. `container` é nulo ou o item devolvido quando a pilha acaba; lava devolve `minecraft:bucket`. A tabela vanilla é um perfil explícito das identidades legadas da fornalha 1.7.10, validadas no registro 26.3; não se inferem combustíveis pelo nome moderno. Carvão vegetal fica sem categoria Coal e usa 1600 ticks/100 calor; carvão usa 2000/200. As classes vanilla ladder/button não são madeira nesse perfil.

Lacunas em `fuel_gaps`: `book_guide` não está no catálogo (é o livro, que pede a tela do guia); `coal_eternal` entrou na rodada 6 (`ntm:coal_eternal`, pilha 1, `single * 16` = 3200 ticks na fornalha e 400 no forno, e **o seu próprio item como recipiente**: aceso, volta inteiro, queima para sempre; cinza do tipo MISC; a fonte o dá só pelo tijolo esquecido, que não existe no port); combustíveis de outros mods/configurações, identidades exclusivamente modernas e subclasses genéricas de bloco NTM com `Material.wood` ainda não verificadas ficam de fora.

## IDs e receitas

São **34 IDs novos**: oito blocos e 26 itens. O catálogo passa a **2551 IDs**, com 351 blocos, 2200 itens e 21 máquinas.

Blocos: `ntm:machine_crucible`, `ntm:heater_firebox`, `ntm:foundry_channel`, `ntm:foundry_tank`, `ntm:foundry_outlet`, `ntm:foundry_slagtap`, `ntm:foundry_mold`, `ntm:foundry_basin`. Todos usam `minecraft:beehive`, `facing`, `honey_level=0..5`, 24 variantes, loot próprio, textura fonte `brick_fire`/`block_steel` e traduções pt_BR/en_US. Na fonte, somente cadinho, firebox e saída têm orientação; `facing` nos demais é parte do contrato uniforme do port. O fallback usa o cubo; OBJ, telas e multibloco pertencem à frente de visual.

Itens novos (prefixo `ntm:mold__`):

```text
nugget billet ingot plate wire plate_cast wire_dense blade blades stamp shell pipe
ingots plates plates_cast wires_dense block c9 c50 barrel_light barrel_heavy
receiver_light receiver_heavy mechanism stock grip
```

As seis receitas vanilla de `CraftingManager.java:922–927` passam a resolver: bacia, base de moldagem, quatro canais, tanque, saída e slagtap. A receita vanilla de `mold_base` já existia. Cadinho/firebox e gravação dos moldes usam a bigorna NTM, que ainda exige o port dessa máquina; não se criam receitas alternativas. Scraps são produzidos pela fundição; não há receita vanilla de conversão sintética. Hazards, abas e índice de receitas são regenerados.

## Limites e validação

O contrato original fixava os scraps em 72. Isso foi fechado pela decisão D4 (ver "Sucata exata" abaixo). O runtime de canais/moldagem pertence às etapas 2/3. Esta entrega inclui seus blocos e tabela de moldes, sem afirmar aceitação funcional dessas etapas.

```sh
/usr/local/bin/agent-run python3 tools/content-gen/generate.py
/usr/local/bin/agent-run python3 tools/visual-gen/generate.py
/usr/local/bin/agent-run python3 tools/content-gen/test_foundry.py
/usr/local/bin/agent-run python3 tools/content-gen/test_blast_furnace.py
/usr/local/bin/agent-run python3 tools/content-gen/test_shredder.py
/usr/local/bin/agent-run python3 tools/content-gen/test_m3.py
/usr/local/bin/agent-run python3 tools/content-gen/validate-bulk.py
/usr/local/bin/agent-run cargo test --workspace -j 3
```

Nove testes novos verificam materiais/conversões/cores, aritmética e unidades, precedência e entradas fixas, scraps, ordem/condicionais das ligas, moldes e custos, combustíveis/recipientes, grafo completo de IDs/blocos/loot e determinismo. Logs em `content-gen-evidence/foundry/`. Build/check do pacote são executados em cópia temporária sob `/tmp`, sem instalar nem executar jogo em `~/Games/horizon*`.

Validação desta entrega: nove testes da fundição, seis do alto-forno, sete do triturador e sete M3 passaram. A validação estática passou para 2551 IDs e 813 receitas vanilla registradas no pacote (inclui os outros geradores; o índice M1 conserva 235 fiéis e 10 temporárias). O pacote `dist/ntm-content-foundry.hmod` passou build/check API 0.2: 4.471.803 bytes, SHA-256 `dd06e62c9fecee0dedd413d398ec1a020736ce290e0af0fa804f8767cf92bbae`. O artefato é local/ignorado pelo git.

`cargo test --workspace -j 3` também passou: 201 testes, dois ignorados. O Rust foi testado no checkout compartilhado durante a integração do agente de máquinas; somente `hazards.json`, gerado pelos IDs novos, entra no commit de conteúdo dentro de `crates/`.

## Forno rotativo (`ntm:machine_rotary_furnace`)

Port de `TileEntityMachineRotaryFurnace` (`updateEntity`:140-227, `canProcess`:324-338, `consumeItems`:339-352) e de `RotaryFurnaceRecipes`. As 13 receitas saem de `content/machine-recipes/rotary_furnace.json` (`tools/content-gen/rotary_furnace_recipes.py`) e viram `crates/ntm-core/data/machines/rotary_furnace.json` por `tools/machine-gen/build.py`.

- 5 slots: três entradas sem ordem, o identificador de fluido (tipa o tanque de 16 000) e o combustível (`burnTime/2`, módulo de queima e calor `fuel::Module::ROTARY_FURNACE`). Vapor 12 000 e vapor gasto 120, na razão 100:1; `steamUseMult = 10·log10(velocidade) + 1`.
- O bico fica a `rot·2,875`, `y+1,25`, com `rot = dir.getRotation(DOWN)`; despeja `INGOT.q(1)` = 72 quanta por tick, `safe = true`, num raio de até 6 blocos (`molten_tick`, `foundry_server.rs`).
- **R8**: o `output` fundido não está em `held_contents`; o Java não tem `breakBlock` próprio e o perde ao quebrar. Teste `r8_breaking_rotary_furnace_loses_molten_output_and_keeps_only_slots`.
- Lacunas declaradas: fumaça e fuligem (o port não tem poluição).

## Sucata exata (D4)

`ItemScraps` guarda o material como dano e a quantidade na tag `amount`; sem tag vale um lingote (`getMats`, `ItemScraps.java:118-132`). No port o item é `ntm:scraps__<material>` com a quantidade em `custom_data` como `{ntm_amount:N}` (`foundry/scrap.rs`). Em 72 o dado é omitido: é o mesmo valor e empilha com o item simples.

- **Quem solta**: cadinho (uma por pilha, `MachineCrucible.breakBlock`:68,117), canal, tanque, molde, bacia (`FoundryCastingBase`:114,137; `FoundryTank`:105,128), forno a arco e eletrolisador. Cada um solta **uma** sucata por material com **todos** os quanta, como o Java. Antes eram `quanta/72` lingotes e o resto se perdia. O rotativo não solta nada (R8).
- **Quem consome**: o cadinho (`Mats.getMaterialsFromItem`:250-252, depois a conversão `smeltsInto`) e o forno a arco em modo líquido (`ArcFurnaceRecipes.getOutput`:170-177) fundem a quantidade da sucata, sem arredondar.
- **Caminho de queda**: `ServerHost::drop_stacks`. Dentro de uma transação vira `WorldEffect::EncodedItems` (SNBT do item no formato do ledger), reaplicado por id.
- **Desvio declarado**: dado de item sem `ntm_amount` vale um lingote (o Java lê 0); nada no port grava esse dado.

## Alvo do jato e lingotamento contínuo (F1, F4, F5)

- **F1, célula → núcleo** (`foundry_server.rs`, `below`/`Hit`): o jato acha o primeiro bloco que não é ar; se for uma das 4 células do funil (`getMetalPourPos`: `y+2` sobre `núcleo + rot − dir`, `núcleo − dir`, `núcleo + rot`, `núcleo`, com `rot = dir.getRotation(UP)`), resolve para o núcleo e passa a célula e a face UP ao aceitador (`MachineStrandCaster.canAcceptPartialPour`:107-117, `tryPourStack`: a face é a do raio). Outra célula, outra face, sem molde ou outro material: o jato fica parado e nada se perde (`safe`).
- **F4, o modelo** (`strand_caster.rs`, de `TileEntityMachineStrandCaster` e `TileEntityFoundryCastingBase`): 7 slots (molde e 6 saídas); água 64 000 e vapor gasto 64 000, `5·custo` por molde fundido; aceita até `custo·9` com molde, capacidade `custo·10` (50 000 sem molde); funde em lote com 9 moldes ou 200 ticks depois do último progresso (`lastProgressTick`, o `t` do NBT, gravado em duas palavras; contado no tick do servidor); portas de fluido nas 4 células de `getPorts`; o molde entra só à mão e as saídas saem em qualquer face.
- **F5, a sucata de excesso**: o que passa da capacidade (molde mais leve) sai como **uma** sucata com a quantidade exata em `y+2` (`Logic::world_effects` → `MachineEffect::DropStack` → `WorldEffect::EncodedItems`, no mesmo lote do modelo). Quebrar solta o molde, as saídas e uma sucata com todo o metal do poço (D4).

Fechado depois (revisão 3):
- **`PortDef` com direção**: as 4 portas do lingotamento (`getPorts`: `núcleo + rot − dir`, `núcleo − dir` e as mesmas 4 células atrás, `rot = dir.getRotation(UP)`, abertas ora a `rot` ora contra ela) e as 4 do rotativo (vapor para trás, `dir.getOpposite()`; fluido para `rot = getRotation(DOWN)`) só aceitam um duto vindo da direção da conexão (`container_bridge::fluid_port_faces`, pelas faces que as redes de fluido já usam para o ZIRNOX).
- **Pilha de saída**: o lingotamento usa o tamanho de pilha do item (`getMaxStackSize`): 64, menos as lâminas de triturador e os carimbos, que são 1 (`data/machines/mold_stack_limits.json`, gerado de `content/ids.json` por `tools/machine-gen/build.py`). Seis carimbos enchem os 6 slots e o resto do poço espera.
- **A fundição por clique** (`foundry_hand.rs`, `Server::native_foundry_use`, ligada no glue depois da serraria, sem agachar): `MachineStrandCaster`, `FoundryCastingBase` (molde e bacia), `FoundryChannel`, `FoundryTank` e `MachineCrucible .onBlockActivated`. **Só a mão principal** age (o Java lê `getHeldItem`; o clique da mão esquerda segue adiante e abre a tela). Uma pá é uma ferramenta de classe `shovel`: as `ItemToolAbility` SHOVEL e MINER do conteúdo (`data/tool_shovels.json`, gerado por `tools/machine-gen/build.py`) e as `minecraft:*_shovel`; um nome terminado em `_shovel` de outro mod não basta.
  - **lingotamento**: um molde (de qualquer tamanho) no slot vazio; uma pá leva o poço como **uma** sucata exata (com o poço vazio a pá toma o clique e nada acontece);
  - **molde e bacia**: primeiro o item moldado sai do slot (a mão pode estar vazia), depois um molde **do tamanho do bloco** entra no slot vazio (um da pilha, com os dados), depois a pá leva o poço;
  - **canal e tanque**: só a pá, o poço como uma sucata exata (o Java toma o clique mesmo vazio);
  - **cadinho**: a pá leva cada pilha (a da receita e as de resíduo) como **uma sucata exata cada**, num só `give` (chamadas separadas escolhem o mesmo slot livre).
  Tudo num só `commit_player_transfer`: a recusa não move nada. O molde também sai à mão pela tela (slot 0 do lingotamento, provado em jogo), então não fica preso sem a chave de fenda (`onScrew`, que o conteúdo não tem). Desvios: o que não cabe é lançado dos olhos, não onde o Java o gera; e só o que não coube é lançado.
- **Pá e molde no lingotamento** (primeira versão, `MachineStrandCaster.onBlockActivated`)

Lacunas declaradas: o desenho animado do lingotamento

### Mutantes (rodados à mão, todos mortos)

| Mutante | Morto por |
|---|---|
| F1: a face do raio não é checada | `strand_caster::only_the_face_up…` |
| F1: a resolução perde uma coluna do funil | `a_jet_on_a_funnel_cell_resolves…` |
| F4: limite `custo·10` em vez de `·9` | `it_accepts_nine_molds…` |
| F4: descarga parcial em 100 ticks | `a_partial_batch_waits_200…` |
| F4: o transbordo não carimba `lastProgressTick` | `it_accepts_nine_molds…` |
| F4: o lote não gasta água | `nine_molds_cast_at_once…` |
| F5: o excesso se perde (sem efeito) | `the_excess_of_a_lighter_mold…` |
| F5: o excesso arredondado a lingotes | `the_excess_of_a_lighter_mold…` |
| F5: os efeitos não são drenados | `the_excess_of_a_lighter_mold…` |
| D4: o poço arredondado ao quebrar | `a_broken_block_gives_back_every_quantum` |
| D4: o cadinho ignora a quantidade da sucata | `a_scrap_melts_into_its_own_amount` |
| R8: o fundido do rotativo vira sucata | `r8_molten_output_is_lost_on_break` (unidade e servidor) |

## A bica (`foundry_slagtap`) e a escória (`ntm:slag`)

`TileEntityFoundrySlagtap` é uma saída (`TileEntityFoundryOutlet`) que, em vez de vazar num aceitador, **cria o bloco de escória** `ModBlocks.slag` (`BlockDynamicSlag`, com `TileEntitySlag`: material e quantidade, no máximo `BLOCK.q(16)` = 10 368) no primeiro bloco atingido por um raio de 15 blocos para baixo (de `y − 0,125` a `y + 0,125 − 15`, isto é, as células `y−1` a `y−15`): se já é escória do mesmo material, soma até o máximo; o que sobra vai uma célula acima do bloco atingido, se ela é substituível (`slagtap.flow`, linhas 36-111). A escória tem altura pela quantidade (`setBlockBoundsBasedOnState`), cor pelo material, escoa por tiques agendados (`updateTick`) e solta uma sucata exata ao ser quebrada (`getDrops`, D4).

**Decisão do coordenador: opção B**, implementada (rodada 5):

- **`ntm:foundry_slagtap`** é `Kind::Slagtap` / `Data::Slagtap(Outlet)` (o mesmo filtro, inversão e fechamento por redstone da saída). Recebe metal só por trás (`side == opposite(facing)`), com o filtro, e só se o raio acha um bloco (`canAcceptPartialFlow`, `foundry_server::accept_flow`).
- **`ntm:slag`** é um bloco de máquina (`Kind::Slag` / `Data::Slag`, `foundry/slag.rs`) sem item e sem `facing`; a altura é a propriedade `layer` (0..8, oitavos do máximo: `fill_level`) e a forma é a mesma altura, na seleção e na colisão (`shapes` do conteúdo, template `minecraft:iron_block` com `state_properties`). Guarda tudo no registro (não tem slots: `slots_recorded` verdadeiro) e o quebrar solta **uma sucata exata** (`Logic::scraps`, D4).
- **O vazamento é um commit só** (`server/slag_server.rs`). O canal ou tanque de origem, ao entregar à bica, não muda nada em memória: apenas registra o pedido. No fim do tique, `slag_pour` lê tudo (a escória atingida, a célula de cima, o registro da origem) e publica **de uma vez** (`ServerHost::commit_machines`): o registro menor da origem, o registro maior da escória atingida (se é do mesmo material) e o bloco novo com o primeiro registro (`CellWrite.initial`). Recusado (chunk descarregado, mudança de bloco pendente, `refuse_commits`), nada muda em memória nem no mundo e a origem guarda o metal; aceito, o modelo em memória é trocado depois. Assim, em qualquer ponto, o metal está na origem ou na escória, nunca nos dois e nunca em nenhum.
- **Um chunk só.** A origem, a escória que ela enche e a célula do bloco novo precisam estar no mesmo chunk. O registro de um chunk é o que o mundo grava de forma atômica; os registros de dois chunks chegam ao disco em momentos diferentes, e o diário que cobre um passo em vários chunks (`Survivor`) não sabe criar o bloco que falta numa gravação rasgada: o canal ficaria vazio sem a escória, o metal perdido. Uma bica cujo vazamento sairia do chunk da origem **não vaza nada** e a origem guarda o metal (lacuna declarada, com teste e prova em jogo `slagtap-border`). Fazer o vazamento atravessar chunks pede o diário recriar o bloco ausente: fatia à parte.
- A altura (`layer`) de uma escória que já existia e cresceu é escrita pelo laço de aparência do servidor (`desired_state`), depois do commit: é só visual, o registro já é o novo.
- O que sobra quando a escória atingida enche e a célula de cima não é ar fica **na origem** (vazamento parcial, nada se perde).

Lacunas declaradas desta primeira versão (visuais, de poluição ou fora do escopo pedido):
0. **O vazamento não atravessa chunks** (acima).
1. **O escoamento da escória** existe dentro de um chunk (abaixo); fora dele e entre reinícios não (ver o fim).
2. **Cor neutra**: o Java pinta a textura pelo material (`colorMultiplier`); aqui o bloco usa `slag.png` sem tinta.
3. **Blocos substituíveis** (mato, flor, tocha, neve, água): o Java atravessa os de colisão nula no raio (mato, flor, tocha) e substitui o atingido se for substituível (mato, neve, água); aqui só o ar (`air`, `cave_air`, `void_air`) é livre, o resto é um acerto e a escória pousa **em cima** (o Java a poria no lugar do mato).
4. A **partícula do jato** (`AuxParticlePacketNT`) e o **pick-block** (sucata do conteúdo) não existem.
5. Uma escória posta por comando (`/setblock`) nasce vazia (`layer=0`, forma vazia); o Java a removeria no primeiro tique.
6. A bica não tem modelo próprio (cubo orientado, como as outras saídas antes do visual).

As outras duas lacunas da lista são **visuais ou de poluição**: a "conexão canal/tanque" (`FoundryChannel.canConnectTo`) só muda a forma e os braços desenhados do canal, e a "porta firebox" é a porta de fumaça (`TilePortShapes.assembler`, `moduleSmokePorts`) da fornalha, que depende de um sistema de poluição que o port não tem (aceitas).

## Checklist do conteúdo e notas de rodada 5

- **`data/tool_shovels.json` precisa ser regerado** (`python3 tools/machine-gen/build.py`, que lê `ModItems.java` e `content/ids.json`) **quando outras `ItemToolAbility` SHOVEL ou MINER forem portadas** (`steel_shovel`, `desh_shovel` etc. ainda não estão no catálogo): sem isso a pá nova não age nos blocos de fundição. Um teste (`the_shovels_come_from_the_content`) pega só os casos conhecidos.
- `foundry::scrap::stacks` **descarta a pilha de quantia 0** (o Java criaria uma sucata de 0 quanta, um item sem valor: improvável de ocorrer, porque nenhuma máquina guarda uma pilha de metal com quantia 0).

## Fechamento por redstone da saída e da bica (`isClosed`)

`TileEntityFoundryOutlet.isClosed() = invertRedstone ^ isBlockIndirectlyGettingPowered`; fechada, a saída (e a bica, que a estende) não aceita metal (`canAcceptPartialFlow`). No port: `Outlet.invert_redstone`, lido em `foundry_server::accept_flow` por `ServerHost::redstone_many` (qualquer sinal > 0). **Sem leitura** (chunk não carregado ou host que não lê) conta como **fechada**: nada verte e a origem guarda o metal; a leitura de volta, a saída abre no tique seguinte (teste de servidor). Os dois sentidos vão num inteiro só do BE (bit 0 filtro invertido, bit 1 redstone invertida): os registros antigos (0 ou 1) leem como antes e o layout de dados não muda. A inversão se muda por um botão da tela antiga ("Redstone: fecha/abre com sinal"), porque o Java usa a chave de fenda (`onScrew`), que não existe. Fora: a barreira desenhada quando fechada (visual).
Prova em jogo simples (`slagtap-redstone`, ponto `none`): um bloco de redstone ao lado da bica a fecha (o canal guarda os 144), tirado ele a bica abre e verte; a inversão só por teste de servidor.

## Tickets abertos da fundição

- **`book_guide`** (o livro de guia, `FuelHandler` single, `ModEventHandler` o dá no início): fora do catálogo por **depender da tela do guia** (`ItemGuideBook`, os tipos `BookType`); entra com ela.
- **Vazamento da bica entre chunks**: o diário (`Survivor`) recriar o bloco ausente (`WorldEffect::Blocks` com registro inicial) para a origem e a escória em chunks diferentes sem perda. Hoje a bica não vaza nesse caso (o metal fica na origem). Aceito pelo coordenador como fatia à parte.

## O escoamento da escória (`BlockDynamicSlag.updateTick`), dentro de um chunk

Agendado um tique depois de um vazamento (a escória enchida e a nova) ou de um passo de escoamento (`slag_active`, `slag_server::slag_flow`, no começo do tique, antes dos looks das máquinas), cada bloco de escória faz, em **um commit por passo**:

1. **Cai inteira** na célula de baixo se ela é ar: a de baixo nasce com a quantia toda e a de cima vira ar (`commit_structural`: sem quebra, sem sucata). Como no Java, a nova não é reagendada (cai uma célula por agendamento).
2. Senão, **dá à escória de baixo do mesmo material** o que cabe (`min(10368 − a dela, a minha)`), um `commit_machines` de duas máquinas; a de baixo fica agendada. Se a de cima esvaziou, **fica vazia e é removida no tique seguinte** (um commit de máquina que escrevesse ar sobre ela seria lido como quebra e soltaria sucata: duplicação). Uma escória vazia lida de um mundo reaberto é agendada ao ser adotada.
3. Senão, de **1/5 da capacidade** (2073) em diante, **espalha** `max(quantia / (n·2), 1)` para cada um dos `n` lados livres (norte, sul, leste, oeste: ar, no mesmo chunk); a de cima perde `n·toSpread`, tudo em um `commit_machines` (a de cima menor e as novas com `initial`); as novas ficam agendadas.

**Nada cruza a divisa de chunk** (um lado em outro chunk não conta como livre; descer é sempre no mesmo chunk). Recusado, nada muda e o passo não é repetido. Fidelidade: o Java conta os lados substituíveis (mato, neve) e aqui só o ar; o agendamento não é gravado (um reinício interrompe um escoamento em curso; o metal fica onde estava).
**Escória congelada** (máquina `frozen` no tique do seu passo): sai de `slag_active` sem reagendar. Uma vazia é reagendada ao ser adotada de novo; uma cheia deixa de escoar até outro vazamento a agendar, sem perda (o metal fica onde estava).
Testes: soma exata a cada tick nos modelos e nos registros (queda, doação, esvaziamento, espalhar com 4/2 lados, limiar 2072/2073, divisa de chunk, recusa, quebra da vazia sem sucata de 0, reabertura entre os dois passos pelo registro e pelo diário); 9 mutantes mortos.
