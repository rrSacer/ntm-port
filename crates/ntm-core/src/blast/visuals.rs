//! What a detonation shows and sounds like: vanilla particles, sounds and
//! effects the server sends through its host (`ServerHost::particles`,
//! `sound`, `effect`). This is the extension point of the `aprimorado`
//! mode: a [`Visuals`] backend turns a blast into [`Show`]s; the enhanced
//! one needs engine APIs (shaders, camera, sky) another agent is
//! researching, so for now it reports itself unavailable and the optimized
//! one is used.

use super::config::{Config, Mode};
use crate::{Pos, bomb::Blast, server::PlayerInfo};

/// A blast, as the visuals see it.
pub struct Shot<'a> {
    pub dim: &'a str,
    pub pos: Pos,
    pub blast: Blast,
    /// Players in the blast's dimension.
    pub players: &'a [PlayerInfo],
}

/// One thing a blast shows, in its dimension.
#[derive(Clone, Debug, PartialEq)]
pub enum Show {
    /// Vanilla particles (`ServerLevel.sendParticles`): `id` as
    /// `minecraft:<name>`, RGBA for the coloured ones.
    Particles { id: &'static str, color: [f32; 4], at: [f64; 3], spread: [f64; 3], speed: f64, count: i32 },
    /// A vanilla sound event from a vanilla source (`block`, `weather`...),
    /// heard to 16·volume blocks.
    Sound { id: &'static str, source: &'static str, at: [f64; 3], volume: f32, pitch: f32 },
    /// A status effect on a player (by id).
    Effect { player: String, id: &'static str, seconds: u32, amplifier: u8 },
}

pub trait Visuals {
    fn name(&self) -> &'static str;
    /// Whether this backend can run here (the enhanced one: engine APIs).
    fn available(&self) -> bool {
        true
    }
    /// What the blast shows at its moment.
    fn detonation(&self, cfg: &Config, shot: &Shot) -> Vec<Show>;
    /// What a charge shows when its fuse is lit.
    fn fuse(&self, cfg: &Config, dim: &str, pos: Pos) -> Vec<Show>;
}

/// Vanilla particles and sounds: a handful per blast, no matter its size
/// (the clients draw them; the server pays nothing).
pub struct Optimized;

fn center(p: Pos) -> [f64; 3] {
    [f64::from(p[0]) + 0.5, f64::from(p[1]) + 0.5, f64::from(p[2]) + 0.5]
}

const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

impl Visuals for Optimized {
    fn name(&self) -> &'static str {
        "otimizado"
    }

    fn detonation(&self, cfg: &Config, s: &Shot) -> Vec<Show> {
        if !cfg.effects {
            return Vec::new();
        }
        let at = center(s.pos);
        let n = cfg.particles as i32;
        let mut out = Vec::new();
        match s.blast {
            Blast::Conventional { power } => {
                let v = (power / 4.0).max(1.0);
                out.push(Show::Sound { id: "minecraft:entity.generic.explode", source: "block", at, volume: v, pitch: 0.7 });
                if n > 0 {
                    let spread = f64::from((power / 4.0).max(1.0));
                    out.push(Show::Particles { id: "minecraft:explosion_emitter", color: WHITE, at, spread: [spread; 3], speed: 0.0, count: (n / 8).max(1) });
                }
            }
            Blast::Nuke { radius, .. } | Blast::Balefire { radius } => {
                let r = radius as f32;
                // Heard to 16·volume blocks.
                let v = (r / 8.0).clamp(4.0, 64.0);
                out.push(Show::Sound { id: "minecraft:entity.generic.explode", source: "master", at, volume: v, pitch: 0.5 });
                out.push(Show::Sound { id: "minecraft:entity.lightning_bolt.thunder", source: "weather", at, volume: v, pitch: 0.5 });
                if n > 0 {
                    let flash = if matches!(s.blast, Blast::Balefire { .. }) { [0.3, 1.0, 0.3, 1.0] } else { WHITE };
                    out.push(Show::Particles { id: "minecraft:flash", color: flash, at, spread: [0.0; 3], speed: 0.0, count: 1 });
                    let sp = f64::from((r / 8.0).min(24.0));
                    out.push(Show::Particles { id: "minecraft:explosion_emitter", color: WHITE, at, spread: [sp, sp / 2.0, sp], speed: 0.0, count: n });
                    // The mushroom: a stem of signal smoke, a cap of large smoke.
                    let r = f64::from(r);
                    let (stem, cap) = (f64::from(s.pos[1]) + r / 4.0, f64::from(s.pos[1]) + r / 2.0);
                    out.push(Show::Particles { id: "minecraft:campfire_signal_smoke", color: WHITE, at: [at[0], stem, at[2]], spread: [r / 24.0, r / 8.0, r / 24.0], speed: 0.02, count: n * 4 });
                    out.push(Show::Particles { id: "minecraft:large_smoke", color: WHITE, at: [at[0], cap, at[2]], spread: [r / 6.0, r / 16.0, r / 6.0], speed: 0.05, count: n * 4 });
                }
                // The shock: nausea for who is inside the radius.
                for pl in s.players {
                    let d2 = (0..3).map(|i| (pl.pos[i] - f64::from(s.pos[i])).powi(2)).sum::<f64>();
                    if d2 <= f64::from(radius).powi(2) && !pl.creative {
                        out.push(Show::Effect { player: pl.id.clone(), id: "nausea", seconds: 8, amplifier: 0 });
                    }
                }
            }
        }
        out
    }

    fn fuse(&self, cfg: &Config, _dim: &str, pos: Pos) -> Vec<Show> {
        if !cfg.effects {
            return Vec::new();
        }
        let at = center(pos);
        let mut out = vec![Show::Sound { id: "minecraft:entity.tnt.primed", source: "block", at, volume: 1.0, pitch: 1.0 }];
        if cfg.particles > 0 {
            out.push(Show::Particles { id: "minecraft:smoke", color: WHITE, at: [at[0], at[1] + 0.5, at[2]], spread: [0.1; 3], speed: 0.01, count: 8 });
        }
        out
    }
}

/// The enhanced mode's backend: not available until the engine has the APIs
/// it needs. What it shows is the optimized one's (the fallback).
pub struct Enhanced;

impl Visuals for Enhanced {
    fn name(&self) -> &'static str {
        "aprimorado"
    }
    fn available(&self) -> bool {
        false
    }
    fn detonation(&self, cfg: &Config, s: &Shot) -> Vec<Show> {
        Optimized.detonation(cfg, s)
    }
    fn fuse(&self, cfg: &Config, dim: &str, pos: Pos) -> Vec<Show> {
        Optimized.fuse(cfg, dim, pos)
    }
}

/// The backend for `mode`, and whether it fell back to the optimized one.
pub fn backend(mode: Mode) -> (&'static dyn Visuals, bool) {
    match mode {
        Mode::Otimizado => (&Optimized, false),
        Mode::Aprimorado if Enhanced.available() => (&Enhanced, false),
        Mode::Aprimorado => (&Optimized, true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A nuke shows a flash, the emitter and the mushroom, and sounds; a
    /// player inside the radius (not in creative) gets nausea. With effects
    /// off, nothing.
    #[test]
    fn a_nuke_shows_its_flash_cloud_and_shock() {
        let cfg = Config::default();
        let p = PlayerInfo { id: "p".into(), name: "P".into(), dim: "d".into(), pos: [3.0, 0.0, 0.0], creative: false };
        let shot = Shot { dim: "d", pos: [0, 0, 0], blast: Blast::Nuke { radius: 40, fallout: true }, players: std::slice::from_ref(&p) };
        let shows = Optimized.detonation(&cfg, &shot);
        let ids: Vec<&str> = shows.iter().filter_map(|s| if let Show::Particles { id, .. } = s { Some(*id) } else { None }).collect();
        if cfg.particles > 0 {
            assert_eq!(ids, ["minecraft:flash", "minecraft:explosion_emitter", "minecraft:campfire_signal_smoke", "minecraft:large_smoke"]);
        }
        assert!(shows.iter().any(|s| matches!(s, Show::Sound { id: "minecraft:entity.lightning_bolt.thunder", .. })));
        assert!(shows.contains(&Show::Effect { player: "p".into(), id: "nausea", seconds: 8, amplifier: 0 }));
        let off = Config { effects: false, ..cfg };
        assert!(Optimized.detonation(&off, &shot).is_empty());
    }
}
