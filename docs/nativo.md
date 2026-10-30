# O NTM como mod nativo (migração para o Horizon atual)

O Horizon atual não roda mais mods WebAssembly. Mod com código é um **bundle
horizon-mixins** compilado junto com o jogo (`docs/native-mods.md` do Horizon).
O plano completo está no artifact `horizon-migracao` do Traycer. Aqui fica o
que o repositório tem dele.

## Onde está

| Caminho | O que é |
| --- | --- |
| `native/ntm/mixins/ntm/horizon-mixin.toml` | O bundle: o crate da lógica e os hooks nomeados |
| `native/ntm/mixins/ntm/*.rs` | Um helper pequeno por hook |
| `native/ntm/compatibility.json` | Os commits fixados: Horizon (`ntm-next`), horizon-mixins 0.6, horizon-mods (Recipes) |
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

1. Exporta o commit de `ntm-next` (`HORIZON_NEXT`, `NEXT_REV`) para `$BUILD_DIR/source`. Nunca usa o checkout em si.
2. Compila o horizon-mixins no commit fixado, uma vez.
3. Junta os bundles em `$BUILD_DIR/mods`: o do NTM, por `package-native.sh`, e o `recipes` do horizon-mods no commit fixado.
4. Roda `horizon-mixins apply` para `$BUILD_DIR/stage`.
5. Roda `cargo build --locked --release -p horizon-client`, o comando do próprio release, sem a feature `mods`.

O binário sai em `$BUILD_DIR/target/release/horizon-client`.

## Hooks em uso

| Hook | Versão | Helper | O que faz |
| --- | --- | --- | --- |
| `world_lifecycle` | 1 | `lifecycle.rs` | Ao abrir um mundo, loga `ntm native: N machine blocks` (a L0). |
| `block_entity` | 1 | `be.rs` | Um bloco de máquina posto recebe o block entity, com o estado novo em JSON sob `ntm` no compound. |
| `block_entity_ticks` | 1 | `ticks.rs` | Os block entities das máquinas fazem tick. |
| `block_entity_container` | 1 | `container.rs` | Os slots, as faces e o menu vanilla de cada máquina. |
| `block_entity_tick` | 1 | `tick.rs` | O tick do servidor: estado, slots e data do menu passam pelo `ntm_core::native`. |
| `block_entity_types` | 1 | `types.rs` | Registra os ids de block entity das máquinas (o próprio id do bloco), iguais em todos os pares. |
| `block_entity_update_tag` | 1 | `update_tag.rs` | Manda aos clientes `data` (IntArray, o vetor do menu) e `facing` (só nos blocos sem a propriedade, as bombas), o contrato com os modelos da V3. |

O tick grava o `data` no compound, porque o `update_tag` só enxerga o compound, e chama `native_block_entity_changed` só quando muda um índice listado em `synced` no `horizon_machine/<bloco>.json` (lido uma vez por bloco com `native_data`). Os outros valores só importam à tela aberta. O hook `slot_filters` (V2, API 20) dá as regras dos slots, de `ntm_core::native::slot_filters`: a fornalha aceita o que funde, uma bateria criativa, e o resultado é só de saída. As regras valem para o menu, os cliques e os funis.

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
- **Máquinas só com a tela antiga:** 12 máquinas registradas não têm `horizon_machine` (bateria, queimador a lenha, duto, barril, tanque, `ntm:teste` e as 5 do lote de vapor; a fornalha elétrica usa o menu de fornalha). Os controles delas eram botões da tela antiga (o modo da bateria, o tipo do duto, despejar no barril), que sai com a camada WASM. Precisam de arquivo de máquina e tela nativa (`horizon_screen` e o intérprete da L2).

**A fornalha no caminho comum** (`native_l1.py` e `native_l3.py montar` no mesmo build): a fornalha fundiu pelos funis como na L1 (5 lingotes em 25 s), e a firebox e a prensa seguiram iguais.

## A alavanca das turbinas (30/10, `ntm-next` `c6ad4a73e`, API 24)

O hook `use_block` leva o clique direito numa célula de máquina ao `Server::native_use` (célula clicada e núcleo, da V4). Nas células de alavanca do NTM, ele troca o vapor com a turbina parada e consome o clique, então nenhum menu abre. Em jogo, uma turbina industrial posta pelo jogador (63 células): o clique na célula `(0, 1, −3)` do referencial de quem pôs passou o vapor de `steam` a `hotsteam` e o tanque de 750.000 a 75.000. Foi uma troca só por clique; um clique em outra célula não mudou nada.

## O duto pelo identificador de fluido (30/10)

No NTM, o duto não tem tela: o tipo vem do identificador de fluido na mão (`FluidDuctBase.handleActivation`). O conteúdo tem o `ntm:fluid_identifier_multi`, com o tipo em `custom_data` `{ntm_fluid:"<fluido>"}` (`bec9b460`). O hook `use_item` lê esse tipo e o `Server::native_use_item` o põe no duto. Em jogo, um duto posto por `/setblock` passou de sem tipo a `water` com um clique do identificador. Ainda faltam: o clique agachado que muda os dutos ligados (`changeTypeRecursively`), o ALT que copia o tipo do duto para o identificador, a GUI de escolha do identificador e o slot dele no barril.

## Redstone (30/10, `ntm-next` `15d31b261`, API 25)

O host do bundle lê o sinal com `native_redstone_many`, uma chamada por dimensão para todas as baterias e bombas carregadas (`Server::native_redstone`). A bateria usa o modo redHigh com sinal e o redLow sem (as duas do `GUIMachineBattery`). A bomba usa o sinal no lugar do `lit` da lâmpada: as nukes da C2 estão em beehive e não têm `lit`; as 4 cargas simples seguem na lâmpada e leem igual. Em jogo, uma bateria posta por `/setblock` encheu com a bateria criativa vinda de um funil (dados `[1.000.000, 1.000.000, 1, 2, 1]`) e seguiu sem pânico com um bloco de redstone ao lado. O efeito do redHigh se vê pela tela nativa, que depende da L2.
