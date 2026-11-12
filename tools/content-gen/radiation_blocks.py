#!/usr/bin/env python3
"""Content of the radiation blocks of Onda 1, slice 5d: the six levels of `BlockSellafield` (metadata 0..5).

The content has no block states of its own, so each level is a block, as the port does for every metadata subtype
(`<registry path>__<n>`); level 0 keeps `ntm:sellafield`, the id worlds already hold. Each level's texture is the
Java's runtime tint (`BlockSellafield.registerBlockIcons`): `RGBMutatorInterpolatedComponentRemap(0x858384, 0x434343,
light, dark)` over `sellafield_slaked`, with the level's pair of `colors`; the names are `tile.sellafield.<n>.name`.
Slice 4b adds the Geiger counter block (`GeigerCounter`). Writes radiation_blocks.json, which generate.py merges into
the catalog.

  python3 tools/content-gen/radiation_blocks.py
"""
import re
from pathlib import Path
from generate import lang, write_json

HERE = Path(__file__).resolve().parent
SRC = HERE.parents[1].parent / 'ntm-src'
JAVA = SRC / 'src/main/java/com/hbm/blocks/generic/BlockSellafield.java'
BASE = 'src/main/resources/assets/hbm/textures/blocks/sellafield_slaked.png'


def levels():
    """The Java's levels: (light, dark) of each, and the remap's source bounds."""
    text = JAVA.read_text()
    count = int(re.search(r'SELLAFITE_LEVELS = (\d+);', text)[1])
    block = text[text.index('int[][] colors'):text.index('};', text.index('int[][] colors'))]
    colors = re.findall(r'\{0x([0-9A-Fa-f]{6}), 0x([0-9A-Fa-f]{6})\}', block)
    source = re.search(r'RGBMutatorInterpolatedComponentRemap\(0x([0-9A-Fa-f]{6}), 0x([0-9A-Fa-f]{6}), tint\[0\], tint\[1\]\)', text)
    assert len(colors) == count, (count, colors)
    return [(light.upper(), dark.upper()) for light, dark in colors], (source[1].upper(), source[2].upper())


def rows():
    colors, (source_light, source_dark) = levels()
    translations = {loc: lang(SRC / f'src/main/resources/assets/hbm/lang/{old}.lang') for loc, old in [('en_us', 'en_US'), ('pt_br', 'pt_BR')]}
    out = []
    for level, (light, dark) in enumerate(colors):
        out.append({
            'id': 'sellafield' if level == 0 else f'sellafield__{level}',
            'source_variable': 'sellafield', 'type': 'block', 'category': 'decorative', 'source_class': 'BlockSellafield',
            'template': 'minecraft:stone', 'metadata': level, 'subtype_kind': 'level',
            # `needsRandomTick = true` (BlockSellafield's constructor): updateTick goes down a level (slice 5d).
            'block_properties': {'random_ticks': True},
            'source_texture': BASE,
            'tint': {'light': light, 'dark': dark, 'source_light': source_light, 'source_dark': source_dark},
            'names': {loc: table[f'tile.sellafield.{level}.name'] for loc, table in translations.items()},
            'behavior_gap': 'Walking potion and random-tick decay in ntm-core and the native random tick (slice 5d); the chunk radiation of updateTick '
                            '(incrementRad) waits for slice 4; the four position texture variants are reduced to the first',
        })
    return out + [geiger(translations)]


def geiger(translations):
    """`GeigerCounter` (ModBlocks:1635): Material.iron, hardness 15, resistance 0.25, the machine tab. Drawn by
    RenderGeiger's OBJ (visual.json); turned by the meta its placer's yaw gives, which is the way the placer looks
    (`onBlockPlacedBy`: yaw 0, south, is meta 3). The beehive faces the placer, the other way: the model takes 180
    more (visual.json) and each facing takes the bounds of the meta opposite it (`setBlockBoundsBasedOnState`, the
    same for the collision box)."""
    f = 1 / 16
    meta_bounds = {4: [0, 0, 0, 14 * f, 9 * f, 14.5 * f], 2: [1.5 * f, 0, 0, 1, 9 * f, 14 * f],
                   5: [2 * f, 0, 1.5 * f, 1, 9 * f, 1], 3: [0, 0, 2 * f, 14.5 * f, 9 * f, 1]}
    # The beehive's facing and the meta the Java gives the same placement (opposite ways).
    opposite_meta = {'north': 3, 'south': 2, 'west': 5, 'east': 4}
    return {
        'id': 'geiger', 'source_variable': 'geiger', 'type': 'block', 'category': 'machine', 'source_class': 'GeigerCounter',
        'template': 'minecraft:beehive',
        'textures': {'side': 'blocks/geiger', 'top': 'blocks/geiger', 'front': 'blocks/geiger'},
        'source_texture': 'src/main/resources/assets/hbm/textures/blocks/geiger.png',
        # setHardness(15.0F).setResistance(0.25F); no step sound set: the stone one.
        'block_properties': {'hardness': 15.0, 'explosion_resistance': 0.25, 'sounds': 'minecraft:stone'},
        'shapes': {'property': 'facing', 'values': {facing: [[round(v, 6) for v in meta_bounds[meta]]] for facing, meta in opposite_meta.items()}},
        'names': {loc: table['tile.geiger.name'] for loc, table in translations.items()},
        'behavior_gap': 'TileEntityGeiger (clicks, comparator, report) in the native glue and ntm-core geiger_block (slice 4b); '
                        'the OpenComputers and Energy Control readings have no mods to read them',
    }


if __name__ == '__main__':
    entries = rows()
    write_json(HERE / 'radiation_blocks.json', {'schema': 1, 'entries': entries})
    print(len(entries), 'radiation blocks')
