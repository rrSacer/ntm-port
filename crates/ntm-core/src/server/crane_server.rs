//! The RBMK crane on the server (T11, `rbmk::crane`): the keys the players' clients hold, each crane's input before the
//! machines step (the first player in its console's area, what its lamps and meters show), and after them the transfer
//! of a hook at the bottom: the item between the crane's hidden slot and the column under it, both blocks in one
//! publication (`commit_machines_moving`), or nothing.
use super::*;
use crate::machine::Data;
use crate::rbmk::crane::{self, Keys};
use crate::rbmk::persist;
use crate::server::container_bridge::Slots;

impl Server {
    /// The crane keys a player's client holds now (`KeybindPacket`, sent at each change): kept until the next change
    /// or until the player leaves.
    pub fn native_crane_keys(&mut self, player: &str, bits: u8) {
        if bits == 0 {
            self.crane_keys.remove(player);
        } else {
            self.crane_keys.insert(player.to_string(), Keys::from_bits(bits));
        }
    }

    fn crane_facing(m: &Machine) -> Option<[i32; 2]> {
        m.facing.as_deref().and_then(registry::facing_dir).map(|f| [f[0], f[2]])
    }

    fn cranes(&self) -> Vec<Key> {
        self.ready.world.machines.iter().filter(|(k, m)| matches!(m.data, Data::RbmkCrane(_)) && self.ready.loaded.get(*k) == Some(&true)).map(|(k, _)| k.clone()).collect()
    }

    /// Before the steps (`updateEntity`'s reads): each loaded crane's keys, those of the first player in its console's
    /// area (none: nobody there, and it stands still); whether a loadable column is under the hook
    /// (`isAboveValidTarget`); the held rod's hull heat and enrichment (`loadedHeat`, `loadedEnrichment`).
    pub(super) fn rbmk_crane_input(&mut self, host: &mut impl ServerHost) {
        for key in self.cranes() {
            let m = &self.ready.world.machines[&key];
            let Some(facing) = Self::crane_facing(m) else { continue };
            let Data::RbmkCrane(c) = &m.data else { continue };
            let area = crane::area(key.1, facing);
            let input = self.players.iter().find(|p| p.dim == key.0 && crane::in_area(p.pos, area)).map(|p| self.crane_keys.get(&p.id).copied().unwrap_or_default());
            let cell = c.column_cell(facing);
            let meters = c.held().and_then(|s| persist::rod_of(&s.id).map(|rod| (rod, persist::stack_rod(s)))).map_or([0.0; 2], |(rod, mut r)| [r.hull_heat(rod), r.enrichment(rod)]);
            let valid = Self::rbmk_core_of(host, &key.0, cell)
                .and_then(|core| self.ready.world.machines.get(&(key.0.clone(), core)))
                .is_some_and(|m| crane::loadable_slots(&m.block).is_some());
            if let Some(Data::RbmkCrane(c)) = self.ready.world.machines.get_mut(&key).map(|m| &mut m.data) {
                (c.input, c.valid_target, c.meters) = (input, valid, meters);
            }
        }
    }

    /// After the steps: a crane whose hook reached the bottom (`arrived`) loads the column under it with the item it
    /// holds, or takes the column's next (`IRBMKLoadable`), the two blocks in one publication; a column it cannot use, a
    /// cell not loaded, or a world that changed under it leaves both as they were (the hook goes up as it came).
    pub(super) fn rbmk_crane_transfer(&mut self, host: &mut impl ServerHost) {
        for key in self.cranes() {
            let Some(m) = self.ready.world.machines.get_mut(&key) else { continue };
            let Data::RbmkCrane(c) = &mut m.data else { continue };
            if !std::mem::take(&mut c.arrived) {
                continue;
            }
            self.crane_transfer(host, &key);
        }
    }

    fn crane_transfer(&mut self, host: &mut impl ServerHost, key: &Key) -> bool {
        let dim = key.0.clone();
        let Some(m) = self.ready.world.machines.get(key).cloned() else { return false };
        let (Some(facing), Data::RbmkCrane(c)) = (Self::crane_facing(&m), &m.data) else { return false };
        let Some(core) = Self::rbmk_core_of(host, &dim, c.column_cell(facing)) else { return false };
        let ck = (dim.clone(), core);
        let Some(col) = self.ready.world.machines.get(&ck).cloned() else { return false };
        let Some((into, from)) = crane::loadable_slots(&col.block) else { return false };
        for (k, x) in [(key, &m), (&ck, &col)] {
            if self.ready.broken.contains_key(k) || !self.model_current(k, x) || self.ready.loaded.get(k) != Some(&true) {
                return false;
            }
        }
        let (Some(crane_was), Some(col_was)) = (host.machine_slots(&dim, key.1), host.machine_slots(&dim, core)) else { return false };
        let (mut crane_now, mut col_now) = (crane_was.clone(), col_was.clone());
        let held = crane_was.first().cloned().flatten();
        // `canTargetInteract`, then `load` (the held item into the column) or `provideNext` and `unload`.
        let mv = if held.is_some() {
            if !crane::can_load(&col.block, &col_was) {
                return false;
            }
            col_now[into] = crane_now[0].take();
            SlotMove { dim: dim.clone(), from: (key.1, 0), to: (core, into) }
        } else {
            if !crane::can_unload(&col.block, &col_was) || crane_now.is_empty() {
                return false;
            }
            crane_now[0] = col_now[from].take();
            SlotMove { dim: dim.clone(), from: (core, from), to: (key.1, 0) }
        };
        let (mut crane_m, mut col_m) = (m.clone(), col.clone());
        if let Data::RbmkCrane(c) = &mut crane_m.data {
            c.slots = crane_now;
        }
        if let Data::Rbmk(c) = &mut col_m.data {
            c.slots = col_now;
        }
        let commit = |k: &Key, was: &Machine, now: &mut Machine, expected: Slots| {
            let (desired, data) = Self::machine_output(now, &expected);
            let json = serde_json::to_string(&*now).expect("machine serialization");
            MachineCommit { cells: Vec::new(), guard: CellGuard::machine(k, was), expected_saved: self.ready.native_saved.contains_key(k), dim: dim.clone(), pos: k.1, expected, desired, data, json }
        };
        let commits = [commit(key, &m, &mut crane_m, crane_was), commit(&ck, &col, &mut col_m, col_was)];
        if !host.commit_machines_moving(&commits, std::slice::from_ref(&mv)) {
            return false;
        }
        let [a, b] = commits;
        for (k, machine, commit) in [(key.clone(), crane_m, a), (ck, col_m, b)] {
            self.ready.world.machines.insert(k.clone(), machine);
            self.ready.bridge.last.insert(k.clone(), commit.desired);
            self.ready.bridge.data.insert(k.clone(), commit.data);
            self.ready.native_saved.insert(k, commit.json);
        }
        true
    }

    /// The RBMK tool on a crane console with a column linked (`setTarget`): the spans found now around the column, at
    /// the girders' height (a cell of a chunk not loaded stops a span); the link waits for the console's next tick.
    /// False when `core` is no crane console.
    pub fn native_rbmk_crane_link(&mut self, host: &mut impl ServerHost, dim: &str, core: Pos, target: Pos) -> bool {
        let key = (dim.to_string(), core);
        let Some(m) = self.ready.world.machines.get(&key) else { return false };
        let Some(facing) = Self::crane_facing(m) else { return false };
        if !matches!(m.data, Data::RbmkCrane(_)) || !self.model_current(&key, m) || self.ready.broken.contains_key(&key) {
            return false;
        }
        let height = self.rbmk_dials_peek(dim, rbmk_columns::HEIGHT_KEY).column_height;
        let mut c = crane::Crane::default();
        c.link(target, height, facing, |p| host.get_block(dim, p).map(|b| matches!(blockstate::id_of(&b), "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")));
        self.queue_pending(&key, crate::rbmk::screen::Pending::CraneLink(c.center, [c.span_f, c.span_r, c.span_b, c.span_l]));
        true
    }

    /// A screwdriver on a crane console (`RBMKCraneConsole.onScrew` → `cycleCraneRotation`): the bridge's look a quarter
    /// turn, at the console's next tick. False when `core` is no crane console.
    pub fn native_rbmk_crane_rotate(&mut self, dim: &str, core: Pos) -> bool {
        let key = (dim.to_string(), core);
        let Some(m) = self.ready.world.machines.get(&key) else { return false };
        if !matches!(m.data, Data::RbmkCrane(_)) || !self.model_current(&key, m) || self.ready.broken.contains_key(&key) {
            return false;
        }
        self.queue_pending(&key, crate::rbmk::screen::Pending::Rotate(1));
        true
    }
}
