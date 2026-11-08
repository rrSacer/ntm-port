//! Parity of the Watz / PWR / Hephaestus / flare domain against the Java oracle
//! (`tools/watz-pwr-oracle`): the fixture holds the inputs and the values the original
//! `TileEntity*` methods produced, tick by tick. Doubles travel as raw IEEE bits.
use ntm_core::petroleo_e::*;
use ntm_core::watz_pwr::heatable::{heatable, HEATABLES};
use ntm_core::watz_pwr::pwr::*;
use ntm_core::watz_pwr::watz::*;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

fn fx() -> &'static Value {
    static F: OnceLock<Value> = OnceLock::new();
    F.get_or_init(|| {
        let text = include_str!("../../../tools/watz-pwr-oracle/fixtures/java.json");
        serde_json::from_str::<Value>(text).expect("fixture")["data"].clone()
    })
}

fn bits(v: &Value) -> f64 {
    f64::from_bits(u64::from_str_radix(v.as_str().expect("bits"), 16).expect("hex"))
}
fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}
fn i(v: &Value) -> i64 {
    v.as_i64().expect("int")
}
fn arr(v: &Value) -> &Vec<Value> {
    v.as_array().expect("array")
}
fn s(v: &Value) -> &str {
    v.as_str().expect("string")
}
/// Leaks a fixture string: fluid names in the domain are `&'static str`.
fn st(v: &Value) -> &'static str {
    Box::leak(s(v).to_string().into_boxed_str())
}

#[test]
fn pellet_constants_and_curves_match_java() {
    for row in arr(&fx()["functions"]["watz"]) {
        let t = WatzType::from_meta(i(&row["ordinal"]) as i32);
        assert_eq!(format!("{t:?}").to_uppercase(), s(&row["name"]));
        let spec = t.spec();
        assert_eq!(spec.color_light as i64, i(&row["color_light"]));
        assert_eq!(spec.color_dark as i64, i(&row["color_dark"]));
        assert!(same(spec.passive, bits(&row["passive"])), "{t:?} passive");
        assert!(same(spec.heat_emission, bits(&row["heat_emission"])));
        assert!(same(spec.mud_content, bits(&row["mud_content"])), "{t:?} mud");
        assert!(same(spec.yield_, bits(&row["yield"])));
        let fns = row["fn"].as_object().unwrap();
        for (key, f) in [("burn", spec.burn), ("heat_div", spec.heat_div), ("absorb", spec.absorb)] {
            assert_eq!(f.is_some(), fns.contains_key(key), "{t:?} {key}");
            if let Some(f) = f {
                for pair in arr(&fns[key]) {
                    let (x, y) = (bits(&pair[0]), bits(&pair[1]));
                    assert!(same(f.effonix(x), y), "{t:?} {key}({x}) = {} java {y}", f.effonix(x));
                }
            }
        }
    }
}

#[test]
fn pwr_fuels_match_java_including_the_ignored_yield() {
    let rows = arr(&fx()["functions"]["pwr"]);
    assert_eq!(rows.len(), 15);
    for row in rows {
        let f = PwrFuel::from_meta(i(&row["ordinal"]) as i32);
        assert_eq!(format!("{f:?}").to_uppercase(), s(&row["name"]).replace('_', ""));
        assert!(same(f.heat_emission(), bits(&row["heat_emission"])));
        // the Java constructor ignores its yield argument, so the real value is always 1e9
        assert!(same(f.yield_(), bits(&row["yield"])));
        assert_eq!(f.yield_(), 1_000_000_000.0);
        for pair in arr(&row["fn"]) {
            let (x, y) = (bits(&pair[0]), bits(&pair[1]));
            assert!(same(f.function().effonix(x), y), "{f:?}({x}) = {} java {y}", f.function().effonix(x));
        }
    }
    assert_eq!(PwrFuel::BfbAmMix.declared_yield(), Some(250_000_000.0));
    assert_eq!(PwrFuel::BfbPu241.declared_yield(), Some(250_000_000.0));
    assert_eq!(PwrFuel::Meu.declared_yield(), None);
}

#[test]
fn reactor_math_helpers_match_java() {
    for pair in arr(&fx()["functions"]["connectin"]) {
        let (x, y) = (bits(&pair[0]), bits(&pair[1]));
        assert!(same(connectin_func(x), y), "connectin({x})");
    }
    for t in arr(&fx()["functions"]["x_over_e"]) {
        let (x, d, y) = (bits(&t[0]), bits(&t[1]), bits(&t[2]));
        assert!(same(get_x_over_e(x, d), y), "xOverE({x},{d})");
    }
}

#[test]
fn heatable_table_matches_the_registered_traits() {
    let rows = arr(&fx()["fluids"]);
    assert_eq!(rows.len(), HEATABLES.len());
    for r in rows {
        let h = heatable(s(&r["name"])).unwrap_or_else(|| panic!("missing {}", r["name"]));
        assert_eq!(h.hot, s(&r["hot"]));
        assert_eq!(h.heat_req as i64, i(&r["heat_req"]));
        assert_eq!(h.amount_req as i64, i(&r["amount_req"]));
        assert_eq!(h.amount_produced as i64, i(&r["amount_produced"]));
        assert!(same(h.eff_boiler, bits(&r["eff_boiler"])));
        assert!(same(h.eff_heatexchanger, bits(&r["eff_heatexchanger"])));
        assert!(same(h.eff_pwr, bits(&r["eff_pwr"])));
        assert_eq!(h.moderator.map(f64::to_bits), r["moderator"].as_str().map(|_| bits(&r["moderator"]).to_bits()));
    }
}

// ------------------------------------------------------------------ Watz

fn watz_segment(v: &Value) -> WatzSegment {
    let mut seg = WatzSegment {
        heat: i(&v["heat"]) as i32,
        flux_last_base: bits(&v["flux_base"]),
        flux_last_reaction: bits(&v["flux_reaction"]),
        tanks: [0; 3],
        ..Default::default()
    };
    for (n, t) in arr(&v["tanks"]).iter().enumerate() {
        seg.tanks[n] = i(t) as i32;
    }
    for slot in arr(&v["slots"]) {
        let y = bits(&slot[4]);
        seg.slots[i(&slot[0]) as usize] = Some(PelletStack {
            depleted: i(&slot[1]) == 1,
            meta: i(&slot[2]) as i32,
            count: i(&slot[3]) as i32,
            yield_: (y.to_bits() != (-1.0f64).to_bits()).then_some(y),
        });
    }
    seg
}

fn assert_watz_eq(seg: &WatzSegment, v: &Value, ctx: &str) {
    let want = watz_segment(v);
    assert_eq!(seg.heat, want.heat, "{ctx} heat");
    assert!(same(seg.flux_last_base, want.flux_last_base), "{ctx} flux base");
    assert!(same(seg.flux_last_reaction, want.flux_last_reaction), "{ctx} flux reaction");
    assert_eq!(seg.tanks, want.tanks, "{ctx} tanks");
    for n in 0..SLOTS {
        match (seg.slots[n], want.slots[n]) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                assert_eq!((a.depleted, a.meta, a.count), (b.depleted, b.meta, b.count), "{ctx} slot {n}");
                match (a.yield_, b.yield_) {
                    (None, None) => {}
                    (Some(x), Some(y)) => assert!(same(x, y), "{ctx} slot {n} yield {x} vs {y}"),
                    other => panic!("{ctx} slot {n} yield tag {other:?}"),
                }
            }
            other => panic!("{ctx} slot {n}: {other:?}"),
        }
    }
}

#[test]
fn watz_columns_match_java_tick_by_tick() {
    let scenarios = arr(&fx()["watz"]);
    assert_eq!(scenarios.len(), 19);
    let mut overflows = 0;
    for sc in scenarios {
        let name = s(&sc["name"]);
        let mut segs: Vec<WatzSegment> = arr(&sc["initial"]).iter().map(watz_segment).collect();
        let pump = sc["pump"].as_bool().unwrap();
        let power = i(&sc["power"]) as i32;
        for (k, tick) in arr(&sc["ticks"]).iter().enumerate() {
            let ev = tick_stack(&mut segs, pump, power);
            let log: Vec<&str> = arr(&tick["log"]).iter().map(s).collect();
            assert_eq!(ev.is_some(), log.contains(&"disassemble"), "{name} tick {k} overflow");
            if ev.is_some() {
                overflows += 1;
                assert!(log.iter().any(|l| l.starts_with("rad ") && l.ends_with(" 1000.0")), "{name} radiation");
                assert_eq!(log.iter().filter(|l| l.ends_with(" air")).count(), 7 * 3 * 7);
            }
            for (n, seg) in segs.iter().enumerate() {
                assert_watz_eq(seg, &tick["segments"][n], &format!("{name} tick {k} seg {n}"));
            }
        }
    }
    assert!(overflows >= 2, "overflow scenarios must exercise the failure path");
}

#[test]
fn watz_extras_match_java() {
    let x = &fx()["watz_extra"];
    let ports: Vec<(i32, i32, i32, bool)> = arr(&x["ports"]).iter().map(|p| (i(&p[0]) as i32, i(&p[1]) as i32, i(&p[2]) as i32, i(&p[3]) == 0)).collect();
    assert_eq!(ports, PORTS.to_vec());
    let conv = &arr(&x["conversions"])[0];
    assert_eq!((s(&conv["tool"]), s(&conv["block"]), i(&conv["meta"])), ("bolt", "watz_end", 0));
    assert_eq!((s(&conv["ore"]), i(&conv["count"]), i(&conv["to_meta"])), ("bolt_dura", 4, 1));
    assert_eq!(bolt_casing(0), Some((1, 4)));
    assert_eq!(bolt_casing(1), None);
    let drops: Vec<String> = BREAK_DROPS
        .iter()
        .flat_map(|(n, c)| std::iter::repeat((*n, *c)).take(if *n == "bolt_dura" { BREAK_BOLT_STACKS as usize } else { 1 }))
        .map(|(n, c)| format!("drop {n} {c}"))
        .collect();
    let java: Vec<String> = arr(&x["break_drops"]).iter().map(|l| s(l).to_string()).filter(|l| l.starts_with("drop ")).collect();
    assert_eq!(drops, java);
    assert!(arr(&x["break_no_drops"]).iter().all(|l| !s(l).starts_with("drop ")));
}

fn runs(v: &Value) -> HashMap<(i32, i32, i32), String> {
    let mut m = HashMap::new();
    for r in arr(v) {
        for dx in 0..i(&r[3]) {
            m.insert(((i(&r[0]) + dx) as i32, i(&r[1]) as i32, i(&r[2]) as i32), s(&r[4]).to_string());
        }
    }
    m
}

#[test]
fn watz_structure_check_matches_java() {
    let list = arr(&fx()["watz_extra"]["structures"]);
    let (mut formed, mut broken) = (0, 0);
    for sc in list {
        let blocks = runs(&sc["blocks"]);
        // metas are recorded as [x, y, z, meta]
        let meta_of = |p: (i32, i32, i32)| -> i32 {
            arr(&sc["metas"]).iter().find(|m| (i(&m[0]) as i32, i(&m[1]) as i32, i(&m[2]) as i32) == p).map_or(0, |m| i(&m[3]) as i32)
        };
        let at = |x: i32, y: i32, z: i32| match blocks.get(&(x, y, z)).map(String::as_str) {
            None => StructBlock::Air,
            Some("watz_cooler") => StructBlock::WatzCooler,
            Some("watz_element") => StructBlock::WatzElement,
            Some("watz_end") => StructBlock::WatzEnd { meta: meta_of((x, y, z)) },
            Some(_) => StructBlock::Other,
        };
        let time = i(&sc["time"]);
        let java_formed = !arr(&sc["log"]).is_empty();
        // the source only checks every 20 ticks
        let ours = time % 20 == 0 && structure_complete(&at);
        assert_eq!(ours, java_formed, "structure {} (mode {}, hit {})", sc["id"], sc["mode"], sc["hit"]);
        if java_formed { formed += 1 } else { broken += 1 }
    }
    assert!(formed >= 1 && broken >= 40);
}

// ------------------------------------------------------------------ PWR

fn pwr_block(name: &str) -> PwrBlock {
    match name {
        "pwr_fuel" => PwrBlock::Fuel,
        "pwr_control" => PwrBlock::Control,
        "pwr_channel" => PwrBlock::Channel,
        "pwr_heatex" => PwrBlock::Heatex,
        "pwr_heatsink" => PwrBlock::Heatsink,
        "pwr_neutron_source" => PwrBlock::NeutronSource,
        "pwr_casing" => PwrBlock::Casing,
        "pwr_reflector" => PwrBlock::Reflector,
        "pwr_port" => PwrBlock::Port,
        "pwr_controller" => PwrBlock::Controller,
        _ => PwrBlock::Air,
    }
}

fn p3(v: &Value) -> Pos3 {
    (i(&v[0]) as i32, i(&v[1]) as i32, i(&v[2]) as i32)
}

#[test]
fn pwr_structure_scan_matches_java() {
    let mut ok = 0;
    for sc in arr(&fx()["pwr_structure"]) {
        let name = s(&sc["name"]);
        let blocks = runs(&sc["blocks"]);
        let world = |p: Pos3| blocks.get(&p).map_or(PwrBlock::Air, |n| pwr_block(n));
        let a = assemble(&world, p3(&sc["controller"]), i(&sc["meta"]) as i32);
        let errs: Vec<&str> = arr(&sc["errors"]).iter().map(s).collect();
        assert_eq!(a.errors, errs, "{name} errors");
        assert_eq!(a.assembled, sc["assembled"].as_bool().unwrap(), "{name}");
        if let Some(su) = &a.setup {
            ok += 1;
            assert_eq!(su.rod_count as i64, i(&sc["rod_count"]), "{name}");
            assert_eq!(su.connections as i64, i(&sc["connections"]), "{name}");
            assert_eq!(su.connections_controlled as i64, i(&sc["connections_controlled"]), "{name}");
            assert_eq!(su.heatex_count as i64, i(&sc["heatex"]), "{name}");
            assert_eq!(su.channel_count as i64, i(&sc["channel"]), "{name}");
            assert_eq!(su.heatsink_count as i64, i(&sc["heatsink"]), "{name}");
            assert_eq!(su.source_count as i64, i(&sc["source"]), "{name}");
            assert_eq!(su.core_heat_capacity, i(&sc["core_heat_capacity"]), "{name}");
            let mut rods = su.rods.clone();
            rods.sort_unstable();
            assert_eq!(rods, arr(&sc["rods"]).iter().map(p3).collect::<Vec<_>>(), "{name} rods");
            let mut ports = su.ports.clone();
            ports.sort_unstable();
            assert_eq!(ports, arr(&sc["ports"]).iter().map(p3).collect::<Vec<_>>(), "{name} ports");
            let port_blocks = a.parts.values().filter(|b| **b == PwrBlock::Port).count();
            let internal = a.parts.values().filter(|b| **b != PwrBlock::Controller && **b != PwrBlock::Port).count();
            assert_eq!(port_blocks as i64, i(&sc["port_blocks"]), "{name}");
            assert_eq!(internal as i64, i(&sc["internal_blocks"]), "{name}");
        } else {
            assert_eq!(i(&sc["rod_count"]), 0, "{name}");
        }
    }
    assert!(ok >= 9);
}

fn fuel_stack(v: &Value) -> Option<FuelStack> {
    v.as_array().map(|a| FuelStack { hot: i(&a[0]) == 1, meta: i(&a[1]) as i32, count: i(&a[2]) as i32 })
}

fn pwr_from(initial: &Value, setup: &Value) -> PwrController {
    let mut c = PwrController {
        core_heat: i(&initial["core_heat"]),
        hull_heat: i(&initial["hull_heat"]),
        flux: bits(&initial["flux"]),
        rod_level: bits(&initial["rod_level"]),
        rod_target: bits(&initial["rod_target"]),
        type_loaded: i(&initial["type_loaded"]) as i32,
        amount_loaded: i(&initial["amount_loaded"]) as i32,
        progress: bits(&initial["progress"]),
        process_time: bits(&initial["process_time"]),
        unload_delay: i(&initial["unload_delay"]) as i32,
        assembled: true,
        cold_type: Fluid(st(&initial["tank_types"][0])),
        cold_fill: i(&initial["tanks"][0]) as i32,
        hot_type: Fluid(st(&initial["tank_types"][1])),
        hot_fill: i(&initial["tanks"][1]) as i32,
        slot_in: fuel_stack(&initial["slot0"]),
        slot_out: fuel_stack(&initial["slot1"]),
        ..Default::default()
    };
    c.rod_count = i(&setup["rod_count"]) as i32;
    c.connections = i(&setup["connections"]) as i32;
    c.connections_controlled = i(&setup["connections_controlled"]) as i32;
    c.heatex_count = i(&setup["heatex"]) as i32;
    c.channel_count = i(&setup["channel"]) as i32;
    c.heatsink_count = i(&setup["heatsink"]) as i32;
    c.source_count = i(&setup["source"]) as i32;
    c.core_heat_capacity = i(&setup["core_heat_capacity"]);
    c.rods = arr(&setup["rods"]).iter().map(p3).collect();
    c
}

fn assert_pwr_eq(c: &PwrController, v: &Value, ctx: &str) {
    assert_eq!(c.core_heat, i(&v["core_heat"]), "{ctx} core_heat");
    assert_eq!(c.hull_heat, i(&v["hull_heat"]), "{ctx} hull_heat");
    assert!(same(c.flux, bits(&v["flux"])), "{ctx} flux {} vs {}", c.flux, bits(&v["flux"]));
    // progress accumulates log10/pow results, which Java's intrinsics and libm round differently by
    // up to an ulp; everything else is bit-exact
    let (pa, pb) = (c.progress, bits(&v["progress"]));
    assert!(same(pa, pb) || (pa - pb).abs() <= 1e-12 * pa.abs().max(pb.abs()), "{ctx} progress {pa} vs {pb}");
    assert!(same(c.process_time, bits(&v["process_time"])), "{ctx} process_time");
    assert!(same(c.rod_level, bits(&v["rod_level"])), "{ctx} rod_level");
    assert!(same(c.rod_target, bits(&v["rod_target"])), "{ctx} rod_target");
    assert_eq!(c.type_loaded as i64, i(&v["type_loaded"]), "{ctx} type_loaded");
    assert_eq!(c.amount_loaded as i64, i(&v["amount_loaded"]), "{ctx} amount_loaded");
    assert_eq!(c.unload_delay as i64, i(&v["unload_delay"]), "{ctx} unload_delay");
    assert_eq!((c.cold_fill as i64, c.hot_fill as i64), (i(&v["tanks"][0]), i(&v["tanks"][1])), "{ctx} tanks");
    assert_eq!((c.cold_type.0, c.hot_type.0), (s(&v["tank_types"][0]), s(&v["tank_types"][1])), "{ctx} tank types");
    assert_eq!(c.slot_in, fuel_stack(&v["slot0"]), "{ctx} slot0");
    assert_eq!(c.slot_out, fuel_stack(&v["slot1"]), "{ctx} slot1");
}

#[test]
fn pwr_controller_ticks_match_java() {
    let scenarios = arr(&fx()["pwr_fission"]);
    assert!(scenarios.len() >= 35);
    let mut melts = 0;
    for sc in scenarios {
        let name = s(&sc["name"]);
        let chunks = sc["chunks_loaded"].as_bool().unwrap();
        let mut c = pwr_from(&sc["initial"], &sc["setup"]);
        c.assembled = sc["name"] != "not_assembled";
        let mut time = 0i64;
        for (k, tick) in arr(&sc["ticks"]).iter().enumerate() {
            time += 1;
            let ev = c.tick(chunks, time);
            let log: Vec<&str> = arr(&tick["log"]).iter().map(s).collect();
            let explode = log.iter().find(|l| l.starts_with("explode "));
            assert_eq!(ev.meltdown.is_some(), explode.is_some(), "{name} tick {k} meltdown");
            if let (Some(m), Some(line)) = (&ev.meltdown, explode) {
                melts += 1;
                let p: Vec<f64> = line.split(' ').skip(1).map(|t| t.parse().unwrap()).collect();
                assert!(same(m.center.0, p[0]) && same(m.center.1, p[1]) && same(m.center.2, p[2]), "{name} centre {:?} vs {p:?}", m.center);
                assert_eq!(m.explosion_power as f64, p[3]);
                assert_eq!(log.iter().filter(|l| l.ends_with("corium_block 5")).count(), m.rods.len());
            }
            assert_pwr_eq(&c, &tick["state"], &format!("{name} tick {k}"));
        }
    }
    assert!(melts >= 1);
}

// ------------------------------------------------------------------ Petroleum E

#[test]
fn hephaestus_matches_java_tick_by_tick() {
    let h = &fx()["hephaestus"];
    let worlds: Vec<(Pos3, HashMap<Pos3, String>)> = arr(&h["worlds"]).iter().map(|w| (p3(&w["machine"]), runs(&w["blocks"]))).collect();
    assert_eq!(arr(&h["cases"]).len(), 45);
    let mut triples = 0;
    for case in arr(&h["cases"]) {
        let name = s(&case["name"]);
        let (pos, blocks) = &worlds[i(&case["world"]) as usize];
        let ini = &case["initial"];
        let mut m = Hephaestus {
            input_type: st(&ini["in_type"]),
            input_fill: i(&ini["input"][0]) as i32,
            output_type: st(&ini["out_type"]),
            output_fill: i(&ini["output"][0]) as i32,
            ..Default::default()
        };
        let mut time = i(&case["time"]);
        let at = |x: i32, y: i32, z: i32| match blocks.get(&(x, y, z)).map(String::as_str) {
            Some("lava") | Some("flowing_lava") => HeatBlock::Lava,
            Some("volcanic_lava_block") => HeatBlock::VolcanicLava,
            Some("ore_volcano") => HeatBlock::OreVolcano,
            _ => HeatBlock::Other,
        };
        for (k, want) in arr(&case["ticks"]).iter().enumerate() {
            m.tick(*pos, time, &|x, y, z| at(x, y, z));
            assert_eq!(m.input_fill as i64, i(&want["input"][0]), "{name} tick {k} input");
            assert_eq!(m.output_fill as i64, i(&want["output"][0]), "{name} tick {k} output");
            assert_eq!((m.input_type, m.output_type), (s(&want["in_type"]), s(&want["out_type"])), "{name} tick {k}");
            let heat: Vec<i64> = arr(&want["heat"]).iter().map(i).collect();
            assert_eq!(m.heat.iter().map(|h| *h as i64).collect::<Vec<_>>(), heat, "{name} tick {k} planes");
            assert_eq!(m.fissure_scan_time, i(&want["fissure"]), "{name} tick {k} fissure");
            assert_eq!(m.total_heat(time) as i64, i(&want["total_heat"]), "{name} tick {k} total");
            if k == 29 && m.output_fill > 0 && m.input_type == "water" {
                triples += 1;
            }
            time += 1;
        }
    }
    assert!(triples >= 3);
    let ports: Vec<(i32, i32, i32)> = arr(&fx()["watz_extra"]["heph_ports"]).iter().map(p3).collect();
    assert_eq!(ports, HEPHAESTUS_PORTS.to_vec());
}

#[test]
fn flare_matches_java_tick_by_tick() {
    let table = fuel_traits();
    assert_eq!(table.len(), 156);
    assert_eq!(table.values().filter(|t| t.flammable).count(), 60);
    assert_eq!(table.values().filter(|t| t.ventable()).count(), 38);
    let mut cases = 0;
    for case in arr(&fx()["flare"]) {
        let name = s(&case["name"]);
        let fluid = s(&case["fluid"]);
        // "gas" in the default-tank case is the plain GAS fluid; every row also exists in the table
        let traits = *table.get(fluid).unwrap_or_else(|| panic!("no traits for {fluid}"));
        let mut f = Flare { power: i(&case["power"]), fill: i(&case["fill"]) as i32, is_on: case["on"].as_bool().unwrap(), does_burn: case["burn"].as_bool().unwrap() };
        let mut time = i(&case["time"]);
        for (k, want) in arr(&case["ticks"]).iter().enumerate() {
            let out = f.tick(traits, i(&case["speed"]) as i32, i(&case["effect"]) as i32, case["tilted"].as_bool().unwrap(), time);
            assert_eq!(f.fill as i64, i(&want["fill"]), "{name} tick {k} fill");
            assert_eq!(f.power, i(&want["power"]), "{name} tick {k} power");
            assert_eq!(out.fluid_used as i64, i(&want["fluid_used"]), "{name} tick {k} fluid_used");
            assert_eq!(out.output as i64, i(&want["output"]), "{name} tick {k} output");
            let mut ours: Vec<String> = out
                .events
                .iter()
                .filter_map(|e| match e {
                    FlareEvent::Release { amount } => Some(format!("release {fluid} {amount}")),
                    FlareEvent::Spill { amount } => Some(format!("spill {fluid} {amount}")),
                    FlareEvent::Burn { amount } => Some(format!("burn {fluid} {amount}")),
                    _ => None,
                })
                .collect();
            let mut java: Vec<String> = arr(&want["events"]).iter().map(|e| s(e).to_string()).collect();
            ours.sort();
            java.sort();
            assert_eq!(ours, java, "{name} tick {k} events");
            time += 1;
        }
        cases += 1;
    }
    assert!(cases > 312);
}
