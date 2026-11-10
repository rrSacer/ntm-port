//! Pure domain of the Watz reactor and the pressurized water reactor (PWR).
//!
//! Nothing here touches the server, the save or the menus: the functions take plain state and
//! return the new state plus the world effects they request. Wiring to the tick, persistence
//! and GUI is still to do (after Onda 0).
//!
//! **Left out on purpose (named gaps):**
//! - Watz mud overflow only returns [`watz::WatzEvent::MudOverflow`]; the shrapnel entities, the
//!   random ruined/mud columns, the radiation injection into the chunk and the achievement need
//!   the entity and world-effect systems that do not exist yet.
//! - PWR meltdown only returns the rods and the explosion centre; corium blocks, the explosion and
//!   `SatelliteRayScan` are the caller's. The fluid-identifier slot that picks the cold fluid, the
//!   OpenComputers/ROR plumbing other than the numeric rules, the printer item and audio are not
//!   ported either.
//! - Port discovery, `TileEntityProxyCombo` forwarding and the multiblock dummy blocks are
//!   world-side; only the geometry (ports, dimensions) is recorded.
pub mod function;
pub mod heatable;
pub mod pwr;
pub mod watz;
