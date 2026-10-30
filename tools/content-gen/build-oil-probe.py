#!/usr/bin/env python3
"""Build a disposable content package with the measured oil shape candidates."""
import argparse
import json
import shutil
import subprocess
import tempfile
from pathlib import Path
from oil_worldgen import geode,ore,ore_patch,generate,protect_geode

ROOT=Path(__file__).resolve().parents[2]


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--packer',type=Path,required=True)
    parser.add_argument('--resources',type=Path,required=True)
    parser.add_argument('--content',type=Path,default=ROOT/'content')
    args=parser.parse_args()
    scratch=ROOT.parent/'.agent-tmp';scratch.mkdir(exist_ok=True)
    out=Path(tempfile.mkdtemp(prefix='content-oil-candidates-',dir=scratch))
    shutil.copytree(args.content,out,dirs_exist_ok=True,ignore=shutil.ignore_patterns('target'))
    def emit(path,value):
        p=out/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(value,indent=2)+'\n')
    rows=json.loads((args.content/'ids.json').read_text())['entries']
    generate(emit,rows)
    protect_geode(emit,rows,args.resources/'registries.json')
    for value in [2,3,4]:emit(f'data/ntm/worldgen/configured_feature/oil_test_geode{value}.json',geode(value))
    emit('data/ntm/worldgen/configured_feature/oil_test_ore64.json',ore())
    emit('data/ntm/worldgen/configured_feature/oil_test_geode35.json',geode(3.5))
    for count in [2,3]:emit(f'data/ntm/worldgen/configured_feature/oil_test_patch{count}.json',ore_patch(count))
    subprocess.run([str(args.packer.resolve()),'build'],cwd=out,check=True)
    package=out/'target/horizon-mod/ntm.hmod'
    subprocess.run([str(args.packer.resolve()),'check',str(package)],cwd=out,check=True)
    print(package,flush=True)


if __name__=='__main__':main()
