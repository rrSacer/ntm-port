#!/usr/bin/env python3
"""Check oil substrate protection, hot-dry compensation and floor bounds."""
import json
import tempfile
import unittest
from pathlib import Path
from oil_worldgen import generate,protect_geode,geode,HOT_DRY


class OilWorldgen(unittest.TestCase):
    def setUp(self):
        self.output={}
        with tempfile.TemporaryDirectory() as directory:
            p=Path(directory)/'registries.json'
            p.write_text(json.dumps({'minecraft:block':{'entries':{k:{} for k in ['minecraft:stone','minecraft:air','minecraft:bedrock','minecraft:diamond_ore','minecraft:water']}}}))
            rows=[{'id':'ntm:ore_oil','type':'block'},{'id':'ntm:ingot_steel','type':'item'}]
            generate(self.output.__setitem__,rows)
            protect_geode(self.output.__setitem__,rows,p)

    def get(self,kind,name):return self.output[f'data/ntm/worldgen/{kind}/{name}.json']

    def test_protects_every_known_nonstone_substrate(self):
        tags={p.removeprefix('data/ntm/tags/block/').removesuffix('.json'):d['values'] for p,d in self.output.items() if '/tags/block/' in p}
        direct={i for values in tags.values() for i in values if not i.startswith('#')}
        self.assertEqual(direct,{'minecraft:air','minecraft:bedrock','minecraft:diamond_ore','minecraft:water','ntm:ore_oil'})
        self.assertTrue(all(v[1:].removeprefix('ntm:') in tags for v in tags['oil_protected']))

    def test_hot_dry_compensation_is_additional_and_biome_restricted(self):
        self.assertEqual(self.output['data/ntm/tags/worldgen/biome/oil_hot_dry.json']['values'],HOT_DRY)
        for base,extra,target in [('oil_bubble','oil_bubble_hot_dry',1/33),('oil_sand','oil_sand_hot_dry',1/66)]:
            first=self.get('placed_feature',base)['placement'][0]['chance']
            second=self.get('placed_feature',extra)['placement'][0]['chance']
            probability=1/first+1/second-1/(first*second)
            self.assertLess(abs(probability/target-1),.004)
            self.assertEqual(self.get('structure',extra)['biomes'],'#ntm:oil_hot_dry')
        self.assertEqual(self.get('structure','oil_bubble')['biomes'],'#minecraft:is_overworld')

    def test_bedrock_deposit_stays_in_five_adapted_layers(self):
        cfg=self.get('configured_feature','oil_bedrock');placement=self.get('placed_feature','oil_bedrock')['placement']
        self.assertEqual(cfg['type'],'minecraft:disk');self.assertEqual(cfg['half_height'],2)
        self.assertEqual(placement[-1]['height']['value']['absolute'],-62)
        self.assertEqual(set(cfg['target']['blocks']),{'minecraft:bedrock','minecraft:stone','minecraft:deepslate'})
        self.assertEqual(placement[0]['chance'],200)

    def test_sand_only_and_bounded_surface_height(self):
        cfg=self.get('configured_feature','oil_sand');height=self.get('placed_feature','oil_sand')['placement'][-1]['height']
        self.assertEqual(cfg['target']['blocks'],['minecraft:sand','minecraft:red_sand'])
        self.assertEqual(height['min_inclusive']['absolute']-cfg['half_height'],56)
        self.assertEqual(height['max_inclusive']['absolute']+cfg['half_height'],71)

    def test_chosen_shape_uses_three_overlapping_ore_lobes(self):
        cfg=self.get('configured_feature','oil_bubble')
        self.assertEqual(cfg['type'],'minecraft:random_selector')
        placed=cfg['default'];self.assertEqual(placed['feature']['size'],64)
        self.assertEqual(placed['feature']['targets'][0]['target']['block'],'minecraft:stone')
        self.assertEqual(placed['placement'][0],{'type':'minecraft:count','count':3})
        jitter=placed['placement'][1];self.assertEqual(jitter['y'],0)
        self.assertEqual(jitter['x'],{'type':'minecraft:uniform','min_inclusive':-2,'max_inclusive':2})
        self.assertEqual(jitter['x'],jitter['z'])

    def test_geode_cannot_make_air_or_replace_other_ores(self):
        cfg=geode()
        self.assertEqual(cfg['blocks']['filling_provider']['state']['id'],'ntm:ore_oil')
        for key in ['inner_layer_provider','alternate_inner_layer_provider','middle_layer_provider','outer_layer_provider']:
            self.assertEqual(cfg['blocks'][key]['state']['id'],'minecraft:stone')
        self.assertEqual(cfg['blocks']['cannot_replace'],'#ntm:oil_protected')
        self.assertEqual(cfg['crack']['generate_crack_chance'],0)
        self.assertEqual(cfg['use_potential_placements_chance'],0)


if __name__=='__main__':unittest.main()
