package net.minecraft.world;
import java.util.HashSet;
import java.util.Set;
public class World {
	public boolean isRemote = false;
	public Set<String> powered = new HashSet<>();
	public boolean isBlockIndirectlyGettingPowered(int x, int y, int z) { return powered.contains(x + "," + y + "," + z); }
}
