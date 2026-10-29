#!/bin/bash
# Acceptance of the patched Horizon: mods INSTALLED (horizon-mods/ + horizon-mods.txt),
# muted, on Xvfb :11, low CPU (agent-run). Game folder: /tmp/ntm-accept/home.
set -euo pipefail
game=/tmp/ntm-accept/home
build=/home/sacer/Workspace/horizon-patched/target/release
mkdir -p "$game/horizon-mods/.horizon-mod-data/recipes"
[ -f "$game/options.txt" ] || printf 'soundCategory_master:0.0\nsoundCategory_music:0.0\n' > "$game/options.txt"
if [ ! -f "$game/horizon-mods.txt" ]; then
  printf 'mod.ntm:true\nmod.ntmlogic:true\nmod.recipes:true\n' > "$game/horizon-mods.txt"
  cp /home/sacer/Workspace/ntm-port/target/rad-dist/{ntm,ntmlogic-client,ntmlogic-server}.hmod "$game/horizon-mods/"
  cp /home/sacer/Workspace/horizon-patched/target/horizon-mod/recipes.hmod "$game/horizon-mods/recipes.hmod"
  printf 'true' > "$game/horizon-mods/.horizon-mod-data/recipes/give"
fi
chown -R sacer:sacer /tmp/ntm-accept
python3 /home/sacer/Workspace/ntm-port/tools/test-game/mute.py "$game" &
cd "$game"
exec /usr/local/bin/agent-run --user sacer env HORIZON_HOME="$game" HORIZON_LAUNCHER_HOME="$game/launcher" \
  XDG_DATA_HOME="$game/xdg-data" XDG_CONFIG_HOME="$game/xdg-config" \
  HORIZON_NO_AUDIO=1 HORIZON_LAUNCHER_NO_SELF_UPDATE=1 XDG_RUNTIME_DIR=/run/user/1000 \
  WAYLAND_DISPLAY= DISPLAY=:11 HORIZON_NO_UPDATE_CHECK=1 \
  "$build/horizon-client" \
  --resources /home/sacer/Games/horizon/.local/horizon-client/26.3 \
  --control-socket "$game/control.sock" \
  --offscreen --render-distance 6 --width 960 --height 540 --name NtmTeste "$@" \
  > "$game/client.log" 2>&1
