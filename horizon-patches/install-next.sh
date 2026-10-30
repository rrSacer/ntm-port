#!/bin/bash
# Installs the native Horizon build beside ~/Games/horizon-ntm. This install
# starts with new worlds: only display/game options are copied; saves and mod
# data are left empty.
#
#   ~/Games/horizon-ntm-next/       native Horizon, the NTM content mod
#   ~/.local/bin/horizon-ntm-next   launcher
#   ~/.local/share/applications/horizon-ntm-next.desktop
#
# Without --apply this validates inputs and prints the actions. Run as the
# player (sacer). The existing ~/Games/horizon-ntm is read only.
set -euo pipefail

apply=false
for arg in "$@"; do
  case "$arg" in
    --apply) apply=true ;;
    *) echo "usage: $0 [--apply]" >&2; exit 2 ;;
  esac
done

repo="$(cd "$(dirname "$0")/.." && pwd)"
old="${HORIZON_SOURCE:-$HOME/Games/horizon-ntm}"
new="$HOME/Games/horizon-ntm-next"
build="${HORIZON_NEXT_BUILD:-$HOME/Workspace/horizon-ntm-next/target/release}"
ntm_hmod="${NTM_HMOD:-$repo/dist/ntm-content-c1.hmod}"
resources="${HORIZON_RESOURCES:-$HOME/Games/horizon/.local/horizon-client/26.3}"
launcher="$HOME/.local/bin/horizon-ntm-next"
desktop="$HOME/.local/share/applications/horizon-ntm-next.desktop"
uninstall="$HOME/.local/bin/horizon-ntm-next-uninstall"

run() {
  printf '+'
  printf ' %q' "$@"
  printf '\n'
  if $apply; then "$@"; fi
}
zip_ok() {
  python3 -c 'import sys, zipfile; sys.exit(zipfile.ZipFile(sys.argv[1]).testzip() is not None)' "$1" 2>/dev/null
}
install_mod() {
  local tmp="$(dirname "$2")/.$(basename "$2").tmp"
  cp "$1" "$tmp"
  if ! zip_ok "$tmp" || ! cmp -s "$1" "$tmp"; then
    rm -f "$tmp"
    echo "copy of $1 is not whole: nothing more installed" >&2
    exit 1
  fi
  mv -f "$tmp" "$2"
}

[ "$(id -un)" != root ] || { echo "run as the player, not root" >&2; exit 1; }
[ -d "$old/bin" ] || { echo "missing source installation: $old" >&2; exit 1; }
[ -e "$new" ] && { echo "$new already exists; remove or move it first" >&2; exit 1; }
for f in "$build/horizon-client" "$build/horizon-ctl" "$ntm_hmod" "$resources"; do
  [ -e "$f" ] || { echo "missing: $f" >&2; exit 1; }
done
zip_ok "$ntm_hmod" || { echo "$ntm_hmod is not a whole zip" >&2; exit 1; }
pgrep -u "$(id -u)" -x horizon-client >/dev/null && { echo "close the game first" >&2; exit 1; }

echo "== new-world game folder $new"
run mkdir -p "$new/bin" "$new/horizon-mods"
run cp "$build/horizon-client" "$build/horizon-ctl" "$new/bin/"
# Keep user display settings. Do not copy saves or horizon-mods/.horizon-mod-data.
for f in "$old/options.txt" "$old"/horizon-*.txt; do
  [ -f "$f" ] && run cp --reflink=auto -p "$f" "$new/"
done
run install_mod "$ntm_hmod" "$new/horizon-mods/ntm.hmod"
# A mod in horizon-mods/ loads only when Options > Mods has it on
# (horizon-mods.txt); the copied list may lack it, or there is none.
if ! grep -qx 'mod.ntm:true' "$new/horizon-mods.txt" 2>/dev/null; then
  run sh -c "sed -i '/^mod\.ntm:/d' '$new/horizon-mods.txt' 2>/dev/null; echo mod.ntm:true >> '$new/horizon-mods.txt'"
fi

echo "== launcher $launcher"
launcher_text="#!/bin/sh
# Native Horizon with NTM content (see ~/Workspace/ntm-port/horizon-patches/README.md).
cd \"$new\" || exit 1
exec ./bin/horizon-client --resources \"$resources\" \"\$@\"
"
printf '%s' "$launcher_text"
if $apply; then
  mkdir -p "$(dirname "$launcher")"
  printf '%s' "$launcher_text" > "$launcher"
  chmod +x "$launcher"
fi

echo "== menu entry $desktop"
desktop_text="[Desktop Entry]
Type=Application
Name=Horizon (NTM next)
Comment=Horizon com NTM nativo
Exec=$launcher
Icon=$HOME/.local/share/icons/horizon.png
Terminal=false
Categories=Game;
"
printf '%s' "$desktop_text"
if $apply; then
  mkdir -p "$(dirname "$desktop")"
  printf '%s' "$desktop_text" > "$desktop"
fi

undo_text="#!/bin/sh
# Move the next install to a dated backup; never delete its worlds.
set -e
mv \"$new\" \"$new.removed-\$(date +%Y%m%d-%H%M%S)\"
rm -f \"$launcher\" \"$desktop\" \"$uninstall\"
"
echo "== undo: $uninstall"
if $apply; then
  mkdir -p "$(dirname "$uninstall")"
  printf '%s' "$undo_text" > "$uninstall"
  chmod +x "$uninstall"
  echo "Done. Start Horizon (NTM next) from the menu or with: horizon-ntm-next"
else
  echo "Dry run: nothing was written. Run again with --apply to install."
fi
