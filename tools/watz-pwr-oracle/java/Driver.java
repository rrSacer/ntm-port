import java.util.*;
import com.hbm.util.function.Function;

/** Drives the extracted HBM code and prints inputs + outputs as JSON. Doubles are raw IEEE bits (hex). */
public class Driver {

	static String d(double v) { return "\"" + Long.toHexString(Double.doubleToRawLongBits(v)) + "\""; }
	static String s(String v) { return "\"" + v + "\""; }
	static String arr(List<String> l) { return "[" + String.join(",", l) + "]"; }
	static String obj(Object... kv) {
		StringBuilder b = new StringBuilder("{");
		for(int i = 0; i < kv.length; i += 2) { if(i > 0) b.append(','); b.append('"').append(kv[i]).append("\":").append(kv[i + 1]); }
		return b.append('}').toString();
	}
	static String ints(int... v) { List<String> l = new ArrayList<>(); for(int x : v) l.add("" + x); return arr(l); }

	public static void main(String[] a) {
		List<String> top = new ArrayList<>();
		top.add("\"functions\":" + functions());
		top.add("\"watz\":" + watzAll());
		top.add("\"watz_extra\":" + watzExtra());
		top.add("\"fluids\":" + fluids());
		top.add("\"pwr_structure\":" + pwrStructures());
		top.add("\"pwr_fission\":" + pwrFission());
		top.add("\"hephaestus\":" + hephaestus());
		top.add("\"flare\":" + flare());
		System.out.println("{" + String.join(",", top) + "}");
	}

	/* ---------------- functions & tables ---------------- */

	static double[] XS = {0, 1, 2, 10, 100, 1000, 2000, 2500, 3750.5, 12345.678, 1e5, 1e6, 3e7};

	static String functions() {
		List<String> w = new ArrayList<>(), p = new ArrayList<>();
		for(WatzTypes.EnumWatzType t : WatzTypes.EnumWatzType.values()) {
			List<String> fs = new ArrayList<>();
			Function[] f = {t.burnFunc, t.heatDiv, t.absorbFunc};
			String[] n = {"burn", "heat_div", "absorb"};
			for(int i = 0; i < 3; i++) {
				if(f[i] == null) continue;
				List<String> ys = new ArrayList<>();
				for(double x : XS) ys.add(arr(Arrays.asList(d(x), d(f[i].effonix(x)))));
				fs.add("\"" + n[i] + "\":" + arr(ys));
			}
			w.add(obj("name", s(t.name()), "ordinal", t.ordinal(), "color_light", t.colorLight, "color_dark", t.colorDark, "passive", d(t.passive),
					"heat_emission", d(t.heatEmission), "mud_content", d(t.mudContent), "yield", d(t.yield), "fn", "{" + String.join(",", fs) + "}"));
		}
		for(PwrTypes.EnumPWRFuel t : PwrTypes.EnumPWRFuel.values()) {
			List<String> ys = new ArrayList<>();
			for(double x : XS) ys.add(arr(Arrays.asList(d(x), d(t.function.effonix(x)))));
			p.add(obj("name", s(t.name()), "ordinal", t.ordinal(), "heat_emission", d(t.heatEmission), "yield", d(t.yield), "fn", arr(ys)));
		}
		TileEntityPWRController c = new TileEntityPWRController();
		List<String> con = new ArrayList<>(), xoe = new ArrayList<>();
		for(double x : new double[] {0, 0.5, 1, 2, 5, 10, 50, 100, 299, 300, 301, 1000, 5000, 100000}) {
			con.add(arr(Arrays.asList(d(x), d(c.connectinFunc(x)))));
			for(double dd : new double[] {2, 300}) xoe.add(arr(Arrays.asList(d(x), d(dd), d(c.getXOverE(x, dd)))));
		}
		return obj("watz", arr(w), "pwr", arr(p), "connectin", arr(con), "x_over_e", arr(xoe));
	}

	/* ---------------- Watz ---------------- */

	static String slotDump(ItemStack[] slots) {
		List<String> l = new ArrayList<>();
		for(int i = 0; i < slots.length; i++) {
			ItemStack st = slots[i];
			if(st == null) continue;
			boolean fresh = st.getItem() == ModItems.watz_pellet;
			l.add(arr(Arrays.asList("" + i, fresh ? "0" : "1", "" + st.getItemDamage(), "" + st.stackSize, d(st.stackTagCompound == null ? -1 : st.stackTagCompound.getDouble("yield")))));
		}
		return arr(l);
	}

	static String watzState(TileEntityWatz t) {
		return obj("heat", t.heat, "flux_base", d(t.fluxLastBase), "flux_reaction", d(t.fluxLastReaction),
				"tanks", ints(t.tanks[0].getFill(), t.tanks[1].getFill(), t.tanks[2].getFill()), "slots", slotDump(t.slots));
	}

	static ItemStack pellet(int meta, double yield) {
		ItemStack st = new ItemStack(ModItems.watz_pellet, 1, meta);
		ItemWatzPellet.setYield(st, yield);
		return st;
	}
	static ItemStack depleted(int meta) { return new ItemStack(ModItems.watz_pellet_depleted, 1, meta); }

	interface WatzInit { void run(TileEntityWatz[] segs, Random r); }

	static String watzRun(String name, int n, boolean pump, int power, boolean topFirst, int ticks, WatzInit init) {
		World w = new World();
		w.power = power;
		TileEntityWatz[] segs = new TileEntityWatz[n]; // index 0 = top
		int topY = 3 * (n - 1) + 10;
		for(int i = 0; i < n; i++) {
			TileEntityWatz t = new TileEntityWatz();
			t.worldObj = w; t.xCoord = 0; t.yCoord = topY - 3 * i; t.zCoord = 0;
			segs[i] = t;
			w.tiles.put("0," + t.yCoord + ",0", t);
		}
		if(pump) w.blocks.put("0," + (topY + 3) + ",0", ModBlocks.watz_pump);
		// the original scan stops at y < 0, so the lowest segment sits in the world
		init.run(segs, new Random(name.hashCode()));
		List<String> initial = new ArrayList<>(), tickDump = new ArrayList<>();
		for(TileEntityWatz t : segs) initial.add(watzState(t));
		for(int k = 0; k < ticks; k++) {
			w.log.clear();
			if(topFirst) for(int i = 0; i < n; i++) segs[i].updateEntity();
			else for(int i = n - 1; i >= 0; i--) segs[i].updateEntity();
			List<String> st = new ArrayList<>();
			for(TileEntityWatz t : segs) st.add(watzState(t));
			List<String> lg = new ArrayList<>();
			for(String e : w.log) lg.add(s(e));
			tickDump.add(obj("segments", arr(st), "log", arr(lg)));
		}
		return obj("name", s(name), "segments", n, "pump", pump, "power", power, "top_first", topFirst, "initial", arr(initial), "ticks", arr(tickDump));
	}

	static void fill(TileEntityWatz t, int cold, int hot, int mud) {
		t.tanks[0].setFill(cold); t.tanks[1].setFill(hot); t.tanks[2].setFill(mud);
	}

	static String watzAll() {
		List<String> l = new ArrayList<>();
		for(int ty = 0; ty < 12; ty++) {
			final int type = ty;
			l.add(watzRun("type" + ty, 1, true, 15, true, 70, (segs, r) -> {
				TileEntityWatz t = segs[0];
				fill(t, 64000, 0, 0);
				t.heat = 120_000;
				t.slots[0] = pellet(type, 2.5e5 + type * 7e4);
				t.slots[1] = pellet(0, 3e5);
				t.slots[2] = pellet(7, 1e6);
				t.slots[3] = pellet((type + 5) % 12, 4e5);
				t.slots[4] = pellet(9, 1e6);
				t.slots[5] = pellet(8, 1e6);
			}));
		}
		l.add(watzRun("stack3", 3, true, 15, true, 120, (segs, r) -> {
			for(TileEntityWatz t : segs) {
				fill(t, 40000 + r.nextInt(20000), r.nextInt(30000), r.nextInt(100));
				t.heat = r.nextInt(300_000);
				for(int i = 0; i < 24; i++) if(r.nextInt(3) != 0) t.slots[i] = r.nextInt(5) == 0 ? depleted(r.nextInt(12)) : pellet(r.nextInt(12), 1e5 + r.nextInt(900_000));
			}
		}));
		l.add(watzRun("stack3_bottom_first", 3, true, 15, false, 60, (segs, r) -> {
			for(TileEntityWatz t : segs) {
				fill(t, 64000, 0, 0);
				for(int i = 0; i < 24; i++) if(r.nextInt(2) == 0) t.slots[i] = pellet(r.nextInt(12), 5e5);
			}
			segs[0].slots[3] = depleted(2); segs[1].slots[3] = pellet(1, 5e5); segs[2].slots[3] = null;
			segs[0].slots[4] = pellet(0, 5e5); segs[1].slots[4] = null; segs[2].slots[4] = null;
		}));
		l.add(watzRun("mud_overflow", 1, true, 15, true, 8, (segs, r) -> {
			fill(segs[0], 64000, 0, 63990);
			segs[0].slots[0] = pellet(0, 5e8); segs[0].slots[1] = pellet(10, 5e8); segs[0].slots[2] = pellet(11, 5e8);
		}));
		l.add(watzRun("stack2_mud_overflow", 2, true, 15, true, 6, (segs, r) -> {
			fill(segs[0], 64000, 0, 100); fill(segs[1], 64000, 0, 64000);
			segs[0].slots[0] = pellet(1, 5e8);
		}));
		l.add(watzRun("no_pump", 1, false, 15, true, 10, (segs, r) -> { fill(segs[0], 64000, 0, 0); segs[0].slots[0] = pellet(0, 5e8); segs[0].fluxLastReaction = 500; segs[0].heat = 1000; }));
		l.add(watzRun("no_power", 1, true, 0, true, 10, (segs, r) -> { fill(segs[0], 64000, 0, 0); segs[0].slots[0] = pellet(0, 5e8); segs[0].fluxLastReaction = 500; segs[0].heat = 1000; }));
		l.add(watzRun("coolant_limited", 2, true, 15, true, 40, (segs, r) -> {
			fill(segs[0], 900, 63500, 0); fill(segs[1], 100, 63990, 0);
			segs[0].heat = 5_000_000; segs[1].heat = 2_000_000;
			segs[0].slots[0] = pellet(2, 5e8); segs[1].slots[0] = pellet(3, 5e8);
		}));
		return arr(l);
	}

	static String fluids() {
		List<String> l = new ArrayList<>();
		HeatingType[] ht = {HeatingType.BOILER, HeatingType.HEATEXCHANGER, HeatingType.PWR};
		for(java.lang.reflect.Field f : Fluids.class.getDeclaredFields()) {
			try {
				FluidType t = (FluidType) f.get(null);
				FT_Heatable h = t.getTrait(FT_Heatable.class);
				if(h == null) continue;
				HeatingStep st = h.getFirstStep();
				FT_PWRModerator m = t.getTrait(FT_PWRModerator.class);
				l.add(obj("name", s(t.name), "hot", s(st.typeProduced.name), "heat_req", st.heatReq, "amount_req", st.amountReq, "amount_produced", st.amountProduced,
						"eff_boiler", d(h.getEfficiency(ht[0])), "eff_heatexchanger", d(h.getEfficiency(ht[1])), "eff_pwr", d(h.getEfficiency(ht[2])),
						"moderator", m == null ? "null" : d(m.getMultiplier())));
			} catch(Exception e) { throw new RuntimeException(e); }
		}
		return arr(l);
	}

	static String watzExtra() {
		World w0 = new World();
		TileEntityWatz tw = new TileEntityWatz();
		tw.xCoord = 100; tw.yCoord = 50; tw.zCoord = 70; tw.worldObj = w0;
		List<String> ports = new ArrayList<>();
		for(PortDef p : tw.getPorts()) ports.add(ints(p.pos[0] - 100, p.pos[1] - 50, p.pos[2] - 70, p.dir));
		TileEntityMachineHephaestus th = new TileEntityMachineHephaestus();
		th.xCoord = 100; th.yCoord = 50; th.zCoord = 70; th.worldObj = w0;
		List<String> hports = new ArrayList<>();
		for(PortDef p : th.getPorts()) hports.add(ints(p.pos[0] - 100, p.pos[1] - 50, p.pos[2] - 70));
		BlockToolConversion.reg();
		List<String> conv = new ArrayList<>();
		for(Object o : BlockToolConversion.conversions.entrySet()) {
			Map.Entry<Pair, Pair> e = (Map.Entry<Pair, Pair>) o;
			Pair<ToolType, MetaBlock> k = e.getKey(); Pair<AStack[], MetaBlock> v = e.getValue();
			OreDictStack req = (OreDictStack) v.key[0];
			conv.add(obj("tool", s(k.key.n), "block", s(k.value.block.name), "meta", k.value.meta, "ore", s(req.ore), "count", req.n, "to_block", s(v.value.block.name), "to_meta", v.value.meta));
		}
		World w = new World();
		WatzBlock wb = new WatzBlock();
		w.log.clear();
		wb.breakBlock(w, 1, 2, 3, wb, 12);
		List<String> drops = new ArrayList<>();
		for(String e : w.log) drops.add(s(e));
		w.log.clear();
		wb.breakBlock(w, 1, 2, 3, wb, 11);
		List<String> noDrops = new ArrayList<>();
		for(String e : w.log) noDrops.add(s(e));
		// structure core: the base shell once, then one case per required position and damage mode
		World base = watzBase(40);
		List<String> keys = new ArrayList<>(new TreeSet<>(base.blocks.keySet()));
		List<String> cases = new ArrayList<>();
		for(String key : keys) {
			String[] k = key.split(",");
			for(int mode = 0; mode < 3; mode++) cases.add(arr(Arrays.asList(k[0], k[1], k[2], "" + mode, watzFormed(base, key, mode, 40) ? "1" : "0")));
		}
		List<String> times = new ArrayList<>();
		for(long tm : new long[] {0, 1, 19, 20, 40, 43, 60, 99, 100}) times.add(arr(Arrays.asList("" + tm, watzFormed(base, null, 0, tm) ? "1" : "0")));
		String structs = obj("base_blocks", arr(blockList(base)), "base_metas", arr(metaList(base)), "cases", arr(cases), "times", arr(times));
		return obj("ports", arr(ports), "heph_ports", arr(hports), "conversions", arr(conv), "break_drops", arr(drops), "break_no_drops", arr(noDrops), "structures", structs);
	}

	static World watzBase(long time) {
		World w = new World();
		w.time = time;
		put(w, 0, 1, 0, ModBlocks.watz_cooler); put(w, 0, 2, 0, ModBlocks.watz_cooler);
		int[][] el = {{1, 0}, {2, 0}, {0, 1}, {0, 2}, {-1, 0}, {-2, 0}, {0, -1}, {0, -2}, {1, 1}, {1, -1}, {-1, 1}, {-1, -1}};
		int[][] co = {{2, 1}, {2, -1}, {1, 2}, {-1, 2}, {-2, 1}, {-2, -1}, {1, -2}, {-1, -2}};
		List<int[]> cells = new ArrayList<>();
		for(int i = 0; i < 3; i++) {
			for(int[] e : el) put(w, e[0], i, e[1], ModBlocks.watz_element);
			for(int[] e : co) put(w, e[0], i, e[1], ModBlocks.watz_cooler);
			for(int j = -1; j < 2; j++) { cells.add(new int[] {3, i, j}); cells.add(new int[] {j, i, 3}); cells.add(new int[] {-3, i, j}); cells.add(new int[] {j, i, -3}); }
			cells.add(new int[] {2, i, 2}); cells.add(new int[] {2, i, -2}); cells.add(new int[] {-2, i, 2}); cells.add(new int[] {-2, i, -2});
		}
		for(int[] c : cells) { put(w, c[0], c[1], c[2], ModBlocks.watz_end); w.metaAt.put(c[0] + "," + c[1] + "," + c[2], 1); }
		return w;
	}

	/** mode 0 removes the block, 1 sets its metadata to 0, 2 replaces it with stone; formed = the core placed the reactor. */
	static boolean watzFormed(World base, String key, int mode, long time) {
		World w = new World();
		w.time = time;
		w.blocks.putAll(base.blocks); w.metaAt.putAll(base.metaAt);
		if(key != null) {
			if(mode == 0) w.blocks.remove(key); else if(mode == 1) w.metaAt.put(key, 0); else w.blocks.put(key, STONE);
		}
		TileEntityWatzStruct t = new TileEntityWatzStruct();
		t.worldObj = w; t.xCoord = 0; t.yCoord = 0; t.zCoord = 0;
		t.updateEntity();
		return !w.log.isEmpty();
	}

	static List<String> metaList(World w) {
		List<String> l = new ArrayList<>();
		for(Map.Entry<String, Integer> e : new TreeMap<>(w.metaAt).entrySet()) l.add("[" + e.getKey() + "," + e.getValue() + "]");
		return l;
	}

	/* ---------------- PWR structure ---------------- */

	static void put(World w, int x, int y, int z, Block b) { w.blocks.put(x + "," + y + "," + z, b); }
	static final Block STONE = new Block("stone");
	static Block[] CORE = {ModBlocks.pwr_fuel, ModBlocks.pwr_fuel, ModBlocks.pwr_fuel, ModBlocks.pwr_control, ModBlocks.pwr_channel, ModBlocks.pwr_channel,
			ModBlocks.pwr_heatex, ModBlocks.pwr_heatsink, ModBlocks.pwr_neutron_source};

	/** shell + interior; controller on a wall that faces into the interior along `meta`. */
	static String structure(String name, long seed, int sx, int sy, int sz, int meta, int mode, int heatsinkBias) {
		Random r = new Random(seed);
		World w = new World();
		int ox = 10, oy = 10, oz = 10;
		for(int x = 0; x < sx; x++) for(int y = 0; y < sy; y++) for(int z = 0; z < sz; z++) {
			boolean shell = x == 0 || y == 0 || z == 0 || x == sx - 1 || y == sy - 1 || z == sz - 1;
			Block b;
			if(shell) { int q = r.nextInt(10); b = q == 0 ? ModBlocks.pwr_port : q < 3 ? ModBlocks.pwr_reflector : ModBlocks.pwr_casing; }
			else {
				b = CORE[r.nextInt(CORE.length)];
				if(heatsinkBias > 0 && r.nextInt(100) < heatsinkBias) b = ModBlocks.pwr_heatsink;
			}
			put(w, ox + x, oy + y, oz + z, b);
		}
		// controller position: wall opposite to the flood direction
		int cx, cy = oy + sy / 2, cz;
		ForgeDirection dir = ForgeDirection.getOrientation(meta).getOpposite();
		cx = ox + sx / 2; cz = oz + sz / 2;
		if(dir.offsetX > 0) cx = ox; if(dir.offsetX < 0) cx = ox + sx - 1;
		if(dir.offsetZ > 0) cz = oz; if(dir.offsetZ < 0) cz = oz + sz - 1;
		put(w, cx, cy, cz, ModBlocks.pwr_controller);
		if(mode == 5 || mode == 6) for(int x = 1; x < sx - 1; x++) for(int y = 1; y < sy - 1; y++) for(int z = 1; z < sz - 1; z++) if(r.nextInt(8) == 0) // casing / reflector / port parts inside the core
			put(w, ox + x, oy + y, oz + z, mode == 5 ? ModBlocks.pwr_casing : (r.nextBoolean() ? ModBlocks.pwr_reflector : ModBlocks.pwr_port));
		if(mode == 1) { for(int x = 1; x < sx - 1; x++) for(int y = 1; y < sy - 1; y++) for(int z = 1; z < sz - 1; z++) if(w.getBlock(ox + x, oy + y, oz + z) == ModBlocks.pwr_fuel) put(w, ox + x, oy + y, oz + z, ModBlocks.pwr_control); } // no fuel
		if(mode == 2) { for(int x = 1; x < sx - 1; x++) for(int y = 1; y < sy - 1; y++) for(int z = 1; z < sz - 1; z++) if(w.getBlock(ox + x, oy + y, oz + z) == ModBlocks.pwr_neutron_source) put(w, ox + x, oy + y, oz + z, ModBlocks.pwr_heatsink); } // no source
		if(mode == 3) put(w, ox + sx / 2, oy + sy / 2, oz + sz / 2, STONE); // foreign block inside
		if(mode == 4) w.blocks.remove((ox + sx - 1) + "," + (oy + sy / 2) + "," + (oz + sz / 2)); // hole in the shell
		return runStructure(name, w, cx, cy, cz, meta);
	}

	static String runStructure(String name, World w, int cx, int cy, int cz, int meta) {
		List<String> blocks = blockList(w);
		w.metaAt.put(cx + "," + cy + "," + cz, meta);
		TileEntityPWRController c = new TileEntityPWRController();
		c.worldObj = w; c.xCoord = cx; c.yCoord = cy; c.zCoord = cz;
		MachinePWRController.lastController = c;
		MachinePWRController.errors.clear();
		((MachinePWRController) ModBlocks.pwr_controller).assemble(w, cx, cy, cz, null);
		List<String> errs = new ArrayList<>(), rods = new ArrayList<>(), ports = new ArrayList<>();
		for(String e : MachinePWRController.errors) errs.add(s(e));
		for(BlockPos p : c.rods) rods.add(ints(p.getX(), p.getY(), p.getZ()));
		Collections.sort(rods);
		for(PortDef p : c.ports) ports.add(ints(p.pos));
		Collections.sort(ports);
		int internal = 0, portBlocks = 0;
		for(String e : w.log) { if(e.endsWith("pwr_block 0")) internal++; if(e.endsWith("pwr_block 1")) portBlocks++; }
		return obj("name", s(name), "controller", ints(cx, cy, cz), "meta", meta, "blocks", arr(blocks),
				"errors", arr(errs), "assembled", c.assembled, "rod_count", c.rodCount, "connections", c.connections,
				"connections_controlled", c.connectionsControlled, "heatex", c.heatexCount, "channel", c.channelCount,
				"heatsink", c.heatsinkCount, "source", c.sourceCount, "core_heat_capacity", c.coreHeatCapacity,
				"rods", arr(rods), "ports", arr(ports), "internal_blocks", internal, "port_blocks", portBlocks);
	}

	/** blocks as runs along +x: [x, y, z, count, name] */
	static List<String> blockList(World w) {
		List<int[]> ps = new ArrayList<>(); List<String> nm = new ArrayList<>();
		TreeMap<String, String> sorted = new TreeMap<>();
		for(Map.Entry<String, Block> e : w.blocks.entrySet()) {
			String[] k = e.getKey().split(",");
			sorted.put(String.format("%08d,%08d,%08d", Integer.parseInt(k[1]) + 1000000, Integer.parseInt(k[2]) + 1000000, Integer.parseInt(k[0]) + 1000000), e.getKey() + "|" + e.getValue().name);
		}
		List<String> out = new ArrayList<>();
		int[] cur = null; String curName = null;
		for(String v : sorted.values()) {
			String[] kv = v.split("\\|"); String[] k = kv[0].split(",");
			int x = Integer.parseInt(k[0]), y = Integer.parseInt(k[1]), z = Integer.parseInt(k[2]);
			if(cur != null && cur[1] == y && cur[2] == z && cur[0] + cur[3] == x && kv[1].equals(curName)) { cur[3]++; continue; }
			if(cur != null) out.add("[" + cur[0] + "," + cur[1] + "," + cur[2] + "," + cur[3] + ",\"" + curName + "\"]");
			cur = new int[] {x, y, z, 1}; curName = kv[1];
		}
		if(cur != null) out.add("[" + cur[0] + "," + cur[1] + "," + cur[2] + "," + cur[3] + ",\"" + curName + "\"]");
		return out;
	}

	/** Interior of 1x1xn: a fuel rod at each end, control rods between, one neutron source in the middle. */
	static String fuelLine(String name, int n) {
		World w = new World();
		int ox = 10, oy = 10, oz = 10, sx = n + 2;
		for(int x = 0; x < sx; x++) for(int y = 0; y < 3; y++) for(int z = 0; z < 3; z++) {
			boolean shell = x == 0 || x == sx - 1 || y != 1 || z != 1;
			put(w, ox + x, oy + y, oz + z, shell ? ModBlocks.pwr_casing : ModBlocks.pwr_control);
		}
		put(w, ox + 1, oy + 1, oz + 1, ModBlocks.pwr_fuel); put(w, ox + n, oy + 1, oz + 1, ModBlocks.pwr_fuel);
		put(w, ox + sx / 2, oy + 1, oz + 1, ModBlocks.pwr_neutron_source);
		int cx = ox + sx / 2, cz = oz;
		put(w, cx, oy + 1, cz, ModBlocks.pwr_controller); // flood direction +z, into the middle of the line
		return runStructure(name, w, cx, oy + 1, cz, 2);
	}

	static String pwrStructures() {
		List<String> l = new ArrayList<>();
		int[] metas = {2, 3, 4, 5};
		for(int i = 0; i < 8; i++) l.add(structure("valid" + i, 100 + i, 4 + i, 4 + (i % 4) * 2, 5 + i, metas[i % 4], 0, i == 5 ? 60 : 0));
		l.add(structure("heatsinks_capped", 77, 12, 12, 12, 2, 0, 90));
		l.add(structure("up_facing", 11, 6, 6, 6, 1, 0, 0));
		l.add(structure("down_facing", 12, 6, 6, 6, 0, 0, 0));
		l.add(structure("no_fuel", 13, 7, 6, 7, 3, 1, 0));
		l.add(structure("no_source", 14, 7, 6, 7, 4, 2, 0));
		l.add(structure("foreign_block", 15, 7, 6, 7, 5, 3, 0));
		for(int q = 0; q < 3; q++) { l.add(structure("casing_inside" + q, 300 + q, 8, 7, 9, metas[q], 5, 0)); l.add(structure("reflector_port_inside" + q, 400 + q, 8, 7, 9, metas[q + 1], 6, 0)); }
		for(int n = 14; n <= 18; n++) l.add(fuelLine("fuel_line" + n, n));
		l.add(structure("open_shell", 16, 7, 6, 7, 5, 4, 0));
		// oversize: a long control column, one fuel at the end and one source
		World w = new World();
		for(int x = 0; x < 4200; x++) put(w, 100 + x, 70, 70, ModBlocks.pwr_control);
		put(w, 100, 70, 70, ModBlocks.pwr_fuel); put(w, 101, 70, 70, ModBlocks.pwr_neutron_source);
		put(w, 99, 70, 70, ModBlocks.pwr_controller);
		l.add(runStructure("oversize", w, 99, 70, 70, 4));
		w = new World();
		for(int x = 0; x < 4095; x++) put(w, 100 + x, 70, 70, ModBlocks.pwr_control);
		put(w, 100, 70, 70, ModBlocks.pwr_fuel); put(w, 101, 70, 70, ModBlocks.pwr_neutron_source);
		put(w, 4195, 70, 70, ModBlocks.pwr_casing); put(w, 99, 70, 70, ModBlocks.pwr_controller);
		
		w = new World();
		for(int x = 0; x < 4094; x++) put(w, 100 + x, 70, 70, ModBlocks.pwr_control);
		put(w, 100, 70, 70, ModBlocks.pwr_fuel); put(w, 101, 70, 70, ModBlocks.pwr_neutron_source);
		put(w, 4194, 70, 70, ModBlocks.pwr_casing); put(w, 99, 70, 70, ModBlocks.pwr_controller);
		l.add(runStructure("edge_4095", w, 99, 70, 70, 4));
		return arr(l);
	}

	/* ---------------- PWR fission ---------------- */

	static FluidType fluid(String n) {
		try { return (FluidType) Fluids.class.getDeclaredField(n.toUpperCase()).get(null); } catch(Exception e) { throw new RuntimeException(n, e); }
	}

	static String pwrState(TileEntityPWRController c) {
		return obj("core_heat", c.coreHeat, "hull_heat", c.hullHeat, "flux", d(c.flux), "progress", d(c.progress), "process_time", d(c.processTime),
				"type_loaded", c.typeLoaded, "amount_loaded", c.amountLoaded, "rod_level", d(c.rodLevel), "rod_target", d(c.rodTarget), "unload_delay", c.unloadDelay,
				"tanks", ints(c.tanks[0].getFill(), c.tanks[1].getFill()), "tank_types", arr(Arrays.asList(s(c.tanks[0].getTankType().name), s(c.tanks[1].getTankType().name))),
				"slot0", slotP(c.slots[0]), "slot1", slotP(c.slots[1]));
	}
	static String slotP(ItemStack st) { return st == null ? "null" : ints(st.getItem() == ModItems.pwr_fuel ? 0 : 1, st.getItemDamage(), st.stackSize); }

	interface PwrInit { void run(TileEntityPWRController c, World w); }

	static String pwrRun(String name, int ticks, PwrInit init) {
		World w = new World();
		TileEntityPWRController c = new TileEntityPWRController();
		c.worldObj = w; c.xCoord = 5; c.yCoord = 5; c.zCoord = 5;
		c.assembled = true; c.typeLoaded = -1;
		c.rodCount = 60; c.connections = 140; c.connectionsControlled = 60; c.heatexCount = 20; c.channelCount = 40; c.heatsinkCount = 12; c.sourceCount = 4;
		c.coreHeatCapacity = 10_000_000 + 12 * 500_000L;
		c.tanks[0].setTankType(Fluids.COOLANT);
		init.run(c, w);
		if(!name.startsWith("meltdown")) c.coreHeatCapacity = Long.MAX_VALUE / 4; // only the meltdown scenarios may melt
		String initial = pwrState(c);
		List<String> tl = new ArrayList<>();
		int melted = 0;
		for(int k = 0; k < ticks; k++) {
			w.log.clear();
			w.time++;
			c.updateEntity();
			List<String> lg = new ArrayList<>();
			for(String e : w.log) lg.add(s(e));
			tl.add(obj("state", pwrState(c), "log", arr(lg)));
			if(w.log.stream().anyMatch(e -> e.startsWith("explode")) && ++melted > 1) break;
		}
		List<String> rods = new ArrayList<>();
		for(BlockPos p : c.rods) rods.add(ints(p.getX(), p.getY(), p.getZ()));
		return obj("name", s(name), "chunks_loaded", w.chunksLoaded, "initial", initial, "setup", obj("rod_count", c.rodCount, "connections", c.connections, "connections_controlled", c.connectionsControlled,
				"heatex", c.heatexCount, "channel", c.channelCount, "heatsink", c.heatsinkCount, "source", c.sourceCount, "core_heat_capacity", c.coreHeatCapacity, "rods", arr(rods)),
				"ticks", arr(tl));
	}

	static void load(TileEntityPWRController c, int type, int amount) { c.typeLoaded = type; c.amountLoaded = amount; }

	static String pwrFission() {
		List<String> l = new ArrayList<>();
		for(int f = 0; f < 15; f++) {
			final int fuel = f;
			l.add(pwrRun("fuel" + f, 80, (c, w) -> {
				load(c, fuel, 60); c.flux = 5000; c.rodLevel = 100; c.rodTarget = 40; c.tanks[0].setFill(120_000); c.coreHeat = 1_000_000; c.hullHeat = 200_000;
				c.progress = 1e9 - 3e5;
			}));
		}
		for(String cf : new String[] {"coolant", "perfluoromethyl", "mug", "heavywater", "sodium", "lead", "thorium_salt", "water", "none"}) {
			l.add(pwrRun("coolant_" + cf, 60, (c, w) -> {
				load(c, 2, 60); c.flux = 8000; c.rodLevel = 30; c.rodTarget = 30;
				c.tanks[0].setTankType(cf.equals("none") ? Fluids.NONE : fluid(cf)); c.tanks[0].setFill(cf.equals("none") ? 0 : 90_000);
				c.coreHeat = 3_000_000; c.hullHeat = 3_000_000; c.tanks[1].setFill(100_000);
			}));
		}
		l.add(pwrRun("unloaded_chunks", 70, (c, w) -> { w.chunksLoaded = false; load(c, 3, 60); c.coreHeat = 5000; c.hullHeat = 4000; c.tanks[0].setFill(1000); }));
		l.add(pwrRun("unloaded_then_loaded", 70, (c, w) -> { w.chunksLoaded = true; c.unloadDelay = 25; load(c, 3, 60); c.coreHeat = 5000; c.hullHeat = 4000; c.flux = 3000; }));
		l.add(pwrRun("auto_load", 80, (c, w) -> {
			c.slots[0] = new ItemStack(ModItems.pwr_fuel, 70, 4); c.flux = 100; c.rodTarget = 0; c.rodLevel = 100;
		}));
		l.add(pwrRun("auto_load_wrong_type", 20, (c, w) -> {
			c.slots[0] = new ItemStack(ModItems.pwr_fuel, 5, 9); load(c, 4, 10); c.flux = 100;
		}));
		l.add(pwrRun("depletion_stack", 6, (c, w) -> {
			load(c, 7, 12); c.flux = 9e4; c.rodTarget = 0; c.rodLevel = 0; c.progress = 1e9 - 10;
			c.slots[1] = new ItemStack(ModItems.pwr_fuel_hot, 63, 7);
		}));
		l.add(pwrRun("depletion_blocked", 6, (c, w) -> {
			load(c, 7, 12); c.flux = 9e4; c.rodTarget = 0; c.rodLevel = 0; c.progress = 1e9 - 10;
			c.slots[1] = new ItemStack(ModItems.pwr_fuel_hot, 64, 7);
		}));
		l.add(pwrRun("depletion_other_type", 6, (c, w) -> {
			load(c, 7, 12); c.flux = 9e4; c.rodTarget = 0; c.rodLevel = 0; c.progress = 1e9 - 10;
			c.slots[1] = new ItemStack(ModItems.pwr_fuel_hot, 3, 8);
		}));
		l.add(pwrRun("rod_ramp", 140, (c, w) -> { load(c, 1, 60); c.flux = 100; c.rodLevel = 100; c.rodTarget = 0; }));
		l.add(pwrRun("rod_ramp_up", 140, (c, w) -> { load(c, 1, 60); c.flux = 100; c.rodLevel = 0; c.rodTarget = 100; }));
		l.add(pwrRun("rod_fractional_up", 12, (c, w) -> { load(c, 1, 60); c.flux = 100; c.rodLevel = 49.6; c.rodTarget = 50; }));
		l.add(pwrRun("rod_partial_up", 12, (c, w) -> { load(c, 1, 60); c.flux = 100; c.rodLevel = 20.3; c.rodTarget = 24; }));
		for(int hs : new int[] {13, 14, 15, 17}) l.add(pwrRun("heatsinks" + hs, 60, (c, w) -> {
			load(c, 2, 60); c.flux = 8000; c.heatsinkCount = hs; c.channelCount = 40; c.tanks[0].setFill(90_000); c.coreHeat = 3_000_000; c.hullHeat = 3_000_000;
		}));
		l.add(pwrRun("rod_fractional", 12, (c, w) -> { load(c, 1, 60); c.flux = 100; c.rodLevel = 50.4; c.rodTarget = 50; }));
		l.add(pwrRun("meltdown", 6, (c, w) -> {
			load(c, 11, 60); c.flux = 50000; c.coreHeat = c.coreHeatCapacity - 10; c.hullHeat = c.coreHeatCapacity - 10; c.rodTarget = 0; c.rodLevel = 0;
			c.rods.add(new BlockPos(1, 2, 3)); c.rods.add(new BlockPos(4, 2, 3)); c.rods.add(new BlockPos(1, 7, 9)); c.rods.add(new BlockPos(-3, 5, 2));
		}));
		l.add(pwrRun("meltdown_many_rods", 4, (c, w) -> {
			load(c, 0, 60); c.flux = 80000; c.coreHeat = c.coreHeatCapacity * 2; c.rodTarget = 0; c.rodLevel = 0;
			for(int q = 0; q < 37; q++) c.rods.add(new BlockPos(q * 3 - 20, 64 + q % 5, 7 * q - 90));
		}));
		l.add(pwrRun("meltdown_no_rods", 3, (c, w) -> { load(c, 0, 60); c.flux = 80000; c.coreHeat = c.coreHeatCapacity * 2; }));
		l.add(pwrRun("zero_rods", 4, (c, w) -> { c.rodCount = 0; load(c, 0, 0); c.flux = 100; }));
		l.add(pwrRun("not_assembled", 4, (c, w) -> { c.assembled = false; load(c, 0, 5); c.flux = 100; c.coreHeat = 99; }));
		l.add(pwrRun("overfull_load", 4, (c, w) -> { load(c, 2, 200); c.flux = 100; }));
		return arr(l);
	}

	/* ---------------- Hephaestus ---------------- */

	static String hephaestus() {
		List<String> worlds = new ArrayList<>(), l = new ArrayList<>();
		World[] ws = new World[3];
		int mx = 3, mz = -4;
		for(int variant = 0; variant < 3; variant++) {
			Random r = new Random(1000 + variant);
			World w = new World();
			int my = variant == 2 ? 6 : 40;
			for(int k = 0; k < 400; k++) {
				int x = mx + r.nextInt(19) - 9, z = mz + r.nextInt(19) - 9, y = my - 1 - r.nextInt(14) + (r.nextInt(4) == 0 ? -4 : 0);
				int q = r.nextInt(100);
				Block b = q < 60 ? Blocks.lava : q < 70 ? Blocks.flowing_lava : q < 90 ? ModBlocks.volcanic_lava_block : q < 93 && variant == 1 ? ModBlocks.ore_volcano : q < 94 ? ModBlocks.ore_volcano : Blocks.air;
				if(b == ModBlocks.ore_volcano && variant == 0) b = Blocks.lava;
				put(w, x, y, z, b);
			}
			ws[variant] = w;
			worlds.add(obj("machine", ints(mx, my, mz), "blocks", arr(blockList(w))));
		}
		String[] fl = {"water", "oil", "oil_ds", "crackoil", "crackoil_ds", "coolant", "perfluoromethyl", "mug", "blood", "air", "perfluoromethyl_cold", "heavywater", "gas", "sodium", "none"};
		for(int i = 0; i < fl.length; i++) {
			for(int variant = 0; variant < 3; variant++) {
				final String f = fl[i];
				World w = ws[variant];
				int my = variant == 2 ? 6 : 40;
				long time0 = 17 + variant * 13 + i;
				w.time = time0;
				TileEntityMachineHephaestus t = new TileEntityMachineHephaestus();
				t.worldObj = w; t.xCoord = mx; t.yCoord = my; t.zCoord = mz;
				t.input.setTankType(fluid(f)); t.input.setFill(i % 3 == 0 ? 24000 : 5000 + 1371 * i);
				t.output.setFill(variant == 1 ? 23000 : 0);
				List<String> ticks = new ArrayList<>();
				String initial = hephState(t);
				for(int k = 0; k < 30; k++) {
					t.updateEntity();
					ticks.add(hephState(t));
					w.time++;
				}
				l.add(obj("name", s(f + "_v" + variant), "fluid", s(f), "time", time0, "world", variant, "initial", initial, "ticks", arr(ticks)));
			}
		}
		return obj("worlds", arr(worlds), "cases", arr(l));
	}

	static String hephState(TileEntityMachineHephaestus t) {
		return obj("input", ints(t.input.getFill()), "output", ints(t.output.getFill()), "in_type", s(t.input.getTankType().name), "out_type", s(t.output.getTankType().name),
				"heat", ints(t.heatDump()), "fissure", t.fissureDump(), "total_heat", t.getTotalHeat());
	}

	/* ---------------- Flare ---------------- */

	static FluidType flareFluid(Object[] row) {
		FluidType f = new FluidType((String) row[0]);
		if((Boolean) row[1]) f.addTraits(new FT_Gaseous());
		if((Boolean) row[2]) f.addTraits(new FT_Flammable((Integer) row[3]));
		return f;
	}

	static String flareRun(String name, FluidType f, boolean on, boolean burn, int speed, int effect, int fill, long power, boolean tilted, long t0, int ticks) {
		World w = new World();
		TileEntityMachineGasFlare t = new TileEntityMachineGasFlare();
		t.worldObj = w; t.isOn = on; t.doesBurn = burn; t.tilted = tilted;
		t.upgradeManager.speed = speed; t.upgradeManager.effect = effect;
		t.tank.setTankType(f); t.tank.setFill(fill); t.power = power; w.time = t0;
		List<String> tl = new ArrayList<>();
		for(int k = 0; k < ticks; k++) {
			w.log.clear(); t.releases.clear();
			t.updateEntity();
			List<String> ev = new ArrayList<>();
			for(String e : t.releases) ev.add(s(e));
			for(String e : w.log) ev.add(s(e));
			tl.add(obj("fill", t.tank.getFill(), "power", t.power, "fluid_used", t.fluidUsed, "output", t.output, "events", arr(ev)));
			w.time++;
		}
		return obj("name", s(name), "fluid", s(f.name), "on", on, "burn", burn, "speed", speed, "effect", effect, "fill", fill, "power", power, "tilted", tilted, "time", t0, "ticks", arr(tl));
	}

	static String flare() {
		List<String> l = new ArrayList<>();
		int i = 0;
		for(Object[] row : FlareFluids.ROWS) {
			FluidType f = flareFluid(row);
			l.add(flareRun(row[0] + "_vent", f, true, false, 1, 2, 2500, 0, false, 4 + (i % 7), 5));
			l.add(flareRun(row[0] + "_burn", f, true, true, 1, 2, 2500, 99_000, false, 3 + (i % 5), 5));
			i++;
		}
		Object[][] rows = FlareFluids.ROWS;
		Object[] pick = null, gas = null, liquid = null, vent = null;
		for(Object[] row : rows) {
			if((Boolean) row[2] && (Boolean) row[1] && gas == null) gas = row;
			if((Boolean) row[2] && !(Boolean) row[1] && liquid == null) liquid = row;
			if(!(Boolean) row[2] && (Boolean) row[1] && vent == null) vent = row;
		}
		for(Object[] row : new Object[][] {gas, liquid, vent}) {
			for(int sp = 0; sp < 4; sp++) for(int ef = 0; ef < 4; ef++)
				l.add(flareRun(row[0] + "_up" + sp + "_" + ef, flareFluid(row), true, true, sp, ef, 40_000, 5_000, false, 100, 4));
		}
		l.add(flareRun("off", flareFluid(gas), false, true, 0, 0, 2500, 10, false, 5, 3));
		l.add(flareRun("tilted", flareFluid(gas), true, true, 0, 0, 2500, 10, true, 5, 3));
		l.add(flareRun("empty", flareFluid(gas), true, true, 0, 0, 0, 10, false, 5, 3));
		l.add(flareRun("drain", flareFluid(gas), true, true, 0, 0, 25, 99_999, false, 5, 6));
		for(Object[] row : rows) if(row[0].equals("gas")) l.add(flareRun("default_tank", flareFluid(row), true, false, 0, 0, 64000, 0, false, 6, 3));
		return arr(l);
	}
}
