//! NTM port: the logic shared by `ntm-logic-server` and `ntm-logic-client`
//! (docs/fundacao.md).
//!
//! Nothing here calls the Horizon API: the mods hand the engines a host
//! ([`server::ServerHost`], [`client::ClientHost`]) that does. So every piece
//! (protocol, persistence, world identity, machines, energy) runs in
//! `cargo test` against a fake world.

pub mod blast;
pub mod blockstate;
pub mod bomb;
pub mod client;
pub mod crafter;
pub mod energy;
pub mod fluid;
pub mod fluidbox;
pub mod gascent;
pub mod hash;
pub mod kv;
pub mod machine;
pub mod persist;
pub mod press;
pub mod probes;
pub mod proto;
pub mod recipes;
pub mod shredder;
pub mod radiation;
pub mod registry;
pub mod server;
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
