#!/bin/bash
# The NTM native bundle (native/ntm, docs/nativo.md) ready for
# horizon-mixins: the bundle's manifest and helpers, with crates/ntm-core
# copied in as its [[crate]]. A bundle crate takes only plain registry
# dependencies, so the workspace's inherited fields and dependencies are
# written out as literals.
# usage: package-native.sh [OUT]   (default target/native-bundle/ntm)
set -euo pipefail
P="$(cd "$(dirname "$0")/.." && pwd)"
OUT=${1:-$P/target/native-bundle/ntm}

rm -rf "$OUT"
mkdir -p "$OUT"
cp -a "$P/native/ntm/mixins/ntm/." "$OUT/"
mkdir -p "$OUT/ntm-core"
# The crate's sources and tables; its tests and examples stay out (they
# read the repository).
cp -a "$P/crates/ntm-core/src" "$P/crates/ntm-core/data" "$OUT/ntm-core/"
python3 - "$P/Cargo.toml" "$P/crates/ntm-core/Cargo.toml" "$OUT/ntm-core/Cargo.toml" <<'PY'
import sys, tomllib
root = tomllib.load(open(sys.argv[1], 'rb'))
crate = tomllib.load(open(sys.argv[2], 'rb'))
pkg, deps = root['workspace']['package'], root['workspace']['dependencies']

def value(v):
    if isinstance(v, str):
        return f'"{v}"'
    if isinstance(v, list):
        return '[' + ', '.join(value(x) for x in v) + ']'
    if isinstance(v, bool):
        return 'true' if v else 'false'
    if isinstance(v, dict):
        return '{ ' + ', '.join(f'{k} = {value(x)}' for k, x in v.items()) + ' }'
    raise SystemExit(f'package-native: {v!r}')

out = ['[package]']
for k, v in crate['package'].items():
    out.append(f'{k} = {value(pkg[k] if isinstance(v, dict) and v.get("workspace") else v)}')
out.append('\n[dependencies]')
for k, v in crate.get('dependencies', {}).items():
    if isinstance(v, dict) and v.get('workspace'):
        v = deps[k]
    if isinstance(v, dict) and 'path' in v:
        raise SystemExit(f'package-native: {k} is a path dependency; a bundle crate takes registry crates only')
    out.append(f'{k} = {value(v)}')
for table in ('features',):
    if table in crate:
        out.append(f'\n[{table}]')
        out += [f'{k} = {value(v)}' for k, v in crate[table].items()]
open(sys.argv[3], 'w').write('\n'.join(out) + '\n')
PY
echo "$OUT"
