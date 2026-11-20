//! The automatic crafting table (`machine_autocrafter`), from `tileentity/machine/TileEntityMachineAutocrafter.java`,
//! `inventory/container/ContainerAutocrafter.java` and `module/ModulePatternMatcher.java`. 21 slots:
//! - 0-8 the template (`SlotPattern`: a copy of what the cursor holds, never an item), each with a filter mode;
//! - 9 the template's result, one of the crafting recipes the template matches (a right click shows the next);
//! - 10-18 the inputs, 19 the output, 20 the battery.
//!
//! Each tick with 100 HE and a recipe chosen, when that recipe also matches the inputs, it crafts one into the output
//! and takes one of each input, a container left where an input ran out (`updateEntity`, lines 77-115). The recipes
//! are the world's crafting tables, asked of the engine (`ServerHost::crafting_matches`) when the template or the
//! inputs change. A template slot's mode sits on its copy (`MODE_KEY` in its custom data), so the slots' rules, which
//! see only the slots, read it there; a right click on a template slot is menu button `MODE_BUTTON + slot`, on the
//! result `NEXT_RECIPE`.

use crate::{
    energy::{EnergyNode, Role},
    machine::{Logic, charge_from_slot},
    server::container_bridge::{Slots, Slotted, Stack},
    snbt,
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub const BLOCK: &str = "ntm:machine_autocrafter";
pub const SLOTS: usize = 21;
pub const TEMPLATE: std::ops::Range<usize> = 0..9;
pub const RESULT: usize = 9;
pub const INPUTS: std::ops::Range<usize> = 10..19;
pub const OUTPUT: usize = 19;
pub const BATTERY: usize = 20;
/// `consumption` and `maxPower` (lines 334-335).
pub const CONSUMPTION: u64 = 100;
pub const MAX_POWER: u64 = CONSUMPTION * 100;
/// The menu button of template slot `i`'s right click (`nextMode(i)`) is `MODE_BUTTON + i`.
pub const MODE_BUTTON: u32 = 100;
/// The result slot's right click (`nextTemplate`).
pub const NEXT_RECIPE: u32 = 109;
/// The custom data field of a template copy that holds its filter mode.
pub const MODE_KEY: &str = "ntm_autocrafter_mode";
/// `isItemValidForSlot`: no input slot holds more than 4 (line 216).
pub const INPUT_MAX: u32 = 4;

/// `ModulePatternMatcher`'s modes: an OreDict name for any other.
pub const EXACT: &str = "exact";
pub const WILDCARD: &str = "wildcard";
pub const BEDROCK: &str = "bedrock";

/// A crafting recipe that matches a grid (`IRecipe.matches`), as the engine answers it: its id, what it makes and how
/// many of that an output slot holds, and what each of the nine cells keeps once it is made (a container).
#[derive(Clone, Debug, PartialEq)]
pub struct CraftMatch {
    pub id: String,
    pub result: Stack,
    pub max_stack: u32,
    pub remainders: Vec<Option<Stack>>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Autocrafter {
    pub slots: Slots,
    pub power: u64,
    /// `recipeIndex`: which of the template's recipes it makes.
    pub recipe_index: usize,
    /// The recipes the template matches (`recipes`, `getMatchingRecipes(getTemplateGrid())`).
    #[serde(skip)]
    pub recipes: Vec<CraftMatch>,
    /// The template the recipes are of (without the modes); None before the first look in a session.
    #[serde(skip)]
    template_seen: Option<Vec<Option<Stack>>>,
    /// The recipes the inputs match, and the inputs they are of.
    #[serde(skip)]
    pub inputs_match: Vec<CraftMatch>,
    #[serde(skip)]
    inputs_seen: Option<Vec<Option<Stack>>>,
}

impl Autocrafter {
    pub fn new() -> Self {
        Self { slots: vec![None; SLOTS], ..Default::default() }
    }

    /// Reads the recipes again where the template or the inputs changed, through `query` (the engine's
    /// `crafting_matches` of a 3x3 grid). A template slot clicked (`slotClick`: its item changed, or a copy without a
    /// mode, which the engine has just put there, even of the same item) takes its `initPatternSmart` mode and the
    /// recipe goes back to the first (`updateTemplateGrid`); the first look of a session keeps the saved modes and
    /// recipe (`readFromNBT`).
    pub fn refresh(&mut self, mut query: impl FnMut(&[Option<Stack>]) -> Vec<CraftMatch>) {
        self.slots.resize(SLOTS, None);
        let template: Vec<Option<Stack>> = self.slots[TEMPLATE].iter().map(|s| s.as_ref().map(without_mode)).collect();
        let first = self.template_seen.is_none();
        let clicked: Vec<bool> = TEMPLATE
            .map(|i| {
                self.slots[i].as_ref().is_some_and(|s| mode_of(s).is_none())
                    || self.template_seen.as_ref().is_some_and(|seen| seen[i] != template[i])
            })
            .collect();
        if first || clicked.contains(&true) {
            for i in TEMPLATE.filter(|&i| clicked[i]) {
                let mode = template[i].as_ref().map(|s| smart_mode(&s.id));
                self.slots[i] = template[i].clone().map(|s| with_mode(s, mode.as_deref()));
            }
            self.recipes = query(&template);
            if clicked.contains(&true) || self.recipe_index >= self.recipes.len() {
                self.recipe_index = 0;
            }
            self.template_seen = Some(template);
            self.show_result();
        }
        let inputs: Vec<Option<Stack>> = self.slots[INPUTS].to_vec();
        if self.inputs_seen.as_ref() != Some(&inputs) {
            self.inputs_match = query(&inputs);
            self.inputs_seen = Some(inputs);
        }
    }

    /// `slots[9]`: the chosen recipe's result on the template.
    fn show_result(&mut self) {
        self.slots[RESULT] = self.recipes.get(self.recipe_index).map(|r| r.result.clone());
    }

    /// `nextTemplate`: the next recipe the template matches, back to the first after the last.
    pub fn next_recipe(&mut self) {
        self.recipe_index += 1;
        if self.recipe_index >= self.recipes.len() {
            self.recipe_index = 0;
        }
        self.show_result();
    }

    /// `nextMode(i)` on template slot `i`.
    pub fn next_mode(&mut self, i: usize) {
        if let Some(s) = self.slots.get_mut(i).filter(|_| TEMPLATE.contains(&i)).and_then(Option::take) {
            let mode = next_mode(&s.id, mode_of(&s).as_deref());
            self.slots[i] = Some(with_mode(s, Some(&mode)));
        }
    }

    /// One `updateEntity`; true when it crafted.
    pub fn step(&mut self) -> bool {
        self.slots.resize(SLOTS, None);
        self.power = charge_from_slot(&self.slots, BATTERY, self.power, MAX_POWER);
        if self.recipes.is_empty() || self.power < CONSUMPTION {
            return false;
        }
        let Some(chosen) = self.recipes.get(self.recipe_index) else { return false };
        // `recipe.matches(getRecipeGrid())`, then `getCraftingResult` of the inputs.
        let Some(made) = self.inputs_match.iter().find(|m| m.id == chosen.id).cloned() else { return false };
        let fits = match &self.slots[OUTPUT] {
            None => true,
            Some(o) => same_item(o, &made.result) && o.count + made.result.count <= made.max_stack,
        };
        if !fits {
            return false;
        }
        match &mut self.slots[OUTPUT] {
            Some(o) => o.count += made.result.count,
            slot => *slot = Some(made.result.clone()),
        }
        for (k, i) in INPUTS.enumerate() {
            let Some(input) = self.slots[i].as_mut() else { continue };
            input.count -= 1;
            if input.count == 0 {
                // A container where the stack ran out (`getContainerItem`); one beside items still there is lost.
                self.slots[i] = made.remainders.get(k).cloned().flatten();
            }
        }
        self.power -= CONSUMPTION;
        // The inputs changed: what they match is asked again before the next craft.
        self.inputs_seen = None;
        self.inputs_match.clear();
        true
    }
}

/// `isItemEqual` and `areItemStackTagsEqual`: what piles on the output.
fn same_item(a: &Stack, b: &Stack) -> bool {
    a.id == b.id && a.damage == b.damage && a.custom == b.custom
}

/// A template copy's mode (`modes[i]`), or None.
pub fn mode_of(s: &Stack) -> Option<String> {
    let value = snbt::fields(s.custom.as_deref()?)?.remove(MODE_KEY)?;
    Some(value.trim_matches(|c| c == '"' || c == '\'').to_string())
}

/// The copy with mode `mode` (None: no mode).
pub fn with_mode(mut s: Stack, mode: Option<&str>) -> Stack {
    s.custom = match mode {
        Some(m) => Some(snbt::with_field(s.custom.as_deref(), MODE_KEY, &format!("{m:?}"))),
        None => s.custom.as_deref().and_then(|c| snbt::without_field(c, MODE_KEY)),
    };
    s
}

/// The copy without its mode: the item the template names.
pub fn without_mode(s: &Stack) -> Stack {
    with_mode(s.clone(), None)
}

const COLORS: [&str; 16] = [
    "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple", "blue", "brown", "green", "red",
    "black",
];
const DYES: [&str; 16] = [
    "ink_sac", "red_dye", "green_dye", "cocoa_beans", "lapis_lazuli", "purple_dye", "cyan_dye", "light_gray_dye", "gray_dye", "pink_dye", "lime_dye",
    "yellow_dye", "light_blue_dye", "magenta_dye", "orange_dye", "bone_meal",
];
const WOODS: [&str; 6] = ["oak", "spruce", "birch", "jungle", "acacia", "dark_oak"];

/// The 1.7.10 item (one id, several metas) a vanilla id of 26.3 was a meta of: `minecraft:planks` for every plank,
/// `minecraft:dye` for every dye. None for an item that had one meta.
fn vanilla_family(id: &str) -> Option<&'static str> {
    let path = id.strip_prefix("minecraft:")?;
    let colored = |suffix: &str| path.strip_suffix(suffix).is_some_and(|c| COLORS.contains(&c));
    let wood = |suffix: &str| path.strip_suffix(suffix).is_some_and(|w| WOODS.contains(&w));
    Some(match path {
        _ if wood("_planks") => "planks",
        _ if wood("_slab") => "wooden_slab",
        _ if wood("_sapling") => "sapling",
        "oak_log" | "spruce_log" | "birch_log" | "jungle_log" => "log",
        "acacia_log" | "dark_oak_log" => "log2",
        "oak_leaves" | "spruce_leaves" | "birch_leaves" | "jungle_leaves" => "leaves",
        "acacia_leaves" | "dark_oak_leaves" => "leaves2",
        _ if colored("_wool") => "wool",
        _ if colored("_carpet") => "carpet",
        _ if colored("_terracotta") => "stained_hardened_clay",
        _ if colored("_stained_glass") => "stained_glass",
        _ if colored("_stained_glass_pane") => "stained_glass_pane",
        _ if DYES.contains(&path) => "dye",
        "sand" | "red_sand" => "sand",
        "stone_bricks" | "mossy_stone_bricks" | "cracked_stone_bricks" | "chiseled_stone_bricks" => "stonebrick",
        "sandstone" | "chiseled_sandstone" | "cut_sandstone" => "sandstone",
        "quartz_block" | "chiseled_quartz_block" | "quartz_pillar" => "quartz_block",
        "coal" | "charcoal" => "coal",
        "golden_apple" | "enchanted_golden_apple" => "golden_apple",
        _ => return None,
    })
}

/// `getHasSubtypes`: an item of several metas, which the port names `base__variant` (the NTM's) or splits into ids
/// of one 1.7.10 item (vanilla's: `vanilla_family`).
fn has_subtypes(id: &str) -> bool {
    (id.starts_with("ntm:") && id.contains("__")) || vanilla_family(id).is_some()
}

/// `getItem()`: the item without its meta.
fn base(id: &str) -> &str {
    vanilla_family(id).unwrap_or_else(|| id.split_once("__").map_or(id, |(b, _)| b))
}

/// `ItemBedrockOreNew.getGrade`: `ntm:bedrock_ore__<grade>_<type>`.
fn bedrock_grade(id: &str) -> Option<&str> {
    let variant = id.strip_prefix("ntm:bedrock_ore__")?;
    variant.rsplit_once('_').map(|(grade, _)| grade)
}

/// `initPatternSmart`: the first OreDict name for an ingot, a block, a dust, a nugget or a plate, else bedrock for a
/// bedrock ore, exact for an item of several metas and wildcard for any other. "First" is in name order: the port's
/// OreDict (`crate::oredict::names`) keeps an item's names sorted, where `getOreIDs` gives them in the order they
/// were registered, so an item with two names of one prefix (gunpowder: `dustAnyPropellant`, `dustGunpowder`) may
/// take the other one.
pub fn smart_mode(id: &str) -> String {
    let names = crate::oredict::names(id);
    for prefix in ["ingot", "block", "dust", "nugget", "plate"] {
        if let Some(n) = names.iter().find(|n| n.starts_with(prefix)) {
            return n.clone();
        }
    }
    if bedrock_grade(id).is_some() {
        BEDROCK.into()
    } else if has_subtypes(id) {
        EXACT.into()
    } else {
        WILDCARD.into()
    }
}

/// `nextMode`: exact, then bedrock (a bedrock ore) or wildcard, then each OreDict name in turn, then exact again.
pub fn next_mode(id: &str, mode: Option<&str>) -> String {
    let names = crate::oredict::names(id);
    match mode {
        None => EXACT.into(),
        Some(EXACT) if bedrock_grade(id).is_some() => BEDROCK.into(),
        Some(EXACT) | Some(BEDROCK) => WILDCARD.into(),
        Some(WILDCARD) => names.first().cloned().unwrap_or_else(|| EXACT.into()),
        Some(name) => match names.iter().position(|n| n == name) {
            Some(k) if k + 1 < names.len() => names[k + 1].clone(),
            _ => EXACT.into(),
        },
    }
}

/// `isValidForFilter`: whether `input` passes template `filter` in `mode` (exact when it has none). Exact is
/// `isItemEqual`: the item, its meta (the port's id) and its damage.
pub fn valid_for_filter(filter: &Stack, mode: Option<&str>, input: &Stack) -> bool {
    let (filter_id, input_id) = (filter.id.as_str(), input.id.as_str());
    match mode.unwrap_or(EXACT) {
        EXACT => input_id == filter_id && input.damage == filter.damage,
        WILDCARD => base(input_id) == base(filter_id),
        BEDROCK => base(input_id) == base(filter_id) && bedrock_grade(input_id).is_some() && bedrock_grade(input_id) == bedrock_grade(filter_id),
        name => crate::oredict::has(input_id, name),
    }
}

/// The label of a mode (`ModulePatternMatcher.getLabel`), for the template slot's tooltip.
pub fn mode_label(mode: &str) -> String {
    match mode {
        EXACT => "Item and meta match".into(),
        WILDCARD => "Item matches".into(),
        BEDROCK => "Item and bedrock grade match".into(),
        name => format!("Ore dict key matches: {name}"),
    }
}

/// A template copy's tooltip lines (`GUIAutocrafter.drawScreen`, lines 38-44): how to change its mode and the mode.
pub fn template_lines(s: &Stack) -> Vec<String> {
    match mode_of(s) {
        Some(mode) => vec!["§cRight click to change".into(), format!("§e{}", mode_label(&mode))],
        None => Vec::new(),
    }
}

/// `hasContainerItem`: a filled container (a bucket, a fluid container) leaves its empty one in the grid.
fn has_container_item(id: &str) -> bool {
    (id.ends_with("_bucket") && id != "minecraft:bucket")
        || matches!(id, "minecraft:honey_bottle" | "minecraft:dragon_breath")
        || crate::fluid::contents(id).is_some()
}

/// `isItemValidForSlot` (lines 244-299): a hopper puts an item into an input whose template it passes, one at a time
/// to the emptiest of the inputs it passes, never more than 4 and never a stacked container.
pub fn can_insert(slots: &[Option<Stack>], slot: usize, item: &Stack) -> bool {
    if item.count > 1 && has_container_item(&item.id) {
        return false;
    }
    if !INPUTS.contains(&slot) {
        return false;
    }
    // No template above it: no input.
    if slots.get(slot - INPUTS.start).is_none_or(Option::is_none) {
        return false;
    }
    let held = slots.get(slot).cloned().flatten();
    if held.as_ref().is_some_and(|h| h.count + item.count > INPUT_MAX) || item.count > INPUT_MAX {
        return false;
    }
    let mut valid = Vec::new();
    for i in TEMPLATE {
        let Some(filter) = slots.get(i).cloned().flatten() else { continue };
        let Some(mode) = mode_of(&filter).filter(|m| !m.is_empty()) else { continue };
        if valid_for_filter(&without_mode(&filter), Some(&mode), item) {
            valid.push(i + INPUTS.start);
            if i + INPUTS.start == slot && held.is_none() {
                return true;
            }
        }
    }
    if !valid.contains(&slot) {
        return false;
    }
    let size = held.as_ref().map_or(0, |h| h.count);
    for i in valid {
        let Some(other) = slots.get(i).cloned().flatten() else { return false };
        if other.id != item.id {
            continue;
        }
        if other.count < size {
            return false;
        }
    }
    !has_container_item(&item.id)
}

/// `canExtractItem` (lines 230-241): a hopper takes the output, and from an input what its template no longer passes.
pub fn can_extract(slots: &[Option<Stack>], slot: usize, item: &Stack) -> bool {
    if slot == OUTPUT {
        return true;
    }
    if !INPUTS.contains(&slot) {
        return false;
    }
    let Some(filter) = slots.get(slot - INPUTS.start).cloned().flatten() else { return true };
    match mode_of(&filter).filter(|m| !m.is_empty()) {
        None => true,
        Some(mode) => !valid_for_filter(&without_mode(&filter), Some(&mode), item),
    }
}

/// What a hand and a hopper may do with slot `slot` (the template's are the engine's template slots). The inputs are
/// `SlotNonRetarded`s (`ContainerAutocrafter`, line 30): a hand puts into them what `isItemValidForSlot` takes, as a
/// hopper does; the output and the battery are plain `Slot`s, which take anything by hand. A hand takes from all; a
/// hopper reaches only the inputs and the output (`getAccessibleSlotsFromSide`, `canExtractItem`).
pub fn allows(slots: &[Option<Stack>], slot: usize, item: &Stack, insert: bool, manual: bool) -> bool {
    if slot >= SLOTS || slot <= RESULT {
        return false;
    }
    match (manual, insert) {
        (true, true) if slot == OUTPUT || slot == BATTERY => true,
        (_, true) => can_insert(slots, slot, item),
        (true, false) => true,
        (false, false) => can_extract(slots, slot, item),
    }
}

/// `getAccessibleSlotsFromSide`: every face reaches the inputs and the output (`access`, line 226).
pub fn container() -> crate::native::Container {
    let access: Vec<usize> = INPUTS.chain(std::iter::once(OUTPUT)).collect();
    crate::native::Container {
        slots: SLOTS,
        faces: std::array::from_fn(|_| access.clone()),
        inputs: INPUTS.collect(),
        outputs: vec![OUTPUT],
        menu: Some("minecraft:generic_9x3"),
    }
}

impl Logic for Autocrafter {
    fn tick(&mut self) -> bool {
        self.step()
    }
    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        Some(self)
    }
    fn level(&self, _max: u8) -> u8 {
        0
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Mesa de montagem automática").bar("Energia", self.power, MAX_POWER, "HE").text(
            format!("Receita {} / {}", self.recipe_index + usize::from(!self.recipes.is_empty()), self.recipes.len()),
            view::GREY,
        )
    }
}

impl EnergyNode for Autocrafter {
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
        self.power = (self.power + he).min(MAX_POWER);
    }
    fn take(&mut self, _he: u64) {}
}

impl Slotted for Autocrafter {
    /// Its record keeps its slots (the model's JSON): a restore from the journal has them.
    fn slots_recorded(&self) -> bool {
        true
    }
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    /// The power bar (`drawGuiContainerBackgroundLayer`) and the result's `recipeIndex + 1 / recipeCount`.
    fn screen_data(&self) -> Vec<i32> {
        let n = self.recipes.len();
        vec![self.power as i32, MAX_POWER as i32, (self.recipe_index + usize::from(n > 0)) as i32, n as i32]
    }
    fn button(&mut self, id: u32) -> bool {
        match id {
            NEXT_RECIPE => self.next_recipe(),
            b if (MODE_BUTTON..MODE_BUTTON + TEMPLATE.end as u32).contains(&b) => self.next_mode((b - MODE_BUTTON) as usize),
            _ => {}
        }
        false
    }
    fn save_state(&self) -> Vec<i32> {
        vec![self.power as i32, self.recipe_index as i32]
    }
    fn load_state(&mut self, v: &[i32]) {
        if let [power, index, ..] = v {
            self.power = (*power).clamp(0, MAX_POWER as i32) as u64;
            self.recipe_index = (*index).max(0) as usize;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(id: &str, n: u32) -> Option<Stack> {
        Some(Stack::new(id, n))
    }

    /// The engine's answer for a grid of planks above planks (sticks) or of one log (planks): two recipes for the
    /// two planks, so the result can be turned.
    fn query(grid: &[Option<Stack>]) -> Vec<CraftMatch> {
        let ids: Vec<&str> = grid.iter().flatten().map(|s| s.id.as_str()).collect();
        let craft = |id: &str, out: &str, n: u32| CraftMatch {
            id: id.into(),
            result: Stack::new(out, n),
            max_stack: 64,
            remainders: grid.iter().map(|c| c.as_ref().filter(|c| c.id == "minecraft:water_bucket").map(|_| Stack::new("minecraft:bucket", 1))).collect(),
        };
        match ids.as_slice() {
            ["minecraft:oak_planks", "minecraft:oak_planks"] => vec![craft("minecraft:stick", "minecraft:stick", 4), craft("mod:other_stick", "minecraft:ladder", 1)],
            ["minecraft:water_bucket", "minecraft:sand"] => vec![craft("mod:mud", "minecraft:mud", 1)],
            _ => Vec::new(),
        }
    }

    /// A placed machine is first seen empty (`refresh` before the template), as the server sees it.
    fn placed() -> Autocrafter {
        let mut a = Autocrafter::new();
        a.refresh(query);
        a
    }

    fn template(a: &mut Autocrafter, cells: &[(usize, &str)]) {
        for (i, id) in cells {
            a.slots[*i] = s(id, 1);
        }
        a.refresh(query);
    }

    #[test]
    fn the_template_picks_the_recipes_and_the_result_turns() {
        let mut a = Autocrafter::new();
        a.refresh(query);
        assert!(a.recipes.is_empty() && a.slots[RESULT].is_none());
        template(&mut a, &[(1, "minecraft:oak_planks"), (4, "minecraft:oak_planks")]);
        assert_eq!(a.recipes.len(), 2);
        assert_eq!(a.slots[RESULT], s("minecraft:stick", 4));
        assert_eq!(a.screen_data()[2..], [1, 2]);
        // initPatternSmart: planks have the OreDict name plankWood (no ingot, block...) and several metas in 1.7.10
        // (`Blocks.planks`): exact.
        assert_eq!(mode_of(a.slots[1].as_ref().unwrap()).as_deref(), Some(EXACT));
        a.button(NEXT_RECIPE);
        assert_eq!((a.recipe_index, a.slots[RESULT].clone()), (1, s("minecraft:ladder", 1)));
        a.button(NEXT_RECIPE);
        assert_eq!(a.recipe_index, 0, "after the last, the first");
        // A changed template goes back to its first recipe; an unchanged one keeps it.
        a.button(NEXT_RECIPE);
        a.refresh(query);
        assert_eq!(a.recipe_index, 1);
        template(&mut a, &[(4, "minecraft:dirt")]);
        assert!(a.recipes.is_empty() && a.slots[RESULT].is_none() && a.recipe_index == 0);
    }

    #[test]
    fn it_crafts_one_a_tick_with_power_into_the_output() {
        let mut a = placed();
        template(&mut a, &[(1, "minecraft:oak_planks"), (4, "minecraft:oak_planks")]);
        a.slots[11] = s("minecraft:oak_planks", 2);
        a.slots[14] = s("minecraft:oak_planks", 3);
        a.refresh(query);
        assert!(!a.step(), "no power");
        a.power = 250;
        assert!(a.step());
        assert_eq!((a.slots[OUTPUT].clone(), a.slots[11].clone(), a.slots[14].clone(), a.power), (s("minecraft:stick", 4), s("minecraft:oak_planks", 1), s("minecraft:oak_planks", 2), 150));
        assert!(!a.step(), "the inputs are asked again before the next craft");
        a.refresh(query);
        assert!(a.step());
        assert_eq!((a.slots[OUTPUT].clone(), a.slots[11].clone(), a.power), (s("minecraft:stick", 8), None, 50));
        a.refresh(query);
        a.power = 1_000;
        assert!(!a.step(), "the inputs no longer match");
        // The chosen recipe must be the one the inputs make, and the output must take it.
        a.slots[11] = s("minecraft:oak_planks", 1);
        a.slots[OUTPUT] = s("minecraft:stick", 62);
        a.refresh(query);
        assert!(!a.step(), "62 + 4 passes 64");
        a.slots[OUTPUT] = s("minecraft:ladder", 1);
        assert!(!a.step(), "another item in the output");
        a.slots[OUTPUT] = None;
        a.button(NEXT_RECIPE);
        assert!(a.step());
        assert_eq!(a.slots[OUTPUT], s("minecraft:ladder", 1));
    }

    #[test]
    fn a_container_stays_where_its_stack_ran_out() {
        let mut a = placed();
        template(&mut a, &[(0, "minecraft:water_bucket"), (1, "minecraft:sand")]);
        a.slots[10] = s("minecraft:water_bucket", 1);
        a.slots[11] = s("minecraft:sand", 2);
        a.power = MAX_POWER;
        a.refresh(query);
        assert!(a.step());
        assert_eq!((a.slots[10].clone(), a.slots[11].clone(), a.slots[OUTPUT].clone()), (s("minecraft:bucket", 1), s("minecraft:sand", 1), s("minecraft:mud", 1)));
    }

    #[test]
    fn the_modes_turn_as_the_pattern_matcher_does() {
        assert_eq!(smart_mode("minecraft:iron_ingot"), "ingotIron");
        assert_eq!(smart_mode("ntm:bedrock_ore__base_heavy"), BEDROCK);
        assert_eq!(smart_mode("ntm:powder_ash__misc"), EXACT, "an item of several metas");
        assert_eq!(smart_mode("minecraft:stick"), WILDCARD, "stickWood is no ingot, block, dust, nugget or plate");
        assert_eq!(smart_mode("minecraft:oak_planks"), EXACT, "Blocks.planks has several metas");
        assert_eq!(next_mode("minecraft:oak_planks", Some(EXACT)), WILDCARD);
        assert_eq!(next_mode("minecraft:oak_planks", Some(WILDCARD)), "plankWood");
        assert_eq!(smart_mode("minecraft:dirt"), WILDCARD);
        assert_eq!(next_mode("minecraft:dirt", None), EXACT);
        assert_eq!(next_mode("minecraft:dirt", Some(EXACT)), WILDCARD);
        assert_eq!(next_mode("minecraft:dirt", Some(WILDCARD)), EXACT, "no OreDict name");
        assert_eq!(next_mode("ntm:bedrock_ore__base_heavy", Some(EXACT)), BEDROCK);
        assert_eq!(next_mode("ntm:bedrock_ore__base_heavy", Some(BEDROCK)), WILDCARD);
        let names = crate::oredict::names("minecraft:iron_ingot");
        assert_eq!(next_mode("minecraft:iron_ingot", Some(WILDCARD)), names[0]);
        assert_eq!(next_mode("minecraft:iron_ingot", Some(names.last().unwrap())), EXACT);
        // Through the menu button, on the copy.
        let mut a = placed();
        template(&mut a, &[(3, "minecraft:dirt")]);
        a.button(MODE_BUTTON + 3);
        assert_eq!(mode_of(a.slots[3].as_ref().unwrap()).as_deref(), Some(EXACT));
        a.button(MODE_BUTTON + 3);
        assert_eq!(mode_of(a.slots[3].as_ref().unwrap()).as_deref(), Some(WILDCARD));
        assert_eq!(template_lines(a.slots[3].as_ref().unwrap()), ["§cRight click to change", "§eItem matches"]);
        a.refresh(query);
        assert_eq!(mode_of(a.slots[3].as_ref().unwrap()).as_deref(), Some(WILDCARD), "a mode is not a new template");
        a.button(MODE_BUTTON + 4);
        assert!(a.slots[4].is_none(), "an empty template slot has no mode");
    }

    #[test]
    fn the_filter_modes_match_as_isvalidforfilter() {
        let st = |id: &str| Stack::new(id, 1);
        let valid = |filter: &str, mode: Option<&str>, input: &str| valid_for_filter(&st(filter), mode, &st(input));
        assert!(valid("minecraft:iron_ingot", Some(EXACT), "minecraft:iron_ingot"));
        assert!(!valid("ntm:powder_ash__misc", Some(EXACT), "ntm:powder_ash__wood"));
        assert!(valid("ntm:powder_ash__misc", Some(WILDCARD), "ntm:powder_ash__wood"));
        assert!(valid("ntm:bedrock_ore__base_heavy", Some(BEDROCK), "ntm:bedrock_ore__base_light"));
        assert!(!valid("ntm:bedrock_ore__base_heavy", Some(BEDROCK), "ntm:bedrock_ore__base_roasted_heavy"));
        assert!(valid("minecraft:oak_log", Some("logWood"), "minecraft:birch_log"));
        assert!(!valid("minecraft:oak_log", None, "minecraft:birch_log"), "no mode is exact");
        // A 1.7.10 item of several metas: wildcard takes every meta (every plank), exact only its own.
        assert!(valid("minecraft:oak_planks", Some(WILDCARD), "minecraft:birch_planks"));
        assert!(!valid("minecraft:oak_planks", Some(EXACT), "minecraft:birch_planks"));
        assert!(!valid("minecraft:oak_planks", Some(WILDCARD), "minecraft:oak_log"));
        assert!(valid("minecraft:white_wool", Some(WILDCARD), "minecraft:black_wool") && valid("minecraft:ink_sac", Some(WILDCARD), "minecraft:bone_meal"));
        // Exact is `isItemEqual`: the damage too.
        let mut worn = st("minecraft:iron_sword");
        worn.damage = 5;
        assert!(!valid_for_filter(&st("minecraft:iron_sword"), Some(EXACT), &worn));
        assert!(valid_for_filter(&st("minecraft:iron_sword"), Some(WILDCARD), &worn));
    }

    #[test]
    fn a_hopper_fills_the_emptiest_input_its_template_takes() {
        let mut slots: Slots = vec![None; SLOTS];
        slots[0] = Some(with_mode(Stack::new("minecraft:oak_planks", 1), Some(WILDCARD)));
        slots[1] = Some(with_mode(Stack::new("minecraft:oak_planks", 1), Some(WILDCARD)));
        let planks = Stack::new("minecraft:oak_planks", 1);
        assert!(can_insert(&slots, 10, &planks) && can_insert(&slots, 11, &planks));
        assert!(!can_insert(&slots, 12, &planks), "no template above it");
        assert!(!can_insert(&slots, 10, &Stack::new("minecraft:dirt", 1)), "the template refuses it");
        assert!(!can_insert(&slots, 10, &Stack::new("minecraft:oak_planks", 5)), "more than 4");
        slots[10] = Some(Stack::new("minecraft:oak_planks", 1));
        assert!(!can_insert(&slots, 10, &planks), "the other input is emptier");
        slots[11] = Some(Stack::new("minecraft:oak_planks", 1));
        assert!(can_insert(&slots, 10, &planks), "as full as the other");
        slots[10] = Some(Stack::new("minecraft:oak_planks", 4));
        assert!(!can_insert(&slots, 10, &planks), "4 at most");
        assert!(!can_insert(&slots, 10, &Stack::new("minecraft:water_bucket", 2)), "stacked containers");
        // A hopper takes the output and what an input's template no longer passes.
        assert!(can_extract(&slots, OUTPUT, &planks) && !can_extract(&slots, 10, &planks));
        assert!(can_extract(&slots, 10, &Stack::new("minecraft:dirt", 1)) && can_extract(&slots, 12, &planks));
        assert!(!allows(&slots, BATTERY, &planks, true, false) && !allows(&slots, 3, &planks, true, true));
        // A hand follows the inputs' rule (`SlotNonRetarded`): a template above, its mode, 4 at most, no stacked container.
        assert!(!allows(&slots, 12, &Stack::new("minecraft:dirt", 1), true, true), "no template above it");
        slots[10] = None;
        slots[11] = None;
        assert!(allows(&slots, 10, &planks, true, true));
        assert!(!allows(&slots, 10, &Stack::new("minecraft:oak_planks", 16), true, true), "a stack of 16 by hand");
        assert!(!allows(&slots, 10, &Stack::new("minecraft:dirt", 1), true, true), "the template refuses it");
        // The output and the battery are plain slots: a hand puts anything there; a hand takes from every slot.
        assert!(allows(&slots, OUTPUT, &Stack::new("minecraft:dirt", 64), true, true));
        assert!(allows(&slots, BATTERY, &Stack::new("minecraft:dirt", 1), true, true));
        assert!(allows(&slots, 13, &planks, false, true), "a hand takes anything back");
        assert_eq!(container().faces[0], [10, 11, 12, 13, 14, 15, 16, 17, 18, 19]);
        assert_eq!(container().menu, Some("minecraft:generic_9x3"), "a menu to open it");
    }

    /// What the first round of mutants found untested: a changed template's recipe goes back to the first, the power
    /// a craft needs, a different output in custom data, the template's slots out of a hand's reach, a stacked
    /// container refused and the first OreDict name of a prefix.
    #[test]
    fn the_edges_the_mutants_found() {
        let mut a = placed();
        template(&mut a, &[(1, "minecraft:oak_planks"), (4, "minecraft:oak_planks")]);
        a.button(NEXT_RECIPE);
        assert_eq!(a.recipe_index, 1);
        a.slots[1] = None;
        a.slots[4] = None;
        template(&mut a, &[(2, "minecraft:oak_planks"), (5, "minecraft:oak_planks")]);
        assert_eq!((a.recipes.len(), a.recipe_index), (2, 0), "another template: its first recipe");
        a.slots[12] = s("minecraft:oak_planks", 1);
        a.slots[15] = s("minecraft:oak_planks", 1);
        a.refresh(query);
        a.power = CONSUMPTION - 1;
        assert!(!a.step(), "99 HE is short of a craft");
        a.power = CONSUMPTION;
        let mut marked = Stack::new("minecraft:stick", 4);
        marked.custom = Some("{x:1b}".into());
        a.slots[OUTPUT] = Some(marked);
        assert!(!a.step(), "the same item with other custom data does not pile");
        let slots: Slots = vec![None; SLOTS];
        assert!(!allows(&slots, RESULT, &Stack::new("minecraft:stick", 1), true, true), "the result is the template's");
        assert!(!allows(&slots, RESULT, &Stack::new("minecraft:stick", 1), false, true), "nor does a hand take its copy");
        let mut slots: Slots = vec![None; SLOTS];
        slots[2] = Some(with_mode(Stack::new("minecraft:water_bucket", 1), Some(EXACT)));
        assert!(can_insert(&slots, 12, &Stack::new("minecraft:water_bucket", 1)));
        assert!(!can_insert(&slots, 12, &Stack::new("minecraft:water_bucket", 2)), "two buckets at once");
        assert_eq!(smart_mode("minecraft:gunpowder"), "dustAnyPropellant", "the first dust name, in name order");
    }

    /// `slotClick` puts the held stack and `initPatternSmart`s it every time, the same item too: the engine's copy comes
    /// without a mode, and gets the smart one again (and the recipe goes back to the first).
    #[test]
    fn a_copy_put_again_gets_its_mode_again() {
        let mut a = placed();
        template(&mut a, &[(1, "minecraft:oak_planks"), (4, "minecraft:oak_planks")]);
        a.button(MODE_BUTTON + 1);
        a.button(NEXT_RECIPE);
        assert_eq!((mode_of(a.slots[1].as_ref().unwrap()).as_deref(), a.recipe_index), (Some(WILDCARD), 1));
        // The same planks clicked onto slot 1 again: a copy without a mode.
        a.slots[1] = s("minecraft:oak_planks", 1);
        a.refresh(query);
        assert_eq!((mode_of(a.slots[1].as_ref().unwrap()).as_deref(), a.recipe_index), (Some(EXACT), 0));
        assert_eq!(mode_of(a.slots[4].as_ref().unwrap()).as_deref(), Some(EXACT), "the other copy keeps its own");
    }

    #[test]
    fn its_state_survives_the_save() {
        let mut a = placed();
        template(&mut a, &[(1, "minecraft:oak_planks"), (4, "minecraft:oak_planks")]);
        a.button(NEXT_RECIPE);
        a.power = 4_321;
        let mut b = Autocrafter::new();
        b.slots = a.slots.clone();
        b.load_state(&a.save_state());
        b.refresh(query);
        assert_eq!((b.power, b.recipe_index, b.slots[RESULT].clone()), (4_321, 1, s("minecraft:ladder", 1)));
        assert_eq!(mode_of(b.slots[1].as_ref().unwrap()).as_deref(), Some(EXACT), "the modes ride on the copies");
        let mut c = Autocrafter::new();
        c.slots = a.slots.clone();
        c.load_state(&[0, 7]);
        c.refresh(query);
        assert_eq!(c.recipe_index, 0, "a saved recipe past the list");
        assert_eq!(c.demand(), MAX_POWER);
    }
}
