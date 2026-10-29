//! Which blocks the logic knows, and how their states carry the machine's
//! look. Mirrors `content/ids.json` (`machines`, `logic`, `internal_blocks`);
//! a test checks the two agree.

use crate::machine::Kind;

/// How a block's state shows a machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockSpec {
    pub id: &'static str,
    pub kind: Kind,
    /// Horizontal facing, set by the template on placement; kept as placed.
    pub facing: Option<&'static str>,
    /// A numeric property 0..=max: 0 idle, 1..=max the machine's level.
    pub level: Option<(&'static str, u8)>,
    /// Its HE ports, as the NTM's `getPorts` (on one block).
    pub ports: Ports,
}

/// The NTM's power port shapes, brought to one block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ports {
    /// No power.
    None,
    /// `TilePortShapes.around`: one port open on all six faces (electric
    /// furnace, electric press): it joins everything around it.
    AllRound,
    /// The battery's `solderer` shape: a separate port on each horizontal
    /// side, so energy never crosses the battery.
    Sides,
    /// The wood burner: one port at its back (`dir.getOpposite()`).
    Back,
}

/// A horizontal facing as an offset.
pub fn facing_dir(facing: &str) -> Option<crate::Pos> {
    match facing {
        "north" => Some([0, 0, -1]),
        "south" => Some([0, 0, 1]),
        "east" => Some([1, 0, 0]),
        "west" => Some([-1, 0, 0]),
        _ => None,
    }
}

/// The ports of a machine of `spec` facing `facing`.
pub fn ports(spec: &BlockSpec, facing: Option<&str>) -> Vec<crate::energy::Port> {
    use crate::energy::DIRS;
    match spec.ports {
        Ports::None => Vec::new(),
        Ports::AllRound => vec![DIRS.to_vec()],
        Ports::Sides => DIRS.iter().filter(|d| d[1] == 0).map(|d| vec![*d]).collect(),
        Ports::Back => match facing.and_then(facing_dir) {
            Some([x, y, z]) => vec![vec![[-x, -y, -z]]],
            None => vec![DIRS.to_vec()],
        },
    }
}

const HONEY: Option<(&str, u8)> = Some(("honey_level", 5));

/// The property a bomb block's template (`minecraft:redstone_lamp`) sets
/// while redstone powers it.
pub const BOMB_POWERED: &str = "lit";

const fn bomb(id: &'static str) -> BlockSpec {
    BlockSpec {
        id,
        kind: Kind::Bomb,
        facing: None,
        level: None,
        ports: Ports::None,
    }
}

pub fn is_bomb(id: &str) -> bool {
    spec(id).is_some_and(|s| s.kind == Kind::Bomb)
}

pub const BLOCKS: &[BlockSpec] = &[
    BlockSpec {
        id: "ntm:machine_battery",
        kind: Kind::Battery,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::Sides,
    },
    BlockSpec {
        id: "ntm:machine_wood_burner",
        kind: Kind::WoodBurner,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::Back,
    },
    BlockSpec {
        id: "ntm:machine_electric_furnace_off",
        kind: Kind::ElectricFurnace,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_press",
        kind: Kind::BurnerPress,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_epress",
        kind: Kind::ElectricPress,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    // Recipe machines (`crate::crafter`, docs/maquinas.md).
    BlockSpec {
        id: "ntm:machine_assembly_machine",
        kind: Kind::Crafter,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_chemical_plant",
        kind: Kind::Crafter,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_centrifuge",
        kind: Kind::Crafter,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    BlockSpec {
        id: "ntm:machine_gascent",
        kind: Kind::GasCent,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    // The blast furnace (`crate::blastfurnace`): no energy; fluid ports
    // (hot air in, flue gas out) all round the core.
    BlockSpec {
        id: "ntm:machine_blast_furnace",
        kind: Kind::BlastFurnace,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    // The shredder (`crate::shredder`): `TilePortShapes.around`, energy only.
    BlockSpec {
        id: "ntm:machine_shredder",
        kind: Kind::Shredder,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::AllRound,
    },
    // Fluids (`crate::fluidbox`, docs/fluidos.md): the duct is a "cable" of
    // one fluid type, the barrel and the tank store it.
    BlockSpec {
        id: "ntm:fluid_duct_neo",
        kind: Kind::Duct,
        facing: None,
        level: None,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:barrel_steel",
        kind: Kind::FluidStore,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    BlockSpec {
        id: "ntm:machine_fluidtank",
        kind: Kind::FluidStore,
        facing: Some("facing"),
        level: HONEY,
        ports: Ports::None,
    },
    // Bombs (`crate::bomb`, docs/bombas.md): redstone-lamp template, so
    // vanilla redstone sets `lit` (`BOMB_POWERED`) and the server reads it.
    bomb("ntm:tnt_ntm"),
    bomb("ntm:dynamite"),
    bomb("ntm:semtex"),
    bomb("ntm:c4"),
    bomb("ntm:nuke_boy"),
    bomb("ntm:nuke_gadget"),
    bomb("ntm:nuke_man"),
    bomb("ntm:nuke_mike"),
    bomb("ntm:nuke_tsar"),
    bomb("ntm:nuke_n2"),
    bomb("ntm:nuke_fstbmb"),
    // The spikes' state block (composter template): a test battery while the
    // content pack isn't installed.
    BlockSpec {
        id: "ntm:teste",
        kind: Kind::Battery,
        facing: None,
        level: Some(("level", 8)),
        ports: Ports::Sides,
    },
];

/// HE conductors: no state, the server reads the neighbours.
pub const CABLES: &[&str] = &["ntm:red_cable"];

/// The internal block the client writes to answer the server
/// (`token`): a composter-template block, `level` 0..=8.
pub const TOKEN: &str = "ntm:logic_token";

/// A machine's fluid ports (the NTM's `TilePortShapes`, on one block): every
/// machine with tanks has one port open all round; ducts are the cables.
pub fn fluid_ports(spec: &BlockSpec, _facing: Option<&str>) -> Vec<crate::energy::Port> {
    match spec.kind {
        Kind::FluidStore | Kind::Crafter | Kind::GasCent | Kind::BlastFurnace => vec![crate::energy::DIRS.to_vec()],
        _ => Vec::new(),
    }
}

pub fn is_duct(id: &str) -> bool {
    spec(id).is_some_and(|s| s.kind == Kind::Duct)
}

pub fn spec(id: &str) -> Option<&'static BlockSpec> {
    BLOCKS.iter().find(|b| b.id == id)
}

pub fn is_cable(id: &str) -> bool {
    CABLES.contains(&id)
}

/// Every machine block id, for the client's "did the player use a
/// machine" check.
pub fn machine_ids() -> impl Iterator<Item = &'static str> {
    BLOCKS.iter().map(|b| b.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn content(path: &str) -> Option<Value> {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content/");
        let text = std::fs::read_to_string(format!("{root}{path}")).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// The table above against the content pack, when the pack is there.
    #[test]
    fn agrees_with_content_ids() {
        let Some(ids) = content("ids.json") else {
            eprintln!("content/ids.json missing: skipped");
            return;
        };
        let machines: Vec<&str> = ids["machines"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        for m in &machines {
            let s = spec(m).unwrap_or_else(|| panic!("{m} is in ids.json, not in BLOCKS"));
            let entry = ids["entries"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["id"] == *m)
                .unwrap_or_else(|| panic!("{m} has no entry"));
            let logic = &entry["logic"];
            if logic.is_object() {
                assert_eq!(logic["facing"].as_str(), s.facing, "{m} facing");
                let level = logic["level"]["prop"].as_str().map(|p| {
                    (p, logic["level"]["max"].as_u64().unwrap() as u8)
                });
                assert_eq!(level, s.level, "{m} level");
            }
        }
        let ducts: Vec<&str> = ids["ducts"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        for b in BLOCKS.iter().filter(|b| b.kind == Kind::Duct) {
            assert!(ducts.contains(&b.id), "{} not in ids.json ducts", b.id);
        }
        for b in BLOCKS.iter().filter(|b| b.id != "ntm:teste" && !matches!(b.kind, Kind::Bomb | Kind::Duct)) {
            assert!(machines.contains(&b.id), "{} not in ids.json machines", b.id);
        }
        // Bombs: `bombs`, each a redstone lamp whose `lit` the logic reads,
        // with every component its screen asks for.
        let bombs: Vec<&str> = ids["bombs"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        let entry = |id: &str| {
            ids["entries"].as_array().unwrap().iter().find(|e| e["id"] == id).cloned().unwrap_or_else(|| panic!("{id} has no entry"))
        };
        for b in BLOCKS.iter().filter(|b| b.kind == Kind::Bomb) {
            assert!(bombs.contains(&b.id), "{} not in ids.json bombs", b.id);
            let e = entry(b.id);
            assert_eq!(e["template"], "minecraft:redstone_lamp", "{}", b.id);
            assert_eq!(e["logic"]["powered"], BOMB_POWERED, "{}", b.id);
            for s in crate::bomb::bomb_type(b.id).unwrap().slots {
                assert_eq!(entry(s.item)["type"], "item", "{}", s.item);
            }
        }
        for b in &bombs {
            assert!(is_bomb(b), "{b} in ids.json bombs, not in BLOCKS");
        }
        assert_eq!(entry(crate::bomb::DETONATOR)["type"], "item");
        let internal = ids["internal_blocks"].as_array().unwrap();
        assert!(internal.iter().any(|b| b["id"] == TOKEN));
        let token = content("data/ntm/horizon/block/logic_token.json").unwrap();
        assert_eq!(token["template"], "minecraft:composter");
    }
}
