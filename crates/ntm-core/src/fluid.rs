//! Fluids (docs/fluidos.md): the NTM's fluid types, tanks, the items that
//! carry fluid, and how a network of ducts moves fluid between machines.
//!
//! Sources, relative to `ntm-src/src/main/java/com/hbm/`: the types are
//! `inventory/fluid/Fluids.java:261-417` (extracted by
//! `tools/fluid-gen/extract.py` to `data/fluids.json`, id = order of
//! registration); the network is `api/hbm/fluidmk2/FluidNetMK2.java:29-149`
//! (per tick: providers offer, receivers ask by priority, highest first,
//! shared in proportion); containers are `inventory/FluidContainerRegistry
//! .java:33-91`. Pressure (0..5 PU, `FluidTank.pressure`) is per tank and
//! per network tick: a fluid only moves between tanks at the same pressure
//! (`FluidNetMK2.transferFluid` runs once per pressure level); every tank
//! that is not a compressor's or the oil chain's is at 0.

use crate::Pos;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Clone, Debug, Deserialize)]
pub struct FluidType {
    pub id: u16,
    pub name: String,
    /// `uf6`: the name in saves, recipes and item ids.
    pub string_id: String,
    pub color: u32,
    pub temp: i32,
    #[serde(default)]
    pub gaseous: bool,
    #[serde(default)]
    pub liquid: bool,
    #[serde(default)]
    pub corrosive: u32,
    #[serde(default)]
    pub vent_radiation: f32,
    #[serde(default)]
    pub antimatter: bool,
    #[serde(default)]
    pub no_container: bool,
    #[serde(default)]
    pub lead_container: bool,
    #[serde(default)]
    pub canister: bool,
    #[serde(default)]
    pub gastank: bool,
}

#[derive(Deserialize)]
struct Table {
    fluids: Vec<FluidType>,
}

pub fn types() -> &'static [FluidType] {
    static T: OnceLock<Vec<FluidType>> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str::<Table>(include_str!("../data/fluids.json"))
            .expect("data/fluids.json")
            .fluids
    })
}

/// The type of a fluid by its `string_id` (`uf6`); `none` is no fluid.
pub fn get(name: &str) -> Option<&'static FluidType> {
    types().iter().find(|f| f.string_id == name)
}

/// A pretty name: `UF6`, `Sulfuric acid`.
pub fn label(name: &str) -> String {
    match get(name) {
        Some(f) => {
            let mut s = f.name.replace('_', " ").to_lowercase();
            if let Some(c) = s.get_mut(0..1) {
                c.make_ascii_uppercase();
            }
            if f.name.len() <= 4 {
                f.name.clone()
            } else {
                s
            }
        }
        None => name.to_string(),
    }
}

/// A tank: one fluid type (or none yet) and how much of it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tank {
    /// `string_id`, empty while the tank has no type.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fluid: String,
    pub mb: u32,
    pub cap: u32,
    /// `FluidTank.pressure` (`getShort(name + "_p")` in the NBT): 0 when
    /// the field is missing, which is every save from before pressure.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub pressure: u8,
}

fn is_zero(p: &u8) -> bool {
    *p == 0
}

/// `IFluidUserMK2.HIGHEST_VALID_PRESSURE`: the networks run pressures 0..=5.
pub const HIGHEST_VALID_PRESSURE: u8 = 5;

impl Tank {
    pub fn new(cap: u32) -> Self {
        Self { fluid: String::new(), mb: 0, cap, pressure: 0 }
    }

    pub fn typed(fluid: &str, cap: u32) -> Self {
        Self { fluid: fluid.into(), mb: 0, cap, pressure: 0 }
    }

    /// `FluidTank.withPressure`: a new pressure empties the tank.
    pub fn with_pressure(mut self, pressure: u8) -> Self {
        self.set_pressure(pressure);
        self
    }

    /// `FluidTank.withPressure(pressure)` on a tank in place.
    pub fn set_pressure(&mut self, pressure: u8) {
        if self.pressure != pressure {
            self.mb = 0;
        }
        self.pressure = pressure;
    }

    pub fn free(&self) -> u32 {
        self.cap.saturating_sub(self.mb)
    }

    /// Whether `fluid` may go in: the same type, or an empty untyped tank.
    pub fn accepts(&self, fluid: &str) -> bool {
        self.fluid == fluid || (self.fluid.is_empty() && self.mb == 0)
    }

    /// Puts up to `mb` of `fluid` in; returns what went in.
    pub fn fill(&mut self, fluid: &str, mb: u32) -> u32 {
        if !self.accepts(fluid) {
            return 0;
        }
        let n = mb.min(self.free());
        if n > 0 {
            self.fluid = fluid.into();
            self.mb += n;
        }
        n
    }

    /// Takes up to `mb` out; returns what came out.
    pub fn drain(&mut self, mb: u32) -> u32 {
        let n = mb.min(self.mb);
        self.mb -= n;
        n
    }

    /// Sets the type (a fluid identifier or a recipe): the content goes if
    /// the type changes (`FluidTank.setTankType`).
    pub fn set_type(&mut self, fluid: &str) {
        if self.fluid != fluid {
            self.fluid = fluid.into();
            self.mb = 0;
        }
    }

    /// "UF6: 1.200 / 16.000 mB".
    pub fn line(&self) -> String {
        let f = if self.fluid.is_empty() { "vazio".to_string() } else { label(&self.fluid) };
        format!("{f}: {} / {} mB", self.mb, self.cap)
    }
}

// ------------------------------------------------------------ containers

/// An item that carries fluid: its fluid, how much, and the empty item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Container {
    pub fluid: String,
    pub mb: u32,
    pub empty: String,
}

/// The generic containers (`FluidContainerRegistry.java:69-91`): full id
/// prefix, empty id, mB, and which fluids have one.
/// (full prefix, empty id, mB, which fluids).
type Generic = (&'static str, &'static str, u32, fn(&FluidType) -> bool);

const GENERIC: &[Generic] = &[
    ("ntm:fluid_tank_full__", "ntm:fluid_tank_empty", 1_000, |f| !f.no_container && !f.lead_container),
    ("ntm:fluid_tank_lead_full__", "ntm:fluid_tank_lead_empty", 1_000, |f| !f.no_container),
    ("ntm:fluid_barrel_full__", "ntm:fluid_barrel_empty", 16_000, |f| !f.no_container && !f.lead_container),
    ("ntm:canister_full__", "ntm:canister_empty", 1_000, |f| f.canister),
    ("ntm:gas_full__", "ntm:gas_empty", 1_000, |f| f.gastank),
];

/// Containers of their own (`FluidContainerRegistry.java:33-53`).
const SPECIAL: &[(&str, &str, u32, &str)] = &[
    ("minecraft:water_bucket", "minecraft:bucket", 1_000, "water"),
    ("minecraft:lava_bucket", "minecraft:bucket", 1_000, "lava"),
    ("ntm:cell_uf6", "ntm:cell_empty", 1_000, "uf6"),
    ("ntm:cell_puf6", "ntm:cell_empty", 1_000, "puf6"),
    ("ntm:cell_deuterium", "ntm:cell_empty", 1_000, "deuterium"),
    ("ntm:cell_tritium", "ntm:cell_empty", 1_000, "tritium"),
    ("ntm:cell_sas3", "ntm:cell_empty", 1_000, "sas3"),
];

/// What a full container holds; None for anything else.
pub fn contents(item: &str) -> Option<Container> {
    if let Some(&(_, empty, mb, fluid)) = SPECIAL.iter().find(|s| s.0 == item) {
        return Some(Container { fluid: fluid.into(), mb, empty: empty.into() });
    }
    GENERIC.iter().find_map(|&(prefix, empty, mb, has)| {
        let name = item.strip_prefix(prefix)?;
        let f = get(name).filter(|f| has(f))?;
        Some(Container { fluid: f.string_id.clone(), mb, empty: empty.into() })
    })
}

/// The full container an empty one becomes with `fluid`, and how much it
/// takes; None if that empty item can't hold it.
pub fn fill_container(empty: &str, fluid: &str) -> Option<(String, u32)> {
    if let Some(&(full, _, mb, _)) = SPECIAL.iter().find(|s| s.1 == empty && s.3 == fluid) {
        return Some((full.into(), mb));
    }
    let f = get(fluid)?;
    GENERIC
        .iter()
        .find(|&&(_, e, _, has)| e == empty && has(f))
        .map(|&(prefix, _, mb, _)| (format!("{prefix}{fluid}"), mb))
}

/// Whether `item` is an empty fluid container.
pub fn is_empty_container(item: &str) -> bool {
    GENERIC.iter().any(|g| g.1 == item) || SPECIAL.iter().any(|s| s.1 == item)
}

/// Every full container id (for the agreement test with `ids.json`).
pub fn all_full_containers() -> Vec<String> {
    let mut out: Vec<String> = SPECIAL.iter().map(|s| s.0.to_string()).collect();
    for f in types().iter().filter(|f| f.id != 0) {
        for &(prefix, _, _, has) in GENERIC {
            if has(f) {
                out.push(format!("{prefix}{}", f.string_id));
            }
        }
    }
    out
}

// --------------------------------------------------------------- network

/// `ConnectionPriority`: who is served first.
pub const PRIORITY_LOW: u8 = 1;
pub const PRIORITY_NORMAL: u8 = 2;

/// A machine as a node of a fluid network.
pub trait FluidNode {
    /// mB of `fluid` it can send this tick.
    fn fluid_offer(&self, fluid: &str) -> u64;
    /// mB of `fluid` it can take this tick.
    fn fluid_demand(&self, fluid: &str) -> u64;
    fn fluid_priority(&self) -> u8 {
        PRIORITY_NORMAL
    }
    /// The fluid types it holds or wants (the networks it is part of).
    fn fluid_types(&self) -> Vec<String>;
    /// It takes any fluid now (an empty tank whose type the first fluid
    /// sets): its proxy cells join every network of its dimension.
    fn accepts_any(&self) -> bool {
        false
    }
    /// `mb` of `fluid` arrived.
    fn fluid_give(&mut self, fluid: &str, mb: u64);
    /// `mb` of `fluid` left.
    fn fluid_take(&mut self, fluid: &str, mb: u64);
    /// `getFluidAvailable(type, pressure)`: what it can send at `pressure`.
    /// Machines whose tanks are all at 0 (all but the compressors and the
    /// oil chain's pressurised inputs) answer at 0 only.
    fn fluid_offer_at(&self, fluid: &str, pressure: u8) -> u64 {
        if pressure == 0 { self.fluid_offer(fluid) } else { 0 }
    }
    /// `getDemand(type, pressure)`.
    fn fluid_demand_at(&self, fluid: &str, pressure: u8) -> u64 {
        if pressure == 0 { self.fluid_demand(fluid) } else { 0 }
    }
    /// `transferFluid(type, pressure, amount)`: it arrived at `pressure`.
    fn fluid_give_at(&mut self, fluid: &str, pressure: u8, mb: u64) {
        if pressure == 0 {
            self.fluid_give(fluid, mb);
        }
    }
    /// `useUpFluid(type, pressure, amount)`: it left at `pressure`.
    fn fluid_take_at(&mut self, fluid: &str, pressure: u8, mb: u64) {
        if pressure == 0 {
            self.fluid_take(fluid, mb);
        }
    }
}

/// One tick of a network of `fluid` (`FluidNetMK2.update`): each pressure
/// level, 0 up to `HIGHEST_VALID_PRESSURE`, is its own round (`transferFluid`
/// runs `for p` over providers and receivers of that pressure only), so a
/// fluid never goes between tanks at different pressures.
/// Returns the mB moved.
pub fn distribute(fluid: &str, nodes: &mut [&mut dyn FluidNode]) -> u64 {
    (0..=HIGHEST_VALID_PRESSURE).map(|p| distribute_at(fluid, p, nodes)).sum()
}

/// One pressure level of `distribute`: offers and demands read once;
/// receivers served by priority, highest first, each priority in
/// proportion to demand; providers drained in proportion to their offer.
pub fn distribute_at(fluid: &str, pressure: u8, nodes: &mut [&mut dyn FluidNode]) -> u64 {
    let mut offer: Vec<u64> = nodes.iter().map(|n| n.fluid_offer_at(fluid, pressure)).collect();
    let mut total_offer: u64 = offer.iter().sum();
    if total_offer == 0 {
        return 0;
    }
    let demand: Vec<u64> = nodes.iter().map(|n| n.fluid_demand_at(fluid, pressure)).collect();
    let mut moved = 0;
    let mut prios: Vec<u8> = (0..nodes.len()).filter(|&i| demand[i] > 0).map(|i| nodes[i].fluid_priority()).collect();
    prios.sort_unstable_by(|a, b| b.cmp(a));
    prios.dedup();
    for prio in prios {
        if total_offer == 0 {
            break;
        }
        let to: Vec<usize> = (0..nodes.len()).filter(|&i| demand[i] > 0 && nodes[i].fluid_priority() == prio).collect();
        // What each receiver can get from the others (not from itself).
        let want: u64 = to.iter().map(|&i| demand[i]).sum();
        let amount = want.min(total_offer);
        if amount == 0 {
            continue;
        }
        let mut given = 0;
        for (i, part) in split(&demand, &to, want, amount) {
            nodes[i].fluid_give_at(fluid, pressure, part);
            given += part;
        }
        let from: Vec<usize> = (0..nodes.len()).filter(|&i| offer[i] > 0).collect();
        for (i, part) in split(&offer, &from, total_offer, given) {
            offer[i] -= part;
            nodes[i].fluid_take_at(fluid, pressure, part);
        }
        total_offer -= given;
        moved += given;
    }
    moved
}

/// `amount` over `who` by weight, the rounding rest to the first with room.
fn split(weights: &[u64], who: &[usize], total: u64, amount: u64) -> Vec<(usize, u64)> {
    let mut parts: Vec<(usize, u64, u64)> = who
        .iter()
        .map(|&i| (i, (u128::from(amount) * u128::from(weights[i]) / u128::from(total.max(1))) as u64, weights[i]))
        .collect();
    let mut rest = amount - parts.iter().map(|p| p.1).sum::<u64>();
    for p in &mut parts {
        let extra = (p.2 - p.1).min(rest);
        p.1 += extra;
        rest -= extra;
    }
    parts.into_iter().filter(|p| p.1 > 0).map(|(i, n, _)| (i, n)).collect()
}

/// The fluid networks of a world: per dimension and fluid type, which
/// machines share ducts of that type (or touch each other).
pub type Networks = BTreeMap<(String, String), Vec<Vec<Pos>>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_has_the_ntm_ids() {
        assert_eq!(types().len(), 157);
        assert_eq!(get("water").unwrap().id, 1);
        assert_eq!(get("uf6").unwrap().id, 29);
        assert_eq!(get("air").unwrap().id, 151);
        assert!(get("uf6").unwrap().gaseous);
        assert_eq!(get("sulfuric_acid").unwrap().corrosive, 50);
        assert_eq!(label("uf6"), "UF6");
        assert_eq!(label("sulfuric_acid"), "Sulfuric acid");
    }

    #[test]
    fn containers_map_both_ways() {
        let c = contents("ntm:fluid_tank_full__water").unwrap();
        assert_eq!((c.fluid.as_str(), c.mb, c.empty.as_str()), ("water", 1000, "ntm:fluid_tank_empty"));
        assert_eq!(contents("ntm:fluid_barrel_full__uf6").unwrap().mb, 16_000);
        assert_eq!(contents("ntm:cell_uf6").unwrap().fluid, "uf6");
        assert_eq!(contents("minecraft:water_bucket").unwrap().empty, "minecraft:bucket");
        assert!(contents("ntm:fluid_tank_full__nonsense").is_none());
        assert!(contents("ntm:canister_full__water").is_none(), "água não tem lata");
        assert_eq!(fill_container("ntm:fluid_tank_empty", "uf6"), Some(("ntm:fluid_tank_full__uf6".into(), 1000)));
        assert_eq!(fill_container("ntm:cell_empty", "uf6"), Some(("ntm:cell_uf6".into(), 1000)));
        assert_eq!(fill_container("minecraft:bucket", "water"), Some(("minecraft:water_bucket".into(), 1000)));
        assert!(fill_container("ntm:gas_empty", "water").is_none());
        assert!(is_empty_container("ntm:fluid_barrel_empty"));
    }

    /// The containers here against the content pack's (`ids.json` entries
    /// with `fluid`), both ways, when the pack is there.
    #[test]
    fn containers_agree_with_content_ids() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content/ids.json");
        let Ok(text) = std::fs::read_to_string(path) else {
            eprintln!("content/ids.json missing: skipped");
            return;
        };
        let ids: serde_json::Value = serde_json::from_str(&text).unwrap();
        let mut theirs = std::collections::BTreeSet::new();
        for e in ids["entries"].as_array().unwrap() {
            let Some(f) = e.get("fluid").filter(|f| f.is_object()) else { continue };
            let id = e["id"].as_str().unwrap();
            // Fluid packs only come and go through assembly recipes
            // (`AssemblyMachineRecipes.java:1114-1121`), never poured.
            if id.starts_with("ntm:fluid_pack_full__") {
                continue;
            }
            theirs.insert(id.to_string());
            let c = contents(id).unwrap_or_else(|| panic!("{id} no content, não aqui"));
            assert_eq!(c.fluid, f["id"].as_str().unwrap(), "{id}");
            assert_eq!(u64::from(c.mb), f["amount_mb"].as_u64().unwrap(), "{id}");
            assert_eq!(c.empty, f["empty"].as_str().unwrap(), "{id}");
        }
        let entries: std::collections::BTreeSet<&str> = ids["entries"].as_array().unwrap().iter().filter_map(|e| e["id"].as_str()).collect();
        for c in all_full_containers() {
            if c.starts_with("minecraft:") {
                continue;
            }
            assert!(entries.contains(c.as_str()), "{c} aqui, não no content");
        }
    }

    #[test]
    fn tanks_keep_one_type() {
        let mut t = Tank::new(1000);
        assert_eq!(t.fill("water", 600), 600);
        assert_eq!(t.fill("uf6", 100), 0);
        assert_eq!(t.fill("water", 600), 400);
        assert_eq!(t.drain(300), 300);
        t.set_type("uf6");
        assert_eq!(t.mb, 0);
    }

    struct N {
        tank: Tank,
        send: bool,
        prio: u8,
    }
    impl FluidNode for N {
        fn fluid_offer(&self, f: &str) -> u64 {
            if self.send && self.tank.fluid == f { u64::from(self.tank.mb) } else { 0 }
        }
        fn fluid_demand(&self, f: &str) -> u64 {
            if !self.send && self.tank.accepts(f) { u64::from(self.tank.free()) } else { 0 }
        }
        fn fluid_priority(&self) -> u8 {
            self.prio
        }
        fn fluid_types(&self) -> Vec<String> {
            vec![self.tank.fluid.clone()]
        }
        fn fluid_give(&mut self, f: &str, mb: u64) {
            self.tank.fill(f, mb as u32);
        }
        fn fluid_take(&mut self, _f: &str, mb: u64) {
            self.tank.drain(mb as u32);
        }
    }

    #[test]
    fn higher_priority_first_then_in_proportion() {
        let src = |mb| N { tank: Tank { fluid: "water".into(), mb, cap: 10_000, pressure: 0 }, send: true, prio: 2 };
        let dst = |cap, prio| N { tank: Tank::new(cap), send: false, prio };
        let (mut a, mut b) = (src(3000), src(1000));
        let (mut hi, mut lo1, mut lo2) = (dst(1000, 3), dst(3000, 1), dst(1000, 1));
        let moved = {
            let mut nodes: Vec<&mut dyn FluidNode> = vec![&mut a, &mut b, &mut hi, &mut lo1, &mut lo2];
            distribute("water", &mut nodes)
        };
        assert_eq!(moved, 4000);
        assert_eq!(hi.tank.mb, 1000, "a prioridade alta enche primeiro");
        assert_eq!((lo1.tank.mb, lo2.tank.mb), (2250, 750), "o resto em proporção");
        assert_eq!(a.tank.mb + b.tank.mb, 0);
        // Nothing of another fluid moves.
        let mut c = src(500);
        let mut d = dst(1000, 2);
        let mut nodes: Vec<&mut dyn FluidNode> = vec![&mut c, &mut d];
        assert_eq!(distribute("uf6", &mut nodes), 0);
    }

    /// A node whose one tank has a pressure (`getFluidAvailable`,
    /// `getDemand` and `transferFluid` all compare `tank.getPressure()`).
    struct P {
        tank: Tank,
        send: bool,
    }
    impl FluidNode for P {
        fn fluid_offer(&self, f: &str) -> u64 {
            self.fluid_offer_at(f, 0)
        }
        fn fluid_demand(&self, f: &str) -> u64 {
            self.fluid_demand_at(f, 0)
        }
        fn fluid_types(&self) -> Vec<String> {
            vec![self.tank.fluid.clone()]
        }
        fn fluid_give(&mut self, f: &str, mb: u64) {
            self.fluid_give_at(f, 0, mb);
        }
        fn fluid_take(&mut self, f: &str, mb: u64) {
            self.fluid_take_at(f, 0, mb);
        }
        fn fluid_offer_at(&self, f: &str, p: u8) -> u64 {
            if self.send && self.tank.fluid == f && self.tank.pressure == p { u64::from(self.tank.mb) } else { 0 }
        }
        fn fluid_demand_at(&self, f: &str, p: u8) -> u64 {
            if !self.send && self.tank.accepts(f) && self.tank.pressure == p { u64::from(self.tank.free()) } else { 0 }
        }
        fn fluid_give_at(&mut self, f: &str, p: u8, mb: u64) {
            if self.tank.pressure == p {
                self.tank.fill(f, mb as u32);
            }
        }
        fn fluid_take_at(&mut self, _f: &str, p: u8, mb: u64) {
            if self.tank.pressure == p {
                self.tank.drain(mb as u32);
            }
        }
    }

    fn pn(send: bool, mb: u32, pressure: u8) -> P {
        let mut tank = Tank::new(10_000).with_pressure(pressure);
        tank.fluid = "hydrogen".into();
        tank.mb = mb;
        P { tank, send }
    }

    fn run(nodes: &mut [&mut dyn FluidNode]) -> u64 {
        distribute("hydrogen", nodes)
    }

    #[test]
    fn a_tank_at_another_pressure_gets_nothing() {
        for (src, dst) in [(1, 0), (0, 1), (2, 1), (5, 0), (0, 5)] {
            let (mut a, mut b) = (pn(true, 4_000, src), pn(false, 0, dst));
            assert_eq!(run(&mut [&mut a, &mut b]), 0, "{src} -> {dst}");
            assert_eq!((a.tank.mb, b.tank.mb), (4_000, 0), "{src} -> {dst}");
        }
    }

    #[test]
    fn only_equal_pressures_trade_and_each_level_is_its_own_round() {
        let mut src0 = pn(true, 1_000, 0);
        let mut src1 = pn(true, 3_000, 1);
        let mut dst0 = pn(false, 0, 0);
        let mut dst1 = pn(false, 0, 1);
        let mut dst2 = pn(false, 0, 2);
        let moved = run(&mut [&mut src0, &mut src1, &mut dst0, &mut dst1, &mut dst2]);
        assert_eq!(moved, 4_000);
        assert_eq!((src0.tank.mb, src1.tank.mb), (0, 0));
        assert_eq!((dst0.tank.mb, dst1.tank.mb, dst2.tank.mb), (1_000, 3_000, 0));
    }

    #[test]
    fn a_pressure_above_the_highest_is_never_served() {
        let (mut a, mut b) = (pn(true, 500, 6), pn(false, 0, 6));
        assert_eq!(run(&mut [&mut a, &mut b]), 0);
        assert_eq!(HIGHEST_VALID_PRESSURE, 5);
    }

    #[test]
    fn nodes_that_know_no_pressure_trade_at_zero_only() {
        let src = |mb| N { tank: Tank { fluid: "water".into(), mb, cap: 10_000, pressure: 0 }, send: true, prio: 2 };
        let mut a = src(1_000);
        let mut b = N { tank: Tank::new(10_000), send: false, prio: 2 };
        let mut nodes: Vec<&mut dyn FluidNode> = vec![&mut a, &mut b];
        assert_eq!(distribute("water", &mut nodes), 1_000);
        assert_eq!(distribute_at("water", 1, &mut nodes), 0);
        // The defaults answer at 0 only, whatever they hold.
        let full = N { tank: Tank { fluid: "water".into(), mb: 500, cap: 1_000, pressure: 0 }, send: true, prio: 2 };
        let empty = N { tank: Tank::new(1_000), send: false, prio: 2 };
        for p in 1..=HIGHEST_VALID_PRESSURE {
            assert_eq!((full.fluid_offer_at("water", p), empty.fluid_demand_at("water", p)), (0, 0), "@{p}");
        }
        assert_eq!((full.fluid_offer_at("water", 0), empty.fluid_demand_at("water", 0)), (500, 1_000));
    }

    #[test]
    fn a_new_pressure_empties_the_tank_and_the_same_one_keeps_it() {
        let mut t = Tank::typed("hydrogen", 1_000);
        t.fill("hydrogen", 600);
        t.set_pressure(0);
        assert_eq!(t.mb, 600);
        t.set_pressure(1);
        assert_eq!((t.mb, t.pressure), (0, 1));
        t.fill("hydrogen", 100);
        t.set_pressure(1);
        assert_eq!(t.mb, 100);
        assert_eq!(Tank::new(10).with_pressure(3).pressure, 3);
    }

    /// A save from before pressure has no field: the tank is at 0; a
    /// pressure-0 tank is written without it, so old readers still open it.
    #[test]
    fn a_tank_without_a_pressure_field_is_at_zero() {
        let old: Tank = serde_json::from_str(r#"{"fluid":"water","mb":5,"cap":10}"#).unwrap();
        assert_eq!(old.pressure, 0);
        assert!(!serde_json::to_string(&old).unwrap().contains("pressure"));
        let p = Tank::typed("hydrogen", 10).with_pressure(2);
        let text = serde_json::to_string(&p).unwrap();
        assert!(text.contains(r#""pressure":2"#), "{text}");
        assert_eq!(serde_json::from_str::<Tank>(&text).unwrap(), p);
    }
}
