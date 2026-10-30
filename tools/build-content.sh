#!/bin/bash
# Builds the content mod (blocks, items, data) into dist/ntm.hmod, the one
# the native game loads beside its bundle (docs/nativo.md).
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
packer="${HORIZON_MOD:-/home/sacer/Games/horizon/target/release/cargo-horizon-mod}"
export CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}"
dist="${DIST:-$root/dist}"
mkdir -p "$dist"
(cd "$root/content" && "$packer" build)
cp "$root/content/target/horizon-mod/ntm.hmod" "$dist/ntm.hmod"
ls -l "$dist"
