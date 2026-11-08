#!/usr/bin/env python3
"""T04 oracle: compile the pinned RBMK tile entities and neutron handler whole,
with controlled host stubs (stubs_flux.py), and run the scenarios in
java/FluxOracle.java. Reuses the T00 harness (run.py / stubs.py) for the item,
dial and registry slices."""
import argparse, hashlib, json, shutil, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import run  # noqa: E402
from stubs import STUBS  # noqa: E402
from stubs_flux import FLUX_STUBS, REMOVE  # noqa: E402

TILE = 'tileentity/machine/rbmk/'
EXTRA = tuple(TILE + n + '.java' for n in (
    'TileEntityRBMKBase', 'TileEntityRBMKRod', 'TileEntityRBMKRodReaSim', 'TileEntityRBMKSlottedBase',
    'TileEntityRBMKControl', 'TileEntityRBMKControlManual', 'TileEntityRBMKControlAuto',
    'TileEntityRBMKAbsorber', 'TileEntityRBMKModerator', 'TileEntityRBMKReflector', 'TileEntityRBMKBlank',
    'TileEntityRBMKActiveBase', 'TileEntityRBMKOutgasser', 'IRBMKLoadable')) + (
    'handler/neutron/RBMKNeutronHandler.java', 'handler/neutron/NeutronStream.java',
    'handler/neutron/NeutronNode.java', 'handler/neutron/NeutronNodeWorld.java',
    'handler/neutron/NeutronHandler.java',
    'util/fauxpointtwelve/BlockPos.java', 'util/fauxpointtwelve/Rotation.java', 'util/Vec3NT.java')


def execute(source, scratch):
    run.CLASSES = (*run.CLASSES, *EXTRA)
    scratch.mkdir(parents=True, exist_ok=True)
    src, classes = scratch / 'src', scratch / 'classes'
    for folder in (src, classes):
        if folder.exists():
            shutil.rmtree(folder)
        folder.mkdir()
    for name in run.CLASSES:
        dest = src / 'com/hbm' / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source / run.JAVA_ROOT / name, dest)
    stubs = {k: v for k, v in STUBS.items() if k not in REMOVE}
    for name, text in {**stubs, **run.generated(source), **FLUX_STUBS}.items():
        dest = src / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text(text)
    shutil.copyfile(HERE / 'java/FluxOracle.java', src / 'FluxOracle.java')
    files = sorted(str(p) for p in src.rglob('*.java'))
    compiled = subprocess.run(['javac', '--release', '8', '-encoding', 'UTF-8', '-Xmaxerrs', '1000', '-d', str(classes), *files],
                              capture_output=True, text=True)
    (scratch / 'javac.log').write_text(compiled.stdout + compiled.stderr)
    if compiled.returncode:
        raise RuntimeError('javac failed; see ' + str(scratch / 'javac.log'))
    result = subprocess.run(['java', '-cp', str(classes), 'FluxOracle'], check=True, capture_output=True, text=True)
    (scratch / 'java.stderr.log').write_text(result.stderr)
    data = json.loads(result.stdout)
    data['provenance'] = {**run.manifest(source),
                          'harness_sha256': {p: run.sha(HERE / p) for p in ('run.py', 'stubs.py', 'run_flux.py', 'stubs_flux.py', 'java/FluxOracle.java')},
                          'flux_whole_classes': list(EXTRA)}
    return data


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--source', type=Path, default=run.ROOT.parent / 'ntm-src')
    p.add_argument('--scratch', type=Path, required=True)
    p.add_argument('--output', type=Path, default=HERE / 'fixtures/flux.json')
    p.add_argument('--check', action='store_true')
    a = p.parse_args()
    data = execute(a.source.resolve(), a.scratch.resolve())
    enc = json.dumps(data, ensure_ascii=False, indent=1, allow_nan=False) + '\n'
    if a.check:
        if a.output.read_text() != enc:
            raise SystemExit('flux fixture differs: regenerate and inspect')
    else:
        a.output.write_text(enc)
    print(json.dumps({'ok': True, 'sha256': hashlib.sha256(enc.encode()).hexdigest()}))


if __name__ == '__main__':
    main()
