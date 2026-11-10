package net.minecraft.nbt;
import java.util.HashMap;
import java.util.Map;
public class NBTTagCompound {
	private final Map<String, Long> longs = new HashMap<>();
	public void setLong(String k, long v) { longs.put(k, v); }
	public boolean hasKey(String k) { return longs.containsKey(k); }
	public long getLong(String k) { Long v = longs.get(k); return v == null ? 0L : v; }
}
