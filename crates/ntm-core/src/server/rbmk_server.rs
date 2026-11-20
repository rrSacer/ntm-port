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
//! - Java has one random and one stream list per world (`world.rand`, `StreamWorld.streams`), so there is no
//!   component to merge or split; with D3's random per simulation: components that merge keep the random of the one
//!   whose first column was registered first, their streams in the order of their origins' registration (the order of
//!   the Java list, whose streams come out in update order); a component that splits leaves its random to the part with
//!   its first surviving column, and each other part draws a seed from the parent's state and its own place, drawing
//!   nothing from the parent's random; each stream goes with its origin.
//!
//! T04 (docs/horizon-current.md "RBMK T04"): all eighteen columns step (`persist::kind_of`); the dials are each
//! dimension's cache over the save's rules, read as the first column reads them (`rbmk_columns::first_dial`, 3A);
//! occlusion is Java's `isOpaqueCube` (`rbmk_opaque`); the step's `incrementRad` goes to the world's radiation by
//! chunk and its `SatelliteRayScan` events to the dimension's kept list (`persist::report_ray`, 4A), both with the
//! step's commit. Still out of the step: ports and fluids (T08), the outgasser's recipes (T09),
//! meltdowns (logged once, not run: T16).
use super::*;
use crate::{
    machine::Data,
    rbmk::{
        fluids::{self, Tank},
        outgasser,
        persist::{self, Layer, Member, RbmkColumn},
        reactor::{AutoFunction, Column, Data as ReactorData, Effect, Kind as ColumnKind, Reactor},
    },
    server::container_bridge::Stack,
};

type Cell = [i32; 2];

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
        // A cooler cools the columns of its 5×5 (`neighborCache`, T08), linked or not, and they are its.
        for dx in -2..=2 {
            for dz in -2..=2 {
                let n = [c[0] + dx, c[1] + dz];
                if n != c && self.node(n).is_some_and(|k| kind == ColumnKind::Cooler || k == ColumnKind::Cooler) {
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
    /// `block.isOpaqueCube()` of the block at `p`, as `RBMKNeutronStream.getHits` asks it (T04): an RBMK-family block
    /// by its Java override (`persist::java_opaque`); a column's dummy as its column's block, which it is in Java (the
    /// core under its run of dummies; none found: `RBMKBase`'s true); any other block by the engine's facts. None: a
    /// cell not loaded (D4: never air).
    fn rbmk_opaque(host: &mut impl ServerHost, dim: &str, p: Pos) -> Option<bool> {
        let (b, fact) = host.block_opaque(dim, p)?;
        let id = blockstate::id_of(&b);
        if id == persist::DUMMY {
            for dy in 1..=persist::DUMMY_REACH {
                let below = host.get_block(dim, [p[0], p[1] - dy, p[2]])?;
                match blockstate::id_of(&below) {
                    persist::DUMMY => continue,
                    core => return Some(persist::java_opaque(core).filter(|_| persist::is_column(core)).unwrap_or(true)),
                }
            }
            return Some(true);
        }
        Some(persist::java_opaque(id).unwrap_or(fact))
    }

    /// The block entity columns of the layer of `k`, if it is one: (dimension, base Y).
    fn rbmk_layer_of(k: &Key, m: &Machine) -> Option<(String, i32)> {
        (matches!(m.data, Data::Rbmk(_)) && persist::kind_of(&m.block).is_some()).then(|| (k.0.clone(), k.1[1]))
    }

    /// What a remembered column is, as the world shows it now (a column the step removed has left the layer before:
    /// `rbmk_tick`).
    fn rbmk_absent(host: &mut impl ServerHost, dim: &str, y: i32, c: Cell) -> Absent {
        match host.get_block(dim, [c[0], y, c[1]]) {
            None => Absent::Unknown,
            Some(b) if persist::kind_of(blockstate::id_of(&b)).is_some() => Absent::NotAdopted,
            Some(_) => Absent::Gone,
        }
    }

    /// The RBMK groups of a step: one per reactor, the machines of each component the layers' columns form. A
    /// component whose remembered columns are all out of reach has no group: it waits, its state kept.
    pub(super) fn rbmk_groups(&mut self, host: &mut impl ServerHost, outside: &BTreeSet<Key>) -> Vec<Vec<Key>> {
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
            // The range the dimension's step will read (no dial's first read writes the range back).
            let range = self.rbmk_dials_peek(&dim, "KEY_REASIM_BOILERS").flux_range;
            let (rod, reasim) = (persist::ray_offsets(ColumnKind::Rod, range), persist::ray_offsets(ColumnKind::RodReaSim, range));
            let empty = Layer::new();
            let members = self.ready.world.rbmk.get(&(dim.clone(), y)).unwrap_or(&empty);
            let mut g = Graph { present: &present, members: &members.members, absent: |c, _: &str| Self::rbmk_absent(host, &dim, y, c), seen: BTreeMap::new(), rod: rod.clone(), reasim: reasim.clone() };
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
        // The dimension's dials (3A), read as the group's first column in the logical order reads them first.
        let first = present.keys().min_by_key(|c| layer.members[*c].seq).copied().expect("a column");
        let first_machine = &self.ready.world.machines[&(dim.clone(), [first[0], y, first[1]])];
        let fuelled = matches!(&first_machine.data, Data::Rbmk(c) if c.slots.first().and_then(|s| s.as_ref()).is_some_and(|s| persist::rod_of(&s.id).is_some()));
        let dials = self.rbmk_dials(&dim, super::rbmk_columns::first_dial(&first_machine.block.clone(), fuelled));
        let range = dials.flux_range;
        let (rod, reasim) = (persist::ray_offsets(ColumnKind::Rod, range), persist::ray_offsets(ColumnKind::RodReaSim, range));
        let (mine, siblings, gone) = {
            let mut g = Graph { present: &present, members: &layer.members, absent: |c, _: &str| Self::rbmk_absent(host, &dim, y, c), seen: BTreeMap::new(), rod: rod.clone(), reasim: reasim.clone() };
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
        let (orders, mut states) = persist::settle(&layer, mine.iter().chain(&siblings).map(|m| &m.cells), &dim, y);
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
                // Placed without one (`getDirModified`: DIR_NO_LID); a lid put on since (T02), or a control rod's own.
                col.lid = c.has_lid(&m.block);
                col.moderated = moderated;
                col.heat = self.ready.rbmk_heats.remove(&(dim.clone(), [p[0], y, p[1]])).unwrap_or(c.heat);
                col.reasim_water = c.reasim_water;
                col.reasim_steam = c.reasim_steam;
                if let Some(rod) = col.rod_mut() {
                    rod.flux_quantity = c.flux_quantity;
                    rod.flux_fast_ratio = c.flux_fast_ratio;
                    rod.last_flux_quantity = c.last_flux_quantity;
                    rod.last_flux_ratio = c.last_flux_ratio;
                    rod.has_rod = c.has_rod;
                    rod.fuel = c.slots.first().cloned().flatten().and_then(|s| persist::rod_of(&s.id).map(|t| (t, persist::stack_rod(&s))));
                }
                // A control rod as its TE (T07): where it is and goes, the manual rods' start, the ReaSim rods' power
                // (in memory), the automatic rods' settings (T06). A save from before reads as the fresh TE.
                if let Some(cc) = col.control_mut() {
                    let st = c.control.clone().unwrap_or_else(|| persist::ControlState::of(&m.block));
                    (cc.level, cc.target_level, cc.starting_level, cc.power, cc.has_power) = (st.level, st.target_level, st.starting_level, st.power, st.has_power);
                    if let Some(a) = &c.auto {
                        (cc.level_lower, cc.level_upper, cc.heat_lower, cc.heat_upper) = (a.level_lower, a.level_upper, a.heat_lower, a.heat_upper);
                        cc.function = Some(match a.function {
                            1 => AutoFunction::QuadUp,
                            2 => AutoFunction::QuadDown,
                            _ => AutoFunction::Linear,
                        });
                    }
                }
                // A boiler, heater or cooler as its TE (T08): its tanks, and what the Java keeps in memory only. A save
                // from before has none and reads as the fresh TE (`Column::new`'s).
                if let Some(st) = &c.piped {
                    let tank = |k: &persist::KeptTank, max: i32| Tank { fluid: fluids::java_fluid(&k.fluid), fill: k.fill, max };
                    match &mut col.data {
                        ReactorData::Boiler(b) => {
                            (b.feed, b.steam) = (tank(&st.input, b.feed.max), tank(&st.output, b.steam.max));
                            (b.vent_delay, b.consumption, b.output) = (st.vent_delay, st.consumption, st.produced);
                        }
                        ReactorData::Heater(h) => (h.feed, h.steam) = (tank(&st.input, h.feed.max), tank(&st.output, h.steam.max)),
                        ReactorData::Cooler(k) => {
                            (k.cold, k.hot) = (tank(&st.input, k.cold.max), tank(&st.output, k.hot.max));
                            k.timer = st.timer;
                        }
                        _ => {}
                    }
                }
                // An outgasser as its TE (T09): its progress and gas, and its two slots in the recipes' terms (the Java
                // item of a port item a recipe names; an input whose recipe makes an item the port lacks has none).
                if let ReactorData::Outgasser(o) = &mut col.data {
                    let st = c.outgasser.clone().unwrap_or_else(|| persist::OutgasserState::of(&m.block).expect("an outgasser"));
                    (o.progress, o.gas) = (st.progress, Tank { fluid: fluids::java_fluid(&st.gas.fluid), fill: st.gas.fill, max: o.gas.max });
                    let as_recipe = |s: &Stack, input: bool| {
                        let (item, meta) = if !input || outgasser::recipe_of_port(&s.id).is_some() { outgasser::java_item(&s.id) } else { (s.id.clone(), 0) };
                        let max_stack = outgasser::port_item(&item, meta).map_or(64, |(_, n)| n);
                        outgasser::Stack { item, count: s.count as i32, meta, max_stack }
                    };
                    o.input = c.slots.first().cloned().flatten().map(|s| as_recipe(&s, true));
                    o.output = c.slots.get(1).cloned().flatten().map(|s| as_recipe(&s, false));
                    if let (Some(i), Some(Some(s))) = (&o.input, c.slots.first())
                        && outgasser::recipe_of_port(&s.id).is_some()
                    {
                        r.ore_names.insert((i.item.clone(), i.meta), crate::oredict::names(&s.id).to_vec());
                    }
                    for (item, _, _, n) in outgasser::PORT_ITEMS {
                        r.max_stack.insert(item.to_string(), n);
                    }
                }
                r.add(col);
            }
            // The heaters' fluid identifiers (`feed.setType(0, slots)`): a stack that is one types the feed, one naming
            // no fluid as NONE.
            // The coolers' caches (T08): each cached cell's column, if it is still the one cached (the same
            // registration); a cell left empty, or holding a column put there since, is not cooled until the refresh.
            for (j, p) in order.iter().enumerate() {
                let m = &self.ready.world.machines[&(dim.clone(), [p[0], y, p[1]])];
                let Data::Rbmk(c) = &m.data else { unreachable!() };
                if m.block == "ntm:rbmk_heater"
                    && let Some(kind) = c.slots.first().cloned().flatten().as_ref().and_then(crate::combustion::identifier_type)
                {
                    r.identifiers.insert(j, crate::rbmk::fluids::fluid_of_network(&kind).unwrap_or("NONE"));
                }
                let Some(st) = &c.piped else { continue };
                if let ReactorData::Cooler(k) = &mut r.columns[j].data {
                    for (slot, cached) in k.neighbors.iter_mut().zip(&st.cache) {
                        *slot = cached.and_then(|(cell, seq)| order.iter().position(|o| *o == cell).filter(|_| layer.members.get(&cell).is_some_and(|mb| mb.seq == seq)));
                    }
                }
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
            // Occlusion (T04, `getHits`): the opaque cubes of the cells this reactor's streams can read, as Java's
            // `isOpaqueCube` (the RBMK family's own overrides, the engine's facts for the rest). A cell with a column
            // of the reactor is the column's (`Reactor::hits`); unknown is never air (D4).
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
                    let Some(opaque) = Self::rbmk_opaque(host, &dim, [c[0], y + h, c[1]]) else { return false };
                    if opaque {
                        r.obstacles.insert((c[0], c[1], h));
                    }
                }
            }
            // Test hook: the step's input, written once the step is accepted (`rbmk_flush_dumps`).
            if let Some(path) = order.iter().find_map(|p| self.ready.rbmk_dump_asked.remove(&(dim.clone(), [p[0], y, p[1]]))) {
                let d = crate::rbmk::dump::ReactorDump::of(&r, &dim);
                self.ready.rbmk_dumps.push((path, serde_json::to_string(&d).expect("a dump")));
            }
            r.step();
            // Test hook: the channels whose rod took flux in a step on a multiple of 200 (the flux it burnt, kept as
            // `lastFluxQuantity`; an overheating channel's is zeroed and does not count), apart from its reports.
            if r.time % 200 == 0 {
                let seen = self.ready.rbmk_flux_200.entry(dim.clone()).or_default();
                if seen.0 != r.time {
                    *seen = (r.time, BTreeSet::new());
                }
                let took = |c: &crate::rbmk::reactor::Column| c.rod().is_some_and(|rc| rc.fuel.is_some() && rc.last_flux_quantity > 0.0);
                seen.1.extend(order.iter().zip(&r.columns).filter(|(_, c)| took(c)).map(|(p, _)| [p[0], y, p[1]]));
            }
            // incrementRad of the open rods and of the streams over and into open columns (4d), as they came. D2: a NaN
            // flux makes a NaN increment, which Java stores and spreads (`clamp_float` lets it through) and which the
            // field's JSON cannot keep (it would write `null` and lose the whole key on reading): it adds nothing.
            self.ready.chunk_rad_pending.extend(r.rad.drain(..).filter(|e| !e.amount.is_nan()).map(|e| (dim.clone(), [e.x, e.y, e.z], e.amount)));
            // `SatelliteRayScan.reportEvent(INFO_NUCLEAR, 300)` of the rods that took flux (4A), with the step.
            for e in r.effects.drain(..) {
                match e {
                    Effect::SatelliteScan { column } => {
                        let p = order[column];
                        crate::rbmk::persist::report_ray(&mut self.ready.world.rbmk_rays, &dim, [p[0], y, p[1]], crate::rbmk::persist::INFO_NUCLEAR, self.ready.world.ticks as i64, 300);
                    }
                    // A full boiler's vent (T08): `playSoundEffect(x, y + getColumnHeight, z, "hbm:block.steamEngineOperate",
                    // 2, pitch)` at the core's corner, with the step's commit. Its `rbmksteam` particle
                    // (`ParticleRBMKSteam`) is T19's.
                    Effect::SteamVent { column, pitch, .. } => {
                        let p = order[column];
                        let at = [f64::from(p[0]), f64::from(y + dials.column_height), f64::from(p[1])];
                        host.show(&dim, &crate::blast::visuals::Show::Sound { id: "ntm:block.steamengineoperate", source: "block", at, volume: 2.0, pitch });
                    }
                }
            }
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
                let block = m.block.clone();
                let Data::Rbmk(c) = &mut m.data else { unreachable!() };
                c.heat = col.heat;
                if let Some(cc) = col.control() {
                    let st = c.control.get_or_insert_with(|| persist::ControlState::of(&block));
                    (st.level, st.target_level, st.starting_level, st.power, st.has_power) = (cc.level, cc.target_level, cc.starting_level, cc.power, cc.has_power);
                }
                c.reasim_water = col.reasim_water;
                c.reasim_steam = col.reasim_steam;
                let kept = |t: &Tank| persist::KeptTank { fluid: t.fluid.to_string(), fill: t.fill, max: t.max };
                match &col.data {
                    ReactorData::Boiler(b) => {
                        let st = c.piped.get_or_insert_with(|| persist::PipedState::of(&block).expect("a boiler"));
                        (st.input, st.output) = (kept(&b.feed), kept(&b.steam));
                        (st.vent_delay, st.consumption, st.produced) = (b.vent_delay, b.consumption, b.output);
                    }
                    ReactorData::Heater(h) => {
                        let st = c.piped.get_or_insert_with(|| persist::PipedState::of(&block).expect("a heater"));
                        (st.input, st.output) = (kept(&h.feed), kept(&h.steam));
                    }
                    ReactorData::Outgasser(o) => {
                        let st = c.outgasser.get_or_insert_with(|| persist::OutgasserState::of(&block).expect("an outgasser"));
                        (st.progress, st.gas) = (o.progress, kept(&o.gas));
                        // The slots back as the engine's stacks: the input's own stack, fewer; the output's, more, or the
                        // product's port item in an empty slot.
                        let back = |was: Option<Stack>, now: &Option<outgasser::Stack>| match (was, now) {
                            (_, None) => None,
                            (Some(s), Some(n)) => Some(Stack { count: n.count as u32, ..s }),
                            (None, Some(n)) => outgasser::port_item(&n.item, n.meta).map(|(id, _)| Stack::new(id, n.count as u32)),
                        };
                        let (input, output) = (c.slots.first().cloned().flatten(), c.slots.get(1).cloned().flatten());
                        if c.slots.len() >= 2 {
                            (c.slots[0], c.slots[1]) = (back(input, &o.input), back(output, &o.output));
                        }
                    }
                    ReactorData::Cooler(k) => {
                        let st = c.piped.get_or_insert_with(|| persist::PipedState::of(&block).expect("a cooler"));
                        (st.input, st.output) = (kept(&k.cold), kept(&k.hot));
                        st.timer = k.timer;
                        st.cache = k.neighbors.iter().map(|n| n.map(|n| (order[n], layer.members[&order[n]].seq))).collect();
                    }
                    _ => {}
                }
                // `TileEntityRBMKStorage.updateEntity`: its queue's pass every 10 ticks, in the column's own commit.
                if block == crate::rbmk::crane::STORAGE && r.time % 10 == 0 {
                    crate::rbmk::crane::storage_pass(&mut c.slots);
                }
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

    /// The ReaSim inlets and outlets (T08, `TileEntityRBMKInlet/Outlet.updateEntity`), after their networks moved
    /// fluid this tick: with `dialReasimBoilers`, each fills the ReaSim water of the column at each of its sides
    /// (NORTH, SOUTH, WEST, EAST; any cell of it, `findCore`) up to `maxWater`, or takes the column's steam into its
    /// tank. Out of the reactor step, as the networks are: a column steps with what the last tick left it.
    pub(super) fn rbmk_reasim_ports(&mut self, host: &mut impl ServerHost) {
        let ports: Vec<Key> = self.ready.world.machines.iter().filter(|(k, m)| matches!(m.data, Data::RbmkReasim(_)) && self.ready.loaded.get(*k) == Some(&true)).map(|(k, _)| k.clone()).collect();
        for key in ports {
            let (dim, [x, y, z]) = (key.0.clone(), key.1);
            if !self.rbmk_dials(&dim, "KEY_REASIM_BOILERS").reasim_boilers {
                continue;
            }
            for (dx, dz) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
                let Some(core) = Self::rbmk_core_of(host, &dim, [x + dx, y, z + dz]) else { continue };
                let ck = (dim.clone(), core);
                if self.ready.loaded.get(&ck) != Some(&true) {
                    continue;
                }
                let Some(Data::Rbmk(col)) = self.ready.world.machines.get(&ck).map(|m| &m.data) else { continue };
                let (water, steam) = (col.reasim_water, col.reasim_steam);
                let Some(Data::RbmkReasim(p)) = self.ready.world.machines.get_mut(&key).map(|m| &mut m.data) else { break };
                let (water, steam) = if p.inlet {
                    let prov = (persist::REASIM_MAX - water).min(p.tank.fill);
                    p.tank.fill -= prov;
                    (water + prov, steam)
                } else {
                    let prov = (p.tank.max - p.tank.fill).min(steam);
                    p.tank.fill += prov;
                    (water, steam - prov)
                };
                if let Some(Data::Rbmk(col)) = self.ready.world.machines.get_mut(&ck).map(|m| &mut m.data) {
                    (col.reasim_water, col.reasim_steam) = (water, steam);
                }
            }
        }
    }

    /// `RBMKBase.findCore` from a cell of a column: the core, the cell itself or the first column block under its run
    /// of dummies. None: no column there, or a cell on the way not loaded.
    pub(super) fn rbmk_core_of(host: &mut impl ServerHost, dim: &str, p: Pos) -> Option<Pos> {
        for dy in 0..=persist::DUMMY_REACH {
            let at = [p[0], p[1] - dy, p[2]];
            let b = host.get_block(dim, at)?;
            match blockstate::id_of(&b) {
                persist::DUMMY => continue,
                id if persist::is_column(id) => return Some(at),
                _ => return None,
            }
        }
        None
    }

    /// `TileEntityRBMKConsole.rescan` of every console in an open level when the world time is a multiple of 10 (T10):
    /// the 225 cells around its target, turned by its rotation, as the columns' block entities stand (each a column's
    /// type, heat, and a control rod's level and group); a cell with no column, or one not loaded, is none. Kept in
    /// memory, as the Java's `columns`.
    pub(super) fn rbmk_console_scan(&mut self) {
        if !self.ready.world.ticks.is_multiple_of(10) {
            return;
        }
        use crate::rbmk::console::{self, ColumnView};
        let consoles: Vec<Key> = self.ready.world.machines.iter().filter(|(k, m)| matches!(m.data, Data::RbmkConsole(_)) && self.ready.loaded.get(*k) == Some(&true)).map(|(k, _)| k.clone()).collect();
        for key in consoles {
            let Some(Data::RbmkConsole(c)) = self.ready.world.machines.get(&key).map(|m| &m.data) else { continue };
            let (st, dim) = (c.state.clone(), key.0.clone());
            let mut flux = 0.0;
            let scan: Vec<Option<ColumnView>> = (0..console::CELLS)
                .map(|i| {
                    let (x, z) = console::offset(st.rotation, i);
                    let k = (dim.clone(), [st.target[0] + x, st.target[1], st.target[2] + z]);
                    if self.ready.loaded.get(&k) != Some(&true) {
                        return None;
                    }
                    let m = self.ready.world.machines.get(&k)?;
                    let Data::Rbmk(col) = &m.data else { return None };
                    // `fuel.lastFluxQuantity` of each `TileEntityRBMKRod` (a ReaSim channel is one).
                    if persist::is_channel(&m.block) {
                        flux += col.last_flux_quantity;
                    }
                    console::view_of(&m.block, col)
                })
                .collect();
            if let Some(Data::RbmkConsole(c)) = self.ready.world.machines.get_mut(&key).map(|m| &mut m.data) {
                c.scan = scan;
                console::push_flux(&mut c.flux, flux);
            }
        }
    }

    /// What a world tick does for the RBMK before its tile entities (T04): `SatelliteRayScan.updateSystem` of each
    /// open dimension when the world time is 10 past a multiple of 20 (`ModEventHandler.worldTick:661`, a dimension
    /// whose world is not open is not ticked and keeps its events, as an unloaded `WorldServer`); and the engine's
    /// column height for where the acting player is (`rbmk_engine_height`).
    pub(super) fn rbmk_world_tick(&mut self, host: &mut impl ServerHost) {
        let now = self.ready.world.ticks as i64;
        if now % 20 == 10 {
            let dims: Vec<String> = self.ready.world.rbmk_rays.keys().cloned().collect();
            for dim in dims {
                if host.pile_reachable(&dim) && persist::expire_rays(&self.ready.world.rbmk_rays, &dim, now) {
                    persist::drop_expired_rays(&mut self.ready.world.rbmk_rays, &dim, now);
                }
            }
        }
        let dim = self.players.first().map_or_else(|| super::rbmk_columns::OVERWORLD.to_string(), |p| p.dim.clone());
        self.rbmk_engine_height(host, &dim);
    }

    /// Test hook (`NTM_TEST_HOOKS=1`): the input of the next accepted step of the reactor of the column at `pos`
    /// (`rbmk::dump`), written to `path` after it. False: no column there.
    pub fn native_test_rbmk_dump(&mut self, dim: &str, pos: Pos, path: &str) -> bool {
        let key = (dim.to_string(), pos);
        let column = self.ready.world.machines.get(&key).is_some_and(|m| matches!(m.data, Data::Rbmk(_)));
        if column {
            self.ready.rbmk_dump_asked.insert(key, path.to_string());
        }
        column
    }

    /// The dumps of this tick's accepted steps, to their files.
    pub(super) fn rbmk_flush_dumps(&mut self, host: &mut impl ServerHost) {
        for (path, json) in std::mem::take(&mut self.ready.rbmk_dumps) {
            if let Err(e) = std::fs::write(&path, json) {
                host.log(&format!("rbmk dump {path}: {e}"));
            }
        }
    }

    /// Test hook (`NTM_TEST_HOOKS=1`): the power a battery or a control rod's buffer holds now, as the server keeps it
    /// (a block entity's copy lags; a rod's buffer is never saved). None for another machine or none there.
    pub fn native_test_power(&self, dim: &str, pos: Pos) -> Option<i64> {
        match &self.ready.world.machines.get(&(dim.to_string(), pos))?.data {
            Data::Battery(b) => Some(b.power as i64),
            Data::Rbmk(c) => c.control.as_ref().map(|c| c.power),
            _ => None,
        }
    }

    /// Test hook (`NTM_TEST_HOOKS=1`): the dimension's kept `SatelliteRayScan` events, one `x,y,z:info:expiresOn` each,
    /// in their order, and the world tick.
    pub fn native_test_rays(&self, dim: &str) -> String {
        let list: Vec<String> = self.ready.world.rbmk_rays.get(dim).into_iter().flatten().map(|e| format!("{},{},{}:{}:{}", e.pos[0], e.pos[1], e.pos[2], e.info, e.expires_on)).collect();
        let (at, took) = self.ready.rbmk_flux_200.get(dim).cloned().unwrap_or((-1, BTreeSet::new()));
        let took: Vec<String> = took.iter().map(|p| format!("{},{},{}", p[0], p[1], p[2])).collect();
        format!("tick={} rays={} flux200={at}:{}", self.ready.world.ticks, list.join(";"), took.join(";"))
    }

    /// Test hook (`NTM_TEST_HOOKS=1`, RBMK T08's cut): what the tanks of the machines at `cells` hold, read in one go
    /// (a flow between them never splits the reading): `x,y,z=fluid:amount+fluid:amount` for each, `;` between them,
    /// `none` for a cell without a tank. A barrel's tank, a boiler's, heater's or cooler's two, a ReaSim port's; an
    /// outgasser's gas and the stacks of its slots (`id:count`, T09).
    pub fn native_test_fluids(&self, dim: &str, cells: &[Pos]) -> String {
        let read = |p: &Pos| -> Vec<(String, i64)> {
            match self.ready.world.machines.get(&(dim.to_string(), *p)).map(|m| &m.data) {
                Some(Data::FluidStore(s)) => vec![(s.tank.fluid.clone(), i64::from(s.tank.mb))],
                Some(Data::Rbmk(c)) => c
                    .piped
                    .iter()
                    .flat_map(|t| [(t.input.fluid.clone(), i64::from(t.input.fill)), (t.output.fluid.clone(), i64::from(t.output.fill))])
                    .chain(c.outgasser.iter().map(|o| (o.gas.fluid.clone(), i64::from(o.gas.fill))))
                    // An outgasser's slots too (T09's cut): what it consumes and makes, in the same reading.
                    .chain(c.outgasser.iter().flat_map(|_| c.slots.iter().flatten().map(|s| (s.id.clone(), i64::from(s.count)))))
                    .collect(),
                Some(Data::RbmkReasim(r)) => vec![(r.tank.fluid.clone(), i64::from(r.tank.fill))],
                _ => Vec::new(),
            }
        };
        cells
            .iter()
            .map(|p| {
                let tanks = read(p);
                let text = if tanks.is_empty() { "none".to_string() } else { tanks.iter().map(|(f, n)| format!("{f}:{n}")).collect::<Vec<_>>().join("+") };
                format!("{},{},{}={text}", p[0], p[1], p[2])
            })
            .collect::<Vec<_>>()
            .join(";")
    }

    /// Test hook (`NTM_TEST_HOOKS=1`, RBMK T08): the column at `pos` starts its reactor's next step at `heat` (a
    /// boiler, heater or cooler made hot without a reactor around it). False: no column there, or a heat not finite.
    pub fn native_test_heat(&mut self, dim: &str, pos: Pos, heat: f64) -> bool {
        let key = (dim.to_string(), pos);
        let column = self.ready.world.machines.get(&key).is_some_and(|m| matches!(m.data, Data::Rbmk(_)));
        if column && heat.is_finite() {
            self.ready.rbmk_heats.insert(key, heat);
        }
        column && heat.is_finite()
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
    pub fn native_test_take(&mut self, host: &mut impl ServerHost, dim: &str, pos: Pos, player: &str, cursor: bool) -> Result<(), &'static str> {
        let key = (dim.to_string(), pos);
        let Some(m) = self.ready.world.machines.get(&key).cloned() else { return Err("no machine") };
        if self.ready.broken.contains_key(&key) || !self.model_current(&key, &m) || !matches!(m.data, Data::Rbmk(_)) {
            return Err("not a current RBMK column");
        }
        let Some(hand) = host.player_slots(player) else { return Err("no acting player's stacks") };
        let Some(expected) = host.machine_slots(dim, pos) else { return Err("no slots") };
        let Some(Some(rod)) = expected.first().cloned() else { return Err("no rod in the channel") };
        let mut machine = m.clone();
        let Data::Rbmk(col) = &mut machine.data else { unreachable!() };
        col.slots = vec![None];
        let (desired, data) = Self::machine_output(&mut machine, &expected);
        let json = serde_json::to_string(&machine).expect("machine serialization");
        let commit = MachineCommit { cells: Vec::new(), guard: CellGuard::machine(&key, &m), expected_saved: self.ready.native_saved.contains_key(&key), dim: dim.into(), pos, expected, desired, data, json };
        let (edits, drops) = if cursor {
            if hand.carried.is_some() {
                return Err("the cursor is full");
            }
            (vec![SlotEdit { slot: PlayerSlot::Carried, expected: None, desired: Some(rod) }], Vec::new())
        } else {
            (Vec::new(), vec![(rod, TransferDrop::Thrown)])
        };
        let transfer = PlayerTransfer { player: player.into(), edits, drops, copies: Vec::new() };
        if !host.commit_player_transfer(&transfer, std::slice::from_ref(&commit)) {
            return Err("the transfer was refused");
        }
        self.ready.world.machines.insert(key.clone(), machine);
        self.ready.bridge.last.insert(key.clone(), commit.desired);
        self.ready.bridge.data.insert(key.clone(), commit.data);
        self.ready.native_saved.insert(key, commit.json);
        Ok(())
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
