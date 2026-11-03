"""Shared socket driver for native acceptance scripts (no WASM UI or scoreboard).

Use a fresh creative world from build-native.sh and control.sock in GAME.
Commands and failed checks are retained in the scenario's evidence JSON.
"""
import json
import socket
import time
from pathlib import Path


class Game:
    def __init__(self, folder, scenario):
        self.folder = Path(folder)
        self.output = self.folder / f'native-{scenario}.json'
        self.evidence = {'scenario': scenario, 'ok': False, 'steps': [], 'checks': {}}

    def ctl(self, line):
        if line.startswith('wait-ready '):
            deadline = time.monotonic() + float(line.split()[1])
            while time.monotonic() < deadline:
                status = self.ctl('status')
                if (status.get('player') is not None
                        and status.get('pending_chunks') == 0
                        and status.get('server', {}).get('loaded') is not False):
                    return status
                time.sleep(.25)
            raise TimeoutError('player/chunks not ready')
        with socket.socket(socket.AF_UNIX) as sock:
            sock.settimeout(120)
            sock.connect(str(self.folder / 'control.sock'))
            sock.sendall((line + '\n').encode())
            with sock.makefile() as stream:
                result = json.loads(stream.readline())
        self.evidence['steps'].append({'command': line, 'result': result})
        self.save()
        if result.get('error'):
            raise RuntimeError(f'{line}: {result["error"]}')
        return result

    def save(self):
        self.output.write_text(json.dumps(self.evidence, indent=2, ensure_ascii=False) + '\n')

    def check(self, name, passed, actual):
        name = self.evidence.get('phase', 'setup') + ':' + name
        self.evidence['checks'][name] = {'ok': bool(passed), 'actual': actual}
        self.save()
        if not passed:
            raise AssertionError(f'{name}: {actual}')

    def command(self, text):
        result = self.ctl('command ' + text)
        time.sleep(.3)  # Commands travel through the integrated server.
        return result

    def ground(self, x, y, z, width=12, height=6):
        self.command(f'forceload add {x-2} {z-2} {x+width} {z+4}')
        time.sleep(2)
        self.command(f'fill {x-2} {y} {z-2} {x+width} {y+height} {z+4} minecraft:air')
        self.command(f'fill {x-2} {y-1} {z-2} {x+width} {y-1} {z+4} minecraft:stone')
        self.command(f'tp @s {x+.5} {y} {z+3.5}')
        time.sleep(2)

    def battery_output(self, x, y, z, dimension=None):
        """What a player does to make a battery give power: it starts input-only (`redLow = 0`), and a
        redstone signal puts it in `redHigh`, output. A redstone block under it is that signal."""
        where = f'execute in {dimension} run ' if dimension else ''
        self.command(f'{where}setblock {x} {y - 1} {z} minecraft:redstone_block')

    def slots(self):
        return [s for s in self.ctl('screen').get('slots', []) if s['slot'] >= 300]

    def close(self):
        self.ctl('close')

    def open(self, x, y, z, dummy_height=0):
        self.close()
        self.command(f'tp @s {x+.5} {y} {z+3.5}')
        self.ctl(f'interact {x} {y+dummy_height} {z} south')
        time.sleep(.7)
        screen = self.ctl('screen')
        self.check('menu_open', screen.get('screen') == 'container', screen)
        return screen

    def put(self, item, count, slot, damage=None):
        component = '' if damage is None else f'[minecraft:damage={damage}]'
        self.command(f'give @s {item}{component} {count}')
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            screen = self.ctl('screen')
            source = next((s['slot'] for s in screen.get('slots', [])
                           if s['slot'] < 36 and s.get('item') == item), None)
            if source is not None:
                break
            time.sleep(.2)
        else:
            raise AssertionError(f'item not received: {item}')
        self.ctl(f'click-slot {source}')
        self.ctl(f'click-slot {300+slot}')
        time.sleep(.4)
        screen = self.ctl('screen')
        self.check(f'insert_{item}_{slot}', not screen.get('carried'), screen)

    def poll(self, predicate, timeout=180):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            slots = self.slots()
            if predicate(slots):
                return slots
            time.sleep(1)
        return slots

    def button(self, rect, size):
        """horizon_screen rect -> native click; the interpreter has no widget id."""
        import math
        width, height = self.ctl('ui')['gui_size']
        ox = math.trunc((width - size[0]) / 2)
        oy = math.trunc((height - size[1]) / 2)
        x, y, w, h = rect
        return self.ctl(f'click-at {ox+x+w/2} {oy+y+h/2}')

    def snapshot(self, name):
        self.ctl('screenshot ' + str(self.folder / f'native-{name}.png'))
        self.ctl('save')
