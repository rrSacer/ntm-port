#!/bin/bash
# Updates only ~/Games/horizon-ntm-next from the native Horizon build and the
# content-only ntm.hmod. The older ~/Games/horizon-ntm is never written.
# Without --apply this validates the inputs and prints the proposed actions.
set -euo pipefail

apply=false
no_native_check=false
for arg in "$@"; do
  case "$arg" in
    --apply) apply=true ;;
    --no-native-check) no_native_check=true ;;
    *) echo "usage: $0 [--apply] [--no-native-check]" >&2; exit 2 ;;
  esac
done

repo="$(cd "$(dirname "$0")/.." && pwd)"
game="$HOME/Games/horizon-ntm-next"
build="${HORIZON_NEXT_BUILD:-$HOME/Workspace/horizon-ntm-next/target/release}"
ntm_hmod="${NTM_HMOD:-$repo/dist/ntm-content-c1.hmod}"
cargo_horizon_mod="${CARGO_HORIZON_MOD:-$build/cargo-horizon-mod}"
backup="$game/backup-$(date +%Y%m%d-%H%M%S)"

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
    echo "copy of $1 is not whole (a build may have written it): aborting" >&2
    exit 1
  fi
  mv -f "$tmp" "$2"
}

[ "$(id -un)" != root ] || { echo "run as the player, not root" >&2; exit 1; }
[ -d "$game/bin" ] || { echo "no $game: install it with install-next.sh" >&2; exit 1; }
for f in "$game/bin/horizon-client" "$game/bin/horizon-ctl" \
  "$build/horizon-client" "$build/horizon-ctl" "$ntm_hmod" "$cargo_horizon_mod"; do
  [ -e "$f" ] || { echo "missing: $f" >&2; exit 1; }
done
zip_ok "$ntm_hmod" || { echo "$ntm_hmod is not a whole zip" >&2; exit 1; }
if ! $no_native_check && ! grep -qa 'ntm native' "$build/horizon-client"; then
  echo "$build/horizon-client has no \"ntm native\" marker (use --no-native-check only for an explicit pre-bundle test)" >&2
  exit 1
fi
"$cargo_horizon_mod" check "$ntm_hmod"
pgrep -u "$(id -u)" -x horizon-client >/dev/null && { echo "close the game first" >&2; exit 1; }

shopt -s nullglob
obsolete=("$game/horizon-mods"/ntmlogic-*.hmod "$game/horizon-mods/recipes.hmod")
echo "== backup of replaced files: $backup"
run mkdir -p "$backup/bin" "$backup/horizon-mods"
run cp -p "$game/bin/horizon-client" "$game/bin/horizon-ctl" "$backup/bin/"
for f in "$game/horizon-mods/ntm.hmod" "${obsolete[@]}"; do
  [ -f "$f" ] && run cp -p "$f" "$backup/horizon-mods/"
done

echo "== new native build"
run cp "$build/horizon-client" "$build/horizon-ctl" "$game/bin/"
run mkdir -p "$game/horizon-mods"
run install_mod "$ntm_hmod" "$game/horizon-mods/ntm.hmod"
for f in "${obsolete[@]}"; do
  [ -f "$f" ] && run rm -f "$f"
done
if $apply; then
  zip_ok "$game/horizon-mods/ntm.hmod" || {
    echo "$game/horizon-mods/ntm.hmod is not whole; restore from $backup" >&2
    exit 1
  }
  echo "== ntm.hmod is a whole zip; obsolete WASM mods removed"
fi
echo "== undo: cp -p $backup/bin/* $game/bin/ && cp -p $backup/horizon-mods/* $game/horizon-mods/"
$apply || echo "Dry run: nothing was written. Run again with --apply to update."
