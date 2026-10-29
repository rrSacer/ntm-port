//! Hashing for checksums and entropy: the mods have no random source
//! (wasm32-unknown-unknown), so ids come from mixing what they can observe.

/// FNV-1a, 64 bits.
pub fn fnv64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// SplitMix64's finaliser: spreads every input bit over the output.
pub fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// An entropy pool: feed it whatever varies (frame times, positions,
/// ticks, counters kept in storage), then draw.
#[derive(Clone, Debug, Default)]
pub struct Entropy(u64);

impl Entropy {
    pub fn feed(&mut self, v: u64) {
        self.0 = mix(self.0 ^ mix(v));
    }
    pub fn feed_bytes(&mut self, b: &[u8]) {
        self.feed(fnv64(b));
    }
    pub fn draw(&mut self) -> u64 {
        self.feed(0x6e74_6d00);
        self.0
    }
}
