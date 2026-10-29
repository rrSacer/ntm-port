//! Mundo e storage falsos para testar o motor sem o jogo.
#![allow(dead_code)]
use ntm_explosion::{Change, Store, World, crater::{Kind, classify, rand01}};
use std::collections::{HashMap, HashSet};

/// Terreno sintético: colinas, grama/terra/pedra/deepslate/bedrock, água
/// abaixo de y=62, árvores esparsas, cavernas e minério raro.
pub fn surface(x: i32, z: i32) -> i32 {
    64 + (4.0 * (x as f32 / 23.0).sin() + 3.0 * (z as f32 / 17.0).cos()).round() as i32
}

pub fn terrain(x: i32, y: i32, z: i32) -> &'static str {
    let h = surface(x, z);
    if y < -59 {
        return "minecraft:bedrock";
    }
    let (tx, tz) = (x.div_euclid(7) * 7 + 3, z.div_euclid(7) * 7 + 3);
    if rand01(tx, 0, tz, 3) < 0.25 {
        let th = surface(tx, tz);
        if th >= 62 {
            if x == tx && z == tz && y > th && y <= th + 5 {
                return "minecraft:oak_log[axis=y]";
            }
            let (dx, dy, dz) = (x - tx, y - (th + 5), z - tz);
            if dx * dx + dy * dy + dz * dz <= 5 && y > th + 2 {
                return "minecraft:oak_leaves[distance=1,persistent=false,waterlogged=false]";
            }
        }
    }
    if y > h {
        return if y <= 62 { "minecraft:water[level=0]" } else { "minecraft:air" };
    }
    if y < h - 6 && rand01(x >> 2, y >> 2, z >> 2, 5) < 0.04 {
        return "minecraft:cave_air";
    }
    if y == h {
        return if h < 62 { "minecraft:sand" } else { "minecraft:grass_block[snowy=false]" };
    }
    if y >= h - 3 {
        return "minecraft:dirt";
    }
    if rand01(x, y, z, 9) < 0.01 {
        return "minecraft:coal_ore";
    }
    if y < 0 { "minecraft:deepslate[axis=y]" } else { "minecraft:stone" }
}

#[derive(Default, Clone)]
pub struct FakeWorld {
    pub over: HashMap<(i32, i32, i32), String>,
    pub unloaded: HashSet<(i32, i32)>,
    pub viewers: Vec<[f64; 3]>,
    /// Escritas por tick (o teste avança `tick_writes` a cada tick).
    pub tick_writes: Vec<usize>,
    pub max_call: usize,
    pub calls: usize,
    /// Leituras por tick (`begin_tick` abre uma entrada).
    pub tick_reads: Vec<usize>,
}

impl FakeWorld {
    pub fn get(&self, x: i32, y: i32, z: i32) -> String {
        self.over.get(&(x, y, z)).cloned().unwrap_or_else(|| terrain(x, y, z).to_string())
    }
    pub fn begin_tick(&mut self) {
        self.tick_writes.push(0);
        self.tick_reads.push(0);
    }
    /// Mudanças em relação ao terreno original.
    pub fn diff(&self) -> HashMap<(i32, i32, i32), String> {
        self.over.iter().filter(|(k, v)| terrain(k.0, k.1, k.2) != v.as_str()).map(|(k, v)| (*k, v.clone())).collect()
    }
}

impl World for FakeWorld {
    fn block(&mut self, _dim: &str, x: i32, y: i32, z: i32) -> Option<String> {
        if let Some(t) = self.tick_reads.last_mut() {
            *t += 1;
        }
        if self.unloaded.contains(&(x >> 4, z >> 4)) || !(-64..=319).contains(&y) {
            return None;
        }
        Some(self.get(x, y, z))
    }
    fn set_blocks(&mut self, _dim: &str, changes: Vec<Change>) -> Result<(), String> {
        assert!(changes.len() <= 4096, "{} por chamada", changes.len());
        self.calls += 1;
        self.max_call = self.max_call.max(changes.len());
        if let Some(t) = self.tick_writes.last_mut() {
            *t += changes.len();
        }
        for c in changes {
            // Como o Horizon: chunk não carregado = mudança ignorada.
            if !self.unloaded.contains(&(c.x >> 4, c.z >> 4)) {
                self.over.insert((c.x, c.y, c.z), c.state);
            }
        }
        Ok(())
    }
    fn viewers(&mut self, _dim: &str) -> Vec<[f64; 3]> {
        self.viewers.clone()
    }
}

/// Storage com os limites do Horizon: 4 MiB e 64 chaves por mod, 1 MiB por gravação.
#[derive(Default, Clone)]
pub struct FakeStore {
    pub keys: HashMap<String, Vec<u8>>,
    pub max_total: usize,
    pub writes: usize,
    /// Queda no meio de uma gravação: a primeira cujo valor `tear` aceita
    /// grava só a metade, e o "servidor" cai (panic).
    pub tear: Option<fn(&str, &[u8]) -> bool>,
}

/// O panic de [`FakeStore::tear`].
pub const TORN: &str = "queda no meio da gravação";

impl FakeStore {
    pub fn total(&self) -> usize {
        self.keys.values().map(Vec::len).sum()
    }
}

impl Store for FakeStore {
    fn get(&mut self, key: &str) -> Option<Vec<u8>> {
        self.keys.get(key).cloned()
    }
    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), String> {
        if value.len() > 1 << 20 {
            return Err("mais de 1 MiB numa gravação".into());
        }
        let old = self.keys.get(key).map_or(0, Vec::len);
        if self.total() - old + value.len() > 4 << 20 {
            return Err("quota de 4 MiB".into());
        }
        if !self.keys.contains_key(key) && self.keys.len() >= 64 {
            return Err("64 chaves".into());
        }
        if let Some(f) = self.tear
            && f(key, value)
        {
            self.tear = None;
            self.keys.insert(key.into(), value[..value.len() / 2].to_vec());
            std::panic::panic_any(TORN);
        }
        self.keys.insert(key.into(), value.to_vec());
        self.writes += 1;
        self.max_total = self.max_total.max(self.total());
        Ok(())
    }
    fn delete(&mut self, key: &str) {
        self.keys.remove(key);
    }
}

pub fn center() -> [i32; 3] {
    [0, surface(0, 0) + 1, 0]
}

fn is_air(s: &str) -> bool {
    matches!(classify(s).kind, Kind::Air)
}

/// Reimplementação fiel do `ExplosionNukeRayBatched` do NTM (referência).
pub fn ntm(c: [i32; 3], r: i32) -> (HashSet<[i32; 3]>, u64) {
    let strength = 2 * r;
    let length = r;
    let max = (2.5 * std::f64::consts::PI * (strength as f64).powi(2)) as i64;
    let (mut gx, mut gy) = (std::f64::consts::PI, 0.0f64);
    let mut num = 1i64;
    let mut reads = 0u64;
    let mut out = HashSet::new();
    while num < max {
        let v = [gx.sin() * gy.cos(), gx.cos(), gx.sin() * gy.sin()];
        // generateGspUp
        if num < max {
            let k = num + 1;
            let hk = -1.0 + 2.0 * (k as f64 - 1.0) / (max as f64 - 1.0);
            gx = hk.acos();
            gy = (gy + 3.6 / (max as f64).sqrt() / (1.0 - hk * hk).sqrt()) % (std::f64::consts::PI * 2.0);
        }
        num += 1;
        let mut res = strength as f32;
        let mut tip: Option<[f32; 3]> = None;
        let len_i = strength; // (int) ceil(strength)
        for i in 0..len_i {
            if i > length {
                break;
            }
            let p = [
                (c[0] as f64 + v[0] * i as f64) as f32,
                (c[1] as f64 + v[1] * i as f64) as f32,
                (c[2] as f64 + v[2] * i as f64) as f32,
            ];
            let b = terrain(p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32);
            reads += 1;
            let m = classify(b);
            let fac = (100.0 - i as f64 / len_i as f64 * 100.0) * 0.07;
            if m.kind != Kind::Liquid {
                res -= (m.res as f64).powf(7.5 - fac) as f32;
            }
            if res > 0.0 && m.kind != Kind::Air {
                tip = Some(p);
            }
            if res <= 0.0 || i + 1 >= length || i == len_i - 1 {
                break;
            }
        }
        // processChunk: refaz a reta até a ponta e remove o que não é ar.
        if let Some(t) = tip {
            let d = [t[0] as f64 - c[0] as f64, t[1] as f64 - c[1] as f64, t[2] as f64 - c[2] as f64];
            let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            if l == 0.0 {
                continue;
            }
            let mut i = 0.0;
            while i < l {
                let q = [
                    (c[0] as f64 + d[0] / l * i).floor() as i32,
                    (c[1] as f64 + d[1] / l * i).floor() as i32,
                    (c[2] as f64 + d[2] / l * i).floor() as i32,
                ];
                reads += 1;
                if !is_air(terrain(q[0], q[1], q[2])) {
                    out.insert(q);
                }
                i += 1.0;
            }
            let q = [t[0].floor() as i32, t[1].floor() as i32, t[2].floor() as i32];
            out.insert(q);
        }
    }
    (out, reads)
}

