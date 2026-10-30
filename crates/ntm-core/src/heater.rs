//! The heaters of energy batch 1 (docs/maquinas.md, "Energia, lote 1"):
//! `tileentity/machine/TileEntityHeaterOilburner.java`,
//! `TileEntityHeaterElectric.java` and `TileEntityHeaterHeatex.java`. The
//! heating oven is a firebox with its own fuels and cap
//! (`crate::foundry::firebox`, `oven`).
//!
//! Each stores heat (TU) for the machine above it: boilers, the coker, the
//! crucible, or another heater (`server/oil_server.rs`). Their tables come
//! from the content agent's `heaters.json` and `fluid_traits.json`
//! (`tools/machine-gen/build.py`). Left out: the oil burner's smoke and
//! pollution; the fluid identifiers (their slots are there, unread: the oil
//! burner takes the first flammable fluid while empty, the heat exchanger
//! the first coolable one); the electric heater's hum. The NTM's screwdriver
//! isn't in the content: the electric heater's setting is a button.

use crate::{
    energy::{EnergyNode, Role},
    fluid::{FluidNode, Tank},
    foundry::{Fuel, HeatSource},
    machine::{Effect, Logic, fill_level},
    proto::Held,
    server::container_bridge::{Slots, Slotted, Stack},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::OnceLock};

/// A coolable fluid (`FT_Coolable`): `in_mb` of it into `out_mb` of `out`,
/// `heat` TU an operation times the device's efficiency.
#[derive(Deserialize)]
pub struct Coolable {
    pub out: String,
    pub out_mb: u32,
    pub in_mb: u32,
    pub heat: u32,
    pub heatex: f64,
    pub turbine: f64,
}

#[derive(Deserialize)]
pub struct Tables {
    pub oven_fuels: HashMap<String, Fuel>,
    /// TU a bucket (`FT_Flammable.getHeatEnergy`).
    pub flammable: HashMap<String, u64>,
    pub coolable: HashMap<String, Coolable>,
}

pub fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(include_str!("../data/machines/heaters.json")).expect("heaters.json"))
}

pub fn flammable(fluid: &str) -> Option<u64> {
    tables().flammable.get(fluid).copied()
}

/// A fluid the heat exchanger cools (a positive `HEATEXCHANGER` efficiency).
pub fn heatex_coolable(fluid: &str) -> Option<&'static Coolable> {
    tables().coolable.get(fluid).filter(|c| c.heatex > 0.0)
}

pub mod actions {
    pub const TOGGLE: u32 = 1;
    pub const LESS: u32 = 2;
    pub const MORE: u32 = 3;
    pub const DELAY_LESS: u32 = 4;
    pub const DELAY_MORE: u32 = 5;
}

// ------------------------------------------------------------- oil burner

pub const OILBURNER_HEAT: u32 = 500_000;
pub const OILBURNER_RATE: u32 = 10;

/// `ContainerOilburner`: a full container in, the empty out, the
/// identifier, an overdrive upgrade.
pub mod oil_slot {
    pub const IN: usize = 0;
    pub const OUT: usize = 1;
    pub const UPGRADE: usize = 3;
    pub const COUNT: usize = 4;
}

fn oil_slots() -> Slots {
    vec![None; oil_slot::COUNT]
}

fn tank_values(t: &Tank) -> [i32; 3] {
    [t.mb as i32, t.cap as i32, crate::fluid::get(&t.fluid).map_or(0, |f| f.color as i32)]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Oilburner {
    #[serde(skip, default = "oil_slots")]
    pub slots: Slots,
    pub tank: Tank,
    pub on: bool,
    /// mB a tick (`burnRate`, 1..=10).
    pub rate: u32,
    pub heat: u32,
    #[serde(skip)]
    pub burning: bool,
}

impl Default for Oilburner {
    fn default() -> Self {
        Self { slots: oil_slots(), tank: Tank::typed("heatingoil", 16_000), on: false, rate: 1, heat: 0, burning: false }
    }
}

impl Oilburner {
    /// The overdrive upgrade's level (0..=3).
    pub fn overdrive(&self) -> u32 {
        match self.slots.get(oil_slot::UPGRADE).and_then(|s| s.as_ref()).and_then(|s| crate::oil_drill::upgrade(&s.id)) {
            Some(("overdrive", t)) => t,
            _ => 0,
        }
    }

    pub fn max_rate(&self) -> u32 {
        OILBURNER_RATE * (self.overdrive() + 1)
    }

    pub fn max_heat(&self) -> u32 {
        OILBURNER_HEAT * (self.overdrive() + 1)
    }

    /// `loadTank(0, 1)`: a full container of a fluid it takes empties into
    /// the tank when it fits, the empty one to the output.
    fn load(&mut self) -> bool {
        let Some(c) = self.slots[oil_slot::IN].as_ref().and_then(|s| crate::fluid::contents(&s.id)) else { return false };
        let takes = self.tank.fluid == c.fluid || (self.tank.mb == 0 && flammable(&c.fluid).is_some());
        let out_ok = match &self.slots[oil_slot::OUT] {
            None => true,
            Some(o) => o.id == c.empty && o.count < 64,
        };
        if !takes || !out_ok || self.tank.free() < c.mb {
            return false;
        }
        if self.tank.mb == 0 {
            self.tank.set_type(&c.fluid);
        }
        self.tank.fill(&c.fluid, c.mb);
        if let Some(s) = &mut self.slots[oil_slot::IN] {
            s.count -= 1;
            if s.count == 0 {
                self.slots[oil_slot::IN] = None;
            }
        }
        match &mut self.slots[oil_slot::OUT] {
            Some(o) => o.count += 1,
            None => self.slots[oil_slot::OUT] = Some(Stack::new(&c.empty, 1)),
        }
        true
    }

    /// TU a tick at its rate, burning.
    pub fn heat_rate(&self) -> u32 {
        flammable(&self.tank.fluid).map_or(0, |tu| (tu / 1000) as u32 * self.rate)
    }

    /// `updateEntity`: while on and below its cap, a flammable fluid burns
    /// `rate` mB into a thousandth of its TU a bucket each (the division
    /// first, as the NTM's int); it may pass the cap this tick, clamped at
    /// the next. Off, out of fuel or not flammable, it cools by a thousandth.
    /// Overdrive multiplies the rate and the cap. True when its items moved.
    pub fn step(&mut self) -> bool {
        let moved = self.load();
        let (max_rate, max_heat) = (self.max_rate(), self.max_heat());
        self.rate = self.rate.clamp(1, max_rate);
        self.heat = self.heat.min(max_heat);
        let mut cool = true;
        self.burning = false;
        if self.on
            && self.heat < max_heat
            && let Some(tu) = flammable(&self.tank.fluid)
        {
            let burn = self.rate.min(self.tank.mb);
            self.tank.drain(burn);
            self.heat += (tu / 1000) as u32 * burn;
            self.burning = burn > 0;
            cool = false;
        }
        if self.heat >= max_heat {
            cool = false;
        }
        if cool {
            self.heat = self.heat.saturating_sub((self.heat / 1000).max(1));
        }
        moved
    }

    /// The screen's values: [0]/[1] heat, [2..4] the tank, [5] on, [6]
    /// flame, [7]/[8] rate and its maximum, [9] the slider (0..=32), [10]
    /// TU a tick.
    pub fn screen_data(&self) -> Vec<i32> {
        let max = self.max_rate();
        let mut v = vec![self.heat as i32, self.max_heat() as i32];
        v.extend(tank_values(&self.tank));
        let slider = (self.rate.saturating_sub(1) * 32).checked_div(max - 1).unwrap_or(0);
        v.extend([i32::from(self.on), i32::from(self.burning), self.rate as i32, max as i32, slider as i32, self.heat_rate() as i32]);
        v
    }

    /// 0 toggles; 100 + k is a click on pixel k of the slider
    /// (`GUIOilburner`): `k · max / 32`, at least 1.
    pub fn button(&mut self, id: u32) {
        match id {
            0 => self.on = !self.on,
            100..=135 => {
                let max = self.max_rate();
                self.rate = ((id - 100) * max / 32).clamp(1, max);
            }
            _ => {}
        }
    }
}

impl FluidNode for Oilburner {
    fn fluid_offer(&self, _f: &str) -> u64 {
        0
    }
    fn fluid_demand(&self, f: &str) -> u64 {
        if (self.tank.fluid == f || self.tank.mb == 0) && flammable(f).is_some() { u64::from(self.tank.free()) } else { 0 }
    }
    fn fluid_types(&self) -> Vec<String> {
        [&self.tank].into_iter().filter(|t| !t.fluid.is_empty()).map(|t| t.fluid.clone()).collect()
    }
    fn accepts_any(&self) -> bool {
        self.tank.mb == 0
    }
    fn fluid_give(&mut self, f: &str, mb: u64) {
        if self.tank.mb == 0 {
            self.tank.set_type(f);
        }
        self.tank.fill(f, mb.min(u64::from(u32::MAX)) as u32);
    }
    fn fluid_take(&mut self, _f: &str, _mb: u64) {}
}

// ------------------------------------------------------- electric heater

pub const ELECTRIC_SETTING: u8 = 10;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Electric {
    pub power: u64,
    pub heat: u32,
    /// 0..=10 (the screwdriver's `toggleSetting`).
    pub setting: u8,
    #[serde(skip)]
    pub on: bool,
}

impl Electric {
    /// `getConsumption`: `setting^1.4 · 200` HE a tick.
    pub fn consumption(&self) -> u64 {
        (f64::from(self.setting).powf(1.4) * 200.0) as u64
    }

    pub fn max_power(&self) -> u64 {
        self.consumption() * 20
    }

    /// The first half of `updateEntity`: it loses a thousandth (rounded
    /// down) and takes all the heat under it at 85 %. Returns the heat taken.
    pub fn pull_heat(&mut self, source: Option<u32>) -> u32 {
        self.heat = (f64::from(self.heat) * 0.999) as u32;
        let Some(s) = source else { return 0 };
        self.heat = (f64::from(self.heat) + f64::from(s) * 0.85) as u32;
        s
    }

    /// `toggleSetting`: 0..=10, round.
    pub fn toggle(&mut self) {
        self.setting = if self.setting >= ELECTRIC_SETTING { 0 } else { self.setting + 1 };
    }

    /// The rest: with a setting and the power, `setting · 100` TU.
    pub fn step(&mut self) -> bool {
        let was = self.on;
        self.on = false;
        let cost = self.consumption();
        if self.setting > 0 && self.power >= cost {
            self.power -= cost;
            self.heat = self.heat.saturating_add(u32::from(self.setting) * 100);
            self.on = true;
        }
        self.on || was
    }
}

impl EnergyNode for Electric {
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

// ---------------------------------------------------------- heat exchanger

pub const HEATEX_TANK: u32 = 24_000;

/// `ContainerHeaterHeatex`: the identifier.
fn heatex_slots() -> Slots {
    vec![None; 1]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Heatex {
    #[serde(skip, default = "heatex_slots")]
    pub slots: Slots,
    pub input: Tank,
    pub output: Tank,
    /// Operations at most a conversion (`amountToCool`).
    pub amount: u32,
    /// Ticks between conversions (`tickDelay`).
    pub delay: u32,
    pub heat: u32,
    /// Its own clock for the delay (the NTM reads the world's).
    pub age: u64,
    #[serde(skip)]
    pub on: bool,
}

impl Default for Heatex {
    fn default() -> Self {
        Self { slots: heatex_slots(), input: Tank::typed("coolant_hot", HEATEX_TANK), output: Tank::typed("coolant", HEATEX_TANK), amount: HEATEX_TANK, delay: 1, heat: 0, age: 0, on: false }
    }
}

impl Heatex {
    /// `setupTanks`, the cooling by a thousandth and `tryConvert`.
    pub fn step(&mut self) -> bool {
        let Some(c) = heatex_coolable(&self.input.fluid) else {
            self.input.set_type("");
            self.output.set_type("");
            self.heat = (f64::from(self.heat) * 0.999) as u32;
            return false;
        };
        self.output.set_type(&c.out);
        self.heat = (f64::from(self.heat) * 0.999) as u32;
        self.delay = self.delay.max(1);
        self.age += 1;
        self.on = false;
        if !self.age.is_multiple_of(u64::from(self.delay)) {
            return false;
        }
        let ops = (self.input.mb / c.in_mb).min(self.output.free() / c.out_mb).min(self.amount);
        self.input.drain(c.in_mb * ops);
        let out = c.out.clone();
        self.output.fill(&out, c.out_mb * ops);
        self.heat = (f64::from(self.heat) + f64::from(c.heat) * f64::from(ops) * c.heatex) as u32;
        self.on = ops > 0;
        self.on
    }

    /// The screen's values: [0..2] the input, [3..5] the output, [6] the
    /// operations a conversion, [7] the delay, [8] heat.
    pub fn screen_data(&self) -> Vec<i32> {
        let mut v = tank_values(&self.input).to_vec();
        v.extend(tank_values(&self.output));
        v.extend([self.amount as i32, self.delay as i32, self.heat as i32]);
        v
    }

    /// The NTM's text fields as steps: operations 0 +1000, 1 +1, 2 −1000,
    /// 3 −1; delay 4 +1, 5 +20, 6 −1, 7 −20. Both at least 1, the
    /// operations at most the tank (`receiveControl`).
    pub fn button(&mut self, id: u32) {
        let step = |v: u32, d: i64, max: u32| (i64::from(v) + d).clamp(1, i64::from(max)) as u32;
        match id {
            0..=3 => self.amount = step(self.amount, [1000, 1, -1000, -1][id as usize], self.input.cap),
            4..=7 => self.delay = step(self.delay, [1, 20, -1, -20][id as usize - 4], u32::MAX),
            _ => {}
        }
    }
}

impl FluidNode for Heatex {
    fn fluid_offer(&self, f: &str) -> u64 {
        if self.output.fluid == f { u64::from(self.output.mb) } else { 0 }
    }
    fn fluid_demand(&self, f: &str) -> u64 {
        if (self.input.fluid == f || self.input.mb == 0) && heatex_coolable(f).is_some() { u64::from(self.input.free()) } else { 0 }
    }
    fn fluid_types(&self) -> Vec<String> {
        [&self.input, &self.output].into_iter().filter(|t| !t.fluid.is_empty()).map(|t| t.fluid.clone()).collect()
    }
    fn accepts_any(&self) -> bool {
        self.input.mb == 0
    }
    fn fluid_give(&mut self, f: &str, mb: u64) {
        if self.input.mb == 0 {
            self.input.set_type(f);
        }
        self.input.fill(f, mb.min(u64::from(u32::MAX)) as u32);
    }
    fn fluid_take(&mut self, f: &str, mb: u64) {
        if self.output.fluid == f {
            self.output.drain(mb.min(u64::from(u32::MAX)) as u32);
        }
    }
}

// ------------------------------------------------------------------ all

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Heater {
    Oilburner(Oilburner),
    Electric(Electric),
    Heatex(Heatex),
}

impl Heater {
    pub fn of(block: &str) -> Option<Self> {
        match block {
            "ntm:heater_oilburner" => Some(Self::Oilburner(Oilburner::default())),
            "ntm:heater_electric" => Some(Self::Electric(Electric::default())),
            "ntm:heater_heatex" => Some(Self::Heatex(Heatex::default())),
            _ => None,
        }
    }

    /// How many blocks high it stands: what sits on it has its core that
    /// much above (the oil burner's top centre is its heat proxy).
    pub fn height(&self) -> i32 {
        if matches!(self, Self::Oilburner(_)) { 2 } else { 1 }
    }

    fn heat(&self) -> u32 {
        match self {
            Self::Oilburner(h) => h.heat,
            Self::Electric(h) => h.heat,
            Self::Heatex(h) => h.heat,
        }
    }

    fn heat_mut(&mut self) -> &mut u32 {
        match self {
            Self::Oilburner(h) => &mut h.heat,
            Self::Electric(h) => &mut h.heat,
            Self::Heatex(h) => &mut h.heat,
        }
    }
}

impl Heater {
    /// The heaters with the engine's slots and a screen of their own.
    pub fn slotted(&mut self) -> Option<&mut dyn Slotted> {
        match self {
            Self::Oilburner(h) => Some(h),
            Self::Heatex(h) => Some(h),
            Self::Electric(_) => None,
        }
    }
}

impl Slotted for Oilburner {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        Oilburner::screen_data(self)
    }
    fn button(&mut self, id: u32) -> bool {
        Oilburner::button(self, id);
        false
    }
    /// The tank, which the containers in its slots fill.
    fn save_state(&self) -> Vec<i32> {
        vec![self.tank.mb as i32]
    }
    fn load_state(&mut self, v: &[i32]) {
        if let [mb, ..] = v {
            self.tank.mb = (*mb).clamp(0, self.tank.cap as i32) as u32;
        }
    }
}

impl Slotted for Heatex {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        Heatex::screen_data(self)
    }
    fn button(&mut self, id: u32) -> bool {
        Heatex::button(self, id);
        false
    }
}

impl HeatSource for Heater {
    fn heat_stored(&self) -> u32 {
        self.heat()
    }
    fn use_heat(&mut self, heat: u32) {
        let h = self.heat_mut();
        *h = h.saturating_sub(heat);
    }
}

impl Logic for Heater {
    /// The electric heater's pull and cooling are the server's first
    /// (`heat_tick`: it reads the block under).
    fn tick(&mut self) -> bool {
        match self {
            Self::Oilburner(h) => {
                let was = h.burning;
                h.step() || was || h.burning
            }
            Self::Electric(h) => h.step(),
            Self::Heatex(h) => {
                let was = h.on;
                h.step() || was
            }
        }
    }
    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        match self {
            Self::Electric(h) => Some(h),
            _ => None,
        }
    }
    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        match self {
            Self::Oilburner(h) => Some(h),
            Self::Heatex(h) => Some(h),
            Self::Electric(_) => None,
        }
    }
    fn level(&self, max: u8) -> u8 {
        let (on, heat, cap) = match self {
            Self::Oilburner(h) => (h.burning, h.heat, h.max_heat()),
            Self::Electric(h) => (h.on, h.heat, 100_000),
            Self::Heatex(h) => (h.on, h.heat, 100_000),
        };
        if on { fill_level(u64::from(heat.max(1)).min(u64::from(cap)), u64::from(cap), max) } else { 0 }
    }
    fn view(&self) -> ViewModel {
        use actions::*;
        match self {
            Self::Oilburner(h) => ViewModel::new("Queimador de óleo")
                .bar("Calor", u64::from(h.heat), u64::from(h.max_heat()), "TU")
                .text(h.tank.line(), view::GREY)
                .text(format!("{} mB/t, {}", h.rate, if h.on { "ligado" } else { "desligado" }), view::GREY)
                .button(TOGGLE, if h.on { "Desligar" } else { "Ligar" })
                .button(LESS, "− mB/t")
                .button(MORE, "+ mB/t"),
            Self::Electric(h) => ViewModel::new("Aquecedor elétrico")
                .bar("Energia", h.power, h.max_power(), "HE")
                .text(format!("Calor: {} TU", h.heat), view::GREY)
                .text(format!("Potência {} de {ELECTRIC_SETTING}: {} HE/t → {} TU/t", h.setting, h.consumption(), u32::from(h.setting) * 100), view::GREY)
                .button(TOGGLE, "Trocar potência (chave de fenda)"),
            Self::Heatex(h) => ViewModel::new("Trocador de calor")
                .text(format!("Calor: {} TU", h.heat), view::GREY)
                .text(h.input.line(), view::GREY)
                .text(h.output.line(), view::GREY)
                .text(format!("{} operações a cada {} t", h.amount, h.delay), view::GREY)
                .button(LESS, "− 1000 operações")
                .button(MORE, "+ 1000 operações")
                .button(DELAY_LESS, "− 1 t")
                .button(DELAY_MORE, "+ 1 t"),
        }
    }
    fn action(&mut self, action: u32, _held: Option<&Held>, _fx: &mut Vec<Effect>) {
        use actions::*;
        match (self, action) {
            (Self::Oilburner(h), TOGGLE) => h.button(0),
            (Self::Oilburner(h), LESS) => h.rate = h.rate.saturating_sub(1).max(1),
            (Self::Oilburner(h), MORE) => h.rate = (h.rate + 1).min(h.max_rate()),
            (Self::Electric(h), TOGGLE) => h.toggle(),
            // `receiveControl`: 1..=the tank's capacity, a delay of 1 or more.
            (Self::Heatex(h), LESS) => h.button(2),
            (Self::Heatex(h), MORE) => h.button(0),
            (Self::Heatex(h), DELAY_LESS) => h.button(6),
            (Self::Heatex(h), DELAY_MORE) => h.button(4),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A thousandth of the fluid's TU a bucket for each mB, `rate` mB a
    /// tick, only while on.
    #[test]
    fn the_oil_burner_burns_its_rate_into_a_thousandth_of_the_bucket() {
        let tu = flammable("heatingoil").expect("óleo de aquecimento inflamável");
        let mut b = Oilburner::default();
        b.fluid_give("heatingoil", 1_000);
        b.step();
        assert_eq!((b.heat, b.tank.mb), (0, 1_000), "desligado: não queima");
        b.on = true;
        b.rate = 4;
        b.step();
        assert_eq!((b.heat, b.tank.mb), ((tu / 1000) as u32 * 4, 996));
        // Near its cap it passes it this tick and is clamped at the next.
        b.heat = OILBURNER_HEAT - 1;
        b.step();
        assert!(b.heat > OILBURNER_HEAT);
        b.step();
        assert_eq!((b.heat, b.tank.mb), (OILBURNER_HEAT, 992), "no teto: não queima nem esfria");
        // Off, it cools by a thousandth, but not at its cap.
        b.on = false;
        b.step();
        assert_eq!(b.heat, OILBURNER_HEAT);
        b.heat = 100_000;
        b.step();
        assert_eq!(b.heat, 99_900);
        assert_eq!(b.fluid_demand("water"), 0);
    }

    /// Overdrive 2: three times the rate and the cap; the slider's pixel k
    /// sets `k · max / 32`; a full tank empties into the tank.
    #[test]
    fn the_oil_burner_reads_its_overdrive_slider_and_containers() {
        let mut b = Oilburner::default();
        b.slots[oil_slot::UPGRADE] = Some(Stack::new("ntm:upgrade_overdrive_2", 1));
        assert_eq!((b.max_rate(), b.max_heat()), (30, 1_500_000));
        b.button(100 + 16);
        assert_eq!(b.rate, 15);
        b.button(100);
        assert_eq!(b.rate, 1, "no mínimo 1");
        b.button(100 + 32);
        assert_eq!(b.screen_data()[7..10], [30, 30, 32]);
        b.tank.set_type("");
        b.slots[oil_slot::IN] = Some(Stack::new("ntm:fluid_tank_full__diesel", 2));
        assert!(b.step());
        assert_eq!((b.tank.fluid.as_str(), b.tank.mb), ("diesel", 1_000));
        assert_eq!(b.slots[oil_slot::OUT], Some(Stack::new("ntm:fluid_tank_empty", 1)));
        b.button(0);
        b.step();
        assert_eq!(b.tank.mb, 2_000 - 30, "o segundo tanque entrou e 30 mB queimaram");
        assert!(b.screen_data()[6] == 1 && b.screen_data()[10] > 0);
    }

    /// An empty burner takes the first flammable fluid, then only it.
    #[test]
    fn the_oil_burner_takes_any_flammable_fluid_while_empty() {
        let mut b = Oilburner::default();
        assert!(b.fluid_demand("diesel") > 0);
        b.fluid_give("diesel", 500);
        assert_eq!(b.tank.fluid, "diesel");
        assert_eq!(b.fluid_demand("heatingoil"), 0);
    }

    /// `setting^1.4 · 200` HE into `setting · 100` TU; it loses a thousandth
    /// and takes all the heat under it at 85 %.
    #[test]
    fn the_electric_heater_turns_power_into_heat_by_its_setting() {
        let mut e = Electric::default();
        assert_eq!((e.consumption(), e.demand()), (0, 0));
        for _ in 0..10 {
            e.toggle();
        }
        assert_eq!((e.setting, e.consumption(), e.max_power()), (10, 5023, 100_460));
        e.give(e.demand());
        e.step();
        assert_eq!((e.heat, e.power), (1_000, 100_460 - 5023));
        assert_eq!(e.pull_heat(Some(2_000)), 2_000);
        assert_eq!(e.heat, 999 + 1_700);
        e.toggle();
        assert_eq!(e.setting, 0, "volta a zero depois de 10");
        e.step();
        assert!(!e.on);
    }

    /// Hot coolant: 1 mB into 1 of coolant, 300 TU at efficiency 1; steam
    /// at 0.5, 100 mB into 1 of spent steam.
    #[test]
    fn the_heat_exchanger_cools_by_its_operations() {
        let mut h = Heatex::default();
        h.fluid_give("coolant_hot", 1_000);
        h.amount = 100;
        h.step();
        assert_eq!((h.input.mb, h.output.fluid.as_str(), h.output.mb, h.heat), (900, "coolant", 100, 30_000));
        let mut s = Heatex::default();
        s.input.set_type("");
        assert_eq!(s.fluid_demand("water"), 0);
        s.fluid_give("steam", 1_050);
        s.step();
        assert_eq!((s.input.mb, s.output.fluid.as_str(), s.output.mb, s.heat), (50, "spentsteam", 10, 1_000));
        // Every second tick.
        s.delay = 2;
        s.fluid_give("steam", 150);
        let before = s.output.mb;
        s.step();
        s.step();
        assert_eq!(s.output.mb, before + 2);
    }

    /// The steps that stand for the NTM's text fields, never under 1.
    #[test]
    fn the_heat_exchanger_buttons_step_its_fields() {
        let mut h = Heatex::default();
        h.button(0);
        assert_eq!(h.amount, HEATEX_TANK, "no máximo o tanque");
        h.button(2);
        h.button(3);
        assert_eq!(h.amount, HEATEX_TANK - 1_001);
        h.button(7);
        assert_eq!(h.delay, 1);
        h.button(5);
        h.button(4);
        assert_eq!(h.screen_data()[6..8], [(HEATEX_TANK - 1_001) as i32, 22]);
    }
}
