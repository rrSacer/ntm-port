"""Registry validation catches runtime rejection before packaging."""
import json
import tempfile
import unittest
from pathlib import Path
from vanilla_references import VanillaReferences, sound_block


class VanillaReferenceTests(unittest.TestCase):
    def checker(self):
        return VanillaReferences(registries={
            'minecraft:block': {'entries': dict.fromkeys(['minecraft:poppy', 'minecraft:short_grass', 'minecraft:stone'])},
            'minecraft:item': {'entries': dict.fromkeys(['minecraft:poppy', 'minecraft:iron_ingot'])}})

    def test_sound_alias_is_a_current_block_not_a_sound_event(self):
        checker = self.checker()
        self.assertEqual(sound_block('minecraft:grass'), 'minecraft:short_grass')
        checker.require('block', sound_block('minecraft:grass'), 'sounds')
        for invalid in ('minecraft:grass', 'minecraft:block.grass.break', 'minecraft:iron_ingot'):
            with self.assertRaisesRegex(ValueError, 'unknown 26.3 vanilla block'):
                checker.require('block', invalid, 'sounds')

    def test_block_and_item_templates_have_distinct_registries(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); path = root / 'data/ntm/horizon/block/flower.json'
            path.parent.mkdir(parents=True)
            for data in ({'template': 'minecraft:missing'}, {'template': 'minecraft:poppy', 'properties': {'sounds': 'minecraft:grass'}}):
                path.write_text(json.dumps(data))
                with self.assertRaises(ValueError): self.checker().validate_content(root)
            path.write_text(json.dumps({'template': 'minecraft:poppy', 'properties': {'sounds': 'minecraft:short_grass'}}))
            self.checker().validate_content(root)

    def test_nested_recipe_loot_machine_and_worldgen_references(self):
        checker = self.checker()
        invalid = [
            {'result': {'id': 'minecraft:missing'}},
            {'key': {'A': 'minecraft:missing'}},
            {'inputItems': [{'ids': ['minecraft:missing'], 'count': 1}]},
            {'outputItems': [['minecraft:missing', 1]]},
            {'type': 'minecraft:item', 'name': 'minecraft:missing'},
            {'state': {'Name': 'minecraft:missing'}},
            {'target': {'block': 'minecraft:missing'}},
            {'matching_blocks': ['minecraft:missing']},
        ]
        for node in invalid:
            with self.subTest(node=node), self.assertRaises(ValueError): checker.walk(node, 'fixture')
        # Resource types, textures and models are not block or item identities.
        checker.walk({'type': 'minecraft:ore', 'textures': {'all': 'minecraft:block/stone'},
                      'parent': 'minecraft:block/cross', 'result': {'id': 'minecraft:iron_ingot'}}, 'fixture')

    def test_actual_catalog_uses_current_registry(self):
        root = Path(__file__).resolve().parents[2]
        self.assertGreater(VanillaReferences().validate_content(root / 'content'), 3000)


if __name__ == '__main__': unittest.main()
