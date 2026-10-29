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
    format!("{{ntm_ore:{{{}}}}}", parts.join(","))
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(s, "{ntm_ore:{light:1.5d,heavy:0.6d,rare:0d,actinide:0.15d,nonmetal:2.4d,crystal:0.9d}}");
        let back = read_ore_data(Some(&s));
        for (a, b) in back.iter().zip(d.iter().map(|v| v * 1.2)) {
            assert!((a - b).abs() < 1e-6);
        }
        assert_eq!(read_ore_data(None), [0.0; 6]);
    }
}
