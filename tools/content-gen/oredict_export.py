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
from shredder_recipes import ShredderResolver, dictionary

root = Path(__file__).resolve().parents[2]
source = Path(sys.argv[1]) if len(sys.argv) > 1 else root.parent / 'ntm-src'
rows = json.loads((root / 'content/ids.json').read_text())['entries']
resolver = ShredderResolver(source, rows)
ores, sources = dictionary(resolver)
# Forge 1.7.10's `stickWood` and `slabWood` (OreDictionary.java:57-70, 71-105),
# and the tar group's `anyTar` (`DictGroup.addPrefix(ANY)`: the four tars).
woods = ('oak', 'spruce', 'birch', 'jungle', 'acacia', 'dark_oak')
ores['stickWood'] = [resolver.vanilla_item('Items.stick')]
ores['slabWood'] = [resolver.vanilla_item('Items.' + w + '_slab') for w in woods]
ores['anyTar'] = [i for k in ('oiltar', 'cracktar', 'coaltar', 'woodtar') for i in ores[k]]
out = root / 'crates/ntm-core/data/oredict.json'
out.write_text(json.dumps({'schema': 1, 'names': {k: ores[k] for k in sorted(ores)}}, separators=(',', ':')) + '\n')
print(f'{len(ores)} names, {sum(map(len, ores.values()))} entries -> {out.relative_to(root)}')
