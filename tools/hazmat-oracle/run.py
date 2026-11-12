#!/usr/bin/env python3
"""Executable Java oracle for the player's radiation resistance (Onda 1, fatia 1).

The Java bodies of `HazmatRegistry.getResistance(EntityPlayer)`, `getResistance(ItemStack)`, `getCladding`,
`ContaminationUtil.calculateRadiationMod` and the two number lines of `printGeigerData` are cut **verbatim** from the
HBM source and compiled with minimal stubs (`ItemStack`, `NBTTagCompound.getFloat`, `ArmorModHandler`, `EntityPlayer`);
nothing of the arithmetic is translated. The registry is filled from `crates/ntm-core/data/hazmat.json` (whose values
`tools/content-gen/test_hazmat.py` checks against `initDefault`). The cases are mixed armor sets in `armorInventory`
order (boots first), with `hfr_cladding` tags of every numeric type, cladding mods, non-cladding mods and Rad-X, plus
exponents for the `(float) Math.pow(10.0F, -res)` alone, chosen where a `powf` in `float` would differ.

Writes `tools/hazmat-oracle/fixture.json`; `--check` compares (exit 1 when it differs). Needs `java`/`javac`.
  python3 tools/hazmat-oracle/run.py --scratch DIR [--check]
"""
import argparse
import ctypes
import json
import os
import random
import re
import shutil
import struct
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
JAVA = Path(os.environ.get('NTM_SRC', ROOT.parent / 'ntm-src')) / 'src/main/java/com/hbm'
FIXTURE = ROOT / 'tools/hazmat-oracle/fixture.json'
TABLE = json.loads((ROOT / 'crates/ntm-core/data/hazmat.json').read_text())


def method(text, signature):
    """The whole method starting at `signature`, braces matched."""
    start = text.index(signature)
    depth, i = 0, text.index('{', start)
    while True:
        depth += {'{': 1, '}': -1}.get(text[i], 0)
        i += 1
        if depth == 0:
            return text[start:i]


def java_source():
    registry = (JAVA / 'handler/HazmatRegistry.java').read_text()
    contamination = (JAVA / 'util/ContaminationUtil.java').read_text()
    bodies = '\n'.join(method(registry, s) for s in (
        'public static double getResistance(ItemStack stack)',
        'public static double getCladding(ItemStack stack)',
        'public static float getResistance(EntityPlayer player)'))
    mod = method(contamination, 'public static float calculateRadiationMod(EntityLivingBase entity)')
    geiger = [l.strip() for l in contamination.splitlines() if l.strip().startswith(('double res = ', 'double resKoeff = '))]
    if len(geiger) != 2:
        raise SystemExit('printGeigerData drifted')
    return '''import java.util.*;
class Item { final String id; Item(String id) { this.id = id; } }
class ItemModCladding extends Item { final double rad; ItemModCladding(String id, double rad) { super(id); this.rad = rad; } }
class NBTTagCompound {
    final Map<String, Number> values = new HashMap<>();
    // NBTTagCompound.getFloat: any numeric tag (hasKey(key, 99)), converted to float; 0 otherwise.
    float getFloat(String key) { Number n = values.get(key); return n == null ? 0F : n.floatValue(); }
}
class ItemStack {
    final Item item; NBTTagCompound stackTagCompound; ItemStack[] mods;
    ItemStack(Item item) { this.item = item; }
    Item getItem() { return item; }
    boolean hasTagCompound() { return stackTagCompound != null; }
}
class ArmorModHandler {
    static final int cladding = 5;
    static boolean hasMods(ItemStack stack) { return stack.mods != null; }
    static ItemStack[] pryMods(ItemStack stack) { return stack.mods; }
}
class HbmPotion { static final Object radx = new Object(); }
class ShadyUtil { static final String Pu_238 = "not a player of the oracle"; }
class EntityLivingBase {}
class InventoryPlayer { final ItemStack[] armorInventory = new ItemStack[4]; }
class EntityPlayer extends EntityLivingBase {
    final InventoryPlayer inventory = new InventoryPlayer(); boolean radx;
    UUID getUniqueID() { return new UUID(0, 0); }
    boolean isPotionActive(Object potion) { return potion == HbmPotion.radx && radx; }
}
class HazmatRegistry {
    static final HashMap<Item, Double> entries = new HashMap<>();
''' + bodies + '''
}
class ContaminationUtil {
''' + mod + '''
}
public class Oracle {
    static final Map<String, Item> items = new HashMap<>();
    static Item item(String id, double cladding) {
        return items.computeIfAbsent(id, k -> cladding >= 0 ? new ItemModCladding(k, cladding) : new Item(k));
    }
    public static void main(String[] args) throws Exception {
        Scanner in = new Scanner(System.in);
        int n = in.nextInt();
        for (int i = 0; i < n; i++) { String id = in.next(); double v = Double.longBitsToDouble(in.nextLong()); HazmatRegistry.entries.put(item(id, -1), v); }
        int c = in.nextInt();
        for (int i = 0; i < c; i++) { String id = in.next(); double v = Double.longBitsToDouble(in.nextLong()); item(id, v); }
        int cases = in.nextInt();
        for (int k = 0; k < cases; k++) {
            EntityPlayer player = new EntityPlayer();
            player.radx = in.nextInt() == 1;
            for (int s = 0; s < 4; s++) {
                String id = in.next();
                if (id.equals("-")) { in.next(); in.next(); in.next(); continue; }
                ItemStack stack = new ItemStack(item(id, -1));
                String tag = in.next(); String raw = in.next();
                if (!tag.equals("-")) {
                    stack.stackTagCompound = new NBTTagCompound();
                    Number value;
                    switch (tag) {
                        case "byte": value = Byte.parseByte(raw); break;
                        case "short": value = Short.parseShort(raw); break;
                        case "int": value = Integer.parseInt(raw); break;
                        case "long": value = Long.parseLong(raw); break;
                        case "float": value = Float.intBitsToFloat(Integer.parseInt(raw)); break;
                        default: value = Double.longBitsToDouble(Long.parseLong(raw));
                    }
                    stack.stackTagCompound.values.put("hfr_cladding", value);
                }
                String mod = in.next();
                if (!mod.equals("-")) { stack.mods = new ItemStack[9]; stack.mods[ArmorModHandler.cladding] = new ItemStack(items.containsKey(mod) ? items.get(mod) : item(mod, -1)); }
                player.inventory.armorInventory[s] = stack;
            }
            float r = HazmatRegistry.getResistance(player);
            float f = ContaminationUtil.calculateRadiationMod(player);
            ''' + geiger[0] + '''
            ''' + geiger[1] + '''
            System.out.println(Float.floatToIntBits(r) + " " + Float.floatToIntBits(f) + " " + res + " " + resKoeff);
        }
        int e = in.nextInt();
        for (int k = 0; k < e; k++) {
            float res = Float.intBitsToFloat(in.nextInt());
            float koeff = 10.0F;
            System.out.println(Float.floatToIntBits((float) Math.pow(koeff, -res)));
        }
    }
}
'''


def f32(x):
    return struct.unpack('f', struct.pack('f', x))[0]


def bits32(x):
    return struct.unpack('<i', struct.pack('<f', x))[0]


def cases():
    rnd = random.Random(1)
    armor = sorted(TABLE['resistance'])
    claddings = sorted(TABLE['cladding'])
    out = []
    # The reviewer's mixed set, armorInventory order: boots, legs, chest, helmet.
    out.append({'radx': False, 'slots': [
        {'id': 'ntm:hazmat_boots_red'}, {'id': 'ntm:cobalt_legs'}, {'id': 'ntm:steel_plate'}, {'id': 'minecraft:iron_helmet'}]})
    out.append({'radx': False, 'slots': [
        {'id': 'ntm:hazmat_boots_grey'}, {'id': 'ntm:cobalt_legs'}, {'id': 'ntm:steel_plate'},
        {'id': 'minecraft:iron_helmet', 'mod': 'ntm:cladding_desh'}]})
    out.append({'radx': False, 'slots': [{'id': 'ntm:steel_boots'}, {'id': 'ntm:steel_legs'}, {'id': 'ntm:steel_plate'}, {'id': 'ntm:steel_helmet'}]})
    for _ in range(1500):
        slots = []
        for _ in range(4):
            if rnd.random() < 0.15:
                slots.append(None)
                continue
            s = {'id': rnd.choice(armor) if rnd.random() < 0.9 else 'minecraft:diamond_helmet'}
            roll = rnd.random()
            if roll < 0.15:
                kind = rnd.choice(['byte', 'short', 'int', 'long', 'float', 'double'])
                value = {'byte': rnd.randint(-1, 2), 'short': rnd.randint(-1, 2), 'int': rnd.randint(-1, 2),
                         'long': rnd.randint(-1, 2), 'float': f32(rnd.uniform(-0.2, 0.8)), 'double': rnd.uniform(-0.2, 0.8)}[kind]
                s['hfr'] = {'type': kind, 'value': value}
            if rnd.random() < 0.25:
                s['mod'] = rnd.choice(claddings + ['ntm:attachment_mask'])
            slots.append(s)
        out.append({'radx': rnd.random() < 0.3, 'slots': slots})
    return out


def exponents():
    """`(float) Math.pow(10.0F, -res)` alone: random exponents, and those where a `powf` in `float` disagrees."""
    libm = ctypes.CDLL('libm.so.6')
    libm.powf.restype, libm.powf.argtypes = ctypes.c_float, [ctypes.c_float, ctypes.c_float]
    rnd = random.Random(2)
    chosen, apart = [], []
    while len(apart) < 150:
        x = f32(rnd.uniform(0, 12))
        if libm.powf(10.0, -x) != f32(10 ** -x):
            apart.append(x)
        elif len(chosen) < 350:
            chosen.append(x)
    return chosen + apart


def feed(case_list, exps):
    lines = [str(len(TABLE['resistance']))]
    lines += [f'{k} {struct.unpack("<q", struct.pack("<d", v["value"]))[0]}' for k, v in sorted(TABLE['resistance'].items())]
    lines.append(str(len(TABLE['cladding'])))
    lines += [f'{k} {struct.unpack("<q", struct.pack("<d", v["value"]))[0]}' for k, v in sorted(TABLE['cladding'].items())]
    lines.append(str(len(case_list)))
    for c in case_list:
        row = ['1' if c['radx'] else '0']
        for s in c['slots']:
            if s is None:
                row += ['-', '-', '-', '-']
                continue
            hfr = s.get('hfr')
            if hfr is None:
                tag, raw = '-', '-'
            elif hfr['type'] == 'float':
                tag, raw = 'float', str(bits32(hfr['value']))
            elif hfr['type'] == 'double':
                tag, raw = 'double', str(struct.unpack('<q', struct.pack('<d', hfr['value']))[0])
            else:
                tag, raw = hfr['type'], str(hfr['value'])
            row += [s['id'], tag, raw, s.get('mod', '-')]
        lines.append(' '.join(row))
    lines.append(str(len(exps)))
    lines += [str(bits32(x)) for x in exps]
    return '\n'.join(lines) + '\n'


def run(scratch):
    scratch = Path(scratch)
    if scratch.exists():
        shutil.rmtree(scratch)
    (scratch / 'classes').mkdir(parents=True)
    src = scratch / 'Oracle.java'
    src.write_text(java_source())
    subprocess.run(['javac', '--release', '8', '-nowarn', '-d', str(scratch / 'classes'), str(src)], check=True,
                   stderr=subprocess.DEVNULL)
    case_list, exps = cases(), exponents()
    out = subprocess.run(['java', '-cp', str(scratch / 'classes'), 'Oracle'], input=feed(case_list, exps), text=True,
                         capture_output=True, check=True).stdout.splitlines()
    results = out[:len(case_list)]
    for c, line in zip(case_list, results):
        res_bits, mod_bits, res, koeff = line.split()
        c['java'] = {'resistance_bits': int(res_bits), 'factor_bits': int(mod_bits), 'res': res, 'koeff': koeff}
    pows = [{'res_bits': bits32(x), 'factor_bits': int(v)} for x, v in zip(exps, out[len(case_list):])]
    return {'about': 'tools/hazmat-oracle/run.py: Java bodies of HazmatRegistry.getResistance/getCladding, '
                     'ContaminationUtil.calculateRadiationMod and printGeigerData, armorInventory order (boots first)',
            'cases': case_list, 'pow': pows}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scratch', required=True)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    data = json.dumps(run(args.scratch), separators=(',', ':')) + '\n'
    if args.check:
        if FIXTURE.read_text() != data:
            print(f'{FIXTURE} differs from the oracle', file=sys.stderr)
            return 1
        return 0
    FIXTURE.write_text(data)
    return 0


if __name__ == '__main__':
    sys.exit(main())
