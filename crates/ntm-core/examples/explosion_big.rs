//! Memória e tempo do plano esparso em raios grandes (docs/explosoes.md §9).
//! uso: cargo run --release --example big -- 150 175 250 500
//! Passos do tamanho do tick (34 M de fuel estimado): mede o pico de memória
//! do plano, os buffers de energia vivos e quantos "ticks" a varredura leva.
use ntm_core::explosion::crater::{Plan, rand01};
use ntm_core::explosion::engine::FUEL_PLAN;
use std::time::Instant;

fn surface(x: i32, z: i32) -> i32 {
    64 + (4.0 * (x as f32 / 23.0).sin() + 3.0 * (z as f32 / 17.0).cos()).round() as i32
}

fn block(x: i32, y: i32, z: i32) -> &'static str {
    let h = surface(x, z);
    if y < -59 {
        "minecraft:bedrock"
    } else if y > h {
        if y <= 62 { "minecraft:water[level=0]" } else { "minecraft:air" }
    } else if y < h - 6 && rand01(x >> 2, y >> 2, z >> 2, 5) < 0.04 {
        "minecraft:cave_air"
    } else if y == h {
        "minecraft:grass_block[snowy=false]"
    } else if y >= h - 3 {
        "minecraft:dirt"
    } else if y < 0 {
        "minecraft:deepslate[axis=y]"
    } else {
        "minecraft:stone"
    }
}

fn main() {
    let radii: Vec<i32> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    for r in if radii.is_empty() { vec![150, 175, 250] } else { radii } {
        let t = Instant::now();
        let mut plan = Plan::new([0, surface(0, 0) + 1, 0], r);
        let new_ms = t.elapsed().as_secs_f64() * 1e3;
        let mut read = |x: i32, y: i32, z: i32| Some(block(x, y, z).to_string());
        let (mut ticks, mut peak, mut peak_live) = (0u64, plan.memory(), 0usize);
        while !plan.finished() {
            plan.step(FUEL_PLAN, &mut read);
            ticks += 1;
            peak = peak.max(plan.memory());
            peak_live = peak_live.max(plan.live_buffers());
        }
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let dense = plan.voxels() * 2 + plan.voxels() / 8;
        println!(
            "R={r}: colunas={} removidos={} leituras={} bordas={} ticks(34M)={ticks} pico={:.1} MB (denso seria {:.1} MB) buffers_vivos={peak_live} aprox={} new={new_ms:.2} ms total={ms:.0} ms",
            plan.columns(),
            plan.removed,
            plan.reads,
            plan.edges.len(),
            peak as f64 / 1e6,
            dense as f64 / 1e6,
            plan.approx
        );
    }
}
