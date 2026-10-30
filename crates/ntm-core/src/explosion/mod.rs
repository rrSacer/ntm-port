//! Motor de explosões nucleares do port do HBM's NTM para o Horizon.
//!
//! Reproduz a cratera do `ExplosionNukeRayBatched` do NTM (energia `2R`, custo
//! `res^(0,5 + 3,5·i/R)` por bloco) lendo cada bloco no máximo uma vez, e a
//! aplica em lotes por seção sem derrubar o TPS nem estourar o fuel do server
//! mod (ver `docs/explosoes.md`). Não depende do SDK: o mod implementa
//! [`World`], [`Store`] e, se quiser, [`Hooks`].
//!
//! ```ignore
//! // No server mod:
//! #[global_allocator]
//! static A: ntm_core::explosion::pool::Pool = ntm_core::explosion::pool::Pool::new();
//!
//! let mut ex = Explosions::new();
//! ex.resume(&mut store, 0, &mut NoHooks);          // em init: retoma após reinício
//! ex.detonate(Detonation::new("boy1", "minecraft:overworld", pos, 120), tick)?;
//! ex.tick(tick, &mut world, &mut store, &mut hooks); // todo tick
//! ```
pub mod crater;
pub mod effects;
pub mod engine;
pub mod fallout;
pub mod persist;
pub mod pool;

/// Marca de fuel (feature `trace`, só no harness `spikes/m2-fuel`): o host
/// anota o fuel restante em cada marca.
#[cfg(all(feature = "trace", target_arch = "wasm32"))]
pub fn trace(id: u32) {
    #[link(wasm_import_module = "host")]
    unsafe extern "C" {
        fn trace(id: u32);
    }
    unsafe { trace(id) }
}

#[cfg(not(all(feature = "trace", target_arch = "wasm32")))]
#[inline(always)]
pub fn trace(_id: u32) {}

pub use crater::{Kind, MAX_RADIUS, Plan};
pub use effects::{EdgeEffect, EdgeHit, EffectProfile, Event, Hooks, NoHooks};
pub use engine::{BLOCKS_PER_CALL, Change, DEFAULT_BUDGET, Detonation, Explosion, Explosions, Phase, Progress, World};
pub use fallout::FalloutProfile;
pub use persist::Store;
