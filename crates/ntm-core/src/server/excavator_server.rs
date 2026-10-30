//! The excavator's world (docs/maquinas.md, "Bedrock"; `crate::excavator`):
//! the 3×3 shaft under it and the deposit at its bottom
//! (`TileEntityMachineExcavator.tryDrill`, `collectBedrock`). A layer is
//! read once when the drill reaches it (and again before it breaks it), not
//! every tick: a block read costs.
use super::*;
use crate::bedrock;
use crate::excavator::{self, BEDROCK_WORK, Drill, Excavator, HE_PER_TICK, Layer};
use crate::machine::Data;
use crate::server::container_bridge::Stack;

const DEPOSIT: &str = "ntm:ore_bedrock";

/// The lowest layer the drill reaches.
fn floor(dim: &str) -> i32 {
    if dim == "minecraft:overworld" { -64 } else { 0 }
}

impl Server {
    pub(super) fn excavator_tick(&mut self, host: &mut impl ServerHost) {
        let Session::Ready(r) = &mut self.session else { return };
        let keys: Vec<Key> = r
            .world
            .machines
            .iter()
            .filter(|(k, m)| r.loaded.get(*k) == Some(&true) && matches!(m.data, Data::Excavator(_)))
            .map(|(k, _)| k.clone())
            .collect();
        let mut changed = false;
        for key in keys {
            if let Some(Data::Excavator(e)) = r.world.machines.get_mut(&key).map(|m| &mut m.data) {
                changed |= step(e, host, &key);
            }
        }
        if changed {
            r.dirty = true;
        }
    }
}

/// The 3×3 at `y` under the core: its blocks' work, or the deposit in it
/// (`None`: a chunk not there).
fn scan(host: &mut impl ServerHost, key: &Key, y: i32) -> Option<Layer> {
    let mut work = 0.0;
    for dx in -1..=1 {
        for dz in -1..=1 {
            let p = [key.1[0] + dx, y, key.1[2] + dz];
            let b = host.get_block(&key.0, p)?;
            let id = blockstate::id_of(&b);
            if id == DEPOSIT {
                return Some(Layer { y, work: BEDROCK_WORK, deposit: Some(p) });
            }
            work += excavator::hardness(id).unwrap_or(0.0);
        }
    }
    Some(Layer { y, work, deposit: None })
}

fn step(e: &mut Excavator, host: &mut impl ServerHost, key: &Key) -> bool {
    let was = e.operational;
    e.operational = false;
    e.power = crate::machine::charge_from_slot(&e.slots, excavator::slot::BATTERY, e.power, excavator::MAX_POWER);
    let Some(d) = e.drill() else { return was };
    if !e.enabled || e.power < HE_PER_TICK {
        return was;
    }
    let y = key.1[1] - 4 - e.depth as i32;
    if y < floor(&key.0) {
        e.enabled = false;
        return true;
    }
    if e.layer.as_ref().is_none_or(|l| l.y != y) {
        let Some(l) = scan(host, key, y) else { return was };
        e.layer = Some(l);
    }
    let Some(l) = e.layer.clone() else { return was };
    e.power -= HE_PER_TICK;
    e.operational = true;
    if l.deposit.is_none() && l.work == 0.0 {
        // Nothing to dig here: down a layer.
        e.depth += 1;
        e.layer = None;
        e.worked = 0;
        return true;
    }
    e.worked += 1;
    if f64::from(e.worked) < (l.work / d.speed).ceil() {
        return true;
    }
    e.worked = 0;
    match l.deposit {
        Some(p) => collect(e, &key.0, p, d),
        None => {
            dig(e, host, key, y);
            e.layer = None;
        }
    }
    true
}

/// The layer's blocks into the buffer, as many as fit (a full buffer
/// stops the drill at the block that doesn't).
fn dig(e: &mut Excavator, host: &mut impl ServerHost, key: &Key, y: i32) {
    let mut air = Vec::new();
    for dx in -1..=1 {
        for dz in -1..=1 {
            let p = [key.1[0] + dx, y, key.1[2] + dz];
            let Some(b) = host.get_block(&key.0, p) else { continue };
            let id = blockstate::id_of(&b);
            if id == DEPOSIT || excavator::hardness(id).is_none() {
                continue;
            }
            if let Some(item) = excavator::drop_of(&b) {
                let s = Stack::new(&item, 1);
                if !e.fits(&s) {
                    break;
                }
                e.put(s);
            }
            air.push((p, "minecraft:air".to_string()));
        }
    }
    if !air.is_empty()
        && let Err(err) = host.set_blocks(&key.0, &air)
    {
        host.log(&format!("excavator: {err}"));
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
