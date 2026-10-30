#!/usr/bin/env python3
"""Source outcomes, overwrite semantics, molten units and mode precedence."""
import json
import tempfile
import unittest
from pathlib import Path
from pyrooven_recipes import export as pyro_export, fuel_heat
from electrolyser_recipes import export as electro_export
from arc_furnace_recipes import export as arc_export

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT.parent / 'ntm-src'


class LaterBedrockContent(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog = json.loads((ROOT/'content/ids.json').read_text())
        cls.rows = {r['id']:r for r in cls.catalog['entries']}
        cls.pyro = json.loads((ROOT/'content/machine-recipes/pyrooven.json').read_text())
        cls.electro = json.loads((ROOT/'content/machine-recipes/electrolyser.json').read_text())
        cls.arc = json.loads((ROOT/'content/machine-recipes/arc_furnace.json').read_text())

    def test_machines_source_alias_and_electrodes(self):
        for ident in ['machine_pyrooven','machine_electrolyser','machine_arc_furnace_large']:
            r = self.rows['ntm:'+ident]
            self.assertIn(r['id'],self.catalog['machines'])
            self.assertEqual(r['template'],'minecraft:beehive')
            self.assertEqual(r['logic']['level'],{'prop':'honey_level','max':5})
        self.assertEqual(self.rows['ntm:machine_arc_furnace_large']['source_variable'],'machine_arc_furnace')
        for name,uses in [('graphite',10),('lanthanium',100),('desh',500),('saturnite',1500)]:
            r=self.rows['ntm:arc_electrode__'+name]
            self.assertEqual((r['metadata'],r['electrode']['max_uses']),(['graphite','lanthanium','desh','saturnite'].index(name),uses))
            self.assertEqual(r['max_stack_size'],1)
            self.assertIn(r['electrode']['burnt_item'],self.rows)

    def test_pyro_all_auto_heat_is_final_and_soot_setter_overwrites(self):
        self.assertEqual(self.pyro['coverage']['expanded'],71)
        self.assertEqual(self.pyro['coverage']['solid_fuel_auto'],27)
        self.assertEqual(self.pyro['coverage']['bedrock'],30)
        self.assertEqual(self.pyro['skipped'],[])
        heat=fuel_heat(SOURCE)
        self.assertEqual(heat['KEROSENE'],2560000)
        self.assertEqual(heat['BALEFIRE'],256000000)
        bf=next(r for r in self.pyro['recipes'] if r['outputItems']==[['ntm:solid_fuel_bf',1]])
        self.assertEqual(bf['inputFluids'],[['balefire',46]])
        soot=next(r for r in self.pyro['recipes'] if r.get('overwritten_source_fields'))
        self.assertEqual(soot['outputFluids'],[['carbondioxide',1000]])
        self.assertEqual(soot['overwritten_source_fields'],[{'outputFluids':[['hydrogen',250]]}])

    def test_pyro_bedrock_roasting(self):
        recipe=next(r for r in self.pyro['recipes'] if r['inputItems'] and r['inputItems'][0]['ids']==['ntm:bedrock_ore__base_light'])
        self.assertEqual((recipe['duration'],recipe['power']),(10,10000))
        self.assertEqual(recipe['outputFluids'],[['vitriol',50]])
        self.assertEqual(recipe['outputItems'],[['ntm:bedrock_ore__base_roasted_light',1]])
        self.assertEqual(recipe['inputFluids'],[])

    def test_electrolyser_fluid_duration_and_empty_slot(self):
        self.assertEqual(len(self.electro['fluid']['recipes']),8)
        water=next(r for r in self.electro['fluid']['recipes'] if r['inputFluids']==[['water',2000]])
        self.assertEqual((water['duration'],water['outputFluids']),(10,[['hydrogen',200],['oxygen',200]]))
        alumina=next(r for r in self.electro['fluid']['recipes'] if r['inputFluids']==[['alumina',200]])
        self.assertEqual(alumina['duration'],40)
        self.assertEqual(alumina['outputFluidSlots'][1],['none',0])
        self.assertEqual(alumina['outputItems'],[['ntm:powder_aluminium',7],['ntm:fluorite',2]])

    def test_electrolyser_metal_quanta_and_nonmetal_fallback(self):
        rows=self.electro['metal']['recipes']
        self.assertEqual(len(rows),36)
        iron=next(r for r in rows if r['inputItems'][0]['ids']==['ntm:crystal_iron'])
        self.assertEqual(iron['outputMaterials'],[['iron',432],['titanium',144]])
        self.assertEqual((iron['duration'],iron['inputFluids']),(600,[['nitric_acid',100]]))
        light=next(r for r in rows if r['inputItems'][0]['ids']==['ntm:bedrock_ore__primary_first_light'])
        self.assertEqual(light['outputMaterials'],[['iron',576],['copper',288]])
        nonmetal=next(r for r in rows if r['inputItems'][0]['ids']==['ntm:bedrock_ore__crumbs_nonmetal'])
        self.assertEqual(nonmetal['outputMaterials'],[['slag',144]])
        self.assertEqual(nonmetal['outputItems'],[['ntm:bedrock_ore_fragment__coal',18],['ntm:bedrock_ore_fragment__sulfur',18]])

    def test_arc_modes_quanta_and_no_occupied_input_twice(self):
        solid=self.arc['solid']['recipes'];fluid=self.arc['fluid']['recipes']
        flint=next(r for r in fluid if r['inputItems'][0]['ids']==['minecraft:flint'])
        self.assertEqual(flint['outputMaterials'],[['silicon',36]])
        iron=next(r for r in fluid if r['inputItems'][0]['ids']==['ntm:bedrock_ore__primary_first_light'])
        self.assertEqual(iron['outputMaterials'],[['iron',360],['copper',144]])
        self.assertFalse(any(r['inputItems'][0]['ids']==['ntm:bedrock_ore__primary_first_nonmetal'] for r in fluid))
        roast=next(r for r in solid if r['inputItems'][0]['ids']==['ntm:bedrock_ore__sulfuric_roasted_light'])
        self.assertEqual(roast['outputItems'],[['ntm:bedrock_ore__sulfuric_arc_light',4]])
        for mode in ['solid','fluid']:
            used=[]
            for r in self.arc[mode]['recipes']:used.extend(r['inputItems'][0]['ids'])
            self.assertEqual(len(used),len(set(used)))
        self.assertTrue(self.arc['collisions'])
        self.assertEqual(self.arc['machine_config']['molten_capacity_quanta'],82944)

    def test_all_exported_id_material_and_fluid_graphs(self):
        materials={m['name'] for m in json.loads((ROOT/'content/machine-recipes/foundry.json').read_text())['materials']}
        fluids={f['string_id'] for f in json.loads((ROOT/'content/fluids.json').read_text())['fluids']}
        for section in [self.pyro,self.electro['fluid'],self.electro['metal'],self.arc['solid'],self.arc['fluid']]:
            for r in section['recipes']:
                self.assertTrue(r['label']); self.assertTrue(r['label_en'])
                self.assertGreater(r['duration'],0)
                ids=[i for s in r['inputItems'] for i in s['ids']]+[s[0] for s in r['outputItems']]
                for ident in ids:
                    if ident.startswith('ntm:'):self.assertIn(ident,self.rows)
                for mat,amount in r['outputMaterials']:self.assertIn(mat,materials);self.assertGreater(amount,0)
                for fluid,amount in r['inputFluids']+r['outputFluids']:self.assertIn(fluid,fluids);self.assertGreater(amount,0)
            for r in section['skipped']:self.assertTrue(r['reason']);self.assertTrue(r['source'].startswith('src/main/java/'))

    def test_fresh_directory_determinism(self):
        with tempfile.TemporaryDirectory() as d:
            for fn,name in [(pyro_export,'pyrooven'),(electro_export,'electrolyser'),(arc_export,'arc_furnace')]:
                result=fn(SOURCE,Path(d),list(self.rows.values()))
                self.assertEqual(result,getattr(self,{'pyrooven':'pyro','electrolyser':'electro','arc_furnace':'arc'}[name]))


if __name__=='__main__':unittest.main()
