//! ZIRNOX's sequential rod reactions and Java float thermodynamics. The
//! exported reactor snapshot is the source of fuel, adjacency and tank data.
use crate::{
    Pos,
    fluid::{FluidNode, Tank},
    machine::Logic,
    server::container_bridge::{Slots, Slotted, Stack},
    view::ViewModel,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock};

pub const BLOCK: &str = "ntm:machine_zirnox";
pub const DESTROYED: &str = "ntm:zirnox_destroyed";
pub const MAX: i32 = 100_000;

/// World-owned remnants outlive the reactor block, like Java entities.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Remnants {
    pub debris: Vec<Debris>,
    pub blasts: Vec<(String, [f64; 3])>,
    pub waste: Vec<Waste>,
    /// Expiring radiation potion from fuel/graphite debris, by player UUID.
    pub irradiated: BTreeMap<String, (u32, u8)>,
}
impl Remnants {
    pub fn irradiate(&mut self, id: &str, duration: u32, amplifier: u8) {
        self.irradiated
            .entry(id.into())
            .and_modify(|(n, a)| {
                if amplifier > *a {
                    *a = amplifier;
                    *n = duration;
                } else if amplifier == *a {
                    *n = (*n).max(duration);
                }
            })
            .or_insert((duration, amplifier));
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Waste {
    pub dim: String,
    pub center: Pos,
    pub cursor: u32,
    pub seed: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Debris {
    pub dim: String,
    pub pos: [f64; 3],
    pub velocity: [f64; 3],
    pub kind: u8,
    pub age: u32,
    pub lifetime: u32,
}
impl Debris {
    pub fn spawn(dim: &str, at: Pos, seed: u64) -> Vec<Self> {
        let mut random = seed;
        let mut unit = || {
            random = crate::hash::mix(random);
            (random >> 11) as f64 / (1u64 << 53) as f64
        };
        // Box-Muller: the original uses Gaussian horizontal velocities.
        let kinds = [(5, 2), (4, 20), (0, 20), (1, 10), (3, 10), (2, 10)];
        let mut out = Vec::new();
        for (kind, n) in kinds {
            for _ in 0..n {
                let radius = (-2. * unit().max(f64::MIN_POSITIVE).ln()).sqrt() * 0.75;
                let angle = std::f64::consts::TAU * unit();
                let mut v = [
                    radius * angle.cos(),
                    0.01 + unit() * 1.25,
                    radius * angle.sin(),
                ];
                if kind == 4 {
                    v[0] *= 0.25;
                    v[1] += unit();
                    v[2] *= 0.25;
                }
                if kind == 5 {
                    v[0] += 0.5;
                    v[1] *= 0.1;
                    v[2] += 0.5;
                }
                let lifetime = match kind {
                    0 => 3600,
                    1 => 12000,
                    2 | 3 => 18000,
                    _ => 1200,
                } + out.len() as u32 % 50;
                out.push(Self {
                    dim: dim.into(),
                    pos: [
                        f64::from(at[0]) + 0.5,
                        f64::from(at[1]) + 4.,
                        f64::from(at[2]) + 0.5,
                    ],
                    velocity: v,
                    kind,
                    age: 0,
                    lifetime,
                });
            }
        }
        out
    }
}

#[derive(Debug, Deserialize)]
pub struct Fuel {
    pub item: String,
    pub output: String,
    pub max_life: i32,
    pub heat_per_decay: i32,
    pub breeding: bool,
}
#[derive(Deserialize)]
struct TankSpec {
    capacity_mb: u32,
}
#[derive(Deserialize)]
pub struct Table {
    pub fuels: Vec<Fuel>,
    pub neighbors: BTreeMap<String, Vec<usize>>,
    tanks: Vec<TankSpec>,
}
pub fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        #[derive(Deserialize)]
        struct Snapshot {
            zirnox: Table,
        }
        serde_json::from_str::<Snapshot>(include_str!("../data/reactors_simple.json"))
            .expect("reactor snapshot")
            .zirnox
    })
}
pub fn fuel(id: &str) -> Option<&'static Fuel> {
    table().fuels.iter().find(|f| f.item == id)
}
/// Java canExtractItem: the same rod slots become extractable after conversion.
/// Prepared for a future conditional native hopper-extraction hook.
pub fn can_extract(slot: usize, item: &str) -> bool {
    slot < 24 && fuel(item).is_none()
}
pub fn life(stack: &Stack) -> i32 {
    stack
        .custom
        .as_deref()
        .and_then(crate::snbt::fields)
        .and_then(|m| {
            m.get("life").and_then(|v| {
                v.trim_end_matches(['b', 's', 'l', 'f', 'd', 'B', 'S', 'L', 'F', 'D'])
                    .parse::<f64>()
                    .ok()
            })
        })
        .filter(|n| n.is_finite())
        .unwrap_or(0.)
        .clamp(0., f64::from(i32::MAX)) as i32
}

fn slots() -> Slots {
    vec![None; 28]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Zirnox {
    pub heat: i32,
    pub pressure: i32,
    pub on: bool,
    pub powered: bool,
    pub steam: Tank,
    pub co2: Tank,
    pub water: Tank,
    #[serde(skip)]
    pub output: u32,
    #[serde(skip, default = "slots")]
    pub slots: Slots,
    /// Destroyed reactors remain radioactive after their fire goes out.
    #[serde(default)]
    pub destroyed: bool,
    #[serde(default)]
    pub on_fire: bool,
    #[serde(default)]
    pub gas_emissions: u64,
}
impl Default for Zirnox {
    fn default() -> Self {
        Self {
            heat: 0,
            pressure: 0,
            on: false,
            powered: false,
            steam: Tank::typed("superhotsteam", table().tanks[0].capacity_mb),
            co2: Tank::typed("carbondioxide", table().tanks[1].capacity_mb),
            water: Tank::typed("water", table().tanks[2].capacity_mb),
            output: 0,
            slots: slots(),
            destroyed: false,
            on_fire: false,
            gas_emissions: 0,
        }
    }
}
impl Zirnox {
    pub fn of(block: &str) -> Self {
        Self {
            destroyed: block == DESTROYED,
            on_fire: block == DESTROYED,
            ..Self::default()
        }
    }
    /// Saves may contain unbounded tank fills/capacities or negative heat.
    pub fn normalize(&mut self) {
        self.heat = self.heat.max(-9);
        self.pressure = self.pressure.max(0);
        for (i, t) in [&mut self.steam, &mut self.co2, &mut self.water]
            .into_iter()
            .enumerate()
        {
            t.cap = table().tanks[i].capacity_mb;
            t.mb = t.mb.min(t.cap);
            t.fluid = ["superhotsteam", "carbondioxide", "water"][i].into();
        }
        self.slots.resize(28, None);
    }
    pub fn set_powered(&mut self, powered: bool) {
        if self.powered && !powered {
            self.on = false;
        }
        self.powered = powered;
        if powered {
            self.on = true;
        }
    }
    /// Returns failure after cooling. CO₂ is retained, except when vented.
    pub fn step(&mut self) -> bool {
        self.normalize();
        self.output = 0;
        if self.destroyed {
            return false;
        }
        if self.powered {
            self.on = true;
        }
        crate::combustion::load_tank(&mut self.co2, &mut self.slots, 24, 26);
        crate::combustion::load_tank(&mut self.water, &mut self.slots, 25, 27);
        if self.on {
            for slot in 0..24 {
                let Some(s) = self.slots[slot].as_ref() else {
                    continue;
                };
                if s.id == "ntm:meteorite_sword_bred" {
                    self.slots[slot] = Some(Stack::new("ntm:meteorite_sword_irradiated", 1));
                    continue;
                }
                let Some(f) = fuel(&s.id) else { continue };
                let decays = table().neighbors[&slot.to_string()]
                    .iter()
                    .filter(|&&n| {
                        self.slots[n]
                            .as_ref()
                            .and_then(|s| fuel(&s.id))
                            .is_some_and(|f| !f.breeding)
                    })
                    .count()
                    + usize::from(!f.breeding);
                for _ in 0..decays {
                    self.heat = self.heat.saturating_add(f.heat_per_decay);
                    let s = self.slots[slot].as_mut().expect("rod");
                    let n = life(s).saturating_add(1);
                    if n > f.max_life {
                        self.slots[slot] = Some(Stack::new(&f.output, 1));
                        break;
                    }
                    s.custom = Some(crate::snbt::with_field(
                        s.custom.as_deref(),
                        "life",
                        &n.to_string(),
                    ));
                }
            }
        }
        // f32 and operation order are intentional: these are Java floats.
        self.pressure = (self.co2.mb as i32)
            .saturating_mul(2)
            .saturating_add((self.heat as f32 * (self.co2.mb as f32 / self.co2.cap as f32)) as i32);
        if self.heat > 0 && self.heat < MAX {
            if self.water.mb > 0 && self.co2.mb > 0 && self.steam.free() > 0 {
                if self.heat > 10256 {
                    self.output = (((self.heat as f32 - 10256.) / 100000.)
                        * (self.co2.mb as f32 / 14000.).min(1.)
                        * 25.
                        * 7.5) as u32;
                    // Source deliberately does not cap cycle to remaining water
                    // or steam room. The tank setters clamp separately.
                    self.water.drain(self.output);
                    self.steam.fill("superhotsteam", self.output);
                }
                self.heat -= (self.heat as f32 * self.pressure as f32 / 1000000.) as i32;
            } else {
                self.heat -= 10;
            }
        }
        self.pressure > MAX || self.heat > MAX
    }
    pub fn port_cells(core: Pos, facing: Option<&str>) -> Vec<(Pos, Pos)> {
        let f = facing
            .and_then(crate::registry::facing_dir)
            .unwrap_or([0, 0, -1]);
        let r = [-f[2], 0, f[0]];
        [-1, 1]
            .into_iter()
            .flat_map(|sign| {
                [1, 3].map(|y| {
                    (
                        [
                            core[0] + r[0] * 2 * sign,
                            core[1] + y,
                            core[2] + r[2] * 2 * sign,
                        ],
                        [r[0] * sign, 0, r[2] * sign],
                    )
                })
            })
            .collect()
    }
    /// Surface cells in Java's 5x5x5 redstone scan that belong to the reactor.
    pub fn redstone_cells(core: Pos, facing: Option<&str>) -> Vec<Pos> {
        let mut out = Vec::new();
        for y in 0..=4 {
            for x in -2_i32..=2 {
                for z in -2_i32..=2 {
                    let inside =
                        y <= 1 || (x.abs() <= 1 && z.abs() <= 1) || (z == 0 && x.abs() == 2);
                    let surface = x.abs() == 2 || z.abs() == 2 || y == 0 || y == 4;
                    if inside && surface {
                        out.push(crate::server::container_bridge::world_cell(
                            core,
                            facing
                                .and_then(crate::registry::facing_dir)
                                .unwrap_or([0, 0, -1]),
                            [x, y, z],
                        ));
                    }
                }
            }
        }
        out
    }
}
impl Logic for Zirnox {
    fn level(&self, _: u8) -> u8 {
        0
    }
    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        (!self.destroyed).then_some(self)
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("ZIRNOX")
    }
}
impl FluidNode for Zirnox {
    fn fluid_types(&self) -> Vec<String> {
        vec![
            "water".into(),
            "carbondioxide".into(),
            "superhotsteam".into(),
        ]
    }
    fn fluid_offer(&self, f: &str) -> u64 {
        if f == "superhotsteam" {
            u64::from(self.steam.mb.min(8000))
        } else {
            0
        }
    }
    fn fluid_demand(&self, f: &str) -> u64 {
        match f {
            "water" => u64::from(32000u32.saturating_sub(self.water.mb)),
            "carbondioxide" => u64::from(16000u32.saturating_sub(self.co2.mb)),
            _ => 0,
        }
    }
    fn fluid_give(&mut self, f: &str, n: u64) {
        self.normalize();
        let t = match f {
            "water" => &mut self.water,
            "carbondioxide" => &mut self.co2,
            _ => return,
        };
        t.fill(f, n.min(u64::from(t.free())) as u32);
    }
    fn fluid_take(&mut self, f: &str, n: u64) {
        if f == "superhotsteam" {
            self.steam.drain(n.min(u64::from(self.steam.mb)) as u32);
        }
    }
}
impl Slotted for Zirnox {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn sync_in(&mut self) {
        self.normalize();
    }
    fn screen_data(&self) -> Vec<i32> {
        let mut v = vec![self.heat, self.pressure, i32::from(self.on)];
        for t in [&self.steam, &self.co2, &self.water] {
            v.extend([
                t.mb as i32,
                t.cap as i32,
                crate::fluid::get(&t.fluid).map_or(0, |f| f.color as i32),
            ]);
        }
        v.extend([
            i32::from(self.powered),
            (f64::from(self.heat) * 0.00001 * 780. + 20.).round() as i32,
            (f64::from(self.pressure) * 0.00001 * 30.).round() as i32,
        ]);
        v
    }
    fn button(&mut self, id: u32) -> bool {
        match id {
            0 if !self.powered => self.on = !self.on,
            1 => {
                self.co2.drain(1000);
            }
            _ => {}
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rod(id: usize, n: i32) -> Stack {
        let mut s = Stack::new(&table().fuels[id].item, 1);
        s.custom = Some(format!("{{life:{n},name:'kept',nested:{{life:99}}}}"));
        s
    }
    #[test]
    fn all_eleven_fuels_use_strict_completion_and_clean_products() {
        for i in 0..11 {
            let f = &table().fuels[i];
            let mut z = Zirnox {
                on: true,
                ..Default::default()
            };
            z.slots[0] = Some(rod(i, f.max_life - 1));
            if f.breeding {
                z.slots[7] = Some(rod(0, 0));
            }
            assert!(!z.step());
            assert_eq!(life(z.slots[0].as_ref().unwrap()), f.max_life);
            assert_eq!(z.slots[0].as_ref().unwrap().id, f.item);
            assert!(
                z.slots[0]
                    .as_ref()
                    .unwrap()
                    .custom
                    .as_ref()
                    .unwrap()
                    .contains("nested")
            );
            z.step();
            assert_eq!(z.slots[0], Some(Stack::new(&f.output, 1)));
        }
    }
    #[test]
    fn breeding_waits_for_active_neighbors_and_conversions_affect_later_slots() {
        let mut z = Zirnox {
            on: true,
            ..Default::default()
        };
        z.slots[0] = Some(rod(2, 20000));
        z.slots[1] = Some(rod(9, 0));
        z.step();
        assert_eq!(life(z.slots[1].as_ref().unwrap()), 0);
        z.slots[7] = Some(rod(0, 0));
        z.step();
        assert_eq!(z.slots[0].as_ref().unwrap().id, table().fuels[3].item);
        assert_eq!(
            life(z.slots[1].as_ref().unwrap()),
            1,
            "converted thorium is active immediately"
        );
        assert_eq!(life(z.slots[7].as_ref().unwrap()), 2);
        assert_eq!(z.heat, 50, "two NU decays, then passive -10");
    }
    #[test]
    fn adjacency_counts_each_active_neighbor_plus_self() {
        let mut z = Zirnox {
            on: true,
            ..Default::default()
        };
        for i in [1, 7, 8, 9, 15] {
            z.slots[i] = Some(rod(1, 0));
        }
        z.step();
        assert_eq!(life(z.slots[8].as_ref().unwrap()), 5);
        assert_eq!(life(z.slots[1].as_ref().unwrap()), 2);
        assert_eq!(z.heat, 640, "13 decays and passive cooling");
    }
    #[test]
    fn steam_uses_java_float_order_and_independently_clamps_tanks() {
        let mut z = Zirnox {
            heat: 50000,
            ..Default::default()
        };
        z.co2.mb = 14000;
        z.water.mb = 1;
        z.steam.mb = 7999;
        assert!(!z.step());
        assert_eq!(z.pressure, 71750);
        assert_eq!(z.output, 74);
        assert_eq!((z.water.mb, z.steam.mb, z.co2.mb), (0, 8000, 14000));
        assert_eq!(z.heat, 46413);
    }
    #[test]
    fn cooling_is_conditional_and_failure_comparisons_are_strict() {
        let mut z = Zirnox {
            heat: 1,
            ..Default::default()
        };
        assert!(!z.step());
        assert_eq!(z.heat, -9, "source may cross zero on cooling");
        z.heat = MAX;
        assert!(!z.step());
        assert_eq!(z.heat, MAX);
        z.heat = MAX + 1;
        assert!(z.step());
        z.heat = 68000;
        z.co2.mb = 16000;
        z.water.mb = 100;
        assert!(!z.step());
        assert_eq!(z.pressure, MAX);
        z.heat = 68001;
        assert!(z.step());
        assert!(z.heat < 68001, "pressure was captured before cooling");
        z.heat = 10500;
        z.co2.mb = 14000;
        z.water.mb = 100;
        z.steam.mb = 0;
        z.step();
        assert_eq!(z.output, 0);
        assert!(z.heat < 10500);
    }
    #[test]
    fn redstone_latches_overrides_button_and_falling_edge_stops() {
        let mut z = Zirnox::default();
        z.button(0);
        assert!(z.on);
        z.set_powered(true);
        z.button(0);
        assert!(z.on);
        z.set_powered(false);
        assert!(!z.on);
        z.co2.mb = 400;
        z.button(1);
        assert_eq!(z.co2.mb, 0);
    }
    #[test]
    fn containers_wait_for_room_and_inputs_and_outputs_keep_source_indices() {
        let mut z = Zirnox::default();
        z.slots[24] = Some(Stack::new("ntm:fluid_barrel_full__carbondioxide", 1));
        z.slots[25] = Some(Stack::new("minecraft:water_bucket", 2));
        z.step();
        assert_eq!((z.co2.mb, z.water.mb), (16000, 1000));
        assert_eq!(z.slots[26], Some(Stack::new("ntm:fluid_barrel_empty", 1)));
        assert_eq!(z.slots[27], Some(Stack::new("minecraft:bucket", 1)));
        z.slots[27] = Some(Stack::new("minecraft:bucket", 64));
        z.step();
        assert_eq!(z.water.mb, 1000);
        assert_eq!(z.slots[25].as_ref().unwrap().count, 1);
    }
    #[test]
    fn malformed_save_values_do_not_overflow_or_change_fluid_capacities() {
        let mut z = Zirnox {
            on: true,
            heat: i32::MAX,
            ..Default::default()
        };
        z.co2.mb = u32::MAX;
        z.co2.cap = u32::MAX;
        z.water.mb = u32::MAX;
        z.slots[0] = Some(rod(0, i32::MAX));
        assert!(z.step());
        assert_eq!(z.co2.mb, 16000);
        assert_eq!(z.water.cap, 32000);
        assert_eq!(z.slots[0].as_ref().unwrap().id, table().fuels[0].output);
        assert!(z.pressure > MAX);
        let back: Zirnox = serde_json::from_str(&serde_json::to_string(&z).unwrap()).unwrap();
        assert_eq!(
            (back.heat, back.on, back.powered, back.co2),
            (z.heat, z.on, z.powered, z.co2)
        );
    }
    #[test]
    fn four_ports_rotate_and_surface_scan_uses_only_own_blocks() {
        for f in ["north", "east", "south", "west"] {
            let ports = Zirnox::port_cells([0, 64, 0], Some(f));
            assert_eq!(ports.len(), 4);
            for (p, d) in ports {
                assert_eq!(p[0], d[0] * 2);
                assert_eq!(p[2], d[2] * 2);
                assert!([65, 67].contains(&p[1]));
            }
            let scan = Zirnox::redstone_cells([0, 64, 0], Some(f));
            assert!(scan.contains(&[0, 64, 0]));
            assert!(!scan.contains(&[3, 64, 0]));
            assert!(!scan.contains(&[2, 68, 2]));
        }
    }
    #[test]
    fn debris_has_exact_counts_lifetimes_and_gaussian_launch_velocity() {
        let d = Debris::spawn("d", [1, 64, 1], 42);
        assert_eq!(d.len(), 72);
        assert_eq!(
            (0..6)
                .map(|k| d.iter().filter(|d| d.kind == k).count())
                .collect::<Vec<_>>(),
            vec![20, 10, 10, 10, 20, 2]
        );
        assert!(
            d.iter()
                .all(|d| d.pos == [1.5, 68., 1.5] && d.lifetime >= 1200 && d.lifetime < 18050)
        );
        let a: Remnants = Remnants {
            debris: d,
            ..Default::default()
        };
        assert_eq!(
            serde_json::from_str::<Remnants>(&serde_json::to_string(&a).unwrap()).unwrap(),
            a
        );
    }
}
