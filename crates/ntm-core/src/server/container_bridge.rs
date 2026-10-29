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

/// A slot's stack as the engine reports it: the item, how many and its
/// `minecraft:damage` (a blade's wear).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Stack {
    pub id: String,
    pub count: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub damage: u32,
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

impl Stack {
    pub fn new(id: &str, count: u32) -> Self {
        Self {
            id: id.into(),
            count,
            damage: 0,
        }
    }
}

/// The engine's slots of a machine.
pub type Slots = Vec<Option<Stack>>;

/// A machine that keeps its items as the engine's slots (new machines: no
/// transactions, nothing to reconcile). Before the machines run its slots
/// become the engine's; after, the slots it changed are written back.
pub trait Slotted {
    /// Its slots, in the order of its `horizon_machine/<block>.json`.
    fn slots_mut(&mut self) -> &mut Slots;
    /// The values its screen shows (`horizon_screen/<block>.json`).
    fn screen_data(&self) -> Vec<i32>;
    /// A button of its screen; true when it opens the machine's old
    /// screen (a list, as the crucible's alloy selector).
    fn button(&mut self, _id: u32) -> bool {
        false
    }
    /// What it holds outside its slots (molten metal as scraps), dropped
    /// when it is broken: the engine spills the slots, not this.
    fn held_contents(&self) -> Vec<(String, u32)> {
        Vec::new()
    }
    /// Its state outside the slots, saved with them in the engine's block
    /// entity (after the screen's values): the world's save then has the
    /// slots and the state from the same moment, so a crash between the
    /// world's save and the mod's can't have an item both in a slot and
    /// melted (or in neither).
    fn save_state(&self) -> Vec<i32> {
        Vec::new()
    }
    /// The state back from the block entity, at the first sight of it in a
    /// session.
    fn load_state(&mut self, _v: &[i32]) {}
}

/// What the machine in `data` holds outside the engine's slots.
pub(super) fn held_contents(data: &Data) -> Vec<(String, u32)> {
    let mut d = data.clone();
    slotted(&mut d).map_or_else(Vec::new, |n| n.held_contents())
}

/// The machines that implement [`Slotted`] (`Layout::Native`).
fn slotted(data: &mut Data) -> Option<&mut dyn Slotted> {
    match data {
        Data::Shredder(s) => Some(s),
        Data::BlastFurnace(b) => Some(b),
        Data::Firebox(f) => Some(f),
        Data::Crucible(c) => Some(c),
        Data::Mold(c) | Data::Basin(c) => Some(c),
        Data::Channel(c) => Some(c),
        Data::FoundryTank(c) => Some(c),
        Data::Outlet(c) => Some(c),
        Data::Anvil(a) => Some(a),
        _ => None,
    }
}

/// The blocks whose machines use the engine's slots, and their layout:
/// (block, first input slot, inputs, output slot).
pub const SLOTTED: &[(&str, Layout)] = &[
    ("ntm:machine_shredder", Layout::Native),
    ("ntm:machine_blast_furnace", Layout::Native),
    ("ntm:heater_firebox", Layout::Native),
    ("ntm:machine_crucible", Layout::Native),
    ("ntm:foundry_mold", Layout::Native),
    ("ntm:foundry_basin", Layout::Native),
    ("ntm:foundry_channel", Layout::Native),
    ("ntm:foundry_tank", Layout::Native),
    ("ntm:foundry_outlet", Layout::Native),
    ("ntm:anvil_iron", Layout::Native),
    ("ntm:anvil_lead", Layout::Native),
    ("ntm:anvil_steel", Layout::Native),
    ("ntm:anvil_desh", Layout::Native),
    ("ntm:anvil_ferrouranium", Layout::Native),
    ("ntm:anvil_saturnite", Layout::Native),
    ("ntm:anvil_bismuth_bronze", Layout::Native),
    ("ntm:anvil_arsenic_bronze", Layout::Native),
    ("ntm:anvil_schrabidate", Layout::Native),
    ("ntm:anvil_dnt", Layout::Native),
    ("ntm:anvil_osmiridium", Layout::Native),
    ("ntm:anvil_murky", Layout::Native),
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
    /// A machine that keeps its items as slots ([`Slotted`]).
    Native,
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
    let mut out = Vec::new();
    for z in min[2]..=max[2] {
        for x in min[0]..=max[0] {
            if (x, z) != (0, 0) {
                out.push(world_cell(core, f, [x, min[1], z]));
            }
        }
    }
    out
}

/// Multiblocks' fluid proxies (HBM's `getAllPorts` and `makeExtra`): the
/// cells, in the placer's frame (x right, y up, z away from the player),
/// that join the core's fluid networks as ducts. A chain of them reaches the
/// core: the blast furnace's ports up its front reach it down its column.
pub const FLUID_PROXIES: &[(&str, &[[i32; 3]])] = &[(
    "ntm:machine_blast_furnace",
    &[
        [1, 0, 0],
        [-1, 0, 0],
        [0, 0, 1],
        [0, 0, -1],
        [0, 1, 0],
        [0, 2, 0],
        [0, 3, 0],
        [0, 4, 0],
        [0, 5, 0],
        [0, 6, 0],
        [0, 3, 1],
        [0, 3, -1],
        [0, 5, 1],
        [0, 5, -1],
    ],
)];

/// Cell `l` (placer's frame) of the multiblock with its core at `core`.
fn world_cell(core: Pos, facing: [i32; 3], l: [i32; 3]) -> Pos {
    let fwd = [-facing[0], -facing[2]];
    let right = [-fwd[1], fwd[0]];
    [
        core[0] + right[0] * l[0] + fwd[0] * l[2],
        core[1] + l[1],
        core[2] + right[1] * l[0] + fwd[1] * l[2],
    ]
}

/// The cells that carry the fluids of the multiblock of `block` at `core`.
pub fn fluid_proxy_cells(block: &str, core: Pos, facing: Option<&str>) -> Vec<Pos> {
    let Some((_, cells)) = FLUID_PROXIES.iter().find(|(b, _)| *b == block) else {
        return Vec::new();
    };
    let Some(f) = facing.and_then(crate::registry::facing_dir) else {
        return Vec::new();
    };
    cells.iter().map(|&l| world_cell(core, f, l)).collect()
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

fn totals<'a>(slots: impl Iterator<Item = &'a Option<Stack>>) -> BTreeMap<String, i64> {
    let mut out = BTreeMap::new();
    for s in slots.flatten() {
        *out.entry(s.id.clone()).or_insert(0) += i64::from(s.count);
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
        if let Some(stack) = &slots[s] {
            let have = left.iter_mut().find(|(i, _)| *i == stack.id).map_or(0, |e| {
                let t = e.1.min(max);
                e.1 -= t;
                t
            });
            slots[s] = (have > 0).then(|| Stack {
                count: have,
                ..stack.clone()
            });
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
        slots[s] = Some(Stack::new(&e.0, t));
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
                        c.out.retain(|(i, _)| slot.as_ref().is_some_and(|s| s.id != *i));
                        if let Some(s) = slot {
                            c.out.push((s.id, s.count));
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
                            *s = now.get(i).cloned().flatten().map(|s| s.id);
                        }
                    }
                }
                (data, Layout::Native) => {
                    let Some(n) = slotted(data) else { continue };
                    if first
                        && let Some(saved) = host.machine_data(&key.0, key.1)
                    {
                        // The world's save: its state with its slots.
                        let at = n.screen_data().len();
                        if saved.len() > at {
                            n.load_state(&saved[at..]);
                        }
                    }
                    *n.slots_mut() = now.clone();
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
            let Some(m) = r.world.machines.get_mut(&key) else {
                r.bridge.last.remove(&key);
                r.bridge.data.remove(&key);
                continue;
            };
            let Some(layout) = layout(&m.block) else { continue };
            let Some(last) = r.bridge.last.get(&key).cloned() else { continue };
            let mut want = last.clone();
            let data = match (&mut m.data, layout) {
                (Data::Crafter(c), Layout::Crafter { inputs, output }) => {
                    lay_out(&mut want, inputs.clone(), &c.items, crate::crafter::STACK);
                    // The output slot shows the first of the outputs; others
                    // stay in the machine (its old screen gives them).
                    let keep = want[*output].as_ref().map(|s| s.id.clone());
                    let shown = c
                        .out
                        .iter()
                        .find(|(i, _)| Some(i) == keep.as_ref())
                        .or_else(|| c.out.first())
                        .cloned();
                    let keep_damage = want[*output].as_ref().map_or(0, |s| s.damage);
                    want[*output] = shown.map(|(i, n)| Stack {
                        damage: if Some(&i) == keep.as_ref() { keep_damage } else { 0 },
                        id: i,
                        count: n.min(crate::crafter::STACK),
                    });
                    crafter_data(c)
                }
                (Data::Bomb(b), Layout::Bomb) => {
                    for (i, s) in b.slots.iter().enumerate() {
                        if let Some(cell) = want.get_mut(i) {
                            // A part keeps the damage its slot gave it.
                            let damage = cell.as_ref().filter(|c| Some(&c.id) == s.as_ref()).map_or(0, |c| c.damage);
                            *cell = s.clone().map(|id| Stack { id, count: 1, damage });
                        }
                    }
                    vec![b.blast().is_some() as i32]
                }
                (data, Layout::Native) => {
                    let Some(n) = slotted(data) else { continue };
                    want = n.slots_mut().clone();
                    let mut v = n.screen_data();
                    v.extend(n.save_state());
                    v
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

    /// Whether the engine opens its own screen of the machine at `key`
    /// (the bridge has seen its slots): the use button then opens no other.
    pub(super) fn slots_open(&mut self, key: &Key) -> bool {
        self.ready().is_some_and(|r| r.bridge.last.contains_key(key))
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
        if !self.within_reach(key) {
            return;
        }
        let Some(r) = self.ready() else { return };
        let Some(m) = r.world.machines.get_mut(key) else { return };
        if let Some(n) = slotted(&mut m.data) {
            let open = n.button(button);
            r.dirty = true;
            if open {
                r.viewer = Some((key.clone(), None));
                self.mail.send(S2C::Show {
                    dim: key.0.clone(),
                    pos: key.1,
                });
                self.push_view(true);
            }
            return;
        }
        if let Data::Crafter(c) = &mut m.data
            && button == 0
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
    parts.iter().map(|p| p.map(|id| Stack::new(id, 1))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_layout_keeps_slots_and_fills_holes() {
        let a = |n| Some(Stack::new("a", n));
        let mut slots: Slots = vec![None, Some(Stack { damage: 7, ..Stack::new("a", 5) }), None];
        lay_out(&mut slots, 0..3, &[("a".into(), 3), ("b".into(), 2)], 64);
        assert_eq!(
            slots,
            vec![Some(Stack::new("b", 2)), Some(Stack { damage: 7, ..Stack::new("a", 3) }), None]
        );
        let mut slots: Slots = vec![a(5), None];
        lay_out(&mut slots, 0..2, &[("a".into(), 100)], 64);
        assert_eq!(slots, vec![a(64), a(36)]);
    }

    #[test]
    fn proxies_turn_with_the_facing() {
        // Facing north, the placer looked south: their right is west.
        assert_eq!(world_cell([0, 64, 0], [0, 0, -1], [1, 0, 0]), [-1, 64, 0]);
        assert_eq!(world_cell([0, 64, 0], [0, 0, -1], [0, 3, 1]), [0, 67, 1]);
        let cells = fluid_proxy_cells("ntm:machine_blast_furnace", [0, 64, 0], Some("east"));
        assert_eq!(cells.len(), 14);
        assert!(cells.contains(&[0, 70, 0]));
        assert!(proxy_cells("ntm:machine_assembly_machine", [0, 64, 0], Some("north")).len() == 8);
    }

    #[test]
    fn a_delta_moves_only_what_players_moved() {
        let mut list = vec![("a".to_string(), 10)];
        let last = totals([Some(Stack::new("a", 4))].iter());
        let now = totals([Some(Stack::new("a", 1)), Some(Stack::new("b", 2))].iter());
        apply_delta(&mut list, &last, &now);
        assert_eq!(list, vec![("a".to_string(), 7), ("b".to_string(), 2)]);
    }
}
