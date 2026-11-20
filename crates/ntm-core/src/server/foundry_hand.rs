//! The strand caster by hand (`MachineStrandCaster.onBlockActivated`:137-168): a mold in hand goes into the empty mold
//! slot, a shovel takes the pool as one scrap. Like the sawmill's uses, the model and the player's stacks change in one
//! transfer (`commit_player_transfer`): a refusal moves nothing.
use super::*;
use crate::foundry::scrap;
use crate::machine::Data;
use crate::strand_caster::slot;
use container_bridge::Stack;

/// A shovel for `ItemTool.getToolClasses(...).contains("shovel")`: the vanilla shovels and the NTM's.
pub fn is_shovel(item: &str) -> bool {
    item.ends_with("_shovel")
}

impl Server {
    /// A player used the caster whose core is `core` (not sneaking): a mold in hand into an empty mold slot (one of the
    /// stack, with its data), else a shovel: the whole pool to the player as one exact scrap (what does not fit is thrown),
    /// the pool emptied. True when the use is taken, which with a shovel is whatever the pool holds (the Java returns true
    /// with an empty pool too and does nothing); false when none applies, so the click opens the screen. The slots read
    /// are the block entity's now.
    ///
    /// Deviations: what does not fit is thrown from the eyes, as the sawmill's, not spawned over the machine
    /// (`y + maxY`); and a scrap thrown is only what did not fit (the Java adds to the inventory or spawns, never both).
    pub fn native_caster_use(&mut self, host: &mut impl ServerHost, dim: &str, at: Pos, core: Pos, player: &str) -> bool {
        let key = (dim.to_string(), core);
        let Some(m) = self.ready.world.machines.get(&key).cloned() else { return false };
        let Data::StrandCaster(c) = &m.data else { return false };
        let Some(hand) = host.player_slots(player) else { return false };
        let held = hand.held().cloned();
        let Some(held_id) = held.as_ref().map(|h| h.stack.id.clone()) else { return false };
        let mold = crate::foundry::Tables::ntm().mold(&held_id).is_some();
        let shovel = is_shovel(&held_id);
        if !mold && !shovel {
            return false;
        }
        if self.ready.broken.contains_key(&key) || !self.model_current(&key, &m) {
            return true;
        }
        if !host.get_block(dim, core).is_some_and(|b| blockstate::id_of(&b) == m.block) {
            return false;
        }
        let Some(expected) = host.machine_slots(dim, core) else { return true };
        let from = hand.hand;
        let mut machine = m.clone();
        let Data::StrandCaster(next) = &mut machine.data else { unreachable!() };
        next.slots = expected.clone();
        let mut edits = Vec::new();
        let mut drops = Vec::new();
        let mut copies = Vec::new();
        let (mold_in, scoop) = (mold && next.slots[slot::MOLD].is_none(), shovel && c.melt.amount > 0);
        if mold_in {
            let (Some(h), Some(f)) = (held.as_ref(), from) else { return false };
            next.slots[slot::MOLD] = Some(Stack { count: 1, ..h.stack.clone() });
            edits.extend(hand.take(f, 1));
            copies.push(SlotCopy { dim: dim.into(), pos: core, slot: slot::MOLD, from: f });
        } else if scoop {
            let Some(mat) = next.melt.mat.clone() else { return false };
            let (given, left) = hand.give(&[scrap::stack(&mat, next.melt.amount)], 64);
            edits = given;
            drops = left.into_iter().map(|s| (s, TransferDrop::Thrown)).collect();
            next.melt = Default::default();
        } else {
            // A shovel and an empty pool: the Java takes the click and does nothing.
            return shovel;
        }
        let (desired, data) = Self::machine_output(&mut machine, &expected);
        let json = serde_json::to_string(&machine).expect("machine serialization");
        let commit = MachineCommit { cells: Vec::new(), guard: CellGuard::machine(&key, &m), expected_saved: self.ready.native_saved.contains_key(&key), dim: dim.into(), pos: core, expected, desired, data, json };
        let transfer = PlayerTransfer { player: player.into(), edits, drops, copies };
        if !host.commit_player_transfer(&transfer, std::slice::from_ref(&commit)) {
            return true;
        }
        self.ready.world.machines.insert(key.clone(), machine);
        self.ready.bridge.last.insert(key.clone(), commit.desired);
        self.ready.bridge.data.insert(key.clone(), commit.data);
        self.ready.native_saved.insert(key, commit.json);
        if mold_in {
            host.show(dim, &crate::blast::visuals::Show::Sound { id: "ntm:item.upgradeplug", source: "block", at: at.map(|v| f64::from(v) + 0.5), volume: 1.0, pitch: 1.0 });
        }
        true
    }
}
