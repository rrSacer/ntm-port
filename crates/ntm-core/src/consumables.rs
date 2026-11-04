//! Use behaviour of the consumable families, independent of the native host.
//!
//! `data/consumables.json` is the table; each entry names the Java class and
//! `ModItems` line its numbers come from, and `tools/content-gen/
//! test_consumables.py` re-reads those Java sources and compares every
//! number. The glue (`native/ntm/mixins/ntm/item_hooks.rs`) only applies a
//! spec to the player in one atomic commit.
use serde::Deserialize;
use std::{collections::HashMap, sync::OnceLock};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Animation {
    #[default]
    None,
    Eat,
    Drink,
    /// `EnumAction.bow`, the cigarette's pose.
    Bow,
}

/// What `onItemRightClick` checks before it calls `setItemInUse`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gate {
    /// Starts unconditionally (`ItemEnergy`, `ItemPill`, `ItemMuchoMango`).
    #[default]
    Always,
    /// `ItemFood.onItemRightClick` with `alwaysEdible` false:
    /// `EntityPlayer.canEat(false)` is `needFood() && !disableDamage`.
    Food,
    /// Same with `alwaysEdible`: `!disableDamage` only. In 1.7.10 the
    /// creative abilities set `disableDamage`, so creative cannot eat.
    FoodAlways,
    /// `ItemCanteen`: only a flask whose damage (the cooldown) is zero.
    Cooled,
}

/// How the used stack shrinks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Consume {
    Never,
    /// `stackSize--` whatever the game mode (`ItemFood`, `ItemSyringe`).
    #[default]
    Always,
    /// `if(!creative) --stackSize` (`ItemEnergy`, `ItemBDCL`).
    UnlessCreative,
    /// The stack stays and takes `set_damage` (`ItemCanteen`).
    SetDamage,
}

/// Where a returned item goes when the inventory has no room for it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Overflow {
    /// `addItemStackToInventory` with the result ignored: the item is gone.
    Lost,
    /// `tryAddItem` / `dropPlayerItemWithRandomChoice(stack, false)`.
    #[default]
    Thrown,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Give {
    pub item: String,
    #[serde(default = "one")]
    pub count: u32,
    /// `if(stack.stackSize <= 0) return new ItemStack(item)`: the last unit
    /// of the stack is replaced in its slot instead of searching a free one.
    #[serde(default)]
    pub replace: bool,
    #[serde(default)]
    pub overflow: Overflow,
    /// Whether the creative player gets it. `ItemEnergy` skips creative.
    #[serde(default = "yes")]
    pub creative: bool,
}
fn one() -> u32 {
    1
}
fn yes() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(from = "(String, i32, u8)")]
pub struct Effect {
    pub id: String,
    pub ticks: i32,
    pub amplifier: u8,
}
impl From<(String, i32, u8)> for Effect {
    fn from((id, ticks, amplifier): (String, i32, u8)) -> Self {
        Self {
            id,
            ticks,
            amplifier,
        }
    }
}

/// `ContaminationUtil.contaminate(.., RAD_BYPASS, n)` or
/// `HbmLivingProps.incrementRadiation(n)`, in milliRAD.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Dose {
    pub millirad: i64,
    /// `contaminate` refuses creative players; `incrementRadiation` does not.
    #[serde(default)]
    pub creative: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct UseSpec {
    #[serde(skip)]
    pub id: String,
    /// Java class, registration line and any rule that is not obvious.
    pub origin: String,
    #[serde(default)]
    pub ticks: i32,
    #[serde(default)]
    pub animation: Animation,
    #[serde(default)]
    pub gate: Gate,
    /// An item the player must carry (`inventory.hasItem`).
    #[serde(default)]
    pub requires: Option<String>,
    #[serde(default)]
    pub consume: Consume,
    #[serde(default)]
    pub set_damage: u32,
    #[serde(default)]
    pub food: Option<(i32, f32)>,
    #[serde(default)]
    pub heal: f32,
    #[serde(default)]
    pub effects: Vec<Effect>,
    #[serde(default)]
    pub dose: Option<Dose>,
    #[serde(default)]
    pub radaway: u32,
    #[serde(default)]
    pub cooldown: i32,
    #[serde(default)]
    pub gives: Vec<Give>,
    /// Java sound id; see [`sound`].
    #[serde(default)]
    pub sound: Option<String>,
    /// A behaviour the glue implements by name.
    #[serde(default)]
    pub special: Option<String>,
}

#[derive(Deserialize)]
struct File {
    entries: HashMap<String, UseSpec>,
}

fn table() -> &'static HashMap<String, UseSpec> {
    static T: OnceLock<HashMap<String, UseSpec>> = OnceLock::new();
    T.get_or_init(|| {
        let mut file: File = serde_json::from_str(include_str!("../data/consumables.json"))
            .expect("checked-in consumables table");
        for (id, spec) in &mut file.entries {
            spec.id = id.clone();
        }
        file.entries
    })
}

/// A missing definition is inert.
pub fn use_spec(id: &str) -> Option<&'static UseSpec> {
    table().get(id)
}

pub fn ids() -> impl Iterator<Item = &'static str> {
    table().keys().map(String::as_str)
}

impl UseSpec {
    /// Whether `onItemRightClick` starts the use. `food_level` and `damage`
    /// are the player's and the stack's.
    pub fn may_start(
        &self,
        creative: bool,
        food_level: i32,
        damage: u32,
        has_required: bool,
    ) -> bool {
        if self.requires.is_some() && !has_required {
            return false;
        }
        match self.gate {
            Gate::Always => true,
            Gate::Food => !creative && food_level < 20,
            Gate::FoodAlways => !creative,
            Gate::Cooled => damage == 0,
        }
    }

    /// `FoodStats.addStats(heal, modifier)` clamps the food level to 20 and
    /// the saturation to the food level, so a heal beyond 20 (`nugget` heals
    /// 200) ends in the same state as 20. The engine takes 0..=20.
    pub fn food_stats(&self) -> Option<(i32, f32)> {
        self.food.map(|(n, s)| (n.clamp(0, 20), s))
    }

    /// Whether the stack shrinks for this player.
    pub fn consumes(&self, creative: bool) -> bool {
        match self.consume {
            Consume::Always => true,
            Consume::UnlessCreative => !creative,
            Consume::Never | Consume::SetDamage => false,
        }
    }
}

/// A Java sound and the engine's vanilla event that stands for it: the port
/// carries no `sounds.json` of its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sound {
    pub event: &'static str,
    pub volume: f32,
    pub pitch: f32,
    /// `pitch + random * spread`, as `nextFloat() * 0.1F + 0.9F`.
    pub spread: f32,
}

pub fn sound(java: &str) -> Option<Sound> {
    let (event, volume, pitch, spread) = match java {
        "random.burp" => ("minecraft:entity.player.burp", 0.5, 0.9, 0.1),
        "random.orb" => ("minecraft:entity.experience_orb.pickup", 1.0, 1.0, 0.0),
        "hbm:item.syringe" => ("minecraft:entity.arrow.hit_player", 1.0, 1.0, 0.0),
        "hbm:item.radaway" => ("minecraft:item.bottle.fill", 1.0, 1.0, 0.0),
        "hbm:item.unpack" => ("minecraft:item.bundle.drop_contents", 1.0, 1.0, 0.0),
        "hbm:player.gulp" => ("minecraft:entity.generic.drink", 1.0, 1.0, 0.0),
        "hbm:player.groan" => ("minecraft:entity.player.breath", 1.0, 1.0, 0.0),
        "hbm:player.cough" => ("minecraft:entity.panda.sneeze", 1.0, 1.0, 0.0),
        "hbm:entity.vomit" => ("minecraft:entity.fox.spit", 1.0, 1.0, 0.0),
        _ => return None,
    };
    Some(Sound {
        event,
        volume,
        pitch,
        spread,
    })
}

/// How many player ticks `ItemBDCL` takes: `getMaxItemUseDuration` is 40, but
/// `onUsingTick(count)` also does `itemInUseCount--` whenever `count <= 24 &&
/// count % 4 == 0`, and drinks (`count == 1`) on the tick it sees 1.
pub fn bdcl_ticks() -> i32 {
    let mut count = 40;
    let mut ticks = 0;
    loop {
        ticks += 1;
        if count == 1 {
            return ticks;
        }
        if count <= 24 && count % 4 == 0 {
            count -= 1;
        }
        count -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(id: &str) -> &'static UseSpec {
        use_spec(id).unwrap_or_else(|| panic!("{id} has no spec"))
    }
    fn fx(id: &str) -> Vec<(&str, i32, u8)> {
        spec(id)
            .effects
            .iter()
            .map(|e| (e.id.as_str(), e.ticks, e.amplifier))
            .collect()
    }
    fn content_ids() -> std::collections::HashSet<String> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content/ids.json");
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        json["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_str().unwrap().to_owned())
            .collect()
    }

    /// Every item a spec names, and every sound it plays, exists.
    #[test]
    fn every_reference_resolves() {
        let known = content_ids();
        for id in ids() {
            let s = spec(id);
            assert!(known.contains(id), "{id} is not in content/ids.json");
            for give in &s.gives {
                assert!(known.contains(&give.item), "{id} gives {}", give.item);
            }
            if let Some(required) = &s.requires {
                assert!(known.contains(required), "{id} requires {required}");
            }
            if let Some(java) = &s.sound {
                assert!(sound(java).is_some(), "{id}: no event for {java}");
            }
            for e in &s.effects {
                assert!(e.ticks > 0, "{id}: {} lasts nothing", e.id);
            }
        }
    }

    /// ItemEnergy: a can is `makeCan()` (can_empty + ring_pull, no opener), a
    /// bottle is `makeBottle(bottle, cap)` and needs the bottle opener.
    #[test]
    fn energy_drinks_follow_item_energy() {
        let can = spec("ntm:can_smart");
        assert_eq!((can.ticks, can.animation), (32, Animation::Drink));
        assert_eq!(can.requires, None);
        assert_eq!(can.gives.len(), 2);
        assert_eq!(can.gives[0].item, "ntm:ring_pull");
        assert!(!can.gives[0].replace);
        assert_eq!(can.gives[1].item, "ntm:can_empty");
        assert!(can.gives[1].replace);
        assert_eq!(
            fx("ntm:can_smart"),
            [
                ("speed", 600, 1),
                ("resistance", 600, 2),
                ("strength", 600, 0)
            ]
        );
        let bottle = spec("ntm:bottle_nuka");
        assert_eq!(bottle.requires.as_deref(), Some("ntm:bottle_opener"));
        assert_eq!(bottle.heal, 4.0);
        assert_eq!(bottle.dose.as_ref().map(|d| d.millirad), Some(5000));
        // `bottle_cherry` takes cap_nuka too (makeBottle(bottle_empty, cap_nuka)).
        assert_eq!(spec("ntm:bottle_cherry").gives[0].item, "ntm:cap_nuka");
        assert_eq!(spec("ntm:bottle2_korl").gives[1].item, "ntm:bottle2_empty");
        // coffee_radium raises the dose directly, creative included.
        let radium = spec("ntm:coffee_radium").dose.as_ref().unwrap();
        assert_eq!((radium.millirad, radium.creative), (500_000, true));
        assert!(!spec("ntm:bottle_quantum").dose.as_ref().unwrap().creative);
        assert_eq!(
            spec("ntm:bottle_quantum").dose.as_ref().unwrap().millirad,
            15_000
        );
        // Every drink skips creative's stack and returns nothing there.
        for id in ids().filter(|i| spec(i).origin.starts_with("ItemEnergy")) {
            let s = spec(id);
            assert_eq!(s.consume, Consume::UnlessCreative, "{id}");
            assert!(
                s.gives
                    .iter()
                    .all(|g| !g.creative && g.overflow == Overflow::Lost),
                "{id}"
            );
        }
    }

    #[test]
    fn gates_follow_can_eat_and_the_canteen() {
        let candy = spec("ntm:cotton_candy");
        assert!(
            candy.may_start(false, 20, 0, false),
            "alwaysEdible at full hunger"
        );
        assert!(
            !candy.may_start(true, 5, 0, false),
            "1.7.10 creative cannot eat"
        );
        let can = spec("ntm:can_bepis");
        assert!(
            can.may_start(true, 20, 0, false),
            "ItemEnergy never asks canEat"
        );
        let bottle = spec("ntm:bottle_rad");
        assert!(!bottle.may_start(false, 5, 0, false), "no bottle opener");
        assert!(bottle.may_start(false, 5, 0, true));
        let canteen = spec("ntm:canteen_vodka");
        assert!(canteen.may_start(true, 20, 0, false));
        assert!(!canteen.may_start(false, 5, 1, false), "still cooling down");
        assert_eq!(
            (canteen.consume, canteen.set_damage),
            (Consume::SetDamage, 180)
        );
        assert!(!canteen.consumes(false));
    }

    #[test]
    fn mango_is_a_long_drink_with_item_food_stats() {
        let mango = spec("ntm:mucho_mango");
        assert_eq!((mango.ticks, mango.animation), (200, Animation::Drink));
        assert_eq!(mango.food_stats(), Some((10, 0.6)));
        assert_eq!(fx("ntm:mucho_mango"), [("speed", 200, 0)]);
        assert!(
            mango.may_start(false, 20, 0, false),
            "right click is unconditional"
        );
        assert!(mango.consumes(true), "ItemFood decrements in every mode");
    }

    /// `FoodStats.addStats` with the nugget's heal of 200 saturates the same
    /// way as the engine's 0..=20 limit.
    #[test]
    fn a_heal_beyond_twenty_ends_in_the_same_state() {
        fn java(food: i32, sat: f32, heal: i32, modifier: f32) -> (i32, f32) {
            let food = (food + heal).min(20);
            (food, (sat + heal as f32 * modifier * 2.0).min(food as f32))
        }
        fn engine(food: i32, sat: f32, (n, s): (i32, f32)) -> (i32, f32) {
            let food = (food + n).clamp(0, 20);
            (food, (sat + n as f32 * s * 2.0).clamp(0.0, food as f32))
        }
        let nugget = UseSpec {
            food: Some((200, 1.0)),
            ..Default::default()
        };
        for (food, sat) in [(0, 0.0), (7, 3.5), (19, 19.0)] {
            assert_eq!(
                java(food, sat, 200, 1.0),
                engine(food, sat, nugget.food_stats().unwrap())
            );
        }
    }

    /// `ItemBDCL.onUsingTick` runs the count 40 → 1 with the extra decrements.
    #[test]
    fn the_bdcl_drinks_in_thirty_four_ticks() {
        assert_eq!(bdcl_ticks(), 34);
        assert_eq!(spec("ntm:bdcl").ticks, bdcl_ticks());
    }

    #[test]
    fn sounds_map_to_vanilla_events() {
        let burp = sound("random.burp").unwrap();
        assert_eq!((burp.volume, burp.pitch, burp.spread), (0.5, 0.9, 0.1));
        assert!(
            sound("hbm:item.syringe")
                .unwrap()
                .event
                .starts_with("minecraft:")
        );
        assert!(sound("hbm:missing").is_none());
    }

    /// The five items the proof batch already carried keep their numbers.
    #[test]
    fn the_proof_batch_is_unchanged() {
        assert_eq!(
            fx("ntm:cotton_candy"),
            [
                ("poison", 300, 0),
                ("wither", 100, 0),
                ("weakness", 500, 2),
                ("speed", 500, 2),
                ("resistance", 600, 4)
            ]
        );
        assert_eq!(spec("ntm:cotton_candy").food_stats(), Some((5, 0.6)));
        assert_eq!(fx("ntm:syringe_metal_medx"), [("resistance", 4800, 2)]);
        let medx = spec("ntm:syringe_metal_medx");
        assert!(medx.gives[0].replace && medx.gives[0].creative);
        assert_eq!(spec("ntm:radaway").radaway, 140);
        assert!(!spec("ntm:radaway").gives[0].replace);
    }
}
