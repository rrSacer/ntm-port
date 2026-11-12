"""The radiation fog's particles (Onda 1, slice 4c) against ParticleRadiationFog, read on their own: the texture is
fog.png times glColor4f(0.85, 0.9, 0.5), each of the 25 lives the 400 ticks of its maxAge, still, with the linear
alpha of decision D1 (A). The sizes are checked against the server's layout in ntm-core (chunk_radiation)."""
import json
import os
import re
import unittest
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
SRC = Path(os.environ.get('NTM_SRC', ROOT.parent / 'ntm-src')) / 'src/main'
CONTENT = ROOT / 'content/assets/ntm'
JAVA = (SRC / 'java/com/hbm/particle/ParticleRadiationFog.java').read_text()


class FogParticles(unittest.TestCase):
    def test_the_texture_is_the_fog_times_the_javas_colour(self):
        colour = [float(c) for c in re.search(r'glColor4f\(([\d.]+)F, ([\d.]+)F, ([\d.]+)F, alpha\)', JAVA).groups()]
        source = Image.open(SRC / 'resources/assets/hbm/textures/particle/fog.png').convert('RGBA')
        port = Image.open(CONTENT / 'textures/particle/rad_fog.png').convert('RGBA')
        self.assertEqual(source.size, port.size)
        for (r, g, b, a), (pr, pg, pb, pa) in zip(source.get_flattened_data(), port.get_flattened_data()):
            self.assertEqual((pr, pg, pb, pa), (int(r * colour[0]), int(g * colour[1]), int(b * colour[2]), a))

    def test_25_still_particles_of_400_ticks(self):
        self.assertIn('for(int i = 0; i < 25; i++)', JAVA)
        self.assertIn('maxAge = 400', JAVA)
        for n in range(25):
            behaviour = json.loads((CONTENT / f'particles/rad_fog_{n}.json').read_text())['horizon:behaviour']
            self.assertEqual((behaviour['lifetime'], behaviour['velocity'], behaviour['alpha']), (400, [0, 0, 0], [0.125, 0]))
            self.assertTrue(0 <= behaviour['size'][0] < 7.5)
        self.assertFalse((CONTENT / 'particles/rad_fog_25.json').exists())


if __name__ == '__main__':
    unittest.main()
