//! The outgasser's recipes (`OutgasserRecipes.registerDefaults`, executed by
//! the oracle) and its fluid/item bookkeeping.

use super::tables_data::OUTGASSER;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutgasserInput {
    /// A `ComparableStack`: item name and metadata.
    Stack(&'static str, i32),
    /// An `OreDictStack`.
    Ore(&'static str),
}

#[derive(Clone, Copy, Debug)]
pub struct OutgasserRecipe {
    pub input: OutgasserInput,
    /// `(item, count, metadata)`.
    pub solid: Option<(&'static str, i32, i32)>,
    /// `(fluid, amount)`.
    pub fluid: Option<(&'static str, i32)>,
    pub fusion_only: bool,
}

/// An item stack as the outgasser sees it. `max_stack` is the item's stack limit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack {
    pub item: String,
    pub count: i32,
    pub meta: i32,
    pub max_stack: i32,
}

/// `OutgasserRecipes.getOutput`: the exact item first, then its ore-dictionary
/// names in the order the host gives them.
pub fn recipe_for(item: &str, meta: i32, ore_names: &[String]) -> Option<&'static OutgasserRecipe> {
    if let Some(r) = OUTGASSER
        .iter()
        .find(|r| matches!(r.input, OutgasserInput::Stack(i, m) if i == item && m == meta))
    {
        return Some(r);
    }
    for key in ore_names {
        if let Some(r) = OUTGASSER
            .iter()
            .find(|r| matches!(r.input, OutgasserInput::Ore(k) if k == key))
        {
            return Some(r);
        }
    }
    None
}

/// `TileEntityRBMKOutgasser.duration`.
pub const DURATION: f64 = 10_000.0;
/// The gas tank's size.
pub const GAS_MAX: i32 = 64_000;

/// The port's items for the Java items the recipes name, both ways (T09): `(Java item, meta, port id, stack limit)`.
/// The meta of `oil_tar` is `ItemEnums.EnumTarType`'s ordinal (CRUDE, CRACK, COAL, WOOD, WAX, PARAFFIN); the vanilla
/// items are 1.7.10's (`minecraft_1_7_10:`); the glowing stew is an `ItemSoup` (`setMaxStackSize(1)`). The glowing
/// mushroom block (`ModBlocks.mush`, the mushrooms' product) is not in the port's content: [`MISSING`].
pub const PORT_ITEMS: [(&str, i32, &str, i32); 13] = [
    ("billet_thorium_fuel", 0, "ntm:billet_thorium_fuel", 64),
    ("oil_tar", 2, "ntm:oil_tar__coal", 64),
    ("oil_tar", 4, "ntm:oil_tar__wax", 64),
    ("powder_au198", 0, "ntm:powder_au198", 64),
    ("ingot_au198", 0, "ntm:ingot_au198", 64),
    ("ingot_c4", 0, "ntm:ingot_c4", 64),
    ("ingot_thorium_fuel", 0, "ntm:ingot_thorium_fuel", 64),
    ("nugget_au198", 0, "ntm:nugget_au198", 64),
    ("nugget_thorium_fuel", 0, "ntm:nugget_thorium_fuel", 64),
    ("glowing_stew", 0, "ntm:glowing_stew", 1),
    ("minecraft_1_7_10:brown_mushroom", 0, "minecraft:brown_mushroom", 64),
    ("minecraft_1_7_10:red_mushroom", 0, "minecraft:red_mushroom", 64),
    ("minecraft_1_7_10:mushroom_stew", 0, "minecraft:mushroom_stew", 1),
];

/// Java items a recipe makes that the port does not have yet: a recipe that would make one does not run (its input is
/// refused as one without a recipe), never another item in its place.
pub const MISSING: [&str; 1] = ["mush"];

/// The port's id of a Java item (and its stack limit), none for one the port lacks.
pub fn port_item(item: &str, meta: i32) -> Option<(&'static str, i32)> {
    PORT_ITEMS.iter().find(|e| e.0 == item && e.1 == meta).map(|e| (e.2, e.3))
}

/// The outgasser's view of a port item: its Java item and meta when the recipes name it, else the port's id at meta 0
/// (an item only an OreDict name of which a recipe asks for).
pub fn java_item(id: &str) -> (String, i32) {
    PORT_ITEMS.iter().find(|e| e.2 == id).map_or_else(|| (id.to_string(), 0), |e| (e.0.to_string(), e.1))
}

/// The recipe of a port item (`getOutput` over the exact item, then its OreDict names), none when it has none or its
/// product is one the port lacks.
pub fn recipe_of_port(id: &str) -> Option<&'static OutgasserRecipe> {
    let (item, meta) = java_item(id);
    let names: Vec<String> = crate::oredict::names(id).to_vec();
    recipe_for(&item, meta, &names).filter(|r| r.solid.is_none_or(|(s, _, _)| !MISSING.contains(&s)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every item a recipe names is a port item or a named gap, and every port id is in the content.
    #[test]
    fn every_recipe_item_is_the_ports_or_a_named_gap() {
        for r in &OUTGASSER {
            if let OutgasserInput::Stack(i, m) = r.input {
                assert!(port_item(i, m).is_some(), "input {i}:{m}");
            }
            if let Some((i, _, m)) = r.solid {
                assert!(port_item(i, m).is_some() || MISSING.contains(&i), "product {i}:{m}");
            }
        }
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content/ids.json");
        let Ok(text) = std::fs::read_to_string(path) else { return };
        let ids: serde_json::Value = serde_json::from_str(&text).unwrap();
        let known: std::collections::BTreeSet<&str> = ids["entries"].as_array().unwrap().iter().filter_map(|e| e["id"].as_str()).collect();
        for (_, _, id, _) in PORT_ITEMS {
            assert!(id.starts_with("minecraft:") || known.contains(id), "{id}");
        }
        assert!(!known.contains("ntm:mush"), "the glowing mushroom came in: take it off MISSING");
    }

    /// The stack limits the recipes' products use are the content's (`max_stack_size` in `ids.json`, 64 when it says
    /// none), but for the content's known gap: the glowing stew is an `ItemSoup` (`setMaxStackSize(1)`) and the content
    /// lets it pile to 64. The outgasser keeps the Java's 1; the test fails when the content is fixed or another limit
    /// parts from it.
    #[test]
    fn stack_limits_are_the_contents_but_its_known_gap() {
        const CONTENT_GAP: [(&str, i64); 1] = [("ntm:glowing_stew", 64)];
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content/ids.json");
        let Ok(text) = std::fs::read_to_string(path) else { return };
        let ids: serde_json::Value = serde_json::from_str(&text).unwrap();
        let limit = |id: &str| {
            ids["entries"].as_array().unwrap().iter().find(|e| e["id"] == id).and_then(|e| e["max_stack_size"].as_i64()).unwrap_or(64)
        };
        for (_, _, id, n) in PORT_ITEMS.iter().filter(|e| e.2.starts_with("ntm:")) {
            match CONTENT_GAP.iter().find(|g| g.0 == *id) {
                Some((_, content)) => assert_eq!((limit(id), *n), (*content, 1), "{id}: the content's gap changed"),
                None => assert_eq!(limit(id), i64::from(*n), "{id}"),
            }
        }
    }

    /// `getOutput` walks an item's OreDict names in the OreDict's order, here in the port's: the same for every port item
    /// that has at most one name a recipe asks for, which is every one of them.
    #[test]
    fn no_port_item_has_two_recipe_names() {
        let keys: Vec<&str> = OUTGASSER.iter().filter_map(|r| if let OutgasserInput::Ore(k) = r.input { Some(k) } else { None }).collect();
        for k in &keys {
            for item in crate::oredict::items(k) {
                let hits: Vec<&String> = crate::oredict::names(item).iter().filter(|n| keys.contains(&n.as_str())).collect();
                assert!(hits.len() <= 1, "{item}: {hits:?}");
            }
        }
    }

    /// The port's items find their recipes: by the exact item (the tar, the vanilla mushroom stew), by OreDict name,
    /// and none for a mushroom (its product is missing) or an item without one.
    #[test]
    fn port_items_find_their_recipes() {
        let fluid = |id: &str| recipe_of_port(id).and_then(|r| r.fluid);
        assert_eq!(fluid("ntm:oil_tar__coal"), Some(("COALOIL", 100)));
        assert_eq!(fluid("ntm:oil_tar__wax"), Some(("RADIOSOLVENT", 100)));
        assert_eq!(recipe_of_port("minecraft:mushroom_stew").and_then(|r| r.solid), Some(("glowing_stew", 1, 0)));
        assert_eq!(recipe_of_port("minecraft:gold_ingot").and_then(|r| r.solid), Some(("ingot_au198", 1, 0)));
        assert!(recipe_of_port("minecraft:brown_mushroom").is_none() && recipe_of_port("minecraft:stone").is_none());
        assert_eq!(java_item("ntm:oil_tar__coal"), ("oil_tar".to_string(), 2));
        assert_eq!(java_item("minecraft:gold_ingot"), ("minecraft:gold_ingot".to_string(), 0));
    }
}
