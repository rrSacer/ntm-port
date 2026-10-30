//! Machines (docs/fundacao.md, "Máquinas"): the state the server keeps per
//! machine block, its tick, its screen and its buttons.
//!
//! A machine never touches the world or the player itself: the server
//! (`server/native_server.rs`) moves its items through its block entity's
//! slots.

use crate::{
    energy::{EnergyNode, Role},
    press::{BurnerPress, ElectricPress},
    recipes,
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

/// What the main hand holds (the old screen's actions).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Held {
    pub item: String,
    pub count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Battery,
    WoodBurner,
    ElectricFurnace,
    BurnerPress,
    ElectricPress,
    /// A bomb (`crate::bomb`): components, and what sets it off.
    Bomb,
    /// A fluid duct (`crate::fluidbox`): its fluid type.
    Duct,
    /// A barrel or tank of fluid (`crate::fluidbox`).
    FluidStore,
    /// A recipe machine (`crate::crafter`): assembly machine, chemical
    /// plant, centrifuge.
    Crafter,
    /// The gas centrifuge (`crate::gascent`).
    GasCent,
    /// The shredder (`crate::shredder`): its items are the engine's slots.
    Shredder,
    /// The blast furnace (`crate::blastfurnace`): engine slots too.
    BlastFurnace,
    /// The foundry's heat source and crucible (`crate::foundry`).
    Firebox,
    Crucible,
    /// Where the molten metal goes (`crate::foundry::flow`).
    Channel,
    FoundryTank,
    Outlet,
    /// Casting bases (`crate::foundry::cast`): small molds, and the basin.
    Mold,
    Basin,
    /// An anvil of any tier (`crate::anvil`): smithing and construction.
    Anvil,
    /// The bedrock chain (`crate::excavator`, `crate::bedrock::slopper`).
    Excavator,
    OreSlopper,
    /// The bedrock chain's stage 4 (`crate::arc_furnace`,
    /// `crate::electrolyser`).
    ArcFurnace,
    Electrolyser,
    /// The oil chain's batch B (`crate::refinery`, `crate::fraction`).
    Refinery,
    FractionTower,
    /// The oil chain's batch C (`crate::boiler`, `crate::coker`,
    /// `crate::processing`).
    Boiler,
    Coker,
    Processor,
    /// The oil chain's batch D (`crate::oil_drill`).
    Drill,
    /// Energy batch 1's heaters but the oven (`crate::heater`): the oven is
    /// a [`Kind::Firebox`].
    Heater,
    /// Energy batch 2's steam engine, turbines and condensers
    /// (`crate::steam`).
    Steam,
    /// Energy batch 3's diesel generator, combustion engine, turbofan and
    /// gas turbine (`crate::combustion`).
    Combustion,
    /// A machine block whose logic isn't written yet: tracked, screen says so.
    Inert,
}

/// What a button asks the server to do outside the machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Take one `item` from the player's main hand; on success the server
    /// calls [`Logic::consumed`].
    TakeHeld { item: String },
    /// As `TakeHeld`, and read which of `buckets` values (1..=buckets) the
    /// item's `minecraft:custom_data` holds under `key` (0: none of them):
    /// [`Logic::consumed_tagged`] gets it. A stamp's wear.
    TakeTagged {
        item: String,
        key: String,
        buckets: u8,
    },
    /// Put items in the player's inventory (a full one drops them).
    Give { item: String, count: u32 },
    Toast(String),
}

/// What every machine kind implements.
pub trait Logic {
    /// Once per tick while its chunk is loaded, before the networks move
    /// energy: whether its state changed (the world then needs a save).
    fn tick(&mut self) -> bool {
        false
    }
    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        None
    }
    /// Its tanks as a fluid-network node (`crate::fluid`).
    fn fluid(&mut self) -> Option<&mut dyn crate::fluid::FluidNode> {
        None
    }
    /// 0 idle, else 1..=max: what the block shows.
    fn level(&self, max: u8) -> u8;
    fn view(&self) -> ViewModel;
    fn action(&mut self, _action: u32, _held: Option<&Held>, _fx: &mut Vec<Effect>) {}
    /// A [`Effect::TakeHeld`] went through: the item is the machine's now.
    fn consumed(&mut self, _item: &str, _fx: &mut Vec<Effect>) {}
    /// The world forgot a take it had credited (it went back in time, a
    /// crash): one `item` leaves the machine again, if it is still there.
    fn unconsume(&mut self, _item: &str) -> bool {
        false
    }
    /// A [`Effect::TakeTagged`] went through, `tag` the bucket read.
    fn consumed_tagged(&mut self, item: &str, _tag: u8, fx: &mut Vec<Effect>) {
        self.consumed(item, fx)
    }
    /// What the machine holds, dropped when it is broken.
    fn contents(&self) -> Vec<(String, u32)> {
        Vec::new()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "k")]
pub enum Data {
    Battery(Battery),
    WoodBurner(WoodBurner),
    ElectricFurnace(ElectricFurnace),
    BurnerPress(BurnerPress),
    ElectricPress(ElectricPress),
    Bomb(crate::bomb::Bomb),
    Duct(crate::fluidbox::Duct),
    FluidStore(crate::fluidbox::FluidStore),
    Crafter(crate::crafter::Crafter),
    GasCent(crate::gascent::GasCent),
    Shredder(crate::shredder::Shredder),
    BlastFurnace(crate::blastfurnace::BlastFurnace),
    Firebox(crate::foundry::firebox::Firebox),
    Crucible(crate::foundry::crucible::Crucible),
    Channel(crate::foundry::flow::Channel),
    FoundryTank(crate::foundry::flow::Tank),
    Outlet(crate::foundry::flow::Outlet),
    Mold(crate::foundry::cast::Cast),
    Basin(crate::foundry::cast::Cast),
    Anvil(crate::anvil::Anvil),
    Excavator(crate::excavator::Excavator),
    OreSlopper(crate::bedrock::slopper::OreSlopper),
    ArcFurnace(crate::arc_furnace::ArcFurnace),
    /// Boxed: the fluid line's crafter and the metal line's stacks.
    Electrolyser(Box<crate::electrolyser::Electrolyser>),
    Refinery(Box<crate::refinery::Refinery>),
    FractionTower(crate::fraction::FractionTower),
    Boiler(crate::boiler::Boiler),
    Coker(Box<crate::coker::Coker>),
    Processor(Box<crate::processing::Processor>),
    Drill(Box<crate::oil_drill::Drill>),
    Heater(Box<crate::heater::Heater>),
    Steam(Box<crate::steam::Steam>),
    Combustion(Box<crate::combustion::Combustion>),
    Inert { name: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Machine {
    /// The block it is (`ntm:machine_battery`): its look comes from
    /// [`crate::registry::spec`].
    pub block: String,
    /// As placed: `north`, `south`, `east`, `west`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facing: Option<String>,
    pub data: Data,
}

impl Machine {
    pub fn new(kind: Kind, block_id: &str, facing: Option<String>) -> Self {
        let data = match kind {
            Kind::Battery => Data::Battery(Battery::new(if block_id == "ntm:teste" {
                100_000
            } else {
                1_000_000
            })),
            Kind::WoodBurner => Data::WoodBurner(WoodBurner::default()),
            Kind::ElectricFurnace => Data::ElectricFurnace(ElectricFurnace::default()),
            Kind::BurnerPress => Data::BurnerPress(BurnerPress::default()),
            Kind::ElectricPress => Data::ElectricPress(ElectricPress::default()),
            Kind::Bomb => Data::Bomb(crate::bomb::Bomb::new(block_id)),
            Kind::Duct => Data::Duct(crate::fluidbox::Duct::default()),
            Kind::GasCent => Data::GasCent(crate::gascent::GasCent::default()),
            Kind::Shredder => Data::Shredder(crate::shredder::Shredder::default()),
            Kind::BlastFurnace => Data::BlastFurnace(crate::blastfurnace::BlastFurnace::default()),
            Kind::Firebox if block_id == "ntm:heater_oven" => Data::Firebox(crate::foundry::firebox::Firebox::oven()),
            Kind::Firebox => Data::Firebox(crate::foundry::firebox::Firebox::default()),
            Kind::Crucible => Data::Crucible(crate::foundry::crucible::Crucible::default()),
            Kind::Channel => Data::Channel(Default::default()),
            Kind::FoundryTank => Data::FoundryTank(Default::default()),
            Kind::Outlet => Data::Outlet(Default::default()),
            Kind::Mold => Data::Mold(crate::foundry::cast::Cast::new(0)),
            Kind::Basin => Data::Basin(crate::foundry::cast::Cast::new(1)),
            Kind::Anvil => Data::Anvil(crate::anvil::Anvil::new(crate::anvil::tier(block_id))),
            Kind::Excavator => Data::Excavator(crate::excavator::Excavator::new()),
            Kind::OreSlopper => Data::OreSlopper(Default::default()),
            Kind::ArcFurnace => Data::ArcFurnace(Default::default()),
            Kind::Electrolyser => Data::Electrolyser(Default::default()),
            Kind::Refinery => Data::Refinery(Default::default()),
            Kind::FractionTower => Data::FractionTower(Default::default()),
            Kind::Boiler => Data::Boiler(crate::boiler::Boiler::new(block_id == "ntm:machine_industrial_boiler")),
            Kind::Coker => Data::Coker(Default::default()),
            Kind::Drill => match crate::oil_drill::Shape::of(block_id) {
                Some(s) => Data::Drill(Box::new(crate::oil_drill::Drill::new(s))),
                None => Data::Inert { name: block_id.into() },
            },
            Kind::Heater => match crate::heater::Heater::of(block_id) {
                Some(h) => Data::Heater(Box::new(h)),
                None => Data::Inert { name: block_id.into() },
            },
            Kind::Steam => match crate::steam::Steam::of(block_id) {
                Some(s) => Data::Steam(Box::new(s)),
                None => Data::Inert { name: block_id.into() },
            },
            Kind::Combustion => match crate::combustion::Combustion::of(block_id) {
                Some(c) => Data::Combustion(Box::new(c)),
                None => Data::Inert { name: block_id.into() },
            },
            Kind::Processor => match crate::processing::Shape::of(block_id) {
                Some(s) => Data::Processor(Box::new(crate::processing::Processor::new(s))),
                None => Data::Inert { name: block_id.into() },
            },
            Kind::Crafter => match crate::crafter::Station::of(block_id) {
                Some(st) => Data::Crafter(crate::crafter::Crafter::new(st)),
                None => Data::Inert { name: block_id.into() },
            },
            // `TileEntityBarrel` steel 16 000 mB; `TileEntityMachineFluidTank`
            // 256 000 mB at max(500, fill/100) a tick.
            Kind::FluidStore => Data::FluidStore(match block_id {
                "ntm:machine_fluidtank" => crate::fluidbox::FluidStore::new(256_000, 500),
                _ => crate::fluidbox::FluidStore::new(16_000, 0),
            }),
            Kind::Inert => Data::Inert {
                name: block_id.into(),
            },
        };
        Self {
            block: block_id.into(),
            facing,
            data,
        }
    }

    /// A machine saved under an older kind of its block (one inert until
    /// its stage, the electrolyser's fluid line before its metal line): its
    /// block's kind now, keeping what carries over.
    pub fn upgraded(self) -> Self {
        let Some(spec) = crate::registry::spec(&self.block) else { return self };
        let no_station = spec.kind == Kind::Crafter && crate::crafter::Station::of(&self.block).is_none();
        if spec.kind == self.kind() || matches!(self.data, Data::Inert { .. }) && no_station {
            return self;
        }
        let mut new = Machine::new(spec.kind, &self.block, self.facing.clone());
        if let (Data::Crafter(mut c), Data::Electrolyser(e)) = (self.data, &mut new.data) {
            // Its fluid line had no nitric acid tank then.
            c.tin.extend(e.fluid.tin.iter().skip(c.tin.len()).cloned());
            e.fluid = c;
        }
        new
    }

    pub fn kind(&self) -> Kind {
        match self.data {
            Data::Battery(_) => Kind::Battery,
            Data::WoodBurner(_) => Kind::WoodBurner,
            Data::ElectricFurnace(_) => Kind::ElectricFurnace,
            Data::BurnerPress(_) => Kind::BurnerPress,
            Data::ElectricPress(_) => Kind::ElectricPress,
            Data::Bomb(_) => Kind::Bomb,
            Data::Duct(_) => Kind::Duct,
            Data::FluidStore(_) => Kind::FluidStore,
            Data::Crafter(_) => Kind::Crafter,
            Data::GasCent(_) => Kind::GasCent,
            Data::Shredder(_) => Kind::Shredder,
            Data::BlastFurnace(_) => Kind::BlastFurnace,
            Data::Firebox(_) => Kind::Firebox,
            Data::Crucible(_) => Kind::Crucible,
            Data::Channel(_) => Kind::Channel,
            Data::FoundryTank(_) => Kind::FoundryTank,
            Data::Outlet(_) => Kind::Outlet,
            Data::Mold(_) => Kind::Mold,
            Data::Basin(_) => Kind::Basin,
            Data::Anvil(_) => Kind::Anvil,
            Data::Excavator(_) => Kind::Excavator,
            Data::OreSlopper(_) => Kind::OreSlopper,
            Data::ArcFurnace(_) => Kind::ArcFurnace,
            Data::Electrolyser(_) => Kind::Electrolyser,
            Data::Refinery(_) => Kind::Refinery,
            Data::FractionTower(_) => Kind::FractionTower,
            Data::Boiler(_) => Kind::Boiler,
            Data::Coker(_) => Kind::Coker,
            Data::Processor(_) => Kind::Processor,
            Data::Drill(_) => Kind::Drill,
            Data::Heater(_) => Kind::Heater,
            Data::Steam(_) => Kind::Steam,
            Data::Combustion(_) => Kind::Combustion,
            Data::Inert { .. } => Kind::Inert,
        }
    }

    pub fn logic(&self) -> &dyn Logic {
        match &self.data {
            Data::Battery(m) => m,
            Data::WoodBurner(m) => m,
            Data::ElectricFurnace(m) => m,
            Data::BurnerPress(m) => m,
            Data::ElectricPress(m) => m,
            Data::Bomb(m) => m,
            Data::Duct(m) => m,
            Data::FluidStore(m) => m,
            Data::Crafter(m) => m,
            Data::GasCent(m) => m,
            Data::Shredder(m) => m,
            Data::BlastFurnace(m) => m,
            Data::Firebox(m) => m,
            Data::Crucible(m) => m,
            Data::Channel(m) => m,
            Data::FoundryTank(m) => m,
            Data::Outlet(m) => m,
            Data::Mold(m) => m,
            Data::Basin(m) => m,
            Data::Anvil(m) => m,
            Data::Excavator(m) => m,
            Data::OreSlopper(m) => m,
            Data::ArcFurnace(m) => m,
            Data::Electrolyser(m) => &**m,
            Data::Refinery(m) => &**m,
            Data::FractionTower(m) => m,
            Data::Boiler(m) => m,
            Data::Coker(m) => &**m,
            Data::Processor(m) => &**m,
            Data::Drill(m) => &**m,
            Data::Heater(m) => &**m,
            Data::Steam(m) => &**m,
            Data::Combustion(m) => &**m,
            Data::Inert { name } => name,
        }
    }

    pub fn logic_mut(&mut self) -> &mut dyn Logic {
        match &mut self.data {
            Data::Battery(m) => m,
            Data::WoodBurner(m) => m,
            Data::ElectricFurnace(m) => m,
            Data::BurnerPress(m) => m,
            Data::ElectricPress(m) => m,
            Data::Bomb(m) => m,
            Data::Duct(m) => m,
            Data::FluidStore(m) => m,
            Data::Crafter(m) => m,
            Data::GasCent(m) => m,
            Data::Shredder(m) => m,
            Data::BlastFurnace(m) => m,
            Data::Firebox(m) => m,
            Data::Crucible(m) => m,
            Data::Channel(m) => m,
            Data::FoundryTank(m) => m,
            Data::Outlet(m) => m,
            Data::Mold(m) => m,
            Data::Basin(m) => m,
            Data::Anvil(m) => m,
            Data::Excavator(m) => m,
            Data::OreSlopper(m) => m,
            Data::ArcFurnace(m) => m,
            Data::Electrolyser(m) => &mut **m,
            Data::Refinery(m) => &mut **m,
            Data::FractionTower(m) => m,
            Data::Boiler(m) => m,
            Data::Coker(m) => &mut **m,
            Data::Processor(m) => &mut **m,
            Data::Drill(m) => &mut **m,
            Data::Heater(m) => &mut **m,
            Data::Steam(m) => &mut **m,
            Data::Combustion(m) => &mut **m,
            Data::Inert { name } => name,
        }
    }
}

impl Logic for String {
    fn level(&self, _max: u8) -> u8 {
        0
    }
    fn view(&self) -> ViewModel {
        ViewModel::new(self.trim_start_matches("ntm:").to_string())
            .text("A lógica desta máquina ainda não foi portada.", view::GREY)
    }
}

/// Takes one `item` out of `slot`, if it holds it.
pub fn remove_one(slot: &mut Option<(String, u32)>, item: &str) -> bool {
    match slot {
        Some((i, n)) if i == item && *n > 0 => {
            *n -= 1;
            if *n == 0 {
                *slot = None;
            }
            true
        }
        _ => false,
    }
}

/// `level` for a fill fraction: 0 when empty, else 1..=max.
pub fn fill_level(value: u64, cap: u64, max: u8) -> u8 {
    if value == 0 || cap == 0 {
        return 0;
    }
    let l = (u128::from(value) * u128::from(max) / u128::from(cap)) as u8;
    l.clamp(1, max)
}

// ---------------------------------------------------------------- battery

/// `TileEntityMachineBattery`: modes as NTM's (redstone not read: the
/// low-signal mode is the one in force).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatteryMode {
    Input,
    Buffer,
    Output,
    None,
}

impl BatteryMode {
    fn next(self) -> Self {
        match self {
            Self::Input => Self::Buffer,
            Self::Buffer => Self::Output,
            Self::Output => Self::None,
            Self::None => Self::Input,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Input => "Entrada (só carrega)",
            Self::Buffer => "Buffer (carrega e descarrega)",
            Self::Output => "Saída (só descarrega)",
            Self::None => "Desligada",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Battery {
    pub power: u64,
    pub max: u64,
    pub mode: BatteryMode,
    /// The mode with a redstone signal (`redHigh`); `mode` is `redLow`.
    #[serde(default = "high_mode")]
    pub mode_high: BatteryMode,
    /// It receives a redstone signal (the native host reads it): `mode_high`
    /// is in force.
    #[serde(skip)]
    pub powered: bool,
    /// `ContainerMachineBattery`: the item it discharges into itself (the
    /// creative battery fills it), the item it charges.
    #[serde(skip, default = "battery_slots")]
    pub slots: crate::server::container_bridge::Slots,
}

fn high_mode() -> BatteryMode {
    BatteryMode::Output
}

fn battery_slots() -> crate::server::container_bridge::Slots {
    vec![None; 2]
}

pub mod actions {
    pub const BATTERY_MODE: u32 = 1;
    /// Creative test: adds a tenth of the capacity.
    pub const BATTERY_DEBUG_FILL: u32 = 90;
    pub const BURNER_TOGGLE: u32 = 1;
    pub const BURNER_FUEL: u32 = 2;
    pub const FURNACE_INSERT: u32 = 1;
    pub const FURNACE_TAKE: u32 = 2;
}

impl Battery {
    pub fn new(max: u64) -> Self {
        Self {
            power: 0,
            max,
            mode: BatteryMode::Buffer,
            mode_high: high_mode(),
            powered: false,
            slots: battery_slots(),
        }
    }
}

impl Logic for Battery {
    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        Some(self)
    }
    fn level(&self, max: u8) -> u8 {
        fill_level(self.power, self.max, max)
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Bateria")
            .bar("Energia", self.power, self.max, "HE")
            .text(format!("Modo: {}", self.mode.label()), view::GREEN)
            .button(actions::BATTERY_MODE, "Trocar modo")
            .button(actions::BATTERY_DEBUG_FILL, "Teste: +10% de energia")
    }
    fn action(&mut self, action: u32, _held: Option<&Held>, _fx: &mut Vec<Effect>) {
        match action {
            actions::BATTERY_MODE => self.mode = self.mode.next(),
            actions::BATTERY_DEBUG_FILL => self.power = (self.power + self.max / 10).min(self.max),
            _ => {}
        }
    }
}

impl BatteryMode {
    /// Its number on the screen: input, buffer, output, off.
    fn index(self) -> i32 {
        match self {
            Self::Input => 0,
            Self::Buffer => 1,
            Self::Output => 2,
            Self::None => 3,
        }
    }
}

/// The native screen (`horizon_machine/machine_battery.json`,
/// `GUIMachineBattery`): slot 0 discharges into it (`chargeTEFromItems`),
/// slot 1 is charged; data [power, max, redLow, redHigh, priority]; buttons
/// 0 and 1 change the two modes. The network has no priorities: 1, normal.
impl crate::server::container_bridge::Slotted for Battery {
    fn slots_mut(&mut self) -> &mut crate::server::container_bridge::Slots {
        &mut self.slots
    }
    fn sync_in(&mut self) {
        self.power = charge_from_slot(&self.slots, 0, self.power, self.max);
    }
    fn screen_data(&self) -> Vec<i32> {
        vec![self.power.min(i32::MAX as u64) as i32, self.max.min(i32::MAX as u64) as i32, self.mode.index(), self.mode_high.index(), 1]
    }
    fn button(&mut self, id: u32) -> bool {
        match id {
            0 => self.mode = self.mode.next(),
            1 => self.mode_high = self.mode_high.next(),
            _ => {}
        }
        false
    }
}

impl Battery {
    /// The mode in force: `redHigh` with a signal, `redLow` without.
    pub fn effective(&self) -> BatteryMode {
        if self.powered { self.mode_high } else { self.mode }
    }
}

impl EnergyNode for Battery {
    fn role(&self) -> Role {
        Role::Storage
    }
    fn offer(&self) -> u64 {
        match self.effective() {
            BatteryMode::Output | BatteryMode::Buffer => self.power.min(self.max / 600),
            _ => 0,
        }
    }
    fn demand(&self) -> u64 {
        match self.effective() {
            BatteryMode::Input | BatteryMode::Buffer => (self.max - self.power).min(self.max / 200),
            _ => 0,
        }
    }
    fn give(&mut self, he: u64) {
        self.power = (self.power + he).min(self.max);
    }
    fn take(&mut self, he: u64) {
        self.power = self.power.saturating_sub(he);
    }
}

// ------------------------------------------------------------ wood burner

/// `TileEntityMachineWoodBurner`, solid fuel only: 100 HE a tick while it
/// burns, is on, and its buffer has room.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WoodBurner {
    pub power: u64,
    pub burn: u32,
    pub max_burn: u32,
    pub on: bool,
    /// Fuel items waiting (the NTM's fuel slot).
    pub fuel: Option<(String, u32)>,
    /// `ContainerMachineWoodBurner`: fuel, ash, the fluid containers (2, 3
    /// and 4), the battery it charges.
    #[serde(skip, default = "burner_slots")]
    pub slots: crate::server::container_bridge::Slots,
}

fn burner_slots() -> crate::server::container_bridge::Slots {
    vec![None; 6]
}

impl Default for WoodBurner {
    fn default() -> Self {
        Self {
            power: 0,
            burn: 0,
            max_burn: 0,
            on: true,
            fuel: None,
            slots: burner_slots(),
        }
    }
}

/// The native screen (`horizon_machine/machine_wood_burner.json`,
/// `GUIMachineWoodBurner`): slot 0 is its fuel; data [power, max, burn,
/// max burn, on, fluid mode, tank, capacity, colour] (no fluid burning
/// here: the last four 0); button 0 turns it on and off.
impl crate::server::container_bridge::Slotted for WoodBurner {
    fn slots_mut(&mut self) -> &mut crate::server::container_bridge::Slots {
        &mut self.slots
    }
    fn sync_in(&mut self) {
        self.fuel = self.slots[0].as_ref().filter(|s| s.count > 0 && burn_ticks(&s.id) > 0).map(|s| (s.id.clone(), s.count));
    }
    fn sync_out(&mut self) {
        if self.slots[0].as_ref().is_none_or(|s| burn_ticks(&s.id) > 0) {
            self.slots[0] = self.fuel.as_ref().map(|(i, n)| crate::server::container_bridge::Stack::new(i, *n));
        }
    }
    fn screen_data(&self) -> Vec<i32> {
        vec![self.power as i32, Self::MAX as i32, self.burn as i32, self.max_burn as i32, i32::from(self.on), 0, 0, 0, 0]
    }
    fn button(&mut self, id: u32) -> bool {
        if id == 0 {
            self.on = !self.on;
        }
        false
    }
}

impl WoodBurner {
    pub const MAX: u64 = 100_000;
    pub const GEN: u64 = 100;
    pub const FUEL_STACK: u32 = 64;
}

/// Burn ticks of a solid fuel (vanilla's furnace values; NTM's wood burner
/// reads the same table for these).
/// The creative battery (`ModItems.battery_creative`), in a machine's
/// battery slot: always full (`Library.chargeTEFromItems`). The port has no
/// other charge in items.
pub const CREATIVE_BATTERY: &str = "ntm:battery_creative";

/// `chargeTEFromItems` on slot `i` of `slots`: the power the machine has
/// after it.
pub fn charge_from_slot(slots: &[Option<crate::server::container_bridge::Stack>], i: usize, power: u64, max: u64) -> u64 {
    if slots.get(i).and_then(|s| s.as_ref()).is_some_and(|s| s.id == CREATIVE_BATTERY) { max } else { power }
}

pub fn burn_ticks(item: &str) -> u32 {
    let path = item.split(':').nth(1).unwrap_or(item);
    match path {
        "coal" | "charcoal" => 1600,
        "coal_block" => 16000,
        "blaze_rod" => 2400,
        "stick" | "bamboo" => 100,
        "lava_bucket" => 0, // a bucket would be lost: not taken
        _ if path.ends_with("_log")
            || path.ends_with("_wood")
            || path.ends_with("_stem")
            || path.ends_with("_hyphae")
            || path.ends_with("_planks") =>
        {
            300
        }
        _ if path.ends_with("_sapling") => 100,
        _ => 0,
    }
}

impl Logic for WoodBurner {
    fn tick(&mut self) -> bool {
        let before = (self.burn, self.power);
        if self.burn == 0
            && self.on
            && self.power < Self::MAX
            && let Some((item, n)) = &mut self.fuel
        {
            self.max_burn = burn_ticks(item);
            self.burn = self.max_burn;
            *n -= 1;
            if *n == 0 {
                self.fuel = None;
            }
        }
        if self.burn > 0 && self.on && self.power < Self::MAX {
            self.burn -= 1;
            self.power = (self.power + Self::GEN).min(Self::MAX);
        }
        (self.burn, self.power) != before
    }
    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        Some(self)
    }
    fn level(&self, max: u8) -> u8 {
        if self.burn == 0 || !self.on {
            0
        } else {
            fill_level(u64::from(self.burn), u64::from(self.max_burn), max)
        }
    }
    fn view(&self) -> ViewModel {
        let mut vm = ViewModel::new("Queimador de madeira")
            .bar("Energia", self.power, Self::MAX, "HE")
            .bar("Queima", u64::from(self.burn), u64::from(self.max_burn), "t")
            .text(
                if self.on { "Ligado" } else { "Desligado" },
                if self.on { view::GREEN } else { view::GREY },
            );
        vm = match &self.fuel {
            Some((item, n)) => vm.item(item, format!("Combustível: {n}")),
            None => vm.text("Sem combustível", view::GREY),
        };
        vm.button(actions::BURNER_FUEL, "Pôr combustível da mão (1)")
            .button(actions::BURNER_TOGGLE, if self.on { "Desligar" } else { "Ligar" })
    }
    fn action(&mut self, action: u32, held: Option<&Held>, fx: &mut Vec<Effect>) {
        match action {
            actions::BURNER_TOGGLE => self.on = !self.on,
            actions::BURNER_FUEL => {
                let Some(held) = held.filter(|h| burn_ticks(&h.item) > 0) else {
                    fx.push(Effect::Toast("Segure um combustível sólido na mão.".into()));
                    return;
                };
                match &self.fuel {
                    Some((item, n)) if *item != held.item || *n >= Self::FUEL_STACK => {
                        fx.push(Effect::Toast("O compartimento de combustível está ocupado.".into()))
                    }
                    _ => fx.push(Effect::TakeHeld {
                        item: held.item.clone(),
                    }),
                }
            }
            _ => {}
        }
    }
    fn unconsume(&mut self, item: &str) -> bool {
        remove_one(&mut self.fuel, item)
    }
    fn consumed(&mut self, item: &str, fx: &mut Vec<Effect>) {
        match &mut self.fuel {
            None => self.fuel = Some((item.into(), 1)),
            Some((i, n)) if i == item => *n += 1,
            // Raced another insert: hand it back.
            Some(_) => fx.push(Effect::Give {
                item: item.into(),
                count: 1,
            }),
        }
    }
    fn contents(&self) -> Vec<(String, u32)> {
        self.fuel.iter().cloned().collect()
    }
}

impl EnergyNode for WoodBurner {
    fn role(&self) -> Role {
        Role::Provider
    }
    fn offer(&self) -> u64 {
        self.power
    }
    fn demand(&self) -> u64 {
        0
    }
    fn give(&mut self, _he: u64) {}
    fn take(&mut self, he: u64) {
        self.power = self.power.saturating_sub(he);
    }
}

// ------------------------------------------------------- electric furnace

/// `TileEntityMachineElectricFurnace` without upgrades: 50 HE a tick, 100
/// ticks an item (`furnace_recipes.json`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ElectricFurnace {
    pub power: u64,
    pub progress: u32,
    pub input: Option<(String, u32)>,
    pub output: Option<(String, u32)>,
    /// The vanilla furnace menu's slots ([`furnace_slot`]).
    #[serde(skip, default = "furnace_slots")]
    pub slots: crate::server::container_bridge::Slots,
}

fn furnace_slots() -> crate::server::container_bridge::Slots {
    vec![None; furnace_slot::SLOTS]
}

impl Default for ElectricFurnace {
    fn default() -> Self {
        Self { power: 0, progress: 0, input: None, output: None, slots: furnace_slots() }
    }
}

/// The electric furnace's slots (`ContainerElectricFurnace`): what it
/// smelts, its battery, what it made.
pub mod furnace_slot {
    pub const INPUT: usize = 0;
    pub const BATTERY: usize = 1;
    pub const OUTPUT: usize = 2;
    pub const SLOTS: usize = 3;
}

/// The native screen (the vanilla furnace menu): the slots in and out, the
/// battery charging it (`chargeTEFromItems`); data power, max, progress,
/// total.
impl crate::server::container_bridge::Slotted for ElectricFurnace {
    fn slots_mut(&mut self) -> &mut crate::server::container_bridge::Slots {
        &mut self.slots
    }
    fn sync_in(&mut self) {
        use furnace_slot::*;
        let pile = |s: &Option<crate::server::container_bridge::Stack>| s.as_ref().filter(|s| s.count > 0).map(|s| (s.id.clone(), s.count));
        self.input = pile(&self.slots[INPUT]);
        self.output = pile(&self.slots[OUTPUT]);
        self.power = charge_from_slot(&self.slots, BATTERY, self.power, Self::MAX);
    }
    fn sync_out(&mut self) {
        use furnace_slot::*;
        let stack = |p: &Option<(String, u32)>| p.as_ref().map(|(i, n)| crate::server::container_bridge::Stack::new(i, *n));
        self.slots[INPUT] = stack(&self.input);
        self.slots[OUTPUT] = stack(&self.output);
    }
    fn screen_data(&self) -> Vec<i32> {
        crate::native::furnace_data(self)
    }
}

impl ElectricFurnace {
    pub const MAX: u64 = 100_000;
    pub const STACK: u32 = 64;

    fn cfg() -> (u32, u64) {
        let m = recipes::furnace();
        (m.ticks, m.energy_per_tick)
    }
    fn can_smelt(&self) -> Option<&'static recipes::Smelt> {
        let (item, _) = self.input.as_ref()?;
        let r = recipes::furnace().smelt(item)?;
        match &self.output {
            None => Some(r),
            Some((o, n)) if *o == r.output && n + r.count <= Self::STACK => Some(r),
            _ => None,
        }
    }
}

impl Logic for ElectricFurnace {
    fn tick(&mut self) -> bool {
        let (ticks, per_tick) = Self::cfg();
        let Some(r) = self.can_smelt() else {
            let changed = self.progress != 0;
            self.progress = 0;
            return changed;
        };
        if self.power < per_tick {
            return false;
        }
        self.power -= per_tick;
        self.progress += 1;
        if self.progress >= ticks {
            self.progress = 0;
            let (out, count) = (r.output.clone(), r.count);
            match &mut self.output {
                Some((_, n)) => *n += count,
                None => self.output = Some((out, count)),
            }
            if let Some((_, n)) = &mut self.input {
                *n -= 1;
                if *n == 0 {
                    self.input = None;
                }
            }
        }
        true
    }
    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        Some(self)
    }
    fn level(&self, max: u8) -> u8 {
        if self.progress == 0 {
            0
        } else {
            fill_level(u64::from(self.progress), u64::from(Self::cfg().0), max)
        }
    }
    fn view(&self) -> ViewModel {
        let (ticks, _) = Self::cfg();
        let mut vm = ViewModel::new("Fornalha elétrica")
            .bar("Energia", self.power, Self::MAX, "HE")
            .bar("Progresso", u64::from(self.progress), u64::from(ticks), "t");
        vm = match &self.input {
            Some((i, n)) => vm.item(i, format!("Entrada: {n}")),
            None => vm.text("Entrada vazia", view::GREY),
        };
        vm = match &self.output {
            Some((i, n)) => vm.item(i, format!("Saída: {n}")),
            None => vm.text("Saída vazia", view::GREY),
        };
        vm.button(actions::FURNACE_INSERT, "Pôr item da mão (1)")
            .button(actions::FURNACE_TAKE, "Retirar a saída")
    }
    fn action(&mut self, action: u32, held: Option<&Held>, fx: &mut Vec<Effect>) {
        match action {
            actions::FURNACE_INSERT => {
                let Some(held) = held.filter(|h| recipes::furnace().smelt(&h.item).is_some())
                else {
                    fx.push(Effect::Toast("Segure um item que a fornalha funde.".into()));
                    return;
                };
                match &self.input {
                    Some((i, n)) if *i != held.item || *n >= Self::STACK => {
                        fx.push(Effect::Toast("A entrada está ocupada.".into()))
                    }
                    _ => fx.push(Effect::TakeHeld {
                        item: held.item.clone(),
                    }),
                }
            }
            actions::FURNACE_TAKE => {
                if let Some((item, count)) = self.output.take() {
                    fx.push(Effect::Give { item, count });
                }
            }
            _ => {}
        }
    }
    fn unconsume(&mut self, item: &str) -> bool {
        remove_one(&mut self.input, item)
    }
    fn consumed(&mut self, item: &str, fx: &mut Vec<Effect>) {
        match &mut self.input {
            None => self.input = Some((item.into(), 1)),
            Some((i, n)) if i == item => *n += 1,
            Some(_) => fx.push(Effect::Give {
                item: item.into(),
                count: 1,
            }),
        }
    }
    fn contents(&self) -> Vec<(String, u32)> {
        self.input.iter().chain(&self.output).cloned().collect()
    }
}

impl EnergyNode for ElectricFurnace {
    fn role(&self) -> Role {
        Role::Consumer
    }
    fn offer(&self) -> u64 {
        0
    }
    fn demand(&self) -> u64 {
        Self::MAX - self.power.min(Self::MAX)
    }
    fn give(&mut self, he: u64) {
        self.power = (self.power + he).min(Self::MAX);
    }
    fn take(&mut self, _he: u64) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burner_burns_fuel_into_power() {
        let mut b = WoodBurner::default();
        let mut fx = vec![];
        b.action(
            actions::BURNER_FUEL,
            Some(&Held { item: "minecraft:oak_log".into(), count: 3 }),
            &mut fx,
        );
        assert_eq!(fx, vec![Effect::TakeHeld { item: "minecraft:oak_log".into() }]);
        b.consumed("minecraft:oak_log", &mut fx);
        b.tick();
        assert_eq!((b.burn, b.power, b.fuel.clone()), (299, 100, None));
        assert!(b.level(5) >= 4);
    }

    #[test]
    fn furnace_smelts_with_power() {
        let mut f = ElectricFurnace {
            power: ElectricFurnace::MAX,
            input: Some(("minecraft:raw_iron".into(), 1)),
            ..Default::default()
        };
        for _ in 0..100 {
            f.tick();
        }
        assert_eq!(f.output, Some(("minecraft:iron_ingot".into(), 1)));
        assert_eq!(f.input, None);
        assert_eq!(f.power, ElectricFurnace::MAX - 5000);
    }

    #[test]
    fn battery_mode_cycles_and_serialises() {
        let mut m = Machine::new(Kind::Battery, "ntm:machine_battery", Some("east".into()));
        m.logic_mut().action(actions::BATTERY_MODE, None, &mut vec![]);
        let json = serde_json::to_string(&m).unwrap();
        let back: Machine = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
        assert!(json.contains("Output"), "{json}");
    }
}
