"""Every content multiblock (content/data/ntm/horizon_machine/*.json) within the engine's limits
(horizon-world `mod_multiblocks::parse`): the main box holds the core and spans at most 16 across and 32 up; each extra
box spans as much and lies as far from the core at most. A file past them is only a warning at load and the block is
placed as one cell (the LPW2 was, 20 across, until its x 7..10 went into an extra box)."""
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MOST = (15, 31, 15)


class MultiblockLimits(unittest.TestCase):
    def test_every_multiblock_is_within_the_engines_limits(self):
        checked = 0
        for path in sorted((ROOT / 'content/data/ntm/horizon_machine').glob('*.json')):
            m = json.loads(path.read_text()).get('multiblock')
            if not m:
                continue
            lo, hi = m['min'], m['max']
            for i in range(3):
                self.assertTrue(lo[i] <= 0 <= hi[i] and hi[i] - lo[i] <= MOST[i], (path.name, 'main', i))
            for box in m.get('extra', []):
                for i in range(3):
                    a, b = box['min'][i], box['max'][i]
                    self.assertTrue(a <= b and b - a <= MOST[i] and max(abs(a), abs(b)) <= MOST[i], (path.name, 'extra', i))
            checked += 1
        self.assertGreater(checked, 20)


if __name__ == '__main__':
    unittest.main()
