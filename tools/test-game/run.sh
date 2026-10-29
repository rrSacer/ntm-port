#!/bin/bash
# Starts a muted, offscreen Horizon on a throwaway game folder with the NTM
# mods (dist/), driven through a control socket. Usage: run.sh <name> [args]
# The folder is target/test-game/<name>: never ~/.minecraft or Horizon's own.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
name="${1:?name}"; shift
game="$root/target/test-game/$name"
# DIST: another folder of .hmod files (each one has its own storage beside it).
dist="${DIST:-$root/dist}"
mkdir -p "$game"
if [ ! -f "$game/options.txt" ]; then
  # Silent from the first frame: master and music at 0.
  printf 'soundCategory_master:0.0\nsoundCategory_music:0.0\n' > "$game/options.txt"
fi
chown -R sacer:sacer "$root/target/test-game"
python3 "$root/tools/test-game/mute.py" "$game" &
# The game writes its content cache in the working directory.
cd "$game"
exec runuser -u sacer -- env HORIZON_HOME="$game" HORIZON_LAUNCHER_HOME="$game/launcher" \
  XDG_DATA_HOME="$game/xdg-data" XDG_CONFIG_HOME="$game/xdg-config" \
  HORIZON_NO_AUDIO=1 HORIZON_LAUNCHER_NO_SELF_UPDATE=1 XDG_RUNTIME_DIR=/run/user/1000 \
  WAYLAND_DISPLAY=wayland-0 DISPLAY=:0 HORIZON_NO_UPDATE_CHECK=1 \
  /home/sacer/Games/horizon/target/release/horizon-client \
  --resources /home/sacer/Games/horizon/.local/horizon-client/26.3 \
  --dev-mod "$dist/ntm.hmod" \
  --dev-mod "$dist/ntmlogic-server.hmod" \
  --dev-mod "$dist/ntmlogic-client.hmod" \
  --control-socket "$game/control.sock" \
  --offscreen --render-distance 4 --width 960 --height 540 --name NtmTeste "$@" \
  > "$game/client.log" 2>&1
