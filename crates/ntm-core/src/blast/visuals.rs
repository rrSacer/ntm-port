//! What a detonation shows and sounds like, as vanilla commands the client
//! runs (`S2C::Run`). This is the extension point of the `aprimorado` mode:
//! a [`Visuals`] backend turns a blast into lines; the enhanced one needs
//! engine APIs (shaders, camera, sky) another agent is researching, so for
//! now it reports itself unavailable and the optimized one is used.

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

pub trait Visuals {
    fn name(&self) -> &'static str;
    /// Whether this backend can run here (the enhanced one: engine APIs).
    fn available(&self) -> bool {
        true
    }
    /// Lines at the moment of the blast.
    fn detonation(&self, cfg: &Config, shot: &Shot) -> Vec<String>;
    /// Lines when a charge's fuse is lit.
    fn fuse(&self, cfg: &Config, dim: &str, pos: Pos) -> Vec<String>;
}

/// Vanilla `playsound` and `particle`: a handful of lines per blast, no
/// matter its size (the client draws them; the server pays nothing).
pub struct Optimized;

fn at(p: Pos) -> String {
    format!("{}.5 {}.5 {}.5", p[0], p[1], p[2])
}

impl Visuals for Optimized {
    fn name(&self) -> &'static str {
        "otimizado"
    }

    fn detonation(&self, cfg: &Config, s: &Shot) -> Vec<String> {
        if !cfg.effects {
            return Vec::new();
        }
        let p = at(s.pos);
        let n = cfg.particles;
        let mut out = Vec::new();
        match s.blast {
            Blast::Conventional { power } => {
                let v = (power / 4.0).max(1.0);
                out.push(format!("playsound minecraft:entity.generic.explode block @a {p} {v:.1} 0.7"));
                if n > 0 {
                    let spread = (power / 4.0).max(1.0);
                    out.push(format!("particle minecraft:explosion_emitter {p} {spread:.1} {spread:.1} {spread:.1} 0 {} force", (n / 8).max(1)));
                }
            }
            Blast::Nuke { radius, .. } | Blast::Balefire { radius } => {
                let r = radius as f32;
                // Heard to 16·volume blocks.
                let v = (r / 8.0).clamp(4.0, 64.0);
                out.push(format!("playsound minecraft:entity.generic.explode master @a {p} {v:.0} 0.5 1"));
                out.push(format!("playsound minecraft:entity.lightning_bolt.thunder weather @a {p} {v:.0} 0.5 1"));
                if n > 0 {
                    let flash = if matches!(s.blast, Blast::Balefire { .. }) { "0.3,1.0,0.3,1.0" } else { "1.0,1.0,1.0,1.0" };
                    out.push(format!("particle minecraft:flash{{color:[{flash}]}} {p} 0 0 0 0 1 force"));
                    let sp = (r / 8.0).min(24.0);
                    out.push(format!("particle minecraft:explosion_emitter {p} {sp:.1} {:.1} {sp:.1} 0 {n} force", sp / 2.0));
                    // The mushroom: a stem of signal smoke, a cap of large smoke.
                    let [x, y, z] = s.pos;
                    let (stem, cap) = (y + radius / 4, y + radius / 2);
                    out.push(format!("particle minecraft:campfire_signal_smoke {x}.5 {stem} {z}.5 {:.1} {:.1} {:.1} 0.02 {} force", r / 24.0, r / 8.0, r / 24.0, n * 4));
                    out.push(format!("particle minecraft:large_smoke {x}.5 {cap} {z}.5 {:.1} {:.1} {:.1} 0.05 {} force", r / 6.0, r / 16.0, r / 6.0, n * 4));
                }
                // The shock: nausea for who is inside the radius.
                for pl in s.players {
                    let d2 = (0..3).map(|i| (pl.pos[i] - f64::from(s.pos[i])).powi(2)).sum::<f64>();
                    if d2 <= f64::from(radius).powi(2) && !pl.creative {
                        out.push(format!("effect give {} minecraft:nausea 8 0 true", pl.name));
                    }
                }
            }
        }
        out
    }

    fn fuse(&self, cfg: &Config, _dim: &str, pos: Pos) -> Vec<String> {
        if !cfg.effects {
            return Vec::new();
        }
        vec![format!("playsound minecraft:entity.tnt.primed block @a {} 1 1", at(pos))]
    }
}

/// The enhanced mode's backend: not available until the engine has the APIs
/// it needs. Its lines are the optimized ones (the fallback).
pub struct Enhanced;

impl Visuals for Enhanced {
    fn name(&self) -> &'static str {
        "aprimorado"
    }
    fn available(&self) -> bool {
        false
    }
    fn detonation(&self, cfg: &Config, s: &Shot) -> Vec<String> {
        Optimized.detonation(cfg, s)
    }
    fn fuse(&self, cfg: &Config, dim: &str, pos: Pos) -> Vec<String> {
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
