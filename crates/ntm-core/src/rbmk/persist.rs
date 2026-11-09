//! What the save keeps of an RBMK (`rbmk-plano` T01, decisions D1, D3, D4 and D-c of the coordinator): each column's
//! state in its own block entity, beside the fuel rod in its slot (the stack's `minecraft:custom_data` holds the rod's
//! `yield`, `xenon`, `core` and `hull`, as the Java tag does); each layer's simulation state (the logical order of its
//! columns, its `java.util.Random` and the streams queued for the next step) in the world's state, advanced with the
//! columns' commit and set back with it when that is refused. Doubles go by their bits: a save and a reopening give
//! the very same numbers, NaN included.
//!
//! **Provisional (T01, approved D-b):** only the columns the minimal step needs are wired (fuel channels, ReaSim
//! channels, blank, moderator, reflector, absorber), at the default height and dials, without lids, ports, menus,
//! radiation, meltdowns or occlusion other than "any block is opaque". T02 (height, lids, ports), T03 (the rod
//! stack's codec, tooltips, hazards), T04 (scheduler, occlusion, radiation, dials) and T05 (slots by context) replace
//! what is marked here.

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

/// The column kind of an RBMK block and whether it is moderated (`RBMKRod(true)`). None: not a column the
/// provisional step runs (it stays inert).
pub fn kind_of(block: &str) -> Option<(Kind, bool)> {
    Some(match block {
        "ntm:rbmk_rod" => (Kind::Rod, false),
        "ntm:rbmk_rod_mod" => (Kind::Rod, true),
        "ntm:rbmk_rod_reasim" => (Kind::RodReaSim, false),
        "ntm:rbmk_rod_reasim_mod" => (Kind::RodReaSim, true),
        "ntm:rbmk_blank" => (Kind::Blank, false),
        "ntm:rbmk_moderator" => (Kind::Moderator, false),
        "ntm:rbmk_reflector" => (Kind::Reflector, false),
        "ntm:rbmk_absorber" => (Kind::Absorber, false),
        _ => return None,
    })
}

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
        }
    }

    /// `TileEntityRBMKBase.hasLid` of a column of `block`: a control rod's own lid always, else the lid put on it.
    pub fn has_lid(&self, block: &str) -> bool {
        has_own_lid(block) || self.lid != Lid::None
    }
}

impl Logic for RbmkColumn {
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
