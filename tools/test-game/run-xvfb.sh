#!/bin/bash
# As run.sh, but on a virtual display (Xvfb, never the user's screen), at low
# priority in the ntm-builds slice (agent-run --user sacer, nice 10), muted:
# HORIZON_NO_AUDIO=1, options.txt with master/music 0.0 and mute.py, cwd in
# the throwaway folder /tmp/ntm-test-game/<name> (TEST_GAME_ROOT; tmpfs, off /home).
# Usage: run-xvfb.sh <name> [game args]   (DISPLAY_NUM: default 9; DIST: dist/)
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
name="${1:?name}"; shift
game="${TEST_GAME_ROOT:-/tmp/ntm-test-game}/$name"
dist="${DIST:-$root/dist}"
num="${DISPLAY_NUM:-9}"
mkdir -p "$game"
if [ ! -f "$game/options.txt" ]; then
  printf 'soundCategory_master:0.0\nsoundCategory_music:0.0\n' > "$game/options.txt"
fi
chown -R sacer:sacer "$game"
if [ ! -e "/tmp/.X11-unix/X$num" ]; then
  /usr/local/bin/agent-run --user sacer Xvfb ":$num" -screen 0 1280x720x24 -nolisten tcp > "$game/xvfb.log" 2>&1 &
  sleep 2
fi
python3 "$root/tools/test-game/mute.py" "$game" &
cd "$game"
exec /usr/local/bin/agent-run --user sacer nice -n 10 env HORIZON_HOME="$game" HORIZON_LAUNCHER_HOME="$game/launcher" \
  XDG_DATA_HOME="$game/xdg-data" XDG_CONFIG_HOME="$game/xdg-config" \
  HORIZON_NO_AUDIO=1 HORIZON_LAUNCHER_NO_SELF_UPDATE=1 \
  WAYLAND_DISPLAY= DISPLAY=":$num" HORIZON_NO_UPDATE_CHECK=1 \
  /home/sacer/Games/horizon/target/release/horizon-client \
  --resources /home/sacer/Games/horizon/.local/horizon-client/26.3 \
  --dev-mod "$dist/ntm.hmod" \
  --dev-mod "$dist/ntmlogic-server.hmod" \
  --dev-mod "$dist/ntmlogic-client.hmod" \
  --control-socket "$game/control.sock" \
  --offscreen --render-distance 6 --width 960 --height 540 --name NtmTeste "$@" \
  > "$game/client.log" 2>&1
