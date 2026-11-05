#!/usr/bin/env python3
"""Compile/call pinned HBM methods with controlled host stubs, on the SSD.

No HBM source or downloaded jars are redistributed. Whole target classes are
copied byte-for-byte. Registry slices retain the original Java statements.
"""
import argparse
import hashlib
import json
import re
import shutil
import subprocess
from pathlib import Path

from stubs import STUBS

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
JAVA_ROOT = 'src/main/java/com/hbm/'
PIN = '4f5924e'
CLASSES = (
    'items/machine/ItemRBMKRod.java', 'items/machine/ItemRBMKPellet.java',
    'items/machine/ItemFELCrystal.java', 'items/special/ItemNuclearWaste.java',
    'items/special/ItemWasteLong.java', 'items/special/ItemWasteShort.java',
    'tileentity/machine/rbmk/RBMKDials.java',
    'tileentity/machine/rbmk/IRBMKFluxReceiver.java', 'util/GameRuleHelper.java',
    'util/Tuple.java', 'util/WeightedRandomObject.java',
    'inventory/recipes/SILEXRecipes.java', 'inventory/recipes/OutgasserRecipes.java',
)
REGISTRIES = ('items/ModItems.java', 'items/ItemEnums.java', 'inventory/OreDictManager.java',
              'inventory/material/MaterialShapes.java', 'inventory/fluid/Fluids.java', 'inventory/fluid/FluidType.java',
              'blocks/ModBlocks.java', 'main/CraftingManager.java',
              'crafting/RodRecipes.java', 'inventory/recipes/AssemblyMachineRecipes.java',
              'inventory/recipes/anvil/AnvilRecipes.java', 'config/GeneralConfig.java')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def clean(text):
    """Remove comments while preserving strings and source line positions."""
    pattern = r'"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'|//[^\n]*|/\*.*?\*/'
    return re.sub(pattern, lambda m: re.sub(r'[^\n]', ' ', m[0])
                  if m[0].startswith(('/',)) else m[0], text, flags=re.S)


def balanced(text, opening, left='{', right='}'):
    """Return the source body of a balanced block, without rewriting it."""
    masked = re.sub(r'"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'',
                    lambda m: ' ' * len(m[0]), clean(text))
    depth = 1
    for end in range(opening + 1, len(text)):
        depth += (masked[end] == left) - (masked[end] == right)
        if depth == 0:
            return text[opening + 1:end]
    raise ValueError('unbalanced source block')


def enum(text, name):
    match = re.search(r'public static enum ' + name + r'\s*\{', text)
    if not match:
        raise ValueError('enum not found: ' + name)
    return text[match.start():match.end()] + balanced(text, match.end() - 1) + '}'


def manifest(source):
    rev = subprocess.check_output(['git', '-C', str(source), 'rev-parse', 'HEAD'], text=True).strip()
    if not rev.startswith(PIN):
        raise ValueError('Java pin drift: ' + rev)
    paths = [JAVA_ROOT + p for p in (*CLASSES, *REGISTRIES)]
    for relative in paths:
        committed = subprocess.check_output(['git', '-C', str(source), 'rev-parse', rev + ':' + relative], text=True).strip()
        actual = subprocess.check_output(['git', '-C', str(source), 'hash-object', relative], text=True).strip()
        if actual != committed:
            raise ValueError('Java source differs from pin: ' + relative)
    return {'java_revision': rev,
            'sources_sha256': {p: sha(source / p) for p in paths},
            'harness_sha256': {p: sha(HERE / p) for p in ('run.py', 'stubs.py', 'java/Oracle.java')},
            'whole_classes': list(CLASSES), 'registry_slices': list(REGISTRIES),
            'external_jars': [], 'external_mods': False,
            'stub_contract': 'README.md; JSON overrides/render/entities not executed'}


def generated(source):
    """Generate identity stubs and verbatim registry slices from local source."""
    texts = {p: (source / JAVA_ROOT / p).read_text() for p in (*CLASSES, *REGISTRIES)}
    joined = '\n'.join(texts[p] for p in CLASSES)
    items = texts['items/ModItems.java']
    rods = re.findall(r'public static ItemRBMKRod (rbmk_fuel_\w+);', clean(items))
    pellets = re.findall(r'public static ItemRBMKPellet (rbmk_pellet_\w+);', clean(items))
    if (len(rods), len(pellets)) != (33, 32):
        raise ValueError('registry drift')
    names = set(re.findall(r'\bModItems\.(\w+)', joined)) | set(rods + pellets)
    names.add('rbmk_fuel_empty')
    fields = []
    for name in sorted(names):
        typ = 'ItemRBMKRod' if name in rods else 'ItemRBMKPellet' if name in pellets else 'Item'
        fields.append(f'public static {typ} {name}' +
                      (';' if name in rods + pellets else f'=new Item("{name}");'))
    statements = []
    stripped = clean(items)
    for name in pellets + rods:
        m = re.search(r'^\s*' + name + r'\s*=.*?;', stripped, re.M | re.S)
        if not m:
            raise ValueError('missing registry statement: ' + name)
        # Keep original comments too; the compiled statement is unchanged.
        statements.append(items[m.start():m.end()])
    tints = re.findall(r'int tint\w+ = 0x[0-9a-fA-F]+;', items)
    moditems = '''package com.hbm.items;
import net.minecraft.item.Item;import com.hbm.items.machine.*;
import com.hbm.items.machine.ItemRBMKRod.*;
import com.hbm.tileentity.machine.rbmk.IRBMKFluxReceiver.NType;
import com.hbm.lib.RefStrings;import com.hbm.main.MainRegistry;
import net.minecraft.util.EnumChatFormatting;
public class ModItems {\n''' + '\n'.join(fields) + '\nstatic {\n' + '\n'.join(tints + statements) + '\n}}\n'
    result = {'com/hbm/items/ModItems.java': moditems}
    enums = texts['items/ItemEnums.java']
    result['com/hbm/items/ItemEnums.java'] = ('package com.hbm.items; public class ItemEnums {' +
        enum(enums, 'EnumAshType') + enum(enums, 'EnumTarType') + '}')
    # Identity-only block/vanilla fields: registrations are not block simulation.
    for cls, pkg, typ in [('ModBlocks', 'com/hbm/blocks', 'Block'),
                          ('Blocks', 'net/minecraft/init', 'Block'),
                          ('Items', 'net/minecraft/init', 'Item')]:
        used = sorted(set(re.findall(r'(?<![\w.])' + cls + r'\.(\w+)', joined)))
        fields = '\n'.join(f'public static final {typ} {n}=new {typ}("' +
                           ('minecraft_1_7_10:' if pkg.startswith('net/') else '') + n + '");' for n in used)
        result[pkg + '/' + cls + '.java'] = ('package ' + pkg.replace('/', '.') +
            ';import net.minecraft.block.Block;import net.minecraft.item.Item;public class ' + cls + '{' + fields + '}')
    # FluidType calls registerSelf: IDs follow ALL declarations, not merely
    # the subset referenced by RBMK. Verify forced IDs from that same source.
    fluid_names = sorted(set(re.findall(r'Fluids\.(\w+)', joined)))
    fluid_src = texts['inventory/fluid/Fluids.java']
    declarations = list(re.finditer(r'^\s*([A-Z0-9_]+)\s*=\s*new FluidType\((?:(\d+),\s*)?"([A-Z0-9_]+)"', clean(fluid_src), re.M))
    ids = {}
    for fid, declaration in enumerate(declarations):
        if declaration[2] and int(declaration[2]) != fid:
            raise ValueError('forced fluid ID drift: ' + declaration[1])
        ids[declaration[1]] = fid
    fields = []
    for name in fluid_names:
        if name not in ids:
            raise ValueError('fluid id declaration not found: ' + name)
        fields.append(f'public static final FluidType {name}=new FluidType("{name}",{ids[name]});')
    result['com/hbm/inventory/fluid/Fluids.java'] = ('package com.hbm.inventory.fluid; public class Fluids {' + '\n'.join(fields) + '}')
    # Keep actual DictFrame helper bodies, named material declarations and shape
    # prefixes. These slices do not implement any formula under test.
    odm = texts['inventory/OreDictManager.java']
    shape_src = texts['inventory/material/MaterialShapes.java']
    shape_names = ('NUGGET', 'INGOT', 'DUSTTINY', 'DUST', 'GEM', 'BILLET', 'BLOCK')
    shape_fields = []
    for name in shape_names:
        m = re.search(r'\b' + name + r'\s*=\s*new MaterialShapes\([^;]*?"([^"]+)"', shape_src)
        if not m:
            raise ValueError('shape prefix not found: ' + name)
        shape_fields.append(f'private static final Shape {name}=new Shape("{m[1]}");')
    method_names = ('nugget', 'ingot', 'dustTiny', 'dust', 'gem', 'billet', 'block')
    method_bodies = []
    for name in method_names:
        m = re.search(r'public String ' + name + r'\(\)\s*\{', odm)
        method_bodies.append(odm[m.start():m.end()] + balanced(odm, m.end() - 1) + '}')
    for m in re.finditer(r'public static ItemStack fromOne\([^)]*\)\s*\{', odm):
        method_bodies.append(odm[m.start():m.end()] + balanced(odm, m.end() - 1) + '}')
    constants = sorted(set(re.findall(r'\b([A-Z][A-Z0-9_]*)\.(?:' + '|'.join(method_names) + r')\(', joined)))
    const_fields = []
    for name in constants:
        m = re.search(r'\b' + name + r'\s*=\s*new DictFrame\(([^;]*?)\)', odm)
        if not m:
            raise ValueError('material declaration not found: ' + name)
        args = m[1]
        if name == 'U':
            # The supported profile has no GregTech 6, as approved in D7.
            args = '"Uranium"'
        const_fields.append(f'public static final DictFrame {name}=new DictFrame({args});')
    result['com/hbm/inventory/OreDictManager.java'] = '''package com.hbm.inventory;
import net.minecraft.item.*;import net.minecraft.block.Block;
public class OreDictManager {
static class Shape {String n;Shape(String n){this.n=n;}String name(){return n;}}
''' + '\n'.join(shape_fields + const_fields) + '''
public static class DictFrame {public String[] mats;public DictFrame(String... m){mats=m;}
''' + '\n'.join(method_bodies) + '}}'
    return result


def execute(source, scratch):
    scratch.mkdir(parents=True, exist_ok=True)
    src = scratch / 'src'
    classes = scratch / 'classes'
    for folder in (src, classes):
        if folder.exists():
            shutil.rmtree(folder)
        folder.mkdir()
    for name in CLASSES:
        dest = src / 'com/hbm' / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source / JAVA_ROOT / name, dest)
    for name, text in {**STUBS, **generated(source)}.items():
        dest = src / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text(text)
    shutil.copyfile(HERE / 'java/Oracle.java', src / 'Oracle.java')
    files = sorted(str(p) for p in src.rglob('*.java'))
    command = ['javac', '--release', '8', '-encoding', 'UTF-8', '-d', str(classes), *files]
    compiled = subprocess.run(command, capture_output=True, text=True)
    (scratch / 'javac.log').write_text(compiled.stdout + compiled.stderr)
    if compiled.returncode:
        raise RuntimeError('javac failed; see ' + str(scratch / 'javac.log') + '\n' + compiled.stderr)
    result = subprocess.run(['java', '-cp', str(classes), 'Oracle'], check=True, capture_output=True, text=True)
    (scratch / 'java.stderr.log').write_text(result.stderr)
    data = json.loads(result.stdout)
    data['provenance'] = manifest(source)
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, default=ROOT.parent / 'ntm-src')
    parser.add_argument('--scratch', type=Path, required=True)
    parser.add_argument('--output', type=Path, default=HERE / 'fixtures/java.json')
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    data = execute(args.source.resolve(), args.scratch.resolve())
    encoded = json.dumps(data, ensure_ascii=False, indent=2, allow_nan=False) + '\n'
    if args.check:
        if args.output.read_text() != encoded:
            raise SystemExit('Java fixture differs: regenerate and inspect ' + str(args.output))
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded)
    report = {'ok': True, 'fixture_sha256': hashlib.sha256(encoded.encode()).hexdigest(),
              'java': subprocess.run(['java', '-version'], capture_output=True, text=True).stderr.strip(),
              'javac': subprocess.check_output(['javac', '-version'], text=True).strip(),
              'external_jars': [], 'scratch': str(args.scratch.resolve())}
    (args.scratch / 'execution.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
