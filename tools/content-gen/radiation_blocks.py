#!/usr/bin/env python3
"""Content of the radiation blocks of Onda 1, slice 5d: the six levels of `BlockSellafield` (metadata 0..5).

The content has no block states of its own, so each level is a block, as the port does for every metadata subtype
(`<registry path>__<n>`); level 0 keeps `ntm:sellafield`, the id worlds already hold. Each level's texture is the
Java's runtime tint (`BlockSellafield.registerBlockIcons`): `RGBMutatorInterpolatedComponentRemap(0x858384, 0x434343,
light, dark)` over `sellafield_slaked`, with the level's pair of `colors`; the names are `tile.sellafield.<n>.name`.
Writes radiation_blocks.json, which generate.py merges into the catalog.

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
    return out


if __name__ == '__main__':
    entries = rows()
    write_json(HERE / 'radiation_blocks.json', {'schema': 1, 'entries': entries})
    print(len(entries), 'radiation block levels')
