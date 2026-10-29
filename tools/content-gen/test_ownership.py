#!/usr/bin/env python3
"""Exercise file handover between content-gen and visual-gen in isolated output."""
import json
import tempfile
import unittest
from pathlib import Path
from generate import generate, write_json

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT.parent / 'ntm-src'


class GeneratorOwnership(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # A real foundry-shaped machine fallback exercises JSON and PNG writes.
        cls.row = {'id': 'ntm:foundry_mold', 'source_variable': 'foundry_mold', 'type': 'block', 'category': 'machine',
                   'names': {'en_us': 'Shallow Foundry Basin', 'pt_br': 'Bacia Rasa de Fundição'},
                   'source_texture': 'src/main/resources/assets/hbm/textures/blocks/brick_fire.png',
                   'textures': {face: 'blocks/brick_fire' for face in ('side', 'top', 'front')},
                   'opaque_machine_textures': True}
        cls.state = 'assets/ntm/blockstates/foundry_mold.json'
        cls.model = 'assets/ntm/models/block/foundry_mold_0.json'
        cls.texture = 'assets/ntm/textures/block/foundry_mold_front_opaque.png'

    def test_visual_claim_prevents_overwrite_and_cleanup_during_handover(self):
        with tempfile.TemporaryDirectory() as directory:
            out = Path(directory)
            generate(SOURCE, out, [self.row], [])
            old_only = 'assets/ntm/models/block/formerly_content.json'
            obsolete = 'assets/ntm/models/block/obsolete.json'
            claims = [self.state, self.model, self.texture, old_only]
            sentinels = {rel: ('visual owns ' + rel).encode() for rel in claims}
            for rel, data in sentinels.items():
                (out / rel).parent.mkdir(parents=True, exist_ok=True)
                (out / rel).write_bytes(data)
            (out / obsolete).write_text('obsolete content file')
            previous = out / '.content-gen-files.json'
            write_json(previous, json.loads(previous.read_text()) + [old_only, obsolete])
            write_json(out / '.visual-files.json', claims)
            generate(SOURCE, out, [self.row], [])
            for rel, data in sentinels.items(): self.assertEqual((out / rel).read_bytes(), data)
            self.assertFalse((out / obsolete).exists())
            self.assertTrue(set(claims).isdisjoint(json.loads(previous.read_text())))
            declaration = json.loads((out / 'data/ntm/horizon/block/foundry_mold.json').read_text())
            self.assertEqual(declaration['template'], 'minecraft:beehive')

    def test_missing_claimed_file_is_left_to_visual_and_release_restores_fallback(self):
        with tempfile.TemporaryDirectory() as directory:
            out = Path(directory)
            write_json(out / '.visual-files.json', [self.state])
            generate(SOURCE, out, [self.row], [])
            self.assertFalse((out / self.state).exists())
            write_json(out / '.visual-files.json', [])
            generate(SOURCE, out, [self.row], [])
            variants = json.loads((out / self.state).read_text())['variants']
            self.assertEqual(len(variants), 24)
            self.assertEqual(variants['facing=north,honey_level=0']['model'], 'ntm:block/foundry_mold_0')
            self.assertIn(self.state, json.loads((out / '.content-gen-files.json').read_text()))


if __name__ == '__main__': unittest.main()
