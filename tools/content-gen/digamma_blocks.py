#!/usr/bin/env python3
"""Content of the blocks the DRX laser leaves (Onda 2, sublote 2, machine_fel): `DigammaFlame` and `DigammaMatter`.

- `fire_digamma` (`ModBlocks:2028`, `blocks/bomb/DigammaFlame.java`): Material.fire, hardness 0, resistance 150, light
  1.0 (15), no creative tab, render type 1 (the crossed squares), no collision box (`getCollisionBoundingBoxFromPool`
  null) but the whole cell to aim at, no drop. It stands only on a block with a solid top (`canPlaceBlockAt`,
  `onNeighborBlockChange`): the dead bush's template asks `mayPlaceOn` of its ground, which the native glue answers
  for it, and it is replaceable as Material.fire is. A body touching it takes 0.05 digamma a tick (ntm-core
  `digamma_server`).
- `digamma_matter` (`ModBlocks:2029`, `blocks/bomb/DigammaMatter.java`): Material.portal, unbreakable, resistance
  18000000, no creative tab, no collision box. Its metadata 0..7 are one block each (`digamma_matter`,
  `digamma_matter__<n>`), as the port does for every metadata subtype; each schedules its tick 10 to 49 ticks on
  (`onBlockAdded`) and spreads (`updateTick`, ntm-core `digamma::matter_spread`) in the native glue. The en_US lang has
  no name for it: the 1.7.10 shows the key.

Writes digamma_blocks.json, which generate.py merges into the catalog.

  python3 tools/content-gen/digamma_blocks.py
"""
from pathlib import Path
from generate import lang, write_json

HERE = Path(__file__).resolve().parent
SRC = HERE.parents[1].parent / 'ntm-src'
TEXTURES = 'src/main/resources/assets/hbm/textures/blocks/'
MATTER_LAST = 7
NO_COLLISION = {'default': []}
FULL_CELL = {'default': [[0, 0, 0, 1, 1, 1]]}


def names(translations, key):
    return {loc: table.get(key, key) for loc, table in translations.items()}


def rows():
    translations = {loc: lang(SRC / f'src/main/resources/assets/hbm/lang/{old}.lang') for loc, old in [('en_us', 'en_US'), ('pt_br', 'pt_BR')]}
    flame = {
        'id': 'fire_digamma', 'source_variable': 'fire_digamma', 'type': 'block', 'category': 'decorative',
        'source_class': 'DigammaFlame', 'source_location': 'src/main/java/com/hbm/blocks/ModBlocks.java:2028',
        'template': 'minecraft:dead_bush', 'cross_model': True, 'loot_none': True, 'creative_hidden': True,
        'block_properties': {'hardness': 0.0, 'explosion_resistance': 150.0, 'requires_tool': False, 'light': 15,
                             'sounds': 'minecraft:stone', 'occludes': False},
        'shapes': FULL_CELL, 'collision_shapes': NO_COLLISION,
        'source_texture': TEXTURES + 'fire_digamma.png',
        'names': names(translations, 'tile.fire_digamma.name'),
        'behavior_gap': 'Only players take its digamma (the other living things wait for the port\'s entities); a solid top '
                        'is an opaque full cube (isSideSolid\'s rule), not its slab, stairs, hopper and snow cases',
    }
    out = [flame]
    for meta in range(MATTER_LAST + 1):
        out.append({
            'id': 'digamma_matter' if meta == 0 else f'digamma_matter__{meta}', 'source_variable': 'digamma_matter',
            'type': 'block', 'category': 'decorative', 'source_class': 'DigammaMatter',
            'source_location': 'src/main/java/com/hbm/blocks/ModBlocks.java:2029',
            'template': 'minecraft:glass', 'metadata': meta, 'subtype_kind': 'level', 'loot_none': True, 'creative_hidden': True,
            'block_properties': {'hardness': -1.0, 'explosion_resistance': 18000000.0, 'requires_tool': False,
                                 'sounds': 'minecraft:stone', 'occludes': False},
            'collision_shapes': NO_COLLISION,
            'source_texture': TEXTURES + 'digamma_matter.png',
            'names': names(translations, 'tile.digamma_matter.name'),
            'behavior_gap': 'setBlockBoundsBasedOnState\'s random bounds (a look) are the whole cell',
        })
    return out


if __name__ == '__main__':
    entries = rows()
    write_json(HERE / 'digamma_blocks.json', {'schema': 1, 'entries': entries})
    print(len(entries), 'digamma blocks')
