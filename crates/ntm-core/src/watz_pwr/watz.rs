//! Watz reactor: pellets, the stacked-segment tick, mud, the structure core and the pump gate.
//!
//! Sources (HBM pin 4f5924e): `ItemWatzPellet`, `TileEntityWatz`, `TileEntityWatzStruct`,
//! `BlockToolConversion` (casing bolting). Gaps that need systems the port does not have yet are
//! listed on [`WatzEvent`] and in the module docs of `watz_pwr`.
use super::function::{java_round_to_int, Function};
use super::heatable::heatable;

/// `EnumWatzType`, in source order. The metadata of a pellet stack indexes this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatzType {
    Schrabidium, Hes, Mes, Les, Hen, Meu, Mep, Lead, Boron, Du, Nqd, Nqr,
}

/// Constants of one pellet type (`EnumWatzType` fields).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PelletSpec {
    pub color_light: u32,
    pub color_dark: u32,
    /// Base flux emission (`passive`).
    pub passive: f64,
    /// Heat per outgoing flux (`heatEmission`).
    pub heat_emission: f64,
    /// Mud per reaction flux; already the constructor value divided by 2.
    pub mud_content: f64,
    pub burn: Option<Function>,
    pub heat_div: Option<Function>,
    pub absorb: Option<Function>,
    pub yield_: f64,
}

impl WatzType {
    pub const ALL: [WatzType; 12] = [
        WatzType::Schrabidium, WatzType::Hes, WatzType::Mes, WatzType::Les, WatzType::Hen, WatzType::Meu,
        WatzType::Mep, WatzType::Lead, WatzType::Boron, WatzType::Du, WatzType::Nqd, WatzType::Nqr,
    ];

    /// `EnumUtil.grabEnumSafely`: `Math.abs(index % length)`, so any metadata maps to a type.
    pub fn from_meta(meta: i32) -> WatzType {
        Self::ALL[(meta % 12).unsigned_abs() as usize]
    }

    pub fn spec(self) -> PelletSpec {
        use WatzType::*;
        let s = |cl, cd, passive, heat, mud: f64, burn, div, absorb| PelletSpec {
            color_light: cl, color_dark: cd, passive, heat_emission: heat, mud_content: mud / 2.0,
            burn, heat_div: div, absorb, yield_: 500_000_000.0,
        };
        let falling = |f| Some(Function::sqrt_falling(f));
        let nq_div = Some(Function::sqrt(1.0 / 25.0).with_off(25.0 * 25.0));
        match self {
            Schrabidium => s(0x32FFFF, 0x005C5C, 2_000.0, 20.0, 0.01, Some(Function::linear(1.5)), falling(10.0), None),
            Hes => s(0x66DCD6, 0x023933, 1_750.0, 20.0, 0.005, Some(Function::linear(1.25)), falling(15.0), None),
            Mes => s(0xCBEADF, 0x28473C, 1_500.0, 15.0, 0.0025, Some(Function::linear(1.15)), falling(15.0), None),
            Les => s(0xABB4A8, 0x0C1105, 1_250.0, 15.0, 0.00125, Some(Function::linear(1.0)), falling(20.0), None),
            Hen => s(0xA6B2A6, 0x030F03, 0.0, 10.0, 0.0005, Some(Function::sqrt(100.0)), falling(10.0), None),
            Meu => s(0xC1C7BD, 0x2B3227, 0.0, 10.0, 0.0005, Some(Function::sqrt(75.0)), falling(10.0), None),
            Mep => s(0x9AA3A0, 0x111A17, 0.0, 15.0, 0.0005, Some(Function::sqrt(150.0)), falling(10.0), None),
            Lead => s(0xA6A6B2, 0x03030F, 0.0, 0.0, 0.0025, None, None, Some(Function::sqrt(10.0))),
            Boron => s(0xBDC8D2, 0x29343E, 0.0, 0.0, 0.0025, None, None, Some(Function::linear(10.0))),
            Du => s(0xC1C7BD, 0x2B3227, 0.0, 0.0, 0.0025, None, None, Some(Function::quadratic(1.0, 1.0).with_div(100.0))),
            Nqd => s(0x4B4B4B, 0x121212, 2_000.0, 20.0, 0.01, Some(Function::linear(2.0)), nq_div, None),
            Nqr => s(0x2D2D2D, 0x0B0B0B, 2_500.0, 30.0, 0.01, Some(Function::linear(1.5)), nq_div, None),
        }
    }

    /// `ItemWatzPellet.desaturate`, used for the depleted pellet icons (client side).
    pub fn desaturate(color: u32) -> u32 {
        let (mut r, mut g, mut b) = (((color & 0xff0000) >> 16) as i32, ((color & 0x00ff00) >> 8) as i32, (color & 0xff) as i32);
        let avg = (r + g + b) / 3;
        let (approach, mult) = (0.9f64, 0.75f64);
        r = (r as f64 - (r - avg) as f64 * approach) as i32;
        g = (g as f64 - (g - avg) as f64 * approach) as i32;
        b = (b as f64 - (b - avg) as f64 * approach) as i32;
        r = (r as f64 * mult) as i32;
        g = (g as f64 * mult) as i32;
        b = (b as f64 * mult) as i32;
        ((r << 16) | (g << 8) | b) as u32
    }
}

/// One item stack in a Watz slot. `yield_ == None` is a stack that has no NBT tag yet; the first
/// read creates it with the type's full yield (`ItemWatzPellet.setNBTDefaults`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PelletStack {
    pub depleted: bool,
    pub meta: i32,
    pub count: i32,
    pub yield_: Option<f64>,
}

impl PelletStack {
    pub fn fresh(meta: i32, yield_: f64) -> Self {
        PelletStack { depleted: false, meta, count: 1, yield_: Some(yield_) }
    }
    pub fn depleted(meta: i32) -> Self {
        PelletStack { depleted: true, meta, count: 1, yield_: None }
    }
    /// `ItemWatzPellet.getYield`
    pub fn get_yield(&mut self) -> f64 {
        *self.yield_.get_or_insert(WatzType::from_meta(self.meta).spec().yield_)
    }
    /// `ItemWatzPellet.setYield`
    pub fn set_yield(&mut self, v: f64) {
        self.get_yield();
        self.yield_ = Some(v);
    }
    /// `ItemWatzPellet.getEnrichment`
    pub fn enrichment(&mut self) -> f64 {
        let spec = WatzType::from_meta(self.meta).spec();
        self.get_yield() / spec.yield_
    }
}

pub const SLOTS: usize = 24;
pub const TANK_CAPACITY: i32 = 64_000;

/// Raised by [`tick_stack`]; the world effects are not ported (no pollution, entity or
/// radiation-injection systems yet) and are left to the caller.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WatzEvent {
    /// Mud left in the shared tank after the redistribution: the reactor blows up. The source
    /// clears the 7×3×7 volume above the controller, scatters shrapnel and mud/ruined blocks
    /// (random columns), adds `radiation` to the chunk at `y + 1` and plays the explosion.
    MudOverflow { radiation: f32 },
}

/// State of one stacked segment (one `TileEntityWatz`). Tanks: `[coolant, hot coolant, mud]`.
#[derive(Debug, Clone, PartialEq)]
pub struct WatzSegment {
    pub slots: [Option<PelletStack>; SLOTS],
    pub heat: i32,
    pub flux_last_base: f64,
    pub flux_last_reaction: f64,
    pub tanks: [i32; 3],
    pub is_on: bool,
}

impl Default for WatzSegment {
    fn default() -> Self {
        WatzSegment { slots: [None; SLOTS], heat: 0, flux_last_base: 0.0, flux_last_reaction: 0.0, tanks: [0; 3], is_on: false }
    }
}

/// `TileEntityWatz.updateCoolant`: the first step of `COOLANT` (300 TU, 1 mB → 1 mB hot).
fn update_coolant(heat: &mut i32, shared: &mut [i32; 3], shared_max: [i32; 3]) {
    let step = heatable("coolant").expect("coolant is heatable");
    let heat_to_use = *heat as f64 * 0.2;
    let heat_cycles = (heat_to_use / step.heat_req as f64) as i32;
    let cool_cycles = shared[0] / step.amount_req;
    let hot_cycles = (shared_max[1] - shared[1]) / step.amount_produced;
    let cycles = heat_cycles.min(hot_cycles.min(cool_cycles));
    *heat = heat.wrapping_sub(cycles.wrapping_mul(step.heat_req));
    shared[0] = shared[0].wrapping_sub(cycles.wrapping_mul(step.amount_req));
    shared[1] = shared[1].wrapping_add(cycles.wrapping_mul(step.amount_produced));
}

impl WatzSegment {
    /// `TileEntityWatz.updateReaction` (the part after the shared tanks are built).
    fn update_reaction(&mut self, above: Option<&mut WatzSegment>, shared: &mut [i32; 3], turned_on: bool) {
        if turned_on {
            let pellets: Vec<usize> = (0..SLOTS).filter(|&i| matches!(self.slots[i], Some(s) if !s.depleted)).collect();
            let mut base_flux = 0.0f64;
            for &i in &pellets {
                base_flux += WatzType::from_meta(self.slots[i].unwrap().meta).spec().passive;
            }
            let input_flux = base_flux + self.flux_last_reaction;
            let mut added_flux = 0.0f64;
            let mut added_heat = 0.0f64;

            for &i in &pellets {
                let stack = self.slots[i].as_mut().unwrap();
                let spec = WatzType::from_meta(stack.meta).spec();
                if let Some(burn_func) = spec.burn {
                    let div = spec.heat_div.map_or(1.0, |d| d.effonix(self.heat as f64));
                    let burn = burn_func.effonix(input_flux) / div;
                    let y = stack.get_yield();
                    stack.set_yield(y - burn);
                    added_flux += burn;
                    added_heat += spec.heat_emission * burn;
                    shared[2] = shared[2].wrapping_add(java_round_to_int(spec.mud_content * burn));
                }
            }
            for &i in &pellets {
                let stack = self.slots[i].as_mut().unwrap();
                let spec = WatzType::from_meta(stack.meta).spec();
                if let Some(absorb_func) = spec.absorb {
                    let absorb = absorb_func.effonix(base_flux + self.flux_last_reaction);
                    added_heat += absorb;
                    let y = stack.get_yield();
                    stack.set_yield(y - absorb);
                    shared[2] = shared[2].wrapping_add(java_round_to_int(spec.mud_content * absorb));
                }
            }
            self.heat = (self.heat as f64 + added_heat) as i32;
            self.flux_last_base = base_flux;
            self.flux_last_reaction = added_flux;
        } else {
            self.flux_last_base = 0.0;
            self.flux_last_reaction = 0.0;
        }

        for slot in self.slots.iter_mut() {
            let spent = match slot {
                Some(s) if !s.depleted => s.enrichment() <= 0.0,
                _ => false,
            };
            if spent {
                *slot = Some(PelletStack::depleted(slot.unwrap().meta));
            }
        }

        if let Some(above) = above {
            for i in 0..SLOTS {
                let bottom = self.slots[i];
                let top = above.slots[i];
                if bottom.is_none() && top.is_some() {
                    self.slots[i] = top;
                    above.slots[i] = None; // decrStackSize(i, stackSize): the whole stack leaves
                }
                if matches!(bottom, Some(b) if !b.depleted) && matches!(top, Some(t) if t.depleted) {
                    above.slots[i] = bottom;
                    self.slots[i] = top;
                }
            }
        }
    }
}

/// One server tick of a whole Watz column. `segments[0]` is the top segment (the one whose
/// `updateEntity` does the work; lower segments skip it through `updateLock`). `pump_present` is
/// "the block three above the top segment is the pump" and `power_level > 0` is the indirect
/// redstone level read five above it.
pub fn tick_stack(segments: &mut [WatzSegment], pump_present: bool, power_level: i32) -> Option<WatzEvent> {
    if segments.is_empty() {
        return None;
    }
    let turned_on = pump_present && power_level > 0;
    let n = segments.len() as i32;
    let shared_max = [TANK_CAPACITY * n; 3];
    let mut shared = [0i32; 3];
    for seg in segments.iter() {
        for (sh, t) in shared.iter_mut().zip(seg.tanks) {
            *sh = sh.wrapping_add(t);
        }
    }

    for seg in segments.iter_mut().rev() {
        update_coolant(&mut seg.heat, &mut shared, shared_max);
    }

    for i in 0..segments.len() {
        if i == 0 {
            segments[0].update_reaction(None, &mut shared, turned_on);
        } else {
            let (head, tail) = segments.split_at_mut(i);
            tail[0].update_reaction(Some(&mut head[i - 1]), &mut shared, turned_on);
        }
    }

    for seg in segments.iter_mut() {
        seg.is_on = turned_on;
        seg.heat = (seg.heat as f64 * 0.99) as i32; // cool 1% per tick
    }

    for seg in segments.iter_mut().rev() {
        for (sh, t) in shared.iter_mut().zip(seg.tanks.iter_mut()) {
            let min = TANK_CAPACITY.min(*sh);
            *sh = sh.wrapping_sub(min);
            *t = min;
        }
    }

    if shared[2] > 0 {
        return Some(WatzEvent::MudOverflow { radiation: 1_000.0 });
    }
    None
}

/// `TileEntityWatz.isItemValidForSlot` (only fresh pellets, honoring the slot lock).
pub fn pellet_accepted(locked: bool, lock: Option<(bool, i32)>, stack: &PelletStack) -> bool {
    if stack.depleted {
        return false;
    }
    !locked || matches!(lock, Some((false, meta)) if meta == stack.meta)
}

/// `TileEntityWatz.canExtractItem`: automation may only take things that are not fresh pellets.
pub fn can_extract(stack: &PelletStack) -> bool {
    stack.depleted
}

/// Casing bolting (`BlockToolConversion`): a bolt tool turns `watz_end` meta 0 into meta 1 for four
/// Dura bolts from the player's inventory. Returns `(new_meta, bolt_count)`.
pub fn bolt_casing(meta: i32) -> Option<(i32, i32)> {
    (meta == 0).then_some((1, 4))
}

/// What the complete Watz drops when broken (`Watz.breakBlock`, meta ≥ 12 and `drop`):
/// `(item, count)` pairs; the bolts are 64 Dura bolts three times.
pub const BREAK_DROPS: [(&str, i32); 5] = [
    ("watz_end", 48),
    ("bolt_dura", 64),
    ("watz_element", 36),
    ("watz_cooler", 26),
    ("struct_watz_core", 1),
];
pub const BREAK_BOLT_STACKS: i32 = 3;

/// A block as seen by the structure check: its registry name and metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructBlock {
    Air,
    WatzCooler,
    WatzElement,
    WatzEnd { meta: i32 },
    Other,
}

/// `TileEntityWatzStruct.updateEntity`: the check that runs every 20 ticks on the core block.
/// `at(dx, dy, dz)` returns the block at that offset from the core. `true` means the
/// multiblock is complete and the core becomes a `watz` (meta 12) with `fillSpace`.
pub fn structure_complete(at: &dyn Fn(i32, i32, i32) -> StructBlock) -> bool {
    use StructBlock::*;
    let cbr = |b: StructBlock, x, y, z| std::mem::discriminant(&at(x, y, z)) == std::mem::discriminant(&b);
    let cbr_meta = |meta: i32, x, y, z| at(x, y, z) == (WatzEnd { meta });
    if !cbr(WatzCooler, 0, 1, 0) || !cbr(WatzCooler, 0, 2, 0) {
        return false;
    }
    for i in 0..3 {
        for (x, z) in [(1, 0), (2, 0), (0, 1), (0, 2), (-1, 0), (-2, 0), (0, -1), (0, -2), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            if !cbr(WatzElement, x, i, z) {
                return false;
            }
        }
        for (x, z) in [(2, 1), (2, -1), (1, 2), (-1, 2), (-2, 1), (-2, -1), (1, -2), (-1, -2)] {
            if !cbr(WatzCooler, x, i, z) {
                return false;
            }
        }
        for j in -1..2 {
            if !cbr_meta(1, 3, i, j) || !cbr_meta(1, j, i, 3) || !cbr_meta(1, -3, i, j) || !cbr_meta(1, j, i, -3) {
                return false;
            }
        }
        for (x, z) in [(2, 2), (2, -2), (-2, 2), (-2, -2)] {
            if !cbr_meta(1, x, i, z) {
                return false;
            }
        }
    }
    true
}

/// Fluid ports of one segment: offsets from the controller and whether the port faces down.
/// Source: `TileEntityWatz.getPorts` (`NEG_Y` for the first five, `POS_Y` for the last five).
pub const PORTS: [(i32, i32, i32, bool); 10] = [
    (2, 0, 0, true), (-2, 0, 0, true), (0, 0, 2, true), (0, 0, -2, true), (0, 0, 0, true),
    (2, 2, 0, false), (-2, 2, 0, false), (0, 2, 2, false), (0, 2, -2, false), (0, 2, 0, false),
];
