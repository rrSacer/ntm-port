//! Vanilla's explosion (`Explosion.explode`), for the NTM's conventional
//! charges, which call `world.createExplosion(entity, x, y, z, strength,
//! true)` (`BlockTNT` 10, `BlockDynamite` 8, `BlockSemtex` 12, `BlockC4`
//! 15). 16³ rays from the cube's surface, each with `power·(0.7..1.3)`,
//! stepping 0.3 blocks and losing `(resistance + 0.3)·0.3` per block and
//! 0.225 per step. Each block is read once (a flat cache over the reach),
//! the random is a hash of the position (deterministic), and no drops.

use crate::Pos;
use crate::explosion::crater::{Kind, Mats, classify, rand01};

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

/// The strongest explosion this computes: the NTM's charges go to 15 (C4);
/// `/ntmbomba teste` goes to here. Power 20 reads a cube of 73³ blocks and
/// costs ~23 M of fuel.
pub const MAX_POWER: f32 = 20.0;

/// The blocks a power-`power` explosion at `center` removes, top first.
/// `read` is `None` where no chunk is loaded (the ray stops there). The
/// power is held to [`MAX_POWER`] (the cube grows as its cube).
pub fn explode(center: [f64; 3], power: f32, read: &mut impl FnMut(Pos) -> Option<String>) -> (Vec<Pos>, u32) {
    let power = if power.is_finite() { power.clamp(0.0, MAX_POWER) } else { 0.0 };
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

/// Whether a block stops a ray of `exposure` (it has a collision box:
/// air, liquids and plants don't).
fn solid(state: &str) -> bool {
    !matches!(classify(state).kind, Kind::Air | Kind::Liquid | Kind::Plant)
}

/// The share of a player's box (feet at `feet`, 0.6 × 1.8) that sees
/// `center` (`Explosion.getSeenPercent`: rays from a grid over the box to
/// the center, here 3 × 5 × 3, stepping half a block; a block hit stops
/// one). Blocks are read once (cache).
pub fn exposure(center: [f64; 3], feet: [f64; 3], read: &mut impl FnMut(Pos) -> Option<String>) -> f32 {
    let mut seen: std::collections::HashMap<Pos, bool> = Default::default();
    let mut solid_at = |p: Pos| *seen.entry(p).or_insert_with(|| read(p).is_some_and(|s| solid(&s)));
    let (mut open, mut total) = (0, 0);
    for ix in 0..3 {
        for iy in 0..5 {
            for iz in 0..3 {
                let a = [feet[0] - 0.3 + 0.3 * f64::from(ix), feet[1] + 0.45 * f64::from(iy), feet[2] - 0.3 + 0.3 * f64::from(iz)];
                let d = [center[0] - a[0], center[1] - a[1], center[2] - a[2]];
                let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                let steps = (len / 0.5).ceil().max(1.0) as i32;
                let end = center.map(|v| v.floor() as i32);
                total += 1;
                let blocked = (0..steps).any(|k| {
                    let t = f64::from(k) / f64::from(steps);
                    let p = [0, 1, 2].map(|i| (a[i] + d[i] * t).floor() as i32);
                    p != end && solid_at(p)
                });
                if !blocked {
                    open += 1;
                }
            }
        }
    }
    open as f32 / total as f32
}

/// Vanilla's damage to an entity at `dist` from a power-`power` blast that
/// sees `exposure` of it: `(i² + i)/2 · 7 · 2p + 1`, `i = (1 − d/2p) · exposure`.
pub fn damage(power: f32, dist: f64, exposure: f32) -> f32 {
    let i = ((1.0 - dist / (2.0 * f64::from(power))).max(0.0) as f32) * exposure.clamp(0.0, 1.0);
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
        assert!(damage(10.0, 1.0, 1.0) > 100.0);
        assert!(damage(10.0, 15.0, 1.0) < damage(10.0, 5.0, 1.0));
        assert_eq!(damage(10.0, 25.0, 1.0), 0.0);
        assert_eq!(damage(10.0, 5.0, 0.0), 0.0);
    }

    /// Revisão do M2, P1-13: a força passa do teto, a conta fica no teto
    /// (antes, força 100 alocava um cubo de ~165 MiB).
    #[test]
    fn power_is_held_to_the_maximum() {
        let mut reads = 0usize;
        let (_, n) = explode([0.5, 64.5, 0.5], 100.0, &mut |p| {
            reads += 1;
            flat(p)
        });
        let (_, m) = explode([0.5, 64.5, 0.5], MAX_POWER, &mut flat);
        assert_eq!(n, m);
        assert!(explode([0.5, 64.5, 0.5], f32::NAN, &mut flat).0.is_empty());
    }

    #[test]
    fn exposure_sees_through_air_not_walls() {
        let open = exposure([0.5, 70.5, 0.5], [6.5, 70.0, 0.5], &mut |_| Some("minecraft:air".into()));
        assert_eq!(open, 1.0);
        let wall = exposure([0.5, 70.5, 0.5], [6.5, 70.0, 0.5], &mut |p| Some(if p[0] == 3 { "minecraft:obsidian" } else { "minecraft:air" }.into()));
        assert_eq!(wall, 0.0);
    }
}
