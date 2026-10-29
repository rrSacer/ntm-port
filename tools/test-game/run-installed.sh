#!/bin/bash
# Like run.sh, but the mods are INSTALLED (copied into horizon-mods/ of the
# game folder, turned on in Options > Mods), as docs/INSTALAR.md tells the
# player: no --dev-mod. Usage: run-installed.sh <name>
# HORIZON_CLIENT and HORIZON_RES pick another build (a patched Horizon) and
# its prepared resources.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
name="${1:?name}"; shift
game="$root/target/test-game/$name"
mkdir -p "$game/horizon-mods"
if [ ! -f "$game/options.txt" ]; then
  printf 'soundCategory_master:0.0\nsoundCategory_music:0.0\n' > "$game/options.txt"
fi
chown -R sacer:sacer "$root/target/test-game"
python3 "$root/tools/test-game/mute.py" "$game" &
cd "$game"
# Low CPU (agent-run's slice, as sacer) on the virtual display :12 (Xvfb),
# never on the user's screen.
exec /usr/local/bin/agent-run --user sacer env HORIZON_HOME="$game" HORIZON_LAUNCHER_HOME="$game/launcher" \
  XDG_DATA_HOME="$game/xdg-data" XDG_CONFIG_HOME="$game/xdg-config" \
  HORIZON_NO_AUDIO=1 HORIZON_LAUNCHER_NO_SELF_UPDATE=1 XDG_RUNTIME_DIR=/run/user/1000 \
  WAYLAND_DISPLAY= DISPLAY="${TEST_DISPLAY:-:12}" HORIZON_NO_UPDATE_CHECK=1 \
  "${HORIZON_CLIENT:-/home/sacer/Games/horizon/target/release/horizon-client}" \
  --resources "${HORIZON_RES:-/home/sacer/Games/horizon/.local/horizon-client/26.3}" \
  --control-socket "$game/control.sock" \
  --offscreen --render-distance 6 --width 960 --height 540 --name NtmTeste "$@" \
  > "$game/client.log" 2>&1
