//! Parity of `ntm_core::rbmk` with the Java oracle (`tools/rbmk-oracle`): the
//! fixture is produced by running the real HBM classes (see its README), and
//! every double is compared by its raw IEEE bits.

use ntm_core::rbmk::dials::{DialState, Dials, GameRules, KEYS};
use ntm_core::rbmk::rods::{BurnFunc, DepleteFunc, NType, RodStack, RodType, RODS};
use serde_json::Value;

fn fixture() -> Value {
    serde_json::from_str(include_str!("../../../tools/rbmk-oracle/fixtures/java.json")).unwrap()
}

/// `{"value": .., "bits": ".."}` -> bits.
fn bits(v: &Value) -> u64 {
    u64::from_str_radix(v["bits"].as_str().unwrap(), 16).unwrap()
}

fn assert_bits(got: f64, expected: &Value, what: &str) {
    assert_eq!(got.to_bits(), bits(expected), "{what}: got {got:?}, expected {expected}");
}

fn burn_func(s: &str) -> BurnFunc {
    match s {
        "PASSIVE" => BurnFunc::Passive,
        "LOG_TEN" => BurnFunc::LogTen,
        "PLATEU" => BurnFunc::Plateu,
        "ARCH" => BurnFunc::Arch,
        "SIGMOID" => BurnFunc::Sigmoid,
        "SQUARE_ROOT" => BurnFunc::SquareRoot,
        "LINEAR" => BurnFunc::Linear,
        "QUADRATIC" => BurnFunc::Quadratic,
        "EXPERIMENTAL" => BurnFunc::Experimental,
        _ => panic!("burn function {s}"),
    }
}

fn dep_func(s: &str) -> DepleteFunc {
    match s {
        "LINEAR" => DepleteFunc::Linear,
        "RAISING_SLOPE" => DepleteFunc::RaisingSlope,
        "BOOSTED_SLOPE" => DepleteFunc::BoostedSlope,
        "GENTLE_SLOPE" => DepleteFunc::GentleSlope,
        "STATIC" => DepleteFunc::Static,
        _ => panic!("depletion function {s}"),
    }
}

fn ntype(s: &str) -> NType {
    match s {
        "SLOW" => NType::Slow,
        "FAST" => NType::Fast,
        "ANY" => NType::Any,
        _ => panic!("neutron type {s}"),
    }
}

fn default_dials() -> Dials {
    Dials::refresh(&GameRules::default(), false)
}

/// Compares a stack with the oracle's `nbt` object (exact bits per key).
fn assert_stack(got: &RodStack, expected: &Value, what: &str) {
    let nbt = expected["nbt"].as_object().expect("stack has a tag");
    let mut keys = Vec::new();
    for (k, v) in [("yield", got.yield_), ("xenon", got.xenon), ("core", got.core), ("hull", got.hull)] {
        if let Some(v) = v {
            keys.push(k);
            let e = nbt.get(k).unwrap_or_else(|| panic!("{what}: extra key {k}")).as_f64().unwrap();
            assert_eq!(v.to_bits(), e.to_bits(), "{what}: {k} got {v:?} expected {e:?}");
        }
    }
    for (k, v) in &got.foreign {
        keys.push(k.as_str());
        assert_eq!(v.to_bits(), nbt[k.as_str()].as_f64().unwrap().to_bits(), "{what}: {k}");
    }
    assert_eq!(keys.len(), nbt.len(), "{what}: key sets differ: {keys:?} vs {nbt:?}");
}

#[test]
fn dial_defaults_and_probes_match_java() {
    let fx = fixture();
    let dials = fx["dials"].as_array().unwrap();
    assert_eq!(dials.len(), KEYS.len());
    for d in dials {
        let name = d["key"].as_str().unwrap();
        let key = KEYS.iter().find(|k| k.name == name).unwrap();
        assert_eq!(key.rule, d["game_rule"].as_str().unwrap(), "{name}");
        for p in d["probes"].as_array().unwrap() {
            let mut rules = GameRules::default();
            rules.set(key.rule, p["input"].as_str().unwrap());
            let eff = Dials::refresh(&rules, false);
            let e = &p["effective"];
            let ctx = format!("{name} <- {}", p["input"]);
            match name {
                "KEY_SAVE_DIALS" => assert_eq!(rules.boolean(key.rule), e.as_bool().unwrap(), "{ctx}"),
                "KEY_PASSIVE_COOLING" => assert_bits_at(eff.passive_cooling, e, &ctx),
                "KEY_PASSIVE_COOLING_INNER" => assert_bits_at(eff.passive_cooling_inner, e, &ctx),
                "KEY_COLUMN_HEAT_FLOW" => assert_bits_at(eff.column_heat_flow, e, &ctx),
                "KEY_FUEL_DIFFUSION_MOD" => assert_bits_at(eff.fuel_diffusion_mod, e, &ctx),
                "KEY_HEAT_PROVISION" => assert_bits_at(eff.heat_provision, e, &ctx),
                "KEY_COLUMN_HEIGHT" => assert_eq!(i64::from(eff.column_height), e.as_i64().unwrap(), "{ctx}"),
                "KEY_PERMANENT_SCRAP" => assert_eq!(eff.permanent_scrap, e.as_bool().unwrap(), "{ctx}"),
                "KEY_BOILER_HEAT_CONSUMPTION" => assert_bits_at(eff.boiler_heat_consumption, e, &ctx),
                "KEY_CONTROL_SPEED_MOD" => assert_bits_at(eff.control_speed, e, &ctx),
                "KEY_REACTIVITY_MOD" => assert_bits_at(eff.reactivity_mod, e, &ctx),
                "KEY_OUTGASSER_MOD" => assert_bits_at(eff.outgasser_mod, e, &ctx),
                "KEY_SURGE_MOD" => assert_bits_at(eff.surge_mod, e, &ctx),
                "KEY_FLUX_RANGE" => assert_eq!(i64::from(eff.flux_range), e.as_i64().unwrap(), "{ctx}"),
                "KEY_REASIM_RANGE" => assert_eq!(i64::from(eff.reasim_range), e.as_i64().unwrap(), "{ctx}"),
                "KEY_REASIM_BOILERS" => assert_eq!(eff.reasim_boilers, e.as_bool().unwrap(), "{ctx}"),
                "KEY_REASIM_BOILER_SPEED" => assert_bits_at(eff.reasim_boiler_speed, e, &ctx),
                "KEY_DISABLE_MELTDOWNS" => assert_eq!(eff.meltdowns_disabled, e.as_bool().unwrap(), "{ctx}"),
                "KEY_ENABLE_MELTDOWN_OVERPRESSURE" => assert_eq!(eff.overpressure, e.as_bool().unwrap(), "{ctx}"),
                "KEY_MODERATOR_EFFICIENCY" => assert_bits_at(eff.moderator_efficiency, e, &ctx),
                "KEY_ABSORBER_EFFICIENCY" => assert_bits_at(eff.absorber_efficiency, e, &ctx),
                "KEY_REFLECTOR_EFFICIENCY" => assert_bits_at(eff.reflector_efficiency, e, &ctx),
                "KEY_DISABLE_DEPLETION" => assert_eq!(eff.depletion_disabled, e.as_bool().unwrap(), "{ctx}"),
                "KEY_DISABLE_XENON" => assert_eq!(eff.xenon_disabled, e.as_bool().unwrap(), "{ctx}"),
                "KEY_ABSORBER_HEAT_CONVERSION" => assert_bits_at(eff.absorber_heat_conversion, e, &ctx),
                other => panic!("unchecked dial {other}"),
            }
        }
    }
}

fn assert_bits_at(got: f64, e: &Value, ctx: &str) {
    assert_bits(got, e, ctx);
}

#[test]
fn dial_cache_quirks_match_java() {
    let fx = fixture();
    let cases = fx["dial_special_cases"].as_array().unwrap();

    // client getSurgeMod reads dialPassiveCooling
    let c = &cases[0];
    let mut rules = GameRules::default();
    rules.set("dialControlSurgeMod", "7");
    rules.set("dialPassiveCooling", "3");
    assert_eq!(Dials::refresh(&rules, false).surge_mod, c["server"].as_f64().unwrap());
    assert_eq!(Dials::surge_mod_client(&rules), c["client"].as_f64().unwrap());

    // enable528ReasimBoilers
    for c in &cases[1..3] {
        let on = c["config"].as_bool().unwrap();
        let r = Dials::refresh(&GameRules::default(), on);
        assert_eq!(r.reasim_boilers, c["effective"].as_bool().unwrap());
    }

    // createDials(force) refreshes before resetting the raw rules
    let mut st = DialState::default();
    st.rules.set("dialColumnHeight", "16");
    st.create(true);
    assert_eq!(st.rules.string("dialColumnHeight"), cases[3]["raw"].as_str().unwrap());
    assert_eq!(i64::from(st.get("KEY_COLUMN_HEIGHT").column_height), cases[3]["cached"].as_i64().unwrap());
    st.refresh();
    assert_eq!(i64::from(st.get("KEY_COLUMN_HEIGHT").column_height), cases[4]["cached"].as_i64().unwrap());
}

#[test]
fn rod_table_matches_the_fixture() {
    let fx = fixture();
    let rods = fx["rods"].as_array().unwrap();
    assert_eq!(rods.len(), 33);
    assert_eq!(RODS.len(), 33);
    for (r, t) in rods.iter().zip(RODS.iter()) {
        let v = r["variable"].as_str().unwrap();
        assert_eq!(t.variable, v);
        assert_eq!(t.full_name, r["full_name"].as_str().unwrap(), "{v}");
        assert_eq!(t.pellet, r["pellet"].as_str().unwrap_or(""), "{v}");
        let f = |k: &str| r[k].as_f64().unwrap().to_bits();
        assert_eq!(t.yield_.to_bits(), f("yield"), "{v}");
        assert_eq!(t.reactivity.to_bits(), f("func_end"), "{v}");
        assert_eq!(t.self_rate.to_bits(), f("self_rate"), "{v}");
        assert_eq!(t.x_gen.to_bits(), f("xenon_generation"), "{v}");
        assert_eq!(t.x_burn.to_bits(), f("xenon_burn_divisor"), "{v}");
        assert_eq!(t.heat.to_bits(), f("heat_per_flux"), "{v}");
        assert_eq!(t.melting_point.to_bits(), f("melting_point"), "{v}");
        assert_eq!(t.diffusion.to_bits(), f("diffusion"), "{v}");
        assert_eq!(t.function, burn_func(r["function"].as_str().unwrap()), "{v}");
        assert_eq!(t.depletion, dep_func(r["depletion_function"].as_str().unwrap()), "{v}");
        assert_eq!(t.heat_coeff_start.to_bits(), r["heat_coefficient"]["start"].as_f64().unwrap().to_bits(), "{v}");
        assert_eq!(t.heat_coeff_length.to_bits(), r["heat_coefficient"]["length"].as_f64().unwrap().to_bits(), "{v}");
        assert_eq!(t.n_type, ntype(r["neutron_types"]["absorbs"].as_str().unwrap()), "{v}");
        assert_eq!(t.r_type, ntype(r["neutron_types"]["releases"].as_str().unwrap()), "{v}");
        assert_eq!(u64::from(t.tint), r["tint"].as_u64().unwrap(), "{v}");
        assert!(!r["special_flux_curve"].as_bool().unwrap(), "{v}");
    }
    // the blank rod carries the Java field initializers
    let d = &fx["defaults"]["rod"];
    let b = RodType::blank();
    assert_eq!(b.x_gen, d["xenon_generation"].as_f64().unwrap());
    assert_eq!(b.x_burn, d["xenon_burn_divisor"].as_f64().unwrap());
    assert_eq!(b.heat, d["heat_per_flux"].as_f64().unwrap());
    assert_eq!(b.melting_point, d["melting_point"].as_f64().unwrap());
    assert_eq!(b.diffusion, d["diffusion"].as_f64().unwrap());
    assert_eq!(u64::from(b.tint), d["tint"].as_u64().unwrap());
    assert_eq!(b.function, burn_func(d["function"].as_str().unwrap()));
    assert_eq!(b.depletion, dep_func(d["depletion_function"].as_str().unwrap()));
}

#[test]
fn burn_curves_match_java_bits() {
    let fx = fixture();
    let mut rod = RodType::blank();
    rod.reactivity = 20.0;
    rod.self_rate = 10.0;
    rod.depletion = DepleteFunc::Linear;
    let burn = fx["functions"]["burn"].as_array().unwrap();
    assert_eq!(burn.len(), 360);
    for c in burn {
        rod.function = burn_func(c["function"].as_str().unwrap());
        let got = rod.reactivity_func(c["influx"].as_f64().unwrap(), c["enrichment"].as_f64().unwrap());
        assert_bits(got, &c["result"], &format!("{} @ {} / {}", c["function"], c["influx"], c["enrichment"]));
    }
    let dep = fx["functions"]["depletion"].as_array().unwrap();
    assert_eq!(dep.len(), 35);
    for c in dep {
        rod.depletion = dep_func(c["function"].as_str().unwrap());
        let got = rod.reactivity_mod_by_enrichment(c["enrichment"].as_f64().unwrap());
        assert_bits(got, &c["result"], &format!("{} @ {}", c["function"], c["enrichment"]));
    }
}

#[test]
fn fresh_stack_and_setter_cases_match_java() {
    let fx = fixture();
    let rod = RodType::by_variable("rbmk_fuel_ueu").unwrap();
    let dflt = &fx["defaults"];

    let mut absent = RodStack::bare();
    absent.yield_(rod); // a getter on a bare stack writes the defaults
    assert_stack(&absent, &dflt["nbt_absent_after_get"], "absent");

    let empty = RodStack { has_tag: true, ..RodStack::default() };
    let mut e = empty.clone();
    let r = &dflt["nbt_existing_empty_reads"];
    assert_eq!(e.yield_(rod), r["yield"].as_f64().unwrap());
    assert_eq!(e.poison(rod), r["xenon"].as_f64().unwrap());
    assert_eq!(e.core_heat(rod), r["core"].as_f64().unwrap());
    assert_eq!(e.hull_heat(rod), r["hull"].as_f64().unwrap());

    let mut foreign = absent.clone();
    foreign.foreign.insert("foreign".into(), 123.0);
    foreign.set_poison(rod, 50.0);
    assert_stack(&foreign, &dflt["nbt_preserved_by_setter"], "foreign");
}

#[test]
fn every_rod_burn_and_heat_trace_matches_java() {
    let fx = fixture();
    let dials = default_dials();
    for r in fx["rods"].as_array().unwrap() {
        let name = r["variable"].as_str().unwrap();
        let rod = RodType::by_variable(name).unwrap();

        let fresh = RodStack::fresh(rod);
        assert_stack(&fresh, &r["fresh_stack"], name);
        let mut f2 = fresh.clone();
        assert_bits(f2.durability_for_display(rod), &r["fresh_durability"], name);

        let mut used = fresh;
        used.set_yield(rod, rod.yield_ * 0.2);
        used.set_poison(rod, 50.0);
        used.set_core_heat(rod, 500.0);
        used.set_hull_heat(rod, 200.0);
        assert_stack(&used, &r["used_stack"], name);

        for t in r["burn_heat_trace"].as_array().unwrap() {
            let out = used.burn(rod, &dials, 10.0);
            used.update_heat(rod, &dials, 1.0);
            let provided = used.provide_heat(rod, &dials, 20.0, 1.0);
            let ctx = format!("{name} tick {}", t["tick"]);
            assert_bits(out, &t["out_flux"], &ctx);
            assert_bits(provided, &t["provided_heat"], &ctx);
            assert_stack(&used, &t["stack"], &ctx);
        }
    }
}

#[test]
fn discriminating_cases_match_java() {
    let fx = fixture();
    let dials = default_dials();
    let mut rod = RodType::blank();
    rod.yield_ = 1000.0;
    rod.reactivity = 20.0;
    rod.function = BurnFunc::LogTen;
    rod.depletion = DepleteFunc::Linear;
    let cases = fx["discriminants"].as_array().unwrap();
    let mut it = cases.iter();

    let c = it.next().unwrap();
    assert_bits(rod.reactivity_func(9.0, 1.0), &c["result"], "log_ten_9_20");

    rod.function = BurnFunc::Linear;
    let mut s = RodStack::fresh(&rod);
    s.set_poison(&rod, 50.0);
    let c = it.next().unwrap();
    let out = s.burn(&rod, &dials, 10.0);
    assert_bits(out, &c["out_flux"], "xenon_uses_old_poison");
    assert_stack(&s, &c["stack"], "xenon_uses_old_poison");

    for hull in [999.0, 1000.0, 1001.0] {
        let c = it.next().unwrap();
        let mut s = RodStack::fresh(&rod);
        s.set_hull_heat(&rod, hull);
        s.set_core_heat(&rod, 2000.0);
        let p = s.provide_heat(&rod, &dials, 100.0, 1.0);
        assert_bits(p, &c["provided"], "hull_melting_point");
        assert_stack(&s, &c["stack"], "hull_melting_point");
    }

    rod.heat_coeff_start = 1000.0;
    rod.heat_coeff_length = 500.0;
    for core in [999.0, 1000.0, 1250.0, 1500.0, 1501.0, f64::NAN, f64::INFINITY] {
        let c = it.next().unwrap();
        let mut s = RodStack::fresh(&rod);
        s.set_core_heat(&rod, core);
        assert_bits(core, &c["core_before"], "core_before");
        let f = s.burn(&rod, &dials, 10.0);
        assert_bits(f, &c["out_flux"], "heat_coefficient_rectify");
        assert_stack(&s, &c["stack"], "heat_coefficient_rectify");
    }

    for key in ["dialDisableDepletion", "dialDisableXenon"] {
        let c = it.next().unwrap();
        assert_eq!(c["name"], key);
        let mut rules = GameRules::default();
        rules.set(key, "true");
        let d = Dials::refresh(&rules, false);
        let mut s = RodStack::fresh(&rod);
        s.set_poison(&rod, 50.0);
        let out = s.burn(&rod, &d, 10.0);
        assert_bits(out, &c["out_flux"], key);
        assert_stack(&s, &c["stack"], key);
    }
    assert!(it.next().is_none());
}

/// `jmath` against the bits HotSpot returned (tools/rbmk-oracle/java/MathProbe.java).
#[test]
fn jmath_matches_hotspot() {
    use ntm_core::rbmk::jmath;
    let mut miss = std::collections::BTreeMap::<&str, (u32, u32)>::new();
    for line in include_str!("../../../tools/rbmk-oracle/fixtures/jmath.txt").lines() {
        let mut it = line.split(' ');
        let kind = it.next().unwrap();
        let arg = f64::from_bits(it.next().unwrap().parse::<i64>().unwrap() as u64);
        let want = it.next().unwrap().parse::<i64>().unwrap() as u64;
        let got = match kind {
            "log10" => jmath::log10(arg),
            "powe" => jmath::pow_e(arg),
            "sin" => jmath::sin(arg),
            "cos" => jmath::cos(arg),
            _ => panic!("{kind}"),
        };
        let e = miss.entry(kind).or_default();
        e.0 += 1;
        e.1 += u32::from(got.to_bits() != want);
    }
    eprintln!("jmath misses (total, mismatched): {miss:?}");
    // log10 reproduces the stubs; pow(E, y), sin and cos have a known tiny residual
    assert_eq!(miss["log10"].1, 0);
    assert!(miss["powe"].1 * 100 <= miss["powe"].0, "powe {:?}", miss["powe"]);
    assert!(miss["sin"].1 * 100 <= miss["sin"].0, "sin {:?}", miss["sin"]);
    assert!(miss["cos"].1 * 100 <= miss["cos"].0, "cos {:?}", miss["cos"]);
}
