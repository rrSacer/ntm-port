#!/bin/bash
# Builds the NTM mods for the ntm-visual engine (docs/visual-fiel.md) in a copy
# of this working tree (BUILD_DIR, default /tmp/ntm-visual-build): the SDK
# of horizon-visual, the machines feature on, the server mod with capability
# machines. Output: $BUILD_DIR/dist. The repo itself is not touched.
# usage: build-visual.sh [test]
set -euo pipefail
B=${BUILD_DIR:-/tmp/ntm-visual-build}
P="$(cd "$(dirname "$0")/.." && pwd)"
H=${HORIZON_VISUAL:-/home/sacer/Workspace/horizon-visual}

mkdir -p $B
# Fresh sources every time, the target kept.
find $B -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} +
# The working tree (HEAD's ntm-explosion does not build on its own), minus
# targets and outputs.
rsync -a --exclude target --exclude dist --exclude spikes --exclude '*-evidence' --exclude .git $P/ $B/
sed -i "s#/home/sacer/Games/horizon/crates/horizon-mod-sdk#$H/crates/horizon-mod-sdk#" $B/Cargo.toml
for c in $B/mods/ntm-logic-server/Cargo.toml $B/mods/ntm-logic-client/Cargo.toml; do
  sed -i 's/^machines = \["ntm-core\/machines"\]$/default = ["machines"]\nmachines = ["ntm-core\/machines"]/' $c
  grep -q '^default = \["machines"\]' $c
done
m=$B/mods/ntm-logic-server/horizon-mod.toml
sed -i 's/^capabilities = \["storage", "world", "players"\]$/capabilities = ["storage", "world", "players", "machines"]\nexperimental = ["machines"]/' $m
grep -q machines $m
cd $B
if [ "${1:-}" = test ]; then
  exec agent-run cargo test --release -j 6 -p ntm-core --lib "${@:2}"
fi
export CARGO_TARGET_DIR=$B/target HORIZON_MOD=$H/target/release/cargo-horizon-mod DIST=$B/dist
export CARGO_NET_OFFLINE=false
agent-run bash -c "tools/build-logic.sh  # cargo"
