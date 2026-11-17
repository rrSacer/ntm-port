//! Air intake (`ntm:machine_intake`, `MachineIntake.java`,
//! `TileEntityMachineIntake.java`, `RenderIntake.java`).
//!
//! A 2×1×2 multiblock (`{0,0,1,0,1,0}`, offset 0) with no GUI (`onBlockActivated`
//! is not overridden; only `ILookOverlay`). Each tick it holds at least
//! `MAX_POWER / 20` (100 HE), it refills its 1 000 mB `air` tank to capacity
//! and drains 100 HE, even if the tank was already full. Power and fluid connect
//! only on the horizontal outward faces of its 4 cells (`dir != UP && dir != DOWN`,
//! `TilePortShapes.solderer`).

use crate::{
    Pos,
    energy::{DIRS, EnergyNode, Role},
    fluid::{FluidNode, Tank},
    machine::Logic,
    server::container_bridge::{Slots, Slotted},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub const BLOCK: &str = "ntm:machine_intake";

/// `TileEntityMachineIntake.getMaxPower`: 2 000 HE (`TileEntityMachineIntake.java:131`).
pub const MAX_POWER: u64 = 2_000;
/// `this.getMaxPower() / 20`: 100 HE/tick (`TileEntityMachineIntake.java:45,47`).
pub const STEP_POWER: u64 = MAX_POWER / 20;
/// `new FluidTank(Fluids.AIR, 1_000)` (`TileEntityMachineIntake.java:31`).
pub const AIR_CAP: u32 = 1_000;
pub const FLUID: &str = "air";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Intake {
    #[serde(skip)]
    pub slots: Slots,
    pub power: u64,
    pub tank: Tank,
    /// Ran this tick (`power >= STEP_POWER` before subtracting): drives the
    /// client's fan animation (`data[0]`) and motor audio loop (`sounds::machine`).
    #[serde(skip)]
    pub on: bool,
}

impl Default for Intake {
    fn default() -> Self {
        Self {
            slots: Slots::new(),
            power: 0,
            tank: Tank::typed(FLUID, AIR_CAP),
            on: false,
        }
    }
}

impl Intake {
    pub fn new() -> Self {
        Self::default()
    }

    /// `TileEntityMachineIntake.updateEntity` (`TileEntityMachineIntake.java:45-48`):
    /// fills `compair` to `1_000` mB and drains `100` HE whenever `power >= 100`.
    pub fn step(&mut self) -> bool {
        let was_power = self.power;
        let was_mb = self.tank.mb;
        let was_on = self.on;
        self.on = self.power >= STEP_POWER;
        if self.on {
            self.tank.mb = self.tank.cap;
            self.power -= STEP_POWER;
        }
        self.power != was_power || self.tank.mb != was_mb || self.on != was_on
    }

    /// The 4 cells of the 2×1×2 multiblock and their outward horizontal port
    /// faces (`MachineIntake.getAllPorts`, `TilePortShapes.solderer`,
    /// `TileEntityMachineIntake.canConnect`: horizontal only, never `UP`/`DOWN`).
    pub fn port_cells(core: Pos, facing: Option<&str>) -> Vec<(Pos, Vec<Pos>)> {
        let Some(f) = facing.and_then(crate::registry::facing_dir) else {
            let horiz: Vec<Pos> = DIRS.iter().filter(|d| d[1] == 0).copied().collect();
            return vec![(core, horiz)];
        };
        let fwd = [-f[0], -f[2]];
        let right = [-fwd[1], fwd[0]];
        let cell = |lx: i32, lz: i32| {
            [
                core[0] + right[0] * lx + fwd[0] * lz,
                core[1],
                core[2] + right[1] * lx + fwd[1] * lz,
            ]
        };
        let dir = |v: [i32; 2]| [v[0], 0, v[1]];
        let pos_right = dir(right);
        let neg_right = dir([-right[0], -right[1]]);
        let pos_fwd = dir(fwd);
        let neg_fwd = dir([-fwd[0], -fwd[1]]);
        vec![
            (cell(0, 0), vec![pos_right, neg_fwd]),
            (cell(-1, 0), vec![neg_right, neg_fwd]),
            (cell(0, 1), vec![pos_right, pos_fwd]),
            (cell(-1, 1), vec![neg_right, pos_fwd]),
        ]
    }
}

impl Logic for Intake {
    fn tick(&mut self) -> bool {
        self.step()
    }

    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        Some(self)
    }

    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(self)
    }

    fn level(&self, _max: u8) -> u8 {
        0
    }

    fn view(&self) -> ViewModel {
        ViewModel::new("Tomada de ar")
            .bar("Energia", self.power, MAX_POWER, "HE")
            .text(self.tank.line(), view::GREY)
    }
}

impl EnergyNode for Intake {
    fn role(&self) -> Role {
        Role::Consumer
    }

    fn offer(&self) -> u64 {
        0
    }

    fn demand(&self) -> u64 {
        MAX_POWER.saturating_sub(self.power)
    }

    fn give(&mut self, he: u64) {
        self.power = (self.power + he).min(MAX_POWER);
    }

    fn take(&mut self, _he: u64) {}
}

impl FluidNode for Intake {
    fn fluid_offer(&self, fluid: &str) -> u64 {
        if fluid == FLUID && self.tank.fluid == FLUID {
            u64::from(self.tank.mb)
        } else {
            0
        }
    }

    fn fluid_demand(&self, _fluid: &str) -> u64 {
        0
    }

    fn fluid_types(&self) -> Vec<String> {
        vec![FLUID.into()]
    }

    fn fluid_give(&mut self, _fluid: &str, _mb: u64) {}

    fn fluid_take(&mut self, fluid: &str, mb: u64) {
        if fluid == FLUID && self.tank.fluid == FLUID {
            self.tank.drain(mb.min(u64::from(u32::MAX)) as u32);
        }
    }
}

impl Slotted for Intake {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }

    fn screen_data(&self) -> Vec<i32> {
        vec![i32::from(self.on)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_constants_match_java() {
        let intake = Intake::new();
        assert_eq!(MAX_POWER, 2_000);
        assert_eq!(STEP_POWER, 100);
        assert_eq!(AIR_CAP, 1_000);
        assert_eq!(intake.tank.fluid, "air");
        assert_eq!(intake.tank.cap, 1_000);
        assert_eq!(intake.tank.mb, 0);
        assert_eq!(intake.demand(), 2_000);
        assert_eq!(intake.offer(), 0);
        assert_eq!(intake.fluid_demand("air"), 0);
        assert_eq!(intake.fluid_offer("air"), 0);
        assert_eq!(intake.fluid_types(), vec!["air".to_string()]);
        assert_eq!(intake.screen_data(), vec![0]);
    }

    #[test]
    fn power_threshold_fills_air_even_when_already_full_and_tracks_on() {
        let mut intake = Intake::new();
        intake.give(99);
        assert!(!intake.step());
        assert_eq!(intake.power, 99);
        assert_eq!(intake.tank.mb, 0);
        assert!(!intake.on);
        assert_eq!(intake.screen_data(), vec![0]);

        intake.give(1);
        assert_eq!(intake.power, 100);
        assert!(intake.step());
        assert!(intake.on);
        assert_eq!(intake.power, 0);
        assert_eq!(intake.tank.mb, 1_000);
        assert_eq!(intake.screen_data(), vec![1]);
        assert_eq!(intake.fluid_offer("air"), 1_000);
        assert_eq!(intake.fluid_offer("water"), 0);

        // Next tick with 100 HE drains 100 HE again even though compair is still 1000 mB.
        intake.give(100);
        assert!(intake.step());
        assert_eq!(intake.power, 0);
        assert_eq!(intake.tank.mb, 1_000);
        assert!(intake.on);

        // Without power, turns off and emits changed=true so animation/audio stop.
        assert!(intake.step());
        assert!(!intake.on);
        assert_eq!(intake.screen_data(), vec![0]);
        assert!(!intake.step());
    }

    #[test]
    fn energy_cap_and_fluid_take_only_air() {
        let mut intake = Intake::new();
        intake.give(5_000);
        assert_eq!(intake.power, 2_000);
        assert_eq!(intake.demand(), 0);
        assert!(intake.step());
        assert_eq!(intake.power, 1_900);
        intake.fluid_take("water", 500);
        assert_eq!(intake.tank.mb, 1_000);
        intake.fluid_take("air", 350);
        assert_eq!(intake.tank.mb, 650);
        intake.fluid_give("air", 100);
        assert_eq!(intake.tank.mb, 650);
    }

    #[test]
    fn serde_roundtrip_keeps_power_and_tank_but_skips_transient_on() {
        let mut intake = Intake::new();
        intake.give(350);
        assert!(intake.step());
        assert!(intake.on);
        let json = serde_json::to_string(&intake).unwrap();
        let back: Intake = serde_json::from_str(&json).unwrap();
        assert_eq!(back.power, 250);
        assert_eq!(back.tank.mb, 1_000);
        assert_eq!(back.tank.fluid, "air");
        assert!(!back.on);
    }

    #[test]
    fn port_cells_match_solderer_shape_and_exclude_up_down() {
        let cells = Intake::port_cells([10, 64, 20], Some("north"));
        assert_eq!(cells.len(), 4);
        for (_, dirs) in &cells {
            assert_eq!(dirs.len(), 2);
            assert!(dirs.iter().all(|d| d[1] == 0));
        }
        // Core at [10, 64, 20] with facing=north (fwd=+Z, right=-X):
        // external faces are -X and -Z; cell[-1,0] = [11,64,20] (+X, -Z);
        // cell[0,1] = [10,64,21] (-X, +Z); cell[-1,1] = [11,64,21] (+X, +Z).
        assert_eq!(cells[0], ([10, 64, 20], vec![[-1, 0, 0], [0, 0, -1]]));
        assert_eq!(cells[1], ([11, 64, 20], vec![[1, 0, 0], [0, 0, -1]]));
        assert_eq!(cells[2], ([10, 64, 21], vec![[-1, 0, 0], [0, 0, 1]]));
        assert_eq!(cells[3], ([11, 64, 21], vec![[1, 0, 0], [0, 0, 1]]));
    }
}
