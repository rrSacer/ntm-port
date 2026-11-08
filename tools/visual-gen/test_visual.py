"""Checks of the visual spec and the generator's output (docs/visual-fiel.md §6).

Run from this folder: python3 -m unittest test_visual
"""
import json
import unittest
from pathlib import Path

import generate

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
CONTENT = ROOT / 'content'
JAVA_TANK = ROOT.parent / 'ntm-src/src/main/java/com/hbm/render/block/RenderFoundryTank.java'


def spec_blocks():
    return json.loads((HERE / 'visual.json').read_text())['blocks']


class SpecFormat(unittest.TestCase):
    def test_visual_json_is_written_by_indent_2(self):
        # The spec is edited by hand and by scripts: one format keeps diffs small.
        text = (HERE / 'visual.json').read_text()
        self.assertEqual(json.dumps(json.loads(text), indent=2, ensure_ascii=False) + '\n', text)


class FoundryTank(unittest.TestCase):
    def setUp(self):
        self.model = spec_blocks()['ntm:foundry_tank']['model']

    def test_a_wall_leaves_out_only_toward_a_tank(self):
        # RenderFoundryTank: each side is drawn unless tank is beside it.
        show = self.model['show']
        self.assertEqual(set(show), {'WallNorth', 'WallSouth', 'WallWest', 'WallEast'})
        offsets = {'WallNorth': [0, 0, -1], 'WallSouth': [0, 0, 1], 'WallWest': [-1, 0, 0], 'WallEast': [1, 0, 0]}
        for wall, rule in show.items():
            self.assertEqual(rule['neighbor'], offsets[wall])
            self.assertEqual(rule['not_blocks'], ['ntm:foundry_tank'])
            self.assertNotIn('blocks', rule)

    def test_walls_and_floor_match_the_java_bounds(self):
        # The walls and the floor are 0.125 thick (2 of 16 px) and 1 tall, as
        # RenderFoundryTank's setRenderBounds say.
        java = JAVA_TANK.read_text() if JAVA_TANK.exists() else None
        if java is None:
            self.skipTest('ntm-src ausente')
        self.assertIn('setRenderBounds(0.875D, 0D, 0D, 1D, 1D, 1D)', java)
        boxes = self.model['boxes']
        self.assertEqual(boxes['Floor'][0][1], [16, 2, 16])
        self.assertEqual(boxes['WallEast'][0][0], [14, 0, 0])
        self.assertEqual(boxes['WallNorth'][0][1], [16, 16, 2])
        for wall in ('WallNorth', 'WallSouth', 'WallWest', 'WallEast'):
            frm, to, _ = boxes[wall][0]
            self.assertEqual(to[1] - frm[1], 16, wall)

    def test_generated_model_names_its_bones(self):
        out = (CONTENT / 'assets/ntm/models/obj/foundry_tank.obj').read_text()
        for bone in ('Floor', 'WallNorth', 'WallSouth', 'WallWest', 'WallEast'):
            self.assertIn(f'o {bone}\n', out)


class ExtractorGate(unittest.TestCase):
    def test_single_cell_panels_have_models(self):
        for name in ('rbmk_gauge', 'rbmk_terminal', 'fan', 'machine_microwave'):
            model = json.loads((CONTENT / f'assets/ntm/horizon_model/{name}.json').read_text())
            self.assertTrue(model['obj'].startswith('ntm:models/obj/'), name)

    def test_multiblocks_stay_hand_only(self):
        # A multiblock's model in one cell would be wrong: no extracted model for them.
        blocks = spec_blocks()
        for name in ('machine_silex', 'machine_cyclotron', 'machine_fel'):
            self.assertIs(blocks[f'ntm:{name}'].get('auto'), False, name)
            self.assertFalse((CONTENT / f'assets/ntm/horizon_model/{name}.json').exists(), name)


if __name__ == '__main__':
    unittest.main()
