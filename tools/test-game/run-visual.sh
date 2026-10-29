#!/bin/bash
# The ntm-visual prototype on Xvfb :15, muted, in a throwaway folder
# (never ~/Games/horizon or ~/Games/horizon-ntm; their prepared resources
# are read only). usage: run-visual.sh <name> [game args]
set -euo pipefail

H=${HORIZON_VISUAL:-/home/sacer/Workspace/horizon-visual}
name="${1:?name}"; shift
game="/tmp/ntm-visual-game/$name"
# The mods into the shared dist (their storage, .horizon-mod-data, stays
# beside them), whole: a .hmod copied while tools/build-visual.sh writes it
# is truncated, the content mod doesn't load and the engine turns every
# ntm: block of the world into air. So each one is checked as a zip
# (waiting out a build), copied to a temporary name and renamed, under a
# lock against another agent's game doing the same.
src=${BUILD_DIR:-/home/sacer/Workspace/ntm-port/target/visual-build}/dist
mkdir -p /tmp/ntm-visual-game/dist
(
  flock 9
  for try in $(seq 1 60); do
    ok=1
    for f in "$src"/*.hmod; do
      python3 -c 'import sys, zipfile; sys.exit(zipfile.ZipFile(sys.argv[1]).testzip() is not None)' "$f" 2>/dev/null || { ok=0; break; }
    done
    [ $ok = 1 ] && break
    echo "run-visual: $src incompleto (build em andamento?), esperando..." >&2
    sleep 5
  done
  [ $ok = 1 ] || { echo "run-visual: mods inválidos em $src" >&2; exit 1; }
  for f in "$src"/*.hmod; do
    cp "$f" "/tmp/ntm-visual-game/dist/.$(basename "$f").tmp"
    mv -f "/tmp/ntm-visual-game/dist/.$(basename "$f").tmp" "/tmp/ntm-visual-game/dist/$(basename "$f")"
  done
) 9>/tmp/ntm-visual-game/dist.lock
dist="${DIST:-/tmp/ntm-visual-game/dist}"
mkdir -p "$game"
[ -f "$game/options.txt" ] || printf 'soundCategory_master:0.0\nsoundCategory_music:0.0\n' > "$game/options.txt"
chown -R sacer:sacer /tmp/ntm-visual-game
if [ ! -e /tmp/.X11-unix/X15 ]; then
  /usr/local/bin/agent-run --user sacer Xvfb :15 -screen 0 1280x720x24 -nolisten tcp > /tmp/ntm-visual-game/xvfb.log 2>&1 &
  sleep 2
fi
python3 /home/sacer/Workspace/ntm-port/tools/test-game/mute.py "$game" &
cd "$game"
exec /usr/local/bin/agent-run --user sacer nice -n 10 env HORIZON_HOME="$game" HORIZON_LAUNCHER_HOME="$game/launcher" \
  XDG_DATA_HOME="$game/xdg-data" XDG_CONFIG_HOME="$game/xdg-config" \
  HORIZON_NO_AUDIO=1 HORIZON_LAUNCHER_NO_SELF_UPDATE=1 HORIZON_NO_UPDATE_CHECK=1 \
  HORIZON_LAUNCHER_TRASH="$game/trash" HORIZON_LAUNCHER_CLIPBOARD="$game/clipboard" HORIZON_LAUNCHER_DESKTOP="$game/desktop" \
  WAYLAND_DISPLAY= DISPLAY=:15 \
  $H/target/release/horizon-client \
  --resources /home/sacer/Games/horizon/.local/horizon-client/26.3 \
  --dev-mod "$dist/ntm.hmod" \
  --dev-mod "$dist/ntmlogic-server.hmod" \
  --dev-mod "$dist/ntmlogic-client.hmod" \
  --control-socket "$game/control.sock" \
  --offscreen --render-distance 6 --width 1280 --height 720 --name NtmTeste "$@" \
  > "$game/client.log" 2>&1
