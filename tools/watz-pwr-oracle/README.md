# Watz / PWR / Hephaestus / flare oracle (T8)

Runs the **original HBM code** and records what it does, so the Rust domain in
`crates/ntm-core/src/{watz_pwr,petroleo_e.rs}` can be checked against it tick by tick.

```sh
python3 tools/watz-pwr-oracle/run.py \
  --source /mnt/ssd120/sacer/horizon-ntm/ntm-src \
  --scratch /mnt/ssd120/sacer/horizon-ntm/.agent-tmp/<front>/oracle          # regenerate the fixture
python3 tools/watz-pwr-oracle/run.py --source ... --scratch ... --check     # fail if it differs
```

Needs `git`, Python 3 and OpenJDK (`javac --release 8`). `fixtures/java.json` records the
pinned Java revision and the SHA-256 of every source it reads; a source or harness change fails
closed until the fixture is regenerated.

## What is real and what is stubbed

- Copied **whole**: `Function.java`, `EnumUtil.java`.
- Cut **verbatim** (brace matching, no rewriting) from the pinned sources: `EnumWatzType` and the
  `ItemWatzPellet` NBT statics, `EnumPWRFuel`, `BobMathUtil.squirt`, every `FT_Heatable` /
  `FT_PWRModerator` statement of `Fluids.java`, and these methods:
  `TileEntityWatz` (`updateEntity`, `updateReaction`, `updateCoolant`, `setupCoolant`, `getPorts`),
  `TileEntityPWRController` (`setup`, `updateEntity`, `meltDown`, `updateCoolant`, `setupTanks`,
  the multiplier helpers, `receiveControl`), `MachinePWRController` (`assemble`, `floodFill`,
  validity checks), `TileEntityMachineHephaestus` (`updateEntity`, `heatFluid`, `setupTanks`,
  `heatFromBlock`, `getTotalHeat`), `TileEntityMachineGasFlare.updateEntity`,
  `TileEntityWatzStruct.updateEntity`, `Watz.breakBlock`, the `watz_end` line of
  `BlockToolConversion.registerRecipes`, and `TileEntityMachineBase.decrStackSize`.
- `java/Stubs.java` only holds identity/container data (item stacks, blocks, a hash-map world,
  tanks, trait lookup) and no-ops for networking, sound, particles and ports. Effects the original
  would cause are recorded in the world log (`set`, `explode`, `rad`, `disassemble`, `drop`).
- The flare's per-fluid traits (gaseous / flammable / heat energy) come from the checked-in
  `content/machine-recipes/flare.json`, not from re-evaluating `Fluids.java`.
- Doubles are written as raw IEEE bits. The Rust test compares bit for bit, except the PWR
  `progress` accumulator, which carries `log10`/`pow` results (Java intrinsics and libm round
  differently by up to 1 ulp) and is compared at 1e-12 relative.

Scenarios (inputs are in the fixture, so the Rust test mirrors nothing by hand): 12 pellet types,
stacked columns, mud overflow, pump/power gating (19 Watz runs); the Watz core structure at each
of its 110 positions (remove, wrong metadata, foreign block) and at 9 world times; 28 PWR
structures including the facing quirks, fuel lines of range 14 to 18 and the 4096 limit; 46 PWR
tick runs (15 fuels, 9 coolants, rods up and down, 13 to 17 heatsinks, unloaded chunks,
auto-loading, depletion into the output slot, 3 meltdowns); 45 Hephaestus runs over 15 fluids and
3 lava fields; the flare for all 156 fluids (vent and burn) plus upgrade, tilt, empty and cap cases
(365 runs).
