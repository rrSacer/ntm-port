#!/usr/bin/env python3
"""Energy-network oracle: compile pinned HBM classes/methods and record their output.

Whole files are copied byte-for-byte (PowerNetMK2, NodeNet, the IEnergy*MK2 interfaces,
IBatteryItem, Tuple, BlockPos/DirPos/Rotation, and Forge/Minecraft's ForgeDirection, Vec3,
MathHelper). Tile entities and ItemBatteryPack drag in the whole game, so for those the
method bodies are *sliced verbatim* out of the real file into a generated class
(`slice_*` below); nothing is retyped. Hand-written stubs live in java/stubs.
"""
import argparse, hashlib, json, os, re, shutil, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
JAVA = 'src/main/java/'
PIN = '4f5924e'
WHOLE = [
    'api/hbm/energymk2/PowerNetMK2.java', 'com/hbm/uninos/NodeNet.java',
    'api/hbm/energymk2/IEnergyReceiverMK2.java', 'api/hbm/energymk2/IEnergyProviderMK2.java',
    'api/hbm/energymk2/IEnergyHandlerMK2.java', 'api/hbm/energymk2/IEnergyConnectorMK2.java',
    'api/hbm/energymk2/IBatteryItem.java', 'com/hbm/util/Tuple.java', 'com/hbm/util/EnumUtil.java',
    'com/hbm/util/fauxpointtwelve/BlockPos.java', 'com/hbm/util/fauxpointtwelve/DirPos.java',
    'com/hbm/util/fauxpointtwelve/Rotation.java', 'com/hbm/interfaces/Spaghetti.java', 'com/hbm/items/machine/ItemBatteryCreative.java',
]
SLICED = [
    'com/hbm/lib/Library.java', 'com/hbm/items/machine/ItemBatteryPack.java',
    'com/hbm/tileentity/machine/storage/TileEntityBatteryBase.java',
    'com/hbm/tileentity/machine/storage/TileEntityBatteryREDD.java',
    'com/hbm/tileentity/machine/storage/TileEntityBatterySocket.java',
    'com/hbm/tileentity/network/TileEntityPylonBase.java', 'com/hbm/tileentity/network/TileEntityPylon.java',
    'com/hbm/tileentity/network/TileEntityPylonMedium.java', 'com/hbm/tileentity/network/TileEntityPylonLarge.java',
    'com/hbm/tileentity/network/TileEntityConnector.java', 'com/hbm/tileentity/network/TileEntityConnectorSuper.java',
    'com/hbm/tileentity/network/TileEntityCableSwitch.java', 'com/hbm/blocks/BlockDummyable.java',
]
EXTERNAL = {  # not in the HBM tree: read from a decompiled 1.7.10 Forge/Minecraft
    'net/minecraftforge/common/util/ForgeDirection.java': 'forge',
    'net/minecraft/util/Vec3.java': 'mc', 'net/minecraft/util/MathHelper.java': 'mc',
}


def sha(b): return hashlib.sha256(b).hexdigest()


def mask(text):
    pat = r'"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'|//[^\n]*|/\*.*?\*/'
    return re.sub(pat, lambda m: re.sub(r'[^\n]', ' ', m[0]), text, flags=re.S)


def balanced(text, opening):
    m = mask(text); depth = 1
    for end in range(opening + 1, len(text)):
        depth += (m[end] == '{') - (m[end] == '}')
        if depth == 0:
            return end
    raise ValueError('unbalanced')


def meth(src, header):
    """Verbatim text of the method/enum whose declaration matches `header` (regex)."""
    ms = list(re.finditer(header, mask(src)))
    if len(ms) != 1:
        raise ValueError(f'{header!r}: {len(ms)} matches')
    start = ms[0].start()
    start = src.rfind('\n', 0, start) + 1
    end = balanced(src, src.index('{', ms[0].start()))
    return src[start:end + 1]


def line(src, regex):
    ms = list(re.finditer(regex, mask(src), flags=re.M))
    if len(ms) != 1:
        raise ValueError(f'{regex!r}: {len(ms)} matches')
    a = src.rfind('\n', 0, ms[0].start()) + 1
    return src[a:src.index('\n', ms[0].end())]


def span(src, first, last):
    a = src.index(first); a = src.rfind('\n', 0, a) + 1
    b = src.index(last, a); b = src.index('\n', b)
    return src[a:b]


def gen(args, S, out):
    def w(rel, text):
        p = out / rel; p.parent.mkdir(parents=True, exist_ok=True); p.write_text(text)
    L = S['com/hbm/lib/Library.java']
    w('com/hbm/lib/Library.java', 'package com.hbm.lib;\nimport net.minecraftforge.common.util.ForgeDirection;\nimport net.minecraft.item.ItemStack;\nimport api.hbm.energymk2.IBatteryItem;\nimport com.hbm.items.ModItems;\npublic class Library {\n'
      + '\n'.join(line(L, rf'ForgeDirection {n} =') for n in ('POS_X', 'NEG_X', 'POS_Z', 'NEG_Z')) + '\n'
      + meth(L, r'public static long chargeItemsFromTE\(') + '\n' + meth(L, r'public static long chargeTEFromItems\(') + '\n}\n')
    B = S['com/hbm/items/machine/ItemBatteryPack.java']
    w('com/hbm/items/machine/ItemBatteryPack.java', 'package com.hbm.items.machine;\nimport api.hbm.energymk2.IBatteryItem;\nimport com.hbm.items.ItemEnumMulti;\nimport com.hbm.lib.RefStrings;\nimport com.hbm.main.MainRegistry;\nimport com.hbm.items.ModItems;\nimport com.hbm.util.EnumUtil;\nimport net.minecraft.item.ItemStack;\nimport net.minecraft.nbt.NBTTagCompound;\nimport net.minecraft.util.ResourceLocation;\npublic class ItemBatteryPack extends ItemEnumMulti implements IBatteryItem {\n'
      + meth(B, r'public ItemBatteryPack\(\)') + '\n' + meth(B, r'public static enum EnumBatteryPack') + '\n'
      + '\n'.join(meth(B, rf'public (void|long) {n}\(') for n in ('chargeBattery', 'setCharge', 'dischargeBattery', 'getCharge', 'getMaxCharge', 'getChargeRate', 'getDischargeRate')) + '\n}\n')
    K = S['com/hbm/tileentity/machine/storage/TileEntityBatteryBase.java']
    w('com/hbm/tileentity/machine/storage/TileEntityBatteryBase.java', '''package com.hbm.tileentity.machine.storage;
import api.hbm.energymk2.*;
import api.hbm.energymk2.IEnergyReceiverMK2.ConnectionPriority;
import api.hbm.energymk2.Nodespace.PowerNode;
import com.hbm.tileentity.TileEntityMachineBase;
import com.hbm.uninos.UniNodespace;
import com.hbm.util.Compat;
import com.hbm.util.EnumUtil;
import com.hbm.util.fauxpointtwelve.BlockPos;
import com.hbm.util.fauxpointtwelve.DirPos;
import net.minecraft.entity.player.EntityPlayer;
import net.minecraft.nbt.NBTTagCompound;
import net.minecraft.tileentity.TileEntity;
import net.minecraft.util.MathHelper;
public abstract class TileEntityBatteryBase extends TileEntityMachineBase implements IEnergyProviderMK2, IEnergyReceiverMK2 {
''' + span(K, 'public byte lastRedstone', 'protected PowerNode node;') + '\n'
      + meth(K, r'public TileEntityBatteryBase\(') + '\n'
      + '\n'.join(meth(K, h) for h in (r'public void updateEntity\(', r'public byte getComparatorPower\(', r'public PowerNode createNode\(', r'public short getRelevantMode\(', r'public void receiveControl\(')) + '\n'
      + line(K, r'public ConnectionPriority getPriority\(\)') + '\n'
      + line(K, r'public abstract BlockPos\[\] getPortPos') + '\n' + line(K, r'public abstract DirPos\[\] getConPos') + '\n'
      + line(K, r'private short modeCache') + '\n}\n')
    w('net/minecraft/entity/player/EntityPlayer.java', 'package net.minecraft.entity.player;\npublic class EntityPlayer { }\n')
    R = S['com/hbm/tileentity/machine/storage/TileEntityBatteryREDD.java']
    w('com/hbm/tileentity/machine/storage/TileEntityBatteryREDD.java', '''package com.hbm.tileentity.machine.storage;
import api.hbm.energymk2.*;
import com.hbm.lib.Library;
import com.hbm.main.MainRegistry;
import com.hbm.sound.AudioWrapper;
import com.hbm.util.fauxpointtwelve.BlockPos;
import com.hbm.util.fauxpointtwelve.DirPos;
import java.math.BigInteger;
import net.minecraftforge.common.util.ForgeDirection;
public class TileEntityBatteryREDD extends TileEntityBatteryBase {
''' + span(R, 'public float prevRotation', 'private AudioWrapper audio;') + '\n'
      + '\n'.join(meth(R, h) for h in (r'public TileEntityBatteryREDD\(\)', r'public void updateEntity\(', r'public float getSpeed\(', r'public BlockPos\[\] getPortPos\(', r'public DirPos\[\] getConPos\(', r'public void usePower\(', r'public long transferPower\(')) + '\n'
      + '\n'.join(line(R, rf'@Override public \w+ {n}\(') for n in ('getPower', 'setPower', 'getMaxPower')) + '\n}\n')
    # (the Java method `getPower` etc. are one-liners)
    So = S['com/hbm/tileentity/machine/storage/TileEntityBatterySocket.java']
    w('com/hbm/tileentity/machine/storage/TileEntityBatterySocket.java', '''package com.hbm.tileentity.machine.storage;
import api.hbm.energymk2.*;
import com.hbm.items.ItemEnums.EnumBatterySC;
import com.hbm.items.ModItems;
import com.hbm.util.fauxpointtwelve.BlockPos;
import com.hbm.util.fauxpointtwelve.DirPos;
import net.minecraft.item.ItemStack;
import net.minecraftforge.common.util.ForgeDirection;
public class TileEntityBatterySocket extends TileEntityBatteryBase {
	public double scPowerMult = 1D;
''' + meth(So, r'public TileEntityBatterySocket\(\)') + '\n'
      + '\n'.join(meth(So, h) for h in (r'protected boolean hasSCLoaded\(', r'public boolean canExtractItem\(', r'public static long powerFromStack\(', r'public static long maxPowerFromStack\(', r'public long getProviderSpeed\(', r'public long getReceiverSpeed\(', r'public BlockPos\[\] getPortPos\(', r'public DirPos\[\] getConPos\(', r'public void setPower\(')) + '\n'
      + meth(So, r'@Override public long getPower\(\)') + '\n' + line(So, r'@Override public long getMaxPower\(\)') + '\n' + line(So, r'@Override public boolean isItemValidForSlot') + '\n}\n')
    P = S['com/hbm/tileentity/network/TileEntityPylonBase.java']
    w('com/hbm/tileentity/network/TileEntityPylonBase.java', '''package com.hbm.tileentity.network;
import api.hbm.energymk2.Nodespace;
import api.hbm.energymk2.Nodespace.PowerNode;
import com.hbm.lib.Library;
import com.hbm.util.fauxpointtwelve.BlockPos;
import com.hbm.util.fauxpointtwelve.DirPos;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.tileentity.TileEntity;
import net.minecraft.util.Vec3;
import net.minecraftforge.common.util.ForgeDirection;
public abstract class TileEntityPylonBase extends TileEntity implements api.hbm.energymk2.IEnergyConnectorMK2 {
	protected List<int[]> connected = new ArrayList<>();
	public int color;
''' + meth(P, r'public PowerNode createNode\(') + '\n' + meth(P, r'public static int canConnect\(') + '\n'
      + line(P, r'public abstract ConnectionType getConnectionType') + '\n' + line(P, r'public abstract Vec3\[\] getMountPos') + '\n' + line(P, r'public abstract double getMaxWireLength') + '\n'
      + meth(P, r'public Vec3 getConnectionPoint\(') + '\n' + meth(P, r'public List<int\[\]> getConnected\(') + '\n' + meth(P, r'public enum ConnectionType') + '\n}\n')
    def tile(name, base, wanted, extra=''):
        t = S[f'com/hbm/tileentity/network/{name}.java']
        body = ''
        for h in wanted:
            body += meth(t, h) + '\n'
        w(f'com/hbm/tileentity/network/{name}.java', f'''package com.hbm.tileentity.network;
import api.hbm.energymk2.Nodespace;
import api.hbm.energymk2.Nodespace.PowerNode;
import com.hbm.blocks.BlockDummyable;
import com.hbm.blocks.ModBlocks;
import com.hbm.lib.Library;
import com.hbm.util.fauxpointtwelve.BlockPos;
import com.hbm.util.fauxpointtwelve.DirPos;
import net.minecraft.block.Block;
import net.minecraft.tileentity.TileEntity;
import net.minecraft.util.Vec3;
import net.minecraftforge.common.util.ForgeDirection;
public class {name} extends {base} {{
{body}}}
''')
    common = [r'public ConnectionType getConnectionType\(', r'public Vec3\[\] getMountPos\(', r'public double getMaxWireLength\(']
    tile('TileEntityPylon', 'TileEntityPylonBase', common + [r'public boolean canConnect\(', r'public PowerNode createNode\('])
    tile('TileEntityPylonMedium', 'TileEntityPylonBase', common + [r'public boolean canConnect\(', r'public PowerNode createNode\(', r'public boolean hasTransformer\('])
    tile('TileEntityPylonLarge', 'TileEntityPylonBase', common)
    tile('TileEntityConnector', 'TileEntityPylonBase', common + [r'public PowerNode createNode\(', r'public boolean canConnect\('])
    tile('TileEntityConnectorSuper', 'TileEntityConnector', [r'public Vec3\[\] getMountPos\(', r'public double getMaxWireLength\('])
    sw = S['com/hbm/tileentity/network/TileEntityCableSwitch.java']
    w('com/hbm/tileentity/network/TileEntityCableSwitch.java', 'package com.hbm.tileentity.network;\npublic class TileEntityCableSwitch extends net.minecraft.tileentity.TileEntity {\n' + meth(sw, r'public boolean shouldCreateNode\(') + '\n}\n')
    d = S['com/hbm/blocks/BlockDummyable.java']
    assert re.search(r'public static final int offset = 10;', d), 'BlockDummyable.offset changed'


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--source', default=os.environ.get('ENERGY_NET_HBM_SRC'), help='HBM checkout at the pin (or $ENERGY_NET_HBM_SRC)')
    ap.add_argument('--mc-src', default=os.environ.get('ENERGY_NET_MC_SRC'), help='root with net/minecraft/util/{Vec3,MathHelper}.java, a decompiled 1.7.10 (or $ENERGY_NET_MC_SRC)')
    ap.add_argument('--forge-src', default=os.environ.get('ENERGY_NET_FORGE_SRC'), help='root with net/minecraftforge/common/util/ForgeDirection.java (or $ENERGY_NET_FORGE_SRC)')
    ap.add_argument('--scratch', required=True)
    ap.add_argument('--check', action='store_true', help='fail if the output differs from the committed fixture')
    a = ap.parse_args()
    for name in ('source', 'mc_src', 'forge_src'):
        if not getattr(a, name):
            sys.exit(f'--{name.replace("_", "-")} (or its environment variable) is required; see README.md')
    src = Path(a.source); scratch = Path(a.scratch)
    head = subprocess.run(['git', '-C', str(src), 'rev-parse', 'HEAD'], capture_output=True, text=True).stdout.strip()
    if not head.startswith(PIN):
        sys.exit(f'ntm-src is at {head}, expected pin {PIN}')
    for sub in ('src', 'classes'):
        shutil.rmtree(scratch / sub, ignore_errors=True)
    out = scratch / 'src'; out.mkdir(parents=True)
    shutil.copytree(HERE / 'java' / 'stubs', out, dirs_exist_ok=True)
    hashes = {}
    for rel in WHOLE:
        b = (src / JAVA / rel).read_bytes(); hashes[rel] = sha(b)
        (out / rel).parent.mkdir(parents=True, exist_ok=True); (out / rel).write_bytes(b)
    for rel, root in EXTERNAL.items():
        b = (Path(a.forge_src if root == 'forge' else a.mc_src) / rel).read_bytes(); hashes[rel] = sha(b)
        (out / rel).parent.mkdir(parents=True, exist_ok=True); (out / rel).write_bytes(b)
    S = {}
    for rel in SLICED:
        b = (src / JAVA / rel).read_bytes(); hashes[rel] = sha(b); S[rel] = b.decode()
    gen(a, S, out)
    shutil.copy(HERE / 'java' / 'Oracle.java', out / 'Oracle.java')
    classes = scratch / 'classes'; classes.mkdir()
    files = [str(p) for p in out.rglob('*.java')]
    r = subprocess.run(['javac', '--release', '8', '-nowarn', '-d', str(classes)] + files, capture_output=True, text=True)
    if r.returncode:
        sys.exit(r.stderr[-6000:])
    r = subprocess.run(['java', '-cp', str(classes), 'Oracle'], capture_output=True, text=True)
    if r.returncode:
        sys.exit(r.stderr[-4000:])
    data = json.loads(r.stdout)
    data['provenance'] = {'hbm_pin': head, 'input_sha256': hashes, 'harness_sha256': {
        'Oracle.java': sha((HERE / 'java' / 'Oracle.java').read_bytes()), 'run.py': sha(Path(__file__).read_bytes()),
        'stubs': sha(b''.join(p.read_bytes() for p in sorted((HERE / 'java' / 'stubs').rglob('*.java'))))}}
    fx = HERE / 'fixtures' / 'java.json'
    text = json.dumps(data, sort_keys=True, separators=(',', ':')) + '\n'
    if a.check:
        old = json.loads(fx.read_text()); old.pop('provenance', None); new = dict(data); new.pop('provenance')
        sys.exit(0 if old == new else 'fixture differs')
    fx.write_text(text)
    print('wrote', fx, len(text), 'bytes')


main()
