#!/usr/bin/env python3
"""The twelve source NTM anvils, on the shared machine state template."""
import re
from generate import lang, registration

BLOCKS = 'src/main/java/com/hbm/blocks/ModBlocks.java'
ANVIL = 'src/main/java/com/hbm/blocks/machine/NTMAnvil.java'


def build(source):
    text = (source / BLOCKS).read_text()
    tiers = dict((name, int(value)) for name, value in re.findall(
        r'public static final int (TIER_\w+) = (\d+);', (source / ANVIL).read_text()))
    translations = {loc: lang(source / f'src/main/resources/assets/hbm/lang/{old}.lang')
                    for loc, old in [('en_us', 'en_US'), ('pt_br', 'pt_BR')]}
    result = []
    for match in re.finditer(r'\b(anvil_\w+)\s*=\s*new NTMAnvil\(Material.iron,\s*(?:NTMAnvil\.)?(\w+)\)', text):
        var, tier = match.groups()
        tier = int(tier) if tier.isdigit() else tiers[tier]
        name, texture = registration(text, var, 'block')
        result.append({'id': name, 'source_variable': var, 'source_class': 'NTMAnvil',
                       'type': 'block', 'category': 'machine', 'tier': tier,
                       'template': 'minecraft:beehive',
                       'names': {loc: tr['tile.' + name + '.name'] for loc, tr in translations.items()},
                       'source_texture': f'src/main/resources/assets/hbm/textures/blocks/{texture}.png',
                       'textures': {'side': 'blocks/' + texture, 'front': 'blocks/' + texture,
                                    'bottom': 'blocks/' + texture,
                                    'top': 'blocks/anvil_murky' if var == 'anvil_murky' else 'blocks/' + texture},
                       'block_properties': {'hardness': 5.0, 'explosion_resistance': 100.0,
                                            'sounds': 'minecraft:anvil'},
                       'source_location': BLOCKS + ':' + str(text[:match.start()].count('\n') + 1),
                       'model_gap': 'Oriented cube fallback; source anvil OBJ/bounds belong to visual pipeline',
                       'behavior_gap': 'Source BlockFallingNT gravity/falling damage absent from beehive template; machine interaction belongs to runtime'})
    if len(result) != 12: raise ValueError('NTM anvil registration count drift')
    return result
