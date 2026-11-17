"""tools/chunk-radiation-oracle/fixture.json against the Java it was cut from (Onda 1, fatia 4a).

`crates/ntm-core/src/chunk_radiation.rs` compares the port's field with the fixture; this checks the fixture is still
what `ChunkRadiationHandlerSimple`, cut from the HBM source and run on `java.util.HashMap`, gives.
"""
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class ChunkRadiationOracle(unittest.TestCase):
    @unittest.skipUnless(shutil.which('javac'), 'needs a JDK')
    def test_the_chunk_radiation_oracle_fixture_is_current(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get('HAZMAT_ORACLE_SCRATCH')) as scratch:
            run = subprocess.run(
                [sys.executable, str(ROOT / 'tools/chunk-radiation-oracle/run.py'), '--scratch', scratch + '/o', '--check'],
                capture_output=True, text=True)
        self.assertEqual(run.returncode, 0, run.stderr)


if __name__ == '__main__':
    unittest.main()
