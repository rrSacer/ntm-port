# RBMK T00: executable Java oracle

This harness compiles **unchanged whole classes** from the HBM pin and calls their
actual methods. It does not translate RBMK equations to Python or use enum
comments as expected results. Required tools: Python 3, `git`, OpenJDK `java`
and `javac` (validated with OpenJDK 25.0.5.0.3, compilation `--release 8`).

From the NTM checkout:

```sh
python3 tools/rbmk-oracle/run.py \
  --source /mnt/ssd120/sacer/horizon-ntm/ntm-src \
  --scratch /mnt/ssd120/sacer/horizon-ntm/.agent-tmp/rbmk/oracle
python3 tools/rbmk-oracle/run.py \
  --source /mnt/ssd120/sacer/horizon-ntm/ntm-src \
  --scratch /mnt/ssd120/sacer/horizon-ntm/.agent-tmp/rbmk/oracle-check --check
python3 -m unittest discover -s tools/content-gen -p '*rbmk*.py' -v
```

The test suite recompiles the real classes in a new SSD scratch directory and
compares **all** observed output, including raw double bits, to the checked
fixture. `RBMK_ORACLE_SCRATCH` overrides its parent directory. Never point
`--scratch` at an existing source tree: its owned `src/` and `classes/` children
are rebuilt. Runtime build outputs, copied HBM sources and execution logs stay
outside Git. No jars were downloaded or used; `external_jars` is empty, so
there are no third-party jar origins/hashes to list or redistribute.

`fixtures/java.json` records the full Java revision, SHA-256 of all input files
and of the harness, which classes were copied whole, and which registries were
sliced. Every input is checked against its committed Git blob before the run.
Content generation validates these fingerprints before consuming the fixture;
a source/harness change fails closed until regeneration. `execution.json` in
scratch records toolchain versions and fixture hash separately from the
deterministic fixture.

## What is executed

Thirteen whole HBM classes: `ItemRBMKRod`, `ItemRBMKPellet`, `ItemFELCrystal`,
`ItemNuclearWaste`, `ItemWasteLong`, `ItemWasteShort`, `RBMKDials`,
`IRBMKFluxReceiver`, `GameRuleHelper`, `Tuple`, `WeightedRandomObject`,
`SILEXRecipes`, `OutgasserRecipes`. Whole-file copying is checked byte-for-byte.

The actual 32 pellet / 33 rod initializer statements and tint declarations
from `ModItems` are placed verbatim in an identity-only `ModItems` host.
They instantiate the real classes, so omitted setters retain real field
initializers. The two small metadata enums from `ItemEnums` are copied verbatim.
OreDict shape prefixes and material names are read from their declarations;
the used `DictFrame` helper method bodies are copied verbatim. Fluid IDs follow
all registration declarations (`registerSelf` order, with forced IDs verified),
not the order the subset is encountered. GregTech 6 is absent in this profile;
the source's conditional uranium name therefore resolves to `Uranium`.

Observed calls include fresh/used NBT and tooltip key+argument traces for each
rod; four actual burn/updateHeat/provideHeat steps per rod; all setter overloads;
all nine burn functions and five depletion functions; old-poison ordering,
melting-point equality/overflow, thermal coefficient and rectify cases; 25 dial
defaults, coercion/clamping/error/NaN/Infinity probes and special client/reset
paths. The output uses explicit strings for nonfinite doubles plus their raw
IEEE bits. The harness does not sanitize the original Java behavior.

Actual `SILEXRecipes.register()` and `OutgasserRecipes.registerDefaults()` are
executed. SILEX entries come directly from the registry, preserving output
order, metadata, quantity, weights and duplicate DRX outputs. Tiny-waste
entries come from the actual `getOutput` lookup. Missing pellet states are
explicit. Recipe output ordering in JSON is sorted only for registry-map
enumeration; each recipe's output list remains in Java order.

## Controlled host surface and limits

`stubs.py` supplies MC/Forge data containers, item/block identity, the standard
1.7.10 clamp semantics, string gamerules, annotations and unused presentation
interfaces. NBT is deliberately a **double-only** map matching the exercised
RBMK fields, returning zero for a missing numeric key. `Item` stores identity,
container and stack limit. Tooltip localization returns the actual translation
key plus arguments; it is **not** a fabricated translation or a game renderer.
Recipe stack equality implements identity/count/metadata for the singular
registry lookups exercised here. Vanilla source identities use the explicit
`minecraft_1_7_10:` audit namespace (for example `dye` with metadata), so they
cannot be mistaken for executable 26.3 recipe references. Translating those
legacy references to the current registry belongs to the consuming runtime.
OreDict membership enumeration returns no
external entries: recipes registered by ore key retain the real key in the
fixture, while arbitrary mod-dependent ore resolution is outside this oracle.

The Gson/SerializableRecipe APIs exist only so the whole Outgasser class can
compile. Their unexercised JSON read/write methods throw; no config override
is silently accepted. Waste entities, rendering and world simulation are not
executed. Crafting/assembly/anvil **conditionals are a lexical inventory**, with
guards, source lines and exact arguments, clearly separate from executable
SILEX/Outgasser evidence. This is not a Forge/MC integration test.

Two dial quirks are retained: client `getSurgeMod` reads passive cooling;
`createDials(force)` refreshes before resetting raw rules, leaving the old
cache until the next refresh. `KEY_SAVE_DIALS` is a raw gamerule, not an entry
added by `refresh`, so its `cached_effective` is null. Missing compound and
missing key are distinct: only an absent compound initializes full yield and
core/hull 20. `enrichment` is a derived getter without a clamp, not an NBT key.

T00 supplies data and proof, not RBMK runtime. All 33 machines remain Inert;
dynamic rod tooltip/hazards/physics belong to later tickets. In-game T00
acceptance links delivered IDs/custom data and captured current tooltips to
the fixture, and labels those runtime gaps. D8 `leverStop` remains blocked;
this harness neither imports its asset nor replaces its sound.
