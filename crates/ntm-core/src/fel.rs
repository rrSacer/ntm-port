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

impl crate::energy::EnergyNode for Fel {
    fn role(&self) -> crate::energy::Role { crate::energy::Role::Consumer }
    fn offer(&self) -> u64 { 0 }
    fn demand(&self) -> u64 { MAX_POWER.saturating_sub(self.power) }
    fn give(&mut self, amount: u64) { self.power = self.power.saturating_add(amount).min(MAX_POWER); }
    fn take(&mut self, _: u64) {}
}

impl crate::machine::Logic for Fel {
    // Beam simulation must be coupled with world effects by the server.
    fn energy(&mut self) -> Option<&mut dyn crate::energy::EnergyNode> { Some(self) }
    fn level(&self, max: u8) -> u8 { if self.on && self.mode != Wavelength::Null { max } else { 0 } }
    fn view(&self) -> crate::view::ViewModel {
        crate::view::ViewModel::new("Laser de elétrons livres")
            .text(format!("{} / {} HE", self.power, MAX_POWER), crate::view::GREY)
    }
}

impl crate::server::container_bridge::Slotted for Fel {
    fn slots_mut(&mut self) -> &mut Slots { &mut self.slots }
    fn screen_data(&self) -> Vec<i32> { vec![self.power as i32, MAX_POWER as i32, i32::from(self.on), self.mode as i32, self.distance, i32::from(self.missing_valid_silex)] }
    fn button(&mut self, id: u32) -> bool {
        if id == 0 { self.on = !self.on; }
        false
    }
    fn save_state(&self) -> Vec<i32> { vec![self.power as i32, i32::from(self.on), self.mode as i32, self.distance, i32::from(self.missing_valid_silex)] }
    fn load_state(&mut self, v: &[i32]) {
        if let [power, on, mode, distance, missing, ..] = v {
            self.power = (*power).clamp(0, MAX_POWER as i32) as u64;
            self.on = *on != 0;
            self.mode = match mode { 1 => Wavelength::Ir, 2 => Wavelength::Visible, 3 => Wavelength::Uv, 4 => Wavelength::Gamma, 5 => Wavelength::Drx, _ => Wavelength::Null };
            self.distance = *distance;
            self.missing_valid_silex = *missing != 0;
        }
    }
}

/// `TileEntityMachineBase` exposes no slots to automation; the menu has two plain slots.
pub fn allows(slot: usize, manual: bool) -> bool { slot < 2 && manual }
pub fn container() -> crate::native::Container {
    crate::native::Container { slots: 2, faces: std::array::from_fn(|_| Vec::new()), inputs: Vec::new(), outputs: Vec::new(), menu: Some("minecraft:generic_9x3") }
}

/// What Java observes in each beam cell. A SILEX refers to the core one
/// block beyond the struck part; an absent core falls through without stopping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BeamCell {
    Transparent,
    Obstacle { liquid: bool, resistance: f32 },
    Silex { core: Option<(bool, Wavelength)> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeamEffect {
    SetSilex { at: i32, mode: Wavelength },
    BreakSilex { at: i32 },
    RemoveLiquid { at: i32 },
    Ignite { at: i32, digamma: bool },
}

impl Fel {
    /// Trace the active beam. The caller supplies world observations and Java's
    /// one-in-five random decision; only an opaque obstacle draws that roll.
    /// Transparent cells reset the spacing flag as in the original loop.
    pub fn trace_beam(
        &mut self,
        mode: Wavelength,
        mut cell: impl FnMut(i32) -> BeamCell,
        mut one_in_five: impl FnMut() -> bool,
    ) -> Vec<BeamEffect> {
        let mut effects = Vec::new();
        let mut spacing = false;
        for i in 3..RANGE {
            match cell(i) {
                BeamCell::Transparent => {
                    self.distance = RANGE;
                    spacing = false;
                }
                BeamCell::Silex { core: Some((valid_rotation, existing)) } => {
                    if valid_rotation && i >= 5 && !spacing {
                        if existing != mode {
                            self.missing_valid_silex = false;
                            spacing = true;
                            effects.push(BeamEffect::SetSilex { at: i, mode });
                        }
                    } else {
                        effects.push(BeamEffect::BreakSilex { at: i });
                    }
                }
                BeamCell::Silex { core: None } => {}
                BeamCell::Obstacle { liquid, resistance } => {
                    self.distance = i;
                    if liquid {
                        effects.push(BeamEffect::RemoveLiquid { at: i });
                    } else if resistance < 75.0 && one_in_five() {
                        effects.push(BeamEffect::Ignite { at: i, digamma: mode == Wavelength::Drx });
                    }
                    break;
                }
            }
        }
        effects
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

    #[test]
    fn beam_range_obstacles_and_random_rolls() {
        let mut f = Fel::default();
        let mut visited = Vec::new();
        let mut rolls = 0;
        let effects = f.trace_beam(Wavelength::Drx, |i| {
            visited.push(i);
            if i == 8 { BeamCell::Obstacle { liquid: false, resistance: 74.9 } } else { BeamCell::Transparent }
        }, || { rolls += 1; true });
        assert_eq!(visited, (3..=8).collect::<Vec<_>>());
        assert_eq!(f.distance, 8);
        assert_eq!(rolls, 1);
        assert_eq!(effects, vec![BeamEffect::Ignite { at: 8, digamma: true }]);
        assert!(f.trace_beam(Wavelength::Ir, |_| BeamCell::Transparent, || panic!("no obstacle")).is_empty());
        assert_eq!(f.distance, 24);
        assert!(f.trace_beam(Wavelength::Ir, |_| BeamCell::Obstacle { liquid: false, resistance: 75.0 }, || panic!("too hard")).is_empty());
    }

    #[test]
    fn silex_alignment_spacing_and_existing_mode_follow_java() {
        let mut f = Fel::default();
        let effects = f.trace_beam(Wavelength::Uv, |i| match i {
            3 => BeamCell::Silex { core: Some((true, Wavelength::Null)) },
            5 | 6 => BeamCell::Silex { core: Some((true, Wavelength::Null)) },
            8 => BeamCell::Silex { core: Some((true, Wavelength::Uv)) },
            9 => BeamCell::Silex { core: Some((false, Wavelength::Null)) },
            _ => BeamCell::Transparent,
        }, || false);
        assert_eq!(effects, vec![BeamEffect::BreakSilex { at: 3 }, BeamEffect::SetSilex { at: 5, mode: Wavelength::Uv }, BeamEffect::BreakSilex { at: 6 }, BeamEffect::BreakSilex { at: 9 }]);
        assert!(!f.missing_valid_silex);
    }


    #[test]
    fn menu_button_energy_and_saved_state_are_roundtripped() {
        use crate::energy::EnergyNode;
        use crate::server::container_bridge::Slotted;
        let mut f = Fel::default();
        f.give(MAX_POWER + 1);
        assert_eq!(f.power, MAX_POWER);
        assert_eq!(f.demand(), 0);
        f.button(0);
        f.mode = Wavelength::Gamma;
        f.distance = 8;
        f.missing_valid_silex = false;
        let state = f.save_state();
        let mut loaded = Fel::default();
        loaded.load_state(&state);
        assert_eq!(loaded, f);
        assert!(allows(0, true) && allows(1, true));
        assert!(!allows(0, false) && !allows(2, true));
        assert!(container().faces.iter().all(Vec::is_empty));
    }

}
