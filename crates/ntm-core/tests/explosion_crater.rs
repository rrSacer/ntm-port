mod common;
use common::*;
use ntm_core::explosion::Plan;
use std::collections::HashSet;

fn ours(r: i32) -> (HashSet<[i32; 3]>, Plan) {
    let mut plan = Plan::new(center(), r);
    let mut read = |x: i32, y: i32, z: i32| Some(terrain(x, y, z).to_string());
    plan.step(u64::MAX, &mut read);
    let mut set = HashSet::new();
    for sx in plan.sec0[0]..plan.sec0[0] + plan.sec_dim[0] {
        for sy in plan.sec0[1]..plan.sec0[1] + plan.sec_dim[1] {
            for sz in plan.sec0[2]..plan.sec0[2] + plan.sec_dim[2] {
                set.extend(plan.section_cells([sx, sy, sz]));
            }
        }
    }
    (set, plan)
}

/// A cratera segue o algoritmo de raios do NTM: mesma profundidade no
/// centro, volume a poucos % e Jaccard alto (no benchmark: 0,94 em R=25,
/// 0,92 em R=50, 0,89 em R=100).
#[test]
fn matches_ntm_rays() {
    for (r, min_jaccard) in [(25, 0.92), (40, 0.9)] {
        let c = center();
        let (a, plan) = ours(r);
        let (b, reads) = ntm(c, r);
        let both = a.intersection(&b).count();
        let jaccard = both as f64 / (a.len() + b.len() - both) as f64;
        let depth = |s: &HashSet<[i32; 3]>| (0..=r).take_while(|&d| s.contains(&[0, c[1] - 1 - d, 0])).count();
        assert_eq!(depth(&a), depth(&b), "R={r}");
        assert!(jaccard >= min_jaccard, "R={r}: jaccard {jaccard}");
        let ratio = a.len() as f64 / b.len() as f64;
        assert!((0.95..1.06).contains(&ratio), "R={r}: volume {ratio}");
        assert!(plan.reads * 8 < reads, "R={r}: {} leituras contra {reads}", plan.reads);
    }
}

/// O plano dividido em pedaços (como nos ticks) dá o mesmo resultado que de uma vez.
#[test]
fn stepping_is_deterministic() {
    let (a, _) = ours(30);
    let mut plan = Plan::new(center(), 30);
    let mut read = |x: i32, y: i32, z: i32| Some(terrain(x, y, z).to_string());
    while !plan.finished() {
        plan.step(50_000, &mut read);
    }
    let mut b = HashSet::new();
    for sx in plan.sec0[0]..plan.sec0[0] + plan.sec_dim[0] {
        for sy in plan.sec0[1]..plan.sec0[1] + plan.sec_dim[1] {
            for sz in plan.sec0[2]..plan.sec0[2] + plan.sec_dim[2] {
                b.extend(plan.section_cells([sx, sy, sz]));
            }
        }
    }
    assert_eq!(a, b);
}
