"""Mutes the PipeWire stream of the test client running in argv[1]."""
import json, os, subprocess, sys, time
from pathlib import Path

game = Path(sys.argv[1]).resolve()
pactl = (['runuser', '-u', 'sacer', '--'] if os.geteuid() == 0 else []) + ['env', 'XDG_RUNTIME_DIR=/run/user/1000', 'pactl']
log = game / 'audio-muted.log'
for _ in range(120):
    try:
        streams = json.loads(subprocess.check_output(pactl + ['-f', 'json', 'list', 'sink-inputs'], text=True))
        for s in streams:
            props = s.get('properties', {})
            if props.get('application.process.binary') != 'horizon-client' and \
               props.get('application.name') != 'PipeWire ALSA [horizon-client]':
                continue
            pid = props.get('application.process.id')
            if pid and Path(f'/proc/{pid}/cwd').exists() and \
               Path(f'/proc/{pid}/environ').read_bytes().find(str(game).encode()) < 0:
                continue
            subprocess.run(pactl + ['set-sink-input-mute', str(s['index']), '1'], check=True)
            log.write_text(f"muted stream {s['index']} pid {pid}\n")
            sys.exit(0)
    except (subprocess.SubprocessError, ValueError, OSError):
        pass
    time.sleep(0.5)
log.write_text('no client stream found in 60 s; master and music stay at 0.0\n')
