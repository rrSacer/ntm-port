package com.hbm.tileentity;
import net.minecraft.item.ItemStack;
import net.minecraft.tileentity.TileEntity;
/** Stub: the slot array, the no-op network packet and the volume getter the sliced code calls. */
public class TileEntityMachineBase extends TileEntity {
	public ItemStack[] slots;
	public boolean muffled = false;
	public TileEntityMachineBase(int slotCount) { slots = new ItemStack[slotCount]; }
	public void updateEntity() { }
	public boolean isLoaded() { return true; }
	public boolean isItemValidForSlot(int slot, ItemStack stack) { return false; }
	public void networkPackNT(int range) { }
	public float getVolume(float v) { return v; }
}
