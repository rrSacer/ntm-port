# Onda 2 — rede elétrica: chaves, longa distância e armazenamento

Branch `onda2-k3-c`. Os 14 blocos da família `PowerNetMK2` que seguem `Kind::Inert` (`machine_battery_socket`,
`machine_battery_redd`, `machine_transformer`, `cable_switch`, `cable_detector`, `red_connector`, `red_connector_super`,
`red_pylon`, `red_pylon_steel`, `red_pylon_medium_wood`, `red_pylon_medium_steel` e as duas variantes `_transformer`,
`red_pylon_large`). As regras puras (`crates/ntm-core/src/energy_network.rs`) já existiam e batem com o Java bit a bit
(`tools/energy-net-oracle`); esta frente liga cada bloco ao servidor, à tela, ao save e ao jogo.

| # | Bloco | Estado |
| --- | --- | --- |
| 1 | `cable_switch`, `cable_detector` | Concluída |
| 2 | `red_connector`, `red_connector_super`, `red_pylon`, `red_pylon_steel`, `red_pylon_medium_*`, `red_pylon_large` | Pendente |
| 3 | `machine_battery_socket`, `machine_battery_redd` | Pendente (depende do catálogo de baterias) |
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
