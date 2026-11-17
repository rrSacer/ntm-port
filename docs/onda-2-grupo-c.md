# Onda 2 — Grupo C: ar, compressores, torres de resfriamento e deutério

Máquinas do grupo C na tabela de registro (`crates/ntm-core/src/registry.rs`, `onda2-k3-c` a partir de `fa3c1df3`):

| # | Bloco (`ntm:*`) | Contrato Java | Estado |
| --- | --- | --- | --- |
| 1 | `ntm:machine_intake` | `MachineIntake`, `TileEntityMachineIntake`, `RenderIntake` | Concluída |
| 2 | `ntm:machine_compressor` | `MachineCompressor`, `TileEntityMachineCompressor`, `RenderCompressor` | Aguardando decisão do coordenador sobre política de pressão (`thread 9ff9ea27`) |
| 3 | `ntm:machine_compressor_compact` | `MachineCompressorCompact`, `TileEntityMachineCompressorCompact`, `RenderCompressorCompact` | Aguardando a mesma decisão |
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
  - Gameplay: **10/10 checks `ok: true`** (extrator colocado com tanques `water 1 000` / `heavywater 100`; sem GUI ao interagir; sem energia (bateria em modo entrada) não produz água pesada nem gasta a bateria; com sinal de redstone liga (`honey_level=5`); `16 000 mB` de água viram exatamente `320 mB` de `heavywater` (`160` ticks × `500 HE` = **80 000 HE** exatos, medidos pela bateria + buffer do extrator); desliga (`honey_level=0`) quando acaba a água e parado não consome; quebra em survival remove o bloco). A bateria criativa que carrega a bateria de teste é retirada do slot antes da medida (senão ela recarregaria a cada tick e esconderia o consumo).
  - Corte de gravação (`HORIZON_NO_EAGER_SAVE=1`): **7/7 pontos `ok: true`** (`none`, `kill`, `kv_committed`, `save_begin`, `chunks_written`, `entities_written`, `world_saved`), `kv_items=0` em todos, `.mca` inalterados em `kill`/`kv_committed`/`save_begin`. Nos cortes antes da escrita as 2 reaberturas voltam ao último save (`heavywater 0`, água `16 000 mB` = barril `15 000` + `1 000` no extrator, conservada); nos cortes após a escrita (e em `none`) as 2 reaberturas preservam `heavywater 320`, água `0`.
  - Histórico honesto da prova: a primeira execução falhou no check de energia (a bateria criativa no slot recarregava a bateria de teste) e a segunda no ponto `kill` (a asserção esperava os `16 000 mB` todos no barril, mas `1 000 mB` já estão no tanque do extrator); os dois erros eram do roteiro, não da máquina, e foram corrigidos (conservação `barril + extrator = 16 000`) antes da execução v3 que passou. Evidência: `/mnt/ssd120/sacer/so/c/deuterium-gameplay.json`.
  - Os testes unitários/integração e os 12 mutantes acima foram rodados no início desta frente (sessão anterior), sobre o mesmo código commitado aqui.

### Lacunas nominais

- **`ILookOverlay`:** `MachineDeuteriumExtractor.printHook` exibe energia e tanques ao olhar no 1.7.10; o port nativo não implementa o overlay de bloco ao olhar (mantém `view()` interno).
- **Orientação:** o Java força `meta = 0`; o bloco nativo guarda `facing` do `place`, mas todas as faces laterais usam a mesma textura, então não há diferença visível.
