#!/usr/bin/env python3
"""NTM's look for the content mod: machines with real slots and textured
screens, OBJ models with animations, multiblocks (docs/visual-fiel.md).

Reads tools/visual-gen/visual.json and ntm-src, writes into content/:

  data/ntm/horizon_machine/<block>.json      slots, values, multiblock (engine patch ntm-visual)
  assets/ntm/horizon_screen/<block>.json     the GUI texture, bars, lights, tooltips, buttons
  assets/ntm/horizon_model/<block>.json      OBJ + texture + bones + animations
  assets/ntm/animations/<block>.animation.json   GeckoLib/Bedrock animation format
  assets/ntm/models/obj/*.obj, assets/ntm/textures/{gui,models}/...   copied from ntm-src
  assets/ntm/blockstates/<block>.json, models/block/visual/<block>.json,
  items/<block>.json   a block HBM draws with its own block renderer (ISBRH), as
                       JSON elements; content-gen leaves these paths to us
  the dummy block of the multiblocks (ntm:dummy_machine)

A Horizon without the patch ignores these folders: the blocks keep their
cubes and the machines their old screens.
Files it wrote last time and no longer writes are removed
(content/.visual-files.json).
"""
import argparse, json, shutil, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
SRC_ASSETS = 'src/main/resources/assets/hbm'


def write_json(path: Path, doc):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(doc, indent=2, ensure_ascii=False) + '\n')


def arm_animation(spec):
    """HBM's AssemblerArm cycle as keyframes: each of its positions, a
    strike of the striker, back; the ring turns between stops."""
    a = spec['arm']
    step, strike, hold = a['move'], a['strike'], a['hold']
    t = 0.0
    rot = {1: {}, 2: {}}
    pos = {1: {}, 2: {}}
    keys = {n: {'lower': {}, 'upper': {}, 'head': {}, 'spike': {}} for n in (1, 2)}
    def put(n, time, angles, spike):
        sign = 1 if n == 1 else -1
        k = keys[n]
        k['lower'][f'{time:.2f}'] = [sign * angles[0], 0, 0]
        k['upper'][f'{time:.2f}'] = [sign * angles[1], 0, 0]
        k['head'][f'{time:.2f}'] = [sign * angles[2], 0, 0]
        k['spike'][f'{time:.2f}'] = [0, spike * 16, 0]
    zero = [0, 0, 0]
    for n in (1, 2):
        put(n, 0.0, zero, 0)
    positions = a['positions']
    for i, p in enumerate(positions):
        # The second arm runs the list the other way round.
        q = positions[-1 - i]
        t += step
        put(1, t, p, 0); put(2, t, q, 0)
        t += hold
        put(1, t, p, 0); put(2, t, q, 0)
        t += strike
        put(1, t, p, a['striker']); put(2, t, q, a['striker'])
        t += strike
        put(1, t, p, 0); put(2, t, q, 0)
    t += step
    for n in (1, 2):
        put(n, t, zero, 0)
    # The loop lasts until the ring is back too.
    length = round(max(t, max(k for k, _ in spec['ring']) + 0.4), 2)
    for n in (1, 2):
        put(n, length, zero, 0)
    ring = {f'{k:.2f}': [0, v, 0] for k, v in spec['ring']}
    ring[f'{length:.2f}'] = [0, 0, 0]
    bones = {spec['ring_bone']: {'rotation': ring}}
    for n in (1, 2):
        b = spec['bones_of_arm'][str(n)]
        bones[b['lower']] = {'rotation': keys[n]['lower']}
        bones[b['upper']] = {'rotation': keys[n]['upper']}
        bones[b['head']] = {'rotation': keys[n]['head']}
        bones[b['spike']] = {'position': keys[n]['spike']}
    return {'format_version': '1.8.0', 'animations': {spec['name']: {'loop': True, 'animation_length': length, 'bones': bones}}}


def machine_slots(machine):
    """A slot's `accept_recipes: <machine>` becomes `accept`: the input items
    of content/machine-recipes/<machine>.json, as HBM's isItemValidForSlot
    takes only what a recipe uses (a battery then goes to its own slot)."""
    slots = []
    for slot in machine.get('slots', []):
        if 'accept_recipes' in slot:
            slot = dict(slot)
            recipes = json.loads((ROOT / 'content/machine-recipes' / f"{slot.pop('accept_recipes')}.json").read_text())
            slot['accept'] = sorted({i for r in recipes['recipes'] for it in r['inputItems'] for i in it['ids']})
        slots.append(slot)
    return {**machine, 'slots': slots} if 'slots' in machine else machine


def obj_info(path):
    """The OBJ's object names and how far it reaches from the block's centre."""
    names, reach = [], 1.0
    for line in path.read_text(errors='replace').splitlines():
        if line.startswith('o '):
            names.append(line[2:].strip())
        elif line.startswith('v '):
            try:
                x, y, z = (float(v) for v in line.split()[1:4])
            except ValueError:
                continue
            reach = max(reach, abs(x) + 0.5, abs(y), abs(z) + 0.5)
    return names, reach


def loop_animation(spec):
    """A loop played while the machine works, as HBM's renderers move parts
    by their `anim` tick counter: `turn` is degrees per loop about the bone's
    pivot (two keyframes, linear), `sway` a slide along an axis by
    BobMathUtil.sps, `cycles` times per loop, sampled 24 times a cycle,
    around `offset` pixels;
    `swing` a turn about an axis by `amp` * sin, `cycles` times per loop;
    `position` keyframes as they are (a press head's stroke). `loop: false`
    holds the last frame (a door posed open by its screen)."""
    import math
    length = spec['length']
    bones = {}
    for bone, b in spec['bones'].items():
        out = {}
        if 'turn' in b:
            out['rotation'] = {'0.0': [0, 0, 0], f'{length:.2f}': b['turn']}
        if 'position' in b:
            out['position'] = b['position']
        if 'sway' in b:
            w = b['sway']
            axis = 'xyz'.index(w['axis'])
            keys = {}
            n = 24 * w.get('cycles', 1)
            for k in range(n + 1):
                t = length * k / n
                v = w['amp'] * math.sin(math.pi / 2 * math.cos(2 * math.pi * w.get('cycles', 1) * t / length))
                pos = [0.0, 0.0, 0.0]
                pos[axis] = round(v + w.get('offset', 0), 4)
                keys[f'{t:.4f}'] = pos
            out['position'] = keys
        if 'swing' in b:
            w = b['swing']
            axis = 'xyz'.index(w['axis'])
            keys = {}
            n = 24 * w.get('cycles', 1)
            for k in range(n + 1):
                t = length * k / n
                rot = [0.0, 0.0, 0.0]
                rot[axis] = round(w['amp'] * math.sin(2 * math.pi * w.get('cycles', 1) * t / length), 4)
                keys[f'{t:.4f}'] = rot
            out['rotation'] = keys
        bones[bone] = out
    return {'format_version': '1.8.0', 'animations': {spec['name']: {'loop': spec.get('loop', True), 'animation_length': length, 'bones': bones}}}


def merge_obj(src, model):
    """One OBJ and one texture from a model's own and its `merge` parts
    (RenderPress draws press_body and press_head, each with its texture):
    the textures side by side on one sheet, each part's UVs moved onto its
    place there, each part one object (bone) named by its `name`, turned
    `turn` degrees about y and lifted `lift` blocks into the model's frame.
    Returns the OBJ text and the sheet."""
    import math
    from PIL import Image
    parts = [{'obj_from': model['obj_from'], 'texture_from': model['texture_from']}] + model['merge']
    images = [Image.open(src / q['texture_from']).convert('RGBA') for q in parts]
    W, H = sum(i.width for i in images), max(i.height for i in images)
    sheet = Image.new('RGBA', (W, H))
    lines, base = ['# merged by tools/visual-gen/generate.py'], [0, 0, 0]
    x0 = 0
    for q, img in zip(parts, images):
        sheet.paste(img, (x0, 0))
        a = math.radians(q.get('turn', 0))
        cos, sin = round(math.cos(a), 6), round(math.sin(a), 6)
        lift = q.get('lift', 0)
        n = [0, 0, 0]
        if 'name' in q:
            # A part is one bone: its own objects become one.
            lines.append(f"o {q['name']}")
        for raw in (src / q['obj_from']).read_text().splitlines():
            t = raw.split()
            if not t:
                continue
            if t[0] == 'o' and 'name' in q:
                continue
            if t[0] in ('v', 'vn'):
                x, y, z = map(float, t[1:4])
                x, z = x * cos + z * sin, -x * sin + z * cos
                if t[0] == 'v':
                    y += lift
                lines.append(f'{t[0]} {x:.6f} {y:.6f} {z:.6f}')
                n[0 if t[0] == 'v' else 2] += 1
            elif t[0] == 'vt':
                u, v = float(t[1]), float(t[2])
                lines.append(f'vt {(x0 + u * img.width) / W:.6f} {1 - ((1 - v) * img.height) / H:.6f}')
                n[1] += 1
            elif t[0] == 'f':
                def shift(c):
                    idx = c.split('/')
                    return '/'.join(str(int(i) + base[k]) if i else '' for k, i in enumerate(idx))
                lines.append('f ' + ' '.join(shift(c) for c in t[1:]))
            elif t[0] == 'o':
                lines.append(raw)
        base = [b + k for b, k in zip(base, n)]
        x0 += img.width
    return '\n'.join(lines) + '\n', sheet


def block_model(spec, block):
    """A block HBM draws with an ISBRH (RenderFoundryMold...): its boxes as
    JSON elements, each face naming one of HBM's block textures. Default UVs
    follow the element's position, as RenderBlocks' faces follow the render
    bounds, so a face shows the part of HBM's texture it showed there. Faces
    on the block's edge are culled against neighbours; no ambient occlusion,
    as HBM's renderers have none."""
    name = block.split(':', 1)[1]
    textures = {k: f'ntm:block/visual/{v}' for k, v in spec['textures'].items()}
    textures.setdefault('particle', textures[spec.get('particle', 'side')])
    edge = {'down': (1, 0), 'up': (1, 16), 'north': (2, 0), 'south': (2, 16), 'west': (0, 0), 'east': (0, 16)}
    elements = []
    for frm, to, faces in spec['elements']:
        out = {}
        for face, tex in faces.items():
            f = {'texture': '#' + tex}
            axis, at = edge[face]
            if (frm if at == 0 else to)[axis] == at:
                f['cullface'] = face
            out[face] = f
        elements.append({'from': frm, 'to': to, 'faces': out})
    model = {'ambientocclusion': False, 'textures': textures, 'elements': elements}
    ref = f'ntm:block/visual/{name}'
    if spec.get('facing'):
        turns = {'north': 0, 'east': 90, 'south': 180, 'west': 270}
        variants = {f'facing={f}': ({'model': ref, 'y': y} if y else {'model': ref}) for f, y in turns.items()}
    else:
        variants = {'': {'model': ref}}
    return model, {'variants': variants}, {'model': {'type': 'minecraft:model', 'model': ref}}


def auto_models(args, src, hand):
    """The port's blocks with an HBM model (extracted.json) that visual.json
    does not describe: their model as HBM's renderer turns it. No slots and
    no multiblock: those blocks keep their logic's screen and their one cell
    (a multiblock needs its ports as proxies, which only a hand entry gives)."""
    extracted = HERE / 'extracted.json'
    if not extracted.exists():
        return {}
    ex = json.loads(extracted.read_text())
    ids = {e['id'] for e in json.loads((args.out / 'ids.json').read_text())['entries']}
    out = {}
    for block, e in sorted(ex.items()):
        m = e.get('model')
        if block in hand or block not in ids or not m or not m.get('files'):
            continue
        obj = src / m['obj']
        names, reach = obj_info(obj)
        name = block.split(':', 1)[1]
        model = {
            # Resource paths are lower case; HBM's files are not always.
            'obj': f"ntm:models/obj/{Path(m['obj']).name.lower()}",
            'obj_from': m['obj'],
            'texture': 'ntm:' + m['texture'].lower(),
            'texture_from': m['texture'],
            'rotation': m.get('rotation') or 0,
            'radius': round(reach + 0.5, 1),
        }
        if m.get('translate'):
            model['translate'] = m['translate']
        if not m.get('render_all') and m.get('parts'):
            model['hide'] = [n for n in names if n not in m['parts']]
        out[block] = {'source': f"extracted: {e.get('renderer')}", 'model': model}
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--spec', type=Path, default=HERE / 'visual.json')
    ap.add_argument('--ntm-src', type=Path, default=ROOT.parent / 'ntm-src')
    ap.add_argument('--out', type=Path, default=ROOT / 'content')
    args = ap.parse_args()
    spec = json.loads(args.spec.read_text())
    src = args.ntm_src / SRC_ASSETS
    out = args.out
    owned = []

    def own(rel):
        owned.append(str(rel))
        return out / rel

    def copy(from_rel, to_rel):
        target = own(to_rel)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(src / from_rel, target)

    def liquid_texture(x):
        target = 'assets/ntm/' + x['texture'].split(':', 1)[1]
        copy(x.pop('texture_from'), target)
        # `crop`: the part of the texture HBM's face shows (RenderBlocks takes
        # its UVs from the face's bounds), or a plain patch to tint as HBM's
        # untextured fluids, as the whole sprite.
        if 'crop' in x:
            from PIL import Image
            cx, cy, cw, ch = x.pop('crop')
            Image.open(out / target).crop((cx, cy, cx + cw, cy + ch)).save(out / target)

    dummy = spec['dummy']
    blocks = dict(spec['blocks'])
    # An entry `like` another (the twelve anvils) takes its machine, on that
    # one's screen, and its model, with its own keys over them.
    for k, b in list(blocks.items()):
        if 'like' in b:
            base = blocks[b['like']]
            e = {key: v for key, v in base.items() if key not in ('screen', 'source')}
            e['machine'] = {**base['machine'], 'screen': base['machine'].get('screen', b['like'])}
            if 'model' in base:
                e['model'] = {**base['model'], **b.get('model', {})}
            e.update({key: v for key, v in b.items() if key not in ('like', 'model')})
            blocks[k] = e
    # A hand entry without a model takes the extracted one.
    auto = auto_models(args, src, {k for k, b in blocks.items() if 'model' in b})
    ex_file = HERE / 'extracted.json'
    extracted = json.loads(ex_file.read_text()) if ex_file.exists() else {}
    for k, a in auto.items():
        blocks[k] = {**a, **blocks.get(k, {})}
    for block, b in blocks.items():
        name = block.split(':', 1)[1]
        if 'machine' in b:
            write_json(own(f'data/ntm/horizon_machine/{name}.json'), machine_slots(b['machine']))
        if 'screen' in b:
            s = dict(b['screen'])
            # HBM's title colour (extracted.json), when the entry names none.
            color = extracted.get(block, {}).get('gui', {}).get('title_color')
            if color and 'title_color' not in s:
                s['title_color'] = color
            target = 'assets/ntm/' + s['texture'].split(':', 1)[1]
            copy(s.pop('texture_from'), target)
            # Other GUI textures a screen draws from (the Tsar's lamps are
            # Ivy Mike's), pasted below or beside it into one file; with `src`,
            # one rectangle of it (a slot frame where the port has a slot HBM's
            # screen has not).
            if 'compose' in s or 'keep' in s:
                from PIL import Image
                base = Image.open(out / target).convert('RGBA')
                def part(c):
                    im = Image.open(src / c['from']).convert('RGBA')
                    if 'src' in c:
                        x, y, w, h = c['src']
                        im = im.crop((x, y, x + w, y + h))
                    return im, c['at']
                parts = [part(c) for c in s.pop('compose', [])]
                w = max([base.width] + [im.width + at[0] for im, at in parts])
                h = max([base.height] + [im.height + at[1] for im, at in parts])
                sheet = Image.new('RGBA', (w, h))
                sheet.paste(base, (0, 0))
                # `keep`: HBM draws the background in pieces (the excavator's
                # top and its inventory); the rest of the screen's area, where
                # the overlays' sprites lie, is cleared (they are read from a
                # copy composed elsewhere).
                if 'keep' in s:
                    sw, sh = s['size']
                    mask = Image.new('L', (w, h), 255)
                    from PIL import ImageDraw
                    draw = ImageDraw.Draw(mask)
                    draw.rectangle((0, 0, sw - 1, sh - 1), fill=0)
                    for kx, ky, kw, kh in s.pop('keep'):
                        draw.rectangle((kx, ky, kx + kw - 1, ky + kh - 1), fill=255)
                    clear = Image.new('RGBA', (w, h))
                    sheet = Image.composite(sheet, clear, mask)
                for im, at in parts:
                    sheet.paste(im, tuple(at))
                sheet.save(out / target)
            write_json(own(f'assets/ntm/horizon_screen/{name}.json'), s)
        if 'liquid' in b:
            # A liquid file: drawn over the block's own model (horizon_liquid).
            l = {'liquids': [dict(x) for x in b['liquid']['liquids']]}
            for x in l['liquids']:
                liquid_texture(x)
            write_json(own(f'assets/ntm/horizon_liquid/{name}.json'), l)
        if 'block_model' in b:
            bm = b['block_model']
            for tex in sorted(set(bm['textures'].values())):
                copy(f'textures/blocks/{tex}.png', f'assets/ntm/textures/block/visual/{tex}.png')
            model, states, item = block_model(bm, block)
            write_json(own(f'assets/ntm/models/block/visual/{name}.json'), model)
            write_json(own(f'assets/ntm/blockstates/{name}.json'), states)
            write_json(own(f'assets/ntm/items/{name}.json'), item)
        if 'model' in b:
            m = dict(b['model'])
            m['liquids'] = [dict(x) for x in m.get('liquids', [])]
            for x in m['liquids']:
                liquid_texture(x)
            if not m['liquids']:
                del m['liquids']
            if 'merge' in m:
                text, sheet = merge_obj(src, m)
                for k in ('obj_from', 'texture_from', 'merge'):
                    m.pop(k)
                obj_out = own('assets/ntm/' + m['obj'].split(':', 1)[1])
                obj_out.parent.mkdir(parents=True, exist_ok=True)
                obj_out.write_text(text)
                tex_out = own('assets/ntm/' + m['texture'].split(':', 1)[1])
                tex_out.parent.mkdir(parents=True, exist_ok=True)
                sheet.save(tex_out)
            else:
                copy(m.pop('obj_from'), 'assets/ntm/' + m['obj'].split(':', 1)[1])
                copy(m.pop('texture_from'), 'assets/ntm/' + m['texture'].split(':', 1)[1])
            loop = m.pop('loop_animation', None)
            if loop:
                rel = f'animations/{name}.animation.json'
                write_json(own('assets/ntm/' + rel), loop_animation(loop))
                m['animations'] = 'ntm:' + rel
            anim = m.pop('arm_animation', None)
            if anim:
                rel = f'animations/{name}.animation.json'
                write_json(own('assets/ntm/' + rel), arm_animation(anim))
                m['animations'] = 'ntm:' + rel
            write_json(own(f'assets/ntm/horizon_model/{name}.json'), m)
            # The block keeps its blockstate: the patched game leaves its cube
            # out of the chunk mesh and draws the OBJ; another game shows the cube.
    # The multiblocks' filler: a barrier that takes its time to mine.
    d = dummy.split(':', 1)[1]
    write_json(own(f'data/ntm/horizon/block/{d}.json'), {
        'template': 'minecraft:barrier',
        'properties': {'hardness': 5.0, 'explosion_resistance': 10.0, 'requires_tool': True, 'sounds': 'minecraft:iron_block'},
        'item': False,
    })
    write_json(own(f'assets/ntm/blockstates/{d}.json'), {'variants': {'': {'model': f'ntm:block/{d}'}}})
    write_json(own(f'assets/ntm/models/block/{d}.json'), {'textures': {'particle': spec['dummy_particle']}})
    for loc, text in spec['dummy_names'].items():
        path = out / f'assets/ntm/lang/{loc}.json'
        if path.exists():
            lang = json.loads(path.read_text())
            lang[f'block.ntm.{d}'] = text
            write_json(path, lang)
    previous = out / '.visual-files.json'
    if previous.exists():
        for rel in json.loads(previous.read_text()):
            if rel not in owned:
                (out / rel).unlink(missing_ok=True)
    write_json(previous, owned)
    print(json.dumps({'blocks': len(spec['blocks']), 'files': len(owned)}))


if __name__ == '__main__':
    sys.exit(main())
