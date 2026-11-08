import api.hbm.energymk2.*;
import api.hbm.energymk2.IEnergyReceiverMK2.ConnectionPriority;
import api.hbm.energymk2.Nodespace.PowerNode;
import api.hbm.tile.ILoadedTile;
import com.hbm.items.ModItems;
import com.hbm.items.machine.ItemBatteryPack;
import com.hbm.items.machine.ItemBatteryPack.EnumBatteryPack;
import com.hbm.tileentity.machine.storage.*;
import com.hbm.tileentity.network.*;
import com.hbm.uninos.NodeNet;
import com.hbm.uninos.UniNodespace;
import com.hbm.util.fauxpointtwelve.BlockPos;
import com.hbm.util.fauxpointtwelve.DirPos;
import java.math.BigInteger;
import java.util.*;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.tileentity.TileEntity;
import net.minecraft.util.Vec3;
import net.minecraftforge.common.util.ForgeDirection;

/** Drives the real PowerNetMK2 and sliced tile-entity code; prints every observation as JSON. */
public class Oracle {
	// ---- tiny JSON writer (stable key order) ----
	static Map<String, Object> o(Object... kv) { Map<String, Object> m = new LinkedHashMap<>(); for (int i = 0; i < kv.length; i += 2) m.put((String) kv[i], kv[i + 1]); return m; }
	static void js(StringBuilder b, Object v) {
		if (v == null) b.append("null");
		else if (v instanceof Map) { b.append('{'); boolean f = true; for (Map.Entry<?, ?> e : ((Map<?, ?>) v).entrySet()) { if (!f) b.append(','); f = false; js(b, e.getKey()); b.append(':'); js(b, e.getValue()); } b.append('}'); }
		else if (v instanceof List) { b.append('['); boolean f = true; for (Object x : (List<?>) v) { if (!f) b.append(','); f = false; js(b, x); } b.append(']'); }
		else if (v instanceof String) b.append('"').append(((String) v).replace("\\", "\\\\").replace("\"", "\\\"")).append('"');
		else b.append(v);
	}
	@SafeVarargs static <T> List<T> l(T... xs) { return new ArrayList<>(Arrays.asList(xs)); }
	static long bits(double d) { return Double.doubleToLongBits(d); }

	// ---- PowerNetMK2 participants ----
	static class TN implements IEnergyProviderMK2, IEnergyReceiverMK2 {
		int id; long power, max, prov, rec; int pri; boolean provider, receiver, loaded = true;
		public int hashCode() { return id; }
		public boolean isLoaded() { return loaded; }
		public long getPower() { return power; }
		public void setPower(long p) { power = p; }
		public long getMaxPower() { return max; }
		public long getProviderSpeed() { return prov < 0 ? max : prov; }
		public long getReceiverSpeed() { return rec < 0 ? max : rec; }
		public ConnectionPriority getPriority() { return ConnectionPriority.values()[pri]; }
	}
	static class RD extends TileEntityBatteryREDD {
		int id; boolean loaded = true;
		public int hashCode() { return id; }
		public boolean isLoaded() { return loaded; }
	}
	static class SK extends TileEntityBatterySocket {
		int id; boolean loaded = true;
		public int hashCode() { return id; }
		public boolean isLoaded() { return loaded; }
	}

	static ItemStack stack(Map<String, Object> s) {
		if (s == null) return null;
		String kind = (String) s.get("kind");
		if (kind.equals("creative")) return new ItemStack(ModItems.battery_creative, 1, 0);
		if (kind.equals("other")) return new ItemStack(new Item(), 1, 0);
		ItemStack st = new ItemStack(ModItems.battery_pack, 1, (Integer) s.get("damage"));
		if (s.get("charge") != null) ((ItemBatteryPack) ModItems.battery_pack).setCharge(st, (Long) s.get("charge"));
		return st;
	}
	static Object stackState(ItemStack st) {
		if (st == null) return null;
		if (!(st.getItem() instanceof ItemBatteryPack)) return o("kind", "nonpack");
		return o("tag", st.hasTagCompound(), "charge", st.hasTagCompound() ? ((ItemBatteryPack) st.getItem()).getCharge(st) : 0L);
	}
	static Map<String, Object> stk(String kind, Integer damage, Long charge) { return o("kind", kind, "damage", damage, "charge", charge); }

	/** NodeNet keeps its entries in a HashSet, whose order follows identity hash codes (random in the game).
	 *  The oracle swaps in LinkedHashSets so the order is the insertion order, which is what the Rust port takes. */
	static PowerNetMK2 newNet() { PowerNetMK2 n = new PowerNetMK2(); n.providerEntries = new LinkedHashSet<>(); n.receiverEntries = new LinkedHashSet<>(); UniNodespace.testNet = n; UniNodespace.nodes.clear(); return n; }
	static void addTo(PowerNetMK2 net, Object e, boolean prov, boolean rec) {
		if (prov) net.providerEntries.add((IEnergyProviderMK2) e);
		if (rec) net.receiverEntries.add((IEnergyReceiverMK2) e);
	}
	static String bigOf(Object e) {
		if (e instanceof RD) return ((RD) e).power.toString();
		return Long.toString(((IEnergyHandlerMK2) e).getPower());
	}

	// ---- scenario 1: PowerNetMK2 ----
	static Map<String, Object> netCase(long seed, int updates, List<Map<String, Object>> specs) {
		PowerNetMK2 net = newNet();
		List<Object> ents = new ArrayList<>();
		for (Map<String, Object> s : specs) {
			String kind = (String) s.get("kind");
			if (kind.equals("plain")) {
				TN t = new TN(); t.id = (Integer) s.get("id"); t.power = (Long) s.get("power"); t.max = (Long) s.get("max");
				t.prov = (Long) s.get("prov"); t.rec = (Long) s.get("rec"); t.pri = (Integer) s.get("pri");
				t.provider = (Boolean) s.get("provider"); t.receiver = (Boolean) s.get("receiver"); t.loaded = (Boolean) s.get("loaded");
				ents.add(t); addTo(net, t, t.provider, t.receiver);
			} else {
				RD r = new RD(); r.id = (Integer) s.get("id"); r.power = new BigInteger((String) s.get("power")); r.loaded = (Boolean) s.get("loaded");
				ents.add(r); addTo(net, r, (Boolean) s.get("provider"), (Boolean) s.get("receiver"));
			}
		}
		NodeNet.rand = new Random(seed);
		List<Object> after = new ArrayList<>();
		for (int u = 0; u < updates; u++) {
			net.update();
			List<Object> ps = new ArrayList<>();
			for (Object e : ents) ps.add(bigOf(e));
			after.add(o("tracker", net.energyTracker, "powers", ps, "providers", net.providerEntries.size(), "receivers", net.receiverEntries.size()));
		}
		return o("seed", seed, "updates", updates, "nodes", specs, "after", after, "next_int", NodeNet.rand.nextInt(1000000));
	}
	static Map<String, Object> plain(int id, long power, long max, long prov, long rec, int pri, boolean p, boolean r) {
		return o("kind", "plain", "id", id, "power", power, "max", max, "prov", prov, "rec", rec, "pri", pri, "provider", p, "receiver", r, "loaded", true);
	}
	static long pick(Random r, long[] xs) { return xs[r.nextInt(xs.length)]; }
	static List<Object> netCases() {
		List<Object> cs = new ArrayList<>();
		// hand-made
		cs.add(netCase(1, 1, l(plain(0, 1000, 1000, -1, -1, 2, true, false), plain(1, 0, 600, -1, -1, 2, false, true))));
		// the cumulative energyUsed quirk: two priority groups, first group leaves unsent remainder
		cs.add(netCase(2, 1, l(plain(0, 1000, 1000, -1, -1, 2, true, false), plain(1, 0, 300, -1, -1, 4, false, true), plain(2, 0, 300, -1, -1, 3, false, true), plain(3, 0, 300, -1, -1, 1, false, true), plain(4, 0, 300, -1, -1, 0, false, true))));
		cs.add(netCase(3, 3, l(plain(0, 777, 1000, 100, -1, 2, true, false), plain(1, 555, 1000, 33, -1, 2, true, false), plain(2, 0, 1000, -1, 7, 2, false, true), plain(3, 500, 1000, -1, 11, 2, false, true))));
		cs.add(netCase(4, 2, l(plain(0, 5, 10, -1, -1, 2, true, true), plain(1, 5, 10, -1, -1, 2, true, true))));
		cs.add(netCase(5, 1, l(plain(0, 10, 10, -1, -1, 2, true, false), plain(1, 0, 10, -1, -1, 2, false, false))));
		cs.add(netCase(6, 1, l(plain(0, 0, 10, -1, -1, 2, true, false), plain(1, 0, 10, -1, -1, 2, false, true))));
		cs.add(netCase(7, 1, l(plain(0, 100, 100, -1, -1, 2, true, false))));
		cs.add(netCase(8, 1, l(plain(0, 100, 100, -1, -1, 2, false, true))));
		// unloaded entries are dropped
		Map<String, Object> unl = plain(1, 0, 100, -1, -1, 2, false, true); unl.put("loaded", false);
		cs.add(netCase(9, 2, l(plain(0, 100, 100, -1, -1, 2, true, false), unl)));
		// overflow of the long sums and Long extremes
		long big = Long.MAX_VALUE / 100;
		cs.add(netCase(10, 2, l(plain(0, big, big, -1, -1, 2, true, false), plain(1, big, big, -1, -1, 2, true, false), plain(2, 0, big, -1, -1, 2, false, true), plain(3, 0, big, -1, -1, 3, false, true))));
		List<Map<String, Object>> many = new ArrayList<>();
		for (int i = 0; i < 120; i++) many.add(plain(i, big, big, -1, -1, 2, i % 2 == 0, i % 2 == 1));
		cs.add(netCase(11, 1, many));
		cs.add(netCase(12, 1, l(plain(0, Long.MAX_VALUE, Long.MAX_VALUE, -1, -1, 2, true, false), plain(1, 0, Long.MAX_VALUE, -1, -1, 2, false, true))));
		cs.add(netCase(13, 1, l(plain(0, 10, 10, -1, -1, 2, true, false), plain(1, -50, 100, -1, -1, 2, false, true), plain(2, 0, 100, 0, 0, 2, false, true))));
		// long overflow in the demand/supply sums (sums wrap to 0 or negative; weights become NaN/Infinity)
		long p62 = 1L << 62;
		cs.add(netCase(9001, 2, l(plain(0, Long.MAX_VALUE / 2, Long.MAX_VALUE / 2, -1, -1, 2, true, false), plain(1, 0, p62, -1, -1, 2, false, true), plain(2, 0, p62, -1, -1, 2, false, true))));
		cs.add(netCase(9002, 2, l(plain(0, Long.MAX_VALUE / 2, Long.MAX_VALUE / 2, -1, -1, 2, true, false), plain(1, 0, p62, -1, -1, 2, false, true), plain(2, 0, p62, -1, -1, 2, false, true), plain(3, 0, p62, -1, -1, 2, false, true), plain(4, 0, p62, -1, -1, 2, false, true))));
		cs.add(netCase(9003, 2, l(plain(0, p62, p62, -1, -1, 2, true, false), plain(1, p62, p62, -1, -1, 2, true, false), plain(2, p62, p62, -1, -1, 2, true, false), plain(3, 0, Long.MAX_VALUE, -1, -1, 2, false, true))));
		cs.add(netCase(9004, 2, l(plain(0, p62, p62, -1, -1, 2, true, false), plain(1, p62, p62, -1, -1, 2, true, false), plain(2, p62, p62, -1, -1, 2, true, false), plain(3, p62, p62, -1, -1, 2, true, false), plain(4, 0, p62, -1, -1, 3, false, true), plain(5, 0, p62, -1, -1, 3, false, true), plain(6, 0, p62, -1, -1, 1, false, true))));
		// 200 providers of 1 HE and a 199 HE receiver: every share truncates to 0, leaving 199 HE of
		// "rounding error" that needs more than the 100 compensation draws
		List<Map<String, Object>> cap = new ArrayList<>();
		for (int i = 0; i < 200; i++) cap.add(plain(i, 1, 1, -1, -1, 2, true, false));
		cap.add(plain(200, 0, 199, -1, -1, 2, false, true));
		cs.add(netCase(9005, 1, cap));
		// REDD in a net (its transferPower never clamps; its getPower caps at max/2)
		cs.add(netCase(14, 3, l(o("kind", "redd", "id", 0, "power", "123456789012345678901234567890", "provider", true, "receiver", false, "loaded", true), plain(1, 0, 1000000, -1, -1, 2, false, true), plain(2, 0, big, -1, -1, 3, false, true))));
		cs.add(netCase(15, 3, l(o("kind", "redd", "id", 0, "power", "50", "provider", true, "receiver", true, "loaded", true), plain(1, 100, 1000, -1, -1, 2, true, false))));
		cs.add(netCase(16, 2, l(o("kind", "redd", "id", 0, "power", "-5", "provider", true, "receiver", true, "loaded", true), plain(1, 100, 1000, -1, -1, 2, true, false), plain(2, 0, 1000, -1, -1, 1, false, true))));
		// randomized
		long[] mags = {0, 1, 7, 100, 1000, 12345, 1_000_000, 1L << 40, 1L << 53, (1L << 53) + 1, big, Long.MAX_VALUE / 2};
		for (int c = 0; c < 400; c++) {
			Random r = new Random(1000 + c);
			int n = 2 + r.nextInt(r.nextInt(5) == 0 ? 40 : 9);
			List<Map<String, Object>> ns = new ArrayList<>();
			for (int i = 0; i < n; i++) {
				long max = pick(r, mags); if (r.nextInt(3) == 0) max = 1 + (Math.abs(r.nextLong()) % 100000);
				long power = r.nextInt(4) == 0 ? 0 : (max == 0 ? 0 : Math.abs(r.nextLong()) % (max + 1));
				long prov = r.nextInt(3) == 0 ? -1 : pick(r, mags), rec = r.nextInt(3) == 0 ? -1 : pick(r, mags);
				if (r.nextInt(12) == 0) { Map<String, Object> m = o("kind", "redd", "id", i, "power", new BigInteger(r.nextInt(100) + 1, r).toString(), "provider", r.nextBoolean(), "receiver", r.nextBoolean(), "loaded", true); ns.add(m); continue; }
				boolean p = r.nextInt(3) != 0, rcv = r.nextInt(3) != 0; if (!p && !rcv) p = true;
				Map<String, Object> m = plain(i, power, max, prov, rec, r.nextInt(5), p, rcv); if (r.nextInt(25) == 0) m.put("loaded", false); ns.add(m);
			}
			cs.add(netCase(5000 + c, 1 + r.nextInt(4), ns));
		}
		return cs;
	}

	// ---- scenario 2/3: REDD + socket through the real base updateEntity ----
	static Object members(PowerNetMK2 net, Object e) { return o("provider", net.providerEntries.contains(e), "receiver", net.receiverEntries.contains(e)); }
	static void place(TileEntity te, int x, int y, int z, int meta) { te.xCoord = x; te.yCoord = y; te.zCoord = z; te.meta = meta; }
	static void power(TileEntityBatteryBase te, boolean on) {
		te.worldObj.powered.clear();
		if (on) for (BlockPos p : te.getPortPos()) te.worldObj.powered.add(p.getX() + "," + p.getY() + "," + p.getZ());
	}
	static List<Object> reddCases() {
		List<Object> cs = new ArrayList<>();
		long[] charges = {0, 1, 1000, 1800000, 5L << 30};
		Object[][] setups = {
			{null, null}, {stk("pack", 0, 1800000L), null}, {stk("pack", 5, 72000000000L), stk("pack", 11, 0L)}, {null, stk("pack", 2, 0L)},
			{stk("pack", 4, 4500000000L), stk("pack", 7, 100L)}, {stk("creative", null, null), stk("pack", 1, 5L)}, {stk("pack", 6, null), stk("creative", null, null)},
			{stk("other", null, null), stk("other", null, null)}, {stk("pack", 3, 0L), stk("pack", 3, 0L)}, {stk("pack", 9, 123456789L), stk("pack", 9, 0L)},
		};
		String[] powers = {"0", "1000", "92233720368547758", "9223372036854775807", "184467440737095516170000000000", "-300", "46116860184273879"};
		int[][] modes = {{0, 2, 0}, {1, 1, 0}, {2, 2, 1}, {3, 3, 0}, {0, 3, 1}, {2, 0, 1}};
		int k = 0;
		for (Object[] su : setups) for (String pw : powers) {
			int[] md = modes[k++ % modes.length];
			RD r = new RD(); r.id = 0; place(r, 10, 64, -20, 10 + 2 + (k % 4));
			PowerNetMK2 net = newNet();
			r.power = new BigInteger(pw);
			r.slots[0] = stack((Map<String, Object>) su[0]); r.slots[1] = stack((Map<String, Object>) su[1]);
			r.redLow = (short) md[0]; r.redHigh = (short) md[1]; power(r, md[2] == 1);
			List<Object> ops = new ArrayList<>(), obs = new ArrayList<>();
			Object[] seq = {"tick", "tick", "transfer:123456789", "tick", "use:5000", "tick", "tick", "transfer:9223372036854775807", "tick", "tick"};
			for (Object op : seq) {
				String s = (String) op; ops.add(s);
				if (s.equals("tick")) r.updateEntity();
				else if (s.startsWith("transfer")) r.transferPower(Long.parseLong(s.substring(9)));
				else r.usePower(Long.parseLong(s.substring(4)));
				List<Object> log = new ArrayList<>(); for (BigInteger b : r.log) log.add(b == null ? null : b.toString());
				obs.add(o("power", r.power.toString(), "delta", r.delta.toString(), "log", log, "get_power", r.getPower(), "get_max", r.getMaxPower(),
					"speed_bits", Float.floatToIntBits(r.getSpeed()), "slot0", stackState(r.slots[0]), "slot1", stackState(r.slots[1]),
					"members", members(net, r), "priority", r.getPriority().ordinal(), "comparator", (int) r.getComparatorPower(), "mode", (int) r.getRelevantMode(true)));
			}
			cs.add(o("power0", pw, "slot0", su[0], "slot1", su[1], "red_low", md[0], "red_high", md[1], "powered", md[2] == 1, "meta", r.meta, "ops", ops, "obs", obs));
		}
		return cs;
	}
	static List<Object> socketCases() {
		List<Object> cs = new ArrayList<>();
		List<Map<String, Object>> stacks = new ArrayList<>();
		stacks.add(null); stacks.add(stk("creative", null, null)); stacks.add(stk("other", null, null));
		long[] fr = {0, 1, 50, 100};
		for (int d = 0; d < 12; d++) {
			stacks.add(stk("pack", d, null));
			EnumBatteryPack pk = EnumBatteryPack.values()[d];
			for (long f : new long[]{0, 1, pk.capacity / 3, pk.capacity / 2, pk.capacity - 1, pk.capacity, pk.capacity + 5}) stacks.add(stk("pack", d, f));
		}
		stacks.add(stk("pack", 12, 5L)); stacks.add(stk("pack", 13, 9000L)); stacks.add(stk("pack", -1, 1L));
		int k = 0;
		for (Map<String, Object> st : stacks) for (int[] md : new int[][]{{0, 2, 0}, {0, 2, 1}, {1, 3, 0}, {3, 1, 1}, {2, 0, 0}, {1, 1, 1}}) {
			SK s = new SK(); s.id = 0; place(s, -4, 70, 9, 12 + (k++ % 4));
			PowerNetMK2 net = newNet();
			s.slots[0] = stack(st); s.redLow = (short) md[0]; s.redHigh = (short) md[1]; power(s, md[2] == 1);
			s.priority = ConnectionPriority.values()[k % 5];
			s.updateEntity();
			ItemStack ex = s.slots[0];
			cs.add(o("slot", st, "red_low", md[0], "red_high", md[1], "powered", md[2] == 1, "meta", s.meta, "priority_in", ConnectionPriority.values()[(k) % 5].ordinal(),
				"obs", o("priority", s.priority.ordinal(), "get_power", s.getPower(), "get_max", s.getMaxPower(), "provider_speed", s.getProviderSpeed(), "receiver_speed", s.getReceiverSpeed(),
					"mode", (int) s.getRelevantMode(true), "comparator", (int) s.getComparatorPower(), "members", members(net, s),
					"can_extract", ex != null && s.canExtractItem(0, ex, 0), "valid", ex != null && s.isItemValidForSlot(0, ex), "slot", stackState(s.slots[0]))));
		}
		return cs;
	}
	static List<Object> controlCases() {
		// receiveControl: low/high advance modulo 4; priority advances LOW..HIGH and wraps
		List<Object> cs = new ArrayList<>();
		for (int low = 0; low < 4; low++) for (int high = 0; high < 4; high++) for (int pri = 1; pri <= 3; pri++) {
			SK s = new SK(); s.redLow = (short) low; s.redHigh = (short) high; s.priority = ConnectionPriority.values()[pri];
			List<Object> obs = new ArrayList<>();
			String[] keys = {"low", "high", "priority", "low", "priority", "priority", "high", "low", "high", "priority"};
			for (String key : keys) {
				// the sliced receiveControl reads NBT keys through data.hasKey; our stub compound answers via a set of names
				com.hbm.stubs.Control d = new com.hbm.stubs.Control(key);
				s.receiveControl(null, d);
				obs.add(o("low", (int) s.redLow, "high", (int) s.redHigh, "priority", s.priority.ordinal()));
			}
			cs.add(o("low", low, "high", high, "priority", pri, "keys", Arrays.asList(keys), "obs", obs));
		}
		return cs;
	}
	static List<Object> geometry() {
		List<Object> out = new ArrayList<>();
		for (int meta = 10; meta <= 15; meta++) {
			for (String kind : new String[]{"socket", "redd"}) {
				TileEntityBatteryBase t = kind.equals("socket") ? new SK() : new RD();
				place(t, 5, 64, -7, meta);
				List<Object> ports = new ArrayList<>(), cons = new ArrayList<>();
				for (BlockPos p : t.getPortPos()) ports.add(l(p.getX(), p.getY(), p.getZ()));
				for (DirPos p : t.getConPos()) cons.add(l(p.getX(), p.getY(), p.getZ(), p.getDir().ordinal()));
				out.add(o("kind", kind, "meta", meta, "ports", ports, "cons", cons));
			}
		}
		return out;
	}

	// ---- scenario 4: wires ----
	static TileEntityPylonBase mk(String kind, int x, int y, int z, int meta, int blockVariant) {
		TileEntityPylonBase t;
		switch (kind) {
			case "connector": t = new TileEntityConnector(); break;
			case "connector_super": t = new TileEntityConnectorSuper(); break;
			case "pylon": t = new TileEntityPylon(); break;
			case "medium": t = new TileEntityPylonMedium(); break;
			default: t = new TileEntityPylonLarge();
		}
		place(t, x, y, z, meta);
		t.block = blockVariant == 0 ? null : blockVariant == 1 ? com.hbm.blocks.ModBlocks.red_pylon_medium_wood : blockVariant == 2 ? com.hbm.blocks.ModBlocks.red_pylon_medium_wood_transformer : blockVariant == 3 ? com.hbm.blocks.ModBlocks.red_pylon_medium_steel : com.hbm.blocks.ModBlocks.red_pylon_medium_steel_transformer;
		return t;
	}
	static Object vec(Vec3 v) { return l(bits(v.xCoord), bits(v.yCoord), bits(v.zCoord)); }
	static Object nodeState(PowerNode n) {
		List<Object> pos = new ArrayList<>(), con = new ArrayList<>(), ex = new ArrayList<>();
		for (BlockPos p : n.positions) pos.add(l(p.getX(), p.getY(), p.getZ()));
		if (n.connections != null) for (DirPos p : n.connections) con.add(l(p.getX(), p.getY(), p.getZ(), p.getDir().ordinal()));
		return o("positions", pos, "connections", con);
	}
	static List<Object> wireCases() {
		List<Object> out = new ArrayList<>();
		String[] kinds = {"connector", "connector_super", "pylon", "medium", "large"};
		for (String kind : kinds) for (int meta = 0; meta <= 15; meta++) for (int bv = 0; bv < (kind.equals("medium") ? 5 : 1); bv++) {
			TileEntityPylonBase t = mk(kind, 3, 70, -9, meta, bv);
			List<Object> mounts = new ArrayList<>(); for (Vec3 v : t.getMountPos()) mounts.add(vec(v));
			List<Object> cd = new ArrayList<>(); for (ForgeDirection d : ForgeDirection.values()) cd.add(t.canConnect(d));
			Map<String, Object> m = o("kind", kind, "meta", meta, "block", bv, "type", t.getConnectionType().ordinal(), "max_len_bits", bits(t.getMaxWireLength()),
				"mounts", mounts, "point", vec(t.getConnectionPoint()), "can_connect_dir", cd);
			t.getConnected().add(new int[]{1, 2, 3}); t.getConnected().add(new int[]{-40, 5, 99});
			PowerNode nd = t.createNode();
			m.put("node", nodeState(nd)); List<Object> ex = new ArrayList<>(); for (DirPos p : nd.extra) ex.add(l(p.getX(), p.getY(), p.getZ(), p.getDir().ordinal())); m.put("node_extra", ex);
			out.add(m);
		}
		return out;
	}
	static List<Object> linkCases() {
		List<Object> out = new ArrayList<>();
		String[] kinds = {"connector", "connector_super", "pylon", "medium", "large"};
		Random r = new Random(77);
		int[] dist = {0, 1, 5, 10, 11, 24, 25, 26, 44, 45, 46, 99, 100, 101, 150};
		// float rounding of the distance: two large pylons whose true distance is 100.0000033 (> 100) but
		// whose float-rounded length is exactly 100, so the original accepts the wire
		int[][] edge = {{12, 15, -50, -76, -31}, {12, 15, -50, 76, -31}, {15, 12, 50, -76, 31}, {15, 12, 50, 76, 31}};
		for (int[] e : edge) {
			TileEntityPylonBase a = mk("large", 0, 100, 0, e[0], 0), b = mk("large", e[2], 100 + e[3], e[4], e[1], 0);
			out.add(o("a", o("kind", "large", "pos", l(0, 100, 0), "meta", e[0]), "b", o("kind", "large", "pos", l(e[2], 100 + e[3], e[4]), "meta", e[1]), "same", false, "code", TileEntityPylonBase.canConnect(a, b), "float_edge", true));
		}
		for (int c = 0; c < 1500; c++) {
			String ka = kinds[r.nextInt(5)], kb = kinds[r.nextInt(5)];
			int ma = r.nextInt(16), mb = r.nextInt(16);
			int ax = r.nextInt(200) - 100, ay = r.nextInt(100), az = r.nextInt(200) - 100;
			int bx, by, bz;
			if (c % 3 == 0) { // near the boundary along a random axis
				int d = dist[r.nextInt(dist.length)] + r.nextInt(3) - 1; bx = ax + d; by = ay + r.nextInt(3) - 1; bz = az;
			} else { bx = ax + r.nextInt(120) - 60; by = ay + r.nextInt(60) - 30; bz = az + r.nextInt(120) - 60; }
			boolean same = c % 50 == 0;
			TileEntityPylonBase a = mk(ka, ax, ay, az, ma, 0), b = same ? a : mk(kb, bx, by, bz, mb, 0);
			out.add(o("a", o("kind", ka, "pos", l(ax, ay, az), "meta", ma), "b", same ? null : o("kind", kb, "pos", l(bx, by, bz), "meta", mb), "same", same, "code", TileEntityPylonBase.canConnect(a, b)));
		}
		return out;
	}
	static List<Object> packs() {
		List<Object> out = new ArrayList<>();
		ItemBatteryPack it = (ItemBatteryPack) ModItems.battery_pack;
		for (int d = -2; d < 30; d++) {
			ItemStack s = new ItemStack(it, 1, d);
			out.add(o("damage", d, "capacity", it.getMaxCharge(s), "charge_rate", it.getChargeRate(s), "discharge_rate", it.getDischargeRate(s)));
		}
		return out;
	}
	static List<Object> switches() {
		List<Object> out = new ArrayList<>();
		for (int m = 0; m < 3; m++) { TileEntityCableSwitch s = new TileEntityCableSwitch(); s.meta = m; out.add(o("meta", m, "creates_node", s.shouldCreateNode())); }
		return out;
	}

	static List<Object> randoms() {
		List<Object> out = new ArrayList<>();
		int[] bounds = {1, 2, 3, 5, 7, 10, 16, 100, 1000, 1 << 20, (1 << 30) + 1, Integer.MAX_VALUE};
		for (long seed : new long[]{0, 1, 42, -7, 123456789L, Long.MAX_VALUE, Long.MIN_VALUE}) {
			Random r = new Random(seed); List<Object> draws = new ArrayList<>();
			for (int i = 0; i < 48; i++) { int b = bounds[i % bounds.length]; draws.add(l(b, r.nextInt(b))); }
			out.add(o("seed", seed, "draws", draws));
		}
		return out;
	}

	static List<Object> bigBytes() {
		List<Object> out = new ArrayList<>();
		String[] vals = {"0", "1", "-1", "127", "128", "-128", "-129", "255", "256", "-255", "-256", "32767", "32768", "-32768", "-32769",
			"92233720368547758", "9223372036854775807", "-9223372036854775808", "9223372036854775808", "18446744073709551615", "18446744073709551616",
			"123456789012345678901234567890", "-123456789012345678901234567890", "340282366920938463463374607431768211456", "-340282366920938463463374607431768211456"};
		for (String v : vals) {
			byte[] b = new BigInteger(v).toByteArray(); StringBuilder h = new StringBuilder();
			for (byte x : b) h.append(String.format("%02x", x));
			out.add(l(v, h.toString()));
		}
		return out;
	}

	public static void main(String[] a) {
		StringBuilder b = new StringBuilder();
		js(b, o("schema", 1, "packs", packs(), "net_cases", netCases(), "redd_cases", reddCases(), "socket_cases", socketCases(), "control_cases", controlCases(),
			"geometry", geometry(), "wires", wireCases(), "links", linkCases(), "switches", switches(), "randoms", randoms(), "big_bytes", bigBytes()));
		System.out.println(b);
	}
}
