import subprocess,json,time,os,sys
from pathlib import Path
prefix=['runuser','-u','sacer','--','env','XDG_RUNTIME_DIR=/run/user/1000','pactl']
game=Path(sys.argv[1]); logfile=Path(sys.argv[2])
for i in range(60):
 try:
  streams=json.loads(subprocess.check_output(prefix+['-f','json','list','sink-inputs'],text=True))
  for stream in streams:
   props=stream.get('properties',{})
   if props.get('application.process.binary')!='horizon-client' and props.get('application.name')!='PipeWire ALSA [horizon-client]':continue
   pid=props.get('application.process.id')
   if pid and Path(f'/proc/{pid}/cwd').resolve()==game:
    subprocess.run(prefix+['set-sink-input-mute',str(stream['index']),'1'],check=True)
    logfile.write_text(f"Muted test client PID {pid} stream {stream['index']}\n")
    raise SystemExit
 except (subprocess.SubprocessError,ValueError,OSError):pass
 time.sleep(.5)
logfile.write_text('No test client audio stream found in 30s; options master/music remain 0.0\n')
