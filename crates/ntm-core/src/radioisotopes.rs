//! RTG and radgen, from RTGUtil/ItemRTGPellet and TileEntityMachineRadGen.
//! Slot edits are speculative inside server::transaction. `pending` preserves
//! Java's offer-before-generation order across the shared HE network phase.
use crate::{
    energy::{EnergyNode, Role},
    machine::Logic,
    server::container_bridge::{Slots, Slotted, Stack},
    view::ViewModel,
};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub const RTG: &str = "ntm:machine_rtg_grey";
pub const RADGEN: &str = "ntm:machine_radgen";
pub const DEPLETION: &str = "PELLET_DEPLETION";

#[derive(Debug, Deserialize)]
pub struct Heat {
    pub decay_enabled: i32,
    pub decay_disabled: i32,
}
#[derive(Debug, Deserialize)]
pub struct Pellet {
    pub item: String,
    pub heat_tu: Heat,
    pub lifespan_ticks: i64,
    pub decay_item: String,
}
#[derive(Debug, Deserialize)]
pub struct Item {
    pub item: String,
    pub count: u32,
}
#[derive(Debug, Deserialize)]
pub struct Fuel {
    pub input: Item,
    pub output: Option<Item>,
    pub power_he_per_tick: u64,
    pub duration_ticks: i32,
}
#[derive(Debug, Deserialize)]
pub struct Table {
    pub rtg_pellets: Vec<Pellet>,
    pub radgen_fuels: Vec<Fuel>,
}
pub fn table() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!("../data/radioisotopes.json"))
            .expect("radioisotope source snapshot")
    })
}
pub fn pellet(id: &str) -> Option<&'static Pellet> {
    table().rtg_pellets.iter().find(|p| p.item == id)
}
pub fn fuel(id: &str) -> Option<&'static Fuel> {
    table().radgen_fuels.iter().find(|f| f.input.item == id)
}

/// Java distinguishes no compound (fresh) from an existing compound with no
/// depletion field (NBT getLong returns zero). Keep the full signed long.
pub fn life(s: &Stack) -> i64 {
    let Some(p) = pellet(&s.id) else { return 0 };
    let Some(custom) = &s.custom else {
        return p.lifespan_ticks;
    };
    crate::snbt::fields(custom)
        .and_then(|m| m.get(DEPLETION).cloned())
        .and_then(|v| {
            let v = v.trim_end_matches(['b', 's', 'l', 'f', 'd', 'B', 'S', 'L', 'F', 'D']);
            v.parse::<i64>().ok().or_else(|| {
                v.parse::<f64>()
                    .ok()
                    .filter(|n| n.is_finite())
                    .map(|n| n as i64)
            })
        })
        .unwrap_or(0)
}

/// HazardModifierRTGRadiation: subtract endpoints in float, interpolate in
/// double, cast to float, then quantize to the shared milliRAD/s protocol.
pub fn radiation_rate(s: &Stack) -> Option<u32> {
    let p = pellet(&s.id)?;
    let h = crate::radiation::rtg_hazard(&s.id)?;
    let depletion = 1. - life(s) as f64 / p.lifespan_ticks as f64;
    let target = h.rtg_target_rad_s?;
    crate::item_hooks::hazard_rate(h.radiation_rad_s,target,depletion)
}

/// Automation: RTG inherits TileEntityMachineBase's insertion refusal. Radgen balances
/// same-item/meta stacks, ignoring NBT exactly as isItemValidForSlot does.
/// The engine additionally enforces stackability and the item's stack limit.
pub fn can_insert(block: &str, slots: &[Option<Stack>], slot: usize, item: &Stack) -> bool {
    if block == RTG {
        return false;
    }
    if block != RADGEN || slot >= 12 || fuel(&item.id).is_none() {
        return false;
    }
    let Some(current) = slots.get(slot).and_then(Option::as_ref) else {
        return true;
    };
    (0..12).all(|i| {
        slots
            .get(i)
            .and_then(Option::as_ref)
            .is_some_and(|s| s.id != item.id || s.damage != item.damage || s.count >= current.count)
    })
}
/// Slot vanilla accepts any manual input, independent of inventory validation.
pub fn can_insert_manual(block: &str, slot: usize) -> bool {
    match block {
        RTG => slot < 15,
        RADGEN => slot < 12,
        _ => false,
    }
}
pub fn can_extract(block: &str, slot: usize) -> bool {
    block == RADGEN && (12..24).contains(&slot)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rtg {
    pub power: u64,
    pub heat: i32,
    pub decay: bool,
    pub scale_power: bool,
    #[serde(default)]
    pub pending: u64,
    #[serde(skip)]
    pub slots: Slots,
}
impl Default for Rtg {
    fn default() -> Self {
        Self {
            power: 0,
            heat: 0,
            decay: true,
            scale_power: false,
            pending: 0,
            slots: vec![None; 15],
        }
    }
}
impl Rtg {
    pub const MAX: u64 = 100_000;
    pub fn max_heat(&self) -> i32 {
        if self.decay { 600 } else { 200 }
    }
    pub fn step(&mut self) {
        self.finish();
        self.heat = 0;
        for s in self.slots.iter_mut().flatten() {
            let Some(p) = pellet(&s.id) else { continue };
            let remaining = life(s);
            let base = if self.decay {
                p.heat_tu.decay_enabled
            } else {
                p.heat_tu.decay_disabled
            };
            let heat = if self.scale_power {
                (f64::from(base) * (remaining as f64 / p.lifespan_ticks as f64)).ceil() as i32
                    as i16 as i32
            } else {
                base
            };
            self.heat = self.heat.wrapping_add(heat);
            if self.decay {
                if remaining <= 0 {
                    *s = Stack::new(&p.decay_item, 1);
                } else {
                    s.custom = Some(crate::snbt::with_field(
                        s.custom.as_deref(),
                        DEPLETION,
                        &format!("{}l", remaining - 1),
                    ));
                }
            }
        }
        self.heat = self.heat.min(self.max_heat());
        self.pending = (self.heat * 5).max(0) as u64;
    }
    pub fn finish(&mut self) {
        self.power = self
            .power
            .saturating_add(std::mem::take(&mut self.pending))
            .min(Self::MAX);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Queue {
    pub progress: i32,
    pub max_progress: i32,
    pub production: u64,
    pub processing: Option<Stack>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Radgen {
    pub power: u64,
    pub queues: [Queue; 12],
    pub output: u64,
    pub on: bool,
    #[serde(default)]
    pub pending: u64,
    #[serde(skip)]
    pub slots: Slots,
}
impl Default for Radgen {
    fn default() -> Self {
        Self {
            power: 0,
            queues: std::array::from_fn(|_| Queue::default()),
            output: 0,
            on: false,
            pending: 0,
            slots: vec![None; 24],
        }
    }
}
impl Radgen {
    pub const MAX: u64 = 1_000_000;
    pub fn step(&mut self) {
        self.finish();
        for (i, q) in self.queues.iter_mut().enumerate() {
            if q.processing.is_some() {
                continue;
            }
            let Some(s) = self.slots.get(i).and_then(Option::as_ref) else {
                continue;
            };
            let Some(f) = fuel(&s.id) else { continue };
            if f.output.as_ref().is_some_and(|out| {
                self.slots[i + 12]
                    .as_ref()
                    .is_some_and(|s| s.id != out.item || s.damage != 0 || s.count + out.count > 64)
            }) {
                continue;
            }
            q.progress = 0;
            q.max_progress = f.duration_ticks;
            q.production = f.power_he_per_tick;
            q.processing = Some(Stack {
                custom: None,
                count: 1,
                ..s.clone()
            });
            let s = self.slots[i].as_mut().unwrap();
            s.count -= 1;
            if s.count == 0 {
                self.slots[i] = None;
            }
        }
        self.output = 0;
        self.on = false;
        for (i, q) in self.queues.iter_mut().enumerate() {
            let Some(s) = &q.processing else { continue };
            self.on = true;
            self.output += q.production;
            q.progress += 1;
            if q.progress >= q.max_progress {
                q.progress = 0;
                if let Some(out) = fuel(&s.id).and_then(|f| f.output.as_ref()) {
                    // Java checks capacity on admission only. Hoppers only
                    // remove from outputs; operators may still change them.
                    if let Some(s) = &mut self.slots[i + 12] {
                        s.count += out.count;
                    } else {
                        self.slots[i + 12] = Some(Stack::new(&out.item, out.count));
                    }
                }
                q.processing = None;
            }
        }
        self.pending = self.output;
    }
    pub fn finish(&mut self) {
        self.power = self
            .power
            .saturating_add(std::mem::take(&mut self.pending))
            .min(Self::MAX);
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Radioisotope {
    Rtg(Rtg),
    Radgen(Box<Radgen>),
}
impl Radioisotope {
    pub fn of(block: &str) -> Self {
        if block == RTG {
            Self::Rtg(Rtg::default())
        } else {
            Self::Radgen(Box::default())
        }
    }
    pub fn finish(&mut self) {
        match self {
            Self::Rtg(r) => r.finish(),
            Self::Radgen(r) => r.finish(),
        }
    }
    fn power(&self) -> u64 {
        match self {
            Self::Rtg(r) => r.power,
            Self::Radgen(r) => r.power,
        }
    }
    fn max(&self) -> u64 {
        match self {
            Self::Rtg(_) => Rtg::MAX,
            Self::Radgen(_) => Radgen::MAX,
        }
    }
}
impl Logic for Radioisotope {
    fn tick(&mut self) -> bool {
        match self {
            Self::Rtg(r) => r.step(),
            Self::Radgen(r) => r.step(),
        }
        true
    }
    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        Some(self)
    }
    fn level(&self, max: u8) -> u8 {
        match self {
            Self::Rtg(r) if r.heat > 0 => max,
            Self::Radgen(r) if r.on => max,
            _ => 0,
        }
    }
    fn view(&self) -> ViewModel {
        ViewModel::new(match self {
            Self::Rtg(_) => "RTG",
            Self::Radgen(_) => "Radgen",
        })
        .text(
            format!("{} / {} HE", self.power(), self.max()),
            crate::view::GREY,
        )
    }
}
impl EnergyNode for Radioisotope {
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
        match self {
            Self::Rtg(r) => r.power = r.power.saturating_sub(he),
            Self::Radgen(r) => r.power = r.power.saturating_sub(he),
        }
    }
}
impl Slotted for Radioisotope {
    fn slots_mut(&mut self) -> &mut Slots {
        match self {
            Self::Rtg(r) => &mut r.slots,
            Self::Radgen(r) => &mut r.slots,
        }
    }
    fn screen_data(&self) -> Vec<i32> {
        match self {
            Self::Rtg(r) => vec![r.heat, r.max_heat(), r.power as i32, Rtg::MAX as i32],
            Self::Radgen(r) => [r.power as i32, Radgen::MAX as i32]
                .into_iter()
                .chain(r.queues.iter().flat_map(|q| [q.progress, q.max_progress]))
                .chain([r.on as i32, r.output as i32])
                .chain(r.queues.iter().map(|q| q.production as i32))
                .collect(),
        }
    }
    // BlockDummyable.breakBlock drops only the inventory slots. An already
    // consumed radgen fuel is not refunded on destruction (which would permit
    // repeated partial burns of the same gem).
    fn save_state(&self) -> Vec<i32> {
        // JSON state is already saved atomically with slots. The legacy i32
        // channel has no extra state; it carries only screen data.
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tagged(id: &str, n: i64) -> Stack {
        Stack {
            custom: Some(format!("{{{DEPLETION}:{n}l,other:{{x:7}}}}")),
            ..Stack::new(id, 1)
        }
    }
    #[test]
    fn lifetimes_preserve_java_float_rounding_and_signed_long() {
        assert_eq!(table().rtg_pellets.len(), 10);
        assert_eq!(table().radgen_fuels.len(), 28);
        let expected = [
            11_520_000_000,
            720_000_000,
            631_440_000,
            208_800_000,
            38_160_000,
            156_960_000,
            3_384_000_000,
            9_936_000,
            194_400,
            21_600,
        ];
        for (p, n) in table().rtg_pellets.iter().zip(expected) {
            assert_eq!(p.lifespan_ticks, n);
            assert_eq!(life(&Stack::new(&p.item, 1)), n);
            for n in [0, -1, 1, i32::MAX as i64 + 1, i64::MAX, i64::MIN] {
                assert_eq!(life(&tagged(&p.item, n)), n);
            }
            assert_eq!(
                life(&Stack {
                    custom: Some("{other:7}".into()),
                    ..Stack::new(&p.item, 1)
                }),
                0
            );
        }
    }
    #[test]
    fn rtg_samples_heat_before_decay_and_replacement_and_preserves_other_tags() {
        for p in &table().rtg_pellets {
            let mut r = Rtg::default();
            r.slots[0] = Some(Stack::new(&p.item, 1));
            r.step();
            assert_eq!(r.power, 0);
            assert_eq!(r.pending, p.heat_tu.decay_enabled as u64 * 5);
            assert_eq!(life(r.slots[0].as_ref().unwrap()), p.lifespan_ticks - 1);
            r.slots[0] = Some(tagged(&p.item, 1));
            r.step();
            assert_eq!(life(r.slots[0].as_ref().unwrap()), 0);
            assert!(
                r.slots[0]
                    .as_ref()
                    .unwrap()
                    .custom
                    .as_ref()
                    .unwrap()
                    .contains("other:{x:7}")
            );
            r.step();
            assert_eq!(r.heat, p.heat_tu.decay_enabled);
            assert_eq!(r.slots[0], Some(Stack::new(&p.decay_item, 1)));
            r.step();
            assert_eq!(r.heat, 0);
        }
        let mut r = Rtg::default();
        r.slots[0] = Some(Stack {
            custom: Some("{}".into()),
            ..Stack::new("ntm:pellet_rtg_lead", 1)
        });
        r.step();
        assert_eq!(
            r.slots[0].as_ref().unwrap().id,
            "ntm:pellet_rtg_depleted__bismuth"
        );
        assert_eq!(r.heat, 600);
    }
    #[test]
    fn caps_config_and_stack_life_match_java() {
        let mut r = Rtg::default();
        r.slots.fill(Some(Stack::new("ntm:pellet_rtg_lead", 1)));
        r.step();
        assert_eq!(r.heat, 600);
        r.decay = false;
        r.step();
        assert_eq!(r.heat, 200);
        let old = r.slots.clone();
        r.step();
        assert_eq!(r.slots, old);
        r.scale_power = true;
        r.decay = true;
        r.slots.fill(None);
        r.slots[0] = Some(tagged("ntm:pellet_rtg", 315_720_000));
        r.step();
        assert_eq!(r.heat, 5);
        r.slots[0] = Some(tagged("ntm:pellet_rtg", 1));
        r.step();
        assert_eq!(r.heat, 1);
        r.step();
        assert_eq!(r.heat, 0);
        // Commands may bypass the item's max=1: heat and depletion are per
        // stack; Java replaces the entire expired stack with ONE product.
        let mut s = tagged("ntm:pellet_rtg_lead", 0);
        s.count = 12;
        r.slots[0] = Some(s);
        r.scale_power = false;
        r.step();
        assert_eq!(r.heat, 600);
        assert_eq!(r.slots[0].as_ref().unwrap().count, 1);
    }
    #[test]
    fn every_radgen_fuel_runs_exact_duration_and_conserves_items_and_energy() {
        for f in &table().radgen_fuels {
            let mut r = Radioisotope::of(RADGEN);
            r.slots_mut()[0] = Some(Stack {
                custom: Some("{ignored:7}".into()),
                ..Stack::new(&f.input.item, 2)
            });
            let mut exported = 0;
            for tick in 1..=f.duration_ticks {
                r.tick();
                let he = r.offer();
                exported += he;
                r.take(he);
                r.finish();
                let Radioisotope::Radgen(g) = &r else {
                    panic!()
                };
                assert_eq!(g.power, f.power_he_per_tick);
                assert_eq!(g.slots[0].as_ref().unwrap().count, 1);
                if tick < f.duration_ticks {
                    assert_eq!(g.queues[0].progress, tick);
                    assert!(g.slots[12].is_none());
                    assert!(g.queues[0].processing.as_ref().unwrap().custom.is_none());
                } else {
                    assert!(g.queues[0].processing.is_none());
                    assert_eq!(g.queues[0].progress, 0);
                    assert_eq!(
                        g.slots[12],
                        f.output.as_ref().map(|o| Stack::new(&o.item, o.count))
                    );
                }
            }
            let Radioisotope::Radgen(g) = &r else {
                panic!()
            };
            assert_eq!(
                exported + g.power,
                f.power_he_per_tick * f.duration_ticks as u64
            );
            assert_eq!(
                g.slots.iter().flatten().map(|s| s.count).sum::<u32>(),
                1 + f.output.as_ref().map_or(0, |o| o.count)
            );
        }
    }
    #[test]
    fn twelve_queues_capacity_only_at_admission_and_full_buffer_keeps_burning() {
        let mut g = Radgen::default();
        for i in 0..12 {
            g.slots[i] = Some(Stack::new("ntm:nuclear_waste_short_tiny__uranium235", 1));
        }
        g.slots[12] = Some(Stack::new("minecraft:stone", 1));
        g.step();
        assert!(g.queues[0].processing.is_none());
        assert_eq!(g.output, 11 * 150);
        g.slots[12] = Some(Stack::new(
            "ntm:nuclear_waste_short_depleted_tiny__uranium235",
            64,
        ));
        g.step();
        assert!(g.queues[0].processing.is_none());
        g.slots[12].as_mut().unwrap().count = 63;
        g.step();
        assert!(g.queues.iter().all(|q| q.processing.is_some()));
        g.power = Radgen::MAX;
        for _ in 0..3600 {
            g.step();
            g.finish();
        }
        assert_eq!(g.power, Radgen::MAX);
        assert!(g.queues.iter().all(|q| q.processing.is_none()));
        assert_eq!(g.slots.iter().flatten().map(|s| s.count).sum::<u32>(), 75);
        assert_eq!(g.slots[12].as_ref().unwrap().count, 64);
        g.step();
        assert!(!g.on);
        // A privileged command can replace an output mid-run. Match Java's
        // intentional absence of a second capacity/item check at completion.
        g.slots[0] = Some(Stack::new("ntm:gem_rad", 1));
        g.slots[12] = None;
        g.step();
        g.slots[12] = Some(Stack::new("minecraft:stone", 64));
        g.queues[0].progress = 35999;
        g.step();
        assert_eq!(g.slots[12], Some(Stack::new("minecraft:stone", 65)));
    }
    #[test]
    fn offer_before_generation_and_clamp_after_transfer() {
        for block in [RTG, RADGEN] {
            let mut r = Radioisotope::of(block);
            match &mut r {
                Radioisotope::Rtg(g) => {
                    g.power = Rtg::MAX;
                    g.slots[0] = Some(Stack::new("ntm:pellet_rtg_lead", 1));
                }
                Radioisotope::Radgen(g) => {
                    g.power = Radgen::MAX;
                    g.slots[0] = Some(Stack::new("ntm:gem_rad", 1));
                }
            }
            let cap = r.max();
            r.tick();
            assert_eq!(r.offer(), cap);
            r.take(cap);
            r.finish();
            assert_eq!(r.offer(), if block == RTG { 3000 } else { 25000 });
            let he = r.offer();
            r.tick();
            assert_eq!(r.offer(), he);
            r.take(he);
            r.finish();
            assert_eq!(r.offer(), if block == RTG { 3000 } else { 25000 });
        }
    }
    #[test]
    fn interpolation_uses_remaining_long_and_f32_endpoints() {
        for p in &table().rtg_pellets {
            let fresh = crate::radiation::rtg_hazard(&p.item)
                .unwrap()
                .radiation_millirad_s;
            assert_eq!(radiation_rate(&Stack::new(&p.item, 1)), Some(fresh));
            assert_eq!(radiation_rate(&tagged(&p.item, 0)), Some(0));
            assert_eq!(
                radiation_rate(&tagged(&p.item, p.lifespan_ticks / 2)),
                Some(fresh / 2)
            );
        }
    }
    #[test]
    fn inventory_rules_exhaust_positions_counts_kinds_and_metadata() {
        let a = Stack::new("ntm:scrap_nuclear", 1);
        let b = Stack::new("ntm:gem_rad", 1);
        for slot in 0..25 {
            assert!(!can_extract(RTG, slot));
            assert_eq!(can_extract(RADGEN, slot), (12..24).contains(&slot));
            assert!(!can_insert(RTG, &[], slot, &b));
            assert_eq!(can_insert_manual(RTG, slot), slot < 15);
            assert_eq!(can_insert_manual(RADGEN, slot), slot < 12);
            assert!(!can_insert(
                RADGEN,
                &[],
                slot,
                &Stack::new("minecraft:stone", 1)
            ));
            assert_eq!(can_insert(RADGEN, &[], slot, &a), slot < 12);
        }
        // Every vacancy/target pair and every smaller/equal/larger count,
        // same/different item and metadata. Different NBT doesn't affect rule.
        for slot in 0..12 {
            for other in 0..12 {
                let mut slots = vec![Some(a.clone()); 24];
                for count in [1, 2, 63, 64] {
                    slots[slot] = Some(Stack { count, ..a.clone() });
                    for smaller in [1, 2, 63, 64] {
                        for kind in 0..3 {
                            slots[other] = Some(match kind {
                                0 => Stack {
                                    count: smaller,
                                    custom: Some("{x:1}".into()),
                                    ..a.clone()
                                },
                                1 => Stack {
                                    count: smaller,
                                    ..b.clone()
                                },
                                _ => Stack {
                                    count: smaller,
                                    damage: 1,
                                    ..a.clone()
                                },
                            });
                            let current = slots[slot].as_ref().unwrap().count;
                            let expected = slots[..12].iter().flatten().all(|s| {
                                s.id != a.id || s.damage != a.damage || s.count >= current
                            });
                            assert_eq!(can_insert(RADGEN, &slots, slot, &a), expected);
                        }
                    }
                    slots[other] = None;
                    assert_eq!(can_insert(RADGEN, &slots, slot, &a), slot == other);
                }
            }
        }
    }
    #[test]
    fn native_policy_composes_origins_and_java_quick_move() {
        use crate::native::{
            SlotAction::{Extract, Insert},
            SlotOrigin::{Automation, Manual},
            slot_allows,
        };
        let a = Stack::new("ntm:gem_rad", 1);
        let junk = Stack::new("minecraft:stone", 1);
        for block in [RTG, RADGEN] {
            let filters = crate::native::slot_filters(block);
            // Both Java transferStackInSlot implementations merge ANY item
            // into the input range, so quick_move must add no item restriction.
            assert!(filters.iter().all(|f| f.quick_move.is_none()));
            let mut slots = vec![None; filters.len()];
            for slot in 0..filters.len() {
                for item in [&a, &junk] {
                    assert_eq!(
                        slot_allows(block, &slots, slot, item, Insert, Manual),
                        can_insert_manual(block, slot)
                    );
                    assert_eq!(
                        slot_allows(block, &slots, slot, item, Insert, Automation),
                        can_insert(block, &slots, slot, item)
                    );
                    assert_eq!(
                        slot_allows(block, &slots, slot, item, Extract, Automation),
                        can_extract(block, slot)
                    );
                    assert!(slot_allows(block, &slots, slot, item, Extract, Manual));
                }
            }
            slots[0] = Some(a.clone());
            assert!(slot_allows(block, &slots, 0, &junk, Insert, Manual));
            assert!(!slot_allows(block, &slots, 0, &a, Insert, Automation));
        }
    }
    #[test]
    fn native_container_separates_manual_slots_from_automation() {
        let r = crate::native::container(RTG).unwrap();
        assert_eq!(r.slots, 15);
        assert!(r.inputs.is_empty());
        assert!(r.outputs.is_empty());
        assert!(
            r.faces
                .iter()
                .all(|face| face == &(0..15).collect::<Vec<_>>())
        );
        let manual = crate::native::slot_filters(RTG);
        assert_eq!(manual.len(), 15);
        assert!(manual.iter().all(|s| s.accept.is_none() && !s.output));
        let g = crate::native::container(RADGEN).unwrap();
        assert_eq!(g.inputs, (0..12).collect::<Vec<_>>());
        assert_eq!(g.outputs, (12..24).collect::<Vec<_>>());
        let manual = crate::native::slot_filters(RADGEN);
        assert!(manual[..12].iter().all(|s| s.accept.is_none() && !s.output));
        assert!(manual[12..].iter().all(|s| s.output));
    }
    #[test]
    fn committed_phase_roundtrip_keeps_queues_pending_and_products() {
        let mut r = Radioisotope::of(RADGEN);
        r.slots_mut()[0] = Some(Stack::new("ntm:gem_rad", 1));
        r.tick();
        let json = serde_json::to_string(&r).unwrap();
        let mut resumed: Radioisotope = serde_json::from_str(&json).unwrap();
        resumed.finish();
        assert_eq!(resumed.offer(), 25000);
        let Radioisotope::Radgen(g) = &mut resumed else {
            panic!()
        };
        assert_eq!(g.queues[0].progress, 1);
        assert_eq!(g.queues[0].processing, Some(Stack::new("ntm:gem_rad", 1)));
        g.slots = vec![None; 24];
        g.step();
        g.finish();
        assert_eq!(g.power, 50000);
        assert_eq!(g.queues[0].progress, 2);
    }
}
