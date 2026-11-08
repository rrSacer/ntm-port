import java.util.*;
import java.nio.file.*;
import net.minecraft.block.Block;
import net.minecraft.item.ItemStack;
import net.minecraft.tileentity.TileEntity;
import net.minecraft.world.World;
import com.hbm.blocks.ModBlocks;
import com.hbm.blocks.machine.rbmk.*;
import com.hbm.handler.neutron.*;
import com.hbm.handler.radiation.ChunkRadiationManager;
import com.hbm.items.ModItems;
import com.hbm.items.machine.ItemRBMKRod;
import com.hbm.tileentity.machine.rbmk.*;
import cpw.mods.fml.common.gameevent.TickEvent;
import com.hbm.inventory.fluid.Fluids;
import com.hbm.inventory.fluid.FluidType;
import com.hbm.inventory.fluid.tank.FluidTank;
import com.hbm.items.machine.ItemFluidIdentifierStub;
import com.hbm.packet.toclient.AuxParticlePacketNT;
import net.minecraft.item.Item;
import net.minecraft.init.Blocks;
import net.minecraft.init.Items;
import net.minecraftforge.oredict.OreDictionary;

/** Runs the scenarios in scenarios.json through the pinned RBMK tile entities and the neutron
 *  handler, one server tick at a time, and prints the whole state after every tick.
 *  The scenario file is input; every number below comes from the Java classes. */
public final class FluxOracle {
    static final int Y = 10;

    // ---- tiny JSON reader (scenario input only)
    static final class P {
        final String s; int i;
        P(String s) { this.s = s; }
        void ws() { while (i < s.length() && Character.isWhitespace(s.charAt(i))) i++; }
        Object v() {
            ws(); char c = s.charAt(i);
            if (c == '{') {
                Map<String, Object> m = new LinkedHashMap<>(); i++; ws();
                if (s.charAt(i) == '}') { i++; return m; }
                while (true) { ws(); String k = (String) v(); ws(); i++; m.put(k, v()); ws(); if (s.charAt(i++) == '}') return m; }
            }
            if (c == '[') {
                List<Object> l = new ArrayList<>(); i++; ws();
                if (s.charAt(i) == ']') { i++; return l; }
                while (true) { l.add(v()); ws(); if (s.charAt(i++) == ']') return l; }
            }
            if (c == '"') { StringBuilder b = new StringBuilder(); i++; while (s.charAt(i) != '"') { if (s.charAt(i) == '\\') i++; b.append(s.charAt(i++)); } i++; return b.toString(); }
            if (s.startsWith("true", i)) { i += 4; return true; }
            if (s.startsWith("false", i)) { i += 5; return false; }
            if (s.startsWith("null", i)) { i += 4; return null; }
            int j = i; while (j < s.length() && "+-.eE0123456789".indexOf(s.charAt(j)) >= 0) j++;
            double d = Double.parseDouble(s.substring(i, j)); i = j; return d;
        }
    }
    @SuppressWarnings("unchecked") static Map<String, Object> m(Object o) { return (Map<String, Object>) o; }
    @SuppressWarnings("unchecked") static List<Object> l(Object o) { return (List<Object>) o; }
    static double d(Map<String, Object> m, String k, double def) { return m.containsKey(k) ? ((Number) m.get(k)).doubleValue() : def; }
    static int n(Map<String, Object> m, String k, int def) { return (int) d(m, k, def); }
    static boolean b(Map<String, Object> m, String k, boolean def) { return m.containsKey(k) ? (Boolean) m.get(k) : def; }

    static Map<String, Object> obj(Object... a) { Map<String, Object> m = new LinkedHashMap<>(); for (int i = 0; i < a.length; i += 2) m.put((String) a[i], a[i + 1]); return m; }
    static Map<String, Object> fp(double v) { return obj("value", Double.isFinite(v) ? (Object) v : Double.toString(v), "bits", String.format(Locale.ROOT, "%016x", Double.doubleToRawLongBits(v))); }

    static final Map<String, Item> probeItems = new HashMap<>();
    static Object field(Class<?> c, String n) { try { return c.getField(n).get(null); } catch (Exception e) { return null; } }
    static Item item(String name) {
        if (probeItems.containsKey(name)) return probeItems.get(name);
        String bare = name.startsWith("minecraft_1_7_10:") ? name.substring(17) : name;
        Object o = name.startsWith("minecraft_1_7_10:") ? null : field(ModItems.class, bare);
        if (o == null) o = field(Blocks.class, bare);
        if (o instanceof Block) return ((Block) o).item;
        if (o == null) o = field(Items.class, bare);
        if (o == null) throw new IllegalArgumentException(name);
        return (Item) o;
    }
    static ItemStack stack(Map<String, Object> m) { return m == null ? null : new ItemStack(item((String) m.get("item")), n(m, "count", 1), n(m, "meta", 0)); }
    static FluidType fluid(String name) { return (FluidType) field(Fluids.class, name); }
    static Object priv(Object o, String f) {
        for (Class<?> c = o.getClass(); c != null; c = c.getSuperclass()) {
            try { java.lang.reflect.Field x = c.getDeclaredField(f); x.setAccessible(true); return x.get(o); } catch (NoSuchFieldException e) { }
            catch (IllegalAccessException e) { throw new RuntimeException(e); }
        }
        throw new IllegalArgumentException(f);
    }
    static Map<String, Object> tank(FluidTank t) { return obj("fluid", t.getTankType().name, "fill", t.getFill()); }
    static void fill(FluidTank t, Map<String, Object> m) { if (m == null) return; if (m.containsKey("fluid")) t.setTankType(fluid((String) m.get("fluid"))); if (m.containsKey("fill")) t.setFill(n(m, "fill", 0)); }

    static ItemRBMKRod rodItem(String variable) throws Exception { return (ItemRBMKRod) ModItems.class.getField(variable).get(null); }

    static Map<String, Object> stackState(ItemStack s) {
        if (s == null) return null;
        Map<String, Object> nbt = new LinkedHashMap<>();
        if (s.hasTagCompound()) for (Map.Entry<String, Object> e : s.stackTagCompound.values.entrySet()) nbt.put(e.getKey(), fp((Double) e.getValue()));
        return obj("item", s.getItem().id, "count", s.stackSize, "meta", s.getItemDamage(), "nbt", s.hasTagCompound() ? nbt : null);
    }

    static Map<String, Object> state(TileEntity t) {
        Map<String, Object> o = obj();
        TileEntityRBMKBase c = (TileEntityRBMKBase) t;
        o.put("heat", fp(c.heat)); o.put("water", c.reasimWater); o.put("steam", c.reasimSteam); o.put("crane", c.craneIndicator);
        if (t instanceof TileEntityRBMKRod) {
            TileEntityRBMKRod r = (TileEntityRBMKRod) t;
            o.put("has_rod", r.hasRod); o.put("flux_q", fp(r.fluxQuantity)); o.put("flux_r", fp(r.fluxFastRatio));
            o.put("last_q", fp(r.lastFluxQuantity)); o.put("last_r", fp(r.lastFluxRatio)); o.put("stack", stackState(r.slots[0]));
        }
        if (t instanceof TileEntityRBMKControl) {
            TileEntityRBMKControl r = (TileEntityRBMKControl) t;
            o.put("level", fp(r.level)); o.put("last_level", fp(r.lastLevel)); o.put("target", fp(r.targetLevel));
            o.put("has_power", r.hasPower); o.put("power", r.power); o.put("mult", fp(r.getMult()));
            if (t instanceof TileEntityRBMKControlManual) o.put("starting", fp(((TileEntityRBMKControlManual) t).startingLevel));
        }
        if (t instanceof TileEntityRBMKOutgasser) {
            TileEntityRBMKOutgasser r = (TileEntityRBMKOutgasser) t;
            o.put("progress", fp(r.progress)); o.put("gas", r.gas.getFill()); o.put("gas_type", r.gas.getTankType().name);
            o.put("in", stackState(r.slots[0])); o.put("out", stackState(r.slots[1]));
        }
        if (t instanceof TileEntityRBMKBoiler) {
            TileEntityRBMKBoiler r = (TileEntityRBMKBoiler) t;
            o.put("feed", tank(r.feed)); o.put("steam_tank", tank(r.steam));
            o.put("consumption", priv(r, "consumption")); o.put("output", priv(r, "output")); o.put("vent_delay", priv(r, "ventDelay"));
        }
        if (t instanceof TileEntityRBMKHeater) {
            TileEntityRBMKHeater r = (TileEntityRBMKHeater) t;
            o.put("feed", tank(r.feed)); o.put("steam_tank", tank(r.steam));
        }
        if (t instanceof TileEntityRBMKCooler) {
            TileEntityRBMKCooler r = (TileEntityRBMKCooler) t;
            o.put("timer", priv(r, "timer")); o.put("cold", tank(r.getAllTanks()[0])); o.put("hot", tank(r.getAllTanks()[1]));
        }
        return o;
    }

    static TileEntity make(World w, Map<String, Object> c) throws Exception {
        String kind = (String) c.get("kind");
        int x = n(c, "x", 0), z = n(c, "z", 0);
        boolean moderated = b(c, "moderated", false);
        int meta = (b(c, "lid", true) ? RBMKBase.DIR_NORMAL_LID : RBMKBase.DIR_NO_LID).ordinal() + RBMKBase.offset;
        TileEntity t; Block blk;
        switch (kind) {
            case "rod": t = new TileEntityRBMKRod(); blk = new RBMKRod("rbmk_rod", moderated); break;
            case "rod_reasim": t = new TileEntityRBMKRodReaSim(); blk = new RBMKRod("rbmk_rod_reasim", moderated); break;
            case "control": t = new TileEntityRBMKControlManual(); blk = new RBMKControl("rbmk_control", moderated); break;
            case "control_reasim": t = new TileEntityRBMKControlManual(); blk = ModBlocks.rbmk_control_reasim; break;
            case "control_auto": t = new TileEntityRBMKControlAuto(); blk = new Block("rbmk_control_auto"); break;
            case "control_reasim_auto": t = new TileEntityRBMKControlAuto(); blk = ModBlocks.rbmk_control_reasim_auto; break;
            case "moderator": t = new TileEntityRBMKModerator(); blk = new Block("rbmk_moderator"); break;
            case "reflector": t = new TileEntityRBMKReflector(); blk = new Block("rbmk_reflector"); break;
            case "absorber": t = new TileEntityRBMKAbsorber(); blk = new Block("rbmk_absorber"); break;
            case "blank": t = new TileEntityRBMKBlank(); blk = new Block("rbmk_blank"); break;
            case "outgasser": t = new TileEntityRBMKOutgasser(); blk = new Block("rbmk_outgasser"); break;
            case "boiler": t = new TileEntityRBMKBoiler(); blk = new Block("rbmk_boiler"); break;
            case "heater": t = new TileEntityRBMKHeater(); blk = new Block("rbmk_heater"); break;
            case "cooler": t = new TileEntityRBMKCooler(); blk = new Block("rbmk_cooler"); break;
            default: throw new IllegalArgumentException(kind);
        }
        blk.opaque = !kind.startsWith("rod"); // RBMKBase.isOpaqueCube is true; RBMKRod overrides it to false
        w.addTile(t, x, Y, z, blk, meta);
        for (int h = 1; h <= RBMKDials.getColumnHeight(w); h++) w.blocks.put(World.key(x, Y + h, z), blk);
        TileEntityRBMKBase base = (TileEntityRBMKBase) t;
        base.heat = d(c, "heat", 20);
        base.reasimWater = n(c, "water", 0); base.reasimSteam = n(c, "steam", 0);
        if (t instanceof TileEntityRBMKRod) {
            TileEntityRBMKRod r = (TileEntityRBMKRod) t;
            if (c.get("rod") != null) r.slots[0] = loadRod(m(c), (String) c.get("rod"));
            if (c.containsKey("flux")) { r.fluxQuantity = ((Number) l(c.get("flux")).get(0)).doubleValue(); r.fluxFastRatio = ((Number) l(c.get("flux")).get(1)).doubleValue(); }
        }
        if (t instanceof TileEntityRBMKControl) {
            TileEntityRBMKControl r = (TileEntityRBMKControl) t;
            r.level = d(c, "level", 0); r.targetLevel = d(c, "target", r.level); r.power = (long) d(c, "power", 0);
            if (t instanceof TileEntityRBMKControlManual) ((TileEntityRBMKControlManual) t).startingLevel = d(c, "starting", 0);
            if (t instanceof TileEntityRBMKControlAuto) {
                TileEntityRBMKControlAuto a = (TileEntityRBMKControlAuto) t;
                a.levelLower = d(c, "level_lower", 0); a.levelUpper = d(c, "level_upper", 0);
                a.heatLower = d(c, "heat_lower", 0); a.heatUpper = d(c, "heat_upper", 0);
                a.function = TileEntityRBMKControlAuto.RBMKFunction.values()[n(c, "function", 0)];
            }
        }
        if (t instanceof TileEntityRBMKOutgasser) {
            TileEntityRBMKOutgasser r = (TileEntityRBMKOutgasser) t;
            r.progress = d(c, "progress", 0);
            r.slots[0] = stack(c.containsKey("in") ? m(c.get("in")) : null); r.slots[1] = stack(c.containsKey("out") ? m(c.get("out")) : null);
            fill(r.gas, c.containsKey("gas") ? m(c.get("gas")) : null);
        }
        if (t instanceof TileEntityRBMKBoiler) {
            TileEntityRBMKBoiler r = (TileEntityRBMKBoiler) t;
            fill(r.feed, c.containsKey("feed") ? m(c.get("feed")) : null); fill(r.steam, c.containsKey("steam_tank") ? m(c.get("steam_tank")) : null);
        }
        if (t instanceof TileEntityRBMKHeater) {
            TileEntityRBMKHeater r = (TileEntityRBMKHeater) t;
            fill(r.feed, c.containsKey("feed") ? m(c.get("feed")) : null); fill(r.steam, c.containsKey("steam_tank") ? m(c.get("steam_tank")) : null);
            if (c.get("identifier") != null) r.slots[0] = new ItemStack(new ItemFluidIdentifierStub(fluid((String) c.get("identifier"))));
        }
        if (t instanceof TileEntityRBMKCooler) {
            TileEntityRBMKCooler r = (TileEntityRBMKCooler) t;
            fill(r.getAllTanks()[0], c.containsKey("cold") ? m(c.get("cold")) : null); fill(r.getAllTanks()[1], c.containsKey("hot") ? m(c.get("hot")) : null);
        }
        return t;
    }

    static ItemStack loadRod(Map<String, Object> c, String variable) throws Exception {
        ItemRBMKRod item = rodItem(variable);
        ItemStack s = new ItemStack(item); item.onCreated(s, null, null);
        if (c.containsKey("yield_fraction")) ItemRBMKRod.setYield(s, item.yield * d(c, "yield_fraction", 1));
        if (c.containsKey("xenon")) ItemRBMKRod.setPoison(s, d(c, "xenon", 0));
        if (c.containsKey("core")) ItemRBMKRod.setCoreHeat(s, d(c, "core", 20));
        if (c.containsKey("hull")) ItemRBMKRod.setHullHeat(s, d(c, "hull", 20));
        return s;
    }

    static Map<String, Object> run(Map<String, Object> sc) throws Exception {
        World w = new World();
        w.rand = new Random((long) d(sc, "seed", 1));
        w.getGameRules().setOrCreateGameRule("dialSaveDials", "true");
        for (Map.Entry<String, Object> e : m(sc.getOrDefault("rules", obj())).entrySet()) w.getGameRules().setOrCreateGameRule(e.getKey(), (String) e.getValue());
        RBMKDials.createDials(w);
        OreDictionary.clearProbes(); probeItems.clear();
        for (Object po : l(sc.getOrDefault("probes", new ArrayList<>()))) {
            Map<String, Object> pr = m(po);
            Item pi = probeItems.computeIfAbsent((String) pr.get("item"), Item::new);
            OreDictionary.addProbe((String) pr.get("ore"), new ItemStack(pi, 1, n(pr, "meta", 0)));
        }
        AuxParticlePacketNT.log.clear();
        RBMKBase.dropLids = true; RBMKBase.digamma = false;
        List<TileEntity> tiles = new ArrayList<>();
        for (Object c : l(sc.get("columns"))) tiles.add(make(w, m(c)));
        int height = RBMKDials.getColumnHeight(w) + 1;
        Block wall = new Block("wall"); wall.opaque = true;
        for (Object o : l(sc.getOrDefault("obstacles", new ArrayList<>()))) {
            Map<String, Object> ob = m(o);
            for (int h = 0; h < n(ob, "height", height); h++) w.setBlock(n(ob, "x", 0), Y + n(ob, "from", 0) + h, n(ob, "z", 0), wall);
        }
        w.log.clear();
        NeutronNodeWorld.removeAllWorlds();
        ChunkRadiationManager.proxy.events.clear();
        NeutronHandler handler = new NeutronHandler();
        List<Object> trace = new ArrayList<>();
        int ticks = n(sc, "ticks", 1);
        Map<Integer, List<Map<String, Object>>> actions = new HashMap<>();
        for (Object a : l(sc.getOrDefault("actions", new ArrayList<>()))) actions.computeIfAbsent(n(m(a), "tick", 0), k -> new ArrayList<>()).add(m(a));
        Object melt = null;
        for (int tick = 0; tick < ticks; tick++) {
            for (Map<String, Object> a : actions.getOrDefault(tick, new ArrayList<>())) {
                TileEntity t = tiles.get(n(a, "column", 0));
                if (a.containsKey("target")) ((TileEntityRBMKControl) t).setTarget(d(a, "target", 0));
                if (a.containsKey("insert_rod")) ((TileEntityRBMKRod) t).slots[0] = loadRod(a, (String) a.get("insert_rod"));
                if (a.containsKey("remove_rod")) ((TileEntityRBMKRod) t).slots[0] = null;
                if (a.containsKey("power")) ((TileEntityRBMKControl) t).power = (long) d(a, "power", 0);
                if (a.containsKey("compress")) ((TileEntityRBMKBoiler) t).cyceCompressor();
                if (a.containsKey("identifier")) ((TileEntityRBMKHeater) t).slots[0] = a.get("identifier") == null ? null : new ItemStack(new ItemFluidIdentifierStub(fluid((String) a.get("identifier"))));
                if (a.containsKey("in")) ((TileEntityRBMKOutgasser) t).slots[0] = stack(m(a.get("in")));
                if (a.containsKey("set_tanks")) {
                    Map<String, Object> st = m(a.get("set_tanks"));
                    if (t instanceof TileEntityRBMKBoiler) { fill(((TileEntityRBMKBoiler) t).feed, st.containsKey("feed") ? m(st.get("feed")) : null); fill(((TileEntityRBMKBoiler) t).steam, st.containsKey("steam") ? m(st.get("steam")) : null); }
                    if (t instanceof TileEntityRBMKHeater) { fill(((TileEntityRBMKHeater) t).feed, st.containsKey("feed") ? m(st.get("feed")) : null); }
                    if (t instanceof TileEntityRBMKCooler) { fill(((TileEntityRBMKCooler) t).getAllTanks()[0], st.containsKey("cold") ? m(st.get("cold")) : null); fill(((TileEntityRBMKCooler) t).getAllTanks()[1], st.containsKey("hot") ? m(st.get("hot")) : null); }
                }
            }
            TickEvent.ServerTickEvent ev = new TickEvent.ServerTickEvent(); ev.phase = TickEvent.Phase.START;
            handler.onServerTick(ev);
            for (TileEntity t : tiles) t.updateEntity();
            w.time++;
            List<Object> cols = new ArrayList<>();
            for (TileEntity t : tiles) cols.add(state(t));
            List<Object> rad = new ArrayList<>();
            for (float[] r : ChunkRadiationManager.proxy.events) rad.add(Arrays.asList((int) r[0], (int) r[2], String.format("%08x", Float.floatToRawIntBits(r[3]))));
            ChunkRadiationManager.proxy.events.clear();
            boolean meltdown = !w.log.isEmpty();
            List<Object> fx = new ArrayList<>();
            for (double[] p : AuxParticlePacketNT.log) fx.add(Arrays.asList("particle", p[0], p[2]));
            AuxParticlePacketNT.log.clear();
            for (String f : w.fx) fx.add(Arrays.asList(f.split(",")));
            for (int[] sc2 : com.hbm.saveddata.satellites.SatelliteRayScan.log) fx.add(Arrays.asList("scan", sc2[0], sc2[1], sc2[2]));
            com.hbm.saveddata.satellites.SatelliteRayScan.log.clear();
            w.fx.clear();
            trace.add(obj("tick", tick, "cols", cols, "rad", rad, "fx", fx, "meltdown", meltdown));
            if (meltdown) { melt = tick; break; }
        }
        return obj("name", sc.get("name"), "meltdown_tick", melt, "trace", trace);
    }

    public static void main(String[] args) throws Exception {
        Object root = new P(new String(Files.readAllBytes(Paths.get(args[0])), "UTF-8")).v();
        new com.hbm.inventory.recipes.OutgasserRecipes().registerDefaults();
        List<Object> out = new ArrayList<>();
        for (Object sc : l(m(root).get("scenarios"))) out.add(run(m(sc)));
        System.out.println(Oracle.json(obj("schema", 1, "scenarios", m(root).get("scenarios"), "results", out)));
    }
}
