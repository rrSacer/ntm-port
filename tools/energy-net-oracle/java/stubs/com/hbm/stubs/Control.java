package com.hbm.stubs;
/** Stand-in for the NBT compound a GUI button sends: only hasKey is read. */
public class Control extends net.minecraft.nbt.NBTTagCompound {
	private final String key;
	public Control(String key) { this.key = key; }
	public boolean hasKey(String k) { return k.equals(key); }
}
