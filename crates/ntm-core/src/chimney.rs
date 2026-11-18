//! The chimneys (`ntm:chimney_brick`, `ntm:chimney_industrial`; `MachineChimneyBrick.java`, `MachineChimneyIndustrial.java`,
//! `TileEntityChimneyBase.java` and its two bodies).
//!
//! A 3x3 base with a tall shaft over it (13 and 23 blocks) whose four side cells at the foot (`getAllPorts`, the `flare` shape: the
//! cell on each side of the core, open outwards) take smoke from the fluid network: only `smoke`, `smoke_leaded` and `smoke_poison`
//! (`canConnect`), whatever the pressure and as much as comes (`getDemand` is 1 000 000, `transferFluid` returns 0, so everything is
//! consumed). Each tick it takes smoke the machine is "on" for 20 ticks (the client's smoke column), and what came in as ash
//! (and, in the industrial one, as soot) is added to the ashpit right under the core (`updateEntity`).
//!
//! What is not here: the pollution it releases (`PollutionHandler.incrementPollution`: the port has no pollution), and the smoke column
//! of the client. The smoke goes nowhere (docs/onda-2-sumidouros.md).

use crate::{
    Pos,
    fluid::FluidNode,
    machine::Logic,
    server::container_bridge::world_cell,
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub const BRICK: &str = "ntm:chimney_brick";
pub const INDUSTRIAL: &str = "ntm:chimney_industrial";

/// The three smoke fluids (`Fluids.SMOKE`, `SMOKE_LEADED`, `SMOKE_POISON`).
pub const SMOKES: [&str; 3] = ["smoke", "smoke_leaded", "smoke_poison"];

/// `getDemand`: 1 000 000 for any type and pressure.
pub const DEMAND: u64 = 1_000_000;

/// `onTicks = 20` when smoke arrives.
pub const ON_TICKS: i32 = 20;

/// The four ports in the placer's frame, each with the way out (`MachineChimney*.getAllPorts`: east, west, south, north).
pub const PORTS: &[(Pos, Pos)] = &[
    ([1, 0, 0], [1, 0, 0]),
    ([-1, 0, 0], [-1, 0, 0]),
    ([0, 0, 1], [0, 0, 1]),
    ([0, 0, -1], [0, 0, -1]),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Body {
    #[default]
    Brick,
    Industrial,
}

impl Body {
    pub fn of(block: &str) -> Option<Body> {
        match block {
            BRICK => Some(Body::Brick),
            INDUSTRIAL => Some(Body::Industrial),
            _ => None,
        }
    }

    /// `capturesAsh` (the base's `true`) and `capturesSoot` (the industrial one's).
    fn captures_soot(self) -> bool {
        self == Body::Industrial
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Chimney {
    pub body: Body,
    /// `ashTick` and `sootTick`: the smoke taken since the last time they went to the ashpit (not saved by the Java).
    #[serde(skip)]
    pub ash: i64,
    #[serde(skip)]
    pub soot: i64,
    /// `onTicks`.
    #[serde(skip)]
    pub on: i32,
}

impl Chimney {
    pub fn new(body: Body) -> Self {
        Self { body, ..Default::default() }
    }

    /// The ports of the chimney at `core`: (cell, way out).
    pub fn port_cells(core: Pos, facing: Option<&str>) -> Vec<(Pos, Pos)> {
        let Some(f) = facing.and_then(crate::registry::facing_dir) else { return Vec::new() };
        PORTS.iter().map(|&(cell, face)| (world_cell(core, f, cell), world_cell([0, 0, 0], f, face))).collect()
    }

    /// What goes to the ashpit under: the ash and the soot taken, and the two cleared (`updateEntity`).
    pub fn take_ash(&mut self) -> Option<(i64, i64)> {
        if self.ash > 0 || self.soot > 0 {
            Some((std::mem::take(&mut self.ash), std::mem::take(&mut self.soot)))
        } else {
            None
        }
    }
}

impl Logic for Chimney {
    /// `if(onTicks > 0) onTicks--`.
    fn tick(&mut self) -> bool {
        if self.on > 0 {
            self.on -= 1;
        }
        false
    }

    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(self)
    }

    fn level(&self, _max: u8) -> u8 {
        0
    }

    fn view(&self) -> ViewModel {
        ViewModel::new(match self.body {
            Body::Brick => "Chaminé de tijolo",
            Body::Industrial => "Chaminé industrial",
        })
        .text(if self.on > 0 { "Soltando fumaça" } else { "Parada" }, view::GREY)
    }
}

fn smoke(fluid: &str) -> bool {
    SMOKES.contains(&fluid)
}

impl FluidNode for Chimney {
    fn fluid_offer(&self, _fluid: &str) -> u64 {
        0
    }

    fn fluid_demand(&self, fluid: &str) -> u64 {
        if smoke(fluid) { DEMAND } else { 0 }
    }

    fn fluid_types(&self) -> Vec<String> {
        SMOKES.iter().map(|s| s.to_string()).collect()
    }

    /// `getDemand(type, pressure)` does not look at the pressure.
    fn fluid_demand_at(&self, fluid: &str, _pressure: u8) -> u64 {
        self.fluid_demand(fluid)
    }

    fn fluid_give(&mut self, fluid: &str, mb: u64) {
        if !smoke(fluid) {
            return;
        }
        self.on = ON_TICKS;
        // `ashTick += fluid`: it is a `long`, a wrap would need 2^63 mB.
        let mb = mb.min(i64::MAX as u64) as i64;
        self.ash = self.ash.saturating_add(mb);
        if self.body.captures_soot() {
            self.soot = self.soot.saturating_add(mb);
        }
    }

    /// `transferFluid(type, pressure, fluid)`: whatever the pressure.
    fn fluid_give_at(&mut self, fluid: &str, _pressure: u8, mb: u64) {
        self.fluid_give(fluid, mb);
    }

    fn fluid_take(&mut self, _fluid: &str, _mb: u64) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_takes_only_smoke_at_any_pressure_and_all_of_it() {
        assert_eq!((DEMAND, ON_TICKS), (1_000_000, 20), "getDemand and onTicks");
        let c = Chimney::new(Body::Brick);
        for f in SMOKES {
            assert_eq!((c.fluid_demand(f), c.fluid_demand_at(f, 0), c.fluid_demand_at(f, 5), c.fluid_offer(f)), (DEMAND, DEMAND, DEMAND, 0), "{f}");
        }
        for f in ["water", "steam", "air", ""] {
            assert_eq!((c.fluid_demand(f), c.fluid_demand_at(f, 3)), (0, 0), "{f}");
        }
        assert_eq!(c.fluid_types(), SMOKES.map(String::from).to_vec());
    }

    #[test]
    fn what_comes_in_is_ash_and_the_industrial_one_adds_soot_and_both_switch_on_for_20_ticks() {
        let mut brick = Chimney::new(Body::Brick);
        brick.fluid_give("smoke", 300);
        brick.fluid_give_at("smoke_leaded", 4, 50);
        brick.fluid_give("water", 999);
        assert_eq!((brick.ash, brick.soot, brick.on), (350, 0, ON_TICKS));
        let mut industrial = Chimney::new(Body::Industrial);
        industrial.fluid_give("smoke_poison", 70);
        assert_eq!((industrial.ash, industrial.soot, industrial.on), (70, 70, ON_TICKS));
        // The take empties both, once.
        assert_eq!(industrial.take_ash(), Some((70, 70)));
        assert_eq!(industrial.take_ash(), None);
    }

    #[test]
    fn it_counts_its_on_ticks_down_and_stops_at_zero() {
        let mut c = Chimney::new(Body::Brick);
        c.fluid_give("smoke", 1);
        for expected in (0..ON_TICKS).rev() {
            c.tick();
            assert_eq!(c.on, expected);
        }
        c.tick();
        assert_eq!(c.on, 0);
        c.fluid_give("smoke", 1);
        assert_eq!(c.on, ON_TICKS, "smoke again starts it over");
    }

    #[test]
    fn the_ports_are_the_four_cells_beside_the_core_open_outwards_for_every_facing() {
        for facing in ["north", "south", "east", "west"] {
            let ports = Chimney::port_cells([10, 70, 10], Some(facing));
            let mut cells: Vec<_> = ports.iter().map(|(c, _)| *c).collect();
            cells.sort();
            assert_eq!(cells, vec![[9, 70, 10], [10, 70, 9], [10, 70, 11], [11, 70, 10]], "{facing}");
            for (cell, face) in ports {
                assert_eq!([cell[0] - 10, cell[1] - 70, cell[2] - 10], face, "{facing}: the way out is away from the core");
            }
        }
        assert!(Chimney::port_cells([0, 0, 0], None).is_empty());
    }

    #[test]
    fn the_bodies_come_from_the_block_and_nothing_is_saved_but_the_body() {
        assert_eq!((Body::of(BRICK), Body::of(INDUSTRIAL), Body::of("ntm:machine_drain")), (Some(Body::Brick), Some(Body::Industrial), None));
        let mut c = Chimney::new(Body::Industrial);
        c.fluid_give("smoke", 500);
        let back: Chimney = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!((back.body, back.ash, back.soot, back.on), (Body::Industrial, 0, 0, 0));
    }
}
