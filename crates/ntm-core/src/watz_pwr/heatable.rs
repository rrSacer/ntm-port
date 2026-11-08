//! `FT_Heatable` (first heating step and per-purpose efficiency) and `FT_PWRModerator`, for the
//! fluids that Fluids.java registers a heating trait on. The first step is the only one the
//! Watz, the PWR and the Hephaestus ever use (`getFirstStep`).

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Heatable {
    pub name: &'static str,
    /// Fluid produced by the first step.
    pub hot: &'static str,
    /// `HeatingStep.heatReq` (TU per cycle).
    pub heat_req: i32,
    /// `HeatingStep.amountReq` (mB consumed per cycle).
    pub amount_req: i32,
    /// `HeatingStep.amountProduced` (mB produced per cycle).
    pub amount_produced: i32,
    pub eff_boiler: f64,
    pub eff_heatexchanger: f64,
    pub eff_pwr: f64,
    /// `FT_PWRModerator.multiplier`, when the fluid carries that trait.
    pub moderator: Option<f64>,
}

const fn h(
    name: &'static str, hot: &'static str, heat_req: i32, amount_req: i32, amount_produced: i32,
    eff_boiler: f64, eff_heatexchanger: f64, eff_pwr: f64, moderator: Option<f64>,
) -> Heatable {
    Heatable { name, hot, heat_req, amount_req, amount_produced, eff_boiler, eff_heatexchanger, eff_pwr, moderator }
}

/// Every `FT_Heatable` registered in Fluids.java, in declaration order.
pub const HEATABLES: [Heatable; 15] = [
    h("air", "airblast", 5, 1, 1, 1.0, 0.0, 0.0, None),
    h("water", "steam", 200, 1, 100, 1.0, 0.25, 0.0, None),
    h("oil", "hotoil", 10, 1, 1, 1.0, 1.0, 0.0, None),
    h("oil_ds", "hotoil_ds", 10, 1, 1, 1.0, 1.0, 0.0, None),
    h("crackoil", "hotcrackoil", 10, 1, 1, 1.0, 1.0, 0.0, None),
    h("crackoil_ds", "hotcrackoil_ds", 10, 1, 1, 1.0, 1.0, 0.0, None),
    h("coolant", "coolant_hot", 300, 1, 1, 0.0, 1.0, 1.0, None),
    h("perfluoromethyl_cold", "perfluoromethyl", 300, 1, 1, 0.0, 0.0, 0.0, None),
    h("perfluoromethyl", "perfluoromethyl_hot", 300, 1, 1, 0.0, 1.0, 1.0, None),
    h("mug", "mug_hot", 400, 1, 1, 0.0, 1.0, 1.0, Some(1.15)),
    h("blood", "blood_hot", 500, 1, 1, 0.0, 1.0, 0.0, None),
    h("heavywater", "heavywater_hot", 300, 1, 1, 0.0, 0.0, 1.0, Some(1.25)),
    h("sodium", "sodium_hot", 400, 1, 1, 0.0, 0.0, 2.5, None),
    h("lead", "lead_hot", 800, 1, 1, 0.0, 0.0, 0.75, Some(0.75)),
    h("thorium_salt", "thorium_salt_hot", 400, 1, 1, 0.0, 0.0, 1.0, Some(2.5)),
];

/// The heating trait of `fluid`, if it has one.
pub fn heatable(fluid: &str) -> Option<&'static Heatable> {
    HEATABLES.iter().find(|f| f.name == fluid)
}
