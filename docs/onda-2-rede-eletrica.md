# Onda 2 — rede elétrica: chaves, longa distância e armazenamento

Branch `onda2-k3-c`. Os 14 blocos da família `PowerNetMK2` que seguem `Kind::Inert` (`machine_battery_socket`,
`machine_battery_redd`, `machine_transformer`, `cable_switch`, `cable_detector`, `red_connector`, `red_connector_super`,
`red_pylon`, `red_pylon_steel`, `red_pylon_medium_wood`, `red_pylon_medium_steel` e as duas variantes `_transformer`,
`red_pylon_large`). As regras puras (`crates/ntm-core/src/energy_network.rs`) já existiam e batem com o Java bit a bit
(`tools/energy-net-oracle`); esta frente liga cada bloco ao servidor, à tela, ao save e ao jogo.

| # | Bloco | Estado |
| --- | --- | --- |
| 1 | `cable_switch`, `cable_detector` | Concluída |
| 2 | `red_connector`, `red_connector_super`, `red_pylon`, `red_pylon_steel`, `red_pylon_medium_*` (4), `red_pylon_large` | Concluída (lacunas de orientação e de fio visível, abaixo) |
| 3 | `machine_battery_socket`, `machine_battery_redd` | Concluída (lacunas abaixo; o catálogo cobre as 12 baterias e a criativa) |
| — | `machine_transformer` | Sem lógica a portar: `MachineTransformer extends Block`, sem `TileEntity` |

---

## 1. `ntm:cable_switch` e `ntm:cable_detector`

### Contrato fonte

- `TileEntityCableSwitch.java:5-21` (um tile para os dois blocos): estende `TileEntityCableBaseNT`; `shouldCreateNode()` é
  `getBlockMetadata() == 1`. Ligado, é um cabo (nó da rede de energia, aberto nas seis faces); desligado, `updateState()` destrói o
  nó e a rede se divide ali.
- `CableSwitch.java:43-64` (`onBlockActivated`): clique direito **sem agachar** alterna `meta 0 ↔ 1`, toca `hbm:block.reactorStart`
  (tom `1,0` ao ligar, `0,85` ao desligar) e chama `updateState()`; agachado devolve `false`. Só o lado do servidor age.
- `CableDetector.java:43-65` (`onNeighborBlockChange`): apesar do nome **não mede energia**; lê
  `isBlockIndirectlyGettingPowered(x, y, z)` e, quando o sinal sobe com `meta 0` ou desce com `meta 1`, muda o `meta`, toca o mesmo
  som e chama `updateState()`. Não tem `onBlockActivated`: clicar nele não faz nada. A mudança só acontece quando **um vizinho muda**
  (a colocação do próprio bloco não chama `onNeighborBlockChange` nele).
- Regras puras já provadas contra o Java: `switch_toggled`, `detector_meta_after`, `switch_joins_net` (`energy_network.rs`).

### Implementação

- [`crates/ntm-core/src/cable_switch.rs`](../crates/ntm-core/src/cable_switch.rs): `CableSwitch { detector, on, seen }` (`Kind::CableSwitch`,
  `Data::CableSwitch`). O `meta` do Java é o `on`, salvo no próprio block entity (o `honey_level` do bloco só o mostra: `5` ligado, `0`
  desligado, a textura `front_on`).
- Rede: `server.rs::build_topology` põe um interruptor **ligado** no conjunto de cabos (seis faces) e ignora um desligado; as
  mudanças marcam a topologia como suja e as redes são refeitas no tick seguinte.
- Clique: `native_server.rs::native_switch_use` (chamado por `glue.rs::use_block` só quando **não** se está agachado) alterna o
  interruptor; o detector devolve `false`.
- Redstone: `native_redstone` amostra o sinal no bloco do detector a cada tick; como o Java age quando um vizinho muda, o detector age
  **na mudança** do sinal: a primeira leitura (e a primeira depois de recarregar o mundo) só guarda o nível, então um detector colocado
  ao lado de um bloco já energizado fica desligado até o sinal mudar, como no Java.
- Save: o estado vai no block entity pelo caminho atômico comum (`native_save`); nenhum campo novo no save do mundo.

### Provas

- **Testes (`ntm-core`):** `cable_switch::tests` (6) e `server::tests::cable_switch_tests` (4): um cabo–interruptor–cabo entre duas baterias
  só conduz ligado (energia medida), o clique alterna bloco e máquina, o detector segue o sinal só nas mudanças e não aceita clique, o
  interruptor ignora redstone, quebrar leva a rede junto, os dois nascem de `Kind::Inert`. `cargo test -p ntm-core --lib`:
  799 passaram (eram 784; +15: 6 + 4 do interruptor, 2 da ventilação, 2 da entrada de ar, 1 do barril de 0 PU); `energy_network_tests` + `ledger_persistence` 24 + 6; `cargo clippy --workspace --all-targets -- -D warnings` limpo.
- **Mutantes (`/mnt/ssd120/sacer/so/c/mutate_cable_switch.py`):** **15/15 mortos** (clique, detector com clique, interruptor com redstone,
  primeira leitura, `joins_net`, `level`, `meta`, topologia suja no clique e na redstone, interruptor desligado conduzindo, amostragem do
  detector, limiar do sinal, registro dos dois blocos, `detector` por bloco).
- **Cliente nativo e pacote:** `horizon-client` release SHA-256 `21490d563f97a2612faa63da68dd69de8ae86eb5b6b320590acf1e88b3b863d2`;
  `dist/ntm.hmod` SHA-256 `cb8fe1ef35d399c17251f0eb65c92ff4431f526d1138f8188a57d662d98b5ac8` (sem conteúdo novo nesta etapa).
  (O binário foi construído antes do commit `119b7cb0`, o conserto da ventilação da pile, que não toca estes blocos.)
- **Prova em jogo + corte de gravação estrito (`HORIZON_NO_EAGER_SAVE=1`, `/mnt/ssd120/sacer/so/c/run_cable_switch_proof.py`):**
  - Duas linhas ao longo de +X: bateria de origem (carga finita: a bateria criativa que a enche é retirada do slot; bloco de redstone sob
    ela, modo saída) – 2 cabos – interruptor – 2 cabos – bateria de destino. Linha 0 com `cable_switch` (clique real), linha 1 com
    `cable_detector` (bloco de redstone por cima).
  - Gameplay: **16/16 checks `ok: true`**. Desligado nada atravessa (`0, 0`); o clique liga bloco **e** máquina (`honey_level=5`) e a energia
    atravessa e é **conservada exatamente** (origem perde `68 306 HE`, destino ganha `68 306 HE`, os dois lidos com o jogo congelado); o
    clique **agachado não vale**; o segundo clique desliga e nada atravessa; o detector **não aceita clique**; o bloco de redstone o liga,
    a energia atravessa e é conservada (`71 638 = 71 638`); sem redstone ele desliga; quebrar o interruptor ligado em survival remove o
    bloco e a energia para.
  - Corte de gravação: **7/7 pontos `ok: true`** (`none`, `kill`, `kv_committed`, `save_begin`, `chunks_written`, `entities_written`,
    `world_saved`), `kv_items=0`, `.mca` inalterados antes da escrita. Base salva com os dois desligados; no ato o interruptor é clicado e
    o detector energizado (ambos ligados, energia fluindo). Antes da escrita as duas reaberturas voltam ao último save: **bloco e máquina
    desligados e nada atravessa** (nunca bloco ligado com máquina desligada, nem o contrário); depois da escrita as duas reaberturas
    mantêm **ligados, bloco e máquina concordando, e a energia segue atravessando** (`68 306 = 68 306` em 20 ticks).
  - Histórico honesto: o roteiro falhou três vezes por erros dele (o clique agachado precisa de `key shift down` e não de `--sneak`; a
    origem e o destino eram lidos em instantes diferentes, e agora o jogo é congelado na leitura; e a conservação só valia depois disso).

### Lacunas nominais

- **Som `hbm:block.reactorStart`:** o catálogo o exclui por licença (`crates/ntm-core/data/sound-events.json`, `excluded_by_license`), então
  ligar e desligar são silenciosos; o tom `1,0`/`0,85` fica registrado aqui.
- **Detector: mudança de vizinho que não é o sinal.** O Java chama `onNeighborBlockChange` com qualquer vizinho que mude (um bloco posto ao
  lado, por exemplo) e compara o `meta` com o sinal **naquele momento**; o porto age só quando o sinal muda. A diferença aparece só quando
  um detector que ficou desligado ao lado de um bloco energizado (colocado assim) recebe depois uma mudança de outro vizinho: no Java liga,
  no porto continua desligado até o sinal mudar.
- **Aparência:** o Java mostra o ícone ligado/desligado em todas as faces; o porto mostra `front_on` só na face da frente (`facing`), como
  as demais caixas da família `beehive`.
- **Pintura, disfarce, calibre e caixas** dos cabos do catálogo (`red_cable_paintable`, `red_cable_gauge`, `red_cable_box`) não pertencem a
  este item.

---

## 2. Conectores e pylons (os 9 pontos de fio) e o carretel de cabo

### Contrato fonte

- `TileEntityPylonBase.java`: o nó (`createNode`) é a própria célula mais os vizinhos que cada classe lista; as ligações por fio ficam em `connected`
  (`addConnection` **acrescenta sem checar repetição**) e no outro ponto; `canConnect(first, second)` (`:25-44`): tipos de ligação iguais
  (`SINGLE`/`TRIPLE`/`QUAD`), pontos distintos e distância entre os pontos de montagem `<=` o menor alcance; `disconnectAll` (ao quebrar) tira o
  ponto das listas dos outros. Alcances: conector 10, conector super 100, pylon 25, médios 45, grande 100.
- `TileEntityConnector.java`/`Super`: um bloco só, `meta` = a face em que foi posto (`onBlockPlaced` devolve `side`), aberto só atrás
  (`canConnect`: `getOrientation(meta).getOpposite() == dir`); `TileEntityPylon.java`: as quatro faces horizontais; `PylonMedium`: aberto só pela
  face do transformador, se tiver; `PylonLarge`: sem faces, só fio.
- `ItemWiring.onItemUse` (`ItemWiring.java:19-84`), **sem agachar**: o primeiro clique num ponto (o núcleo, achado a partir de qualquer célula do
  multibloco) guarda `x, y, z` no item e diz "Wire start"; o segundo liga se `canConnect == 0` ("Wire end") ou diz o motivo ("not the same type",
  "Cannot connect to the same pylon", "too far away"); em qualquer desfecho o item esquece o ponto, e **não é consumido**. Se o primeiro ponto
  deixou de ser um ponto de fio: "Wire error".
- Blocos: `PylonRedWire {4,0,0,0,0,0}`, `PylonMedium {6,0,0,0,0,0}`, `PylonLarge {13,0,1,1,1,1}` (`getOffset() == 0`); quebrar qualquer célula chama
  `disconnectAll`.
- Regras puras (`energy_network.rs`: `WireKind`, `can_connect_wires`, `node`) já estavam provadas contra o Java bit a bit.

### Implementação

- [`crates/ntm-core/src/wire.rs`](../crates/ntm-core/src/wire.rs) (`Kind::Wire`, `Data::Wire { connected, color }`): as faces de cada ponto saem de
  `WireKind::node` e viram a porta única do núcleo (`registry::ports`). O `facing` do bloco é o `meta` do Java.
- Topologia: `Topology::build_linked` (`energy.rs`) junta, a qualquer distância, os pontos de fio que **se listam mutuamente**
  (`UniNodespace.checkConnection`); `server.rs::build_topology` monta as ligações e um passo por tick (`simulate`) tira da lista de um ponto o
  outro que sumiu de uma célula carregada e não é mais ponto de fio (`disconnectAll`, qualquer que seja o jeito de o ponto sair).
- Carretel: `native_server.rs::native_wire_use` (regras do Java, mensagens em inglês iguais) e `glue.rs::use_wire` (lê e grava `x, y, z` no
  `custom_data` do item; não age agachado).
- Conteúdo: `tools/visual-gen/visual.json` → arquivos de máquina com o multibloco (`1×5×1`, `1×7×1`, `3×14×3`) e os modelos OBJ dos 7 pylons
  (`pylon.obj` com a peça `Pylon` ou `Pylon_steel`, `pylon_medium.obj` com ou sem `Transformer`, `pylon_large.obj`) com as texturas originais.

### Provas

- **Testes (`ntm-core`):** `server::tests::wire_tests` (10) e dois de `energy::tests`: o fio leva a energia só depois do segundo clique e **não perde nada**;
  os três erros dizem o porquê e não deixam ligação; os alcances (conector 10, super 100) e a repetição mantida; quebrar tira o fio do outro lado;
  um fio que só um lado lista não conduz (`checkConnection`); o conector só recebe de trás (5 posições de bateria); as faces de cada um dos 9; um
  fio para um bloco que ainda é ponto é mantido e um ponto num chunk descarregado não é mexido; os 9 nascem de `Kind::Inert` e o save volta igual.
  `cargo test -p ntm-core --lib`: **811 passaram** (eram 799: +12, 10 de `wire_tests` e 2 de `energy::tests`); `energy_network_tests` + `ledger_persistence` 24 + 6; `cargo clippy --workspace --all-targets -- -D warnings` limpo.
- **Mutantes (`/mnt/ssd120/sacer/so/c/mutate_wire.py`):** **19/20 mortos** (20 mutantes: resultado do clique, ligação de um lado só, topologia suja, mensagens, `meta` do conector e do médio, topologia de um lado só, retirada do fio do outro lado, repetição, índice da porta do fio, registro de 3 dos 9 blocos, filtro e lista das faces). O sobrevivente (`W20`, tirar o `loaded` do passo que desfaz fios) é **equivalente neste harness**: a máquina de um chunk descarregado sai do mapa antes do passo.
- **Cliente nativo e pacote:** `horizon-client` release SHA-256 `38b612b216e08d858dcf97845abd1b1d181be1ee3c4f94a5bb056dc33e3f4411`; `dist/ntm.hmod`
  SHA-256 `9dc00df6408acfa8ccd47fb33efc775b95d49118d3f7687cb554cc9aa491fc8b`.
- **Prova em jogo (`/mnt/ssd120/sacer/so/c/run_wire_proof.py`):** dois `red_pylon` postos pelo motor a 20 blocos um do outro, uma bateria finita na origem e outra
  no destino. **16/16 checks `ok: true`**: os pylons nascem como máquinas sem ligações; sem fio nada atravessa; o **primeiro clique real com o carretel
  diz "Wire start" e o segundo "Wire end"**; as duas pontas listam uma à outra; a energia atravessa e é **conservada exatamente** (`68 306` HE saem
  da origem e entram no destino, lidos com o jogo congelado); o mesmo pylon duas vezes dá "Cannot connect to the same pylon" sem ligação nova; um pylon
  a 40 blocos dá "too far away" sem ligação; um pylon médio dá "not the same type"; **o clique agachado não começa fio** e o simples logo depois
  começa; quebrar uma ponta em survival tira o fio da outra e a energia para.
- **Corte de gravação estrito (`HORIZON_NO_EAGER_SAVE=1`):** **7/7 pontos `ok: true`** (`none`, `kill`, `kv_committed`, `save_begin`, `chunks_written`,
  `entities_written`, `world_saved`), `kv_items=0`, `.mca` inalterados antes da escrita. Base salva sem fio; no ato o fio é feito com **cliques reais** e a energia
  flui. Antes da escrita as duas reaberturas voltam ao último save: **nenhum dos dois pylons lista o outro e nada atravessa**; depois da escrita (e em `none`) as duas
  reaberturas mantêm **as duas pontas listando uma à outra** (`[[60,100,12]]` e `[[40,100,12]]`) e a energia segue atravessando e conservada. Em nenhum ponto
  apareceu um fio de um lado só nem energia sem fio.

### Lacunas nominais

- **O fio não é desenhado:** `RenderPylonBase.renderLinesGeneric` traça os cabos entre os pontos de montagem no cliente; o motor não tem um desenho
  de linhas por block entity, então os fios ficam **invisíveis**. A ligação se vê pela mensagem do chat, pela energia e pelo estado da máquina.
  O texto do carretel com as coordenadas ("Wire start x: ...") e o aviso de distância durante o uso também não existem.
- **Conectores só nas quatro paredes:** o `meta` do Java é a face clicada, também cima e baixo; o bloco do porto só tem `facing` horizontal (o
  motor escolhe pela direção do jogador), então um conector posto no chão ou no teto fica de lado, e o modelo extraído (`connector.obj`, girado
  em X) não pôde ser usado: os conectores ficam com o cubo de antes.
- **Pylon grande a 90°:** o Java gira o modelo e os pontos de montagem em passos de 45° (`getMetaForCore` com `rotationYaw * 4 / 180`); o motor só
  tem quatro direções, então o modelo gira em passos de 90° e a distância é medida com o `meta` da direção escolhida.
- **Cor:** `setColor` (a tinta no pylon) e o desenho da cor não foram portados; o campo `color` é guardado e salvo.
- **Substation:** o pylon grande avisa que exige uma subestação (`TileEntitySubstation`), que não é deste item.
- **`IPersistentNBT`/descrição:** o texto de alcance e tipo do `addInformation` não aparece no item.

---

## 3. `ntm:machine_battery_socket` e `ntm:machine_battery_redd`

### Contrato fonte

- `TileEntityBatteryBase.java`: `redLow` (0) e `redHigh` (2), `priority` (LOW), `lastRedstone`; modos 0 entrada, 1 buffer, 2 saída, 3 nenhum; o
  modo em vigor é `redHigh` se **qualquer** célula de `getPortPos` recebe redstone, senão `redLow`; o modo põe a máquina nos conjuntos de provedores e
  receptores da rede; prioridade fora de `LOW..HIGH` volta a `LOW`; `receiveControl` (botões `low`, `high`, `priority`); ambas são **condutoras**
  (`IEnergyConductorMK2`: todas as portas são um nó só); `isItemValidForSlot` = só `IBatteryItem`.
- `TileEntityBatterySocket`: 1 slot; **não guarda energia**, a carga é a da bateria no slot (`ItemBatteryPack` guarda o `charge` no NBT do item e
  `getCharge` cria a tag com 0); velocidades de provisão/recepção = taxas da bateria, só nos modos certos; `delta`/`log` de 20 médias; `canExtractItem`
  (saída com bateria vazia, entrada com bateria cheia); 8 portas (`getConPos`).
- `TileEntityBatteryREDD`: 2 slots (0 alimenta a REDD, 1 é carregado por ela); reserva `BigInteger` sem limite, salva como `toByteArray`; a rede vê
  só uma janela (`getPower` = `min(power, getMaxPower()/2)`, `getMaxPower` = `Long.MAX_VALUE/100`); `transferPower` aceita tudo; a bateria criativa no
  slot 0 soma `Long.MAX_VALUE/100` por tick; `getSpeed` (a roda); 6 portas.
- `MachineBatterySocket {1,0,1,0,1,0}` offset 0 e `MachineBatteryREDD {9,0,2,2,4,4}` offset 2; GUIs 176x181 (`GUIBatterySocket`, `GUIBatteryREDD`).
- As regras puras (`BatterySocket`, `BatteryRedd`, `BatteryBase`, `ItemSlot`, `BigInt`) já estavam em `energy_network.rs`, provadas contra o Java.

### Implementação

- [`crates/ntm-core/src/storage.rs`](../crates/ntm-core/src/storage.rs) (`Kind::Storage`, `Data::Storage`, um corpo `Socket`/`Redd`): monta as regras puras
  sobre os slots do motor. A carga da bateria é o `charge` do `custom_data` do item (`{charge:123l}`); o **vidro do motor só devolvia o `custom_data` de
  alguns itens** (`glue.rs::stack`), as baterias entraram na lista.
- **Cópia própria dos slots:** a rede move energia depois que as máquinas rodaram, fora do passo que grava os slots; entre o passo e o fim do tick a
  verdade é a cópia da máquina (`kept`). **No fim do mesmo tick** (`native_save`, depois da rede) o slot do motor recebe a cópia no mesmo commit do estado
  da máquina, então nenhum jogador vê o slot atrás da rede; o que um jogador (ou um funil) mexe no slot ganha (`sync_in`). A cópia **não** é salva: o
  slot é, no mesmo commit, e é o que a máquina lê ao abrir o mundo (uma bateria trocada por outra do mesmo tipo é a outra, com a carga dela).
- **Condutoras:** `Topology::build_conducting` (`energy.rs`) junta todas as portas da máquina; portas nas células da borda do volume (`registry::ports`,
  `energy_port_cells`): 8 do soquete e 6 da REDD, uma face cada, derivadas das posições do Java (`celula = posição - direção`).
- Redstone: `native_redstone` lê as células de `getPortPos` (as 4 do soquete, as 6 da REDD); qualquer uma com sinal põe `redHigh`.
- Tela: `{a.b}` nos textos e tooltips lê um `long` em dois slots (`screens::format`: `:si`, `:sis`, `:int`), porque o motor só tem `i32`; a barra do
  soquete usa os 52 pixels do Java (`power*52/max`, `2p-1` de 104 para cada pixel sair exato); a REDD mostra a reserva (saturada em `Long.MAX_VALUE` na tela,
  a reserva real é maior) e a roda gira em graus inteiros por tick (`getSpeed` arredondado, dado 7).
- Slots: a lista de itens aceitos é a das 12 baterias e da criativa, 1 por slot; mão sempre retira; funil põe bateria no soquete e não retira (ver lacunas);
  a REDD não tem automação (`getAccessibleSlotsFromSide` vazio).
- Comparador: o valor calculado (`lastRedstone`) é salvo e lido pelo bloco-núcleo.
- Conteúdo: `visual.json` → multibloco `2x2x2` (soquete) e `9x10x5` (REDD), telas com botões de modo e prioridade, modelos OBJ (`battery.obj`: peça `Socket`;
  `fensu2.obj`: `Base`, `Wheel`, `Lights`) com as texturas originais.

### Provas

- **Testes (`ntm-core`):** `storage::tests` (22) e `server::tests::storage_tests` (9), mais o de `screens` para `{a.b}`: geometria das 8 e das 6 portas para os quatro
  sentidos (portas na borda do volume, faces para fora, células de redstone dentro, as oito do Java escritas à mão), os quatro modos e a redstone, velocidades
  das baterias, tag criada em 0 e salva no item, bateria criativa que não se gasta, pixels da barra (0 a 52, exatos), botões de modo e prioridade, prioridade
  inválida volta a `LOW`, reserva `BigInteger` além do `long`, bytes de `toByteArray`, `delta`/`log`, comparador lido antes dos itens, roda,
  cópia dos slots (o slot recebe a cópia no fim do tick, um jogador que mexe vence, o mundo reaberto lê o slot), retirar a bateria à mão com a energia fluindo (rede + inventário conservados), condutora (a energia atravessa em qualquer modo, sem perda),
  carga da bateria exata, redstone em qualquer célula, regras de slot, save e reabertura. `cargo test -p ntm-core --lib`: **842 passaram** (eram 811);
  `cargo clippy --workspace --all-targets -- -D warnings` limpo.
- **Mutantes (`/mnt/ssd120/sacer/so/c/mutate_storage.py`):** **34/35 mortos**. O sobrevivente (`S25`, `>=` no limite do `long` em `saturating_long`) é equivalente: no limite
  o resultado é o mesmo valor. Três sobreviveram na primeira rodada e ganharam teste (`S30` ordem do comparador, `S35` item que não é bateria na tela).
- **Cliente nativo e pacote:** `horizon-client` release SHA-256 `4da54c16d7fa028d8a8c4370362131f2122f6578703ca37bb3069403f63b6f39`; o pacote do conteúdo foi
  reconstruído (`dist/ntm.hmod` do repositório não foi alterado).
- **Prova em jogo (`/mnt/ssd120/sacer/so/c/run_storage_proof.py`):** **soquete** (bateria de origem, soquete, bateria de destino, nas duas portas longas): sem bateria
  conduz sem perda; um lingote de aço é recusado e uma bateria de chumbo entra pela mão e ganha a tag `charge` de 0; no modo entrada carrega e a soma
  origem + destino + bateria **é conservada exatamente** (lida com o jogo congelado, a carga da cópia salva); um cabo em cima não é porta; os botões da tela
  mudam os modos e a prioridade; na saída a bateria dá a carga à rede exatamente; um bloco de redstone numa célula distante põe `redHigh` e tirá-lo
  volta ao `redLow`; a bateria criativa enche o destino e não se gasta. **REDD** (9 de largura, 10 de altura, 6 portas): conduz no modo nenhum; no buffer absorve
  e conserva; a bateria criativa no slot 0 soma `Long.MAX_VALUE/100` por tick, **inteiro e exato**, e a reserva **passa do maior `long`**; o slot 1 carrega uma bateria
  da reserva, e **a reserva perde exatamente o que a bateria ganha**; na saída dá ao destino e perde exatamente isso. Os dois corpos passam todos os checks.
  Um defeito real apareceu aqui e foi corrigido: o vidro do motor só devolvia o `custom_data` de alguns itens (`glue.rs::stack`), então a carga da bateria nunca chegava à máquina.
- **Corte de gravação estrito (`HORIZON_NO_EAGER_SAVE=1`), os dois corpos, 7/7 pontos `ok: true` cada** (`none`, `kill`, `kv_committed`, `save_begin`, `chunks_written`,
  `entities_written`, `world_saved`; na REDD o `kv_committed` falhou uma vez por tempo esgotado do socket de controle com a máquina cheia de outras provas e passou ao ser repetido sozinho),
  `kv_items=0`, `.mca` inalterados antes da escrita. Base salva sem bateria; no ato uma bateria de 9 000 000 HE entra no slot e alimenta a rede. Antes da escrita as duas
  reaberturas voltam ao último save (sem bateria, soma `1 000 000`); depois da escrita (e em `none`) mantêm a bateria **e a soma de toda a energia é exatamente `10 000 000` nas duas
  reaberturas**, mesmo com a energia fluindo. Em nenhum ponto apareceu bateria duplicada, perdida ou energia criada ou destruída.

### Lacunas nominais

- **Prioridade:** guardada, mostrada e salva, **não aplicada**: a rede simplificada (`energy::distribute`) não tem prioridades, como a `Battery` de antes.
- **Bateria desenhada no soquete, suportes, descarga das baterias SC:** o modelo só tem a peça `Socket`; as peças da bateria dentro e do suporte dependem do item
  do slot e o motor não tem esse desenho por block entity. Baterias SC, `battery_spark`/`battery_trixite` e as demais `IBatteryItem` (ferramentas, armaduras) não
  estão no catálogo, então não entram.
- **`IPersistentNBT` da REDD:** quebrar a REDD **perde a reserva** (o Java a guarda no item e a devolve ao pôr o bloco).
- **REDD:** o plasma, os raios, o brilho e o som (`fensuHum`) não são desenhados; a roda gira em graus inteiros por tick; a tela mostra o texto em tamanho normal
  (sem a escala de meia fonte) em formato curto e o número inteiro no tooltip; a cor do delta é fixa.
- **Soquete:** o tooltip da barra aparece sempre (o Java só com bateria); as cores do delta (verde, vermelho, amarelo) não existem; o funil **não retira** a bateria
  (`canExtractItem` depende do modo e da carga, que a regra de slot não vê).
- **Comparador:** só lê o bloco-núcleo, não as células do volume.
