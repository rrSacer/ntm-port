#!/bin/bash
# horizon-ctl on the test game <name>: ctl.sh <name> <command...>
# HORIZON_CTL picks another build's horizon-ctl (a patched Horizon).
root="$(cd "$(dirname "$0")/../.." && pwd)"
name="$1"; shift
exec "${HORIZON_CTL:-/home/sacer/Games/horizon/target/release/horizon-ctl}" --socket "$root/target/test-game/$name/control.sock" "$@"
