//! Machines whose items live in the engine's slots (Horizon patch
//! `ntm-visual`, capability `machines`; docs/visual-fiel.md). The slots
//! are the truth: each tick, before the machines run, what players moved in
//! the slots since the last write is applied to the machine; after the
//! machines (and the transactions) ran, the machine's items are written
//! back. Transactions keep working beside the slots: they change the
//! machine, which the next write shows.
//!
//! The machine's screen values go with the write (`set-data`); a press of
//! the recipe button opens the recipe list of the old screen.
use super::{Key, Server, ServerHost};
use crate::{
    Pos,
    crafter::{Crafter, Screen, Station},
    machine::Data,
    proto::S2C,
};
use std::collections::BTreeMap;

/// Slot stacks as the engine reports them.
pub type Slots = Vec<Option<(String, u32)>>;

/// The blocks whose machines use the engine's slots, and their layout:
/// (block, first input slot, inputs, output slot).
pub const SLOTTED: &[(&str, Layout)] = &[
    (
        "ntm:machine_assembly_machine",
        Layout::Crafter {
            inputs: 4..16,
            output: 16,
        },
    ),
    ("ntm:nuke_boy", Layout::Bomb),
];

#[derive(Clone, Debug, PartialEq)]
pub enum Layout {
    /// HBM's processing machines: inputs, one output slot.
    Crafter {
        inputs: std::ops::Range<usize>,
        output: usize,
    },
    /// A bomb: one slot per part, in the bomb type's order.
    Bomb,
}

/// Multiblocks (tools/visual-gen/visual.json): the volume around the core
/// in the placer's frame (x right, z away from the player), as the engine
/// places it.
pub const MULTIBLOCKS: &[(&str, [i32; 3], [i32; 3])] = &[("ntm:machine_assembly_machine", [-1, 0, -1], [1, 0, 1])];

/// The cells of the multiblock of `block` at `core` that pass energy to
/// it (HBM's `makeExtra` proxies): its bottom layer but the core.
pub fn proxy_cells(block: &str, core: Pos, facing: Option<&str>) -> Vec<Pos> {
    let Some((_, min, max)) = MULTIBLOCKS.iter().find(|(b, ..)| *b == block) else {
        return Vec::new();
    };
    let Some(f) = facing.and_then(crate::registry::facing_dir) else {
        return Vec::new();
    };
    // The placer looked against the facing: z runs that way, x to their right.
    let fwd = [-f[0], -f[2]];
    let right = [-fwd[1], fwd[0]];
    let mut out = Vec::new();
    for z in min[2]..=max[2] {
        for x in min[0]..=max[0] {
            if (x, z) != (0, 0) {
                out.push([core[0] + right[0] * x + fwd[0] * z, core[1] + min[1], core[2] + right[1] * x + fwd[1] * z]);
            }
        }
    }
    out
}

pub fn layout(block: &str) -> Option<&'static Layout> {
    SLOTTED.iter().find(|(b, _)| *b == block).map(|(_, l)| l)
}

/// What the bridge last wrote to (or read from) each machine's slots.
#[derive(Debug, Default)]
pub struct Bridge {
    last: BTreeMap<Key, Slots>,
    data: BTreeMap<Key, Vec<i32>>,
}

fn totals<'a>(slots: impl Iterator<Item = &'a Option<(String, u32)>>) -> BTreeMap<String, i64> {
    let mut out = BTreeMap::new();
    for (id, n) in slots.flatten() {
        *out.entry(id.clone()).or_insert(0) += i64::from(*n);
    }
    out
}

fn add(list: &mut Vec<(String, u32)>, id: &str, d: i64) {
    match list.iter_mut().find(|(i, _)| i == id) {
        Some(e) => e.1 = (i64::from(e.1) + d).max(0) as u32,
        None if d > 0 => list.push((id.into(), d as u32)),
        None => {}
    }
    list.retain(|e| e.1 > 0);
}

/// What players did to the slots between `last` and `now`, applied to `list`.
fn apply_delta(list: &mut Vec<(String, u32)>, last: &BTreeMap<String, i64>, now: &BTreeMap<String, i64>) {
    let ids: std::collections::BTreeSet<&String> = last.keys().chain(now.keys()).collect();
    for id in ids {
        let d = now.get(id).copied().unwrap_or(0) - last.get(id).copied().unwrap_or(0);
        if d != 0 {
            add(list, id, d);
        }
    }
}

/// `want` laid over the slots `range` of `slots`: each slot keeps its item
/// while there is some of it left, the rest fills the empty slots.
fn lay_out(slots: &mut Slots, range: std::ops::Range<usize>, want: &[(String, u32)], max: u32) {
    let mut left: Vec<(String, u32)> = want.to_vec();
    for s in range.clone() {
        if let Some((id, _)) = &slots[s] {
            let have = left.iter_mut().find(|(i, _)| i == id).map_or(0, |e| {
                let t = e.1.min(max);
                e.1 -= t;
                t
            });
            slots[s] = (have > 0).then(|| (id.clone(), have));
        }
    }
    for s in range {
        if slots[s].is_some() {
            continue;
        }
        let Some(e) = left.iter_mut().find(|e| e.1 > 0) else {
            break;
        };
        let t = e.1.min(max);
        e.1 -= t;
        slots[s] = Some((e.0.clone(), t));
    }
}

/// The values the assembly machine's screen shows (tools/visual-gen/visual.json).
fn crafter_data(c: &Crafter) -> Vec<i32> {
    let clamp = |v: u64| v.min(i32::MAX as u64) as i32;
    let recipe = c.current();
    let progress = recipe.map_or(0, |r| (u64::from(c.progress) * 1000 / u64::from(r.duration.max(1))) as i32);
    let tank = |t: Option<&crate::fluid::Tank>| match t {
        Some(t) => [
            t.mb as i32,
            t.cap as i32,
            crate::fluid::get(&t.fluid).map_or(0, |f| f.color as i32),
        ],
        None => [0, 0, 0],
    };
    let [ia, ic, icol] = tank(c.tin.first());
    let [oa, oc, ocol] = tank(c.tout.first());
    let led = match recipe {
        Some(_) if c.progress > 0 => 2,
        Some(_) => 1,
        None => 0,
    };
    let power_ok = recipe.is_some_and(|r| c.power >= r.power) as i32;
    vec![
        clamp(c.power),
        clamp(c.max_power()),
        progress,
        1000,
        ia,
        ic,
        icol,
        oa,
        oc,
        ocol,
        led,
        power_ok,
    ]
}

impl Server {
    /// Before the machines run: what players moved in the slots.
    pub(super) fn slots_in(&mut self, host: &mut impl ServerHost) {
        let presses = host.machine_buttons();
        let Some(r) = self.ready() else { return };
        let keys: Vec<Key> = r
            .world
            .machines
            .iter()
            .filter(|(k, m)| layout(&m.block).is_some() && r.loaded.get(*k) != Some(&false))
            .map(|(k, _)| k.clone())
            .collect();
        for key in keys {
            let Some(now) = host.machine_slots(&key.0, key.1) else {
                continue;
            };
            let Some(m) = r.world.machines.get_mut(&key) else { continue };
            let Some(layout) = layout(&m.block) else { continue };
            let first = !r.bridge.last.contains_key(&key);
            let empty = now.iter().all(Option::is_none);
            let last = r.bridge.last.get(&key).cloned().unwrap_or_else(|| now.clone());
            match (&mut m.data, layout) {
                (Data::Crafter(c), Layout::Crafter { inputs, output }) => {
                    if first && !empty {
                        // A new session: the slots, saved with the world, are
                        // the truth; a machine whose slots are empty keeps what
                        // it had from transactions, written to them next.
                        c.items = totals(now[inputs.clone()].iter())
                            .into_iter()
                            .map(|(i, n)| (i, n as u32))
                            .collect();
                        // Outputs of other items than the slot's stay.
                        let slot = now.get(*output).cloned().flatten();
                        c.out.retain(|(i, _)| slot.as_ref().is_some_and(|(s, _)| s != i));
                        if let Some(s) = slot {
                            c.out.push(s);
                        }
                    } else {
                        apply_delta(&mut c.items, &totals(last[inputs.clone()].iter()), &totals(now[inputs.clone()].iter()));
                        let one = |s: &Slots| totals(s.get(*output).into_iter());
                        apply_delta(&mut c.out, &one(&last), &one(&now));
                    }
                }
                (Data::Bomb(b), Layout::Bomb) => {
                    if !(first && empty) {
                        // The slots are the truth; on a new session with empty
                        // slots, the parts it had from transactions go to them.
                        for (i, s) in b.slots.iter_mut().enumerate() {
                            *s = now.get(i).cloned().flatten().map(|(id, _)| id);
                        }
                    }
                }
                _ => continue,
            }
            r.bridge.last.insert(key, now);
            r.dirty = true;
        }
        for (_, dim, pos, button) in presses {
            self.machine_button(&(dim, pos), button);
        }
    }

    /// After the machines and the transactions: the machines' items back to
    /// the slots, and their screens' values.
    pub(super) fn slots_out(&mut self, host: &mut impl ServerHost) {
        let Some(r) = self.ready() else { return };
        let keys: Vec<Key> = r.bridge.last.keys().cloned().collect();
        for key in keys {
            let Some(m) = r.world.machines.get(&key) else {
                r.bridge.last.remove(&key);
                r.bridge.data.remove(&key);
                continue;
            };
            let Some(layout) = layout(&m.block) else { continue };
            let Some(last) = r.bridge.last.get(&key).cloned() else { continue };
            let mut want = last.clone();
            let data = match (&m.data, layout) {
                (Data::Crafter(c), Layout::Crafter { inputs, output }) => {
                    lay_out(&mut want, inputs.clone(), &c.items, crate::crafter::STACK);
                    // The output slot shows the first of the outputs; others
                    // stay in the machine (its old screen gives them).
                    let keep = want[*output].as_ref().map(|(i, _)| i.clone());
                    let shown = c
                        .out
                        .iter()
                        .find(|(i, _)| Some(i) == keep.as_ref())
                        .or_else(|| c.out.first())
                        .cloned();
                    want[*output] = shown.map(|(i, n)| (i, n.min(crate::crafter::STACK)));
                    crafter_data(c)
                }
                (Data::Bomb(b), Layout::Bomb) => {
                    for (i, s) in b.slots.iter().enumerate() {
                        if let Some(cell) = want.get_mut(i) {
                            *cell = s.clone().map(|id| (id, 1));
                        }
                    }
                    vec![b.blast().is_some() as i32]
                }
                _ => continue,
            };
            for (slot, (old, new)) in last.iter().zip(&want).enumerate() {
                if old != new {
                    host.set_machine_slot(&key.0, key.1, slot as u32, new.clone());
                }
            }
            if r.bridge.data.get(&key) != Some(&data) {
                host.set_machine_data(&key.0, key.1, &data);
                r.bridge.data.insert(key.clone(), data);
            }
            r.bridge.last.insert(key, want);
        }
    }

    /// The machine at `key` went away: true when its items were the
    /// engine's slots, which the engine drops itself.
    pub(super) fn slots_gone(&mut self, key: &Key) -> bool {
        let Some(r) = self.ready() else { return false };
        r.bridge.data.remove(key);
        r.bridge.last.remove(key).is_some()
    }

    /// A screen button: 0 opens the recipe list (the old screen's).
    fn machine_button(&mut self, key: &Key, button: u32) {
        if button != 0 || !self.within_reach(key) {
            return;
        }
        let Some(r) = self.ready() else { return };
        let Some(m) = r.world.machines.get_mut(key) else { return };
        if let Data::Crafter(c) = &mut m.data
            && c.station != Station::Centrifuge
        {
            c.screen = Screen::Pick;
            r.viewer = Some((key.clone(), None));
            self.mail.send(S2C::Show {
                dim: key.0.clone(),
                pos: key.1,
            });
            self.push_view(true);
        }
    }
}

/// The slots a bomb's parts take (one each), for tests.
pub fn bomb_slots(parts: &[Option<&str>]) -> Slots {
    parts.iter().map(|p| p.map(|id| (id.to_owned(), 1))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_layout_keeps_slots_and_fills_holes() {
        let mut slots: Slots = vec![None, Some(("a".into(), 5)), None];
        lay_out(&mut slots, 0..3, &[("a".into(), 3), ("b".into(), 2)], 64);
        assert_eq!(slots, vec![Some(("b".into(), 2)), Some(("a".into(), 3)), None]);
        let mut slots: Slots = vec![Some(("a".into(), 5)), None];
        lay_out(&mut slots, 0..2, &[("a".into(), 100)], 64);
        assert_eq!(slots, vec![Some(("a".into(), 64)), Some(("a".into(), 36))]);
    }

    #[test]
    fn a_delta_moves_only_what_players_moved() {
        let mut list = vec![("a".to_string(), 10)];
        let last = totals([Some(("a".to_string(), 4))].iter());
        let now = totals([Some(("a".to_string(), 1)), Some(("b".to_string(), 2))].iter());
        apply_delta(&mut list, &last, &now);
        assert_eq!(list, vec![("a".to_string(), 7), ("b".to_string(), 2)]);
    }
}
