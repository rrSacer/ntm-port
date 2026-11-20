#!/usr/bin/env python3
"""Foundry source contracts: quantities, precedence, conversions and live IDs."""
import json
import tempfile
import unittest
from collections import Counter
from pathlib import Path
from foundry_recipes import export, item_table, number, quantity, Unsupported
from foundry_catalog import BLOCKS

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT.parent / 'ntm-src'


class FoundryContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog = json.loads((ROOT / 'content/ids.json').read_text())
        cls.rows = {r['id']:r for r in cls.catalog['entries']}
        cls.data = json.loads((ROOT / 'content/machine-recipes/foundry.json').read_text())
        cls.mats = {m['name']:m for m in cls.data['materials']}

    def test_material_identities_colors_and_conversion(self):
        self.assertEqual(len(self.mats), 109)
        self.assertEqual(Counter(m['behavior'] for m in self.mats.values()), {'SMELTABLE':78,'ADDITIVE':4,'NOT_SMELTABLE':27})
        self.assertEqual(len({m['id'] for m in self.mats.values()}), 109)
        self.assertEqual(self.mats['iron']['id'], 2600)
        self.assertEqual(self.mats['iron']['colors'], [0xFFFFFF,0xFFA259,0x353535])
        self.assertEqual(self.mats['obsidian']['colors'], [0x3D234D]*3)
        self.assertEqual(self.mats['graphite']['colors'], [0xFF4A00,0xFF4A00,0x802000])
        self.assertEqual(self.mats['emerald']['conv'], [4,3])
        self.assertEqual(self.mats['emerald']['smelts_into'], 'beryllium')
        for m in self.mats.values():
            self.assertIn(m['smelts_into'], self.mats)
            self.assertTrue(all(0 <= c <= 0xFFFFFF for c in m['colors']))
            self.assertTrue(all(n > 0 for n in m['conv']))

    def test_quanta_and_floor_arithmetic(self):
        for name, value in [('quantum',1),('nugget',8),('ingot',72),('billet',48),('block',648),('castplate',216),('heavybarrel',432),('quart',162)]:
            self.assertEqual(self.data['quanta'][name], value)
        self.assertEqual(quantity('INGOT.q(6, 16)', self.data['shapes']), 27)
        self.assertEqual(number('_ES + 03', {'_ES':20000}), 20003)
        with self.assertRaises(Unsupported): number('__import__("os")', {})

    def test_ore_overrides_and_fixed_items_are_unconverted(self):
        items = self.data['items']
        for ident, value in [('minecraft:iron_ingot',[['iron',72]]),
                             ('minecraft:iron_ore',[['iron',144],['titanium',24],['stone',162]]),
                             ('minecraft:rail',[['iron',27]]), ('minecraft:minecart',[['iron',360]]),
                             ('minecraft:powered_rail',[['gold',72],['redstone',12]]),
                             ('minecraft:coal',[['coal',72]]), ('minecraft:charcoal',[['carbon',24]]),
                             ('minecraft:diamond',[['diamond',72]]),
                             ('ntm:plate_cast__steel',[['steel',216]]), ('ntm:part_barrel_heavy__steel',[['steel',432]])]:
            self.assertEqual(items[ident], value, ident)
        self.assertNotIn('minecraft:deepslate_iron_ore', items)
        self.assertNotIn('minecraft:copper_ingot', items)
        self.assertEqual(self.data['skipped'], [])
        self.assertEqual(self.data['coverage']['distribution_rules'],44)
        self.assertEqual(items['ntm:ingot_graphite'], [['graphite',72]])

    def test_exact_entries_append_and_scraps_are_singular(self):
        class Resolver:
            byvar = {'scraps':[]}
        materials = [{'name':'iron','constant':'MAT_IRON','names':['Iron'],'smelts_into':'iron','behavior':'SMELTABLE'}]
        items, _, collisions = item_table(Resolver(), materials, {'INGOT':{'prefixes':['ingot'],'amount':72}},
                                           {'ingotIron':['test:iron']}, {'test:iron':[['iron',8]]}, {'ingotIron':[['iron',144]]})
        self.assertEqual(items['test:iron'], [['iron',144],['iron',8]])
        self.assertEqual(collisions, [])
        scraps = [r for r in self.rows.values() if r['source_variable']=='scraps']
        self.assertEqual(len(scraps),82)
        for row in scraps:
            name = row['source_material'][4:].lower()
            self.assertEqual(self.data['items'][row['id']], [[name,72]])
            self.assertEqual(self.mats[name]['id'], row['metadata'])

    def test_alloy_order_frequencies_and_gt6_gate(self):
        active = self.data['alloys']; disabled = self.data['disabled_alloys']
        self.assertEqual(len(active),13); self.assertEqual(len(disabled),3)
        self.assertEqual([r['source_index'] for r in active], [0]+list(range(4,16)))
        self.assertEqual([r['source_index'] for r in disabled], [1,2,3])
        self.assertTrue(all(not r['enabled_by_default'] and r['requires_mod']=='gregtech' for r in disabled))
        self.assertEqual(active[0]['name'],'crucible.steel')
        self.assertEqual(active[0]['frequency'],20)
        self.assertEqual(active[0]['in'],[['iron',16],['carbon',24],['flux',8]])
        self.assertEqual(active[0]['out'],[['steel',16]])
        self.assertEqual(active[1]['out'],[['iron',72],['slag',24]])
        for row in active+disabled:
            self.assertGreater(row['frequency'],0)
            for name,q in row['in']+row['out']:
                self.assertIn(name,self.mats); self.assertGreater(q,0)

    def test_molds_source_cost_and_output_policy(self):
        molds = {r['name']:r for r in self.data['molds']}
        self.assertEqual(len(molds),26)
        self.assertEqual(Counter(r['size'] for r in molds.values()),{0:21,1:5})
        for name,cost,output in [('ingot',72,['minecraft:iron_ingot',1]),
                                 ('ingots',648,['minecraft:iron_ingot',9]), ('plate_cast',216,['ntm:plate_cast__iron',1])]:
            self.assertEqual(molds[name]['cost'],cost); self.assertEqual(molds[name]['outputs']['iron'],output)
        self.assertEqual(molds['block']['outputs']['stone'],['minecraft:stone',1])
        self.assertEqual(molds['wire']['amount'],8)
        self.assertEqual(molds['stamp']['cost'],288)  # source recycling returns 216; don't normalize this difference
        self.assertEqual(molds['c9']['cost'],18); self.assertEqual(molds['c50']['cost'],36)
        self.assertEqual(self.data['mold_gaps'],[])

    def test_firebox_modifiers_wood_and_containers(self):
        fuels = self.data['firebox_fuels']
        for ident,ticks,heat,container in [('minecraft:coal',2000,200,None), ('minecraft:coal_block',20000,200,None),
                                          ('minecraft:charcoal',1600,100,None), ('minecraft:oak_log',300,100,None),
                                          ('minecraft:oak_slab',150,100,None), ('minecraft:lava_bucket',20000,100,'minecraft:bucket'),
                                          ('ntm:solid_fuel',4800,300,None), ('ntm:rocket_fuel',9600,500,None),
                                          ('ntm:solid_fuel_presto_triplet_bf',200000,1500,None), ('ntm:briquette__wood',400,100,None)]:
            self.assertEqual(fuels[ident], {'ticks':ticks,'heat':heat,'container':container})
        self.assertNotIn('minecraft:ladder',fuels)
        self.assertNotIn('minecraft:oak_button',fuels)
        self.assertEqual({r['variable'] for r in self.data['fuel_gaps'] if 'variable' in r},{'book_guide'})

    def test_registered_item_and_block_graph(self):
        vanilla = Path('/home/sacer/Games/horizon/.local/horizon-client/26.3/assets/minecraft/items')
        visual_owned = set(json.loads((ROOT / 'content/.visual-files.json').read_text())) if (ROOT / 'content/.visual-files.json').exists() else set()
        ids = list(self.data['items']) + list(self.data['firebox_fuels'])
        for row in self.data['alloys']+self.data['disabled_alloys']: ids.append(row['icon'])
        for row in self.data['molds']: ids.extend([row['item']]+[out[0] for out in row['outputs'].values()])
        ids.extend(f['container'] for f in self.data['firebox_fuels'].values() if f['container'])
        for ident in ids:
            if ident.startswith('ntm:'): self.assertIn(ident,self.rows)
            else: self.assertTrue((vanilla/(ident.split(':')[1]+'.json')).is_file(),ident)
        for name in BLOCKS:
            ident='ntm:'+name; row=self.rows[ident]
            self.assertIn(ident,self.catalog['machines'])
            self.assertEqual(row['template'],'minecraft:beehive')
            self.assertEqual(row['logic']['level'],{'prop':'honey_level','max':5})
            rel_state=f'assets/ntm/blockstates/{name}.json'
            blockstates=json.loads((ROOT/'content'/rel_state).read_text())
            if rel_state in visual_owned:
                self.assertIn(len(blockstates['variants']), (1, 4))
            else:
                self.assertEqual(len(blockstates['variants']),24)
            loot=json.loads((ROOT/f'content/data/ntm/loot_table/blocks/{name}.json').read_text())
            self.assertIn(ident,json.dumps(loot))

    def test_export_is_reproducible(self):
        with tempfile.TemporaryDirectory(prefix='ntm-foundry-test-') as tmp:
            out=Path(tmp); export(SOURCE,out,list(self.rows.values()))
            self.assertEqual((out/'machine-recipes/foundry.json').read_bytes(),(ROOT/'content/machine-recipes/foundry.json').read_bytes())


if __name__ == '__main__': unittest.main()
