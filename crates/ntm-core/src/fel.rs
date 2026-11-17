//! FEL operating state, from `TileEntityFEL.updateEntity` (lines 83–193).
//! World effects and machine integration are implemented separately.
use crate::server::container_bridge::Slots;
use serde::{Deserialize, Serialize};

pub const BLOCK: &str = "ntm:machine_fel";
pub const MAX_POWER: u64 = 20_000_000;
pub const RANGE: i32 = 24;

/// Ordinals match `ItemFELCrystal.EnumWavelengths`, including NULL at zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Wavelength {
    #[default]
    Null,
    Ir,
    Visible,
    Uv,
    Gamma,
    Drx,
}

impl Wavelength {
    pub fn from_crystal(id: &str) -> Self {
        match id {
            "ntm:laser_crystal_co2" => Self::Ir,
            "ntm:laser_crystal_bismuth" => Self::Visible,
            "ntm:laser_crystal_cmb" => Self::Uv,
            "ntm:laser_crystal_dnt" => Self::Gamma,
            "ntm:laser_crystal_digamma" => Self::Drx,
            _ => Self::Null,
        }
    }
    pub fn cost(self) -> u64 {
        match self {
            Self::Null => 0,
            Self::Ir => 3_750,
            Self::Visible => 11_250,
            Self::Uv => 33_750,
            Self::Gamma => 101_250,
            Self::Drx => 303_750,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fel {
    pub slots: Slots,
    pub power: u64,
    pub on: bool,
    pub mode: Wavelength,
    pub distance: i32,
    pub missing_valid_silex: bool,
}

impl Default for Fel {
    fn default() -> Self {
        Self { slots: vec![None; 2], power: 0, on: false, mode: Wavelength::Null, distance: 0, missing_valid_silex: true }
    }
}

impl Fel {
    /// Select the crystal and spend a tick's energy. Returns the wavelength of
    /// a live beam, whose world effects must be applied by the caller.
    pub fn begin_tick(&mut self) -> Option<Wavelength> {
        self.power = crate::machine::charge_from_slot(&self.slots, 0, self.power, MAX_POWER);
        self.mode = if self.on {
            self.slots.get(1).and_then(Option::as_ref).map_or(Wavelength::Null, |s| Wavelength::from_crystal(&s.id))
        } else {
            Wavelength::Null
        };
        if self.mode == Wavelength::Null {
            return None;
        }
        let cost = self.mode.cost();
        if self.power < cost {
            self.power = 0;
            return None;
        }
        self.power -= cost;
        Some(self.mode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::container_bridge::Stack;

    #[test]
    fn crystals_and_costs_match_java_ordinals() {
        for (id, mode, cost) in [
            ("ntm:laser_crystal_co2", Wavelength::Ir, 3_750),
            ("ntm:laser_crystal_bismuth", Wavelength::Visible, 11_250),
            ("ntm:laser_crystal_cmb", Wavelength::Uv, 33_750),
            ("ntm:laser_crystal_dnt", Wavelength::Gamma, 101_250),
            ("ntm:laser_crystal_digamma", Wavelength::Drx, 303_750),
        ] {
            assert_eq!(Wavelength::from_crystal(id), mode);
            assert_eq!(mode.cost(), cost);
            let mut fel = Fel { on: true, power: cost, ..Default::default() };
            fel.slots[1] = Some(Stack::new(id, 1));
            assert_eq!(fel.begin_tick(), Some(mode));
            assert_eq!(fel.power, 0);
        }
        assert_eq!(Wavelength::from_crystal("minecraft:stone"), Wavelength::Null);
    }

    #[test]
    fn disabled_invalid_and_underpowered_behaviour() {
        let mut fel = Fel { power: 3_749, ..Default::default() };
        fel.slots[1] = Some(Stack::new("ntm:laser_crystal_co2", 1));
        assert_eq!(fel.begin_tick(), None);
        assert_eq!(fel.power, 3_749);
        fel.on = true;
        assert_eq!(fel.begin_tick(), None);
        assert_eq!(fel.power, 0);
        assert_eq!(fel.mode, Wavelength::Ir);
        fel.power = 500;
        fel.slots[1] = Some(Stack::new("minecraft:stone", 1));
        assert_eq!(fel.begin_tick(), None);
        assert_eq!((fel.mode, fel.power), (Wavelength::Null, 500));
    }

    #[test]
    fn state_survives_json_roundtrip() {
        let fel = Fel { power: 123_456, on: true, mode: Wavelength::Uv, distance: 17, missing_valid_silex: false, ..Default::default() };
        let json = serde_json::to_string(&fel).unwrap();
        assert_eq!(serde_json::from_str::<Fel>(&json).unwrap(), fel);
    }
}
