//! Molds and basins, stage 3 (docs/maquinas.md, "Fundição"):
//! `tileentity/machine/TileEntityFoundryCastingBase.java` and
//! `items/machine/ItemMold.java`. Two slots (the engine's): the mold and
//! the cast. The mold sets what it holds (its cost in quanta) and what it
//! makes of each material; full, the metal cools for 200 ticks into the
//! cast. A mold block takes small molds (size 0) and metal from the sides
//! or from above; a basin takes the large ones (size 1) and metal from
//! above only.

use super::{Tables, flow::Melt, quanta_name};
use crate::{
    machine::{Logic, fill_level},
    server::container_bridge::{Slots, Slotted, Stack},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const SLOTS: usize = 2;
pub const MOLD: usize = 0;
pub const OUT: usize = 1;
/// Ticks a full cast cools (`cooloff`).
pub const COOL: u32 = 200;

/// A mold (`ItemMold.Mold`): its item, size, cost and what each material
/// casts into.
#[derive(Clone, Debug, Deserialize)]
pub struct Mold {
    pub id: u32,
    pub size: u8,
    pub name: String,
    pub item: String,
    pub cost: u32,
    #[serde(default)]
    pub outputs: HashMap<String, (String, u32)>,
}

impl Tables {
    pub fn mold(&self, item: &str) -> Option<&Mold> {
        self.molds.iter().find(|m| m.item == item)
    }
}

/// A casting base: a mold block (`size` 0) or a basin (1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cast {
    pub slots: Slots,
    #[serde(flatten)]
    pub melt: Melt,
    pub cooloff: u32,
    pub size: u8,
}

impl Cast {
    pub fn new(size: u8) -> Self {
        Self { slots: vec![None; SLOTS], melt: Melt::default(), cooloff: COOL, size }
    }

    /// `getInstalledMold`: the mold in its slot, if its size fits.
    pub fn mold<'a>(&self, t: &'a Tables) -> Option<&'a Mold> {
        self.slots[MOLD].as_ref().and_then(|s| t.mold(&s.id)).filter(|m| m.size == self.size)
    }

    pub fn capacity(&self, t: &Tables) -> u32 {
        self.mold(t).map_or(0, |m| m.cost)
    }

    /// `standardCheck` of a casting base: the base rules, the cast slot
    /// empty, a mold that makes something of `mat`.
    pub fn check(&self, t: &Tables, mat: &str) -> bool {
        self.melt.check(mat, self.capacity(t)) && self.slots[OUT].is_none() && self.mold(t).is_some_and(|m| m.outputs.contains_key(mat))
    }

    /// Metal in (from above, or from a side for a mold block): what is left.
    pub fn accept(&mut self, t: &Tables, mat: &str, q: u32) -> Option<u32> {
        if !self.check(t, mat) {
            return None;
        }
        let cap = self.capacity(t);
        Some(self.melt.add(mat, q, cap))
    }

    /// One tick (`updateEntity`); true when the cast came out.
    pub fn step(&mut self, t: &Tables) -> bool {
        let cap = self.capacity(t);
        self.melt.amount = self.melt.amount.min(cap);
        if self.melt.amount == 0 {
            self.melt.mat = None;
        }
        let out = self.mold(t).and_then(|m| self.melt.mat.as_ref().and_then(|x| m.outputs.get(x)).cloned());
        if self.mold(t).is_some() && cap > 0 && self.melt.amount == cap && self.slots[OUT].is_none() {
            self.cooloff = self.cooloff.saturating_sub(1);
            if self.cooloff == 0 {
                self.melt.amount = 0;
                self.slots[OUT] = out.map(|(id, n)| Stack::new(&id, n));
                self.cooloff = COOL;
                return true;
            }
        } else {
            self.cooloff = COOL;
        }
        false
    }

    /// The values for its model and screen: quanta, capacity, molten
    /// colour, cooling left, its maximum.
    pub fn screen_data(&self, t: &Tables) -> Vec<i32> {
        let mut v = self.melt.synced(t, self.capacity(t));
        v.extend([self.cooloff as i32, COOL as i32]);
        v
    }
}

impl Logic for Cast {
    fn tick(&mut self) -> bool {
        self.step(Tables::ntm())
    }

    fn level(&self, max: u8) -> u8 {
        fill_level(u64::from(self.melt.amount), u64::from(self.capacity(Tables::ntm()).max(1)), max)
    }

    /// The mold and the cast go through the engine's slots.
    fn view(&self) -> ViewModel {
        let t = Tables::ntm();
        let mold = self.mold(t).map_or("nenhum (ou do tamanho errado)".to_string(), |m| m.name.clone());
        let metal = match &self.melt.mat {
            Some(m) if self.melt.amount > 0 => format!("{m}: {} de {}", quanta_name(self.melt.amount), quanta_name(self.capacity(t))),
            _ => "vazio".into(),
        };
        ViewModel::new(if self.size == 0 { "Molde de fundição" } else { "Bacia de fundição" })
            .text(format!("Molde: {mold}"), view::GREY)
            .text(format!("Metal: {metal}"), view::GREY)
    }

    /// The mold, the cast and the metal as scraps (an ingot's worth each).
    fn contents(&self) -> Vec<(String, u32)> {
        let mut out: Vec<(String, u32)> = self.slots.iter().flatten().map(|s| (s.id.clone(), s.count)).collect();
        if let Some(m) = &self.melt.mat
            && self.melt.amount >= 72
        {
            out.push((format!("ntm:scraps__{m}"), self.melt.amount / 72));
        }
        out
    }
}

impl Slotted for Cast {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        Cast::screen_data(self, Tables::ntm())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundry::testing::tables;

    fn with_molds() -> Tables {
        let mut t = tables();
        t.molds = vec![
            Mold {
                id: 2,
                size: 0,
                name: "ingot".into(),
                item: "ntm:mold__ingot".into(),
                cost: 72,
                outputs: [("iron".to_string(), ("minecraft:iron_ingot".to_string(), 1)), ("steel".to_string(), ("ntm:ingot_steel".to_string(), 1))].into_iter().collect(),
            },
            Mold {
                id: 12,
                size: 1,
                name: "block".into(),
                item: "ntm:mold__block".into(),
                cost: 648,
                outputs: [("iron".to_string(), ("minecraft:iron_block".to_string(), 1))].into_iter().collect(),
            },
        ];
        t
    }

    #[test]
    fn an_ingot_mold_casts_after_200_ticks() {
        let t = with_molds();
        let mut c = Cast::new(0);
        assert_eq!(c.accept(&t, "iron", 24), None, "sem molde");
        c.slots[MOLD] = Some(Stack::new("ntm:mold__ingot", 1));
        assert_eq!(c.accept(&t, "copper", 24), None, "o molde não faz cobre");
        assert_eq!(c.accept(&t, "iron", 48), Some(0));
        assert_eq!(c.accept(&t, "iron", 48), Some(24), "cabe só 72");
        for _ in 0..199 {
            assert!(!c.step(&t));
        }
        assert!(c.step(&t), "200 ticks");
        assert_eq!(c.slots[OUT], Some(Stack::new("minecraft:iron_ingot", 1)));
        assert_eq!(c.melt.amount, 0);
        // The cast still in its slot: no more metal.
        assert_eq!(c.accept(&t, "iron", 24), None);
    }

    #[test]
    fn a_basin_takes_large_molds_only() {
        let t = with_molds();
        let mut b = Cast::new(1);
        b.slots[MOLD] = Some(Stack::new("ntm:mold__ingot", 1));
        assert_eq!(b.capacity(&t), 0, "molde pequeno na bacia");
        b.slots[MOLD] = Some(Stack::new("ntm:mold__block", 1));
        assert_eq!(b.capacity(&t), 648);
        assert_eq!(b.accept(&t, "iron", 24), Some(0));
    }

    #[test]
    fn the_ntm_molds_are_embedded() {
        let t = Tables::ntm();
        assert_eq!(t.molds.len(), 26);
        let ingot = t.mold("ntm:mold__ingot").expect("o molde de lingote");
        assert_eq!((ingot.size, ingot.cost), (0, 72));
        assert_eq!(ingot.outputs.get("steel").map(|o| o.0.as_str()), Some("ntm:ingot_steel"));
        let barrel = t.molds.iter().find(|m| m.name == "barrel_heavy").unwrap();
        assert_eq!(barrel.cost, 432);
    }
}
