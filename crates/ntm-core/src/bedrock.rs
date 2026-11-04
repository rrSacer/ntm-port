//! The bedrock ore chain, stage 1 (docs/maquinas.md, "Bedrock"): the
//! deposits' ore densities (`items/special/ItemBedrockOreBase.java`), their
//! tier and drilling fluid (`world/feature/BedrockOre.java`), the raw ore's
//! densities as item data, and the ore slopper
//! (`tileentity/machine/TileEntityMachineOreSlopper.java`).
//!
//! A deposit (`ntm:ore_bedrock`) holds nothing: what it gives is a function
//! of where it is, as in the NTM, whose densities use a fixed seed (not the
//! world's).

/// The 6 ore types, in `BedrockOreType`'s order, by their `suffix`.
pub const TYPES: [&str; 6] = ["light", "heavy", "rare", "actinide", "nonmetal", "crystal"];

/// `java.util.Random`, for the noise's seeds.
pub struct JavaRandom(u64);

impl JavaRandom {
    const MUL: u64 = 0x5_DEEC_E66D;
    const MASK: u64 = (1 << 48) - 1;

    pub fn new(seed: i64) -> Self {
        Self((seed as u64 ^ Self::MUL) & Self::MASK)
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.0 = (self.0.wrapping_mul(Self::MUL).wrapping_add(0xB)) & Self::MASK;
        (self.0 >> (48 - bits)) as i64 as i32
    }

    pub fn next_int(&mut self, bound: i32) -> i32 {
        if bound.isolate_lowest_one() == bound {
            return ((i64::from(bound) * i64::from(self.next(31))) >> 31) as i32;
        }
        loop {
            let bits = self.next(31);
            let val = bits % bound;
            if bits.wrapping_sub(val).wrapping_add(bound - 1) >= 0 {
                return val;
            }
        }
    }

    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 / (1u32 << 24) as f32
    }

    pub fn next_double(&mut self) -> f64 {
        let hi = i64::from(self.next(26)) << 27;
        (hi + i64::from(self.next(27))) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

/// `NoiseGeneratorSimplex` (1.7.10), its 2D noise: a reconstruction (the
/// vanilla source isn't in the repo). It agrees with Horizon's
/// `SimplexNoise::get2` (gradients, permutation, radius 0.5, ×70) but for
/// the octave offsets, which 1.7.10's `NoiseGeneratorPerlin` doesn't add
/// (the modern `PerlinSimplexNoise` does); parity with the NTM's densities
/// isn't verified.
struct Simplex {
    p: [usize; 512],
}

const GRAD3: [[f64; 2]; 12] = [
    [1.0, 1.0],
    [-1.0, 1.0],
    [1.0, -1.0],
    [-1.0, -1.0],
    [1.0, 0.0],
    [-1.0, 0.0],
    [1.0, 0.0],
    [-1.0, 0.0],
    [0.0, 1.0],
    [0.0, -1.0],
    [0.0, 1.0],
    [0.0, -1.0],
];

fn fast_floor(x: f64) -> i32 {
    if x > 0.0 { x as i32 } else { x as i32 - 1 }
}

impl Simplex {
    fn new(r: &mut JavaRandom) -> Self {
        // The offsets (xo, yo, zo) are drawn but the 2D noise doesn't use them.
        for _ in 0..3 {
            r.next_double();
        }
        let mut p = [0usize; 512];
        for (i, v) in p.iter_mut().enumerate().take(256) {
            *v = i;
        }
        for i in 0..256 {
            let j = r.next_int(256 - i as i32) as usize + i;
            p.swap(i, j);
            p[i + 256] = p[i];
        }
        Self { p }
    }

    fn noise(&self, x: f64, y: f64) -> f64 {
        let sqrt3 = 3f64.sqrt();
        let f2 = 0.5 * (sqrt3 - 1.0);
        let s = (x + y) * f2;
        let (i, j) = (fast_floor(x + s), fast_floor(y + s));
        let g2 = (3.0 - sqrt3) / 6.0;
        let t = f64::from(i + j) * g2;
        let (x0, y0) = (x - (f64::from(i) - t), y - (f64::from(j) - t));
        let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };
        let (x1, y1) = (x0 - f64::from(i1) + g2, y0 - f64::from(j1) + g2);
        let (x2, y2) = (x0 - 1.0 + 2.0 * g2, y0 - 1.0 + 2.0 * g2);
        let (ii, jj) = ((i & 255) as usize, (j & 255) as usize);
        let p = &self.p;
        let g0 = p[ii + p[jj]] % 12;
        let g1 = p[ii + i1 as usize + p[jj + j1 as usize]] % 12;
        let g2i = p[ii + 1 + p[jj + 1]] % 12;
        let corner = |g: usize, x: f64, y: f64| {
            let t = 0.5 - x * x - y * y;
            if t < 0.0 { 0.0 } else { t * t * t * t * (GRAD3[g][0] * x + GRAD3[g][1] * y) }
        };
        70.0 * (corner(g0, x0, y0) + corner(g1, x1, y1) + corner(g2i, x2, y2))
    }
}

/// `NoiseGeneratorPerlin`: octaves of simplex noise.
struct Perlin(Vec<Simplex>);

impl Perlin {
    fn new(seed: i64, octaves: usize) -> Self {
        let mut r = JavaRandom::new(seed);
        Self((0..octaves).map(|_| Simplex::new(&mut r)).collect())
    }

    fn noise(&self, x: f64, y: f64) -> f64 {
        let (mut sum, mut f) = (0.0, 1.0);
        for o in &self.0 {
            sum += o.noise(x * f, y * f) / f;
            f /= 2.0;
        }
        sum
    }
}

fn noises() -> &'static (Perlin, Vec<Perlin>) {
    static N: std::sync::OnceLock<(Perlin, Vec<Perlin>)> = std::sync::OnceLock::new();
    N.get_or_init(|| (Perlin::new(2_114_043, 4), (0..6).map(|t| Perlin::new(2_082_127 + t, 4)).collect()))
}

/// `getOreLevel`: the density of ore type `t` (0..6) at the chunk of
/// (`x`, `z`), 0..=2.
pub fn ore_level(x: i32, z: i32, t: usize) -> f64 {
    let (level, ores) = noises();
    let scale = 0.01;
    let (x, z) = (f64::from(((x >> 4) << 4) + 8), f64::from(((z >> 4) << 4) + 8));
    (level.noise(x * scale, z * scale) * ores[t].noise(x * scale, z * scale)).abs().mul_add(0.05, 0.0).clamp(0.0, 2.0)
}

/// The 6 densities at (`x`, `z`).
pub fn densities(x: i32, z: i32) -> [f64; 6] {
    std::array::from_fn(|t| ore_level(x, z, t))
}

/// An overworld deposit: its tier (1..=4) and the fluid each extraction
/// takes (`getTier`, `getBoreFluid` of the average density).
#[derive(Clone, Debug, PartialEq)]
pub struct Deposit {
    pub tier: u8,
    pub fluid: Option<(&'static str, u32)>,
}

pub fn deposit(x: i32, z: i32) -> Deposit {
    let avg = densities(x, z).iter().sum::<f64>() / 6.0;
    let (tier, fluid) = if avg > 1.5 {
        (4, Some(("solvent", 2_000)))
    } else if avg > 1.0 {
        (3, Some(("sulfuric_acid", 1_000)))
    } else if avg > 0.75 {
        (2, Some(("water", 1_000)))
    } else {
        (1, None)
    };
    Deposit { tier, fluid }
}

/// The raw ore's item data: the densities as `minecraft:custom_data` SNBT
/// (`{ntm_ore:{light:1.234d,...}}`), each times `mult` (the drill's
/// fortune: 1 + 0.1 a level).
pub fn ore_data(d: &[f64; 6], mult: f64) -> String {
    let parts: Vec<String> = TYPES.iter().zip(d).map(|(t, v)| format!("{t}:{}d", (v * mult * 1e6).round() / 1e6)).collect();
    let s = format!("{{ntm_ore:{{{}}}}}", parts.join(","));
    crate::snbt::canonical(&s).unwrap_or(s)
}

/// The densities back from the item data (0 for what isn't there).
pub fn read_ore_data(snbt: Option<&str>) -> [f64; 6] {
    let mut out = [0.0; 6];
    let Some(s) = snbt else { return out };
    for (i, t) in TYPES.iter().enumerate() {
        let key = format!("{t}:");
        if let Some(at) = s.find(&key) {
            let rest = &s[at + key.len()..];
            let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'E' || c == 'e')).unwrap_or(rest.len());
            out[i] = rest[..end].parse().unwrap_or(0.0);
        }
    }
    out
}

/// The ore slopper (`TileEntityMachineOreSlopper`), without upgrades: 200
/// HE a tick for 600 ticks, a raw ore and 1 000 mB of water into 1 000 mB
/// of slop and the ore's densities; each whole unit of a type's density
/// becomes a BASE ore of that type in the outputs (what doesn't fit waits,
/// as in the NTM: the machine doesn't check its outputs to work).
pub mod slopper {
    use super::TYPES;
    use crate::energy::{EnergyNode, Role};
    use crate::fluid::{FluidNode, Tank};
    use crate::machine::{Logic, fill_level};
    use crate::server::container_bridge::{Slots, Slotted, Stack};
    use crate::view::{self, ViewModel};
    use serde::{Deserialize, Serialize};

    pub const SLOTS: usize = 11;
    pub const BATTERY: usize = 0;
    pub const INPUT: usize = 2;
    pub const OUTPUTS: std::ops::RangeInclusive<usize> = 3..=8;
    pub const RAW: &str = "ntm:bedrock_ore_base";
    pub const TICKS: u32 = 600;
    pub const HE_PER_TICK: u64 = 200;
    pub const MAX_POWER: u64 = 100_000;
    pub const WATER: u32 = 1_000;
    pub const TANK: u32 = 16_000;
    pub const STACK: u32 = 64;

    /// The BASE ore of type `t`.
    pub fn base(t: usize) -> String {
        format!("ntm:bedrock_ore__base_{}", TYPES[t])
    }

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct OreSlopper {
        #[serde(skip)]
        pub slots: Slots,
        pub power: u64,
        pub progress: u32,
        pub water: Tank,
        pub slop: Tank,
        /// What the ores slopped so far hold of each type, not yet an item.
        pub ores: [f64; 6],
    }

    impl Default for OreSlopper {
        fn default() -> Self {
            Self { slots: vec![None; SLOTS], power: 0, progress: 0, water: Tank::typed("water", TANK), slop: Tank::typed("slop", TANK), ores: [0.0; 6] }
        }
    }

    impl EnergyNode for OreSlopper {
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

    /// Water in, slop out.
    impl FluidNode for OreSlopper {
        fn fluid_offer(&self, f: &str) -> u64 {
            if f == "slop" { u64::from(self.slop.mb) } else { 0 }
        }
        fn fluid_demand(&self, f: &str) -> u64 {
            if f == "water" { u64::from(self.water.free()) } else { 0 }
        }
        fn fluid_types(&self) -> Vec<String> {
            vec!["water".into(), "slop".into()]
        }
        fn fluid_give(&mut self, f: &str, mb: u64) {
            self.water.fill(f, mb.min(u64::from(u32::MAX)) as u32);
        }
        fn fluid_take(&mut self, _f: &str, mb: u64) {
            self.slop.drain(mb.min(u64::from(u32::MAX)) as u32);
        }
    }

    impl Logic for OreSlopper {
        fn tick(&mut self) -> bool {
            self.step(|s| super::read_ore_data(s.custom.as_deref()))
        }
        fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
            Some(self)
        }
        fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
            Some(self)
        }
        fn level(&self, max: u8) -> u8 {
            if self.progress == 0 { 0 } else { fill_level(u64::from(self.progress), u64::from(TICKS), max) }
        }
        fn view(&self) -> ViewModel {
            ViewModel::new("Ore slopper")
                .bar("Energia", self.power, MAX_POWER, "HE")
                .bar("Progresso", u64::from(self.progress), u64::from(TICKS), "ticks")
                .text(format!("Água {} mB, slop {} mB", self.water.mb, self.slop.mb), view::GREY)
                .text("Os itens ficam na tela com slots (motor com o patch ntm-visual).", view::GREY)
        }
    }

    impl Slotted for OreSlopper {
        fn slots_mut(&mut self) -> &mut Slots {
            &mut self.slots
        }
        /// Energy, its maximum, progress, its end, water and slop (mB, cap).
        fn screen_data(&self) -> Vec<i32> {
            vec![
                self.power.min(MAX_POWER) as i32,
                MAX_POWER as i32,
                self.progress as i32,
                TICKS as i32,
                self.water.mb as i32,
                self.water.cap as i32,
                self.slop.mb as i32,
                self.slop.cap as i32,
            ]
        }
        /// Progress and what each type has piled up, in millionths.
        fn save_state(&self) -> Vec<i32> {
            let mut v = vec![self.progress as i32];
            v.extend(self.ores.iter().map(|o| (o * 1e6) as i32));
            v
        }
        fn load_state(&mut self, v: &[i32]) {
            if let [progress, ores @ ..] = v {
                self.progress = (*progress).clamp(0, TICKS as i32) as u32;
                for (o, s) in self.ores.iter_mut().zip(ores) {
                    *o = f64::from((*s).max(0)) / 1e6;
                }
            }
        }
    }

    impl OreSlopper {
        /// `canSlop`.
        pub fn can(&self) -> bool {
            self.water.mb >= WATER
                && self.slop.mb + WATER <= self.slop.cap
                && self.power >= HE_PER_TICK
                && self.slots.get(INPUT).and_then(|s| s.as_ref()).is_some_and(|s| s.id == RAW && s.count > 0)
        }

        /// One tick; `densities` reads the raw ore's (its item data).
        /// True when something changed.
        pub fn step(&mut self, densities: impl Fn(&Stack) -> [f64; 6]) -> bool {
            self.power = crate::machine::charge_from_slot(&self.slots, BATTERY, self.power, MAX_POWER);
            let before = (self.progress, self.power);
            if self.can() {
                self.power -= HE_PER_TICK;
                self.progress += 1;
                if self.progress >= TICKS {
                    self.progress = 0;
                    let Some(raw) = self.slots[INPUT].as_mut() else { return true };
                    let d = densities(raw);
                    for (o, v) in self.ores.iter_mut().zip(d) {
                        *o += v;
                    }
                    raw.count -= 1;
                    if raw.count == 0 {
                        self.slots[INPUT] = None;
                    }
                    self.water.drain(WATER);
                    self.slop.fill("slop", WATER);
                }
            } else {
                self.progress = 0;
            }
            let out = self.lay_out();
            out || before != (self.progress, self.power)
        }

        /// The whole units into the output slots: a pile of the same ore
        /// first, then an empty slot.
        fn lay_out(&mut self) -> bool {
            let mut moved = false;
            for t in 0..TYPES.len() {
                let id = base(t);
                'unit: while self.ores[t] >= 1.0 {
                    for i in OUTPUTS {
                        if let Some(s) = &mut self.slots[i]
                            && s.id == id
                            && s.count < STACK
                        {
                            s.count += 1;
                            self.ores[t] -= 1.0;
                            moved = true;
                            continue 'unit;
                        }
                    }
                    for i in OUTPUTS {
                        if self.slots[i].is_none() {
                            self.slots[i] = Some(Stack::new(&id, 1));
                            self.ores[t] -= 1.0;
                            moved = true;
                            continue 'unit;
                        }
                    }
                    break;
                }
            }
            moved
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::container_bridge::Stack;

    /// `java.util.Random` against the JDK's (`java` 21, printed once).
    #[test]
    fn java_random_matches_the_jdk() {
        let mut r = JavaRandom::new(42);
        assert_eq!([r.next_int(100), r.next_int(100), r.next_int(100)], [30, 63, 48]);
        let mut r = JavaRandom::new(42);
        assert_eq!(r.next_double(), 0.7275636800328681);
        let mut r = JavaRandom::new(0);
        assert_eq!(r.next_int(256), 187);
        let mut r = JavaRandom::new(2_114_043);
        assert_eq!((r.next_double(), r.next_int(253), r.next_int(7)), (0.42419897326573597, 18, 2));
    }

    #[test]
    fn densities_are_per_chunk_and_bounded() {
        let a = densities(100, 200);
        assert_eq!(a, densities(111, 207), "o mesmo chunk");
        assert!(a.iter().all(|v| (0.0..=2.0).contains(v)));
        // Over a region, some deposits of every tier.
        let tiers: std::collections::BTreeSet<u8> = (0..60).flat_map(|i| (0..60).map(move |j| deposit(i * 160, j * 160).tier)).collect();
        assert!(tiers.len() >= 3, "{tiers:?}");
    }

    #[test]
    fn ore_data_round_trips() {
        let d = [1.25, 0.5, 0.0, 0.125, 2.0, 0.75];
        let s = ore_data(&d, 1.2);
        assert_eq!(s, "{ntm_ore:{actinide:0.15d,crystal:0.9d,heavy:0.6d,light:1.5d,nonmetal:2.4d,rare:0.0d}}");
        let back = read_ore_data(Some(&s));
        for (a, b) in back.iter().zip(d.iter().map(|v| v * 1.2)) {
            assert!((a - b).abs() < 1e-6);
        }
        assert_eq!(read_ore_data(None), [0.0; 6]);
        // As the engine writes it back (`to_snbt`: 0d as 0.0d, absent types).
        assert_eq!(read_ore_data(Some("{ntm_ore:{light:1.5d,rare:0.0d,crystal:2.0E-4d}}")), [1.5, 0.0, 0.0, 0.0, 0.0, 2.0e-4]);
    }

    #[test]
    fn the_slopper_turns_a_raw_ore_into_base_ores_by_its_densities() {
        use slopper::*;
        let mut m = OreSlopper::default();
        m.slots[INPUT] = Some(Stack::new(RAW, 2));
        m.water.mb = 16_000;
        let dens = |_: &Stack| [1.5, 0.25, 0.0, 2.0, 0.75, 0.5];
        for _ in 0..TICKS {
            m.power = MAX_POWER;
            m.step(dens);
        }
        // One ore: 1 light (0.5 left), 2 actinide; the rest under 1.
        let n = |m: &OreSlopper, t: usize| m.slots[OUTPUTS].iter().flatten().filter(|s| s.id == base(t)).map(|s| s.count).sum::<u32>();
        assert_eq!((n(&m, 0), n(&m, 3), n(&m, 4)), (1, 2, 0));
        assert_eq!((m.water.mb, m.slop.mb, m.slots[INPUT].as_ref().map(|s| s.count)), (15_000, 1_000, Some(1)));
        for _ in 0..TICKS {
            m.power = MAX_POWER;
            m.step(dens);
        }
        assert_eq!((n(&m, 0), n(&m, 3), n(&m, 4), n(&m, 5)), (3, 4, 1, 1));
        assert_eq!(m.slots[INPUT], None);
        // Nothing left to slop: it stops, the fractions wait.
        m.power = MAX_POWER;
        assert!(!m.can());
        // The creative battery keeps it full.
        m.power = 0;
        m.slots[BATTERY] = Some(Stack::new(crate::machine::CREATIVE_BATTERY, 1));
        m.step(dens);
        assert_eq!(m.power, MAX_POWER);
        assert!(!m.can());
        assert!((m.ores[1] - 0.5).abs() < 1e-9);
    }

    #[test]
    fn the_slopper_needs_water_room_for_slop_and_power() {
        use slopper::*;
        let mut m = OreSlopper::default();
        m.slots[INPUT] = Some(Stack::new(RAW, 1));
        m.power = MAX_POWER;
        assert!(!m.can(), "sem água");
        m.water.mb = 1_000;
        m.slop.mb = 15_500;
        assert!(!m.can(), "o slop não cabe");
        m.slop.mb = 0;
        assert!(m.can());
        m.slots[INPUT] = Some(Stack::new("minecraft:stone", 1));
        assert!(!m.can(), "só o minério bruto");
    }
}
