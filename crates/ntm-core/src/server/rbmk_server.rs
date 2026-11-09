//! The RBMK in the world (`rbmk-plano` T01): one coupled group per layer (dimension and base Y) whose step runs the
//! pure `Reactor` over the columns' block entities and the layer's kept state (order, random, queued streams), all
//! committed together or not at all; and the fuel rod put in by hand, moved in one transfer with the player.
//!
//! **Provisional (approved D-b):** the minimal step needed to prove the transactional contract. Default dials and
//! height, no lids, ports, menus by context, radiation, meltdowns (logged, not run) or the HBM opacity of blocks (any
//! block counts as opaque); `crate::rbmk::persist` lists which ticket takes each.
use super::*;
use crate::{
    machine::Data,
    server::container_bridge::Stack,
    rbmk::{
        dials::{DialState, Dials},
        persist::{self, Layer, RbmkColumn},
        reactor::{Column, Reactor},
    },
};

/// The default dials, as a world that never set its rules reads them (T04 brings the rules).
fn default_dials() -> Dials {
    let mut d = DialState::default();
    d.create(false);
    d.refresh();
    *d.cached().expect("refreshed dials")
}

impl Server {
    /// The layer of an RBMK column: its dimension and base Y. None for other machines.
    pub(super) fn rbmk_layer(k: &Key, m: &Machine) -> Option<(String, i32)> {
        matches!(m.data, Data::Rbmk(_)).then(|| (k.0.clone(), k.1[1]))
    }

    /// One step of the layer whose columns are the group's machines. False when the layer cannot step now (a column
    /// of its order in a chunk not loaded, or not adopted yet, or a cell of its streams' reach unknown: D4 never takes
    /// an unknown cell for air): the group's step is refused and nothing advances.
    pub(super) fn rbmk_tick(&mut self, host: &mut impl ServerHost) -> bool {
        let keys: Vec<Key> = self.ready.world.machines.iter().filter(|(_, m)| matches!(m.data, Data::Rbmk(_))).map(|(k, _)| k.clone()).collect();
        let Some(first) = keys.first() else { return true };
        let (dim, y) = (first.0.clone(), first.1[1]);
        let mut layer = self.ready.world.rbmk.get(&(dim.clone(), y)).cloned().unwrap_or_else(|| Layer::new(&dim, y));
        let present: BTreeMap<[i32; 2], Key> = keys.iter().map(|k| ([k.1[0], k.1[2]], k.clone())).collect();
        // D1: the kept order; a column the world no longer shows leaves it, one it shows but the server has not
        // adopted yet (or cannot see) holds the whole layer.
        let mut order = Vec::new();
        for p in &layer.order {
            if present.contains_key(p) {
                order.push(*p);
                continue;
            }
            match host.get_block(&dim, [p[0], y, p[1]]) {
                None => return false,
                Some(b) if persist::kind_of(blockstate::id_of(&b)).is_some() => return false,
                Some(_) => {}
            }
        }
        let mut new: Vec<[i32; 2]> = present.keys().filter(|p| !order.contains(p)).copied().collect();
        new.sort();
        order.extend(new);
        layer.order = order.clone();
        let dials = default_dials();
        let mut r = Reactor::new(dials, layer.rng(), y);
        r.time = self.ready.world.ticks as i64;
        for p in &order {
            let m = &self.ready.world.machines[&present[p]];
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
        r.set_queued(layer.queue());
        // Provisional occlusion (T04): every block that is not air, in every non-column cell a stream can reach.
        let (reach, height) = (dials.flux_range, dials.column_height + 1);
        let (xs, zs) = (order.iter().map(|p| p[0]), order.iter().map(|p| p[1]));
        let (x0, x1) = (xs.clone().min().unwrap() - reach, xs.max().unwrap() + reach);
        let (z0, z1) = (zs.clone().min().unwrap() - reach, zs.max().unwrap() + reach);
        for x in x0..=x1 {
            for z in z0..=z1 {
                if present.contains_key(&[x, z]) {
                    continue;
                }
                for h in 0..height {
                    let Some(b) = host.get_block(&dim, [x, y + h, z]) else { return false };
                    if !blockstate::id_of(&b).ends_with("air") {
                        r.obstacles.insert((x, z, h));
                    }
                }
            }
        }
        r.step();
        for i in &r.meltdowns {
            let c = &r.columns[*i];
            host.log(&format!("rbmk meltdown due at {dim} [{}, {y}, {}]: not run before T16", c.x, c.z));
        }
        for (i, p) in order.iter().enumerate() {
            let col = &r.columns[i];
            let m = self.ready.world.machines.get_mut(&present[p]).unwrap();
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
        layer.set_rng(&r.rng);
        layer.set_queue(&r.queued());
        if self.ready.world.rbmk.get(&(dim.clone(), y)) != Some(&layer) {
            self.ready.world.rbmk.insert((dim, y), layer);
        }
        true
    }

    /// A player's use of an RBMK column (`core` its base): a fuel rod in hand goes into an empty fuel channel, one of
    /// the stack, the hand debited in survival only, in one transfer (`RBMKRod.onBlockActivated`). True when the use
    /// was the insertion (or was refused as a whole); false lets the click go on (the menu: T05).
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
        let edits = if hand.creative { Vec::new() } else { hand.take(hand.hand.expect("a held stack has a hand"), 1).into_iter().collect() };
        let mut machine = m.clone();
        let Data::Rbmk(col) = &mut machine.data else { unreachable!() };
        **col = RbmkColumn { slots: vec![Some(Stack { count: 1, ..held.stack.clone() })], ..(**col).clone() };
        let (desired, data) = Self::machine_output(&mut machine, &expected);
        let json = serde_json::to_string(&machine).expect("machine serialization");
        let commit = MachineCommit { cells: Vec::new(), guard: CellGuard::machine(&key, &m), expected_saved: self.ready.native_saved.contains_key(&key), dim: dim.into(), pos: core, expected, desired, data, json };
        let transfer = PlayerTransfer { player: player.into(), edits, drops: Vec::new() };
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
