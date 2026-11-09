#!/usr/bin/env python3
"""Export the OreDict the machines read (name -> item ids), as the shredder's
extraction builds it: Forge 1.7.10's vanilla entries mapped to the 26.3 ids,
and every `OreDictionary.registerOre` / `OreDictManager` shape of the NTM.

Machines ask for names (`ore*`, `logWood`, `anyTar`, `plankWood`...); the port
asks the reverse (`crates/ntm-core/src/oredict.rs`).

  python3 tools/content-gen/oredict_export.py [NTM_SRC]
"""
import json
import sys
from pathlib import Path
from machine_recipes import MachineResolver

root = Path(__file__).resolve().parents[2]
source = Path(sys.argv[1]) if len(sys.argv) > 1 else root.parent / 'ntm-src'
rows = json.loads((root / 'content/ids.json').read_text())['entries']
# Two views of the same dictionary: the shredder's (Forge 1.7.10's vanilla
# entries on the 26.3 ids, the NTM's registrations) and the machine extractor's
# (DictGroup shapes and the `any*` aliases, tags expanded); their union.
from shredder_recipes import ShredderResolver, dictionary
resolver = MachineResolver(source, rows)
ores = {k: list(v) for k, v in dictionary(ShredderResolver(source, rows))[0].items()}
for key, values in resolver.ore.items():
    for value in sorted(values):
        for ident in (resolver.expand_tag(value, {}) if value.startswith('#') else [value]):
            if ident not in ores.setdefault(key, []):
                ores[key].append(ident)
# Forge 1.7.10's `stickWood` and `slabWood` (OreDictionary.java:57-70, 71-105).
woods = ('oak', 'spruce', 'birch', 'jungle', 'acacia', 'dark_oak')
ores['stickWood'] = [resolver.vanilla_item('Items.stick')]
ores['slabWood'] = [resolver.vanilla_item('Items.' + w + '_slab') for w in woods]
out = root / 'crates/ntm-core/data/oredict.json'
out.write_text(json.dumps({'schema': 1, 'names': {k: ores[k] for k in sorted(ores)}}, separators=(',', ':')) + '\n')
print(f'{len(ores)} names, {sum(map(len, ores.values()))} entries -> {out.relative_to(root)}')
