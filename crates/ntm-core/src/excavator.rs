//! The excavator, bedrock stage 1 (docs/maquinas.md, "Bedrock"):
//! `tileentity/machine/TileEntityMachineExcavator.java`, its drill bits
//! (`items/machine/ItemDrillbit.java`). The world part (the shaft, the
//! deposit) is `server/excavator_server.rs`.
//!
//! What the port does: with a drill bit and 10 000 HE a tick, it digs a
//! shaft down from 4 blocks under its core, ring by ring (3×3, then 5×5,
//! 7×7 with the effect upgrade), a ring after its combined hardness ÷ the
//! bit's speed ticks; the blocks go to its buffer. Its four switches: the
//! crusher (the drops through the shredder), the walls (sandbags round the
//! shaft), the vein miner and silk touch (both as the bit allows). On a
//! deposit (`ntm:ore_bedrock`) it stays: every 6 000 ÷ speed ticks one
//! extraction, if the bit's tier reaches the deposit's and the tank holds
//! the fluid it asks (which it takes). Left for later: the output into a
//! container in front; ores drop themselves (the server has no loot
//! tables, so no fortune but the deposit's).

use crate::{
    energy::{EnergyNode, Role},
    fluid::{FluidNode, Tank},
    machine::Logic,
    server::container_bridge::{Slots, Slotted, Stack},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub mod slot {
    pub const BATTERY: usize = 0;
    pub const FLUID_ID: usize = 1;
    pub const UPGRADES: std::ops::RangeInclusive<usize> = 2..=3;
    pub const DRILL: usize = 4;
    pub const BUFFER: std::ops::Range<usize> = 5..14;
    pub const COUNT: usize = 14;
}

pub const MAX_POWER: u64 = 1_000_000;
pub const HE_PER_TICK: u64 = 10_000;
pub const TANK: u32 = 16_000;
/// A bedrock deposit's "hardness" (`5 * 60 * 20`).
pub const BEDROCK_WORK: f64 = 6_000.0;
pub const STACK: u32 = 64;

/// `EnumDrillType`: (item, speed, tier, fortune, vein, silk).
pub const DRILLS: [(&str, f64, u8, u8, bool, bool); 10] = [
    ("ntm:drillbit__steel", 1.0, 1, 0, false, false),
    ("ntm:drillbit__steel_diamond", 1.0, 1, 2, false, true),
    ("ntm:drillbit__hss", 1.2, 2, 0, true, false),
    ("ntm:drillbit__hss_diamond", 1.2, 2, 3, true, true),
    ("ntm:drillbit__desh", 1.5, 3, 1, true, true),
    ("ntm:drillbit__desh_diamond", 1.5, 3, 4, true, true),
    ("ntm:drillbit__tcalloy", 2.0, 4, 1, true, true),
    ("ntm:drillbit__tcalloy_diamond", 2.0, 4, 4, true, true),
    ("ntm:drillbit__ferro", 2.5, 5, 1, true, true),
    ("ntm:drillbit__ferro_diamond", 2.5, 5, 4, true, true),
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drill {
    pub speed: f64,
    pub tier: u8,
    pub fortune: u8,
    pub vein: bool,
    pub silk: bool,
}

pub fn drill(item: &str) -> Option<Drill> {
    DRILLS.iter().find(|d| d.0 == item).map(|d| Drill { speed: d.1, tier: d.2, fortune: d.3, vein: d.4, silk: d.5 })
}

/// The upgrades in slots 2–3 (`UpgradeManagerNT`: the tiers of a kind
/// added, to 3 at most).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Upgrades {
    pub speed: u32,
    pub power: u32,
    pub effect: u32,
}

/// The upgrades the excavator takes (`getValidUpgrades`).
pub fn upgrade(item: &str) -> Option<(&'static str, u32)> {
    let rest = item.strip_prefix("ntm:upgrade_")?;
    let (kind, tier) = rest.rsplit_once('_')?;
    let kind = ["speed", "power", "effect"].into_iter().find(|k| *k == kind)?;
    let tier = tier.parse().ok().filter(|t| (1..=3).contains(t))?;
    Some((kind, tier))
}

/// The sandbag the walls are made of (`ModBlocks.barricade`): it drops
/// nothing.
pub const BARRICADE: &str = "ntm:barricade";

/// A block's hardness, for the time a layer takes (the server has no block
/// table: vanilla's for the common ones, 1.5 like stone otherwise). `None`:
/// the drill passes it by (`shouldIgnoreBlock`: air, fluids, unbreakable).
pub fn hardness(id: &str) -> Option<f64> {
    let path = id.split(':').nth(1).unwrap_or(id);
    let h = match path {
        "air" | "cave_air" | "void_air" | "water" | "lava" | "bubble_column" | "bedrock" | "barrier" | "light" | "end_portal_frame" => return None,
        _ if id == "ntm:ore_bedrock" => return Some(BEDROCK_WORK),
        "dirt" | "grass_block" | "sand" | "red_sand" | "gravel" | "clay" | "soul_sand" | "soul_soil" | "mycelium" | "podzol" | "coarse_dirt" | "rooted_dirt" | "mud" => 0.5,
        "netherrack" => 0.4,
        "short_grass" | "tall_grass" | "fern" | "large_fern" | "dead_bush" | "torch" | "wall_torch" | "redstone_wire" | "dandelion" | "poppy" => 0.0,
        "snow" => 0.1,
        "barricade" => 1.0,
        "deepslate" | "cobbled_deepslate" | "tuff" => 3.0,
        "obsidian" | "crying_obsidian" => 50.0,
        "ancient_debris" => 30.0,
        _ if path.starts_with("deepslate_") && path.ends_with("_ore") => 4.5,
        _ if path.ends_with("_ore") => 3.0,
        _ if path.contains("leaves") => 0.2,
        _ if path.ends_with("_log") || path.ends_with("_planks") || path.ends_with("_wood") => 2.0,
        _ => 1.5,
    };
    Some(h)
}

/// What a broken block gives (the port has no loot tables: the block
/// itself, as with silk touch, but for the commonest).
pub fn drop_of(id: &str) -> Option<String> {
    let path = id.split(':').nth(1).unwrap_or(id);
    Some(match path {
        "stone" => "minecraft:cobblestone".into(),
        "deepslate" => "minecraft:cobbled_deepslate".into(),
        "grass_block" | "mycelium" | "podzol" | "rooted_dirt" => "minecraft:dirt".into(),
        "short_grass" | "tall_grass" | "fern" | "large_fern" | "snow" | "fire" | "dead_bush" => return None,
        _ if path.contains("leaves") => return None,
        _ => blockstate_id(id).to_string(),
    })
}

/// What silk touch gives (`createStackedBlock`): the block itself.
pub fn silk_drop_of(id: &str) -> Option<String> {
    let id = blockstate_id(id);
    match id.split(':').nth(1).unwrap_or(id) {
        "fire" | "soul_fire" | "wall_torch" | "redstone_wire" => drop_of(id),
        _ => Some(id.to_string()),
    }
}

/// `isOre`: an ore by its name (the NTM asks the ore dictionary).
pub fn is_ore(id: &str) -> bool {
    let id = blockstate_id(id);
    let path = id.split(':').nth(1).unwrap_or(id);
    path.ends_with("_ore") || (id.starts_with("ntm:ore_") && !path.starts_with("ore_bedrock") && !path.starts_with("ore_oil"))
}

/// A liquid block (the walls stop it, the shaft is emptied of it).
pub fn is_liquid(id: &str) -> bool {
    matches!(blockstate_id(id), "minecraft:water" | "minecraft:lava" | "minecraft:bubble_column")
}

/// `isReplaceable`: what a wall may take the place of.
pub fn replaceable(id: &str) -> bool {
    let id = blockstate_id(id);
    is_liquid(id)
        || matches!(
            id.split(':').nth(1).unwrap_or(id),
            "air" | "cave_air" | "void_air" | "short_grass" | "tall_grass" | "fern" | "large_fern" | "dead_bush" | "snow" | "vine" | "seagrass" | "tall_seagrass" | "fire" | "soul_fire" | "light" | "structure_void" | "glow_lichen" | "hanging_roots"
        )
}

/// `BlockDepth`: depth rock, which turns the drill off.
pub fn is_depth_rock(id: &str) -> bool {
    let id = blockstate_id(id);
    id.starts_with("ntm:stone_depth") || id.starts_with("ntm:ore_depth_")
}

fn blockstate_id(state: &str) -> &str {
    state.split('[').next().unwrap_or(state)
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Excavator {
    /// The engine's slots, laid over on the first tick; a machine loaded
    /// from the save starts with them empty but the right length.
    #[serde(skip, default = "empty_slots")]
    pub slots: Slots,
    pub power: u64,
    pub tank: Tank,
    /// The drill is on (its screen's button 0).
    #[serde(default)]
    pub enabled: bool,
    /// The switches 1–4: the crusher, the walls, the vein miner, silk touch.
    #[serde(default)]
    pub crusher: bool,
    #[serde(default)]
    pub walling: bool,
    #[serde(default)]
    pub vein: bool,
    #[serde(default)]
    pub silk: bool,
    /// Layers dug below the first (`targetDepth`).
    pub depth: u32,
    /// Ticks into the current layer (`ticksWorked`).
    pub worked: u32,
    /// The current layer, as scanned: its work and the deposit in it.
    #[serde(skip)]
    pub layer: Option<Layer>,
    /// Working this tick (the model's animation).
    #[serde(skip)]
    pub operational: bool,
}

/// A ring of the current layer, as scanned (1: the 3×3 round the shaft's
/// axis, then each square's edge).
fn empty_slots() -> Slots {
    vec![None; slot::COUNT]
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub y: i32,
    pub ring: i32,
    pub work: f64,
    /// Nothing in it to dig (`ignoreAll`).
    pub empty: bool,
    pub deposit: Option<crate::Pos>,
}

impl Excavator {
    pub fn new() -> Self {
        Self { slots: vec![None; slot::COUNT], tank: Tank::new(TANK), ..Default::default() }
    }

    pub fn drill(&self) -> Option<Drill> {
        self.slots.get(slot::DRILL).and_then(|s| s.as_ref()).and_then(|s| drill(&s.id))
    }

    pub fn upgrades(&self) -> Upgrades {
        let mut u = Upgrades::default();
        for s in slot::UPGRADES.filter_map(|i| self.slots.get(i)).flatten() {
            match upgrade(&s.id) {
                Some(("speed", t)) => u.speed = (u.speed + t).min(3),
                Some(("power", t)) => u.power = (u.power + t).min(3),
                Some((_, t)) => u.effect = (u.effect + t).min(3),
                None => {}
            }
        }
        u
    }

    /// HE a tick: ×(1 + speed) ÷ (1 + power).
    pub fn consumption(&self) -> u64 {
        let u = self.upgrades();
        HE_PER_TICK * (1 + u64::from(u.speed)) / (1 + u64::from(u.power))
    }

    /// The bit's speed, ×(1 + speed ÷ 2).
    pub fn speed(&self, d: Drill) -> f64 {
        d.speed * (1.0 + f64::from(self.upgrades().speed) / 2.0)
    }

    /// The rings a layer has: 1 + 2 a level of the effect upgrade.
    pub fn radius(&self) -> i32 {
        1 + 2 * self.upgrades().effect as i32
    }

    pub fn can_vein_mine(&self) -> bool {
        self.vein && self.drill().is_some_and(|d| d.vein)
    }

    pub fn can_silk_touch(&self) -> bool {
        self.silk && self.drill().is_some_and(|d| d.silk)
    }

    /// What breaking `block` gives (`breakSingleBlock`): silk touch, then
    /// the crusher (what the shredder turns to scrap or dust stays as it
    /// is); a sandbag nothing.
    pub fn drops(&self, block: &str) -> Vec<Stack> {
        if blockstate_id(block) == BARRICADE {
            return Vec::new();
        }
        let item = if self.can_silk_touch() { silk_drop_of(block) } else { drop_of(block) };
        let Some(item) = item else { return Vec::new() };
        if self.crusher
            && let Some((out, n)) = crate::shredder::Recipes::ntm().result(&item)
            && out != crate::shredder::SCRAP.0
            && out != "ntm:dust"
        {
            return vec![Stack::new(out, n)];
        }
        vec![Stack::new(&item, 1)]
    }

    /// Room in the buffer for all of `stacks` together.
    pub fn fits_all(&self, stacks: &[Stack]) -> bool {
        let mut e = Excavator { slots: self.slots.clone(), ..Default::default() };
        stacks.iter().all(|s| {
            let ok = e.fits(s);
            e.put(s.clone());
            ok
        })
    }

    /// Room in the buffer for `stack` (same item and data piles).
    pub fn fits(&self, stack: &Stack) -> bool {
        let mut n = stack.count;
        for s in &self.slots[slot::BUFFER] {
            match s {
                None => return true,
                Some(o) if same(o, stack) => n = n.saturating_sub(STACK.saturating_sub(o.count)),
                _ => {}
            }
            if n == 0 {
                return true;
            }
        }
        false
    }

    pub fn put(&mut self, stack: Stack) {
        let mut n = stack.count;
        for s in self.slots[slot::BUFFER].iter_mut().flatten() {
            if same(s, &stack) {
                let k = n.min(STACK.saturating_sub(s.count));
                s.count += k;
                n -= k;
            }
        }
        for s in &mut self.slots[slot::BUFFER] {
            if n == 0 {
                break;
            }
            if s.is_none() {
                let k = n.min(STACK);
                *s = Some(Stack { count: k, ..stack.clone() });
                n -= k;
            }
        }
    }

    pub fn screen_data(&self) -> Vec<i32> {
        vec![
            self.power.min(MAX_POWER) as i32,
            MAX_POWER as i32,
            self.tank.mb as i32,
            self.tank.cap as i32,
            crate::fluid::get(&self.tank.fluid).map_or(0, |f| f.color as i32),
            i32::from(self.enabled),
            i32::from(self.operational),
            self.depth as i32,
            i32::from(self.crusher),
            i32::from(self.walling),
            switch(self.vein, self.can_vein_mine()),
            switch(self.silk, self.can_silk_touch()),
            // The drill's amber lamp: on without a bit or the power.
            i32::from(self.enabled && !(self.drill().is_some() && self.power >= self.consumption())),
        ]
    }
}

/// A switch's lamp: 0 off, 1 on (green), 2 on but the bit can't (amber).
fn switch(on: bool, can: bool) -> i32 {
    match (on, can) {
        (false, _) => 0,
        (true, true) => 1,
        (true, false) => 2,
    }
}

fn same(a: &Stack, b: &Stack) -> bool {
    a.id == b.id && a.damage == b.damage && a.custom == b.custom
}

pub mod buttons {
    pub const DRILL: u32 = 0;
    pub const CRUSHER: u32 = 1;
    pub const WALLING: u32 = 2;
    pub const VEIN: u32 = 3;
    pub const SILK: u32 = 4;
}

impl EnergyNode for Excavator {
    fn role(&self) -> Role {
        Role::Consumer
    }
    fn offer(&self) -> u64 {
        0
    }
    fn demand(&self) -> u64 {
        MAX_POWER.saturating_sub(self.power)
    }
    fn give(&mut self, he: u64) {
        self.power += he;
    }
    fn take(&mut self, _he: u64) {}
}

/// Any fluid, the first that comes (the NTM sets it with an identifier).
impl FluidNode for Excavator {
    fn fluid_offer(&self, _f: &str) -> u64 {
        0
    }
    fn fluid_demand(&self, f: &str) -> u64 {
        if self.tank.fluid.is_empty() || self.tank.mb == 0 || self.tank.fluid == f { u64::from(self.tank.cap - self.tank.mb.min(self.tank.cap)) } else { 0 }
    }
    fn fluid_types(&self) -> Vec<String> {
        if self.tank.fluid.is_empty() { Vec::new() } else { vec![self.tank.fluid.clone()] }
    }
    fn accepts_any(&self) -> bool {
        self.tank.mb == 0
    }
    fn fluid_give(&mut self, f: &str, mb: u64) {
        if self.tank.mb == 0 {
            self.tank.set_type(f);
        }
        self.tank.fill(f, mb.min(u64::from(u32::MAX)) as u32);
    }
    fn fluid_take(&mut self, _f: &str, _mb: u64) {}
}

impl Logic for Excavator {
    fn level(&self, max: u8) -> u8 {
        if self.operational { max } else { 0 }
    }
    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        Some(self)
    }
    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(self)
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Escavadora")
            .bar("Energia", self.power, MAX_POWER, "HE")
            .text(format!("Profundidade: {} camadas", self.depth), view::GREY)
            .text("Os itens e o botão ficam na tela com slots (motor com o patch ntm-visual).", view::GREY)
    }
}

impl Slotted for Excavator {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        Excavator::screen_data(self)
    }
    fn button(&mut self, id: u32) -> bool {
        let flag = match id {
            buttons::DRILL => &mut self.enabled,
            buttons::CRUSHER => &mut self.crusher,
            buttons::WALLING => &mut self.walling,
            buttons::VEIN => &mut self.vein,
            buttons::SILK => &mut self.silk,
            _ => return false,
        };
        *flag = !*flag;
        false
    }
    fn save_state(&self) -> Vec<i32> {
        let b = i32::from;
        vec![b(self.enabled), self.depth as i32, self.worked as i32, b(self.crusher), b(self.walling), b(self.vein), b(self.silk)]
    }
    fn load_state(&mut self, v: &[i32]) {
        if let [enabled, depth, worked, rest @ ..] = v {
            self.enabled = *enabled != 0;
            self.depth = (*depth).max(0) as u32;
            self.worked = (*worked).max(0) as u32;
            if let [crusher, walling, vein, silk, ..] = rest {
                (self.crusher, self.walling, self.vein, self.silk) = (*crusher != 0, *walling != 0, *vein != 0, *silk != 0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drills_hardness_and_drops() {
        assert_eq!(drill("ntm:drillbit__hss_diamond"), Some(Drill { speed: 1.2, tier: 2, fortune: 3, vein: true, silk: true }));
        assert_eq!(hardness("minecraft:air"), None);
        assert_eq!(hardness("minecraft:bedrock"), None);
        assert_eq!(hardness("minecraft:deepslate_iron_ore"), Some(4.5));
        assert_eq!(hardness("ntm:ore_bedrock"), Some(BEDROCK_WORK));
        assert_eq!(drop_of("minecraft:stone").as_deref(), Some("minecraft:cobblestone"));
        assert_eq!(drop_of("minecraft:oak_log[axis=y]").as_deref(), Some("minecraft:oak_log"));
        assert_eq!(drop_of("minecraft:oak_leaves[distance=1]"), None);
    }

    #[test]
    fn the_buffer_piles_alike_and_keeps_raw_ores_apart() {
        let mut e = Excavator::new();
        let raw = |c: &str| Stack { custom: Some(c.into()), ..Stack::new("ntm:bedrock_ore_base", 1) };
        e.put(raw("{ntm_ore:{light:1d}}"));
        e.put(raw("{ntm_ore:{light:1d}}"));
        e.put(raw("{ntm_ore:{light:2d}}"));
        let b: Vec<_> = e.slots[slot::BUFFER].iter().flatten().map(|s| s.count).collect();
        assert_eq!(b, vec![2, 1]);
        for _ in 0..7 {
            e.put(Stack::new("minecraft:cobblestone", 64));
        }
        assert!(!e.fits(&Stack::new("minecraft:dirt", 1)), "9 slots cheios");
        assert!(!e.fits(&raw("{ntm_ore:{light:3d}}")));
        assert!(e.fits(&raw("{ntm_ore:{light:1d}}")));
    }
}
