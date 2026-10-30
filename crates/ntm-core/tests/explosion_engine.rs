mod common;
use common::*;
use ntm_core::explosion::{Detonation, EffectProfile, Event, Explosions, Hooks, Phase, Plan};
use ntm_core::explosion::persist::Store;
use std::collections::HashSet;

const DIM: &str = "minecraft:overworld";

#[derive(Default)]
struct Rec(Vec<Event>);
impl Hooks for Rec {
    fn event(&mut self, _id: &str, e: &Event) {
        self.0.push(e.clone());
    }
}

/// Só a cratera e as bordas (o fallout tem os testes dele, em `fallout.rs`).
fn det(r: i32, budget: usize) -> Detonation {
    let mut d = Detonation::new("t1", DIM, center(), r);
    d.budget = budget;
    d.profile.fallout = None;
    d
}

/// Roda até `stop(tick, ex)` ou até não haver explosão; devolve o último tick.
fn run(ex: &mut Explosions, w: &mut FakeWorld, s: &mut FakeStore, h: &mut Rec, from: u64, limit: u64, mut stop: impl FnMut(u64, &Explosions, &mut FakeWorld) -> bool) -> u64 {
    let mut t = from;
    while !ex.is_empty() && t < from + limit {
        t += 1;
        w.begin_tick();
        let _ = ex.tick(t, w, s, h);
        if stop(t, ex, w) {
            break;
        }
    }
    t
}

fn full_run(r: i32, budget: usize) -> (FakeWorld, FakeStore, Rec, u64) {
    let (mut w, mut s, mut h) = (FakeWorld::default(), FakeStore::default(), Rec::default());
    let mut ex = Explosions::new();
    ex.detonate(det(r, budget), 0).unwrap();
    let t = run(&mut ex, &mut w, &mut s, &mut h, 0, 5000, |_, _, _| false);
    assert!(ex.is_empty(), "não terminou em {t} ticks");
    (w, s, h, t)
}

#[test]
fn completes_within_budget_and_matches_plan() {
    let (w, s, h, _) = full_run(40, 2048);
    assert!(w.tick_writes.iter().all(|&n| n <= 2048), "tick com {:?}", w.tick_writes.iter().max());
    assert!(w.max_call <= 4096);
    assert!(h.0.iter().any(|e| matches!(e, Event::Done { .. })));
    assert!(!h.0.iter().any(|e| matches!(e, Event::PersistFailed { .. })));
    // O mundo final tem exatamente a cratera do plano (mais os efeitos de borda).
    let mut plan = Plan::new(center(), 40);
    let mut read = |x: i32, y: i32, z: i32| Some(terrain(x, y, z).to_string());
    plan.step(u64::MAX, &mut read);
    let mut removed = HashSet::new();
    for sx in plan.sec0[0]..plan.sec0[0] + plan.sec_dim[0] {
        for sy in plan.sec0[1]..plan.sec0[1] + plan.sec_dim[1] {
            for sz in plan.sec0[2]..plan.sec0[2] + plan.sec_dim[2] {
                removed.extend(plan.section_cells([sx, sy, sz]).into_iter().map(|p| (p[0], p[1], p[2])));
            }
        }
    }
    assert_eq!(removed.len() as u64, plan.removed);
    let diff = w.diff();
    let effects: HashSet<&str> = ["minecraft:blackstone", "minecraft:deepslate", "minecraft:tuff", "minecraft:basalt", "minecraft:dark_oak_planks", "minecraft:green_stained_glass", "minecraft:fire", "minecraft:air"].into();
    for p in &removed {
        let v = w.get(p.0, p.1, p.2);
        assert!(v == "minecraft:air" || v == "minecraft:fire", "{p:?} ficou {v}");
    }
    for (p, v) in &diff {
        assert!(removed.contains(p) || effects.contains(v.as_str()), "mudança inesperada em {p:?}: {v}");
    }
    // Storage limpo no fim.
    assert!(s.keys.is_empty(), "sobrou {:?}", s.keys.keys().collect::<Vec<_>>());
}

#[test]
fn unloaded_chunks_wait_and_are_recomputed() {
    let (reference, ..) = full_run(40, 4096);
    let (mut w, mut s, mut h) = (FakeWorld::default(), FakeStore::default(), Rec::default());
    let c = center();
    for cx in (c[0] - 48) >> 4..=(c[0] + 48) >> 4 {
        for cz in (c[2] - 48) >> 4..=(c[2] + 48) >> 4 {
            w.unloaded.insert((cx, cz));
        }
    }
    let mut ex = Explosions::new();
    ex.detonate(det(40, 4096), 0).unwrap();
    // Com tudo descarregado: o plano termina e as seções esperam.
    let t = run(&mut ex, &mut w, &mut s, &mut h, 0, 2000, |_, ex, _| {
        ex.progress("t1").is_some_and(|p| p.phase == Phase::Waiting)
    });
    let p = ex.progress("t1").unwrap();
    assert_eq!(p.phase, Phase::Waiting, "{} (tick {t})", p.line());
    assert_eq!(p.writes, 0);
    assert!(w.diff().is_empty());
    w.unloaded.clear();
    run(&mut ex, &mut w, &mut s, &mut h, t, 5000, |_, _, _| false);
    assert!(ex.is_empty());
    let (a, b) = (reference.diff(), w.diff());
    let same = a.iter().filter(|(k, v)| b.get(k) == Some(v)).count();
    let diff = a.len() + b.len() - 2 * same;
    // Chunks recalculados em lotes: raios que cruzam dois chunks recalculados
    // fora de ordem podem diferir um pouco (no jogo: 41 de 179 mil).
    assert!(diff * 200 <= a.len(), "{diff} de {} diferentes", a.len());
}

/// Interrompe (como um reinício do servidor) no tick `k`, retoma do storage e
/// compara com a explosão sem interrupção. `rollback` = o mundo volta ao
/// estado de antes da explosão (queda com autosave antigo).
fn resume_case(k: u64, rollback: bool) {
    resume_case_loading(k, rollback, 0)
}

/// Como `resume_case`, com o mundo sem nenhum chunk carregado nos primeiros
/// `late` ticks depois da retomada (servidor recém-iniciado).
fn resume_case_loading(k: u64, rollback: bool, late: u64) {
    let (reference, ..) = full_run(40, 2048);
    let (mut w, mut s, mut h) = (FakeWorld::default(), FakeStore::default(), Rec::default());
    let mut ex = Explosions::new();
    ex.detonate(det(40, 2048), 0).unwrap();
    let t = run(&mut ex, &mut w, &mut s, &mut h, 0, 5000, |t, _, _| t >= k);
    assert!(!ex.is_empty(), "terminou antes do tick {k}");
    drop(ex);
    if rollback {
        w.over.clear();
    }
    let mut ex = Explosions::new();
    let r = ex.resume(&mut s, t, &mut h);
    assert_eq!(r, vec![Ok("t1".to_string())]);
    let c = center();
    if late > 0 {
        for cx in (c[0] - 48) >> 4..=(c[0] + 48) >> 4 {
            for cz in (c[2] - 48) >> 4..=(c[2] + 48) >> 4 {
                w.unloaded.insert((cx, cz));
            }
        }
    }
    let t2 = run(&mut ex, &mut w, &mut s, &mut h, t, late, |_, _, _| false);
    if late > 0 {
        eprintln!("depois de {late} ticks sem chunks: {:?}", ex.progress("t1").map(|p| p.line()));
    }
    w.unloaded.clear();
    let mut last = None;
    run(&mut ex, &mut w, &mut s, &mut h, t2, 5000, |_, ex, _| {
        if let Some(p) = ex.progress("t1") {
            last = Some(p.line());
        }
        false
    });
    if late > 0 {
        eprintln!("fim: {last:?}");
    }
    assert!(ex.is_empty());
    let (a, b) = (reference.diff(), w.diff());
    if a != b {
        let only_ref = a.iter().filter(|(k, v)| b.get(*k) != Some(*v)).count();
        let only_new = b.iter().filter(|(k, v)| a.get(*k) != Some(*v)).count();
        panic!("interrompido no tick {k} (rollback={rollback}, late={late}): {only_ref} só na referência, {only_new} só na retomada, de {}", a.len());
    }
    assert!(s.keys.is_empty());
}

#[test]
fn resume_mid_plan() {
    for k in [2, 3, 5, 8] {
        resume_case(k, false);
    }
}

#[test]
fn resume_at_every_third_tick() {
    let (_, _, _, total) = full_run(40, 2048);
    for k in (1..total).step_by(3) {
        resume_case(k, k % 2 == 0);
    }
}

#[test]
fn resume_late_and_with_rollback() {
    let (_, _, _, total) = full_run(40, 2048);
    for k in [total / 2, total - 3, total - 1] {
        resume_case(k, false);
        resume_case(k, true);
    }
    resume_case(4, true);
}

#[test]
fn resume_waits_for_chunks_after_restart() {
    for k in [3, 6] {
        resume_case_loading(k, false, 40);
        resume_case_loading(k, true, 40);
    }
}

#[test]
fn persistence_fits_horizon_limits_at_r150() {
    let (mut w, mut s, mut h) = (FakeWorld::default(), FakeStore::default(), Rec::default());
    let mut ex = Explosions::new();
    ex.detonate(det(150, 8192), 0).unwrap();
    let mut peak = 0;
    let mut peak_keys = 0;
    run(&mut ex, &mut w, &mut s, &mut h, 0, 20000, |_, _, _| false);
    for e in &h.0 {
        assert!(!matches!(e, Event::PersistFailed { .. }), "{e:?}");
    }
    peak = peak.max(s.max_total);
    peak_keys = peak_keys.max(s.keys.len());
    println!("R=150: pico do storage {} bytes", s.max_total);
    assert!(peak < 4 << 20);
    assert!(peak_keys <= 64);
    assert!(ex.is_empty());
}

#[test]
fn cancel_forgets_storage() {
    let (mut w, mut s, mut h) = (FakeWorld::default(), FakeStore::default(), Rec::default());
    let mut ex = Explosions::new();
    ex.detonate(det(30, 1024), 0).unwrap();
    run(&mut ex, &mut w, &mut s, &mut h, 0, 5, |_, _, _| false);
    assert!(!s.keys.is_empty());
    assert!(ex.cancel("t1", &mut s));
    assert!(s.keys.is_empty());
}

#[test]
fn profile_roundtrip() {
    for p in [EffectProfile::ntm_placeholder(), EffectProfile::none()] {
        assert_eq!(EffectProfile::decode(&p.encode()).unwrap(), p);
    }
}

#[test]
fn rejects_bad_detonations() {
    let mut ex = Explosions::new();
    assert!(ex.detonate(Detonation::new("a", DIM, [0, 64, 0], 0), 0).is_err());
    assert!(ex.detonate(Detonation::new("a", DIM, [0, 64, 0], 501), 0).is_err());
    assert!(ex.detonate(Detonation::new("a.b", DIM, [0, 64, 0], 10), 0).is_err());
    assert!(ex.detonate(Detonation::new("a", DIM, [0, 64, 0], 10), 0).is_ok());
    assert!(ex.detonate(Detonation::new("a", DIM, [0, 64, 0], 10), 0).is_err());
    let _ = Store::get(&mut FakeStore::default(), "x");
}
