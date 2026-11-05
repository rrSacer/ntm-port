"""Independent contract checks against the unconverted HBM definitions."""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

from generate import ROOT, HERE, convert, inventory, split_args, classify_tags
from validate import validate_tree


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
        self.assertEqual(classify_tags([{'ARTIST': 'qubodup', 'COMMENTS': 'CC0, mix of CC0 sounds https://freesound.org/example'}]), 'CC0-1.0')

    def test_rights_notices_are_checked_in_every_metadata_field(self):
        for field in ['Software', 'Encoder', 'Encoded_by', 'TITLE', 'COMMENT', 'Copyright']:
            for value in ['Sounds Copyrighted by Sound Dogs.', 'Copyright Example Studio. All rights reserved.']:
                with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                    classify_tags([{field: value}])

    def test_license_negations_incidental_mentions_and_conflicts_are_rejected(self):
        for tags in [
            {'Copyright': 'Copyright Example Studio. All rights reserved.', 'COMMENT': 'Edited with LGPL tools'},
            {'Copyright': 'Copyright Example Studio.', 'COMMENT': 'Not CC0'},
            {'Copyright': 'All rights reserved', 'LICENSE': 'CC0'},
            {'COMMENT': 'Not CC0'}, {'COMMENT': 'Not LGPL'},
            {'Copyright': 'Copyright Studio', 'COMMENT': 'Edited with LGPL tools'},
            {'LICENSE': 'CC0', 'COMMENT': 'Licensed under LGPL-3.0-only'},
            {'LICENSE': 'CC0', 'licence': 'All rights reserved'},
            {'LICENSE': 'CC0', 'licence': 'CC-BY-NC'},
            {'LICENSE': 'CC0', 'COMMENT': 'All rights are reserved'},
            {'LICENSE': 'CC0', 'COMMENT': 'Not licensed as CC0'},
            {'LICENSE': 'CC0', 'COMMENT': 'License: CC-BY-NC'},
            {'COMMENT': 'NotCC0'},
            {'Copyright': 'Example Studio'},
            {'Rights': 'Example Studio'},
        ]:
            with self.subTest(tags=tags), self.assertRaises(ValueError):
                classify_tags([tags])
        self.assertEqual(classify_tags([{'LICENSE': 'CC0-1.0', 'Copyright': 'Copyright Studio'}]), 'CC0-1.0')
        self.assertEqual(classify_tags([{'LICENSE': 'LGPL-3.0-only', 'Copyright': 'Copyright Studio'}]), 'LGPL-3.0-only')

    def test_only_the_known_encoder_notice_is_exempted(self):
        notice = 'File created by GoldWave.  GoldWave copyright (C) Chris Craig'
        self.assertEqual(classify_tags([{'Software': notice}]), 'LGPL-3.0-only')
        self.assertEqual(classify_tags([{'Software': notice + ';Sonic Foundry Sound Forge 6.0'}]), 'LGPL-3.0-only')
        for suffix in [';Sounds Copyrighted by Sound Dogs.', ';Copyright Studio', ';All rights reserved']:
            with self.subTest(suffix=suffix), self.assertRaises(ValueError):
                classify_tags([{'Software': notice + suffix}])

    def test_allowed_license_cannot_mask_another_values_declaration(self):
        for value in [
            'CC0 does not apply to this recording.',
            'CC0 is not the license of this sound.',
            'Licensed under GPL-3.0-only',
            'License: unknown custom terms',
            'CC0, mix of CC0 sounds https://freesound.org/example but CC0 does not apply.',
        ]:
            for field in ['COMMENT', 'COMMENTS', 'TITLE', 'Software', 'Encoder', 'Encoded_by']:
                for allowed in [[], [{'LICENSE': 'CC0'}], [{'LICENSE': 'LGPL-3.0-only'}]]:
                    for groups in [allowed + [{field: value}], [{field: value}] + allowed]:
                        with self.subTest(groups=groups), self.assertRaises(ValueError):
                            classify_tags(groups)

    def test_remuxed_ogg_with_contradictory_tags_is_rejected_by_tree_guard(self):
        source = ROOT / 'content/assets/ntm/sounds/block/steamengineoperate.ogg'
        with tempfile.TemporaryDirectory(prefix='sound-license-contract-', dir=ROOT.parent) as scratch:
            root = Path(scratch)
            target = root / 'contradictory.ogg'
            for comment in ['CC0 does not apply to this recording.',
                            'CC0 is not the license of this sound.',
                            'Licensed under GPL-3.0-only']:
                subprocess.run(['ffmpeg', '-v', 'error', '-y', '-i', str(source),
                                '-map_metadata', '-1', '-c', 'copy', '-metadata', 'LICENSE=CC0',
                                '-metadata', 'COMMENT=' + comment, str(target)], check=True)
                with self.subTest(comment=comment), self.assertRaises(ValueError):
                    validate_tree(root)

    def test_java_arguments_include_nested_random_expressions_and_strings(self):
        self.assertEqual(split_args('x, y, z, "hbm:test", getVolume(1F), .5F + rand.nextFloat() * .25F);'),
                         ['x', 'y', 'z', '"hbm:test"', 'getVolume(1F)', '.5F + rand.nextFloat() * .25F'])


if __name__ == '__main__':
    unittest.main()
