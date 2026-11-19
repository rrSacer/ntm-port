//! The electrolyser, bedrock stages 3 and 4 (docs/maquinas.md):
//! `tileentity/machine/TileEntityElectrolyser.java`. Two lines on one
//! power: the fluid line (`ElectrolyserFluidRecipes`, a crafter station:
//! `crate::crafter::Station::Electrolyser`) and the metal line
//! (`ElectrolyserMetalRecipes`): an item in slot 14 and 100 mB of nitric
//! acid make two molten stacks (left and right, up to 10 368 quanta each),
//! poured at its two ends (`server/foundry_server`), and byproducts in
//! slots 15–20, each in its own slot.

use crate::{
    crafter::{Crafter, Recipe, Station},
    energy::{EnergyNode, Role},
    fluid::FluidNode,
    foundry::Mat,
    machine::Logic,
    server::container_bridge::{Slots, Slotted, Stack},
    view::ViewModel,
};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub mod slot {
    pub const BATTERY: usize = 0;
    pub const FLUID_OUT: std::ops::Range<usize> = 11..14;
    pub const METAL_IN: usize = 14;
    pub const METAL_OUT: usize = 15;
    pub const COUNT: usize = 21;
}

/// `maxMaterial`: a molten stack's capacity (`BLOCK.q(16)`).
pub const MAX_MATERIAL: u32 = 10_368;
pub const NITRIC: u32 = 100;
pub const METAL_POWER: u64 = 10_000;
/// Quanta a tick a stack pours (`NUGGET.q(3)`).
pub const POUR: u32 = 24;

#[derive(Deserialize)]
struct Table {
    recipes: Vec<Recipe>,
}

pub fn metal_recipes() -> &'static [Recipe] {
    static T: OnceLock<Table> = OnceLock::new();
    &T.get_or_init(|| serde_json::from_str(include_str!("../data/machines/electrolyser_metal.json")).expect("electrolyser_metal.json")).recipes
}

pub fn metal_recipe(item: &str) -> Option<&'static Recipe> {
    metal_recipes().iter().find(|r| r.items_in.first().is_some_and(|i| i.ids.iter().any(|x| x == item)))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Electrolyser {
    /// The fluid line, its tanks (the 2nd input is the metal line's nitric
    /// acid) and the power both lines draw.
    pub fluid: Crafter,
    /// The engine's slots, laid over on the first tick; a machine loaded
    /// from the save starts with them empty but the right length.
    #[serde(skip, default = "empty_slots")]
    pub slots: Slots,
    #[serde(default)]
    pub progress_metal: u32,
    #[serde(default)]
    pub left: Option<Mat>,
    #[serde(default)]
    pub right: Option<Mat>,
}

fn empty_slots() -> Slots {
    vec![None; slot::COUNT]
}

impl Default for Electrolyser {
    fn default() -> Self {
        Self { fluid: Crafter::new(Station::Electrolyser), slots: vec![None; slot::COUNT], progress_metal: 0, left: None, right: None }
    }
}

fn fits(stack: &Option<Mat>, out: Option<&(String, u32)>) -> bool {
    match (stack, out) {
        (Some(s), Some(o)) => s.0 == o.0 && s.1 + o.1 <= MAX_MATERIAL,
        _ => true,
    }
}

fn add(stack: &mut Option<Mat>, out: Option<&(String, u32)>) {
    if let Some((m, q)) = out {
        match stack {
            Some(s) => s.1 += q,
            None => *stack = Some((m.clone(), *q)),
        }
    }
}

impl Electrolyser {
    fn metal(&self) -> Option<&'static Recipe> {
        self.slots.get(slot::METAL_IN).and_then(|s| s.as_ref()).and_then(|s| metal_recipe(&s.id))
    }

    /// `canProcessMetal`.
    pub fn can_metal(&self) -> bool {
        let Some(r) = self.metal() else { return false };
        let input = self.slots[slot::METAL_IN].as_ref().map_or(0, |s| s.count);
        self.fluid.power >= METAL_POWER
            && self.fluid.tin[1].mb >= NITRIC
            && input >= r.items_in.first().map_or(1, |i| i.n)
            && fits(&self.left, r.mout.first())
            && fits(&self.right, r.mout.get(1))
            && r.items_out.iter().enumerate().all(|(i, (id, n, _))| match &self.slots[slot::METAL_OUT + i] {
                None => true,
                Some(s) => s.id == *id && s.count + n <= 64,
            })
    }

    /// `processMetal`.
    fn finish_metal(&mut self, r: &Recipe) {
        add(&mut self.left, r.mout.first());
        add(&mut self.right, r.mout.get(1));
        for (i, (id, n, _)) in r.items_out.iter().enumerate() {
            let s = &mut self.slots[slot::METAL_OUT + i];
            match s {
                Some(x) => x.count += n,
                None => *s = Some(Stack::new(id, *n)),
            }
        }
        self.fluid.tin[1].drain(NITRIC);
        if let Some(s) = &mut self.slots[slot::METAL_IN] {
            s.count -= r.items_in.first().map_or(1, |i| i.n);
            if s.count == 0 {
                self.slots[slot::METAL_IN] = None;
            }
        }
    }

    /// The metal line's tick: progress kept when it can't run, as the NTM.
    fn step_metal(&mut self) -> bool {
        if !self.can_metal() {
            return false;
        }
        let Some(r) = self.metal() else { return false };
        self.progress_metal += 1;
        self.fluid.power -= METAL_POWER;
        if self.progress_metal >= r.duration.max(1) {
            self.finish_metal(r);
            self.progress_metal = 0;
        }
        true
    }

    /// Up to `POUR` quanta of the left or right stack, to `accept` (what's
    /// left, `None` when it takes none).
    pub fn pour(&mut self, right: bool, accept: impl FnOnce(&str, u32) -> Option<u32>) -> bool {
        let stack = if right { &mut self.right } else { &mut self.left };
        let Some((m, q)) = stack.clone() else { return false };
        let n = q.min(POUR);
        let Some(left) = accept(&m, n) else { return false };
        let rest = q - (n - left.min(n));
        *stack = (rest > 0).then_some((m, rest));
        true
    }

    pub fn screen_data(&self) -> Vec<i32> {
        let t = crate::foundry::Tables::ntm();
        let mut v = crate::server::container_bridge::crafter_data(&self.fluid);
        v.resize(30, 0);
        let stack = |s: &Option<Mat>| s.as_ref().map_or([0, 0], |(m, q)| [*q as i32, t.color(m) as i32]);
        v.extend(stack(&self.left));
        v.extend(stack(&self.right));
        let duration = self.metal().map_or(600, |r| r.duration);
        v.extend([MAX_MATERIAL as i32, self.progress_metal as i32, duration as i32, i32::from(self.fluid.power >= METAL_POWER)]);
        v
    }
}

impl EnergyNode for Electrolyser {
    fn role(&self) -> Role {
        Role::Consumer
    }
    fn offer(&self) -> u64 {
        0
    }
    fn demand(&self) -> u64 {
        self.fluid.max_power().saturating_sub(self.fluid.power)
    }
    fn give(&mut self, he: u64) {
        self.fluid.power += he;
    }
    fn take(&mut self, _he: u64) {}
}

impl Logic for Electrolyser {
    /// The molten stacks, one scrap each when it is broken.
    fn scraps(&self) -> Vec<(String, u32)> {
        [&self.left, &self.right].into_iter().flatten().filter(|m| m.1 > 0).cloned().collect()
    }
    fn tick(&mut self) -> bool {
        self.fluid.power = crate::machine::charge_from_slot(&self.slots, slot::BATTERY, self.fluid.power, self.fluid.max_power());
        let a = self.fluid.tick();
        let b = self.step_metal();
        a || b
    }
    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        Some(self)
    }
    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(&mut self.fluid)
    }
    fn level(&self, max: u8) -> u8 {
        if self.progress_metal > 0 || self.fluid.progress > 0 { max } else { 0 }
    }
    fn view(&self) -> ViewModel {
        self.fluid.view()
    }
}

impl Slotted for Electrolyser {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    /// The fluid line's byproducts (11–13) are its pile.
    fn sync_in(&mut self) {
        self.fluid.out = self.slots[slot::FLUID_OUT].iter().flatten().fold(Vec::new(), |mut acc: Vec<(String, u32)>, s| {
            match acc.iter_mut().find(|x| x.0 == s.id) {
                Some(x) => x.1 += s.count,
                None => acc.push((s.id.clone(), s.count)),
            }
            acc
        });
    }
    fn sync_out(&mut self) {
        self.slots.resize(slot::COUNT, None);
        crate::server::container_bridge::lay_out(&mut self.slots, slot::FLUID_OUT, &self.fluid.out, crate::crafter::STACK);
    }
    fn screen_data(&self) -> Vec<i32> {
        Electrolyser::screen_data(self)
    }
    /// The metal line's progress and stacks.
    fn save_state(&self) -> Vec<i32> {
        let t = crate::foundry::Tables::ntm();
        let mut v = vec![self.progress_metal as i32];
        for s in [&self.left, &self.right] {
            v.extend(s.as_ref().map_or([0, 0], |(m, q)| [t.mat_id(Some(m)), *q as i32]));
        }
        v
    }
    fn load_state(&mut self, v: &[i32]) {
        let t = crate::foundry::Tables::ntm();
        if let [p, lm, lq, rm, rq, ..] = v {
            self.progress_metal = (*p).max(0) as u32;
            self.left = t.mat_name(*lm).filter(|_| *lq > 0).map(|m| (m, (*lq) as u32));
            self.right = t.mat_name(*rm).filter(|_| *rq > 0).map(|m| (m, (*rq) as u32));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bedrock ore (primary, first): 100 mB of nitric acid, 20 ticks, iron
    /// to the left, copper to the right, crumbs in slot 15.
    #[test]
    fn the_metal_line_makes_two_molten_stacks_and_pours_them() {
        let mut e = Electrolyser::default();
        e.slots[slot::BATTERY] = Some(Stack::new(crate::machine::CREATIVE_BATTERY, 1));
        e.slots[slot::METAL_IN] = Some(Stack::new("ntm:bedrock_ore__primary_first_light", 2));
        e.fluid.fluid_give("nitric_acid", 1_000);
        assert_eq!((e.fluid.tin[0].mb, e.fluid.tin[1].mb), (0, 1_000), "o ácido nítrico vai ao tanque dele");
        for _ in 0..40 {
            e.tick();
        }
        assert_eq!(e.left, Some(("iron".into(), 1_152)));
        assert_eq!(e.right, Some(("copper".into(), 576)));
        assert_eq!(e.slots[slot::METAL_OUT], Some(Stack::new("ntm:bedrock_ore__crumbs_light", 2)));
        assert_eq!((e.slots[slot::METAL_IN].clone(), e.fluid.tin[1].mb), (None, 800));
        let mut iron = 0;
        while e.pour(false, |m, q| {
            assert_eq!(m, "iron");
            iron += q;
            Some(0)
        }) {}
        assert_eq!((iron, e.left.clone()), (1_152, None));
        // No target: nothing pours, nothing is lost.
        assert!(!e.pour(true, |_, _| None));
        assert_eq!(e.right, Some(("copper".into(), 576)));
    }

    /// Loaded from the save, it ticks before the engine's slots come.
    #[test]
    fn an_electrolyser_loaded_from_the_save_ticks() {
        let mut e: Electrolyser = serde_json::from_str(&serde_json::to_string(&Electrolyser::default()).unwrap()).unwrap();
        assert_eq!(e.slots.len(), slot::COUNT);
        e.tick();
        e.screen_data();
    }

    /// Without nitric acid, or a stack of another metal, it waits.
    #[test]
    fn the_metal_line_waits_for_acid_and_for_room() {
        let mut e = Electrolyser::default();
        e.slots[slot::BATTERY] = Some(Stack::new(crate::machine::CREATIVE_BATTERY, 1));
        e.slots[slot::METAL_IN] = Some(Stack::new("ntm:bedrock_ore__primary_first_light", 1));
        e.tick();
        assert!(!e.can_metal(), "sem ácido");
        e.fluid.fluid_give("nitric_acid", 1_000);
        e.left = Some(("gold".into(), 72));
        assert!(!e.can_metal(), "outra liga na pilha esquerda");
        e.left = None;
        assert!(e.can_metal());
    }
}
