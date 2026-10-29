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
if [ "$(id -un)" = root ]; then echo "run as the player, not root" >&2; exit 1; fi
[ -d "$game/bin" ] || { echo "no $game: install it with install-side-by-side.sh" >&2; exit 1; }
for f in "$build/horizon-client" "$build/horizon-ctl" "$mods/ntm.hmod" "$mods/ntmlogic-client.hmod" \
  "$mods/ntmlogic-server.hmod" "$recipes"; do
  [ -e "$f" ] || { echo "missing: $f" >&2; exit 1; }
done
grep -qa "content mod machines" "$build/horizon-client" ||
  { echo "$build/horizon-client has no ntm-visual" >&2; exit 1; }
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
run cp "$mods/ntm.hmod" "$mods/ntmlogic-client.hmod" "$mods/ntmlogic-server.hmod" "$game/horizon-mods/"
run cp "$recipes" "$game/horizon-mods/recipes.hmod"
echo "== undo: cp -p $backup/bin/* $game/bin/ && cp -p $backup/horizon-mods/* $game/horizon-mods/"
$apply || echo "Dry run: nothing was written. Run again with --apply to update."
