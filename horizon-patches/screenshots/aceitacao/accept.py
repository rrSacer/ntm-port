#!/usr/bin/env python3
"""Acceptance of the patched Horizon (creative-plus + recipes-tick + mod-item-tags)
with the final NTM mods, driven by horizon-ctl. Phase 1 on a fresh creative world;
phase 2 (--reenter) after quitting and starting the game again.
Evidence: /tmp/ntm-accept/evidence.json and /tmp/ntm-accept/shots/*.png."""
import json, subprocess, sys, time
from pathlib import Path

HOME = Path('/tmp/ntm-accept/home')
SHOTS = Path('/tmp/ntm-accept/shots')
EVID = Path('/tmp/ntm-accept/evidence.json')
CTL = ['/home/sacer/Projects/horizon-ntm/horizon-patched/target/release/horizon-ctl', '--socket', str(HOME / 'control.sock')]
ev = json.loads(EVID.read_text()) if EVID.exists() else {}
reenter = '--reenter' in sys.argv


def ctl(*args, check=True):
    out = subprocess.run(CTL + [str(a) for a in args], capture_output=True, text=True).stdout
    try:
        r = json.loads(out)
    except ValueError:
        r = {'raw': out}
    if check and isinstance(r, dict) and 'error' in r:
        raise RuntimeError(f'{args}: {r}')
    return r


def shot(name):
    ctl('screenshot', SHOTS / f'{name}.png')
    time.sleep(1)


def cmd(line):
    r = ctl('command', line)
    time.sleep(0.7)
    return r


def messages(n=6):
    return ctl('messages', n)


def dose():
    best = 0
    for p in (HOME / 'horizon-mods/.horizon-mod-data/ntmlogic').glob('w.*'):
        try:
            data = json.loads(p.read_bytes().split(b'\n', 1)[1])
            best = max([best] + [d['millirad'] for d in data.get('radiation', {}).values()])
        except (ValueError, IndexError, KeyError):
            pass
    return best


def ui_widgets():
    r = ctl('ui')
    return r.get('widgets', r)


def find(widgets, text):
    for w in widgets if isinstance(widgets, list) else []:
        if text in json.dumps(w, ensure_ascii=False):
            return w
    return None


def playing():
    for _ in range(3):
        if ctl('state', check=False).get('screen') in (None, 'playing'):
            return
        ctl('key', 'escape', 'tap', check=False)
        time.sleep(1)


def step(name, fn):
    if name not in ('log', 'creative_tabs_search', 'log_reentered'):
        playing()
    try:
        ev[name] = {'ok': True, 'result': fn()}
    except Exception as e:  # keep going: the report says which step failed
        ev[name] = {'ok': False, 'error': repr(e)}
    EVID.write_text(json.dumps(ev, indent=1, ensure_ascii=False))
    print(name, 'OK' if ev[name]['ok'] else 'FAIL', file=sys.stderr)


def log_checks():
    log = (HOME / 'client.log').read_text(errors='replace')
    tabs = [l for l in log.splitlines() if 'creative-plus' in l]
    stopped = [l for l in log.splitlines() if 'stopped' in l]
    assert tabs and not stopped, (tabs, stopped)
    return {'tabs': tabs[:1], 'stopped': stopped}


def creative():
    ctl('key', 'e', 'tap'); time.sleep(2)
    shot('a01-criativo-pagina1-painel')
    ctl('click-at', 327, 27); time.sleep(1)
    shot('a02-criativo-pagina2-abas-ntm')
    ctl('click-at', 263, 55); time.sleep(1)
    shot('a03-aba-bombas')
    ctl('click-at', 324, 55); time.sleep(1)
    ctl('type', 'uranium'); time.sleep(1.5)
    shot('a04-busca-criativa-uranium')
    return ctl('screen').get('title') or 'ok'


def panel_cheat():
    if ctl('state', check=False).get('screen') != 'creative':
        ctl('key', 'e', 'tap'); time.sleep(2)
    before = ctl('inventory').get('counts', {})
    ctl('click-at', 400, 254); time.sleep(0.5)
    ctl('type', 'zzzz'); time.sleep(1.5)
    shot('a05a-painel-sem-resultado')
    for _ in range(4):
        ctl('type', '\\b'); time.sleep(0.2)
    ctl('type', 'uranium'); time.sleep(1.5)
    shot('a05-painel-busca')
    ctl('click-at', 356, 37); time.sleep(1.5)
    ctl('click-at', 356, 37, 'right'); time.sleep(1.5)
    after = ctl('inventory').get('counts', {})
    shot('a06-cheat')
    assert after != before, (before, after)
    return {'before': before, 'after': after}


def keys_r_u():
    if ctl('state', check=False).get('screen') != 'creative':
        ctl('key', 'e', 'tap'); time.sleep(2)
    ctl('click-at', 50, 200); time.sleep(0.5)
    ctl('drag-at', 50, 200, 356, 37); time.sleep(0.5)
    ctl('key', 'r', 'tap'); time.sleep(2)
    shot('a07-r-no-painel')
    ctl('click-at', 155, 55); time.sleep(1)  # back to the NTM parts tab
    ctl('click-at', 327, 27); time.sleep(1)
    ctl('drag-at', 50, 200, 177, 93); time.sleep(0.5)
    ctl('key', 'u', 'tap'); time.sleep(2)
    shot('a08-u-na-grade')
    ctl('key', 'escape', 'tap'); time.sleep(1)
    return 'ok'


def tags():
    cmd('clear @s')
    cmd('give @s ntm:ingot_uranium 2')
    out = {}
    for line in ['clear @s #ntm:rad/bit_8 0', 'clear @s #ntm:rad/bit_0 0']:
        cmd(line)
        out[line] = messages(3)
    p = ctl('state')['player']['position']
    x, y, z = int(p[0]) + 3, int(p[1]), int(p[2])
    cmd(f'setblock {x} {y} {z} ntm:ore_uranium')
    cmd(f'execute if block {x} {y} {z} #minecraft:needs_iron_tool run say ore_tag_ok')
    out['block'] = messages(3)
    text = json.dumps(out, ensure_ascii=False)
    assert 'Unknown' not in text, text
    return out


def radiation():
    cmd('gamemode survival')
    cmd('clear @s')
    start = dose()
    cmd('give @s ntm:ingot_uranium 2')
    time.sleep(40)
    shot('a09-radiacao-hud')
    d = dose()
    assert d > start, (start, d)
    return {'start_millirad': start, 'after_40s_millirad': d}


def machine():
    cmd('gamemode creative')
    p = ctl('state')['player']['position']
    px, pz = int(p[0]), int(p[2])
    cmd(f'fill {px - 3} 119 {pz - 3} {px + 3} 119 {pz + 3} minecraft:glass')
    cmd(f'fill {px - 3} 120 {pz - 3} {px + 3} 123 {pz + 3} minecraft:air')
    cmd(f'tp @s {px}.5 120 {pz}.5')
    time.sleep(2)
    x, y, z = px + 2, 120, pz
    cmd(f'setblock {x} {y} {z} ntm:machine_wood_burner')
    cmd('give @s minecraft:coal 4')
    ctl('select', 'minecraft:coal')
    ctl('look-at-block', x, y, z); time.sleep(1)
    ctl('key', 'r', 'tap'); time.sleep(2)
    w = ui_widgets()
    b = find(w, 'Pôr combustível')
    assert b, w
    ctl('click', b['id']); time.sleep(1.5)
    w = ui_widgets()
    on = find(w, '"Ligar"')
    if on:
        ctl('click', on['id']); time.sleep(1)
    time.sleep(8)
    w = ui_widgets()
    shot('a10-queimador')
    ev['machine_pos'] = [x, y, z]
    energy = find(w, 'Energia')
    ctl('key', 'escape', 'tap'); time.sleep(1)
    return {'energia': energy, 'inventory': ctl('inventory').get('counts')}


def reenter_checks():
    x, y, z = ev['machine_pos']
    ctl('look-at-block', x, y, z); time.sleep(1)
    ctl('key', 'r', 'tap'); time.sleep(2)
    w = ui_widgets()
    shot('b02-queimador-depois')
    ctl('key', 'escape', 'tap'); time.sleep(1)
    ctl('key', 'e', 'tap'); time.sleep(2)
    shot('b03-criativo-depois')
    ctl('key', 'escape', 'tap')
    return {'energia': find(w, 'Energia'), 'dose_millirad': dose(),
            'inventory': ctl('inventory').get('counts'), 'world': ctl('state').get('world')}


SHOTS.mkdir(exist_ok=True)
subprocess.run(['chown', 'sacer:sacer', str(SHOTS)], check=True)
if not reenter:
    step('log', log_checks)
    step('creative_tabs_search', creative)
    step('panel_cheat', panel_cheat)
    step('keys_r_u', keys_r_u)
    step('mod_tags_in_commands', tags)
    step('radiation', radiation)
    step('machine_wood_burner', machine)
    ev['saved_dose'] = dose()
    ctl('save'); time.sleep(3)
    EVID.write_text(json.dumps(ev, indent=1, ensure_ascii=False))
    ctl('quit', check=False)
else:
    step('log_reentered', log_checks)
    time.sleep(10)
    shot('b01-hud-depois')
    step('reentered', reenter_checks)
    ctl('quit', check=False)
print(json.dumps({k: v.get('ok') if isinstance(v, dict) else v for k, v in ev.items()}, ensure_ascii=False))
