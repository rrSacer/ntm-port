//! The continuous caster (`ntm:machine_strand_caster`, docs/content-foundry.md):
//! `tileentity/machine/TileEntityMachineStrandCaster.java` and
//! `blocks/machine/MachineStrandCaster.java`.
//!
//! The model is `TileEntityMachineStrandCaster` over `TileEntityFoundryCastingBase`: seven slots (the mold
//! and six outputs), the molten metal (`type`, `amount`), water 64 000 and spent steam 64 000, a batch cast when
//! 9 molds are ready or 200 ticks after the last progress, and the excess a lighter mold leaves as one exact scrap
//! at `y + 2` (F5, through [`Logic::world_effects`]).
//!
//! Besides, it holds what a jet of molten metal needs to find it: the four funnel cells above its core
//! (`getMetalPourPos`, `MachineStrandCaster.fillSpace` makes them the `moltenMetal` proxies) and the face rule of
//! `canAcceptPartialPour`.

use crate::{
    Pos,
    fluid::{FluidNode, Tank},
    foundry::{Tables, flow::Melt, quanta_name, scrap},
    machine::{Logic, MachineEffect, fill_level},
    registry::facing_dir,
    server::container_bridge::{Slots, Slotted, Stack},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub const BLOCK: &str = "ntm:machine_strand_caster";

/// `getMetalPourPos` (`TileEntityMachineStrandCaster.java:204-209`): with
/// `rot = dir.getRotation(UP)`, the cells at `y + 2` over `core + rot - dir`,
/// `core - dir`, `core + rot` and `core`.
pub fn pour_cells(core: Pos, facing: &str) -> Vec<Pos> {
    let Some(d) = facing_dir(facing) else { return Vec::new() };
    let rot = [-d[2], 0, d[0]];
    let at = |r: i32, k: i32| [core[0] + rot[0] * r - d[0] * k, core[1] + 2, core[2] + rot[2] * r - d[2] * k];
    vec![at(1, 1), at(0, 1), at(1, 0), at(0, 0)]
}

/// Where a jet struck: the core of the machine, its facing, the cell and the face.
pub struct Jet<'a> {
    pub core: Pos,
    pub facing: &'a str,
    pub cell: Pos,
    pub face: [i32; 3],
}

/// The geometry half of `canAcceptPartialPour`
/// (`TileEntityMachineStrandCaster.java:211-224`): only the face UP, and only
/// one of the four funnel cells.
pub fn takes_pour_at(core: Pos, facing: &str, cell: Pos, face: [i32; 3]) -> bool {
    face == [0, 1, 0] && pour_cells(core, facing).contains(&cell)
}


pub mod slot {
    pub const MOLD: usize = 0;
    /// The six outputs, `1..7` (`getAccessibleSlotsFromSide`).
    pub const OUT: std::ops::Range<usize> = 1..7;
    pub const COUNT: usize = 7;
}

pub const WATER_CAP: u32 = 64_000;
pub const STEAM_CAP: u32 = 64_000;
/// The capacity with no mold (`getCapacity`:229).
pub const NO_MOLD_CAP: u32 = 50_000;
/// Ticks without progress before a partial batch is cast (`updateEntity`:93).
pub const IDLE_FLUSH: u64 = 200;
/// What a stack of an output holds: the molds make ingots, plates, wires and blocks, all 64.
const STACK: u32 = 64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StrandCaster {
    /// The engine's slots (the mold, then the outputs).
    #[serde(skip, default = "empty_slots")]
    pub slots: Slots,
    #[serde(flatten)]
    pub melt: Melt,
    pub water: Tank,
    pub steam: Tank,
    /// `lastProgressTick`, the NBT `t`: the world tick of the last cast or overflow.
    #[serde(default)]
    pub last_progress: u64,
    /// The world's tick now, set by the server before each tick.
    #[serde(skip)]
    pub now: u64,
    /// What the last tick puts into the world (the excess scrap), drained by `world_effects`.
    #[serde(skip)]
    pub pending: Vec<MachineEffect>,
}

fn empty_slots() -> Slots {
    vec![None; slot::COUNT]
}

impl Default for StrandCaster {
    fn default() -> Self {
        Self {
            slots: empty_slots(),
            melt: Melt::default(),
            water: Tank::typed("water", WATER_CAP),
            steam: Tank::typed("spentsteam", STEAM_CAP),
            last_progress: 0,
            now: 0,
            pending: Vec::new(),
        }
    }
}

impl StrandCaster {
    /// `getInstalledMold`: any mold item in slot 0, whatever its size.
    pub fn mold<'a>(&self, t: &'a Tables) -> Option<&'a crate::foundry::cast::Mold> {
        self.slots.get(slot::MOLD).and_then(|s| s.as_ref()).and_then(|s| t.mold(&s.id))
    }

    /// `getCapacity`: ten molds' worth, 50 000 with none.
    pub fn capacity(&self, t: &Tables) -> u32 {
        self.mold(t).map_or(NO_MOLD_CAP, |m| m.cost * 10)
    }

    /// What it accepts (`standardCheck`:226-233): nine molds' worth with a mold, and only the material it holds.
    fn limit(&self, t: &Tables) -> u32 {
        self.mold(t).map_or_else(|| self.capacity(t), |m| m.cost * 9)
    }

    /// `standardCheck`.
    pub fn check(&self, t: &Tables, mat: &str) -> bool {
        if self.melt.mat.as_deref().is_some_and(|m| m != mat) {
            return false;
        }
        self.mold(t).is_some() && self.melt.amount < self.limit(t)
    }

    /// `canAcceptPartialPour` of the core at `core` for a jet on `cell`, then `standardAdd` (`pour`): what is left,
    /// or `None` when it takes none.
    pub fn pour(&mut self, t: &Tables, jet: &Jet, mat: &str, q: u32) -> Option<u32> {
        if !takes_pour_at(jet.core, jet.facing, jet.cell, jet.face) || !self.check(t, mat) {
            return None;
        }
        self.melt.mat = Some(mat.to_string());
        let limit = self.limit(t);
        if q + self.melt.amount <= limit {
            self.melt.amount += q;
            return Some(0);
        }
        let required = limit - self.melt.amount;
        self.melt.amount = limit;
        self.last_progress = self.now;
        Some(q - required)
    }

    fn water_required(&self, t: &Tables) -> u32 {
        self.mold(t).map_or(50, |m| 5 * m.cost)
    }

    /// `maxProcessable`:117-139: the molds it can cast now.
    pub fn max_processable(&self, t: &Tables) -> u32 {
        let (Some(mat), Some(mold)) = (self.melt.mat.as_deref(), self.mold(t)) else { return 0 };
        let Some((out_id, per)) = mold.outputs.get(mat) else { return 0 };
        let free: u32 = slot::OUT
            .map(|i| match &self.slots[i] {
                None => STACK,
                Some(s) if s.id == *out_id => STACK.saturating_sub(s.count),
                Some(_) => 0,
            })
            .sum();
        let req = 5 * mold.cost;
        (self.melt.amount / mold.cost.max(1)).min(free / (*per).max(1)).min(self.water.mb / req.max(1)).min(self.steam.free() / req.max(1))
    }

    /// One tick (`updateEntity`:60-111).
    pub fn step(&mut self, t: &Tables) -> bool {
        if self.slots.len() < slot::COUNT {
            self.slots.resize(slot::COUNT, None);
        }
        let before = (self.melt.clone(), self.water.mb, self.steam.mb, self.last_progress, self.slots.clone());
        // In case of overfill problems, the excess goes out as a scrap (F5).
        let cap = self.capacity(t);
        if self.melt.amount > cap {
            if let Some(mat) = &self.melt.mat {
                self.pending.push(MachineEffect::DropStack {
                    offset: [0, 2, 0],
                    item: format!("{}{mat}", scrap::PREFIX),
                    count: 1,
                    custom: scrap::stack(mat, self.melt.amount - cap).custom,
                });
            }
            self.melt.amount = cap;
        }
        if self.melt.amount == 0 {
            self.melt.mat = None;
        }
        let molds = self.max_processable(t);
        if molds > 0 && (molds >= 9 || self.now >= self.last_progress + IDLE_FLUSH) {
            self.cast(t, molds);
        }
        before != (self.melt.clone(), self.water.mb, self.steam.mb, self.last_progress, self.slots.clone())
    }

    fn cast(&mut self, t: &Tables, molds: u32) {
        let (mat, mold) = (self.melt.mat.clone().unwrap_or_default(), self.mold(t).cloned());
        let Some(mold) = mold else { return };
        let Some((out_id, per)) = mold.outputs.get(&mat).cloned() else { return };
        self.melt.amount -= molds * mold.cost;
        let mut remaining = per * molds;
        for i in slot::OUT {
            if remaining == 0 {
                break;
            }
            if self.slots[i].is_none() {
                self.slots[i] = Some(Stack::new(&out_id, 0));
            }
            if let Some(s) = self.slots[i].as_mut().filter(|s| s.id == out_id) {
                let n = remaining.min(STACK.saturating_sub(s.count));
                s.count += n;
                remaining -= n;
            }
        }
        self.slots.iter_mut().for_each(|s| {
            if s.as_ref().is_some_and(|x| x.count == 0) {
                *s = None;
            }
        });
        let used = self.water_required(t) * molds;
        self.water.mb = self.water.mb.saturating_sub(used);
        self.steam.mb += used;
        self.last_progress = self.now;
    }
}

impl FluidNode for StrandCaster {
    fn accepts_any(&self) -> bool {
        false
    }
    fn fluid_types(&self) -> Vec<String> {
        vec!["water".into(), "spentsteam".into()]
    }
    fn fluid_offer(&self, fluid: &str) -> u64 {
        if fluid == "spentsteam" { u64::from(self.steam.mb) } else { 0 }
    }
    fn fluid_demand(&self, fluid: &str) -> u64 {
        if fluid == "water" { u64::from(self.water.free()) } else { 0 }
    }
    fn fluid_give(&mut self, fluid: &str, mb: u64) {
        if fluid == "water" {
            self.water.fill(fluid, mb.min(u64::from(u32::MAX)) as u32);
        }
    }
    fn fluid_take(&mut self, fluid: &str, mb: u64) {
        if fluid == "spentsteam" {
            self.steam.drain(mb.min(u64::from(u32::MAX)) as u32);
        }
    }
}

impl Logic for StrandCaster {
    fn tick(&mut self) -> bool {
        self.step(Tables::ntm())
    }
    fn world_effects(&mut self) -> Vec<MachineEffect> {
        std::mem::take(&mut self.pending)
    }
    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(self)
    }
    fn level(&self, max: u8) -> u8 {
        fill_level(u64::from(self.melt.amount), u64::from(self.capacity(Tables::ntm()).max(1)), max)
    }
    fn view(&self) -> ViewModel {
        let t = Tables::ntm();
        let metal = match &self.melt.mat {
            Some(m) if self.melt.amount > 0 => format!("{m}: {}", quanta_name(self.melt.amount)),
            _ => "vazio".into(),
        };
        ViewModel::new("Lingotamento contínuo")
            .text(format!("Metal: {metal} de {}", quanta_name(self.capacity(t))), view::GREY)
            .text(format!("Água: {}", self.water.line()), view::GREY)
    }
    /// Its items: the mold and what it cast.
    fn contents(&self) -> Vec<(String, u32)> {
        self.slots.iter().flatten().map(|s| (s.id.clone(), s.count)).collect()
    }
    /// What it holds, as one scrap with every quantum (`MachineStrandCaster.breakBlock` drops it at `y + maxY`).
    fn scraps(&self) -> Vec<(String, u32)> {
        self.melt.molten()
    }
}

impl Slotted for StrandCaster {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    /// Its record keeps no slots (`serde(skip)`): they are the block entity's alone, so a model the journal restored
    /// takes them from it instead of writing empty ones over them (the review's P1-1).
    fn slots_recorded(&self) -> bool {
        false
    }
    /// `horizon_screen/machine_strand_caster.json`: quanta, capacity, molten colour, then water and spent steam.
    fn screen_data(&self) -> Vec<i32> {
        let t = Tables::ntm();
        let col = |f: &str| crate::fluid::get(f).map_or(0, |ft| ft.color as i32);
        let mut v = self.melt.synced(t, self.capacity(t));
        v.extend([self.water.mb as i32, WATER_CAP as i32, col("water"), self.steam.mb as i32, STEAM_CAP as i32, col("spentsteam")]);
        v
    }
    /// The metal (material id, quanta), both tanks and `lastProgressTick` (the NBT `t`, two words).
    fn save_state(&self) -> Vec<i32> {
        vec![
            Tables::ntm().mat_id(self.melt.mat.as_deref()),
            self.melt.amount as i32,
            self.water.mb as i32,
            self.steam.mb as i32,
            (self.last_progress >> 32) as i32,
            self.last_progress as i32,
        ]
    }
    fn load_state(&mut self, v: &[i32]) {
        if v.len() < 6 {
            return;
        }
        self.melt.amount = v[1].max(0) as u32;
        self.melt.mat = Tables::ntm().mat_name(v[0]).filter(|_| self.melt.amount > 0);
        if self.melt.mat.is_none() {
            self.melt.amount = 0;
        }
        self.water.mb = (v[2].max(0) as u32).min(WATER_CAP);
        self.steam.mb = (v[3].max(0) as u32).min(STEAM_CAP);
        self.last_progress = (u64::from(v[4] as u32) << 32) | u64::from(v[5] as u32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_distinct_funnel_cells_two_above_the_core() {
        for facing in ["north", "south", "east", "west"] {
            let core = [10, 64, -3];
            let cells = pour_cells(core, facing);
            assert_eq!(cells.len(), 4, "{facing}");
            assert!(cells.iter().all(|c| c[1] == 66), "{facing}");
            assert!(cells.contains(&[10, 66, -3]), "the cell over the core: {facing}");
            let mut sorted = cells.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), 4, "{facing}");
        }
    }

    #[test]
    fn funnel_cells_follow_dir_and_its_rotation_up() {
        // North: dir = (0,0,-1), rot = dir.getRotation(UP) = (1,0,0).
        let core = [0, 64, 0];
        let mut cells = pour_cells(core, "north");
        cells.sort();
        assert_eq!(cells, vec![[0, 66, 0], [0, 66, 1], [1, 66, 0], [1, 66, 1]]);
        // East: dir = (1,0,0), rot = (0,0,1).
        let mut cells = pour_cells(core, "east");
        cells.sort();
        assert_eq!(cells, vec![[-1, 66, 0], [-1, 66, 1], [0, 66, 0], [0, 66, 1]]);
    }

    #[test]
    fn only_the_face_up_of_a_funnel_cell_takes_a_pour() {
        let core = [0, 64, 0];
        assert!(takes_pour_at(core, "north", [1, 66, 1], [0, 1, 0]));
        assert!(takes_pour_at(core, "north", [0, 66, 0], [0, 1, 0]));
        for face in [[0, -1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
            assert!(!takes_pour_at(core, "north", [1, 66, 1], face), "{face:?}");
        }
        // The funnel is 3 wide: the third column and the layer under are not pour cells.
        assert!(!takes_pour_at(core, "north", [-1, 66, 0], [0, 1, 0]));
        assert!(!takes_pour_at(core, "north", [1, 65, 1], [0, 1, 0]));
        assert!(!takes_pour_at(core, "bogus", [0, 66, 0], [0, 1, 0]));
    }

    // ---- the model (F4/F5) ----

    const CORE: Pos = [0, 64, 0];
    const FUNNEL: Pos = [0, 66, 0];
    const UP: [i32; 3] = [0, 1, 0];

    fn with_mold(item: &str) -> StrandCaster {
        let mut c = StrandCaster::default();
        c.slots[slot::MOLD] = Some(Stack::new(item, 1));
        c.water.mb = WATER_CAP;
        c
    }

    fn pour(c: &mut StrandCaster, mat: &str, q: u32) -> Option<u32> {
        c.pour(Tables::ntm(), &Jet { core: CORE, facing: "north", cell: FUNNEL, face: UP }, mat, q)
    }

    fn out_total(c: &StrandCaster) -> u32 {
        c.slots[slot::OUT].iter().flatten().map(|s| s.count).sum()
    }

    #[test]
    fn it_takes_metal_only_with_a_mold_on_a_funnel_cell_from_above() {
        let mut none = StrandCaster::default();
        assert_eq!(pour(&mut none, "steel", 72), None, "no mold: standardCheck refuses");
        let mut c = with_mold("ntm:mold__ingot");
        assert_eq!(c.pour(Tables::ntm(), &Jet { core: CORE, facing: "north", cell: FUNNEL, face: [1, 0, 0] }, "steel", 72), None, "a side face");
        assert_eq!(c.pour(Tables::ntm(), &Jet { core: CORE, facing: "north", cell: [5, 66, 5], face: UP }, "steel", 72), None, "a cell that is no funnel");
        assert_eq!(pour(&mut c, "steel", 72), Some(0));
        assert_eq!(pour(&mut c, "iron", 72), None, "another material than the one it holds");
        assert_eq!((c.melt.mat.as_deref(), c.melt.amount), (Some("steel"), 72));
    }

    #[test]
    fn it_accepts_nine_molds_and_hands_back_the_rest() {
        let mut c = with_mold("ntm:mold__ingot");
        // The limit is cost * 9 = 648: a jet of 700 fills it and 52 are left; the overflow stamps `lastProgressTick`.
        c.now = 1234;
        assert_eq!(pour(&mut c, "steel", 700), Some(52));
        assert_eq!(c.melt.amount, 648);
        assert_eq!(c.last_progress, 1234);
        assert_eq!(pour(&mut c, "steel", 1), None, "full to the limit");
        assert_eq!(c.capacity(Tables::ntm()), 720, "ten molds' worth is the capacity");
    }

    #[test]
    fn nine_molds_cast_at_once_and_use_the_water_and_make_steam() {
        let mut c = with_mold("ntm:mold__ingot");
        pour(&mut c, "steel", 648);
        c.now = 1; // `lastProgressTick` 0 + 200 not reached: only the full batch goes
        assert!(c.step(Tables::ntm()));
        assert_eq!(out_total(&c), 9);
        assert_eq!(c.melt.amount, 0);
        assert_eq!(c.melt.mat.as_deref(), Some("steel"), "the type stays until the next tick");
        c.step(Tables::ntm());
        assert_eq!(c.melt.mat, None, "type is cleared when it is empty on the next tick");
        // 5 * cost per mold = 360, nine molds.
        assert_eq!((c.water.mb, c.steam.mb), (WATER_CAP - 3_240, 3_240));
        assert_eq!(c.last_progress, 1);
    }

    #[test]
    fn a_partial_batch_waits_200_ticks_after_the_last_progress() {
        let mut c = with_mold("ntm:mold__ingot");
        c.last_progress = 1_000;
        pour(&mut c, "steel", 216);
        c.now = 1_199;
        c.step(Tables::ntm());
        assert_eq!(out_total(&c), 0, "199 ticks: not yet");
        c.now = 1_200;
        c.step(Tables::ntm());
        assert_eq!(out_total(&c), 3);
        assert_eq!(c.last_progress, 1_200);
    }

    #[test]
    fn what_does_not_fill_a_mold_stays() {
        let mut c = with_mold("ntm:mold__ingot");
        c.now = 10_000;
        pour(&mut c, "steel", 100);
        c.step(Tables::ntm());
        assert_eq!((out_total(&c), c.melt.amount), (1, 28), "one ingot, 28 quanta left in the pool");
    }

    #[test]
    fn the_outputs_fill_the_six_slots_and_stop_at_the_room() {
        let mut c = with_mold("ntm:mold__wire"); // 8 wires a mold, 72 each
        c.now = 10_000;
        pour(&mut c, "steel", 648);
        // 6 slots of 64 hold 384; one mold makes 8: 48 molds fit, 9 are poured.
        c.step(Tables::ntm());
        assert_eq!(out_total(&c), 72);
        assert_eq!(c.slots[1].as_ref().map(|s| s.count), Some(64));
        assert_eq!(c.slots[2].as_ref().map(|s| s.count), Some(8));
        // Fill every output but 5 places: only floor(5 / 8) = 0 molds can be cast.
        let mut full = with_mold("ntm:mold__wire");
        full.now = 10_000;
        for i in slot::OUT {
            full.slots[i] = Some(Stack::new("ntm:wire_fine__steel", 64));
        }
        full.slots[6] = Some(Stack::new("ntm:wire_fine__steel", 59));
        pour(&mut full, "steel", 72);
        assert_eq!(full.max_processable(Tables::ntm()), 0);
        full.step(Tables::ntm());
        assert_eq!(full.melt.amount, 72, "nothing is cast and nothing is lost");
    }

    #[test]
    fn no_water_or_no_room_for_steam_casts_nothing() {
        let mut c = with_mold("ntm:mold__ingot");
        c.now = 10_000;
        pour(&mut c, "steel", 72);
        c.water.mb = 359;
        assert_eq!(c.max_processable(Tables::ntm()), 0);
        c.water.mb = 360;
        c.steam.mb = STEAM_CAP - 359;
        assert_eq!(c.max_processable(Tables::ntm()), 0);
        c.steam.mb = STEAM_CAP - 360;
        assert_eq!(c.max_processable(Tables::ntm()), 1);
    }

    /// F5: a lighter mold lowers the capacity; what does not fit goes out as one scrap with the exact excess at `y + 2`,
    /// and the metal that stays and the scrap are what it held.
    #[test]
    fn the_excess_of_a_lighter_mold_is_one_exact_scrap() {
        let mut c = with_mold("ntm:mold__ingot");
        c.melt = Melt { mat: Some("steel".into()), amount: 700 };
        c.slots[slot::MOLD] = Some(Stack::new("ntm:mold__nugget", 1)); // cost 8: capacity 80
        c.water.mb = 0; // nothing casts
        c.step(Tables::ntm());
        assert_eq!(c.melt.amount, 80);
        let fx = c.world_effects();
        assert_eq!(
            fx,
            vec![MachineEffect::DropStack { offset: [0, 2, 0], item: "ntm:scraps__steel".into(), count: 1, custom: Some("{ntm_amount:620}".into()) }]
        );
        assert_eq!(80 + 620, 700, "conservation");
        assert!(c.world_effects().is_empty(), "drained once");
    }

    #[test]
    fn with_no_mold_the_capacity_is_fifty_thousand() {
        let mut c = StrandCaster { melt: Melt { mat: Some("steel".into()), amount: 49_999 }, ..Default::default() };
        c.step(Tables::ntm());
        assert_eq!(c.melt.amount, 49_999);
        assert!(c.world_effects().is_empty());
        c.melt.amount = 50_001;
        c.step(Tables::ntm());
        assert_eq!(c.melt.amount, 50_000);
        assert_eq!(c.world_effects().len(), 1);
    }

    #[test]
    fn a_broken_caster_gives_back_its_items_and_every_quantum() {
        let mut c = with_mold("ntm:mold__ingot");
        pour(&mut c, "steel", 100);
        assert_eq!(Logic::scraps(&c), vec![("steel".to_string(), 100)]);
        assert_eq!(Logic::contents(&c), vec![("ntm:mold__ingot".to_string(), 1)]);
    }

    #[test]
    fn its_state_survives_a_save_and_load() {
        let mut c = with_mold("ntm:mold__ingot");
        pour(&mut c, "steel", 300);
        c.water.mb = 12_345;
        c.steam.mb = 678;
        c.last_progress = 0x1_2345_6789;
        let saved = c.save_state();
        let mut d = StrandCaster::default();
        d.load_state(&saved);
        assert_eq!((d.melt.clone(), d.water.mb, d.steam.mb, d.last_progress), (c.melt.clone(), 12_345, 678, 0x1_2345_6789));
        d.load_state(&[0, 0, 0, 0, 0, 0]);
        assert_eq!((d.melt.mat, d.melt.amount), (None, 0));
    }

    #[test]
    fn water_comes_in_and_spent_steam_goes_out() {
        let mut c = StrandCaster::default();
        assert_eq!(c.fluid_demand("water"), u64::from(WATER_CAP));
        assert_eq!(c.fluid_demand("steam"), 0);
        c.fluid_give("water", 1_000);
        c.fluid_give("steam", 1_000);
        assert_eq!(c.water.mb, 1_000);
        c.steam.mb = 500;
        assert_eq!(c.fluid_offer("spentsteam"), 500);
        c.fluid_take("spentsteam", 200);
        assert_eq!(c.steam.mb, 300);
        c.fluid_take("water", 200);
        assert_eq!(c.water.mb, 1_000, "the water is not taken back out");
    }
}
