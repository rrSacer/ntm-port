//! Energy batch 3 (docs/content-energy-combustion.md): the diesel generator,
//! the combustion engine, the turbofan and the gas turbine
//! (`tileentity/machine/TileEntityMachineDiesel.java`,
//! `TileEntityMachineCombustionEngine.java`, `TileEntityMachineTurbofan.java`
//! and `TileEntityMachineTurbineGas.java`). What a fuel gives is its
//! `FT_Combustible` (grade and HE a bucket, `data/machines/combustion.json`
//! from the content's `fluid_traits.json`); each machine rounds it its own way
//! and the port keeps the NTM's integers: the diesel divides the bucket by a
//! thousand before the efficiency, the engine adds in a `double` to a `long`,
//! the turbofan casts its output to an `int`.
//!
//! Each keeps a power buffer the network draws from (`IEnergyProviderMK2`)
//! and caps it at the end of the tick. Not here: pollution (the smoke
//! tanks), sounds, the turbofan's wind, fire and gibs (and so its blood tank,
//! which nothing fills), the engine's door, items charged from the buffer
//! (the port has no charge in items) and the gas turbine's random ramp of
//! its instant power, which takes the middle of the NTM's random step.

use crate::{
    energy::{EnergyNode, Role},
    fluid::{self, FluidNode, Tank},
    machine::Logic,
    server::container_bridge::{Slots, Slotted, Stack},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

// ------------------------------------------------------------------ tables

#[derive(Deserialize)]
struct Table {
    /// `FuelGrade`, in ordinal order.
    grades: Vec<String>,
    /// Fluid → (grade ordinal, HE a bucket).
    fuels: HashMap<String, (usize, u64)>,
    diesel_efficiency: HashMap<String, f64>,
    /// Piston set item → its efficiency by grade ordinal.
    pistons: HashMap<String, Vec<f64>>,
}

fn table() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!("../data/machines/combustion.json"))
            .expect("combustion.json")
    })
}

pub const LOW: usize = 0;
pub const MEDIUM: usize = 1;
pub const HIGH: usize = 2;
pub const AERO: usize = 3;
pub const GAS: usize = 4;

/// `FT_Combustible` of `fluid`: (grade ordinal, `getCombustionEnergy`).
pub fn fuel(fluid: &str) -> Option<(usize, u64)> {
    table().fuels.get(fluid).copied()
}

pub fn warm() {
    table();
}

/// `TileEntityMachineDiesel.getHEFromFuel`: the bucket's HE over a thousand
/// (an integer division), times the grade's efficiency, cut to a long; none
/// for LOW and for grades without an efficiency.
pub fn diesel_he(fluid: &str) -> u64 {
    let Some((grade, he)) = fuel(fluid).filter(|f| f.0 != LOW) else {
        return 0;
    };
    let eff = table()
        .grades
        .get(grade)
        .and_then(|g| table().diesel_efficiency.get(g))
        .copied()
        .unwrap_or(0.0);
    ((he / 1000) as f64 * eff) as u64
}

/// The piston sets' efficiency for `grade`; the item is the slot's.
fn piston_eff(item: &str, grade: usize) -> Option<f64> {
    table()
        .pistons
        .get(item)
        .and_then(|e| e.get(grade))
        .copied()
}

/// The piston the engine's screen shows: 1..=4 by metadata, 0 for none.
fn piston_index(item: &str) -> i32 {
    ["steel", "dura", "desh", "starmetal"]
        .iter()
        .position(|m| item == format!("ntm:piston_set__{m}"))
        .map_or(0, |i| i as i32 + 1)
}

pub const UPGRADES: [&str; 3] = [
    "ntm:upgrade_afterburn_1",
    "ntm:upgrade_afterburn_2",
    "ntm:upgrade_afterburn_3",
];
/// The fluid identifier (`ItemFluidIDMulti`; `server::native_server::IDENTIFIER`).
const IDENTIFIER: &str = "ntm:fluid_identifier_multi";
pub const FLAME_PONY: &str = "ntm:flame_pony";

// ----------------------------------------------------------------- helpers

fn ensure(slots: &mut Slots, n: usize) {
    if slots.len() < n {
        slots.resize(n, None);
    }
}

fn item(slots: &Slots, i: usize) -> Option<&str> {
    slots.get(i).and_then(|s| s.as_ref()).map(|s| s.id.as_str())
}

/// `ItemFluidIDMulti`'s type in the stack's `custom_data` (`ntm_fluid`).
fn identifier_type(stack: &Stack) -> Option<String> {
    if stack.id != IDENTIFIER {
        return None;
    }
    let custom = stack.custom.as_deref()?;
    let rest = &custom[custom.find("ntm_fluid")? + "ntm_fluid".len()..];
    let rest = &rest[rest.find('"')? + 1..];
    let name = &rest[..rest.find('"')?];
    fluid::get(name)
        .filter(|t| t.id != 0)
        .map(|t| t.string_id.clone())
}

/// `FluidTank.setType(slot, slots)`: the identifier in `slot` types the tank
/// (the content goes with the old type). True when the type changed. `only`
/// narrows the types the machine takes.
fn type_from_identifier(
    tank: &mut Tank,
    slots: &Slots,
    slot: usize,
    only: impl Fn(&str) -> bool,
) -> bool {
    let Some(kind) = slots
        .get(slot)
        .and_then(|s| s.as_ref())
        .and_then(identifier_type)
    else {
        return false;
    };
    if tank.fluid == kind || !only(&kind) {
        return false;
    }
    tank.set_type(&kind);
    true
}

/// `FluidTank.loadTank(in, out, slots)`: one full container of the tank's
/// type into it, the empty one into `out`, when both fit.
pub(crate) fn load_tank(tank: &mut Tank, slots: &mut Slots, from: usize, to: usize) {
    ensure(slots, from.max(to) + 1);
    let Some(id) = slots[from].as_ref().map(|s| s.id.clone()) else {
        return;
    };
    let Some(c) = fluid::contents(&id).filter(|c| c.fluid == tank.fluid && tank.free() >= c.mb)
    else {
        return;
    };
    if !slots[to]
        .as_ref()
        .is_none_or(|o| o.id == c.empty && o.count < 64)
    {
        return;
    }
    tank.fill(&c.fluid, c.mb);
    if let Some(s) = &mut slots[from] {
        s.count -= 1;
        if s.count == 0 {
            slots[from] = None;
        }
    }
    match &mut slots[to] {
        Some(o) => o.count += 1,
        None => slots[to] = Some(Stack::new(&c.empty, 1)),
    }
}

fn color(tank: &Tank) -> i32 {
    fluid::get(&tank.fluid).map_or(0, |f| f.color as i32)
}

fn clamp32(v: u64) -> i32 {
    v.min(i32::MAX as u64) as i32
}

pub mod actions {
    /// The diesel's and the engine's ignition, the turbine's start.
    pub const IGNITION: u32 = 0;
    /// The turbine's auto mode.
    pub const AUTO: u32 = 1;
    /// The engine's setting and the turbine's slider: `SLIDER + n`.
    pub const SLIDER: u32 = 100;
}

// ------------------------------------------------------------------ diesel

pub const DIESEL_POWER: u64 = 50_000;
pub const DIESEL_TANK: u32 = 16_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Diesel {
    #[serde(skip)]
    pub slots: Slots,
    pub on: bool,
    pub power: u64,
    pub tank: Tank,
    #[serde(skip)]
    pub was_on: bool,
    /// A redstone signal on the block.
    #[serde(skip)]
    pub powered: bool,
}

impl Default for Diesel {
    fn default() -> Self {
        Self {
            slots: Slots::new(),
            on: false,
            power: 0,
            tank: Tank::typed("diesel", DIESEL_TANK),
            was_on: false,
            powered: false,
        }
    }
}

impl Diesel {
    /// `updateEntity`: the container in, the identifier, then `generate`.
    pub fn step(&mut self) {
        ensure(&mut self.slots, 4);
        self.was_on = false;
        type_from_identifier(&mut self.tank, &self.slots, 3, |_| true);
        load_tank(&mut self.tank, &mut self.slots, 0, 1);
        if self.on {
            self.generate();
        }
    }

    /// `generate`: 1 mB a tick for `getHEFromFuel` HE, up to the buffer.
    fn generate(&mut self) {
        let he = diesel_he(&self.tank.fluid);
        if self.powered || he == 0 || self.tank.mb == 0 {
            return;
        }
        self.was_on = true;
        self.tank.drain(1);
        self.power = if self.power + he <= DIESEL_POWER {
            self.power + he
        } else {
            DIESEL_POWER
        };
    }
}

impl Slotted for Diesel {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    /// `GUIMachineDiesel`: power, its cap, the tank, its colour, on, burning,
    /// the HE a tick the fuel gives.
    fn screen_data(&self) -> Vec<i32> {
        vec![
            clamp32(self.power),
            DIESEL_POWER as i32,
            clamp32(u64::from(self.tank.mb)),
            DIESEL_TANK as i32,
            color(&self.tank),
            i32::from(self.on),
            i32::from(self.was_on),
            clamp32(diesel_he(&self.tank.fluid)),
        ]
    }
    fn button(&mut self, id: u32) -> bool {
        if id == actions::IGNITION {
            self.on = !self.on;
        }
        false
    }
}

// ------------------------------------------------------------------ engine

pub const ENGINE_POWER: u64 = 2_500_000;
pub const ENGINE_TANK: u32 = 24_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Engine {
    #[serde(skip)]
    pub slots: Slots,
    pub on: bool,
    /// `setting`: what the screen's slider says, 0..=30 there, any int here.
    pub setting: i32,
    pub power: u64,
    pub tank: Tank,
    /// Tenths of a mB the tank's last mB is split into.
    pub tenth: i32,
    #[serde(skip)]
    pub was_on: bool,
}

impl Default for Engine {
    fn default() -> Self {
        Self {
            slots: Slots::new(),
            on: false,
            setting: 0,
            power: 0,
            tank: Tank::typed("diesel", ENGINE_TANK),
            tenth: 0,
            was_on: false,
        }
    }
}

impl Engine {
    /// `updateEntity`, the burn: `setting · 2` tenths of a mB a tick of what
    /// is in the tank, `he / 10 000 · efficiency` each, added to the long in
    /// a double.
    pub fn step(&mut self) {
        ensure(&mut self.slots, 5);
        load_tank(&mut self.tank, &mut self.slots, 0, 1);
        if type_from_identifier(&mut self.tank, &self.slots, 4, |_| true) {
            self.tenth = 0;
        }
        self.was_on = false;
        let mut fill = self.tank.mb as i32 * 10 + self.tenth;
        let piston = item(&self.slots, 2)
            .filter(|id| id.starts_with("ntm:piston_set__"))
            .map(str::to_owned);
        if let (true, true, Some(piston), true, Some((grade, he))) = (
            self.on,
            self.setting > 0,
            piston,
            fill > 0,
            fuel(&self.tank.fluid),
        ) {
            let eff = piston_eff(&piston, grade).unwrap_or(0.0);
            if eff > 0.0 {
                let speed = self.setting.wrapping_mul(2);
                let to_burn = fill.min(speed);
                self.power =
                    (self.power as f64 + f64::from(to_burn) * (he as f64 / 10_000.0) * eff) as u64;
                fill -= to_burn;
                if to_burn > 0 {
                    self.was_on = true;
                }
                self.tank.mb = (fill / 10).max(0) as u32;
                self.tenth = fill % 10;
            }
        }
        self.power = self.power.min(ENGINE_POWER);
    }

    /// `GUICombustionEngine`'s HE a tick: `setting · 0.2 · he / 1000 · eff`.
    fn display_he(&self) -> i32 {
        let (Some(piston), Some((grade, he))) = (item(&self.slots, 2), fuel(&self.tank.fluid))
        else {
            return 0;
        };
        let eff = piston_eff(piston, grade).unwrap_or(0.0);
        (f64::from(self.setting) * 0.2 * he as f64 / 1_000.0 * eff) as i32
    }
}

impl Slotted for Engine {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    /// Power, cap, tank, cap, colour, on, setting, the piston (1..=4), HE a
    /// tick. The model's canister reads [4].
    fn screen_data(&self) -> Vec<i32> {
        vec![
            clamp32(self.power),
            ENGINE_POWER as i32,
            clamp32(u64::from(self.tank.mb)),
            ENGINE_TANK as i32,
            color(&self.tank),
            i32::from(self.on),
            self.setting,
            item(&self.slots, 2).map_or(0, piston_index),
            self.display_he(),
        ]
    }
    /// 0: ignition; 100 + n: the setting n (the slider).
    fn button(&mut self, id: u32) -> bool {
        if id == actions::IGNITION {
            self.on = !self.on;
        } else if (actions::SLIDER..actions::SLIDER + 31).contains(&id) {
            self.setting = (id - actions::SLIDER) as i32;
        }
        false
    }
}

// --------------------------------------------------------------- turbofan

pub const FAN_POWER: u64 = 1_000_000;
pub const FAN_TANK: u32 = 24_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Turbofan {
    #[serde(skip)]
    pub slots: Slots,
    pub power: u64,
    pub tank: Tank,
    pub blood: Tank,
    #[serde(skip)]
    pub afterburner: i32,
    #[serde(skip)]
    pub was_on: bool,
    /// A redstone signal on any of its four connectors.
    #[serde(skip)]
    pub powered: bool,
    #[serde(skip)]
    pub output: i32,
}

impl Default for Turbofan {
    fn default() -> Self {
        Self {
            slots: Slots::new(),
            power: 0,
            tank: Tank::typed("kerosene", FAN_TANK),
            blood: Tank::typed("blood", FAN_TANK),
            afterburner: 0,
            was_on: false,
            powered: false,
            output: 0,
        }
    }
}

impl Turbofan {
    /// `updateEntity`: only AERO fuel burns, `1 + afterburner` mB a tick, its
    /// energy times `1 + min(afterburner / 3, 4)`.
    pub fn step(&mut self) {
        ensure(&mut self.slots, 5);
        self.output = 0;
        load_tank(&mut self.tank, &mut self.slots, 0, 1);
        type_from_identifier(&mut self.tank, &self.slots, 4, |_| true);
        self.was_on = false;
        self.afterburner = match item(&self.slots, 2) {
            Some(FLAME_PONY) => 100,
            Some(u) => UPGRADES
                .iter()
                .position(|x| *x == u)
                .map_or(0, |i| i as i32 + 1),
            None => 0,
        };
        let amount = 1 + self.afterburner;
        let to_burn = amount.min(self.tank.mb as i32);
        if !self.powered {
            let burn_value = fuel(&self.tank.fluid)
                .filter(|f| f.0 == AERO)
                .map_or(0, |f| (f.1 / 1000) as i64);
            if to_burn > 0 {
                self.was_on = true;
                self.tank.drain(to_burn as u32);
                self.output = ((burn_value * i64::from(to_burn)) as f64
                    * (1.0 + (f64::from(self.afterburner) / 3.0).min(4.0)))
                    as i32;
                self.power = (self.power as i64 + i64::from(self.output)).max(0) as u64;
            }
        }
        self.power = self.power.min(FAN_POWER);
    }
}

impl Slotted for Turbofan {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    /// Power, cap, fuel, cap, colour, the afterburner (the model's cover is
    /// closed at 0), spinning.
    fn screen_data(&self) -> Vec<i32> {
        vec![
            clamp32(self.power),
            FAN_POWER as i32,
            clamp32(u64::from(self.tank.mb)),
            FAN_TANK as i32,
            color(&self.tank),
            self.afterburner,
            i32::from(self.was_on),
        ]
    }
}

// ------------------------------------------------------------ gas turbine

pub const TURBINE_POWER: u64 = 1_000_000;
pub const RPM_IDLE: i32 = 10;
pub const TEMP_IDLE: i32 = 300;

/// `fuelMaxCons`: mB a tick at full power, 5 for any other gas.
fn max_consumption(fluid: &str) -> f64 {
    match fluid {
        "gas" => 50.0,
        "syngas" => 10.0,
        "oxyhydrogen" => 100.0,
        "reformgas" => 5.0,
        _ => 5.0,
    }
}

/// `getFluidBurnTemp`: from the fuel's energy, 300 to 800 degrees.
fn burn_temp(fluid: &str) -> i32 {
    let energy = fuel(fluid).map_or(0.0, |f| f.1 as f64);
    (800.0 - (-energy / 100_000.0).exp() * 300.0).floor() as i32
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GasTurbine {
    #[serde(skip)]
    pub slots: Slots,
    pub power: u64,
    pub gas: Tank,
    pub lube: Tank,
    pub water: Tank,
    pub steam: Tank,
    /// -1 starting, 0 off, 1 running.
    pub state: i32,
    pub auto_mode: bool,
    pub slider: i32,
    pub rpm: i32,
    pub temp: i32,
    pub counter: i32,
    pub instant: i32,
    rpm_last: i32,
    temp_last: i32,
    fuel_to_consume: f64,
    #[serde(skip)]
    pub throttle: i32,
    #[serde(skip)]
    age: u64,
}

impl Default for GasTurbine {
    fn default() -> Self {
        Self {
            slots: Slots::new(),
            power: 0,
            gas: Tank::typed("gas", 100_000),
            lube: Tank::typed("lubricant", 16_000),
            water: Tank::typed("water", 16_000),
            steam: Tank::typed("hotsteam", 160_000),
            state: 0,
            auto_mode: false,
            slider: 0,
            rpm: 0,
            temp: 0,
            counter: 0,
            instant: 0,
            rpm_last: 0,
            temp_last: 0,
            fuel_to_consume: 0.0,
            throttle: 0,
            age: 0,
        }
    }
}

impl GasTurbine {
    fn acceptable_fuel(&self) -> bool {
        fuel(&self.gas.fluid).is_some_and(|f| f.0 == GAS)
    }

    /// `updateEntity`.
    pub fn step(&mut self) {
        ensure(&mut self.slots, 2);
        self.age += 1;
        self.throttle = self.slider * 100 / 60;
        type_from_identifier(&mut self.gas, &self.slots, 1, |f| {
            fuel(f).is_some_and(|f| f.0 == GAS)
        });
        if self.auto_mode {
            self.auto_slider();
        }
        match self.state {
            0 => self.shutdown(),
            -1 => {
                self.stop_if_not_ready();
                self.startup();
            }
            1 => {
                self.stop_if_not_ready();
                self.run();
            }
            _ => {}
        }
        self.power = self.power.min(TURBINE_POWER);
    }

    /// The auto mode's slider: down with the buffer's fill, down with the
    /// fuel left, one step a tick.
    fn auto_slider(&mut self) {
        let fill = (self.power * 60 / TURBINE_POWER) as i32;
        let target = if i64::from(self.gas.mb) * 10 > i64::from(self.gas.cap) {
            60 - fill
        } else {
            (f64::from(self.gas.mb) * 0.0001 * f64::from(60 - fill)) as i32
        };
        match target.cmp(&self.slider) {
            std::cmp::Ordering::Greater => self.slider += 1,
            std::cmp::Ordering::Less => self.slider -= 1,
            std::cmp::Ordering::Equal => {}
        }
    }

    fn stop_if_not_ready(&mut self) {
        if self.gas.mb == 0 || self.lube.mb == 0 || !self.acceptable_fuel() {
            self.state = 0;
        }
    }

    /// `startup`: 580 ticks; the dial sweeps 0-100-0, then both climb to idle.
    fn startup(&mut self) {
        self.counter += 1;
        let c = self.counter;
        if c <= 20 {
            self.rpm = 5 * c;
        } else if c > 20 && c <= 40 {
            self.rpm = 100 - 5 * (c - 20);
        } else if c > 50 {
            self.rpm = RPM_IDLE * (c - 50) / 530;
            self.temp = TEMP_IDLE * (c - 50) / 530;
        }
        if c == 580 {
            self.counter = 225;
            self.state = 1;
        }
    }

    /// `shutdown`: the rotor slows to idle, then both run down over 225 ticks.
    fn shutdown(&mut self) {
        self.auto_mode = false;
        self.instant = 0;
        if self.slider > 0 {
            self.slider -= 1;
        }
        if self.rpm <= 10 && self.counter > 0 {
            if self.counter == 225 {
                self.rpm_last = self.rpm;
                self.temp_last = self.temp;
            }
            self.counter -= 1;
            self.rpm = self.rpm_last * self.counter / 225;
            self.temp = self.temp_last * self.counter / 225;
        } else if self.rpm > 11 {
            self.counter = 42_069;
            self.rpm -= 1;
        } else if self.rpm == 11 {
            self.counter = 225;
            self.rpm -= 1;
        }
    }

    /// `run`: the rotor's and the exchanger's inertia, then `makePower`.
    fn run(&mut self) {
        let target = (f64::from(self.throttle) * 0.9) as i32;
        if target > self.rpm - RPM_IDLE {
            if self.age.is_multiple_of(5) {
                self.rpm += 1;
            }
        } else if target < self.rpm - RPM_IDLE && self.age.is_multiple_of(2) {
            self.rpm -= 1;
        }
        let max_temp = burn_temp(&self.gas.fluid);
        let goal = self.throttle * 5 * (max_temp - TEMP_IDLE) / 500;
        if goal > self.temp - TEMP_IDLE {
            if self.age.is_multiple_of(2) {
                self.temp += 1;
            }
        } else if goal < self.temp - TEMP_IDLE && self.age.is_multiple_of(2) {
            self.temp -= 1;
        }
        self.make_power(max_consumption(&self.gas.fluid), self.throttle);
    }

    /// `makePower`: fuel by the idle share and the throttle (a fraction
    /// kept between ticks), a mB of lubricant every ten ticks, the power by
    /// the rotor's speed, the heat boiling water into dense steam.
    fn make_power(&mut self, cons_max: f64, throttle: i32) {
        let consumption = cons_max * 0.05 + cons_max * f64::from(throttle) / 100.0;
        self.fuel_to_consume += consumption;
        let whole = self.fuel_to_consume.floor();
        let gas = i64::from(self.gas.mb) - whole as i64;
        self.fuel_to_consume -= whole;
        self.gas.mb = gas.max(0) as u32;
        let mut lube = i64::from(self.lube.mb);
        if self.age.is_multiple_of(10) {
            lube -= 1;
        }
        self.lube.mb = lube.max(0) as u32;
        if gas < 0 || lube < 0 {
            self.state = 0;
        }
        let energy = fuel(&self.gas.fluid).map_or(0, |f| f.1 / 1000) as f64;
        let rpm_eff = f64::from(self.rpm - RPM_IDLE);
        let target = cons_max * energy * rpm_eff / 90.0;
        // The NTM steps by a random share; this takes the middle of it.
        if f64::from(self.instant) < target {
            self.instant = (f64::from(self.instant) + 0.5 * 0.005 * cons_max * energy) as i32;
            if f64::from(self.instant) > target {
                self.instant = target as i32;
            }
        } else if f64::from(self.instant) > target {
            self.instant = (f64::from(self.instant) - 0.5 * 0.011 * cons_max * energy) as i32;
            if f64::from(self.instant) < target {
                self.instant = target as i32;
            }
        }
        self.power = (self.power as i64 + i64::from(self.instant)).max(0) as u64;
        let water_per_tick = cons_max * energy * f64::from(self.temp - TEMP_IDLE) / 220_000.0;
        let heat_cycles = water_per_tick.floor() as i64;
        let cycles = heat_cycles
            .min(i64::from(self.water.mb))
            .min(i64::from(self.steam.free() / 10));
        if cycles > 0 {
            self.water.mb -= cycles as u32;
            self.steam.mb += cycles as u32 * 10;
        }
    }
}

impl Slotted for GasTurbine {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    /// `GUIMachineTurbineGas`: power and cap; the gas, lubricant, water and
    /// dense steam tanks as (mB, cap, colour); state, auto, slider, rpm,
    /// temperature, the power it makes now, the throttle.
    fn screen_data(&self) -> Vec<i32> {
        let mut v = vec![clamp32(self.power), TURBINE_POWER as i32];
        for t in [&self.gas, &self.lube, &self.water, &self.steam] {
            v.extend([
                clamp32(u64::from(t.mb)),
                clamp32(u64::from(t.cap)),
                color(t),
            ]);
        }
        v.extend([
            self.state,
            i32::from(self.auto_mode),
            self.slider,
            self.rpm,
            self.temp,
            self.instant,
            self.throttle,
        ]);
        v
    }
    /// 0: start or stop (as the screen does, only when the dial is at rest);
    /// 1: auto mode (running only); 100 + n: the slider at n, auto off.
    fn button(&mut self, id: u32) -> bool {
        match id {
            actions::IGNITION if self.counter == 0 || self.counter == 579 => self.state -= 1,
            actions::AUTO if self.state == 1 => self.auto_mode = !self.auto_mode,
            n if (actions::SLIDER..actions::SLIDER + 61).contains(&n) => {
                if self.state == 1 {
                    self.auto_mode = false;
                }
                self.slider = (n - actions::SLIDER) as i32;
            }
            _ => {}
        }
        false
    }
}

// --------------------------------------------------------------------- all

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Combustion {
    Diesel(Diesel),
    Engine(Engine),
    Turbofan(Box<Turbofan>),
    Turbine(Box<GasTurbine>),
}

impl Combustion {
    pub fn of(block: &str) -> Option<Self> {
        match block {
            "ntm:machine_diesel" => Some(Self::Diesel(Diesel::default())),
            "ntm:machine_combustion_engine" => Some(Self::Engine(Engine::default())),
            "ntm:machine_turbofan" => Some(Self::Turbofan(Box::default())),
            "ntm:machine_turbinegas" => Some(Self::Turbine(Box::default())),
            _ => None,
        }
    }

    pub fn power(&self) -> u64 {
        match self {
            Self::Diesel(m) => m.power,
            Self::Engine(m) => m.power,
            Self::Turbofan(m) => m.power,
            Self::Turbine(m) => m.power,
        }
    }

    fn set_power(&mut self, power: u64) {
        match self {
            Self::Diesel(m) => m.power = power,
            Self::Engine(m) => m.power = power,
            Self::Turbofan(m) => m.power = power,
            Self::Turbine(m) => m.power = power,
        }
    }

    fn max_power(&self) -> u64 {
        match self {
            Self::Diesel(_) => DIESEL_POWER,
            Self::Engine(_) => ENGINE_POWER,
            Self::Turbofan(_) => FAN_POWER,
            Self::Turbine(_) => TURBINE_POWER,
        }
    }

    /// The tank that takes the fuel, and the other tanks that take theirs.
    fn inputs(&self) -> Vec<&Tank> {
        match self {
            Self::Diesel(m) => vec![&m.tank],
            Self::Engine(m) => vec![&m.tank],
            Self::Turbofan(m) => vec![&m.tank],
            Self::Turbine(m) => vec![&m.gas, &m.lube, &m.water],
        }
    }

    fn inputs_mut(&mut self) -> Vec<&mut Tank> {
        match self {
            Self::Diesel(m) => vec![&mut m.tank],
            Self::Engine(m) => vec![&mut m.tank],
            Self::Turbofan(m) => vec![&mut m.tank],
            Self::Turbine(m) => vec![&mut m.gas, &mut m.lube, &mut m.water],
        }
    }

    /// The tank that gives (the blood and the dense steam; the smoke is not
    /// modelled).
    fn output(&self) -> Option<&Tank> {
        match self {
            Self::Turbofan(m) => Some(&m.blood),
            Self::Turbine(m) => Some(&m.steam),
            _ => None,
        }
    }

    fn output_mut(&mut self) -> Option<&mut Tank> {
        match self {
            Self::Turbofan(m) => Some(&mut m.blood),
            Self::Turbine(m) => Some(&mut m.steam),
            _ => None,
        }
    }

    /// The signal the machine reads (the diesel's block, the turbofan's four
    /// connectors).
    pub fn set_powered(&mut self, powered: bool) {
        match self {
            Self::Diesel(m) => m.powered = powered,
            Self::Turbofan(m) => m.powered = powered,
            _ => {}
        }
    }

    /// Whether the machine reads redstone, and where: cells in the placer's
    /// frame, `[0, 0, 0]` for the block itself.
    pub fn redstone_cells(block: &str) -> &'static [[i32; 3]] {
        match block {
            "ntm:machine_diesel" => &[[0, 0, 0]],
            // `getConPos`: the four connectors.
            "ntm:machine_turbofan" => &[[0, 0, -1], [1, 0, -1], [0, 0, 1], [1, 0, 1]],
            _ => &[],
        }
    }

    pub fn slotted(&mut self) -> &mut dyn Slotted {
        match self {
            Self::Diesel(m) => m,
            Self::Engine(m) => m,
            Self::Turbofan(m) => &mut **m,
            Self::Turbine(m) => &mut **m,
        }
    }

    fn running(&self) -> bool {
        match self {
            Self::Diesel(m) => m.was_on,
            Self::Engine(m) => m.was_on,
            Self::Turbofan(m) => m.was_on,
            Self::Turbine(m) => m.state == 1,
        }
    }
}

/// They offer their buffer to the network.
impl EnergyNode for Combustion {
    fn role(&self) -> Role {
        Role::Provider
    }
    fn offer(&self) -> u64 {
        self.power()
    }
    fn demand(&self) -> u64 {
        0
    }
    fn give(&mut self, _he: u64) {}
    fn take(&mut self, he: u64) {
        self.set_power(self.power().saturating_sub(he));
    }
}

/// The fuel tanks take their own type; the blood and the dense steam leave.
impl FluidNode for Combustion {
    fn fluid_offer(&self, f: &str) -> u64 {
        self.output()
            .filter(|t| t.fluid == f)
            .map_or(0, |t| u64::from(t.mb))
    }
    fn fluid_demand(&self, f: &str) -> u64 {
        self.inputs()
            .iter()
            .filter(|t| t.fluid == f)
            .map(|t| u64::from(t.free()))
            .max()
            .unwrap_or(0)
    }
    fn fluid_types(&self) -> Vec<String> {
        self.inputs()
            .into_iter()
            .chain(self.output())
            .filter(|t| !t.fluid.is_empty())
            .map(|t| t.fluid.clone())
            .collect()
    }
    fn fluid_give(&mut self, f: &str, mb: u64) {
        if let Some(t) = self.inputs_mut().into_iter().find(|t| t.fluid == f) {
            t.fill(f, mb.min(u64::from(u32::MAX)) as u32);
        }
    }
    fn fluid_take(&mut self, f: &str, mb: u64) {
        if let Some(t) = self.output_mut().filter(|t| t.fluid == f) {
            t.drain(mb.min(u64::from(u32::MAX)) as u32);
        }
    }
}

impl Logic for Combustion {
    fn tick(&mut self) -> bool {
        let before = (
            self.power(),
            self.running(),
            self.inputs().iter().map(|t| t.mb).sum::<u32>(),
        );
        match self {
            Self::Diesel(m) => m.step(),
            Self::Engine(m) => m.step(),
            Self::Turbofan(m) => m.step(),
            Self::Turbine(m) => m.step(),
        }
        // The buffer is capped after the tick in every machine; the turbine
        // keeps running states the screen shows.
        let max = self.max_power();
        if self.power() > max {
            self.set_power(max);
        }
        before
            != (
                self.power(),
                self.running(),
                self.inputs().iter().map(|t| t.mb).sum::<u32>(),
            )
            || matches!(self, Self::Turbine(t) if t.state != 0 || t.rpm > 0)
    }
    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        Some(self)
    }
    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(self)
    }
    fn level(&self, max: u8) -> u8 {
        if self.running() { max } else { 0 }
    }
    fn view(&self) -> ViewModel {
        let vm = match self {
            Self::Diesel(m) => ViewModel::new("Gerador a diesel")
                .text(format!("{} HE/t", diesel_he(&m.tank.fluid)), view::GREY),
            Self::Engine(m) => ViewModel::new("Motor a combustão").text(
                format!("{} HE/t, ajuste {}", m.display_he(), m.setting),
                view::GREY,
            ),
            Self::Turbofan(m) => ViewModel::new("Turbofan").text(
                format!("{} HE/t, pós-combustão {}", m.output, m.afterburner),
                view::GREY,
            ),
            Self::Turbine(m) => ViewModel::new("Turbina a gás").text(
                format!("{} rpm, {} °C, {} HE/t", m.rpm, m.temp, m.instant),
                view::GREY,
            ),
        };
        let vm = vm.bar("Energia", self.power(), self.max_power(), "HE");
        self.inputs()
            .into_iter()
            .chain(self.output())
            .fold(vm, |vm, t| vm.text(t.line(), view::GREY))
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    fn stack(id: &str, n: u32) -> Option<Stack> {
        Some(Stack::new(id, n))
    }

    /// 1 370 000 HE a bucket: 1 370 a mB, three quarters of it, one mB a tick;
    /// the buffer stops at its cap.
    #[test]
    fn the_diesel_divides_the_bucket_before_the_efficiency() {
        assert_eq!(diesel_he("diesel"), 1_027);
        assert_eq!(diesel_he("kerosene"), 385);
        assert_eq!(diesel_he("gas"), 0, "GAS has no efficiency");
        assert_eq!(diesel_he("water"), 0);
        let mut d = Diesel::default();
        d.tank.fill("diesel", 100);
        d.step();
        assert_eq!((d.power, d.tank.mb), (0, 100), "off: nothing");
        d.button(actions::IGNITION);
        d.step();
        assert_eq!((d.power, d.tank.mb, d.was_on), (1_027, 99, true));
        d.powered = true;
        d.step();
        assert_eq!(
            (d.power, d.tank.mb, d.was_on),
            (1_027, 99, false),
            "redstone stops it"
        );
        d.powered = false;
        d.power = 49_500;
        d.step();
        assert_eq!(d.power, 50_000, "up to the cap, no more");
    }

    #[test]
    fn the_diesel_takes_a_full_container_and_the_identifier() {
        let mut d = Diesel::default();
        d.slots = vec![stack("ntm:canister_full__diesel", 2), None, None, None];
        d.step();
        assert_eq!(d.tank.mb, 1_000);
        assert_eq!(d.slots[0].as_ref().map(|s| s.count), Some(1));
        assert_eq!(
            d.slots[1].as_ref().map(|s| (s.id.as_str(), s.count)),
            Some(("ntm:canister_empty", 1))
        );
        let mut id = Stack::new(IDENTIFIER, 1);
        id.custom = Some("{ntm_fluid:\"kerosene\"}".into());
        d.slots[3] = Some(id);
        d.step();
        assert_eq!(
            (d.tank.fluid.as_str(), d.tank.mb),
            ("kerosene", 0),
            "a new type empties the tank"
        );
    }

    /// 20 tenths a tick at setting 10: 2 mB of diesel, `137.0 · 0.25` HE each
    /// tenth with a steel piston.
    #[test]
    fn the_engine_burns_tenths_for_a_double_added_to_a_long() {
        let mut e = Engine::default();
        e.tank.fill("diesel", 10);
        e.slots = vec![None, None, stack("ntm:piston_set__steel", 1), None, None];
        e.on = true;
        e.setting = 10;
        e.step();
        assert_eq!((e.power, e.tank.mb, e.tenth, e.was_on), (685, 8, 0, true));
        e.setting = 1;
        e.step();
        // 2 tenths: 2 · 137 · 0.25 = 68.5 on a long: 753.5 cut.
        assert_eq!((e.power, e.tank.mb, e.tenth), (753, 7, 8));
        e.setting = 0;
        e.step();
        assert_eq!(e.power, 753, "setting 0 burns nothing");
        e.setting = 5;
        e.slots[2] = stack("ntm:piston_set__desh", 1);
        e.step();
        // DESH on HIGH: 1.0; 10 tenths of the 78: 1 370.
        assert_eq!((e.power, e.tank.mb, e.tenth), (753 + 1_370, 6, 8));
        e.slots[2] = stack("ntm:piston_set__steel", 1);
        e.tank.set_type("kerosene");
        e.tank.fill("kerosene", 5);
        e.step();
        assert_eq!(e.tank.mb, 5, "steel burns no AERO: nothing burns");
        e.slots[2] = None;
        e.power = ENGINE_POWER - 1;
        e.tank.set_type("diesel");
        e.tank.fill("diesel", 5);
        e.step();
        assert_eq!(e.power, ENGINE_POWER - 1, "no piston, no burn");
    }

    #[test]
    fn the_engine_screen_shows_its_piston_and_its_he() {
        let mut e = Engine::default();
        e.slots = vec![None, None, stack("ntm:piston_set__dura", 1), None, None];
        e.setting = 30;
        // 30 · 0.2 · 1 370 · 0.9 = 7 398.
        assert_eq!(e.screen_data()[7..], [2, 7_398]);
        e.button(actions::SLIDER + 12);
        assert_eq!(e.setting, 12);
        e.button(actions::IGNITION);
        assert!(e.on);
    }

    /// 3 850 HE a mB; the afterburner burns `1 + level` mB for
    /// `1 + min(level / 3, 4)` times it; the flame pony's 100 doesn't wrap
    /// the double, which the cast then clamps to an int.
    #[test]
    fn the_turbofan_burns_aero_fuel_and_casts_to_an_int() {
        let mut t = Turbofan::default();
        t.tank.fill("kerosene", 1_000);
        t.step();
        assert_eq!((t.output, t.power, t.tank.mb), (3_850, 3_850, 999));
        t.slots = vec![None, None, stack("ntm:upgrade_afterburn_3", 1), None, None];
        t.step();
        assert_eq!((t.afterburner, t.output, t.tank.mb), (3, 30_800, 995));
        t.slots[2] = stack(FLAME_PONY, 1);
        t.step();
        assert_eq!(t.afterburner, 100);
        assert_eq!(t.output, (3_850.0 * 101.0 * 5.0) as i32);
        t.powered = true;
        let (power, fuel) = (t.power, t.tank.mb);
        t.step();
        assert_eq!(
            (t.power, t.tank.mb, t.was_on),
            (power, fuel, false),
            "redstone stops it"
        );
        let mut t = Turbofan::default();
        t.tank.set_type("diesel");
        t.tank.fill("diesel", 100);
        t.step();
        assert_eq!(
            (t.power, t.tank.mb, t.was_on),
            (0, 99, true),
            "other grades burn for nothing"
        );
        let mut t = Turbofan::default();
        t.tank.fill("kerosene", 24_000);
        t.slots = vec![None, None, stack(FLAME_PONY, 1), None, None];
        for _ in 0..300 {
            t.step();
        }
        assert!(t.power <= FAN_POWER);
    }

    fn ready_turbine() -> GasTurbine {
        let mut t = GasTurbine::default();
        t.gas.fill("gas", 100_000);
        t.lube.fill("lubricant", 16_000);
        t.water.fill("water", 16_000);
        t
    }

    /// 580 ticks to start: the dial sweeps, then both climb to idle.
    #[test]
    fn the_gas_turbine_starts_in_580_ticks_and_stops_without_fuel_or_lubricant() {
        let mut t = ready_turbine();
        t.button(actions::IGNITION);
        assert_eq!(t.state, -1);
        t.step();
        assert_eq!((t.counter, t.rpm), (1, 5));
        for _ in 0..39 {
            t.step();
        }
        assert_eq!((t.counter, t.rpm), (40, 0));
        for _ in 0..540 {
            t.step();
        }
        assert_eq!((t.state, t.counter, t.rpm, t.temp), (1, 225, 10, 300));
        let mut dry = ready_turbine();
        dry.lube.mb = 0;
        dry.state = -1;
        dry.step();
        assert_eq!(dry.state, 0);
        let mut wrong = ready_turbine();
        wrong.gas.set_type("diesel");
        wrong.gas.fill("diesel", 100);
        wrong.state = 1;
        wrong.step();
        assert_eq!(wrong.state, 0, "a non-GAS fuel never runs");
    }

    /// At full throttle on natural gas: 50 mB at the top, the idle 5 % a tick
    /// below; 75 HE a mB over the rotor's speed; the heat boils the water.
    #[test]
    fn the_gas_turbine_makes_power_and_steam_by_its_rotor_and_its_heat() {
        let mut t = ready_turbine();
        t.state = 1;
        t.rpm = 10;
        t.temp = 300;
        t.slider = 60;
        for _ in 0..800 {
            t.step();
        }
        assert_eq!(t.throttle, 100);
        assert_eq!(t.rpm, 100);
        assert_eq!(
            t.temp - TEMP_IDLE,
            100 * 5 * (burn_temp("gas") - TEMP_IDLE) / 500
        );
        assert_eq!(burn_temp("gas"), 658);
        // 52.5 mB a tick at full throttle (the idle share and the throttle): whole
        // mB each tick, the rest kept; fewer before the rotor is up to speed.
        assert!((50_000..65_000).contains(&t.gas.mb), "{}", t.gas.mb);
        assert!(t.instant > 0 && t.instant <= 50 * 75, "{}", t.instant);
        assert!(t.steam.mb > 0 && t.steam.mb.is_multiple_of(10));
        assert!(t.lube.mb < 16_000 && t.lube.mb >= 16_000 - 200);
        assert_eq!(t.power, TURBINE_POWER, "the buffer is capped");
        // The idle share alone burns 2.5 mB: whole mBs, the half kept.
        let mut idle = ready_turbine();
        idle.state = 1;
        idle.rpm = 10;
        idle.temp = 300;
        idle.step();
        idle.step();
        assert_eq!(idle.gas.mb, 100_000 - 5);
    }

    #[test]
    fn the_gas_turbine_runs_down_over_225_ticks_and_its_controls() {
        let mut t = ready_turbine();
        t.state = 1;
        t.rpm = 50;
        t.temp = 400;
        t.slider = 30;
        t.auto_mode = true;
        t.button(actions::SLIDER + 20);
        assert_eq!((t.slider, t.auto_mode), (20, false));
        t.button(actions::AUTO);
        assert!(t.auto_mode);
        t.button(actions::IGNITION);
        assert_eq!(t.state, 0, "online to offline");
        for _ in 0..60 {
            t.step();
        }
        assert!(t.rpm <= 10 && t.counter < 225 && !t.auto_mode);
        for _ in 0..400 {
            t.step();
        }
        assert_eq!((t.rpm, t.temp, t.slider), (0, 0, 0));
        // The screen's data: 21 values.
        assert_eq!(t.screen_data().len(), 21);
        // The identifier takes only a gas.
        let mut id = Stack::new(IDENTIFIER, 1);
        id.custom = Some("{ntm_fluid:\"syngas\"}".into());
        t.slots[1] = Some(id.clone());
        t.step();
        assert_eq!(t.gas.fluid, "syngas");
        id.custom = Some("{ntm_fluid:\"diesel\"}".into());
        t.slots[1] = Some(id);
        t.step();
        assert_eq!(t.gas.fluid, "syngas", "not a GAS grade");
    }

    #[test]
    fn they_offer_their_buffer_and_take_fuel_by_type() {
        let mut m = Combustion::of("ntm:machine_diesel").unwrap();
        m.fluid_give("diesel", 500);
        assert_eq!(
            (
                m.fluid_demand("diesel"),
                m.fluid_demand("kerosene"),
                m.fluid_offer("diesel")
            ),
            (15_500, 0, 0)
        );
        if let Combustion::Diesel(d) = &mut m {
            d.power = 400;
        }
        assert_eq!((m.role(), m.offer()), (Role::Provider, 400));
        m.take(150);
        assert_eq!(m.offer(), 250);
        let mut t = Combustion::of("ntm:machine_turbinegas").unwrap();
        t.fluid_give("water", 100);
        t.fluid_give("hotsteam", 100);
        assert_eq!(
            (t.fluid_offer("hotsteam"), t.fluid_demand("water")),
            (0, 15_900),
            "the steam only leaves"
        );
        assert!(Combustion::of("ntm:machine_press").is_none());
    }
}
