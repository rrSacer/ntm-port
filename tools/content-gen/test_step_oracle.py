"""tools/step-oracle/fixture.json against the Java arithmetic it was cut from (Onda 1, fatia 5d).

`crates/ntm-core/src/walking.rs` compares the port's step with the fixture; this checks the fixture is still what the
transcribed `Entity.moveEntity` arithmetic gives, compiled and run again.
"""
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class StepOracle(unittest.TestCase):
    @unittest.skipUnless(shutil.which('javac'), 'needs a JDK')
    def test_the_step_oracle_fixture_is_current(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get('HAZMAT_ORACLE_SCRATCH')) as scratch:
            run = subprocess.run([sys.executable, str(ROOT / 'tools/step-oracle/run.py'), '--scratch', scratch + '/o', '--check'],
                                 capture_output=True, text=True)
        self.assertEqual(run.returncode, 0, run.stderr)


if __name__ == '__main__':
    unittest.main()
