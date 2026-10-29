//! The recipe tables of the recipes front (`data/*.json`), built into the
//! mod.

use serde::Deserialize;
use std::{collections::HashMap, sync::OnceLock};

#[derive(Debug, Deserialize)]
struct Stack {
    item: String,
    #[serde(default = "one")]
    count: u32,
}
fn one() -> u32 {
    1
}

#[derive(Debug, Deserialize)]
struct FurnaceFile {
    schema: u32,
    #[serde(default)]
    machines: HashMap<String, MachineCfg>,
    recipes: Vec<FurnaceRecipe>,
}
#[derive(Debug, Default, Deserialize)]
struct MachineCfg {
    ticks: Option<u32>,
    energy_per_tick: Option<u64>,
}
#[derive(Debug, Deserialize)]
struct FurnaceRecipe {
    input: Stack,
    output: Stack,
}

/// One smelting result.
#[derive(Debug)]
pub struct Smelt {
    pub output: String,
    pub count: u32,
}

#[derive(Debug)]
pub struct Furnace {
    pub ticks: u32,
    pub energy_per_tick: u64,
    by_input: HashMap<String, Smelt>,
}

impl Furnace {
    pub fn smelt(&self, item: &str) -> Option<&Smelt> {
        self.by_input.get(item)
    }
    pub fn len(&self) -> usize {
        self.by_input.len()
    }
    pub fn is_empty(&self) -> bool {
        self.by_input.is_empty()
    }
}

fn parse_furnace(text: &str) -> Result<Furnace, String> {
    let f: FurnaceFile = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if f.schema != 1 {
        return Err(format!("furnace_recipes.json schema {}", f.schema));
    }
    let cfg = f
        .machines
        .get("machine_electric_furnace_off")
        .map_or((None, None), |c| (c.ticks, c.energy_per_tick));
    let mut by_input = HashMap::new();
    for r in f.recipes {
        // The first recipe of an input wins, as vanilla's furnace does.
        by_input.entry(r.input.item).or_insert(Smelt {
            output: r.output.item,
            count: r.output.count,
        });
    }
    Ok(Furnace {
        ticks: cfg.0.unwrap_or(100),
        energy_per_tick: cfg.1.unwrap_or(50),
        by_input,
    })
}

/// The electric furnace's table (`data/furnace_recipes.json`).
pub fn furnace() -> &'static Furnace {
    static F: OnceLock<Furnace> = OnceLock::new();
    F.get_or_init(|| {
        parse_furnace(include_str!("../data/furnace_recipes.json")).unwrap_or_else(|_| Furnace {
            ticks: 100,
            energy_per_tick: 50,
            by_input: HashMap::new(),
        })
    })
}

#[derive(Debug, Deserialize)]
struct PressFile {
    schema: u32,
    #[serde(default)]
    stamps: Vec<StampDef>,
    #[serde(default)]
    machines: HashMap<String, PressCfg>,
    recipes: Vec<PressRecipeDef>,
}
#[derive(Debug, Deserialize)]
struct StampDef {
    item: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    durability: u32,
}
#[derive(Debug, Default, Deserialize)]
struct PressCfg {
    energy_per_tick: Option<u64>,
    max_speed: Option<u32>,
    progress_at_max: Option<u32>,
    max_press: Option<u32>,
}
#[derive(Debug, Deserialize)]
struct PressRecipeDef {
    input: Stack,
    output: Stack,
    stamp: String,
}

/// A stamp item: its type (`plate`, `wire`, ...) and how many pressings it
/// lasts (0: forever).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StampKind {
    pub kind: String,
    pub durability: u32,
}

#[derive(Debug)]
pub struct PressRecipe {
    pub input: String,
    pub input_count: u32,
    pub stamp: String,
    pub output: String,
    pub count: u32,
}

/// `press_recipes.json`: both presses share it, as in the NTM.
#[derive(Debug)]
pub struct Press {
    pub stamps: HashMap<String, StampKind>,
    pub recipes: Vec<PressRecipe>,
    /// The electric press's draw per working tick.
    pub epress_energy: u64,
    /// The burner press: speed ramp, progress per tick at full speed, and
    /// the progress of one pressing.
    pub max_speed: u32,
    pub progress_at_max: u32,
    pub max_press: u32,
}

impl Press {
    pub fn stamp(&self, item: &str) -> Option<&StampKind> {
        self.stamps.get(item)
    }
    pub fn recipe(&self, input: &str, stamp_kind: &str) -> Option<&PressRecipe> {
        self.recipes
            .iter()
            .find(|r| r.input == input && r.stamp == stamp_kind)
    }
    /// Whether any recipe takes `item`.
    pub fn is_input(&self, item: &str) -> bool {
        self.recipes.iter().any(|r| r.input == item)
    }
}

fn parse_press(text: &str) -> Result<Press, String> {
    let f: PressFile = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if f.schema != 1 {
        return Err(format!("press_recipes.json schema {}", f.schema));
    }
    let burner = f.machines.get("machine_press");
    let epress = f.machines.get("machine_epress");
    Ok(Press {
        stamps: f
            .stamps
            .into_iter()
            .map(|s| {
                (
                    s.item,
                    StampKind {
                        kind: s.kind,
                        durability: s.durability,
                    },
                )
            })
            .collect(),
        recipes: f
            .recipes
            .into_iter()
            .map(|r| PressRecipe {
                input: r.input.item,
                input_count: r.input.count.max(1),
                stamp: r.stamp,
                output: r.output.item,
                count: r.output.count,
            })
            .collect(),
        epress_energy: epress.and_then(|c| c.energy_per_tick).unwrap_or(100),
        max_speed: burner.and_then(|c| c.max_speed).unwrap_or(400).max(1),
        progress_at_max: burner.and_then(|c| c.progress_at_max).unwrap_or(25),
        max_press: burner.and_then(|c| c.max_press).unwrap_or(200).max(1),
    })
}

/// The presses' table (`data/press_recipes.json`).
pub fn press() -> &'static Press {
    static P: OnceLock<Press> = OnceLock::new();
    P.get_or_init(|| {
        parse_press(include_str!("../data/press_recipes.json")).unwrap_or_else(|_| Press {
            stamps: HashMap::new(),
            recipes: Vec::new(),
            epress_energy: 100,
            max_speed: 400,
            progress_at_max: 25,
            max_press: 200,
        })
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn press_table_parses() {
        let p = super::parse_press(include_str!("../data/press_recipes.json")).unwrap();
        assert_eq!(p.stamp("ntm:stamp_iron_plate").unwrap().kind, "plate");
        assert_eq!(p.stamp("ntm:stamp_iron_plate").unwrap().durability, 64);
        let r = p.recipe("ntm:ingot_lead", "wire").unwrap();
        assert_eq!((r.output.as_str(), r.count), ("ntm:wire_fine__lead", 8));
        assert!(p.recipe("ntm:ingot_lead", "flat").is_none());
        assert_eq!((p.epress_energy, p.max_speed, p.progress_at_max, p.max_press), (100, 400, 25, 200));
        // The NTM's whole table (`PressRecipes`, `ModItems` stamps).
        assert_eq!((p.stamps.len(), p.recipes.len()), (32, 49));
        assert_eq!(p.stamp("ntm:stamp_desh_flat").unwrap().durability, 0);
        assert_eq!(p.stamp("ntm:stamp_9").map(|k| (k.kind.as_str(), k.durability)), Some(("c9", 1000)));
        let gold = p.recipe("minecraft:gold_ingot", "wire").unwrap();
        assert_eq!((gold.output.as_str(), gold.count), ("ntm:wire_fine__gold", 8));
        assert_eq!(p.recipe("minecraft:gold_ingot", "plate").unwrap().output, "ntm:plate_gold");
        assert_eq!(p.recipe("ntm:coke__petroleum", "flat").unwrap().output, "ntm:ingot_graphite");
        assert_eq!(p.recipe("minecraft:paper", "printing3").unwrap().output, "ntm:page_of___page3");
    }

    #[test]
    fn furnace_table_parses() {
        let f = super::parse_furnace(include_str!("../data/furnace_recipes.json")).unwrap();
        assert!(f.len() > 100);
        assert_eq!(f.smelt("minecraft:raw_iron").unwrap().output, "minecraft:iron_ingot");
        assert_eq!((f.ticks, f.energy_per_tick), (100, 50));
    }
}
