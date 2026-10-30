//! The oil chain's batch D drills (docs/maquinas.md): the oil well, the
//! pumpjack and the fracking tower (`tileentity/machine/oil/
//! TileEntityOilDrillBase.java` and its three kinds). One machine in three
//! shapes; the world part (the pipe down, the deposit it sucks) is
//! `server/oil_server.rs`.
//!
//! Every tick with its power and room in both tanks it draws its HE; every
//! `delay` ticks it works the first block under its pipe: a deposit
//! (`ntm:ore_oil`, or an emptied one leading to more, 64 deep) gives oil and
//! some gas, and now and then empties; anything else is drilled into pipe,
//! down to its depth. The fracking tower also takes bedrock oil, wants
//! 10 mB of fracking solution a suck, and spoils the ground round it.
//! Upgrades in slots 5–7: speed, power, overdrive, afterburn (burning its
//! gas into power). Not here: the radon and asbestos pockets the well and
//! the pumpjack let out through uranium and asbestos ores (the content has
//! no such gas blocks), the containers' slots.

use crate::{
    energy::{EnergyNode, Role},
    fluid::{FluidNode, Tank},
    machine::Logic,
    server::container_bridge::{Slots, Slotted},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub mod slot {
    pub const BATTERY: usize = 0;
    pub const UPGRADES: std::ops::RangeInclusive<usize> = 5..=7;
    pub const COUNT: usize = 8;
}

pub const TANK: u32 = 64_000;
pub const PIPE: &str = "ntm:oil_pipe";
pub const OIL: &str = "ntm:ore_oil";
pub const EMPTY: &str = "ntm:ore_oil_empty";
pub const BEDROCK_OIL: &str = "ntm:ore_bedrock_oil";
/// Fracking solution a suck takes (`solutionRequired`).
pub const SOLUTION: u32 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shape {
    Well,
    Pumpjack,
    Fracker,
}

/// The `IConfigurableMachine` defaults of each kind.
pub struct Spec {
    pub max_power: u64,
    pub power: u64,
    pub delay: u64,
    pub oil: u32,
    pub gas: (u32, u32),
    pub drain: f64,
    /// The lowest block the pipe reaches. The NTM's fracker drills to 0;
    /// the 26.3 floor (and the adapted bedrock oil) is at −64.
    pub depth: i32,
}

impl Shape {
    pub fn of(block: &str) -> Option<Self> {
        Some(match block {
            "ntm:machine_well" => Shape::Well,
            "ntm:machine_pumpjack" => Shape::Pumpjack,
            "ntm:machine_fracking_tower" => Shape::Fracker,
            _ => return None,
        })
    }

    pub fn spec(self) -> Spec {
        match self {
            Shape::Well => Spec { max_power: 100_000, power: 100, delay: 50, oil: 500, gas: (100, 500), drain: 0.05, depth: 5 },
            Shape::Pumpjack => Spec { max_power: 250_000, power: 200, delay: 25, oil: 750, gas: (50, 250), drain: 0.025, depth: 5 },
            Shape::Fracker => Spec { max_power: 5_000_000, power: 5_000, delay: 20, oil: 1_000, gas: (100, 500), drain: 0.02, depth: -64 },
        }
    }

    fn title(self) -> &'static str {
        match self {
            Shape::Well => "Poço de petróleo",
            Shape::Pumpjack => "Pumpjack",
            Shape::Fracker => "Torre de fraturamento",
        }
    }

    /// The blocks its pipe may suck from (`canSuckBlock`).
    pub fn sucks(self, block: &str) -> bool {
        block == OIL || block == EMPTY || self == Shape::Fracker && block == BEDROCK_OIL
    }
}

/// The upgrades in slots 5–7, each kind's tiers added to 3 at most.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Upgrades {
    pub speed: u32,
    pub power: u32,
    pub overdrive: u32,
    pub afterburn: u32,
}

pub fn upgrade(item: &str) -> Option<(&'static str, u32)> {
    let rest = item.strip_prefix("ntm:upgrade_")?;
    let (kind, tier) = rest.rsplit_once('_')?;
    let kind = ["speed", "power", "overdrive", "afterburn"].into_iter().find(|k| *k == kind)?;
    Some((kind, tier.parse().ok().filter(|t| (1..=3).contains(t))?))
}

/// What the indicator lamp shows (`indicator`).
pub mod indicator {
    pub const WORKING: u8 = 0;
    pub const BOTTOM: u8 = 1;
    pub const STOPPED: u8 = 2;
    pub const NO_SOLUTION: u8 = 3;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Drill {
    pub shape: Shape,
    #[serde(skip, default = "empty_slots")]
    pub slots: Slots,
    pub power: u64,
    /// Oil, gas, and the fracker's fracking solution.
    pub tanks: Vec<Tank>,
    #[serde(default)]
    pub indicator: u8,
    #[serde(default)]
    pub ticks: u64,
    /// The draws of its chances.
    #[serde(default)]
    pub rolls: u64,
}

fn empty_slots() -> Slots {
    vec![None; slot::COUNT]
}

impl Drill {
    pub fn new(shape: Shape) -> Self {
        let mut tanks = vec![Tank::new(TANK), Tank::new(TANK)];
        tanks[0].set_type("oil");
        tanks[1].set_type("gas");
        if shape == Shape::Fracker {
            let mut t = Tank::new(TANK);
            t.set_type("fracksol");
            tanks.push(t);
        }
        Self { shape, slots: empty_slots(), power: 0, tanks, indicator: 0, ticks: 0, rolls: 0 }
    }

    pub fn upgrades(&self) -> Upgrades {
        let mut u = Upgrades::default();
        for s in slot::UPGRADES.filter_map(|i| self.slots.get(i)).flatten() {
            match upgrade(&s.id) {
                Some(("speed", t)) => u.speed = (u.speed + t).min(3),
                Some(("power", t)) => u.power = (u.power + t).min(3),
                Some(("overdrive", t)) => u.overdrive = (u.overdrive + t).min(3),
                Some((_, t)) => u.afterburn = (u.afterburn + t).min(3),
                None => {}
            }
        }
        u
    }

    /// `getPowerReqEff`: integer quarters, times the overdrive.
    pub fn power_req(&self) -> u64 {
        let u = self.upgrades();
        let req = self.shape.spec().power;
        (req + req / 4 * u64::from(u.speed) - req / 4 * u64::from(u.power)) * (u64::from(u.overdrive) + 1)
    }

    /// `getDelayEff`.
    pub fn delay(&self) -> u64 {
        let u = self.upgrades();
        let d = self.shape.spec().delay;
        ((d - d / 4 * u64::from(u.speed) + d / 10 * u64::from(u.power)) / (u64::from(u.overdrive) + 1)).max(1)
    }

    /// The tick before the world part: the battery, the afterburner, the
    /// power drawn. True when it works the pipe this tick.
    pub fn tick_power(&mut self) -> bool {
        let max = self.shape.spec().max_power;
        let burn = self.tanks[1].mb.min(self.upgrades().afterburn * 10);
        if burn > 0 {
            self.tanks[1].drain(burn);
            self.power = (self.power + u64::from(burn) * 5).min(max);
        }
        self.power = crate::machine::charge_from_slot(&self.slots, slot::BATTERY, self.power, max);
        self.ticks += 1;
        let req = self.power_req();
        if self.power >= req && self.tanks[0].mb < self.tanks[0].cap && self.tanks[1].mb < self.tanks[1].cap {
            self.power -= req;
            self.ticks.is_multiple_of(self.delay())
        } else {
            self.indicator = indicator::STOPPED;
            false
        }
    }

    /// `canPump`: the fracker wants its solution.
    pub fn can_pump(&mut self) -> bool {
        if self.shape == Shape::Fracker && self.tanks[2].mb < SOLUTION {
            self.indicator = indicator::NO_SOLUTION;
            return false;
        }
        true
    }

    /// A draw in [0, 1), from its own count (no world random).
    pub fn roll(&mut self) -> f64 {
        self.rolls += 1;
        f64::from(ntm_explosion::crater::rand01(self.rolls as i32, (self.rolls >> 31) as i32, self.shape as i32, 31))
    }

    /// `onSuck` from `block`: its oil and gas; true when the deposit
    /// empties (`ntm:ore_oil` only).
    pub fn suck(&mut self, block: &str) -> bool {
        let s = self.shape.spec();
        let (oil, (gmin, gmax), drains) = if block == BEDROCK_OIL { (100, (10, 50), false) } else { (s.oil, s.gas, true) };
        let gas = gmin + (self.roll() * f64::from(gmax - gmin + 1)) as u32;
        self.tanks[0].mb = (self.tanks[0].mb + oil).min(self.tanks[0].cap);
        self.tanks[1].mb = (self.tanks[1].mb + gas).min(self.tanks[1].cap);
        if self.shape == Shape::Fracker {
            self.tanks[2].drain(SOLUTION);
        }
        drains && self.roll() < s.drain
    }

    fn tank_values(t: &Tank) -> [i32; 3] {
        [t.mb as i32, t.cap as i32, crate::fluid::get(&t.fluid).map_or(0, |f| f.color as i32)]
    }

    /// The screen's values: [0]/[1] power, [2] the indicator, [3..5] oil,
    /// [6..8] gas, [9..11] the fracker's solution.
    pub fn screen_data(&self) -> Vec<i32> {
        let mut v = vec![self.power.min(i32::MAX as u64) as i32, self.shape.spec().max_power.min(i32::MAX as u64) as i32, i32::from(self.indicator)];
        for t in &self.tanks {
            v.extend(Self::tank_values(t));
        }
        v
    }
}

impl EnergyNode for Drill {
    fn role(&self) -> Role {
        Role::Consumer
    }
    fn offer(&self) -> u64 {
        0
    }
    fn demand(&self) -> u64 {
        self.shape.spec().max_power.saturating_sub(self.power)
    }
    fn give(&mut self, he: u64) {
        self.power += he;
    }
    fn take(&mut self, _he: u64) {}
}

/// Oil and gas out; the fracker's solution in.
impl FluidNode for Drill {
    fn fluid_offer(&self, f: &str) -> u64 {
        self.tanks[..2].iter().filter(|t| t.fluid == f).map(|t| u64::from(t.mb)).sum()
    }
    fn fluid_demand(&self, f: &str) -> u64 {
        self.tanks.get(2).filter(|t| t.fluid == f).map_or(0, |t| u64::from(t.free()))
    }
    fn fluid_types(&self) -> Vec<String> {
        self.tanks.iter().map(|t| t.fluid.clone()).collect()
    }
    fn fluid_give(&mut self, f: &str, mb: u64) {
        if let Some(t) = self.tanks.get_mut(2).filter(|t| t.fluid == f) {
            t.fill(f, mb.min(u64::from(u32::MAX)) as u32);
        }
    }
    fn fluid_take(&mut self, f: &str, mb: u64) {
        if let Some(t) = self.tanks[..2].iter_mut().find(|t| t.fluid == f) {
            t.drain(mb.min(u64::from(u32::MAX)) as u32);
        }
    }
}

impl Logic for Drill {
    /// Its work is the server's (it reads and sets the world).
    fn tick(&mut self) -> bool {
        false
    }
    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        Some(self)
    }
    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(self)
    }
    fn level(&self, max: u8) -> u8 {
        if self.indicator == indicator::WORKING && self.power > 0 { max } else { 0 }
    }
    fn view(&self) -> ViewModel {
        let mut vm = ViewModel::new(self.shape.title()).bar("Energia", self.power, self.shape.spec().max_power, "HE");
        for t in &self.tanks {
            vm = vm.text(t.line(), view::GREY);
        }
        vm
    }
}

impl Slotted for Drill {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        Drill::screen_data(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::container_bridge::Stack;

    #[test]
    fn upgrades_change_power_and_delay() {
        let mut d = Drill::new(Shape::Pumpjack);
        assert_eq!((d.power_req(), d.delay()), (200, 25));
        d.slots[5] = Some(Stack::new("ntm:upgrade_speed_2", 1));
        d.slots[6] = Some(Stack::new("ntm:upgrade_overdrive_1", 1));
        // (200 + 50·2)·2 and (25 − 6·2)/2.
        assert_eq!((d.power_req(), d.delay()), (600, 6));
    }

    #[test]
    fn a_suck_gives_oil_and_some_gas_and_the_fracker_spends_solution() {
        let mut w = Drill::new(Shape::Well);
        let mut drained = 0;
        for _ in 0..100 {
            if w.suck(OIL) {
                drained += 1;
            }
        }
        assert_eq!(w.tanks[0].mb, 50_000);
        assert!((10_000..=50_000).contains(&w.tanks[1].mb));
        assert!((1..=15).contains(&drained), "5% de chance: {drained}");
        let mut f = Drill::new(Shape::Fracker);
        f.fluid_give("fracksol", 1_000);
        assert!(!f.suck(BEDROCK_OIL), "o petróleo do bedrock não se esgota");
        assert_eq!((f.tanks[0].mb, f.tanks[2].mb), (100, 990));
        assert!(Shape::Fracker.sucks(BEDROCK_OIL) && !Shape::Well.sucks(BEDROCK_OIL));
    }
}
