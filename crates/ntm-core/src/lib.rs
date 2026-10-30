//! NTM port: the logic shared by `ntm-logic-server` and `ntm-logic-client`
//! (docs/fundacao.md).
//!
//! Nothing here calls the Horizon API: the mods hand the engines a host
//! ([`server::ServerHost`], [`client::ClientHost`]) that does. So every piece
//! (protocol, persistence, world identity, machines, energy) runs in
//! `cargo test` against a fake world.

pub mod anvil;
pub mod arc_furnace;
pub mod bedrock;
pub mod blast;
pub mod blastfurnace;
pub mod blockstate;
pub mod boiler;
pub mod bomb;
pub mod client;
pub mod coker;
pub mod crafter;
pub mod electrolyser;
pub mod energy;
pub mod excavator;
pub mod fluid;
pub mod fluidbox;
pub mod foundry;
pub mod fraction;
pub mod gascent;
pub mod hash;
pub mod kv;
pub mod machine;
pub mod persist;
pub mod processing;
pub mod press;
pub mod probes;
pub mod proto;
pub mod recipes;
pub mod refinery;
pub mod shredder;
pub mod radiation;
pub mod registry;
pub mod server;
pub mod snbt;
pub mod token;
pub mod tx;
pub mod view;
pub mod worldid;

/// The pool allocator of the explosion engine, for the server mod's
/// `#[global_allocator]` (single-threaded WebAssembly only).
pub use ntm_explosion::pool::Pool;

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
    radiation::bits();
    fluid::types();
    bedrock::deposit(0, 0);
}
