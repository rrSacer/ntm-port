//! Achados da revisão independente do M2 (P1-x, P2-x, ver docs/explosoes.md
//! §11): cada teste aqui falhava antes da correção.
mod common;
use common::*;
use ntm_explosion::fallout::{self, FalloutProfile, Sweep};
use ntm_explosion::{Detonation, Event, Explosions, Hooks, Phase};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::panic::{AssertUnwindSafe, catch_unwind};

const DIM: &str = "minecraft:overworld";

#[derive(Default)]
struct Rec(Vec<Event>);
impl Hooks for Rec {
    fn event(&mut self, _id: &str, e: &Event) {
        self.0.push(e.clone());
    }
}

fn at(x: i32, z: i32) -> [i32; 3] {
    [x, surface(x, z) + 1, z]
}

/// Só a cratera e as bordas.
fn det(id: &str, c: [i32; 3], r: i32) -> Detonation {
    let mut d = Detonation::new(id, DIM, c, r);
    d.budget = 2048;
    d.profile.fallout = None;
    d
}

/// Com o fallout de 2,5R.
fn det_fallout(id: &str, c: [i32; 3], r: i32) -> Detonation {
    let mut d = Detonation::new(id, DIM, c, r);
    d.budget = 4096;
    d
}

fn run(ex: &mut Explosions, w: &mut FakeWorld, s: &mut FakeStore, h: &mut Rec, from: u64, limit: u64, mut stop: impl FnMut(u64, &Explosions) -> bool) -> u64 {
    let mut t = from;
    while !ex.is_empty() && t < from + limit {
        t += 1;
        w.begin_tick();
        let _ = ex.tick(t, w, s, h);
        if stop(t, ex) {
            break;
        }
    }
    t
}

fn hdist(c: [i32; 3], p: &(i32, i32, i32)) -> f32 {
    (((p.0 - c[0]).pow(2) + (p.2 - c[2]).pow(2)) as f32).sqrt()
}

// ------------------------------------------------------------------ P1-6

/// Uma explosão na fila (atrás de outra que tem o tick) volta depois de um
/// reinício: antes, só a da frente tinha sido gravada.
#[test]
fn p1_6_queued_explosions_survive_a_restart() {
    let (mut w, mut s, mut h) = (FakeWorld::default(), FakeStore::default(), Rec::default());
    let mut ex = Explosions::new();
    ex.detonate(det("a", at(0, 0), 30), 0).unwrap();
    ex.detonate(det("b", at(300, 0), 16), 0).unwrap();
    run(&mut ex, &mut w, &mut s, &mut h, 0, 6, |_, _| false);
    drop(ex);
    let mut ex = Explosions::new();
    let got: HashSet<String> = ex.resume(&mut s, 6, &mut h).into_iter().filter_map(Result::ok).collect();
    assert_eq!(got, HashSet::from(["a".to_string(), "b".to_string()]));
}

// ------------------------------------------------------------------ P1-7

thread_local! {
    static LEFT: Cell<usize> = const { Cell::new(0) };
    static WRITES: Cell<usize> = const { Cell::new(0) };
}

/// Rasga a n-ésima gravação (`LEFT`).
fn nth_write(k: &str, _v: &[u8]) -> bool {
    LEFT.with(|l| {
        let n = l.get();
        l.set(n.saturating_sub(1));
        if n == 1 {
            eprintln!("rasgada: {k}");
        }
        n == 1
    })
}

/// Só conta as gravações.
fn count_writes(_k: &str, _v: &[u8]) -> bool {
    WRITES.with(|w| w.set(w.get() + 1));
    false
}

fn quiet<T>(f: impl FnOnce() -> T) -> std::thread::Result<T> {
    catch_unwind(AssertUnwindSafe(f))
}

/// O servidor cai no meio da n-ésima gravação do storage (o arquivo fica
/// pela metade). Depois do reinício a explosão termina igual à referência,
/// ou, se ainda não estava gravada nada, o mundo não foi tocado.
fn torn_case(n: usize, reference: &FakeWorld) {
    let (mut w, mut s, mut h) = (FakeWorld::default(), FakeStore::default(), Rec::default());
    let mut ex = Explosions::new();
    ex.detonate(det("t", at(0, 0), 40), 0).unwrap();
    LEFT.with(|l| l.set(n));
    s.tear = Some(nth_write);
    let mut t = 0;
    let crashed = quiet(|| run(&mut ex, &mut w, &mut s, &mut h, 0, 5000, |tick, _| {
        t = tick;
        false
    }))
    .is_err();
    assert!(crashed, "a gravação {n} não aconteceu");
    s.tear = None;
    drop(ex);
    let mut ex = Explosions::new();
    let resumed = ex.resume(&mut s, t + 1, &mut h);
    if !resumed.iter().any(Result::is_ok) {
        // Ainda não estava gravada: quem detonou (a fila do mod) refaz.
        assert!(w.diff().is_empty(), "queda na gravação {n}: nada voltou ({resumed:?}), mas o mundo já mudou");
        return;
    }
    assert_eq!(resumed, vec![Ok("t".to_string())], "queda na gravação {n}");
    h.0.clear();
    run(&mut ex, &mut w, &mut s, &mut h, t + 1, 5000, |_, _| false);
    assert!(ex.is_empty());
    if h.0.iter().any(|e| matches!(e, Event::ResumeFailed { .. })) {
        // O índice foi gravado, o estado inicial não: nada a retomar, e as
        // chaves saem.
        assert!(w.diff().is_empty(), "queda na gravação {n}: retomada falhou ({:?}), mas o mundo já mudou", h.0);
        assert!(s.keys.is_empty(), "sobrou {:?}", s.keys.keys().collect::<Vec<_>>());
        return;
    }
    let (a, b) = (reference.diff(), w.diff());
    let only_ref = a.iter().filter(|(k, v)| b.get(*k) != Some(*v)).count();
    let only_new = b.iter().filter(|(k, v)| a.get(*k) != Some(*v)).count();
    assert!(only_ref == 0 && only_new == 0, "queda na gravação {n}: {only_ref} só na referência, {only_new} só na retomada, de {}", a.len());
    assert!(s.keys.is_empty(), "sobrou {:?}", s.keys.keys().collect::<Vec<_>>());
}

#[test]
fn p1_7_a_torn_write_never_loses_the_explosion() {
    let (mut w, mut s, mut h) = (FakeWorld::default(), FakeStore::default(), Rec::default());
    let mut ex = Explosions::new();
    ex.detonate(det("t", at(0, 0), 40), 0).unwrap();
    WRITES.with(|c| c.set(0));
    s.tear = Some(count_writes);
    run(&mut ex, &mut w, &mut s, &mut h, 0, 5000, |_, _| false);
    let total = WRITES.with(Cell::get);
    assert!(total > 10, "{total} gravações");
    let step = (total / 48).max(1);
    for n in (1..total).step_by(step) {
        torn_case(n, &w);
    }
}

/// A passagem para "só pendentes" (`linger`) cai no meio da gravação do
/// cabeçalho novo: antes, o diário já tinha sido apagado e a explosão se
/// perdia.
#[test]
fn p1_7_a_torn_linger_header_keeps_the_pending_fallout() {
    fn linger(_k: &str, v: &[u8]) -> bool {
        String::from_utf8_lossy(v).contains("phase=linger")
    }
    let c = at(3500, -1400);
    let far = ((c[0] + 52) >> 4, c[2] >> 4);
    let (mut w, mut s, mut h) = (FakeWorld::default(), FakeStore::default(), Rec::default());
    w.unloaded.insert(far);
    let mut ex = Explosions::new();
    ex.detonate(det_fallout("f1", c, 24), 0).unwrap();
    s.tear = Some(linger);
    let mut t = 0;
    let crashed = quiet(|| run(&mut ex, &mut w, &mut s, &mut h, 0, 5000, |tick, _| {
        t = tick;
        false
    }))
    .is_err();
    assert!(crashed, "não chegou à gravação do linger");
    s.tear = None;
    drop(ex);
    let mut ex = Explosions::new();
    assert_eq!(ex.resume(&mut s, t + 1, &mut h), vec![Ok("f1".to_string())]);
    run(&mut ex, &mut w, &mut s, &mut h, t + 1, 5000, |_, _| false);
    assert_eq!(ex.lingering().map(|(_, n)| n).collect::<Vec<_>>(), vec![1]);
    w.unloaded.clear();
    w.viewers = vec![[c[0] as f64 + 52.0, 70.0, c[2] as f64]];
    let mut t2 = t + 5000;
    for _ in 0..200 {
        t2 += 1;
        ex.tick(t2, &mut w, &mut s, &mut h);
    }
    assert_eq!(ex.lingering().count(), 0);
    assert!(s.keys.is_empty(), "sobrou {:?}", s.keys.keys().collect::<Vec<_>>());
    let n = w.diff().keys().filter(|p| (p.0 >> 4, p.2 >> 4) == far).count();
    assert!(n > 0, "o chunk pendente não recebeu o fallout");
}

// ------------------------------------------------------------------ P1-4

/// Um chunk pendente do fallout é feito aos pedaços, sem passar das
/// escritas nem do fuel de uma chamada, e o resultado é o mesmo.
#[test]
fn p1_4_a_pending_chunk_is_done_in_pieces_within_budget() {
    let c = at(0, 0);
    let far = ((c[0] + 40) >> 4, c[2] >> 4);
    let p = FalloutProfile::ntm_placeholder();
    let viewers = [[c[0] as f64 + 40.0, 70.0, c[2] as f64]];
    let reads = Cell::new(0u64);
    let mut read = |x: i32, y: i32, z: i32| {
        reads.set(reads.get() + 1);
        (-64..=319).contains(&y).then(|| terrain(x, y, z).to_string())
    };
    let mut whole = Sweep::resume(c, 24, 2.5, 25, vec![far]);
    let mut all = Vec::new();
    whole.pending_step(&p, &viewers, 160.0, u64::MAX / 4, usize::MAX / 2, &mut read, &mut all);
    assert!(whole.pending.is_empty());
    assert!(all.len() > 32, "{} escritas no chunk", all.len());
    let want: HashMap<(i32, i32, i32), String> = all.into_iter().map(|c| ((c.x, c.y, c.z), c.state)).collect();

    let (fuel, budget) = (3_000_000u64, 32usize);
    let mut s = Sweep::resume(c, 24, 2.5, 25, vec![far]);
    let mut got = HashMap::new();
    let mut calls = 0;
    while !s.pending.is_empty() && calls < 200 {
        reads.set(0);
        let mut out = Vec::new();
        s.pending_step(&p, &viewers, 160.0, fuel, budget, &mut read, &mut out);
        assert!(out.len() <= budget, "{} escritas numa chamada (orçamento {budget})", out.len());
        // Uma coluna além do fuel, no máximo (a conferência é por coluna).
        assert!(reads.get() * fallout::COST_READ <= fuel + 400 * fallout::COST_READ, "{} leituras numa chamada", reads.get());
        got.extend(out.into_iter().map(|c| ((c.x, c.y, c.z), c.state)));
        calls += 1;
    }
    assert!(s.pending.is_empty(), "o chunk não terminou em {calls} chamadas");
    assert_eq!(got, want);
}

// ------------------------------------------------------------------ P1-3

/// Vários fallouts pendentes dividem o fuel do tick (antes, cada um recebia
/// o fuel inteiro).
#[test]
fn p1_3_lingering_fallouts_share_the_tick() {
    let (mut w, mut s, mut h) = (FakeWorld::default(), FakeStore::default(), Rec::default());
    let mut ex = Explosions::new();
    let centers: Vec<[i32; 3]> = (0..4).map(|i| at(3500 + i * 400, -1400)).collect();
    let fars: Vec<(i32, i32)> = centers.iter().map(|c| ((c[0] + 28) >> 4, c[2] >> 4)).collect();
    for (i, c) in centers.iter().enumerate() {
        w.unloaded.insert(fars[i]);
        ex.detonate(det_fallout(&format!("f{i}"), *c, 16), 0).unwrap();
    }
    let t = run(&mut ex, &mut w, &mut s, &mut h, 0, 8000, |_, _| false);
    assert!(ex.is_empty());
    assert_eq!(ex.lingering().count(), 4);
    w.unloaded.clear();
    w.viewers = centers.iter().map(|c| [c[0] as f64 + 28.0, 70.0, c[2] as f64]).collect();
    ex.fuel = 6_000_000;
    w.tick_reads.clear();
    let mut t2 = t;
    for _ in 0..400 {
        t2 += 1;
        w.begin_tick();
        ex.tick(t2, &mut w, &mut s, &mut h);
    }
    assert_eq!(ex.lingering().count(), 0, "não terminou");
    let worst = w.tick_reads.iter().copied().max().unwrap_or(0) as u64;
    assert!(worst * fallout::COST_READ <= ex.fuel, "pior tick: {worst} leituras, {} M de fuel estimado contra {} M", worst * fallout::COST_READ / 1_000_000, ex.fuel / 1_000_000);
}

// ------------------------------------------------------------------ P1-2

/// Detonar ou retomar várias explosões no mesmo tick não monta vários planos
/// nele (R=500 custa ~21 M de fuel cada um).
#[test]
fn p1_2_one_plan_per_tick() {
    let (mut w, mut s, mut h) = (FakeWorld::default(), FakeStore::default(), Rec::default());
    let mut ex = Explosions::new();
    for i in 0..3 {
        ex.detonate(det(&format!("p{i}"), at(i * 300, 0), 20), 0).unwrap();
    }
    assert!(ex.active().count() <= 1, "{} planos montados na detonação", ex.active().count());
    let mut built = ex.active().count();
    let (mut t, mut finished) = (0, 0);
    while built < 3 && t < 50 {
        t += 1;
        finished += ex.tick(t, &mut w, &mut s, &mut h).len();
        let now = ex.active().count() + finished;
        assert!(now <= built + 1, "{} planos novos no tick {t}", now - built);
        built = now;
    }
    // Reinício: as três voltam, uma por tick.
    let mut ex = Explosions::new();
    let r = ex.resume(&mut s, t + 1, &mut h);
    assert_eq!(r.len(), 3);
    assert!(ex.active().count() <= 1, "{} planos montados na retomada", ex.active().count());
}

// ------------------------------------------------------------------ P2-8

/// Persistência leve retomada no meio do fallout com seções da cratera
/// ainda esperando chunk: antes, a retomada as dava por feitas.
#[test]
fn p2_8_lite_resume_in_fallout_keeps_deferred_crater() {
    let c = at(3500, -1400);
    let r = 24;
    let hole = ((c[0] + 16) >> 4, c[2] >> 4);
    let mut d = det_fallout("f1", c, r);
    d.lite_above = 10;
    let in_hole = |w: &FakeWorld| {
        w.diff()
            .iter()
            .filter(|(p, v)| (p.0 >> 4, p.2 >> 4) == hole && hdist(c, p) <= r as f32 && v.as_str() == "minecraft:air")
            .count()
    };
    // Referência: sem reinício.
    let (mut rw, mut rs, mut rh) = (FakeWorld::default(), FakeStore::default(), Rec::default());
    rw.unloaded.insert(hole);
    let mut ex = Explosions::new();
    ex.detonate(d.clone(), 0).unwrap();
    let t = run(&mut ex, &mut rw, &mut rs, &mut rh, 0, 3000, |_, ex| ex.progress("f1").is_some_and(|p| p.phase == Phase::Fallout && p.fallout_columns > 0));
    rw.unloaded.clear();
    run(&mut ex, &mut rw, &mut rs, &mut rh, t, 5000, |_, _| false);
    let want = in_hole(&rw);
    assert!(want > 0);

    let (mut w, mut s, mut h) = (FakeWorld::default(), FakeStore::default(), Rec::default());
    w.unloaded.insert(hole);
    let mut ex = Explosions::new();
    ex.detonate(d, 0).unwrap();
    let mut seen = 0;
    let t = run(&mut ex, &mut w, &mut s, &mut h, 0, 3000, |t, ex| {
        if seen == 0 && ex.progress("f1").is_some_and(|p| p.phase == Phase::Fallout && p.fallout_columns > 0) {
            seen = t;
        }
        seen > 0 && t >= seen + 25
    });
    assert!(ex.progress("f1").is_some_and(|p| p.phase == Phase::Fallout && p.deferred > 0), "{:?}", ex.progress("f1").map(|p| p.line()));
    drop(ex);
    let mut ex = Explosions::new();
    assert_eq!(ex.resume(&mut s, t + 1, &mut h), vec![Ok("f1".to_string())]);
    w.unloaded.clear();
    run(&mut ex, &mut w, &mut s, &mut h, t + 1, 5000, |_, _| false);
    assert!(ex.is_empty());
    let got = in_hole(&w);
    assert!(got * 10 >= want * 9, "cratera no chunk atrasado depois da retomada: {got} contra {want}");
}

// ------------------------------------------------------------------ P2-11

/// O diário completo não passa da parte do storage que o mod deu às
/// explosões: quando não cabe, passa à persistência leve (e continua
/// retomável), sem tirar espaço do resto do mod.
#[test]
fn p2_11_the_journal_keeps_to_its_share() {
    let (mut w, mut s, mut h) = (FakeWorld::default(), FakeStore::default(), Rec::default());
    let mut ex = Explosions::new();
    ex.limits.total_bytes = 48 << 10;
    ex.detonate(det("q", at(0, 0), 60), 0).unwrap();
    let mut peak = 0;
    let t = run(&mut ex, &mut w, &mut s, &mut h, 0, 40, |_, _| false);
    peak = peak.max(s.max_total);
    assert!(!ex.is_empty(), "terminou cedo demais");
    assert!(peak <= (48 << 10) + 1024, "{peak} bytes, acima da parte de 48 KiB");
    assert!(h.0.iter().any(|e| matches!(e, Event::PersistFailed { reason } if reason.contains("leve"))), "{:?}", h.0);
    // Ainda retomável (leve).
    drop(ex);
    let mut ex = Explosions::new();
    ex.limits.total_bytes = 48 << 10;
    assert_eq!(ex.resume(&mut s, t + 1, &mut h), vec![Ok("q".to_string())]);
    run(&mut ex, &mut w, &mut s, &mut h, t + 1, 8000, |_, _| false);
    assert!(ex.is_empty());
    assert!(s.max_total <= (48 << 10) + 1024, "{} bytes", s.max_total);
    assert!(s.keys.is_empty());
}
