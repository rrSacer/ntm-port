//! The chemical factory (`ntm:machine_chemical_factory`, `MachineChemicalFactory.java`, `TileEntityMachineChemicalFactory.java`,
//! `ModuleMachineBase.java`, `ModuleMachineChemplant.java`).
//!
//! Four chemical plants in one block: 32 slots (the battery at 0, three upgrades at 1 to 3, and for each module `i` a blueprint slot at
//! `4 + 7i`, three inputs at `5 + 7i` and three outputs at `8 + 7i`), 12 input and 12 output tanks of 24 000 mB (three of each for a module),
//! a coolant pair (4 000 mB of water in, 4 000 mB of spent steam out) and a buffer of `max(power, sum of recipe.power * 100, 1 000 000)` HE.
//!
//! A module works as `ModuleMachineBase.update`: its tanks take the type of its recipe's fluids (`setupTanks`, an unneeded tank is
//! reset), and when the factory can cool (100 mB of water, room for 100 of spent steam) and the recipe can be processed (power, inputs
//! by position, room for the outputs) the module takes `recipe.power * pow` HE and its progress grows by `min(speed / duration, 1)`; at 1
//! it consumes the inputs and makes the outputs (`ChanceOutput` rolls by a fixed hash). Every module that processed takes 100 mB of
//! water for 100 mB of spent steam. The factory runs its modules with `speed * 2` and `pow * 2` (`update(speed * 2D, pow * 2D, ...)`),
//! and the upgrades (speed, power, overdrive, each up to level 3, from the three upgrade slots) set `speed` and `pow`. After the modules
//! an input tank takes up to 50 mB a tick from an output tank of the same fluid and pressure (`internal fluid sharing logic`).
//!
//! What is not here: the blueprint pools (a recipe of a pool needs its blueprint in the module's slot: the port's recipe tables have no
//! pool, as the single chemical plant's), the fluid pressure of a recipe's stacks, the audio and the animation, and the restricted
//! (RoR) mode. The ports and the screen come with their own change.

use crate::{
    crafter::{Recipe, Station},
    energy::{EnergyNode, Role},
    energy_network::charge_te_from_items,
    fluid::{FluidNode, Tank},
    machine::Logic,
    server::container_bridge::{Slots, Slotted, Stack},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub const BLOCK: &str = "ntm:machine_chemical_factory";

pub const SLOTS: usize = 32;
pub const BATTERY: usize = 0;
/// The upgrade slots, 1 to 3.
pub const UPGRADES: std::ops::Range<usize> = 1..4;
pub const MODULES: usize = 4;

/// `FluidTank(Fluids.NONE, 24_000)` of the inputs and the outputs.
pub const TANK_CAP: u32 = 24_000;
/// The coolant tanks, 4 000 mB each.
pub const COOLANT_CAP: u32 = 4_000;
/// What a module that processed takes of water and gives as spent steam.
pub const COOLANT_USE: u32 = 100;
/// The least `maxPower`.
pub const MIN_POWER: u64 = 1_000_000;
/// What an input tank takes from an output tank of its fluid each tick.
pub const SHARE: u32 = 50;
/// The stack limit the outputs check (`getMaxStackSize`).
const STACK: u32 = 64;

/// The blueprint slot of module `i`.
pub const fn blueprint(i: usize) -> usize {
    4 + i * 7
}

/// The `k`-th input slot of module `i`.
pub const fn input(i: usize, k: usize) -> usize {
    5 + i * 7 + k
}

/// The `k`-th output slot of module `i`.
pub const fn output(i: usize, k: usize) -> usize {
    8 + i * 7 + k
}

/// One module (`ModuleMachineChemplant`): its recipe (the `recipe` of the NBT, `null` when there is none) and its progress.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Module {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe: Option<String>,
    /// `progress`, a double: a fraction of the recipe.
    pub progress: f64,
    /// `didProcess`: not saved.
    #[serde(skip)]
    pub did_process: bool,
}

impl Module {
    pub fn current(&self) -> Option<&'static Recipe> {
        let name = self.recipe.as_deref()?;
        Station::ChemicalPlant.recipes().iter().find(|r| r.name == name)
    }
}

/// The levels of the three upgrade kinds in the upgrade slots (`UpgradeManagerNT`: each kind adds its tiers, up to 3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Upgrades {
    pub speed: u32,
    pub power: u32,
    pub overdrive: u32,
}

impl Upgrades {
    pub fn of(slots: &[Option<Stack>]) -> Self {
        let mut up = Upgrades::default();
        for s in slots[UPGRADES].iter().flatten() {
            let Some(rest) = s.id.strip_prefix("ntm:upgrade_") else { continue };
            let Some((kind, tier)) = rest.rsplit_once('_') else { continue };
            let Ok(tier) = tier.parse::<u32>() else { continue };
            match kind {
                "speed" => up.speed += tier,
                "power" => up.power += tier,
                "overdrive" => up.overdrive += tier,
                _ => {}
            }
        }
        Upgrades { speed: up.speed.min(3), power: up.power.min(3), overdrive: up.overdrive.min(3) }
    }

    /// `1 + speed / 3 + overdrive`.
    pub fn speed(self) -> f64 {
        1.0 + f64::from(self.speed) / 3.0 + f64::from(self.overdrive)
    }

    /// `1 - power * 0.25 + speed + overdrive * 10 / 3`.
    pub fn pow(self) -> f64 {
        1.0 - f64::from(self.power) * 0.25 + f64::from(self.speed) + f64::from(self.overdrive) * 10.0 / 3.0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChemFactory {
    /// The 32 slots, saved with the model (the journal then restores them with it).
    pub slots: Slots,
    pub power: u64,
    pub modules: [Module; MODULES],
    pub tin: Vec<Tank>,
    pub tout: Vec<Tank>,
    pub water: Tank,
    pub lps: Tank,
    /// Recipes done, for the chance outputs' dice.
    #[serde(default)]
    pub done: u64,
}

impl Default for ChemFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl ChemFactory {
    pub fn new() -> Self {
        Self {
            slots: vec![None; SLOTS],
            power: 0,
            modules: Default::default(),
            tin: (0..3 * MODULES).map(|_| Tank::new(TANK_CAP)).collect(),
            tout: (0..3 * MODULES).map(|_| Tank::new(TANK_CAP)).collect(),
            water: Tank::typed("water", COOLANT_CAP),
            lps: Tank::typed("spentsteam", COOLANT_CAP),
            done: 0,
        }
    }

    /// The model always has its 32 slots, whatever a record from before them held.
    pub fn fit(&mut self) {
        self.slots.resize(SLOTS, None);
    }

    /// `maxPower`: `max(power, sum of recipe.power * 100, 1 000 000)`.
    pub fn max_power(&self) -> u64 {
        let wanted: u64 = self.modules.iter().filter_map(Module::current).map(|r| r.power * 100).sum();
        self.power.max(wanted).max(MIN_POWER)
    }

    /// `canCool`.
    pub fn can_cool(&self) -> bool {
        self.water.mb >= COOLANT_USE && self.lps.mb + COOLANT_USE <= self.lps.cap
    }

    /// Picks `recipe` for module `i` (what the screen does through `setRecipe`).
    pub fn set_recipe(&mut self, i: usize, recipe: Option<&str>) {
        self.modules[i].recipe = recipe.map(str::to_string);
        self.modules[i].progress = 0.0;
    }

    /// `setupTanks`: the module's tanks take its recipe's fluids by position, the ones it does not use are reset.
    fn setup_tanks(&mut self, i: usize, r: &Recipe) {
        for k in 0..3 {
            let t = &mut self.tin[i * 3 + k];
            match r.fluids_in.get(k) {
                Some(f) => t.set_type(&f.0),
                None => reset(t),
            }
            let t = &mut self.tout[i * 3 + k];
            match r.fluids_out.get(k) {
                Some(f) => t.set_type(&f.0),
                None => reset(t),
            }
        }
    }

    /// `hasInput`: the inputs by position, the input tanks by position.
    fn has_input(&self, i: usize, r: &Recipe) -> bool {
        for (k, ing) in r.items_in.iter().enumerate().take(3) {
            match &self.slots[input(i, k)] {
                Some(s) if ing.ids.contains(&s.id) && s.count >= ing.n => {}
                _ => return false,
            }
        }
        r.fluids_in.iter().enumerate().take(3).all(|(k, f)| self.tin[i * 3 + k].mb >= f.1)
    }

    /// `canFitOutput`.
    fn can_fit(&self, i: usize, r: &Recipe) -> bool {
        for (k, (item, n, _)) in r.items_out.iter().enumerate().take(3) {
            let Some(s) = &self.slots[output(i, k)] else { continue };
            if s.id != *item || s.damage != 0 || s.count + n > STACK {
                return false;
            }
        }
        r.fluids_out.iter().enumerate().take(3).all(|(k, f)| f.1 + self.tout[i * 3 + k].mb <= self.tout[i * 3 + k].cap)
    }

    /// `canProcess`, with the auto switch: false when the input in slot 0 is the first input of another recipe of the group (the
    /// module takes that recipe and the tick is over).
    fn can_process(&mut self, i: usize, r: &'static Recipe, power: f64) -> bool {
        if let Some(group) = &r.group
            && let Some(first) = &self.slots[input(i, 0)]
        {
            let item = first.id.clone();
            let name = self.modules[i].recipe.clone().unwrap_or_default();
            let next = Station::ChemicalPlant
                .recipes()
                .iter()
                .filter(|o| o.group.as_ref() == Some(group) && o.name != name)
                .find(|o| o.items_in.first().is_some_and(|x| x.ids.contains(&item)));
            if let Some(o) = next {
                self.modules[i].recipe = Some(o.name.clone());
                return false;
            }
        }
        if (self.power as f64) < r.power as f64 * power {
            return false;
        }
        self.has_input(i, r) && self.can_fit(i, r)
    }

    /// `process`: takes the power, grows the progress and, at 1, finishes one recipe.
    fn process(&mut self, i: usize, r: &'static Recipe, speed: f64, power: f64) {
        let cost = if power == 1.0 { r.power } else { (r.power as f64 * power) as u64 };
        self.power = self.power.saturating_sub(cost);
        self.modules[i].progress += (speed / f64::from(r.duration.max(1))).min(1.0);
        if self.modules[i].progress >= 1.0 {
            self.consume(i, r);
            self.produce(i, r);
            if self.can_process(i, r, power) {
                self.modules[i].progress -= 1.0;
            } else {
                self.modules[i].progress = 0.0;
            }
        }
    }

    fn consume(&mut self, i: usize, r: &Recipe) {
        for (k, ing) in r.items_in.iter().enumerate().take(3) {
            let slot = &mut self.slots[input(i, k)];
            if let Some(s) = slot {
                s.count = s.count.saturating_sub(ing.n);
                if s.count == 0 {
                    *slot = None;
                }
            }
        }
        for (k, f) in r.fluids_in.iter().enumerate().take(3) {
            self.tin[i * 3 + k].mb = self.tin[i * 3 + k].mb.saturating_sub(f.1);
        }
    }

    fn produce(&mut self, i: usize, r: &Recipe) {
        self.done += 1;
        for (k, (item, n, chance)) in r.items_out.iter().enumerate().take(3) {
            // `ChanceOutput.collapse`: a fixed hash of (recipes done, output) instead of the world's random.
            let roll = crate::explosion::crater::rand01(self.done as i32, k as i32, (self.done >> 31) as i32, 77);
            if *chance < 1.0 && roll >= *chance {
                continue;
            }
            match &mut self.slots[output(i, k)] {
                Some(s) => s.count += n,
                slot @ None => *slot = Some(Stack::new(item, *n)),
            }
        }
        for (k, f) in r.fluids_out.iter().enumerate().take(3) {
            self.tout[i * 3 + k].mb += f.1;
        }
    }

    /// `ModuleMachineBase.update(speed, power, extraCondition, blueprint)` of module `i`.
    fn update_module(&mut self, i: usize, speed: f64, power: f64, extra: bool) {
        let Some(r) = self.modules[i].current() else {
            self.modules[i].did_process = false;
            return;
        };
        self.setup_tanks(i, r);
        self.modules[i].did_process = false;
        if extra && self.can_process(i, r, power) {
            // The switch inside `can_process` may have changed the recipe: this tick is over for it, as the Java's.
            self.process(i, r, speed, power);
            self.modules[i].did_process = true;
        } else {
            self.modules[i].progress = 0.0;
        }
    }

    /// `internal fluid sharing logic`.
    fn share(&mut self) {
        for a in 0..self.tin.len() {
            if self.tin[a].fluid.is_empty() {
                continue;
            }
            for b in 0..self.tout.len() {
                let (input, out) = (&self.tin[a], &self.tout[b]);
                if out.fluid.is_empty() || out.fluid != input.fluid || out.pressure != input.pressure {
                    continue;
                }
                let moved = input.free().min(out.mb).min(SHARE);
                if moved > 0 {
                    self.tin[a].mb += moved;
                    self.tout[b].mb -= moved;
                }
            }
        }
    }

    /// `updateEntity`, server side. True when something changed.
    pub fn step(&mut self) -> bool {
        self.fit();
        let before = (self.power, self.slots.clone(), self.modules.clone(), self.tin.clone(), self.tout.clone(), self.water.mb, self.lps.mb);
        let max = self.max_power();
        let battery = crate::storage::item_slot(&self.slots[BATTERY]);
        self.power = charge_te_from_items(battery.as_ref(), self.power as i64, max as i64).max(0) as u64;
        crate::storage::write_back(&mut self.slots[BATTERY], battery.as_ref());

        let up = Upgrades::of(&self.slots);
        let (speed, pow) = (up.speed(), up.pow());
        for i in 0..MODULES {
            let cool = self.can_cool();
            self.update_module(i, speed * 2.0, pow * 2.0, cool);
            if self.modules[i].did_process {
                self.water.mb = self.water.mb.saturating_sub(COOLANT_USE);
                self.lps.mb += COOLANT_USE;
            }
        }
        self.share();
        before != (self.power, self.slots.clone(), self.modules.clone(), self.tin.clone(), self.tout.clone(), self.water.mb, self.lps.mb)
    }
}

/// `FluidTank.resetTank`: no type, nothing in, no pressure.
fn reset(t: &mut Tank) {
    t.fluid.clear();
    t.mb = 0;
    t.pressure = 0;
}

impl EnergyNode for ChemFactory {
    fn role(&self) -> Role {
        Role::Consumer
    }
    fn offer(&self) -> u64 {
        0
    }
    fn demand(&self) -> u64 {
        self.max_power().saturating_sub(self.power)
    }
    fn give(&mut self, he: u64) {
        self.power += he;
    }
    fn take(&mut self, _he: u64) {}
}

impl FluidNode for ChemFactory {
    /// `getSendingTanks`: the outputs and the spent steam.
    fn fluid_offer(&self, f: &str) -> u64 {
        let outs: u64 = self.tout.iter().filter(|t| t.fluid == f).map(|t| u64::from(t.mb)).sum();
        outs + if self.lps.fluid == f { u64::from(self.lps.mb) } else { 0 }
    }
    /// `getReceivingTanks`: the inputs and the water.
    fn fluid_demand(&self, f: &str) -> u64 {
        let ins: u64 = self.tin.iter().filter(|t| !t.fluid.is_empty() && t.fluid == f).map(|t| u64::from(t.free())).sum();
        ins + if self.water.fluid == f { u64::from(self.water.free()) } else { 0 }
    }
    fn fluid_types(&self) -> Vec<String> {
        let mut types: Vec<String> = self.tin.iter().chain(&self.tout).filter(|t| !t.fluid.is_empty()).map(|t| t.fluid.clone()).collect();
        types.push(self.water.fluid.clone());
        types.push(self.lps.fluid.clone());
        types.sort();
        types.dedup();
        types
    }
    fn fluid_give(&mut self, f: &str, mut mb: u64) {
        for t in self.tin.iter_mut().chain(std::iter::once(&mut self.water)).filter(|t| !t.fluid.is_empty() && t.fluid == f) {
            mb -= u64::from(t.fill(f, mb.min(u64::from(u32::MAX)) as u32));
        }
    }
    fn fluid_take(&mut self, f: &str, mut mb: u64) {
        for t in self.tout.iter_mut().chain(std::iter::once(&mut self.lps)).filter(|t| t.fluid == f) {
            mb -= u64::from(t.drain(mb.min(u64::from(u32::MAX)) as u32));
        }
    }
}

impl Slotted for ChemFactory {
    fn slots_mut(&mut self) -> &mut Slots {
        self.fit();
        &mut self.slots
    }
    /// The screen comes with its own change.
    fn screen_data(&self) -> Vec<i32> {
        Vec::new()
    }
}

/// `getAccessibleSlotsFromSide`: every side reaches the twenty-four item slots of the modules (the inputs and the outputs).
pub fn container() -> crate::native::Container {
    let inputs: Vec<usize> = (0..MODULES).flat_map(|i| (0..3).map(move |k| input(i, k))).collect();
    let outputs: Vec<usize> = (0..MODULES).flat_map(|i| (0..3).map(move |k| output(i, k))).collect();
    let mut all: Vec<usize> = inputs.iter().chain(&outputs).copied().collect();
    all.sort_unstable();
    crate::native::Container { slots: SLOTS, faces: std::array::from_fn(|_| all.clone()), inputs, outputs, menu: Some("minecraft:generic_9x4") }
}

impl Logic for ChemFactory {
    fn tick(&mut self) -> bool {
        self.step()
    }

    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        Some(self)
    }

    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(self)
    }

    fn level(&self, _max: u8) -> u8 {
        0
    }

    fn view(&self) -> ViewModel {
        let mut v = ViewModel::new("Fábrica química").bar("Energia", self.power, self.max_power(), "HE");
        for (i, m) in self.modules.iter().enumerate() {
            let name = m.current().map_or("sem receita".to_string(), |r| r.label.clone());
            v = v.text(format!("Módulo {}: {name}", i + 1), if m.did_process { view::GREEN } else { view::GREY });
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe(name: &str) -> &'static Recipe {
        Station::ChemicalPlant.recipes().iter().find(|r| r.name == name).expect(name)
    }

    /// A factory with the module 0 on `name`, power for a long time, and water for the cooling.
    fn on(name: &str) -> ChemFactory {
        let mut f = ChemFactory::new();
        f.power = 10_000_000;
        f.water.mb = 4_000;
        f.set_recipe(0, Some(name));
        f
    }

    fn give(f: &mut ChemFactory, slot: usize, id: &str, n: u32) {
        f.slots[slot] = Some(Stack::new(id, n));
    }

    #[test]
    fn the_slots_are_laid_out_as_the_java_lays_them() {
        assert_eq!((blueprint(0), input(0, 0), output(0, 0)), (4, 5, 8));
        assert_eq!((blueprint(3), input(3, 2), output(3, 2)), (25, 28, 31));
        assert_eq!(SLOTS, output(3, 2) + 1);
        let f = ChemFactory::new();
        assert_eq!((f.slots.len(), f.tin.len(), f.tout.len(), f.tin[0].cap, f.water.cap, f.lps.cap), (32, 12, 12, 24_000, 4_000, 4_000));
        assert_eq!((f.water.fluid.as_str(), f.lps.fluid.as_str()), ("water", "spentsteam"));
    }

    #[test]
    fn the_buffer_follows_the_recipes_and_never_drops_below_a_million() {
        assert_eq!((MIN_POWER, SHARE, COOLANT_USE, TANK_CAP, COOLANT_CAP), (1_000_000, 50, 100, 24_000, 4_000));
        let mut f = ChemFactory::new();
        assert_eq!(f.max_power(), MIN_POWER);
        f.set_recipe(0, Some("chem.hydrogen"));
        let r = recipe("chem.hydrogen");
        assert_eq!(f.max_power(), (r.power * 100).max(MIN_POWER));
        f.power = 5_000_000;
        assert_eq!(f.max_power(), 5_000_000, "the power held is a floor too");
        for i in 0..4 {
            f.set_recipe(i, Some("chem.hydrogen"));
        }
        f.power = 0;
        assert_eq!(f.max_power(), (4 * r.power * 100).max(MIN_POWER));
    }

    #[test]
    fn the_upgrades_set_the_speed_and_the_power_as_the_java_does() {
        let none = Upgrades::default();
        assert_eq!((none.speed(), none.pow()), (1.0, 1.0));
        let mut slots: Slots = vec![None; SLOTS];
        slots[1] = Some(Stack::new("ntm:upgrade_speed_3", 1));
        slots[2] = Some(Stack::new("ntm:upgrade_power_2", 1));
        slots[3] = Some(Stack::new("ntm:upgrade_overdrive_1", 1));
        let up = Upgrades::of(&slots);
        assert_eq!((up.speed, up.power, up.overdrive), (3, 2, 1));
        assert_eq!(up.speed(), 1.0 + 1.0 + 1.0);
        assert_eq!(up.pow(), 1.0 - 0.5 + 3.0 + 10.0 / 3.0);
        // The levels stop at 3, the other slots count for nothing, an unknown upgrade too.
        slots[1] = Some(Stack::new("ntm:upgrade_speed_3", 1));
        slots[2] = Some(Stack::new("ntm:upgrade_speed_2", 1));
        slots[3] = Some(Stack::new("ntm:upgrade_fortune_1", 1));
        slots[0] = Some(Stack::new("ntm:upgrade_overdrive_3", 1));
        let up = Upgrades::of(&slots);
        assert_eq!((up.speed, up.overdrive), (3, 0), "level 3 at most; the battery slot is no upgrade slot");
    }

    #[test]
    fn a_module_makes_a_recipe_over_its_duration_at_double_speed() {
        // chem.hydrogen: 1 coal, 8 000 mB water, 20 ticks, 500 mB of hydrogen out.
        let r = recipe("chem.hydrogen");
        let mut f = on("chem.hydrogen");
        give(&mut f, input(0, 0), "minecraft:coal", 2);
        f.tin[0].set_type("water");
        f.tin[0].mb = 16_000;
        let mut ticks = 0;
        while f.tout[0].mb == 0 {
            f.step();
            ticks += 1;
            assert!(ticks < 100);
        }
        // Speed 1 x 2 = 2 a tick over a duration of 20: a step of 0.1 in a `double`, which ten times adds up to 0.9999999999999999 and
        // not 1: the Java's eleventh tick finishes it, and so does this.
        assert_eq!(ticks, 11);
        assert_eq!((f.tout[0].fluid.as_str(), f.tout[0].mb), ("hydrogen", 500));
        assert_eq!(f.slots[input(0, 0)].as_ref().map(|s| s.count), Some(1), "one coal used");
        assert_eq!(f.tin[0].mb, 8_000, "8 000 mB of water used");
        // Each tick that worked took 100 mB of water for 100 mB of spent steam, and `recipe.power * 2` HE.
        assert_eq!((f.water.mb, f.lps.mb), (4_000 - 100 * 11, 100 * 11));
        assert_eq!(f.power, 10_000_000 - r.power * 2 * 11);
    }

    #[test]
    fn nothing_runs_without_the_coolant_or_the_power() {
        let mut f = on("chem.hydrogen");
        give(&mut f, input(0, 0), "minecraft:coal", 1);
        f.tin[0].set_type("water");
        f.tin[0].mb = 8_000;
        f.water.mb = 99;
        assert!(!f.can_cool());
        f.step();
        assert_eq!((f.modules[0].progress, f.modules[0].did_process), (0.0, false), "99 mB of water is not enough");
        f.water.mb = 4_000;
        f.lps.mb = 3_950;
        assert!(!f.can_cool(), "no room for the spent steam");
        f.lps.mb = 0;
        f.power = 0;
        f.step();
        assert!(!f.modules[0].did_process, "no power");
        f.power = recipe("chem.hydrogen").power * 2;
        f.step();
        assert!(f.modules[0].did_process, "exactly the power of a tick (recipe.power x 2)");
    }

    #[test]
    fn a_module_without_its_inputs_resets_its_progress() {
        let mut f = on("chem.hydrogen");
        give(&mut f, input(0, 0), "minecraft:coal", 1);
        f.tin[0].set_type("water");
        f.tin[0].mb = 8_000;
        f.step();
        assert!(f.modules[0].progress > 0.0);
        f.slots[input(0, 0)] = None;
        f.step();
        assert_eq!(f.modules[0].progress, 0.0);
    }

    #[test]
    fn a_full_output_stops_it_and_a_foreign_item_in_the_output_too() {
        let mut f = on("chem.hydrogen");
        give(&mut f, input(0, 0), "minecraft:coal", 1);
        f.tin[0].set_type("water");
        f.tin[0].mb = 8_000;
        f.tout[0].set_type("hydrogen");
        f.tout[0].mb = TANK_CAP - 499;
        f.step();
        assert!(!f.modules[0].did_process, "499 mB of room for 500");
        f.tout[0].mb = TANK_CAP - 500;
        f.step();
        assert!(f.modules[0].did_process, "exactly the room");
    }

    #[test]
    fn the_tanks_take_the_recipes_types_and_reset_what_it_does_not_use() {
        let mut f = on("chem.hydrogen");
        f.tin[1].set_type("lava");
        f.tin[1].mb = 100;
        f.tout[2].set_type("oil");
        f.tout[2].mb = 100;
        f.tin[3].set_type("lava");
        f.tin[3].mb = 7;
        f.step();
        assert_eq!(f.tin[0].fluid, "water");
        assert_eq!((f.tin[1].fluid.as_str(), f.tin[1].mb), ("", 0), "a tank the recipe has no fluid for is reset");
        assert_eq!((f.tout[0].fluid.as_str(), f.tout[2].fluid.as_str()), ("hydrogen", ""));
        assert_eq!((f.tin[3].fluid.as_str(), f.tin[3].mb), ("lava", 7), "another module's tanks stay");
    }

    #[test]
    fn an_input_tank_takes_fifty_a_tick_from_an_output_of_its_fluid() {
        let mut f = ChemFactory::new();
        f.tin[4].set_type("water");
        f.tout[7].set_type("water");
        f.tout[7].mb = 120;
        f.step();
        assert_eq!((f.tin[4].mb, f.tout[7].mb), (50, 70));
        f.step();
        f.step();
        assert_eq!((f.tin[4].mb, f.tout[7].mb), (120, 0));
        // Not for another pressure, nor for another fluid, nor for an untyped input.
        f.tout[7].set_type("lava");
        f.tout[7].mb = 100;
        f.step();
        assert_eq!(f.tout[7].mb, 100);
        f.tin[4].set_type("lava");
        f.tin[4].pressure = 2;
        f.step();
        assert_eq!(f.tout[7].mb, 100, "another pressure");
        f.tin[4].pressure = 0;
        f.tin[5].set_type("");
        f.step();
        assert_eq!((f.tin[4].mb, f.tout[7].mb), (50, 50));
    }

    #[test]
    fn the_battery_slot_charges_it_and_the_item_stays() {
        let mut f = ChemFactory::new();
        f.slots[BATTERY] = Some(Stack::new(crate::machine::CREATIVE_BATTERY, 1));
        f.step();
        assert_eq!(f.power, MIN_POWER);
        assert!(f.slots[BATTERY].is_some());
    }

    #[test]
    fn an_auto_switch_takes_the_recipe_of_the_input_and_loses_the_tick() {
        let Some((from, to, item)) = Station::ChemicalPlant.recipes().iter().find_map(|a| {
            let g = a.group.as_ref()?;
            let b = Station::ChemicalPlant.recipes().iter().find(|b| b.name != a.name && b.group.as_ref() == Some(g) && !b.items_in.is_empty())?;
            Some((a.name.clone(), b.name.clone(), b.items_in[0].ids[0].clone()))
        }) else {
            return;
        };
        let mut f = on(&from);
        give(&mut f, input(0, 0), &item, 1);
        f.step();
        assert_eq!(f.modules[0].recipe.as_deref(), Some(to.as_str()));
        assert!(!f.modules[0].did_process, "the tick of the switch does not process");
    }

    #[test]
    fn the_model_saves_everything_including_the_slots() {
        let mut f = on("chem.hydrogen");
        give(&mut f, input(0, 0), "minecraft:coal", 3);
        give(&mut f, output(2, 1), "minecraft:coal", 5);
        f.modules[0].progress = 0.4;
        f.tin[0].set_type("water");
        f.tin[0].mb = 1_234;
        f.step();
        let back: ChemFactory = serde_json::from_str(&serde_json::to_string(&f).unwrap()).unwrap();
        assert_eq!(back, f);
        assert_eq!(back.slots[output(2, 1)].as_ref().map(|s| s.count), Some(5));
    }

    #[test]
    fn the_coolant_needs_exactly_a_hundred_and_room_for_a_hundred() {
        let mut f = ChemFactory::new();
        f.water.mb = COOLANT_USE;
        f.lps.mb = COOLANT_CAP - COOLANT_USE;
        assert!(f.can_cool(), "100 mB of water and room for exactly 100 of spent steam");
        f.water.mb = COOLANT_USE - 1;
        assert!(!f.can_cool());
        f.water.mb = COOLANT_USE;
        f.lps.mb = COOLANT_CAP - COOLANT_USE + 1;
        assert!(!f.can_cool());
    }

    #[test]
    fn the_last_input_goes_and_leaves_its_slot_empty_and_the_progress_back_at_zero() {
        let mut f = on("chem.hydrogen");
        give(&mut f, input(0, 0), "minecraft:coal", 1);
        f.tin[0].set_type("water");
        f.tin[0].mb = 8_000;
        while f.tout[0].mb == 0 {
            f.step();
        }
        assert!(f.slots[input(0, 0)].is_none(), "the slot is empty, not a stack of 0");
        assert_eq!(f.modules[0].progress, 0.0, "nothing left for another round: the progress is dropped, not carried over");
    }

    #[test]
    fn the_item_outputs_stop_at_a_full_stack_and_come_one_by_one_by_position() {
        let r = recipe("chem.tarsand");
        let (item, n, _) = &r.items_out[0];
        let mut f = on("chem.tarsand");
        give(&mut f, input(0, 0), &r.items_in[0].ids[0], r.items_in[0].n);
        give(&mut f, input(0, 1), &r.items_in[1].ids[0], r.items_in[1].n);
        f.slots[output(0, 0)] = Some(Stack::new(item, 64 - n + 1));
        f.step();
        assert!(!f.modules[0].did_process, "one over a full stack");
        f.slots[output(0, 0)] = Some(Stack::new(item, 64 - n));
        f.step();
        assert!(f.modules[0].did_process, "exactly a full stack");
        f.slots[output(0, 0)] = Some(Stack::new("minecraft:dirt", 1));
        f.step();
        assert!(!f.modules[0].did_process, "another item in the output slot stops it");
        assert_eq!(f.slots[output(0, 0)].as_ref().map(|s| s.id.as_str()), Some("minecraft:dirt"), "another item in the output is not touched");
    }

    #[test]
    fn it_asks_for_the_power_it_lacks_and_offers_and_demands_what_it_holds() {
        let mut f = ChemFactory::new();
        f.power = 400_000;
        assert_eq!((f.demand(), f.role()), (MIN_POWER - 400_000, Role::Consumer));
        f.water.mb = 1_000;
        f.lps.mb = 700;
        f.tin[0].set_type("lava");
        f.tout[0].set_type("oil");
        f.tout[0].mb = 300;
        assert_eq!(f.fluid_demand("water"), u64::from(COOLANT_CAP - 1_000), "the water tank takes water");
        assert_eq!(f.fluid_offer("spentsteam"), 700, "the spent steam is on offer");
        assert_eq!((f.fluid_offer("oil"), f.fluid_demand("lava")), (300, u64::from(TANK_CAP)));
        f.fluid_give("water", 500);
        f.fluid_take("spentsteam", 200);
        assert_eq!((f.water.mb, f.lps.mb), (1_500, 500));
        assert!(f.fluid_types().contains(&"spentsteam".to_string()));
    }
}
