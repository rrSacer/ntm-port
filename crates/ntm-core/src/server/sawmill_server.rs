//! The sawmill's server side (Onda 2): its tick after the heat (`sawmill_finish`, the blade's failure through the
//! ledger), its uses by hand (`native_sawmill_use`), and its item without the blade across placing and breaking it
//! (D3: `MachineSawmill.onBlockPlacedBy` / `getDrops`, the Java's damage 1; the host reads and writes the item's
//! `minecraft:custom_data`, the model keeps `has_blade`).
use super::*;
use crate::machine::{Data, MachineEffect};
use crate::sawmill::BLADE;
use container_bridge::Stack;

impl Server {
    /// `finish` of every loaded sawmill (`TileEntitySawmill.updateEntity` after `tryPullHeat`, which `heat_tick` did).
    /// The tick that breaks the blade (128-147) is committed with its two effects as one batch, before this tick's
    /// native_save, in this order (R4: the blast must not kill the blade it throws): the blast at
    /// [x+0.5, y+1, z+0.5], power 5, no fire, no broken block; the blade, one `ntm:sawblade` at the cell in front of
    /// the core one up (the Java's `EntitySawblade` at core + dir, y+1, which gives the blade back when used; the entity
    /// and its flight need the entity API, so the item lies there).
    pub(super) fn sawmill_finish(&mut self, host: &mut impl ServerHost) {
        let r = &mut self.ready;
        let mut failing = Vec::new();
        for (k, m) in &mut r.world.machines {
            if r.loaded.get(k) == Some(&true)
                && let Data::Sawmill(s) = &mut m.data
            {
                if s.fails_on_finish() {
                    failing.push(k.clone());
                } else {
                    s.finish();
                }
            }
        }
        for key in failing {
            let published = self.publish_machine_effects(host, &key, |m| {
                let Data::Sawmill(s) = &mut m.data else { return Vec::new() };
                s.finish();
                vec![
                    MachineEffect::Explode { offset: [0., 1., 0.], power: 5., fire: false, breaks: false },
                    MachineEffect::Drop { offset: [0, 1, 1], item: crate::sawmill::BLADE.into(), count: 1 },
                ]
            });
            // Refused: the live model keeps its blade and count and breaks on the next hot tick. Only the heat of this
            // tick is spent, as `finish` spends it every tick (it is never saved).
            if !published
                && let Some(Data::Sawmill(s)) = self.ready.world.machines.get_mut(&key).map(|m| &mut m.data)
            {
                s.heat = 0;
                host.log(&format!("ntm: the failing sawmill at {:?} was not committed; it breaks on its next hot tick", key.1));
            }
        }
    }

    /// A player placed a sawmill item at `pos` (`native_place_block`, before its `Placed` event is taken): without its
    /// blade, the machine that event creates there has none.
    pub fn native_sawmill_placed(&mut self, dim: &str, pos: Pos, without_blade: bool) {
        let key = (dim.to_string(), pos);
        if without_blade {
            self.ready.sawmills_without_blade.insert(key);
        } else {
            self.ready.sawmills_without_blade.remove(&key);
        }
    }

    /// Whether the sawmill broken at `pos` drops its item without its blade: its model, still held when the loot table
    /// rolls (the `Broken` event is taken after), says so.
    /// A placement not taken yet counts: placed and broken before its `Placed` event, it still has no blade.
    pub fn native_sawmill_without_blade(&self, dim: &str, pos: Pos) -> bool {
        let key = (dim.to_string(), pos);
        self.ready.sawmills_without_blade.contains(&key)
            || matches!(self.ready.world.machines.get(&key).map(|m| &m.data), Some(Data::Sawmill(s)) if !s.has_blade)
    }

    /// Every cell `native_sawmill_without_blade` answers yes for: the host's copy for the drops it hears while the server
    /// is busy (a blast of the server's own breaking a sawmill).
    pub fn native_sawmills_without_blade(&self) -> Vec<Key> {
        let models = self.ready.world.machines.iter().filter(|(_, m)| matches!(&m.data, Data::Sawmill(s) if !s.has_blade));
        self.ready.sawmills_without_blade.iter().cloned().chain(models.map(|(k, _)| k.clone())).collect()
    }

    /// `MachineSawmill.onBlockActivated` without sneaking (83-129), at its core: in the Java's order, a blade in hand
    /// back into a sawmill without one; else its outputs (slots 1 and 2) to the player, what does not fit thrown from
    /// the eyes; else one of the stack in hand into an empty input, when it has an output. The model and the player's
    /// stacks change in one transfer: a refusal moves nothing. False when none applies (the click goes on, as the
    /// Java's `return false`).
    ///
    /// Two deviations, both to keep every item once: what is thrown is only what did not fit (the Java throws a copy
    /// of the whole stack when part of it fit, a duplication); and the input keeps the stack's `custom_data` only (the
    /// Java copies its whole NBT).
    pub fn native_sawmill_use(&mut self, host: &mut impl ServerHost, dim: &str, at: Pos, core: Pos, player: &str) -> bool {
        let key = (dim.to_string(), core);
        let Some(m) = self.ready.world.machines.get(&key).cloned() else { return false };
        let Data::Sawmill(s) = &m.data else { return false };
        if self.ready.broken.contains_key(&key) || !self.model_current(&key, &m) {
            return true;
        }
        if !host.get_block(dim, core).is_some_and(|b| blockstate::id_of(&b) == m.block) {
            return false;
        }
        let Some(hand) = host.player_slots(player) else { return false };
        let held = hand.held().cloned();
        let from = hand.hand;
        let mut machine = m.clone();
        let Data::Sawmill(next) = &mut machine.data else { unreachable!() };
        let mut edits = Vec::new();
        let mut drops = Vec::new();
        let repair = !s.has_blade && held.as_ref().is_some_and(|h| h.stack.id == BLADE);
        if repair {
            next.has_blade = true;
            edits.extend(from.and_then(|f| hand.take(f, 1)));
        } else if s.slot(1).is_some() || s.slot(2).is_some() {
            let out: Vec<Stack> = [1, 2].iter().filter_map(|&i| next.slots[i].take()).collect();
            let (given, left) = hand.give(&out, 64);
            edits = given;
            drops = left.into_iter().map(|s| (s, TransferDrop::Thrown)).collect();
        } else if let Some(h) = held.as_ref().filter(|h| s.slot(0).is_none() && crate::sawmill::output(&h.stack.id).is_some()) {
            next.slots[0] = Some(Stack { count: 1, ..h.stack.clone() });
            edits.extend(from.and_then(|f| hand.take(f, 1)));
        } else {
            return false;
        }
        let expected = host.machine_slots(dim, core).unwrap_or_default();
        let (desired, data) = Self::machine_output(&mut machine, &expected);
        let json = serde_json::to_string(&machine).expect("machine serialization");
        let commit = MachineCommit { cells: Vec::new(), guard: CellGuard::machine(&key, &m), expected_saved: self.ready.native_saved.contains_key(&key), dim: dim.into(), pos: core, expected, desired, data, json };
        let transfer = PlayerTransfer { player: player.into(), edits, drops, copies: Vec::new() };
        if !host.commit_player_transfer(&transfer, std::slice::from_ref(&commit)) {
            return true;
        }
        self.ready.world.machines.insert(key.clone(), machine);
        self.ready.bridge.last.insert(key.clone(), commit.desired);
        self.ready.bridge.data.insert(key.clone(), commit.data);
        self.ready.native_saved.insert(key, commit.json);
        if repair {
            host.show(dim, &crate::blast::visuals::Show::Sound { id: "ntm:item.upgradeplug", source: "block", at: at.map(|v| f64::from(v) + 0.5), volume: 1.5, pitch: 0.75 });
        }
        true
    }
}
