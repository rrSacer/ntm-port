#!/usr/bin/env python3
import json,socket,sys
from pathlib import Path
root=Path(__file__).resolve().parents[2]
scratch=Path((root/'recipe-gen-evidence/run-path.txt').read_text().strip())
s=socket.socket(socket.AF_UNIX);s.settimeout(30);s.connect(str(scratch/'control.sock'))
cmd=' '.join(sys.argv[1:]);s.sendall((cmd+'\n').encode());reply=s.makefile().readline().strip();print(reply)
with (root/'recipe-gen-evidence/control.jsonl').open('a') as f: f.write(json.dumps({'command':cmd,'reply':json.loads(reply)})+'\n')
