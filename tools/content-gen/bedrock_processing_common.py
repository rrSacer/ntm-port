#!/usr/bin/env python3
"""Shared literal-stack translation for the later bedrock processing machines."""
import json
import re
from pathlib import Path
from anvil_recipes import snapshot
from bedrock_recipes import tables
from blast_furnace_recipes import block_end
from bulk_recipes import Unsupported, calls, clean, split
from foundry_recipes import quantity
from machine_recipes import integer
from generate import lang

BASE = 'src/main/java/com/hbm/'


class Context:
    def __init__(self, source, rows):
        self.source = source
        self.rows = rows
        self.resolver, self.materials, self.shapes, _ = snapshot(source, rows)
        self.resolver.ore['sand'].update(['minecraft:sand', 'minecraft:red_sand'])
        self.byname = {m['name']: m for m in self.materials}
        self.byconstant = {m['constant']: m for m in self.materials}
        self.fragment_material = {r['id'].split('__')[1]: self.byconstant[r['source_material']]['name']
                                  for r in rows if r['source_variable'] == 'bedrock_ore_fragment'}
        self.types, self.grades, _ = tables(source, rows)
        self.names = {r['id']: r['names'] for r in rows}
        self.source_langs = {loc:lang(source / f'src/main/resources/assets/hbm/lang/{old}.lang') for loc,old in [('en_us','en_US'),('pt_br','pt_BR')]}
        self.langs = {loc: json.loads((Path('/home/sacer/Games/horizon/.local/horizon-client/26.3/assets/minecraft/lang') / (loc + '.json')).read_text()) for loc in ('en_us', 'pt_br')}

    def stack(self, expr, ore_type=None):
        expr = expr.strip()
        wrapper = re.fullmatch(r'new ComparableStack\((ItemBedrockOreNew.make\(.*\))\)', expr, re.S)
        if wrapper: expr = wrapper[1]
        make = re.fullmatch(r'ItemBedrockOreNew.make\(BedrockOreGrade\.(\w+),\s*type(?:,\s*(\d+))?\)', expr)
        if make and ore_type:
            return ['ntm:bedrock_ore__' + make[1].lower() + '_' + ore_type['suffix'], int(make[2] or 1)]
        return list(self.resolver.item(expr))

    def ingredient(self, expr, ore_type=None):
        if 'ItemBedrockOreNew.make' in expr:
            ident, count = self.stack(expr, ore_type)
            return {'ids': [ident], 'count': count}
        representative, ids = self.resolver.input(expr)
        return {'ids': ids, 'count': representative[1]}

    def fluid(self, expr):
        expr = re.sub(r'new FluidStack\((?!Fluids\.)([A-Z][A-Z0-9_]*)\s*,', r'new FluidStack(Fluids.\1,', expr)
        return self.resolver.fluid(expr)

    def material(self, expr, ore_type=None):
        m = re.fullmatch(r'new MaterialStack\(Mats\.(MAT_\w+),\s*(.*)\)', expr.strip())
        if m:
            return [self.byconstant[m[1]]['name'], quantity(m[2], self.shapes)]
        m = re.fullmatch(r'ItemBedrockOreNew.toFluid\(type\.(\w+),\s*(\d+)\)', expr.strip())
        if m and ore_type:
            fragment, amount = ore_type[m[1]]
            mat = self.byname[self.fragment_material[fragment]]
            # toFluid checks the ORIGINAL material, without conversion.
            if mat['behavior'] != 'SMELTABLE': return None
            return [mat['name'], self.shapes['FRAGMENT']['amount'] * amount * int(m[2])]
        raise Unsupported('unrecognized MaterialStack: ' + expr)

    def row(self, name, origin, index):
        return {'name': name, 'source': origin, 'source_index': index, 'inputItems': [], 'inputFluids': [],
                'outputItems': [], 'outputFluids': [], 'outputMaterials': []}

    def label(self, row):
        for loc, key in [('pt_br', 'label'), ('en_us', 'label_en')]:
            if row['outputItems']:
                ident, count = row['outputItems'][0]
                title = self.names[ident][loc] if ident in self.names else self.langs[loc].get('item.' + ident.replace(':', '.'), self.langs[loc].get('block.' + ident.replace(':', '.'), ident))
                row[key] = title + (f' ×{count}' if count > 1 else '')
            elif row['outputMaterials']:
                material = row['outputMaterials'][0][0]
                title = self.source_langs[loc].get('hbmmat.' + self.byname[material]['names'][0].lower(), material)
                row[key] = ('Metal: ' if loc == 'pt_br' else 'Molten: ') + title
            else:
                fluid = row['outputFluids'][0][0]
                row[key] = self.source_langs[loc].get('hbmfluid.' + fluid, fluid)
        if not (row['outputItems'] or row['outputMaterials'] or row['outputFluids']):
            raise Unsupported('no output in this mode')
        return row


def body(source, filename):
    path = BASE + 'inventory/recipes/' + filename + '.java'
    text = clean((source / path).read_text())
    opening = text.index('{', text.index('public void registerDefaults()'))
    return path, text, opening, block_end(text, opening)


def events(ctx, filename, pattern):
    path, text, start, end = body(ctx.source, filename)
    raw = [(p, args) for p, args, _ in calls(text, pattern) if start < p < end]
    loop = re.search(r'for\(BedrockOreType type : BedrockOreType.values\(\)\)\s*\{', text[start:end])
    lo = start + loop.end() if loop else -1
    hi = block_end(text, lo - 1) if loop else -1
    done = False
    for i, (pos, args) in enumerate(raw):
        origin = path + ':' + str(text[:pos].count('\n') + 1)
        if lo <= pos < hi:
            if done: continue
            done = True
            for ore_type in ctx.types:
                for j, (p, a) in enumerate(raw):
                    if lo <= p < hi:
                        yield j, path + ':' + str(text[:p].count('\n') + 1), a, ore_type
        else: yield i, origin, args, None


def skip(target, name, source, expr, error):
    target.append({'name': name, 'source': source, 'expression': expr, 'reason': str(error)})


def write(output, name, data):
    data = {'schema': 1, 'machine': name, 'profile': {'external_mods': [], 'ore_dictionary': 'conservative NTM + legacy vanilla snapshot'}, **data}
    path = output / 'machine-recipes' / (name + '.json')
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, separators=(',', ':')) + '\n')
    print(json.dumps({name: data['coverage']}))
    return data


def config(source, relative, specs):
    path = BASE + relative
    text = clean((source / path).read_text()); values = {}; origins = {}
    for key, pattern in specs.items():
        match = re.search(pattern, text)
        if match is None: raise Unsupported('machine config drift: ' + key)
        values[key] = integer(match[1]); origins[key] = path + ':' + str(text[:match.start()].count('\n') + 1)
    return {**values, 'sources': origins}
