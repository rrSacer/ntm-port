#!/bin/bash
# Installs the patched Horizon (ntm-patched: creative-plus, recipes-tick,
# mod-item-tags, content-read-once, ntm-visual) BESIDE the player's own
# ~/Games/horizon, which it only reads:
#   ~/Games/horizon-ntm/            the new game folder (its own options, saves, mods)
#     bin/horizon-client, bin/horizon-ctl   from ~/Workspace/horizon-patched/target/release
#     options.txt, horizon-*.txt, saves/    COPIED (never moved) from ~/Games/horizon
#     horizon-mods/                         the NTM .hmod files (tools/build-visual.sh: the
#                                           server mod with capability machines) and the patched Recipes
#   ~/.local/bin/horizon-ntm                launcher (prepared resources of ~/Games/horizon, read only)
#   ~/.local/share/applications/horizon-ntm.desktop   menu entry "Horizon (NTM)"
# Without --apply it only prints what it would do. Run it as the player (sacer).
# Undo: horizon-ntm-uninstall (printed at the end): it moves the folder to a
# dated backup and removes the launcher and menu entry; ~/Games/horizon is
# never written.
set -euo pipefail

apply=false
[ "${1:-}" = "--apply" ] && apply=true

old="$HOME/Games/horizon"
new="$HOME/Games/horizon-ntm"
build="${HORIZON_PATCHED:-$HOME/Workspace/horizon-patched}/target/release"
mods="${NTM_MODS:-$HOME/Workspace/ntm-port/target/visual-build/dist}"
recipes="${RECIPES_HMOD:-$HOME/Workspace/horizon-patched/target/horizon-mod/recipes.hmod}"
resources="$old/.local/horizon-client/26.3"
launcher="$HOME/.local/bin/horizon-ntm"
desktop="$HOME/.local/share/applications/horizon-ntm.desktop"

run() {
  echo "+ $*"
  if $apply; then "$@"; fi
}
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

# What must exist before anything is done.
for f in "$build/horizon-client" "$build/horizon-ctl" "$resources" \
  "$mods/ntm.hmod" "$mods/ntmlogic-client.hmod" "$mods/ntmlogic-server.hmod" "$recipes"; do
  [ -e "$f" ] || { echo "missing: $f" >&2; exit 1; }
done
[ -e "$new" ] && { echo "$new already exists: remove it first (horizon-ntm-uninstall)" >&2; exit 1; }
# The engine has ntm-visual and the server mod uses it (else its machines keep the old screens).
grep -q "mod_machines" "${HORIZON_PATCHED:-$HOME/Workspace/horizon-patched}/crates/horizon-world/src/lib.rs" ||
  { echo "the engine at ${HORIZON_PATCHED:-$HOME/Workspace/horizon-patched} lacks the ntm-visual patch" >&2; exit 1; }
grep -qa "content mod machines" "$build/horizon-client" ||
  { echo "$build/horizon-client is an older build, without ntm-visual (build it, or HORIZON_PATCHED=~/Workspace/horizon-visual)" >&2; exit 1; }
unzip -p "$mods/ntmlogic-server.hmod" horizon-mod.toml 2>/dev/null | grep -q '"machines"' ||
  { echo "$mods/ntmlogic-server.hmod is not built with capability machines (tools/build-visual.sh)" >&2; exit 1; }
if [ "$(id -un)" = root ]; then echo "run as the player, not root" >&2; exit 1; fi
pgrep -f "tools/build-visual.sh" >/dev/null && { echo "tools/build-visual.sh is running: wait for it" >&2; exit 1; }
for f in "$mods/ntm.hmod" "$mods/ntmlogic-client.hmod" "$mods/ntmlogic-server.hmod" "$recipes"; do
  zip_ok "$f" || { echo "$f is not a whole zip (a build in progress?): nothing installed" >&2; exit 1; }
done

echo "== game folder $new"
run mkdir -p "$new/bin" "$new/horizon-mods"
run cp "$build/horizon-client" "$build/horizon-ctl" "$new/bin/"
# Copies (reflinks on btrfs: no extra space until one side changes).
for f in options.txt horizon-extras.txt horizon-graphics.txt horizon-player.txt horizon-mods.txt; do
  [ -f "$old/$f" ] && run cp --reflink=auto -p "$old/$f" "$new/$f"
done
[ -d "$old/saves" ] && run cp -a --reflink=auto "$old/saves" "$new/saves"
# The mods' own settings (Recipes' "Click gives items", NTM's keys).
[ -d "$old/horizon-mods/.horizon-mod-data" ] &&
  run cp -a --reflink=auto "$old/horizon-mods/.horizon-mod-data" "$new/horizon-mods/"
for f in ntm.hmod ntmlogic-client.hmod ntmlogic-server.hmod; do
  run install_mod "$mods/$f" "$new/horizon-mods/$f"
done
run install_mod "$recipes" "$new/horizon-mods/recipes.hmod"

echo "== launcher $launcher"
script="#!/bin/sh
# Horizon with the NTM patches (see ~/Workspace/ntm-port/horizon-patches/README.md).
cd \"$new\" || exit 1
exec ./bin/horizon-client --resources \"$resources\" \"\$@\"
"
echo "$script"
if $apply; then
  mkdir -p "$(dirname "$launcher")"
  printf '%s' "$script" > "$launcher"
  chmod +x "$launcher"
fi

echo "== menu entry $desktop"
entry="[Desktop Entry]
Type=Application
Name=Horizon (NTM)
Comment=Horizon com os patches do NTM (abas criativas, painel de receitas no criativo)
Exec=$launcher
Icon=$HOME/.local/share/icons/horizon.png
Terminal=false
Categories=Game;
"
echo "$entry"
$apply && printf '%s' "$entry" > "$desktop"

uninstall="$HOME/.local/bin/horizon-ntm-uninstall"
undo="#!/bin/sh
# Removes the side-by-side NTM install: its folder (worlds included) is MOVED
# to a dated backup, never deleted. ~/Games/horizon is not touched.
set -e
mv \"$new\" \"$new.removed-\$(date +%Y%m%d-%H%M%S)\"
rm -f \"$launcher\" \"$desktop\" \"$uninstall\"
"
echo "== undo: $uninstall"
$apply && { printf '%s' "$undo" > "$uninstall"; chmod +x "$uninstall"; }

if $apply; then
  echo "Done. Start it from the menu (Horizon (NTM)) or with: horizon-ntm"
else
  echo "Dry run: nothing was written. Run again with --apply to install."
fi
