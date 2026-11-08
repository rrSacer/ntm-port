#!/usr/bin/env python3
"""Spent-fuel items that PUREX recipes consume, declared as static identities."""
import re
from bulk_recipes import clean
from generate import lang, registration

BASE = 'src/main/java/com/hbm/'


def build(source):
    items = (source / (BASE + 'items/ModItems.java')).read_text()
    tr = {loc: lang(source / f'src/main/resources/assets/hbm/lang/{old}.lang')
          for loc, old in [('en_us','en_US'),('pt_br','pt_BR')]}
    variables = re.findall(r'^\s*(waste_\w+)\s*=\s*new ItemDepletedFuel\(\)', clean(items), re.M)
    if len(variables) != 16: raise ValueError('Depleted fuel declarations drift: %d' % len(variables))
    rows = []
    for var in variables:
        name, texture = registration(items, var, 'item')
        rows.append({'id': name, 'type': 'item', 'category': 'component', 'source_variable': var,
                     'source_texture': f'src/main/resources/assets/hbm/textures/items/{texture or name}.png',
                     'names': {loc: loc_tr[f'item.{name}.name'] for loc, loc_tr in tr.items()},
                     'behavior_gap': 'Processing identity only; depletion, heat and radiation of the spent fuel are not reproduced by this item'})
    return rows
