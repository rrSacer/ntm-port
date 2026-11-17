# Onda 2 — Grupo C: ar, compressores, torres de resfriamento e deutério

Máquinas do grupo C na tabela de registro (`crates/ntm-core/src/registry.rs`, `onda2-k3-c` a partir de `fa3c1df3`):

| # | Bloco (`ntm:*`) | Contrato Java | Estado |
| --- | --- | --- | --- |
| 1 | `ntm:machine_intake` | `MachineIntake`, `TileEntityMachineIntake`, `RenderIntake` | Concluída |
| 2 | `ntm:machine_compressor` | `MachineCompressor`, `TileEntityMachineCompressor`, `RenderCompressor` | Aguardando decisão do coordenador sobre política de pressão (`thread 9ff9ea27`) |
| 3 | `ntm:machine_compressor_compact` | `MachineCompressorCompact`, `TileEntityMachineCompressorCompact`, `RenderCompressorCompact` | Aguardando a mesma decisão |
| 4 | `ntm:machine_tower_small` | `MachineTowerSmall`, `TileEntityTowerSmall`, `RenderSmallTower` | Pendente |
| 5 | `ntm:machine_tower_large` | `MachineTowerLarge`, `TileEntityTowerLarge`, `RenderLargeTower` | Pendente |
| 6 | `ntm:machine_deuterium_extractor` | `MachineDeuteriumExtractor`, `TileEntityDeuteriumExtractor` | Pendente |

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
