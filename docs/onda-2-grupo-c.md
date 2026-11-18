# Onda 2 — Grupo C: ar, compressores, torres de resfriamento e deutério

Máquinas do grupo C na tabela de registro (`crates/ntm-core/src/registry.rs`, `onda2-k3-c` a partir de `fa3c1df3`):

| # | Bloco (`ntm:*`) | Contrato Java | Estado |
| --- | --- | --- | --- |
| 1 | `ntm:machine_intake` | `MachineIntake`, `TileEntityMachineIntake`, `RenderIntake` | Concluída |
| 2 | `ntm:machine_compressor` | `MachineCompressor`, `TileEntityMachineCompressor`, `RenderCompressor` | Concluída (pressão Opção A) |
| 3 | `ntm:machine_compressor_compact` | `MachineCompressorCompact`, `TileEntityMachineCompressorCompact`, `RenderCompressorCompact` | Concluída (mesma base de pressão do #2) |
| 4 | `ntm:machine_tower_small` | `MachineTowerSmall`, `TileEntityTowerSmall`, `RenderSmallTower` | Concluída |
| 5 | `ntm:machine_tower_large` | `MachineTowerLarge`, `TileEntityTowerLarge`, `RenderLargeTower` | Concluída |
| 6 | `ntm:machine_deuterium_extractor` | `MachineDeuteriumExtractor`, `TileEntityDeuteriumExtractor` | Concluída |

---

## 1. `ntm:machine_intake`

### Contrato fonte

- `MachineIntake.java` (`{0, 0, 1, 0, 1, 0}`, `offset = 0`, 4 células `2×1×2`, sem GUI; só `ILookOverlay`).
- `TileEntityMachineIntake.java`: buffer de energia `2 000 HE` (`maxPower`), consumo `100 HE/t` (`maxPower / 20`) sempre que `power >= 100` (mesmo com o tanque já cheio), tanque de `1 000 mB` de `air` (`compair`), saída pelas portas horizontais `TilePortShapes.solderer` (excluindo cima/baixo), loop sonoro `hbm:block.motor` (`volume = 0.25`, `pitch = 1.0`, `range = 10.0`) enquanto ativa (`on`).
- `RenderIntake.java`: rotação `case 2: 90`, `translate [-0.5, 0.0, 0.5]`, `Fan` girando `-45°/tick` enquanto `on` (`data[0] = 1`).

### Implementação

- Lógica em [`crates/ntm-core/src/intake.rs`](../crates/ntm-core/src/intake.rs) (`Kind::Intake`, `Data::Intake`, `MAX_POWER = 2_000`, `STEP_POWER = 100`, `AIR_CAP = 1_000`).
- Portas horizontais `solderer` (duas faces externas horizontais em cada uma das 4 células, sem cima/baixo e sem condução de energia através da máquina): [`Intake::port_cells`](../crates/ntm-core/src/intake.rs), `registry::ports` / `registry::fluid_ports` (`registry.rs`), `container_bridge::energy_port_cells` / `FLUID_PROXIES` (`container_bridge.rs`) e `fluid_rebuild_step` (`fluid_server.rs`).
- Áudio em `sounds.rs` (`Data::Intake(i) if i.on => sound("hbm:block.motor", 0.25, 1., 10.)`), modelo/animação/multibloco em `tools/visual-gen/visual.json` e artefatos em `content/` + `crates/ntm-core/data/machine_files.json`.
- Testes de servidor em [`crates/ntm-core/src/server/tests/intake_tests.rs`](../crates/ntm-core/src/server/tests/intake_tests.rs).

### Provas

- **Testes unitários e de integração (`ntm-core`):**
  - `cargo test -p ntm-core --lib`: **737 passaram**, 0 falhas, 5 ignorados (inclui 5 testes novos em `intake::tests`, 1 em `sounds::tests` e 3 em `server::tests::intake_tests`).
  - `cargo test -p ntm-core --test energy_network_tests --test ledger_persistence`: **24 + 6 = 30 passaram**, 0 falhas.
  - `cargo clippy --workspace --all-targets -- -D warnings`: limpo (0 avisos).
- **Mutantes (`/mnt/ssd120/sacer/so/c/mutate_intake.py`):** **13/13 mutantes mortos** (`M1_max_power`..`M13_container_bridge_no_fluid_proxy`).
- **Cliente nativo e pacote:**
  - `horizon-client` release SHA-256: `683d1ef547409367a82381e1ffe566280bd1b2fde8b4d12950c801fbc261dd69`
  - `dist/ntm.hmod` SHA-256: `f7acc67b39c33a9e71f0d9b32ebb2c2f1545ce7988b16e765bfcff27553d6b49`
- **Prova em jogo + corte de gravação estrito (`HORIZON_NO_EAGER_SAVE=1`, `/mnt/ssd120/sacer/so/c/run_intake_proof.py`):**
  - Gameplay: **16/16 checks `ok: true`** (`ntm:machine_intake` + 3 `ntm:dummy_machine`, sem GUI em nenhuma das 4 células, cabo vertical ignorado, cabo horizontal alimenta e aciona `data=[1]` + loop `ntm:block.motor` `volume=0.25`, barril horizontal recebe `air` até `16 000 mB`, barris verticais sobre core e proxy permanecem em `0 mB`, desliga ao remover energia, remoção do core limpa todas as 4 células).
  - Corte de gravação (`HORIZON_NO_EAGER_SAVE=1`): **7/7 pontos `ok: true`** (`none`, `kill`, `kv_committed`, `save_begin`, `chunks_written`, `entities_written`, `world_saved`). Nos cortes antes da escrita de chunks (`kill`, `kv_committed`, `save_begin`), bytes `.mca` inalterados e 2 reaberturas voltam ao estado anterior (`[0]`, barril `('', 0)`); nos cortes após escrita (`chunks_written`, `entities_written`, `world_saved`) e em `none`, 2 reaberturas preservam `[1]` e `('air', 16000)`.

### Lacunas nominais

- **`ILookOverlay`:** `MachineIntake.printHook` exibe energia e enchimento do tanque ao olhar para o bloco no 1.7.10; o port nativo não implementa overlay de bloco ao olhar (`ILookOverlay`), mantendo `view()` interno em `Intake`.

---

## 4. `ntm:machine_tower_small`

### Contrato fonte

- `MachineTowerSmall.java` (`{18, 0, 2, 2, 2, 2}`, `offset = 2`, 475 células `5×19×5`, sem GUI; só `ILookOverlay`).
- `TileEntityTowerSmall.java` (estende `TileEntityCondenser`): condensação passiva sem energia elétrica, tanque de entrada `spentsteam` (`1 000 mB`) e saída `water` (`1 000 mB`), conversão 1:1 (`input.mb.min(output.free())`), e portas fluidas somente em `TilePortShapes.auxTower` (`[0, 0, -2] -> -Z`, `[2, 0, 0] -> +X`, `[0, 0, 2] -> +Z`, `[-2, 0, 0] -> -X`, `y = core.y`, sem conexão vertical nem nos demais proxies).
- `RenderSmallTower.java`: rotação fixa `0`, modelo `tower_small.obj` / `tower_small.png`, raio de render `20.0`.

### Implementação

- Lógica em [`crates/ntm-core/src/steam.rs`](../crates/ntm-core/src/steam.rs): `TOWER_SMALL`, `TOWER_SMALL_CAP = 1_000`, `is_tower`, `tower_ports` (`auxTower`), `Condenser::with_caps(powered, cap)`, `Steam::of(TOWER_SMALL)` (`Kind::Steam`, `Ports::None` em [`crates/ntm-core/src/registry.rs`](../crates/ntm-core/src/registry.rs)).
- Portas direcionadas exclusivas de fluido: `registry::fluid_ports` (`vec![vec![]]` no core), `container_bridge::fluid_proxy_cells` (`crates/ntm-core/src/server/container_bridge.rs`) e `fluid_rebuild_step` (`crates/ntm-core/src/server/fluid_server.rs`, inserindo apenas a face horizontal externa de cada uma das 4 células `auxTower`).
- Modelo, multibloco (`5×19×5`, `offset = 2`) e artefatos em `tools/visual-gen/visual.json`, `content/` e `crates/ntm-core/data/machine_files.json`.
- Testes em `steam::tests::tower_small_matches_java_caps_no_power_and_aux_tower_ports` e [`crates/ntm-core/src/server/tests/steam_tests.rs`](../crates/ntm-core/src/server/tests/steam_tests.rs).

### Provas

- **Testes unitários e de integração (`ntm-core`):**
  - `cargo test -p ntm-core --lib`: **739 passaram**, 0 falhas, 5 ignorados.
  - `cargo test -p ntm-core --test energy_network_tests --test ledger_persistence`: **24 + 6 = 30 passaram**, 0 falhas.
  - `cargo clippy --workspace --all-targets -- -D warnings`: limpo (0 avisos).
- **Mutantes (`/mnt/ssd120/sacer/so/c/mutate_tower_small.py`):** **9/9 mutantes mortos** (`M1_tower_small_cap`..`M9_skip_changed_when_turning_off`).
- **Cliente nativo e pacote:**
  - `horizon-client` release SHA-256: `1223f28d163061da33b372ba24beb29cc0f34a58cb774aa008a8116af56b5915`
  - `dist/ntm.hmod` SHA-256: `debf0fbf37a278209b8dc7f627189083551eb5b8ecb0a7dc7e5f105ade7f7abf`
- **Prova em jogo + corte de gravação estrito (`HORIZON_NO_EAGER_SAVE=1`, `/mnt/ssd120/sacer/so/c/run_tower_small_proof.py`):**
  - Gameplay: **11/11 checks `ok: true`** (`475` células colocadas, sem GUI no proxy de porta e no dummy superior, `steam_engine` alimentando `spentsteam` na porta `-Z`, saída exata de `160 mB` de `water` nas 3 outras portas `+X`, `+Z`, `-X`, proxy lateral não-porta permanece `('', 0)`, face vertical sob porta `-Z` permanece `('', 0)`, bateria conectada por `red_cable` em dummy lateral e sob porta `-X` mantém `1 000 000 / 1 000 000 HE`, quebra em survival de um dummy limpa todas as `475` células).
  - Corte de gravação (`HORIZON_NO_EAGER_SAVE=1`): **7/7 pontos `ok: true`** (`none`, `kill`, `kv_committed`, `save_begin`, `chunks_written`, `entities_written`, `world_saved`), `kv_items=0` em todos, `.mca` inalterados em `kill`/`kv_committed`/`save_begin` com 2 reaberturas revertendo para `('', 0)` água / `('steam', 16000)` e 2 reaberturas pós-escrita preservando `('water', 160)` / `('steam', 0)`.

### Lacunas nominais

- **`ILookOverlay`:** `MachineTowerSmall.printHook` exibe enchimento dos tanques ao olhar no 1.7.10; no nativo a máquina segue sem GUI e sem overlay de bloco ao olhar (`Condenser::view()` interno).
- **`COOLING_TOWER_PARTICLES`:** partículas puramente cosméticas em `TileEntityTowerSmall.updateEntity` quando `worldObj.isRemote && waterTimer > 0` não são emitidas no cliente nativo.
---

## 5. `ntm:machine_tower_large`

### Contrato fonte

- `MachineTowerLarge.java` (`{12, 0, 4, 4, 4, 4}`, `offset = 4`, 1 053 células `9×13×9`, sem GUI; só `ILookOverlay`).
- `TileEntityTowerLarge.java` (estende `TileEntityCondenser`): condensação passiva sem energia elétrica, tanque de entrada `spentsteam` (`10 000 mB`) e saída `water` (`10 000 mB`), conversão 1:1 (`input.mb.min(output.free())`), e exatamente 12 portas horizontais externas em `TilePortShapes.bigTower` (`(-3,0,-4)`, `(0,0,-4)`, `(3,0,-4)` em `-Z`; `(4,0,-3)`, `(4,0,0)`, `(4,0,3)` em `+X`; `(-3,0,4)`, `(0,0,4)`, `(3,0,4)` em `+Z`; `(-4,0,-3)`, `(-4,0,0)`, `(-4,0,3)` em `-X`, `y = core.y`, sem conexão vertical nem nos demais proxies ou quinas).
- `RenderLargeTower.java`: rotação fixa `0`, modelo `tower_large.obj` / `tower_large.png`, `getRenderBoundingBox` de `9×13×9` (`radius = 13.5`).

### Implementação

- Lógica em [`crates/ntm-core/src/steam.rs`](../crates/ntm-core/src/steam.rs): `TOWER_LARGE`, `TOWER_LARGE_CAP = 10_000`, `is_tower`, `tower_ports` (`bigTower`, 12 portas), `Steam::of(TOWER_LARGE)` (`Kind::Steam`, `Ports::None` em [`crates/ntm-core/src/registry.rs`](../crates/ntm-core/src/registry.rs)).
- Portas direcionadas exclusivas de fluido reutilizando `registry::fluid_ports`, `container_bridge::fluid_proxy_cells` e `fluid_rebuild_step` (`fluid_server.rs`).
- Modelo, multibloco (`9×13×9`, `offset = 4`) e artefatos em `tools/visual-gen/visual.json`, `content/` e `crates/ntm-core/data/machine_files.json`.
- Testes em `steam::tests::tower_large_matches_java_caps_no_power_and_big_tower_ports` e [`crates/ntm-core/src/server/tests/steam_tests.rs`](../crates/ntm-core/src/server/tests/steam_tests.rs).

### Provas

- **Testes unitários e de integração (`ntm-core`):**
  - `cargo test -p ntm-core --lib`: **741 passaram**, 0 falhas, 5 ignorados.
  - `cargo test -p ntm-core --test energy_network_tests --test ledger_persistence`: **24 + 6 = 30 passaram**, 0 falhas.
  - `cargo clippy --workspace --all-targets -- -D warnings`: limpo (0 avisos).
- **Mutantes (`/mnt/ssd120/sacer/so/c/mutate_tower_large.py`):** **9/9 mutantes mortos** (`M1_tower_large_cap`..`M9_tower_large_powered_in_of`).
- **Cliente nativo e pacote:**
  - `horizon-client` release SHA-256: `6c3337f6a9de1b99977338463e06bda8b2f73b4fa515c92a3e32e2e91394d111`
  - `dist/ntm.hmod` SHA-256: `8e2bbb5771b3f03866f6a60ebd58fc96e09ab9cb31e3852d7867f0a3def77159`
- **Prova em jogo + corte de gravação estrito (`HORIZON_NO_EAGER_SAVE=1`, `/mnt/ssd120/sacer/so/c/run_tower_large_proof.py`):**
  - Gameplay: **12/12 checks `ok: true`** (`1 053` células `9×13×9` colocadas, sem GUI no dummy de porta e no dummy superior, `steam_engine` alimentando `spentsteam` na porta central `-Z`, saída exata de `160 mB` de `water` nas portas fora de eixo e central `+X` `(4,0,-3)`, `+Z` `(-3,0,4)` e `-X` `(-4,0,0)`, dummy lateral não-porta permanece `('', 0)`, quina não-porta permanece `('', 0)`, face vertical sob porta `-Z` permanece `('', 0)`, bateria conectada por `red_cable` em dummy lateral e sob porta `-X` mantém `1 000 000 / 1 000 000 HE`, quebra em survival de um dummy limpa todas as `1 053` células).
  - Corte de gravação (`HORIZON_NO_EAGER_SAVE=1`): **7/7 pontos `ok: true`** (`none`, `kill`, `kv_committed`, `save_begin`, `chunks_written`, `entities_written`, `world_saved`), `kv_items=0` em todos, `.mca` inalterados em `kill`/`kv_committed`/`save_begin` com 2 reaberturas revertendo para `('', 0)` água / `('steam', 16000)` e 2 reaberturas pós-escrita preservando `('water', 160)` / `('steam', 0)`.

### Lacunas nominais

- **`ILookOverlay`:** `MachineTowerLarge.printHook` exibe enchimento dos tanques ao olhar no 1.7.10; no nativo a máquina segue sem GUI e sem overlay de bloco ao olhar (`Condenser::view()` interno).
- **`COOLING_TOWER_PARTICLES`:** partículas puramente cosméticas em `TileEntityTowerLarge.updateEntity` quando `worldObj.isRemote && waterTimer > 0` não são emitidas no cliente nativo.

---

## 6. `ntm:machine_deuterium_extractor`

### Contrato fonte

- `MachineDeuteriumExtractor.java` (`BlockContainer`, `createNewTileEntity` força `meta = 0`: bloco único sem orientação, sem GUI; só `ILookOverlay`; ícones `deuterium_extractor_side` e `deuterium_extractor_top_water` em cima/baixo).
- `TileEntityDeuteriumExtractor.java:20-54`: `10 000 HE` (`getMaxPower`), consumo `getMaxPower() / 20 = 500 HE` por tick ativo, tanque `water` de `1 000 mB`, tanque `heavywater` de `100 mB`. Condição `hasPower() (power >= 500) && hasEnoughWater() (water >= 100) && heavywater.max > heavywater.fill`; `convert = min(min(100, water) / 50, 100 - heavywater)`; `water -= convert * 50` (divide antes de multiplicar), `heavywater += convert`, `power -= 500` (com água `>= 100` e folga `>= 1`, `convert` nunca é `0`).
- Portas `TilePortShapes.around` (as 6 faces: energia, entrada de `water`, saída de `heavywater`).

### Implementação

- Lógica em [`crates/ntm-core/src/deuterium.rs`](../crates/ntm-core/src/deuterium.rs) (`Kind::Deuterium`, `Data::Deuterium`, `MAX_POWER = 10_000`, `STEP_POWER = 500`, `WATER_CAP = 1_000`, `WATER_MIN = 100`, `HEAVYWATER_CAP = 100`, `WATER_PER_HEAVYWATER = 50`); `registry.rs` (`Ports::AllRound`, `fluid_ports` em todas as faces); textura superior `deuterium_extractor_top_water` em `content/`; artefatos em `crates/ntm-core/data/machine_files.json`.
- Testes: unitários em `deuterium::tests` (4) e de servidor em [`crates/ntm-core/src/server/tests/deuterium_tests.rs`](../crates/ntm-core/src/server/tests/deuterium_tests.rs).

### Provas

- **Testes (`ntm-core`):** `cargo test -p ntm-core --lib` **747 passaram**; integração `energy_network_tests` + `ledger_persistence` **30 passaram**; `cargo clippy --workspace --all-targets -- -D warnings` limpo.
- **Mutantes (`/mnt/ssd120/sacer/so/c/mutate_deuterium.py`):** **12/12 mortos** (`M1_max_power`..`M12_missing_fluid_ports`).
- **Cliente nativo e pacote:** `horizon-client` release SHA-256 `080124857ab16937e663dc2880fbf733b83d8010a8c23612e263de7366ca350e`; `dist/ntm.hmod` SHA-256 `374bb1c10bc34f929952f65e28f26164ce4197ebb5de60b811b3803a3a14142e`.
- **Prova em jogo + corte de gravação estrito (`HORIZON_NO_EAGER_SAVE=1`, `/mnt/ssd120/sacer/so/c/run_deuterium_proof.py`):**
  - Gameplay: **10/10 checks `ok: true`** (extrator colocado com tanques `water 1 000` / `heavywater 100`; sem GUI ao interagir; sem energia (bateria em modo entrada) não produz água pesada nem gasta a bateria; com a bateria de teste em modo saída (o bloco de redstone fica sob a **bateria**, não sob o extrator, que não reage a redstone) o extrator liga (`honey_level=5`); `16 000 mB` de água viram exatamente `320 mB` de `heavywater` (`160` ticks × `500 HE` = **80 000 HE** exatos, medidos pela bateria + buffer do extrator); desliga (`honey_level=0`) quando acaba a água e parado não consome; quebra em survival remove o bloco). A bateria criativa que carrega a bateria de teste é retirada do slot antes da medida (senão ela recarregaria a cada tick e esconderia o consumo).
  - Corte de gravação (`HORIZON_NO_EAGER_SAVE=1`): **7/7 pontos `ok: true`** (`none`, `kill`, `kv_committed`, `save_begin`, `chunks_written`, `entities_written`, `world_saved`), `kv_items=0` em todos, `.mca` inalterados em `kill`/`kv_committed`/`save_begin`. Nos cortes antes da escrita as 2 reaberturas voltam ao último save (`heavywater 0`, água `16 000 mB` = barril `15 000` + `1 000` no extrator, conservada); nos cortes após a escrita (e em `none`) as 2 reaberturas preservam `heavywater 320`, água `0`.
  - Histórico honesto da prova: a primeira execução falhou no check de energia (a bateria criativa no slot recarregava a bateria de teste) e a segunda no ponto `kill` (a asserção esperava os `16 000 mB` todos no barril, mas `1 000 mB` já estão no tanque do extrator); os dois erros eram do roteiro, não da máquina, e foram corrigidos (conservação `barril + extrator = 16 000`) antes da execução v3 que passou. Evidência: `/mnt/ssd120/sacer/so/c/deuterium-gameplay.json`.
  - Os testes unitários/integração e os 12 mutantes acima foram rodados no início desta frente (sessão anterior), sobre o mesmo código commitado aqui.

### Lacunas nominais

- **`ILookOverlay`:** `MachineDeuteriumExtractor.printHook` exibe energia e tanques ao olhar no 1.7.10; o port nativo não implementa o overlay de bloco ao olhar (mantém `view()` interno).
- **Orientação:** o Java força `meta = 0`; o bloco nativo guarda `facing` do `place`, mas todas as faces laterais usam a mesma textura, então não há diferença visível.

---

## Pressão de fluidos (base de #2 e #3, decisão do coordenador: Opção A)

### Contrato fonte

- `FluidTank.java:47-56, 261-291`: cada tanque tem `pressure` (0..5 PU, salvo como `short` em `<nome>_p`); `withPressure(p)` **esvazia o tanque** quando a pressão muda; `loadTank` recusa recipientes quando `pressure != 0` (exceto barril infinito).
- `FluidNetMK2.java:29-149` e `IFluidStandard{Sender,Receiver}MK2`: a rede é por tipo de fluido; `transferFluid` roda **uma vez por pressão** (`for p in 0..=5`): só tanques de mesmo tipo **e mesma pressão** se oferecem e recebem. `HIGHEST_VALID_PRESSURE = 5` (`IFluidUserMK2.java:9`).

### Implementação

- `Tank.pressure: u8` (`#[serde(default, skip_serializing_if = zero)]`, `crates/ntm-core/src/fluid.rs`): saves antigos (sem campo) abrem com pressão 0; tanque de pressão 0 grava sem o campo. `Tank::with_pressure` / `set_pressure` esvaziam ao mudar.
- `FluidNode` ganha `fluid_offer_at`, `fluid_demand_at`, `fluid_give_at` e `fluid_take_at`, com padrão "só pressão 0" (as outras 23 implementações continuam intactas). `distribute` roda `distribute_at` para cada pressão 0..=5.
- Só mudam de pressão: o compressor (entrada 0..4, saída p+1 ou da receita) e, nas máquinas que a pedem, a **ventilação da pile** (ar, 1 PU, `pile.rs`) e, no encadeamento do petróleo, o **hidrotratador** (hidrogênio a 1 PU, `withPressure(1)`) e o **destilador a vácuo** (óleo a 2 PU, `withPressure(2)`), `crates/ntm-core/src/processing.rs`.

### Divergências documentadas (deliberadas: aceitar também 0 PU)

A ventilação da pile (`pile.rs`, `TileEntityPileVent.java:28`, ar a 1 PU) **também aceita a pressão do Java e a pressão 0**, pela mesma razão de compatibilidade (mundos e provas que a alimentam de um barril comum); com ela são três as máquinas com este desvio. O hidrotratador e o destilador a vácuo **aceitam a pressão do Java e também a pressão 0**. No Java eles só recebem à pressão deles. A pressão 0 já era aceita antes desta frente (documentada no cabeçalho de `processing.rs`) e **existe somente porque o script protegido `tools/test-game/bedrock.py` (`petroleo_c`) alimenta as duas máquinas com barris de pressão 0** e não pode ser alterado. Uma versão estrita (só a pressão do Java) depende de decisão futura da usuária.

### Provas da base

- Testes de `fluid::tests` (6 novos): pressões diferentes nunca trocam fluido (5 pares), cada nível de pressão é uma rodada própria, pressão acima de 5 nunca é atendida, nós que não conhecem pressão só trocam a 0, `withPressure` esvazia, **compatibilidade do save** (tanque sem `pressure` = 0; pressão 0 grava sem o campo).
- Testes de `processing::tests` (2 novos): hidrogênio aceito a 0 e 1 (não a 2..5), óleo do destilador aceito a 0 e 2 (não a 1, 3, 5); save antigo recebe a pressão do Java no primeiro tick sem perder o conteúdo.

---

## 2. `ntm:machine_compressor` e 3. `ntm:machine_compressor_compact`

Os dois blocos são **a mesma máquina** com dois corpos (`TileEntityMachineCompressorBase` + `TileEntityMachineCompressor` / `TileEntityMachineCompressorCompact`); a lógica, a tela, as receitas e a persistência são um módulo só (`crates/ntm-core/src/compressor.rs`, `Kind::Compressor`, `Data::Compressor`). Cada bloco tem o seu multibloco, as suas portas e o seu modelo, e a sua própria prova em jogo.

### Contrato fonte

- `TileEntityMachineCompressorBase.java:51-114` (`updateEntity`): carrega a bateria do slot 1 (`chargeTEFromItems`), o tipo do tanque de entrada vem do identificador do slot 0 (`tanks[0].setType(0, slots)`), `setupTanks`, upgrades (`checkSlots(slots, 1, 3)`: o slot 1 também é lido), `processTime` e `powerRequirement`, depois `canProcess` → `progress++`, `power -= powerRequirement` **a cada tick de contagem** (não por operação), `progress >= processTime` → `process()`.
- Tempo (`:81-93`): sem receita, `60/20/10` ticks com velocidade 1/2/3 (senão `100`); com receita, `duração / (velocidade + 1)`; depois `/ (overdrive + 1)`; piso de `1`. Consumo: `2 500 / (poder + 1) * (2 * overdrive + 1)` (divisões inteiras). Upgrades válidos (`getValidUpgrades`): velocidade 3, poder 3, overdrive 9.
- `canProcess` (`:142-153`): `power <= powerRequirement` **não basta** (estritamente maior); sem receita exige `1 000 mB` na entrada e `1 000 mB` de folga na saída; com receita, `inputAmount` e `output.fill`.
- `setupTanks` (`:168-177`) e `receiveControl` (`:213-230`): sem receita para (fluido, pressão), a saída tem o mesmo fluido a `pressão + 1`; com receita, o fluido e a pressão da receita. Pressão ou tipo novo **esvaziam** o tanque (`withPressure`, `setTankType`). `compression` só age se for diferente da pressão atual.
- `CompressorRecipes.java:25-31`: `petroleum@0 → 2 000 mB petroleum@1 (20 ticks)`; `petroleum@1 → 1 000 mB lpg@0 (20)`; `blood@3 → 250 mB heavyoil@0 (200)`; `perfluoromethyl@0 → 1 000 mB perfluoromethyl@1 (50)`; `perfluoromethyl@1 → 1 000 mB perfluoromethyl_cold@0 (50)`.
- GUI (`ContainerCompressor`, `GUICompressor`): 4 slots — identificador `(17,72)`, bateria `(152,72)`, upgrades `(52,72)` e `(70,72)` —, 5 botões `43 + j*11` (`j PU -> j+1 PU`, `compression = j`), marcador da pressão, seta de progresso `progress * 55 / processTime`, lâmpada de energia (`power >= powerRequirement`). `Slot` simples: a mão põe qualquer item; `isItemValidForSlot` é `false` e `getAccessibleSlotsFromSide` é vazio (`TileEntityMachineBase`): **funis não inserem nem tiram**.
- `MachineCompressor.java` (`{2,0,1,2,1,1}` + `{3,-3,1,1,1,1}` + `{8,-4,0,0,1,1}`, `offset = 2`): 60 células (`3×4×3` embaixo, `3×3` em cima, coluna `3×5`). Portas (`getPorts`, `getAllPorts`): `(0,0,+1)` para trás e `(±1,0,0)` para os lados, energia e fluido nas mesmas três células, face para fora.
- `MachineCompressorCompact.java` (`{2,0,1,1,3,3}`, `offset = 1`, `TilePortShapes.condenser`): 63 células (`7×3×3`), seis portas na segunda camada: as duas pontas `(±3,1,0)` e as quatro dos lados `(±1,1,±1)`.
- `RenderCompressor.java`: rotação por `meta` (`2: 90`), `Fan` 15°/tick no eixo X em `y = 1.5`, `Pump` desce `3 - lift*3` blocos (`lift` 0→1 a `0,05`/tick e volta a `~0,1`/tick), som `hbm:item.boltgun` no fim da descida; `RenderCompressorCompact.java`: estático.

### Implementação

- `crates/ntm-core/src/compressor.rs`: `Compressor` (`Shape::Tall` / `Shape::Compact`), `recipe()`, `step()` na ordem exata do Java, `set_compression()`, `port_cells()` (as 3/6 células com a face de saída), `screen_data()` (12 valores), `Slotted::button` (ids 0..4). Os tanques usam `Tank.pressure` e a rede só liga tanques de mesma pressão (seção da pressão acima).
- Fiação: `machine.rs` (`Kind::Compressor`, `Data::Compressor`), `registry.rs` (`ports` e `fluid_ports`: o núcleo não tem porta, as portas são as células do volume), `container_bridge.rs` (`Slotted`, `Layout::Native`, `energy_port_cells`, `fluid_proxy_cells`), `fluid_server.rs` (faces das portas), `native.rs` (`block_slots`: shift-click do identificador/bateria; `slot_allows`: sem automação).
- Conteúdo: `tools/visual-gen/visual.json` → `content/` (arquivo de máquina, tela com os 5 botões e o marcador, modelos OBJ, animações do ventilador e do pistão) e `crates/ntm-core/data/machine_files.json`.
- Testes: `compressor::tests` (24), `fluid::tests` (6 novos), `processing::tests` (2 novos) e [`server/tests/compressor_tests.rs`](../crates/ntm-core/src/server/tests/compressor_tests.rs) (5).

### Provas

- **Testes (`ntm-core`):** `cargo test -p ntm-core --lib`: **784 passaram**, 0 falhas, 5 ignorados (eram 747: +37 desta frente — 24 de `compressor::tests`, 5 de `server::tests::compressor_tests`, 6 de `fluid::tests` e 2 de `processing::tests`); `energy_network_tests` + `ledger_persistence`: **24 + 6 = 30 passaram**; `cargo clippy --workspace --all-targets -- -D warnings`: limpo.
  - Cobertura dos critérios do coordenador: pressão na rede com mistura recusada e entrega só com pressão igual (`fluid::tests`, `compressor::tests::it_takes_and_gives_fluid_only_at_the_tanks_own_pressure`, `it_runs_through_the_network_only_at_equal_pressures`, `compressor_tests::a_compressor_raises_air_to_one_pu_and_only_a_tank_at_one_pu_takes_it`); receitas dos compressores (`the_recipes_are_the_five_of_the_ntm_and_nothing_else`, `a_recipe_sets_the_time_the_input_and_the_output`); retrocompatibilidade do save (`fluid::tests::a_tank_without_a_pressure_field_is_at_zero`, `compressor::tests::the_state_survives_a_save_and_an_old_one_opens`, `processing::tests::an_old_save_gets_the_pressure_without_losing_its_content`).
- **Mutantes (`/mnt/ssd120/sacer/so/c/mutate_compressor.py`):** **43/44 mortos** (36 do módulo do compressor `M01..M36`, 4 da rede `F01..F04`, 4 do hidrotratador/destilador `P01..P04`). O sobrevivente é **equivalente**: `F04` só troca a ordem de um retorno antecipado (`total_offer == 0`) em `distribute_at`, sem efeito observável (só custo). Rodadas anteriores que sobreviveram (`M21` piso do tempo em 1 tick, `M32` mesma compressão sem efeito, `F03` padrão só a 0 PU) ganharam testes e agora morrem.
- **Cliente nativo e pacote:** `horizon-client` release SHA-256 `8dba2d0fc4714cb0421bcc792fbde32213234c4c375c6b1d1dc01917e948a58f`; `dist/ntm.hmod` SHA-256 `cb8fe1ef35d399c17251f0eb65c92ff4431f526d1138f8188a57d662d98b5ac8`.
- **#2 `ntm:machine_compressor` (`/mnt/ssd120/sacer/so/c/run_compressor_proof.py --body tall`):**
  - Gameplay: **14 checks `ok: true`**. Cadeia `barril de ar (16 000 mB, 0 PU) → compressor A → compressor B → (barril de 0 PU na mesma rede)`: A com identificador `air` (slot 0) e bateria criativa colocada **pela tela** (slot 1), tela aberta por um dummy; B com a compressão **1 PU escolhida pelo botão da tela**, alimentado por duas baterias num cabo na porta `(1,0,0)` (uma bateria dá `max/600 = 1 666 HE` por tick e o compressor pede 2 500). Depois de 300 ticks: A eleva o ar **0 → 1 PU** (`a_out` a 1 PU), B recebe o ar a 1 PU e produz ar a **2 PU** (`b_out` a 2 PU), o **barril de pressão 0 na mesma rede não recebe nada** do ar pressurizado, o ar total é conservado (`12 000` em A + `1 000` em B + `3 000` produzidos por B = `16 000`), B pagou `2 500 HE` por tick de contagem (`910 000` HE, múltiplo exato), e a quebra em survival de um dummy remove o volume inteiro (`60` células, incluindo o topo `y + 8`). O GUI tem 5 botões `0..4`.
  - Corte de gravação (`HORIZON_NO_EAGER_SAVE=1`): **7/7 pontos `ok: true`** (`none`, `kill`, `kv_committed`, `save_begin`, `chunks_written`, `entities_written`, `world_saved`), `kv_items=0`, `.mca` inalterados nos pontos antes da escrita; em todas as 2 reaberturas o ar é **conservado exatamente** (`barril + A + B = 16 000`), os tipos e as pressões `0 / 1 / 2` voltam, o barril de 0 PU segue vazio; antes da escrita volta o último save (o trabalho feito depois dele sai, sem duplicar) e depois dela o estado salvo não perde operações.
- **#3 `ntm:machine_compressor_compact` (`--body compact`):**
  - Gameplay (**9/9 checks `ok: true`**): corpo largo `7×3×3` (63 células) colocado, tela aberta pelo dummy da frente com 5 botões, identificador `air` e bateria criativa pela tela; barril de ar de `16 000 mB` (0 PU) na porta da ponta `(+3,1,0)` e um barril de pressão 0 na porta da outra ponta `(-3,1,0)`. Depois de 300 ticks: `13 000 mB` em A e **`3 000 mB` de ar a 1 PU** na saída (0 → 1 PU), ar conservado (`16 000`), o barril de 0 PU **não recebeu nada** do ar pressurizado, e a quebra em survival de um dummy remove o núcleo.
  - Corte de gravação (`HORIZON_NO_EAGER_SAVE=1`): **7/7 pontos `ok: true`**, `kv_items=0`, `.mca` inalterados antes da escrita. O trabalho do compressor (operações × 100 + progresso) é contado em ticks: antes da escrita a reabertura volta ao save-base e só ganha os ticks em que o jogo ficou de pé (as operações dos 300 ticks cortados não voltam); depois da escrita o estado salvo não perde operações; entre a 1ª e a 2ª reabertura o trabalho avança **exatamente** com o tempo de jogo (±2 ticks, medido com `tick freeze`); o ar é conservado e os tipos e pressões (`0` na entrada, `1` na saída) voltam em todas.
  - Histórico honesto: a primeira execução parou na colocação (volume `7×3×3` maior que a caixa limpa pelo roteiro) e duas asserções do ponto de corte eram frouxas demais (o jogo anda ticks ao abrir o mundo; o tempo de jogo do `level.dat` pode ficar à frente dos chunks em `kv_committed`); os erros eram do roteiro, não da máquina.
- **Cadeia do petróleo (`/mnt/ssd120/sacer/so/c/run_pressure_oil_proof.py`, jogo nativo):** parte A (o que o `petroleo_c` faz): um destilador a vácuo com bateria criativa e **um barril de óleo a 0 PU encostado nele** consome os `16 000 mB` e produz as quatro frações (`heavyoil_vacuum 6 400`, `reformate 4 000`, `lightoil_vacuum 3 200`, `sourgas 2 400` = `16 000`, com o tanque de entrada a **2 PU**); parte B (o Java): barril de óleo → compressor A (0→1 PU) → compressor B (1→2 PU) → o mesmo destilador, que **recebe o óleo a 2 PU** e produz `2 800 + 1 750 + 1 400 + 1 050 = 7 000 mB` (conservação: `8 000` em A + `1 000` em B + `7 000` = `16 000`). **10/10 checks `ok: true`.**
- **`petroleo_c` (script protegido):** **não foi executado**: `bedrock.py` usa o motor antigo `ntm-visual` (`/home/sacer/Workspace/horizon-visual/target/release/horizon-ctl`, `run-visual.sh`), que não existe nesta máquina. O script **não foi tocado**: os dez SHA continuam iguais (`sha256sum -c` em `ntm-port` → 10 OK, e nenhum desses arquivos tem diferença no worktree). A garantia que ele dá (hidrotratador e destilador alimentados por barris de pressão 0 continuam funcionando) está provada pela parte A acima (jogo nativo), por `processing::tests::the_pressures_of_the_oil_chain_take_theirs_and_zero` e pelos mutantes `P03`/`P04`.

### Lacunas nominais

- **Som do pistão:** `hbm:item.boltgun` toca no cliente quando o pistão chega ao fundo, com velocidade aleatória por ciclo (`randSpeed`); o motor não tem som por evento de animação, então a animação do pistão é um laço fixo de `1,5 s` (sobe em `1 s`, desce em `0,5 s`), sem o som.
- **Shift-click:** `transferStackInSlot` manda qualquer outro item aos slots de upgrade `2` e `3`; só o identificador (slot 0) e a bateria criativa (slot 1) têm rota própria (`quick_move`). Um identificador com o slot 0 cheio pode cair nos slots de upgrade, e outros itens não são enviados a eles.
- **Baterias:** só a bateria criativa carrega a máquina pelo slot 1 (`charge_from_slot`, o mesmo das demais máquinas do porto); as baterias comuns (`IBatteryItem`) não descarregam no slot.
- **Dicas e cópia:** o tooltip de tanque do Java mostra o nome do fluido e `Pressure: n PU` / `Pressurized, use compressor!`; a tela nativa mostra quantidade e cor (a pressão aparece pelo marcador e pelas dicas dos botões). `IUpgradeInfoProvider` (texto dos upgrades) e `IFluidCopiable` (`getSettings`/`pasteSettings`, colar a compressão com a ferramenta de cópia) não existem no porto.
- **Receitas de usuário:** `hbmCompressor.json` (`SerializableRecipe`) pode substituir as receitas padrão no Java; aqui valem as cinco padrão.
- **Recipientes:** `FluidTank.loadTank` recusa recipientes com pressão diferente de 0; os compressores não têm slots de recipiente, e os tanques/barris do porto ficam todos a 0 PU, então a regra não tem onde atuar ainda.
