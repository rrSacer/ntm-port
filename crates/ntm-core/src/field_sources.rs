//! The world's radiation sources that are blocks (Onda 1, slice 4d): what each one gives the radiation by chunk
//! (`incrementRad`) on its tick. docs/onda-1-protecao.md, "4d", has the inventory with every Java source.
//!
//! - `BlockHazard` (`:120-146`; with `BlockNuclearWaste` and `BlockHotHazard`) and `BlockHazardFalling` (`:39-66`):
//!   every 20 ticks (`tickRate`, scheduled from `onBlockAdded` and again from each tick), the item's radiation level
//!   in the `HazardRegistry` times 0.1. That `rad` is a field of the `Block`, one for the type, written only by
//!   `onBlockAdded`: the glue keeps the types "armed" this way ([`Kind::Hazard`]).
//! - `BlockOre` with a `rad` (`:199-226`, `ore_schrabidium`): on its random ticks and every 20 ticks from them.
//! - `YellowBarrel` (`:73-92`): every 20 ticks from `onBlockAdded`, 5 for the yellow barrel, 0.5 for the vitrified.
//! - `BlockSellafield` (`:56`): on its random tick, 0.5 times its level plus one ([`sellafield_rad`]).

/// `tickRate` of every source here: 20 ticks.
pub const TICK_RATE: i32 = 20;

/// How a source block ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `BlockHazard`, `BlockHazardFalling`: scheduled from `onBlockAdded`, giving and rescheduling only while its type
    /// is armed (a block of it added in this run of the game).
    Hazard,
    /// `BlockOre` with a `rad`: random ticks, and scheduled from `onBlockAdded` and from each tick.
    Ore,
    /// `YellowBarrel`: scheduled from `onBlockAdded` and from each tick.
    Barrel,
}

/// A source block: how it ticks and what it gives a tick (RAD/s of the field).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Source {
    pub kind: Kind,
    pub rad: f32,
}

/// The `BlockHazard` blocks (`ModBlocks:1338-1422`; `block_waste*` are `BlockNuclearWaste`, `block_polonium` and
/// `block_pu238` `BlockHotHazard`) and the `BlockHazardFalling` ones (`:1398, 1406, 1407`).
const HAZARD: [&str; 32] = [
    "ntm:block_uranium",
    "ntm:block_u233",
    "ntm:block_u235",
    "ntm:block_u238",
    "ntm:block_uranium_fuel",
    "ntm:block_thorium",
    "ntm:block_thorium_fuel",
    "ntm:block_neptunium",
    "ntm:block_mox_fuel",
    "ntm:block_plutonium",
    "ntm:block_pu239",
    "ntm:block_pu240",
    "ntm:block_pu_mix",
    "ntm:block_plutonium_fuel",
    "ntm:block_trinitite",
    "ntm:block_corium",
    "ntm:block_schraranium",
    "ntm:block_schrabidium",
    "ntm:block_schrabidate",
    "ntm:block_solinium",
    "ntm:block_schrabidium_fuel",
    "ntm:block_white_phosphorus",
    "ntm:block_ra226",
    "ntm:block_actinium",
    "ntm:block_waste",
    "ntm:block_waste_painted",
    "ntm:block_waste_vitrified",
    "ntm:block_polonium",
    "ntm:block_pu238",
    "ntm:block_yellowcake",
    "ntm:block_red_phosphorus",
    "ntm:block_fallout",
];

/// The source a block is, if any. A hazard block whose item has no radiation (the phosphorus) gives nothing: its
/// `rad` stays 0 and `onBlockAdded` schedules no tick.
pub fn block_source(block: &str) -> Option<Source> {
    let id = crate::blockstate::id_of(block);
    let source = match id {
        _ if HAZARD.contains(&id) => {
            // HazardSystem.getHazardLevelFromStack(new ItemStack(this), RADIATION) * 0.1F, in float.
            let level = crate::radiation::hazards().iter().find(|h| h.id == id).map_or(0.0, |h| h.radiation_rad_s);
            Source { kind: Kind::Hazard, rad: level * 0.1 }
        }
        // ModBlocks:1248, new BlockOre(Material.rock, 0.1F, 0.5F).
        "ntm:ore_schrabidium" => Source { kind: Kind::Ore, rad: 0.1 },
        "ntm:yellow_barrel" => Source { kind: Kind::Barrel, rad: 5.0 },
        "ntm:vitrified_barrel" => Source { kind: Kind::Barrel, rad: 0.5 },
        _ => return None,
    };
    (source.rad > 0.0).then_some(source)
}

/// `BlockSellafield.updateTick:56`: `this.rad * (meta + 1)`, `rad` 0.5; the level is the block's (`__<n>`).
pub fn sellafield_rad(block: &str) -> Option<f32> {
    let id = crate::blockstate::id_of(block);
    let level: u8 = match id.strip_prefix("ntm:sellafield") {
        Some("") => 0,
        Some(rest) => rest.strip_prefix("__")?.parse().ok()?,
        None => return None,
    };
    Some(0.5 * f32::from(level + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hazard_blocks_give_a_tenth_of_their_items_level() {
        let uranium = block_source("ntm:block_uranium").unwrap();
        assert_eq!(uranium, Source { kind: Kind::Hazard, rad: 3.5f32 * 0.1 });
        assert_eq!(block_source("ntm:block_polonium").unwrap().rad, 750.0f32 * 0.1);
        assert_eq!(block_source("ntm:block_fallout").unwrap(), Source { kind: Kind::Hazard, rad: 10.5f32 * 0.1 });
        assert_eq!(block_source("ntm:block_white_phosphorus"), None, "no radiation: rad 0, no tick");
        assert_eq!(block_source("ntm:block_red_phosphorus"), None);
        // Every other one is in the table of radioactive items (the phosphorus are not: no radiation).
        for id in HAZARD.iter().filter(|id| !id.ends_with("_phosphorus")) {
            assert!(crate::radiation::hazards().iter().any(|h| h.id == *id), "{id} in the hazard table");
        }
    }

    #[test]
    fn the_ore_and_the_barrels_have_their_constructors_values() {
        assert_eq!(block_source("ntm:ore_schrabidium"), Some(Source { kind: Kind::Ore, rad: 0.1 }));
        assert_eq!(block_source("ntm:yellow_barrel"), Some(Source { kind: Kind::Barrel, rad: 5.0 }));
        assert_eq!(block_source("ntm:vitrified_barrel"), Some(Source { kind: Kind::Barrel, rad: 0.5 }));
        assert_eq!(block_source("ntm:ore_uranium"), None, "BlockOutgas: BlockOre without a rad");
        assert_eq!(block_source("minecraft:stone"), None);
    }

    #[test]
    fn sellafield_gives_half_its_level_plus_one() {
        assert_eq!(sellafield_rad("ntm:sellafield"), Some(0.5));
        assert_eq!(sellafield_rad("ntm:sellafield__3"), Some(2.0));
        assert_eq!(sellafield_rad("ntm:sellafield__5"), Some(3.0));
        assert_eq!(sellafield_rad("ntm:sellafield_slaked"), None);
    }
}
