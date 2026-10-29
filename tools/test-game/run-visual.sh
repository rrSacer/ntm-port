#!/bin/bash
# The ntm-visual prototype on Xvfb :15, muted, in a throwaway folder
# (never ~/Games/horizon or ~/Games/horizon-ntm; their prepared resources
# are read only). usage: run-visual.sh <name> [game args]
set -euo pipefail

H=${HORIZON_VISUAL:-/home/sacer/Workspace/horizon-visual}
name="${1:?name}"; shift
game="/tmp/ntm-visual-game/$name"
mkdir -p /tmp/ntm-visual-game/dist; cp ${BUILD_DIR:-/tmp/ntm-visual-build}/dist/*.hmod /tmp/ntm-visual-game/dist/
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
