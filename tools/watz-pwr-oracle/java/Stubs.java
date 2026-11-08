import java.util.*;

/* Controlled host surface for the Watz/PWR/Hephaestus/Flare oracle. Identity and container
 * semantics only; every formula under test lives in the extracted HBM code. */

class Item { String name; int maxStack = 64; Item(String n) { name = n; } }

class NBTTagCompound {
	Map<String, Double> d = new HashMap<>();
	void setDouble(String k, double v) { d.put(k, v); }
	double getDouble(String k) { Double v = d.get(k); return v == null ? 0D : v; }
	void setString(String k, String v) {} void setFloat(String k, float v) {} void setInteger(String k, int v) {}
	void setBoolean(String k, boolean v) {}
	boolean hasKey(String k) { return false; } int getInteger(String k) { return 0; }
	NBTTagCompound copy() { NBTTagCompound c = new NBTTagCompound(); c.d.putAll(d); return c; }
}

class ItemStack {
	Item item; int meta; int stackSize; NBTTagCompound stackTagCompound;
	ItemStack(Block b, int s) { item = new Item(b.name); stackSize = s; }
	ItemStack(Item i, int s, int m) { item = i; stackSize = s; meta = m; }
	Item getItem() { return item; }
	int getItemDamage() { return meta; }
	boolean hasTagCompound() { return stackTagCompound != null; }
	int getMaxStackSize() { return item.maxStack; }
	ItemStack copy() { ItemStack c = new ItemStack(item, stackSize, meta); if(stackTagCompound != null) c.stackTagCompound = stackTagCompound.copy(); return c; }
	ItemStack splitStack(int n) { ItemStack c = copy(); c.stackSize = n; stackSize -= n; return c; }
}

class ModItems {
	static Item bolt = new Item("bolt");
	static Item watz_pellet = new Item("watz_pellet"), watz_pellet_depleted = new Item("watz_pellet_depleted");
	static Item pwr_fuel = new Item("pwr_fuel"), pwr_fuel_hot = new Item("pwr_fuel_hot");
}

class Block { String name; Block(String n) { name = n; } void breakBlock(World w, int x, int y, int z, Block b, int meta) { w.log.add("breakBlock " + x + " " + y + " " + z); } }
class Blocks { static Block lava = new Block("lava"), flowing_lava = new Block("flowing_lava"), air = new Block("air"); }
class ModBlocks {
	static Block volcanic_lava_block = new Block("volcanic_lava_block"), ore_volcano = new Block("ore_volcano");
	static Block watz_pump = new Block("watz_pump"), mud_block = new Block("mud_block"), corium_block = new Block("corium_block");
	static Block struct_watz_core = new Block("struct_watz_core"), watz = new Watz(), watz_element = new Block("watz_element"), watz_cooler = new Block("watz_cooler"), watz_end = new Block("watz_end");
	static Block pwr_controller = new MachinePWRController(), pwr_block = new Block("pwr_block"), pwr_fuel = new Block("pwr_fuel");
	static Block pwr_control = new Block("pwr_control"), pwr_channel = new Block("pwr_channel"), pwr_heatex = new Block("pwr_heatex");
	static Block pwr_heatsink = new Block("pwr_heatsink"), pwr_neutron_source = new Block("pwr_neutron_source");
	static Block pwr_casing = new Block("pwr_casing"), pwr_reflector = new Block("pwr_reflector"), pwr_port = new Block("pwr_port");
}

enum ForgeDirection {
	DOWN(0, -1, 0), UP(0, 1, 0), NORTH(0, 0, -1), SOUTH(0, 0, 1), WEST(-1, 0, 0), EAST(1, 0, 0), UNKNOWN(0, 0, 0);
	static final ForgeDirection[] VALID_DIRECTIONS = {DOWN, UP, NORTH, SOUTH, WEST, EAST};
	int offsetX, offsetY, offsetZ;
	ForgeDirection(int x, int y, int z) { offsetX = x; offsetY = y; offsetZ = z; }
	static ForgeDirection getOrientation(int i) { return i >= 0 && i < VALID_DIRECTIONS.length ? VALID_DIRECTIONS[i] : UNKNOWN; }
	ForgeDirection getOpposite() { return this == UNKNOWN ? UNKNOWN : VALID_DIRECTIONS[ordinal() ^ 1]; }
}
class Library { static ForgeDirection NEG_Y = ForgeDirection.DOWN, POS_Y = ForgeDirection.UP, POS_X = ForgeDirection.EAST, NEG_X = ForgeDirection.WEST, POS_Z = ForgeDirection.SOUTH, NEG_Z = ForgeDirection.NORTH;
	static long chargeItemsFromTE(ItemStack[] slots, int index, long power, long maxPower) { return power; } }

class BlockPos {
	private final int x, y, z;
	BlockPos(int x, int y, int z) { this.x = x; this.y = y; this.z = z; }
	int getX() { return x; } int getY() { return y; } int getZ() { return z; }
	BlockPos offset(ForgeDirection d, int n) { return new BlockPos(x + d.offsetX * n, y + d.offsetY * n, z + d.offsetZ * n); }
	@Override public boolean equals(Object o) { return o instanceof BlockPos && ((BlockPos) o).x == x && ((BlockPos) o).y == y && ((BlockPos) o).z == z; }
	@Override public int hashCode() { return (y + z * 31) * 31 + x; }
}
class PortDef { int[] pos; int dir; static PortDef make(int x, int y, int z, ForgeDirection... d) { PortDef p = new PortDef(); p.pos = new int[] {x, y, z}; p.dir = d.length == 1 ? d[0].ordinal() : -1; return p; } }

class AudioWrapper { void startSound() {} boolean isPlaying() { return true; } void updateVolume(float v) {} void keepAlive() {} void stopSound() {} }
class TileEntity { int xCoord, yCoord, zCoord; World worldObj; }
class EntityPlayer {}
class Entity { double getDistanceSq(double a, double b, double c) { return 0; } void setFire(int i) {} boolean attackEntityFrom(Object s, float f) { return true; } }
class AxisAlignedBB { static AxisAlignedBB getBoundingBox(double a, double b, double c, double d, double e, double f) { return new AxisAlignedBB(); } }
class DamageSource { static Object onFire = new Object(); }
class MathHelper {
	static int clamp_int(int v, int lo, int hi) { return v < lo ? lo : (v > hi ? hi : v); }
	static double clamp_double(double v, double lo, double hi) { return v < lo ? lo : (v > hi ? hi : v); }
}
class SatelliteRayScan { static void reportEvent(World w, int x, int y, int z, Object e, int r) {} }
class RayEvent { static Object INFO_NUCLEAR = new Object(); }
class ChunkProvider { World w; boolean chunkExists(int x, int z) { return w.chunksLoaded; } }
class Provider { int dimensionId = 0; }
class TargetPoint { TargetPoint(int a, int b, int c, int d, int e) {} }
class AuxParticlePacketNT { AuxParticlePacketNT(NBTTagCompound d, double x, double y, double z) {} }
class Wrapper { void sendToAllAround(Object a, Object b) {} }
class PacketDispatcher { static Wrapper wrapper = new Wrapper(); }
class Proxy { void effectNT(NBTTagCompound d) {} Entity me() { return new Entity(); } AudioWrapper getLoopedSound(Object... o) { return new AudioWrapper(); } }
class MainRegistry { static Proxy proxy = new Proxy(); }
class NTMSounds { static Object REACTOR_GEIGER_LOOP = new Object(), HEPHAESTUS_LOOP = new Object(); }
class RadProxy { void incrementRad(World w, int x, int y, int z, float v) { w.log.add("rad " + x + " " + y + " " + z + " " + v); } }
class ChunkRadiationManager { static RadProxy proxy = new RadProxy(); }
class Compat { static TileEntity getTileStandard(World w, int x, int y, int z) { return w.tiles.get(x + "," + y + "," + z); } }
class ParticleUtil { static void spawnGasFlame(Object... o) {} }

class World {
	boolean isRemote = false, chunksLoaded = true;
	long time;
	int power = 15;
	Map<String, Block> blocks = new HashMap<>();
	Map<String, TileEntity> tiles = new HashMap<>();
	List<String> log = new ArrayList<>();
	Provider provider = new Provider();
	ChunkProvider chunkprov = new ChunkProvider();
	Random rand = new Random(1);
	World() { chunkprov.w = this; }
	ChunkProvider getChunkProvider() { return chunkprov; }
	long getTotalWorldTime() { return time; }
	Block getBlock(int x, int y, int z) { Block b = blocks.get(x + "," + y + "," + z); return b == null ? Blocks.air : b; }
	Map<String, Integer> metaAt = new HashMap<>();
	int getBlockMetadata(int x, int y, int z) { Integer m = metaAt.get(x + "," + y + "," + z); return m == null ? 0 : m; }
	int getIndirectPowerLevelTo(int x, int y, int z, int d) { return power; }
	void setBlock(int x, int y, int z, Block b) { log.add("set " + x + " " + y + " " + z + " " + b.name); }
	void setBlock(int x, int y, int z, Block b, int m, int f) { log.add("set " + x + " " + y + " " + z + " " + b.name + " " + m); }
	void func_147480_a(int x, int y, int z, boolean b) { log.add("break " + x + " " + y + " " + z); }
	void newExplosion(Object e, double x, double y, double z, float s, boolean a, boolean b) { log.add("explode " + x + " " + y + " " + z + " " + s); }
	void spawnParticle(String s, double a, double b, double c, double d, double e, double f) {}
	void playSoundEffect(double x, double y, double z, String s, float v, float p) {}
	void spawnEntityInWorld(Object e) {}
	void markTileEntityChunkModified(int x, int y, int z, Object t) {}
	List<Entity> getEntitiesWithinAABB(Class<?> c, AxisAlignedBB bb) { return new ArrayList<>(); }
}

class TileEntityMachineBase extends TileEntity {
	ItemStack[] slots;
	TileEntityMachineBase(int n) { slots = new ItemStack[n]; }
	void networkPackNT(int r) {} void markChanged() {}
	void setupFluidPorts(PortDef[] p) {} void updatePortFIFO() {} void updatePortPOFIFO() {} void setupAllPorts(PortDef[] p) {} void destroyAllPorts() {}
	void checkTilt(Object a, boolean b) {} boolean tilted = false;
	float getVolume(float f) { return f; }
}
class TileEntityLoadedBase extends TileEntity { void networkPackNT(int r) {} void setupFluidPorts(PortDef[] p) {} void updatePortFIFO() {} }
class TiltType { static Object CONFIG = new Object(); }

class UpgradeType { static final UpgradeType SPEED = new UpgradeType(), EFFECT = new UpgradeType(); }
class UpgradeManagerNT {
	int speed, effect;
	void checkSlots(Object te, ItemStack[] slots, int a, int b) {}
	int getLevel(UpgradeType t) { return t == UpgradeType.SPEED ? speed : effect; }
}

/* ---- fluids ---- */
class FluidTrait {
	static void onRelease(World w, int x, int y, int z, FluidType t, FluidTank tank, FluidReleaseType r, int amount) { w.log.add((r == FluidReleaseType.BURN ? "burn " : "spill ") + t.name + " " + amount); }
}
class HeatingType { String name; HeatingType(String n) { name = n; } static final HeatingType BOILER = new HeatingType("boiler"), HEATEXCHANGER = new HeatingType("heatexchanger"), PWR = new HeatingType("pwr"), ICF = new HeatingType("icf"), PA = new HeatingType("pa"); }
class HeatingStep {
	final int amountReq, heatReq, amountProduced; final FluidType typeProduced;
	HeatingStep(int req, int heat, FluidType type, int prod) { amountReq = req; heatReq = heat; typeProduced = type; amountProduced = prod; }
}
class FT_Heatable extends FluidTrait {
	List<HeatingStep> steps = new ArrayList<>(); Map<HeatingType, Double> efficiency = new HashMap<>();
	FT_Heatable addStep(int heat, int req, FluidType type, int prod) { steps.add(new HeatingStep(req, heat, type, prod)); return this; }
	FT_Heatable setEff(HeatingType type, double eff) { efficiency.put(type, eff); return this; }
	double getEfficiency(HeatingType type) { Double eff = efficiency.get(type); return eff != null ? eff : 0.0D; }
	HeatingStep getFirstStep() { return steps.get(0); }
}
class FT_PWRModerator extends FluidTrait { double multiplier; FT_PWRModerator(double m) { multiplier = m; } double getMultiplier() { return multiplier; } }
class FT_Flammable extends FluidTrait { long energy; FT_Flammable(long e) { energy = e; } long getHeatEnergy() { return energy; } }
class FT_Gaseous extends FluidTrait {}
class FT_Gaseous_ART extends FluidTrait {}
class FT_Coolable extends FluidTrait { FT_Coolable(Object... o) {} FT_Coolable setEff(Object... o) { return this; } }

class FluidType {
	String name; Map<Class<?>, FluidTrait> traits = new HashMap<>();
	FluidType(String n) { name = n; }
	int getColor() { return 0; }
	FluidType addTraits(FluidTrait... t) { for(FluidTrait x : t) traits.put(x.getClass(), x); return this; }
	boolean hasTrait(Class<? extends FluidTrait> c) { return traits.containsKey(c); }
	<T extends FluidTrait> T getTrait(Class<? extends T> c) { return (T) traits.get(c); }
	void onFluidRelease(Object te, FluidTank t, int amount) { ((TileEntityMachineGasFlare) te).releases.add("release " + name + " " + amount); }
}
class FluidReleaseType { static final FluidReleaseType SPILL = new FluidReleaseType(), BURN = new FluidReleaseType(); }

class FluidTank {
	FluidType type; int fluid, maxFluid;
	FluidTank(FluidType t, int max) { type = t; maxFluid = max; }
	void setFill(int i) { fluid = i; }
	void setTankType(FluidType t) { if(t == null) t = Fluids.NONE; if(this.type == t) return; this.type = t; this.setFill(0); }
	FluidType getTankType() { return type; }
	int getFill() { return fluid; }
	int getMaxFill() { return maxFluid; }
	int changeTankSize(int size) { maxFluid = size; if(fluid > maxFluid) { int dif = fluid - maxFluid; fluid = maxFluid; return dif; } return 0; }
	boolean setType(int in, ItemStack[] slots) { return false; }
	boolean loadTank(int in, int out, ItemStack[] slots) { return false; }
	void serialize(Object buf) {}
}
class ByteBuf { void writeInt(int i) {} void release() {} }
class Unpooled { static ByteBuf buffer() { return new ByteBuf(); } }

class Watz extends Block { Watz() { super("watz"); } int getOffset() { return 3; } void fillSpace(World w, int x, int y, int z, ForgeDirection d, int o) { w.log.add("fill " + x + " " + y + " " + z + " " + o); } }
class BlockDummyable { static boolean safeRem; }
class Pair<K, V> { K key; V value; Pair(K k, V v) { key = k; value = v; } }
class ToolType { String n; ToolType(String n) { this.n = n; } static ToolType BOLT = new ToolType("bolt"), TORCH = new ToolType("torch"); }
class MetaBlock { Block block; int meta; MetaBlock(Block b, int m) { block = b; meta = m; } }
class AStack {}
class OreDictStack extends AStack { String ore; int n; OreDictStack(String o, int n) { ore = o; this.n = n; } }
class Dura { String bolt() { return "bolt_dura"; } }
class OreDictManager { static Dura DURA = new Dura(); }
class EntityItem { EntityItem(World w, double x, double y, double z, ItemStack s) { w.log.add("drop " + s.item.name + " " + s.stackSize); } }
class Mats { static class Mat { ItemStack make(Item i, int n) { return new ItemStack(new Item("bolt_dura"), n, 0); } } static Mat MAT_DURA = new Mat(); }
