//! The OreDict the machines ask by name (`data/oredict.json`, from
//! `tools/content-gen/oredict_export.py`): which names an item has, and
//! which items a name holds. Forge 1.7.10's vanilla entries on the 26.3 ids
//! plus the NTM's registrations; the names are the source's.

use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(serde::Deserialize)]
struct File {
    names: std::collections::BTreeMap<String, Vec<String>>,
}

struct Tables {
    by_name: std::collections::BTreeMap<String, Vec<String>>,
    by_item: HashMap<String, Vec<String>>,
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let f: File =
            serde_json::from_str(include_str!("../data/oredict.json")).expect("data/oredict.json");
        let mut by_item: HashMap<String, Vec<String>> = HashMap::new();
        for (name, items) in &f.names {
            for item in items {
                by_item.entry(item.clone()).or_default().push(name.clone());
            }
        }
        Tables {
            by_name: f.names,
            by_item,
        }
    })
}

/// `ItemStackUtil.getOreDictNames`: the names of `item`, in name order.
pub fn names(item: &str) -> &'static [String] {
    tables().by_item.get(item).map_or(&[], Vec::as_slice)
}

/// The items of the name `name`.
pub fn items(name: &str) -> &'static [String] {
    tables().by_name.get(name).map_or(&[], Vec::as_slice)
}

pub fn has(item: &str, name: &str) -> bool {
    names(item).iter().any(|n| n == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_knows_the_vanilla_and_the_ntm_names() {
        assert!(has("minecraft:iron_ore", "oreIron"));
        assert!(has("minecraft:oak_log", "logWood"));
        assert!(has("minecraft:stick", "stickWood"));
        assert!(has("ntm:oil_tar__coal", "anyTar") && has("ntm:oil_tar__coal", "coaltar"));
        assert!(items("slabWood").contains(&"minecraft:birch_slab".to_string()));
        assert!(names("minecraft:bedrock").is_empty());
        assert!(
            names("ntm:ore_uranium")
                .iter()
                .any(|n| n.starts_with("ore"))
        );
    }
}
