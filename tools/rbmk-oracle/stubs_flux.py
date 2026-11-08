"""Host stubs for the RBMK tile-entity oracle: identity, containers, a block
grid and presentation dependencies only. Columns, neutron handler, rods and
vectors are the pinned classes; nothing here computes flux or heat."""

REMOVE = {'com/hbm/handler/neutron/NeutronStream.java', 'net/minecraft/nbt/NBTTagCompound.java',
          'net/minecraft/world/World.java', 'net/minecraft/block/Block.java'}
FLUX_STUBS = {}


def put(path, src):
    FLUX_STUBS[path] = src


def cls(pkg, name, body='', kind='class', imports=''):
    put(pkg.replace('.', '/') + '/' + name + '.java',
        f'package {pkg}; {imports} public {kind} {name} {{{body}}}')


# ---- Minecraft / Forge
put('net/minecraft/nbt/NBTTagCompound.java', '''package net.minecraft.nbt; import java.util.*;
public class NBTTagCompound { public final Map<String,Object> values=new TreeMap<>();
 public void setDouble(String k,double v){values.put(k,v);} public double getDouble(String k){Object o=values.get(k);return o instanceof Double?(Double)o:0D;}
 public void setInteger(String k,int v){values.put(k,v);} public int getInteger(String k){Object o=values.get(k);return o instanceof Integer?(Integer)o:0;}
 public void setBoolean(String k,boolean v){values.put(k,v);} public boolean getBoolean(String k){Object o=values.get(k);return o instanceof Boolean&&(Boolean)o;}
 public void setString(String k,String v){values.put(k,v);} public String getString(String k){Object o=values.get(k);return o instanceof String?(String)o:"";}
 public void setShort(String k,short v){values.put(k,v);} public short getShort(String k){Object o=values.get(k);return o instanceof Short?(Short)o:0;}
 public void setByte(String k,byte v){values.put(k,v);} public byte getByte(String k){Object o=values.get(k);return o instanceof Byte?(Byte)o:0;}
 public void setLong(String k,long v){values.put(k,v);} public long getLong(String k){Object o=values.get(k);return o instanceof Long?(Long)o:0;}
 public void setFloat(String k,float v){values.put(k,v);} public float getFloat(String k){Object o=values.get(k);return o instanceof Float?(Float)o:0;}
 public boolean hasKey(String k){return values.containsKey(k);} public void setTag(String k,Object v){values.put(k,v);}
 public Object getTag(String k){return values.get(k);} public Set<String> func_150296_c(){return values.keySet();}
 public NBTTagList getTagList(String k,int t){Object o=values.get(k);return o instanceof NBTTagList?(NBTTagList)o:new NBTTagList();}
 public NBTTagCompound getCompoundTag(String k){Object o=values.get(k);return o instanceof NBTTagCompound?(NBTTagCompound)o:new NBTTagCompound();}
 public NBTTagCompound copy(){NBTTagCompound n=new NBTTagCompound();n.values.putAll(values);return n;} }''')
put('net/minecraft/nbt/NBTTagList.java', '''package net.minecraft.nbt; import java.util.*;
public class NBTTagList { public final List<NBTTagCompound> l=new ArrayList<>(); public void appendTag(NBTTagCompound c){l.add(c);}
 public int tagCount(){return l.size();} public NBTTagCompound getCompoundTagAt(int i){return l.get(i);} }''')
put('net/minecraft/block/Block.java', '''package net.minecraft.block; import net.minecraft.item.Item;
public class Block { public String getUnlocalizedName(){return "tile."+name;} public final Item item; public final String name; public boolean opaque=false;
 public Block(String id){name=id;item=new Item(id);} public boolean isOpaqueCube(){return opaque;} }''')
put('net/minecraft/tileentity/TileEntity.java', '''package net.minecraft.tileentity; import net.minecraft.world.World; import net.minecraft.block.Block; import net.minecraft.nbt.NBTTagCompound; import net.minecraft.util.AxisAlignedBB;
public class TileEntity { public World worldObj; public int xCoord,yCoord,zCoord; public boolean invalid=false;
 public World getWorldObj(){return worldObj;} public boolean isInvalid(){return invalid;} public void invalidate(){invalid=true;} public void onChunkUnload(){} public void markDirty(){}
 public int getBlockMetadata(){return worldObj.getBlockMetadata(xCoord,yCoord,zCoord);} public Block getBlockType(){return worldObj.getBlock(xCoord,yCoord,zCoord);}
 public void readFromNBT(NBTTagCompound n){} public void writeToNBT(NBTTagCompound n){} public void updateEntity(){}
 public AxisAlignedBB getRenderBoundingBox(){return null;} public double getMaxRenderDistanceSquared(){return 4096D;} }''')
put('net/minecraft/world/World.java', '''package net.minecraft.world; import java.util.*; import net.minecraft.block.Block; import net.minecraft.tileentity.TileEntity; import net.minecraft.entity.Entity; import net.minecraft.util.AxisAlignedBB; import net.minecraft.init.Blocks;
public class World { public boolean isRemote=false; private final GameRules rules=new GameRules(); public GameRules getGameRules(){return rules;}
 public Random rand=new Random(1L); public long time=0; public final Profiler theProfiler=new Profiler(); public final Provider provider=new Provider();
 public static class Profiler {public void startSection(String s){} public void endSection(){} public void endStartSection(String s){}}
 public static class Provider {public int dimensionId=0;}
 public final Map<Long,Block> blocks=new HashMap<>(); public final Map<Long,Integer> metas=new HashMap<>(); public final Map<Long,TileEntity> tiles=new HashMap<>();
 public final List<String> log=new ArrayList<>();
 public static long key(int x,int y,int z){return ((long)(x+1000000)*4000000L+(z+1000000))*512L+(y+100);}
 public Block getBlock(int x,int y,int z){Block b=blocks.get(key(x,y,z));return b==null?Blocks.air:b;}
 public int getBlockMetadata(int x,int y,int z){return metas.getOrDefault(key(x,y,z),0);}
 public boolean setBlock(int x,int y,int z,Block b){log.add("setBlock "+x+","+y+","+z+" "+b.name);blocks.put(key(x,y,z),b);return true;}
 public boolean setBlock(int x,int y,int z,Block b,int m,int f){metas.put(key(x,y,z),m);return setBlock(x,y,z,b);}
 public TileEntity getTileEntity(int x,int y,int z){return tiles.get(key(x,y,z));}
 public void addTile(TileEntity t,int x,int y,int z,Block b,int meta){t.worldObj=this;t.xCoord=x;t.yCoord=y;t.zCoord=z;tiles.put(key(x,y,z),t);blocks.put(key(x,y,z),b);metas.put(key(x,y,z),meta);}
 public void markBlockForUpdate(int x,int y,int z){} public void markTileEntityChunkModified(int x,int y,int z,TileEntity t){}
 public long getTotalWorldTime(){return time;} public void spawnEntityInWorld(Entity e){log.add("spawn "+e.getClass().getSimpleName());}
 public void playSoundEffect(double x,double y,double z,String s,float v,float p){} public void newExplosion(Entity e,double x,double y,double z,float s,boolean a,boolean b){}
 public <T> List<T> getEntitiesWithinAABB(Class<T> c,AxisAlignedBB bb){return new ArrayList<>();} }''')
put('net/minecraft/util/AxisAlignedBB.java', '''package net.minecraft.util; public class AxisAlignedBB { public static AxisAlignedBB getBoundingBox(double a,double b,double c,double d,double e,double f){return new AxisAlignedBB();} public AxisAlignedBB expand(double a,double b,double c){return this;} }''')
put('net/minecraft/util/Vec3.java', '''package net.minecraft.util; public class Vec3 { public double xCoord,yCoord,zCoord; protected Vec3(double x,double y,double z){xCoord=x;yCoord=y;zCoord=z;}
 public Vec3 setComponents(double x,double y,double z){xCoord=x;yCoord=y;zCoord=z;return this;}
 public static Vec3 createVectorHelper(double x,double y,double z){return new Vec3(x,y,z);} public double lengthVector(){return Math.sqrt(xCoord*xCoord+yCoord*yCoord+zCoord*zCoord);} }''')
put('net/minecraftforge/common/util/ForgeDirection.java', '''package net.minecraftforge.common.util;
public enum ForgeDirection { DOWN(0,-1,0),UP(0,1,0),NORTH(0,0,-1),SOUTH(0,0,1),WEST(-1,0,0),EAST(1,0,0),UNKNOWN(0,0,0);
 public final int offsetX,offsetY,offsetZ; ForgeDirection(int x,int y,int z){offsetX=x;offsetY=y;offsetZ=z;}
 public static final ForgeDirection[] VALID_DIRECTIONS={DOWN,UP,NORTH,SOUTH,WEST,EAST}; }''')
put('net/minecraft/entity/Entity.java', '''package net.minecraft.entity; public class Entity { public double posX,posY,posZ,motionX,motionY,motionZ,height; public void setDead(){} public float getEyeHeight(){return 0;} }''')
put('net/minecraft/entity/player/EntityPlayer.java', '''package net.minecraft.entity.player; public class EntityPlayer extends net.minecraft.entity.Entity { public InventoryPlayer inventory=new InventoryPlayer(); public double getDistanceSq(double a,double b,double c){return 0;}
 public void triggerAchievement(Object o){} }''')
cls('net.minecraft.inventory', 'IInventory', kind='interface')
put('net/minecraft/inventory/ISidedInventory.java', '''package net.minecraft.inventory; import net.minecraft.item.ItemStack; import net.minecraft.entity.player.EntityPlayer;
public interface ISidedInventory { int getSizeInventory(); ItemStack getStackInSlot(int i); ItemStack getStackInSlotOnClosing(int i); void setInventorySlotContents(int i,ItemStack s);
 String getInventoryName(); boolean hasCustomInventoryName(); int getInventoryStackLimit(); void openInventory(); void closeInventory(); boolean isItemValidForSlot(int i,ItemStack s);
 boolean isUseableByPlayer(EntityPlayer p); ItemStack decrStackSize(int slot,int amount); boolean canInsertItem(int i,ItemStack s,int j); boolean canExtractItem(int i,ItemStack s,int j);
 int[] getAccessibleSlotsFromSide(int s); }''')
cls('net.minecraft.inventory', 'Container')
put('io/netty/buffer/ByteBuf.java', '''package io.netty.buffer; public class ByteBuf { public void writeDouble(double d){} public void writeInt(int i){} public void writeByte(int i){} public void writeBoolean(boolean b){} public void writeLong(long l){}
 public double readDouble(){return 0;} public int readInt(){return 0;} public byte readByte(){return 0;} public boolean readBoolean(){return false;} public long readLong(){return 0;} }''')
put('cpw/mods/fml/common/Optional.java', '''package cpw.mods.fml.common; import java.lang.annotation.*;
public @interface Optional { @interface Interface {String iface(); String modid();} @interface InterfaceList {Interface[] value();} @interface Method {String modid();} }''')
put('cpw/mods/fml/common/network/NetworkRegistry.java', '''package cpw.mods.fml.common.network; public class NetworkRegistry { public static class TargetPoint { public TargetPoint(int d,double x,double y,double z,double r){} } }''')
put('li/cil/oc/api/machine/Arguments.java', '''package li.cil.oc.api.machine; public class Arguments { public double checkDouble(int i){return 0;} public int checkInteger(int i){return 0;} }''')
put('li/cil/oc/api/machine/Context.java', '''package li.cil.oc.api.machine; public class Context {}''')
put('li/cil/oc/api/machine/Callback.java', '''package li.cil.oc.api.machine; public @interface Callback { boolean direct() default false; int limit() default 0; String doc() default ""; }''')
put('li/cil/oc/api/network/SimpleComponent.java', '''package li.cil.oc.api.network; public interface SimpleComponent { String getComponentName(); }''')
put('net/minecraft/client/Minecraft.java', '''package net.minecraft.client; public class Minecraft { public static Minecraft getMinecraft(){return null;} public Font fontRenderer; public Engine renderEngine;
 public static class Font {public void drawString(String s,int x,int y,int c){}} public static class Engine {public void bindTexture(Object o){}} }''')
put('net/minecraft/client/gui/Gui.java', '''package net.minecraft.client.gui; public class Gui { public static final Object icons=null; }''')
put('net/minecraft/client/gui/ScaledResolution.java', '''package net.minecraft.client.gui; public class ScaledResolution { public int getScaledWidth(){return 0;} public int getScaledHeight(){return 0;} }''')
put('net/minecraftforge/client/event/RenderGameOverlayEvent.java', '''package net.minecraftforge.client.event; public class RenderGameOverlayEvent { public net.minecraft.client.gui.ScaledResolution resolution; public static class Pre extends RenderGameOverlayEvent {} }''')
put('org/lwjgl/opengl/GL11.java', '''package org.lwjgl.opengl; public class GL11 { public static final int GL_BLEND=0; public static void glPushMatrix(){} public static void glPopMatrix(){} public static void glDisable(int i){} }''')

# ---- HBM host surface
put('com/hbm/tileentity/TileEntityLoadedBase.java', '''package com.hbm.tileentity; import net.minecraft.tileentity.TileEntity; import io.netty.buffer.ByteBuf; import com.hbm.tileentity.TilePort.PortDef;
public class TileEntityLoadedBase extends TileEntity { public boolean muffled=false; public void networkPackNT(int range){} public void serialize(ByteBuf b){} public void deserialize(ByteBuf b){}
 public void setupPowerPorts(PortDef[] p){} public void updateAllPorts(){} public void receivePower(){} public void markChanged(){} }''')
put('com/hbm/tileentity/TilePort.java', '''package com.hbm.tileentity; public class TilePort { public static class PortDef { public static PortDef make(int x,int y,int z,Object... d){return new PortDef();} } }''')
cls('com.hbm.tileentity', 'IGUIProvider', kind='interface', imports='import net.minecraft.entity.player.EntityPlayer; import net.minecraft.world.World; import net.minecraft.inventory.Container;',
    body='Container provideContainer(int ID,EntityPlayer player,World world,int x,int y,int z); Object provideGUI(int ID,EntityPlayer player,World world,int x,int y,int z);')
cls('com.hbm.tileentity', 'IOverpressurable', kind='interface', imports='import net.minecraft.world.World;', body='void explode(World w,int x,int y,int z);')
put('com/hbm/interfaces/IControlReceiver.java', '''package com.hbm.interfaces; import net.minecraft.entity.player.EntityPlayer; import net.minecraft.nbt.NBTTagCompound;
public interface IControlReceiver { boolean hasPermission(EntityPlayer p); void receiveControl(EntityPlayer p,NBTTagCompound d); }''')
put('com/hbm/interfaces/ICopiable.java', '''package com.hbm.interfaces; import net.minecraft.entity.player.EntityPlayer; import net.minecraft.nbt.NBTTagCompound; import net.minecraft.world.World;
public interface ICopiable { NBTTagCompound getSettings(World w,int x,int y,int z); void pasteSettings(NBTTagCompound n,int i,World w,EntityPlayer p,int x,int y,int z); }''')
cls('com.hbm.interfaces', 'NotableComments', kind='@interface')
put('com/hbm/handler/CompatHandler.java', '''package com.hbm.handler; public class CompatHandler { public interface OCComponent {} }''')
put('api/hbm/redstoneoverradio/IRORValueProvider.java', '''package api.hbm.redstoneoverradio; public interface IRORValueProvider { String PREFIX_VALUE="val:"; String[] getFunctionInfo(); String provideRORValue(String name); }''')
put('api/hbm/redstoneoverradio/IRORInteractive.java', '''package api.hbm.redstoneoverradio; public interface IRORInteractive { String PREFIX_FUNCTION="fun:"; String NAME_SEPARATOR=":"; String runRORFunction(String name,String[] params);
 static int parseInt(String s,int min,int max){try{return Math.max(min,Math.min(max,Integer.parseInt(s)));}catch(Exception e){return min;}} }''')
cls('api.hbm.tile', 'IInfoProviderEC', kind='interface', imports='import net.minecraft.nbt.NBTTagCompound;', body='void provideExtraInfo(NBTTagCompound data);')
put('com/hbm/util/CompatEnergyControl.java', '''package com.hbm.util; public class CompatEnergyControl { public static final String D_DEPLETION_PERCENT="a",D_XENON_PERCENT="b",D_SKIN_C="c",D_CORE_C="d",D_MELT_C="e"; }''')
put('api/hbm/energymk2/IEnergyReceiverMK2.java', '''package api.hbm.energymk2; import net.minecraftforge.common.util.ForgeDirection;
public interface IEnergyReceiverMK2 { enum ConnectionPriority {LOW,NORMAL,HIGH} long getPower(); void setPower(long p); long getMaxPower(); boolean canConnect(ForgeDirection d); ConnectionPriority getPriority(); }''')
put('api/hbm/fluidmk2/FluidNode.java', '''package api.hbm.fluidmk2; public class FluidNode { public java.util.List<com.hbm.util.fauxpointtwelve.BlockPos> positions=new java.util.ArrayList<>(); }''')
put('api/hbm/fluidmk2/FluidNetMK2.java', '''package api.hbm.fluidmk2; import java.util.*; public class FluidNetMK2 { public Set<FluidNode> links=new HashSet<>(); public Set<IFluidReceiverMK2> receiverEntries=new HashSet<>(); }''')
cls('api.hbm.fluidmk2', 'IFluidReceiverMK2', kind='interface')
put('com/hbm/lib/Library.java', '''package com.hbm.lib; import net.minecraftforge.common.util.ForgeDirection; public class Library { public static final ForgeDirection POS_Y=ForgeDirection.UP, NEG_Y=ForgeDirection.DOWN; }''')
put('com/hbm/main/MainRegistry.java', '''package com.hbm.main; public class MainRegistry {
 public static final net.minecraft.creativetab.CreativeTabs controlTab=new net.minecraft.creativetab.CreativeTabs(); public static final Logger logger=new Logger();
 public static final Object achRBMKBoom=new Object(); public static final Proxy proxy=new Proxy();
 public static class Logger {public void error(String s,Object... o){} public void warn(String s,Object... o){}}
 public static class Proxy { public void effectNT(net.minecraft.nbt.NBTTagCompound d){} } }''')
cls('com.hbm.packet.toclient', 'AuxParticlePacketNT', body='public AuxParticlePacketNT(net.minecraft.nbt.NBTTagCompound d,double x,double y,double z){}')
put('com/hbm/handler/threading/PacketThreading.java', '''package com.hbm.handler.threading; public class PacketThreading { public static void createAllAroundThreadedPacket(Object p,cpw.mods.fml.common.network.NetworkRegistry.TargetPoint t){} }''')
cls('com.hbm.util', 'ParticleUtil', body='public static void spawnGasFlame(net.minecraft.world.World w,double x,double y,double z,double a,double b,double c){}')
put('com/hbm/util/BufferUtil.java', '''package com.hbm.util; public class BufferUtil { public static void writeString(io.netty.buffer.ByteBuf b,String s){} public static String readString(io.netty.buffer.ByteBuf b){return "";} }''')
put('com/hbm/util/EnumUtil.java', '''package com.hbm.util; public class EnumUtil { public static <T extends Enum<T>> T grabEnumSafely(Class<T> c,int i){T[] v=c.getEnumConstants();return v[Math.abs(i)%v.length];} }''')
put('com/hbm/util/BobMathUtil.java', '''package com.hbm.util; public class BobMathUtil { public static double min(double... nums){double smallest=Double.MAX_VALUE;for(double num:nums)if(num<smallest)smallest=num;return smallest;} }''')
put('com/hbm/util/Compat.java', '''package com.hbm.util; import net.minecraft.world.World; import net.minecraft.tileentity.TileEntity; public class Compat { public static TileEntity getTileStandard(World w,int x,int y,int z){return w.getTileEntity(x,y,z);} }''')
put('com/hbm/saveddata/satellites/SatelliteRayScan.java', '''package com.hbm.saveddata.satellites; public class SatelliteRayScan { public enum RayEvent {INFO_NUCLEAR} public static void reportEvent(net.minecraft.world.World w,int x,int y,int z,RayEvent e,int n){} }''')
put('com/hbm/handler/radiation/ChunkRadiationManager.java', '''package com.hbm.handler.radiation; import java.util.*; import net.minecraft.world.World;
public class ChunkRadiationManager { public static final Proxy proxy=new Proxy(); public static class Proxy { public final List<float[]> events=new ArrayList<>();
 public void incrementRad(World w,int x,int y,int z,float rad){events.add(new float[]{x,y,z,rad});} } }''')

put('com/hbm/entity/effect/EntitySpear.java', '''package com.hbm.entity.effect; public class EntitySpear extends net.minecraft.entity.Entity { public EntitySpear(net.minecraft.world.World w){} }''')
put('com/hbm/entity/projectile/EntityRBMKDebris.java', '''package com.hbm.entity.projectile; public class EntityRBMKDebris extends net.minecraft.entity.Entity {
 public enum DebrisType {BLANK,ELEMENT,SHRAPNEL,GRAPHITE,FUEL,ROD,LID}
 public EntityRBMKDebris(net.minecraft.world.World w,double x,double y,double z,DebrisType t){} }''')
put('com/hbm/blocks/machine/rbmk/RBMKBase.java', '''package com.hbm.blocks.machine.rbmk; import net.minecraft.block.Block;
public class RBMKBase extends Block { public static boolean dropLids=true, digamma=false; public static final int offset=100;
 public enum Dir {DIR_NORMAL_LID, DIR_NO_LID} public static final Dir DIR_NORMAL_LID=Dir.DIR_NORMAL_LID, DIR_NO_LID=Dir.DIR_NO_LID;
 public RBMKBase(String n){super(n);} public int[] findCore(net.minecraft.world.World w,int x,int y,int z){return null;} }''')
put('com/hbm/blocks/machine/rbmk/RBMKRod.java', '''package com.hbm.blocks.machine.rbmk; public class RBMKRod extends RBMKBase { public boolean moderated; public RBMKRod(String n,boolean m){super(n);moderated=m;} }''')
put('com/hbm/blocks/machine/rbmk/RBMKControl.java', '''package com.hbm.blocks.machine.rbmk; public class RBMKControl extends RBMKBase { public boolean moderated; public RBMKControl(String n,boolean m){super(n);moderated=m;} }''')
put('com/hbm/tileentity/machine/rbmk/TileEntityRBMKConsole.java', '''package com.hbm.tileentity.machine.rbmk; public class TileEntityRBMKConsole { public enum ColumnType {BLANK,FUEL,FUEL_SIM,CONTROL,CONTROL_AUTO,MODERATOR,ABSORBER,REFLECTOR,OUTGASSER,BREEDER,STORAGE,COOLER,HEATEX,BOILER,HEATER,INLET,OUTLET} }''')
for n in ('ContainerRBMKControl', 'ContainerRBMKControlAuto', 'ContainerRBMKRod'):
    put(f'com/hbm/inventory/container/{n}.java', f'package com.hbm.inventory.container; public class {n} extends net.minecraft.inventory.Container {{ public {n}(Object inv,Object te){{}} }}')
for n in ('GUIRBMKControl', 'GUIRBMKControlAuto', 'GUIRBMKRod'):
    put(f'com/hbm/inventory/gui/{n}.java', f'package com.hbm.inventory.gui; public class {n} {{ public {n}(Object inv,Object te){{}} }}')
put('net/minecraft/entity/player/InventoryPlayer.java', '''package net.minecraft.entity.player; public class InventoryPlayer {}''')

put('net/minecraft/util/MathHelper.java', '''package net.minecraft.util; public class MathHelper {
 public static int clamp_int(int n,int lo,int hi){return n<lo?lo:(n>hi?hi:n);} public static double clamp_double(double n,double lo,double hi){return n<lo?lo:(n>hi?hi:n);}
 public static float clamp_float(float n,float lo,float hi){return n<lo?lo:(n>hi?hi:n);}
 public static int floor_double(double d){int i=(int)d;return d<(double)i?i-1:i;} public static float sqrt_double(double d){return (float)Math.sqrt(d);} public static float sqrt_float(float f){return (float)Math.sqrt(f);} }''')
put('net/minecraft/item/ItemStack.java', '''package net.minecraft.item; import net.minecraft.block.Block; import net.minecraft.nbt.NBTTagCompound;
public class ItemStack { private Item item; public int stackSize; private int meta; public NBTTagCompound stackTagCompound;
 public ItemStack(Item i){this(i,1,0);} public ItemStack(Item i,int n){this(i,n,0);} public ItemStack(Item i,int n,int m){item=i;stackSize=n;meta=m;}
 public ItemStack(Block b){this(b,1,0);} public ItemStack(Block b,int n){this(b,n,0);} public ItemStack(Block b,int n,int m){this(b.item,n,m);}
 public Item getItem(){return item;} public int getItemDamage(){return meta;} public boolean hasTagCompound(){return stackTagCompound!=null;}
 public NBTTagCompound getTagCompound(){return stackTagCompound;} public void setTagCompound(NBTTagCompound n){stackTagCompound=n;}
 public ItemStack copy(){ItemStack s=new ItemStack(item,stackSize,meta);if(hasTagCompound())s.stackTagCompound=stackTagCompound.copy();return s;}
 public int getMaxStackSize(){return item.maxStackSize;} public String getUnlocalizedName(){return item.getUnlocalizedName();}
 public ItemStack splitStack(int n){ItemStack s=copy();s.stackSize=n;stackSize-=n;return s;}
 public static ItemStack loadItemStackFromNBT(NBTTagCompound n){throw new UnsupportedOperationException("slots are not persisted by this oracle");}
 public NBTTagCompound writeToNBT(NBTTagCompound n){throw new UnsupportedOperationException("slots are not persisted by this oracle");} }''')
for name, fields in (('Container', ''),):
    pass
put('cpw/mods/fml/common/eventhandler/SubscribeEvent.java', 'package cpw.mods.fml.common.eventhandler; public @interface SubscribeEvent {}')
put('cpw/mods/fml/common/gameevent/TickEvent.java', '''package cpw.mods.fml.common.gameevent; public class TickEvent { public Phase phase; public enum Phase {START,END} public static class ServerTickEvent extends TickEvent {} }''')
# ---- Outgasser surface
put('com/hbm/inventory/fluid/tank/FluidTank.java', '''package com.hbm.inventory.fluid.tank; import com.hbm.inventory.fluid.FluidType; import net.minecraft.nbt.NBTTagCompound; import io.netty.buffer.ByteBuf;
public class FluidTank { private FluidType type; private int fill, max; public FluidTank(FluidType t,int max){type=t;this.max=max;}
 public FluidType getTankType(){return type;} public void setTankType(FluidType t){type=t;} public int getFill(){return fill;} public void setFill(int f){fill=f;} public int getMaxFill(){return max;}
 public void readFromNBT(NBTTagCompound n,String k){fill=n.getInteger(k);} public void writeToNBT(NBTTagCompound n,String k){n.setInteger(k,fill);} public void serialize(ByteBuf b){} public void deserialize(ByteBuf b){} }''')
put('com/hbm/module/portmanager/ModulePortManFluidAdaptive.java', '''package com.hbm.module.portmanager; import com.hbm.inventory.fluid.tank.FluidTank; import com.hbm.tileentity.TilePort.PortDef;
public class ModulePortManFluidAdaptive { public ModulePortManFluidAdaptive(Object te){} public ModulePortManFluidAdaptive setOutputTanks(FluidTank... t){return this;} public void update(PortDef[] p){} }''')
put('api/hbm/fluidmk2/IFluidStandardSenderMK2.java', '''package api.hbm.fluidmk2; import com.hbm.inventory.fluid.tank.FluidTank; public interface IFluidStandardSenderMK2 { FluidTank[] getAllTanks(); FluidTank[] getSendingTanks(); }''')
for n in ('ContainerRBMKOutgasser',):
    put(f'com/hbm/inventory/container/{n}.java', f'package com.hbm.inventory.container; public class {n} extends net.minecraft.inventory.Container {{ public {n}(Object inv,Object te){{}} }}')
put('com/hbm/inventory/gui/GUIRBMKOutgasser.java', 'package com.hbm.inventory.gui; public class GUIRBMKOutgasser { public GUIRBMKOutgasser(Object inv,Object te){} }')

put('com/hbm/inventory/fluid/FluidType.java', 'package com.hbm.inventory.fluid; public class FluidType {public final String name;private final int id;public FluidType(String n,int id){name=n;this.id=id;}public int getID(){return id;} public String getName(){return name;}}')
