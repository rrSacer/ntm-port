//! RBMK reactor: the pure simulation core (`rbmk-plano` T01..T04, T07..T09).
//!
//! Everything here is plain data and arithmetic, without the engine: dials
//! (`RBMKDials`), fuel rods (`ItemRBMKRod`), the flux/heat step of the
//! columns, the control rods, the boiler/heater/cooler/outgasser/loader.
//! Each module is cross-checked against the Java oracle in
//! `tools/rbmk-oracle/` (see `tests/rbmk_oracle_parity.rs`). What needs the
//! engine (debris entities, world blocks, menus, saving) is not here; see
//! `docs/rbmk-core.md` for the list of gaps and of what is left to wire.

pub mod console;
pub mod console_gui;
pub mod dials;
pub mod dump;
pub mod fluids;
pub mod hazards;
pub mod jmath;
pub mod jrandom;
pub mod outgasser;
pub mod persist;
pub mod reactor;
pub mod rods;
pub mod screen;
pub mod tooltip;
mod rods_data;
mod tables_data;
