//! Vanilla's explosion (`Explosion.explode`), for the NTM's conventional
//! charges, which call `world.createExplosion(entity, x, y, z, strength,
//! true)` (`BlockTNT` 10, `BlockDynamite` 8, `BlockSemtex` 12, `BlockC4`
//! 15). 16³ rays from the cube's surface, each with `power·(0.7..1.3)`,
//! stepping 0.3 blocks and losing `(resistance + 0.3)·0.3` per block and
//! 0.225 per step. Each block is read once (a flat cache over the reach),
//! the random is a hash of the position (deterministic), and no drops.

use crate::Pos;
use ntm_explosion::crater::{Kind, Mats, classify, rand01};

/// Explosion resistance as vanilla has it: the crater's table, but liquids
/// stop rays (water 100) and obsidian really resists (1200).
/// Air is negative: it costs nothing and isn't destroyed.
pub fn resistance(state: &str) -> f32 {
    let name = state.split('[').next().unwrap_or(state);
    match name {
        "minecraft:obsidian" | "minecraft:crying_obsidian" | "minecraft:ancient_debris" | "minecraft:netherite_block" => 1200.0,
        _ => {
            let m = classify(state);
            match m.kind {
                Kind::Air => -1.0,
                Kind::Liquid => 100.0,
                _ => m.res,
            }
        }
    }
}

/// The blocks a power-`power` explosion at `center` removes, top first.
/// `read` is `None` where no chunk is loaded (the ray stops there).
pub fn explode(center: [f64; 3], power: f32, read: &mut impl FnMut(Pos) -> Option<String>) -> (Vec<Pos>, u32) {
    // Farthest a ray goes: power·1.3 / 0.225 steps of 0.3.
    let reach = ((power * 1.3 / 0.225) * 0.3).ceil() as i32 + 1;
    let side = 2 * reach + 1;
    let c = [center[0].floor() as i32, center[1].floor() as i32, center[2].floor() as i32];
    let idx = |p: Pos| -> Option<usize> {
        let (a, b, d) = (p[0] - c[0] + reach, p[1] - c[1] + reach, p[2] - c[2] + reach);
        (a >= 0 && b >= 0 && d >= 0 && a < side && b < side && d < side).then(|| ((a * side + b) * side + d) as usize)
    };
    // NaN: not read yet; infinity: no chunk.
    let mut res = vec![f32::NAN; (side * side * side) as usize];
    // Resistance per block name, classified once (FNV cache of the crater).
    let mut mats = Mats::new();
    let mut by_mat: Vec<f32> = Vec::new();
    let mut hit = vec![false; res.len()];
    let mut reads = 0;
    let mut out = Vec::new();
    for i in 0..16 {
        for j in 0..16 {
            for k in 0..16 {
                if !(i == 0 || i == 15 || j == 0 || j == 15 || k == 0 || k == 15) {
                    continue;
                }
                let mut d = [i as f64 / 15.0 * 2.0 - 1.0, j as f64 / 15.0 * 2.0 - 1.0, k as f64 / 15.0 * 2.0 - 1.0];
                let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                d = [d[0] / len, d[1] / len, d[2] / len];
                let mut f = power * (0.7 + 0.6 * rand01(c[0] + i, c[1] + j, c[2] + k, 31));
                let mut p = center;
                while f > 0.0 {
                    let b = [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32];
                    let Some(at) = idx(b) else { break };
                    if res[at].is_nan() {
                        reads += 1;
                        res[at] = match read(b) {
                            None => f32::INFINITY,
                            Some(s) => {
                                let id = mats.id(&s) as usize;
                                if by_mat.len() <= id {
                                    by_mat.resize(id + 1, f32::NAN);
                                }
                                if by_mat[id].is_nan() {
                                    by_mat[id] = resistance(&s);
                                }
                                by_mat[id]
                            }
                        };
                    }
                    let r = res[at];
                    if r.is_infinite() {
                        break;
                    }
                    if r >= 0.0 {
                        f -= (r + 0.3) * 0.3;
                    }
                    if f > 0.0 && r >= 0.0 && !hit[at] {
                        hit[at] = true;
                        out.push(b);
                    }
                    p = [p[0] + d[0] * 0.3, p[1] + d[1] * 0.3, p[2] + d[2] * 0.3];
                    f -= 0.225_000_01;
                }
            }
        }
    }
    out.sort_by_key(|p| (-p[1], p[0], p[2]));
    (out, reads)
}

/// Vanilla's damage to an entity at `dist` from a power-`power` blast, in
/// the open (exposure 1): `(i² + i)/2 · 7 · 2p + 1`, `i = 1 − d/2p`.
pub fn damage(power: f32, dist: f64) -> f32 {
    let i = (1.0 - dist / (2.0 * f64::from(power))).max(0.0) as f32;
    if i <= 0.0 {
        return 0.0;
    }
    (i * i + i) / 2.0 * 7.0 * 2.0 * power + 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(p: Pos) -> Option<String> {
        Some(if p[1] < 64 { "minecraft:stone" } else { "minecraft:air" }.to_string())
    }

    #[test]
    fn tnt_digs_a_shallow_pit_in_stone_and_never_obsidian() {
        let (out, reads) = explode([0.5, 64.5, 0.5], 10.0, &mut flat);
        assert!(!out.is_empty());
        let deepest = out.iter().map(|p| p[1]).min().unwrap();
        assert!((55..64).contains(&deepest), "fundo em {deepest}");
        assert!(out.iter().all(|p| p[1] < 64));
        assert!(reads < 60_000, "{reads} leituras");
        let (o, _) = explode([0.5, 64.5, 0.5], 15.0, &mut |p| Some(if p[1] < 64 { "minecraft:obsidian" } else { "minecraft:air" }.into()));
        assert!(o.is_empty());
        // Stronger digs more.
        assert!(explode([0.5, 64.5, 0.5], 15.0, &mut flat).0.len() > out.len());
        // Unloaded chunks stop rays.
        assert!(explode([0.5, 64.5, 0.5], 10.0, &mut |_| None).0.is_empty());
    }

    #[test]
    fn damage_falls_with_distance() {
        assert!(damage(10.0, 1.0) > 100.0);
        assert!(damage(10.0, 15.0) < damage(10.0, 5.0));
        assert_eq!(damage(10.0, 25.0), 0.0);
    }
}
