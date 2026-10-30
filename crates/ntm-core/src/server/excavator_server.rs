//! The excavator's world (docs/maquinas.md, "Bedrock"; `crate::excavator`):
//! the shaft under it, ring by ring, and the deposit at its bottom
//! (`TileEntityMachineExcavator.tryDrill`, `collectBedrock`). A layer is
//! read once when the drill reaches it (and again before it breaks it), not
//! every tick: a block read costs.
use super::*;
use crate::bedrock;
use crate::excavator::{self, BEDROCK_WORK, Drill, Excavator, Layer};
use crate::machine::Data;
use crate::server::container_bridge::Stack;

const DEPOSIT: &str = "ntm:ore_bedrock";

/// The lowest layer the drill reaches.
fn floor(dim: &str) -> i32 {
    if dim == "minecraft:overworld" { -64 } else { 0 }
}

impl Server {
    pub(super) fn excavator_tick(&mut self, host: &mut impl ServerHost) {
        let r = &mut self.ready;
        let keys: Vec<Key> = r
            .world
            .machines
            .iter()
            .filter(|(k, m)| r.loaded.get(*k) == Some(&true) && matches!(m.data, Data::Excavator(_)))
            .map(|(k, _)| k.clone())
            .collect();
        for key in keys {
            if let Some(Data::Excavator(e)) = r.world.machines.get_mut(&key).map(|m| &mut m.data) {
                step(e, host, &key);
            }
        }
    }
}

/// The cells of ring `ring` round the shaft's axis at `y`: the whole 3×3
/// for the first, the square's edge for the others.
fn ring_cells(key: &Key, y: i32, ring: i32) -> impl Iterator<Item = Pos> {
    let [cx, _, cz] = key.1;
    (-ring..=ring).flat_map(move |dx| (-ring..=ring).map(move |dz| (dx, dz))).filter(move |&(dx, dz)| ring == 1 || dx.abs() == ring || dz.abs() == ring).map(move |(dx, dz)| [cx + dx, y, cz + dz])
}

/// A ring's blocks: their work, or the deposit in it (`None`: a chunk not
/// there). Depth rock turns the drill off.
fn scan(e: &mut Excavator, host: &mut impl ServerHost, key: &Key, y: i32, ring: i32) -> Option<Layer> {
    let mut l = Layer { y, ring, work: 0.0, empty: true, deposit: None };
    for p in ring_cells(key, y, ring) {
        let b = host.get_block(&key.0, p)?;
        let id = blockstate::id_of(&b);
        if id == DEPOSIT {
            return Some(Layer { work: BEDROCK_WORK, empty: false, deposit: Some(p), ..l });
        }
        if excavator::is_depth_rock(id) {
            e.enabled = false;
            continue;
        }
        if let Some(h) = excavator::hardness(id) {
            l.work += h;
            l.empty = false;
        }
    }
    Some(l)
}

fn step(e: &mut Excavator, host: &mut impl ServerHost, key: &Key) -> bool {
    let was = (e.operational, e.depth, e.enabled);
    e.operational = false;
    e.power = crate::machine::charge_from_slot(&e.slots, excavator::slot::BATTERY, e.power, excavator::MAX_POWER);
    let need = e.consumption();
    let d = match e.drill() {
        Some(d) if e.enabled && e.power >= need => d,
        _ => {
            // Off, the drill draws back up (`targetDepth = 0`).
            e.depth = 0;
            e.layer = None;
            return was != (e.operational, e.depth, e.enabled);
        }
    };
    let y = key.1[1] - 4 - e.depth as i32;
    if y < floor(&key.0) {
        e.enabled = false;
        return true;
    }
    e.power -= need;
    e.operational = true;
    let radius = if e.depth == 0 || y == floor(&key.0) { 1 } else { e.radius() };
    if try_drill(e, host, key, y, radius, d) == Some(true) {
        e.depth += 1;
        e.layer = None;
        if y - 1 < floor(&key.0) {
            e.enabled = false;
        }
    }
    true
}

/// `tryDrill`: the first ring with something in it is worked; once worked
/// out it is broken and walled (the outermost with the walls on, and the
/// ores just past it mined). True when every ring is empty: down a layer.
/// The ring found empty is kept, so a tick doesn't read the inner ones
/// again.
fn try_drill(e: &mut Excavator, host: &mut impl ServerHost, key: &Key, y: i32, radius: i32, d: Drill) -> Option<bool> {
    let first = e.layer.as_ref().filter(|l| l.y == y).map_or(1, |l| l.ring);
    for ring in first..=radius {
        if e.layer.as_ref().is_none_or(|l| l.y != y || l.ring != ring) {
            e.layer = Some(scan(e, host, key, y, ring)?);
        }
        let l = e.layer.clone()?;
        if l.empty {
            continue;
        }
        if l.deposit.is_some() {
            e.crusher = false;
        }
        e.worked += 1;
        if f64::from(e.worked) >= (l.work / e.speed(d)).ceil() {
            e.worked = 0;
            match l.deposit {
                Some(p) => collect(e, &key.0, p, d),
                None => {
                    let mut w = Writes::default();
                    for p in ring_cells(key, y, ring) {
                        mine_at(e, host, &key.0, p, &mut w, false);
                    }
                    build_wall(host, key, y, ring + 1, ring == radius && e.walling, &mut w);
                    if ring == radius {
                        for p in ring_cells(key, y, ring + 1) {
                            mine_at(e, host, &key.0, p, &mut w, true);
                        }
                    }
                    w.flush(host, &key.0);
                    e.layer = None;
                }
            }
        }
        return Some(false);
    }
    let mut w = Writes::default();
    build_wall(host, key, y, radius + 1, e.walling, &mut w);
    w.flush(host, &key.0);
    e.worked = 0;
    Some(true)
}

/// The blocks a step sets, in one call at the end; read back by the vein
/// miner meanwhile.
#[derive(Default)]
struct Writes(std::collections::BTreeMap<Pos, String>);

impl Writes {
    fn get(&self, host: &mut impl ServerHost, dim: &str, p: Pos) -> Option<String> {
        match self.0.get(&p) {
            Some(b) => Some(b.clone()),
            None => host.get_block(dim, p),
        }
    }
    fn flush(self, host: &mut impl ServerHost, dim: &str) {
        if self.0.is_empty() {
            return;
        }
        let v: Vec<(Pos, String)> = self.0.into_iter().collect();
        if let Err(err) = host.set_blocks(dim, &v) {
            host.log(&format!("excavator: {err}"));
        }
    }
}

const AIR: &str = "minecraft:air";

/// `tryMineAtLocation` (`ores_only`: `mineOuterOres`): an ore with the vein
/// miner takes its whole vein, anything else breaks alone.
fn mine_at(e: &mut Excavator, host: &mut impl ServerHost, dim: &str, p: Pos, w: &mut Writes, ores_only: bool) {
    let Some(b) = w.get(host, dim, p) else { return };
    let id = blockstate::id_of(&b).to_string();
    if id == DEPOSIT || excavator::hardness(&id).is_none() || excavator::is_depth_rock(&id) {
        return;
    }
    let ore = excavator::is_ore(&id);
    if ores_only && !ore {
        return;
    }
    if ore && e.can_vein_mine() {
        let mut seen = std::collections::BTreeSet::new();
        vein(e, host, dim, p, &id, 10, &mut seen, w);
        return;
    }
    break_one(e, &b, p, w);
}

/// `breakRecursively`: the same block on the six sides, 10 deep; a vein
/// broken with the walls on is filled with sandbags.
#[allow(clippy::too_many_arguments)]
fn vein(e: &mut Excavator, host: &mut impl ServerHost, dim: &str, p: Pos, id: &str, depth: i32, seen: &mut std::collections::BTreeSet<Pos>, w: &mut Writes) {
    if depth < 0 || !seen.insert(p) {
        return;
    }
    for d in crate::energy::DIRS {
        let n = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
        if w.get(host, dim, n).is_some_and(|b| blockstate::id_of(&b) == id) {
            vein(e, host, dim, n, id, depth - 1, seen, w);
        }
    }
    if break_one(e, id, p, w) && e.walling {
        w.0.insert(p, excavator::BARRICADE.into());
    }
}

/// `breakSingleBlock`: its drops into the buffer and the block gone; a
/// full buffer leaves it there.
fn break_one(e: &mut Excavator, block: &str, p: Pos, w: &mut Writes) -> bool {
    let drops = e.drops(block);
    if !e.fits_all(&drops) {
        return false;
    }
    for s in drops {
        e.put(s);
    }
    w.0.insert(p, AIR.into());
    true
}

/// `buildWall`: sandbags on ring `ring`'s edge where fluid is (where
/// anything replaceable is, with `all`), and the fluid inside it gone.
fn build_wall(host: &mut impl ServerHost, key: &Key, y: i32, ring: i32, all: bool, w: &mut Writes) {
    let [cx, _, cz] = key.1;
    for dx in -ring..=ring {
        for dz in -ring..=ring {
            let p = [cx + dx, y, cz + dz];
            let Some(b) = w.get(host, &key.0, p) else { continue };
            let edge = dx.abs() == ring || dz.abs() == ring;
            if edge {
                if excavator::replaceable(&b) && (all || excavator::is_liquid(&b)) {
                    w.0.insert(p, excavator::BARRICADE.into());
                }
            } else if excavator::is_liquid(&b) {
                w.0.insert(p, AIR.into());
            }
        }
    }
}

/// `collectBedrock`: one extraction, if the bit reaches the deposit's tier
/// and the tank has its fluid.
fn collect(e: &mut Excavator, dim: &str, p: Pos, d: Drill) {
    let stack = if dim == "minecraft:the_nether" {
        nether(p)
    } else {
        let dep = bedrock::deposit(p[0], p[2]);
        if dep.tier > d.tier {
            return;
        }
        if let Some((f, mb)) = dep.fluid {
            if e.tank.fluid != f || e.tank.mb < mb {
                return;
            }
            if !e.fits(&raw(p, d)) {
                return;
            }
            e.tank.drain(mb);
        }
        raw(p, d)
    };
    if e.fits(&stack) {
        e.put(stack);
    }
}

/// The raw ore of the deposit at `p`, its densities times the bit's
/// fortune (1 + 0.1 a level).
fn raw(p: Pos, d: Drill) -> Stack {
    let data = bedrock::ore_data(&bedrock::densities(p[0], p[2]), 1.0 + 0.1 * f64::from(d.fortune));
    Stack { custom: Some(data), ..Stack::new(bedrock::slopper::RAW, 1) }
}

/// A nether deposit (`BedrockOre.weightedOresNether`, weights 100/50/100),
/// picked by its chunk.
fn nether(p: Pos) -> Stack {
    let h = crate::hash::mix(((p[0] >> 4) as u32 as u64) << 32 | (p[2] >> 4) as u32 as u64) % 250;
    let id = match h {
        0..100 => "minecraft:glowstone_dust",
        100..150 => "ntm:powder_fire",
        _ => "minecraft:quartz",
    };
    Stack::new(id, 4)
}
