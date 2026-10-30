#!/bin/bash
# Builds the NTM mods for the ntm-visual engine (docs/visual-fiel.md) in a copy
# of this working tree (BUILD_DIR, default target/visual-build): the SDK
# of horizon-visual, the machines feature on, the server mod with capability
# machines. Output: $BUILD_DIR/dist. The repo itself is not touched.
# usage: build-visual.sh [test]
set -euo pipefail
P="$(cd "$(dirname "$0")/.." && pwd)"
B=${BUILD_DIR:-$P/target/visual-build}
H=${HORIZON_VISUAL:-/home/sacer/Workspace/horizon-visual}

mkdir -p $B
# Fresh sources every time;  target and dist stay: a `test` run leaves the last
# built mods where install-side-by-side.sh and run-visual.sh read them.
find $B -mindepth 1 -maxdepth 1 ! -name target ! -name dist -exec rm -rf {} +
# The working tree, minus
# targets and outputs.
rsync -a --exclude target --exclude dist --exclude spikes --exclude '*-evidence' --exclude .git $P/ $B/
sed -i "s#/home/sacer/Games/horizon/crates/horizon-mod-sdk#$H/crates/horizon-mod-sdk#" $B/Cargo.toml
c=$B/mods/ntm-logic-server/Cargo.toml
sed -i 's/^machines = \[\]$/default = ["machines"]\nmachines = []/' $c
grep -q '^default = \["machines"\]' $c
m=$B/mods/ntm-logic-server/horizon-mod.toml
sed -i 's/^capabilities = \["storage", "world", "players"\]$/capabilities = ["storage", "world", "players", "machines"]\nexperimental = ["machines"]/' $m
grep -q machines $m
# Durable items use the upstream components schema, matching
# tools/content-gen/generate.py --max-damage. ids.json keeps the source limits.
python3 - $B/content <<'PY'
import json, sys
from pathlib import Path
c = Path(sys.argv[1])
for e in json.loads((c / 'ids.json').read_text())['entries']:
    if e.get('type') == 'item' and e.get('max_damage'):
        f = c / 'data/ntm/horizon/item' / (e['id'].split(':', 1)[1] + '.json')
        d = json.loads(f.read_text())
        assert d.get('max_stack_size') == 1, f
        d.pop('max_damage', None)
        d.setdefault('components', {})['minecraft:max_damage'] = e['max_damage']
        f.write_text(json.dumps(d, indent=2) + '\n')
PY
cd $B
if [ "${1:-}" = test ]; then
  exec agent-run cargo test --release -j 6 -p ntm-core --lib "${@:2}"
fi
export CARGO_TARGET_DIR=$B/target HORIZON_MOD=$H/target/release/cargo-horizon-mod DIST=$B/dist
export CARGO_NET_OFFLINE=false
agent-run bash -c "tools/build-logic.sh  # cargo"
