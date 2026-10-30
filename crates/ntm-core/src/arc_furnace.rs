//! The large arc furnace, bedrock stage 4 (docs/maquinas.md):
//! `tileentity/machine/TileEntityMachineArcFurnaceLarge.java`, its recipes
//! `ArcFurnaceRecipes` (solid and fluid modes), `ItemArcElectrode`.
//!
//! Without upgrades (the port has none yet): 1 000 HE a tick; a batch of
//! the whole 20-slot grid (one item a slot) in 400 ticks with the lid
//! closed (it closes in 60 ticks once there is something to do, and opens
//! again for 120 after). Three electrodes, each worn once a batch. In solid
//! mode an item becomes its output where it lies; in fluid mode the items
//! melt into a buffer of up to 82 944 quanta, which pours (72 a tick, the
//! lid open) onto a foundry block below the spout (`server/foundry_server`).

use crate::{
    crafter::Recipe,
    energy::{EnergyNode, Role},
    foundry::Mat,
    machine::Logic,
    server::container_bridge::{Slots, Slotted, Stack},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

pub mod slot {
    pub const ELECTRODES: std::ops::Range<usize> = 0..3;
    pub const BATTERY: usize = 3;
    pub const GRID: std::ops::Range<usize> = 5..25;
    pub const QUEUE: std::ops::Range<usize> = 25..30;
    pub const COUNT: usize = 30;
}

pub const MAX_POWER: u64 = 2_500_000;
pub const HE_PER_TICK: u64 = 1_000;
pub const TICKS: u32 = 400;
/// The lid, 0 (closed) to `LID` (open); a step a tick: 60 ticks.
pub const LID: u32 = 1_000;
pub const LID_STEP: u32 = 17;
pub const DELAY: u32 = 120;
pub const MAX_LIQUID: u32 = 82_944;
/// Quanta a tick of pouring (`INGOT.q(1)`).
pub const POUR: u32 = 72;

/// `ItemArcElectrode`: the electrodes and their durability (operations).
pub const ELECTRODE_TYPES: [(&str, u32); 4] = [("graphite", 10), ("lanthanium", 100), ("desh", 500), ("saturnite", 1_500)];

pub fn electrode(id: &str) -> Option<(&'static str, u32)> {
    let t = id.strip_prefix("ntm:arc_electrode__")?;
    ELECTRODE_TYPES.iter().find(|e| e.0 == t).copied()
}

#[derive(Deserialize)]
struct Table {
    recipes: Vec<Recipe>,
    #[serde(default)]
    scraps: HashMap<String, Vec<(String, u32)>>,
}

fn solid() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(include_str!("../data/machines/arc_furnace_solid.json")).expect("arc_furnace_solid.json"))
}

fn fluid() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(include_str!("../data/machines/arc_furnace_fluid.json")).expect("arc_furnace_fluid.json"))
}

/// Both modes' tables read now ([`crate::warm_tables`]).
pub fn warm() {
    solid();
    fluid();
}

/// What an item becomes in solid mode: (item, count).
pub fn solid_output(item: &str) -> Option<(String, u32)> {
    solid().recipes.iter().find(|r| r.items_in.first().is_some_and(|i| i.ids.iter().any(|x| x == item))).and_then(|r| r.items_out.first().map(|o| (o.0.clone(), o.1)))
}

/// What an item melts into in fluid mode (scraps into their own material).
pub fn fluid_output(item: &str) -> Option<Vec<(String, u32)>> {
    let t = fluid();
    if let Some(m) = t.scraps.get(item) {
        return Some(m.clone());
    }
    t.recipes.iter().find(|r| r.items_in.first().is_some_and(|i| i.ids.iter().any(|x| x == item))).map(|r| r.mout.clone()).filter(|m| !m.is_empty())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcFurnace {
    /// The engine's slots, laid over on the first tick; a machine loaded
    /// from the save starts with them empty but the right length.
    #[serde(skip, default = "empty_slots")]
    pub slots: Slots,
    pub power: u64,
    pub progress: u32,
    pub lid: u32,
    pub delay: u32,
    #[serde(default)]
    pub liquid_mode: bool,
    /// The molten buffer, in order (the first pours first).
    #[serde(default)]
    pub liquids: Vec<Mat>,
    #[serde(skip)]
    pub progressing: bool,
}

fn empty_slots() -> Slots {
    vec![None; slot::COUNT]
}

impl Default for ArcFurnace {
    fn default() -> Self {
        Self { slots: vec![None; slot::COUNT], power: 0, progress: 0, lid: LID, delay: 0, liquid_mode: false, liquids: Vec::new(), progressing: false }
    }
}

impl ArcFurnace {
    fn has_recipe(&self, item: &str) -> bool {
        if self.liquid_mode { fluid_output(item).is_some() } else { solid_output(item).is_some() }
    }

    /// `hasIngredients`: any grid item with a recipe in the mode.
    pub fn has_ingredients(&self) -> bool {
        self.slots[slot::GRID].iter().flatten().any(|s| self.has_recipe(&s.id))
    }

    /// `hasElectrodes`: three unburnt electrodes.
    pub fn has_electrodes(&self) -> bool {
        self.slots[slot::ELECTRODES].iter().all(|s| s.as_ref().is_some_and(|s| electrode(&s.id).is_some()))
    }

    fn liquid_total(&self) -> u32 {
        self.liquids.iter().map(|m| m.1).sum()
    }

    /// `loadIngredients`: the queue into the grid, one item a slot (the
    /// most a slot holds without speed upgrades).
    fn load(&mut self) -> bool {
        let mut moved = false;
        for q in slot::QUEUE {
            while let Some(s) = self.slots[q].clone() {
                if !self.has_recipe(&s.id) {
                    break;
                }
                let Some(g) = slot::GRID.clone().find(|&g| self.slots[g].is_none()) else { return moved };
                self.slots[g] = Some(Stack { count: 1, ..s.clone() });
                let left = s.count - 1;
                self.slots[q] = (left > 0).then_some(Stack { count: left, ..s });
                moved = true;
            }
        }
        moved
    }

    /// `process`: the whole grid; each electrode worn once.
    fn process(&mut self) {
        for g in slot::GRID {
            let Some(s) = self.slots[g].clone() else { continue };
            if self.liquid_mode {
                let Some(outs) = fluid_output(&s.id) else { continue };
                let need: u32 = outs.iter().map(|o| o.1).sum();
                let mut n = s.count;
                while n > 0 && self.liquid_total() + need <= MAX_LIQUID {
                    for (m, q) in &outs {
                        match self.liquids.iter_mut().find(|x| x.0 == *m) {
                            Some(x) => x.1 += q,
                            None => self.liquids.push((m.clone(), *q)),
                        }
                    }
                    n -= 1;
                }
                self.slots[g] = (n > 0).then_some(Stack { count: n, ..s });
            } else if let Some((id, k)) = solid_output(&s.id) {
                self.slots[g] = Some(Stack::new(&id, k * s.count));
            }
        }
        for e in slot::ELECTRODES {
            if let Some(s) = &mut self.slots[e]
                && let Some((t, max)) = electrode(&s.id)
            {
                s.damage += 1;
                if s.damage >= max {
                    self.slots[e] = Some(Stack::new(&format!("ntm:arc_electrode_burnt__{t}"), 1));
                }
            }
        }
    }

    /// One tick (`updateEntity` but the pouring, the server's): true when
    /// something changed.
    pub fn step(&mut self) -> bool {
        let before = (self.progress, self.lid, self.delay, self.power, self.liquids.len());
        self.power = crate::machine::charge_from_slot(&self.slots, slot::BATTERY, self.power, MAX_POWER);
        self.progressing = false;
        let mut moved = false;
        if self.lid > 0 {
            moved = self.load();
        }
        if self.power > 0 {
            let run = self.has_ingredients() && self.has_electrodes() && self.delay == 0 && self.liquids.is_empty();
            if run {
                if self.lid > 0 {
                    self.lid = self.lid.saturating_sub(LID_STEP);
                    self.progress = 0;
                } else if self.power >= HE_PER_TICK {
                    self.progress += 1;
                    self.progressing = true;
                    self.power -= HE_PER_TICK;
                    if self.progress >= TICKS {
                        self.process();
                        self.progress = 0;
                        self.delay = DELAY;
                        moved = true;
                    }
                }
            } else {
                self.delay = self.delay.saturating_sub(1);
                self.progress = 0;
                self.lid = (self.lid + LID_STEP).min(LID);
            }
        }
        moved || before != (self.progress, self.lid, self.delay, self.power, self.liquids.len())
    }

    /// Up to `POUR` quanta of the first stack `accept` takes (it returns
    /// what's left, `None` when it takes none): the lid open only.
    pub fn pour(&mut self, mut accept: impl FnMut(&str, u32) -> Option<u32>) -> bool {
        if self.lid == 0 {
            return false;
        }
        for i in 0..self.liquids.len() {
            let (m, q) = self.liquids[i].clone();
            let n = q.min(POUR);
            if let Some(left) = accept(&m, n) {
                self.liquids[i].1 -= n - left.min(n);
                self.liquids.retain(|x| x.1 > 0);
                return true;
            }
        }
        false
    }

    fn electrode_state(&self, i: usize) -> i32 {
        match self.slots[slot::ELECTRODES.start + i].as_ref() {
            None => 0,
            Some(s) if s.id.starts_with("ntm:arc_electrode_burnt__") => 3,
            Some(s) if self.progressing || s.damage > 0 => 2,
            Some(_) => 1,
        }
    }

    pub fn screen_data(&self) -> Vec<i32> {
        let mut v = vec![0i32; 29];
        v[0] = self.power.min(MAX_POWER) as i32;
        v[1] = MAX_POWER as i32;
        v[2] = self.progress as i32;
        v[3] = TICKS as i32;
        v[10] = i32::from(self.progressing);
        for i in 0..3 {
            v[20 + i] = self.electrode_state(i);
        }
        v[23] = if self.progressing { 2 } else { i32::from(self.has_ingredients()) };
        v[24] = self.lid as i32;
        v[25] = self.liquid_total() as i32;
        v[26] = MAX_LIQUID as i32;
        v[27] = self.liquids.last().map_or(0, |m| crate::foundry::Tables::ntm().color(&m.0) as i32);
        v[28] = i32::from(self.liquid_mode);
        v
    }
}

pub mod buttons {
    pub const MODE: u32 = 0;
}

impl EnergyNode for ArcFurnace {
    fn role(&self) -> Role {
        Role::Consumer
    }
    fn offer(&self) -> u64 {
        0
    }
    fn demand(&self) -> u64 {
        MAX_POWER.saturating_sub(self.power)
    }
    fn give(&mut self, he: u64) {
        self.power += he;
    }
    fn take(&mut self, _he: u64) {}
}

impl Logic for ArcFurnace {
    fn tick(&mut self) -> bool {
        self.step()
    }
    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        Some(self)
    }
    fn level(&self, max: u8) -> u8 {
        if self.progressing { max } else { 0 }
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Forno a arco grande")
            .bar("Energia", self.power, MAX_POWER, "HE")
            .bar("Progresso", u64::from(self.progress), u64::from(TICKS), "ticks")
            .text(format!("Modo: {}", if self.liquid_mode { "fundido" } else { "sólido" }), view::GREY)
            .text("Os itens e o botão ficam na tela com slots (motor com o patch ntm-visual).", view::GREY)
    }
}

impl Slotted for ArcFurnace {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        ArcFurnace::screen_data(self)
    }
    fn button(&mut self, id: u32) -> bool {
        if id == buttons::MODE {
            self.liquid_mode = !self.liquid_mode;
        }
        false
    }
    /// The molten buffer, as scraps, when it is broken.
    fn held_contents(&self) -> Vec<(String, u32)> {
        self.liquids.iter().filter(|m| m.1 >= 72).map(|m| (format!("ntm:scraps__{}", m.0), m.1 / 72)).collect()
    }
    /// Progress, lid, delay, the mode and the buffer (material id, quanta).
    fn save_state(&self) -> Vec<i32> {
        let t = crate::foundry::Tables::ntm();
        let mut v = vec![self.progress as i32, self.lid as i32, self.delay as i32, i32::from(self.liquid_mode)];
        t.encode_stack(&self.liquids, &mut v);
        v
    }
    fn load_state(&mut self, v: &[i32]) {
        let t = crate::foundry::Tables::ntm();
        let mut it = v.iter();
        let (Some(&p), Some(&l), Some(&d), Some(&m)) = (it.next(), it.next(), it.next(), it.next()) else { return };
        self.progress = p.clamp(0, TICKS as i32) as u32;
        self.lid = l.clamp(0, LID as i32) as u32;
        self.delay = d.clamp(0, DELAY as i32) as u32;
        self.liquid_mode = m != 0;
        if let Some(s) = t.decode_stack(&mut it) {
            self.liquids = s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready(f: &mut ArcFurnace) {
        f.slots[0] = Some(Stack::new("ntm:arc_electrode__graphite", 1));
        f.slots[1] = Some(Stack::new("ntm:arc_electrode__graphite", 1));
        f.slots[2] = Some(Stack::new("ntm:arc_electrode__graphite", 1));
        f.slots[slot::BATTERY] = Some(Stack::new(crate::machine::CREATIVE_BATTERY, 1));
    }

    /// Loaded from the save (its slots not kept there), it ticks before the
    /// engine's slots come: in game that panicked the whole server mod.
    #[test]
    fn a_furnace_loaded_from_the_save_ticks() {
        let mut f: ArcFurnace = serde_json::from_str(&serde_json::to_string(&ArcFurnace::default()).unwrap()).unwrap();
        assert_eq!(f.slots.len(), slot::COUNT);
        f.step();
        f.screen_data();
    }

    #[test]
    fn a_batch_closes_the_lid_takes_400_ticks_and_wears_the_electrodes() {
        let mut f = ArcFurnace::default();
        ready(&mut f);
        f.slots[25] = Some(Stack::new("minecraft:sand", 3));
        let mut t = 0;
        while f.slots[slot::GRID].iter().flatten().all(|s| s.id == "minecraft:sand") && t < 2_000 {
            f.step();
            t += 1;
        }
        // 60 ticks of lid (1000 / 17 → 59 steps) and 400 of work.
        assert!((455..=465).contains(&t), "{t} ticks");
        let grid: Vec<_> = f.slots[slot::GRID].iter().flatten().cloned().collect();
        assert_eq!(grid, vec![Stack::new("ntm:nugget_silicon", 1); 3]);
        assert!(f.slots[0..3].iter().all(|s| s.as_ref().is_some_and(|s| s.damage == 1)));
        // Graphite lasts 10 batches: the 10th burns them.
        for e in 0..3 {
            f.slots[e].as_mut().unwrap().damage = 9;
        }
        f.slots[25] = Some(Stack::new("minecraft:sand", 1));
        f.slots[5] = None;
        for _ in 0..1_000 {
            f.step();
        }
        assert_eq!(f.slots[0], Some(Stack::new("ntm:arc_electrode_burnt__graphite", 1)));
        assert!(!f.has_electrodes());
    }

    #[test]
    fn fluid_mode_melts_into_a_buffer_that_pours_with_the_lid_open() {
        let mut f = ArcFurnace::default();
        ready(&mut f);
        f.button(buttons::MODE);
        f.slots[25] = Some(Stack::new("ntm:bedrock_ore__primary_first_light", 2));
        while f.liquids.is_empty() {
            f.step();
        }
        assert_eq!(f.liquids, vec![("iron".to_string(), 720), ("copper".to_string(), 288)]);
        // Just done, the lid shut: no pouring; then it opens (the buffer
        // isn't empty) and pours 72 at a time.
        assert!(!f.pour(|_, _| Some(0)), "a tampa ainda fechada");
        for _ in 0..60 {
            f.step();
        }
        let mut got = Vec::new();
        while f.pour(|m, q| {
            got.push((m.to_string(), q));
            Some(0)
        }) {}
        assert_eq!(got.iter().filter(|g| g.0 == "iron").map(|g| g.1).sum::<u32>(), 720);
        assert_eq!(got.iter().filter(|g| g.0 == "copper").map(|g| g.1).sum::<u32>(), 288);
        assert!(f.liquids.is_empty());
    }
}
