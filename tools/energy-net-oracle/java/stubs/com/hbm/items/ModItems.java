package com.hbm.items;
import net.minecraft.item.Item;
/** Stub identities: battery_creative and battery_sc are only compared by reference in the sliced code. */
public class ModItems {
	public static Item battery_creative = new CreativeBattery();
	public static Item battery_sc = new Item();
	public static Item battery_pack = new com.hbm.items.machine.ItemBatteryPack();
	public static class CreativeBattery extends Item implements api.hbm.energymk2.IBatteryItem {
		public void chargeBattery(net.minecraft.item.ItemStack s, long i) { }
		public void setCharge(net.minecraft.item.ItemStack s, long i) { }
		public void dischargeBattery(net.minecraft.item.ItemStack s, long i) { }
		public long getCharge(net.minecraft.item.ItemStack s) { return Long.MAX_VALUE; }
		public long getMaxCharge(net.minecraft.item.ItemStack s) { return Long.MAX_VALUE; }
		public long getChargeRate(net.minecraft.item.ItemStack s) { return Long.MAX_VALUE; }
		public long getDischargeRate(net.minecraft.item.ItemStack s) { return Long.MAX_VALUE; }
	}
}
