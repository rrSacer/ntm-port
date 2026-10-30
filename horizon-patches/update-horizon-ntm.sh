#!/bin/bash
# Updates an existing ~/Games/horizon-ntm (made by install-side-by-side.sh) to
# the current build: new horizon-client and horizon-ctl, the NTM .hmod files
# of tools/build-visual.sh and the patched Recipes. Saves, options and the
# mods' settings stay as they are. What it replaces goes to a dated backup
# folder inside the game folder. ~/Games/horizon is never touched.
# Without --apply it only prints what it would do. Run it as the player.
set -euo pipefail
apply=false
[ "${1:-}" = "--apply" ] && apply=true
game="$HOME/Games/horizon-ntm"
build="${HORIZON_PATCHED:-$HOME/Workspace/horizon-visual}/target/release"
mods="${NTM_MODS:-$HOME/Workspace/ntm-port/target/visual-build/dist}"
recipes="${RECIPES_HMOD:-${HORIZON_PATCHED:-$HOME/Workspace/horizon-visual}/target/horizon-mod/recipes.hmod}"
backup="$game/backup-$(date +%Y%m%d-%H%M%S)"
run() { echo "+ $*"; if $apply; then "$@"; fi; }
# A .hmod copied while tools/build-visual.sh writes it is truncated: the
# content mod doesn't load and the engine turns every ntm: block of the
# world into air. So a mod must be a whole zip, no build may be running, and
# each one is copied to a temporary name, checked against its source and
# renamed into place.
zip_ok() { python3 -c 'import sys, zipfile; sys.exit(zipfile.ZipFile(sys.argv[1]).testzip() is not None)' "$1" 2>/dev/null; }
install_mod() {
  local tmp="$(dirname "$2")/.$(basename "$2").tmp"
  cp "$1" "$tmp"
  if ! zip_ok "$tmp" || ! cmp -s "$1" "$tmp"; then
    rm -f "$tmp"; echo "copy of $1 is not whole (a build wrote it meanwhile?): nothing more installed" >&2; exit 1
  fi
  mv -f "$tmp" "$2"
}
if [ "$(id -un)" = root ]; then echo "run as the player, not root" >&2; exit 1; fi
[ -d "$game/bin" ] || { echo "no $game: install it with install-side-by-side.sh" >&2; exit 1; }
for f in "$build/horizon-client" "$build/horizon-ctl" "$mods/ntm.hmod" "$mods/ntmlogic-client.hmod" \
  "$mods/ntmlogic-server.hmod" "$recipes"; do
  [ -e "$f" ] || { echo "missing: $f" >&2; exit 1; }
done
pgrep -f "tools/build-visual.sh" >/dev/null && { echo "tools/build-visual.sh is running: wait for it" >&2; exit 1; }
for f in "$mods/ntm.hmod" "$mods/ntmlogic-client.hmod" "$mods/ntmlogic-server.hmod" "$recipes"; do
  zip_ok "$f" || { echo "$f is not a whole zip (a build in progress?): nothing installed" >&2; exit 1; }
done
grep -qa "content mod machines" "$build/horizon-client" ||
  { echo "$build/horizon-client has no ntm-visual" >&2; exit 1; }
# Without --features horizon-client/mods the game loads the content mod but
# starts no server mod ("this build has no mod runtime"): no NTM logic runs.
if grep -qa "this build has no mod runtime" "$build/horizon-client" || ! grep -qa wasmtime "$build/horizon-client"; then
  echo "$build/horizon-client has no mod runtime: build it with --features horizon-client/mods" >&2; exit 1
fi
unzip -p "$mods/ntmlogic-server.hmod" horizon-mod.toml 2>/dev/null | grep -q '"machines"' ||
  { echo "$mods/ntmlogic-server.hmod lacks capability machines (tools/build-visual.sh)" >&2; exit 1; }
pgrep -u "$(id -u)" -x horizon-client >/dev/null && { echo "close the game first" >&2; exit 1; }
echo "== backup of what is replaced: $backup"
run mkdir -p "$backup/bin" "$backup/horizon-mods"
run cp -p "$game/bin/horizon-client" "$game/bin/horizon-ctl" "$backup/bin/"
for f in ntm.hmod ntmlogic-client.hmod ntmlogic-server.hmod recipes.hmod; do
  [ -f "$game/horizon-mods/$f" ] && run cp -p "$game/horizon-mods/$f" "$backup/horizon-mods/"
done
echo "== new build"
run cp "$build/horizon-client" "$build/horizon-ctl" "$game/bin/"
for f in ntm.hmod ntmlogic-client.hmod ntmlogic-server.hmod; do
  run install_mod "$mods/$f" "$game/horizon-mods/$f"
done
run install_mod "$recipes" "$game/horizon-mods/recipes.hmod"
if $apply; then
  for f in ntm.hmod ntmlogic-client.hmod ntmlogic-server.hmod recipes.hmod; do
    zip_ok "$game/horizon-mods/$f" || { echo "$game/horizon-mods/$f is not whole: undo with the line below" >&2; exit 1; }
  done
  echo "== the four mods are whole zips"
fi
echo "== undo: cp -p $backup/bin/* $game/bin/ && cp -p $backup/horizon-mods/* $game/horizon-mods/"
$apply || echo "Dry run: nothing was written. Run again with --apply to update."
