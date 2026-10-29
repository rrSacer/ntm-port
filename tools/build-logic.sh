#!/bin/bash
# Builds the two ntmlogic mods into dist/ (both have the id ntmlogic, so
# cargo-horizon-mod writes the same target/horizon-mod/ntmlogic.hmod: each is
# copied out right after its build).
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
packer="${HORIZON_MOD:-/home/sacer/Games/horizon/target/release/cargo-horizon-mod}"
export CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}"
dist="${DIST:-$root/dist}"
mkdir -p "$dist"
for side in server client; do
  (cd "$root/mods/ntm-logic-$side" && "$packer" build)
  cp "${CARGO_TARGET_DIR:-$root/target}/horizon-mod/ntmlogic.hmod" "$dist/ntmlogic-$side.hmod"
done
if [ -f "$root/content/horizon-mod.toml" ]; then
  (cd "$root/content" && "$packer" build)
  cp "$root/content/target/horizon-mod/ntm.hmod" "$dist/ntm.hmod"
fi
ls -l "$dist"
