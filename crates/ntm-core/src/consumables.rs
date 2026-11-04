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
    /// The right click never starts the use here (`ItemPancake` without the
    /// BJ armor, which the port does not have); `message` says why.
    Never,
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

/// Which effects a use removes.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cure {
    /// `clearActivePotions`.
    All,
    /// A list of `removePotionEffect`.
    Effects(Vec<String>),
}

/// `attackEntityFrom(source, amount)` on the player, `times` times, each one
/// only with probability `1 / chance` when `chance` is set (`nextInt(n) == 0`).
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Hurt {
    pub amount: f32,
    #[serde(default = "one")]
    pub times: u32,
    #[serde(default)]
    pub chance: u32,
    #[serde(default = "kill")]
    pub damage_type: String,
}
fn kill() -> String {
    "minecraft:generic_kill".into()
}

/// One part a loot crate can hold, drawn with this weight.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Loot {
    pub item: String,
    pub weight: f32,
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
    /// `VersatileConfig.hasPotionSickness(player)` stops the use. The
    /// `applyPotionSickness` calls are no-ops with the default PotionConfig
    /// (OFF), but the effect itself can still come from `pill_herbal`.
    #[serde(default)]
    pub sickness_gate: bool,
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
    pub cure: Option<Cure>,
    #[serde(default)]
    pub hurt: Option<Hurt>,
    /// `setHealth(max(health - n, 0))`, which skips armor and hurt time.
    #[serde(default)]
    pub hp_cost: f32,
    /// Experience points the use takes (refused without them) and gives.
    #[serde(default)]
    pub xp_cost: i32,
    #[serde(default)]
    pub xp_gain: i32,
    #[serde(default)]
    pub radaway: u32,
    #[serde(default)]
    pub cooldown: i32,
    #[serde(default)]
    pub gives: Vec<Give>,
    /// Java sound id; see [`sound`].
    #[serde(default)]
    pub sound: Option<String>,
    /// The chat line a refused use shows (`addChatComponentMessage`).
    #[serde(default)]
    pub message: Option<String>,
    /// `ItemLootCrate`: one part drawn by weight (`rand` over the list, kept
    /// with the part's rarity chance). `Some` with no parts is a crate whose
    /// Java list is empty, which throws there and does nothing here.
    #[serde(default)]
    pub loot: Option<Vec<Loot>>,
    /// Java items the row would add that the port does not have, by their
    /// Java name (`ModItems.geiger_counter x1`); they are not given.
    #[serde(default)]
    pub unported: Vec<String>,
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
        let mut all = HashMap::new();
        // Kits are generated from ItemStarterKit (tools/content-gen/kits.py).
        for text in [
            include_str!("../data/consumables.json"),
            include_str!("../data/kits.json"),
        ] {
            let file: File = serde_json::from_str(text).expect("checked-in consumables table");
            for (id, mut spec) in file.entries {
                spec.id = id.clone();
                assert!(
                    all.insert(id.clone(), spec).is_none(),
                    "{id} is in two tables"
                );
            }
        }
        all
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
        sick: bool,
    ) -> bool {
        if (self.requires.is_some() && !has_required) || (self.sickness_gate && sick) {
            return false;
        }
        match self.gate {
            Gate::Always => true,
            Gate::Food => !creative && food_level < 20,
            Gate::FoodAlways => !creative,
            Gate::Cooled => damage == 0,
            Gate::Never => false,
        }
    }

    /// `FoodStats.addStats(heal, modifier)` clamps the food level to 20 and
    /// the saturation to the food level, so a heal beyond 20 (`nugget` heals
    /// 200) ends in the same state as 20, and so does a modifier beyond 20
    /// (`apple_euphemium` has 100: 20 * 20 * 2 already fills any bar). The
    /// engine takes 0..=20 for both.
    pub fn food_stats(&self) -> Option<(i32, f32)> {
        self.food.map(|(n, s)| (n.clamp(0, 20), s.clamp(0.0, 20.0)))
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

impl UseSpec {
    /// The part a loot crate gives for a uniform `roll` in `[0, 1)`.
    pub fn draw(&self, roll: f32) -> Option<&str> {
        let loot = self.loot.as_ref()?;
        let total: f32 = loot.iter().map(|l| l.weight).sum();
        let mut at = roll.clamp(0.0, 0.999_999) * total;
        for l in loot {
            if at < l.weight {
                return Some(&l.item);
            }
            at -= l.weight;
        }
        loot.last().map(|l| l.item.as_str())
    }
}

/// `new ItemLens(60 * 60 * 60 * 20 * 100)`, the AMS lens's `maxDamage`.
pub const AMS_LENS_MAX: i64 = 60 * 60 * 60 * 20 * 100;

/// `ItemLens.addInformation`: the tooltip line of a lens that took `damage`
/// of its `max` (the `damage` long of its NBT, 0 without one).
pub fn lens_line(max: i64, damage: i64) -> String {
    let left = max - damage;
    format!("Durability: {left}/{max} ({}%)", left * 100 / max)
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
        "hbm:item.techBoop" => ("minecraft:block.note_block.bit", 1.0, 1.0, 0.0),
        "hbm:item.geiger1" => ("minecraft:block.lever.click", 1.0, 1.0, 0.0),
        "hbm:item.geiger2" => ("minecraft:block.lever.click", 1.0, 1.1, 0.0),
        "hbm:item.geiger3" => ("minecraft:block.lever.click", 1.0, 1.2, 0.0),
        "hbm:item.geiger4" => ("minecraft:block.lever.click", 1.0, 1.3, 0.0),
        "hbm:item.geiger5" => ("minecraft:block.lever.click", 1.0, 1.4, 0.0),
        "hbm:item.geiger6" => ("minecraft:block.lever.click", 1.0, 1.5, 0.0),
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
                assert!(
                    give.item.starts_with("minecraft:") || known.contains(&give.item),
                    "{id} gives {}",
                    give.item
                );
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
            candy.may_start(false, 20, 0, false, false),
            "alwaysEdible at full hunger"
        );
        assert!(
            !candy.may_start(true, 5, 0, false, false),
            "1.7.10 creative cannot eat"
        );
        let can = spec("ntm:can_bepis");
        assert!(
            can.may_start(true, 20, 0, false, false),
            "ItemEnergy never asks canEat"
        );
        let bottle = spec("ntm:bottle_rad");
        assert!(
            !bottle.may_start(false, 5, 0, false, false),
            "no bottle opener"
        );
        assert!(bottle.may_start(false, 5, 0, true, false));
        let canteen = spec("ntm:canteen_vodka");
        assert!(canteen.may_start(true, 20, 0, false, false));
        assert!(
            !canteen.may_start(false, 5, 1, false, false),
            "still cooling down"
        );
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
            mango.may_start(false, 20, 0, false, false),
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
    /// ItemSyringe: the metal syringes replace the used-up stack with the
    /// empty one; the taint syringe returns two containers and replaces none.
    #[test]
    fn syringes_follow_item_syringe() {
        let stimpak = spec("ntm:syringe_metal_stimpak");
        assert_eq!((stimpak.heal, stimpak.ticks), (5.0, 0));
        assert!(stimpak.sickness_gate && stimpak.gives[0].replace);
        assert_eq!(
            fx("ntm:syringe_metal_psycho"),
            [("resistance", 2400, 0), ("strength", 2400, 0)]
        );
        let sup = spec("ntm:syringe_metal_super");
        assert_eq!((sup.heal, sup.consumes(true)), (25.0, true));
        assert_eq!(fx("ntm:syringe_metal_super"), [("slowness", 200, 0)]);
        let taint = spec("ntm:syringe_taint");
        assert!(
            !taint.sickness_gate,
            "the taint branch never asks for potion sickness"
        );
        assert_eq!(
            taint
                .gives
                .iter()
                .map(|g| (g.item.as_str(), g.replace))
                .collect::<Vec<_>>(),
            [
                ("ntm:syringe_metal_empty", false),
                ("ntm:bottle2_empty", false)
            ]
        );
        assert_eq!(
            fx("ntm:syringe_taint"),
            [("ntm:taint", 1200, 0), ("nausea", 100, 0)]
        );
    }

    #[test]
    fn the_med_bag_heals_fully_and_cures_the_bad_effects() {
        let bag = spec("ntm:med_bag");
        assert!(
            bag.heal >= 1.0e5,
            "setHealth(getMaxHealth()) whatever the maximum"
        );
        assert!(bag.gives.is_empty() && bag.sound.is_none());
        let Some(Cure::Effects(ids)) = &bag.cure else {
            panic!("cure list")
        };
        assert_eq!(ids.len(), 8);
        assert!(
            [
                "blindness",
                "nausea",
                "mining_fatigue",
                "hunger",
                "slowness",
                "poison",
                "weakness",
                "wither"
            ]
            .iter()
            .all(|e| ids.iter().any(|i| i == e))
        );
        assert_eq!(spec("ntm:pill_iodine").cure, bag.cure);
        assert_eq!(spec("ntm:syringe_antidote").cure, Some(Cure::All));
    }

    #[test]
    fn simple_consumables_follow_item_simple_consumable() {
        assert_eq!(spec("ntm:radaway_strong").radaway, 350);
        assert_eq!(spec("ntm:radaway_flush").radaway, 500);
        for id in ["ntm:radaway", "ntm:radaway_strong", "ntm:radaway_flush"] {
            let s = spec(id);
            assert_eq!(
                (
                    s.gives[0].item.as_str(),
                    s.gives[0].replace,
                    s.gives[0].overflow
                ),
                ("ntm:iv_empty", false, Overflow::Thrown)
            );
        }
        assert_eq!(spec("ntm:iv_empty").hp_cost, 5.0);
        assert_eq!(spec("ntm:iv_empty").gives[0].item, "ntm:iv_blood");
        assert_eq!(spec("ntm:iv_blood").heal, 5.0);
        assert_eq!(
            (spec("ntm:iv_xp_empty").xp_cost, spec("ntm:iv_xp").xp_gain),
            (100, 100)
        );
        assert_eq!(
            spec("ntm:syringe_poison")
                .hurt
                .as_ref()
                .map(|h| (h.amount, h.times, h.chance)),
            Some((30.0, 1, 0))
        );
        let awesome = spec("ntm:syringe_awesome");
        assert_eq!(awesome.effects.len(), 11);
        assert!(
            awesome
                .effects
                .iter()
                .any(|e| e.id == "strength" && e.amplifier == 24)
        );
        assert!(
            awesome
                .effects
                .iter()
                .any(|e| e.id == "ntm:radx" && e.ticks == 1000 && e.amplifier == 9)
        );
    }

    /// ItemPill extends ItemFood with heal 0: ten ticks of eating, no `canEat`,
    /// only the potion-sickness test, and the burp.
    #[test]
    fn pills_follow_item_pill() {
        for id in ids().filter(|i| spec(i).origin.starts_with("ItemPill")) {
            let s = spec(id);
            assert_eq!(
                (s.ticks, s.animation, s.gate, s.consume),
                (10, Animation::Eat, Gate::Always, Consume::Always),
                "{id}"
            );
            assert!(
                s.sickness_gate && s.food.is_none() && s.sound.as_deref() == Some("random.burp"),
                "{id}"
            );
            // Creative may take a pill: the pill's own right click skips canEat.
            assert!(s.may_start(true, 20, 0, false, false), "{id}");
            assert!(
                !s.may_start(false, 20, 0, false, true),
                "{id}: potion sickness"
            );
        }
        assert_eq!(fx("ntm:radx"), [("ntm:radx", 3600, 0)]);
        assert_eq!(fx("ntm:pill_red"), [("ntm:death", 72000, 0)]);
        assert_eq!(fx("ntm:five_htp"), [("ntm:stability", 12000, 0)]);
        assert_eq!(fx("ntm:fmn"), [("blindness", 60, 0)]);
        assert_eq!(
            fx("ntm:chocolate"),
            [
                ("haste", 1200, 3),
                ("speed", 1200, 3),
                ("jump_boost", 1200, 3)
            ]
        );
        let overdose = spec("ntm:chocolate").hurt.as_ref().unwrap();
        assert_eq!(
            (overdose.amount, overdose.times, overdose.chance),
            (1000.0, 1, 25)
        );
        let plan_c = spec("ntm:plan_c").hurt.as_ref().unwrap();
        assert_eq!(
            (plan_c.amount, plan_c.times, plan_c.chance),
            (1000.0, 10, 0)
        );
        let herbal = spec("ntm:pill_herbal");
        assert_eq!(
            herbal.dose.as_ref().map(|d| (d.millirad, d.creative)),
            Some((-100_000, true))
        );
        assert_eq!(
            fx("ntm:pill_herbal"),
            [
                ("nausea", 200, 0),
                ("weakness", 12000, 2),
                ("mining_fatigue", 12000, 2),
                ("poison", 100, 2),
                ("ntm:potion_sickness", 12000, 0)
            ]
        );
    }

    /// The pills whose Java also sets digamma, asbestos or black lung keep
    /// only what the port has; the missing system is named in the catalog doc.
    #[test]
    fn pills_that_need_lung_or_digamma_state_do_their_visible_part_only() {
        for id in ["ntm:siox", "ntm:xanax"] {
            let s = spec(id);
            assert!(
                s.effects.is_empty() && s.cure.is_none() && s.dose.is_none() && s.hurt.is_none(),
                "{id}"
            );
        }
    }

    #[test]
    fn bio_wafer_is_an_item_lemon_that_needs_hunger() {
        let wafer = spec("ntm:bio_wafer");
        assert_eq!(
            (wafer.ticks, wafer.gate, wafer.food_stats()),
            (32, Gate::Food, Some((4, 2.0)))
        );
        assert!(!wafer.may_start(false, 20, 0, false, false));
        assert!(wafer.may_start(false, 19, 0, false, false));
        assert!(
            !wafer.may_start(true, 0, 0, false, false),
            "creative cannot eat"
        );
    }
    /// `ItemLemon` and the vanilla foods it shares a bar with: needs hunger,
    /// 32 ticks, the burp, `ItemFood.onEaten` stats.
    #[test]
    fn item_lemon_foods_follow_item_food() {
        for (id, stats) in [
            ("ntm:lemon", (3, 0.5)),
            ("ntm:loops", (4, 0.25)),
            ("ntm:fooditem", (2, 5.0)),
            ("ntm:cheese_quesadilla", (8, 1.0)),
            ("ntm:ingot_smore", (10, 20.0)),
            ("ntm:glowing_stew", (6, 0.6)),
        ] {
            let s = spec(id);
            assert_eq!(
                (s.ticks, s.animation, s.gate, s.food_stats()),
                (32, Animation::Eat, Gate::Food, Some(stats)),
                "{id}"
            );
            assert!(s.consumes(true), "{id}: ItemFood decrements in every mode");
            assert!(!s.may_start(false, 20, 0, false, false), "{id}: not hungry");
        }
        // setPotionEffect on glyphid_meat_grilled is dead code: ItemLemon.onFoodEaten never calls super.
        assert!(spec("ntm:glyphid_meat_grilled").effects.is_empty());
        let stew = spec("ntm:loop_stew");
        assert_eq!(
            fx("ntm:loop_stew"),
            [
                ("regeneration", 400, 1),
                ("resistance", 1200, 2),
                ("speed", 1200, 1),
                ("strength", 400, 2)
            ]
        );
        assert_eq!(
            (stew.gives[0].item.as_str(), stew.gives[0].replace),
            ("minecraft:bowl", true)
        );
        for soup in [
            "ntm:glowing_stew",
            "ntm:balefire_scrambled",
            "ntm:balefire_and_ham",
        ] {
            assert_eq!(spec(soup).gives[0].item, "minecraft:bowl", "{soup}");
        }
    }

    #[test]
    fn the_two_medicines_need_hunger_and_induce_vomiting() {
        // `setAlwaysEdible` is guarded by `this == ModItems.med_ipecac`, which
        // is still null while the constructor runs: the guard never fires.
        for id in ["ntm:med_ipecac", "ntm:med_ptsd"] {
            let s = spec(id);
            assert_eq!(s.gate, Gate::Food, "{id}");
            assert_eq!(s.food_stats(), Some((0, 0.0)));
            assert_eq!(fx(id), [("hunger", 50, 49)]);
            assert_eq!(s.special.as_deref(), Some("vomit"));
            assert!(!s.may_start(false, 20, 0, false, false), "{id} at full hunger");
            assert!(s.may_start(false, 19, 0, false, false), "{id} hungry");
        }
    }

    #[test]
    fn an_oversized_modifier_ends_in_the_same_state() {
        // Euphemium apple: heal 20, saturation modifier 100.
        let apple = spec("ntm:apple_euphemium");
        assert_eq!(apple.food_stats(), Some((20, 20.0)));
        for (food, sat) in [(0, 0.0), (10, 4.0), (19, 19.0)] {
            let java = {
                let f = (food + 20).min(20);
                (f, (sat + 20.0 * 100.0 * 2.0_f32).min(f as f32))
            };
            let (n, m) = apple.food_stats().unwrap();
            let f = (food + n).clamp(0, 20);
            assert_eq!(java, (f, (sat + n as f32 * m * 2.0).clamp(0.0, f as f32)));
        }
        assert_eq!(
            fx("ntm:apple_euphemium"),
            [
                ("resistance", i32::MAX, 120),
                ("fire_resistance", i32::MAX, 0),
                ("saturation", i32::MAX, 120)
            ]
        );
    }

    #[test]
    fn the_pancake_is_refused_without_the_bj_armor() {
        let pancake = spec("ntm:pancake");
        assert_eq!(pancake.gate, Gate::Never);
        assert!(!pancake.may_start(false, 0, 0, false, false));
        assert_eq!(
            pancake.message.as_deref(),
            Some("Your teeth are too soft to eat this.")
        );
    }

    #[test]
    fn cigarettes_bow_for_thirty_ticks_and_raise_the_dose() {
        let cig = spec("ntm:cigarette");
        assert_eq!(
            (cig.ticks, cig.animation, cig.consume),
            (30, Animation::Bow, Consume::Always)
        );
        assert_eq!(
            cig.dose.as_ref().map(|d| (d.millirad, d.creative)),
            Some((100_000, true))
        );
        let pipe = spec("ntm:crackpipe");
        assert_eq!((pipe.heal, pipe.special.as_deref()), (10.0, Some("smoke")));
        assert_eq!(fx("ntm:crackpipe"), [("nausea", 200, 0)]);
    }

    #[test]
    fn the_rag_and_the_peas() {
        let rag = spec("ntm:rag");
        assert_eq!(
            (rag.gives[0].item.as_str(), rag.gives[0].overflow, rag.ticks),
            ("ntm:rag_piss", Overflow::Lost, 0)
        );
        assert_eq!(spec("ntm:peas").consume, Consume::UnlessCreative);
    }

    #[test]
    fn the_vegan_schnitzel_burns_and_launches() {
        let s = spec("ntm:schnitzel_vegan");
        assert_eq!(
            fx("ntm:schnitzel_vegan"),
            [
                ("blindness", 200, 0),
                ("nausea", 600, 0),
                ("hunger", 3600, 4),
                ("wither", 60, 0)
            ]
        );
        assert_eq!(
            (s.gate, s.special.as_deref()),
            (Gate::Food, Some("schnitzel_vegan"))
        );
    }
    /// ItemStarterKit: every `addItemStackToInventory` result is ignored, so
    /// what does not fit is lost; `giveHaz` is the armor, by tier.
    #[test]
    fn kits_follow_item_starter_kit() {
        let kit = spec("ntm:nuke_starter_kit");
        assert_eq!(kit.sound.as_deref(), Some("hbm:item.unpack"));
        assert_eq!(kit.special.as_deref(), Some("hazmat_kit:1"));
        assert_eq!(
            (kit.gives[0].item.as_str(), kit.gives[0].count),
            ("ntm:ingot_uranium", 32)
        );
        assert!(
            kit.gives
                .iter()
                .all(|g| g.overflow == Overflow::Lost && g.creative)
        );
        assert!(
            kit.gives.iter().any(|g| g.item == "ntm:geiger_counter"),
            "the Geiger counter is in the port now"
        );
        assert!(
            spec("ntm:nuke_advanced_kit")
                .unported
                .iter()
                .any(|u| u == "ModBlocks.machine_uf6_tank x2")
        );
        assert_eq!(
            spec("ntm:hazmat_kit").special.as_deref(),
            Some("hazmat_kit:0")
        );
        assert_eq!(
            spec("ntm:hazmat_red_kit").special.as_deref(),
            Some("hazmat_kit:1")
        );
        assert_eq!(
            spec("ntm:hazmat_grey_kit").special.as_deref(),
            Some("hazmat_kit:2")
        );
        assert!(spec("ntm:hazmat_kit").gives.is_empty());
        // Metadata items resolve to their variants.
        let electric = spec("ntm:nuke_electric_kit");
        assert!(
            electric
                .gives
                .iter()
                .any(|g| g.item == "ntm:circuit__vacuum_tube" && g.count == 16)
        );
        assert!(
            electric
                .gives
                .iter()
                .any(|g| g.item == "ntm:canister_full__diesel" && g.count == 16)
        );
        let advanced = spec("ntm:nuke_advanced_kit");
        assert!(
            advanced
                .gives
                .iter()
                .any(|g| g.item == "ntm:fluid_barrel_full__coolant" && g.count == 4)
        );
        assert!(
            spec("ntm:multi_kit")
                .gives
                .iter()
                .any(|g| g.item == "minecraft:tnt" && g.count == 26)
        );
        assert_eq!(
            spec("ntm:euphemium_kit").gives.len(),
            0,
            "no euphemium armor in the port"
        );
    }

    #[test]
    fn the_stealth_boy_is_thirty_seconds_of_invisibility() {
        let boy = spec("ntm:stealth_boy");
        assert_eq!(fx("ntm:stealth_boy"), [("invisibility", 600, 1)]);
        assert_eq!(boy.special.as_deref(), Some("stealth_boy"));
    }

    /// `ItemLootCrate.choose`: a part is drawn uniformly and kept with its
    /// rarity's chance, so the chance is its weight. Two of the crates have an
    /// empty list in the Java, where `nextInt(0)` throws.
    #[test]
    fn loot_crates_draw_by_rarity_weight() {
        assert_eq!(spec("ntm:loot_10").loot.as_deref(), Some(&[][..]));
        assert_eq!(spec("ntm:loot_10").draw(0.5), None);
        let misc = spec("ntm:loot_misc");
        let total: f32 = misc.loot.as_ref().unwrap().iter().map(|l| l.weight).sum();
        assert!((total - (0.2 + 0.1 + 0.02 + 0.1 + 0.02 + 0.01)).abs() < 1e-6);
        assert_eq!(misc.draw(0.0), Some("ntm:mp_warhead_10_taint"));
        assert_eq!(misc.draw(0.999), Some("ntm:mp_warhead_15_turbine"));
        // Equal weights split a roll evenly.
        let mut seen = std::collections::HashMap::new();
        for i in 0..1000 {
            *seen
                .entry(misc.draw(i as f32 / 1000.0).unwrap())
                .or_insert(0) += 1;
        }
        assert!(
            (430..=460).contains(&seen["ntm:mp_warhead_10_taint"]),
            "0.2 of 0.45"
        );
        assert!(seen["ntm:mp_warhead_10_taint"] > seen["ntm:mp_warhead_10_cloud"]);
    }
    #[test]
    fn the_ams_lens_tooltip_is_the_java_line() {
        assert_eq!(AMS_LENS_MAX, 432_000_000);
        assert_eq!(
            lens_line(AMS_LENS_MAX, 0),
            "Durability: 432000000/432000000 (100%)"
        );
        assert_eq!(
            lens_line(AMS_LENS_MAX, 108_000_000),
            "Durability: 324000000/432000000 (75%)"
        );
        // The percentage truncates, as `(int)((max - damage) * 100 / max)` does.
        assert_eq!(lens_line(1000, 1), "Durability: 999/1000 (99%)");
    }
}
