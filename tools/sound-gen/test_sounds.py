"""Independent contract checks against the unconverted HBM definitions."""
import hashlib
import json
from pathlib import Path
import unittest

from generate import ROOT, HERE, convert, inventory, split_args, classify_tags


class SoundsTest(unittest.TestCase):
    def test_complete_conversion_preserves_every_event_and_property(self):
        original = json.loads((HERE / 'original-sounds.json').read_text())
        current, categories = convert(original)
        self.assertEqual(len(current), len(original))
        for name, old in original.items():
            new = current[name.lower()]
            self.assertEqual(categories['ntm:' + name.lower()], old['category'])
            self.assertEqual(set(new), set(old) - {'category'})
            self.assertEqual(len(new['sounds']), len(old['sounds']))
            for a, b in zip(old['sounds'], new['sounds']):
                if isinstance(a, str):
                    self.assertEqual(b, 'ntm:' + a.lower())
                else:
                    self.assertEqual(b['name'], 'ntm:' + a['name'].lower())
                    self.assertEqual({k: v for k, v in b.items() if k != 'name'},
                                     {k: v for k, v in a.items() if k != 'name'})
            for key in old.keys() - {'category', 'sounds'}:
                self.assertEqual(new[key], old[key])

    def test_packaged_selection_matches_original_and_original_audio_bytes(self):
        original = json.loads((HERE / 'original-sounds.json').read_text())
        manifest = json.loads((HERE / 'manifest.json').read_text())
        packaged = json.loads((ROOT / 'content/assets/ntm/sounds.json').read_text())
        converted, _ = convert(original)
        self.assertEqual(set(packaged), {n.lower() for n in manifest['selected_events']})
        referenced = set()
        for name, definition in packaged.items():
            self.assertEqual(definition, converted[name])
            for sound in definition['sounds']:
                location = sound if isinstance(sound, str) else sound['name']
                referenced.add(location.removeprefix('ntm:') + '.ogg')
        self.assertEqual(referenced, set(manifest['files']))
        self.assertEqual(set(manifest['files']), {str(p.relative_to(ROOT / 'content/assets/ntm/sounds')) for p in (ROOT / 'content/assets/ntm/sounds').rglob('*.ogg')})
        for name, spec in manifest['files'].items():
            path = ROOT / 'content/assets/ntm/sounds' / name
            self.assertTrue(path.read_bytes().startswith(b'OggS'), str(path))
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), spec['sha256'])

    def test_nondefault_properties_and_subtitles_survive(self):
        current, categories = convert({'Test.Event': {'category': 'player', 'replace': True,
            'subtitle': 'subtitles.hbm.test', 'sounds': [{'name': 'Test/File', 'volume': .3,
            'pitch': 1.7, 'weight': 4, 'stream': True, 'attenuation_distance': 25, 'preload': True},
            {'name': 'hbm:Other.Event', 'type': 'event'}]}})
        self.assertEqual(current['test.event']['subtitle'], 'subtitles.hbm.test')
        self.assertEqual(current['test.event']['sounds'][0], {'name': 'ntm:test/file',
            'volume': .3, 'pitch': 1.7, 'weight': 4, 'stream': True,
            'attenuation_distance': 25, 'preload': True})
        self.assertEqual(current['test.event']['sounds'][1], {'name': 'ntm:other.event', 'type': 'event'})
        self.assertEqual(categories, {'ntm:test.event': 'player'})

    def test_import_refuses_conflicting_copyright_metadata(self):
        with self.assertRaisesRegex(ValueError, 'copyright notice'):
            classify_tags([{'Copyright': 'Sounds Copyrighted by Sound Dogs.'}])
        self.assertEqual(classify_tags([{'ARTIST': 'qubodup', 'COMMENTS': 'CC0 mix'}]), 'CC0-1.0')

    def test_java_arguments_include_nested_random_expressions_and_strings(self):
        self.assertEqual(split_args('x, y, z, "hbm:test", getVolume(1F), .5F + rand.nextFloat() * .25F);'),
                         ['x', 'y', 'z', '"hbm:test"', 'getVolume(1F)', '.5F + rand.nextFloat() * .25F'])


if __name__ == '__main__':
    unittest.main()
