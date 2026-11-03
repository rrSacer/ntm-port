// Local bundle bridge: upstream Sim is pub(crate). Remove when upstream
// exposes a post-publication update entry point. All behaviour stays in Sim.
use super::{Behaviours, LevelAccess, flags};
use crate::registry::BlockStateId;
use horizon_core::position::BlockPos;

impl Behaviours {
    /// Complete setBlock(flag 3) after an already published, no-side-effects
    /// write. BE contents were retained by the atomic caller, so they cannot
    /// be spilled again by preRemoveSideEffects.
    pub fn ntm_committed_updates<L: LevelAccess + ?Sized>(
        &self,
        level: &mut L,
        ticks: &mut super::Ticks,
        pos: BlockPos,
        old: BlockStateId,
        new: BlockStateId,
    ) {
        let mut sim = self.sim(level, ticks);
        if self.block(old) != self.block(new) || self.data(new).is(super::classes::Class::BaseRail) {
            sim.affect_neighbors_after_removal(old, pos, false);
        }
        sim.on_place(new, pos, old, false);
        if sim.state(pos) != new {
            return;
        }
        sim.update_neighbors_at(pos, self.block(old));
        if sim.has_analog_output(new) {
            sim.update_neighbour_for_output_signal(pos, self.block(new));
        }
        let updates = flags::UPDATE_CLIENTS;
        sim.update_indirect_neighbour_shapes(old, pos, updates, flags::UPDATE_LIMIT - 1);
        sim.update_neighbour_shapes(new, pos, updates, flags::UPDATE_LIMIT - 1);
        sim.update_indirect_neighbour_shapes(new, pos, updates, flags::UPDATE_LIMIT - 1);
    }
}
