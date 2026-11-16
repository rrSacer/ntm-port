"""The restart observation and the final save share one frozen simulation."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import native_bombs as bombs


class RestartOrder(unittest.TestCase):
    def test_observes_before_saving_and_keeps_ticks_frozen_until_quit(self):
        class Game:
            def __init__(self, folder):
                self.folder = folder
                self.evidence = {}
                self.frozen = False
                self.calls = []

            def command(self, command):
                self.calls.append(command)
                if command == 'tick freeze':
                    self.frozen = True
                if command == 'ntmbomba status':
                    return {'chat': ['nuke-id Planning R=80 balefire-id Writing R=80']}
                return {'chat': ['detonada']}

            def ground(self, *args, **kwargs):
                pass

            def check(self, name, ok, actual):
                assert ok, (name, actual)

            def ctl(self, command):
                assert command == 'save'
                assert self.frozen
                assert (self.folder / 'bombs-restart.json').exists()
                self.calls.append(command)

        def dug(game, x):
            self.assertTrue(game.frozen)
            game.calls.append(f'dug {x}')
            return 5

        with tempfile.TemporaryDirectory() as folder:
            game = Game(Path(folder))
            with patch.object(bombs.time, 'sleep'), patch.object(bombs, 'log', return_value=''), \
                    patch.object(bombs, 'explosion_id', side_effect=lambda text, x: 'nuke-id' if x == 400 else 'balefire-id'), \
                    patch.object(bombs, 'wait', side_effect=lambda g, name, predicate, actual, timeout: self.assertTrue(predicate())), \
                    patch.object(bombs, 'dug', side_effect=dug):
                bombs.restart_arm(game)
            self.assertEqual(game.calls[-1], 'save', 'no socket round trip after saving')
            self.assertTrue(game.frozen, 'quit must save the same frozen crater')
            frozen_at = game.calls.index('tick freeze')
            self.assertLess(frozen_at, game.calls.index('dug 400'))
            self.assertLess(frozen_at, game.calls.index('dug 720'))
            armed = json.loads((game.folder / 'bombs-restart.json').read_text())
            self.assertEqual(armed['dug'], {'nuke': 5, 'balefire': 5})


if __name__ == '__main__':
    unittest.main()
