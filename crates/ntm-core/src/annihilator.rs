//! The annihilator (`machine_annihilator`), from `TileEntityMachineAnnihilator.java`,
//! `MachineAnnihilator.java`, `AnnihilatorSavedData.java` and `AnnihilatorRecipes.java`:
//! destroys items and fluids into named persistent world pools (`ANNIHILATOR_KEY`),
//! vents item and fluid radiation, monitors one key in slot 8, and pays out milestones.
//!
//! 11 slots: 0 input (trash), 1 fluid identifier (stays in slot via `type_from_identifier`),
//! 2..8 outputs (2 rows x 3 cols), 8 monitor key, 9 payout request, 10 payout item (`SlotCraftingOutput`).
//! One 2 500 000 mB tank, low fluid priority, 3 ports (`PORTS`), no energy.

use crate::{
    Pos,
    combustion::{identifier_type, type_from_identifier},
    fluid::{self, FluidNode, PRIORITY_LOW, Tank},
    machine::Logic,
    native::Container,
    oredict,
    radiation,
    registry,
    server::{
        container_bridge::{Slots, Slotted, Stack, world_cell},
        native_server::IDENTIFIER,
    },
    view::{self, ViewModel},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;

pub const BLOCK: &str = "ntm:machine_annihilator";
pub const SLOTS: usize = 11;
pub const INPUT: usize = 0;
pub const FLUID_ID: usize = 1;
pub const OUTPUTS: std::ops::Range<usize> = 2..8;
pub const MONITOR: usize = 8;
pub const REQUEST: usize = 9;
pub const PAYOUT: usize = 10;
pub const MAX_TANK: u32 = 2_500_000;
pub const DEFAULT_POOL: &str = "Recycling";
pub const CHIMNEY: Pos = [0, 9, 3];

/// `TileEntityMachineAnnihilator.getPorts`: three ports around the front end in the placer frame,
/// each open outwards (`dir`, `rot`, `-rot`).
pub const PORTS: &[(Pos, Pos)] = &[
    ([0, 0, -4], [0, 0, -1]),
    ([-1, 0, -3], [-1, 0, 0]),
    ([1, 0, -3], [1, 0, 0]),
];

pub fn port_cells(core: Pos, facing: Option<&str>) -> Vec<(Pos, Pos)> {
    let Some(f) = facing.and_then(registry::facing_dir) else {
        return Vec::new();
    };
    PORTS
        .iter()
        .map(|&(cell, face)| (world_cell(core, f, cell), world_cell([0, 0, 0], f, face)))
        .collect()
}

pub fn chimney_cell(core: Pos, facing: Option<&str>) -> Pos {
    facing
        .and_then(registry::facing_dir)
        .map(|f| world_cell(core, f, CHIMNEY))
        .unwrap_or([core[0], core[1] + CHIMNEY[1], core[2]])
}

mod u128_str {
    use super::*;
    use serde::de::{self, Visitor};
    use std::fmt;

    struct U128Visitor;
    impl<'de> Visitor<'de> for U128Visitor {
        type Value = u128;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a decimal string or unsigned integer")
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<u128, E> {
            Ok(u128::from(v))
        }
        fn visit_u128<E: de::Error>(self, v: u128) -> Result<u128, E> {
            Ok(v)
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<u128, E> {
            v.parse::<u128>().map_err(de::Error::custom)
        }
    }

    pub fn serialize<S: Serializer>(v: &u128, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u128, D::Error> {
        d.deserialize_any(U128Visitor)
    }
}

mod pool_items_map {
    use super::*;

    pub fn serialize<S: Serializer>(map: &BTreeMap<PoolKey, u128>, s: S) -> Result<S::Ok, S::Error> {
        let entries: Vec<(&PoolKey, String)> = map.iter().map(|(k, v)| (k, v.to_string())).collect();
        entries.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<PoolKey, u128>, D::Error> {
        let entries = Vec::<(PoolKey, String)>::deserialize(d)?;
        let mut out = BTreeMap::new();
        for (k, v) in entries {
            let n = v.parse::<u128>().map_err(serde::de::Error::custom)?;
            out.insert(k, n);
        }
        Ok(out)
    }
}

mod vec_map {
    use super::*;

    pub fn serialize<S, K, V>(map: &BTreeMap<K, V>, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        K: Serialize,
        V: Serialize,
    {
        let entries: Vec<(&K, &V)> = map.iter().collect();
        entries.serialize(s)
    }

    pub fn deserialize<'de, D, K, V>(d: D) -> Result<BTreeMap<K, V>, D::Error>
    where
        D: Deserializer<'de>,
        K: Ord + Deserialize<'de>,
        V: Deserialize<'de>,
    {
        let entries = Vec::<(K, V)>::deserialize(d)?;
        Ok(entries.into_iter().collect())
    }
}

/// Keys tracked inside each `AnnihilatorPool` (`AnnihilatorSavedData.AnnihilatorPool`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PoolKey {
    Item(String),
    Comp(String, u32),
    Fluid(String),
    Dict(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Milestone {
    #[serde(with = "u128_str")]
    pub amount: u128,
    pub payout: Stack,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    pub key: PoolKey,
    pub milestones: Vec<Milestone>,
}

/// `AnnihilatorRecipes`: in Java all 16 built-in recipes are gated on `GeneralConfig.enable528`
/// (off in K3-s2) and pay out `ntm:blueprints` (absent from `content/ids.json`), so the active
/// default milestone table is empty.
pub fn recipes() -> &'static [Recipe] {
    &[]
}

/// `AnnihilatorRecipes.getHighestPayoutFromRecipe`.
pub fn highest_payout_from_recipe(
    recipe: &Recipe,
    prev: Option<u128>,
    current: u128,
) -> Option<Stack> {
    let mut highest_yet: u128 = 0;
    let mut highest_payout: Option<&Stack> = None;
    for m in &recipe.milestones {
        if prev.is_some_and(|p| p >= m.amount) {
            continue;
        }
        if m.amount > highest_yet && current >= m.amount {
            highest_yet = m.amount;
            highest_payout = Some(&m.payout);
        }
    }
    highest_payout.cloned()
}

pub fn highest_payout_from_key(
    table: &[Recipe],
    key: &PoolKey,
    prev: Option<u128>,
    current: u128,
) -> Option<Stack> {
    table
        .iter()
        .find(|r| &r.key == key)
        .and_then(|r| highest_payout_from_recipe(r, prev, current))
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pool {
    #[serde(default, with = "pool_items_map")]
    pub items: BTreeMap<PoolKey, u128>,
}

impl Pool {
    pub fn increment(
        &mut self,
        key: PoolKey,
        amount: u128,
        always_payout: bool,
        table: &[Recipe],
    ) -> Option<Stack> {
        let prev = self.items.get(&key).copied();
        let next = prev.unwrap_or(0).saturating_add(amount);
        self.items.insert(key.clone(), next);
        let prev_cmp = if always_payout { None } else { prev };
        highest_payout_from_key(table, &key, prev_cmp, next)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingPush {
    pub seq: u64,
    pub pool: String,
    pub keys: Vec<PoolKey>,
    #[serde(with = "u128_str")]
    pub amount: u128,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pools {
    #[serde(default)]
    pub pools: BTreeMap<String, Pool>,
    #[serde(default, with = "vec_map")]
    pub applied_seq: BTreeMap<((String, Pos), u64), u64>,
}

impl Pools {
    pub fn get(&self, pool: &str, key: &PoolKey) -> u128 {
        self.pools
            .get(pool)
            .and_then(|p| p.items.get(key))
            .copied()
            .unwrap_or(0)
    }

    pub fn applied_seq(&self, site: &(String, Pos), generation: u64) -> u64 {
        self.applied_seq
            .get(&(site.clone(), generation))
            .copied()
            .unwrap_or(0)
    }

    /// Applies a tick's push into `self`, updating `applied_seq` and returning the highest
    /// milestone payout in Java precedence (`dict > comp > item`, or `fluid`).
    pub fn apply_push(
        &mut self,
        site: &(String, Pos),
        generation: u64,
        push: &PendingPush,
        always_payout: bool,
        table: &[Recipe],
    ) -> Option<Stack> {
        self.applied_seq.insert((site.clone(), generation), push.seq);
        let pool = self.pools.entry(push.pool.clone()).or_default();
        let mut item_payout = None;
        let mut comp_payout = None;
        let mut dict_payout = None;
        let mut fluid_payout = None;
        for key in &push.keys {
            let payout = pool.increment(key.clone(), push.amount, always_payout, table);
            match key {
                PoolKey::Item(_) => {
                    if payout.is_some() {
                        item_payout = payout;
                    }
                }
                PoolKey::Comp(_, _) => {
                    if payout.is_some() {
                        comp_payout = payout;
                    }
                }
                PoolKey::Dict(_) => {
                    if payout.is_some() {
                        dict_payout = payout;
                    }
                }
                PoolKey::Fluid(_) => {
                    if payout.is_some() {
                        fluid_payout = payout;
                    }
                }
            }
        }
        dict_payout.or(comp_payout).or(item_payout).or(fluid_payout)
    }

    /// Replays a WAL `PendingPush` during chunk adoption if its `seq` has not yet been applied.
    pub fn replay_push(&mut self, site: &(String, Pos), generation: u64, push: &PendingPush) {
        if push.seq <= self.applied_seq(site, generation) {
            return;
        }
        self.applied_seq.insert((site.clone(), generation), push.seq);
        let pool = self.pools.entry(push.pool.clone()).or_default();
        for key in &push.keys {
            let entry = pool.items.entry(key.clone()).or_insert(0);
            *entry = entry.saturating_add(push.amount);
        }
    }
}

pub fn item_pool_keys(id: &str, damage: u32) -> Vec<PoolKey> {
    let mut keys = vec![
        PoolKey::Item(id.to_string()),
        PoolKey::Comp(id.to_string(), damage),
    ];
    for name in oredict::names(id) {
        if !name.is_empty() {
            keys.push(PoolKey::Dict(name.clone()));
        }
    }
    keys
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Annihilator {
    pub slots: Slots,
    pub tank: Tank,
    pub pool: String,
    #[serde(with = "u128_str")]
    pub monitor: u128,
    #[serde(default)]
    pub seq: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending: Vec<PendingPush>,
}

impl Default for Annihilator {
    fn default() -> Self {
        Self {
            slots: vec![None; SLOTS],
            tank: Tank::new(MAX_TANK),
            pool: DEFAULT_POOL.into(),
            monitor: 0,
            seq: 0,
            pending: Vec::new(),
        }
    }
}

impl Annihilator {
    /// `tryAddPayout`: stacks onto an existing matching stack in `2..=7`, else first empty slot.
    pub fn try_add_payout(&mut self, payout: Stack) {
        for slot in &mut self.slots[OUTPUTS] {
            if let Some(cur) = slot
                && cur.id == payout.id
                && cur.damage == payout.damage
                && cur.custom == payout.custom
                && cur.count + payout.count <= 64
            {
                cur.count += payout.count;
                return;
            }
        }
        for slot in &mut self.slots[OUTPUTS] {
            if slot.is_none() {
                *slot = Some(payout);
                return;
            }
        }
    }

    fn set_payout_slot(&mut self, payout: Stack) {
        match &mut self.slots[PAYOUT] {
            None => self.slots[PAYOUT] = Some(payout),
            Some(cur)
                if cur.id == payout.id
                    && cur.damage == payout.damage
                    && cur.custom == payout.custom
                    && cur.count + payout.count <= 64 =>
            {
                cur.count += payout.count;
            }
            _ => {}
        }
    }

    /// Runs one server tick against `pool_mgr`, updating slots, tank, monitor, WAL `pending`,
    /// and returning `(world_pos, rad)` emissions for `chunk_rad_pending`.
    pub fn step_world(
        &mut self,
        pool_mgr: &mut Pools,
        site: &(String, Pos),
        generation: u64,
        facing: Option<&str>,
        confirmed_seq: u64,
        table: &[Recipe],
    ) -> Vec<(Pos, f32)> {
        self.pending.retain(|p| p.seq > confirmed_seq);
        let mut rads = Vec::new();
        if self.pool.is_empty() {
            return rads;
        }
        let core = site.1;
        let chimney = chimney_cell(core, facing);

        if let Some(stack) = self.slots[INPUT].take() {
            let rad = radiation::item_rad_s(&stack.id);
            if rad > 0.0 {
                rads.push((chimney, (rad * 5.0).min(1_000.0)));
            }
            self.seq += 1;
            let push = PendingPush {
                seq: self.seq,
                pool: self.pool.clone(),
                keys: item_pool_keys(&stack.id, stack.damage),
                amount: u128::from(stack.count),
            };
            let payout = pool_mgr.apply_push(site, generation, &push, false, table);
            self.pending.push(push);
            if let Some(p) = payout {
                self.try_add_payout(p);
            }
        }

        if self.tank.mb > 0 {
            let mb = self.tank.mb;
            self.tank.mb = 0;
            if !self.tank.fluid.is_empty() {
                let vent = fluid::get(&self.tank.fluid).map_or(0.0, |f| f.vent_radiation);
                if vent > 0.0 {
                    rads.push((core, (u64::from(mb) * 2) as f32 * vent));
                }
                self.seq += 1;
                let push = PendingPush {
                    seq: self.seq,
                    pool: self.pool.clone(),
                    keys: vec![PoolKey::Fluid(self.tank.fluid.clone())],
                    amount: u128::from(mb),
                };
                let payout = pool_mgr.apply_push(site, generation, &push, false, table);
                self.pending.push(push);
                if let Some(p) = payout {
                    self.try_add_payout(p);
                }
            }
        }

        if let Some(stack) = &self.slots[MONITOR] {
            let mon_key = match identifier_type(stack) {
                Some(f) => PoolKey::Fluid(f),
                None => PoolKey::Comp(stack.id.clone(), stack.damage),
            };
            self.monitor = pool_mgr.get(&self.pool, &mon_key);
        }

        if let Some(stack) = &mut self.slots[REQUEST]
            && stack.count > 0
        {
            let id = stack.id.clone();
            let damage = stack.damage;
            let rad = radiation::item_rad_s(&id);
            if rad > 0.0 {
                rads.push((chimney, (rad * 5.0).min(1_000.0)));
            }
            stack.count -= 1;
            if stack.count == 0 {
                self.slots[REQUEST] = None;
            }
            self.seq += 1;
            let push = PendingPush {
                seq: self.seq,
                pool: self.pool.clone(),
                keys: item_pool_keys(&id, damage),
                amount: 1,
            };
            let payout = pool_mgr.apply_push(site, generation, &push, true, table);
            self.pending.push(push);
            if let Some(p) = payout {
                self.set_payout_slot(p);
            }
        }

        rads
    }
}

impl Logic for Annihilator {
    fn tick(&mut self) -> bool {
        type_from_identifier(&mut self.tank, &self.slots, FLUID_ID, |_| true)
    }
    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(self)
    }
    fn level(&self, _max: u8) -> u8 {
        0
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Aniquilador")
            .text(format!("Pool: {}", self.pool), view::GREEN)
            .text(format!("Monitorado: {}", self.monitor), view::GREY)
    }
}

impl FluidNode for Annihilator {
    fn fluid_offer(&self, _fluid: &str) -> u64 {
        0
    }
    fn fluid_demand(&self, fluid: &str) -> u64 {
        if !self.tank.fluid.is_empty() && self.tank.fluid == fluid {
            u64::from(self.tank.free())
        } else {
            0
        }
    }
    fn fluid_priority(&self) -> u8 {
        PRIORITY_LOW
    }
    fn fluid_types(&self) -> Vec<String> {
        if self.tank.fluid.is_empty() {
            Vec::new()
        } else {
            vec![self.tank.fluid.clone()]
        }
    }
    fn fluid_give(&mut self, fluid: &str, mb: u64) {
        if !self.tank.fluid.is_empty() && self.tank.fluid == fluid {
            self.tank.fill(fluid, mb.min(u64::from(u32::MAX)) as u32);
        }
    }
    fn fluid_take(&mut self, _fluid: &str, _mb: u64) {}
}

impl Slotted for Annihilator {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        vec![self.monitor.min(i32::MAX as u128) as i32]
    }
}

pub fn container() -> Container {
    Container {
        slots: SLOTS,
        faces: [
            vec![0, 2, 3, 4, 5, 6, 7],
            vec![0, 2, 3, 4, 5, 6, 7],
            vec![0, 2, 3, 4, 5, 6, 7],
            vec![0, 2, 3, 4, 5, 6, 7],
            vec![0, 2, 3, 4, 5, 6, 7],
            vec![0, 2, 3, 4, 5, 6, 7],
        ],
        inputs: vec![INPUT],
        outputs: OUTPUTS.collect(),
        menu: Some("minecraft:generic_9x3"),
    }
}

pub fn allows(slot: usize, item: &str, insert: bool, manual: bool) -> bool {
    if insert {
        match slot {
            INPUT | MONITOR | REQUEST => true,
            FLUID_ID => item == IDENTIFIER,
            _ => false,
        }
    } else {
        manual || OUTPUTS.contains(&slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id_stack(fluid: &str) -> Stack {
        Stack {
            id: IDENTIFIER.into(),
            count: 1,
            damage: 0,
            custom: Some(format!(r#"{{ntm_fluid:"{fluid}"}}"#)),
        }
    }

    fn site() -> (String, Pos) {
        ("minecraft:overworld".into(), [10, 64, 20])
    }

    #[test]
    fn items_increment_item_comp_and_oredict_and_monitor_tracks_comp_or_fluid() {
        let mut a = Annihilator::default();
        let mut pools = Pools::default();
        let s = site();

        a.slots[INPUT] = Some(Stack::new("minecraft:iron_ore", 5));
        a.slots[MONITOR] = Some(Stack::new("minecraft:iron_ore", 1));
        a.step_world(&mut pools, &s, 0, Some("south"), 0, &[]);

        assert!(a.slots[INPUT].is_none());
        assert_eq!(pools.get(DEFAULT_POOL, &PoolKey::Item("minecraft:iron_ore".into())), 5);
        assert_eq!(pools.get(DEFAULT_POOL, &PoolKey::Comp("minecraft:iron_ore".into(), 0)), 5);
        assert_eq!(pools.get(DEFAULT_POOL, &PoolKey::Dict("oreIron".into())), 5);
        assert_eq!(a.monitor, 5);
        assert_eq!(a.seq, 1);
        assert_eq!(a.pending.len(), 1);

        // Removing monitor item retains last monitorBigInt (Java behavior).
        a.slots[MONITOR] = None;
        a.slots[INPUT] = Some(Stack::new("minecraft:iron_ore", 3));
        a.step_world(&mut pools, &s, 0, Some("south"), 0, &[]);
        assert_eq!(a.monitor, 5);

        // Fluid identifier sets tank without leaving slot 1, and drains into pool.
        a.slots[FLUID_ID] = Some(id_stack("water"));
        assert!(a.tick());
        assert_eq!(a.tank.fluid, "water");
        assert!(a.slots[FLUID_ID].is_some(), "identifier stays in slot 1");

        a.fluid_give("water", 1_250);
        assert_eq!(a.tank.mb, 1_250);

        a.slots[MONITOR] = Some(id_stack("water"));
        a.step_world(&mut pools, &s, 0, Some("south"), 0, &[]);
        assert_eq!(a.tank.mb, 0);
        assert_eq!(pools.get(DEFAULT_POOL, &PoolKey::Fluid("water".into())), 1_250);
        assert_eq!(a.monitor, 1_250);
    }

    #[test]
    fn identifier_switch_and_untyped_refusal() {
        let mut a = Annihilator::default();
        assert_eq!(a.fluid_demand(""), 0);
        a.fluid_give("", 100);
        assert_eq!(a.tank.mb, 0);
        assert_eq!(a.fluid_demand("water"), 0);
        a.fluid_give("water", 500);
        assert_eq!(a.tank.mb, 0);
        assert!(a.fluid_types().is_empty());
        assert_eq!(a.fluid_priority(), PRIORITY_LOW);

        a.slots[FLUID_ID] = Some(id_stack("water"));
        assert!(a.tick());
        a.fluid_give("water", 500);
        assert_eq!(a.tank.mb, 500);
        assert_eq!(a.fluid_demand("lava"), 0);
        assert_eq!(a.fluid_demand("water"), MAX_TANK as u64 - 500);

        // Switching identifier empties tank and changes type.
        a.slots[FLUID_ID] = Some(id_stack("lava"));
        assert!(a.tick());
        assert_eq!(a.tank.fluid, "lava");
        assert_eq!(a.tank.mb, 0);

        // Bare identifier untypes tank.
        a.slots[FLUID_ID] = Some(Stack::new(IDENTIFIER, 1));
        assert!(a.tick());
        assert_eq!(a.tank.fluid, "");
    }

    #[test]
    fn milestone_payout_precedence_and_request_slot() {
        let table = vec![
            Recipe {
                key: PoolKey::Item("minecraft:iron_ore".into()),
                milestones: vec![Milestone { amount: 4, payout: Stack::new("minecraft:coal", 1) }],
            },
            Recipe {
                key: PoolKey::Comp("minecraft:iron_ore".into(), 0),
                milestones: vec![Milestone { amount: 4, payout: Stack::new("minecraft:gold_ingot", 1) }],
            },
            Recipe {
                key: PoolKey::Dict("oreIron".into()),
                milestones: vec![
                    Milestone { amount: 4, payout: Stack::new("minecraft:diamond", 1) },
                    Milestone { amount: 8, payout: Stack::new("minecraft:emerald", 2) },
                ],
            },
        ];
        let mut a = Annihilator::default();
        let mut pools = Pools::default();
        let s = site();

        // Dict payout wins over Comp and Item; crossing both 4 and 8 picks highest (emerald x2).
        a.slots[INPUT] = Some(Stack::new("minecraft:iron_ore", 8));
        a.step_world(&mut pools, &s, 0, Some("south"), 0, &table);
        assert_eq!(a.slots[2], Some(Stack::new("minecraft:emerald", 2)));

        // Already past 8: feeding 1 more via input yields no duplicate payout.
        a.slots[INPUT] = Some(Stack::new("minecraft:iron_ore", 1));
        a.step_world(&mut pools, &s, 0, Some("south"), 0, &table);
        assert_eq!(a.slots[2], Some(Stack::new("minecraft:emerald", 2)));
        assert!(a.slots[3].is_none());

        // Exact boundary (prev == m.amount), 0-milestone refusal, and tie-break (`>` keeps first).
        let boundary_table = vec![Recipe {
            key: PoolKey::Item("minecraft:gold_ore".into()),
            milestones: vec![
                Milestone { amount: 0, payout: Stack::new("minecraft:dirt", 1) },
                Milestone { amount: 4, payout: Stack::new("minecraft:diamond", 1) },
                Milestone { amount: 4, payout: Stack::new("minecraft:apple", 1) },
            ],
        }];
        a.slots[INPUT] = Some(Stack::new("minecraft:gold_ore", 4));
        a.step_world(&mut pools, &s, 0, Some("south"), 0, &boundary_table);
        assert_eq!(a.slots[3], Some(Stack::new("minecraft:diamond", 1)));
        a.slots[INPUT] = Some(Stack::new("minecraft:gold_ore", 1));
        a.step_world(&mut pools, &s, 0, Some("south"), 0, &boundary_table);
        assert!(a.slots[4].is_none());

        // Payout stacking hits exact 64 ceiling (`<= 64`) without overflowing to next slot.
        a.try_add_payout(Stack::new("minecraft:emerald", 62));
        assert_eq!(a.slots[2], Some(Stack::new("minecraft:emerald", 64)));
        assert!(a.slots[4].is_none());

        // Request slot (`always_payout = true`) consumes 1 per tick and pays out into slot 10.
        a.slots[REQUEST] = Some(Stack::new("minecraft:iron_ore", 2));
        a.step_world(&mut pools, &s, 0, Some("south"), 0, &table);
        assert_eq!(a.slots[REQUEST], Some(Stack::new("minecraft:iron_ore", 1)));
        assert_eq!(a.slots[PAYOUT], Some(Stack::new("minecraft:emerald", 2)));

        a.step_world(&mut pools, &s, 0, Some("south"), 0, &table);
        assert!(a.slots[REQUEST].is_none());
        assert_eq!(a.slots[PAYOUT], Some(Stack::new("minecraft:emerald", 4)));
        a.set_payout_slot(Stack::new("minecraft:emerald", 60));
        assert_eq!(a.slots[PAYOUT], Some(Stack::new("minecraft:emerald", 64)));
    }

    #[test]
    fn radiation_emissions_use_chimney_for_items_and_core_for_fluids() {
        let mut a = Annihilator::default();
        let mut pools = Pools::default();
        let s = site();

        // Uranium ingot has radiation_rad_s = 0.35 -> 0.35 * 5 = 1.75 RAD at chimney (regardless of count).
        a.slots[INPUT] = Some(Stack::new("ntm:ingot_uranium", 64));
        a.slots[FLUID_ID] = Some(id_stack("sas3"));
        a.tick();
        let expected_vent = fluid::get("sas3").unwrap().vent_radiation;
        assert!(expected_vent > 0.0);
        a.fluid_give("sas3", 100);

        let rads = a.step_world(&mut pools, &s, 0, Some("south"), 0, &[]);
        // Facing south -> fwd = (0, 0, -1) -> chimney = core + [0, 9, -3].
        assert_eq!(rads[0].0, [10, 73, 17]);
        assert!((rads[0].1 - 1.75).abs() < 1e-4, "{rads:?}");
        assert_eq!(rads[1].0, s.1);
        assert!((rads[1].1 - 200.0 * expected_vent).abs() < 1e-6, "{rads:?}");

        // Request slot radioactive item also vents at chimney; empty pool name stops ticking.
        a.slots[REQUEST] = Some(Stack::new("ntm:ingot_uranium", 1));
        let req_rads = a.step_world(&mut pools, &s, 0, Some("south"), 0, &[]);
        assert_eq!(req_rads.len(), 1);
        assert_eq!(req_rads[0].0, [10, 73, 17]);
        assert!((req_rads[0].1 - 1.75).abs() < 1e-4, "{req_rads:?}");

        a.pool.clear();
        a.slots[INPUT] = Some(Stack::new("ntm:ingot_uranium", 1));
        assert!(a.step_world(&mut pools, &s, 0, Some("south"), 0, &[]).is_empty());
        assert!(a.slots[INPUT].is_some());
    }

    #[test]
    fn wal_replay_is_idempotent_and_respects_confirmed_seq() {
        let mut a = Annihilator::default();
        let mut pools = Pools::default();
        let s = site();
        a.slots[INPUT] = Some(Stack::new("minecraft:cobblestone", 12));
        a.step_world(&mut pools, &s, 1, Some("south"), 0, &[]);
        assert_eq!(a.pending.len(), 1);
        let push = a.pending[0].clone();
        a.step_world(&mut pools, &s, 1, Some("south"), 1, &[]);
        assert!(a.pending.is_empty());

        // Fresh Pools (simulating reload after chunks_written before ANNIHILATOR_KEY update).
        let mut replayed = Pools::default();
        replayed.replay_push(&s, 1, &push);
        assert_eq!(replayed.get(DEFAULT_POOL, &PoolKey::Item("minecraft:cobblestone".into())), 12);
        // Replaying the same seq again is a no-op.
        replayed.replay_push(&s, 1, &push);
        assert_eq!(replayed.get(DEFAULT_POOL, &PoolKey::Item("minecraft:cobblestone".into())), 12);
        // Different generation at same pos applies independently.
        replayed.replay_push(&s, 2, &push);
        assert_eq!(replayed.get(DEFAULT_POOL, &PoolKey::Item("minecraft:cobblestone".into())), 24);
    }

    #[test]
    fn slot_rules_match_java_contract() {
        assert!(allows(INPUT, "minecraft:stone", true, false));
        assert!(allows(FLUID_ID, IDENTIFIER, true, true));
        assert!(!allows(FLUID_ID, "minecraft:stone", true, true));
        assert!(allows(MONITOR, "minecraft:stone", true, false));
        assert!(allows(REQUEST, "minecraft:stone", true, false));
        for out in OUTPUTS {
            assert!(!allows(out, "minecraft:stone", true, true));
            assert!(allows(out, "minecraft:stone", false, false));
        }
        assert!(!allows(PAYOUT, "minecraft:stone", true, true));
        assert!(allows(PAYOUT, "minecraft:stone", false, true));
        assert!(!allows(PAYOUT, "minecraft:stone", false, false));
        assert!(!allows(INPUT, "minecraft:stone", false, false));
        assert!(allows(INPUT, "minecraft:stone", false, true));
    }
}
