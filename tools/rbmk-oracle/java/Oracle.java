import java.lang.reflect.*;
import java.util.*;
import com.hbm.items.ModItems;
import com.hbm.items.machine.ItemRBMKRod;
import com.hbm.items.machine.ItemRBMKRod.*;
import com.hbm.items.machine.ItemRBMKPellet;
import com.hbm.tileentity.machine.rbmk.RBMKDials;
import com.hbm.tileentity.machine.rbmk.RBMKDials.RBMKKeys;
import com.hbm.tileentity.machine.rbmk.IRBMKFluxReceiver.NType;
import com.hbm.config.GeneralConfig;
import com.hbm.inventory.RecipesCommon.*;
import com.hbm.inventory.recipes.SILEXRecipes;
import com.hbm.inventory.recipes.SILEXRecipes.SILEXRecipe;
import com.hbm.inventory.recipes.OutgasserRecipes;
import com.hbm.inventory.recipes.OutgasserRecipes.OutgasserRecipe;
import com.hbm.util.WeightedRandomObject;
import net.minecraft.item.*;
import net.minecraft.nbt.NBTTagCompound;
import net.minecraft.world.World;

/** Calls the real HBM classes; no replacement RBMK equations live here. */
public final class Oracle {
    static Map<String,Object> obj(Object... a) {
        Map<String,Object> m=new LinkedHashMap<>();
        for(int i=0;i<a.length;i+=2)m.put((String)a[i],a[i+1]);
        return m;
    }
    static Object finite(double d){return Double.isFinite(d)?d:Double.toString(d);}
    static Map<String,Object> fp(double d){return obj("value",finite(d),"bits",String.format(Locale.ROOT,"%016x",Double.doubleToRawLongBits(d)));}
    static Map<String,Object> stack(ItemStack s) {
        return obj("item",s.getItem().id,"count",s.stackSize,"meta",s.getItemDamage(),
                   "nbt",s.hasTagCompound()?new TreeMap<>(s.stackTagCompound.values):null);
    }
    static Map<String,Object> stats(ItemRBMKRod r) {
        return obj("full_name",r.fullName,"pellet",r.pellet==null?null:r.pellet.id,
            "yield",r.yield,"func_end",r.reactivity,"self_rate",r.selfRate,
            "function",r.function.name(),"depletion_function",r.depFunc.name(),
            "xenon_generation",r.xGen,"xenon_burn_divisor",r.xBurn,
            "heat_per_flux",r.heat,"melting_point",r.meltingPoint,"diffusion",r.diffusion,
            "heat_coefficient",obj("start",r.heatCoeffStart,"length",r.heatCoeffLength),
            "neutron_types",obj("absorbs",r.nType.name(),"releases",r.rType.name()),
            "tint",r.colorTint,"special_flux_curve",r.specialFluxCurve,
            "max_stack_size",r.maxStackSize,"container_item",r.container.id);
    }
    static List<Object> tooltip(ItemRBMKRod r,ItemStack s) {
        List<Object> t=new ArrayList<>();r.addInformation(s,null,t,false);return t;
    }
    static World world() {
        World w=new World();
        for(RBMKKeys k:RBMKKeys.values())w.getGameRules().setOrCreateGameRule(k.keyString,k.defValue.toString());
        RBMKDials.createDials(w);return w;
    }
    static List<Object> rods()throws Exception {
        List<Object> list=new ArrayList<>();
        for(Field f:ModItems.class.getFields())if(f.getType()==ItemRBMKRod.class) {
            ItemRBMKRod r=(ItemRBMKRod)f.get(null);
            Map<String,Object> data=stats(r);data.put("variable",f.getName());data.put("item","ntm:"+r.id);
            ItemStack fresh=new ItemStack(r);r.onCreated(fresh,world(),null);
            data.put("fresh_stack",stack(fresh));data.put("fresh_tooltip_keys",tooltip(r,fresh));
            data.put("fresh_function_description",r.getFuncDescription(fresh));
            data.put("fresh_durability",fp(r.getDurabilityForDisplay(fresh)));
            ItemStack used=fresh.copy();ItemRBMKRod.setYield(used,r.yield*0.2);
            ItemRBMKRod.setPoison(used,50);ItemRBMKRod.setCoreHeat(used,500);ItemRBMKRod.setHullHeat(used,200);
            data.put("used_stack",stack(used));data.put("used_tooltip_keys",tooltip(r,used));
            List<Object> trace=new ArrayList<>();World w=world();
            for(int tick=0;tick<4;tick++) {
                double out=r.burn(w,used,10);r.updateHeat(w,used,1);
                double transferred=r.provideHeat(w,used,20,1);
                trace.add(obj("tick",tick,"out_flux",fp(out),"provided_heat",fp(transferred),"stack",stack(used)));
            }
            data.put("burn_heat_trace",trace);list.add(data);
        }
        return list;
    }
    static List<Object> pellets()throws Exception {
        List<Object> list=new ArrayList<>();
        for(Field f:ModItems.class.getFields())if(f.getType()==ItemRBMKPellet.class) {
            ItemRBMKPellet p=(ItemRBMKPellet)f.get(null);
            List<ItemStack> sub=new ArrayList<>();p.getSubItems(p,null,sub);
            list.add(obj("variable",f.getName(),"item","ntm:"+p.id,"full_name",p.fullName,
                "xenon_enabled",p.isXenonEnabled(),"creative_metadata",sub.stream().map(ItemStack::getItemDamage).toArray()));
        }
        return list;
    }
    static Map<String,Object> functions() {
        List<Object> burn=new ArrayList<>(),depletion=new ArrayList<>();
        ItemRBMKRod r=new ItemRBMKRod("probe").setStats(20,10).setDepletionFunction(EnumDepleteFunc.LINEAR);
        for(EnumBurnFunc b:EnumBurnFunc.values()) {
            r.setFunction(b);
            for(double e:new double[]{0,0.25,0.5,1})for(double in:new double[]{-1,0,0.1,1,9,10,50,100,1000,10000})
                burn.add(obj("function",b.name(),"influx",in,"enrichment",e,"reactivity",r.reactivity,
                    "self_rate",r.selfRate,"depletion_function",r.depFunc.name(),"result",fp(r.reactivityFunc(in,e))));
        }
        for(EnumDepleteFunc d:EnumDepleteFunc.values()) {
            r.setDepletionFunction(d);
            for(double e:new double[]{-0.1,0,0.25,0.5,0.75,1,1.1})
                depletion.add(obj("function",d.name(),"enrichment",e,"result",fp(r.reactivityModByEnrichment(e))));
        }
        return obj("burn",burn,"depletion",depletion);
    }
    static Map<String,Object> defaultsAndSetters() {
        Map<String,Object> defaults=stats(new ItemRBMKRod("default"));
        List<Object> setters=new ArrayList<>();ItemRBMKRod r=new ItemRBMKRod("setters");
        setters.add(obj("method","setStats(double,double)","same_instance",r.setStats(31,7)==r,"state",stats(r)));
        setters.add(obj("method","setStats(double)","same_instance",r.setStats(19)==r,"state",stats(r)));
        setters.add(obj("method","setYield","same_instance",r.setYield(12345)==r,"state",stats(r)));
        setters.add(obj("method","setFunction","same_instance",r.setFunction(EnumBurnFunc.QUADRATIC)==r,"state",stats(r)));
        setters.add(obj("method","setDepletionFunction","same_instance",r.setDepletionFunction(EnumDepleteFunc.STATIC)==r,"state",stats(r)));
        setters.add(obj("method","setXenon","same_instance",r.setXenon(0.125,37)==r,"state",stats(r)));
        setters.add(obj("method","setHeat","same_instance",r.setHeat(0.75)==r,"state",stats(r)));
        setters.add(obj("method","setDiffusion","same_instance",r.setDiffusion(0.125)==r,"state",stats(r)));
        setters.add(obj("method","setMeltingPoint","same_instance",r.setMeltingPoint(1350)==r,"state",stats(r)));
        setters.add(obj("method","setHeatCoeff","same_instance",r.setHeatCoeff(800,400)==r,"state",stats(r)));
        setters.add(obj("method","setTint","same_instance",r.setTint(0x123456)==r,"state",stats(r)));
        setters.add(obj("method","setNeutronTypes","same_instance",r.setNeutronTypes(NType.ANY,NType.SLOW)==r,"state",stats(r)));
        setters.add(obj("method","setFluxCurve","same_instance",r.setFluxCurve(true)==r,"state",stats(r)));
        // Probe callbacks are inputs, not replacements for the HBM wrappers.
        boolean sameRatio=r.setOutputRatioCurve(x->x*2)==r;
        setters.add(obj("method","setOutputRatioCurve","same_instance",sameRatio,"ratio",fp(r.fluxRatioOut(0.75,1))));
        boolean sameFlux=r.setOutputFluxCurve((q,ratio)->q*ratio)==r;
        setters.add(obj("method","setOutputFluxCurve","same_instance",sameFlux,"flux",fp(r.fluxFromRatio(100,0.75))));
        boolean sameDepletion=r.setDepletionOutputRatioCurve((x,e)->x*e)==r;
        setters.add(obj("method","setDepletionOutputRatioCurve","same_instance",sameDepletion,"ratio",fp(r.fluxRatioOut(0.75,0.5))));
        ItemStack absent=new ItemStack(ModItems.rbmk_fuel_ueu);ItemRBMKRod.getYield(absent);
        ItemStack empty=new ItemStack(ModItems.rbmk_fuel_ueu);empty.stackTagCompound=new NBTTagCompound();
        Map<String,Object> emptyReads=obj("yield",ItemRBMKRod.getYield(empty),"xenon",ItemRBMKRod.getPoison(empty),"core",ItemRBMKRod.getCoreHeat(empty),"hull",ItemRBMKRod.getHullHeat(empty));
        ItemStack foreign=absent.copy();foreign.stackTagCompound.setDouble("foreign",123);ItemRBMKRod.setPoison(foreign,50);
        return obj("rod",defaults,"setters",setters,"nbt_absent_after_get",stack(absent),
                   "nbt_existing_empty_reads",emptyReads,"nbt_preserved_by_setter",stack(foreign));
    }
    static List<Object> discriminants() {
        List<Object> list=new ArrayList<>();World w=world();
        ItemRBMKRod r=new ItemRBMKRod("discriminant").setYield(1000).setStats(20).setFunction(EnumBurnFunc.LOG_TEN).setDepletionFunction(EnumDepleteFunc.LINEAR);
        list.add(obj("name","log_ten_9_20","result",fp(r.reactivityFunc(9,1))));
        r.setFunction(EnumBurnFunc.LINEAR);ItemStack s=new ItemStack(r);r.onCreated(s,w,null);ItemRBMKRod.setPoison(s,50);
        double out=r.burn(w,s,10);
        list.add(obj("name","xenon_uses_old_poison","out_flux",fp(out),"stack",stack(s)));
        for(double hull:new double[]{999,1000,1001}) {
            s=new ItemStack(r);r.onCreated(s,w,null);ItemRBMKRod.setHullHeat(s,hull);ItemRBMKRod.setCoreHeat(s,2000);
            double heat=r.provideHeat(w,s,100,1);
            list.add(obj("name","hull_melting_point","hull_before",hull,"provided",fp(heat),"stack",stack(s)));
        }
        r.setHeatCoeff(1000,500);
        for(double core:new double[]{999,1000,1250,1500,1501,Double.NaN,Double.POSITIVE_INFINITY}) {
            s=new ItemStack(r);r.onCreated(s,w,null);ItemRBMKRod.setCoreHeat(s,core);
            double flux=r.burn(w,s,10);
            list.add(obj("name","heat_coefficient_rectify","core_before",fp(core),"out_flux",fp(flux),"stack",stack(s)));
        }
        for(String key:new String[]{"dialDisableDepletion","dialDisableXenon"}) {
            w=world();w.getGameRules().setOrCreateGameRule(key,"true");RBMKDials.refresh(w);
            s=new ItemStack(r);r.onCreated(s,w,null);ItemRBMKRod.setPoison(s,50);
            out=r.burn(w,s,10);list.add(obj("name",key,"out_flux",fp(out),"stack",stack(s)));
        }
        return list;
    }
    static List<Object> dials()throws Exception {
        List<Object> list=new ArrayList<>();
        for(RBMKKeys k:RBMKKeys.values()) {
            World w=world();Object cached=k==RBMKKeys.KEY_SAVE_DIALS?null:RBMKDials.getGameRule(w,k);
            List<Object> probes=new ArrayList<>();
            for(String v:new String[]{"-5","0","0.5","1","1000","bad","NaN","Infinity","true","false"}) {
                w=world();w.getGameRules().setOrCreateGameRule(k.keyString,v);RBMKDials.refresh(w);
                Object effective=k==RBMKKeys.KEY_SAVE_DIALS?w.getGameRules().getGameRuleBooleanValue(k.keyString):RBMKDials.getGameRule(w,k);
                probes.add(obj("input",v,"effective",effective instanceof Double?fp((Double)effective):effective));
            }
            list.add(obj("key",k.name(),"game_rule",k.keyString,"default",k.defValue,
                "default_java_type",k.defValue.getClass().getSimpleName(),"cached_effective",cached,"probes",probes));
        }
        return list;
    }
    static List<Object> dialSpecialCases() {
        List<Object> list=new ArrayList<>();
        World w=world();w.getGameRules().setOrCreateGameRule("dialControlSurgeMod","7");
        w.getGameRules().setOrCreateGameRule("dialPassiveCooling","3");RBMKDials.refresh(w);
        double server=RBMKDials.getSurgeMod(w);w.isRemote=true;
        list.add(obj("name","client_surge_reads_passive_cooling","server",server,"client",RBMKDials.getSurgeMod(w)));
        for(boolean enabled:new boolean[]{false,true}) {
            w=world();GeneralConfig.enable528ReasimBoilers=enabled;RBMKDials.refresh(w);
            list.add(obj("name","enable528ReasimBoilers","config",enabled,"effective",RBMKDials.getReasimBoilers(w)));
        }
        GeneralConfig.enable528ReasimBoilers=false;
        w=world();w.getGameRules().setOrCreateGameRule("dialColumnHeight","16");RBMKDials.createDials(w,true);
        list.add(obj("name","create_force_reset_cache_before_write","raw",w.getGameRules().getGameRuleStringValue("dialColumnHeight"),"cached",RBMKDials.getColumnHeight(w)));
        RBMKDials.refresh(w);list.add(obj("name","after_refresh","cached",RBMKDials.getColumnHeight(w)));
        return list;
    }
    static Map<String,Object> silex(SILEXRecipe r) {
        List<Object> outputs=new ArrayList<>();
        for(WeightedRandomObject o:r.outputs)outputs.add(obj("stack",stack(o.asStack()),"weight",o.itemWeight));
        return obj("fluid_produced",r.fluidProduced,"fluid_consumed",r.fluidConsumed,
            "laser_strength",r.laserStrength.name(),"laser_index",r.laserStrength.ordinal(),"outputs",outputs);
    }
    static Object ingredient(Object k) {
        if(k instanceof ComparableStack)return obj("stack",stack(((ComparableStack)k).toStack()));
        if(k instanceof OreDictStack)return obj("ore",((OreDictStack)k).ore);
        return obj("ore",k.toString());
    }
    static Map<String,Object> recipes()throws Exception {
        SILEXRecipes.register();new OutgasserRecipes().registerDefaults();
        Field field=SILEXRecipes.class.getDeclaredField("recipes");field.setAccessible(true);
        Map<Object,SILEXRecipe> registered=(Map<Object,SILEXRecipe>)field.get(null);
        List<Object> silex=new ArrayList<>(),outgasser=new ArrayList<>(),tiny=new ArrayList<>();
        for(Map.Entry<Object,SILEXRecipe> e:registered.entrySet())
            silex.add(obj("input",ingredient(e.getKey()),"recipe",silex(e.getValue())));
        for(Map.Entry<AStack,OutgasserRecipe> e:OutgasserRecipes.recipes.entrySet()) {
            OutgasserRecipe r=e.getValue();
            outgasser.add(obj("input",ingredient(e.getKey()),"solid",r.solidOutput==null?null:stack(r.solidOutput),
                "fluid",r.liquidOutput==null?null:obj("type",r.liquidOutput.type.name,"amount",r.liquidOutput.fill),"fusion_only",r.fusionOnly));
        }
        for(Field f:ModItems.class.getFields())if(f.getName().matches("nuclear_waste_(short|long)(_depleted)?_tiny")) {
            Item item=(Item)f.get(null);
            for(int meta=0;meta<8;meta++) {
                SILEXRecipe r=SILEXRecipes.getOutput(new ItemStack(item,1,meta));
                if(r!=null)tiny.add(obj("input",stack(new ItemStack(item,1,meta)),"recipe",silex(r)));
            }
        }
        Comparator<Object> order=Comparator.comparing(Oracle::json);
        silex.sort(order);outgasser.sort(order);tiny.sort(order);
        List<Object> missing=new ArrayList<>();
        for(Field f:ModItems.class.getFields())if(f.getType()==ItemRBMKPellet.class) {
            Item item=(Item)f.get(null);List<Integer> absent=new ArrayList<>();
            for(int meta=0;meta<10;meta++)if(SILEXRecipes.getOutput(new ItemStack(item,1,meta))==null)absent.add(meta);
            if(!absent.isEmpty())missing.add(obj("pellet",item.id,"metadata",absent));
        }
        return obj("silex_registered",silex,"silex_tiny_lookup",tiny,"silex_missing_pellet_states",missing,"outgasser_defaults",outgasser);
    }
    static String json(Object o) {
        if(o==null)return "null";
        if(o instanceof String || o instanceof Enum) {
            StringBuilder s=new StringBuilder("\"");
            for(char c:o.toString().toCharArray())switch(c) {
                case '"':s.append("\\\"");break;case '\\':s.append("\\\\");break;
                case '\n':s.append("\\n");break;case '\r':s.append("\\r");break;case '\t':s.append("\\t");break;
                default:if(c<32)s.append(String.format("\\u%04x",(int)c));else s.append(c);
            }
            return s.append('"').toString();
        }
        if(o instanceof Double&&!Double.isFinite((Double)o))return json(o.toString());
        if(o instanceof Number||o instanceof Boolean)return o.toString();
        if(o instanceof Map) {
            List<String> parts=new ArrayList<>();
            for(Object x:((Map<?,?>)o).entrySet()){Map.Entry<?,?> e=(Map.Entry<?,?>)x;parts.add(json(e.getKey())+":"+json(e.getValue()));}
            return "{"+String.join(",",parts)+"}";
        }
        if(o.getClass().isArray())return json(Arrays.asList((Object[])o));
        if(o instanceof Iterable) {List<String> p=new ArrayList<>();for(Object x:(Iterable<?>)o)p.add(json(x));return "["+String.join(",",p)+"]";}
        throw new IllegalArgumentException(o.getClass().getName());
    }
    public static void main(String[] args)throws Exception {
        Map<String,Object> root=obj("schema",1,"rods",rods(),"pellets",pellets(),
            "defaults",defaultsAndSetters(),"functions",functions(),"discriminants",discriminants(),
            "dials",dials(),"dial_special_cases",dialSpecialCases(),"recipes",recipes());
        System.out.println(json(root));
    }
}
