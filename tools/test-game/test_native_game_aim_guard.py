"""The aim guard (INFRA-G): an interact whose crosshair lands on an entity, as a guest's does with the host standing at
its spot, fails before the click with a message that says so; a crosshair on the block lets the interact through."""
import json
import os
from pathlib import Path
import shutil
import socket
import tempfile
import threading
import unittest

from native_game import Game

ON_BLOCK = {'kind': 'block', 'entity': None,
            'block': {'block': 'ntm:dummy_machine', 'pos': [8, 101, 10], 'distance': 2.73}}
ON_HOST = {'kind': 'entity', 'block': ON_BLOCK['block'],
           'entity': {'id': 1, 'type': 'minecraft:player', 'distance': 0.0, 'pos': [8.5, 101.0, 7.5]}}


class FakeControl:
    """control.sock answering one JSON line per command; records the commands."""

    def __init__(self, folder, looking_at):
        self.looking_at = looking_at
        self.lines = []
        self.server = socket.socket(socket.AF_UNIX)
        self.server.bind(str(Path(folder) / 'control.sock'))
        self.server.listen()
        threading.Thread(target=self.serve, daemon=True).start()

    def serve(self):
        while True:
            try:
                conn, _ = self.server.accept()
            except OSError:
                return
            with conn, conn.makefile('rw') as stream:
                line = stream.readline().strip()
                self.lines.append(line)
                reply = self.looking_at if line == 'looking-at' else {'ok': True, 'screen': 'container'}
                stream.write(json.dumps(reply) + '\n')
                stream.flush()

    def close(self):
        self.server.close()


class AimGuard(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.mkdtemp(prefix='aimg')
        self.addCleanup(shutil.rmtree, self.folder, True)
        os.environ.pop('NTM_AIM_GUARD', None)

    def run_interact(self, looking_at, line='interact 8 101 10 north'):
        control = FakeControl(self.folder, looking_at)
        self.addCleanup(control.close)
        game = Game(self.folder, 'aim-guard')
        try:
            return game.ctl(line), control.lines
        except RuntimeError as error:
            return error, control.lines

    def test_the_host_at_the_guests_spot_fails_before_the_click(self):
        error, lines = self.run_interact(ON_HOST)
        self.assertIsInstance(error, RuntimeError)
        self.assertIn('minecraft:player id 1 at 0.00 blocks', str(error))
        self.assertIn('another player standing at this spot', str(error))
        self.assertEqual(lines, ['look-at block 8 101 10 north', 'looking-at'], 'no interact was sent')

    def test_a_crosshair_on_the_block_lets_the_interact_through(self):
        result, lines = self.run_interact(ON_BLOCK)
        self.assertEqual(result['screen'], 'container')
        self.assertEqual(lines, ['look-at block 8 101 10 north', 'looking-at', 'interact 8 101 10 north'])

    def test_another_block_in_front_fails_too(self):
        other = {'kind': 'block', 'entity': None, 'block': {'block': 'minecraft:stone', 'pos': [8, 101, 9]}}
        error, lines = self.run_interact(other)
        self.assertIsInstance(error, RuntimeError)
        self.assertIn('not on block', str(error))
        self.assertNotIn('interact 8 101 10 north', lines)

    def test_flags_are_not_taken_for_a_face(self):
        _, lines = self.run_interact(ON_BLOCK, 'interact 8 101 10 --sneak')
        self.assertEqual(lines[0], 'look-at block 8 101 10')

    def test_the_guard_can_be_turned_off(self):
        os.environ['NTM_AIM_GUARD'] = '0'
        self.addCleanup(os.environ.pop, 'NTM_AIM_GUARD', None)
        result, lines = self.run_interact(ON_HOST)
        self.assertEqual(result['screen'], 'container')
        self.assertEqual(lines, ['interact 8 101 10 north'])


if __name__ == '__main__':
    unittest.main()
