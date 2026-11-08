#!/usr/bin/env python3
"""Watz / PWR / Hephaestus / Flare oracle.

Methods are cut verbatim out of the pinned HBM tile entities (brace matching, no rewriting),
dropped into small host classes whose only job is to hold the fields and no-op the world
plumbing, and driven by Driver.java. Function.java and EnumUtil.java are copied whole; the
pellet/fuel enums, the Fluids trait statements and BobMathUtil.squirt are copied verbatim.
"""
import argparse, hashlib, json, re, shutil, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
J = 'src/main/java/com/hbm/'
PIN = '4f5924e'
WHOLE = ('util/function/Function.java', 'util/EnumUtil.java')
SLICED = ('tileentity/machine/TileEntityWatz.java', 'tileentity/machine/TileEntityPWRController.java',
          'tileentity/machine/TileEntityMachineHephaestus.java', 'tileentity/machine/oil/TileEntityMachineGasFlare.java',
          'items/machine/ItemWatzPellet.java', 'items/machine/ItemPWRFuel.java', 'blocks/machine/MachinePWRController.java',
          'tileentity/TileEntityMachineBase.java', 'util/BobMathUtil.java', 'inventory/fluid/Fluids.java')


def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()


def mask(text):
    pat = r'"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'|//[^\n]*|/\*.*?\*/'
    return re.sub(pat, lambda m: ' ' * len(m[0]) if m[0][0] != '"' and m[0][0] != "'" or True else m[0], text, flags=re.S)


def block_end(text, open_idx):
    m = mask(text)
    depth = 0
    for i in range(open_idx, len(text)):
        if m[i] == '{': depth += 1
        elif m[i] == '}':
            depth -= 1
            if depth == 0: return i
    raise ValueError('unbalanced')


def cut(text, header):
    """Verbatim source of the member whose header starts with the literal `header`."""
    i = text.index(header)
    if text.count(header) != 1: raise ValueError('ambiguous ' + header)
    start = text.rfind('\n', 0, i) + 1
    o = text.index('{', i)
    return text[start:block_end(text, o) + 1] + '\n'


def lines(text, first, last):
    return ''.join(text.splitlines(True)[first - 1:last])


def manifest(src):
    rev = subprocess.check_output(['git', '-C', str(src), 'rev-parse', 'HEAD'], text=True).strip()
    if not rev.startswith(PIN): raise ValueError('Java pin drift: ' + rev)
    for rel in WHOLE + SLICED:
        p = J + rel
        a = subprocess.check_output(['git', '-C', str(src), 'hash-object', p], text=True).strip()
        b = subprocess.check_output(['git', '-C', str(src), 'rev-parse', rev + ':' + p], text=True).strip()
        if a != b: raise ValueError('Java source differs from pin: ' + p)
    return {'java_revision': rev, 'whole_classes': list(WHOLE), 'sliced_sources': list(SLICED),
            'sources_sha256': {J + r: sha(src / J / r) for r in WHOLE + SLICED},
            'harness_sha256': {p: sha(HERE / p) for p in ('run.py', 'java/Stubs.java', 'java/Driver.java')}}


def gen(src, out):
    rd = lambda r: (src / J / r).read_text()
    for d in ('classes',): (out / d).mkdir(parents=True, exist_ok=True)
    s = out / 'src'
    if s.exists(): shutil.rmtree(s)
    (s / 'com/hbm/util/function').mkdir(parents=True)
    (s / 'net/minecraft/util').mkdir(parents=True)
    for r in WHOLE:
        shutil.copyfile(src / J / r, s / 'com/hbm' / r)
        assert (s / 'com/hbm' / r).read_bytes() == (src / J / r).read_bytes()
    bob = rd('util/BobMathUtil.java')
    (s / 'com/hbm/util/BobMathUtil.java').write_text('package com.hbm.util;\npublic class BobMathUtil {\n' + cut(bob, 'public static double squirt(double x)') + '}\n')
    (s / 'net/minecraft/util/EnumChatFormatting.java').write_text('package net.minecraft.util;\npublic class EnumChatFormatting {\n\tpublic static final String YELLOW = "", RED = "", DARK_GREEN = "";\n}\n')
    shutil.copyfile(HERE / 'java/Stubs.java', s / 'Stubs.java')
    shutil.copyfile(HERE / 'java/Driver.java', s / 'Driver.java')

    # pellet / fuel enums, verbatim, with the item statics they need
    pel = rd('items/machine/ItemWatzPellet.java')
    enum_w = cut(pel, 'public static enum EnumWatzType')
    statics = ''.join(cut(pel, h) for h in ('public static double getEnrichment(', 'public static double getYield(', 'public static void setYield(',
                                            'public static void setDouble(', 'public static double getDouble(', 'private static void setNBTDefaults('))
    imp = 'import com.hbm.util.function.Function;\nimport com.hbm.util.function.Function.*;\nimport com.hbm.util.EnumUtil;\n'
    (s / 'WatzTypes.java').write_text(imp + 'interface WatzTypes {\n' + enum_w + '}\n')
    (s / 'ItemWatzPellet.java').write_text(imp + 'class ItemWatzPellet implements WatzTypes {\n' + statics + '}\n')
    fu = rd('items/machine/ItemPWRFuel.java')
    (s / 'PwrTypes.java').write_text(imp + 'interface PwrTypes {\n' + cut(fu, 'public static enum EnumPWRFuel') + '}\n')

    # fluids: every heating/moderator statement of Fluids.java, verbatim, in file order
    fl = rd('inventory/fluid/Fluids.java')
    stm = []
    for m in re.finditer(r'^\t\t(?:double eff_steam_(?:boil|heatex) = [^\n]*|[A-Z_]+\.addTraits\(new FT_Heatable\(\)[^\n]*)\n(?:\t+\.addStep[^\n]*\n)*', fl, flags=re.M):
        stm.append(m.group(0))
    code = ''.join(stm)
    names = set(re.findall(r'\b[A-Z][A-Z0-9_]+\b', code)) - {'BOILER', 'HEATEXCHANGER', 'PWR', 'ICF', 'PA'}
    names |= {'NONE', 'GAS', 'OIL', 'HOTOIL', 'COOLANT', 'COOLANT_HOT', 'WATZ'}
    decl = ''.join('\tstatic FluidType %s = new FluidType("%s");\n' % (n.lower(), n.lower()) for n in sorted(names)).replace('static FluidType ', 'static FluidType ')
    decl = ''.join('\tstatic FluidType %s = new FluidType("%s");\n' % (n, n.lower()) for n in sorted(names))
    (s / 'Fluids.java').write_text('class Fluids {\n' + decl + '\tstatic {\n' + code + '\t}\n}\n')

    # Watz
    w = rd('tileentity/machine/TileEntityWatz.java')
    body = (lines(w, *span(w, 'public FluidTank[] tanks;', 'public ItemStack[] locks;')) + cut(w, 'public TileEntityWatz()') + cut(w, 'protected void resetSharedTanks()')
            + 'protected PortDef[] cachedPorts;\n' + cut(w, 'public PortDef[] getPorts()') + cut(w, 'public void updateEntity()') + cut(w, 'public void setupCoolant()')
            + cut(w, 'public void updateCoolant(FluidTank[] tanks)') + cut(w, 'public void updateReaction(') + cut(w, 'public boolean updateLock()')
            + '\tprivate void disassemble() { worldObj.log.add("disassemble"); }\n' + base_methods(rd))
    (s / 'TileEntityWatz.java').write_text('import java.util.*;\nimport com.hbm.util.EnumUtil;\nimport com.hbm.util.function.Function;\nclass TileEntityWatz extends TileEntityMachineBase implements WatzTypes {\n' + body + '}\n')

    # PWR controller
    p = rd('tileentity/machine/TileEntityPWRController.java')
    fields = lines(p, *span(p, 'public FluidTank[] tanks;', 'protected List<BlockPos> rods = new ArrayList();'))
    pb = (fields + cut(p, 'public TileEntityPWRController()') + cut(p, 'public void setup(') + cut(p, 'public void updateEntity()') + cut(p, 'protected void meltDown()')
          + cut(p, 'protected void updateCoolant()') + cut(p, 'protected int getRodCountForCoolant()') + cut(p, 'protected void setupTanks()')
          + cut(p, 'public double getTotalProcessMultiplier()') + cut(p, 'public double connectinFunc(') + cut(p, 'public double getXOverE(')
          + cut(p, 'public void receiveControl(EntityPlayer').replace('\t@Override\n', '')
          + '\tpublic AudioWrapper createAudioLoop() { return new AudioWrapper(); }\n\tAudioWrapper rebootAudio(AudioWrapper a) { return a; }\n' + base_methods(rd))
    pwr = ('import java.util.*;\nimport com.hbm.util.EnumUtil;\nimport com.hbm.util.function.Function;\nimport java.util.Map.Entry;\n'
           'class TileEntityPWRController extends TileEntityMachineBase implements PwrTypes {\n' + pb + '}\n')
    (s / 'TileEntityPWRController.java').write_text(pwr)

    # PWR block: assemble / floodFill
    mc = rd('blocks/machine/MachinePWRController.java')
    asm = (lines(mc, *span(mc, 'private static HashMap<BlockPos, Block> assembly', 'private static final int maxSize = 4096;')) + cut(mc, 'public void assemble(')
           + cut(mc, 'private void floodFill(') + cut(mc, 'private boolean isValidCore(') + cut(mc, 'private boolean isValidCasing('))
    (s / 'MachinePWRController.java').write_text(
        'import java.util.*;\nimport java.util.Map.Entry;\nclass TileEntityBlockPWR extends TileEntity { Block block; int coreX, coreY, coreZ; void markDirty() {} }\n'
        'class MachinePWRController extends Block {\n\tMachinePWRController() { super("pwr_controller"); }\n\tstatic List<String> errors = new ArrayList<>();\n\tstatic TileEntityPWRController lastController;\n'
        '\tstatic void sendError(World w, int x, int y, int z, String m, EntityPlayer p) { errors.add(m); }\n'
        + asm.replace('(TileEntityPWRController) world.getTileEntity(x, y, z)', 'lastController').replace('(TileEntityBlockPWR) world.getTileEntity(pos.getX(), pos.getY(), pos.getZ())', 'new TileEntityBlockPWR()')
        + '\tstatic ForgeDirection facing(int meta) { return ForgeDirection.getOrientation(meta); }\n}\n')
    t = (s / 'MachinePWRController.java').read_text().replace('world.getBlockMetadata(x, y, z)', 'world.getBlockMetadata(x, y, z)')
    (s / 'MachinePWRController.java').write_text(t)

    # Hephaestus
    h = rd('tileentity/machine/TileEntityMachineHephaestus.java')
    hb = (lines(h, *span(h, 'public FluidTank input;', 'private long fissureScanTime;')) + 'ByteBuf buf;\n\tprivate AudioWrapper audio;\n'
          + cut(h, 'public void updateEntity()') + 'protected PortDef[] cachedPorts;\n' + cut(h, 'public PortDef[] getPorts()') + cut(h, 'protected void heatFluid()')
          + cut(h, 'protected void setupTanks()') + cut(h, 'protected int heatFromBlock(') + cut(h, 'public int getTotalHeat()'))
    (s / 'TileEntityMachineHephaestus.java').write_text('import java.util.*;\nclass TileEntityMachineHephaestus extends TileEntityLoadedBase {\n' + hb
        + '\tint[] heatDump() { return heat; }\n\tlong fissureDump() { return fissureScanTime; }\n}\n')

    # Flare
    f = rd('tileentity/machine/oil/TileEntityMachineGasFlare.java')
    fb = (lines(f, *span(f, 'public long power;', 'public UpgradeManagerNT upgradeManager = new UpgradeManagerNT();')) + 'List<String> releases = new ArrayList<>();\n'
          + 'public TileEntityMachineGasFlare() { super(6); tank = new FluidTank(Fluids.GAS, 64000); }\n' + 'protected PortDef[] cachedPorts;\n'
          + '\tpublic PortDef[] getPorts() { return new PortDef[0]; }\n' + cut(f, 'public void updateEntity()'))
    (s / 'TileEntityMachineGasFlare.java').write_text('import java.util.*;\nclass TileEntityMachineGasFlare extends TileEntityMachineBase {\n' + fb + '}\n')

    # fuel_traits for the flare come from the checked-in content table
    ft = json.loads((ROOT / 'content/machine-recipes/flare.json').read_text())['fuel_traits']
    rows = ''.join('\t\t{"%s", %s, %s, %d},\n' % (r['fluid'], str(r['gaseous_or_art']).lower(), str(r['flammable']).lower(), r['heat_tu_per_bucket']) for r in ft)
    (s / 'FlareFluids.java').write_text('class FlareFluids {\n\tstatic final Object[][] ROWS = {\n' + rows + '\t};\n}\n')
    return stm


def span(text, a, b):
    """1-based inclusive line span from the line containing `a` to the line containing `b`."""
    ls = text.splitlines()
    i = next(n for n, l in enumerate(ls, 1) if a in l)
    j = next(n for n, l in enumerate(ls, 1) if b in l and n >= i)
    return i, j


def base_methods(rd):
    b = rd('tileentity/TileEntityMachineBase.java')
    return cut(b, 'public ItemStack decrStackSize(int slot, int amount)')


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--source', required=True)
    ap.add_argument('--scratch', required=True)
    ap.add_argument('--check', action='store_true')
    a = ap.parse_args()
    src, out = Path(a.source), Path(a.scratch)
    man = manifest(src)
    gen(src, out)
    cls = out / 'classes'
    if cls.exists(): shutil.rmtree(cls)
    cls.mkdir(parents=True)
    files = [str(p) for p in (out / 'src').rglob('*.java')]
    subprocess.check_call(['javac', '--release', '8', '-nowarn', '-Xlint:none', '-d', str(cls)] + files, stderr=subprocess.STDOUT)
    res = subprocess.run(['java', '-Xss64m', '-cp', str(cls), 'Driver'], capture_output=True, text=True, check=True)
    fixture = {'oracle': man, 'data': json.loads(res.stdout)}
    text = json.dumps(fixture, sort_keys=True, separators=(',', ':')) + '\n'
    fx = HERE / 'fixtures/java.json'
    if a.check:
        if fx.read_text() != text: sys.exit('fixture differs from a fresh run')
        print('fixture matches')
    else:
        fx.write_text(text)
        print('wrote', fx, len(text))


if __name__ == '__main__':
    main()
