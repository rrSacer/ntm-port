//! Parity of the reactor step (`ntm_core::rbmk::reactor`) with the flux oracle
//! (`tools/rbmk-oracle/run_flux.py`): the pinned RBMK tile entities and neutron
//! handler run the scenarios of `flux_scenarios.json` tick by tick, and every
//! double of every column is compared by its raw bits after each tick.

use ntm_core::rbmk::dials::{Dials, GameRules};
use ntm_core::rbmk::jrandom::JavaRandom;
use ntm_core::rbmk::reactor::{AutoFunction, Column, Kind, Reactor};
use ntm_core::rbmk::rods::{RodStack, RodType};
use serde_json::Value;

const Y: i32 = 10;

fn fixture() -> Value {
    serde_json::from_str(include_str!("../../../tools/rbmk-oracle/fixtures/flux.json")).unwrap()
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
        _ => panic!("kind {s}"),
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
        col.lid = matches!(k, Kind::Control | Kind::ControlReaSim | Kind::ControlAuto | Kind::ControlReaSimAuto)
            || c.get("lid").is_none_or(|v| v.as_bool().unwrap());
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
                cc.function = Some([AutoFunction::Linear, AutoFunction::QuadUp, AutoFunction::QuadDown][num(c, "function", 0.0) as usize]);
            }
        }
        r.add(col);
    }
    let height = r.dials.column_height + 1;
    for o in sc.get("obstacles").map_or(&vec![], |v| v.as_array().unwrap()) {
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
    assert_eq!(got.to_bits(), bits(want), "{what}: got {got:?}, want {want}");
}

fn same_stack(got: &RodStack, want: &Value, what: &str) {
    let nbt = want["nbt"].as_object().unwrap();
    let mut n = 0;
    for (k, v) in [("yield", got.yield_), ("xenon", got.xenon), ("core", got.core), ("hull", got.hull)] {
        if let Some(v) = v {
            n += 1;
            same(v, nbt.get(k).unwrap_or_else(|| panic!("{what}: extra key {k}")), &format!("{what}.{k}"));
        }
    }
    assert_eq!(n, nbt.len(), "{what}: key sets differ");
}

fn compare(r: &Reactor, i: usize, want: &Value, what: &str) {
    let c = &r.columns[i];
    same(c.heat, &want["heat"], &format!("{what} heat"));
    assert_eq!(i64::from(c.reasim_water), want["water"].as_i64().unwrap(), "{what} water");
    assert_eq!(i64::from(c.reasim_steam), want["steam"].as_i64().unwrap(), "{what} steam");
    assert_eq!(i64::from(c.crane_indicator), want["crane"].as_i64().unwrap(), "{what} crane");
    if let Some(rc) = c.rod() {
        assert_eq!(rc.has_rod, want["has_rod"].as_bool().unwrap(), "{what} has_rod");
        same(rc.flux_quantity, &want["flux_q"], &format!("{what} flux_q"));
        same(rc.flux_fast_ratio, &want["flux_r"], &format!("{what} flux_r"));
        same(rc.last_flux_quantity, &want["last_q"], &format!("{what} last_q"));
        same(rc.last_flux_ratio, &want["last_r"], &format!("{what} last_r"));
        match (&rc.fuel, want["stack"].is_null()) {
            (Some((_, s)), false) => same_stack(s, &want["stack"], &format!("{what} stack")),
            (None, true) => {}
            _ => panic!("{what}: fuel presence differs"),
        }
    }
    if let Some(cc) = c.control() {
        same(cc.level, &want["level"], &format!("{what} level"));
        same(cc.last_level, &want["last_level"], &format!("{what} last_level"));
        same(cc.target_level, &want["target"], &format!("{what} target"));
        assert_eq!(cc.has_power, want["has_power"].as_bool().unwrap(), "{what} has_power");
        assert_eq!(cc.power, want["power"].as_i64().unwrap(), "{what} power");
        same(c.control_mult(&r.dials), &want["mult"], &format!("{what} mult"));
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
        let actions = sc.get("actions").map_or(&no_actions, |a| a.as_array().unwrap());
        for t in res["trace"].as_array().unwrap() {
            let tick = t["tick"].as_u64().unwrap();
            for a in actions.iter().filter(|a| a["tick"].as_f64() == Some(tick as f64)) {
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
                assert_eq!((g.x, g.z), (w[0].as_i64().unwrap() as i32, w[1].as_i64().unwrap() as i32), "{at}: rad #{k} position");
                assert_eq!(format!("{:08x}", g.amount.to_bits()), w[2].as_str().unwrap(), "{at}: rad #{k} amount");
            }
            if Some(tick) == melt {
                assert!(!r.meltdowns.is_empty(), "{at}: Java melted down here");
                break;
            }
            assert!(r.meltdowns.is_empty(), "{at}: unexpected meltdown request");
            for (i, w) in t["cols"].as_array().unwrap().iter().enumerate() {
                compare(&r, i, w, &format!("{at} col {i}"));
            }
        }
    }
}
