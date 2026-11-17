//! The compressors (`ntm:machine_compressor`, `ntm:machine_compressor_compact`):
//! `TileEntityMachineCompressorBase.java`, `TileEntityMachineCompressor.java`,
//! `TileEntityMachineCompressorCompact.java`, `CompressorRecipes.java`,
//! `ContainerCompressor.java`, `GUICompressor.java`.
//!
//! One machine, two bodies. The input tank (16 000 mB) takes its fluid type
//! from the identifier in slot 0 and its pressure from the GUI (`compression`,
//! 0..=4: "`j` PU -> `j + 1` PU"); the output tank (16 000 mB) follows: the
//! recipe's fluid and pressure, or, with no recipe for (fluid, pressure), the
//! same fluid one pressure up. Every tick with `power > powerRequirement` and
//! the input and the room, `progress` counts to `processTime` and one
//! operation turns `inputAmount` of the input into the output (generic:
//! 1 000 mB for 1 000 mB). Power is paid every counting tick, not per
//! operation. Slots: 0 identifier, 1 battery, 2 and 3 upgrades (speed,
//! power, overdrive).
//!
//! The network only carries the fluid between tanks of the same pressure
//! (`crate::fluid::distribute`), so the output at pressure `p + 1` reaches
//! only a tank at `p + 1`.

use crate::{
    Pos,
    energy::{EnergyNode, Role},
    fluid::{FluidNode, HIGHEST_VALID_PRESSURE, Tank},
    machine::{Logic, charge_from_slot},
    server::container_bridge::{Slots, Slotted},
    upgrades,
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub const BLOCK: &str = "ntm:machine_compressor";
pub const COMPACT: &str = "ntm:machine_compressor_compact";

/// `TileEntityMachineCompressorBase.maxPower`.
pub const MAX_POWER: u64 = 100_000;
/// `processTimeBase`: the time of a generic compression, before upgrades.
pub const PROCESS_TIME_BASE: i32 = 100;
/// `powerRequirementBase`: the HE taken every counting tick, before upgrades.
pub const POWER_BASE: i32 = 2_500;
/// Both tanks: `new FluidTank(Fluids.NONE, 16_000)`.
pub const TANK_CAP: u32 = 16_000;
/// A generic compression (no recipe): 1 000 mB in, 1 000 mB out.
pub const GENERIC_AMOUNT: u32 = 1_000;
/// The `compression` the GUI sends: the buttons 0..=4 (`GUICompressor.mouseClicked`).
pub const BUTTONS: u32 = 5;

/// `CompressorRecipes.CompressorRecipe`: what the input (fluid, pressure)
/// does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recipe {
    pub input: u32,
    pub fluid: &'static str,
    pub amount: u32,
    pub pressure: u8,
    pub duration: i32,
}

/// `CompressorRecipes.registerDefaults` (`CompressorRecipes.java:25-31`),
/// keyed by (fluid, pressure).
pub fn recipe(fluid: &str, pressure: u8) -> Option<Recipe> {
    let r = |input, fluid, amount, pressure, duration| Some(Recipe { input, fluid, amount, pressure, duration });
    match (fluid, pressure) {
        ("petroleum", 0) => r(2_000, "petroleum", 2_000, 1, 20),
        ("petroleum", 1) => r(2_000, "lpg", 1_000, 0, 20),
        ("blood", 3) => r(1_000, "heavyoil", 250, 0, 200),
        ("perfluoromethyl", 0) => r(1_000, "perfluoromethyl", 1_000, 1, 50),
        ("perfluoromethyl", 1) => r(1_000, "perfluoromethyl_cold", 1_000, 0, 50),
        _ => None,
    }
}

/// The body: `TileEntityMachineCompressor` (a tall one) or
/// `TileEntityMachineCompressorCompact` (a low, wide one).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shape {
    #[default]
    Tall,
    Compact,
}

impl Shape {
    pub fn of(block: &str) -> Option<Self> {
        match block {
            BLOCK => Some(Shape::Tall),
            COMPACT => Some(Shape::Compact),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Compressor {
    #[serde(default)]
    pub shape: Shape,
    #[serde(skip)]
    pub slots: Slots,
    pub power: u64,
    pub input: Tank,
    pub output: Tank,
    pub progress: i32,
    /// Worked out every tick from the recipe and the upgrades (`processTime`).
    #[serde(skip, default = "default_time")]
    pub process_time: i32,
    #[serde(skip)]
    pub power_req: i32,
    /// Counted this tick (`isOn`): the client's fan and piston.
    #[serde(skip)]
    pub on: bool,
}

fn default_time() -> i32 {
    PROCESS_TIME_BASE
}

impl Default for Compressor {
    fn default() -> Self {
        Self::new(Shape::Tall)
    }
}

fn ensure(slots: &mut Slots) {
    if slots.len() < 4 {
        slots.resize(4, None);
    }
}

impl Compressor {
    pub fn new(shape: Shape) -> Self {
        Self {
            shape,
            slots: vec![None; 4],
            power: 0,
            input: Tank::new(TANK_CAP),
            output: Tank::new(TANK_CAP).with_pressure(1),
            progress: 0,
            process_time: PROCESS_TIME_BASE,
            power_req: 0,
            on: false,
        }
    }

    fn recipe(&self) -> Option<Recipe> {
        recipe(&self.input.fluid, self.input.pressure)
    }

    /// The ports: (cell, outward face) in the placer's frame
    /// (`getPorts`; `MachineCompressor`/`MachineCompressorCompact.getAllPorts`).
    /// Power and fluid go through the same cells.
    pub fn port_cells(block: &str, core: Pos, facing: Option<&str>) -> Vec<(Pos, Pos)> {
        let Some(f) = facing.and_then(crate::registry::facing_dir) else {
            return Vec::new();
        };
        let local: &[([i32; 3], [i32; 2])] = match Shape::of(block) {
            // (x right, y, z away from the placer), then the way out.
            Some(Shape::Tall) => &[([0, 0, 1], [0, 1]), ([-1, 0, 0], [-1, 0]), ([1, 0, 0], [1, 0])],
            Some(Shape::Compact) => &[
                ([-3, 1, 0], [-1, 0]),
                ([3, 1, 0], [1, 0]),
                ([1, 1, -1], [0, -1]),
                ([-1, 1, -1], [0, -1]),
                ([1, 1, 1], [0, 1]),
                ([-1, 1, 1], [0, 1]),
            ],
            None => return Vec::new(),
        };
        let fwd = [-f[0], -f[2]];
        let right = [-fwd[1], fwd[0]];
        local
            .iter()
            .map(|&(l, out)| {
                let cell = [core[0] + right[0] * l[0] + fwd[0] * l[2], core[1] + l[1], core[2] + right[1] * l[0] + fwd[1] * l[2]];
                let face = [right[0] * out[0] + fwd[0] * out[1], 0, right[1] * out[0] + fwd[1] * out[1]];
                (cell, face)
            })
            .collect()
    }

    /// `setupTanks`: the output follows the input's recipe, or one pressure
    /// up with the same fluid. (A pressure or a fluid that changes empties
    /// the tank: `withPressure` and `setTankType`.)
    fn setup_tanks(&mut self) {
        match self.recipe() {
            Some(r) => {
                self.output.set_pressure(r.pressure);
                self.output.set_type(r.fluid);
            }
            None => {
                self.output.set_pressure(self.input.pressure.saturating_add(1));
                let fluid = self.input.fluid.clone();
                self.output.set_type(&fluid);
            }
        }
    }

    /// `canProcess`: more power than it asks (`<=` is not enough), the input
    /// of the recipe and the room for its output.
    fn can_process(&self) -> bool {
        if self.power <= self.power_req.max(0) as u64 {
            return false;
        }
        match self.recipe() {
            None => self.input.mb >= GENERIC_AMOUNT && self.output.mb + GENERIC_AMOUNT <= self.output.cap,
            Some(r) => self.input.mb >= r.input && self.output.mb + r.amount <= self.output.cap,
        }
    }

    fn process(&mut self) {
        let (take, give) = match self.recipe() {
            None => (GENERIC_AMOUNT, GENERIC_AMOUNT),
            Some(r) => (r.input, r.amount),
        };
        self.input.mb -= take;
        self.output.mb += give;
    }

    /// `TileEntityMachineCompressorBase.updateEntity`.
    pub fn step(&mut self) -> bool {
        let was = (self.power, self.progress, self.input.clone(), self.output.clone(), self.on);
        ensure(&mut self.slots);
        self.power = charge_from_slot(&self.slots, 1, self.power, MAX_POWER);
        crate::combustion::type_from_identifier(&mut self.input, &self.slots, 0, |_| true);
        self.setup_tanks();

        // `checkSlots(this, slots, 1, 3)`: the battery's slot counts too.
        let up = &self.slots[1..=3];
        let speed = upgrades::level(up, "speed", 3) as i32;
        let power = upgrades::level(up, "power", 3) as i32;
        let over = upgrades::level(up, "overdrive", 9) as i32;

        let rec = self.recipe();
        let base = rec.map_or(PROCESS_TIME_BASE, |r| r.duration);
        let mut time = match rec {
            None => match speed {
                3 => 10,
                2 => 20,
                1 => 60,
                _ => base,
            },
            Some(_) => base / (speed + 1),
        };
        let mut req = POWER_BASE / (power + 1);
        time /= over + 1;
        req *= over * 2 + 1;
        self.process_time = time.max(1);
        self.power_req = req;

        if self.can_process() {
            self.progress += 1;
            self.on = true;
            self.power -= self.power_req as u64;
            if self.progress >= self.process_time {
                self.progress = 0;
                self.process();
            }
        } else {
            self.progress = 0;
            self.on = false;
        }
        was != (self.power, self.progress, self.input.clone(), self.output.clone(), self.on)
    }

    /// `receiveControl` (`compression`): the input's pressure, and the output's
    /// with it. A new pressure empties the tank that has it.
    pub fn set_compression(&mut self, compression: u8) {
        if compression == self.input.pressure {
            return;
        }
        self.input.set_pressure(compression);
        match recipe(&self.input.fluid, compression) {
            None => self.output.set_pressure(compression.saturating_add(1)),
            Some(r) => {
                self.output.set_pressure(r.pressure);
                self.output.set_type(r.fluid);
            }
        }
    }

    fn color(t: &Tank) -> i32 {
        crate::fluid::get(&t.fluid).map_or(0, |f| f.color as i32)
    }

    /// The screen's values (`horizon_screen/<block>.json`): [0] power, [1]
    /// max power, [2..5) the input (mB, cap, colour), [5..8) the output,
    /// [8] the progress arrow in pixels (`progress * 55 / processTime`), [9]
    /// its length, [10] the pressure the marker is on, [11] the "has power"
    /// lamp (`power >= powerRequirement`).
    pub fn screen_data(&self) -> Vec<i32> {
        let arrow = i64::from(self.progress) * 55 / i64::from(self.process_time.max(1));
        vec![
            self.power.min(i32::MAX as u64) as i32,
            MAX_POWER as i32,
            self.input.mb as i32,
            self.input.cap as i32,
            Self::color(&self.input),
            self.output.mb as i32,
            self.output.cap as i32,
            Self::color(&self.output),
            arrow as i32,
            55,
            i32::from(self.input.pressure),
            i32::from(self.power >= self.power_req.max(0) as u64),
        ]
    }
}

impl Logic for Compressor {
    fn tick(&mut self) -> bool {
        self.step()
    }
    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        Some(self)
    }
    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(self)
    }
    fn level(&self, max: u8) -> u8 {
        if self.on { max } else { 0 }
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Compressor")
            .bar("Energia", self.power, MAX_POWER, "HE")
            .text(format!("{} — {} PU", self.input.line(), self.input.pressure), view::GREY)
            .text(format!("{} — {} PU", self.output.line(), self.output.pressure), view::GREY)
    }
}

impl Slotted for Compressor {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        Compressor::screen_data(self)
    }
    fn button(&mut self, id: u32) -> bool {
        if id < BUTTONS {
            self.set_compression(id as u8);
        }
        false
    }
}

impl EnergyNode for Compressor {
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

/// `IFluidStandardTransceiverMK2`: the input receives and the output sends,
/// each only at its own pressure (`tank.getPressure() == pressure`).
impl FluidNode for Compressor {
    fn fluid_offer(&self, fluid: &str) -> u64 {
        self.fluid_offer_at(fluid, 0)
    }
    fn fluid_demand(&self, fluid: &str) -> u64 {
        self.fluid_demand_at(fluid, 0)
    }
    fn fluid_types(&self) -> Vec<String> {
        [&self.input, &self.output].into_iter().filter(|t| !t.fluid.is_empty()).map(|t| t.fluid.clone()).collect()
    }
    fn fluid_give(&mut self, fluid: &str, mb: u64) {
        self.fluid_give_at(fluid, 0, mb);
    }
    fn fluid_take(&mut self, fluid: &str, mb: u64) {
        self.fluid_take_at(fluid, 0, mb);
    }
    fn fluid_offer_at(&self, fluid: &str, pressure: u8) -> u64 {
        if !self.output.fluid.is_empty() && self.output.fluid == fluid && self.output.pressure == pressure {
            u64::from(self.output.mb)
        } else {
            0
        }
    }
    fn fluid_demand_at(&self, fluid: &str, pressure: u8) -> u64 {
        if !self.input.fluid.is_empty() && self.input.fluid == fluid && self.input.pressure == pressure {
            u64::from(self.input.free())
        } else {
            0
        }
    }
    fn fluid_give_at(&mut self, fluid: &str, pressure: u8, mb: u64) {
        if self.input.pressure == pressure && pressure <= HIGHEST_VALID_PRESSURE {
            self.input.fill(fluid, mb.min(u64::from(u32::MAX)) as u32);
        }
    }
    fn fluid_take_at(&mut self, fluid: &str, pressure: u8, mb: u64) {
        if self.output.fluid == fluid && self.output.pressure == pressure {
            self.output.drain(mb.min(u64::from(u32::MAX)) as u32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::container_bridge::Stack;

    fn typed(fluid: &str, mb: u32, pressure: u8) -> Compressor {
        let mut c = Compressor::new(Shape::Tall);
        c.input.fluid = fluid.into();
        c.input.mb = mb;
        c.input.pressure = pressure;
        c.power = MAX_POWER;
        c
    }

    fn identifier(fluid: &str) -> Option<Stack> {
        let mut s = Stack::new("ntm:fluid_identifier_multi", 1);
        s.custom = Some(format!("{{ntm_fluid:\"{fluid}\"}}"));
        Some(s)
    }

    /// `ticks` ticks with the power topped up before each (the machine only
    /// holds 100 000 HE; a counting tick takes 2 500): the HE taken in all.
    fn run(c: &mut Compressor, ticks: usize) -> u64 {
        let mut paid = 0;
        for _ in 0..ticks {
            c.power = MAX_POWER;
            c.step();
            paid += MAX_POWER - c.power;
        }
        paid
    }

    /// `CompressorRecipes.registerDefaults` (`CompressorRecipes.java:25-31`).
    #[test]
    fn the_recipes_are_the_five_of_the_ntm_and_nothing_else() {
        let r = |input, fluid, amount, pressure, duration| Some(Recipe { input, fluid, amount, pressure, duration });
        assert_eq!(recipe("petroleum", 0), r(2_000, "petroleum", 2_000, 1, 20));
        assert_eq!(recipe("petroleum", 1), r(2_000, "lpg", 1_000, 0, 20));
        assert_eq!(recipe("blood", 3), r(1_000, "heavyoil", 250, 0, 200));
        assert_eq!(recipe("perfluoromethyl", 0), r(1_000, "perfluoromethyl", 1_000, 1, 50));
        assert_eq!(recipe("perfluoromethyl", 1), r(1_000, "perfluoromethyl_cold", 1_000, 0, 50));
        for (f, p) in [("petroleum", 2), ("blood", 0), ("blood", 2), ("perfluoromethyl", 2), ("lpg", 0), ("air", 0), ("", 0)] {
            assert_eq!(recipe(f, p), None, "{f}@{p}");
        }
        for r in ["petroleum", "lpg", "blood", "heavyoil", "perfluoromethyl", "perfluoromethyl_cold"] {
            assert!(crate::fluid::get(r).is_some(), "{r} is no fluid");
        }
    }

    #[test]
    fn it_starts_as_the_java_tanks_do() {
        let c = Compressor::new(Shape::Tall);
        assert_eq!((c.input.cap, c.input.pressure, c.input.fluid.as_str()), (16_000, 0, ""));
        assert_eq!((c.output.cap, c.output.pressure, c.output.fluid.as_str()), (16_000, 1, ""));
        assert_eq!((c.process_time, c.power_req, c.progress, MAX_POWER), (100, 0, 0, 100_000));
        assert_eq!(c.demand(), 100_000);
        assert_eq!(c.slots.len(), 4);
    }

    /// No recipe for (fluid, pressure): 1 000 mB in, 1 000 mB out, one
    /// pressure up, every 100 ticks, paying 2 500 HE on each of them.
    #[test]
    fn a_generic_compression_takes_a_hundred_ticks_and_one_pressure_up() {
        let mut c = typed("air", 16_000, 0);
        assert_eq!(run(&mut c, 99), 99 * 2_500);
        assert_eq!((c.input.mb, c.output.mb, c.progress), (16_000, 0, 99));
        assert_eq!((c.output.fluid.as_str(), c.output.pressure), ("air", 1));
        assert!(c.on);
        assert_eq!(run(&mut c, 1), 2_500);
        assert_eq!((c.input.mb, c.output.mb, c.progress), (15_000, 1_000, 0));
    }

    #[test]
    fn the_generic_one_goes_up_from_any_pressure() {
        for p in 0..=4u8 {
            let mut c = typed("air", 1_000, p);
            c.step();
            assert_eq!((c.output.fluid.as_str(), c.output.pressure), ("air", p + 1), "{p}");
        }
    }

    #[test]
    fn the_power_gate_is_strictly_more_than_the_requirement() {
        let mut c = typed("air", 5_000, 0);
        c.power = 2_500;
        c.step();
        assert!(!c.step());
        assert_eq!((c.progress, c.on, c.power), (0, false, 2_500));
        c.power = 2_501;
        assert!(c.step());
        assert_eq!((c.progress, c.on, c.power), (1, true, 1));
        // 1 HE left: it cannot count again, and the count restarts.
        assert!(c.step());
        assert_eq!((c.progress, c.on), (0, false));
    }

    #[test]
    fn it_waits_for_the_input_and_the_room() {
        let mut c = typed("air", 999, 0);
        c.step();
        assert!(!c.on);
        let mut c = typed("air", 5_000, 0);
        c.output.pressure = 1;
        c.step();
        c.output.mb = 15_001;
        c.step();
        assert!(!c.on, "1 001 mB of output do not fit");
        c.output.mb = 15_000;
        c.step();
        assert!(c.on, "exactly full still fits");
    }

    #[test]
    fn a_recipe_sets_the_time_the_input_and_the_output() {
        // Petroleum at 0 PU: 2 000 mB in, 2 000 mB of petroleum at 1 PU, 20 ticks.
        let mut c = typed("petroleum", 3_999, 0);
        run(&mut c, 19);
        assert_eq!((c.process_time, c.input.mb, c.output.mb, c.progress), (20, 3_999, 0, 19));
        assert_eq!((c.output.fluid.as_str(), c.output.pressure), ("petroleum", 1));
        c.step();
        assert_eq!((c.input.mb, c.output.mb, c.progress), (1_999, 2_000, 0));
        // Not enough input for another (needs 2 000).
        c.step();
        assert!(!c.on);

        // Petroleum at 1 PU: 2 000 mB in, 1 000 mB of LPG at 0 PU.
        let mut c = typed("petroleum", 2_000, 1);
        run(&mut c, 20);
        assert_eq!((c.input.mb, c.output.mb), (0, 1_000));
        assert_eq!((c.output.fluid.as_str(), c.output.pressure), ("lpg", 0));

        // Blood at 3 PU: 1 000 mB in, 250 mB of heavy oil at 0 PU, 200 ticks.
        let mut c = typed("blood", 1_000, 3);
        run(&mut c, 199);
        assert_eq!((c.process_time, c.output.mb), (200, 0));
        c.step();
        assert_eq!((c.input.mb, c.output.mb, c.output.fluid.as_str(), c.output.pressure), (0, 250, "heavyoil", 0));

        // Perfluoromethyl: 50 ticks, 0 -> 1 -> cold at 0.
        let mut c = typed("perfluoromethyl", 1_000, 0);
        run(&mut c, 50);
        assert_eq!((c.input.mb, c.output.mb, c.output.fluid.as_str(), c.output.pressure), (0, 1_000, "perfluoromethyl", 1));
        let mut c = typed("perfluoromethyl", 1_000, 1);
        run(&mut c, 50);
        assert_eq!((c.output.mb, c.output.fluid.as_str(), c.output.pressure), (1_000, "perfluoromethyl_cold", 0));
    }

    #[test]
    fn the_recipes_output_room_is_the_recipes_amount() {
        let mut c = typed("petroleum", 2_000, 0);
        c.step();
        c.output.mb = 14_001;
        run(&mut c, 3);
        assert!(!c.on, "2 000 mB of output do not fit in 1 999");
        c.output.mb = 14_000;
        c.step();
        assert!(c.on);
    }

    /// Speed: generic 60/20/10 ticks, a recipe's time over level + 1;
    /// power: the requirement over level + 1; overdrive: time over level + 1
    /// and the requirement times `2 * level + 1`.
    #[test]
    fn the_upgrades_change_the_time_and_the_power() {
        let up = |ids: &[&str]| {
            let mut c = typed("air", 16_000, 0);
            c.slots[2] = ids.first().map(|i| Stack::new(i, 1));
            c.slots[3] = ids.get(1).map(|i| Stack::new(i, 1));
            c.step();
            (c.process_time, c.power_req)
        };
        assert_eq!(up(&[]), (100, 2_500));
        assert_eq!(up(&["ntm:upgrade_speed_1"]), (60, 2_500));
        assert_eq!(up(&["ntm:upgrade_speed_2"]), (20, 2_500));
        assert_eq!(up(&["ntm:upgrade_speed_3"]), (10, 2_500));
        assert_eq!(up(&["ntm:upgrade_speed_1", "ntm:upgrade_speed_2"]), (10, 2_500));
        assert_eq!(up(&["ntm:upgrade_speed_3", "ntm:upgrade_speed_3"]), (10, 2_500));
        assert_eq!(up(&["ntm:upgrade_power_1"]), (100, 1_250));
        assert_eq!(up(&["ntm:upgrade_power_2"]), (100, 833));
        assert_eq!(up(&["ntm:upgrade_power_3"]), (100, 625));
        assert_eq!(up(&["ntm:upgrade_overdrive_1"]), (50, 7_500));
        assert_eq!(up(&["ntm:upgrade_overdrive_3"]), (25, 17_500));
        assert_eq!(up(&["ntm:upgrade_overdrive_3", "ntm:upgrade_overdrive_3"]), (14, 32_500));
        assert_eq!(up(&["ntm:upgrade_speed_3", "ntm:upgrade_overdrive_3"]), (2, 17_500));
        assert_eq!(up(&["ntm:upgrade_power_3", "ntm:upgrade_overdrive_1"]), (50, 1_875));
        // Not a compressor's upgrades.
        assert_eq!(up(&["ntm:upgrade_effect_3", "ntm:upgrade_fortune_1"]), (100, 2_500));

        // With a recipe the time is the recipe's over level + 1 (not 60/20/10).
        let mut c = typed("petroleum", 2_000, 0);
        c.slots[2] = Some(Stack::new("ntm:upgrade_speed_1", 1));
        c.step();
        assert_eq!(c.process_time, 10);
        let mut c = typed("blood", 1_000, 3);
        c.slots[2] = Some(Stack::new("ntm:upgrade_speed_3", 1));
        c.step();
        assert_eq!(c.process_time, 50);
        c.slots[3] = Some(Stack::new("ntm:upgrade_overdrive_3", 1));
        c.step();
        assert_eq!(c.process_time, 12);
    }

    /// `processTime` never drops under 1; the battery's slot is read too
    /// (`checkSlots(slots, 1, 3)`), the identifier's is not.
    #[test]
    fn the_time_has_a_floor_and_the_upgrade_range_is_slots_one_to_three() {
        let mut c = typed("air", 16_000, 0);
        c.slots[2] = Some(Stack::new("ntm:upgrade_overdrive_3", 1));
        c.slots[3] = Some(Stack::new("ntm:upgrade_overdrive_3", 1));
        c.slots[1] = Some(Stack::new("ntm:upgrade_overdrive_3", 1));
        c.step();
        assert_eq!((c.process_time, c.power_req), (10, 2_500 * 19));
        let mut c = typed("air", 16_000, 0);
        c.slots[0] = Some(Stack::new("ntm:upgrade_speed_3", 1));
        c.step();
        assert_eq!(c.process_time, 100);
        let mut c = typed("air", 16_000, 0);
        c.slots[1] = Some(Stack::new("ntm:upgrade_speed_3", 1));
        c.step();
        assert_eq!(c.process_time, 10);
    }

    /// A recipe's time over speed and overdrive can reach 0: the floor is 1 tick
    /// (`if(processTime <= 0) processTime = 1`), so it still counts and works.
    #[test]
    fn a_time_that_reaches_zero_is_one_tick() {
        let mut c = typed("petroleum", 4_000, 0);
        c.slots[2] = Some(Stack::new("ntm:upgrade_speed_3", 1));
        c.slots[3] = Some(Stack::new("ntm:upgrade_overdrive_3", 1));
        c.step();
        // 20 / 4 = 5, / 4 = 1 (not yet zero): one overdrive more.
        assert_eq!(c.process_time, 1);
        let mut c = typed("petroleum", 4_000, 0);
        c.slots[1] = Some(Stack::new("ntm:upgrade_overdrive_3", 1));
        c.slots[2] = Some(Stack::new("ntm:upgrade_overdrive_3", 1));
        c.slots[3] = Some(Stack::new("ntm:upgrade_speed_3", 1));
        c.step();
        // 20 / 4 = 5, then / (6 + 1) = 0 -> 1.
        assert_eq!(c.process_time, 1);
        assert_eq!(c.input.mb, 2_000, "the operation ran on its first counting tick");
    }

    #[test]
    fn a_creative_battery_in_slot_one_fills_the_power() {
        let mut c = typed("air", 0, 0);
        c.power = 0;
        c.slots[1] = Some(Stack::new("ntm:battery_creative", 1));
        c.step();
        assert_eq!(c.power, MAX_POWER);
    }

    #[test]
    fn the_identifier_types_the_input_and_a_new_type_empties_it() {
        let mut c = Compressor::new(Shape::Tall);
        c.slots[0] = identifier("air");
        c.step();
        assert_eq!(c.input.fluid, "air");
        c.input.mb = 700;
        c.step();
        assert_eq!(c.input.mb, 700, "the same type keeps the fluid");
        c.slots[0] = identifier("petroleum");
        c.step();
        assert_eq!((c.input.fluid.as_str(), c.input.mb), ("petroleum", 0));
        // The output follows: petroleum at 0 PU -> petroleum at 1 PU.
        assert_eq!((c.output.fluid.as_str(), c.output.pressure), ("petroleum", 1));
        c.slots[0] = identifier("none");
        c.step();
        assert_eq!((c.input.fluid.as_str(), c.output.fluid.as_str()), ("", ""));
    }

    #[test]
    fn the_buttons_set_the_compression_and_follow_the_recipe() {
        let mut c = typed("petroleum", 500, 0);
        c.step();
        c.output.mb = 300;
        // Same pressure: nothing happens (the output keeps its 300 mB).
        assert!(!c.button(0));
        assert_eq!((c.input.mb, c.output.mb), (500, 300));
        // 1 PU: the recipe is petroleum@1 -> LPG@0; the new pressures empty both.
        c.button(1);
        assert_eq!((c.input.pressure, c.input.mb), (1, 0));
        assert_eq!((c.output.fluid.as_str(), c.output.pressure, c.output.mb), ("lpg", 0, 0));
        // 2 PU: no recipe for petroleum@2: output at 3 PU (its type stays).
        c.button(2);
        assert_eq!((c.input.pressure, c.output.pressure, c.output.fluid.as_str()), (2, 3, "lpg"));
        // Buttons 0..=4 only.
        c.button(5);
        c.button(9);
        assert_eq!(c.input.pressure, 2);
        c.button(4);
        assert_eq!((c.input.pressure, c.output.pressure), (4, 5));
        c.step();
        assert_eq!((c.output.fluid.as_str(), c.output.pressure), ("petroleum", 5), "the next tick types it");
    }

    /// `receiveControl` does nothing for the pressure the input already has
    /// (`if(compression != tanks[0].getPressure())`): the output keeps its type until the tick.
    #[test]
    fn the_same_compression_changes_nothing() {
        let mut c = typed("petroleum", 500, 0);
        assert_eq!(c.output.fluid, "");
        c.button(0);
        assert_eq!((c.output.fluid.as_str(), c.output.pressure, c.input.mb), ("", 1, 500));
    }

    #[test]
    fn a_pressure_change_loses_the_output_in_the_next_setup_too() {
        let mut c = typed("air", 5_000, 0);
        c.step();
        c.output.mb = 4_000;
        c.input.pressure = 1;
        c.step();
        assert_eq!((c.output.pressure, c.output.mb), (2, 0));
    }

    #[test]
    fn the_screen_values() {
        let mut c = typed("air", 3_000, 2);
        c.power = 50_000;
        c.step();
        c.progress = 50;
        let v = Compressor::screen_data(&c);
        assert_eq!(v.len(), 12);
        assert_eq!((v[0], v[1], v[2], v[3]), (47_500, 100_000, 3_000, 16_000));
        assert_eq!((v[5], v[6]), (0, 16_000));
        assert_eq!(v[4], crate::fluid::get("air").unwrap().color as i32);
        assert_eq!((v[8], v[9], v[10], v[11]), (50 * 55 / 100, 55, 2, 1));
        c.power = 2_499;
        c.step();
        assert_eq!(Compressor::screen_data(&c)[11], 0, "power < powerRequirement");
    }

    #[test]
    fn it_takes_and_gives_fluid_only_at_the_tanks_own_pressure() {
        let mut c = typed("air", 0, 2);
        c.setup_tanks();
        c.output.fill("air", 4_000);
        for p in 0..=5u8 {
            let want_in = if p == 2 { 16_000 } else { 0 };
            let want_out = if p == 3 { 4_000 } else { 0 };
            assert_eq!(c.fluid_demand_at("air", p), want_in, "demand @{p}");
            assert_eq!(c.fluid_offer_at("air", p), want_out, "offer @{p}");
        }
        assert_eq!((c.fluid_demand("air"), c.fluid_offer("air")), (0, 0));
        assert_eq!(c.fluid_demand_at("water", 2), 0);
        assert_eq!(c.fluid_offer_at("water", 3), 0);
        c.fluid_give_at("air", 1, 500);
        c.fluid_give_at("air", 0, 500);
        assert_eq!(c.input.mb, 0);
        c.fluid_give_at("air", 2, 500);
        assert_eq!(c.input.mb, 500);
        c.fluid_take_at("air", 2, 100);
        assert_eq!(c.output.mb, 4_000);
        c.fluid_take_at("air", 3, 100);
        assert_eq!(c.output.mb, 3_900);
        assert_eq!(c.fluid_types(), vec!["air".to_string(), "air".to_string()]);
        assert!(!c.accepts_any());
    }

    #[test]
    fn an_untyped_compressor_asks_for_nothing() {
        let c = Compressor::new(Shape::Tall);
        assert_eq!(c.fluid_demand_at("", 0), 0);
        assert_eq!(c.fluid_offer_at("", 1), 0);
        assert!(c.fluid_types().is_empty());
    }

    #[test]
    fn it_runs_through_the_network_only_at_equal_pressures() {
        let mut src = Compressor::new(Shape::Tall);
        src.input.fluid = "air".into();
        src.input.pressure = 0;
        src.setup_tanks();
        src.output.mb = 5_000;
        let mut same = typed("air", 0, 1);
        let mut other = typed("air", 0, 2);
        {
            let mut nodes: Vec<&mut dyn FluidNode> = vec![&mut src, &mut same, &mut other];
            assert_eq!(crate::fluid::distribute("air", &mut nodes), 5_000);
        }
        assert_eq!((same.input.mb, other.input.mb, src.output.mb), (5_000, 0, 0));
    }

    #[test]
    fn the_state_survives_a_save_and_an_old_one_opens() {
        let mut c = typed("petroleum", 1_234, 1);
        c.power = 777;
        c.progress = 5;
        c.step();
        let text = serde_json::to_string(&c).unwrap();
        let back: Compressor = serde_json::from_str(&text).unwrap();
        assert_eq!((back.power, back.progress, &back.input, &back.output), (c.power, c.progress, &c.input, &c.output));
        assert!(!back.on);
        assert_eq!(back.process_time, 100, "recomputed at the first tick");
        // A save whose tanks have no pressure field: both at 0.
        let old = r#"{"shape":"Compact","power":5,"input":{"fluid":"air","mb":9,"cap":16000},"output":{"mb":0,"cap":16000},"progress":3}"#;
        let o: Compressor = serde_json::from_str(old).unwrap();
        assert_eq!((o.shape, o.input.pressure, o.output.pressure, o.progress), (Shape::Compact, 0, 0, 3));
    }

    #[test]
    fn the_ports_of_the_tall_one_are_three_on_its_core_row() {
        let cells = Compressor::port_cells(BLOCK, [10, 64, 20], Some("north"));
        assert_eq!(cells, vec![([10, 64, 21], [0, 0, 1]), ([11, 64, 20], [1, 0, 0]), ([9, 64, 20], [-1, 0, 0])]);
        // Turned: east.
        let cells = Compressor::port_cells(BLOCK, [0, 0, 0], Some("east"));
        assert_eq!(cells, vec![([-1, 0, 0], [-1, 0, 0]), ([0, 0, 1], [0, 0, 1]), ([0, 0, -1], [0, 0, -1])]);
        assert!(Compressor::port_cells(BLOCK, [0, 0, 0], None).is_empty());
        assert!(Compressor::port_cells("ntm:machine_intake", [0, 0, 0], Some("north")).is_empty());
    }

    #[test]
    fn the_ports_of_the_compact_one_are_six_on_its_second_layer() {
        let cells = Compressor::port_cells(COMPACT, [10, 64, 20], Some("north"));
        assert_eq!(
            cells,
            vec![
                ([13, 65, 20], [1, 0, 0]),
                ([7, 65, 20], [-1, 0, 0]),
                ([9, 65, 19], [0, 0, -1]),
                ([11, 65, 19], [0, 0, -1]),
                ([9, 65, 21], [0, 0, 1]),
                ([11, 65, 21], [0, 0, 1]),
            ]
        );
        let cells = Compressor::port_cells(COMPACT, [0, 0, 0], Some("west"));
        assert_eq!(cells.len(), 6);
        assert!(cells.iter().all(|(c, f)| c[1] == 1 && f[1] == 0));
    }

    #[test]
    fn power_is_taken_up_to_the_max() {
        let mut c = Compressor::new(Shape::Compact);
        assert_eq!((c.role(), c.offer()), (Role::Consumer, 0));
        c.give(99_000);
        c.give(5_000);
        assert_eq!((c.power, c.demand()), (100_000, 0));
        c.take(10);
        assert_eq!(c.power, 100_000);
        assert_eq!(c.level(5), 0);
        c.input.fluid = "air".into();
        c.input.mb = 1_000;
        c.step();
        assert_eq!(c.level(5), 5);
    }
}
