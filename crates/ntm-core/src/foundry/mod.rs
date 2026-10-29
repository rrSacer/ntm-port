//! The NTM's foundry (docs/maquinas.md, "Fundição"): materials in quanta
//! (an ingot is 72), the crucible that melts and alloys them, the heat
//! that runs it and, in later stages, the channels, tanks, outlets and
//! molds the metal goes through.
//!
//! Sources, relative to `ntm-src/src/main/java/com/hbm/`:
//! `inventory/material/{Mats,NTMMaterial,MaterialShapes,MatDistribution}
//! .java`, `inventory/recipes/CrucibleRecipes.java`. Molten metal is not a
//! fluid here, as in the NTM: a stack of `(material, quanta)` with its own
//! rules.
//!
//! The tables come from the content agent's extraction
//! (`content/machine-recipes/foundry.json`), embedded by
//! `tools/machine-gen/build.py`; the logic takes them injected.

pub mod cast;
pub mod crucible;
pub mod firebox;
pub mod flow;

use serde::Deserialize;
use std::collections::HashMap;

/// `NTMMaterial.SmeltingBehavior`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Behavior {
    NotSmeltable,
    Vaporizes,
    Breaks,
    Smeltable,
    Additive,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Material {
    pub id: u32,
    pub name: String,
    pub behavior: Behavior,
    /// What it melts into, at `conv.1 / conv.0` (coal → carbon at 2:1).
    pub smelts_into: String,
    pub conv: (u32, u32),
    /// `[solid light, molten, solid dark]`, 0xRRGGBB.
    #[serde(default)]
    pub colors: [u32; 3],
}

/// A material and its quanta.
pub type Mat = (String, u32);

/// A crucible alloy (`CrucibleRecipe`): every `frequency` ticks, the
/// inputs in the recipe stack become the outputs.
#[derive(Clone, Debug, Deserialize)]
pub struct Alloy {
    pub name: String,
    /// The icon item's name, for the list.
    #[serde(default)]
    pub label: String,
    pub frequency: u32,
    #[serde(default)]
    pub icon: String,
    #[serde(rename = "in")]
    pub inputs: Vec<Mat>,
    #[serde(rename = "out")]
    pub outputs: Vec<Mat>,
}

impl Alloy {
    /// `getInputAmount`.
    pub fn input_amount(&self) -> u32 {
        self.inputs.iter().map(|m| m.1).sum()
    }
    pub fn input(&self, mat: &str) -> u32 {
        self.inputs.iter().find(|m| m.0 == mat).map_or(0, |m| m.1)
    }
    pub fn output(&self, mat: &str) -> u32 {
        self.outputs.iter().find(|m| m.0 == mat).map_or(0, |m| m.1)
    }
}

/// A firebox fuel: burn ticks and heat a tick (the burn module's mods
/// applied), and what is left when the stack runs out (a lava bucket's
/// bucket).
#[derive(Clone, Debug, Deserialize)]
pub struct Fuel {
    pub ticks: u32,
    pub heat: u32,
    #[serde(default)]
    pub container: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Tables {
    #[serde(default)]
    pub quanta: HashMap<String, u32>,
    pub materials: Vec<Material>,
    /// What an item is made of, before `smelts_into`
    /// (`Mats.getMaterialsFromItem`).
    pub items: HashMap<String, Vec<Mat>>,
    pub alloys: Vec<Alloy>,
    #[serde(default)]
    pub firebox_fuels: HashMap<String, Fuel>,
    #[serde(default)]
    pub molds: Vec<cast::Mold>,
    #[serde(skip)]
    by_name: HashMap<String, usize>,
}

impl Tables {
    pub fn new(materials: Vec<Material>, items: HashMap<String, Vec<Mat>>, alloys: Vec<Alloy>, firebox_fuels: HashMap<String, Fuel>) -> Self {
        Self { quanta: HashMap::new(), materials, items, alloys, firebox_fuels, molds: Vec::new(), by_name: HashMap::new() }.indexed()
    }

    /// The NTM's (`data/machines/foundry.json`, from the content agent's
    /// extraction by `tools/machine-gen/build.py`).
    pub fn ntm() -> &'static Tables {
        static T: std::sync::OnceLock<Tables> = std::sync::OnceLock::new();
        T.get_or_init(|| Self::from_json(include_str!("../../data/machines/foundry.json")))
    }

    pub fn from_json(json: &str) -> Self {
        serde_json::from_str::<Tables>(json).expect("data/machines/foundry.json").indexed()
    }

    fn indexed(mut self) -> Self {
        self.by_name = self.materials.iter().enumerate().map(|(i, m)| (m.name.clone(), i)).collect();
        self
    }

    pub fn material(&self, name: &str) -> Option<&Material> {
        self.by_name.get(name).map(|&i| &self.materials[i])
    }

    /// A material as an engine value, its id + 1 (0: none, so the zeros
    /// of a block entity the mod never wrote read as nothing; stone is id
    /// 0), and back.
    pub fn mat_id(&self, name: Option<&str>) -> i32 {
        name.and_then(|n| self.material(n)).map_or(0, |m| m.id as i32 + 1)
    }
    pub fn mat_name(&self, code: i32) -> Option<String> {
        self.materials.iter().find(|m| m.id as i32 + 1 == code).map(|m| m.name.clone())
    }

    /// A stack as engine values: count, then (id, quanta) pairs; and back
    /// (`None` when the values don't fit).
    pub fn encode_stack(&self, s: &[Mat], out: &mut Vec<i32>) {
        out.push(s.len() as i32);
        for (m, q) in s {
            out.extend([self.mat_id(Some(m)), *q as i32]);
        }
    }
    pub fn decode_stack(&self, v: &mut std::slice::Iter<i32>) -> Option<Vec<Mat>> {
        let n = *v.next()?;
        (0..n).map(|_| Some((self.mat_name(*v.next()?)?, u32::try_from(*v.next()?).ok()?))).collect()
    }

    /// Only these are ever poured (`CrucibleUtil.tryPourStack`).
    pub fn pourable(&self, name: &str) -> bool {
        self.material(name).is_some_and(|m| m.behavior == Behavior::Smeltable)
    }

    /// The molten colour of a material (0 when unknown).
    pub fn color(&self, name: &str) -> u32 {
        self.material(name).map_or(0, |m| m.colors[1])
    }

    /// What an item melts into (`Mats.getSmeltingMaterialsFromItem`):
    /// its materials converted by `smelts_into`.
    pub fn smelting(&self, item: &str) -> Vec<Mat> {
        let Some(mats) = self.items.get(item) else { return Vec::new() };
        mats.iter()
            .filter_map(|(name, q)| {
                let m = self.material(name)?;
                let (cin, cout) = m.conv;
                Some((m.smelts_into.clone(), q * cout / cin.max(1)))
            })
            .collect()
    }

    pub fn alloy(&self, name: &str) -> Option<&Alloy> {
        self.alloys.iter().find(|a| a.name == name)
    }
}

/// `getQuantaFromType(list, mat)`: that material's quanta, or the sum of
/// all when `mat` is `None`.
pub fn quanta(stack: &[Mat], mat: Option<&str>) -> u32 {
    match mat {
        Some(m) => stack.iter().find(|s| s.0 == m).map_or(0, |s| s.1),
        None => stack.iter().map(|s| s.1).sum(),
    }
}

/// `addToStack`: onto the same material's pile, else a new one at the end.
pub fn add(stack: &mut Vec<Mat>, mat: &str, q: u32) {
    match stack.iter_mut().find(|s| s.0 == mat) {
        Some(s) => s.1 += q,
        None => stack.push((mat.to_string(), q)),
    }
}

/// Quanta as the NTM's tooltips say them: ingots, nuggets, quanta.
pub fn quanta_name(q: u32) -> String {
    let (ingots, rest) = (q / 72, q % 72);
    let (nuggets, quanta) = (rest / 8, rest % 8);
    let mut parts = Vec::new();
    if ingots > 0 {
        parts.push(format!("{ingots} lingote(s)"));
    }
    if nuggets > 0 {
        parts.push(format!("{nuggets} pepita(s)"));
    }
    if quanta > 0 || parts.is_empty() {
        parts.push(format!("{quanta} quanta"));
    }
    parts.join(", ")
}

/// A heat source under a crucible (`IHeatSource`).
pub trait HeatSource {
    fn heat_stored(&self) -> u32;
    fn use_heat(&mut self, heat: u32);
}

/// Something molten metal can be poured into from above
/// (`ICrucibleAcceptor.canAcceptPartialPour` + `pour`): what is left of
/// `q`, or `None` when it takes none of `mat`.
pub trait Acceptor {
    fn pour(&mut self, t: &Tables, mat: &str, q: u32) -> Option<u32>;
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;

    fn mat(id: u32, name: &str, behavior: Behavior, into: &str, conv: (u32, u32), color: u32) -> Material {
        Material { id, name: name.into(), behavior, smelts_into: into.into(), conv, colors: [0, color, 0] }
    }

    /// A few materials and the steel and red copper alloys, as the NTM has them.
    pub fn tables() -> Tables {
        use Behavior::*;
        let n = 8;
        let i = 72;
        Tables::new(
            vec![
                mat(26, "iron", Smeltable, "iron", (1, 1), 0xFFA259),
                mat(28, "copper", Smeltable, "copper", (1, 1), 0xFF7F32),
                mat(17, "redstone", Smeltable, "redstone", (1, 1), 0xE3260C),
                mat(30, "steel", Smeltable, "steel", (1, 1), 0xFF8F60),
                mat(31, "mingrade", Smeltable, "mingrade", (1, 1), 0xFF6060),
                mat(10, "carbon", Additive, "carbon", (1, 1), 0x404040),
                mat(11, "coal", Breaks, "carbon", (2, 1), 0x404040),
                mat(40, "flux", Additive, "flux", (1, 1), 0xF1E0BB),
                mat(41, "slag", Smeltable, "slag", (1, 1), 0x554A45),
                mat(1, "stone", NotSmeltable, "stone", (1, 1), 0x7F7F7F),
            ],
            [
                ("minecraft:iron_ingot", vec![("iron".to_string(), i)]),
                ("minecraft:iron_nugget", vec![("iron".to_string(), n)]),
                ("minecraft:iron_block", vec![("iron".to_string(), 9 * i)]),
                ("ntm:ingot_copper", vec![("copper".to_string(), i)]),
                ("minecraft:redstone", vec![("redstone".to_string(), i)]),
                ("minecraft:coal", vec![("coal".to_string(), i)]),
                ("ntm:powder_flux", vec![("flux".to_string(), i)]),
                ("minecraft:iron_ore", vec![("iron".to_string(), 2 * i), ("stone".to_string(), 162)]),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
            vec![
                Alloy {
                    name: "steel".into(),
                    label: "Aço".into(),
                    frequency: 20,
                    icon: "ntm:ingot_steel".into(),
                    inputs: vec![("iron".into(), 2 * n), ("carbon".into(), 3 * n), ("flux".into(), n)],
                    outputs: vec![("steel".into(), 2 * n)],
                },
                Alloy {
                    name: "redcopper".into(),
                    label: "Cobre vermelho".into(),
                    frequency: 2,
                    icon: "ntm:ingot_red_copper".into(),
                    inputs: vec![("copper".into(), n), ("redstone".into(), n)],
                    outputs: vec![("mingrade".into(), 2 * n)],
                },
            ],
            [
                ("minecraft:coal", Fuel { ticks: 2000, heat: 200, container: None }),
                ("minecraft:oak_planks", Fuel { ticks: 300, heat: 100, container: None }),
                ("minecraft:lava_bucket", Fuel { ticks: 20_000, heat: 100, container: Some("minecraft:bucket".into()) }),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
        )
    }

    #[test]
    fn the_ntm_tables_are_embedded() {
        let t = Tables::ntm();
        assert_eq!(t.materials.len(), 109);
        assert_eq!(t.alloys.len(), 13, "11 ligas + hematita e malaquita; as 3 do GT6 fora");
        assert_eq!(t.alloys[0].name, "crucible.steel");
        assert_eq!(t.smelting("minecraft:coal"), vec![("carbon".to_string(), 36)]);
        let ore = t.smelting("minecraft:iron_ore");
        assert_eq!(ore.iter().find(|m| m.0 == "iron").map(|m| m.1), Some(144));
        assert!(t.pourable("steel") && !t.pourable("flux"));
        assert!(t.firebox_fuels["minecraft:oak_planks"].ticks > 0, "madeira serve na firebox");
        assert_eq!(t.firebox_fuels["minecraft:lava_bucket"].container.as_deref(), Some("minecraft:bucket"));
    }

    #[test]
    fn items_melt_into_what_their_materials_smelt_into() {
        let t = tables();
        assert_eq!(t.smelting("minecraft:iron_ingot"), vec![("iron".to_string(), 72)]);
        // Coal → carbon at 2:1.
        assert_eq!(t.smelting("minecraft:coal"), vec![("carbon".to_string(), 36)]);
        assert_eq!(t.smelting("minecraft:dirt"), vec![]);
        assert!(t.pourable("steel") && !t.pourable("carbon") && !t.pourable("stone"));
        assert_eq!(t.color("steel"), 0xFF8F60);
    }
}
