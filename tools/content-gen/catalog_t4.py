#!/usr/bin/env python3
"""Spent-fuel items that PUREX recipes consume, declared as static identities."""
import re
from bulk_recipes import clean
from generate import lang, registration

BASE = 'src/main/java/com/hbm/'


# Non-cube block classes declared as static blocks with the closest vanilla template.
# Layered blocks (BlockLayering) are not here: their height steps need a per-layer model set.
# The geometry, connections, climbing, damage, layers and fluid storage are not reproduced;
# each row says so in behavior_gap. Ladders keep the vanilla ladder template (climbable).
SHAPED_BLOCKS = [
    *[(v, 'minecraft:iron_bars', 'Pipe geometry and connections approximated by vanilla iron bars; decorative only')
      for v in ['deco_pipe', 'deco_pipe_rusted', 'deco_pipe_green', 'deco_pipe_green_rusted', 'deco_pipe_red',
                'deco_pipe_marked', 'deco_pipe_rim', 'deco_pipe_rim_rusted', 'deco_pipe_rim_green',
                'deco_pipe_rim_green_rusted', 'deco_pipe_rim_red', 'deco_pipe_rim_marked', 'deco_pipe_framed',
                'deco_pipe_framed_rusted', 'deco_pipe_framed_green', 'deco_pipe_framed_green_rusted',
                'deco_pipe_framed_red', 'deco_pipe_framed_marked', 'deco_pipe_quad', 'deco_pipe_quad_rusted',
                'deco_pipe_quad_green', 'deco_pipe_quad_green_rusted', 'deco_pipe_quad_red', 'deco_pipe_quad_marked']],
    *[(v, 'minecraft:ladder', 'Climbing is vanilla; source ladder model and tier speed are not reproduced')
      for v in ['ladder_sturdy', 'ladder_gold', 'ladder_copper', 'ladder_titanium', 'ladder_steel']],
    *[(v, 'minecraft:iron_bars', 'Wire geometry approximated by vanilla iron bars; source contact damage and effects are not reproduced')
      for v in ['barbed_wire', 'barbed_wire_fire', 'barbed_wire_poison', 'barbed_wire_acid', 'barbed_wire_wither', 'barbed_wire_ultradeath']],
    *[(v, 'minecraft:iron_block', 'Full-cube approximation; the source barrel fluid contents and interaction are not reproduced')
      for v in ['red_barrel', 'pink_barrel', 'yellow_barrel', 'vitrified_barrel', 'lox_barrel', 'taint_barrel']],
    # cm_block, cm_sheet, cm_engine, cm_circuit and cm_tank are EnumCMMaterials variants (lang keys per material): not declared here.
    *[(v, 'minecraft:iron_block', 'Full-cube approximation; custom machine (CM) structure is not reproduced')
      for v in ['cm_flux', 'cm_heat']],
]


def java_block_properties(stmt):
    """Hardness, blast resistance, sound and tool rule of one ModBlocks declaration."""
    hardness = float(re.search(r'setHardness\(([\d.]+)F?\)', stmt)[1])
    resistance = float(re.search(r'setResistance\(([\d.]+)F?\)', stmt)[1])
    cls = re.search(r'new (\w+)\(', stmt)[1]
    step = re.search(r'setStepSound\(([\w.]+)\)', stmt)
    # ModSoundTypes.grate is a metal grate; iron bars carry the same metal step sound.
    sound = 'minecraft:iron_bars' if step and step[1] == 'ModSoundTypes.grate' else 'minecraft:ladder' if cls == 'BlockNTMLadder' else 'minecraft:stone'
    # BlockLadder uses Material.circuits, which breaks without a tool; the iron-cased blocks need one.
    return {'hardness': hardness, 'explosion_resistance': resistance, 'requires_tool': cls != 'BlockNTMLadder', 'sounds': sound}


def shaped_blocks(source, tr):
    blocks = (source / (BASE + 'blocks/ModBlocks.java')).read_text()
    rows = []
    for var, template, gap in SHAPED_BLOCKS:
        name, texture = registration(blocks, var, 'block')
        stmt = re.search(r'^\s*' + var + r'\s*=\s*new\s.*?;', blocks, re.M | re.S).group(0)
        # Barbed wire's registered texture is its 3D model skin (*_model); the icon is the block name.
        if var.startswith('barbed_wire'): texture = name
        rows.append({'id': name, 'type': 'block', 'category': 'decorative', 'source_variable': var,
                     'template': template,
                     'source_texture': f'src/main/resources/assets/hbm/textures/blocks/{texture or name}.png',
                     'names': {loc: loc_tr[f'tile.{name}.name'] for loc, loc_tr in tr.items()},
                     'block_properties': java_block_properties(stmt),
                     'behavior_gap': gap})
    return rows


def build(source):
    items = (source / (BASE + 'items/ModItems.java')).read_text()
    tr = {loc: lang(source / f'src/main/resources/assets/hbm/lang/{old}.lang')
          for loc, old in [('en_us','en_US'),('pt_br','pt_BR')]}
    variables = re.findall(r'^\s*(waste_\w+)\s*=\s*new ItemDepletedFuel\(\)', clean(items), re.M)
    if len(variables) != 16: raise ValueError('Depleted fuel declarations drift: %d' % len(variables))
    rows = shaped_blocks(source, tr)
    for var in variables:
        name, texture = registration(items, var, 'item')
        stmt = re.search(r'^\s*' + var + r'\s*=\s*new ItemDepletedFuel\(\).*?;', clean(items), re.M | re.S).group(0)
        row = {'id': name, 'type': 'item', 'category': 'component', 'source_variable': var,
               'source_texture': f'src/main/resources/assets/hbm/textures/items/{texture or name}.png',
               'names': {loc: loc_tr[f'item.{name}.name'] for loc, loc_tr in tr.items()},
               'behavior_gap': 'Processing identity only; depletion, heat and radiation of the spent fuel are not reproduced by this item'}
        # Java gives the plates setCreativeTab(null): they exist for recipes only.
        if 'setCreativeTab(null)' in stmt: row['creative_hidden'] = True
        rows.append(row)
    return rows
