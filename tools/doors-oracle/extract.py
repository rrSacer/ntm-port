#!/usr/bin/env python3
"""Extract the door numbers from the Java source, as the fixture the Rust test reads.

Reads `tileentity/DoorDecl.java` (the 14 `DoorDecl` constants) and
`blocks/ModBlocks.java` (which registry id each constant is built under), and
writes `crates/ntm-core/tests/fixtures/doordecl.json`. The Rust test compares
`doors_security::spec` against that file, so the Java is the reference and no
number is typed twice by hand.

    python3 extract.py                # regenerate the fixture
    python3 extract.py --check        # exit 1 if the committed fixture is stale

The Java tree is `$NTM_SRC`, or `--java-src`. Nothing from it is copied: only
numbers and the registry name come out.
"""
import argparse
import json
import os
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
FIXTURE = HERE.parents[1] / 'crates' / 'ntm-core' / 'tests' / 'fixtures' / 'doordecl.json'
DEFAULT_SRC = '/mnt/ssd120/sacer/horizon-ntm/ntm-src'
PKG = 'src/main/java/com/hbm/'
INT = r'-?\d+'


def strip_comments(text):
    text = re.sub(r'/\*.*?\*/', ' ', text, flags=re.S)
    return re.sub(r'//[^\n]*', ' ', text)


def norm(text):
    return re.sub(r'\s+', ' ', text).strip()


def ints(fragment):
    return [int(v) for v in re.findall(INT, fragment)]


def rows(fragment):
    """`{ {a,b,c}, {d,e} }` -> [[a,b,c],[d,e]]."""
    return [ints(r) for r in re.findall(r'\{([^{}]*)\}', fragment)]


def first(pattern, text, name, required=True):
    m = re.search(pattern, text)
    if m is None:
        if required:
            raise SystemExit(f'{name}: pattern not found: {pattern}')
        return None
    return m


def parse_decl(name, body):
    """One `new DoorDecl() { ... }` body (or the base-class defaults)."""
    time = int(first(r'timeToOpen\(\) \{ return (\d+); \}', body, f'{name} timeToOpen').group(1))

    ranges_m = first(r'getDoorOpenRanges\(\) \{ return new int\[\]\[\] \{ (.*?) \}; \}', body, f'{name} ranges')
    ranges = rows(ranges_m.group(1))

    dims = ints(first(r'getDimensions\(\) \{ return new int\[\] \{ ([^}]*) \}', body, f'{name} dimensions').group(1))
    if len(dims) != 6:
        raise SystemExit(f'{name}: dimensions has {len(dims)} values')

    extra_m = first(r'getExtraDimensions\(\) \{ return new int\[\]\[\] \{ (.*?) \}', body, f'{name} extra', False)
    extra = rows(extra_m.group(1)) if extra_m else []

    offset_m = first(r'getBlockOffset\(\) \{ return (' + INT + r'); \}', body, f'{name} offset', False)
    offset = int(offset_m.group(1)) if offset_m else 0

    remote = re.search(r'remoteControllable\(\) \{ return true; \}', body) is not None
    never = re.search(r'neverDemandRedstone\(\) \{ return true; \}', body) is not None

    window_m = first(r'getDoorRangeOpenTime\(int ticks, int idx\) \{ return getNormTime\(ticks, (\d+), (\d+)\); \}', body, f'{name} window', False)
    window = [int(window_m.group(1)), int(window_m.group(2))] if window_m else None

    return {
        'java': name,
        'time_to_open': time,
        'open_ranges': ranges,
        'dimensions': dims,
        'extra_dimensions': extra,
        'block_offset': offset,
        'remote_controllable': remote,
        'never_demand_redstone': never,
        'time_window': window,
    }


def extract(src):
    decl_path = src / PKG / 'tileentity' / 'DoorDecl.java'
    mod_path = src / PKG / 'blocks' / 'ModBlocks.java'
    decl_text = strip_comments(decl_path.read_text(encoding='utf-8'))
    mod_text = strip_comments(mod_path.read_text(encoding='utf-8'))

    starts = list(re.finditer(r'public static final DoorDecl (\w+) = new DoorDecl\(\) \{', decl_text))
    if len(starts) != 14:
        raise SystemExit(f'expected 14 DoorDecl constants, found {len(starts)}')
    decls = []
    for i, m in enumerate(starts):
        end = starts[i + 1].start() if i + 1 < len(starts) else len(decl_text)
        decls.append(parse_decl(m.group(1), norm(decl_text[m.end():end])))

    # The registry name: `x = new BlockDoorGeneric(Material.iron, DoorDecl.NAME)...setBlockName("id")`.
    ids = {}
    for m in re.finditer(r'new BlockDoorGeneric\([^;]*?DoorDecl\.(\w+)\)[^;]*?setBlockName\("(\w+)"\)', mod_text):
        ids[m.group(1)] = m.group(2)
    for d in decls:
        if d['java'] not in ids:
            raise SystemExit(f"{d['java']}: no BlockDoorGeneric registered in ModBlocks.java")
        d['block_name'] = ids[d['java']]
    return {'source': 'DoorDecl.java + ModBlocks.java', 'doors': decls}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--java-src', default=os.environ.get('NTM_SRC', DEFAULT_SRC))
    ap.add_argument('--check', action='store_true')
    args = ap.parse_args()
    data = extract(Path(args.java_src))
    text = json.dumps(data, indent=2) + '\n'
    if args.check:
        current = FIXTURE.read_text(encoding='utf-8') if FIXTURE.exists() else ''
        if current != text:
            print(f'{FIXTURE} is stale; run extract.py', file=sys.stderr)
            return 1
        print(f'{FIXTURE} matches the Java source ({len(data["doors"])} doors)')
        return 0
    FIXTURE.parent.mkdir(parents=True, exist_ok=True)
    FIXTURE.write_text(text, encoding='utf-8')
    print(f'wrote {FIXTURE} ({len(data["doors"])} doors)')
    return 0


if __name__ == '__main__':
    sys.exit(main())
