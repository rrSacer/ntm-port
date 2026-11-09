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
pub const SCHEMA: u32 = 1;

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

/// The fuel rod an item is (`ItemRBMKRod`): `ntm:rbmk_fuel_<name>`, the Java variable of `hep` being `hep239`.
pub fn rod_of(item: &str) -> Option<&'static RodType> {
    let name = item.strip_prefix("ntm:")?;
    let variable = if name == "rbmk_fuel_hep" { "rbmk_fuel_hep239" } else { name };
    RodType::by_variable(variable)
}

const ROD_KEYS: [&str; 4] = ["yield", "xenon", "core", "hull"];

/// The rod's tag read from a stack's `minecraft:custom_data` (provisional codec, T03 owns it): no data is no tag
/// (`stackTagCompound == null`); a key absent or not a double reads as never written.
pub fn rod_stack(custom: Option<&str>) -> RodStack {
    let Some(fields) = custom.and_then(crate::snbt::fields) else { return RodStack::bare() };
    let read = |k: &str| fields.get(k).and_then(|v| v.strip_suffix(['d', 'D'])).and_then(|v| v.parse::<f64>().ok());
    RodStack { has_tag: true, yield_: read("yield"), xenon: read("xenon"), core: read("core"), hull: read("hull"), ..RodStack::bare() }
}

/// `custom` with the rod's tag written over its four keys, every other field kept. None: no tag to write (the stack
/// keeps none). A value with no SNBT form (NaN, infinite) leaves its key as it was.
pub fn with_rod_stack(custom: Option<&str>, s: &RodStack) -> Option<String> {
    if !s.has_tag {
        return custom.map(str::to_string);
    }
    let mut out = custom.map(str::to_string).unwrap_or_else(|| "{}".into());
    for (key, value) in ROD_KEYS.into_iter().zip([s.yield_, s.xenon, s.core, s.hull]) {
        if let Some(v) = value.filter(|v| v.is_finite()) {
            out = crate::snbt::with_field(Some(&out), key, &format!("{v:?}d"));
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
}

impl RbmkColumn {
    pub fn of(block: &str) -> Self {
        let rods = kind_of(block).is_some_and(|(k, _)| matches!(k, Kind::Rod | Kind::RodReaSim));
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
            slots: if rods { vec![None] } else { Vec::new() },
        }
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
    /// The models' values (`docs/visual-fiel.md`): still neutral, as before T01 (T19 lights them).
    fn screen_data(&self) -> Vec<i32> {
        vec![0; if self.slots.is_empty() { 1 } else { 7 }]
    }
}

/// A queued stream as kept: its origin column by position, its values by their bits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeptStream {
    pub origin: [i32; 2],
    pub bits: [u64; 4],
}

/// A layer's simulation state (one `Reactor` per dimension and base Y).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub schema: u32,
    /// The columns' logical order (D1): the order they were first registered, by position.
    pub order: Vec<[i32; 2]>,
    /// The layer's `java.util.Random` (D3): its seed and the cached gaussian, by its bits.
    pub seed: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gaussian: Option<u64>,
    /// The streams the last step emitted, for the next one (`NeutronNodeWorld`; the Java keeps them in memory only).
    pub streams: Vec<KeptStream>,
}

impl Layer {
    /// A new layer: its random seeded from its place, the same in every world (D3: a simulation's own RNG, not the
    /// world's stream).
    pub fn new(dim: &str, y: i32) -> Self {
        let seed = crate::ledger::fnv(&[b"rbmk layer", dim.as_bytes(), &y.to_le_bytes()]) as i64;
        let (seed, gaussian) = JavaRandom::new(seed).state();
        Layer { schema: SCHEMA, order: Vec::new(), seed, gaussian: gaussian.map(f64::to_bits), streams: Vec::new() }
    }
    pub fn rng(&self) -> JavaRandom {
        JavaRandom::from_state(self.seed, self.gaussian.map(f64::from_bits))
    }
    pub fn set_rng(&mut self, rng: &JavaRandom) {
        let (seed, gaussian) = rng.state();
        self.seed = seed;
        self.gaussian = gaussian.map(f64::to_bits);
    }
    /// The kept streams for columns in `order` (indices into it); a stream whose origin is no longer a column of the
    /// layer is dropped, as `Reactor::remove` drops it.
    pub fn queue(&self) -> Vec<QueuedStream> {
        self.streams
            .iter()
            .filter_map(|s| {
                let origin = self.order.iter().position(|p| *p == s.origin)?;
                let [vx, vz, q, r] = s.bits.map(f64::from_bits);
                Some(QueuedStream { origin, vx, vz, flux_quantity: q, flux_ratio: r })
            })
            .collect()
    }
    pub fn set_queue(&mut self, queue: &[QueuedStream]) {
        self.streams = queue
            .iter()
            .map(|q| KeptStream {
                origin: self.order[q.origin],
                bits: [q.vx, q.vz, q.flux_quantity, q.flux_ratio].map(f64::to_bits),
            })
            .collect();
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
        let mut l = Layer::new("minecraft:overworld", 64);
        l.order = vec![[0, 0], [1, 0]];
        let q = QueuedStream { origin: 1, vx: 0.5, vz: -1.0, flux_quantity: 1.0e-300, flux_ratio: 0.3 };
        l.set_queue(&[q]);
        let mut rng = l.rng();
        rng.next_gaussian();
        l.set_rng(&rng);
        let back: Layer = serde_json::from_str(&serde_json::to_string(&l).unwrap()).unwrap();
        assert_eq!(back, l);
        assert_eq!(back.queue(), vec![q]);
        assert_eq!(back.rng(), rng, "the cached gaussian too");
        // A stream of a column that left the order is dropped.
        let mut gone = back.clone();
        gone.order = vec![[0, 0]];
        assert!(gone.queue().is_empty());
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
    }
}
