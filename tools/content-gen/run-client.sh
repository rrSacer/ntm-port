#!/bin/bash
set -euo pipefail
base=$(cd "$(dirname "$0")/../.." && pwd)
evidence="$base/content-gen-evidence"
scratch=$(mktemp -d /tmp/ntm-content-gen-XXXXXX)
mkdir -p "$scratch/game" "$scratch/launcher" "$scratch/data" "$scratch/config"
cat > "$scratch/game/options.txt" <<'OPTIONS'
soundCategory_master:0.0
soundCategory_music:0.0
renderDistance:2
simulationDistance:2
maxFps:30
enableVsync:false
OPTIONS
printf '%s\n' "$scratch" > "$evidence/run-path.txt"
chown -R sacer:sacer "$scratch" "$evidence"
python3 "$base/tools/content-gen/mute-client.py" "$scratch/game" "$evidence/audio-muted.log" &
cd "$scratch/game"
exec runuser -u sacer -- env HORIZON_HOME="$scratch/game" HORIZON_LAUNCHER_HOME="$scratch/launcher" XDG_DATA_HOME="$scratch/data" XDG_CONFIG_HOME="$scratch/config" HORIZON_LAUNCHER_NO_SELF_UPDATE=1 HORIZON_NO_UPDATE_CHECK=1 HORIZON_NO_AUDIO=1 XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=wayland-0 DISPLAY=:0 /home/sacer/Games/horizon/target/release/horizon-client --resources /home/sacer/Games/horizon/.local/horizon-client/26.3 --dev-mod "$base/content/target/horizon-mod/ntm.hmod" --control-socket "$scratch/control.sock" --offscreen --test-terrain --creative --world NTMContent --render-distance 2 --width 1280 --height 720 --name ContentProbe "$@"
