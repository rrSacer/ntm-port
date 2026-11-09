#!/usr/bin/env python3
"""bulk.py must reproduce the committed audit files byte for byte, run after run.

Guards the `claimed` set: item_hooks.json overrides base rows of bulk.json, so
claiming its variables drops those rows (the 1,100-line churn of 499d5908).
"""
import filecmp
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SOURCE = ROOT.parent / 'ntm-src'
OUTPUTS = ['bulk.json', 'bulk-coverage.json']


class BulkIsIdempotent(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not (SOURCE / 'src/main/java/com/hbm/items/ModItems.java').is_file():
            raise unittest.SkipTest('ntm-src checkout not available')

    def run_in_copy(self, work):
        """Run bulk.py in a copy of tools/content-gen and content/, so the repository is untouched."""
        shutil.copytree(HERE, work / 'tools/content-gen', ignore=shutil.ignore_patterns('__pycache__'))
        shutil.copytree(ROOT / 'content', work / 'content',
                        ignore=shutil.ignore_patterns('assets', 'data', 'machine-recipes', 'target'))
        subprocess.run([sys.executable, str(work / 'tools/content-gen/bulk.py'), '--source', str(SOURCE)],
                       check=True, stdout=subprocess.DEVNULL)
        return work / 'tools/content-gen'

    def test_two_runs_match_each_other_and_the_committed_files(self):
        with tempfile.TemporaryDirectory(prefix='ntm-bulk-idem-') as tmp:
            first = self.run_in_copy(Path(tmp) / 'a')
            second = self.run_in_copy(Path(tmp) / 'b')
            for name in OUTPUTS:
                self.assertTrue(filecmp.cmp(first / name, HERE / name, shallow=False),
                                f'{name} differs from the committed file')
                self.assertTrue(filecmp.cmp(first / name, second / name, shallow=False),
                                f'{name} differs between two runs')


if __name__ == '__main__':
    unittest.main()
