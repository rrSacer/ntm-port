package net.minecraft.tileentity;
import net.minecraft.block.Block;
import net.minecraft.world.World;
/** Stub: coordinates, metadata, world and block type are test inputs. */
public class TileEntity {
	public int xCoord, yCoord, zCoord;
	public int meta;
	public Block block;
	public World worldObj = new World();
	public boolean invalid = false;
	public boolean isInvalid() { return invalid; }
	public int getBlockMetadata() { return meta; }
	public Block getBlockType() { return block; }
	public void markDirty() { }
}
