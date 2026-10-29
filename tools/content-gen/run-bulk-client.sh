#!/bin/bash
# All game state lives in a disposable cwd. No player installation is touched.
set -euo pipefail
base=$(cd "$(dirname "$0")/../.." && pwd)
evidence="$base/content-gen-evidence/bulk"
scratch=$(mktemp -d /tmp/ntm-bulk-XXXXXX)
mkdir -p "$scratch/game" "$scratch/home" "$scratch/launcher" "$scratch/data" "$scratch/config" "$evidence"
cat > "$scratch/game/options.txt" <<'OPTIONS'
soundCategory_master:0.0
soundCategory_music:0.0
renderDistance:2
simulationDistance:2
maxFps:15
enableVsync:false
OPTIONS
printf '%s\n' "$scratch" > "$evidence/run-path.txt"
python3 "$base/tools/test-game/mute.py" "$scratch/game" &
Xvfb :10 -screen 0 1280x720x24 -nolisten tcp > "$scratch/xvfb.log" 2>&1 &
xvfb_pid=$!
trap 'kill "$xvfb_pid" 2>/dev/null || true' EXIT
for attempt in $(seq 1 50); do
  if [ -S /tmp/.X11-unix/X10 ]; then break; fi
  sleep 0.1
done
cd "$scratch/game"
env HOME="$scratch/home" HORIZON_HOME="$scratch/game" HORIZON_LAUNCHER_HOME="$scratch/launcher" XDG_DATA_HOME="$scratch/data" XDG_CONFIG_HOME="$scratch/config" HORIZON_LAUNCHER_NO_SELF_UPDATE=1 HORIZON_NO_UPDATE_CHECK=1 HORIZON_NO_AUDIO=1 XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY= DISPLAY=:10 SDL_VIDEODRIVER=x11 /home/sacer/Games/horizon/target/release/horizon-client --resources /home/sacer/Games/horizon/.local/horizon-client/26.3 --dev-mod "$base/content/target/horizon-mod/ntm.hmod" --control-socket "$scratch/control.sock" --offscreen --test-terrain --creative --world NTMBulk --render-distance 2 --width 1280 --height 720 --name BulkProbe "$@"
