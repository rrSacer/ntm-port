//! `java.util.Random`, serializable (decision D3: one RNG per simulation).

#[derive(Clone, Debug, PartialEq)]
pub struct JavaRandom {
    seed: u64,
    next_next_gaussian: Option<f64>,
}

impl JavaRandom {
    const MUL: u64 = 0x5_DEEC_E66D;
    const MASK: u64 = (1 << 48) - 1;

    pub fn new(seed: i64) -> Self {
        Self {
            seed: (seed as u64 ^ Self::MUL) & Self::MASK,
            next_next_gaussian: None,
        }
    }

    /// The raw state, for saving.
    pub fn state(&self) -> (u64, Option<f64>) {
        (self.seed, self.next_next_gaussian)
    }

    pub fn from_state(seed: u64, next_next_gaussian: Option<f64>) -> Self {
        Self {
            seed: seed & Self::MASK,
            next_next_gaussian,
        }
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = (self.seed.wrapping_mul(Self::MUL).wrapping_add(0xB)) & Self::MASK;
        (self.seed >> (48 - bits)) as i64 as i32
    }

    pub fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0);
        if bound & (bound - 1) == 0 {
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

    pub fn next_boolean(&mut self) -> bool {
        self.next(1) != 0
    }

    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 / (1u32 << 24) as f32
    }

    pub fn next_double(&mut self) -> f64 {
        let hi = i64::from(self.next(26)) << 27;
        (hi + i64::from(self.next(27))) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    pub fn next_gaussian(&mut self) -> f64 {
        if let Some(g) = self.next_next_gaussian.take() {
            return g;
        }
        loop {
            let v1 = 2.0 * self.next_double() - 1.0;
            let v2 = 2.0 * self.next_double() - 1.0;
            let s = v1 * v1 + v2 * v2;
            if s < 1.0 && s != 0.0 {
                let m = (-2.0 * s.ln() / s).sqrt();
                self.next_next_gaussian = Some(v2 * m);
                return v1 * m;
            }
        }
    }
}
