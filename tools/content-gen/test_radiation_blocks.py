"""The six levels of BlockSellafield (Onda 1, slice 5d) against the Java, read on their own.

Each level is a block (`ntm:sellafield`, `ntm:sellafield__1`..`__5`) with metadata `n`, the Java's name
`tile.sellafield.<n>.name`, the texture `RGBMutatorInterpolatedComponentRemap(0x858384, 0x434343, light, dark)` over
`sellafield_slaked` with the level's colours (recomputed here pixel by pixel), the inventory radiation of
`HazardRegistry:222-227`, the blocks tab of the constructor and the shredder recipes of `ShredderRecipes:294-299`.
"""
import json
import os
import re
import unittest
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
SRC = Path(os.environ.get('NTM_SRC', ROOT.parent / 'ntm-src')) / 'src/main'
JAVA = SRC / 'java/com/hbm'
CONTENT = ROOT / 'content'
SELLAFIELD = (JAVA / 'blocks/generic/BlockSellafield.java').read_text()
IDS = {e['id']: e for e in json.loads((CONTENT / 'ids.json').read_text())['entries']}


def level_id(n):
    return 'ntm:sellafield' if n == 0 else f'ntm:sellafield__{n}'


def java_lang(name):
    out = {}
    for line in (SRC / f'resources/assets/hbm/lang/{name}.lang').read_text(encoding='utf-8-sig').splitlines():
        if '=' in line and not line.lstrip().startswith('#'):
            k, v = line.split('=', 1)
            out[k.strip()] = v.strip()
    return out


def remap(component, src_light, src_dark, light, dark):
    """`RGBMutatorInterpolatedComponentRemap.shiftComponent`, with the Java's `(int)` cast and `& 0xff`."""
    pos = (component - src_light) / (src_dark - src_light)
    value = light + pos * (dark - light)
    return int(value) & 0xff


class SellafieldLevels(unittest.TestCase):
    def test_six_levels_with_the_java_names(self):
        count = int(re.search(r'SELLAFITE_LEVELS = (\d+);', SELLAFIELD)[1])
        self.assertEqual(count, 6)
        langs = {loc: (java_lang(old), json.loads((CONTENT / f'assets/ntm/lang/{loc}.json').read_text()))
                 for loc, old in [('en_us', 'en_US'), ('pt_br', 'pt_BR')]}
        for n in range(count):
            entry = IDS[level_id(n)]
            self.assertEqual((entry['type'], entry['metadata'], entry['source_variable']), ('block', n, 'sellafield'))
            for loc, (java, port) in langs.items():
                self.assertEqual(port[f'block.ntm.{level_id(n).split(":")[1]}'], java[f'tile.sellafield.{n}.name'], (loc, n))
        self.assertNotIn('ntm:sellafield__6', IDS)

    def test_each_levels_texture_is_the_java_tint(self):
        block = SELLAFIELD[SELLAFIELD.index('int[][] colors'):]
        colors = [(int(a, 16), int(b, 16)) for a, b in re.findall(r'\{0x([0-9A-Fa-f]{6}), 0x([0-9A-Fa-f]{6})\}', block)[:6]]
        source = re.search(r'RGBMutatorInterpolatedComponentRemap\(0x([0-9A-Fa-f]{6}), 0x([0-9A-Fa-f]{6})', SELLAFIELD)
        src_light, src_dark = int(source[1], 16), int(source[2], 16)
        base = Image.open(SRC / 'resources/assets/hbm/textures/blocks/sellafield_slaked.png').convert('RGBA')
        for n, (light, dark) in enumerate(colors):
            name = level_id(n).split(':')[1]
            model = json.loads((CONTENT / f'assets/ntm/models/block/{name}.json').read_text())
            texture = next(iter(model['textures'].values())).split(':')[1]
            port = Image.open(CONTENT / f'assets/ntm/textures/{texture}.png').convert('RGBA')
            self.assertEqual(port.size, base.size)
            for x in range(base.width):
                for y in range(base.height):
                    r, g, b, a = base.getpixel((x, y))
                    expected = tuple(remap(c, (src_light >> s) & 0xff, (src_dark >> s) & 0xff, (light >> s) & 0xff, (dark >> s) & 0xff)
                                     for c, s in zip((r, g, b), (16, 8, 0))) + (a,)
                    self.assertEqual(port.getpixel((x, y)), expected, (n, x, y))

    def test_inventory_radiation_by_level(self):
        registry = (JAVA / 'hazard/HazardRegistry.java').read_text()
        java = {int(m): float(v) for m, v in re.findall(r'new ItemStack\(ModBlocks\.sellafield, 1, (\d)\), makeData\(RADIATION, ([\d.]+)F\)', registry)}
        self.assertEqual(sorted(java), list(range(6)))
        hazards = {e['id']: e for e in json.loads((ROOT / 'crates/ntm-core/data/hazards.json').read_text())['entries']}
        for n, rad in java.items():
            self.assertEqual(hazards[level_id(n)]['radiation_rad_s'], rad, n)

    def test_the_levels_are_in_the_blocks_tab_in_order(self):
        tab = json.loads((CONTENT / 'data/ntm/creative_tab/blocks.json').read_text())['items']
        self.assertIn('setCreativeTab(MainRegistry.blockTab)', SELLAFIELD)
        positions = [tab.index(level_id(n)) for n in range(6)]
        self.assertEqual(positions, sorted(positions))
        parts = json.loads((CONTENT / 'data/ntm/creative_tab/parts.json').read_text())['items']
        self.assertNotIn('ntm:sellafield', parts)

    def test_each_level_drops_itself_and_shreds(self):
        shredder = json.loads((CONTENT / 'machine-recipes/shredder.json').read_text())['recipes']
        java = re.findall(r'ShredderRecipes\.setRecipe\(new ItemStack\(ModBlocks\.sellafield, 1, (\d)\), new ItemStack\(ModItems\.scrap_nuclear, (\d+)\)\)',
                          (JAVA / 'inventory/recipes/ShredderRecipes.java').read_text())
        self.assertEqual(len(java), 6)
        for n, count in java:
            loot = json.loads((CONTENT / f'data/ntm/loot_table/blocks/{level_id(int(n)).split(":")[1]}.json').read_text())
            self.assertEqual(loot['pools'][0]['entries'][0]['name'], level_id(int(n)))
            recipe = next(r for r in shredder if r['input'].get('ids') == [level_id(int(n))])
            self.assertEqual(recipe['outputs'], [['ntm:scrap_nuclear', int(count)]], n)


if __name__ == '__main__':
    unittest.main()
