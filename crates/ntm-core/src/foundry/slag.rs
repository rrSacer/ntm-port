//! The slag the slag tap pours (`BlockDynamicSlag.java`, `TileEntitySlag`): a block that holds one material, up to
//! sixteen blocks of metal, and gives it back whole as an exact scrap when it is broken.
//!
//! The tap itself is an [`Outlet`](super::flow::Outlet) in the model (filter, inversion); what it pours is decided by
//! the server (`server::slag_server`), because a pour creates a block and changes the source in one commit.
//! Left out of this version, as declared in `docs/content-foundry.md`: the block's own flow (`updateTick`: down, onto
//! the slag below, sideways), the colour of the material (`colorMultiplier`) and the pick-block.

use super::{Tables, quanta_name};
use super::flow::Melt;
use crate::{
    machine::{Logic, fill_level},
    server::container_bridge::{Slots, Slotted},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

/// `TileEntitySlag.maxAmount = MaterialShapes.BLOCK.q(16)`: sixteen blocks of 648 quanta.
pub const SLAG_CAP: u32 = 648 * 16;
/// The layers of the block's height: `(float) amount / maxAmount` in 1/8 steps (`ntm:slag`'s `layer`).
pub const LAYERS: u8 = 8;

/// One block of slag: one material and its exact quanta. Everything it holds is in its record (it has no slots).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Slag {
    #[serde(flatten)]
    pub melt: Melt,
    #[serde(skip)]
    pub slots: Slots,
}

impl Slag {
    /// A block with `amount` quanta of `mat`.
    pub fn of(mat: &str, amount: u32) -> Self {
        Self { melt: Melt { mat: Some(mat.to_string()), amount }, slots: Slots::new() }
    }

    /// What it takes of `q` quanta of `mat` (`Math.min(tile.maxAmount - tile.amount, stack.amount)`, the same material
    /// only), and so what is left: `q` if it takes none.
    pub fn takes(&self, mat: &str, q: u32) -> u32 {
        if self.melt.mat.as_deref() == Some(mat) { q.min(SLAG_CAP.saturating_sub(self.melt.amount)) } else { 0 }
    }
}

impl Logic for Slag {
    fn level(&self, max: u8) -> u8 {
        fill_level(u64::from(self.melt.amount), u64::from(SLAG_CAP), max)
    }
    fn view(&self) -> ViewModel {
        let line = match &self.melt.mat {
            Some(m) if self.melt.amount > 0 => format!("{m}: {}", quanta_name(self.melt.amount)),
            _ => "vazio".into(),
        };
        ViewModel::new("Escória").text(line, view::GREY)
    }
    /// D4: broken, it gives the exact scrap (`getDrops`: `ItemScraps.create(new MaterialStack(tile.mat, tile.amount))`).
    fn scraps(&self) -> Vec<(String, u32)> {
        self.melt.molten()
    }
}

impl Slotted for Slag {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        self.melt.synced(Tables::ntm(), SLAG_CAP)
    }
    fn save_state(&self) -> Vec<i32> {
        vec![Tables::ntm().mat_id(self.melt.mat.as_deref()), self.melt.amount as i32]
    }
    fn load_state(&mut self, v: &[i32]) {
        if let [id, q, ..] = v {
            self.melt.amount = (*q).max(0) as u32;
            self.melt.mat = Tables::ntm().mat_name(*id).filter(|_| self.melt.amount > 0);
            if self.melt.mat.is_none() {
                self.melt.amount = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_takes_its_own_material_up_to_sixteen_blocks() {
        let s = Slag::of("iron", 10_000);
        assert_eq!(s.takes("iron", 1_000), 368);
        assert_eq!(s.takes("iron", 100), 100);
        assert_eq!(s.takes("copper", 100), 0, "another material");
        assert_eq!(Slag::of("iron", SLAG_CAP).takes("iron", 5), 0, "full");
    }

    /// D4: broken, whatever it holds comes out whole (not in ingots: 1 and 71 quanta too).
    #[test]
    fn a_broken_slag_gives_every_quantum() {
        for q in [1, 71, 72, 648, 10_367, SLAG_CAP] {
            assert_eq!(Logic::scraps(&Slag::of("steel", q)), vec![("steel".to_string(), q)], "{q}");
        }
        assert!(Logic::scraps(&Slag::default()).is_empty());
    }

    #[test]
    fn its_height_is_its_fill_in_eighths() {
        let l = |q| Logic::level(&Slag::of("iron", q), LAYERS);
        assert_eq!((l(0), l(1), l(SLAG_CAP / 8), l(SLAG_CAP / 2), l(SLAG_CAP - 1), l(SLAG_CAP)), (0, 1, 1, 4, 7, 8));
    }

    #[test]
    fn its_state_goes_through_the_block_entity_whole() {
        let s = Slag::of("gold", 1_234);
        let mut back = Slag::default();
        back.load_state(&s.save_state());
        assert_eq!(back.melt, s.melt);
        assert_eq!(s.screen_data().len(), 3);
        // An empty record forgets its material.
        let mut e = Slag::of("gold", 5);
        e.load_state(&[Tables::ntm().mat_id(Some("gold")), 0]);
        assert_eq!(e.melt, Melt::default());
    }
}
