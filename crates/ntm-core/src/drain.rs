//! The drain (`ntm:machine_drain`, `MachineDrain.java`, `TileEntityMachineDrain.java`, `RenderDrain.java`).
//!
//! A 1x1x3 multiblock (`{0,0,2,0,0,0}`, offset 0) with no screen and no inventory: one 2 000 mB tank that a
//! fluid identifier types (a click, not sneaking) and the fluid network fills through the core's front and two
//! sides (`getPorts`: `dir`, and `dir` turned up and down, nothing above or below, `canConnect`). Every tick
//! it spills half of what it holds, at least 1 mB (`max(fill / 2, 1)`), and each fluid's `onFluidRelease` runs.
//!
//! What is not here (nominal gaps, docs/onda-2-sumidouros.md): the spill's pollution
//! (`FT_Polluting`: no `PollutionHandler` in the port), the oil puddle (`oil_spill` is not in the catalog), the
//! antimatter explosion (the fluid stays in the tank) and the particles of the client.

use crate::{
    Pos,
    energy_network::Dir,
    fluid::{FluidNode, Tank},
    machine::Logic,
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub const BLOCK: &str = "ntm:machine_drain";

/// `new FluidTank(Fluids.NONE, 2_000)`.
pub const CAP: u32 = 2_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Drain {
    pub tank: Tank,
    /// Radiation (`FT_VentRadiation`) the last tick released into the chunk, taken by the server.
    #[serde(skip)]
    pub rad: f32,
}

impl Default for Drain {
    fn default() -> Self {
        Self { tank: Tank::new(CAP), rad: 0.0 }
    }
}

impl Drain {
    /// `ForgeDirection.getOrientation(getBlockMetadata() - 10)`: the block's facing.
    pub fn faces(facing: Option<&str>) -> Vec<Pos> {
        let Some(dir) = crate::storage::java_dir(facing) else { return Vec::new() };
        let up = dir.rotate_up();
        let down = rotate_down(dir);
        [dir, up, down].into_iter().map(Dir::offset).collect()
    }

    /// The fluid identifier in a hand: `drain.tank.setTankType(type)` (the content goes with a new type).
    pub fn set_type(&mut self, fluid: &str) {
        self.tank.set_type(fluid);
    }

    /// `updateEntity`: half of the fill (at least 1 mB) is spilled and released. True when the tank changed.
    pub fn step(&mut self) -> bool {
        self.rad = 0.0;
        if self.tank.mb == 0 {
            return false;
        }
        // `FT_Amat`: the Java blows the block up (`newExplosion` 10) and goes no further; here the fluid stays.
        if crate::fluid::get(&self.tank.fluid).is_some_and(|t| t.antimatter) {
            return false;
        }
        let spill = (self.tank.mb / 2).max(1);
        self.tank.mb -= spill;
        // `FT_VentRadiation.onFluidRelease`: `incrementRad(world, x, y, z, overflowAmount * radPerMB)`.
        self.rad = crate::fluid::get(&self.tank.fluid).map_or(0.0, |t| t.vent_radiation) * spill as f32;
        true
    }
}

/// `ForgeDirection.getRotation(DOWN)`: turned the other way round the vertical axis.
fn rotate_down(d: Dir) -> Dir {
    d.rotate_up().opposite()
}

impl Logic for Drain {
    fn tick(&mut self) -> bool {
        self.step()
    }

    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(self)
    }

    fn level(&self, _max: u8) -> u8 {
        0
    }

    fn view(&self) -> ViewModel {
        ViewModel::new("Ralo").text(self.tank.line(), view::GREY)
    }
}

impl FluidNode for Drain {
    fn fluid_offer(&self, _fluid: &str) -> u64 {
        0
    }

    /// `getFluidPriority`: `LOW`, served after every normal receiver.
    fn fluid_priority(&self) -> u8 {
        crate::fluid::PRIORITY_LOW
    }

    /// A receiving tank takes only its own type: an untyped drain takes nothing.
    fn fluid_demand(&self, fluid: &str) -> u64 {
        if !self.tank.fluid.is_empty() && fluid == self.tank.fluid { u64::from(self.tank.free()) } else { 0 }
    }

    fn fluid_types(&self) -> Vec<String> {
        if self.tank.fluid.is_empty() { Vec::new() } else { vec![self.tank.fluid.clone()] }
    }

    fn fluid_give(&mut self, fluid: &str, mb: u64) {
        if fluid == self.tank.fluid {
            self.tank.fill(fluid, mb.min(u64::from(u32::MAX)) as u32);
        }
    }

    fn fluid_take(&mut self, _fluid: &str, _mb: u64) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(fluid: &str, mb: u32) -> Drain {
        let mut d = Drain::default();
        d.set_type(fluid);
        d.tank.mb = mb;
        d
    }

    #[test]
    fn it_spills_half_of_the_fill_a_tick_and_at_least_one() {
        let mut d = typed("water", 2_000);
        let mut seen = Vec::new();
        // Bounded: a drain that spilled nothing from 1 mB would never empty.
        for _ in 0..20 {
            if d.tank.mb == 0 {
                break;
            }
            assert!(d.step());
            seen.push(d.tank.mb);
        }
        assert_eq!(seen, vec![1_000, 500, 250, 125, 63, 32, 16, 8, 4, 2, 1, 0]);
        assert!(!d.step(), "empty: nothing changes");
    }

    #[test]
    fn it_takes_only_the_type_it_was_given_and_only_as_much_as_it_holds_room_for() {
        let mut d = Drain::default();
        assert_eq!((d.fluid_demand("water"), d.fluid_types()), (0, Vec::<String>::new()), "untyped: nothing");
        assert_eq!(d.fluid_demand(""), 0, "not even the no-type");
        assert_eq!(d.fluid_priority(), crate::fluid::PRIORITY_LOW);
        d.set_type("water");
        assert_eq!((d.fluid_demand("water"), d.fluid_demand("lava"), d.fluid_offer("water")), (2_000, 0, 0));
        d.fluid_give("water", 1_500);
        d.fluid_give("lava", 400);
        assert_eq!(d.tank.mb, 1_500);
        assert_eq!(d.fluid_demand("water"), 500);
        d.fluid_give("water", 5_000);
        assert_eq!(d.tank.mb, 2_000, "the tank holds 2 000");
        d.fluid_take("water", 100);
        assert_eq!(d.tank.mb, 2_000, "nothing leaves by the network");
        assert_eq!(d.fluid_types(), vec!["water".to_string()]);
    }

    #[test]
    fn a_new_type_empties_the_tank_the_same_one_does_not() {
        let mut d = typed("water", 700);
        d.set_type("water");
        assert_eq!(d.tank.mb, 700);
        d.set_type("lava");
        assert_eq!((d.tank.fluid.as_str(), d.tank.mb), ("lava", 0));
    }

    #[test]
    fn a_radioactive_fluid_vents_its_radiation_per_mb_spilled() {
        let t = crate::fluid::get("uf6").expect("uf6");
        assert!(t.vent_radiation > 0.0);
        let mut d = typed("uf6", 1_000);
        d.step();
        assert_eq!(d.tank.mb, 500);
        assert_eq!(d.rad, t.vent_radiation * 500.0);
        let mut plain = typed("water", 1_000);
        plain.step();
        assert_eq!(plain.rad, 0.0);
        // The reading is of the tick that made it.
        d.tank.mb = 0;
        d.step();
        assert_eq!(d.rad, 0.0);
    }

    #[test]
    fn antimatter_stays_in_the_tank() {
        let t = crate::fluid::get("amat").expect("amat");
        assert!(t.antimatter);
        let mut d = typed("amat", 1_000);
        assert!(!d.step());
        assert_eq!(d.tank.mb, 1_000);
    }

    #[test]
    fn the_ports_are_the_front_and_the_two_sides_and_never_above_or_below() {
        // `getPorts`: dir, dir turned up, dir turned down.
        let mut cases = Vec::new();
        for facing in ["north", "east", "south", "west"] {
            let f = Drain::faces(Some(facing));
            assert_eq!(f.len(), 3, "{facing}");
            assert!(f.iter().all(|d| d[1] == 0), "horizontal only");
            let front = crate::registry::facing_dir(facing).unwrap();
            assert_eq!(f[0], front, "the front is the block's facing");
            assert_eq!(f[1], [-f[2][0], 0, -f[2][2]], "the sides are opposite");
            assert_ne!(f[1], f[0]);
            cases.push(f);
        }
        // north: dir north, turned up east, turned down west.
        assert_eq!(cases[0], vec![[0, 0, -1], [1, 0, 0], [-1, 0, 0]]);
        assert!(Drain::faces(None).is_empty());
    }

    #[test]
    fn it_saves_its_type_and_its_fill_and_not_the_radiation_of_a_tick() {
        let mut d = typed("uf6", 300);
        d.step();
        let json = serde_json::to_string(&d).unwrap();
        let back: Drain = serde_json::from_str(&json).unwrap();
        assert_eq!((back.tank.fluid.as_str(), back.tank.mb, back.rad), ("uf6", 150, 0.0));
    }
}
