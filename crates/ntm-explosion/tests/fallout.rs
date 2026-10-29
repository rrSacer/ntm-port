//! A segunda fase (fallout de 2,5R) e a persistência leve, no mundo falso.
mod common;
use common::*;
use ntm_explosion::{Detonation, Event, Explosions, Hooks, Phase};
use std::collections::{HashMap, HashSet};

const DIM: &str = "minecraft:overworld";
const R: i32 = 24;

#[derive(Default)]
struct Rec(Vec<Event>);
impl Hooks for Rec {
    fn event(&mut self, _id: &str, e: &Event) {
        self.0.push(e.clone());
    }
}

fn c() -> [i32; 3] {
    [3500, surface(3500, -1400) + 1, -1400]
}

fn det(r: i32) -> Detonation {
    let mut d = Detonation::new("f1", DIM, c(), r);
    d.budget = 4096;
    d
}

/// Roda até `stop` ou até não haver explosão ativa.
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

fn h(p: &(i32, i32, i32)) -> f32 {
    let c = c();
    (((p.0 - c[0]).pow(2) + (p.2 - c[2]).pow(2)) as f32).sqrt()
}

fn full() -> (FakeWorld, FakeStore, Rec) {
    let (mut w, mut s, mut r) = (FakeWorld::default(), FakeStore::default(), Rec::default());
    let mut ex = Explosions::new();
    ex.detonate(det(R), 0).unwrap();
    run(&mut ex, &mut w, &mut s, &mut r, 0, 5000, |_, _| false);
    assert!(ex.is_empty());
    assert_eq!(ex.lingering().count(), 0);
    (w, s, r)
}

/// Mudanças fora do disco da cratera.
fn outside(w: &FakeWorld) -> HashMap<(i32, i32, i32), String> {
    w.diff().into_iter().filter(|(p, _)| h(p) > R as f32).collect()
}

#[test]
fn fallout_covers_two_and_a_half_r_with_the_ntm_rules() {
    let (w, s, r) = full();
    assert!(r.0.iter().any(|e| matches!(e, Event::FalloutStart { .. })));
    assert!(r.0.iter().any(|e| matches!(e, Event::Done { .. })));
    assert!(s.keys.is_empty(), "sobrou {:?}", s.keys.keys().collect::<Vec<_>>());
    let out = outside(&w);
    let outer = 2.5 * R as f32;
    assert!(out.len() > 100, "fallout mudou só {} blocos", out.len());
    let mut kinds: HashMap<&str, usize> = HashMap::new();
    for (p, v) in &out {
        let d = h(p) / outer;
        assert!(d <= 1.0, "mudança além de 2,5R em {p:?}: {v}");
        let name = v.split('[').next().unwrap();
        *kinds.entry(name).or_default() += 1;
        match name {
            "minecraft:blackstone" | "minecraft:deepslate" | "minecraft:tuff" => assert!(d < 0.5, "escória a {d}"),
            "minecraft:basalt" | "minecraft:dark_oak_planks" | "minecraft:fire" => assert!(d < 0.65, "madeira/fogo a {d}"),
            "minecraft:air" => assert!(d < 0.65, "folhagem removida a {d}"),
            "minecraft:light_gray_carpet" | "minecraft:green_stained_glass" => {}
            other => panic!("bloco inesperado {other} em {p:?}"),
        }
    }
    eprintln!("fallout R={R}: {} mudanças {kinds:?}", out.len());
    for k in ["minecraft:tuff", "minecraft:light_gray_carpet"] {
        assert!(kinds.contains_key(k), "nenhum {k}");
    }
    // Folhas expostas até 65% somem; nenhuma ficou nesse anel.
    let c = c();
    for x in c[0] - 40..=c[0] + 40 {
        for z in c[2] - 40..=c[2] + 40 {
            let hh = h(&(x, 0, z));
            if hh <= R as f32 || hh / outer >= 0.6 {
                continue;
            }
            for y in 60..90 {
                assert!(!w.get(x, y, z).contains("leaves"), "folha em {x} {y} {z}");
            }
        }
    }
}

#[test]
fn fallout_resumes_from_its_ring_after_a_restart() {
    let (reference, ..) = full();
    let (mut w, mut s, mut r) = (FakeWorld::default(), FakeStore::default(), Rec::default());
    let mut ex = Explosions::new();
    ex.detonate(det(R), 0).unwrap();
    // Para logo depois de o fallout começar.
    let mut started = 0;
    let t = run(&mut ex, &mut w, &mut s, &mut r, 0, 5000, |t, ex| {
        if started == 0 && ex.progress("f1").is_some_and(|p| p.phase == Phase::Fallout) {
            started = t;
        }
        started > 0 && t > started
    });
    assert!(!ex.is_empty(), "terminou antes de parar");
    drop(ex);
    // E o mundo volta ao último autosave (sem nada da explosão).
    w.over.clear();
    let mut ex = Explosions::new();
    assert_eq!(ex.resume(&mut s, t, &mut r), vec![Ok("f1".to_string())]);
    run(&mut ex, &mut w, &mut s, &mut r, t, 5000, |_, _| false);
    assert!(ex.is_empty());
    assert!(s.keys.is_empty());
    let (a, b) = (reference.diff(), w.diff());
    // A cratera volta inteira; o fallout recomeça do anel salvo (o que veio
    // antes dele, perdido no rollback, não volta: aproximação aceita).
    let crater = |m: &HashMap<(i32, i32, i32), String>| m.iter().filter(|(p, _)| h(p) <= R as f32).map(|(p, v)| (*p, v.clone())).collect::<HashSet<_>>();
    assert_eq!(crater(&a), crater(&b));
    let fa = outside(&reference).len();
    let fb = outside(&w).len();
    assert!(fb * 10 >= fa * 7, "fallout retomado: {fb} de {fa}");
}

#[test]
fn unloaded_fallout_chunks_wait_for_a_player() {
    let (reference, ..) = full();
    let (mut w, mut s, mut r) = (FakeWorld::default(), FakeStore::default(), Rec::default());
    let c = c();
    // Um chunk inteiro do anel do fallout, longe da cratera, sem carregar.
    let far = ((c[0] + 52) >> 4, c[2] >> 4);
    w.unloaded.insert(far);
    let mut ex = Explosions::new();
    ex.detonate(det(R), 0).unwrap();
    let t = run(&mut ex, &mut w, &mut s, &mut r, 0, 5000, |_, _| false);
    assert!(ex.is_empty());
    let pend: Vec<_> = ex.lingering().map(|(id, n)| (id.to_string(), n)).collect();
    assert_eq!(pend, vec![("f1".to_string(), 1)]);
    // Só o estado (dois slots) e o índice ficam no storage, sem o diário.
    assert!(s.keys.keys().all(|k| k.starts_with("nx.index") || k.starts_with("nx.f1.s")), "{:?}", s.keys.keys().collect::<Vec<_>>());
    // Reinício com o chunk ainda longe: volta como pendente (no tick seguinte).
    let mut ex = Explosions::new();
    assert_eq!(ex.resume(&mut s, t, &mut r), vec![Ok("f1".to_string())]);
    ex.tick(t + 1, &mut w, &mut s, &mut r);
    assert_eq!(ex.lingering().count(), 1);
    // Ninguém perto: nada acontece mesmo com o chunk carregado.
    w.unloaded.clear();
    w.viewers = vec![[c[0] as f64 + 2000.0, 70.0, c[2] as f64]];
    let mut t2 = t;
    for _ in 0..100 {
        t2 += 1;
        ex.tick(t2, &mut w, &mut s, &mut r);
    }
    assert_eq!(ex.lingering().count(), 1);
    // Um jogador chega perto: o chunk é feito e o storage fica limpo.
    w.viewers = vec![[c[0] as f64 + 52.0, 70.0, c[2] as f64]];
    for _ in 0..100 {
        t2 += 1;
        ex.tick(t2, &mut w, &mut s, &mut r);
    }
    assert_eq!(ex.lingering().count(), 0);
    assert!(s.keys.is_empty(), "{:?}", s.keys.keys().collect::<Vec<_>>());
    // O chunk recebeu o mesmo fallout que na referência (a busca do topo
    // pode começar diferente: tolerância pequena).
    let in_far = |m: &HashMap<(i32, i32, i32), String>| m.iter().filter(|(p, _)| (p.0 >> 4, p.2 >> 4) == far).count();
    let (a, b) = (in_far(&reference.diff()), in_far(&w.diff()));
    assert!(a > 0 && b * 10 >= a * 9 && b <= a + a / 10 + 2, "chunk pendente: {b} contra {a}");
}

#[test]
fn lite_persistence_skips_written_sections() {
    let r0 = 30;
    let lite = |d: &mut Detonation| {
        d.lite_above = 10;
        d.profile.fallout = None;
    };
    let mut d = det(r0);
    lite(&mut d);
    let (mut rw, mut rs, mut rr) = (FakeWorld::default(), FakeStore::default(), Rec::default());
    let mut ex = Explosions::new();
    ex.detonate(d.clone(), 0).unwrap();
    run(&mut ex, &mut rw, &mut rs, &mut rr, 0, 5000, |_, _| false);
    assert!(rs.keys.is_empty());
    let (mut w, mut s, mut r) = (FakeWorld::default(), FakeStore::default(), Rec::default());
    let mut ex = Explosions::new();
    ex.detonate(d, 0).unwrap();
    let t = run(&mut ex, &mut w, &mut s, &mut r, 0, 5000, |t, ex| {
        let _ = t;
        ex.progress("f1").is_some_and(|p| p.sections_done > 10)
    });
    assert!(!ex.is_empty(), "terminou antes de parar");
    // Só índice, cabeçalho e o bitmap das seções.
    assert!(s.keys.len() <= 3, "{:?}", s.keys.keys().collect::<Vec<_>>());
    assert!(s.keys.keys().all(|k| !k.contains(".d") && !k.contains(".e")));
    drop(ex);
    let mut ex = Explosions::new();
    assert_eq!(ex.resume(&mut s, t, &mut r), vec![Ok("f1".to_string())]);
    run(&mut ex, &mut w, &mut s, &mut r, t, 5000, |_, _| false);
    assert!(ex.is_empty() && s.keys.is_empty());
    // Tudo o que a referência removeu está removido; o recálculo sobre o ar
    // pode passar um pouco (aproximação documentada), nunca muito.
    let (a, b) = (rw.diff(), w.diff());
    for (p, v) in &a {
        if v == "minecraft:air" {
            assert_eq!(w.get(p.0, p.1, p.2), "minecraft:air", "{p:?}");
        }
    }
    // Medido: R=30 interrompido com 10+ seções escritas, +968 de 15 415 (6,3%).
    let extra = b.len().saturating_sub(a.len());
    eprintln!("retomada leve: {extra} a mais de {}", a.len());
    assert!(extra * 10 <= a.len(), "retomada leve removeu {extra} a mais de {}", a.len());
}


#[test]
fn a_crater_waiting_for_chunks_holds_neither_its_fallout_nor_other_blasts() {
    let (reference, ..) = full();
    let (mut w, mut s, mut r) = (FakeWorld::default(), FakeStore::default(), Rec::default());
    let c = c();
    // Um chunk dentro da cratera sem carregar.
    let hole = ((c[0] + 16) >> 4, c[2] >> 4);
    w.unloaded.insert(hole);
    let mut ex = Explosions::new();
    ex.detonate(det(R), 0).unwrap();
    let mut other = Detonation::new("f2", DIM, [c[0] + 400, surface(c[0] + 400, c[2]) + 1, c[2]], 12);
    other.budget = 4096;
    ex.detonate(other, 0).unwrap();
    // O plano de cada uma é montado num tick só para ele: a segunda aparece
    // e some quando termina.
    let mut seen = false;
    let t = run(&mut ex, &mut w, &mut s, &mut r, 0, 1500, |_, ex| {
        seen |= ex.progress("f2").is_some();
        seen && ex.progress("f2").is_none()
    });
    assert!(ex.progress("f2").is_none(), "a segunda ficou presa atrás da primeira");
    let p = ex.progress("f1").expect("a primeira espera o chunk");
    assert!(p.fallout_columns > 0, "o fallout não começou: {}", p.line());
    assert!(p.deferred > 0);
    // O chunk carrega: a cratera completa, e tudo acaba.
    w.unloaded.clear();
    run(&mut ex, &mut w, &mut s, &mut r, t, 3000, |_, _| false);
    assert!(ex.is_empty() && s.keys.is_empty(), "{:?} {:?}", s.keys.keys().collect::<Vec<_>>(), ex.progress("f1").map(|p| p.line()));
    let in_hole = |m: &HashMap<(i32, i32, i32), String>| m.iter().filter(|(p, v)| (p.0 >> 4, p.2 >> 4) == hole && h(p) <= R as f32 && v.as_str() == "minecraft:air").count();
    let (a, b) = (in_hole(&reference.diff()), in_hole(&w.diff()));
    assert!(a > 0 && b * 10 >= a * 9, "cratera no chunk atrasado: {b} contra {a}");
}
