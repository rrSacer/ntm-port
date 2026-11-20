//! What the save keeps of an RBMK (`rbmk-plano` T01, decisions D1, D3, D4 and D-c of the coordinator): each column's
//! state in its own block entity, beside the fuel rod in its slot (the stack's `minecraft:custom_data` holds the rod's
//! `yield`, `xenon`, `core` and `hull`, as the Java tag does); each layer's simulation state (the logical order of its
//! columns, its `java.util.Random` and the streams queued for the next step) in the world's state, advanced with the
//! columns' commit and set back with it when that is refused. Doubles go by their bits: a save and a reopening give
//! the very same numbers, NaN included.
//!
//! T02 (height, lids, ports), T03 (the rod stack's codec) and T04 (every column in the step, Java's opacity, the
//! dials by dimension, the satellite events) replaced T01's provisional wiring; still not kept here: a control rod's
//! level and target and power (T07), the fluid tanks (T08), the outgasser's progress (T09).

use crate::{
    machine::Logic,
    rbmk::{
        jrandom::JavaRandom,
        reactor::{Kind, QueuedStream},
        rods::{RodStack, RodType},
    },
    server::container_bridge::{Slots, Slotted, Stack},
    view::ViewModel,
};
use serde::{Deserialize, Serialize};

/// A double kept by its bits (`f64::to_bits`): lossless, NaN and -0.0 included.
mod bits {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(v.to_bits())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        u64::deserialize(d).map(f64::from_bits)
    }
}

/// The schema of [`RbmkColumn`] and [`Layer`]: a save of another schema is not read as this one.
pub const SCHEMA: u32 = 2;

/// The column kind of an RBMK block and whether it is moderated (`RBMKRod(true)`, `RBMKControl(true)`:
/// `ModBlocks:2113-2130`). None: not a column.
///
/// T04: all eighteen step, each as its tile entity is a node of the streams and a neighbour of `moveHeat`
/// (`RBMKNeutronHandler.getNodes`, `TileEntityRBMKBase.moveHeat`). What the save does not hold yet steps as Java's
/// fresh tile entity: dry boiler, heater and
/// cooler tanks (T08 fills them), an outgasser that never processes (T09 feeds it, its slots are not read here).
pub fn kind_of(block: &str) -> Option<(Kind, bool)> {
    Some(match block {
        "ntm:rbmk_rod" => (Kind::Rod, false),
        "ntm:rbmk_rod_mod" => (Kind::Rod, true),
        "ntm:rbmk_rod_reasim" => (Kind::RodReaSim, false),
        "ntm:rbmk_rod_reasim_mod" => (Kind::RodReaSim, true),
        "ntm:rbmk_control" => (Kind::Control, false),
        "ntm:rbmk_control_mod" => (Kind::Control, true),
        "ntm:rbmk_control_auto" => (Kind::ControlAuto, false),
        "ntm:rbmk_control_reasim" => (Kind::ControlReaSim, false),
        "ntm:rbmk_control_reasim_auto" => (Kind::ControlReaSimAuto, false),
        "ntm:rbmk_blank" => (Kind::Blank, false),
        "ntm:rbmk_boiler" => (Kind::Boiler, false),
        "ntm:rbmk_moderator" => (Kind::Moderator, false),
        "ntm:rbmk_reflector" => (Kind::Reflector, false),
        "ntm:rbmk_absorber" => (Kind::Absorber, false),
        "ntm:rbmk_outgasser" => (Kind::Outgasser, false),
        "ntm:rbmk_storage" => (Kind::Storage, false),
        "ntm:rbmk_cooler" => (Kind::Cooler, false),
        "ntm:rbmk_heater" => (Kind::Heater, false),
        _ => return None,
    })
}

/// `Block.isOpaqueCube` of an RBMK-family block as the pin overrides it, the one thing `RBMKNeutronStream.getHits`
/// asks of a block (`RBMKNeutronHandler:400-414`). None: not one of them (the engine's facts answer).
///
/// - `RBMKBase:58` true for every column; `RBMKRod:33` false for the four fuel channels (`RBMKRodReaSim` extends it);
/// - `BlockDummyable:358` false: console, crane console and autoloader; `RBMKMiniPanelBase:42` false: the nine panels;
/// - `RBMKDebris:21` false: the four `pribris` (the others extend it);
/// - the loader (`BlockGeneric`) and the steam inlet and outlet (`BlockContainer`) keep `Block`'s true.
///
/// A column's dummy is, in Java, the column's own block: the server reads its core below it ([`DUMMY_REACH`]).
pub fn java_opaque(block: &str) -> Option<bool> {
    Some(match block {
        "ntm:rbmk_rod" | "ntm:rbmk_rod_mod" | "ntm:rbmk_rod_reasim" | "ntm:rbmk_rod_reasim_mod" => false,
        b if is_column(b) => true,
        "ntm:rbmk_console" | "ntm:rbmk_crane_console" | "ntm:rbmk_autoloader" => false,
        "ntm:rbmk_display_blank" | "ntm:rbmk_display" | "ntm:rbmk_key_pad" | "ntm:rbmk_gauge" | "ntm:rbmk_numitron" | "ntm:rbmk_graph" | "ntm:rbmk_lever"
        | "ntm:rbmk_indicator" | "ntm:rbmk_terminal" => false,
        "ntm:pribris" | "ntm:pribris_burning" | "ntm:pribris_radiating" | "ntm:pribris_digamma" => false,
        "ntm:rbmk_loader" | "ntm:rbmk_steam_inlet" | "ntm:rbmk_steam_outlet" => true,
        _ => return None,
    })
}

/// How far down a column's dummy looks for its core (a column is at most 16 cells: the core and 15 dummies).
pub const DUMMY_REACH: i32 = 15;

/// The eighteen RBMK columns (`RBMKBase`): engine columns 2..16 cells high on `ntm:dummy_rbmk` (T02), each with its
/// block entity. Only those of [`kind_of`] step yet; the others keep their lid and their slots.
pub const COLUMNS: [&str; 18] = [
    "ntm:rbmk_rod",
    "ntm:rbmk_rod_mod",
    "ntm:rbmk_rod_reasim",
    "ntm:rbmk_rod_reasim_mod",
    "ntm:rbmk_control",
    "ntm:rbmk_control_mod",
    "ntm:rbmk_control_auto",
    "ntm:rbmk_control_reasim",
    "ntm:rbmk_control_reasim_auto",
    "ntm:rbmk_blank",
    "ntm:rbmk_boiler",
    "ntm:rbmk_reflector",
    "ntm:rbmk_absorber",
    "ntm:rbmk_moderator",
    "ntm:rbmk_outgasser",
    "ntm:rbmk_storage",
    "ntm:rbmk_cooler",
    "ntm:rbmk_heater",
];

/// The block every cell of a column but its core holds.
pub const DUMMY: &str = "ntm:dummy_rbmk";

pub fn is_column(block: &str) -> bool {
    COLUMNS.contains(&block)
}

/// `RBMKBase.hasOwnLid`: the control rods, whose lid is theirs and never comes off (`isLidRemovable` false).
pub fn has_own_lid(block: &str) -> bool {
    matches!(
        block,
        "ntm:rbmk_control" | "ntm:rbmk_control_auto" | "ntm:rbmk_control_mod" | "ntm:rbmk_control_reasim" | "ntm:rbmk_control_reasim_auto"
    )
}

/// The slots of a column's block entity (`TileEntityRBMKSlottedBase`'s size, the machine file's slots).
pub fn slot_count(block: &str) -> usize {
    match block {
        "ntm:rbmk_rod" | "ntm:rbmk_rod_mod" | "ntm:rbmk_rod_reasim" | "ntm:rbmk_rod_reasim_mod" | "ntm:rbmk_heater" => 1,
        "ntm:rbmk_outgasser" => 2,
        "ntm:rbmk_storage" => 12,
        _ => 0,
    }
}

/// The values a column's model and screen read (the machine file's `data`).
pub fn screen_len(block: &str) -> usize {
    match block {
        "ntm:rbmk_rod" | "ntm:rbmk_rod_mod" | "ntm:rbmk_rod_reasim" | "ntm:rbmk_rod_reasim_mod" | "ntm:rbmk_heater" => 7,
        "ntm:rbmk_control" | "ntm:rbmk_control_mod" | "ntm:rbmk_control_reasim" | "ntm:rbmk_boiler" => 6,
        "ntm:rbmk_control_auto" | "ntm:rbmk_control_reasim_auto" => 9,
        "ntm:rbmk_outgasser" => 5,
        _ => 1,
    }
}

/// A column's lid (`RBMKBase.metaToLid`: the core's metadata in Java).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lid {
    /// `DIR_NO_LID`: as every column is placed (`getDirModified`).
    #[default]
    None,
    /// `rbmk_lid`, `DIR_NORMAL_LID`.
    Standard,
    /// `rbmk_lid_glass`, `DIR_GLASS_LID`.
    Glass,
}

impl Lid {
    /// The lid item `item` is, if one.
    pub fn of_item(item: &str) -> Option<Lid> {
        match item {
            "ntm:rbmk_lid" => Some(Lid::Standard),
            "ntm:rbmk_lid_glass" => Some(Lid::Glass),
            _ => None,
        }
    }
    /// The item this lid drops as.
    pub fn item(self) -> Option<&'static str> {
        match self {
            Lid::None => None,
            Lid::Standard => Some("ntm:rbmk_lid"),
            Lid::Glass => Some("ntm:rbmk_lid_glass"),
        }
    }
}

/// The six directions in `ForgeDirection.VALID_DIRECTIONS`' order: down, up, north, south, west, east.
pub const VALID_DIRECTIONS: [[i32; 3]; 6] = [[0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];

/// A fluid port (`TilePort.PortDef.make`): the cell it sits in, and each cell it connects to with the direction from
/// the port (`DirPos`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortDef {
    pub at: [i32; 3],
    pub connections: Vec<([i32; 3], [i32; 3])>,
}

impl PortDef {
    pub fn make(at: [i32; 3], dirs: &[[i32; 3]]) -> Self {
        PortDef { at, connections: dirs.iter().map(|d| ([at[0] + d[0], at[1] + d[1], at[2] + d[2]], *d)).collect() }
    }
}

/// Where an `rbmk_loader` stands under a piped column (the boiler, heater, cooler and outgasser `updateEntity`): two
/// cells below wins over one (`ClassicLoader` is asked first).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoaderShape {
    None,
    Close,
    Classic,
}

impl LoaderShape {
    pub fn of(loader_one_below: bool, loader_two_below: bool) -> Self {
        if loader_two_below {
            LoaderShape::Classic
        } else if loader_one_below {
            LoaderShape::Close
        } else {
            LoaderShape::None
        }
    }
}

/// A piped column's fluid ports (`TileEntityRBMKBase.getPortsNoLoader`, `getPortsCloseLoader`,
/// `getPortsClassicLoader`), its core at `core` and `column_height` its `getColumnHeight` (cells above the core). Java
/// builds each shape once per block entity, with the dial as it read then, and keeps it (T08 decides what height it
/// passes; the column's own is the world's).
pub fn column_ports(core: [i32; 3], column_height: i32, shape: LoaderShape) -> Vec<PortDef> {
    let [x, y, z] = core;
    let top = PortDef::make([x, y + column_height, z], &[[0, 1, 0]]);
    match shape {
        LoaderShape::None => vec![top, PortDef::make(core, &[[0, -1, 0]])],
        LoaderShape::Close => vec![top, PortDef::make([x, y - 1, z], &VALID_DIRECTIONS)],
        LoaderShape::Classic => vec![top, PortDef::make(core, &[[0, -1, 0]]), PortDef::make([x, y - 2, z], &VALID_DIRECTIONS)],
    }
}

/// The fuel rod an item is (`ItemRBMKRod`): `ntm:rbmk_fuel_<name>`, the Java variable of `hep` being `hep239`.
pub fn rod_of(item: &str) -> Option<&'static RodType> {
    let name = item.strip_prefix("ntm:")?;
    let variable = if name == "rbmk_fuel_hep" { "rbmk_fuel_hep239" } else { name };
    RodType::by_variable(variable)
}

const ROD_KEYS: [&str; 4] = ["yield", "xenon", "core", "hull"];

/// The rod's tag read from a stack's `minecraft:custom_data` (provisional codec, T03 owns it): no data is no tag
/// (`stackTagCompound == null`); a key absent or not a double reads as never written. The non-finite doubles come as
/// Java writes them (`NaNd`, `Infinityd`, `-Infinityd`, the engine's SNBT): a tag goes through the stack without loss.
pub fn rod_stack(custom: Option<&str>) -> RodStack {
    let Some(fields) = custom.and_then(crate::snbt::fields) else { return RodStack::bare() };
    let read = |k: &str| fields.get(k).and_then(|v| v.strip_suffix(['d', 'D'])).and_then(|v| match v {
        "NaN" => Some(f64::NAN),
        "Infinity" => Some(f64::INFINITY),
        "-Infinity" => Some(f64::NEG_INFINITY),
        v if v.bytes().any(|c| c.is_ascii_alphabetic() && c != b'e' && c != b'E') => None,
        v => v.parse::<f64>().ok(),
    });
    RodStack { has_tag: true, yield_: read("yield"), xenon: read("xenon"), core: read("core"), hull: read("hull"), ..RodStack::bare() }
}

/// The rod's tag of a whole stack (T03): the 1.7.10 stack has its `stackTagCompound` when it holds any data that one
/// held (`Stack::tagged`), its keys in `minecraft:custom_data`; without one, `ItemRBMKRod.setNBTDefaults` on first use.
/// A rod with a name, lore, enchantments or a repair cost and no key yet has a tag with none: its yield reads 0
/// (`getDouble`), a dead rod, as in Java.
pub fn stack_rod(s: &Stack) -> RodStack {
    match (&s.custom, s.tagged) {
        (Some(custom), _) => rod_stack(Some(custom)),
        (None, true) => RodStack { has_tag: true, ..RodStack::bare() },
        (None, false) => RodStack::bare(),
    }
}

/// `custom` with the rod's tag written over its four keys, every other field kept. None: no tag to write (the stack
/// keeps none). Each double goes by its exact value: the shortest form that reads back to its bits, and Java's words
/// for NaN and the infinities (a NaN's payload is the canonical one, as `DataOutputStream.writeDouble` writes it).
pub fn with_rod_stack(custom: Option<&str>, s: &RodStack) -> Option<String> {
    if !s.has_tag {
        return custom.map(str::to_string);
    }
    let mut out = custom.map(str::to_string).unwrap_or_else(|| "{}".into());
    for (key, value) in ROD_KEYS.into_iter().zip([s.yield_, s.xenon, s.core, s.hull]) {
        if let Some(v) = value {
            out = crate::snbt::with_field(Some(&out), key, &format!("{}d", crate::snbt::java_double(v)));
        }
    }
    Some(out)
}

/// A column's block entity state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RbmkColumn {
    pub schema: u32,
    #[serde(with = "bits")]
    pub heat: f64,
    pub reasim_water: i32,
    pub reasim_steam: i32,
    #[serde(with = "bits")]
    pub flux_quantity: f64,
    #[serde(with = "bits")]
    pub flux_fast_ratio: f64,
    #[serde(with = "bits")]
    pub last_flux_quantity: f64,
    #[serde(with = "bits")]
    pub last_flux_ratio: f64,
    pub has_rod: bool,
    /// The engine's slots: the fuel channel's one slot, none for the others.
    pub slots: Slots,
    /// Its lid (T02); none in a save from before.
    #[serde(default, skip_serializing_if = "is_no_lid")]
    pub lid: Lid,
    /// How many values its screen reads ([`screen_len`]); 0 in a save from before T02 (a fuel channel's 7, else 1).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub screen: u8,
    /// An automatic control rod's settings (`TileEntityRBMKControlAuto`, T06): none for the other columns, and in a
    /// save from before T06, where they read as the TE's defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto: Option<AutoControl>,
    /// A control rod's movement (`TileEntityRBMKControl`, T07): none for the other columns, and in a save from before
    /// T07, where it reads as the TE's defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<ControlState>,
}

/// `TileEntityRBMKControl`'s state, and the manual rods' (`TileEntityRBMKControlManual`): what the Java saves (`level`,
/// `targetLevel`; `startingLevel` and `color` for a manual rod), doubles by their bits, and the power buffer of the ReaSim
/// rods, which the Java does not save (`power` is not in `writeToNBT`: a reopened rod starts empty) and so is kept in
/// memory only here too.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ControlState {
    #[serde(with = "bits")]
    pub level: f64,
    #[serde(with = "bits")]
    pub target_level: f64,
    #[serde(with = "bits")]
    pub starting_level: f64,
    /// `RBMKColor`'s ordinal (`RED`..`PURPLE`), none when the rod is in no group. Manual rods only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<u8>,
    /// The ReaSim rods (`isPowered`): `power` and `hasPower` (the last step's), never saved.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub powered: bool,
    #[serde(skip)]
    pub power: i64,
    #[serde(skip)]
    pub has_power: bool,
}

/// `TileEntityRBMKControl.maxPower`: ten ticks of movement.
pub const CONTROL_MAX_POWER: i64 = 50_000;

/// The five control rods (`TileEntityRBMKControl`).
pub fn is_control(block: &str) -> bool {
    matches!(block, "ntm:rbmk_control" | "ntm:rbmk_control_mod" | "ntm:rbmk_control_reasim" | "ntm:rbmk_control_auto" | "ntm:rbmk_control_reasim_auto")
}

/// `TileEntityRBMKControl.isPowered`: the ReaSim rods, which move only on power.
pub fn is_powered_control(block: &str) -> bool {
    matches!(block, "ntm:rbmk_control_reasim" | "ntm:rbmk_control_reasim_auto")
}

/// The manual rods (`TileEntityRBMKControlManual`).
pub fn is_manual_control(block: &str) -> bool {
    matches!(block, "ntm:rbmk_control" | "ntm:rbmk_control_mod" | "ntm:rbmk_control_reasim")
}

impl ControlState {
    pub fn of(block: &str) -> Self {
        ControlState { powered: is_powered_control(block), ..ControlState::default() }
    }
}

/// A ReaSim rod as a receiver on its network (`IEnergyReceiverMK2`): it takes what its buffer has room for
/// (`transferPower`, `getReceiverSpeed` = `getMaxPower`) and gives nothing.
impl crate::energy::EnergyNode for ControlState {
    fn role(&self) -> crate::energy::Role {
        crate::energy::Role::Consumer
    }
    fn offer(&self) -> u64 {
        0
    }
    fn demand(&self) -> u64 {
        (CONTROL_MAX_POWER - self.power).max(0) as u64
    }
    /// Energy the network gives it.
    fn give(&mut self, he: u64) {
        self.power = (self.power + he as i64).min(CONTROL_MAX_POWER);
    }
    /// It offers none, so the network takes none.
    fn take(&mut self, _he: u64) {}
}

/// `TileEntityRBMKControlAuto`'s settings, set from its screen (`receiveControl`): the levels at the heat bounds and
/// the curve between them. Doubles by their bits. The step moves the rod by them (T07).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AutoControl {
    #[serde(with = "bits")]
    pub level_lower: f64,
    #[serde(with = "bits")]
    pub level_upper: f64,
    #[serde(with = "bits")]
    pub heat_lower: f64,
    #[serde(with = "bits")]
    pub heat_upper: f64,
    /// `RBMKFunction`'s ordinal: 0 `LINEAR` (the default), 1 `QUAD_UP`, 2 `QUAD_DOWN`.
    pub function: u8,
}

/// The columns whose block entity is a `TileEntityRBMKControlAuto` (`RBMKControlAuto`).
pub fn is_auto_control(block: &str) -> bool {
    matches!(block, "ntm:rbmk_control_auto" | "ntm:rbmk_control_reasim_auto")
}

fn is_no_lid(l: &Lid) -> bool {
    *l == Lid::None
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}

impl RbmkColumn {
    pub fn of(block: &str) -> Self {
        RbmkColumn {
            schema: SCHEMA,
            heat: 0.0,
            reasim_water: 0,
            reasim_steam: 0,
            flux_quantity: 0.0,
            flux_fast_ratio: 0.0,
            last_flux_quantity: 0.0,
            last_flux_ratio: 0.0,
            has_rod: false,
            slots: vec![None; slot_count(block)],
            lid: Lid::None,
            screen: screen_len(block) as u8,
            auto: is_auto_control(block).then(AutoControl::default),
            control: is_control(block).then(|| ControlState::of(block)),
        }
    }

    /// `TileEntityRBMKBase.hasLid` of a column of `block`: a control rod's own lid always, else the lid put on it.
    pub fn has_lid(&self, block: &str) -> bool {
        has_own_lid(block) || self.lid != Lid::None
    }
}

impl Logic for RbmkColumn {
    /// A ReaSim control rod takes power from below (T07); no other column is on a network.
    fn energy(&mut self) -> Option<&mut dyn crate::energy::EnergyNode> {
        self.control.as_mut().filter(|c| c.powered).map(|c| c as &mut dyn crate::energy::EnergyNode)
    }
    fn level(&self, _: u8) -> u8 {
        0
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("RBMK")
    }
}

impl Slotted for RbmkColumn {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    /// The models' values (`docs/visual-fiel.md`): the first is the lid the model shows (`lid_cover` at 1,
    /// `lid_glass` at 2, as `RBMKBase.renderLid` from the core's metadata; a control rod's own is in its model); the
    /// others still neutral (T19 lights them).
    fn screen_data(&self) -> Vec<i32> {
        let n = match self.screen {
            0 if self.slots.is_empty() => 1,
            0 => 7,
            n => usize::from(n),
        };
        let mut v = vec![0; n];
        v[0] = match self.lid {
            Lid::None => 0,
            Lid::Standard => 1,
            Lid::Glass => 2,
        };
        // A control rod's level for its model (`RenderRBMKControlRod` raises the rod by it, in thousandths, T07), and
        // in [2] the automatic rod's curve (T06) or the manual rod's group, 0 for none (the model's colour).
        if let (Some(c), Some(l)) = (&self.control, v.get_mut(1)) {
            *l = (c.level * 1000.0) as i32;
        }
        if let (Some(c), Some(g)) = (&self.control, v.get_mut(2)) {
            *g = c.color.map_or(0, |c| i32::from(c) + 1);
        }
        if let (Some(a), Some(f)) = (&self.auto, v.get_mut(2)) {
            *f = i32::from(a.function);
        }
        v
    }
}

/// A queued stream as kept: its origin column by position, its values by their bits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeptStream {
    pub origin: [i32; 2],
    pub bits: [u64; 4],
}

/// A map by cell kept as a list of pairs (JSON keys are text).
mod by_cell {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;
    pub fn serialize<V: Serialize, S: Serializer>(m: &BTreeMap<[i32; 2], V>, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(m.iter())
    }
    pub fn deserialize<'de, V: Deserialize<'de>, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<[i32; 2], V>, D::Error> {
        Vec::<([i32; 2], V)>::deserialize(d).map(|v| v.into_iter().collect())
    }
}

/// A column a layer knows (D1, D4): when it was first registered, what it is, and the component whose state it
/// shares. Kept while the world may still show it: a column in a chunk not loaded is not gone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Member {
    pub seq: u64,
    pub block: String,
    pub comp: [i32; 2],
}

/// The simulation state of one component of a layer, one reactor: the columns its streams and heat link. Its
/// `java.util.Random` (D3) by its seed and cached gaussian, and the streams its last step emitted for the next one
/// (`NeutronNodeWorld`; the Java keeps them in memory only), in their order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompState {
    pub seed: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gaussian: Option<u64>,
    pub streams: Vec<KeptStream>,
}

impl CompState {
    fn seeded(seed: u64) -> Self {
        let (seed, gaussian) = JavaRandom::new(seed as i64).state();
        CompState { seed, gaussian: gaussian.map(f64::to_bits), streams: Vec::new() }
    }
    /// A component's first state: its random seeded from its place (its first column), the same in every world.
    pub fn fresh(dim: &str, y: i32, key: [i32; 2]) -> Self {
        Self::seeded(crate::ledger::fnv(&[b"rbmk component", dim.as_bytes(), &y.to_le_bytes(), &cell_bytes(key)]))
    }
    /// The state of a part split from `parent` that does not keep its random: a seed derived from the parent's state
    /// and the part's place, drawing nothing from the parent's random.
    pub fn derived(parent: &CompState, key: [i32; 2]) -> Self {
        let g = parent.gaussian.unwrap_or(u64::MAX).to_le_bytes();
        Self::seeded(crate::ledger::fnv(&[b"rbmk split", &parent.seed.to_le_bytes(), &g, &cell_bytes(key)]))
    }
    pub fn rng(&self) -> JavaRandom {
        JavaRandom::from_state(self.seed, self.gaussian.map(f64::from_bits))
    }
    pub fn set_rng(&mut self, rng: &JavaRandom) {
        let (seed, gaussian) = rng.state();
        self.seed = seed;
        self.gaussian = gaussian.map(f64::to_bits);
    }
    /// The kept streams of the columns in `order` (indices into it); a stream whose origin is not there is dropped,
    /// as `Reactor::remove` drops it.
    pub fn queue(&self, order: &[[i32; 2]]) -> Vec<QueuedStream> {
        self.streams
            .iter()
            .filter_map(|s| {
                let origin = order.iter().position(|p| *p == s.origin)?;
                let [vx, vz, q, r] = s.bits.map(f64::from_bits);
                Some(QueuedStream { origin, vx, vz, flux_quantity: q, flux_ratio: r })
            })
            .collect()
    }
    pub fn set_queue(&mut self, order: &[[i32; 2]], queue: &[QueuedStream]) {
        self.streams = queue
            .iter()
            .map(|q| KeptStream { origin: order[q.origin], bits: [q.vx, q.vz, q.flux_quantity, q.flux_ratio].map(f64::to_bits) })
            .collect();
    }
}

fn cell_bytes(c: [i32; 2]) -> [u8; 8] {
    let mut b = [0; 8];
    b[..4].copy_from_slice(&c[0].to_le_bytes());
    b[4..].copy_from_slice(&c[1].to_le_bytes());
    b
}

/// What a layer (dimension, base Y) keeps: the columns it knows and the state of each of its components.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub schema: u32,
    /// The next registration number (D1: the logical order is the order of registration).
    pub next_seq: u64,
    #[serde(with = "by_cell")]
    pub members: std::collections::BTreeMap<[i32; 2], Member>,
    /// By the cell of the component's first registered column.
    #[serde(with = "by_cell")]
    pub comps: std::collections::BTreeMap<[i32; 2], CompState>,
}

impl Layer {
    pub fn new() -> Self {
        Layer { schema: SCHEMA, next_seq: 0, members: Default::default(), comps: Default::default() }
    }
}

impl Default for Layer {
    fn default() -> Self {
        Self::new()
    }
}

/// The states of components after a merge or a split (D1, D3; docs/horizon-current.md "RBMK T01"): for each part (its
/// cells), its columns in registration order and its state. A parent's random goes to the part with its first
/// surviving column (on a merge, the part keeps the random of the parent whose first column was registered first); a
/// part that inherits none derives a seed from its first parent's state and its own place, drawing nothing from it; a
/// part with no parent starts fresh. Streams go with their origins, in their origins' registration order (the Java
/// list's order: update order).
pub fn settle<'a>(layer: &Layer, parts: impl Iterator<Item = &'a std::collections::BTreeSet<[i32; 2]>>, dim: &str, y: i32) -> (Vec<Vec<[i32; 2]>>, Vec<CompState>) {
    let seq = |c: &[i32; 2]| layer.members[c].seq;
    let orders: Vec<Vec<[i32; 2]>> = parts
        .map(|cells| {
            let mut o: Vec<[i32; 2]> = cells.iter().copied().collect();
            o.sort_by_key(seq);
            o
        })
        .collect();
    let mut heir: std::collections::BTreeMap<[i32; 2], (u64, usize)> = Default::default();
    for (i, o) in orders.iter().enumerate() {
        for c in o {
            let m = &layer.members[c];
            if layer.comps.contains_key(&m.comp) && heir.get(&m.comp).is_none_or(|h| m.seq < h.0) {
                heir.insert(m.comp, (m.seq, i));
            }
        }
    }
    let mut states = Vec::new();
    for (i, o) in orders.iter().enumerate() {
        let key = o[0];
        let mut parents: Vec<[i32; 2]> = o.iter().map(|c| layer.members[c].comp).filter(|p| layer.comps.contains_key(p)).collect();
        parents.sort_by_key(|p| heir[p].0);
        parents.dedup();
        let mut state = match parents.iter().find(|p| heir[*p].1 == i) {
            Some(p) => layer.comps[p].clone(),
            None => match parents.first() {
                Some(p) => CompState::derived(&layer.comps[p], key),
                None => CompState::fresh(dim, y, key),
            },
        };
        let mut streams: Vec<KeptStream> = parents.iter().flat_map(|p| layer.comps[p].streams.iter().filter(|s| o.contains(&s.origin)).cloned()).collect();
        streams.sort_by_key(|s| seq(&s.origin));
        state.streams = streams;
        states.push(state);
    }
    (orders, states)
}

/// The cells a column of `kind` links to, relative to it: the cells its streams can read (`Reactor::ray_cells`) for
/// a fuel channel, none for the others. The same at every place: the rays' cells are `floor(0.5 + v·i)` offsets.
pub fn ray_offsets(kind: Kind, range: i32) -> Vec<[i32; 2]> {
    crate::rbmk::reactor::Reactor::ray_cells(kind, 0, 0, range).into_iter().map(|(x, z)| [x, z]).collect()
}

/// The four fuel channels (`RBMKRod`, `RBMKRodReaSim`): the columns whose menu is `ContainerRBMKRod`.
pub fn is_channel(block: &str) -> bool {
    matches!(block, "ntm:rbmk_rod" | "ntm:rbmk_rod_mod" | "ntm:rbmk_rod_reasim" | "ntm:rbmk_rod_reasim_mod")
}

/// `ItemRBMKRod.getHullHeat` of the stack in a channel's slot, if it is a fuel rod (`instanceof ItemRBMKRod`): the
/// defaults of a stack with no tag (20), 0 for a tag without the key, NaN as written.
fn rod_hull(slot: Option<&Stack>) -> Option<f64> {
    let s = slot?;
    let rod = rod_of(&s.id)?;
    Some(stack_rod(s).hull_heat(rod))
}

/// `TileEntityRBMKRod.coldEnoughForManual`: no rod, or a hull at 200 °C or less (`<=`; NaN is not cold). What the
/// channel's menu asks of every survival gesture on its slot (`ContainerRBMKRod.slotClick`, `transferStackInSlot`).
pub fn cold_enough_for_manual(slot: Option<&Stack>) -> bool {
    rod_hull(slot).is_none_or(|h| h <= 200.0)
}

/// `TileEntityRBMKRod.coldEnoughForAutoloader`: no rod, or a hull at 1000 °C or less (the autoloader, T12).
pub fn cold_enough_for_autoloader(slot: Option<&Stack>) -> bool {
    rod_hull(slot).is_none_or(|h| h <= 1_000.0)
}

/// The hull at which `TileEntityRBMKRod.invalidate` melts the channel down when it goes (`>= 1500`, unless
/// `dialDisableMeltdowns`): the predicate T16's meltdown on break reads.
pub fn hot_enough_to_melt_on_break(slot: Option<&Stack>) -> bool {
    rod_hull(slot).is_some_and(|h| h >= 1_500.0)
}

/// `RayEvent.INFO_NUCLEAR`: what a fuel channel that took flux reports.
pub const INFO_NUCLEAR: &str = "NEUTRON_EMISSION";

/// One `SatelliteRayScan.RayEvent` (T04, decision 4A): what happened where, until when. The Java keeps them in a
/// static `LinkedHashMap<DimPos, RayEvent>` in memory only; here they are kept with the world (the coordinator's 4A),
/// ready for the satellites' front (`CMD_SURVEY` reads a dimension's within 250 blocks).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RayEvent {
    pub pos: [i32; 3],
    pub info: String,
    /// `getTotalWorldTime() + lifetime` as reported.
    pub expires_on: i64,
}

/// `SatelliteRayScan.reportEvent(world, x, y, z, info, lifetime)` in `dim`: the `LinkedHashMap.put` of its `DimPos`,
/// which replaces the event of the same place where it stands in the order, or adds it at the end.
pub fn report_ray(rays: &mut std::collections::BTreeMap<String, Vec<RayEvent>>, dim: &str, pos: [i32; 3], info: &str, now: i64, lifetime: i32) {
    let event = RayEvent { pos, info: info.to_string(), expires_on: now + i64::from(lifetime) };
    let list = rays.entry(dim.to_string()).or_default();
    match list.iter_mut().find(|e| e.pos == pos) {
        Some(e) => *e = event,
        None => list.push(event),
    }
}

/// `SatelliteRayScan.updateSystem(world)` of `dim` at world time `now` (`ModEventHandler.worldTick:661`, when
/// `getTotalWorldTime() % 20 == 10`): its events with `now > expiresOn` go. Whether it removes any.
pub fn expire_rays(rays: &std::collections::BTreeMap<String, Vec<RayEvent>>, dim: &str, now: i64) -> bool {
    rays.get(dim).is_some_and(|l| l.iter().any(|e| e.expired(now)))
}

impl RayEvent {
    /// `world.getTotalWorldTime() > entry.getValue().expiresOn`.
    pub fn expired(&self, now: i64) -> bool {
        now > self.expires_on
    }
}

/// The removal [`expire_rays`] finds.
pub fn drop_expired_rays(rays: &mut std::collections::BTreeMap<String, Vec<RayEvent>>, dim: &str, now: i64) {
    if let Some(l) = rays.get_mut(dim) {
        l.retain(|e| !e.expired(now));
        if l.is_empty() {
            rays.remove(dim);
        }
    }
}

/// The stack a fuel channel's slot shows for a rod tag: `stack` with the tag written in its data.
pub fn rod_in_slot(stack: &Stack, s: &RodStack) -> Stack {
    Stack { custom: with_rod_stack(stack.custom.as_deref(), s), ..stack.clone() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_column_and_a_layer_read_back_bit_for_bit() {
        let mut c = RbmkColumn::of("ntm:rbmk_rod_reasim");
        c.heat = f64::from_bits(0x4091_2345_6789_abcd);
        c.flux_fast_ratio = f64::NAN;
        c.last_flux_quantity = -0.0;
        let back: RbmkColumn = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back.heat.to_bits(), c.heat.to_bits());
        assert!(back.flux_fast_ratio.is_nan() && back.last_flux_quantity.to_bits() == (-0.0f64).to_bits());
        let mut l = Layer::new();
        l.members.insert([1, 0], Member { seq: 3, block: "ntm:rbmk_rod".into(), comp: [1, 0] });
        let mut c = CompState::fresh("minecraft:overworld", 64, [1, 0]);
        let order = [[0, 0], [1, 0]];
        let q = QueuedStream { origin: 1, vx: 0.5, vz: -1.0, flux_quantity: 1.0e-300, flux_ratio: 0.3 };
        c.set_queue(&order, &[q]);
        let mut rng = c.rng();
        rng.next_gaussian();
        c.set_rng(&rng);
        l.comps.insert([1, 0], c);
        let back: Layer = serde_json::from_str(&serde_json::to_string(&l).unwrap()).unwrap();
        assert_eq!(back, l);
        assert_eq!(back.comps[&[1, 0]].queue(&order), vec![q]);
        assert_eq!(back.comps[&[1, 0]].rng(), rng, "the cached gaussian too");
        // A stream of a column that left the component is dropped.
        assert!(back.comps[&[1, 0]].queue(&[[0, 0]]).is_empty());
        // A split part's random is its own, and drawing nothing from the parent's.
        let parent = &back.comps[&[1, 0]];
        let child = CompState::derived(parent, [5, 5]);
        assert_ne!(child.seed, parent.seed);
        assert_eq!(child, CompState::derived(parent, [5, 5]), "the same in every world");
        assert_eq!(parent.rng(), rng);
    }

    /// D1/D3 of a merge with interleaved registrations, and of a split: the random to the earliest parent and to the part
    /// with its first surviving column; streams with their origins, in registration order.
    #[test]
    fn components_settle_on_a_merge_and_a_split() {
        let member = |seq, comp| Member { seq, block: "ntm:rbmk_rod".into(), comp };
        let (a, b) = ([0, 0], [9, 0]);
        let mut l = Layer::new();
        // A registered 0 and 2, B registered 1: interleaved.
        l.members.insert(a, member(0, a));
        l.members.insert(b, member(1, b));
        l.members.insert([1, 0], member(2, a));
        let stream = |origin: [i32; 2], n: u64| KeptStream { origin, bits: [n, 0, 0, 0] };
        let mut sa = CompState::fresh("d", 1, a);
        sa.streams = vec![stream(a, 1), stream([1, 0], 2)];
        let mut sb = CompState::fresh("d", 1, b);
        sb.streams = vec![stream(b, 3)];
        l.comps.insert(a, sa.clone());
        l.comps.insert(b, sb);
        let all: std::collections::BTreeSet<[i32; 2]> = [a, b, [1, 0]].into();
        let (orders, states) = settle(&l, std::iter::once(&all), "d", 1);
        assert_eq!(orders, vec![vec![a, b, [1, 0]]]);
        assert_eq!(states[0].rng(), sa.rng(), "the earliest registered parent's random");
        let origins: Vec<[i32; 2]> = states[0].streams.iter().map(|s| s.origin).collect();
        assert_eq!(origins, vec![a, b, [1, 0]], "streams in registration order, not by component");
        // The merged one splits: A's first column keeps the random, the other part derives its seed.
        let mut m = l.clone();
        for v in m.members.values_mut() {
            v.comp = a;
        }
        m.comps = [(a, states[0].clone())].into();
        let (west, east): (std::collections::BTreeSet<[i32; 2]>, std::collections::BTreeSet<[i32; 2]>) = ([a, [1, 0]].into(), [b].into());
        let (orders, split) = settle(&m, [&east, &west].into_iter(), "d", 1);
        assert_eq!(orders, vec![vec![b], vec![a, [1, 0]]]);
        assert_eq!(split[1].rng(), states[0].rng(), "the part with the first column keeps it");
        assert_eq!(split[0], CompState { streams: vec![stream(b, 3)], ..CompState::derived(&states[0], b) });
        // A part with no parent starts fresh.
        let mut n = Layer::new();
        n.members.insert([5, 5], member(0, [5, 5]));
        let lone: std::collections::BTreeSet<[i32; 2]> = [[5, 5]].into();
        assert_eq!(settle(&n, std::iter::once(&lone), "d", 1).1[0], CompState::fresh("d", 1, [5, 5]));
    }

    /// `reportEvent` is a `LinkedHashMap.put` by place (a second report replaces the first where it stands); `updateSystem`
    /// removes an event only once the time is strictly past it, one dimension at a time.
    #[test]
    fn ray_events_replace_in_place_and_expire_strictly_after() {
        let mut rays = std::collections::BTreeMap::new();
        report_ray(&mut rays, "o", [1, 2, 3], INFO_NUCLEAR, 200, 300);
        report_ray(&mut rays, "o", [4, 5, 6], INFO_NUCLEAR, 200, 300);
        report_ray(&mut rays, "n", [1, 2, 3], INFO_NUCLEAR, 0, 300);
        report_ray(&mut rays, "o", [1, 2, 3], "RADIO_WAVES", 400, 300);
        let o = &rays["o"];
        assert_eq!(o.iter().map(|e| (e.pos, e.info.as_str(), e.expires_on)).collect::<Vec<_>>(), vec![([1, 2, 3], "RADIO_WAVES", 700), ([4, 5, 6], INFO_NUCLEAR, 500)]);
        assert!(!expire_rays(&rays, "o", 500), "not past 500 at 500");
        drop_expired_rays(&mut rays, "o", 500);
        assert_eq!(rays["o"].len(), 2, "nothing goes at 500");
        assert!(expire_rays(&rays, "o", 501));
        drop_expired_rays(&mut rays, "o", 501);
        assert_eq!(rays["o"].len(), 1);
        assert_eq!(rays["n"].len(), 1, "another dimension is not touched");
        drop_expired_rays(&mut rays, "o", 701);
        assert!(!rays.contains_key("o"));
    }

    /// `ItemRBMKRod.getDouble` over the whole stack: no 1.7.10 tag, the defaults on first use; any data that made one
    /// (a name, lore, enchantments, a repair cost) and no key, a yield of 0 (the renamed rod's bug, kept).
    #[test]
    fn a_stack_with_a_tag_but_no_key_reads_a_dead_rod_as_java() {
        let rod = rod_of("ntm:rbmk_fuel_ueu").unwrap();
        let plain = Stack::new("ntm:rbmk_fuel_ueu", 1);
        let mut fresh = stack_rod(&plain);
        assert!(!fresh.has_tag);
        assert_eq!((fresh.yield_(rod), fresh.core_heat(rod), fresh.hull_heat(rod), fresh.poison(rod)), (rod.yield_, 20.0, 20.0, 0.0));
        let renamed = Stack { tagged: true, ..plain.clone() };
        let mut dead = stack_rod(&renamed);
        assert!(dead.has_tag);
        assert_eq!((dead.yield_(rod), dead.core_heat(rod), dead.hull_heat(rod), dead.enrichment(rod)), (0.0, 0.0, 0.0, 0.0));
        // Its first write adds the keys to that tag, as `setDouble` does (the others still read 0).
        dead.set_hull_heat(rod, 30.0);
        let custom = with_rod_stack(None, &dead).unwrap();
        assert_eq!(rod_stack(Some(&custom)), RodStack { has_tag: true, hull: Some(30.0), ..RodStack::bare() });
        // A crafted rod (`onCreated`) carries the defaults: renamed then, it keeps them.
        let crafted = Stack { custom: with_rod_stack(None, &RodStack::fresh(rod)), tagged: true, ..plain };
        assert_eq!(stack_rod(&crafted).yield_, Some(rod.yield_));
    }

    #[test]
    fn the_rod_tag_round_trips_through_the_stack_data_keeping_foreign_fields() {
        let rod = rod_of("ntm:rbmk_fuel_ueu").unwrap();
        assert_eq!(rod_of("ntm:rbmk_fuel_hep").unwrap().variable, "rbmk_fuel_hep239");
        assert!(rod_of("ntm:rbmk_fuel_empty").is_none() && rod_of("minecraft:stone").is_none());
        assert_eq!(rod_stack(None), RodStack::bare(), "no data, no tag: defaults on first use");
        let mut s = RodStack::fresh(rod);
        s.set_poison(rod, 0.1 + 0.2);
        s.set_core_heat(rod, 20.000000000000004);
        let custom = with_rod_stack(Some("{oracle_marker:3,other:{a:1b}}"), &s).unwrap();
        let back = rod_stack(Some(&custom));
        assert_eq!((back.yield_, back.xenon, back.core, back.hull), (s.yield_, s.xenon, s.core, s.hull));
        let f = crate::snbt::fields(&custom).unwrap();
        assert_eq!((f["oracle_marker"].as_str(), f["other"].as_str()), ("3", "{a:1b}"));
        // A tag with a key never written keeps it unwritten.
        let partial = rod_stack(Some("{yield:5.0d}"));
        assert_eq!((partial.yield_, partial.xenon, partial.has_tag), (Some(5.0), None, true));
        // P2-1: every double by its exact value, the non-finite ones and -0.0 too, as the engine's SNBT writes them.
        for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.0, f64::MIN_POSITIVE / 3.0, 1.0e300, -7.25e-310] {
            let s = RodStack { has_tag: true, yield_: Some(v), xenon: Some(-v), core: Some(v), hull: Some(20.0), ..RodStack::bare() };
            let mut custom = with_rod_stack(Some("{a:1b}"), &s).unwrap();
            for _ in 0..3 {
                custom = crate::snbt::canonical(&custom).unwrap();
                custom = with_rod_stack(Some(&custom), &rod_stack(Some(&custom))).unwrap();
            }
            let back = rod_stack(Some(&custom));
            for (a, b) in [(back.yield_, s.yield_), (back.xenon, s.xenon), (back.core, s.core), (back.hull, s.hull)] {
                let (a, b) = (a.unwrap(), b.unwrap());
                assert!(a.to_bits() == b.to_bits() || a.is_nan() && b.is_nan(), "{v:?}: {custom}");
            }
        }
        assert!(with_rod_stack(None, &RodStack { has_tag: true, core: Some(f64::INFINITY), ..RodStack::bare() }).unwrap().contains("core:Infinityd"));
        // A word that is not a double is not one.
        assert_eq!(rod_stack(Some("{yield:\"5.0d\",core:infd}")).yield_, None);
    }
}

#[cfg(test)]
mod port_tests {
    use super::*;

    /// `getPorts*` of a column at (10, 64, -3), 4 high (`getColumnHeight` 3) and 16 high (15), as the Java builds them.
    #[test]
    fn the_piped_columns_ports_are_javas() {
        let core = [10, 64, -3];
        assert_eq!(LoaderShape::of(false, false), LoaderShape::None);
        assert_eq!(LoaderShape::of(true, false), LoaderShape::Close);
        assert_eq!(LoaderShape::of(true, true), LoaderShape::Classic, "two below is asked first");
        assert_eq!(LoaderShape::of(false, true), LoaderShape::Classic);
        let none = column_ports(core, 3, LoaderShape::None);
        assert_eq!(none, vec![
            PortDef { at: [10, 67, -3], connections: vec![([10, 68, -3], [0, 1, 0])] },
            PortDef { at: [10, 64, -3], connections: vec![([10, 63, -3], [0, -1, 0])] },
        ]);
        let close = column_ports(core, 15, LoaderShape::Close);
        assert_eq!(close[0], PortDef { at: [10, 79, -3], connections: vec![([10, 80, -3], [0, 1, 0])] });
        assert_eq!(close[1].at, [10, 63, -3]);
        assert_eq!(close[1].connections.iter().map(|c| c.0).collect::<Vec<_>>(), vec![[10, 62, -3], [10, 64, -3], [10, 63, -4], [10, 63, -2], [9, 63, -3], [11, 63, -3]]);
        let classic = column_ports(core, 1, LoaderShape::Classic);
        assert_eq!(classic.iter().map(|p| p.at).collect::<Vec<_>>(), vec![[10, 65, -3], [10, 64, -3], [10, 62, -3]]);
        assert_eq!(classic[2].connections.len(), 6);
        assert_eq!(classic[2].connections[1], ([10, 63, -3], [0, 1, 0]));
    }
}

#[cfg(test)]
mod hull_tests {
    use super::*;

    fn rod(custom: Option<&str>, tagged: bool) -> Stack {
        let mut s = Stack::new("ntm:rbmk_fuel_ueu", 1);
        s.custom = custom.map(str::to_string);
        s.tagged = tagged;
        s
    }

    fn hull(h: f64) -> Stack {
        rod(Some(&format!("{{hull:{}d}}", crate::snbt::java_double(h))), true)
    }

    #[test]
    fn the_hull_thresholds_are_javas_comparisons() {
        let cases: [(f64, bool, bool, bool); 9] = [
            (200.0, true, true, false),
            (f64::from_bits(200.0f64.to_bits() + 1), false, true, false),
            (1_000.0, false, true, false),
            (f64::from_bits(1_000.0f64.to_bits() + 1), false, false, false),
            (f64::from_bits(1_500.0f64.to_bits() - 1), false, false, false),
            (1_500.0, false, false, true),
            (f64::NAN, false, false, false),
            (f64::INFINITY, false, false, true),
            (f64::NEG_INFINITY, true, true, false),
        ];
        for (h, manual, autoloader, melts) in cases {
            let s = hull(h);
            assert_eq!(cold_enough_for_manual(Some(&s)), manual, "manual {h}");
            assert_eq!(cold_enough_for_autoloader(Some(&s)), autoloader, "autoloader {h}");
            assert_eq!(hot_enough_to_melt_on_break(Some(&s)), melts, "melts {h}");
        }
    }

    #[test]
    fn an_empty_slot_or_another_item_is_cold() {
        let stone = Stack::new("minecraft:stone", 1);
        let hot_stone = Stack { custom: Some("{hull:9000.0d}".into()), ..stone.clone() };
        for s in [None, Some(&stone), Some(&hot_stone)] {
            assert!(cold_enough_for_manual(s) && cold_enough_for_autoloader(s) && !hot_enough_to_melt_on_break(s));
        }
    }

    #[test]
    fn a_rod_without_the_key_reads_its_defaults_or_zero() {
        // No tag: `setNBTDefaults` first (hull 20). A tag without the key: `getDouble` reads 0. Both cold.
        for s in [rod(None, false), rod(None, true), rod(Some("{yield:5.0d}"), true)] {
            assert!(cold_enough_for_manual(Some(&s)) && !hot_enough_to_melt_on_break(Some(&s)), "{s:?}");
        }
    }

    /// `IEnergyReceiverMK2.transferPower`: more than the room left fills the buffer to `maxPower` (the overshoot goes
    /// back), never past it; the network never gives that much, so only a direct give shows it.
    #[test]
    fn a_reasim_rods_buffer_stops_at_its_maximum() {
        use crate::energy::EnergyNode;
        let mut c = ControlState::of("ntm:rbmk_control_reasim");
        c.power = 45_000;
        assert_eq!(c.demand(), 5_000);
        c.give(20_000);
        assert_eq!((c.power, c.demand()), (CONTROL_MAX_POWER, 0));
        c.give(1);
        assert_eq!(c.power, CONTROL_MAX_POWER);
    }
}
