//! Explosions of the NTM's bombs on the server (docs/bombas.md): the crater
//! engine (`crate::explosion`) per world, vanilla's explosion for charges, the
//! effects as vanilla commands, blast damage, and a nuke's radiation pulse.
//! A crater leaves nothing in the chunk field: the fallout rain's blocks are
//! inert (docs/onda-1-protecao.md, slice 4e).

pub mod config;
pub mod vanilla;
pub mod visuals;

use crate::Pos;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use config::{Config, Mode};

/// What a world keeps of its bombs (in its save, `WorldState.bombs`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BombWorld {
    /// The next explosion's number (its id is `w<world>n<number>`).
    #[serde(default)]
    pub next: u64,
    // A world saved before slice 4e may still hold `zones` (the port's lingering areas, which the Java does not
    // have): serde reads past the key and the next save drops it.
    /// Each player's detonator: the bomb it is linked to.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub links: BTreeMap<String, (String, Pos)>,
    /// This world's settings, over the global ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cfg: Option<Config>,
    /// Bombs set off and not yet exploded (or not yet durable in the
    /// engine): saved with the bomb's removal, so a crash never loses one.
    /// They go off a few per tick, within the tick's fuel.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queue: Vec<Queued>,
    /// The balefires whose crater is being dug: they come back with the world, as `EntityBalefire` does (it writes
    /// `age`, `destructionRange`, `speed` and its explosion's `exp_*`), and their damage with them. A nuke's
    /// (`EntityNukeExplosionMK5`) does not: its `writeEntityToNBT` keeps `ticksExisted` alone, so the entity read back
    /// has `strength` 0 and dies on its first update, crater and damage stopped; the HBM 1.7.10 quirk is followed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub balefires: Vec<String>,
    /// The balefires' damage while their craters are dug, on the world's clock.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pulses: Vec<Pulse>,
}

/// A bomb set off, waiting its turn (`BombWorld.queue`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Queued {
    /// Confirmed old generation; physical blasts are never cancelled by reuse.
    #[serde(default)]
    pub origin: Option<crate::server::CellGuard>,
    /// The explosion's number (`w<world>n<number>`).
    pub n: u64,
    pub dim: String,
    pub pos: Pos,
    pub blast: crate::bomb::Blast,
    /// The bomb's block, and what set it off.
    pub name: String,
    pub why: String,
    /// A nuke handed to the engine, its effects done: it stays here until
    /// the engine has it in the folder.
    #[serde(default)]
    pub started: bool,
}

impl Queued {
    /// Fuel the tick it goes off costs (measured in `spikes/m2-fuel`:
    /// vanilla's explosion of power 15 takes 12 M; a nuke only hands its
    /// plan to the engine, which builds one per tick).
    pub fn cost(&self) -> u64 {
        match self.blast {
            crate::bomb::Blast::Conventional { power } => {
                let p = f64::from(power.clamp(1.0, vanilla::MAX_POWER)) / 15.0;
                (13_000_000.0 * p * p) as u64 + 1_000_000
            }
            _ => 1_000_000,
        }
    }
}

/// Blast damage a nuke or a balefire deals while its crater is dug (`ExplosionNukeGeneric.dealDamage`, every tick of
/// `EntityNukeExplosionMK5` until its explosion `isComplete`, and of `EntityBalefire` while `exp.update()` has not
/// finished; here every [`PULSE_EVERY`] ticks). It ends with the crater of `id`, however long that takes. `next` is
/// on the world's clock (`WorldState.ticks`), kept with the world.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pulse {
    /// The explosion whose crater it lasts as long as.
    #[serde(default)]
    pub id: String,
    pub dim: String,
    pub center: [f64; 3],
    /// 2R: `dealDamage(…, length * 2)`.
    pub reach: f64,
    pub next: u64,
}

pub const PULSE_EVERY: u64 = 10;
/// `dealDamage`'s maxDamage.
pub const PULSE_MAX: f64 = 250.0;

/// The bombs' state on the server that isn't saved with the world.
#[derive(Default)]
pub struct Runtime {
    pub nukes: crate::explosion::Explosions,
    /// The world whose explosions are in `nukes`.
    pub wid: Option<String>,
    /// The `lit` each bomb showed last (redstone sets bombs off on its
    /// rising edge, as `onNeighborBlockChange` does).
    pub lit: BTreeMap<(String, Pos), bool>,
    /// The nukes' damage: not kept, as `EntityNukeExplosionMK5` keeps none (`BombWorld.balefires`).
    pub pulses: Vec<Pulse>,
    /// The global settings, read once from the folder.
    pub global: Option<Config>,
    pub warned_persist: bool,
    /// The tick a charge went off: its vanilla explosion used that tick's
    /// fuel, so the nukes wait for the next one.
    pub charged_at: Option<u64>,
    /// Last detonations, for `/ntmbomba status`.
    pub log: Vec<String>,
    /// When each player online was first seen in this world: `contaminate`'s grace (`ticksExisted < 200`) is per
    /// player.
    pub entered: BTreeMap<String, u64>,
    /// The nukes whose `EntityNukeExplosionMK5.radiate` still runs (its first ticks): not kept, as the entity keeps
    /// none of it.
    pub radiating: Vec<Radiating>,
}

/// A nuke's radiation pulse (`EntityNukeExplosionMK5.onUpdate:66-67`): the entity's update `t` (its `ticksExisted`,
/// counted before `onUpdate`, so 1 on the first) radiates `2 500 000 / (5t + 1)` over `2 × length` while `t < 10`
/// and its explosion exists, which it does from the second update on. Only a nuke with fallout and
/// `strength = 2r ≥ 75`.
#[derive(Clone, Debug, PartialEq)]
pub struct Radiating {
    /// The explosion whose entity it is.
    pub id: String,
    pub dim: String,
    pub center: [f64; 3],
    /// `length * 2`.
    pub range: f64,
    /// The entity's `ticksExisted` at its last update.
    pub t: u32,
}

impl Radiating {
    /// `radiate`'s rads at update `t`: none on the first (no explosion yet) nor from the tenth.
    pub fn rads(t: u32) -> Option<f32> {
        (2..10).contains(&t).then(|| 2_500_000f32 / (t * 5 + 1) as f32)
    }

    /// `statFac`'s strength is twice the radius; `onUpdate` radiates only from 75.
    pub fn radiates(radius: i32) -> bool {
        2 * radius >= 75
    }
}

/// What an explosion's engine tells (`crate::explosion::Hooks`).
#[derive(Default)]
pub struct Recorder(pub Vec<(String, crate::explosion::Event)>);

impl crate::explosion::Hooks for Recorder {
    fn event(&mut self, id: &str, e: &crate::explosion::Event) {
        self.0.push((id.to_string(), e.clone()));
    }
}

/// The fallout layer block: the content's `ntm:fallout` (snow template).
pub const FALLOUT_LAYER: &str = "ntm:fallout[layers=1]";

/// The crater's and the fallout's blocks for a blast, with the content
/// mod's waste blocks (`ids.json`, category `fallout`).
pub fn profile(blast: crate::bomb::Blast, fallout: bool) -> crate::explosion::EffectProfile {
    use crate::bomb::Blast;
    use crate::explosion::{EffectProfile, FalloutProfile};
    match blast {
        // statFacNoRad: no fallout rain, so no slag either.
        Blast::Nuke { fallout: false, .. } | Blast::Conventional { .. } => EffectProfile::none(),
        Blast::Balefire { .. } => EffectProfile {
            name: "balefire".into(),
            foliage: Some("minecraft:air".into()),
            fire: Some((0.5, 1.0, "minecraft:fire".into())),
            ..EffectProfile::none()
        },
        Blast::Nuke { fallout: true, .. } => EffectProfile {
            name: "ntm".into(),
            // The fallout slags everything up to 1.25R: the whole crater.
            slag: vec![(f32::INFINITY, "ntm:sellafield_slaked".into())],
            trinitite: Some((0.05, "ntm:waste_trinitite".into())),
            wood: Some("ntm:waste_log".into()),
            planks: Some("ntm:waste_planks".into()),
            foliage: Some("minecraft:air".into()),
            fire: Some((0.2, 0.65, "minecraft:fire".into())),
            fallout: fallout.then(|| FalloutProfile {
                slag: vec![(0.5, "ntm:sellafield_slaked".into())],
                wood: Some("ntm:waste_log".into()),
                planks: Some("ntm:waste_planks".into()),
                dead_leaves: Some("ntm:waste_leaves[persistent=true]".into()),
                trinitite: Some((0.05, "ntm:waste_trinitite".into())),
                layer: Some(FALLOUT_LAYER.into()),
                mycelium: Some("ntm:waste_mycelium".into()),
                ..FalloutProfile::ntm_placeholder()
            }),
        },
    }
}

#[cfg(test)]
mod radiating_tests {
    use super::Radiating;

    /// `onUpdate:66`: `ticksExisted < 10` and the explosion made on the first update; `statFac`'s strength `2r ≥ 75`.
    #[test]
    fn the_pulse_is_updates_two_to_nine_of_a_nuke_from_radius_38() {
        assert_eq!(Radiating::rads(1), None);
        assert_eq!(Radiating::rads(2), Some(2_500_000f32 / 11.0));
        assert_eq!(Radiating::rads(9), Some(2_500_000f32 / 46.0));
        assert_eq!(Radiating::rads(10), None);
        let total: f64 = (2..10).filter_map(Radiating::rads).map(f64::from).sum();
        assert!((total - 864_137.6).abs() < 1.0, "{total}");
        assert!(!Radiating::radiates(37));
        assert!(Radiating::radiates(38));
    }
}
