#!/usr/bin/env python3
import json,socket,sys
from pathlib import Path
base=Path(__file__).resolve().parents[2]/'content-gen-evidence'
run=Path((base/'run-path.txt').read_text().strip())
def ctl(command):
 with socket.socket(socket.AF_UNIX) as s:
  s.settimeout(45); s.connect(str(run/'control.sock'));s.sendall((command+'\n').encode());line=s.makefile().readline().strip()
 with (base/'control.jsonl').open('a') as f:f.write(json.dumps({'command':command,'reply':line})+'\n')
 try:return json.loads(line)
 except ValueError:return line
if __name__=='__main__':print(json.dumps(ctl(' '.join(sys.argv[1:]))))
