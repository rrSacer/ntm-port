//! Benchmark do cálculo da cratera contra o algoritmo de raios do NTM
//! (ExplosionNukeRayBatched) reimplementado aqui, no mesmo terreno sintético.
//! uso: cargo run --release --example compare -- 25 50 100 150
use ntm_core::explosion::crater::{Kind, Plan, classify, rand01};
use std::collections::HashSet;
use std::time::Instant;

/// Terreno sintético: colinas suaves, grama/terra/pedra/deepslate/bedrock,
/// água abaixo de y=62, árvores esparsas, minério raro, cavernas esparsas.
fn surface(x: i32, z: i32) -> i32 {
    64 + (4.0 * (x as f32 / 23.0).sin() + 3.0 * (z as f32 / 17.0).cos()).round() as i32
}

fn block(x: i32, y: i32, z: i32) -> &'static str {
    let h = surface(x, z);
    if y < -59 {
        return "minecraft:bedrock";
    }
    // árvore: tronco 5 + copa raio 2 (posições numa grade 7x7 com sorteio)
    let (tx, tz) = (x.div_euclid(7) * 7 + 3, z.div_euclid(7) * 7 + 3);
    if rand01(tx, 0, tz, 3) < 0.25 {
        let th = surface(tx, tz);
        if th >= 62 {
            if x == tx && z == tz && y > th && y <= th + 5 {
                return "minecraft:oak_log[axis=y]";
            }
            let (dx, dy, dz) = (x - tx, y - (th + 5), z - tz);
            if dx * dx + dy * dy + dz * dz <= 5 && y > th + 2 {
                return "minecraft:oak_leaves[distance=1,persistent=false,waterlogged=false]";
            }
        }
    }
    if y > h {
        return if y <= 62 { "minecraft:water[level=0]" } else { "minecraft:air" };
    }
    if y < h - 6 && rand01(x >> 2, y >> 2, z >> 2, 5) < 0.04 {
        return "minecraft:cave_air";
    }
    if y == h {
        return if h < 62 { "minecraft:sand" } else { "minecraft:grass_block[snowy=false]" };
    }
    if y >= h - 3 {
        return "minecraft:dirt";
    }
    if rand01(x, y, z, 9) < 0.01 {
        return "minecraft:coal_ore";
    }
    if y < 0 { "minecraft:deepslate[axis=y]" } else { "minecraft:stone" }
}

fn is_air(s: &str) -> bool {
    matches!(classify(s).kind, Kind::Air)
}

/// ExplosionNukeRayBatched (collectTip + processChunk), sem a divisão em ticks.
fn ntm(c: [i32; 3], r: i32) -> (HashSet<[i32; 3]>, u64) {
    let strength = 2 * r;
    let length = r;
    let max = (2.5 * std::f64::consts::PI * (strength as f64).powi(2)) as i64;
    let (mut gx, mut gy) = (std::f64::consts::PI, 0.0f64);
    let mut num = 1i64;
    let mut reads = 0u64;
    let mut out = HashSet::new();
    while num < max {
        let v = [gx.sin() * gy.cos(), gx.cos(), gx.sin() * gy.sin()];
        // generateGspUp
        if num < max {
            let k = num + 1;
            let hk = -1.0 + 2.0 * (k as f64 - 1.0) / (max as f64 - 1.0);
            gx = hk.acos();
            gy = (gy + 3.6 / (max as f64).sqrt() / (1.0 - hk * hk).sqrt()) % (std::f64::consts::PI * 2.0);
        }
        num += 1;
        let mut res = strength as f32;
        let mut tip: Option<[f32; 3]> = None;
        let len_i = strength; // (int) ceil(strength)
        for i in 0..len_i {
            if i > length {
                break;
            }
            let p = [
                (c[0] as f64 + v[0] * i as f64) as f32,
                (c[1] as f64 + v[1] * i as f64) as f32,
                (c[2] as f64 + v[2] * i as f64) as f32,
            ];
            let b = block(p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32);
            reads += 1;
            let m = classify(b);
            let fac = (100.0 - i as f64 / len_i as f64 * 100.0) * 0.07;
            if m.kind != Kind::Liquid {
                res -= (m.res as f64).powf(7.5 - fac) as f32;
            }
            if res > 0.0 && m.kind != Kind::Air {
                tip = Some(p);
            }
            if res <= 0.0 || i + 1 >= length || i == len_i - 1 {
                break;
            }
        }
        // processChunk: refaz a reta até a ponta e remove o que não é ar.
        if let Some(t) = tip {
            let d = [t[0] as f64 - c[0] as f64, t[1] as f64 - c[1] as f64, t[2] as f64 - c[2] as f64];
            let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            if l == 0.0 {
                continue;
            }
            let mut i = 0.0;
            while i < l {
                let q = [
                    (c[0] as f64 + d[0] / l * i).floor() as i32,
                    (c[1] as f64 + d[1] / l * i).floor() as i32,
                    (c[2] as f64 + d[2] / l * i).floor() as i32,
                ];
                reads += 1;
                if !is_air(block(q[0], q[1], q[2])) {
                    out.insert(q);
                }
                i += 1.0;
            }
            let q = [t[0].floor() as i32, t[1].floor() as i32, t[2].floor() as i32];
            out.insert(q);
        }
    }
    (out, reads)
}

fn main() {
    let radii: Vec<i32> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    let radii = if radii.is_empty() { vec![25, 50, 100, 150] } else { radii };
    for r in radii {
        let c = [0, surface(0, 0) + 1, 0];
        // Nosso: plano completo de uma vez.
        let t = Instant::now();
        let mut plan = Plan::new(c, r);
        plan.wide = std::env::var("NARROW").is_err();
        if let Ok(b) = std::env::var("BLEND") {
            plan.blend = b.parse().unwrap();
        }
        let t_alloc = t.elapsed();
        let mut read = |x: i32, y: i32, z: i32| Some(block(x, y, z).to_string());
        plan.step(u64::MAX, &mut read);
        let t_plan = t.elapsed();
        // Lista de escritas por seção.
        let mut ours = HashSet::new();
        let mut n_cells = 0usize;
        for sx in plan.sec0[0]..plan.sec0[0] + plan.sec_dim[0] {
            for sy in plan.sec0[1]..plan.sec0[1] + plan.sec_dim[1] {
                for sz in plan.sec0[2]..plan.sec0[2] + plan.sec_dim[2] {
                    let cells = plan.section_cells([sx, sy, sz]);
                    n_cells += cells.len();
                    ours.extend(cells);
                }
            }
        }
        let t_total = t.elapsed();
        let depth = (0..=r).take_while(|&d| ours.contains(&[0, c[1] - 1 - d, 0])).count();
        let surf_r = (0..=r)
            .take_while(|&d| ours.contains(&[d, surface(d, 0), 0]) || is_air(block(d, surface(d, 0), 0)))
            .count();
        println!(
            "R={r} ours: voxels={} reads={} visited={} removed={} edges={} plan={:.1}ms (alloc {:.1}ms) total={:.1}ms depth_center={} surface_r_x={}",
            plan.voxels(),
            plan.reads,
            plan.visited,
            plan.removed,
            plan.edges.len(),
            t_plan.as_secs_f64() * 1e3,
            t_alloc.as_secs_f64() * 1e3,
            t_total.as_secs_f64() * 1e3,
            depth,
            surf_r
        );
        assert_eq!(n_cells as u64, plan.removed);
        if std::env::var("NO_NTM").is_ok() {
            continue;
        }
        let t = Instant::now();
        let (theirs, reads) = ntm(c, r);
        let t_ntm = t.elapsed();
        let both = ours.intersection(&theirs).count();
        let depth_n = (0..=r).take_while(|&d| theirs.contains(&[0, c[1] - 1 - d, 0])).count();
        let surf_n = (0..=r)
            .take_while(|&d| theirs.contains(&[d, surface(d, 0), 0]) || is_air(block(d, surface(d, 0), 0)))
            .count();
        println!(
            "R={r} ntm:  rays={} reads={} removed={} time={:.1}ms depth_center={} surface_r_x={} | comum={} só_nosso={} só_ntm={} jaccard={:.3}",
            (2.5 * std::f64::consts::PI * (2.0 * r as f64).powi(2)) as i64,
            reads,
            theirs.len(),
            t_ntm.as_secs_f64() * 1e3,
            depth_n,
            surf_n,
            both,
            ours.len() - both,
            theirs.len() - both,
            both as f64 / (ours.len() + theirs.len() - both) as f64
        );
        let split = |a: &HashSet<[i32; 3]>, b: &HashSet<[i32; 3]>| {
            let up = a.difference(b).filter(|p| p[1] >= c[1]).count();
            let down = a.difference(b).count() - up;
            let rim = a.difference(b).filter(|p| p[1] < c[1] && ((p[0] * p[0] + p[2] * p[2]) as f32).sqrt() > 0.6 * r as f32).count();
            (up, down, rim)
        };
        println!("R={r} diferenças (acima, abaixo, abaixo&além de 0.6R): só_nosso={:?} só_ntm={:?}", split(&ours, &theirs), split(&theirs, &ours));
        // Perfil: profundidade por distância horizontal (eixo x), nosso vs NTM.
        let prof = |set: &HashSet<[i32; 3]>, d: i32| {
            let h = surface(d, 0);
            (0..=r).take_while(|&k| set.contains(&[d, h - k, 0])).count()
        };
        let step = (r / 10).max(1);
        let line: Vec<String> = (0..=r)
            .step_by(step as usize)
            .map(|d| format!("{d}:{}/{}", prof(&ours, d), prof(&theirs, d)))
            .collect();
        println!("R={r} profundidade sob a superfície (x: nosso/ntm): {}", line.join(" "));
    }
}
