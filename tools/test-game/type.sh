#!/bin/bash
# Types a chat command for real (t, text, enter): horizon-ctl command skips
# client mods in single player. type.sh <name> /cmd args
d="$(dirname "$0")"; name="$1"; shift
"$d/ctl.sh" "$name" key t tap >/dev/null; sleep 0.15
"$d/ctl.sh" "$name" type "$*" >/dev/null; sleep 0.1
"$d/ctl.sh" "$name" key enter tap
