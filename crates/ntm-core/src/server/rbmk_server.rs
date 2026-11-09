//! The RBMK in the world (`rbmk-plano` T01): each reactor is one coupled group, the component of the columns of a
//! layer (dimension and base Y) that its streams and its heat link; its step runs the pure `Reactor` over the columns'
//! block entities and the component's kept state (random and queued streams), all committed together or not at all.
//! The fuel rod put in by hand moves in one transfer with the player.
//!
//! Components (D1, D3, D4, docs/horizon-current.md "RBMK T01"):
//! - two columns are linked when one is a fuel channel and the other stands on a cell its streams can read
//!   (`Reactor::ray_cells`), or when they are cardinal neighbours (`moveHeat`); a column out of every reach is its own
//!   reactor, and a reactor never waits for another one;
//! - the layer remembers every column it registered (`Member`), so a column whose chunk is not loaded, or that the
//!   world shows and the server has not adopted yet, still belongs to its component and holds it (D4: unknown is never
//!   air); one the world shows as something else, or removed in the step, leaves;
//! - the logical order is the order of registration (D1); a component's state is kept under its first column;
//! - Java has one random and one stream list per world (`world.rand`, `NeutronNodeWorld.streamWorlds`), so there is no
//!   component to merge or split; with D3's random per simulation: components that merge keep the random of the one
//!   whose first column was registered first, their streams in the order of their origins' registration (the order of
//!   the Java list, whose streams come out in update order); a component that splits leaves its random to the part with
//!   its first surviving column, and each other part draws a seed from the parent's state and its own place, drawing
//!   nothing from the parent's random; each stream goes with its origin.
//!
//! **Provisional (approved D-b):** the minimal step needed to prove the transactional contract. Default dials and
//! height, no lids, ports, menus by context, radiation, meltdowns (logged once, not run) or the HBM opacity of blocks
//! (any block but air counts as opaque); `crate::rbmk::persist` lists which ticket takes each.
use super::*;
use crate::{
    machine::Data,
    rbmk::{
        dials::{DialState, Dials},
        persist::{self, CompState, Layer, Member, RbmkColumn},
        reactor::{Column, Kind as ColumnKind, Reactor},
    },
    server::container_bridge::Stack,
};

type Cell = [i32; 2];

/// The default dials, as a world that never set its rules reads them (T04 brings the rules).
fn default_dials() -> Dials {
    let mut d = DialState::default();
    d.create(false);
    d.refresh();
    *d.cached().expect("refreshed dials")
}

/// Air, by the block's id (not a suffix).
fn is_air(state: &str) -> bool {
    matches!(blockstate::id_of(state), "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
}

/// What a remembered column the step does not hold is now.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Absent {
    /// Its chunk is not loaded: it still belongs, and holds its component.
    Unknown,
    /// The world shows a column there the server has not adopted yet: the same.
    NotAdopted,
    /// The world shows something else, or the step removed it: it leaves.
    Gone,
}

/// A layer's columns as a graph: the ones the step holds (`present`, with their blocks) and the ones only the layer
/// remembers (classified once by `absent`).
struct Graph<'a, F: FnMut(Cell, &str) -> Absent> {
    present: &'a BTreeMap<Cell, String>,
    members: &'a BTreeMap<Cell, Member>,
    absent: F,
    seen: BTreeMap<Cell, Absent>,
    rod: Vec<Cell>,
    reasim: Vec<Cell>,
}

/// One component: its cells, and whether a remembered column of it is out of reach (it cannot step).
struct Component {
    cells: BTreeSet<Cell>,
    held: bool,
}

impl<F: FnMut(Cell, &str) -> Absent> Graph<'_, F> {
    /// The column at `c` and its kind, if there is one (present, or remembered and not gone).
    fn node(&mut self, c: Cell) -> Option<ColumnKind> {
        if let Some(b) = self.present.get(&c) {
            return persist::kind_of(b).map(|k| k.0);
        }
        let m = self.members.get(&c)?;
        let state = match self.seen.get(&c) {
            Some(s) => *s,
            None => {
                let s = (self.absent)(c, &m.block);
                self.seen.insert(c, s);
                s
            }
        };
        (state != Absent::Gone).then(|| persist::kind_of(&m.block).map(|k| k.0)).flatten()
    }
    /// The columns `c` (of `kind`) is linked to: its cardinal neighbours, the columns its streams can read, and the fuel
    /// channels whose streams can read it.
    fn neighbours(&mut self, c: Cell, kind: ColumnKind) -> Vec<Cell> {
        let mut out = Vec::new();
        let add = |o: &[i32; 2], sign: i32| [c[0] + sign * o[0], c[1] + sign * o[1]];
        for o in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
            let n = add(&o, 1);
            if self.node(n).is_some() {
                out.push(n);
            }
        }
        let own = match kind {
            ColumnKind::Rod => self.rod.clone(),
            ColumnKind::RodReaSim => self.reasim.clone(),
            _ => Vec::new(),
        };
        for o in &own {
            let n = add(o, 1);
            if self.node(n).is_some() {
                out.push(n);
            }
        }
        for (k, offsets) in [(ColumnKind::Rod, self.rod.clone()), (ColumnKind::RodReaSim, self.reasim.clone())] {
            for o in &offsets {
                let n = add(o, -1);
                if self.node(n) == Some(k) {
                    out.push(n);
                }
            }
        }
        out
    }
    /// The component of `start`.
    fn component(&mut self, start: Cell) -> Component {
        let mut cells = BTreeSet::from([start]);
        let mut todo = vec![start];
        while let Some(c) = todo.pop() {
            let Some(kind) = self.node(c) else { continue };
            for n in self.neighbours(c, kind) {
                if cells.insert(n) {
                    todo.push(n);
                }
            }
        }
        let held = cells.iter().any(|c| !self.present.contains_key(c) && self.seen.get(c).is_some_and(|s| *s != Absent::Gone));
        Component { cells, held }
    }
}

impl Server {
    /// The block entity columns of the layer of `k`, if it is one: (dimension, base Y).
    fn rbmk_layer_of(k: &Key, m: &Machine) -> Option<(String, i32)> {
        matches!(m.data, Data::Rbmk(_)).then(|| (k.0.clone(), k.1[1]))
    }

    /// What a remembered column is, as the world shows it now.
    fn rbmk_absent(host: &mut impl ServerHost, dim: &str, y: i32, c: Cell, removed: &BTreeSet<Key>) -> Absent {
        if removed.contains(&(dim.to_string(), [c[0], y, c[1]])) {
            return Absent::Gone;
        }
        match host.get_block(dim, [c[0], y, c[1]]) {
            None => Absent::Unknown,
            Some(b) if persist::kind_of(blockstate::id_of(&b)).is_some() => Absent::NotAdopted,
            Some(_) => Absent::Gone,
        }
    }

    /// The RBMK groups of a step: one per reactor, the machines of each component the layers' columns form. A
    /// component whose remembered columns are all out of reach has no group: it waits, its state kept.
    pub(super) fn rbmk_groups(&mut self, host: &mut impl ServerHost, outside: &BTreeSet<Key>) -> Vec<Vec<Key>> {
        let range = default_dials().flux_range;
        let (rod, reasim) = (persist::ray_offsets(ColumnKind::Rod, range), persist::ray_offsets(ColumnKind::RodReaSim, range));
        let mut layers: BTreeMap<(String, i32), BTreeMap<Cell, String>> = BTreeMap::new();
        for (k, m) in &self.ready.world.machines {
            if outside.contains(k) {
                continue;
            }
            if let Some(l) = Self::rbmk_layer_of(k, m) {
                layers.entry(l).or_default().insert([k.1[0], k.1[2]], m.block.clone());
            }
        }
        let mut groups = Vec::new();
        for ((dim, y), present) in layers {
            let empty = Layer::new();
            let members = self.ready.world.rbmk.get(&(dim.clone(), y)).unwrap_or(&empty);
            let none = BTreeSet::new();
            let mut g = Graph { present: &present, members: &members.members, absent: |c, _: &str| Self::rbmk_absent(host, &dim, y, c, &none), seen: BTreeMap::new(), rod: rod.clone(), reasim: reasim.clone() };
            let mut done = BTreeSet::new();
            for c in present.keys() {
                if done.contains(c) {
                    continue;
                }
                let comp = g.component(*c);
                groups.push(comp.cells.iter().filter(|c| present.contains_key(*c)).map(|c| (dim.clone(), [c[0], y, c[1]])).collect());
                done.extend(comp.cells);
            }
        }
        groups
    }

    /// One step of the reactors whose columns are the group's machines (`removed`: the machines the step removed).
    /// False when one cannot step now (a column of it out of reach or not adopted yet, or a cell its streams read
    /// unknown: D4 never takes an unknown cell for air): the group's step is refused and nothing advances.
    pub(super) fn rbmk_tick(&mut self, host: &mut impl ServerHost, removed: &BTreeSet<Key>) -> bool {
        let mut present: BTreeMap<Cell, String> = BTreeMap::new();
        let mut at = None;
        for (k, m) in &self.ready.world.machines {
            if matches!(m.data, Data::Rbmk(_)) {
                present.insert([k.1[0], k.1[2]], m.block.clone());
                at = Some((k.0.clone(), k.1[1]));
            }
        }
        let Some((dim, y)) = at else { return true };
        let dials = default_dials();
        let range = dials.flux_range;
        let mut layer = self.ready.world.rbmk.get(&(dim.clone(), y)).cloned().unwrap_or_default();
        // A column the step removed leaves (P1-1: one replaced in place by another is a new registration once adopted);
        // a column registered as another block is registered anew.
        layer.members.retain(|c, m| !removed.contains(&(dim.clone(), [c[0], y, c[1]])) && present.get(c).is_none_or(|b| *b == m.block));
        for (c, block) in &present {
            if !layer.members.contains_key(c) {
                let seq = layer.next_seq;
                layer.next_seq += 1;
                layer.members.insert(*c, Member { seq, block: block.clone(), comp: *c });
            }
        }
        let (rod, reasim) = (persist::ray_offsets(ColumnKind::Rod, range), persist::ray_offsets(ColumnKind::RodReaSim, range));
        let (mine, siblings, gone) = {
            let mut g = Graph { present: &present, members: &layer.members, absent: |c, _: &str| Self::rbmk_absent(host, &dim, y, c, removed), seen: BTreeMap::new(), rod: rod.clone(), reasim: reasim.clone() };
            let mut mine: Vec<Component> = Vec::new();
            for c in present.keys() {
                if !mine.iter().any(|m| m.cells.contains(c)) {
                    mine.push(g.component(*c));
                }
            }
            if mine.iter().any(|m| m.held) {
                return false;
            }
            // The other parts of the components these ones came from (a split): their states are settled here too.
            let parents: BTreeSet<Cell> = mine.iter().flat_map(|m| &m.cells).map(|c| layer.members[c].comp).collect();
            let mut siblings: Vec<Component> = Vec::new();
            let others: Vec<Cell> = layer.members.iter().filter(|(c, m)| parents.contains(&m.comp) && !mine.iter().any(|x| x.cells.contains(*c))).map(|(c, _)| *c).collect();
            for c in others {
                if g.node(c).is_some() && !siblings.iter().any(|s| s.cells.contains(&c)) {
                    siblings.push(g.component(c));
                }
            }
            let gone: Vec<Cell> = g.seen.iter().filter(|(_, s)| **s == Absent::Gone).map(|(c, _)| *c).collect();
            (mine, siblings, gone)
        };
        for c in gone {
            layer.members.remove(&c);
        }
        let seq = |c: &Cell| layer.members[c].seq;
        let orders: Vec<Vec<Cell>> = mine.iter().chain(&siblings).map(|m| {
            let mut o: Vec<Cell> = m.cells.iter().copied().collect();
            o.sort_by_key(|c| seq(c));
            o
        }).collect();
        // Each parent's heir: the part with its first surviving column.
        let mut heir: BTreeMap<Cell, (u64, usize)> = BTreeMap::new();
        for (i, o) in orders.iter().enumerate() {
            for c in o {
                let m = &layer.members[c];
                if layer.comps.contains_key(&m.comp) && heir.get(&m.comp).is_none_or(|h| m.seq < h.0) {
                    heir.insert(m.comp, (m.seq, i));
                }
            }
        }
        let mut states: Vec<CompState> = Vec::new();
        for (i, o) in orders.iter().enumerate() {
            let key = o[0];
            let parents: Vec<Cell> = {
                let mut p: Vec<Cell> = o.iter().map(|c| layer.members[c].comp).filter(|p| layer.comps.contains_key(p)).collect();
                p.sort_by_key(|p| heir[p].0);
                p.dedup();
                p
            };
            let mut state = match parents.iter().find(|p| heir[*p].1 == i) {
                Some(p) => layer.comps[p].clone(),
                None => match parents.first() {
                    Some(p) => CompState::derived(&layer.comps[p], key),
                    None => CompState::fresh(&dim, y, key),
                },
            };
            let mut streams: Vec<persist::KeptStream> = parents.iter().flat_map(|p| layer.comps[p].streams.iter().filter(|s| o.contains(&s.origin)).cloned()).collect();
            streams.sort_by_key(|s| seq(&s.origin));
            state.streams = streams;
            states.push(state);
        }
        // Step each reactor of the group.
        let height = dials.column_height + 1;
        for (i, order) in orders.iter().enumerate().take(mine.len()) {
            let mut r = Reactor::new(dials, states[i].rng(), y);
            r.time = self.ready.world.ticks as i64;
            for p in order {
                let m = &self.ready.world.machines[&(dim.clone(), [p[0], y, p[1]])];
                let Data::Rbmk(c) = &m.data else { unreachable!() };
                let (kind, moderated) = persist::kind_of(&m.block).expect("an RBMK column");
                let mut col = Column::new(kind, p[0], p[1]);
                // Java places a column without its lid (`RBMKBase.getDirModified`: DIR_NO_LID); lids are T02's.
                col.lid = false;
                col.moderated = moderated;
                col.heat = c.heat;
                col.reasim_water = c.reasim_water;
                col.reasim_steam = c.reasim_steam;
                if let Some(rod) = col.rod_mut() {
                    rod.flux_quantity = c.flux_quantity;
                    rod.flux_fast_ratio = c.flux_fast_ratio;
                    rod.last_flux_quantity = c.last_flux_quantity;
                    rod.last_flux_ratio = c.last_flux_ratio;
                    rod.has_rod = c.has_rod;
                    rod.fuel = c.slots.first().cloned().flatten().and_then(|s| persist::rod_of(&s.id).map(|t| (t, persist::rod_stack(s.custom.as_deref()))));
                }
                r.add(col);
            }
            r.set_queued(states[i].queue(order));
            // Test hooks: a rod asked out leaves its channel as an item entity, in this step's own commit.
            for (j, p) in order.iter().enumerate() {
                let key = (dim.clone(), [p[0], y, p[1]]);
                let Some(at) = self.ready.rbmk_ejects.remove(&key) else { continue };
                let Some(rod) = r.columns[j].rod_mut() else { continue };
                let Some((_, tag)) = rod.fuel.take() else { continue };
                let m = &self.ready.world.machines[&key];
                let Data::Rbmk(c) = &m.data else { unreachable!() };
                let Some(Some(stack)) = c.slots.first().cloned() else { continue };
                let stack = persist::rod_in_slot(&stack, &tag);
                let components = stack.custom.as_ref().map_or(String::new(), |c| format!(",components:{{\"minecraft:custom_data\":{c}}}"));
                let nbt = format!("{{Item:{{id:\"{}\",count:{}{components}}},PickupDelay:10s}}", stack.id, stack.count);
                let cell = at.map(|v| v.floor() as i32);
                if !host.spawn_entities(&dim, cell, &[crate::server::EntitySpawn { kind: "minecraft:item".into(), pos: at, nbt }]) {
                    return false;
                }
                if let Some(Data::Rbmk(c)) = self.ready.world.machines.get_mut(&key).map(|m| &mut m.data) {
                    c.slots[0] = None;
                }
            }
            // Provisional occlusion (T04): every block but air, in the cells the streams of this reactor can read.
            let mut cells = BTreeSet::new();
            for p in order {
                let offsets = match persist::kind_of(&self.ready.world.machines[&(dim.clone(), [p[0], y, p[1]])].block).map(|k| k.0) {
                    Some(ColumnKind::Rod) => &rod,
                    Some(ColumnKind::RodReaSim) => &reasim,
                    _ => continue,
                };
                cells.extend(offsets.iter().map(|o| [p[0] + o[0], p[1] + o[1]]).filter(|c| !order.contains(c)));
            }
            for c in cells {
                for h in 0..height {
                    let Some(b) = host.get_block(&dim, [c[0], y + h, c[1]]) else { return false };
                    if !is_air(&b) {
                        r.obstacles.insert((c[0], c[1], h));
                    }
                }
            }
            r.step();
            let melting: BTreeSet<usize> = r.meltdowns.iter().copied().collect();
            for (j, p) in order.iter().enumerate() {
                let key = (dim.clone(), [p[0], y, p[1]]);
                if melting.contains(&j) {
                    if self.ready.rbmk_melting.insert(key) {
                        host.log(&format!("rbmk meltdown due at {dim} [{}, {y}, {}]: not run before T16", p[0], p[1]));
                    }
                } else {
                    self.ready.rbmk_melting.remove(&key);
                }
            }
            for (j, p) in order.iter().enumerate() {
                let col = &r.columns[j];
                let m = self.ready.world.machines.get_mut(&(dim.clone(), [p[0], y, p[1]])).unwrap();
                let Data::Rbmk(c) = &mut m.data else { unreachable!() };
                c.heat = col.heat;
                c.reasim_water = col.reasim_water;
                c.reasim_steam = col.reasim_steam;
                if let Some(rod) = col.rod() {
                    c.flux_quantity = rod.flux_quantity;
                    c.flux_fast_ratio = rod.flux_fast_ratio;
                    c.last_flux_quantity = rod.last_flux_quantity;
                    c.last_flux_ratio = rod.last_flux_ratio;
                    c.has_rod = rod.has_rod;
                    // The rod's tag goes back into the very stack in the slot: the rest of its data is kept.
                    if let (Some((_, stack)), Some(Some(slot))) = (&rod.fuel, c.slots.first_mut()) {
                        *slot = persist::rod_in_slot(slot, stack);
                    }
                }
            }
            states[i].set_rng(&r.rng);
            states[i].set_queue(order, &r.queued());
        }
        // The states settle: the parents' entries give way to the parts', every column under its part's first one.
        let parents: BTreeSet<Cell> = orders.iter().flatten().map(|c| layer.members[c].comp).collect();
        for p in parents {
            layer.comps.remove(&p);
        }
        for (o, state) in orders.iter().zip(states) {
            for c in o {
                layer.members.get_mut(c).unwrap().comp = o[0];
            }
            layer.comps.insert(o[0], state);
        }
        layer.comps.retain(|k, _| layer.members.values().any(|m| m.comp == *k));
        if self.ready.world.rbmk.get(&(dim.clone(), y)) != Some(&layer) {
            self.ready.world.rbmk.insert((dim, y), layer);
        }
        true
    }

    /// Test hook (`NTM_TEST_HOOKS=1`): the rod of the fuel channel at `pos` leaves as an item entity at `at` through a
    /// `WorldEffect::Entities` of its reactor's next accepted step (the entity receipt path a debris will take, T20).
    pub fn native_test_eject(&mut self, dim: &str, pos: Pos, at: [f64; 3]) -> bool {
        let key = (dim.to_string(), pos);
        let full = self.ready.world.machines.get(&key).is_some_and(|m| matches!(&m.data, Data::Rbmk(c) if c.slots.first().is_some_and(Option::is_some)));
        if full && at.iter().all(|v| v.is_finite()) {
            self.ready.rbmk_ejects.insert(key, at);
        }
        full
    }

    /// Test hook (`NTM_TEST_HOOKS=1`): the rod of the fuel channel at `pos` goes to the acting player's cursor (`cursor`,
    /// which must be empty) or out of their hand as a thrown drop, in one transfer with the channel.
    pub fn native_test_take(&mut self, host: &mut impl ServerHost, dim: &str, pos: Pos, player: &str, cursor: bool) -> bool {
        let key = (dim.to_string(), pos);
        let Some(m) = self.ready.world.machines.get(&key).cloned() else { return false };
        if self.ready.broken.contains_key(&key) || !self.model_current(&key, &m) || !matches!(m.data, Data::Rbmk(_)) {
            return false;
        }
        let Some(hand) = host.player_slots(player) else { return false };
        let Some(expected) = host.machine_slots(dim, pos) else { return false };
        let Some(Some(rod)) = expected.first().cloned() else { return false };
        let mut machine = m.clone();
        let Data::Rbmk(col) = &mut machine.data else { unreachable!() };
        col.slots = vec![None];
        let (desired, data) = Self::machine_output(&mut machine, &expected);
        let json = serde_json::to_string(&machine).expect("machine serialization");
        let commit = MachineCommit { cells: Vec::new(), guard: CellGuard::machine(&key, &m), expected_saved: self.ready.native_saved.contains_key(&key), dim: dim.into(), pos, expected, desired, data, json };
        let (edits, drops) = if cursor {
            if hand.carried.is_some() {
                return false;
            }
            (vec![SlotEdit { slot: PlayerSlot::Carried, expected: None, desired: Some(rod) }], Vec::new())
        } else {
            (Vec::new(), vec![(rod, TransferDrop::Thrown)])
        };
        let transfer = PlayerTransfer { player: player.into(), edits, drops, copies: Vec::new() };
        if !host.commit_player_transfer(&transfer, std::slice::from_ref(&commit)) {
            return false;
        }
        self.ready.world.machines.insert(key.clone(), machine);
        self.ready.bridge.last.insert(key.clone(), commit.desired);
        self.ready.bridge.data.insert(key.clone(), commit.data);
        self.ready.native_saved.insert(key, commit.json);
        true
    }

    /// A player's use of an RBMK column (`core` its base): a fuel rod in hand goes into an empty fuel channel, one of
    /// the stack with all its data (`copy()`), the hand debited in survival only, in one transfer
    /// (`RBMKRod.onBlockActivated`). True when the use was the insertion (or was refused as a whole); false lets the
    /// click go on (the menu: T05).
    ///
    /// Adaptations (P3-3): the hand may be the off hand, which 1.7.10 does not have (the engine's acting hand); and the
    /// insertion consumes the click where the Java returns `false` after it, a fall-through to `ItemRBMKRod`, which has
    /// no use on a block, so nothing more would happen.
    pub fn native_rbmk_use(&mut self, host: &mut impl ServerHost, dim: &str, at: Pos, core: Pos, player: &str) -> bool {
        let key = (dim.to_string(), core);
        let Some(m) = self.ready.world.machines.get(&key).cloned() else { return false };
        let Data::Rbmk(c) = &m.data else { return false };
        if c.slots.is_empty() || self.ready.broken.contains_key(&key) || !self.model_current(&key, &m) {
            return false;
        }
        let Some(hand) = host.player_slots(player) else { return false };
        let Some(held) = hand.held().filter(|h| persist::rod_of(&h.stack.id).is_some()).cloned() else { return false };
        let Some(expected) = host.machine_slots(dim, core) else { return true };
        if expected.len() != 1 || expected[0].is_some() {
            return false;
        }
        let from = hand.hand.expect("a held stack has a hand");
        let edits = if hand.creative { Vec::new() } else { hand.take(from, 1).into_iter().collect() };
        let mut machine = m.clone();
        let Data::Rbmk(col) = &mut machine.data else { unreachable!() };
        **col = RbmkColumn { slots: vec![Some(Stack { count: 1, ..held.stack.clone() })], ..(**col).clone() };
        let (desired, data) = Self::machine_output(&mut machine, &expected);
        let json = serde_json::to_string(&machine).expect("machine serialization");
        let commit = MachineCommit { cells: Vec::new(), guard: CellGuard::machine(&key, &m), expected_saved: self.ready.native_saved.contains_key(&key), dim: dim.into(), pos: core, expected, desired, data, json };
        let copies = vec![SlotCopy { dim: dim.into(), pos: core, slot: 0, from }];
        let transfer = PlayerTransfer { player: player.into(), edits, drops: Vec::new(), copies };
        if !host.commit_player_transfer(&transfer, std::slice::from_ref(&commit)) {
            return true;
        }
        self.ready.world.machines.insert(key.clone(), machine);
        self.ready.bridge.last.insert(key.clone(), commit.desired);
        self.ready.bridge.data.insert(key.clone(), commit.data);
        self.ready.native_saved.insert(key, commit.json);
        host.show(dim, &crate::blast::visuals::Show::Sound { id: "ntm:item.upgradeplug", source: "block", at: at.map(|v| f64::from(v) + 0.5), volume: 1., pitch: 1. });
        true
    }
}
