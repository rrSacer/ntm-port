//! NTM port: the logic the native bundle runs (docs/nativo.md).
//!
//! Nothing here calls the game: the bundle's glue hands the server a host
//! ([`server::ServerHost`]) that does. So every piece (machines, energy,
//! fluids, bombs, the world's state) runs in `cargo test` against a fake
//! world.

pub mod anvil;
pub mod arc_furnace;
pub mod bedrock;
pub mod blast;
pub mod broken;
pub mod blastfurnace;
pub mod blockstate;
pub mod boiler;
pub mod bomb;
pub mod client_rad;
pub mod coker;
pub mod compressor;
pub mod deuterium;
pub mod combustion;
pub mod solar_stirling;
pub mod crafter;
pub mod doors_security;
pub mod electrolyser;
pub mod energy;
pub mod energy_network;
pub mod explosion;
pub mod excavator;
pub mod fluid;
pub mod fluidbox;
pub mod fuel;
pub mod oredict;
pub mod upgrades;
pub mod welders;
pub mod ashpit;
pub mod furnace_brick;
pub mod furnace_combination;
pub mod furnace_steel;
pub mod furnaces;
pub mod foundry;
pub mod fraction;
pub mod gascent;
pub mod hash;
pub mod heater;
pub mod kv;
pub mod consumables;
pub mod instruments;
pub mod intake;
pub mod item_hooks;
pub mod ledger;
pub mod machine;
pub mod native;
pub mod oil_drill;
pub mod persist;
pub mod processing;
pub mod press;
pub mod recipes;
pub mod refinery;
pub mod screens;
pub mod shredder;
pub mod radiation;
pub mod radioisotopes;
pub mod registry;
pub mod server;
pub mod snbt;
pub mod steam;
pub mod view;
pub mod walking;
pub mod chunk_radiation;
pub mod field_sources;
pub mod geiger_block;

/// The pool allocator of the explosion engine, for the server mod's
/// `#[global_allocator]` (single-threaded WebAssembly only).
pub use crate::explosion::pool::Pool;

/// A block position.
pub type Pos = [i32; 3];

/// The protocol's version: both mods refuse an envelope of another.
pub const PROTOCOL: u32 = 1;

/// The six neighbours of a position.
pub fn neighbours(p: Pos) -> [Pos; 6] {
    let [x, y, z] = p;
    [
        [x + 1, y, z],
        [x - 1, y, z],
        [x, y + 1, z],
        [x, y - 1, z],
        [x, y, z + 1],
        [x, y, z - 1],
    ]
}

/// Reads every recipe table now. They are read lazily, on first use: in
/// the game the first tick after a world loads would read most of them at
/// once and run out of the tick's fuel (50M), so the server mod reads them
/// in its init, which has 2 000M.
pub fn warm_tables() {
    for s in crafter::Station::ALL {
        s.recipes();
    }
    arc_furnace::warm();
    refinery::recipes();
    fraction::recipes();
    boiler::steps();
    coker::recipes();
    heater::tables();
    for s in [processing::Shape::Cracker, processing::Shape::Reformer, processing::Shape::Hydrotreater, processing::Shape::Vacuum] {
        s.recipes();
    }
    electrolyser::metal_recipes();
    foundry::Tables::ntm();
    shredder::Recipes::ntm();
    anvil::Tables::ntm();
    blastfurnace::Tables::ntm();
    recipes::furnace();
    recipes::press();
    radiation::hazards();
    combustion::warm();
    radiation::bits();
    fluid::types();
    bedrock::deposit(0, 0);
}

pub mod pile;

pub mod zirnox;
pub mod petroleo_e;
pub mod watz_pwr;

pub mod rbmk;

pub mod sounds;
