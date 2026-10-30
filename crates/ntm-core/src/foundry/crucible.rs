//! The crucible (`tileentity/machine/TileEntityCrucible.java`): melts the
//! items in its 9 slots (the engine's; slot 0 is unused, as in the NTM)
//! with the heat it pulls from the source below, into two stacks of up to
//! 10 368 quanta: the loaded alloy's materials and the rest ("waste").
//! Every `frequency` ticks the alloy turns its inputs into its outputs.
//! Each tick it pours up to 24 quanta of each stack down beside it: the
//! alloy's outputs on its facing side, the waste on the other (only
//! smeltable materials pour; additives stay until emptied).
//!
//! Left out: picking up dropped items, burning whoever stands in it.

use super::{Acceptor, HeatSource, Mat, Tables, add, quanta};
use crate::{
    machine::{Effect, Logic, fill_level},
    machine::Held,
    server::container_bridge::{Slots, Slotted},
    view::{self, ViewModel},
};

pub mod actions {
    /// The alloy list (the screen with slots' button 0 opens it too).
    pub const PICK: u32 = 1;
    pub const BACK: u32 = 2;
    /// A row of the list: `LIST + row` (row 0: no alloy).
    pub const LIST: u32 = crate::view::LIST_ACTIONS;
}
use serde::{Deserialize, Serialize};

pub const SLOTS: usize = 10;
pub const INPUTS: std::ops::Range<usize> = 1..10;
/// `recipeZCapacity`, `wasteZCapacity` (`BLOCK.q(16)`), `processTime`,
/// `maxHeat`; `diffusion` is 1/4.
pub const CAPACITY: u32 = 10_368;
pub const PROCESS: u32 = 20_000;
pub const MAX_HEAT: u32 = 100_000;
/// Quanta a stream pours a tick (`NUGGET.q(3)`).
pub const POUR: u32 = 24;
/// Materials a stack keeps in the block entity's values (a crash past
/// them loses the rest: more than 20 kinds in one crucible is unusual).
pub const STATE_MATS: usize = 20;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Crucible {
    pub slots: Slots,
    pub heat: u32,
    pub progress: u32,
    /// The loaded alloy (`recipe`), by name.
    pub recipe: Option<String>,
    pub recipe_stack: Vec<Mat>,
    pub waste_stack: Vec<Mat>,
    /// The recipe list is open on the old screen.
    #[serde(default)]
    pub picking: bool,
    /// The colour each stream (recipe, waste) poured last (0: none), for
    /// the model's jets, held for [`JET_HOLD`] ticks after a pour.
    #[serde(skip)]
    pub jets: [u32; 2],
    #[serde(skip)]
    pub jet_hold: [u8; 2],
}

/// Ticks a jet stays drawn after its last pour: a stream pours in bursts
/// (when the block below takes metal), and a one-tick jet only flickers.
pub const JET_HOLD: u8 = 8;

impl Crucible {
    /// A tick of the jets: one that hasn't poured for [`JET_HOLD`] ticks
    /// goes out. True when one did.
    pub fn fade_jets(&mut self) -> bool {
        let mut out = false;
        for i in 0..2 {
            self.jet_hold[i] = self.jet_hold[i].saturating_sub(1);
            if self.jet_hold[i] == 0 && self.jets[i] != 0 {
                self.jets[i] = 0;
                out = true;
            }
        }
        out
    }
}

impl Default for Crucible {
    fn default() -> Self {
        Self { slots: vec![None; SLOTS], heat: 0, progress: 0, recipe: None, recipe_stack: Vec::new(), waste_stack: Vec::new(), picking: false, jets: [0; 2], jet_hold: [0; 2] }
    }
}

/// The two streams: the alloy's outputs (facing side) and the waste
/// (opposite side).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Recipe = 0,
    Waste = 1,
}

impl Crucible {
    /// `tryPullHeat`: a quarter of the difference from the source below,
    /// or a slow fade without one.
    pub fn pull_heat(&mut self, source: Option<&mut dyn HeatSource>) {
        if self.heat >= MAX_HEAT {
            return;
        }
        if let Some(src) = source {
            let diff = src.heat_stored() as i64 - self.heat as i64;
            if diff == 0 {
                return;
            }
            let diff = diff.min(i64::from(MAX_HEAT - self.heat));
            if diff > 0 {
                let d = (diff as f64 * 0.25).ceil() as u32;
                src.use_heat(d);
                self.heat = (self.heat + d).min(MAX_HEAT);
                return;
            }
        }
        self.heat = self.heat.saturating_sub((self.heat / 1000).max(1));
    }

    /// `isItemSmeltable`: it melts into something, the stacks have room,
    /// and with an alloy loaded it brings one of its materials (no more
    /// of an input than the alloy's share of the capacity).
    pub fn smeltable(&self, t: &Tables, item: &str) -> bool {
        let mats = t.smelting(item);
        if mats.is_empty() {
            return false;
        }
        let recipe = self.recipe.as_deref().and_then(|r| t.alloy(r));
        let mut matches = recipe.is_none();
        let content = recipe.map_or(0, |r| r.input_amount());
        let mut recipe_amount = quanta(&self.recipe_stack, None);
        let mut waste_amount = quanta(&self.waste_stack, None);
        for (m, q) in &mats {
            let required = recipe.map_or(0, |r| r.input(m));
            if recipe.is_some_and(|r| r.output(m) > 0) {
                recipe_amount += q;
                matches = true;
                continue;
            }
            if required == 0 {
                waste_amount += q;
            } else {
                let max = required * CAPACITY / content.max(1);
                matches = true;
                recipe_amount += q;
                if quanta(&self.recipe_stack, Some(m)) + q > max {
                    return false;
                }
            }
        }
        recipe_amount <= CAPACITY && waste_amount <= CAPACITY && matches
    }

    /// `trySmelt`: with at least half the heat, progress grows by 5% of
    /// what is above half; at 20 000 the first item that can melt does.
    fn smelt(&mut self, t: &Tables) -> bool {
        if self.heat < MAX_HEAT / 2 {
            return false;
        }
        let Some(slot) = INPUTS.clone().find(|&i| self.slots[i].as_ref().is_some_and(|s| self.smeltable(t, &s.id))) else { return false };
        let delta = ((self.heat - MAX_HEAT / 2) as f64 * 0.05) as u32;
        self.progress += delta;
        self.heat -= delta;
        if self.progress >= PROCESS {
            self.progress = 0;
            let item = self.slots[slot].as_ref().unwrap().id.clone();
            let recipe = self.recipe.as_deref().and_then(|r| t.alloy(r));
            for (m, q) in t.smelting(&item) {
                if recipe.is_some_and(|r| r.input(&m) > 0 || r.output(&m) > 0) {
                    add(&mut self.recipe_stack, &m, q);
                } else {
                    add(&mut self.waste_stack, &m, q);
                }
            }
            if let Some(s) = &mut self.slots[slot] {
                s.count -= 1;
                if s.count == 0 {
                    self.slots[slot] = None;
                }
            }
        }
        true
    }

    /// `tryRecipe`, on the world's tick count.
    fn alloy(&mut self, t: &Tables, world_tick: u64) {
        let Some(r) = self.recipe.as_deref().and_then(|r| t.alloy(r)) else { return };
        if !world_tick.is_multiple_of(u64::from(r.frequency.max(1))) {
            return;
        }
        if r.inputs.iter().any(|(m, q)| quanta(&self.recipe_stack, Some(m)) < *q) {
            return;
        }
        for s in &mut self.recipe_stack {
            s.1 -= r.input(&s.0);
        }
        for (m, q) in &r.outputs {
            add(&mut self.recipe_stack, m, *q);
        }
    }

    /// One tick before pouring: heat, melting, alloying. True when its
    /// items or stacks changed.
    pub fn step(&mut self, t: &Tables, source: Option<&mut dyn HeatSource>, world_tick: u64) -> bool {
        let before = (self.recipe_stack.clone(), self.waste_stack.clone(), self.slots.iter().flatten().map(|s| s.count).sum::<u32>());
        self.pull_heat(source);
        if !self.smelt(t) {
            self.progress = 0;
        }
        self.alloy(t, world_tick);
        self.clean();
        before != (self.recipe_stack.clone(), self.waste_stack.clone(), self.slots.iter().flatten().map(|s| s.count).sum::<u32>())
    }

    fn clean(&mut self) {
        self.recipe_stack.retain(|s| s.1 > 0);
        self.waste_stack.retain(|s| s.1 > 0);
    }

    /// `pourFullStack` of one stream into `target`: the first material it
    /// takes, up to [`POUR`] quanta. Returns what was poured.
    pub fn pour_stream(&mut self, t: &Tables, stream: Stream, target: &mut dyn Acceptor) -> Option<Mat> {
        let recipe = self.recipe.as_deref().and_then(|r| t.alloy(r)).cloned();
        let stack = match stream {
            Stream::Recipe => &mut self.recipe_stack,
            Stream::Waste => &mut self.waste_stack,
        };
        let mut poured: Option<Mat> = None;
        for s in stack.iter_mut() {
            // The recipe stream only pours the alloy's outputs (all of it
            // with no alloy loaded).
            if stream == Stream::Recipe && recipe.as_ref().is_some_and(|r| r.output(&s.0) == 0) {
                continue;
            }
            if !t.pourable(&s.0) {
                continue;
            }
            let q = s.1.min(POUR);
            if let Some(left) = target.pour(t, &s.0, q) {
                s.1 -= q - left;
                poured = Some((s.0.clone(), q - left));
                break;
            }
        }
        self.clean();
        if let Some(p) = &poured {
            self.jets[stream as usize] = t.color(&p.0);
            self.jet_hold[stream as usize] = JET_HOLD;
        }
        poured
    }

    /// The loaded alloy, by its row in the list (0: none).
    pub fn pick(&mut self, t: &Tables, row: usize) {
        self.recipe = row.checked_sub(1).and_then(|i| t.alloys.get(i)).map(|a| a.name.clone());
        self.picking = false;
    }

    /// The screen's values: heat, its maximum, progress, its maximum, the
    /// waste and recipe totals, the capacity, the colour of each stack's
    /// top material, then up to 8 `(material id, quanta)` of the waste and
    /// 8 of the recipe stack (0, 0 when fewer), then the colours of the
    /// recipe and waste jets (0: not pouring).
    pub fn screen_data(&self, t: &Tables) -> Vec<i32> {
        let top = |s: &[Mat]| s.last().map_or(0, |m| t.color(&m.0)) as i32;
        let mut v = vec![
            self.heat as i32,
            MAX_HEAT as i32,
            self.progress as i32,
            PROCESS as i32,
            quanta(&self.waste_stack, None) as i32,
            quanta(&self.recipe_stack, None) as i32,
            CAPACITY as i32,
            top(&self.waste_stack),
            top(&self.recipe_stack),
        ];
        for stack in [&self.waste_stack, &self.recipe_stack] {
            for i in 0..8 {
                match stack.get(i) {
                    Some((m, q)) => v.extend([t.material(m).map_or(-1, |m| m.id as i32), *q as i32]),
                    None => v.extend([0, 0]),
                }
            }
        }
        v.extend(self.jets.map(|c| c as i32));
        v
    }
}

/// The crucible's server side (heat from below, pouring) runs in
/// `server::foundry_server`; this is its screen and contents.
impl Logic for Crucible {
    fn level(&self, max: u8) -> u8 {
        let n = quanta(&self.recipe_stack, None) + quanta(&self.waste_stack, None);
        fill_level(u64::from(n), u64::from(2 * CAPACITY), max)
    }

    fn view(&self) -> ViewModel {
        let t = Tables::ntm();
        if self.picking {
            let mut rows = vec!["(nenhuma)".to_string()];
            rows.extend(t.alloys.iter().map(|a| if a.label.is_empty() { a.name.clone() } else { a.label.clone() }));
            let selected = self.recipe.as_deref().and_then(|r| t.alloys.iter().position(|a| a.name == r)).map_or(0, |i| i as u32 + 1);
            return ViewModel::new("Cadinho: liga").list(actions::LIST, rows, Some(selected)).button(actions::BACK, "Voltar");
        }
        let line = |s: &[Mat]| if s.is_empty() { "vazio".to_string() } else { s.iter().map(|(m, q)| format!("{m} {q}")).collect::<Vec<_>>().join(", ") };
        let alloy = self.recipe.as_deref().and_then(|r| t.alloy(r)).map_or("nenhuma".to_string(), |a| a.label.clone());
        ViewModel::new("Cadinho")
            .bar("Calor", u64::from(self.heat), u64::from(MAX_HEAT), "TU")
            .bar("Progresso", u64::from(self.progress), u64::from(PROCESS), "")
            .text(format!("Liga: {alloy}"), view::GREY)
            .text(format!("Receita: {}", line(&self.recipe_stack)), view::GREY)
            .text(format!("Resto: {}", line(&self.waste_stack)), view::GREY)
            .button(actions::PICK, "Escolher a liga")
    }

    fn action(&mut self, action: u32, _held: Option<&Held>, _fx: &mut Vec<Effect>) {
        match action {
            actions::PICK => self.picking = true,
            actions::BACK => self.picking = false,
            a if (actions::LIST..actions::LIST + view::LIST_MAX as u32).contains(&a) => self.pick(Tables::ntm(), (a - actions::LIST) as usize),
            _ => {}
        }
    }

    /// Its items and its molten metal as scraps.
    fn contents(&self) -> Vec<(String, u32)> {
        let mut out: Vec<(String, u32)> = self.slots.iter().flatten().map(|s| (s.id.clone(), s.count)).collect();
        out.extend(self.scraps());
        out
    }
}

impl Crucible {
    /// Its molten metal as scraps, a whole ingot of each material per 72
    /// quanta (a scrap here is always an ingot's worth: the slots carry no
    /// amount). What is under an ingot is lost.
    pub fn scraps(&self) -> Vec<(String, u32)> {
        self.recipe_stack
            .iter()
            .chain(&self.waste_stack)
            .filter(|(_, q)| q / 72 > 0)
            .map(|(m, q)| (format!("ntm:scraps__{m}"), q / 72))
            .collect()
    }
}

impl Slotted for Crucible {
    fn held_contents(&self) -> Vec<(String, u32)> {
        self.scraps()
    }
    /// Heat, progress, the alloy (row in the list, 0 none), both stacks.
    fn save_state(&self) -> Vec<i32> {
        let t = Tables::ntm();
        let alloy = self.recipe.as_deref().and_then(|r| t.alloys.iter().position(|a| a.name == r)).map_or(0, |i| i as i32 + 1);
        let mut v = vec![self.heat as i32, self.progress as i32, alloy];
        // The block entity's values have a fixed length (`horizon_machine`
        // `data`: 43 for the screen + this, 128 in all): up to
        // STATE_MATS materials a stack go there.
        t.encode_stack(&self.recipe_stack[..self.recipe_stack.len().min(STATE_MATS)], &mut v);
        t.encode_stack(&self.waste_stack[..self.waste_stack.len().min(STATE_MATS)], &mut v);
        v
    }
    fn load_state(&mut self, v: &[i32]) {
        let t = Tables::ntm();
        let mut it = v.iter();
        let (Some(&heat), Some(&progress), Some(&alloy)) = (it.next(), it.next(), it.next()) else { return };
        let (Some(r), Some(w)) = (t.decode_stack(&mut it), t.decode_stack(&mut it)) else { return };
        self.heat = heat.max(0) as u32;
        self.progress = progress.max(0) as u32;
        self.recipe = usize::try_from(alloy - 1).ok().and_then(|i| t.alloys.get(i)).map(|a| a.name.clone());
        self.recipe_stack = r;
        self.waste_stack = w;
    }
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        Crucible::screen_data(self, Tables::ntm())
    }
    /// The recipe selector: the alloy list of the old screen.
    fn button(&mut self, id: u32) -> bool {
        if id == 0 {
            self.picking = true;
        }
        id == 0
    }
}

/// Metal poured into the crucible from above (`pour`): into the waste
/// stack with no alloy loaded, else into the alloy's share of an input.
impl Acceptor for Crucible {
    fn pour(&mut self, t: &Tables, mat: &str, q: u32) -> Option<u32> {
        let Some(r) = self.recipe.as_deref().and_then(|r| t.alloy(r)) else {
            let room = CAPACITY.saturating_sub(quanta(&self.waste_stack, None));
            let n = q.min(room);
            add(&mut self.waste_stack, mat, n);
            return Some(q - n);
        };
        let max = r.input(mat) * CAPACITY / r.input_amount().max(1);
        let n = (max.saturating_sub(quanta(&self.recipe_stack, Some(mat))))
            .min(CAPACITY.saturating_sub(quanta(&self.recipe_stack, None)))
            .min(q);
        if n == 0 {
            return None;
        }
        add(&mut self.recipe_stack, mat, n);
        Some(q - n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundry::testing::tables;
    use crate::foundry::firebox::Firebox;
    use crate::server::container_bridge::Stack;

    /// Takes everything up to `cap` quanta of one material.
    #[derive(Default)]
    struct Pot {
        mat: Option<String>,
        q: u32,
    }
    impl Acceptor for Pot {
        fn pour(&mut self, _t: &Tables, mat: &str, q: u32) -> Option<u32> {
            if self.mat.as_deref().is_some_and(|m| m != mat) {
                return None;
            }
            self.mat = Some(mat.into());
            self.q += q;
            Some(0)
        }
    }

    fn hot() -> Crucible {
        Crucible { heat: MAX_HEAT, ..Default::default() }
    }

    #[test]
    fn heat_comes_from_the_source_below_a_quarter_at_a_time() {
        let mut c = Crucible::default();
        let mut f = Firebox { heat: 80_000, ..Default::default() };
        c.pull_heat(Some(&mut f));
        assert_eq!((c.heat, f.heat), (20_000, 60_000));
        c.pull_heat(Some(&mut f));
        assert_eq!((c.heat, f.heat), (30_000, 50_000));
        // Without a source it fades.
        c.pull_heat(None);
        assert_eq!(c.heat, 29_970);
    }

    #[test]
    fn items_melt_one_at_a_time_with_enough_heat() {
        let t = tables();
        let mut c = Crucible::default();
        c.slots[3] = Some(Stack::new("minecraft:iron_ingot", 2));
        c.heat = 49_999;
        c.step(&t, None, 1);
        assert_eq!(c.waste_stack, vec![], "abaixo da metade do calor não funde");
        let mut c = hot();
        c.slots[3] = Some(Stack::new("minecraft:iron_ingot", 2));
        let mut ticks = 0;
        // Kept at full heat (a source under it), it melts an item in ~8 ticks.
        while c.waste_stack.is_empty() && ticks < 100 {
            c.heat = MAX_HEAT;
            c.step(&t, None, ticks);
            ticks += 1;
        }
        assert_eq!(ticks, 8);
        assert_eq!(c.waste_stack, vec![("iron".to_string(), 72)]);
        assert_eq!(c.slots[3], Some(Stack::new("minecraft:iron_ingot", 1)));
        assert!(!c.smeltable(&t, "minecraft:dirt"));
    }

    #[test]
    fn steel_from_iron_carbon_and_flux() {
        let t = tables();
        let mut c = hot();
        c.pick(&t, 1); // aço
        assert_eq!(c.recipe.as_deref(), Some("steel"));
        // Iron, coal (carbon at 2:1) and flux go to the recipe stack.
        c.recipe_stack = vec![("iron".into(), 16), ("carbon".into(), 24), ("flux".into(), 8)];
        c.step(&t, None, 20);
        assert_eq!(c.recipe_stack, vec![("steel".to_string(), 16)]);
        // Only every 20 ticks.
        c.recipe_stack = vec![("iron".into(), 16), ("carbon".into(), 24), ("flux".into(), 8)];
        c.step(&t, None, 21);
        assert!(c.recipe_stack.iter().all(|m| m.0 != "steel"));
        // An input over its share of the capacity is refused:
        // iron 16 of 48 → at most 3 456 quanta of iron.
        let mut c = hot();
        c.pick(&t, 1);
        c.recipe_stack = vec![("iron".into(), 3_400)];
        assert!(c.smeltable(&t, "minecraft:coal"));
        assert!(!c.smeltable(&t, "minecraft:iron_ingot"), "3 400 + 72 passa de 3 456");
        // With an alloy loaded, what isn't its material isn't taken.
        assert!(!c.smeltable(&t, "minecraft:redstone"));
    }

    #[test]
    fn pouring_takes_24_quanta_of_the_first_pourable_material() {
        let t = tables();
        let mut c = hot();
        c.pick(&t, 1);
        c.recipe_stack = vec![("iron".into(), 10), ("steel".into(), 100)];
        c.waste_stack = vec![("carbon".into(), 50), ("stone".into(), 30), ("copper".into(), 30)];
        let mut out = Pot::default();
        // The recipe stream only pours the alloy's outputs: steel, not iron.
        assert_eq!(c.pour_stream(&t, Stream::Recipe, &mut out), Some(("steel".into(), 24)));
        assert_eq!(c.recipe_stack, vec![("iron".to_string(), 10), ("steel".to_string(), 76)]);
        // The waste stream skips what can't pour (carbon, stone).
        let mut w = Pot::default();
        assert_eq!(c.pour_stream(&t, Stream::Waste, &mut w), Some(("copper".into(), 24)));
        assert_eq!(c.pour_stream(&t, Stream::Waste, &mut w), Some(("copper".into(), 6)));
        assert_eq!(c.pour_stream(&t, Stream::Waste, &mut w), None);
        assert_eq!(c.waste_stack, vec![("carbon".to_string(), 50), ("stone".to_string(), 30)]);
        // Nothing to pour into: nothing is lost.
        struct No;
        impl Acceptor for No {
            fn pour(&mut self, _: &Tables, _: &str, _: u32) -> Option<u32> {
                None
            }
        }
        assert_eq!(c.pour_stream(&t, Stream::Recipe, &mut No), None);
        assert_eq!(quanta(&c.recipe_stack, Some("steel")), 76);
        // The waste jet stays drawn for a while after its last pour.
        assert_eq!(c.jets[Stream::Waste as usize], t.color("copper"));
        for _ in 1..JET_HOLD {
            assert!(!c.fade_jets());
        }
        assert!(c.fade_jets(), "apaga {JET_HOLD} ticks depois");
        assert_eq!(c.jets, [0, 0]);
    }

    #[test]
    fn metal_poured_in_from_above() {
        let t = tables();
        let mut c = Crucible::default();
        assert_eq!(Acceptor::pour(&mut c, &t, "iron", 24), Some(0));
        assert_eq!(c.waste_stack, vec![("iron".to_string(), 24)]);
        c.pick(&t, 2); // cobre vermelho: copper 8 of 16 → at most 5 184
        assert_eq!(Acceptor::pour(&mut c, &t, "copper", 24), Some(0));
        assert_eq!(Acceptor::pour(&mut c, &t, "iron", 24), None, "não é da liga");
        c.recipe_stack = vec![("copper".into(), 5_180)];
        assert_eq!(Acceptor::pour(&mut c, &t, "copper", 24), Some(20));
    }

    #[test]
    fn the_screen_shows_heat_progress_and_both_stacks() {
        let t = tables();
        let mut c = hot();
        c.waste_stack = vec![("iron".into(), 72), ("copper".into(), 8)];
        c.recipe_stack = vec![("steel".into(), 16)];
        let v = c.screen_data(&t);
        assert_eq!(&v[..9], &[100_000, 100_000, 0, 20_000, 80, 16, 10_368, 0xFF7F32, 0xFF8F60]);
        assert_eq!(&v[9..13], &[26, 72, 28, 8]);
        assert_eq!(&v[25..27], &[30, 16]);
        assert_eq!(v.len(), 9 + 32 + 2);
    }
}
