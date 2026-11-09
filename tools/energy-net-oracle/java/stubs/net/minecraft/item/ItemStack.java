package net.minecraft.item;
import net.minecraft.nbt.NBTTagCompound;
public class ItemStack {
	public Item item; public int damage; public NBTTagCompound stackTagCompound;
	public ItemStack(Item item) { this(item, 1, 0); }
	public ItemStack(Item item, int count, int damage) { this.item = item; this.damage = damage; }
	public Item getItem() { return item; }
	public int getItemDamage() { return damage; }
	public boolean hasTagCompound() { return stackTagCompound != null; }
	public ItemStack copy() { ItemStack s = new ItemStack(item, 1, damage); s.stackTagCompound = stackTagCompound; return s; }
}
