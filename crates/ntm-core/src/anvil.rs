//! The NTM's anvils (docs/maquinas.md, "Bigorna"): `blocks/machine/
//! NTMAnvil.java`, `inventory/container/ContainerAnvil.java`,
//! `inventory/gui/GUIAnvil.java`, `inventory/recipes/anvil/AnvilRecipes
//! .java`, `packet/toserver/AnvilCraftPacket.java`.
//!
//! An anvil has a tier (its block) and two kinds of recipe:
//! - **smithing**: a left and a right item make one (an anvil upgrade, a
//!   mold engraved from an example item that isn't used up);
//! - **construction**: a list of inputs makes a list of outputs (some by
//!   chance), between a lowest and a highest tier.
//!
//! In the NTM the anvil has no block entity: smithing slots drop when the
//! screen closes, and construction takes its inputs from the player's
//! inventory. Here (a deviation the coordinator approved) it is a machine
//! on the engine's slots: the smithing slots stay, construction takes its
//! inputs from a 3×4 grid and puts its outputs in 4 slots, and both run
//! on a button in the server's tick, so taking and making are one step (no
//! dupe at a crash). Left out: the hot smithing (heat in item NBT), the
//! name tag and cyanide recipes (NBT), falling like sand.

use crate::{
    machine::Logic,
    server::container_bridge::{Slots, Slotted, Stack},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub const SLOTS: usize = 19;
pub const LEFT: usize = 0;
pub const RIGHT: usize = 1;
pub const FORGE_OUT: usize = 2;
pub const GRID: std::ops::Range<usize> = 3..15;
pub const OUT: std::ops::Range<usize> = 15..19;
pub const STACK: u32 = 64;

pub mod buttons {
    /// The construction recipe list (the old screen's).
    pub const PICK: u32 = 0;
    pub const CRAFT: u32 = 1;
    /// As many as the grid and the outputs allow (the NTM's shift).
    pub const CRAFT_MAX: u32 = 2;
    pub const FORGE: u32 = 3;
}

/// The index of its menu data with the construction picked + 1 (0: none).
pub const RECIPE_DATA: usize = 1;

pub mod actions {
    pub const BACK: u32 = 1;
    /// A row of the list: `LIST + row` (the recipes the anvil's tier allows).
    pub const LIST: u32 = crate::view::LIST_ACTIONS;
}

/// An input: any of `ids`, `count` of them (`exact`: exactly that many, a
/// mold's example).
#[derive(Clone, Debug, Deserialize)]
pub struct Input {
    pub ids: Vec<String>,
    pub count: u32,
    #[serde(default)]
    pub exact: bool,
}

impl Input {
    fn matches(&self, s: Option<&Stack>) -> bool {
        s.is_some_and(|s| self.ids.contains(&s.id) && if self.exact { s.count == self.count } else { s.count >= self.count })
    }
}

/// `AnvilSmithingRecipe`.
#[derive(Clone, Debug, Deserialize)]
pub struct Smithing {
    #[serde(default)]
    pub name: String,
    pub tier: i64,
    pub left: Input,
    pub right: Input,
    pub out: (String, u32),
    #[serde(default)]
    pub shapeless: bool,
    /// What each side uses up (a mold's example: 0).
    pub consume: (u32, u32),
}

/// `AnvilConstructionRecipe`.
#[derive(Clone, Debug, Deserialize)]
pub struct Construction {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub label: String,
    #[serde(rename = "in")]
    pub inputs: Vec<Input>,
    /// (item, count, chance).
    #[serde(rename = "out")]
    pub outputs: Vec<(String, u32, f32)>,
    pub tier_lower: i64,
    #[serde(default = "no_upper")]
    pub tier_upper: i64,
}

fn no_upper() -> i64 {
    -1
}

impl Construction {
    /// `isTierValid`.
    pub fn tier_valid(&self, tier: i64) -> bool {
        tier >= self.tier_lower && (self.tier_upper == -1 || tier <= self.tier_upper)
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Tables {
    pub smithing: Vec<Smithing>,
    pub construction: Vec<Construction>,
}

/// The anvil blocks and their tiers (`ModBlocks`, `NTMAnvil.TIER_*`).
pub const ANVILS: &[(&str, i64)] = &[
    ("ntm:anvil_iron", 1),
    ("ntm:anvil_lead", 1),
    ("ntm:anvil_steel", 2),
    ("ntm:anvil_desh", 3),
    ("ntm:anvil_ferrouranium", 4),
    ("ntm:anvil_saturnite", 5),
    ("ntm:anvil_bismuth_bronze", 5),
    ("ntm:anvil_arsenic_bronze", 5),
    ("ntm:anvil_schrabidate", 6),
    ("ntm:anvil_dnt", 7),
    ("ntm:anvil_osmiridium", 8),
    ("ntm:anvil_murky", 1_916_169),
];

pub fn tier(block: &str) -> i64 {
    ANVILS.iter().find(|a| a.0 == block).map_or(1, |a| a.1)
}

impl Tables {

    /// The construction recipes an anvil of `tier` shows, as indices.
    pub fn listed(&self, tier: i64) -> Vec<usize> {
        (0..self.construction.len()).filter(|&i| self.construction[i].tier_valid(tier)).collect()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Anvil {
    pub slots: Slots,
    pub tier: i64,
    /// The construction recipe picked (index in the table).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe: Option<usize>,
    /// Crafts made: the chance outputs' roll.
    #[serde(default)]
    pub made: u64,
    #[serde(default)]
    pub picking: bool,
}

impl Anvil {
    pub fn new(tier: i64) -> Self {
        Self { slots: vec![None; SLOTS], tier, recipe: None, made: 0, picking: false }
    }

    /// The smithing recipe the left and right slots make now, and whether
    /// mirrored (`updateSmithing`: the first that matches, of the tier).
    pub fn smithing<'a>(&self, t: &'a Tables) -> Option<(&'a Smithing, bool)> {
        let (l, r) = (self.slots[LEFT].as_ref(), self.slots[RIGHT].as_ref());
        t.smithing.iter().filter(|s| s.tier <= self.tier).find_map(|s| {
            if s.left.matches(l) && s.right.matches(r) {
                Some((s, false))
            } else if s.shapeless && s.left.matches(r) && s.right.matches(l) {
                Some((s, true))
            } else {
                None
            }
        })
    }

    /// Room in `range` for `n` more of `id` (one slot or several).
    fn room(slots: &Slots, range: std::ops::Range<usize>, id: &str, mut n: u32) -> bool {
        for i in range {
            match &slots[i] {
                None => n = n.saturating_sub(STACK),
                Some(s) if s.id == id => n = n.saturating_sub(STACK.saturating_sub(s.count)),
                _ => {}
            }
            if n == 0 {
                return true;
            }
        }
        n == 0
    }

    fn put(slots: &mut Slots, range: std::ops::Range<usize>, id: &str, mut n: u32) {
        for i in range.clone() {
            if let Some(s) = &mut slots[i]
                && s.id == id
            {
                let k = n.min(STACK.saturating_sub(s.count));
                s.count += k;
                n -= k;
            }
        }
        for i in range {
            if n == 0 {
                break;
            }
            if slots[i].is_none() {
                let k = n.min(STACK);
                slots[i] = Some(Stack::new(id, k));
                n -= k;
            }
        }
    }

    fn take(&mut self, i: usize, n: u32) {
        if let Some(s) = &mut self.slots[i] {
            s.count = s.count.saturating_sub(n);
            if s.count == 0 {
                self.slots[i] = None;
            }
        }
    }

    /// Smithing on the button: the inputs used up and the result in its
    /// slot, together. True when it made one.
    pub fn forge(&mut self, t: &Tables) -> bool {
        let Some((s, mirrored)) = self.smithing(t) else { return false };
        let (out, n) = (s.out.0.clone(), s.out.1);
        if !Self::room(&self.slots, FORGE_OUT..FORGE_OUT + 1, &out, n) {
            return false;
        }
        let (l, r) = if mirrored { (s.consume.1, s.consume.0) } else { s.consume };
        self.take(LEFT, l);
        self.take(RIGHT, r);
        Self::put(&mut self.slots, FORGE_OUT..FORGE_OUT + 1, &out, n);
        true
    }

    /// Whether the grid has every input of `c` (`doesPlayerHaveAStacks`).
    fn has(&self, c: &Construction) -> bool {
        let mut grid: Vec<Option<Stack>> = self.slots[GRID].to_vec();
        c.inputs.iter().all(|inp| {
            let mut need = inp.count;
            for s in grid.iter_mut().flatten() {
                if need == 0 {
                    break;
                }
                if inp.ids.contains(&s.id) {
                    let k = need.min(s.count);
                    s.count -= k;
                    need -= k;
                }
            }
            need == 0
        })
    }

    fn take_inputs(&mut self, c: &Construction) {
        for inp in &c.inputs {
            let mut need = inp.count;
            for i in GRID {
                if need == 0 {
                    break;
                }
                if let Some(s) = &self.slots[i]
                    && inp.ids.contains(&s.id)
                {
                    let k = need.min(s.count);
                    need -= k;
                    self.take(i, k);
                }
            }
        }
    }

    /// The chance outputs this craft gives (a hash of the crafts made: the
    /// port's rolls are deterministic).
    fn rolled(&self, c: &Construction) -> Vec<(String, u32)> {
        c.outputs
            .iter()
            .enumerate()
            .filter(|(i, (_, _, chance))| *chance >= 1.0 || roll(self.made, *i as u64) < *chance)
            .map(|(_, (id, n, _))| (id.clone(), *n))
            .collect()
    }

    /// Whether every output of a craft (of this roll) fits.
    fn fits(&self, outs: &[(String, u32)]) -> bool {
        let mut sim = self.slots.clone();
        outs.iter().all(|(id, n)| {
            let ok = Self::room(&sim, OUT, id, *n);
            Self::put(&mut sim, OUT, id, *n);
            ok
        })
    }

    /// Construction on the button: up to `times` crafts of the picked
    /// recipe while the grid has the inputs and the outputs have room
    /// (`AnvilCraftPacket`). Returns the crafts made.
    pub fn craft(&mut self, t: &Tables, times: u32) -> u32 {
        let Some(c) = self.recipe.and_then(|i| t.construction.get(i)) else { return 0 };
        if !c.tier_valid(self.tier) {
            return 0;
        }
        let mut made = 0;
        while made < times && self.has(c) {
            let outs = self.rolled(c);
            if !self.fits(&outs) {
                break;
            }
            self.take_inputs(c);
            for (id, n) in outs {
                Self::put(&mut self.slots, OUT, &id, n);
            }
            self.made += 1;
            made += 1;
        }
        made
    }

    /// The NTM's shift-craft: a stack's worth (64 crafts for several
    /// outputs, else a stack of the output).
    pub fn max_times(c: &Construction) -> u32 {
        match c.outputs.as_slice() {
            [(_, n, _)] => (STACK / (*n).max(1)).max(1),
            _ => 64,
        }
    }

    /// How many times the grid makes the picked recipe, up to a "craft
    /// max" (for the screen, every tick: each input counted on its own and
    /// the outputs' room only for the next craft, so it doesn't simulate
    /// them all; 0 when not even one is possible).
    pub fn times(&self, t: &Tables) -> u32 {
        let Some(c) = self.recipe.and_then(|i| t.construction.get(i)) else { return 0 };
        if !c.tier_valid(self.tier) || !self.has(c) || !self.fits(&self.rolled(c)) {
            return 0;
        }
        let grid = &self.slots[GRID];
        let by_input = c.inputs.iter().map(|inp| grid.iter().flatten().filter(|s| inp.ids.contains(&s.id)).map(|s| s.count).sum::<u32>() / inp.count.max(1));
        by_input.min().unwrap_or(1).clamp(1, Self::max_times(c))
    }

    pub fn screen_data(&self, t: &Tables) -> Vec<i32> {
        let times = self.times(t);
        vec![self.tier as i32, self.recipe.map_or(0, |i| i as i32 + 1), i32::from(times > 0), times as i32, i32::from(self.smithing(t).is_some())]
    }

    /// A button of its screen; true when it opens the recipe list.
    pub fn press(&mut self, t: &Tables, id: u32) -> bool {
        if let Some(pick) = crate::native::recipe_button(id) {
            // The native list: only its tier's constructions.
            self.recipe = pick.filter(|&i| t.construction.get(i).is_some_and(|c| c.tier_valid(self.tier)));
            return false;
        }
        match id {
            buttons::PICK => {
                self.picking = true;
                return true;
            }
            buttons::CRAFT => {
                self.craft(t, 1);
            }
            buttons::CRAFT_MAX => {
                if let Some(c) = self.recipe.and_then(|i| t.construction.get(i)) {
                    let n = Self::max_times(c);
                    self.craft(t, n);
                }
            }
            buttons::FORGE => {
                self.forge(t);
            }
            _ => {}
        }
        false
    }
}

/// A number in [0, 1) from the crafts made and the output.
fn roll(made: u64, i: u64) -> f32 {
    (crate::hash::mix(crate::hash::mix(made) ^ i) >> 40) as f32 / (1u64 << 24) as f32
}

impl Logic for Anvil {
    fn level(&self, _max: u8) -> u8 {
        0
    }

    /// The construction list (the anvil's screen with slots opens it);
    /// otherwise what is picked.
    fn view(&self) -> ViewModel {
        self.view_with(Tables::ntm())
    }

    fn action(&mut self, action: u32, _held: Option<&crate::machine::Held>, _fx: &mut Vec<crate::machine::Effect>) {
        match action {
            actions::BACK => self.picking = false,
            a if (actions::LIST..actions::LIST + view::LIST_MAX as u32).contains(&a) => {
                self.recipe = Tables::ntm().listed(self.tier).get((a - actions::LIST) as usize).copied();
                self.picking = false;
            }
            _ => {}
        }
    }

    fn contents(&self) -> Vec<(String, u32)> {
        self.slots.iter().flatten().map(|s| (s.id.clone(), s.count)).collect()
    }
}

impl Anvil {
    fn view_with(&self, t: &Tables) -> ViewModel {
        let label = |c: &Construction| if c.label.is_empty() { c.outputs.first().map_or(c.name.clone(), |o| o.0.clone()) } else { c.label.clone() };
        if self.picking {
            let listed = t.listed(self.tier);
            let rows: Vec<String> = listed.iter().map(|&i| label(&t.construction[i])).collect();
            let selected = self.recipe.and_then(|r| listed.iter().position(|&i| i == r)).map(|p| p as u32);
            return ViewModel::new(format!("Bigorna (nível {}): construção", self.tier)).list(actions::LIST, rows, selected).button(actions::BACK, "Voltar");
        }
        let picked = self.recipe.and_then(|i| t.construction.get(i)).map_or("nenhuma".into(), label);
        ViewModel::new(format!("Bigorna (nível {})", self.tier))
            .text(format!("Construção: {picked}"), view::GREY)
            .text("Os itens e os botões ficam na tela com slots (motor com o patch ntm-visual).", view::GREY)
    }
}

impl Slotted for Anvil {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        Anvil::screen_data(self, Tables::ntm())
    }
    fn button(&mut self, id: u32) -> bool {
        self.press(Tables::ntm(), id)
    }
    /// The recipe picked and the chance roll.
    fn save_state(&self) -> Vec<i32> {
        vec![self.recipe.map_or(0, |i| i as i32 + 1), self.made as i32]
    }
    fn load_state(&mut self, v: &[i32]) {
        if let [r, made, ..] = v {
            self.recipe = usize::try_from(*r - 1).ok();
            self.made = (*made).max(0) as u64;
        }
    }
}

impl Tables {
    /// The NTM's (`data/machines/anvil.json`, from the content agent's
    /// extraction by `tools/machine-gen/build.py`).
    pub fn ntm() -> &'static Tables {
        static T: std::sync::OnceLock<Tables> = std::sync::OnceLock::new();
        T.get_or_init(|| serde_json::from_str(include_str!("../data/machines/anvil.json")).expect("data/machines/anvil.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(ids: &[&str], count: u32) -> Input {
        Input { ids: ids.iter().map(|s| s.to_string()).collect(), count, exact: false }
    }

    fn tables() -> Tables {
        Tables {
            smithing: vec![
                // An anvil upgrade.
                Smithing { name: "steel".into(), tier: 1, left: input(&["ntm:anvil_iron"], 1), right: input(&["ntm:ingot_steel"], 10), out: ("ntm:anvil_steel".into(), 1), shapeless: false, consume: (1, 10) },
                // A mold: the example (exactly 1 ingot) stays.
                Smithing {
                    name: "mold_ingot".into(),
                    tier: 1,
                    left: Input { ids: vec!["minecraft:iron_ingot".into(), "ntm:ingot_steel".into()], count: 1, exact: true },
                    right: input(&["ntm:mold_base"], 1),
                    out: ("ntm:mold__ingot".into(), 1),
                    shapeless: false,
                    consume: (0, 1),
                },
                // Gunmetal, either way round.
                Smithing { name: "gunmetal".into(), tier: 1, left: input(&["ntm:ingot_copper"], 1), right: input(&["ntm:ingot_aluminium"], 1), out: ("ntm:ingot_gunmetal".into(), 1), shapeless: true, consume: (1, 1) },
            ],
            construction: vec![
                Construction {
                    name: "pipe".into(),
                    label: "Cano".into(),
                    inputs: vec![input(&["ntm:plate_steel"], 3)],
                    outputs: vec![("ntm:pipe_steel".into(), 1, 1.0)],
                    tier_lower: 1,
                    tier_upper: -1,
                },
                Construction {
                    name: "plate".into(),
                    label: "Chapa".into(),
                    inputs: vec![input(&["ntm:ingot_steel"], 1)],
                    outputs: vec![("ntm:plate_steel".into(), 1, 1.0)],
                    tier_lower: 3,
                    tier_upper: -1,
                },
                Construction {
                    name: "recycle".into(),
                    label: "Reciclagem".into(),
                    inputs: vec![input(&["ntm:chunk_ore__rare"], 1)],
                    outputs: vec![("ntm:fragment_a".into(), 1, 1.0), ("ntm:fragment_b".into(), 1, 0.5)],
                    tier_lower: 1,
                    tier_upper: 2,
                },
            ],
        }
    }

    fn st(id: &str, n: u32) -> Option<Stack> {
        Some(Stack::new(id, n))
    }

    #[test]
    fn smithing_upgrades_an_anvil_and_engraves_molds() {
        let t = tables();
        let mut a = Anvil::new(1);
        a.slots[LEFT] = st("ntm:anvil_iron", 1);
        a.slots[RIGHT] = st("ntm:ingot_steel", 12);
        assert!(a.forge(&t));
        assert_eq!((a.slots[LEFT].clone(), a.slots[RIGHT].clone(), a.slots[FORGE_OUT].clone()), (None, st("ntm:ingot_steel", 2), st("ntm:anvil_steel", 1)));
        // A mold: exactly one example ingot, which stays.
        let mut a = Anvil::new(1);
        a.slots[LEFT] = st("minecraft:iron_ingot", 2);
        a.slots[RIGHT] = st("ntm:mold_base", 1);
        assert!(!a.forge(&t), "dois lingotes de exemplo: não casa");
        a.slots[LEFT] = st("minecraft:iron_ingot", 1);
        assert!(a.forge(&t));
        assert_eq!((a.slots[LEFT].clone(), a.slots[RIGHT].clone(), a.slots[FORGE_OUT].clone()), (st("minecraft:iron_ingot", 1), None, st("ntm:mold__ingot", 1)));
        // Shapeless: either way round.
        let mut a = Anvil::new(1);
        a.slots[LEFT] = st("ntm:ingot_aluminium", 1);
        a.slots[RIGHT] = st("ntm:ingot_copper", 1);
        assert!(a.forge(&t));
        assert_eq!(a.slots[FORGE_OUT], st("ntm:ingot_gunmetal", 1));
        // The output slot holds something else: nothing.
        a.slots[LEFT] = st("ntm:anvil_iron", 1);
        a.slots[RIGHT] = st("ntm:ingot_steel", 10);
        assert!(!a.forge(&t));
        assert_eq!(a.slots[LEFT], st("ntm:anvil_iron", 1));
    }

    #[test]
    fn construction_takes_from_the_grid_within_the_tier() {
        let t = tables();
        let mut a = Anvil::new(1);
        assert_eq!(t.listed(1), vec![0, 2], "a chapa é de nível 3");
        a.recipe = Some(0);
        a.slots[3] = st("ntm:plate_steel", 4);
        a.slots[7] = st("ntm:plate_steel", 4);
        assert_eq!(a.times(&t), 2);
        assert_eq!(a.craft(&t, 1), 1);
        assert_eq!(a.slots[OUT.start], st("ntm:pipe_steel", 1));
        assert_eq!(a.slots[3].as_ref().map(|s| s.count).unwrap_or(0) + a.slots[7].as_ref().map(|s| s.count).unwrap_or(0), 5);
        // As many as there are: 5 plates, one more pipe.
        assert_eq!(a.craft(&t, Anvil::max_times(&t.construction[0])), 1);
        assert_eq!(a.slots[OUT.start], st("ntm:pipe_steel", 2));
        // A tier 3 recipe on a tier 1 anvil: nothing.
        a.recipe = Some(1);
        a.slots[4] = st("ntm:ingot_steel", 5);
        assert_eq!(a.craft(&t, 1), 0);
        assert_eq!(Anvil::new(3).listed_len(&t), 2, "nível 3: cano e chapa; a reciclagem vai até o 2");
    }

    #[test]
    fn outputs_must_fit_and_chances_are_rolled() {
        let t = tables();
        let mut a = Anvil::new(1);
        a.recipe = Some(2);
        a.slots[3] = st("ntm:chunk_ore__rare", 64);
        let n = a.craft(&t, 64);
        assert_eq!(n, 64);
        let count = |a: &Anvil, id: &str| a.slots[OUT].iter().flatten().filter(|s| s.id == id).map(|s| s.count).sum::<u32>();
        assert_eq!(count(&a, "ntm:fragment_a"), 64);
        let b = count(&a, "ntm:fragment_b");
        assert!((20..=44).contains(&b), "metade, mais ou menos: {b}");
        // The outputs full (4 slots): it stops, the inputs stay.
        a.slots[3] = st("ntm:chunk_ore__rare", 64);
        for i in OUT {
            a.slots[i] = st("ntm:fragment_a", 64);
        }
        assert_eq!(a.craft(&t, 1), 0);
        assert_eq!(a.slots[3], st("ntm:chunk_ore__rare", 64));
    }

    #[test]
    fn the_ntm_tables_are_embedded() {
        let t = Tables::ntm();
        assert_eq!((t.smithing.len(), t.construction.len()), (36, 136));
        // The 17 mold engravings: the example stays, exactly one.
        let molds: Vec<_> = t.smithing.iter().filter(|s| s.out.0.starts_with("ntm:mold__")).collect();
        assert_eq!(molds.len(), 17);
        assert!(molds.iter().all(|s| s.consume == (0, 1) && s.left.exact));
        // Every anvil above lead comes from iron or lead and 10 ingots.
        assert_eq!(t.smithing.iter().filter(|s| s.out.0.starts_with("ntm:anvil_")).count(), 18);
        assert!(ANVILS.iter().all(|(id, _)| crate::registry::spec(id).is_some()));
    }

    impl Anvil {
        fn listed_len(&self, t: &Tables) -> usize {
            t.listed(self.tier).len()
        }
    }
}
