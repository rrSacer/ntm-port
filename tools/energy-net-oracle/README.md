# Energy-network oracle (T7)

Runs the real HBM `PowerNetMK2`/`NodeNet` and the tile-entity code of the
REDD, the battery socket and the wire endpoints, and records every output in
`fixtures/java.json`. `crates/ntm-core/tests/energy_network_tests.rs` compares
the Rust port (`energy_network.rs`) with it exactly (doubles by bits).

```sh
export ENERGY_NET_HBM_SRC=/path/to/ntm-src          # HBM checkout at pin 4f5924e
export ENERGY_NET_MC_SRC=/path/to/minecraft-src     # has net/minecraft/util/{Vec3,MathHelper}.java
export ENERGY_NET_FORGE_SRC=/path/to/forge-src      # has net/minecraftforge/common/util/ForgeDirection.java
python3 tools/energy-net-oracle/run.py --scratch /path/outside/git/scratch        # rewrite fixture
python3 tools/energy-net-oracle/run.py --scratch /path/outside/git/scratch --check # fail on drift
```

The two decompiled trees come from a Forge 1.7.10 dev setup (ForgeGradle/RetroFuturaGradle
`minecraft-src`, or the `unpacked/src/main/java` of the `forge-1.7.10-10.13.4.1614` cache).
The SHA-256 of the three files used are in `fixtures/java.json` (`provenance`); any tree
whose files hash the same reproduces the fixture. The flags `--source/--mc-src/--forge-src`
override the variables.

Needs OpenJDK `javac`/`java` (compiled `--release 8`), `git`, the HBM tree at
pin `4f5924e` (`--source`, default `/mnt/ssd120/sacer/horizon-ntm/ntm-src`) and
a decompiled 1.7.10 Minecraft/Forge for `Vec3`, `MathHelper`, `ForgeDirection`. No jar is used.

What is executed:

- **Whole files, byte for byte:** `PowerNetMK2`, `NodeNet`, `IEnergy{Receiver,Provider,Handler,Connector}MK2`,
  `IBatteryItem`, `ItemBatteryCreative`, `Tuple`, `EnumUtil`, `BlockPos`/`DirPos`/`Rotation`,
  and Minecraft's `Vec3` + `MathHelper` and Forge's `ForgeDirection`.
- **Sliced verbatim** (`run.py` cuts the method/enum text out of the real file by brace matching,
  nothing retyped): `ItemBatteryPack` (enum + charge methods), `Library.chargeTEFromItems/chargeItemsFromTE`,
  `TileEntityBatteryBase` (`updateEntity`, `getRelevantMode`, `receiveControl`, `getComparatorPower`, `createNode`),
  `TileEntityBatteryREDD`, `TileEntityBatterySocket`, `TileEntityPylonBase.canConnect`/`getConnectionPoint`,
  the five pylon/connector classes, `TileEntityCableSwitch.shouldCreateNode`.
- **Stubs** (`java/stubs`): the Minecraft world/NBT/ItemStack shells, `UniNodespace`/`GenNode`/`Nodespace.PowerNode`
  (one shared test net, so the real `updateEntity` membership code runs), `ModItems`/`ModBlocks` identities.

Two deliberate deviations from the game, both about order only: the net's
provider/receiver sets are swapped for `LinkedHashSet` (the original `HashSet`
iterates in identity-hash order, which is random per run), and the pylon
legacy-metadata migration in `TileEntityPylon.updateEntity` is ported by hand
(`pylon_migrated_meta`).

Not covered by the oracle (hand-ported, tested against the source text): the
switch/detector block logic (`CableSwitch.onBlockActivated`, `CableDetector.onNeighborBlockChange`).
