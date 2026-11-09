//! What the K3 furnaces burn: `TileEntityFurnace.getItemBurnTime` of 1.7.10
//! plus the NTM's `handler/FuelHandler.java`, and the ash a fuel leaves
//! (`TileEntityFireboxBase.getAshFromFuel`, lines 171-188).
//!
//! The 1.7.10 rules are by block material and tool material; the port has
//! the modern ids, so wood is read from the id's family (planks, logs and
//! the like). Items that did not exist in 1.7.10 and have no NTM fuel burn 0.

/// `EnumAshType`, in the order that gives the item's meta (`powder_ash`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ash {
    Wood,
    Coal,
    Misc,
}

impl Ash {
    pub fn item(self) -> &'static str {
        match self {
            Ash::Wood => "ntm:powder_ash__wood",
            Ash::Coal => "ntm:powder_ash__coal",
            Ash::Misc => "ntm:powder_ash__misc",
        }
    }
}

fn path(item: &str) -> &str {
    item.split_once(':').map_or(item, |(_, p)| p)
}

/// The woods whose blocks are `Material.wood` (the six of 1.7.10 and the
/// modern ones that grow from them).
const WOODS: &[&str] = &[
    "oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "pale_oak",
    "bamboo", "crimson", "warped",
];

/// `p` is a block of a wood: `<wood><suffix>`, a stripped one too.
fn wood_of(p: &str, suffix: &str) -> bool {
    p.strip_suffix(suffix)
        .map(|w| w.strip_prefix("stripped_").unwrap_or(w))
        .is_some_and(|w| WOODS.contains(&w))
}

/// Blocks of `Material.wood` in 1.7.10 (their furnace value is 300).
fn wood_block(p: &str) -> bool {
    [
        "_planks",
        "_log",
        "_wood",
        "_stem",
        "_hyphae",
        "_fence",
        "_fence_gate",
        "_door",
        "_trapdoor",
        "_pressure_plate",
    ]
    .iter()
    .any(|s| wood_of(p, s))
        || matches!(
            p,
            "crafting_table" | "chest" | "trapped_chest" | "bookshelf" | "jukebox" | "note_block"
        )
}

/// Burn ticks of `item` in a furnace (0: not a fuel).
pub fn burn_time(item: &str) -> u32 {
    let p = path(item);
    if item.starts_with("ntm:") {
        const SINGLE: u32 = 200;
        return match p {
            "solid_fuel" => SINGLE * 16,
            "solid_fuel_presto" => SINGLE * 40,
            "solid_fuel_presto_triplet" => SINGLE * 200,
            "solid_fuel_bf" => SINGLE * 160,
            "solid_fuel_presto_bf" => SINGLE * 400,
            "solid_fuel_presto_triplet_bf" => SINGLE * 2000,
            "rocket_fuel" => SINGLE * 32,
            "biomass" => SINGLE * 2,
            "biomass_compressed" => SINGLE * 4,
            "powder_coal" => SINGLE * 8,
            "scrap" => SINGLE / 4,
            "dust" => SINGLE / 8,
            "block_scrap" => SINGLE * 2,
            "powder_fire" => 6400,
            "lignite" | "powder_lignite" => 1200,
            "coke__coal" | "coke__lignite" | "coke__petroleum" => SINGLE * 16,
            "block_coke__coal" | "block_coke__lignite" | "block_coke__petroleum" => SINGLE * 160,
            "book_guide" => SINGLE,
            "coal_infernal" => 4800,
            "coal_eternal" => SINGLE * 16,
            "crystal_coal" => 6400,
            "powder_sawdust" => SINGLE / 2,
            "briquette__coal" => SINGLE * 10,
            "briquette__lignite" => SINGLE * 8,
            "briquette__wood" => SINGLE * 2,
            "powder_ash__wood" | "powder_ash__misc" | "powder_ash__soot" => SINGLE / 2,
            "powder_ash__coal" | "powder_ash__fly" => SINGLE,
            _ => 0,
        };
    }
    if !item.starts_with("minecraft:") {
        return 0;
    }
    if wood_of(p, "_slab") {
        return 150;
    }
    if wood_block(p) {
        return 300;
    }
    if p == "coal_block" {
        return 16_000;
    }
    if matches!(
        p,
        "wooden_sword" | "wooden_pickaxe" | "wooden_axe" | "wooden_shovel" | "wooden_hoe"
    ) {
        return 200;
    }
    match p {
        "stick" => 100,
        "coal" | "charcoal" => 1600,
        "lava_bucket" => 20_000,
        "blaze_rod" => 2400,
        _ if p.ends_with("_sapling") => 100,
        _ => 0,
    }
}

/// `getContainerItem` of a fuel that has used up its last item.
pub fn container_item(item: &str) -> Option<&'static str> {
    (item == "minecraft:lava_bucket").then_some("minecraft:bucket")
}

/// The ash a fuel leaves, by the OreDict names the source reads: coke, coal
/// and lignite make coal ash; logs, anything "Wood", saplings and charcoal
/// make wood ash; the rest is misc.
pub fn ash(item: &str) -> Ash {
    let p = path(item);
    let coal = matches!(
        p,
        "coal" | "coal_block" | "powder_coal" | "powder_coal_tiny" | "lignite" | "powder_lignite"
    ) || p.starts_with("coke__")
        || p.starts_with("block_coke__")
        || p == "briquette__coal"
        || p == "briquette__lignite";
    if coal && (item.starts_with("ntm:") || p == "coal" || p == "coal_block") {
        return Ash::Coal;
    }
    let wood = p == "charcoal"
        || p == "briquette__wood"
        || item.starts_with("minecraft:")
            && (wood_block(p)
                && !matches!(
                    p,
                    "crafting_table"
                        | "chest"
                        | "trapped_chest"
                        | "bookshelf"
                        | "jukebox"
                        | "note_block"
                )
                || p == "stick"
                || p.ends_with("_sapling")
                || wood_of(p, "_slab"));
    if wood { Ash::Wood } else { Ash::Misc }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_values_of_1_7_10() {
        assert_eq!(burn_time("minecraft:coal"), 1600);
        assert_eq!(burn_time("minecraft:charcoal"), 1600);
        assert_eq!(burn_time("minecraft:coal_block"), 16_000);
        assert_eq!(burn_time("minecraft:oak_planks"), 300);
        assert_eq!(burn_time("minecraft:oak_slab"), 150);
        assert_eq!(burn_time("minecraft:stone_slab"), 0);
        assert_eq!(burn_time("minecraft:stick"), 100);
        assert_eq!(burn_time("minecraft:wooden_pickaxe"), 200);
        assert_eq!(burn_time("minecraft:stone_pickaxe"), 0);
        assert_eq!(burn_time("minecraft:lava_bucket"), 20_000);
        assert_eq!(burn_time("minecraft:blaze_rod"), 2400);
        assert_eq!(burn_time("minecraft:birch_sapling"), 100);
        assert_eq!(burn_time("minecraft:iron_door"), 0);
        assert_eq!(burn_time("minecraft:oak_door"), 300);
        assert_eq!(burn_time("minecraft:stone_pressure_plate"), 0);
        assert_eq!(burn_time("minecraft:iron_ingot"), 0);
    }

    #[test]
    fn fuel_handler_values_of_the_ntm() {
        assert_eq!(burn_time("ntm:coke__coal"), 3200);
        assert_eq!(burn_time("ntm:block_coke__petroleum"), 32_000);
        assert_eq!(burn_time("ntm:solid_fuel_presto_triplet_bf"), 400_000);
        assert_eq!(burn_time("ntm:lignite"), 1200);
        assert_eq!(burn_time("ntm:scrap"), 50);
        assert_eq!(burn_time("ntm:dust"), 25);
        assert_eq!(burn_time("ntm:powder_sawdust"), 100);
        assert_eq!(burn_time("ntm:briquette__lignite"), 1600);
        assert_eq!(burn_time("ntm:powder_ash__coal"), 200);
        assert_eq!(burn_time("ntm:powder_ash__fullerene"), 0);
        assert_eq!(burn_time("ntm:ingot_steel"), 0);
    }

    #[test]
    fn ash_follows_the_oredict_names() {
        assert_eq!(ash("minecraft:coal"), Ash::Coal);
        assert_eq!(ash("minecraft:charcoal"), Ash::Wood);
        assert_eq!(ash("minecraft:oak_log"), Ash::Wood);
        assert_eq!(ash("minecraft:oak_planks"), Ash::Wood);
        assert_eq!(ash("minecraft:stick"), Ash::Wood);
        assert_eq!(ash("minecraft:oak_sapling"), Ash::Wood);
        assert_eq!(ash("ntm:coke__lignite"), Ash::Coal);
        assert_eq!(ash("ntm:briquette__wood"), Ash::Wood);
        assert_eq!(ash("minecraft:lava_bucket"), Ash::Misc);
        assert_eq!(ash("minecraft:blaze_rod"), Ash::Misc);
        assert_eq!(ash("minecraft:crafting_table"), Ash::Misc);
        assert_eq!(ash("ntm:scrap"), Ash::Misc);
    }
}
