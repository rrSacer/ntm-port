"""Check typed vanilla references against the exact exported game registry.

A content block's `sounds` is a BLOCK identity from which to copy its sounds,
not a sound-event identity. Asset file existence cannot validate that contract.
"""
import json
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_RESOURCES = ROOT.parents[1] / 'Games/horizon/.local/horizon-client/26.3'
# Legacy sound-source blocks used by the import catalog. Other identities stay
# unchanged and must pass the registry check; never guess a replacement.
LEGACY_SOUND_BLOCKS = {'minecraft:grass': 'minecraft:short_grass'}


def sound_block(identity):
    return LEGACY_SOUND_BLOCKS.get(identity, identity)


class VanillaReferences:
    def __init__(self, resources=None, registries=None):
        self.resources = Path(resources or os.environ.get('HORIZON_RESOURCES', DEFAULT_RESOURCES))
        data = registries if registries is not None else json.loads((self.resources / 'registries.json').read_text())
        self.ids = {kind: set(data['minecraft:' + kind]['entries']) for kind in ('block', 'item')}
        self.checked = 0

    def require(self, kind, identity, location):
        if isinstance(identity, str) and identity.startswith('minecraft:'):
            if identity not in self.ids[kind]:
                raise ValueError(f'{location}: unknown 26.3 vanilla {kind} {identity}')
            self.checked += 1

    def walk(self, node, location, context=''):
        if isinstance(node, list):
            for i, value in enumerate(node):
                self.walk(value, f'{location}[{i}]', context)
        elif isinstance(node, dict):
            # BlockState / block predicates / item loot entries.
            for key in ('Name', 'block'):
                if key in node: self.require('block', node[key], location + '.' + key)
            if node.get('type') == 'minecraft:item':
                self.require('item', node.get('name'), location + '.name')
            for key, value in node.items():
                if key in ('inputItems', 'outputItems', 'periodicOutputItems', 'catalysts', 'icon', 'item', 'items', 'ingredient', 'ingredients', 'key', 'result'):
                    self.walk(value, location + '.' + key, 'item')
                elif key in ('blocks', 'matching_blocks', 'sounds'):
                    self.walk(value, location + '.' + key, 'block')
                elif key not in ('type', 'Name', 'block', 'name', 'source', 'source_recipe_machine'):
                    self.walk(value, location + '.' + key, context)
        elif context in ('block', 'item'):
            self.require(context, node, location)

    def validate_content(self, content):
        content = Path(content)
        for kind in ('block', 'item'):
            for path in (content / 'data/ntm/horizon' / kind).glob('*.json'):
                data = json.loads(path.read_text())
                self.require(kind, data.get('template'), str(path) + '.template')
                self.require('block', data.get('properties', {}).get('sounds'), str(path) + '.properties.sounds')
        for folder in ('data', 'machine-recipes', 'assets'):
            for path in sorted((content / folder).rglob('*.json')):
                data = json.loads(path.read_text())
                if '/tags/item/' in path.as_posix():
                    for identity in data['values']:
                        self.require('item', identity.get('id') if isinstance(identity, dict) else identity, str(path))
                elif '/tags/block/' in path.as_posix():
                    for identity in data['values']:
                        self.require('block', identity.get('id') if isinstance(identity, dict) else identity, str(path))
                self.walk(data, str(path))
        return self.checked
