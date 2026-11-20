//! Tanks and the few fluid facts the RBMK machines read.

use super::tables_data::HEATABLES;

/// A `FluidTank`: fluid name (as in `Fluids.java`), amount and capacity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tank {
    pub fluid: &'static str,
    pub fill: i32,
    pub max: i32,
}

impl Tank {
    pub const fn new(fluid: &'static str, max: i32) -> Tank {
        Tank {
            fluid,
            fill: 0,
            max,
        }
    }

    /// `FluidTank.setTankType`: a new type empties the tank; the same type keeps it.
    pub fn set_tank_type(&mut self, fluid: &'static str) {
        if self.fluid != fluid {
            self.fluid = fluid;
            self.fill = 0;
        }
    }
}

/// The `Fluids` name `name` as the reactor keeps it (the fluid table's), `NONE` for a name the table does not have
/// (`Fluids.fromName` falls back to NONE).
pub fn java_fluid(name: &str) -> &'static str {
    crate::fluid::types().iter().find(|t| t.name == name).map_or("NONE", |t| t.name.as_str())
}

/// The fluid network's name (`string_id`) of the `Fluids` name `name`, none for NONE (an untyped tank).
pub fn network_fluid(name: &str) -> Option<&'static str> {
    crate::fluid::types().iter().find(|t| t.name == name && t.id != 0).map(|t| t.string_id.as_str())
}

/// The `Fluids` name of the network's `string_id`.
pub fn fluid_of_network(string_id: &str) -> Option<&'static str> {
    crate::fluid::get(string_id).filter(|t| t.id != 0).map(|t| t.name.as_str())
}

/// `TileEntityRBMKBoiler.getHeatFromSteam`.
pub fn heat_from_steam(fluid: &str) -> f64 {
    match fluid {
        "STEAM" => 100.0,
        "HOTSTEAM" => 300.0,
        "SUPERHOTSTEAM" => 450.0,
        "ULTRAHOTSTEAM" => 600.0,
        _ => 0.0,
    }
}

/// `TileEntityRBMKBoiler.getFactorFromSteam`.
pub fn factor_from_steam(fluid: &str) -> f64 {
    match fluid {
        "STEAM" => 1.0,
        "HOTSTEAM" => 10.0,
        "SUPERHOTSTEAM" => 100.0,
        "ULTRAHOTSTEAM" => 1000.0,
        _ => 0.0,
    }
}

/// The first heating step of a fluid with `FT_Heatable`.
#[derive(Clone, Copy, Debug)]
pub struct Heatable {
    pub heat_exchanger_efficiency: f64,
    pub amount_req: i32,
    pub heat_req: i32,
    pub produced: &'static str,
    pub amount_produced: i32,
    pub produced_temperature: i32,
}

pub fn heatable(fluid: &str) -> Option<Heatable> {
    HEATABLES.iter().find(|h| h.0 == fluid).map(|h| Heatable {
        heat_exchanger_efficiency: h.1,
        amount_req: h.2,
        heat_req: h.3,
        produced: h.4,
        amount_produced: h.5,
        produced_temperature: h.6,
    })
}
