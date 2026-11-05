//! The oil chain's world (docs/maquinas.md, "Petróleo"): fractioning towers
//! stacked one on another (`TileEntityMachineFractionTower`: the one below
//! pushes its input up and pulls the fractions down, each tick), and the
//! heat boilers, cokers and heaters take from the heat source under them.
use super::*;
use crate::foundry::HeatSource;
use crate::heater::Heater;
use std::collections::BTreeMap;
use crate::oil_drill::{self, Drill, Shape, indicator};
use crate::fraction::{FractionTower, STACK_DY};
use crate::machine::Data;

impl Server {
    pub(super) fn fraction_tick(&mut self) {
        let r = &mut self.ready;
        let mut keys: Vec<Key> = r
            .world
            .machines
            .iter()
            .filter(|(k, m)| r.loaded.get(*k) == Some(&true) && matches!(m.data, Data::FractionTower(_)))
            .map(|(k, _)| k.clone())
            .collect();
        // From the bottom of each stack up, as the NTM's lower tower acts.
        keys.sort_by_key(|k| k.1[1]);
        for key in keys {
            let above = (key.0.clone(), [key.1[0], key.1[1] + STACK_DY, key.1[2]]);
            let Some(mut upper) = r.world.machines.remove(&above) else { continue };
            if let (Some(Data::FractionTower(low)), Data::FractionTower(high)) = (r.world.machines.get_mut(&key).map(|m| &mut m.data), &mut upper.data) {
                FractionTower::stack(low, high);
            }
            r.world.machines.insert(above, upper);
        }
    }

    /// Boilers, cokers, the heating oven and the electric heater take heat
    /// from the source under them (`tryPullHeat`), before they run; from the
    /// bottom up, so a stack of heaters passes it on within the tick.
    pub(super) fn heat_tick(&mut self) {
        let r = &mut self.ready;
        let mut keys: Vec<Key> = r
            .world
            .machines
            .iter()
            .filter(|(k, m)| {
                r.loaded.get(*k) == Some(&true)
                    && match &m.data {
                        Data::Boiler(_) | Data::Coker(_) => true,
                        Data::SolarStirling(s) => matches!(**s, crate::solar_stirling::SolarStirling::Stirling(_)),
                        Data::Firebox(f) => f.oven,
                        Data::Heater(h) => matches!(**h, Heater::Electric(_)),
                        _ => false,
                    }
            })
            .map(|(k, _)| k.clone())
            .collect();
        keys.sort_by_key(|k| k.1[1]);
        for key in keys {
            let under = heat_source_under(&r.world.machines, &key);
            let source = under.as_ref().and_then(|u| r.world.machines.get_mut(u)).and_then(|m| heat_source(&mut m.data)).map(|s| s.heat_stored());
            let taken = match r.world.machines.get_mut(&key).map(|m| &mut m.data) {
                Some(Data::SolarStirling(s)) => match &mut **s { crate::solar_stirling::SolarStirling::Stirling(s) => s.pull_heat(source), _ => 0 },
                Some(Data::Boiler(b)) => b.pull_heat(source),
                Some(Data::Coker(c)) => c.pull_heat(source),
                Some(Data::Firebox(f)) => f.pull_heat(source),
                Some(Data::Heater(h)) => match &mut **h {
                    Heater::Electric(e) => e.pull_heat(source),
                    _ => 0,
                },
                _ => 0,
            };
            if taken > 0
                && let Some(s) = under.as_ref().and_then(|u| r.world.machines.get_mut(u)).and_then(|m| heat_source(&mut m.data))
            {
                s.use_heat(taken);
            }
        }
    }

    /// The oil drills' work under them (`TileEntityOilDrillBase.updateEntity`):
    /// down the pipe to the first other block, sucked if it is a deposit,
    /// else drilled into pipe.
    pub(super) fn drill_tick(&mut self, host: &mut impl ServerHost) {
        let r = &mut self.ready;
        let keys: Vec<Key> = r
            .world
            .machines
            .iter()
            .filter(|(k, m)| r.loaded.get(*k) == Some(&true) && matches!(m.data, Data::Drill(_)))
            .map(|(k, _)| k.clone())
            .collect();
        for key in keys {
            let Some(Data::Drill(d)) = r.world.machines.get_mut(&key).map(|m| &mut m.data) else { continue };
            if d.tick_power() {
                let writes = work(d, host, &key);
                if !writes.is_empty()
                    && let Err(e) = host.set_blocks(&key.0, &writes)
                {
                    host.log(&format!("drill: {e}"));
                }
            }
        }
    }
}

/// What stores heat for the machine above it (`IHeatSource`): the
/// firebox, the heating oven and the heaters.
pub(super) fn heat_source(d: &mut Data) -> Option<&mut dyn HeatSource> {
    match d {
        Data::Firebox(f) => Some(f),
        Data::Heater(h) => Some(&mut **h),
        _ => None,
    }
}

/// The heat source a machine at `key` sits on: the core right under it, or
/// an oil burner's two under (its top centre is its heat proxy).
pub(super) fn heat_source_under(machines: &BTreeMap<Key, crate::machine::Machine>, key: &Key) -> Option<Key> {
    let at = |dy: i32| (key.0.clone(), [key.1[0], key.1[1] - dy, key.1[2]]);
    match machines.get(&at(1)).map(|m| &m.data) {
        Some(Data::Firebox(_) | Data::Heater(_)) => Some(at(1)),
        Some(_) => None,
        None => matches!(machines.get(&at(2)).map(|m| &m.data), Some(Data::Heater(h)) if h.height() == 2).then(|| at(2)),
    }
}

/// Cells a suck may read through emptied deposits (the NTM: 64 deep).
const SUCK_READS: usize = 512;

/// One working of the pipe: the blocks it sets.
fn work(d: &mut Drill, host: &mut impl ServerHost, key: &Key) -> Vec<(Pos, String)> {
    let floor = if key.0 == "minecraft:overworld" { -64 } else { 0 };
    let depth = d.shape.spec().depth.max(floor);
    let (x, z) = (key.1[0], key.1[2]);
    d.indicator = indicator::WORKING;
    let mut writes = Vec::new();
    let mut y = key.1[1] - 1;
    while y >= depth {
        let Some(b) = host.get_block(&key.0, [x, y, z]) else { break };
        let id = blockstate::id_of(&b).to_string();
        if id != oil_drill::PIPE {
            let p = [x, y, z];
            let sucked = d.shape.sucks(&id) && (!d.can_pump() || suck(d, host, &key.0, key.1, p, &mut writes));
            if !sucked {
                if drillable(&id) {
                    writes.push((p, oil_drill::PIPE.to_string()));
                } else {
                    d.indicator = indicator::STOPPED;
                }
            }
            return writes;
        }
        if y == depth {
            d.indicator = indicator::BOTTOM;
        }
        y -= 1;
    }
    writes
}

/// `suckRec` from `start`: the first oil (or, for the fracker, bedrock oil)
/// through emptied deposits, 64 steps deep; its yield taken. False when
/// there is none.
fn suck(d: &mut Drill, host: &mut impl ServerHost, dim: &str, core:Pos, start: Pos, writes: &mut Vec<(Pos, String)>) -> bool {
    let mut seen: BTreeSet<Pos> = BTreeSet::new();
    let mut stack = vec![(start, 0u32)];
    while let Some((p, layer)) = stack.pop() {
        if layer > 64 || seen.len() >= SUCK_READS || !seen.insert(p) {
            continue;
        }
        let Some(b) = host.get_block(dim, p) else { continue };
        let id = blockstate::id_of(&b);
        if id == oil_drill::OIL || id == oil_drill::BEDROCK_OIL {
            // A well over bedrock oil finds it and takes nothing (`doSuck`).
            if id == oil_drill::OIL || d.shape == Shape::Fracker {
                if d.shape == Shape::Well {
                    host.show(dim,&crate::blast::visuals::Show::Sound{id:"minecraft:entity.generic.splash",source:"neutral",at:core.map(f64::from),volume:2.,pitch:0.5});
                }
                if d.suck(id) {
                    writes.push((p, oil_drill::EMPTY.to_string()));
                }
                if d.shape == Shape::Fracker {
                    oil_spots(d, host, dim, start, writes);
                }
            }
            return true;
        }
        if id == oil_drill::EMPTY {
            // The six sides in an order of its own draws (`getShuffledDirs`).
            let k = (d.roll() * 6.0) as usize;
            for i in 0..6 {
                let dir = crate::energy::DIRS[(k + i) % 6];
                stack.push(([p[0] + dir[0], p[1] + dir[1], p[2] + dir[2]], layer + 1));
            }
        }
    }
    false
}

/// What the pipe goes through: all but the blast-proof (`getExplosionResistance
/// < 1000`).
fn drillable(id: &str) -> bool {
    !matches!(
        id,
        "minecraft:bedrock" | "minecraft:barrier" | "minecraft:obsidian" | "minecraft:crying_obsidian" | "minecraft:end_portal_frame"
            | "minecraft:end_portal" | "minecraft:reinforced_deepslate" | "minecraft:command_block" | "ntm:ore_bedrock" | "ntm:ore_bedrock_oil"
    )
}

/// `OilSpot.generateOilSpot(x, z, 75, 10)`: ten spots round the fracker, each
/// a gaussian draw away; on each the ground spoils (grass and dirt dead or
/// oily, sand dirty, stone cracked) and plants and loose leaves go. The NTM
/// reads the heightmap; here the surface is the first solid block from 20
/// above the tower down (40 cells at most).
fn oil_spots(d: &mut Drill, host: &mut impl ServerHost, dim: &str, at: Pos, writes: &mut Vec<(Pos, String)>) {
    let gauss = |d: &mut Drill| {
        let (u, v) = (d.roll().max(1e-9), d.roll());
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    };
    for _ in 0..10 {
        let x = at[0] + (gauss(d) * 75.0) as i32;
        let z = at[2] + (gauss(d) * 75.0) as i32;
        let top = at[1].max(60) + 20;
        let mut y = top;
        let mut surface = None;
        while y > top - 40 {
            let Some(b) = host.get_block(dim, [x, y, z]) else { break };
            if !matches!(blockstate::id_of(&b), "minecraft:air" | "minecraft:cave_air") {
                surface = Some(y);
                break;
            }
            y -= 1;
        }
        let Some(sy) = surface else { continue };
        for y in (sy - 3..=sy).rev() {
            let Some(b) = host.get_block(dim, [x, y, z]) else { break };
            let id = blockstate::id_of(&b);
            let path = id.strip_prefix("minecraft:").unwrap_or("");
            let set = |w: &mut Vec<(Pos, String)>, s: &str| w.push(([x, y, z], s.to_string()));
            match path {
                "short_grass" | "tall_grass" | "fern" | "large_fern" | "dandelion" | "poppy" | "dead_bush" => set(writes, "minecraft:air"),
                "grass_block" | "dirt" => {
                    let s = if d.roll() < 0.1 { "ntm:dirt_oily" } else { "ntm:dirt_dead" };
                    set(writes, s);
                    break;
                }
                "sand" => {
                    set(writes, "ntm:sand_dirty");
                    break;
                }
                "red_sand" => {
                    set(writes, "ntm:sand_dirty_red");
                    break;
                }
                "stone" => {
                    set(writes, "ntm:stone_cracked");
                    break;
                }
                _ if path.ends_with("leaves") && b.contains("persistent=false") => {
                    set(writes, "minecraft:air");
                    break;
                }
                _ if id == "ntm:ore_oil_sand" => {
                    set(writes, "ntm:sand_dirty");
                    break;
                }
                _ => {}
            }
        }
    }
}
