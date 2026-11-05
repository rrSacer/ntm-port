#!/usr/bin/env python3
"""Import HBM audio and generate the Java call inventory (never scans git branches).

All names have explicit namespaces; case changes satisfy modern Identifier rules.
The legacy category moves to a side table, since modern sounds.json has none.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
CALL = re.compile(r'\.(playSound|playSoundEffect|playSoundAtEntity|playSoundClient|getLoopedSound)\s*\(')
TOKEN = re.compile(r'"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'|//[^\n]*|/\*.*?\*/', re.S)


# These originals carry a conflicting Sound Dogs notice. Exclude the WHOLE
# event, including other variants, rather than quietly changing its pool.
EXCLUDED_FILES = {'tool/boltgun.ogg', 'block/leverStop.ogg', 'block/reactorStart.ogg', 'block/door_close_2.ogg'}


# Only this encoder's own notice was audited in the imported files. Remove
# the exact notice, not its field: an appended audio copyright still fails.
ENCODER_NOTICE = re.compile(r'File created by GoldWave\.\s+GoldWave copyright \(C\) Chris Craig', re.I)


def classify_tags(tags):
    values = [(str(k).lower(), str(v)) for group in tags for k, v in group.items()]
    original = ' '.join(k + ': ' + v for k, v in values)
    if 'sound dogs' in original.lower():
        raise ValueError('Incompatible audio copyright notice: ' + original)
    cleaned = [(k, ENCODER_NOTICE.sub('', v) if k in ('software', 'encoder', 'encoded_by') else v)
               for k, v in values]
    text = ' '.join(v for _, v in cleaned).lower()
    # A dedication elsewhere cannot cancel a contradictory rights notice.
    if re.search(r'all\s+rights(?:\s+are)?\s+reserved|proprietary|\bcc[- ]by\b|no permission|permission[^.;,]{0,20}not granted|\bnot\b[^.;,]{0,40}\b(?:cc0|lgpl|public domain)\b|(?:not|non|no)[\s-]*(?:licensed[\s-]+(?:under[\s-]+)?)?(?:cc0|lgpl)|(?:cc0|lgpl)[^.;,]{0,20}\b(?:denied|not granted)\b', text):
        raise ValueError('Conflicting audio copyright notice: ' + original)
    licenses = set()
    for key, value in cleaned:
        value = value.strip()
        declared = set()
        # Recognize affirmative audio declarations, never e.g. "Edited with
        # LGPL tools" or a license name in an encoder's technical description.
        if key in ('software', 'encoder', 'encoded_by'):
            continue
        if re.fullmatch(r'(?:Licensed under\s+)?CC0(?:-1\.0|\s+1\.0)?[.!]?', value, re.I) or re.match(r'^CC0,\s*mix of CC0 sounds\b', value, re.I) or re.fullmatch(r'https?://creativecommons\.org/publicdomain/zero/1\.0/?', value, re.I):
            declared.add('CC0-1.0')
        if re.fullmatch(r'(?:Licensed under\s+)?(?:LGPL(?:-3\.0-only|v?3(?:\.0)?)|GNU Lesser General Public License(?: version)? 3(?:\.0)?)[.!]?', value, re.I):
            declared.add('LGPL-3.0-only')
        if key in ('license', 'licence') and not declared:
            raise ValueError('Unrecognized audio copyright notice: ' + original)
        licenses.update(declared)
    if len(licenses) > 1:
        raise ValueError('Conflicting audio copyright notice: ' + original)
    if not licenses and re.search(r'copyright|©|\bcc0\b|\blgpl\b|\blicen[cs]e[ds]?\b', text):
        raise ValueError('Incompatible or unspecified audio copyright notice: ' + original)
    return next(iter(licenses), 'LGPL-3.0-only')


def audio_tags(path):
    probe = subprocess.run(['ffprobe', '-v', 'error', '-show_entries', 'format_tags:stream_tags',
                            '-of', 'json', str(path)], check=True, capture_output=True, text=True)
    parsed = json.loads(probe.stdout)
    tags = [stream.get('tags', {}) for stream in parsed.get('streams', [])]
    tags.append(parsed.get('format', {}).get('tags', {}))
    return [group for group in tags if group]


def clean(text):
    return TOKEN.sub(lambda m: '\n' * m[0].count('\n') if m[0].startswith(('/', '/*')) else m[0], text)


def split_args(text):
    out, start, depth, quoted, escaped = [], 0, 0, None, False
    for i, c in enumerate(text):
        if quoted:
            if escaped:
                escaped = False
            elif c == '\\':
                escaped = True
            elif c == quoted:
                quoted = None
        elif c in '\"\'':
            quoted = c
        elif c in '([{':
            depth += 1
        elif c in ')]}':
            if depth == 0:
                return out + [text[start:i].strip()]
            depth -= 1
        elif c == ',' and depth == 0:
            out.append(text[start:i].strip())
            start = i + 1
    raise ValueError('Unbalanced Java call')


def modern(name):
    ns, sep, path = name.partition(':')
    return ('ntm:' + (path if sep else ns).lower()) if not sep or ns == 'hbm' else name.lower()


def convert(original):
    """Lossless current-format definitions plus the displaced 1.7.10 categories."""
    events, categories = {}, {}
    for name, definition in original.items():
        event = name.lower()
        if event in events:
            raise ValueError('Case collision: ' + name)
        result = {k: v for k, v in definition.items() if k not in ('category', 'sounds')}
        sounds = []
        for sound in definition.get('sounds', []):
            if isinstance(sound, str):
                sounds.append(modern(sound))
            else:
                sounds.append({**sound, 'name': modern(sound['name'])})
        result['sounds'] = sounds
        events[event] = result
        categories['ntm:' + event] = definition.get('category', 'master')
    return events, categories


def inventory(source):
    java = source / 'src/main/java'
    constants = dict(re.findall(r'public static final String\s+(\w+)\s*=\s*"([^"\n]+)"',
                               (java / 'com/hbm/main/NTMSounds.java').read_text()))
    mapping = json.loads((HERE / 'systems.json').read_text())
    available = {path.stem for path in java.rglob('*.java')}
    # Runtime modules already name their Java source classes in their prose.
    # Include those automatically, so a new port cannot be missed by a hand list.
    for path in sorted((ROOT / 'crates/ntm-core/src').rglob('*.rs')):
        if 'tests' in path.parts or path.stem.endswith('_tests'):
            continue
        text = re.sub(r'\b(TileEntity\w*)\{([^}]+)\}', lambda m: ' '.join(m[1] + v.strip().replace('//!', '').strip() for v in m[2].split(',')), path.read_text())
        for name in re.findall(r'\bTileEntity\w+\b', text):
            if name in available:
                ports = mapping.setdefault(name, [])
                port = str(path.relative_to(ROOT))
                if port not in ports:
                    ports.append(port)
    scanned = [{'class': name, 'source_exists': name in available, 'port': paths}
               for name, paths in sorted(mapping.items())]
    write_json(ROOT / 'docs/sounds-systems.json', scanned)
    rows = []
    for path in sorted(java.rglob('*.java')):
        text = clean(path.read_text())
        for match in CALL.finditer(text):
            args = split_args(text[match.end():])
            method = match[1]
            index = 0 if method == 'getLoopedSound' else 1 if method == 'playSoundAtEntity' else 3 if method in ('playSoundEffect', 'playSoundClient') else 0 if len(args) <= 3 else 3
            if index >= len(args):
                raise ValueError(f'Cannot parse {path}:{match.start()}: {args}')
            expr = args[index]
            value = re.sub(r'\bNTMSounds\.(\w+)', lambda m: json.dumps(constants.get(m[1], m[0])), expr)
            literal = re.fullmatch(r'"([^"\n]*)"', value)
            ports = mapping.get(path.stem, [])
            rows.append({'java': str(path.relative_to(source)), 'line': text[:match.start()].count('\n') + 1,
                         'method': method, 'arguments': args, 'event_expression': expr,
                         'event': literal[1] if literal else None,
                         'volume': args[index+4] if method == 'getLoopedSound' and len(args)>index+4 else args[index+1] if len(args)>index+1 else None,
                         'pitch': args[index+6] if method == 'getLoopedSound' and len(args)>index+6 else args[index+2] if len(args)>index+2 else None,
                         'port': ports, 'status': 'ported' if ports else 'not_ported'})
    return rows


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def generate(source):
    assets = source / 'src/main/resources/assets/hbm'
    original = json.loads((assets / 'sounds.json').read_text())
    events, categories = convert(original)
    rows = inventory(source)
    # Include the complete reusable machine/item/door families for K2 integration.
    # Music records and unrelated weapons/mobs are outside this work's scope.
    chosen = {name for name in original if name.startswith(('block.', 'item.', 'door.'))}
    chosen.update(r['event'][4:] for r in rows if r['port'] and r['event'] and r['event'].startswith('hbm:'))
    chosen.update(['player.gulp', 'player.groan', 'player.cough', 'entity.vomit', 'weapon.nuclearExplosion', 'weapon.mukeExplosion', 'weapon.explosionMedium',
                   'weapon.explosionLargeNear', 'weapon.explosionLargeFar', 'weapon.explosionSmallNear',
                   'weapon.explosionSmallFar', 'weapon.explosionTiny', 'weapon.reload.boltOpen'])
    chosen &= original.keys()
    excluded = {name: [((value if isinstance(value,str) else value['name']) + '.ogg')
                        for value in definition['sounds']
                        if (value if isinstance(value,str) else value['name']) + '.ogg' in EXCLUDED_FILES]
                for name, definition in original.items()}
    excluded = {name: paths for name, paths in excluded.items() if paths}
    chosen -= excluded.keys()

    selected = {name.lower(): events[name.lower()] for name in sorted(chosen)}
    files = {}
    for name in sorted(chosen):
        for sound in original[name]['sounds']:
            value = {'name': sound} if isinstance(sound, str) else sound
            if value.get('type', 'file') == 'event':
                raise ValueError('Select event reference closure before importing ' + name)
            old = value['name'] + '.ogg'
            new = old.lower()
            if new in files and files[new] != old:
                raise ValueError('Case collision: ' + old)
            files[new] = old
    # Audit BEFORE copying anything. ffprobe is required, not best effort: a
    # future Ogg with an unrecognized copyright tag must stop this import.
    licenses = {}
    metadata = {}
    for new, old in sorted(files.items()):
        tags = audio_tags(assets / 'sounds' / old)
        licenses[new] = classify_tags(tags)
        metadata[new] = tags
    write_json(HERE / 'metadata.json', metadata)
    target = ROOT / 'content/assets/ntm'
    previous = HERE / 'manifest.json'
    if previous.is_file():
        for name in json.loads(previous.read_text())['files'].keys() - files.keys():
            (target / 'sounds' / name).unlink(missing_ok=True)

    for new, old in files.items():
        dest = target / 'sounds' / new
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(assets / 'sounds' / old, dest)
    for source_name, dest_name in [('LICENSE', 'license-gpl.txt'), ('LICENSE.LESSER', 'license-lgpl.txt')]:
        shutil.copyfile(source / source_name, target / 'sounds' / dest_name)
    (target / 'sounds/attribution.txt').write_text('HBM Nuclear Tech Mod audio assets. Source: https://github.com/HbmMods/Hbm-s-Nuclear-Tech-GIT\nLGPL version 3; see license-lgpl.txt and license-gpl.txt.\nOnly resource identifiers and filenames were lowercased; audio bytes unchanged.\n')
    credits = ['HBM Nuclear Tech Mod audio; upstream revision is in tools/sound-gen/manifest.json.',
               'Default declaration: LGPL v3 (license-lgpl.txt and license-gpl.txt).',
               'Audio bytes unchanged. Identifier/file case normalized for modern resources.', '']
    for name, license in licenses.items():
        if license != 'LGPL-3.0-only':
            credits += [name + ': ' + license, json.dumps(metadata[name], ensure_ascii=False), '']
    (target / 'sounds/credits.txt').write_text('\n'.join(credits) + '\n')
    write_json(target / 'sounds.json', selected)
    write_json(ROOT / 'crates/ntm-core/data/sound-events.json', {
        'events': {'hbm:' + name: 'ntm:' + name.lower() for name in sorted(chosen)},
        'categories': {key: categories[key] for key in sorted(categories) if key[4:] in selected},
        'excluded_by_license': {'hbm:' + name: paths for name, paths in sorted(excluded.items())}})
    write_json(HERE / 'original-sounds.json', original)
    write_json(ROOT / 'docs/sounds-inventory.json', rows)
    revision = subprocess.check_output(['git', '-C', str(source), 'rev-parse', 'HEAD'], text=True).strip()
    manifest = {'source_revision': revision, 'license': 'LGPL-3.0-only', 'selected_events': sorted(chosen), 'excluded_by_license': excluded,
                'files': {new: {'source': old, 'license': licenses[new], 'sha256': hashlib.sha256((assets / 'sounds' / old).read_bytes()).hexdigest()} for new,old in sorted(files.items())}}
    write_json(HERE / 'manifest.json', manifest)
    lines = ['# Inventário de áudio Java → port', '',
             'Gerado por `python3 tools/sound-gen/generate.py --source PATH`. Não editar esta tabela.', '',
             f'Fonte `{revision}`. {len(rows)} chamadas ativas, incluindo loops; argumentos preservam expressões Java. `not_ported` significa fora dos sistemas com runtime mapeados neste lote. A presença de um ponto no port não atesta paridade de cada ramo: ver `docs/sons-hbm.md`.', '',
             '| Java | Chamada | Evento/expressão | Volume | Pitch | Ponto do port |', '| --- | --- | --- | --- | --- | --- |']
    for row in rows:
        fields = [f"{row['java']}:{row['line']}", row['method'], row['event'] or row['event_expression'], row['volume'], row['pitch'], ', '.join(row['port']) or 'not_ported']
        lines.append('| ' + ' | '.join(str(f or '').replace('|', '\\|').replace('\n', ' ') for f in fields) + ' |')
    (ROOT / 'docs/sounds-inventory.md').write_text('\n'.join(lines) + '\n')
    print(f'{len(rows)} Java calls, {len(selected)} events, {len(files)} Ogg files')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    generate(parser.parse_args().source.resolve())
