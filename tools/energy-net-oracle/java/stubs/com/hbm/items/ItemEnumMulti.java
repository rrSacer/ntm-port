package com.hbm.items;
import net.minecraft.item.Item;
public class ItemEnumMulti extends Item {
	protected Class<? extends Enum> theEnum;
	public ItemEnumMulti(Class<? extends Enum> theEnum, boolean multiName, boolean multiTexture) { this.theEnum = theEnum; }
	public void setMaxStackSize(int n) { }
	public void setCreativeTab(Object tab) { }
}
