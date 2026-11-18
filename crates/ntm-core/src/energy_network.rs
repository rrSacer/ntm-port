//! Advanced HE network (HBM's `PowerNetMK2` family), as pure rules.
//!
//! This is the logic of the 14 `Inert` network blocks and the 12 `battery_pack`
//! variants (docs/content-energy-storage-network.md), ported from the Java at
//! the pinned HBM revision and proved against it bit for bit by
//! `tools/energy-net-oracle` (the fixture it writes is read by
//! `tests/energy_network_tests.rs`):
//!
//! - [`PowerNet::update`]: the exact `PowerNetMK2.update()` distribution,
//!   with its priority groups, `double` weights, `long` truncation, `long`
//!   overflow wrap, the cumulative `energyUsed` quirk and the random rounding
//!   compensation ([`JavaRandom`] is `java.util.Random`).
//! - [`BatteryRedd`]: the REDD, whose store is a `BigInteger` ([`BigInt`]).
//! - [`BatterySocket`]: the socket, which delegates to the pack in its slot.
//! - [`BatteryBase`]: redstone modes, priority, buttons and the comparator.
//! - [`WireKind`]: connectors and pylons (ranges, connection types, mounts,
//!   link validation, node faces); [`NetBlock`] maps the 14 block ids.
//!
//! Nothing here touches the tick, the save or the world: wiring this to the
//! server tick (entities in a network, redstone sampling, persistence of the
//! REDD's `power` byte array, the wire list and the colour) is left for after
//! the Onda 0 merge.

use std::cell::Cell;
use std::cmp::Ordering;
use std::fmt;
use std::sync::OnceLock;

use crate::Pos;

// ---------------------------------------------------------------- java.util.Random

/// `java.util.Random` (the 48-bit LCG), which `NodeNet.rand` is.
#[derive(Clone, Debug)]
pub struct JavaRandom {
    seed: u64,
}

impl JavaRandom {
    const MULT: u64 = 0x5DEE_CE66D;
    const MASK: u64 = (1 << 48) - 1;

    pub fn new(seed: i64) -> Self {
        JavaRandom {
            seed: (seed as u64 ^ Self::MULT) & Self::MASK,
        }
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = (self.seed.wrapping_mul(Self::MULT).wrapping_add(0xB)) & Self::MASK;
        (self.seed >> (48 - bits)) as i64 as i32
    }

    /// `nextInt(bound)`; `bound` must be positive.
    pub fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "bound must be positive");
        if bound.count_ones() == 1 {
            return ((bound as i64 * self.next(31) as i64) >> 31) as i32;
        }
        loop {
            let bits = self.next(31);
            let val = bits % bound;
            if bits.wrapping_sub(val).wrapping_add(bound - 1) >= 0 {
                return val;
            }
        }
    }
}

// ---------------------------------------------------------------- BigInteger

/// The part of `java.math.BigInteger` the REDD uses: sign and magnitude, with
/// no ceiling. (Truncating division toward zero, `longValue` wrap and a
/// correctly rounded `doubleValue` are the Java behaviours.)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BigInt {
    neg: bool,
    /// Little-endian 32-bit limbs, no trailing zero limb; empty is zero.
    mag: Vec<u32>,
}

fn trim(v: &mut Vec<u32>) {
    while v.last() == Some(&0) {
        v.pop();
    }
}

fn cmp_mag(a: &[u32], b: &[u32]) -> Ordering {
    a.len()
        .cmp(&b.len())
        .then_with(|| a.iter().rev().cmp(b.iter().rev()))
}

fn add_mag(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(a.len().max(b.len()) + 1);
    let mut carry = 0u64;
    for i in 0..a.len().max(b.len()) {
        let s = *a.get(i).unwrap_or(&0) as u64 + *b.get(i).unwrap_or(&0) as u64 + carry;
        out.push(s as u32);
        carry = s >> 32;
    }
    if carry > 0 {
        out.push(carry as u32);
    }
    out
}

/// `a - b`, with `a >= b`.
fn sub_mag(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(a.len());
    let mut borrow = 0i64;
    for (i, &limb) in a.iter().enumerate() {
        let mut d = limb as i64 - *b.get(i).unwrap_or(&0) as i64 - borrow;
        borrow = 0;
        if d < 0 {
            d += 1 << 32;
            borrow = 1;
        }
        out.push(d as u32);
    }
    trim(&mut out);
    out
}

impl BigInt {
    pub fn zero() -> Self {
        BigInt {
            neg: false,
            mag: Vec::new(),
        }
    }

    pub fn from_i64(v: i64) -> Self {
        let m = v.unsigned_abs();
        let mut mag = vec![m as u32, (m >> 32) as u32];
        trim(&mut mag);
        BigInt { neg: v < 0, mag }
    }

    /// Decimal text, optionally signed (`BigInteger(String)`).
    pub fn parse(text: &str) -> Option<Self> {
        let (neg, digits) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text.strip_prefix('+').unwrap_or(text)),
        };
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let mut mag: Vec<u32> = Vec::new();
        for b in digits.bytes() {
            let mut carry = (b - b'0') as u64;
            for limb in mag.iter_mut() {
                let v = *limb as u64 * 10 + carry;
                *limb = v as u32;
                carry = v >> 32;
            }
            if carry > 0 {
                mag.push(carry as u32);
            }
        }
        trim(&mut mag);
        Some(BigInt {
            neg: neg && !mag.is_empty(),
            mag,
        })
    }

    /// `toByteArray()`: big-endian two's complement, the shortest that holds
    /// the sign (this is how the REDD saves `power` in NBT).
    pub fn to_java_bytes(&self) -> Vec<u8> {
        let mut be: Vec<u8> = self
            .mag
            .iter()
            .rev()
            .flat_map(|l| l.to_be_bytes())
            .collect();
        let lead = be.iter().take_while(|&&b| b == 0).count();
        be.drain(..lead);
        if !self.neg {
            if be.first().is_none_or(|b| b & 0x80 != 0) {
                be.insert(0, 0);
            }
            return be;
        }
        // -m = !(m - 1)
        let minus_one = BigInt {
            neg: false,
            mag: sub_mag(&self.mag, &[1]),
        };
        let mut out: Vec<u8> = minus_one
            .mag
            .iter()
            .rev()
            .flat_map(|l| l.to_be_bytes())
            .collect();
        let lead = out.iter().take_while(|&&b| b == 0).count();
        out.drain(..lead);
        for b in out.iter_mut() {
            *b = !*b;
        }
        if out.first().is_none_or(|b| b & 0x80 == 0) {
            out.insert(0, 0xFF);
        }
        out
    }

    /// `new BigInteger(byte[])`; `None` for an empty array (Java throws).
    pub fn from_java_bytes(bytes: &[u8]) -> Option<BigInt> {
        let first = *bytes.first()?;
        let neg = first & 0x80 != 0;
        let mut be: Vec<u8> = bytes.to_vec();
        if neg {
            for b in be.iter_mut() {
                *b = !*b;
            }
        }
        let mut mag: Vec<u32> = Vec::new();
        for chunk in be.rchunks(4) {
            let mut limb = [0u8; 4];
            limb[4 - chunk.len()..].copy_from_slice(chunk);
            mag.push(u32::from_be_bytes(limb));
        }
        trim(&mut mag);
        let mut out = BigInt { neg: false, mag };
        if neg {
            out = out.add_long(1);
            out.neg = !out.mag.is_empty();
        }
        Some(out)
    }

    pub fn is_zero(&self) -> bool {
        self.mag.is_empty()
    }

    pub fn is_negative(&self) -> bool {
        self.neg
    }

    pub fn add(&self, other: &BigInt) -> BigInt {
        if self.neg == other.neg {
            return BigInt {
                neg: self.neg,
                mag: add_mag(&self.mag, &other.mag),
            };
        }
        match cmp_mag(&self.mag, &other.mag) {
            Ordering::Equal => BigInt::zero(),
            Ordering::Greater => BigInt {
                neg: self.neg,
                mag: sub_mag(&self.mag, &other.mag),
            },
            Ordering::Less => BigInt {
                neg: other.neg,
                mag: sub_mag(&other.mag, &self.mag),
            },
        }
    }

    pub fn negate(&self) -> BigInt {
        BigInt {
            neg: !self.neg && !self.mag.is_empty(),
            mag: self.mag.clone(),
        }
    }

    pub fn subtract(&self, other: &BigInt) -> BigInt {
        self.add(&other.negate())
    }

    pub fn add_long(&self, v: i64) -> BigInt {
        self.add(&BigInt::from_i64(v))
    }

    pub fn subtract_long(&self, v: i64) -> BigInt {
        self.subtract(&BigInt::from_i64(v))
    }

    /// `divide(BigInteger.valueOf(2))`: truncates toward zero.
    pub fn half(&self) -> BigInt {
        let mut mag = self.mag.clone();
        let mut carry = 0u32;
        for limb in mag.iter_mut().rev() {
            let next = *limb & 1;
            *limb = (*limb >> 1) | (carry << 31);
            carry = next;
        }
        trim(&mut mag);
        BigInt {
            neg: self.neg && !mag.is_empty(),
            mag,
        }
    }

    pub fn min_of(&self, other: &BigInt) -> BigInt {
        if self.cmp(other) == Ordering::Greater {
            other.clone()
        } else {
            self.clone()
        }
    }

    /// `longValue()`: the low 64 bits of the two's complement.
    pub fn long_value(&self) -> i64 {
        let lo = *self.mag.first().unwrap_or(&0) as u64;
        let hi = *self.mag.get(1).unwrap_or(&0) as u64;
        let m = lo | (hi << 32);
        (if self.neg { m.wrapping_neg() } else { m }) as i64
    }

    /// The value as a `long`, clamped to its range (for screens that hold one).
    pub fn saturating_long(&self) -> i64 {
        let m = u128::from(*self.mag.first().unwrap_or(&0)) | (u128::from(*self.mag.get(1).unwrap_or(&0)) << 32);
        if self.mag.len() > 2 || m > i64::MAX as u128 + u128::from(self.neg) {
            return if self.neg { i64::MIN } else { i64::MAX };
        }
        if self.neg { (m as i128).wrapping_neg() as i64 } else { m as i64 }
    }

    /// `doubleValue()`: round to nearest even, infinity beyond the range.
    pub fn double_value(&self) -> f64 {
        let bits = match self.mag.last() {
            None => return 0.0,
            Some(top) => (self.mag.len() - 1) * 32 + (32 - top.leading_zeros() as usize),
        };
        let magnitude = if bits <= 64 {
            let lo = self.mag[0] as u64;
            let hi = *self.mag.get(1).unwrap_or(&0) as u64;
            (lo | (hi << 32)) as f64
        } else {
            let shift = bits - 64;
            let mut top = 0u64;
            for k in 0..64 {
                let bit = shift + k;
                if (self.mag[bit / 32] >> (bit % 32)) & 1 == 1 {
                    top |= 1 << k;
                }
            }
            let sticky = (0..shift).any(|bit| (self.mag[bit / 32] >> (bit % 32)) & 1 == 1);
            let m = top | sticky as u64;
            if shift > 1100 {
                f64::INFINITY
            } else {
                m as f64 * 2f64.powi(shift as i32)
            }
        };
        if self.neg {
            -magnitude
        } else {
            magnitude
        }
    }
}

impl PartialOrd for BigInt {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BigInt {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.neg, other.neg) {
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            (false, false) => cmp_mag(&self.mag, &other.mag),
            (true, true) => cmp_mag(&other.mag, &self.mag),
        }
    }
}

impl fmt::Display for BigInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.mag.is_empty() {
            return f.write_str("0");
        }
        let mut chunks: Vec<u32> = Vec::new();
        let mut mag = self.mag.clone();
        while !mag.is_empty() {
            let mut rem = 0u64;
            for limb in mag.iter_mut().rev() {
                let cur = (rem << 32) | *limb as u64;
                *limb = (cur / 1_000_000_000) as u32;
                rem = cur % 1_000_000_000;
            }
            trim(&mut mag);
            chunks.push(rem as u32);
        }
        let mut out = String::new();
        if self.neg {
            out.push('-');
        }
        out.push_str(&chunks.pop().unwrap().to_string());
        for c in chunks.iter().rev() {
            out.push_str(&format!("{c:09}"));
        }
        f.write_str(&out)
    }
}

// ---------------------------------------------------------------- directions

/// `ForgeDirection`, in its ordinal order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    Down,
    Up,
    North,
    South,
    West,
    East,
    Unknown,
}

impl Dir {
    pub const ALL: [Dir; 7] = [
        Dir::Down,
        Dir::Up,
        Dir::North,
        Dir::South,
        Dir::West,
        Dir::East,
        Dir::Unknown,
    ];

    /// `ForgeDirection.getOrientation(id)`: anything outside 0..5 is `Unknown`.
    pub fn from_orientation(id: i32) -> Dir {
        if (0..6).contains(&id) {
            Dir::ALL[id as usize]
        } else {
            Dir::Unknown
        }
    }

    pub fn ordinal(self) -> usize {
        self as usize
    }

    pub fn offset(self) -> [i32; 3] {
        match self {
            Dir::Down => [0, -1, 0],
            Dir::Up => [0, 1, 0],
            Dir::North => [0, 0, -1],
            Dir::South => [0, 0, 1],
            Dir::West => [-1, 0, 0],
            Dir::East => [1, 0, 0],
            Dir::Unknown => [0, 0, 0],
        }
    }

    pub fn opposite(self) -> Dir {
        const OPPOSITES: [usize; 7] = [1, 0, 3, 2, 5, 4, 6];
        Dir::ALL[OPPOSITES[self.ordinal()]]
    }

    /// `getRotation(ForgeDirection.UP)`.
    pub fn rotate_up(self) -> Dir {
        const ROW_UP: [usize; 7] = [0, 1, 5, 4, 2, 3, 6];
        Dir::ALL[ROW_UP[self.ordinal()]]
    }
}

fn shifted(p: Pos, dx: i32, dz: i32) -> Pos {
    [p[0] + dx, p[1], p[2] + dz]
}

// ---------------------------------------------------------------- PowerNetMK2

/// `IEnergyReceiverMK2.ConnectionPriority`, in ordinal order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConnectionPriority {
    Lowest,
    Low,
    Normal,
    High,
    Highest,
}

impl ConnectionPriority {
    pub const ALL: [ConnectionPriority; 5] = [
        ConnectionPriority::Lowest,
        ConnectionPriority::Low,
        ConnectionPriority::Normal,
        ConnectionPriority::High,
        ConnectionPriority::Highest,
    ];

    pub fn ordinal(self) -> usize {
        self as usize
    }

    /// `EnumUtil.grabEnumSafely`: `abs(index % len)`, so out-of-range wraps.
    pub fn from_index_safely(index: i32) -> Self {
        Self::ALL[(index % 5).unsigned_abs() as usize]
    }
}

/// A provider and/or receiver in a network (`IEnergyProviderMK2` +
/// `IEnergyReceiverMK2`). The defaults are the interfaces' `default` methods.
pub trait EnergyEntity {
    /// `ILoadedTile.isLoaded`: unloaded entries are dropped from the network.
    fn is_loaded(&self) -> bool {
        true
    }
    fn power(&self) -> i64;
    fn set_power(&mut self, power: i64);
    fn max_power(&self) -> i64;
    fn provider_speed(&self) -> i64 {
        self.max_power()
    }
    fn receiver_speed(&self) -> i64 {
        self.max_power()
    }
    fn priority(&self) -> ConnectionPriority {
        ConnectionPriority::Normal
    }
    /// Returns what did not fit.
    fn transfer_power(&mut self, power: i64) -> i64 {
        let current = self.power();
        if power.wrapping_add(current) <= self.max_power() {
            self.set_power(power.wrapping_add(current));
            return 0;
        }
        let capacity = self.max_power().wrapping_sub(current);
        let overshoot = power.wrapping_sub(capacity);
        let max = self.max_power();
        self.set_power(max);
        overshoot
    }
    fn use_power(&mut self, power: i64) {
        let p = self.power().wrapping_sub(power);
        self.set_power(p);
    }
}

/// Java `Math.max(double, double)`: NaN wins.
fn jmax(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

/// Java `Math.min(double, double)`: NaN wins.
fn jmin(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.min(b)
    }
}

/// One power network: the providers and receivers (indices into the entity
/// slice handed to [`PowerNet::update`], in iteration order) and the tracker.
#[derive(Clone, Debug, Default)]
pub struct PowerNet {
    providers: Vec<usize>,
    receivers: Vec<usize>,
    /// Energy that left the providers since the last `resetTrackers`.
    pub energy_tracker: i64,
}

impl PowerNet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn providers(&self) -> &[usize] {
        &self.providers
    }

    pub fn receivers(&self) -> &[usize] {
        &self.receivers
    }

    pub fn add_provider(&mut self, entity: usize) {
        if !self.providers.contains(&entity) {
            self.providers.push(entity);
        }
    }

    pub fn remove_provider(&mut self, entity: usize) {
        self.providers.retain(|&e| e != entity);
    }

    pub fn add_receiver(&mut self, entity: usize) {
        if !self.receivers.contains(&entity) {
            self.receivers.push(entity);
        }
    }

    pub fn remove_receiver(&mut self, entity: usize) {
        self.receivers.retain(|&e| e != entity);
    }

    pub fn reset_trackers(&mut self) {
        self.energy_tracker = 0;
    }

    /// `PowerNetMK2.update()`, line for line. `rng` is the global
    /// `NodeNet.rand`. Long arithmetic wraps like Java's.
    pub fn update(&mut self, rng: &mut JavaRandom, nodes: &mut [&mut dyn EnergyEntity]) {
        if self.providers.is_empty() || self.receivers.is_empty() {
            return;
        }

        // sum up available power
        let mut providers: Vec<(usize, i64)> = Vec::new();
        let mut total_power_in_network = 0i64;
        let mut power_available = 0i64;
        let mut kept = Vec::with_capacity(self.providers.len());
        for &entry in &self.providers {
            if !nodes[entry].is_loaded() {
                continue;
            }
            kept.push(entry);
            let src = nodes[entry].power().min(nodes[entry].provider_speed());
            if src > 0 {
                providers.push((entry, src));
                power_available = power_available.wrapping_add(src);
                total_power_in_network = total_power_in_network.wrapping_add(nodes[entry].power());
            }
        }
        self.providers = kept;

        // sum up total demand, categorized by priority
        let mut receivers: [Vec<(usize, i64)>; 5] = Default::default();
        let mut demand = [0i64; 5];
        let mut total_demand = 0i64;
        let mut kept = Vec::with_capacity(self.receivers.len());
        for &entry in &self.receivers {
            if !nodes[entry].is_loaded() {
                continue;
            }
            kept.push(entry);
            let rec = nodes[entry]
                .max_power()
                .wrapping_sub(nodes[entry].power())
                .min(nodes[entry].receiver_speed());
            if rec > 0 {
                let p = nodes[entry].priority().ordinal();
                receivers[p].push((entry, rec));
                demand[p] = demand[p].wrapping_add(rec);
                total_demand = total_demand.wrapping_add(rec);
            }
        }
        self.receivers = kept;

        let mut to_transfer = power_available.min(total_demand);
        let mut energy_used = 0i64;

        // add power to receivers, ordered by priority
        for i in (0..5).rev() {
            let priority_demand = demand[i];
            for &(entry, wanted) in &receivers[i] {
                let weight = wanted as f64 / priority_demand as f64;
                let to_send = jmin(jmax(to_transfer as f64 * weight, 0.0), wanted as f64) as i64;
                // leftovers are subtracted from the intended amount to use up
                let leftover = nodes[entry].transfer_power(to_send);
                energy_used = energy_used.wrapping_add(to_send.wrapping_sub(leftover));
            }
            // the original subtracts the *cumulative* energyUsed here
            to_transfer = to_transfer.wrapping_sub(energy_used);
        }

        let mut leftover = energy_used;

        // remove power from providers
        for &(entry, available) in &providers {
            let weight = available as f64 / power_available as f64;
            let to_use = jmax(energy_used as f64 * weight, 0.0) as i64;
            nodes[entry].use_power(to_use);
            leftover = leftover.wrapping_sub(to_use);
        }

        // rounding error compensation: surplus that was not used is taken
        // from random providers
        let mut iterations_left = 100;
        while iterations_left > 0 && leftover > 0 && !providers.is_empty() {
            iterations_left -= 1;
            let selected = rng.next_int(providers.len() as i32) as usize;
            let scapegoat = providers[selected].0;
            let to_use = leftover.min(nodes[scapegoat].power());
            nodes[scapegoat].use_power(to_use);
            leftover = leftover.wrapping_sub(to_use);
        }

        let mut power_available_after = 0i64;
        for &entry in &self.providers {
            let src = nodes[entry].power().max(0);
            if src > 0 {
                power_available_after = power_available_after.wrapping_add(src);
            }
        }

        self.energy_tracker = self
            .energy_tracker
            .wrapping_add(total_power_in_network.wrapping_sub(power_available_after));
    }
}

// ---------------------------------------------------------------- batteries

/// The slot's item, as far as the energy code can tell.
///
/// Closed on purpose: the Java accepts any `IBatteryItem` (cube_power, potato
/// battery, memory, `battery_sc`, fusion core, powered tools and armour...), but
/// none of those is in the catalog yet, so only the 12 packs and the creative
/// battery exist here. When one enters the catalog this becomes a spec
/// (capacity, rates, charge) or a trait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotItem {
    /// `battery_pack` with this metadata (`EnumUtil.grabEnumSafely` picks the
    /// variant, so out-of-range values wrap).
    Pack(i32),
    /// `battery_creative`.
    Creative,
    /// Anything that is not an `IBatteryItem`.
    Other,
}

/// An item stack in a battery slot. `charge` is the NBT long `charge`;
/// `None` means the stack has no tag compound yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemSlot {
    pub item: SlotItem,
    charge: Cell<Option<i64>>,
}

impl ItemSlot {
    pub fn new(item: SlotItem, charge: Option<i64>) -> Self {
        ItemSlot {
            item,
            charge: Cell::new(charge),
        }
    }

    /// The raw NBT state (`None` = no tag).
    pub fn tag(&self) -> Option<i64> {
        self.charge.get()
    }

    pub fn is_battery(&self) -> bool {
        self.item != SlotItem::Other
    }

    pub fn max_charge(&self) -> i64 {
        match self.item {
            SlotItem::Pack(d) => BatteryPack::from_damage(d).spec().capacity,
            SlotItem::Creative => i64::MAX,
            SlotItem::Other => 0,
        }
    }

    pub fn charge_rate(&self) -> i64 {
        match self.item {
            SlotItem::Pack(d) => BatteryPack::from_damage(d).spec().charge_rate,
            SlotItem::Creative => i64::MAX / 100,
            SlotItem::Other => 0,
        }
    }

    pub fn discharge_rate(&self) -> i64 {
        match self.item {
            SlotItem::Pack(d) => BatteryPack::from_damage(d).spec().discharge_rate,
            SlotItem::Creative => i64::MAX / 100,
            SlotItem::Other => 0,
        }
    }

    /// `getCharge`: a pack with no tag gets one set to zero.
    pub fn get_charge(&self) -> i64 {
        match self.item {
            SlotItem::Pack(_) => {
                if self.charge.get().is_none() {
                    self.charge.set(Some(0));
                }
                self.charge.get().unwrap_or(0)
            }
            SlotItem::Creative => i64::MAX / 2,
            SlotItem::Other => 0,
        }
    }

    pub fn charge_battery(&self, i: i64) {
        if let SlotItem::Pack(_) = self.item {
            self.charge.set(Some(match self.charge.get() {
                Some(c) => c.wrapping_add(i),
                None => i,
            }));
        }
    }

    pub fn set_charge(&self, i: i64) {
        if let SlotItem::Pack(_) = self.item {
            self.charge.set(Some(i));
        }
    }

    pub fn discharge_battery(&self, i: i64) {
        if let SlotItem::Pack(_) = self.item {
            self.charge.set(Some(match self.charge.get() {
                Some(c) => c.wrapping_sub(i),
                None => 0,
            }));
        }
    }
}

/// `Library.chargeItemsFromTE`: puts energy from the machine into the item
/// and returns what the machine keeps.
pub fn charge_items_from_te(slot: Option<&ItemSlot>, power: i64, max_power: i64) -> i64 {
    if power < 0 {
        return 0;
    }
    if power > max_power {
        return max_power;
    }
    let Some(slot) = slot else { return power };
    if slot.item == SlotItem::Creative {
        return 0;
    }
    if !slot.is_battery() {
        return power;
    }
    let bat_max = slot.max_charge();
    let bat_charge = slot.get_charge();
    let bat_rate = slot.charge_rate();
    let to_charge = power.min(bat_rate).min(bat_max.wrapping_sub(bat_charge));
    slot.charge_battery(to_charge);
    power.wrapping_sub(to_charge)
}

/// `Library.chargeTEFromItems`: takes energy from the item into the machine
/// and returns the machine's new power.
pub fn charge_te_from_items(slot: Option<&ItemSlot>, power: i64, max_power: i64) -> i64 {
    let Some(slot) = slot else { return power };
    if slot.item == SlotItem::Creative {
        return max_power;
    }
    if !slot.is_battery() {
        return power;
    }
    let bat_charge = slot.get_charge();
    let bat_rate = slot.discharge_rate();
    let to_discharge = max_power.wrapping_sub(power).min(bat_rate).min(bat_charge);
    slot.discharge_battery(to_discharge);
    power.wrapping_add(to_discharge)
}

/// `ItemBatteryPack.EnumBatteryPack`, in metadata order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatteryPack {
    Redstone,
    Lead,
    Lithium,
    Sodium,
    Schrabidium,
    Quantum,
    CapacitorCopper,
    CapacitorGold,
    CapacitorNiobium,
    CapacitorTantalum,
    CapacitorBismuth,
    CapacitorSpark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackSpec {
    /// `ntm:battery_pack__<name>` item id.
    pub id: &'static str,
    pub capacity: i64,
    pub charge_rate: i64,
    pub discharge_rate: i64,
    pub capacitor: bool,
}

const fn battery(id: &'static str, discharge: i64) -> PackSpec {
    PackSpec {
        id,
        capacity: discharge * 20 * 60 * 15,
        charge_rate: discharge * 10,
        discharge_rate: discharge,
        capacitor: false,
    }
}

const fn capacitor(id: &'static str, discharge: i64) -> PackSpec {
    PackSpec {
        id,
        capacity: discharge * 20 * 30,
        charge_rate: discharge,
        discharge_rate: discharge,
        capacitor: true,
    }
}

/// The 12 packs, indexed by metadata.
pub const PACKS: [PackSpec; 12] = [
    battery("ntm:battery_pack__battery_redstone", 100),
    battery("ntm:battery_pack__battery_lead", 1_000),
    battery("ntm:battery_pack__battery_lithium", 10_000),
    battery("ntm:battery_pack__battery_sodium", 50_000),
    battery("ntm:battery_pack__battery_schrabidium", 250_000),
    PackSpec {
        id: "ntm:battery_pack__battery_quantum",
        capacity: 1_000_000 * 20 * 60 * 60,
        charge_rate: 1_000_000 * 10,
        discharge_rate: 1_000_000,
        capacitor: false,
    },
    capacitor("ntm:battery_pack__capacitor_copper", 1_000),
    capacitor("ntm:battery_pack__capacitor_gold", 10_000),
    capacitor("ntm:battery_pack__capacitor_niobium", 100_000),
    capacitor("ntm:battery_pack__capacitor_tantalum", 500_000),
    capacitor("ntm:battery_pack__capacitor_bismuth", 2_500_000),
    capacitor("ntm:battery_pack__capacitor_spark", 10_000_000),
];

impl BatteryPack {
    pub const ALL: [BatteryPack; 12] = [
        BatteryPack::Redstone,
        BatteryPack::Lead,
        BatteryPack::Lithium,
        BatteryPack::Sodium,
        BatteryPack::Schrabidium,
        BatteryPack::Quantum,
        BatteryPack::CapacitorCopper,
        BatteryPack::CapacitorGold,
        BatteryPack::CapacitorNiobium,
        BatteryPack::CapacitorTantalum,
        BatteryPack::CapacitorBismuth,
        BatteryPack::CapacitorSpark,
    ];

    /// `EnumUtil.grabEnumSafely(EnumBatteryPack.class, damage)`.
    pub fn from_damage(damage: i32) -> BatteryPack {
        Self::ALL[(damage % 12).unsigned_abs() as usize]
    }

    pub fn spec(self) -> &'static PackSpec {
        &PACKS[self as usize]
    }

    pub fn is_capacitor(self) -> bool {
        self as usize > BatteryPack::Quantum as usize
    }
}

pub const MODE_INPUT: i16 = 0;
pub const MODE_BUFFER: i16 = 1;
pub const MODE_OUTPUT: i16 = 2;
pub const MODE_NONE: i16 = 3;

/// Which sets of the network a battery sits in after an update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Membership {
    pub provider: bool,
    pub receiver: bool,
}

/// GUI buttons of a battery (`receiveControl`'s NBT keys), applied in the
/// original's order: low, high, priority.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BatteryControl {
    pub low: bool,
    pub high: bool,
    pub priority: bool,
}

/// What the REDD and the socket share (`TileEntityBatteryBase`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatteryBase {
    pub red_low: i16,
    pub red_high: i16,
    pub priority: ConnectionPriority,
    pub last_redstone: u8,
    mode_cache: i16,
}

impl Default for BatteryBase {
    fn default() -> Self {
        BatteryBase {
            red_low: MODE_INPUT,
            red_high: MODE_OUTPUT,
            priority: ConnectionPriority::Low,
            last_redstone: 0,
            mode_cache: 0,
        }
    }
}

impl BatteryBase {
    /// `getRelevantMode(false)`: redHigh when any port is powered, else
    /// redLow; also refreshes the cache that the speeds read.
    pub fn relevant_mode(&mut self, powered: bool) -> i16 {
        self.mode_cache = if powered { self.red_high } else { self.red_low };
        self.mode_cache
    }

    /// `getRelevantMode(true)`.
    pub fn cached_mode(&self) -> i16 {
        self.mode_cache
    }

    /// The start of `updateEntity`: an invalid priority (`LOWEST`/`HIGHEST`)
    /// resets to `LOW`, then the mode picks the network membership. `None`
    /// when the mode is not one of the four (the original's switch does
    /// nothing then).
    pub fn base_update(&mut self, powered: bool) -> Option<Membership> {
        if self.priority.ordinal() == 0 || self.priority.ordinal() == 4 {
            self.priority = ConnectionPriority::Low;
        }
        match self.relevant_mode(powered) {
            MODE_INPUT => Some(Membership {
                provider: false,
                receiver: true,
            }),
            MODE_OUTPUT => Some(Membership {
                provider: true,
                receiver: false,
            }),
            MODE_BUFFER => Some(Membership {
                provider: true,
                receiver: true,
            }),
            MODE_NONE => Some(Membership {
                provider: false,
                receiver: false,
            }),
            _ => None,
        }
    }

    /// `receiveControl`: the low/high buttons advance modulo 4, the priority
    /// button advances `LOW..HIGH` and wraps to `LOW`.
    pub fn receive_control(&mut self, ctl: BatteryControl) {
        if ctl.low {
            self.red_low += 1;
            if self.red_low > 3 {
                self.red_low = 0;
            }
        }
        if ctl.high {
            self.red_high += 1;
            if self.red_high > 3 {
                self.red_high = 0;
            }
        }
        if ctl.priority {
            let mut ordinal = self.priority.ordinal() as i32 + 1;
            if ordinal > ConnectionPriority::High.ordinal() as i32 {
                ordinal = ConnectionPriority::Low.ordinal() as i32;
            }
            self.priority = ConnectionPriority::from_index_safely(ordinal);
        }
    }
}

/// `Math.round(double)`: floor(x + 1/2), NaN to 0, saturating.
fn java_round(x: f64) -> i64 {
    if x.is_nan() {
        return 0;
    }
    let f = x.floor();
    (if x - f >= 0.5 { f + 1.0 } else { f }) as i64
}

/// `getComparatorPower`: `clamp(round(power / max(maxPower, 1) * 15), 0, 15)`.
pub fn comparator_power(power: i64, max_power: i64) -> u8 {
    let frac = power as f64 / max_power.max(1) as f64 * 15.0;
    (java_round(frac) as i32).clamp(0, 15) as u8
}

/// `TileEntityBatterySocket`: no store of its own, everything is the pack's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatterySocket {
    pub base: BatteryBase,
    pub slot: Option<ItemSlot>,
    /// Orientation metadata (`10 + facing`).
    pub meta: i32,
}

impl BatterySocket {
    pub fn new(meta: i32) -> Self {
        BatterySocket {
            base: BatteryBase::default(),
            slot: None,
            meta,
        }
    }

    fn battery(&self) -> Option<&ItemSlot> {
        self.slot.as_ref().filter(|s| s.is_battery())
    }

    /// `updateEntity` (the part that is energy: the membership switch).
    pub fn update(&mut self, powered: bool) -> Option<Membership> {
        self.base.base_update(powered)
    }

    /// `canExtractItem`: a drained pack may leave in output mode, a full one
    /// in input mode. `powered` samples the ports (`getRelevantMode(false)`).
    pub fn can_extract_item(&mut self, powered: bool) -> bool {
        let Some(slot) = self.battery() else {
            return false;
        };
        let (charge, max) = (slot.get_charge(), slot.max_charge());
        let mode = self.base.relevant_mode(powered);
        (mode == MODE_OUTPUT && charge == 0) || (mode == MODE_INPUT && charge == max)
    }

    /// `isItemValidForSlot`.
    pub fn is_item_valid(item: SlotItem) -> bool {
        item != SlotItem::Other
    }

    pub fn port_positions(&self, pos: Pos) -> Vec<Pos> {
        let dir = Dir::from_orientation(self.meta - 10);
        let rot = dir.rotate_up();
        let (d, r) = (dir.offset(), rot.offset());
        vec![
            pos,
            shifted(pos, -d[0], -d[2]),
            shifted(pos, r[0], r[2]),
            shifted(pos, -d[0] + r[0], -d[2] + r[2]),
        ]
    }

    pub fn connection_positions(&self, pos: Pos) -> Vec<(Pos, Dir)> {
        let dir = Dir::from_orientation(self.meta - 10);
        let rot = dir.rotate_up();
        let (d, r) = (dir.offset(), rot.offset());
        vec![
            (shifted(pos, d[0], d[2]), dir),
            (shifted(pos, d[0] + r[0], d[2] + r[2]), dir),
            (shifted(pos, -d[0] * 2, -d[2] * 2), dir.opposite()),
            (
                shifted(pos, -d[0] * 2 + r[0], -d[2] * 2 + r[2]),
                dir.opposite(),
            ),
            (shifted(pos, r[0] * 2, r[2] * 2), rot),
            (shifted(pos, r[0] * 2 - d[0], r[2] * 2 - d[2]), rot),
            (shifted(pos, -r[0], -r[2]), rot.opposite()),
            (shifted(pos, -r[0] - d[0], -r[2] - d[2]), rot.opposite()),
        ]
    }
}

impl EnergyEntity for BatterySocket {
    fn power(&self) -> i64 {
        self.battery().map_or(0, |s| s.get_charge())
    }

    fn set_power(&mut self, power: i64) {
        if let Some(s) = self.battery() {
            s.set_charge(power);
        }
    }

    fn max_power(&self) -> i64 {
        self.battery().map_or(0, |s| s.max_charge())
    }

    fn provider_speed(&self) -> i64 {
        let Some(s) = self.battery() else { return 0 };
        let mode = self.base.cached_mode();
        if mode == MODE_OUTPUT || mode == MODE_BUFFER {
            s.discharge_rate()
        } else {
            0
        }
    }

    fn receiver_speed(&self) -> i64 {
        let Some(s) = self.battery() else { return 0 };
        let mode = self.base.cached_mode();
        if mode == MODE_INPUT || mode == MODE_BUFFER {
            s.charge_rate()
        } else {
            0
        }
    }

    fn priority(&self) -> ConnectionPriority {
        self.base.priority
    }
}

/// `TileEntityBatteryREDD`: the store is unbounded; the network only sees a
/// window of it (`getPower` caps at half of `Long.MAX_VALUE / 100`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatteryRedd {
    pub base: BatteryBase,
    pub power: BigInt,
    pub delta: BigInt,
    pub log: [Option<BigInt>; 20],
    /// Slot 0 feeds the REDD, slot 1 is charged from it.
    pub slots: [Option<ItemSlot>; 2],
    pub meta: i32,
}

impl BatteryRedd {
    pub fn new(meta: i32) -> Self {
        BatteryRedd {
            base: BatteryBase::default(),
            power: BigInt::zero(),
            delta: BigInt::zero(),
            log: Default::default(),
            slots: [None, None],
            meta,
        }
    }

    /// `updateEntity` on the server side. Returns the membership the base
    /// switch chose.
    pub fn update(&mut self, powered: bool) -> Option<Membership> {
        let prev_power = self.power.clone();
        let membership = self.base.base_update(powered);

        let max = self.max_power();
        let to_add = charge_te_from_items(self.slots[0].as_ref(), 0, max);
        if to_add > 0 {
            self.power = self.power.add_long(to_add);
        }

        let to_remove = self.power_window().wrapping_sub(charge_items_from_te(
            self.slots[1].as_ref(),
            self.power_window(),
            max,
        ));
        if to_remove > 0 {
            self.power = self.power.subtract_long(to_remove);
        }

        // the same average/delta log as the batteries, in big integers
        let avg = self.power.add(&prev_power).half();
        let first = self.log[0].clone().unwrap_or_else(BigInt::zero);
        self.delta = avg.subtract(&first);
        for i in 1..self.log.len() {
            self.log[i - 1] = self.log[i].take();
        }
        self.log[19] = Some(avg);
        membership
    }

    fn power_window(&self) -> i64 {
        self.power
            .min_of(&BigInt::from_i64(self.max_power() / 2))
            .long_value()
    }

    /// `getSpeed`: the rotor speed the client animates, as a `float`.
    pub fn speed(&self) -> f32 {
        let inner = (self.power.double_value() * 0.05 + 1.0).ln() * f64::from(0.05f32);
        jmin(inner.powf(5.0), 15.0) as f32
    }

    pub fn port_positions(&self, pos: Pos) -> Vec<Pos> {
        let dir = Dir::from_orientation(self.meta - 10);
        let (d, r) = (dir.offset(), dir.rotate_up().offset());
        vec![
            shifted(pos, d[0] * 2 + r[0] * 2, d[2] * 2 + r[2] * 2),
            shifted(pos, d[0] * 2 - r[0] * 2, d[2] * 2 - r[2] * 2),
            shifted(pos, -d[0] * 2 + r[0] * 2, -d[2] * 2 + r[2] * 2),
            shifted(pos, -d[0] * 2 - r[0] * 2, -d[2] * 2 - r[2] * 2),
            shifted(pos, r[0] * 4, r[2] * 4),
            shifted(pos, -r[0] * 4, -r[2] * 4),
        ]
    }

    pub fn connection_positions(&self, pos: Pos) -> Vec<(Pos, Dir)> {
        let dir = Dir::from_orientation(self.meta - 10);
        let rot = dir.rotate_up();
        let (d, r) = (dir.offset(), rot.offset());
        vec![
            (shifted(pos, d[0] * 3 + r[0] * 2, d[2] * 3 + r[2] * 2), dir),
            (shifted(pos, d[0] * 3 - r[0] * 2, d[2] * 3 - r[2] * 2), dir),
            (
                shifted(pos, -d[0] * 3 + r[0] * 2, -d[2] * 3 + r[2] * 2),
                dir.opposite(),
            ),
            (
                shifted(pos, -d[0] * 3 - r[0] * 2, -d[2] * 3 - r[2] * 2),
                dir.opposite(),
            ),
            (shifted(pos, r[0] * 5, r[2] * 5), rot),
            (shifted(pos, -r[0] * 5, -r[2] * 5), rot.opposite()),
        ]
    }
}

impl EnergyEntity for BatteryRedd {
    /// For provision: capped at half the window, so it fits a `long`.
    fn power(&self) -> i64 {
        self.power_window()
    }

    /// Not needed: the REDD uses `transfer_power` and `use_power` directly.
    fn set_power(&mut self, _power: i64) {}

    /// For connection speed.
    fn max_power(&self) -> i64 {
        i64::MAX / 100
    }

    fn priority(&self) -> ConnectionPriority {
        self.base.priority
    }

    fn transfer_power(&mut self, power: i64) -> i64 {
        self.power = self.power.add_long(power);
        0
    }

    fn use_power(&mut self, power: i64) {
        self.power = self.power.subtract_long(power);
    }
}

// ---------------------------------------------------------------- wires

/// `TileEntityPylonBase.ConnectionType`: wires only join the same type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionType {
    Single,
    Triple,
    Quad,
}

/// Result of `TileEntityPylonBase.canConnect(first, second)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkCheck {
    Ok = 0,
    TypeMismatch = 1,
    SameEndpoint = 2,
    TooFar = 3,
}

/// The block-entity classes behind the nine wire endpoints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireKind {
    Connector,
    ConnectorSuper,
    Pylon,
    PylonSteel,
    MediumWood,
    MediumWoodTransformer,
    MediumSteel,
    MediumSteelTransformer,
    Large,
}

/// What `createNode` builds: the node's own position, its faces and the
/// extra links (other endpoints, and the medium pylon's transformer face).
#[derive(Clone, Debug, PartialEq)]
pub struct WireNode {
    pub positions: Vec<Pos>,
    pub connections: Vec<(Pos, Dir)>,
    pub extra: Vec<(Pos, Dir)>,
}

/// `MathHelper.SIN_TABLE`.
fn sin_table() -> &'static Vec<f32> {
    static TABLE: OnceLock<Vec<f32>> = OnceLock::new();
    TABLE.get_or_init(|| {
        (0..65536)
            .map(|i| (i as f64 * std::f64::consts::PI * 2.0 / 65536.0).sin() as f32)
            .collect()
    })
}

fn mc_sin(x: f32) -> f32 {
    sin_table()[((x * 10430.378f32) as i32 & 65535) as usize]
}

fn mc_cos(x: f32) -> f32 {
    sin_table()[((x * 10430.378f32 + 16384.0f32) as i32 & 65535) as usize]
}

impl WireKind {
    pub const ALL: [WireKind; 9] = [
        WireKind::Connector,
        WireKind::ConnectorSuper,
        WireKind::Pylon,
        WireKind::PylonSteel,
        WireKind::MediumWood,
        WireKind::MediumWoodTransformer,
        WireKind::MediumSteel,
        WireKind::MediumSteelTransformer,
        WireKind::Large,
    ];

    pub fn id(self) -> &'static str {
        match self {
            WireKind::Connector => "ntm:red_connector",
            WireKind::ConnectorSuper => "ntm:red_connector_super",
            WireKind::Pylon => "ntm:red_pylon",
            WireKind::PylonSteel => "ntm:red_pylon_steel",
            WireKind::MediumWood => "ntm:red_pylon_medium_wood",
            WireKind::MediumWoodTransformer => "ntm:red_pylon_medium_wood_transformer",
            WireKind::MediumSteel => "ntm:red_pylon_medium_steel",
            WireKind::MediumSteelTransformer => "ntm:red_pylon_medium_steel_transformer",
            WireKind::Large => "ntm:red_pylon_large",
        }
    }

    pub fn from_id(id: &str) -> Option<WireKind> {
        Self::ALL.into_iter().find(|k| k.id() == id)
    }

    pub fn connection_type(self) -> ConnectionType {
        match self {
            WireKind::Connector
            | WireKind::ConnectorSuper
            | WireKind::Pylon
            | WireKind::PylonSteel => ConnectionType::Single,
            WireKind::Large => ConnectionType::Quad,
            _ => ConnectionType::Triple,
        }
    }

    /// `getMaxWireLength`.
    pub fn max_wire_length(self) -> f64 {
        match self {
            WireKind::Connector => 10.0,
            WireKind::ConnectorSuper | WireKind::Large => 100.0,
            WireKind::Pylon | WireKind::PylonSteel => 25.0,
            _ => 45.0,
        }
    }

    pub fn is_medium(self) -> bool {
        matches!(
            self,
            WireKind::MediumWood
                | WireKind::MediumWoodTransformer
                | WireKind::MediumSteel
                | WireKind::MediumSteelTransformer
        )
    }

    /// `hasTransformer`.
    pub fn has_transformer(self) -> bool {
        matches!(
            self,
            WireKind::MediumWoodTransformer | WireKind::MediumSteelTransformer
        )
    }

    /// `getMountPos`, from the block metadata.
    pub fn mount_points(self, meta: i32) -> Vec<[f64; 3]> {
        match self {
            WireKind::Connector => vec![[0.5, 0.5, 0.5]],
            WireKind::ConnectorSuper => {
                let d = Dir::from_orientation(meta).offset();
                vec![[
                    0.5 + d[0] as f64 * 0.375,
                    0.5 + d[1] as f64 * 0.375,
                    0.5 + d[2] as f64 * 0.375,
                ]]
            }
            WireKind::Pylon | WireKind::PylonSteel => vec![[0.5, 5.5, 0.5]],
            WireKind::Large => {
                let top_off = 0.75 + 0.0625;
                let side_off = 3.375;
                let (mut vx, mut vz) = (side_off, 0.0f64);
                let angle = match meta - 10 {
                    2 => Some((std::f64::consts::PI as f32) * 0.0f32),
                    4 => Some((std::f64::consts::PI as f32) * 0.25f32),
                    3 => Some((std::f64::consts::PI as f32) * 0.5f32),
                    5 => Some((std::f64::consts::PI as f32) * 0.75f32),
                    _ => None,
                };
                if let Some(angle) = angle {
                    // Vec3.rotateAroundY
                    let (f1, f2) = (mc_cos(angle) as f64, mc_sin(angle) as f64);
                    let (nx, nz) = (vx * f1 + vz * f2, vz * f1 - vx * f2);
                    vx = nx;
                    vz = nz;
                }
                vec![
                    [0.5 + vx, 11.5 + top_off, 0.5 + vz],
                    [0.5 + vx, 11.5 - top_off, 0.5 + vz],
                    [0.5 - vx, 11.5 + top_off, 0.5 - vz],
                    [0.5 - vx, 11.5 - top_off, 0.5 - vz],
                ]
            }
            _ => {
                let d = Dir::from_orientation(meta - 10).offset();
                let height = 7.5;
                vec![
                    [0.5, height, 0.5],
                    [0.5 + d[0] as f64, height, 0.5 + d[2] as f64],
                    [0.5 + d[0] as f64 * 2.0, height, 0.5 + d[2] as f64 * 2.0],
                ]
            }
        }
    }

    /// `getConnectionPoint`: the first mount, in world coordinates.
    pub fn connection_point(self, pos: Pos, meta: i32) -> [f64; 3] {
        match self.mount_points(meta).first() {
            Some(m) => [
                m[0] + pos[0] as f64,
                m[1] + pos[1] as f64,
                m[2] + pos[2] as f64,
            ],
            None => [
                pos[0] as f64 + 0.5,
                pos[1] as f64 + 0.5,
                pos[2] as f64 + 0.5,
            ],
        }
    }

    /// `canConnect(ForgeDirection)`: whether a neighbour can join this face.
    pub fn can_connect_dir(self, meta: i32, dir: Dir) -> bool {
        match self {
            WireKind::Connector | WireKind::ConnectorSuper => {
                Dir::from_orientation(meta).opposite() == dir
            }
            WireKind::Pylon | WireKind::PylonSteel => dir != Dir::Down,
            WireKind::Large => dir != Dir::Unknown,
            _ => self.has_transformer() && Dir::from_orientation(meta - 10).opposite() == dir,
        }
    }

    /// `createNode`, given the endpoints already linked to this one.
    pub fn node(self, pos: Pos, meta: i32, connected: &[Pos]) -> WireNode {
        let own = (pos, Dir::Unknown);
        let mut extra: Vec<(Pos, Dir)> = connected.iter().map(|&c| (c, Dir::Unknown)).collect();
        let connections = match self {
            WireKind::Connector | WireKind::ConnectorSuper => {
                let dir = Dir::from_orientation(meta).opposite();
                let o = dir.offset();
                vec![own, ([pos[0] + o[0], pos[1] + o[1], pos[2] + o[2]], dir)]
            }
            WireKind::Pylon | WireKind::PylonSteel => vec![
                own,
                ([pos[0] + 1, pos[1], pos[2]], Dir::East),
                ([pos[0] - 1, pos[1], pos[2]], Dir::West),
                ([pos[0], pos[1], pos[2] + 1], Dir::South),
                ([pos[0], pos[1], pos[2] - 1], Dir::North),
            ],
            WireKind::Large => vec![own],
            _ => {
                if self.has_transformer() {
                    let dir = Dir::from_orientation(meta - 10).opposite();
                    let o = dir.offset();
                    extra.push(([pos[0] + o[0], pos[1], pos[2] + o[2]], dir));
                }
                vec![own]
            }
        };
        WireNode {
            positions: vec![pos],
            connections,
            extra,
        }
    }
}

/// One end of a prospective wire.
#[derive(Clone, Copy, Debug)]
pub struct WireEnd {
    pub kind: WireKind,
    pub pos: Pos,
    pub meta: i32,
}

/// `TileEntityPylonBase.canConnect(first, second)`. Two ends at the same
/// position are the same endpoint (a block cannot hold two). The distance is
/// `Vec3.lengthVector`: the square root rounded to `float`.
pub fn can_connect_wires(first: &WireEnd, second: &WireEnd) -> LinkCheck {
    if first.kind.connection_type() != second.kind.connection_type() {
        return LinkCheck::TypeMismatch;
    }
    if first.pos == second.pos {
        return LinkCheck::SameEndpoint;
    }
    let len = first
        .kind
        .max_wire_length()
        .min(second.kind.max_wire_length());
    let a = first.kind.connection_point(first.pos, first.meta);
    let b = second.kind.connection_point(second.pos, second.meta);
    let (dx, dy, dz) = (b[0] - a[0], b[1] - a[1], b[2] - a[2]);
    let length = f64::from((dx * dx + dy * dy + dz * dz).sqrt() as f32);
    if len >= length {
        LinkCheck::Ok
    } else {
        LinkCheck::TooFar
    }
}

// ---------------------------------------------------------------- switch, detector

/// Metadata the switch and detector blocks carry: 0 off, 1 on.
pub fn switch_toggled(meta: u8) -> u8 {
    if meta == 0 {
        1
    } else {
        0
    }
}

/// `CableDetector.onNeighborBlockChange`: the new metadata when redstone
/// changes it, `None` when it stays.
pub fn detector_meta_after(powered: bool, meta: u8) -> Option<u8> {
    match (powered, meta) {
        (true, 0) => Some(1),
        (false, 1) => Some(0),
        _ => None,
    }
}

/// `TileEntityCableSwitch.shouldCreateNode`: only the on state joins a net;
/// switching off destroys the node.
pub fn switch_joins_net(meta: u8) -> bool {
    meta == 1
}

/// `TileEntityPylon.updateEntity` migration: metadata 0 becomes 12.
pub fn pylon_migrated_meta(meta: i32) -> i32 {
    if meta == 0 {
        12
    } else {
        meta
    }
}

// ---------------------------------------------------------------- the 14 ids

/// The role of each of the 14 formerly inert block ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetBlock {
    BatterySocket,
    BatteryRedd,
    /// `machine_transformer` has no block entity in the original (a plain
    /// block), so there is no behaviour to port.
    Transformer,
    CableSwitch,
    CableDetector,
    Wire(WireKind),
}

pub const NET_BLOCK_IDS: [&str; 14] = [
    "ntm:machine_battery_socket",
    "ntm:machine_battery_redd",
    "ntm:machine_transformer",
    "ntm:cable_switch",
    "ntm:cable_detector",
    "ntm:red_connector",
    "ntm:red_connector_super",
    "ntm:red_pylon",
    "ntm:red_pylon_steel",
    "ntm:red_pylon_medium_wood",
    "ntm:red_pylon_medium_wood_transformer",
    "ntm:red_pylon_medium_steel",
    "ntm:red_pylon_medium_steel_transformer",
    "ntm:red_pylon_large",
];

impl NetBlock {
    pub fn from_id(id: &str) -> Option<NetBlock> {
        Some(match id {
            "ntm:machine_battery_socket" => NetBlock::BatterySocket,
            "ntm:machine_battery_redd" => NetBlock::BatteryRedd,
            "ntm:machine_transformer" => NetBlock::Transformer,
            "ntm:cable_switch" => NetBlock::CableSwitch,
            "ntm:cable_detector" => NetBlock::CableDetector,
            other => NetBlock::Wire(WireKind::from_id(other)?),
        })
    }
}
