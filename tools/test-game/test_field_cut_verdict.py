"""run-player-cut-proof.py `field`: the verdict on the field's key at a reopening (slice 4a). The key is one of the two
commits, chunk by chunk; a chunk dropped (the review's mutant: a non-source entry truncated) or over its values is
neither."""
import importlib.util
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location('cut', Path(__file__).resolve().parent / 'run-player-cut-proof.py')
cut = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cut)

BASE = {'1,0': 6012.827, '0,0': 1411.8021, '2,0': 1842.4926, '5,5': 3.25}
# Before the save: raw, the next update's, loaded. 5,5 is not loaded: its save stays. 4,0 is loaded, at 0 in neither
# instant's map: no save.
PRE = {'1,0': [100000.0, 59395.0, True], '0,0': [1200.5, 8000.25, True], '2,0': [1300.75, 8100.5, True],
       '3,0': [None, 0.125, True], '4,0': [None, 0.0, True], '5,5': [3.0, 2.5, False]}
# The act's save at one instant: before the update (3,0 not in the map: no save) or after it.
NEW = {'1,0': 100000.0, '0,0': 1200.5, '2,0': 1300.75, '5,5': 3.25}
NEW_NEXT = {'1,0': 59395.0, '0,0': 8000.25, '2,0': 8100.5, '3,0': 0.125, '5,5': 3.25}


def mine(first, second=None):
    store = lambda s: {'field': {'store': s}}
    return {'act_before': store(BASE), 'before_cut': {'chunks': PRE}, 'first_reopen': store(first),
            'second_reopen': store(second or first)}


class FieldVerdict(unittest.TestCase):
    def test_the_last_commit_exactly(self):
        self.assertEqual(cut.field_coherent(mine(dict(BASE)))[1]['commit'], 'last')
        self.assertTrue(cut.field_coherent(mine(dict(BASE)))[0])

    def test_the_new_commit_at_either_instant(self):
        for new in (NEW, NEW_NEXT):
            ok, info = cut.field_coherent(mine(dict(new)))
            self.assertEqual((ok, info['commit']), (True, 'new'))

    def test_an_unloaded_chunk_rewritten_is_neither(self):
        self.assertFalse(cut.field_coherent(mine(dict(NEW, **{'5,5': 3.0})))[0])

    def test_two_instants_mixed_are_neither(self):
        self.assertFalse(cut.field_coherent(mine(dict(NEW, **{'2,0': 8100.5})))[0])

    def test_shortest_decimals_read_back_as_their_f32(self):
        shortest = dict(BASE, **{'0,0': 1411.8021000001})
        self.assertEqual(cut.field_coherent(mine(shortest))[1]['commit'], 'last')

    def test_a_non_source_chunk_dropped_from_the_last_commit_is_neither(self):
        dropped = {k: v for k, v in BASE.items() if k != '0,0'}
        ok, info = cut.field_coherent(mine(dropped))
        self.assertEqual((ok, info['commit']), (False, 'neither'))
        self.assertIn('0,0', info['wrong'])

    def test_a_non_source_chunk_dropped_from_the_new_commit_is_neither(self):
        for new, key in ((NEW, '0,0'), (NEW, '2,0'), (NEW, '5,5'), (NEW_NEXT, '3,0'), (NEW_NEXT, '0,0')):
            dropped = {k: v for k, v in new.items() if k != key}
            ok, info = cut.field_coherent(mine(dropped))
            self.assertFalse(ok, key)
            self.assertIn(key, info['wrong'])

    def test_a_chunk_over_its_values_is_neither(self):
        self.assertFalse(cut.field_coherent(mine(dict(NEW, **{'0,0': 9000.0})))[0])

    def test_the_source_lost_at_the_second_reopen_fails(self):
        self.assertFalse(cut.field_coherent(mine(dict(NEW), {k: v for k, v in NEW.items() if k != '1,0'}))[0])


if __name__ == '__main__':
    unittest.main()
