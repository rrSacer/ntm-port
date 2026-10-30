//! Fallout de 2,5R (docs/explosoes.md §8): a segunda fase de uma explosão.
//!
//! O NTM (`EntityFalloutRain`, linhas 57-220, regras em `FalloutConfigJSON`
//! 54-120) varre, depois da cratera, toda coluna até `escala = 2,5R`, de
//! y=255 para baixo até 3 blocos sólidos, e aplica regras por distância
//! (`d` = distância horizontal / escala):
//!
//! - madeira vira `waste_log`, tábuas `waste_planks`, folhas e plantas somem
//!   (até 65%); folhas além de 60% viram `waste_leaves`;
//! - pedra, terra e areia viram `sellafield_slaked` em faixas até 50% (grama
//!   até 45%); areia além disso tem 5% de trinitite;
//! - camada `fallout` no topo com chance `0,1 − (d − 0,7)²`;
//! - fogo em 1/5 dos inflamáveis até 65%.
//!
//! Aqui: colunas em anéis de R a 2,5R (dentro do disco as bordas expostas da
//! cratera já cuidam); cada coluna começa a busca em `max(topo das vizinhas já
//! feitas) + 8` e sobe 16 por vez enquanto o bloco inicial não for ar: ~15
//! leituras em vez de ~185. O motor nunca força carga de chunk: uma coluna
//! de chunk descarregado fica **pendente por chunk** e é feita quando um
//! jogador chega perto (`Linger`). Troca de bioma e blocos caindo como
//! entidade não existem na API: ficam de fora.
use crate::explosion::crater::{BuildFnv, Kind, MAX_Y, MIN_Y, Mats, rand01};
use crate::explosion::engine::{AIR, Change};
use std::collections::{HashMap, HashSet};

/// Fuel por leitura (get-block + classificação em cache), por coluna (busca
/// do topo nas vizinhas) e por escrita: medidos no harness de fuel
/// (`spikes/m2-fuel`, docs/bombas.md), com folga.
pub const COST_READ: u64 = 1_100;
pub const COST_COLUMN: u64 = 4_000;
pub const COST_WRITE: u64 = 900;
/// Uma coluna pulada (chunk pendente).
pub const COST_SKIP: u64 = 600;

/// O que o fallout precisa saber de um bloco, por material (classificado uma
/// vez por nome: `classify` compara padrões de texto e é caro em wasm).
#[derive(Clone, Copy, Default)]
struct Info {
    kind: Option<Kind>,
    air: bool,
    clay: bool,
    grassy: bool,
    layer: bool,
}

/// Cache de [`Info`] por material.
#[derive(Default)]
pub struct Classes {
    mats: Mats,
    info: Vec<Info>,
    layer: Option<String>,
}

impl Classes {
    fn get(&mut self, state: &str) -> Info {
        if self.info.is_empty() {
            self.mats = Mats::new();
        }
        let id = self.mats.id(state) as usize;
        if self.info.len() <= id {
            self.info.resize(id + 1, Info::default());
        }
        if self.info[id].kind.is_none() {
            let name = state.split('[').next().unwrap_or(state);
            let m = self.mats.table[id];
            self.info[id] = Info {
                kind: Some(m.kind),
                air: m.kind == Kind::Air,
                clay: name == "minecraft:clay",
                grassy: matches!(name, "minecraft:grass_block" | "minecraft:mycelium" | "minecraft:podzol"),
                layer: self.layer.as_deref().is_some_and(|l| l.split('[').next() == Some(name)),
            };
        }
        self.info[id]
    }
}

/// Regras do fallout, como dados (vão no perfil persistido).
#[derive(Clone, Debug, PartialEq)]
pub struct FalloutProfile {
    /// Raio do fallout em múltiplos do raio da cratera (NTM: 2,5).
    pub scale: f32,
    /// Pedra/terra/areia por faixa de `d`: (até d, bloco).
    pub slag: Vec<(f32, String)>,
    /// Grama só até aqui (NTM: 45%).
    pub grass_max: f32,
    /// Madeira, tábuas, folhagem e fogo até aqui (NTM: 65%).
    pub wood_max: f32,
    pub wood: Option<String>,
    pub planks: Option<String>,
    /// Folhas além de 60%: este bloco (`waste_leaves`); None as deixa.
    pub dead_leaves: Option<String>,
    /// Areia além das faixas: (chance, bloco).
    pub trinitite: Option<(f32, String)>,
    /// Camada no topo (NTM `fallout`), com a chance `0,1 − (d − 0,7)²`.
    pub layer: Option<String>,
    /// Fogo acima de inflamáveis: (chance, bloco).
    pub fire: Option<(f32, String)>,
    /// Argila vira isto (NTM: `hardened_clay`).
    pub clay: Option<String>,
}

impl FalloutProfile {
    /// Regras do NTM com blocos vanilla no lugar dos do NTM.
    pub fn ntm_placeholder() -> Self {
        Self {
            scale: 2.5,
            slag: vec![
                (0.2, "minecraft:blackstone".into()),
                (0.35, "minecraft:deepslate".into()),
                (0.5, "minecraft:tuff".into()),
            ],
            grass_max: 0.45,
            wood_max: 0.65,
            wood: Some("minecraft:basalt".into()),
            planks: Some("minecraft:dark_oak_planks".into()),
            dead_leaves: None,
            trinitite: Some((0.05, "minecraft:green_stained_glass".into())),
            layer: Some("minecraft:light_gray_carpet".into()),
            fire: Some((0.2, "minecraft:fire".into())),
            clay: Some("minecraft:terracotta".into()),
        }
    }

    pub fn encode(&self) -> String {
        let o = |v: &Option<String>| v.clone().unwrap_or_default();
        let nb = |v: &Option<(f32, String)>| v.as_ref().map(|(c, b)| format!("{c}~{b}")).unwrap_or_default();
        let slag: Vec<String> = self.slag.iter().map(|(f, b)| format!("{f}~{b}")).collect();
        [
            self.scale.to_string(),
            slag.join(" "),
            self.grass_max.to_string(),
            self.wood_max.to_string(),
            o(&self.wood),
            o(&self.planks),
            o(&self.dead_leaves),
            nb(&self.trinitite),
            o(&self.layer),
            nb(&self.fire),
            o(&self.clay),
        ]
        .join(";")
    }

    pub fn decode(s: &str) -> Result<Self, String> {
        let p: Vec<&str> = s.split(';').collect();
        if p.len() != 11 {
            return Err(format!("fallout: {s}"));
        }
        let num = |v: &str| v.parse::<f32>().map_err(|e| format!("fallout {v}: {e}"));
        let opt = |v: &str| (!v.is_empty()).then(|| v.to_string());
        let nb = |v: &str| -> Result<Option<(f32, String)>, String> {
            if v.is_empty() {
                return Ok(None);
            }
            let (n, b) = v.split_once('~').ok_or(format!("fallout: {v}"))?;
            Ok(Some((num(n)?, b.to_string())))
        };
        Ok(Self {
            scale: num(p[0])?,
            slag: p[1].split(' ').filter(|v| !v.is_empty()).map(|v| nb(v).map(Option::unwrap)).collect::<Result<_, _>>()?,
            grass_max: num(p[2])?,
            wood_max: num(p[3])?,
            wood: opt(p[4]),
            planks: opt(p[5]),
            dead_leaves: opt(p[6]),
            trinitite: nb(p[7])?,
            layer: opt(p[8]),
            fire: nb(p[9])?,
            clay: opt(p[10]),
        })
    }

    fn slag_at(&self, d: f32) -> Option<&String> {
        self.slag.iter().find(|(max, _)| d < *max).map(|(_, b)| b)
    }
}

/// O que a varredura precisa do mundo (o `World` do motor, sem escrita).
pub trait Reader {
    fn block(&mut self, x: i32, y: i32, z: i32) -> Option<String>;
}

impl<F: FnMut(i32, i32, i32) -> Option<String>> Reader for F {
    fn block(&mut self, x: i32, y: i32, z: i32) -> Option<String> {
        self(x, y, z)
    }
}

/// Resultado de uma coluna: leituras, topo achado, ou chunk sem carregar.
pub enum Column {
    Done { reads: u32, top: i32 },
    Unloaded,
}

/// Aplica as regras numa coluna (x, z) a `d` (fração da escala), começando a
/// busca do topo em `start`. Escreve as mudanças em `out`.
#[allow(clippy::too_many_arguments)]
pub fn stomp(p: &FalloutProfile, cls: &mut Classes, x: i32, z: i32, d: f32, start: i32, world: &mut dyn Reader, out: &mut Vec<Change>) -> Column {
    if cls.layer.is_none() {
        cls.layer = p.layer.clone();
    }
    let mut reads = 0u32;
    let mut y = start.clamp(MIN_Y, MAX_Y);
    // Sobe até achar ar (o começo pode ter caído dentro de terreno ou copa).
    loop {
        reads += 1;
        let Some(s) = world.block(x, y, z) else { return Column::Unloaded };
        if cls.get(&s).air || y >= MAX_Y {
            break;
        }
        y = (y + 16).min(MAX_Y);
    }
    let layer_chance = 0.1 - (d - 0.7).powi(2);
    let mut depth = 0;
    let mut top = MIN_Y;
    let mut above_air = true;
    let mut first = true;
    while y > MIN_Y && depth < 3 {
        y -= 1;
        reads += 1;
        let Some(s) = world.block(x, y, z) else { return Column::Unloaded };
        let info = cls.get(&s);
        if info.air || info.layer {
            above_air = true;
            continue;
        }
        if first {
            top = y;
        }
        let m = crate::explosion::crater::Mat { res: 0.0, kind: info.kind.unwrap_or(Kind::Other), gravity: false };
        let mut put = |state: &str, at: i32| out.push(Change { x, y: at, z, state: state.into() });
        if first
            && above_air
            && !matches!(m.kind, Kind::Liquid | Kind::Plant | Kind::Leaves)
            && layer_chance > 0.0
            && rand01(x, y, z, 21) < layer_chance
            && let Some(l) = &p.layer
        {
            put(l, y + 1);
        }
        first = false;
        let flammable = matches!(m.kind, Kind::Wood | Kind::Planks | Kind::Leaves);
        if d < p.wood_max
            && flammable
            && above_air
            && let Some((chance, f)) = &p.fire
            && rand01(x, y, z, 23) < *chance
        {
            put(f, y + 1);
        }
        let mut solid = false;
        match m.kind {
            Kind::Wood if d < p.wood_max => {
                if let Some(w) = &p.wood {
                    // Mantém o eixo do tronco (`axis=`), se houver.
                    let axis = s.find("axis=").map(|i| &s[i + 5..i + 6]).filter(|a| matches!(*a, "x" | "y" | "z"));
                    match axis {
                        Some(a) if !w.contains('[') => put(&format!("{w}[axis={a}]"), y),
                        _ => put(w, y),
                    }
                }
                solid = true;
            }
            Kind::Planks if d < p.wood_max => {
                if let Some(w) = &p.planks {
                    put(w, y);
                }
                solid = true;
            }
            Kind::Leaves | Kind::Plant if d < p.wood_max => put(AIR, y),
            Kind::Leaves => {
                if let Some(l) = &p.dead_leaves {
                    put(l, y);
                }
            }
            Kind::Soil if info.clay => {
                if let Some(c) = &p.clay {
                    put(c, y);
                }
                solid = true;
            }
            Kind::Rock | Kind::Soil | Kind::Sand => {
                let limit = if info.grassy { p.grass_max } else { f32::INFINITY };
                match p.slag_at(d).filter(|_| d < limit) {
                    Some(b) => put(b, y),
                    None if m.kind == Kind::Sand => {
                        if let Some((chance, t)) = &p.trinitite
                            && rand01(x, y, z, 7) < *chance
                        {
                            put(t, y);
                        }
                    }
                    None => {}
                }
                solid = true;
            }
            Kind::Liquid | Kind::Plant => {}
            _ => solid = true,
        }
        if solid {
            depth += 1;
        }
        above_air = false;
    }
    Column::Done { reads, top }
}

/// Anéis de colunas de R a `escala` em volta do centro, com o topo das já
/// feitas para começar as vizinhas, e os chunks pendentes.
pub struct Sweep {
    pub c: [i32; 3],
    pub r: i32,
    pub outer: i32,
    /// Próximo anel a gerar, colunas do anel atual e posição nele.
    pub ring_k: i32,
    ring: Vec<(i32, i32)>,
    pos: usize,
    tops_prev: HashMap<(i32, i32), i16, BuildFnv>,
    tops_cur: HashMap<(i32, i32), i16, BuildFnv>,
    cls: Classes,
    /// Chunks com colunas que ficaram para quando carregarem.
    pub pending: Vec<(i32, i32)>,
    pending_set: HashSet<(i32, i32), BuildFnv>,
    /// Chunk pendente em curso: a próxima coluna (0..256, x depois z) e o
    /// topo da última feita. Um chunk é feito aos pedaços, dentro do fuel e
    /// das escritas de cada chamada (revisão do M2, P1-4).
    pcur: Option<((i32, i32), u16, Option<i32>)>,
    /// Chunks começados que descarregaram no meio (de onde continuar).
    partial: HashMap<(i32, i32), (u16, Option<i32>), BuildFnv>,
    /// Próximo pendente a conferir (rodízio).
    pscan: usize,
    pub columns: u64,
    pub reads: u64,
    pub writes: u64,
}

impl Sweep {
    pub fn new(c: [i32; 3], r: i32, scale: f32) -> Self {
        let outer = ((r as f32) * scale).ceil() as i32;
        Self {
            c,
            r,
            outer,
            ring_k: r,
            ring: Vec::new(),
            pos: 0,
            tops_prev: HashMap::default(),
            tops_cur: HashMap::default(),
            cls: Classes::default(),
            pending: Vec::new(),
            pending_set: HashSet::default(),
            pcur: None,
            partial: HashMap::default(),
            pscan: 0,
            columns: 0,
            reads: 0,
            writes: 0,
        }
    }

    /// Retoma no anel `k` com os chunks pendentes já conhecidos.
    pub fn resume(c: [i32; 3], r: i32, scale: f32, k: i32, pending: Vec<(i32, i32)>) -> Self {
        let mut s = Self::new(c, r, scale);
        s.ring_k = k.max(r);
        s.pending_set = pending.iter().copied().collect::<HashSet<_, BuildFnv>>();
        s.pending = pending;
        s
    }

    /// A passada principal acabou (pode haver pendentes).
    pub fn finished(&self) -> bool {
        self.ring_k > self.outer && self.pos >= self.ring.len()
    }

    /// Anel atual (o próximo a começar, para a persistência).
    pub fn ring_now(&self) -> i32 {
        if self.pos < self.ring.len() { self.ring_k - 1 } else { self.ring_k }
    }

    fn in_annulus(&self, dx: i32, dz: i32) -> bool {
        let h2 = i64::from(dx) * i64::from(dx) + i64::from(dz) * i64::from(dz);
        h2 > i64::from(self.r) * i64::from(self.r) && h2 <= i64::from(self.outer) * i64::from(self.outer)
    }

    fn next_ring(&mut self) {
        let k = self.ring_k;
        self.ring_k += 1;
        self.pos = 0;
        self.ring.clear();
        // k² ≤ h² < (k+1)², limitado ao anel R < h ≤ externo: por dx, as
        // faixas de dz (O(k) por anel, não O(k²)).
        let (lo2, hi2) = (i64::from(k) * i64::from(k), i64::from(k + 1) * i64::from(k + 1));
        let (r2, o2) = (i64::from(self.r) * i64::from(self.r), i64::from(self.outer) * i64::from(self.outer));
        let (lo2, hi2) = (lo2.max(r2 + 1), hi2.min(o2 + 1));
        if lo2 < hi2 {
            for dx in -(k + 1)..=(k + 1) {
                let dx2 = i64::from(dx) * i64::from(dx);
                if dx2 >= hi2 {
                    continue;
                }
                let a = lo2 - dx2;
                let mut dz0 = if a <= 0 { 0 } else { isqrt(a) };
                if a > 0 && i64::from(dz0) * i64::from(dz0) < a {
                    dz0 += 1;
                }
                let dz1 = isqrt(hi2 - 1 - dx2);
                for dz in dz0..=dz1 {
                    self.ring.push((dx, -dz));
                    if dz != 0 {
                        self.ring.push((dx, dz));
                    }
                }
            }
        }
        self.tops_prev = std::mem::take(&mut self.tops_cur);
    }

    fn start_y(&self, dx: i32, dz: i32) -> i32 {
        let mut best: Option<i32> = None;
        for ox in -1..=1 {
            for oz in -1..=1 {
                for m in [&self.tops_prev, &self.tops_cur] {
                    if let Some(&t) = m.get(&(dx + ox, dz + oz)) {
                        best = Some(best.map_or(i32::from(t), |b| b.max(i32::from(t))));
                    }
                }
            }
        }
        best.map_or(self.c[1] + 8, |t| t + 8)
    }

    fn pend(&mut self, chunk: (i32, i32)) {
        if self.pending_set.insert(chunk) {
            self.pending.push(chunk);
        }
    }

    /// Avança a passada principal até gastar `fuel` ou `budget` escritas.
    pub fn step(&mut self, p: &FalloutProfile, mut fuel: u64, budget: usize, world: &mut dyn Reader, out: &mut Vec<Change>) -> u64 {
        let used0 = fuel;
        loop {
            if self.pos >= self.ring.len() {
                if self.ring_k > self.outer {
                    break;
                }
                fuel = fuel.saturating_sub(self.ring_k as u64 * 400 + 2_000);
                self.next_ring();
                continue;
            }
            if fuel < 30 * COST_READ || out.len() + 16 > budget {
                break;
            }
            let (dx, dz) = self.ring[self.pos];
            self.pos += 1;
            let (x, z) = (self.c[0] + dx, self.c[2] + dz);
            let chunk = (x >> 4, z >> 4);
            if self.pending_set.contains(&chunk) {
                // Coluna de chunk que não estava carregado: só o laço e o hash
                // (medido: num anel quase todo descarregado são milhares por tick).
                fuel = fuel.saturating_sub(COST_SKIP);
                continue;
            }
            let before = out.len();
            let d = ((dx * dx + dz * dz) as f32).sqrt() / self.outer as f32;
            let start = self.start_y(dx, dz);
            match stomp(p, &mut self.cls, x, z, d, start, world, out) {
                Column::Unloaded => {
                    out.truncate(before);
                    self.pend(chunk);
                    fuel = fuel.saturating_sub(COST_READ);
                }
                Column::Done { reads, top } => {
                    self.tops_cur.insert((dx, dz), top as i16);
                    self.columns += 1;
                    self.reads += u64::from(reads);
                    fuel = fuel.saturating_sub(u64::from(reads) * COST_READ + COST_COLUMN + (out.len() - before) as u64 * COST_WRITE);
                }
            }
        }
        self.writes += out.len() as u64;
        used0 - fuel
    }

    /// Faz os chunks pendentes que carregaram, perto de `viewers` (até
    /// `near` blocos), coluna a coluna, até gastar `fuel` ou `budget`
    /// escritas; um chunk pode ficar pela metade para a próxima chamada.
    /// Devolve o fuel estimado gasto.
    #[allow(clippy::too_many_arguments)]
    pub fn pending_step(&mut self, p: &FalloutProfile, viewers: &[[f64; 3]], near: f64, mut fuel: u64, budget: usize, world: &mut dyn Reader, out: &mut Vec<Change>) -> u64 {
        // Uma coluna cabe nisto (com folga: ~10 leituras na média).
        const COLUMN: u64 = 64 * COST_READ + COST_COLUMN;
        let f0 = fuel;
        let w0 = out.len();
        let mut scanned = 0;
        loop {
            if fuel < COLUMN || out.len() + 16 > budget {
                break;
            }
            let Some((ch, col, top)) = self.pcur else {
                // Próximo chunk pendente perto de alguém e carregado.
                if scanned >= self.pending.len() {
                    break;
                }
                scanned += 1;
                if self.pscan >= self.pending.len() {
                    self.pscan = 0;
                }
                let ch = self.pending[self.pscan];
                self.pscan += 1;
                let (cx, cz) = (f64::from(ch.0 * 16 + 8), f64::from(ch.1 * 16 + 8));
                let close = viewers.iter().any(|v| (v[0] - cx).powi(2) + (v[2] - cz).powi(2) <= near * near);
                fuel = fuel.saturating_sub(50);
                if !close {
                    continue;
                }
                fuel = fuel.saturating_sub(COST_READ);
                if world.block(ch.0 * 16, self.c[1], ch.1 * 16).is_none() {
                    continue;
                }
                let (col, top) = self.partial.remove(&ch).unwrap_or((0, None));
                self.pcur = Some((ch, col, top));
                continue;
            };
            if col >= 256 {
                // Chunk inteiro.
                if let Some(i) = self.pending.iter().position(|c| *c == ch) {
                    self.pending.swap_remove(i);
                }
                fuel = fuel.saturating_sub(self.pending.len() as u64 * 4);
                self.pending_set.remove(&ch);
                self.pcur = None;
                continue;
            }
            let (x, z) = (ch.0 * 16 + i32::from(col / 16), ch.1 * 16 + i32::from(col % 16));
            let (dx, dz) = (x - self.c[0], z - self.c[2]);
            if !self.in_annulus(dx, dz) {
                fuel = fuel.saturating_sub(20);
                self.pcur = Some((ch, col + 1, top));
                continue;
            }
            let d = ((dx * dx + dz * dz) as f32).sqrt() / self.outer as f32;
            let start = top.map_or(self.c[1] + 8, |t| t + 8);
            let w = out.len();
            match stomp(p, &mut self.cls, x, z, d, start, world, out) {
                Column::Done { reads, top } => {
                    self.columns += 1;
                    self.reads += u64::from(reads);
                    fuel = fuel.saturating_sub(u64::from(reads) * COST_READ + COST_COLUMN + (out.len() - w) as u64 * COST_WRITE);
                    self.pcur = Some((ch, col + 1, Some(top)));
                }
                Column::Unloaded => {
                    // Descarregou no meio: continua dali quando carregar.
                    out.truncate(w);
                    fuel = fuel.saturating_sub(COST_READ);
                    self.partial.insert(ch, (col, top));
                    self.pcur = None;
                }
            }
        }
        self.writes += (out.len() - w0) as u64;
        f0 - fuel
    }
}

fn isqrt(v: i64) -> i32 {
    if v <= 0 {
        return 0;
    }
    let mut s = (v as f64).sqrt() as i64;
    while s * s > v {
        s -= 1;
    }
    while (s + 1) * (s + 1) <= v {
        s += 1;
    }
    s as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rings_cover_the_annulus_once() {
        let mut s = Sweep::new([0, 64, 0], 7, 2.5);
        let mut seen = HashSet::new();
        while s.ring_k <= s.outer {
            s.next_ring();
            for &c in &s.ring {
                assert!(seen.insert(c), "{c:?} duas vezes");
            }
        }
        let want: HashSet<(i32, i32)> = (-20..=20)
            .flat_map(|x| (-20..=20).map(move |z| (x, z)))
            .filter(|&(x, z)| s.in_annulus(x, z))
            .collect();
        assert_eq!(seen, want);
    }
}
