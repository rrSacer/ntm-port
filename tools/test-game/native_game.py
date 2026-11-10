"""Shared socket driver for native acceptance scripts (no WASM UI or scoreboard).

Use a fresh creative world from build-native.sh and control.sock in GAME.
Commands and failed checks are retained in the scenario's evidence JSON.
"""
import atexit
from contextlib import contextmanager
import json
import os
import socket
import time
from pathlib import Path


class Game:
    def __init__(self, folder, scenario):
        self.folder = Path(folder)
        self.output = self.folder / f'native-{scenario}.json'
        self._deferred_saves=0
        self._pending_input_delay=0
        self._last_save=0.0
        self._unsaved=False
        self.evidence = {'scenario': scenario, 'ok': False, 'steps': [], 'checks': {}}

    def ctl(self, line, _retry=0):
        if line.startswith('wait-ready '):
            # Pinned to one thread among up to ten games, a world loads slower: never less than 600 s.
            deadline = time.monotonic() + max(float(line.split()[1]), float(os.environ.get('NTM_WAIT_READY_MIN', '600')))
            while time.monotonic() < deadline:
                status = self.ctl('status')
                if (status.get('player') is not None
                        and status.get('pending_chunks') == 0
                        and status.get('server', {}).get('loaded') is not False):
                    return status
                time.sleep(.25)
            raise TimeoutError('player/chunks not ready')
        started=time.monotonic()
        with socket.socket(socket.AF_UNIX) as sock:
            sock.settimeout(360)
            sock.connect(str(self.folder / 'control.sock'))
            sock.sendall((line + '\n').encode())
            with sock.makefile() as stream:
                result = json.loads(stream.readline())
        if 'did not answer within' in str(result.get('error')) and _retry < 12:
            # A heavy server tick (a meltdown's blast) can outlast the controller's wait: ask again.
            time.sleep(5)
            return self.ctl(line, _retry + 1)
        step={'command': line, 'result': result}
        if os.environ.get('NTM_CTL_TIMING'):
            step['started']=started;step['reply_seconds']=time.monotonic()-started
        self.evidence['steps'].append(step)
        saved=time.monotonic();self.save()
        if os.environ.get('NTM_CTL_TIMING'):step['save_seconds']=time.monotonic()-saved
        if result.get('error'):
            raise RuntimeError(f'{line}: {result["error"]}')
        return result

    @contextmanager
    def defer_evidence(self):
        """Keep timed input independent of filesystem latency; flush after releasing it, including on failure."""
        self._deferred_saves+=1
        try:
            yield
        finally:
            self._deferred_saves-=1
            self.save()

    # Seconds between two writes of the evidence by `ctl` (None: it is written after every call). A write takes
    # seconds when the disk is busy, and that is wall time in which the game runs on without the script: a script
    # that counts game ticks (native_items.py) asks for a pause between writes; a check and the end of the run
    # always write.
    save_every = None

    def save(self, force=False):
        delay=float(os.environ.get('NTM_CTL_SAVE_DELAY','0'))
        if delay and self.evidence['steps'] and self.evidence['steps'][-1]['command']=='mouse right down':
            self._pending_input_delay=delay
        if self._deferred_saves:return
        if not force and self.save_every and time.monotonic()-self._last_save<self.save_every:
            if not self._unsaved:
                self._unsaved=True
                atexit.register(self._flush)  # however the run ends, what the pause held back is written
            return
        if self._pending_input_delay:
            self.evidence['injected_input_evidence_delay_seconds']=self._pending_input_delay
            time.sleep(self._pending_input_delay)
            self._pending_input_delay=0
        self.output.write_text(json.dumps(self.evidence, indent=2, ensure_ascii=False) + '\n')
        self._last_save=time.monotonic()
        self._unsaved=False

    def _flush(self):
        if self._unsaved:
            try:self.save(True)
            except OSError:pass

    def check(self, name, passed, actual):
        name = self.evidence.get('phase', 'setup') + ':' + name
        self.evidence['checks'][name] = {'ok': bool(passed), 'actual': actual}
        self.save(True)
        if not passed:
            raise AssertionError(f'{name}: {actual}')

    def note(self, name, text):
        """Something the run could not prove, kept in the evidence instead of a passing check."""
        self.evidence.setdefault('notes', {})[name] = text
        self.save()

    # When set, `command` settles for this many game ticks instead of .3 s of wall time: a script that
    # counts ticks (native_items.py) then does not depend on how loaded the machine is.
    command_ticks = None

    def ticks(self, count):
        """Wait `count` game ticks of the server (20 a second when it keeps up, whatever the wall time); keys
        and mouse buttons held stay held. The `ticks` of the reply is the client's own counter, which runs apart
        from the server's: read the server's with `time query gametime`."""
        return self.ctl(f'wait {int(count)}')

    def command(self, text):
        result = self.ctl('command ' + text)
        if self.command_ticks is None:
            time.sleep(.3)  # Commands travel through the integrated server.
        else:
            self.ticks(self.command_ticks)
        return result

    def ground(self, x, y, z, width=12, height=6, depth=4):
        self.command(f'forceload add {x-2} {z-2} {x+width} {z+depth}')
        time.sleep(2)
        self.command(f'fill {x-2} {y} {z-2} {x+width} {y+height} {z+depth} minecraft:air')
        self.command(f'fill {x-2} {y-1} {z-2} {x+width} {y-1} {z+depth} minecraft:stone')
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
