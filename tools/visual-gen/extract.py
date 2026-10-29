#!/usr/bin/env python3
"""What HBM's Java says about each block's look, for visual.json
(docs/visual-fiel.md, stage c).

For every block of ModBlocks it follows: the block class (getDimensions,
getOffset, createNewTileEntity), the tile entity's renderer (ClientProxy's
bindTileEntitySpecialRenderer), the renderer's model and texture
(ResourceManager), its turn for metadata 2 (north) and the translation
after it, the parts it draws, and the GUI (provideGUI: texture, size) and
container (slot positions, the player inventory's row).

Writes tools/visual-gen/extracted.json; generate.py uses it for the blocks
visual.json does not describe by hand. Best effort: each entry says what
it could not read ("unread").
usage: extract.py [--ntm-src DIR] [--out FILE]
"""
import argparse, json, re, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
JAVA = 'src/main/java/com/hbm'
ASSETS = 'src/main/resources/assets/hbm'


def read(p):
    try:
        return p.read_text(errors='replace')
    except OSError:
        return ''


def classes(java):
    """Every class name to its file."""
    out = {}
    for p in java.rglob('*.java'):
        out.setdefault(p.stem, p)
    return out


def resources(java):
    """ResourceManager's models and textures: field -> asset path."""
    src = read(java / 'main/ResourceManager.java')
    models, textures = {}, {}
    for m in re.finditer(r'(\w+)\s*=\s*new\s+(?:HFRWavefrontObject|WavefrontObjDisplayList)\(\s*(?:new\s+ResourceLocation\(\s*RefStrings\.MODID\s*,\s*)?"([^"]+\.obj)"', src):
        models[m[1]] = m[2]
    for m in re.finditer(r'(\w+)\s*=\s*AdvancedModelLoader\.loadModel\(\s*new\s+ResourceLocation\(\s*RefStrings\.MODID\s*,\s*"([^"]+\.obj)"', src):
        models[m[1]] = m[2]
    for m in re.finditer(r'(\w+)\s*=\s*new\s+ResourceLocation\(\s*RefStrings\.MODID\s*,\s*"([^"]+\.png)"\s*\)', src):
        textures[m[1]] = m[2]
    return models, textures


def method_body(src, name):
    """The body of the first method `name`, braces balanced."""
    m = re.search(r'\b' + name + r'\s*\([^)]*\)\s*(?:throws[^{]*)?\{', src)
    if not m:
        return None
    i, depth = m.end(), 1
    while i < len(src) and depth:
        depth += {'{': 1, '}': -1}.get(src[i], 0)
        i += 1
    return src[m.end():i - 1]


def num(s):
    s = s.strip().rstrip('DdFf')
    try:
        return float(s)
    except ValueError:
        return None


def rotation(body):
    """The turn about y for metadata 2 (north) plus any turn before the
    switch, and the translation after it, as RenderAssemblyMachine and
    RenderNukeBoy write them."""
    unread = []
    before, after_switch = body, ''
    sw = re.search(r'switch\s*\((.*?getBlockMetadata.*?)\)\s*\{', body)
    if not sw:
        return None, None, ['no metadata switch']
    before = body[:sw.start()]
    pre = 0.0
    for m in re.finditer(r'glRotate[fd]\(\s*([-\d.]+)[FfDd]?\s*,\s*0[FfDd.]*\s*,\s*1[FfDd.]*\s*,\s*0[FfDd.]*\s*\)', before):
        pre += num(m[1]) or 0
    case = re.search(r'case\s+2\s*:(.*?)(?:break\s*;|case\s+\d)', body[sw.end():], re.S)
    if not case:
        return None, None, ['no case 2']
    c = case[1]
    rot = 0.0
    r = re.search(r'glRotate[fd]\(\s*([-\d.]+)[FfDd]?\s*,\s*0[FfDd.]*\s*,\s*1[FfDd.]*\s*,\s*0[FfDd.]*\s*\)', c)
    if r:
        rot = num(r[1]) or 0
    t = re.search(r'glTranslate[fd]\(\s*([-\d.]+)[DdFf]?\s*,\s*([-\d.]+)[DdFf]?\s*,\s*([-\d.]+)[DdFf]?\s*\)', c)
    translate = [num(t[1]), num(t[2]), num(t[3])] if t else None
    return (pre + rot) % 360, translate, unread


def renderer_model(src, models, textures):
    """The model and texture a renderer binds, and its parts."""
    tex = re.findall(r'bindTexture\(\s*ResourceManager\.(\w+)\s*\)', src)
    mod = re.findall(r'ResourceManager\.(\w+)\.render(All|Part|AllExcept)\s*\(\s*(?:"([^"]+)")?', src)
    model = next((m[0] for m in mod if m[0] in models), None)
    texture = next((t for t in tex if t in textures), None)
    parts = sorted({m[2] for m in mod if m[0] == model and m[1] == 'Part' and m[2]})
    render_all = any(m[0] == model and m[1] == 'All' for m in mod)
    return model, texture, parts, render_all


def gui_and_container(te_src, cls):
    """provideGUI's GUI: its texture and size; its container's slots."""
    g = re.search(r'provideGUI[^{]*\{[^}]*?new\s+(GUI\w+)\(', te_src, re.S)
    if not g or g[1] not in cls:
        return None
    gsrc = read(cls[g[1]])
    out = {'class': g[1]}
    t = re.search(r'ResourceLocation\(\s*RefStrings\.MODID\s*\+\s*":([^"]+\.png)"\s*\)', gsrc)
    if t:
        out['texture'] = t[1]
    for k in ('xSize', 'ySize'):
        m = re.search(r'this\.' + k + r'\s*=\s*(\d+)', gsrc)
        if m:
            out[k] = int(m[1])
    # The title's colour, when a literal other than vanilla's grey (0x404040).
    tc = re.search(r'drawString\(\s*name\s*,[^;]*,\s*(0x[0-9a-fA-F]+|\d+)\s*\)\s*;', gsrc)
    if tc and int(tc[1], 0) != 0x404040:
        out['title_color'] = '#%06x' % int(tc[1], 0)
    c = re.search(r'super\(\s*new\s+(Container\w+)\(', gsrc)
    if c and c[1] in cls:
        csrc = read(cls[c[1]])
        slots = []
        for m in re.finditer(r'new\s+(\w*Slot\w*)\(\s*(?:[\w.]+\s*,\s*)?\w+\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)', csrc):
            slots.append({'kind': m[1], 'index': int(m[2]), 'x': int(m[3]), 'y': int(m[4])})
        for m in re.finditer(r'addSlots\(\s*\w+\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)', csrc):
            frm, x, y, rows, cols = map(int, m.groups())
            for r in range(rows):
                for co in range(cols):
                    slots.append({'kind': 'Slot', 'index': frm + co + r * cols, 'x': x + co * 18, 'y': y + r * 18})
        # Slots over the player's inventory use expressions (8 + j * 18), so
        # only the machine's own, written as numbers, are read.
        inv = re.search(r'playerInv\(\s*\w+\s*,\s*(?:(\d+)\s*,\s*)?(\d+)', csrc)
        out['container'] = {
            'class': c[1],
            'slots': sorted({(s['index'], s['x'], s['y'], s['kind']) for s in slots}),
            'inventory': [int(inv[1]) if inv[1] else 8, int(inv[2])] if inv else None,
        }
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--ntm-src', type=Path, default=ROOT.parent / 'ntm-src')
    ap.add_argument('--out', type=Path, default=HERE / 'extracted.json')
    args = ap.parse_args()
    java = args.ntm_src / JAVA
    assets = args.ntm_src / ASSETS
    cls = classes(java)
    models, textures = resources(java)
    proxy = read(java / 'main/ClientProxy.java')
    tesr = dict(re.findall(r'bindTileEntitySpecialRenderer\(\s*(\w+)\.class\s*,\s*new\s+(\w+)\(', proxy))
    blocks_src = read(java / 'blocks/ModBlocks.java')
    out = {}
    for m in re.finditer(r'(\w+)\s*=\s*new\s+(\w+)\([^;]*?\.setBlockName\("([^"]+)"\)', blocks_src):
        field, klass, name = m[1], m[2], m[3]
        if klass not in cls:
            continue
        bsrc = read(cls[klass])
        e = {'class': klass}
        unread = []
        d = re.search(r'getDimensions\(\)\s*\{\s*return\s+new\s+int\[\]\s*\{([^}]*)\}', bsrc)
        if d:
            e['dimensions'] = [int(x) for x in d[1].replace(' ', '').split(',') if x.lstrip('-').isdigit()]
            o = re.search(r'getOffset\(\)\s*\{\s*return\s+(-?\d+)', bsrc)
            e['offset'] = int(o[1]) if o else 0
        tes = re.findall(r'return\s+new\s+(TileEntity\w+)\(\)', bsrc)
        tes = [t for t in tes if not t.startswith('TileEntityProxy')]
        if not tes:
            if len(e) > 1:
                out[f'ntm:{name}'] = e
            continue
        te = tes[0]
        e['tile_entity'] = te
        r = tesr.get(te)
        if r and r in cls:
            rsrc = read(cls[r])
            e['renderer'] = r
            model, tex, parts, render_all = renderer_model(rsrc, models, textures)
            body = method_body(rsrc, 'renderTileEntityAt') or rsrc
            rot, translate, why = rotation(body)
            unread += why
            if model and tex:
                obj, png = models[model], textures[tex]
                e['model'] = {
                    'obj': obj,
                    'texture': png,
                    'rotation': rot,
                    'translate': translate,
                    'parts': parts,
                    'render_all': render_all,
                    'files': (assets / obj).is_file() and (assets / png).is_file(),
                }
            else:
                unread.append('model or texture not in ResourceManager')
        if te in cls:
            g = gui_and_container(read(cls[te]), cls)
            if g:
                e['gui'] = g
        if unread:
            e['unread'] = unread
        out[f'ntm:{name}'] = e
    args.out.write_text(json.dumps(out, indent=1, sort_keys=True) + '\n')
    n = lambda k: sum(1 for e in out.values() if k in e)
    print(json.dumps({'blocks': len(out), 'with_model': n('model'), 'multiblocks': n('dimensions'), 'with_gui': n('gui')}))


if __name__ == '__main__':
    sys.exit(main())
