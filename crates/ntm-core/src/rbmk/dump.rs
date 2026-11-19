//! A reactor's step input as data (T04, the in-game proof): what the server builds for one step of one reactor — its
//! dials, random, queued streams, occlusion and columns, every double by its bits — so that the pure `Reactor`
//! stepped from one dump the number of ticks between two must give the next one bit for bit. Two dumps of the same
//! reactor with nothing changed by hand between them, across a reopen too, then prove that every tick stepped it once,
//! no round lost or repeated, as the core the Java oracle checks (`tests/rbmk_flux_parity.rs`).

use super::dials::Dials;
use super::jrandom::JavaRandom;
use super::persist::rod_of;
use super::reactor::{Column, Kind, QueuedStream, Reactor};
use super::rods::RodStack;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StackDump {
    pub has_tag: bool,
    /// `yield`, `xenon`, `core`, `hull` by their bits.
    pub keys: [Option<u64>; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RodDump {
    /// `fluxQuantity`, `fluxFastRatio`, `lastFluxQuantity`, `lastFluxRatio` by their bits.
    pub flux: [u64; 4],
    pub has_rod: bool,
    pub fuel: Option<(String, StackDump)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColumnDump {
    pub kind: String,
    pub x: i32,
    pub z: i32,
    pub lid: bool,
    pub moderated: bool,
    pub heat: u64,
    pub water: i32,
    pub steam: i32,
    pub rod: Option<RodDump>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReactorDump {
    /// The world tick this step ran at (`Reactor::time`).
    pub tick: i64,
    pub dim: String,
    pub y: i32,
    /// The 24 cached dials in `Dials`' field order (`dialSaveDials` has no cache): doubles by their bits, integers
    /// and booleans as they are.
    pub dials: Vec<u64>,
    pub rng: (u64, Option<u64>),
    /// Each queued stream: its origin's index, then `vx`, `vz`, quantity, ratio by their bits.
    pub queue: Vec<(usize, [u64; 4])>,
    pub obstacles: Vec<(i32, i32, i32)>,
    pub columns: Vec<ColumnDump>,
}

const KINDS: [(Kind, &str); 15] = [
    (Kind::Rod, "Rod"),
    (Kind::RodReaSim, "RodReaSim"),
    (Kind::Control, "Control"),
    (Kind::ControlReaSim, "ControlReaSim"),
    (Kind::ControlAuto, "ControlAuto"),
    (Kind::ControlReaSimAuto, "ControlReaSimAuto"),
    (Kind::Moderator, "Moderator"),
    (Kind::Reflector, "Reflector"),
    (Kind::Absorber, "Absorber"),
    (Kind::Blank, "Blank"),
    (Kind::Storage, "Storage"),
    (Kind::Outgasser, "Outgasser"),
    (Kind::Boiler, "Boiler"),
    (Kind::Heater, "Heater"),
    (Kind::Cooler, "Cooler"),
];

fn dials_bits(d: &Dials) -> Vec<u64> {
    let f = |v: f64| v.to_bits();
    let i = |v: i32| v as u32 as u64;
    let b = |v: bool| u64::from(v);
    vec![
        f(d.passive_cooling),
        f(d.passive_cooling_inner),
        f(d.column_heat_flow),
        f(d.fuel_diffusion_mod),
        f(d.heat_provision),
        i(d.column_height),
        b(d.permanent_scrap),
        f(d.boiler_heat_consumption),
        f(d.control_speed),
        f(d.reactivity_mod),
        f(d.outgasser_mod),
        f(d.surge_mod),
        i(d.flux_range),
        i(d.reasim_range),
        b(d.reasim_boilers),
        f(d.reasim_boiler_speed),
        b(d.meltdowns_disabled),
        b(d.overpressure),
        f(d.moderator_efficiency),
        f(d.absorber_efficiency),
        f(d.reflector_efficiency),
        b(d.depletion_disabled),
        b(d.xenon_disabled),
        f(d.absorber_heat_conversion),
    ]
}

fn dials_of(v: &[u64]) -> Option<Dials> {
    let f = |k: usize| v.get(k).map(|b| f64::from_bits(*b));
    let i = |k: usize| v.get(k).map(|b| *b as u32 as i32);
    let b = |k: usize| v.get(k).map(|b| *b != 0);
    Some(Dials {
        passive_cooling: f(0)?,
        passive_cooling_inner: f(1)?,
        column_heat_flow: f(2)?,
        fuel_diffusion_mod: f(3)?,
        heat_provision: f(4)?,
        column_height: i(5)?,
        permanent_scrap: b(6)?,
        boiler_heat_consumption: f(7)?,
        control_speed: f(8)?,
        reactivity_mod: f(9)?,
        outgasser_mod: f(10)?,
        surge_mod: f(11)?,
        flux_range: i(12)?,
        reasim_range: i(13)?,
        reasim_boilers: b(14)?,
        reasim_boiler_speed: f(15)?,
        meltdowns_disabled: b(16)?,
        overpressure: b(17)?,
        moderator_efficiency: f(18)?,
        absorber_efficiency: f(19)?,
        reflector_efficiency: f(20)?,
        depletion_disabled: b(21)?,
        xenon_disabled: b(22)?,
        absorber_heat_conversion: f(23)?,
    })
}

impl ReactorDump {
    /// The reactor `r` as it is (the input of its next step, or the result of its last one).
    pub fn of(r: &Reactor, dim: &str) -> Self {
        let columns = r
            .columns
            .iter()
            .map(|c| ColumnDump {
                kind: KINDS
                    .iter()
                    .find(|k| k.0 == c.kind)
                    .map_or("?", |k| k.1)
                    .to_string(),
                x: c.x,
                z: c.z,
                lid: c.lid,
                moderated: c.moderated,
                heat: c.heat.to_bits(),
                water: c.reasim_water,
                steam: c.reasim_steam,
                rod: c.rod().map(|r| RodDump {
                    flux: [
                        r.flux_quantity,
                        r.flux_fast_ratio,
                        r.last_flux_quantity,
                        r.last_flux_ratio,
                    ]
                    .map(f64::to_bits),
                    has_rod: r.has_rod,
                    fuel: r.fuel.as_ref().map(|(t, s)| {
                        (
                            format!(
                                "ntm:{}",
                                t.variable.replace("rbmk_fuel_hep239", "rbmk_fuel_hep")
                            ),
                            StackDump {
                                has_tag: s.has_tag,
                                keys: [s.yield_, s.xenon, s.core, s.hull]
                                    .map(|v| v.map(f64::to_bits)),
                            },
                        )
                    }),
                }),
            })
            .collect();
        let mut obstacles: Vec<(i32, i32, i32)> = r.obstacles.iter().copied().collect();
        obstacles.sort();
        let (seed, gaussian) = r.rng.state();
        ReactorDump {
            tick: r.time,
            dim: dim.to_string(),
            y: r.y,
            dials: dials_bits(&r.dials),
            rng: (seed, gaussian.map(f64::to_bits)),
            queue: r
                .queued()
                .iter()
                .map(|q| {
                    (
                        q.origin,
                        [q.vx, q.vz, q.flux_quantity, q.flux_ratio].map(f64::to_bits),
                    )
                })
                .collect(),
            obstacles,
            columns,
        }
    }

    /// The reactor this dump describes, as the server builds it for a step (the columns' other data at their fresh
    /// values, as the server gives them).
    pub fn reactor(&self) -> Option<Reactor> {
        let mut r = Reactor::new(
            dials_of(&self.dials)?,
            JavaRandom::from_state(self.rng.0, self.rng.1.map(f64::from_bits)),
            self.y,
        );
        r.time = self.tick;
        for c in &self.columns {
            let kind = KINDS.iter().find(|k| k.1 == c.kind)?.0;
            let mut col = Column::new(kind, c.x, c.z);
            col.lid = c.lid;
            col.moderated = c.moderated;
            col.heat = f64::from_bits(c.heat);
            col.reasim_water = c.water;
            col.reasim_steam = c.steam;
            if let (Some(rod), Some(d)) = (col.rod_mut(), &c.rod) {
                let [q, fr, lq, lr] = d.flux.map(f64::from_bits);
                (
                    rod.flux_quantity,
                    rod.flux_fast_ratio,
                    rod.last_flux_quantity,
                    rod.last_flux_ratio,
                    rod.has_rod,
                ) = (q, fr, lq, lr, d.has_rod);
                if let Some((item, s)) = &d.fuel {
                    let [y, x, co, h] = s.keys.map(|v| v.map(f64::from_bits));
                    rod.fuel = Some((
                        rod_of(item)?,
                        RodStack {
                            has_tag: s.has_tag,
                            yield_: y,
                            xenon: x,
                            core: co,
                            hull: h,
                            ..RodStack::bare()
                        },
                    ));
                }
            }
            r.add(col);
        }
        r.set_queued(
            self.queue
                .iter()
                .map(|(o, b)| QueuedStream {
                    origin: *o,
                    vx: f64::from_bits(b[0]),
                    vz: f64::from_bits(b[1]),
                    flux_quantity: f64::from_bits(b[2]),
                    flux_ratio: f64::from_bits(b[3]),
                })
                .collect(),
        );
        for o in &self.obstacles {
            r.obstacles.insert(*o);
        }
        Some(r)
    }

    /// Whether `later`, a dump of the same reactor `later.tick - self.tick` ticks on, is what stepping this one that
    /// many times gives: its columns, random and queue bit for bit (the dials and occlusion are inputs, kept as they
    /// were). The first difference otherwise.
    pub fn replays_to(&self, later: &ReactorDump) -> Result<u64, String> {
        let steps = later.tick - self.tick;
        if steps < 0 {
            return Err(format!(
                "the later dump is earlier ({} < {})",
                later.tick, self.tick
            ));
        }
        let mut r = self.reactor().ok_or("an unreadable dump")?;
        for _ in 0..steps {
            r.step();
        }
        let mut got = ReactorDump::of(&r, &self.dim);
        got.dials.clone_from(&later.dials);
        got.obstacles.clone_from(&later.obstacles);
        if got == *later {
            return Ok(steps as u64);
        }
        let what = if got.rng != later.rng {
            "random".to_string()
        } else if got.queue != later.queue {
            format!(
                "queue ({} streams, {} expected)",
                got.queue.len(),
                later.queue.len()
            )
        } else {
            let i = got
                .columns
                .iter()
                .zip(&later.columns)
                .position(|(a, b)| a != b)
                .unwrap_or(0);
            format!(
                "column {i}: {:?} against {:?}",
                got.columns.get(i),
                later.columns.get(i)
            )
        };
        Err(format!("{steps} steps from tick {}: {what}", self.tick))
    }

    /// How many of the ticks between this dump and `later` the reactor stepped, when it may have waited up to
    /// `max_wait` of them (D4: a reactor whose columns the world shows and the server has not adopted yet waits, as at
    /// a world's opening): `(steps, waited)` for the one count that gives `later` bit for bit, its tick aside.
    pub fn steps_to(&self, later: &ReactorDump, max_wait: u64) -> Result<(u64, u64), String> {
        let span = later.tick - self.tick;
        if span < 0 {
            return Err(format!(
                "the later dump is earlier ({} < {})",
                later.tick, self.tick
            ));
        }
        let span = span as u64;
        let mut r = self.reactor().ok_or("an unreadable dump")?;
        let first = span.saturating_sub(max_wait);
        for _ in 0..first {
            r.step();
        }
        for steps in first..=span {
            let mut got = ReactorDump::of(&r, &self.dim);
            got.tick = later.tick;
            got.dials.clone_from(&later.dials);
            got.obstacles.clone_from(&later.obstacles);
            if got == *later {
                return Ok((steps, span - steps));
            }
            r.step();
        }
        Err(format!(
            "no count of steps from {first} to {span} gives it (tick {} to {})",
            self.tick, later.tick
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rbmk::dials::{DialState, GameRules};
    use crate::rbmk::reactor::Kind as K;

    fn small() -> Reactor {
        let (mut d, mut rules) = (DialState::default(), GameRules::default());
        d.create(&mut rules, "o", false);
        let mut r = Reactor::new(*d.cached("o").unwrap(), JavaRandom::new(11), 64);
        for (x, k, rod) in [
            (0, K::RodReaSim, Some("ntm:rbmk_fuel_po210be")),
            (1, K::Moderator, None),
            (2, K::Rod, Some("ntm:rbmk_fuel_ueu")),
            (3, K::Control, None),
        ] {
            let mut c = Column::new(k, x, 0);
            if let Some(item) = rod {
                let t = rod_of(item).unwrap();
                c.rod_mut().unwrap().fuel = Some((t, RodStack::fresh(t)));
            }
            r.add(c);
        }
        r.obstacles.insert((0, 2, 1));
        r
    }

    /// A dump reads back as the reactor it was taken of, and a later dump is what stepping the earlier gives; a dump
    /// one tick off, or of a reactor changed by hand, is told apart.
    #[test]
    fn a_dump_replays_to_the_next_bit_for_bit() {
        let mut r = small();
        for _ in 0..5 {
            r.step();
        }
        let a = ReactorDump::of(&r, "o");
        let back = a.reactor().unwrap();
        assert_eq!(ReactorDump::of(&back, "o"), a);
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(serde_json::from_str::<ReactorDump>(&json).unwrap(), a);
        for _ in 0..37 {
            r.step();
        }
        let b = ReactorDump::of(&r, "o");
        assert_eq!(a.replays_to(&b), Ok(37));
        let mut early = b.clone();
        early.tick -= 1;
        assert!(a.replays_to(&early).is_err(), "a round lost");
        let mut touched = b.clone();
        touched.columns[1].heat = (f64::from_bits(touched.columns[1].heat) + 1e-9).to_bits();
        assert!(a.replays_to(&touched).is_err());
        // Waited ticks are told: `b` read as two ticks later is 37 steps and 2 ticks waited; never more than asked.
        let mut late = b.clone();
        late.tick += 2;
        assert_eq!(a.steps_to(&late, 3), Ok((37, 2)));
        assert_eq!(a.steps_to(&b, 3), Ok((37, 0)));
        assert!(a.steps_to(&late, 1).is_err());
        assert!(a.steps_to(&touched, 3).is_err());
    }

    /// The dumps of a proof (`RBMK_T04_DUMPS`, a folder of `*.json` from `ntmtest rbmkdump`): each pair a chain names
    /// (`<chain>-<n>.json`, in order of `n`) must replay, every tick a step, or with up to `RBMK_T04_WAIT` ticks
    /// (default 0) the reactor waited (D4, a world's opening), each reported. Run after the proof, not by the suite.
    #[test]
    #[ignore]
    fn the_games_dumps_replay() {
        let Some(dir) = std::env::var_os("RBMK_T04_DUMPS") else {
            return;
        };
        let mut chains: std::collections::BTreeMap<String, Vec<(u32, ReactorDump)>> =
            Default::default();
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let name = p.file_stem().unwrap().to_string_lossy().to_string();
            let Some((chain, n)) = name.rsplit_once('-') else {
                continue;
            };
            let Ok(n) = n.parse::<u32>() else { continue };
            let d: ReactorDump = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
            chains.entry(chain.to_string()).or_default().push((n, d));
        }
        assert!(!chains.is_empty(), "no dumps");
        let wait = std::env::var("RBMK_T04_WAIT")
            .ok()
            .and_then(|w| w.parse().ok())
            .unwrap_or(0);
        let mut failed = Vec::new();
        for (chain, mut dumps) in chains {
            dumps.sort_by_key(|d| d.0);
            for w in dumps.windows(2) {
                let r = w[0].1.steps_to(&w[1].1, wait);
                println!(
                    "{chain} {}→{}: ticks {}→{}, {} columns: {r:?}",
                    w[0].0,
                    w[1].0,
                    w[0].1.tick,
                    w[1].1.tick,
                    w[1].1.columns.len()
                );
                if r.is_err() {
                    failed.push(format!("{chain} {}→{}", w[0].0, w[1].0));
                }
            }
        }
        assert!(failed.is_empty(), "{failed:?}");
    }
}
