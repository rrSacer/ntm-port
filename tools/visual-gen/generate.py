#!/usr/bin/env python3
"""NTM's look for the content mod: machines with real slots and textured
screens, OBJ models with animations, multiblocks (docs/visual-fiel.md).

Reads tools/visual-gen/visual.json and ntm-src, writes into content/:

  data/ntm/horizon_machine/<block>.json      slots, values, multiblock (engine patch ntm-visual)
  assets/ntm/horizon_screen/<block>.json     the GUI texture, bars, lights, tooltips, buttons
  assets/ntm/horizon_model/<block>.json      OBJ + texture + bones + animations
  assets/ntm/animations/<block>.animation.json   GeckoLib/Bedrock animation format
  assets/ntm/models/obj/*.obj, assets/ntm/textures/{gui,models}/...   copied from ntm-src
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

    dummy = spec['dummy']
    blocks = dict(spec['blocks'])
    auto = auto_models(args, src, set(blocks))
    blocks.update(auto)
    for block, b in blocks.items():
        name = block.split(':', 1)[1]
        if 'machine' in b:
            write_json(own(f'data/ntm/horizon_machine/{name}.json'), b['machine'])
        if 'screen' in b:
            s = dict(b['screen'])
            copy(s.pop('texture_from'), 'assets/ntm/' + s['texture'].split(':', 1)[1])
            write_json(own(f'assets/ntm/horizon_screen/{name}.json'), s)
        if 'model' in b:
            m = dict(b['model'])
            copy(m.pop('obj_from'), 'assets/ntm/' + m['obj'].split(':', 1)[1])
            copy(m.pop('texture_from'), 'assets/ntm/' + m['texture'].split(':', 1)[1])
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
