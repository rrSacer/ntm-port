"""Controlled 1.7.10 host surface, NOT an implementation of RBMK physics.

Only data containers, identity, clamping, lookup and presentation dependencies
are supplied here. RBMK methods and recipe registration come from the pin.
Unexercised JSON-loader APIs fail closed instead of fabricating recipes.
"""

STUBS = {
    'net/minecraft/util/MathHelper.java': '''
package net.minecraft.util;
public class MathHelper {
 public static double clamp_double(double x,double a,double b){return x<a?a:(x>b?b:x);}
 public static int clamp_int(int x,int a,int b){return x<a?a:(x>b?b:x);}
}
''',
    'net/minecraft/util/EnumChatFormatting.java': '''
package net.minecraft.util;
public enum EnumChatFormatting {
 BLACK,DARK_BLUE,DARK_GREEN,DARK_AQUA,DARK_RED,DARK_PURPLE,GOLD,GRAY,
 DARK_GRAY,BLUE,GREEN,AQUA,RED,LIGHT_PURPLE,YELLOW,WHITE,OBFUSCATED,
 BOLD,STRIKETHROUGH,UNDERLINE,ITALIC,RESET;
 public String toString(){return "\\u00a7"+"0123456789abcdefklmnor".charAt(ordinal());}
}
''',
    'net/minecraft/nbt/NBTTagCompound.java': '''
package net.minecraft.nbt;
import java.util.*;
public class NBTTagCompound {
 public final Map<String,Double> values=new TreeMap<>();
 public void setDouble(String k,double v){values.put(k,v);}
 public double getDouble(String k){return values.getOrDefault(k,0D);}
 public NBTTagCompound copy(){NBTTagCompound n=new NBTTagCompound();n.values.putAll(values);return n;}
}
''',
    'net/minecraft/item/Item.java': '''
package net.minecraft.item;
import java.util.List;
import net.minecraft.world.World;
import net.minecraft.entity.Entity;
import net.minecraft.entity.player.EntityPlayer;
import net.minecraft.creativetab.CreativeTabs;
import net.minecraft.util.IIcon;
import net.minecraft.client.renderer.texture.IIconRegister;
public class Item {
 public String id="unregistered"; public int maxStackSize=64; public Item container;
 public IIcon itemIcon;
 public Item(){} public Item(String id){this.id=id;}
 public Item setUnlocalizedName(String n){id=n;return this;}
 public String getUnlocalizedName(){return "item."+id;}
 public String getUnlocalizedNameInefficiently(ItemStack s){return getUnlocalizedName();}
 public Item setTextureName(String n){return this;}
 public Item setCreativeTab(CreativeTabs t){return this;}
 public Item setContainerItem(Item i){container=i;return this;}
 public Item setMaxStackSize(int n){maxStackSize=n;return this;}
 public Item setHasSubtypes(boolean b){return this;}
 public Item setMaxDamage(int n){return this;}
 public void addInformation(ItemStack s,EntityPlayer p,List l,boolean b){}
 public boolean showDurabilityBar(ItemStack s){return false;}
 public double getDurabilityForDisplay(ItemStack s){return 0;}
 public void onCreated(ItemStack s,World w,EntityPlayer p){}
 public void getSubItems(Item i,CreativeTabs t,List l){}
 public void registerIcons(IIconRegister r){}
 public boolean requiresMultipleRenderPasses(){return false;}
 public int getRenderPasses(int m){return 1;}
 public IIcon getIconFromDamageForRenderPass(int m,int p){return null;}
 public boolean hasCustomEntity(ItemStack s){return false;}
 public int getEntityLifespan(ItemStack s,World w){return 6000;}
 public Entity createEntity(World w,Entity e,ItemStack s){throw new UnsupportedOperationException();}
}
''',
    'net/minecraft/item/ItemStack.java': '''
package net.minecraft.item;
import net.minecraft.block.Block;
import net.minecraft.nbt.NBTTagCompound;
public class ItemStack {
 private Item item; public int stackSize; private int meta;
 public NBTTagCompound stackTagCompound;
 public ItemStack(Item i){this(i,1,0);} public ItemStack(Item i,int n){this(i,n,0);}
 public ItemStack(Item i,int n,int m){item=i;stackSize=n;meta=m;}
 public ItemStack(Block b){this(b,1,0);} public ItemStack(Block b,int n){this(b,n,0);}
 public ItemStack(Block b,int n,int m){this(b.item,n,m);}
 public Item getItem(){return item;} public int getItemDamage(){return meta;}
 public boolean hasTagCompound(){return stackTagCompound!=null;}
 public NBTTagCompound getTagCompound(){return stackTagCompound;}
 public void setTagCompound(NBTTagCompound n){stackTagCompound=n;}
 public ItemStack copy(){ItemStack s=new ItemStack(item,stackSize,meta);if(hasTagCompound())s.stackTagCompound=stackTagCompound.copy();return s;}
}
''',
    'net/minecraft/block/Block.java': '''
package net.minecraft.block;
import net.minecraft.item.Item;
public class Block {public final Item item;public Block(String id){item=new Item(id);}}
''',
    'net/minecraft/world/GameRules.java': '''
package net.minecraft.world;
import java.util.*;
public class GameRules {
 public final Map<String,String> values=new TreeMap<>();
 public String getGameRuleStringValue(String k){return values.getOrDefault(k,"");}
 public boolean getGameRuleBooleanValue(String k){return Boolean.parseBoolean(getGameRuleStringValue(k));}
 public boolean hasRule(String k){return values.containsKey(k);}
 public void addGameRule(String k,String v){values.put(k,v);}
 public void setOrCreateGameRule(String k,String v){values.put(k,v);}
}
''',
    'net/minecraft/world/World.java': '''
package net.minecraft.world;
public class World {public boolean isRemote=false;private final GameRules rules=new GameRules();public GameRules getGameRules(){return rules;}}
''',
    'net/minecraft/entity/Entity.java': '''
package net.minecraft.entity;
public class Entity {public double posX,posY,posZ,motionX,motionY,motionZ;public void setDead(){}}
''',
    'net/minecraft/entity/player/EntityPlayer.java': '''
package net.minecraft.entity.player;
public class EntityPlayer extends net.minecraft.entity.Entity {}
''',
    'net/minecraft/creativetab/CreativeTabs.java': '''
package net.minecraft.creativetab; public class CreativeTabs {}
''',
    'net/minecraft/util/IIcon.java': '''
package net.minecraft.util; public interface IIcon {}
''',
    'net/minecraft/client/renderer/texture/IIconRegister.java': '''
package net.minecraft.client.renderer.texture;
public interface IIconRegister {net.minecraft.util.IIcon registerIcon(String s);}
''',
    'cpw/mods/fml/relauncher/Side.java': '''
package cpw.mods.fml.relauncher; public enum Side {CLIENT,SERVER}
''',
    'cpw/mods/fml/relauncher/SideOnly.java': '''
package cpw.mods.fml.relauncher;
public @interface SideOnly {Side value();}
''',
    'com/hbm/main/MainRegistry.java': '''
package com.hbm.main;
public class MainRegistry {
 public static final net.minecraft.creativetab.CreativeTabs controlTab=new net.minecraft.creativetab.CreativeTabs();
 public static final Logger logger=new Logger();
 public static class Logger {public void error(String s,Object... o){}public void warn(String s,Object... o){}}
}
''',
    'com/hbm/lib/RefStrings.java': '''
package com.hbm.lib; public class RefStrings {public static final String MODID="hbm";}
''',
    'com/hbm/config/GeneralConfig.java': '''
package com.hbm.config; public class GeneralConfig {public static boolean enable528,enable528ReasimBoilers;}
''',
    'com/hbm/util/i18n/I18nUtil.java': '''
package com.hbm.util.i18n;
public class I18nUtil {
 // Preserve keys/arguments, deliberately do not pretend to be MC localization.
 public static String resolveKey(String s,Object... a){return s+java.util.Arrays.toString(a);}
}
''',
    'com/hbm/handler/neutron/NeutronStream.java': '''
package com.hbm.handler.neutron; public class NeutronStream {}
''',
    'com/hbm/entity/item/EntityItemWaste.java': '''
package com.hbm.entity.item;
public class EntityItemWaste extends net.minecraft.entity.Entity {
 public int delayBeforeCanPickup;
 public EntityItemWaste(net.minecraft.world.World w,double x,double y,double z,net.minecraft.item.ItemStack s){throw new UnsupportedOperationException("waste entity not exercised");}
}
''',
    'com/hbm/interfaces/Spaghetti.java': '''
package com.hbm.interfaces; public @interface Spaghetti {String value();}
''',
    'net/minecraft/util/WeightedRandom.java': '''
package net.minecraft.util;
public class WeightedRandom {public static class Item {public int itemWeight;public Item(int w){itemWeight=w;}}}
''',
    'com/hbm/inventory/RecipesCommon.java': '''
package com.hbm.inventory;
import net.minecraft.item.*; import net.minecraft.block.Block;
public class RecipesCommon {
 public static abstract class AStack {}
 public static class ComparableStack extends AStack {
  public Item item;public int stacksize,meta;
  public ComparableStack(Item i){this(i,1,0);}public ComparableStack(Item i,int n){this(i,n,0);}
  public ComparableStack(Item i,int n,int m){item=i;stacksize=n;meta=m;}
  public ComparableStack(Block b){this(b.item);}public ComparableStack(Block b,int n){this(b.item,n);}
  public ComparableStack(Block b,int n,int m){this(b.item,n,m);}
  public ComparableStack(ItemStack s){this(s.getItem(),s.stackSize,s.getItemDamage());}
  public ComparableStack makeSingular(){stacksize=1;return this;}
  public ItemStack toStack(){return new ItemStack(item,stacksize,meta);}
  public String[] getDictKeys(){return new String[0];}
  public int hashCode(){return java.util.Objects.hash(item,stacksize,meta);}
  public boolean equals(Object o){if(!(o instanceof ComparableStack))return false;ComparableStack c=(ComparableStack)o;return item==c.item&&stacksize==c.stacksize&&meta==c.meta;}
 }
 public static class OreDictStack extends AStack {
  public final String ore; public OreDictStack(String s){ore=s;}
  public int hashCode(){return ore.hashCode();}
  public boolean equals(Object o){return o instanceof OreDictStack&&ore.equals(((OreDictStack)o).ore);}
 }
}
''',
    'net/minecraftforge/oredict/OreDictionary.java': '''
package net.minecraftforge.oredict;
public class OreDictionary {public static java.util.List<net.minecraft.item.ItemStack> getOres(String s){return new java.util.ArrayList<>();}}
''',
    'com/hbm/inventory/fluid/FluidType.java': '''
package com.hbm.inventory.fluid;
public class FluidType {public final String name;private final int id;public FluidType(String n,int id){name=n;this.id=id;}public int getID(){return id;}}
''',
    'com/hbm/inventory/FluidStack.java': '''
package com.hbm.inventory;
public class FluidStack {public com.hbm.inventory.fluid.FluidType type;public int fill;public FluidStack(com.hbm.inventory.fluid.FluidType t,int n){type=t;fill=n;}}
''',
    'com/hbm/items/machine/ItemFluidIcon.java': '''
package com.hbm.items.machine;
public class ItemFluidIcon {public static net.minecraft.item.ItemStack make(com.hbm.inventory.FluidStack f){return new net.minecraft.item.ItemStack(com.hbm.items.ModItems.fluid_icon,1,f.type.getID());}}
''',
    'com/hbm/inventory/recipes/loader/SerializableRecipe.java': '''
package com.hbm.inventory.recipes.loader;
import com.google.gson.*;import com.google.gson.stream.JsonWriter;
import com.hbm.inventory.*;import com.hbm.inventory.RecipesCommon.*;import net.minecraft.item.ItemStack;
public abstract class SerializableRecipe {
 public abstract void registerDefaults();public abstract String getFileName();public abstract Object getRecipeObject();
 public abstract void readRecipe(JsonElement e);public abstract void writeRecipe(Object o,JsonWriter w)throws java.io.IOException;public abstract void deleteRecipes();
 protected AStack readAStack(JsonArray a){throw new UnsupportedOperationException();}
 protected ItemStack readItemStack(JsonArray a){throw new UnsupportedOperationException();}
 protected FluidStack readFluidStack(JsonArray a){throw new UnsupportedOperationException();}
 protected void writeAStack(AStack a,JsonWriter w){throw new UnsupportedOperationException();}
 protected void writeItemStack(ItemStack a,JsonWriter w){throw new UnsupportedOperationException();}
 protected void writeFluidStack(FluidStack a,JsonWriter w){throw new UnsupportedOperationException();}
}
''',
    'com/google/gson/JsonElement.java': '''
package com.google.gson;
public class JsonElement {public JsonArray getAsJsonArray(){throw new UnsupportedOperationException();}public boolean getAsBoolean(){throw new UnsupportedOperationException();}}
''',
    'com/google/gson/JsonObject.java': '''
package com.google.gson;
public class JsonObject extends JsonElement {public boolean has(String s){throw new UnsupportedOperationException();}public JsonElement get(String s){throw new UnsupportedOperationException();}}
''',
    'com/google/gson/JsonArray.java': '''
package com.google.gson; public class JsonArray extends JsonElement {}
''',
    'com/google/gson/stream/JsonWriter.java': '''
package com.google.gson.stream;
public class JsonWriter {public JsonWriter name(String s){throw new UnsupportedOperationException();}public JsonWriter value(boolean b){throw new UnsupportedOperationException();}}
''',
}
