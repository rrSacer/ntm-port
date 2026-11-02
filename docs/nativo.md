# O NTM como mod nativo (migração para o Horizon atual)

O Horizon atual não roda mais mods WebAssembly. Mod com código é um **bundle
horizon-mixins** compilado junto com o jogo (`docs/native-mods.md` do Horizon).
O plano completo está no artifact `horizon-migracao` do Traycer. Aqui fica o
que o repositório tem dele.

**Base ativa em 02/11/2026:** upstream `5d65d7b50`, integrado na branch
`ntm-current` com as extensões genéricas ainda necessárias ao port. O arquivo
`native/ntm/compatibility.json` fixa a revisão completa. A série antiga `ntm-next`
fica preservada no Git. As bombas seguem somente no modo otimizado.

## Onde está

| Caminho | O que é |
| --- | --- |
| `native/ntm/mixins/ntm/horizon-mixin.toml` | O bundle: o crate da lógica e os hooks nomeados |
| `native/ntm/mixins/ntm/*.rs` | Um helper pequeno por hook |
| `native/ntm/compatibility.json` | Os commits fixados: Horizon (`ntm-current`), horizon-mixins 0.6, horizon-mods (Recipes) |
| `tools/package-native.sh [SAÍDA]` | O bundle pronto, com o `crates/ntm-core` copiado como `[[crate]]` |
| `tools/build-native.sh` | O jogo com o NTM e o Recipes oficial dentro |

O crate fica fora do bundle no repositório e é copiado no empacotamento. Os
campos herdados do workspace (`version.workspace`, `serde.workspace`...) viram
literais, porque um crate de bundle só aceita dependências de registro que já
estão no `Cargo.lock` do Horizon (`serde` e `serde_json` estão).

**Um crate só.** Um crate de bundle não pode depender de outro por `path`
(horizon-mixins recusa com `E_CRATE_DEPENDENCY`), e um crate de bundle não pode
ser o `dependent` de outro. Por isso o antigo `crates/ntm-explosion` virou o
módulo `ntm_core::explosion`, com os seus testes (`crates/ntm-core/tests/explosion_*.rs`)
e exemplos (`examples/explosion_*.rs`).

## Construir

```sh
BUILD_DIR=<pasta grande> agent-run env QUEUE=cargo bash tools/build-native.sh
```

1. Confere o `main` oficial e exporta a revisão de `compatibility.json` (`HORIZON_NEXT`, `NEXT_REV`) para `$BUILD_DIR/source`. Nunca usa o checkout em si.
2. Compila o horizon-mixins no commit fixado, uma vez.
3. Junta os bundles em `$BUILD_DIR/mods`: o do NTM, por `package-native.sh`, e o `recipes` do horizon-mods no commit fixado.
4. Roda `horizon-mixins apply` para `$BUILD_DIR/generated` e sincroniza `$BUILD_DIR/stage` preservando os arquivos idênticos.
5. Compila `horizon-client`, `horizon-ctl` (agora no `horizon-client-lib`) e o empacotador `cargo-horizon-mod`, com o perfil `release` e sem a feature antiga `mods`.

O binário sai em `$BUILD_DIR/target/release/horizon-client`, ou em `$CARGO_TARGET_DIR/release/horizon-client` quando definido. A consulta do upstream só pode ser omitida com `SKIP_UPSTREAM_CHECK=1` para reproduzir o snapshot já selecionado.

## Hooks em uso

| Hook | Versão | Helper | O que faz |
| --- | --- | --- | --- |
| `world_lifecycle` | 1 | `lifecycle.rs` | Ao abrir um mundo, loga `ntm native: N machine blocks` (a L0). |
| `block_entity` | 1 | `be.rs` | Um bloco de máquina posto recebe o block entity, com o estado novo em JSON sob `ntm` no compound. |
| `block_entity_ticks` | 1 | `ticks.rs` | Os block entities das máquinas fazem tick. |
| `block_entity_container` | 1 | `container.rs` | Os slots, as faces e o menu vanilla de cada máquina. |
| `block_entity_tick` | 1 | `tick.rs` | O tick do servidor: estado, slots e data do menu passam pelo `ntm_core::native`. |
| `block_entity_update_tag` | 1 | `update_tag.rs` | Manda aos clientes `data` (IntArray, o vetor do menu) e `facing` (só nos blocos sem a propriedade, as bombas), o contrato com os modelos da V3. |

O registro e a sincronização dos tipos de block entity são do upstream. O `update_tag` recebe a entidade oficial e seleciona seus dados visuais. O tick grava o `data` no compound e chama `native_block_entity_changed` só quando muda um índice listado em `synced` no `horizon_machine/<bloco>.json` (lido uma vez por bloco com `native_data`). Os outros valores só importam à tela aberta. O hook `slot_filters` (V2, API 20) dá as regras dos slots, de `ntm_core::native::slot_filters`: a fornalha aceita o que funde, uma bateria criativa, e o resultado é só de saída. As regras valem para o menu, os cliques e os funis.

A cola fica no módulo `ntm_glue` (`glue.rs`), que converte `ModBlockEntity` e `ItemStack` para os tipos simples do `ntm_core::native` e de volta. Um slot que a lógica não mudou mantém a pilha do jogo, com todos os componentes.

## A L1: a fornalha elétrica

(Desde a L3 a fornalha passa pelo servidor como as outras: `ElectricFurnace` é `Slotted`, e o `ntm_core::native` só dá o container, as regras e os dados dela. O que segue descreve a prova da L1.)

`ntm_core::native` roda a fornalha elétrica (`machine_electric_furnace_off`) no menu vanilla de fornalha: entrada (slot 0, por cima), bateria (slot 1, pelos lados) e resultado (slot 2, por baixo). Os 4 data slots são energia, máximo, progresso e total, desenhados como a chama e a seta. A energia vem da bateria criativa no slot (`charge_from_slot`); as redes de energia entram na L3.

Com a V2 na `ntm-next` (`88b45bdb2`), o menu abre pelo jogador, independente do template `beehive` das máquinas.

## Prova da L0 (30/10)

- Build: `ntm-next` `f867e476`, horizon-mixins `5140cdf8`, horizon-mods `ae217daa`. O `apply` encontrou 1 crate, 6 injeções e 2 módulos (NTM e Recipes); o `cargo build --release` levou 5 min 19 s.
- Jogo em Xvfb com o `ntm.hmod` do lote 4 de energia (já com `components`, da C1), num mundo novo: o log traz `ntm native: 89 machine blocks` ao abrir o mundo.
- O script repete o que o launcher faz e o horizon-mixins exige: tira os links do export (o `CLAUDE.md` do Horizon é um) e põe o commit do build no `horizon_revision` de cada bundle copiado.

## Prova da L1 (30/10)

`tools/test-game/native_l1.py <jogo> montar|conferir <horizon-ctl>`, no jogo de `build-native.sh` (`ntm-next` `f867e476`, 10 injeções e 3 módulos, 4 min 20 s):

- **montar:** por `/setblock`, a fornalha com um funil de 8 minérios de ferro por cima, um funil com a bateria criativa ao lado e um funil embaixo para um baú. O block entity nasceu com o estado `ntm`. Em 25 s, 5 lingotes no baú e a fornalha no meio do sexto (progresso 13 de 100, energia cheia).
- **conferir**, depois de fechar e reabrir o jogo: o baú com os 8 lingotes, a fornalha com a bateria no slot 1 e o mesmo estado. Um minério novo no funil virou o nono lingote.

## A tela da fornalha (30/10, `ntm-next` `88b45bdb2`)

Fase `tela` do `native_l1.py`: o jogador abre a fornalha (menu `container`, título "Electric Furnace"), e a terra é recusada na entrada pelo filtro. Faltam duas coisas, da V2/L2:

- a tela não desenha o fundo: os itens aparecem soltos (`native-l1-tela.png`). Isso é o intérprete de telas da L2 ou o desenho do menu vanilla para um `Kind::Mod`;
- o `horizon-ctl screen` não lista os slots do container (só os 36 do jogador), então o roteiro não clica neles.

## L3, passo 1: o servidor do ntm-core no `server_tick` (30/10, `ntm-next` `88b45bdb2`)

Desenho no artifact "L3: todas as máquinas nativas". O `Server` roda as mesmas fases de antes (`Server::native_tick`, em `crates/ntm-core/src/server/native_server.rs`) no fim de cada tick do servidor. Ele fala com o jogo pelo `ServerHost` da cola (`Host` em `glue.rs`):

- `get_block` e `set_blocks` são `native_loaded_block` e `native_set_blocks`;
- slots, data e estado são os do `ModBlockEntity` (`native_with_mod_block_entity`, só na dimensão em tick por ora);
- o `block_changed` vira os eventos de máquina e cabo, e o `menu_button` vai para a fila de botões;
- o tick do block entity só adota a máquina do estado salvo quando o servidor não a tem (o mundo foi reaberto).

Os slots, as regras e o `synced` de cada máquina vêm dos `horizon_machine/*.json` do conteúdo, embutidos no ntm-core pelo `tools/machine-gen/build.py` (`data/machine_files.json`). O `native_data` não lê o `data/` dos content mods, e o `native_asset` só lê `assets/`.

**Prova** (`tools/test-game/native_l3.py`): uma firebox e uma prensa a carvão postas por `/setblock` e alimentadas por funis. As regras dos slots levaram o carvão ao combustível, a matriz ao slot dela e os lingotes à entrada. Em 20 s, a firebox chegou a 85.800 de calor e apareceu acesa (`honey_level=4`); a prensa fez 7 placas, com a matriz em desgaste 7 (`minecraft:damage`). Depois de fechar e reabrir, o servidor adotou as duas do estado salvo: a prensa tinha 8 placas e a matriz em desgaste 8, e a firebox seguia acesa com calor cheio.

## Cabos, e lacunas da L3 (30/10)

**Cabos:** não são guardados em lugar nenhum. Quando o servidor adota uma máquina, ele procura nos blocos vizinhos os cabos (`native_find_cables`, até 256 leituras por tick), e de cada cabo achado os vizinhos dele; um cabo posto depois chega como mudança de bloco, e o `simulate` tira os que somem. O mundo é a única fonte. Os dutos são máquinas e voltam como as outras.

**Lacunas:**

- **Nether e End:** o block entity de uma máquina só é alcançado na dimensão em tick (`native_with_mod_block_entity` usa `game.dim`); as máquinas das outras dimensões ainda não passam pelo servidor.
- **Máquinas só com a tela antiga:** 12 máquinas registradas não têm `horizon_machine` (bateria, queimador a lenha, duto, barril, tanque, `ntm:teste` e as 5 do lote de vapor; a fornalha elétrica usa o menu de fornalha). Os controles delas eram botões da tela antiga (o modo da bateria, o tipo do duto, despejar no barril), que saiu com a camada WASM. Precisam de arquivo de máquina e tela nativa (`horizon_screen` e o intérprete da L2).

**A fornalha no caminho comum** (`native_l1.py` e `native_l3.py montar` no mesmo build): a fornalha fundiu pelos funis como na L1 (5 lingotes em 25 s), e a firebox e a prensa seguiram iguais.

## Sem a camada WASM (30/10)

O servidor do `ntm-core` agora só existe no modo nativo (`Server::native`, `native_tick`). Saíram os mods `mods/ntm-logic-*`, o `client.rs`, o protocolo e as caixas de correio (`proto`), as transações de itens (`tx`), as sondas e fichas de bloco (`probes`, `token`), a identidade do mundo (`worldid`), a amostra de radiação por comandos (`radiation_server`) e o save do mundo inteiro (`persist` antigo). Com eles foram os testes que só valiam para esse caminho: transações, quedas entre os dois saves, lag do cliente, identidade e sondas. O `tools/build-logic.sh` virou `tools/build-content.sh`, que só monta o `ntm.hmod` de conteúdo.

O que o mundo guarda fora dos block entities (`ntm_core::persist`): as detonações na fila, as zonas de fallout, os detonadores ligados, as doses dos jogadores e o relógio do mundo, que é o das zonas. Vai na chave `world` do `Kv` do host, quando muda; o relógio sozinho vai a cada 600 ticks. As explosões em andamento (`nx.*`) e a configuração das bombas (`bombcfg`) usam o mesmo `Kv`. No bundle, cada chave é um arquivo em `<mundo>/data/ntm/<chave>` (`native_world_dir`; sem mundo aberto, nenhum), escrito inteiro e renomeado por cima do antigo.

**Lacunas que a remoção deixa** (todas com hook ou helper já previstos no plano):

- ~~**Radiação do inventário**~~, ~~**dano das bombas**~~, ~~**detonador**~~ e ~~**lista de jogadores**~~: voltaram na L4 (abaixo).
- ~~**Sons e partículas das bombas**~~: voltaram (abaixo).
- **O que a máquina guarda fora dos slots** (metal fundido como sucata, itens de uma máquina sem slots) vai a `ServerHost::drop_items`, que o bundle ainda não implementa: não há helper para soltar itens no mundo.
- O código da tela antiga (`view.rs` e os `view`/`action` das máquinas) ficou, sem caminho que o chame. Sai quando as telas nativas cobrirem tudo.

## A L4 no servidor: radiação, dano e detonador (30/10, `ntm-next` `29b4726b4`, API 28)

O host do bundle lê os jogadores vivos (`native_players`) a cada tick; o UUID é a chave da dose, no estado do mundo.

- **Radiação** (`server/radiation_server.rs`): uma vez por segundo, a taxa de cada jogador é a do que ele carrega (`native_player_inventory`, pela tabela de hazards) mais a das zonas de fallout onde está. Passada a carência de 10 s ao entrar, a dose cresce pelo que ele carrega (a das zonas vem das bombas) e a doença age pelas faixas (`radiation::sickness`): efeitos de 5 s (`native_effect`), dano de magia acima de 800 RAD e morte em 1000 RAD, quando a dose recomeça (`native_hurt`). Criativo e espectador não recebem dose nem doença. O cliente lê `ntm_rad_dose` (mRAD) e `ntm_rad_rate` (mRAD/s) por `native_set_player_value`, mandados só quando mudam: é o contrato com o lado do cliente.
- **Dano das bombas:** a carga fere pelo que a explosão vê do jogador (exposição vanilla), e as nukes por pulsos enquanto cavam a cratera, bloqueados por obstáculos (`native_hurt`, `minecraft:explosion`).
- **Detonador** (`ntm:detonator`): usado numa bomba, liga (hook `use_item_on`); usado em outro bloco ou no ar (hook `use_item`), detona a bomba ligada ou acende o pavio de uma carga. No NTM a ligação é com o jogador agachado; o hook com item na mão não diz se ele agacha. O texto do detonador vai ao log: o servidor não tem helper para o chat do jogador.

**O que o upstream tem para as lacunas** (conferido em 30/10):

| Lacuna | Upstream | Estado |
| --- | --- | --- |
| Partículas das bombas | `GameLevel::native_send_particles` (#4390, contraparte de `ServerLevel.sendParticles`) | Em uso: o pavio solta fumaça; a carga, o `explosion_emitter`; a nuke, o clarão, o emissor e o cogumelo (`blast::visuals`, `ServerHost::show`). Só na dimensão em tick. |
| Sons das bombas | `GameLevel::native_play_sound` (#4574, issue #4400) | Em uso desde a `ntm-next` `908d043ef` (API 32, #4574 e #4595 fixados pelo fbef4942): estouro, trovão da nuke e pavio, cada um com a sua fonte vanilla. Em jogo, o motor aceitou todos; ouvir depende de um dispositivo de áudio, que o ambiente dos agentes não tem. |
| Texto ao próprio jogador (detonador) | Nada. `native_tell` só alcança convidados LAN; `native_chat` é do cliente. | Lacuna; pedir ao upstream. |
| Agachar com item na mão (ligar o detonador) | Nada. `use_block` traz `sneaking` só de mãos vazias. | Lacuna; pedir ao upstream. |

Prova em jogo (`tools/test-game/native_l4.py`, mundo novo):

- **radiacao:** em sobrevivência com 64 lingotes de urânio (22,4 RAD/s), a dose ficou em 0 na carência e chegou a 470,4 RAD depois de ~21 s, guardada em `<mundo>/data/ntm/world`. O jogador ficou com náusea, fraqueza II e lentidão, a faixa de 400 RAD.
- **detonador:** uma carga `ntm:tnt_ntm` posta por `/setblock`: o detonador nela a ligou, o clique seguinte acendeu o pavio, e a carga explodiu (força 10, 63 blocos), deixando a cratera.

## Candidato de instalação lado a lado (I1, 30/10)

O binário vem do `tools/build-native.sh` na `ntm-next` `29b4726b4`, com o bundle do HEAD. O `ntm.hmod` sai do `tools/build-content.sh` com o `cargo-horizon-mod` do mesmo motor (o crate está no stage do build: `cargo build --release -p cargo-horizon-mod`). Os dois vão para `horizon-patches/install-next.sh` e `update-horizon-ntm-next.sh` (`HORIZON_NEXT_BUILD`, `NTM_HMOD`).

- **Conteúdo ligado:** um mod em `horizon-mods/` só carrega com `mod.<id>:true` no `horizon-mods.txt` (Options > Mods); o `--dev-mod` dos roteiros sempre liga. Os dois scripts agora garantem `mod.ntm:true`; sem isso, uma instalação a partir de uma antiga sem esse arquivo abria sem os blocos do NTM.
- **Prova** (HOME temporária, a antiga instalação falsa sem `horizon-mods.txt`): simulação e `--apply` dos dois scripts. Depois, o jogo aberto pelo próprio lançador instalado, num mundo novo. Passaram:
  - a prensa (8 chapas, matriz com desgaste 8);
  - a rede de energia (4 lingotes no baú);
  - a de fluido (1000 mB no barril B, dutos tipados pelo identificador);
  - a lista de receitas (escolha mandada ao fechar, 4 chapas);
  - a radiação (627 RAD, os efeitos da faixa de 600 e o dano);
  - o detonador (ligou, acendeu o pavio, cratera).

  Zero pânicos; jogo fechado.

## A alavanca das turbinas (30/10, `ntm-next` `c6ad4a73e`, API 24)

O hook `use_block` leva o clique direito numa célula de máquina ao `Server::native_use` (célula clicada e núcleo, da V4). Nas células de alavanca do NTM, ele troca o vapor com a turbina parada e consome o clique, então nenhum menu abre. Em jogo, uma turbina industrial posta pelo jogador (63 células): o clique na célula `(0, 1, −3)` do referencial de quem pôs passou o vapor de `steam` a `hotsteam` e o tanque de 750.000 a 75.000. Foi uma troca só por clique; um clique em outra célula não mudou nada.

## O duto pelo identificador de fluido (30/10)

No NTM, o duto não tem tela: o tipo vem do identificador de fluido na mão (`FluidDuctBase.handleActivation`). O conteúdo tem o `ntm:fluid_identifier_multi`, com o tipo em `custom_data` `{ntm_fluid:"<fluido>"}` (`bec9b460`). O hook `use_item_on` (o clique com item num bloco) lê esse tipo e o `Server::native_use_item` o põe no duto. Em jogo, um duto posto por `/setblock` passou de sem tipo a `water` com um clique do identificador. Ainda faltam: o clique agachado que muda os dutos ligados (`changeTypeRecursively`), o ALT que copia o tipo do duto para o identificador, a GUI de escolha do identificador e o slot dele no barril.

## Redstone (30/10, `ntm-next` `15d31b261`, API 25)

O host do bundle lê o sinal com `native_redstone_many`, uma chamada por dimensão para todas as baterias e bombas carregadas (`Server::native_redstone`). A bateria usa o modo redHigh com sinal e o redLow sem (as duas do `GUIMachineBattery`). A bomba usa o sinal no lugar do `lit` da lâmpada: as nukes da C2 estão em beehive e não têm `lit`; as 4 cargas simples seguem na lâmpada e leem igual. Em jogo, uma bateria posta por `/setblock` encheu com a bateria criativa vinda de um funil (dados `[1.000.000, 1.000.000, 1, 2, 1]`) e seguiu sem pânico com um bloco de redstone ao lado. O efeito do redHigh se vê pela tela nativa, que depende da L2.

## As redes em jogo (30/10, `tools/test-game/native_redes.py`)

- **energia:** uma bateria em buffer, enchida pela bateria criativa de um funil, três `red_cable` e uma fornalha elétrica sem bateria, com minério de um funil e um baú embaixo. A fornalha ficou com a energia cheia e mandou os 4 lingotes ao baú só com a energia da rede.
- **fluido:** dois barris ligados por três dutos. Os dutos ganharam o tipo `water` pelo identificador de fluido na mão; o balde de água entrou no barril A pela tela nativa (slot 302); o botão de modo, clicado por pixel (a origem é o slot 0 da barra menos a sua posição no `horizon_machine`), levou A a saída; os 1.000 mB foram ao barril B.
- O botão só chegou à lógica depois do `7b56d7d7`: o servidor nativo conferia o alcance pela posição do jogador da sessão WASM, que no nativo não existe; o motor já valida o alcance do menu.

## A lista de receitas (L2, 30/10, `ntm-next-fixes` `b9d905189`, API 31)

Substitui a tela de seleção que era WASM. A montadora, a planta química, o PUREX, o misturador e as bigornas (as que `ntm_core::native::recipes` lista) abrem a lista pelo botão 0 da sua tela: a janela de 176×132 do `GUIScreenRecipeSelector` (`ntm:gui/processing/gui_recipe_selector`), grade 8×5 de células de 18 px a partir de (7, 17), rolagem de uma linha, busca de até 32 caracteres, e a escolha atual no quadro da direita.

| Camada | O que faz |
| --- | --- |
| Motor (API 31) | `native_screen_event` (clique, tecla, texto e roda, com `claimed`), `native_screen_set_layout` (com `hide_inventory`) e `native_screen_send_button`. Não sabe o que é receita. |
| `ntm_core::screens::list` | O estado puro: filtro (sem diferenciar maiúsculas, pelo nome), vistas que se sobrepõem, campo de busca com caret, as teclas do `keyTyped` do NTM (Enter alterna o foco; com o campo focado ele leva as letras, E inclusive; setas, Page, Home e End movem a vista; Escape e a tecla do inventário fecham), a roda, os botões subir, descer, limpar busca, fechar e a moldura da escolha. Só uma escolha **que mudou** vira botão. |
| Bundle (`screens.rs`, `screen_event.rs`) | Desenha (textura, brilho dos botões sob o ponteiro, ícones, caret) e dá as dicas (nome, entradas e saídas); ao abrir, troca o layout (sem slots e sem o inventário); ao fechar, manda `10000 + índice` ou `9999` e devolve o layout da máquina. |

O botão 0 da máquina só abre a lista quando a máquina tem `recipes()`; qualquer outro botão segue o caminho de antes. A busca, a página e a escolha provisória ficam no cliente; o servidor só ouve o botão. O estado da lista recomeça em cada abertura da tela (`native_screen`). Se a tela for fechada por outro caminho com a lista aberta (o jogador se afasta, morre), a escolha provisória se perde.

Prova em jogo (`tools/test-game/native_recipes.py`, ver `docs/native-game-tests.md`).

**Lacunas da lista:**

- Fechar a tela por outro caminho com a lista aberta (o jogador se afasta, morre, o servidor fecha) perde a escolha provisória; o NTM a manda no `onGuiClosed`.
- A busca não tem seleção de texto nem ctrl+A/C/V (o `GuiTextField` do NTM tem); só inserir, Backspace, Delete, Left, Right, Home e End.
- As dicas dos botões estão em inglês, como no NTM; as dos itens usam os nomes da lang do conteúdo.

## O cliente da radiação e o relayout das telas (L4, lado do cliente, 30/10, API 31)

| O quê | Onde | Como |
| --- | --- | --- |
| Caixa do Geiger | `client.rs` (`hud`), hook `hud` | No canto superior esquerdo, `NTM Geiger \| x RAD/s` e `Dose: y RAD`, na cor da dose (verde, amarelo, laranja, vermelho, os limiares de `Reading::color`). Lê os valores i64 do jogador (`ntm_core::client_rad::KEY_DOSE` e `KEY_RATE`, em milésimos de RAD e de RAD/s) com `native_player_value`; sem dose enviada, nada aparece. |
| Dica de item | `client.rs` (`tooltip`), hook `tooltip` | Como `HazardTypeRadiation.addHazardInformation`: `[Radioactive]` em verde, `0.175RAD/s` em amarelo e, com mais de um item, `Stack: 11.2RAD/s`. A taxa por item é `radiation_millirad_s` da tabela de hazards (`client_rad::item_rate`). |
| Troca de tela | `screens.rs` (`switch_screen`) | O botão que mostra a outra tela de uma máquina (o eletrolisador: fluidos e metal) agora recoloca os slots na hora, com `native_screen_set_layout`: cada tela põe os slots onde a sua textura os tem e esconde os que não usa. |

O servidor (870f7ba7) grava os valores por jogador com `native_set_player_value`: `ntm_rad_dose` (dose em milésimos de RAD) e `ntm_rad_rate` (milésimos de RAD/s no ponto do jogador, com blindagem), os dois i64, só quando mudam. O servidor publica os dois valores a cada segundo, só quando mudam (`radiation_server.rs`); com 64 tarugos de urânio a caixa mostra 11,2 RAD/s em jogo.

Prova em jogo: `tools/test-game/native_client.py` (ver `docs/native-game-tests.md`).
