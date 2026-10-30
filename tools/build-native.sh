#!/bin/bash
# The game with the NTM native bundle and the official Recipes bundle built
# in (docs/nativo.md): ntm-next's source (an export of a commit,
# never the checkout), horizon-mixins apply, cargo build. The repository and
# the ntm-next checkout are not touched.
# usage: build-native.sh            → $BUILD_DIR/target/release/horizon-client
#   HORIZON_NEXT  the ntm-next checkout (default /home/sacer/Workspace/horizon-ntm-next)
#   NEXT_REV      its commit or branch (default ntm-next)
#   BUILD_DIR     default target/native-build
set -euo pipefail
P="$(cd "$(dirname "$0")/.." && pwd)"
H=${HORIZON_NEXT:-/home/sacer/Workspace/horizon-ntm-next}
REV=${NEXT_REV:-ntm-next}
B=${BUILD_DIR:-$P/target/native-build}
# horizon-mixins 0.6 (named hooks, bundle crates) and the official Recipes.
compat() { python3 -c "import json,sys; print(json.load(open(sys.argv[1]))[sys.argv[2]])" "$P/native/ntm/compatibility.json" "$1"; }
MIXINS_REV=$(compat horizon_mixins_revision)
MODS_REV=${MODS_REV:-$(compat horizon_mods_revision)}
MODS_REPO=https://github.com/horizon-rust/horizon-mods
mkdir -p "$B"

# The source: one export per commit.
commit=$(git -C "$H" -c safe.directory="$H" rev-parse "$REV")
if [ "$(cat "$B/source/.horizon-revision" 2>/dev/null)" != "$commit" ]; then
  rm -rf "$B/source"
  mkdir -p "$B/source"
  git -C "$H" -c safe.directory="$H" archive "$commit" | tar -x -C "$B/source"
  # As the launcher does: links are left out (horizon-mixins refuses them).
  find "$B/source" -type l -delete
  echo "$commit" > "$B/source/.horizon-revision"
fi

# horizon-mixins, built once at its commit.
tool="$B/tools/$MIXINS_REV/bin/horizon-mixins"
if [ ! -x "$tool" ]; then
  cargo install --locked --git https://github.com/horizon-rust/horizon-mixins --rev "$MIXINS_REV" \
    --root "$B/tools/$MIXINS_REV" horizon-mixins-cli
fi

# The bundles: NTM packaged from this repository, Recipes from horizon-mods.
if [ ! -d "$B/horizon-mods/.git" ]; then
  git clone -q "$MODS_REPO" "$B/horizon-mods"
fi
git -C "$B/horizon-mods" -c advice.detachedHead=false checkout -q "$MODS_REV"
rm -rf "$B/mods"
mkdir -p "$B/mods"
"$P/tools/package-native.sh" "$B/mods/ntm" > /dev/null
cp -a "$B/horizon-mods/native/recipes/mixins/recipes" "$B/mods/recipes"
# As the launcher does in its copy: each bundle builds on the commit at hand
# (they use only named hook points, so any commit with their API serves).
for m in "$B"/mods/*/horizon-mixin.toml; do
  sed -i "s/^horizon_revision = .*/horizon_revision = \"$commit\"/" "$m"
done

# Mixins, then the release's own command (no `mods` feature: no WebAssembly).
"$tool" apply --workspace "$B/source" --mods "$B/mods" --out "$B/stage"
(cd "$B/stage" && CARGO_TARGET_DIR="$B/target" cargo build --locked --release -p horizon-client)
echo "$B/target/release/horizon-client"
