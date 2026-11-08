//! Petroleum batch E: the Hephaestus thermal exchanger and the gas flare.
//!
//! Sources (HBM pin 4f5924e): `TileEntityMachineHephaestus`, `TileEntityMachineGasFlare`. Pollution,
//! fluid release traits, fire damage, tilt detection, particles, sound and the battery-charging slot
//! are not ported; the flare reports them as [`FlareEvent`]s so the caller can apply them later.
use crate::watz_pwr::heatable::heatable;
use std::collections::HashMap;

// ---------------------------------------------------------------- Hephaestus

pub const HEPHAESTUS_TANK: i32 = 24_000;
pub const SCAN_RANGE: i32 = 7;
pub const SCAN_LAYERS: usize = 10;

/// What the thermal scan can tell apart. `Lava` covers both `lava` and `flowing_lava`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeatBlock {
    Other,
    Lava,
    VolcanicLava,
    OreVolcano,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hephaestus {
    pub input_type: &'static str,
    pub input_fill: i32,
    pub output_type: &'static str,
    pub output_fill: i32,
    /// Heat of each of the ten scanned horizontal planes.
    pub heat: [i32; SCAN_LAYERS],
    pub fissure_scan_time: i64,
}

impl Default for Hephaestus {
    fn default() -> Self {
        Hephaestus { input_type: "oil", input_fill: 0, output_type: "hotoil", output_fill: 0, heat: [0; SCAN_LAYERS], fissure_scan_time: 0 }
    }
}

impl Hephaestus {
    /// `getTotalHeat`: the sum of the ten planes, tripled for 20 ticks after an `ore_volcano` was seen.
    pub fn total_heat(&self, world_time: i64) -> i32 {
        let sum = self.heat.iter().fold(0i32, |a, h| a.wrapping_add(*h));
        if world_time - self.fissure_scan_time < 20 { sum.wrapping_mul(3) } else { sum }
    }

    fn setup_tanks(&mut self) {
        if let Some(h) = heatable(self.input_type).filter(|h| h.eff_heatexchanger > 0.0) {
            if self.output_type != h.hot {
                self.output_type = h.hot;
                self.output_fill = 0;
            }
            return;
        }
        if self.input_type != "none" {
            self.input_type = "none";
            self.input_fill = 0;
        }
        if self.output_type != "none" {
            self.output_type = "none";
            self.output_fill = 0;
        }
    }

    /// One server tick. `pos` is the machine's block position; `block_at` reads the world.
    /// The scan keeps the original `y >= 0` bound (no adaptation to the lower build limit).
    pub fn tick(&mut self, pos: (i32, i32, i32), world_time: i64, block_at: &dyn Fn(i32, i32, i32) -> HeatBlock) {
        self.setup_tanks();
        let height = (world_time % 10) as usize;
        let y = pos.1 - 1 - height as i32;
        self.heat[height] = 0;
        if y >= 0 {
            for x in -SCAN_RANGE..=SCAN_RANGE {
                for z in -SCAN_RANGE..=SCAN_RANGE {
                    let add = match block_at(pos.0 + x, y, pos.2 + z) {
                        HeatBlock::Lava => 5,
                        HeatBlock::VolcanicLava => 150,
                        HeatBlock::OreVolcano => {
                            self.fissure_scan_time = world_time;
                            300
                        }
                        HeatBlock::Other => 0,
                    };
                    self.heat[height] = self.heat[height].wrapping_add(add);
                }
            }
        }
        self.heat_fluid(world_time);
    }

    fn heat_fluid(&mut self, world_time: i64) {
        let Some(h) = heatable(self.input_type) else { return };
        let heat = self.total_heat(world_time);
        let input_ops = self.input_fill / h.amount_req;
        let output_ops = (HEPHAESTUS_TANK - self.output_fill) / h.amount_produced;
        let heat_ops = heat / h.heat_req;
        let ops = input_ops.min(output_ops).min(heat_ops);
        self.input_fill = self.input_fill.wrapping_sub(h.amount_req.wrapping_mul(ops));
        self.output_fill = self.output_fill.wrapping_add(h.amount_produced.wrapping_mul(ops));
    }
}

/// Fluid ports of the Hephaestus relative to the machine: the four sides at the bottom and at +11.
pub const HEPHAESTUS_PORTS: [(i32, i32, i32); 8] =
    [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1), (1, 11, 0), (-1, 11, 0), (0, 11, 1), (0, 11, -1)];

// ---------------------------------------------------------------- Flare

pub const FLARE_MAX_POWER: i64 = 100_000;
pub const FLARE_TANK: i32 = 64_000;

/// The traits of a fluid the flare looks at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuelTraits {
    /// `FT_Gaseous` or `FT_Gaseous_ART`.
    pub gaseous: bool,
    /// `FT_Flammable`.
    pub flammable: bool,
    /// `FT_Flammable.getHeatEnergy` (TU per bucket).
    pub heat_energy: i64,
}

impl FuelTraits {
    /// Fluids that can be vented are the gaseous ones.
    pub fn ventable(&self) -> bool {
        self.gaseous
    }
    /// Penalty divisor applied to the burn energy.
    pub fn burn_penalty(&self) -> i64 {
        if self.gaseous { 5 } else { 10 }
    }
}

/// `fuel_traits` of `content/machine-recipes/flare.json` (embedded copy), by fluid name.
pub fn fuel_traits() -> &'static HashMap<String, FuelTraits> {
    static T: std::sync::OnceLock<HashMap<String, FuelTraits>> = std::sync::OnceLock::new();
    T.get_or_init(|| parse_fuel_traits(include_str!("../data/machines/flare.json")).expect("flare.json"))
}

pub fn parse_fuel_traits(json: &str) -> Result<HashMap<String, FuelTraits>, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let mut out = HashMap::new();
    for row in v["fuel_traits"].as_array().ok_or("fuel_traits missing")? {
        let name = row["fluid"].as_str().ok_or("fluid")?.to_string();
        out.insert(name, FuelTraits {
            gaseous: row["gaseous_or_art"].as_bool().ok_or("gaseous_or_art")?,
            flammable: row["flammable"].as_bool().ok_or("flammable")?,
            heat_energy: row["heat_tu_per_bucket"].as_i64().ok_or("heat_tu_per_bucket")?,
        });
    }
    Ok(out)
}

/// Requests the flare makes to systems that do not exist yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlareEvent {
    /// `FluidType.onFluidRelease` with the vented amount (mB).
    Release { amount: i32 },
    /// `FluidTrait.onRelease(SPILL, eject * 5)`, every 5 ticks while venting.
    Spill { amount: i32 },
    /// `FluidTrait.onRelease(BURN, eject * 5)`, every 5 ticks while burning.
    Burn { amount: i32 },
    /// Entities in the flame box (x-1..x+2, y+12..y+17, z-2..z+2) are set on fire for 5 s and take 5 damage.
    FireDamage,
    /// Fizz sound (vent, every 7 ticks) or flamethrower sound (burn, every 3 ticks).
    Sound,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FlareTick {
    /// mB removed from the tank this tick (`fluidUsed`).
    pub fluid_used: i32,
    /// HE added this tick (`output`).
    pub output: i32,
    pub events: Vec<FlareEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flare {
    pub power: i64,
    pub fill: i32,
    pub is_on: bool,
    pub does_burn: bool,
}

impl Default for Flare {
    fn default() -> Self {
        Flare { power: 0, fill: 0, is_on: false, does_burn: false }
    }
}

impl Flare {
    /// `receiveControl`: the valve and the burn dial toggle.
    pub fn toggle_valve(&mut self) {
        self.is_on = !self.is_on;
    }
    pub fn toggle_burn(&mut self) {
        self.does_burn = !self.does_burn;
    }

    /// One server tick. `speed`/`effect` are the installed upgrade levels, `world_time` is
    /// `getTotalWorldTime`. The battery-charging slot (`Library.chargeItemsFromTE`) is not modelled.
    pub fn tick(&mut self, traits: FuelTraits, speed: i32, effect: i32, tilted: bool, world_time: i64) -> FlareTick {
        let mut out = FlareTick::default();
        if self.is_on && self.fill > 0 && !tilted {
            let max_vent = 50 + 50 * speed;
            let max_burn = 10 + 10 * speed;
            if !self.does_burn || !traits.flammable {
                if traits.gaseous {
                    let eject = max_vent.min(self.fill);
                    out.fluid_used = eject;
                    self.fill -= eject;
                    out.events.push(FlareEvent::Release { amount: eject });
                    if world_time % 7 == 0 {
                        out.events.push(FlareEvent::Sound);
                    }
                    if world_time % 5 == 0 && eject > 0 {
                        out.events.push(FlareEvent::Spill { amount: eject.wrapping_mul(5) });
                    }
                }
            } else {
                let eject = max_burn.min(self.fill);
                out.fluid_used = eject;
                self.fill -= eject;
                let mut prod = traits.heat_energy * eject as i64 / 1_000;
                prod /= traits.burn_penalty();
                prod += prod * effect as i64 / 3;
                out.output = prod as i32;
                self.power += prod;
                if self.power > FLARE_MAX_POWER {
                    self.power = FLARE_MAX_POWER;
                }
                out.events.push(FlareEvent::FireDamage);
                if world_time % 3 == 0 {
                    out.events.push(FlareEvent::Sound);
                }
                if world_time % 5 == 0 && eject > 0 {
                    out.events.push(FlareEvent::Burn { amount: eject.wrapping_mul(5) });
                }
            }
        }
        out
    }
}
