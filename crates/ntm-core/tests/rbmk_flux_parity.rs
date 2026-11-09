//! Parity of the reactor step (`ntm_core::rbmk::reactor`) with the flux oracle
//! (`tools/rbmk-oracle/run_flux.py`): the pinned RBMK tile entities and neutron
//! handler run the scenarios of `flux_scenarios.json` tick by tick, and every
//! double of every column is compared by its raw bits after each tick.

use ntm_core::rbmk::dials::{Dials, GameRules};
use ntm_core::rbmk::jrandom::JavaRandom;
use ntm_core::rbmk::outgasser::Stack;
use ntm_core::rbmk::reactor::{AutoFunction, Column, Data, Effect, Kind, Reactor};
use ntm_core::rbmk::rods::{DepleteFunc, RodStack, RodType};
use serde_json::Value;

const Y: i32 = 10;

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../tools/rbmk-oracle/fixtures/flux.json"
    ))
    .unwrap()
}

fn num(c: &Value, k: &str, def: f64) -> f64 {
    c.get(k).map_or(def, |v| v.as_f64().unwrap())
}

fn kind(s: &str) -> Kind {
    match s {
        "rod" => Kind::Rod,
        "rod_reasim" => Kind::RodReaSim,
        "control" => Kind::Control,
        "control_reasim" => Kind::ControlReaSim,
        "control_auto" => Kind::ControlAuto,
        "control_reasim_auto" => Kind::ControlReaSimAuto,
        "moderator" => Kind::Moderator,
        "reflector" => Kind::Reflector,
        "absorber" => Kind::Absorber,
        "blank" => Kind::Blank,
        "storage" => Kind::Storage,
        "outgasser" => Kind::Outgasser,
        "boiler" => Kind::Boiler,
        "heater" => Kind::Heater,
        "cooler" => Kind::Cooler,
        _ => panic!("kind {s}"),
    }
}

fn leak(v: &Value) -> &'static str {
    Box::leak(v.as_str().unwrap().to_string().into_boxed_str())
}

fn stack(v: &Value) -> Option<Stack> {
    if v.is_null() {
        return None;
    }
    Some(Stack {
        item: v["item"].as_str().unwrap().to_string(),
        count: num(v, "count", 1.0) as i32,
        meta: num(v, "meta", 0.0) as i32,
        max_stack: 64,
    })
}

fn set_tank(t: &mut ntm_core::rbmk::fluids::Tank, v: Option<&Value>) {
    if let Some(v) = v {
        if let Some(f) = v.get("fluid") {
            t.fluid = leak(f);
        }
        if let Some(n) = v.get("fill") {
            t.fill = n.as_f64().unwrap() as i32;
        }
    }
}

fn fuel(c: &Value, variable: &str) -> (&'static RodType, RodStack) {
    let rod = RodType::by_variable(variable).unwrap();
    let mut s = RodStack::fresh(rod);
    if let Some(f) = c.get("yield_fraction") {
        s.set_yield(rod, rod.yield_ * f.as_f64().unwrap());
    }
    if let Some(v) = c.get("xenon") {
        s.set_poison(rod, v.as_f64().unwrap());
    }
    if let Some(v) = c.get("core") {
        s.set_core_heat(rod, v.as_f64().unwrap());
    }
    if let Some(v) = c.get("hull") {
        s.set_hull_heat(rod, v.as_f64().unwrap());
    }
    (rod, s)
}

fn build(sc: &Value) -> Reactor {
    let mut rules = GameRules::default();
    if let Some(r) = sc.get("rules") {
        for (k, v) in r.as_object().unwrap() {
            rules.set(k, v.as_str().unwrap());
        }
    }
    let dials = Dials::refresh(&rules, false);
    let mut r = Reactor::new(dials, JavaRandom::new(num(sc, "seed", 1.0) as i64), Y);
    for c in sc["columns"].as_array().unwrap() {
        let k = kind(c["kind"].as_str().unwrap());
        let mut col = Column::new(k, num(c, "x", 0.0) as i32, num(c, "z", 0.0) as i32);
        col.moderated = c.get("moderated").is_some_and(|v| v.as_bool().unwrap());
        // control rods have no removable lid
        col.lid = matches!(
            k,
            Kind::Control | Kind::ControlReaSim | Kind::ControlAuto | Kind::ControlReaSimAuto
        ) || c.get("lid").is_none_or(|v| v.as_bool().unwrap());
        col.heat = num(c, "heat", 20.0);
        col.reasim_water = num(c, "water", 0.0) as i32;
        col.reasim_steam = num(c, "steam", 0.0) as i32;
        if let Some(rc) = col.rod_mut() {
            if let Some(v) = c.get("rod").and_then(Value::as_str) {
                rc.fuel = Some(fuel(c, v));
            }
            if let Some(f) = c.get("flux") {
                rc.flux_quantity = f[0].as_f64().unwrap();
                rc.flux_fast_ratio = f[1].as_f64().unwrap();
            }
        }
        if let Some(cc) = col.control_mut() {
            cc.level = num(c, "level", 0.0);
            cc.target_level = num(c, "target", cc.level);
            cc.power = num(c, "power", 0.0) as i64;
            cc.starting_level = num(c, "starting", 0.0);
            cc.level_lower = num(c, "level_lower", 0.0);
            cc.level_upper = num(c, "level_upper", 0.0);
            cc.heat_lower = num(c, "heat_lower", 0.0);
            cc.heat_upper = num(c, "heat_upper", 0.0);
            if cc.function.is_some() {
                cc.function = Some(
                    [
                        AutoFunction::Linear,
                        AutoFunction::QuadUp,
                        AutoFunction::QuadDown,
                    ][num(c, "function", 0.0) as usize],
                );
            }
        }
        match &mut col.data {
            Data::Outgasser(o) => {
                o.progress = num(c, "progress", 0.0);
                o.input = c.get("in").and_then(stack);
                o.output = c.get("out").and_then(stack);
                set_tank(&mut o.gas, c.get("gas"));
            }
            Data::Boiler(b) => {
                set_tank(&mut b.feed, c.get("feed"));
                set_tank(&mut b.steam, c.get("steam_tank"));
            }
            Data::Heater(h) => {
                set_tank(&mut h.feed, c.get("feed"));
                set_tank(&mut h.steam, c.get("steam_tank"));
            }
            Data::Cooler(k) => {
                set_tank(&mut k.cold, c.get("cold"));
                set_tank(&mut k.hot, c.get("hot"));
            }
            _ => {}
        }
        let i = r.add(col);
        if let Some(id) = c.get("identifier").filter(|v| !v.is_null()) {
            r.identifiers.insert(i, leak(id));
        }
    }
    for p in sc.get("probes").map_or(&vec![], |v| v.as_array().unwrap()) {
        r.ore_names
            .entry((
                p["item"].as_str().unwrap().to_string(),
                num(p, "meta", 0.0) as i32,
            ))
            .or_default()
            .push(p["ore"].as_str().unwrap().to_string());
    }
    let height = r.dials.column_height + 1;
    for o in sc
        .get("obstacles")
        .map_or(&vec![], |v| v.as_array().unwrap())
    {
        let (x, z) = (num(o, "x", 0.0) as i32, num(o, "z", 0.0) as i32);
        let from = num(o, "from", 0.0) as i32;
        for h in 0..num(o, "height", f64::from(height)) as i32 {
            r.obstacles.insert((x, z, from + h));
        }
    }
    r
}

fn bits(v: &Value) -> u64 {
    u64::from_str_radix(v["bits"].as_str().unwrap(), 16).unwrap()
}

fn same(got: f64, want: &Value, what: &str) {
    if std::env::var_os("RBMK_PARITY_ALL").is_some() {
        if got.to_bits() != bits(want) {
            eprintln!("MISMATCH {what}: got {got:?}, want {want}");
        }
        return;
    }
    assert_eq!(
        got.to_bits(),
        bits(want),
        "{what}: got {got:?}, want {want}"
    );
}

fn same_stack(got: &RodStack, want: &Value, what: &str) {
    let nbt = want["nbt"].as_object().unwrap();
    let mut n = 0;
    for (k, v) in [
        ("yield", got.yield_),
        ("xenon", got.xenon),
        ("core", got.core),
        ("hull", got.hull),
    ] {
        if let Some(v) = v {
            n += 1;
            same(
                v,
                nbt.get(k)
                    .unwrap_or_else(|| panic!("{what}: extra key {k}")),
                &format!("{what}.{k}"),
            );
        }
    }
    assert_eq!(n, nbt.len(), "{what}: key sets differ");
}

/// The heat and flux buffers at the tick a meltdown starts (the fuel itself is
/// rewritten by `onMelt`, so the stack is not compared).
fn compare_melted(r: &Reactor, i: usize, want: &Value, what: &str) {
    let c = &r.columns[i];
    same(c.heat, &want["heat"], &format!("{what} heat"));
    if let Some(rc) = c.rod() {
        same(rc.flux_quantity, &want["flux_q"], &format!("{what} flux_q"));
        same(
            rc.last_flux_quantity,
            &want["last_q"],
            &format!("{what} last_q"),
        );
        same(
            rc.last_flux_ratio,
            &want["last_r"],
            &format!("{what} last_r"),
        );
    }
}

fn compare(r: &Reactor, i: usize, want: &Value, what: &str) {
    let c = &r.columns[i];
    same(c.heat, &want["heat"], &format!("{what} heat"));
    assert_eq!(
        i64::from(c.reasim_water),
        want["water"].as_i64().unwrap(),
        "{what} water"
    );
    assert_eq!(
        i64::from(c.reasim_steam),
        want["steam"].as_i64().unwrap(),
        "{what} steam"
    );
    assert_eq!(
        i64::from(c.crane_indicator),
        want["crane"].as_i64().unwrap(),
        "{what} crane"
    );
    if let Some(rc) = c.rod() {
        assert_eq!(
            rc.has_rod,
            want["has_rod"].as_bool().unwrap(),
            "{what} has_rod"
        );
        same(rc.flux_quantity, &want["flux_q"], &format!("{what} flux_q"));
        same(
            rc.flux_fast_ratio,
            &want["flux_r"],
            &format!("{what} flux_r"),
        );
        same(
            rc.last_flux_quantity,
            &want["last_q"],
            &format!("{what} last_q"),
        );
        same(
            rc.last_flux_ratio,
            &want["last_r"],
            &format!("{what} last_r"),
        );
        match (&rc.fuel, want["stack"].is_null()) {
            (Some((_, s)), false) => same_stack(s, &want["stack"], &format!("{what} stack")),
            (None, true) => {}
            _ => panic!("{what}: fuel presence differs"),
        }
    }
    match &c.data {
        Data::Outgasser(o) => {
            same(o.progress, &want["progress"], &format!("{what} progress"));
            assert_eq!(
                (o.gas.fluid, i64::from(o.gas.fill)),
                (
                    want["gas_type"].as_str().unwrap(),
                    want["gas"].as_i64().unwrap()
                ),
                "{what} gas"
            );
            for (k, got) in [("in", &o.input), ("out", &o.output)] {
                match (got, want[k].is_null()) {
                    (Some(g), false) => assert_eq!(
                        (g.item.as_str(), i64::from(g.count), i64::from(g.meta)),
                        (
                            want[k]["item"].as_str().unwrap(),
                            want[k]["count"].as_i64().unwrap(),
                            want[k]["meta"].as_i64().unwrap()
                        ),
                        "{what} {k}"
                    ),
                    (None, true) => {}
                    _ => panic!("{what}: {k} presence differs"),
                }
            }
        }
        Data::Boiler(b) => {
            for (k, t) in [("feed", &b.feed), ("steam_tank", &b.steam)] {
                assert_eq!(
                    (t.fluid, i64::from(t.fill)),
                    (
                        want[k]["fluid"].as_str().unwrap(),
                        want[k]["fill"].as_i64().unwrap()
                    ),
                    "{what} {k}"
                );
            }
            assert_eq!(
                (
                    i64::from(b.consumption),
                    i64::from(b.output),
                    i64::from(b.vent_delay)
                ),
                (
                    want["consumption"].as_i64().unwrap(),
                    want["output"].as_i64().unwrap(),
                    want["vent_delay"].as_i64().unwrap()
                ),
                "{what} boiler counters"
            );
        }
        Data::Heater(h) => {
            for (k, t) in [("feed", &h.feed), ("steam_tank", &h.steam)] {
                assert_eq!(
                    (t.fluid, i64::from(t.fill)),
                    (
                        want[k]["fluid"].as_str().unwrap(),
                        want[k]["fill"].as_i64().unwrap()
                    ),
                    "{what} {k}"
                );
            }
        }
        Data::Cooler(k) => {
            assert_eq!(
                i64::from(k.timer),
                want["timer"].as_i64().unwrap(),
                "{what} timer"
            );
            for (n, t) in [("cold", &k.cold), ("hot", &k.hot)] {
                assert_eq!(
                    (t.fluid, i64::from(t.fill)),
                    (
                        want[n]["fluid"].as_str().unwrap(),
                        want[n]["fill"].as_i64().unwrap()
                    ),
                    "{what} {n}"
                );
            }
        }
        _ => {}
    }
    if let Some(cc) = c.control() {
        same(cc.level, &want["level"], &format!("{what} level"));
        same(
            cc.last_level,
            &want["last_level"],
            &format!("{what} last_level"),
        );
        same(cc.target_level, &want["target"], &format!("{what} target"));
        assert_eq!(
            cc.has_power,
            want["has_power"].as_bool().unwrap(),
            "{what} has_power"
        );
        assert_eq!(cc.power, want["power"].as_i64().unwrap(), "{what} power");
        same(
            c.control_mult(&r.dials),
            &want["mult"],
            &format!("{what} mult"),
        );
        if let Some(s) = want.get("starting") {
            same(cc.starting_level, s, &format!("{what} starting"));
        }
    }
}

#[test]
fn every_scenario_matches_the_java_oracle_tick_by_tick() {
    let fx = fixture();
    let scenarios = fx["scenarios"].as_array().unwrap();
    let results = fx["results"].as_array().unwrap();
    assert_eq!(scenarios.len(), results.len());
    for (sc, res) in scenarios.iter().zip(results) {
        let name = sc["name"].as_str().unwrap();
        let mut r = build(sc);
        let melt = res["meltdown_tick"].as_u64();
        let no_actions = Vec::new();
        let actions = sc
            .get("actions")
            .map_or(&no_actions, |a| a.as_array().unwrap());
        for t in res["trace"].as_array().unwrap() {
            let tick = t["tick"].as_u64().unwrap();
            for a in actions
                .iter()
                .filter(|a| a["tick"].as_f64() == Some(tick as f64))
            {
                let i = a["column"].as_f64().unwrap() as usize;
                if let Some(v) = a.get("target") {
                    r.columns[i].set_target(v.as_f64().unwrap());
                }
                if let Some(v) = a.get("insert_rod") {
                    r.columns[i].rod_mut().unwrap().fuel = Some(fuel(a, v.as_str().unwrap()));
                }
                if a.get("remove_rod").is_some() {
                    r.columns[i].rod_mut().unwrap().fuel = None;
                }
                if a.get("compress").is_some() {
                    r.boiler_compress(i);
                }
                if let Some(v) = a.get("identifier") {
                    if v.is_null() {
                        r.identifiers.remove(&i);
                    } else {
                        r.identifiers.insert(i, leak(v));
                    }
                }
                if let (Some(v), Data::Outgasser(o)) = (a.get("in"), &mut r.columns[i].data) {
                    o.input = stack(v);
                }
                if let Some(t) = a.get("set_tanks") {
                    match &mut r.columns[i].data {
                        Data::Boiler(b) => {
                            set_tank(&mut b.feed, t.get("feed"));
                            set_tank(&mut b.steam, t.get("steam"));
                        }
                        Data::Heater(h) => set_tank(&mut h.feed, t.get("feed")),
                        Data::Cooler(k) => {
                            set_tank(&mut k.cold, t.get("cold"));
                            set_tank(&mut k.hot, t.get("hot"));
                        }
                        _ => {}
                    }
                }
                if let Some(v) = a.get("power") {
                    r.columns[i].control_mut().unwrap().power = v.as_f64().unwrap() as i64;
                }
            }
            r.step();
            let at = format!("{name} tick {tick}");
            // radiation, in order
            let rad = t["rad"].as_array().unwrap();
            assert_eq!(r.rad.len(), rad.len(), "{at}: radiation event count");
            for (k, (g, w)) in r.rad.iter().zip(rad).enumerate() {
                assert_eq!(
                    (g.x, g.z),
                    (w[0].as_i64().unwrap() as i32, w[1].as_i64().unwrap() as i32),
                    "{at}: rad #{k} position"
                );
                assert_eq!(
                    format!("{:08x}", g.amount.to_bits()),
                    w[2].as_str().unwrap(),
                    "{at}: rad #{k} amount"
                );
            }
            // effects: all particles first, then the sounds (the oracle logs them apart)
            let fx = t["fx"].as_array().unwrap();
            let skip_fx = Some(tick) == melt; // the meltdown's own sound and particle belong to the engine
            let mut vents = Vec::new();
            let mut scans = Vec::new();
            for e in &r.effects {
                match *e {
                    Effect::SteamVent {
                        column,
                        x_offset,
                        z_offset,
                        pitch,
                    } => vents.push((column, x_offset, z_offset, pitch)),
                    Effect::SatelliteScan { column } => scans.push(column),
                }
            }
            // the oracle logs particles, then sounds, then satellite scans
            assert!(
                skip_fx || fx.len() == vents.len() * 2 + scans.len(),
                "{at}: effect count"
            );
            for (k, (col, xo, zo, pitch)) in vents.iter().enumerate().filter(|_| !skip_fx) {
                let c = &r.columns[*col];
                let p = &fx[k];
                assert_eq!(p[0], "particle", "{at}");
                assert_eq!(
                    (p[1].as_f64().unwrap(), p[2].as_f64().unwrap()),
                    (f64::from(c.x) + xo, f64::from(c.z) + zo),
                    "{at}: vent position"
                );
                assert_eq!(
                    fx[vents.len() + k][4].as_str().unwrap(),
                    format!("{:08x}", pitch.to_bits()),
                    "{at}: vent pitch"
                );
            }
            for (k, col) in scans.iter().enumerate().filter(|_| !skip_fx) {
                let w = &fx[vents.len() * 2 + k];
                let c = &r.columns[*col];
                assert_eq!(
                    (
                        w[0].as_str().unwrap(),
                        w[1].as_i64().unwrap(),
                        w[2].as_i64().unwrap(),
                        w[3].as_i64().unwrap()
                    ),
                    ("scan", i64::from(c.x), i64::from(c.z), 300),
                    "{at}: scan"
                );
            }
            if Some(tick) == melt {
                assert!(!r.meltdowns.is_empty(), "{at}: Java melted down here");
                // the column state after the request: everything the meltdown does not rewrite
                for (i, w) in t["cols"].as_array().unwrap().iter().enumerate() {
                    compare_melted(&r, i, w, &format!("{at} col {i}"));
                }
                break;
            }
            assert!(r.meltdowns.is_empty(), "{at}: unexpected meltdown request");
            for (i, w) in t["cols"].as_array().unwrap().iter().enumerate() {
                compare(&r, i, w, &format!("{at} col {i}"));
            }
        }
    }
}

fn unhex(v: &Value) -> f64 {
    f64::from_bits(u64::from_str_radix(v.as_str().unwrap(), 16).unwrap())
}

/// Thousands of points through the depletion curves' `sin` and the control surge's
/// `pow(1 - level, 15)`, which the scenarios alone cannot tell from the platform libm.
#[test]
fn depletion_and_surge_sweeps_match_java_bits() {
    let fx = fixture();
    let sweeps = &fx["sweeps"];
    let (mut misses, mut total) = ([0usize; 2], 0usize);
    for (name, depletion) in [
        ("RAISING_SLOPE", DepleteFunc::RaisingSlope),
        ("BOOSTED_SLOPE", DepleteFunc::BoostedSlope),
        ("GENTLE_SLOPE", DepleteFunc::GentleSlope),
    ] {
        let mut rod = RodType::blank();
        rod.depletion = depletion;
        for row in sweeps["depletion"][name].as_array().unwrap() {
            let (e, want) = (unhex(&row[0]), unhex(&row[1]));
            misses[0] +=
                usize::from(rod.reactivity_mod_by_enrichment(e).to_bits() != want.to_bits());
            total += 1;
        }
    }
    let mut rules = GameRules::default();
    rules.set("dialControlSurgeMod", "1.7");
    let mut r = Reactor::new(Dials::refresh(&rules, false), JavaRandom::new(1), Y);
    let i = r.add(Column::new(Kind::Control, 0, 0));
    for row in sweeps["surge"].as_array().unwrap() {
        let c = r.columns[i].control_mut().unwrap();
        c.level = unhex(&row[0]);
        c.starting_level = unhex(&row[1]);
        c.target_level = unhex(&row[2]);
        let want = unhex(&row[3]);
        misses[1] += usize::from(r.columns[i].control_mult(&r.dials).to_bits() != want.to_bits());
    }
    eprintln!(
        "sweep misses: depletion {} of {total}, surge {} of 3000",
        misses[0], misses[1]
    );
    // pinned one-ulp residual of the HotSpot stubs (see jmath)
    assert!(misses[1] <= SURGE_MISSES, "surge misses {}", misses[1]);
    assert!(
        misses[0] <= DEPLETION_SIN_MISSES,
        "depletion misses {} of {total}",
        misses[0]
    );
}

const DEPLETION_SIN_MISSES: usize = 6; // of 9000, measured
const SURGE_MISSES: usize = 1; // of 3000, measured
