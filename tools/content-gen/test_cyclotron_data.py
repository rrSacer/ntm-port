#!/usr/bin/env python3
"""The cyclotron's recipes: sources, order and that the embedded file is what they give."""
import json
import unittest
from pathlib import Path
import cyclotron_data as c

SOURCE = Path('/mnt/ssd120/sacer/horizon-ntm/ntm-src')


class CyclotronTables(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.t = c.build()

    def test_embedded_file_is_what_the_sources_give(self):
        self.assertEqual(json.loads(c.OUTPUT.read_text()), self.t)

    def test_all_forty_two_source_lines_are_read_even_the_unspaced_one(self):
        self.assertEqual(self.t['coverage']['recipes'] + self.t['coverage']['skipped'], 42)
        lead = [r for r in self.t['recipes'] if r['source'] == 'CyclotronRecipes.java:70' or r['output'] == 'ntm:powder_ra226']
        self.assertEqual([(r['particle'], r['ore'], r['amat']) for r in lead], [('ntm:part_carbon', 'dustLead', 10)])

    def test_amounts_come_from_the_group_variables_and_the_literal(self):
        amat = {(r['particle'], r['output']): r['amat'] for r in self.t['recipes']}
        self.assertEqual(amat[('ntm:part_lithium', 'ntm:powder_beryllium')], 50)
        self.assertEqual(amat[('ntm:part_beryllium', 'ntm:powder_boron')], 25)
        self.assertEqual(amat[('ntm:part_copper', 'ntm:powder_quartz')], 15)
        self.assertEqual(amat[('ntm:part_plutonium', 'ntm:nugget_schrabidium')], 1000)

    def test_frames_and_item_ingredients(self):
        by = {(r['particle'], r['output']): r for r in self.t['recipes']}
        self.assertEqual(by[('ntm:part_lithium', 'ntm:powder_neptunium')]['ore'], 'dustUranium')
        self.assertEqual(by[('ntm:part_lithium', 'ntm:powder_plutonium')]['ore'], 'dustNeptunium237')
        self.assertIsNone(by[('ntm:part_lithium', 'ntm:powder_zirconium')]['ore'])
        self.assertEqual(by[('ntm:part_lithium', 'ntm:powder_zirconium')]['ingredients'], ['ntm:powder_strontium'])

    def test_what_the_catalog_lacks_is_skipped_with_the_reason_and_nothing_collides(self):
        self.assertEqual(sorted({s['reason'] for s in self.t['skipped']}), ['no item behind dustActinium', 'no item behind dustLanthanium', 'no item behind dustPhosphorus'])
        self.assertEqual(self.t['collisions'], [])


if __name__ == '__main__':
    unittest.main()
