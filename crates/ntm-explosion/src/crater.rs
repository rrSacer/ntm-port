//! Cálculo da cratera, sem SDK (compila nativo e em WebAssembly).
//!
//! Modelo de energia do NTM MK5 (`ExplosionNukeRayBatched.collectTip`,
//! linhas 86-127): energia inicial `E0 = 2R`; a cada passo `i` ao longo do
//! raio, um bloco não-líquido custa `res^(0.5 + 3.5·i/R)`; o bloco é
//! destruído enquanto a energia restante for > 0. Líquidos não custam, ar não
//! custa e não é "tip".
//!
//! Diferença de algoritmo: o NTM lança 10πR² raios de até R passos e lê cada
//! bloco ~7,5 vezes (mais uma segunda caminhada na destruição). Aqui cada
//! voxel tem um "pai" — o voxel um passo mais perto do centro na reta até ele
//! — e herda a energia do pai menos o próprio custo (uma árvore de raios).
//! As colunas (dx, dz) do disco são varridas em espiral (por h² = dx²+dz²),
//! então o pai sempre já foi calculado. Cada voxel é lido no máximo uma vez, e
//! um voxel cujo pai já esgotou nem é lido (poda: o subsolo além da cratera
//! custa zero leituras).
//!
//! **Memória esparsa** (docs/explosoes.md §9): nada é guardado por voxel do
//! disco inteiro. Os pais e os cantos da interpolação de um voxel ficam nas
//! 3×3 colunas em volta da dele, a menos de ~2,5 blocos na horizontal do
//! centro; então a energia vive só em colunas da **frente da espiral**, num
//! buffer `u16` por coluna tirado de uma pilha reciclada e devolvido quando a
//! frente passa [`RELEASE`] blocos dele. As remoções viram **faixas por
//! coluna** (início, comprimento) num vetor só anexado, e as bordas são
//! filtradas na hora (só as expostas ficam). A ordem em espiral é gerada anel
//! a anel, sob demanda. Com isso o plano de R=500 (Tsar) cabe em ~20 MB e
//! montá-lo custa O(R), não O(R²).
use std::collections::HashMap;

pub const MIN_Y: i32 = -64;
pub const MAX_Y: i32 = 319;
/// Energia guardada em u16: `E * SCALE`. E0 = 2R ≤ 300 cabe; acima de R=327
/// a escala cai para manter E0·SCALE < 65 535 (ver `Plan::new`).
const SCALE: f32 = 100.0;
pub const MAX_RADIUS: i32 = 500;
/// Fuel estimado por voxel visitado e por leitura (get-block + classificação),
/// e por início de coluna; somados dão ~800 por voxel lido, o pior caso medido.
pub const COST_VISIT: u64 = 440;
pub const COST_READ: u64 = 570;
pub const COST_COLUMN: u64 = 600;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Kind {
    Air,
    Liquid,
    Rock,
    Soil,
    Sand,
    Wood,
    Planks,
    Leaves,
    Plant,
    Other,
    /// Indestrutível na prática (bedrock, portal do End...).
    Wall,
}

#[derive(Clone, Copy, Debug)]
pub struct Mat {
    pub res: f32,
    pub kind: Kind,
    /// Cai com gravidade (areia, cascalho...): diferença após a escrita é física, não perda.
    pub gravity: bool,
}

/// Resistência a explosão (escala moderna = `getExplosionResistance` do 1.7.10),
/// com o `masqueradeResistance` do NTM (Batched:156-161): arenito vale pedra,
/// obsidiana vale 3× pedra.
pub fn classify(name: &str) -> Mat {
    let n = name.split('[').next().unwrap_or(name);
    let n = n.strip_prefix("minecraft:").unwrap_or(n);
    let m = |res: f32, kind: Kind| Mat {
        res,
        kind,
        gravity: false,
    };
    let g = |res: f32, kind: Kind| Mat {
        res,
        kind,
        gravity: true,
    };
    match n {
        "air" | "cave_air" | "void_air" | "light" => return m(0.0, Kind::Air),
        "water" | "lava" | "bubble_column" => return m(0.0, Kind::Liquid),
        "bedrock" | "barrier" | "end_portal_frame" | "end_portal" | "end_gateway"
        | "command_block" | "chain_command_block" | "repeating_command_block"
        | "structure_block" | "jigsaw" | "reinforced_deepslate" => {
            return m(3_600_000.0, Kind::Wall);
        }
        "obsidian" | "crying_obsidian" => return m(18.0, Kind::Rock),
        "sandstone" | "red_sandstone" | "chiseled_sandstone" | "cut_sandstone"
        | "smooth_sandstone" => return m(6.0, Kind::Rock),
        "respawn_anchor" | "netherite_block" | "ancient_debris" | "anvil" | "chipped_anvil"
        | "damaged_anvil" | "enchanting_table" => return m(1200.0, Kind::Other),
        "ender_chest" => return m(600.0, Kind::Other),
        "dirt" | "coarse_dirt" | "rooted_dirt" | "podzol" | "mud" | "muddy_mangrove_roots" => {
            return m(0.5, Kind::Soil);
        }
        "grass_block" | "mycelium" | "farmland" | "clay" => return m(0.6, Kind::Soil),
        "dirt_path" => return m(0.65, Kind::Soil),
        "sand" | "red_sand" | "suspicious_sand" => return g(0.5, Kind::Sand),
        "gravel" | "suspicious_gravel" => return g(0.6, Kind::Soil),
        "soul_sand" | "soul_soil" => return m(0.5, Kind::Soil),
        "snow" => return m(0.1, Kind::Plant),
        "snow_block" | "powder_snow" => return m(0.2, Kind::Soil),
        "ice" | "packed_ice" | "frosted_ice" => return m(0.5, Kind::Other),
        "blue_ice" => return m(2.8, Kind::Other),
        "netherrack" => return m(0.4, Kind::Rock),
        "end_stone" => return m(9.0, Kind::Rock),
        "calcite" => return m(0.75, Kind::Rock),
        "dripstone_block" | "pointed_dripstone" => return m(1.0, Kind::Rock),
        "basalt" | "smooth_basalt" | "polished_basalt" => return m(4.2, Kind::Rock),
        "moss_block" | "moss_carpet" => return m(0.1, Kind::Plant),
        "amethyst_block" | "budding_amethyst" => return m(1.5, Kind::Rock),
        "glowstone" => return m(0.3, Kind::Other),
        _ => {}
    }
    let has = |s: &str| n.contains(s);
    if has("leaves") {
        m(0.2, Kind::Leaves)
    } else if has("_log") || has("_wood") || has("_stem") || has("hyphae") || has("mushroom_block") {
        m(2.0, Kind::Wood)
    } else if has("planks") || has("_fence") || has("_door") || has("bookshelf") || has("chest") || has("crafting_table") {
        m(3.0, Kind::Planks)
    } else if has("glass") {
        m(0.3, Kind::Other)
    } else if has("_ore") {
        m(3.0, Kind::Rock)
    } else if has("concrete_powder") {
        g(0.5, Kind::Sand)
    } else if has("concrete") {
        m(1.8, Kind::Rock)
    } else if has("terracotta") {
        m(4.2, Kind::Rock)
    } else if has("wool") {
        m(0.8, Kind::Other)
    } else if has("carpet") || has("torch") || has("rail") || has("button") || has("pressure_plate") {
        m(0.1, Kind::Plant)
    } else if has("stone") || has("deepslate") || has("tuff") || has("brick") || has("granite")
        || has("diorite") || has("andesite") || has("prismarine") || has("purpur")
        || has("_block") || has("copper") || has("cobble")
    {
        m(6.0, Kind::Rock)
    } else if has("grass") || has("fern") || has("flower") || has("sapling") || has("vine")
        || has("kelp") || has("seagrass") || has("bush") || has("tulip") || has("orchid")
        || has("dandelion") || has("poppy") || has("allium") || has("bluet") || has("daisy")
        || has("lily") || has("rose") || has("peony") || has("lilac") || has("sugar_cane")
        || has("wheat") || has("carrots") || has("potatoes") || has("beetroots") || has("mushroom")
        || has("roots") || has("sprouts") || has("lichen") || has("dripleaf") || has("azalea")
        || has("coral") || has("pickle") || has("bamboo") || has("cactus") || has("pumpkin")
        || has("melon") || has("cocoa") || has("berry") || has("petals") || has("leaf_litter")
    {
        m(0.0, Kind::Plant)
    } else {
        m(3.0, Kind::Other)
    }
}

/// Cache nome → material (FNV-1a de 64 bits; a classificação por padrões só
/// roda na primeira vez que um estado aparece).
#[derive(Default)]
pub struct Mats {
    ids: HashMap<u64, u8, BuildFnv>,
    pub table: Vec<Mat>,
}

#[derive(Default, Clone, Copy)]
pub struct BuildFnv;
pub struct Fnv(u64);
impl std::hash::BuildHasher for BuildFnv {
    type Hasher = Fnv;
    fn build_hasher(&self) -> Fnv {
        Fnv(0xcbf29ce484222325)
    }
}
impl std::hash::Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ b as u64).wrapping_mul(0x100000001b3);
        }
    }
    fn write_u64(&mut self, v: u64) {
        // Chave já é um hash: mistura barata.
        self.0 = v ^ (v >> 29);
    }
}

fn fnv(s: &str) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for &b in s.as_bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    h
}

pub const AIR_ID: u8 = 0;

impl Mats {
    pub fn new() -> Self {
        let mut s = Self::default();
        s.table.push(classify("minecraft:air"));
        s.ids.insert(fnv("minecraft:air"), AIR_ID);
        s
    }
    pub fn id(&mut self, state: &str) -> u8 {
        self.lookup(state)
    }

    /// Material sintético com só o tipo (bordas restauradas do diário).
    pub fn for_kind(&mut self, kind: Kind) -> u8 {
        let key = 0x6b69_6e64_0000_0000u64 | kind as u64;
        if let Some(&id) = self.ids.get(&key) {
            return id;
        }
        let id = self.table.len().min(255) as u8;
        if self.table.len() < 255 {
            self.table.push(Mat { res: 0.0, kind, gravity: false });
        }
        self.ids.insert(key, id);
        id
    }
    fn lookup(&mut self, state: &str) -> u8 {
        // Só o nome do bloco importa (propriedades não mudam a resistência).
        let name = match state.find('[') {
            Some(i) => &state[..i],
            None => state,
        };
        let h = fnv(name);
        if let Some(&id) = self.ids.get(&h) {
            return id;
        }
        let id = self.table.len().min(255) as u8;
        if self.table.len() < 255 {
            self.table.push(classify(state));
        }
        self.ids.insert(h, id);
        id
    }
}


/// Altura máxima de uma coluna (o mundo tem 384 blocos).
pub const MAX_COLUMN: usize = (MAX_Y - MIN_Y + 1) as usize;
/// Blocos de raio horizontal atrás da frente a partir dos quais a energia de
/// uma coluna é devolvida: pais e cantos ficam a < 2,42 (√2 + 1).
pub const RELEASE: f32 = 3.0;
/// Valores `u16` de energia que podem ficar retidos além da frente, perto de
/// chunks que estavam sem carregar (são os pais do recálculo deles): 8 MB.
pub const RETAIN_MAX: usize = 4 << 20;
const NONE: u32 = u32::MAX;
/// `alive` guardado como dy + BIAS (0 = nenhum voxel vivo).
const ALIVE_BIAS: i32 = 1024;

/// Uma coluna do disco: índice, dy mínimo e quantidade (dentro da esfera e do mundo).
#[derive(Clone, Copy, Debug)]
pub struct ColView {
    pub ci: u32,
    pub lo: i32,
    pub len: i32,
}

/// A coluna vizinha (3×3) da coluna em curso, como a varredura a consulta.
#[derive(Clone, Copy)]
struct Nb {
    present: bool,
    lo: i32,
    len: i32,
    /// Buffer de energia (ou NONE: devolvido; usa a aproximação).
    slot: u32,
    alive_lo: i32,
}

const NB_NONE: Nb = Nb { present: false, lo: 0, len: 0, slot: NONE, alive_lo: i16::MAX as i32 };

/// Voxel da borda exposta: lido, não-ar, sem energia para ser destruído, com o
/// voxel de cima ainda com energia (ar ou removido). Só essas são guardadas.
#[derive(Clone, Copy, Debug)]
pub struct Edge {
    pub o: [i16; 3],
    pub mat: u8,
}

pub struct Plan {
    pub c: [i32; 3],
    pub r: i32,
    e0: f32,
    scale: f32,
    inv_r: f32,
    /// Por dx (+r): primeiro índice de coluna da linha e meia-largura em dz.
    row_start: Vec<u32>,
    row_w: Vec<i32>,
    total: usize,
    /// Por coluna: buffer de energia (slot + 1, 0 = nenhum), início das
    /// faixas removidas em `runs`, e (quantas faixas << 16 | alive).
    col_slot: Vec<u32>,
    col_runs: Vec<u32>,
    col_meta: Vec<u32>,
    /// Faixas removidas (início, comprimento), em offsets da coluna.
    runs: Vec<(u16, u16)>,
    bufs: Vec<Vec<u16>>,
    free: Vec<u32>,
    /// Colunas da passada principal com buffer, em ordem de espiral.
    live: std::collections::VecDeque<u32>,
    /// Valores retidos além da frente (recálculo de chunks descarregados).
    retained: usize,
    pub edges: Vec<Edge>,
    pub mats: Mats,
    /// Espiral gerada anel a anel: anel atual (colunas em ordem), posição nele
    /// e o próximo anel a gerar.
    ring: Vec<u32>,
    ring_pos: usize,
    ring_k: i32,
    pub cols_done: u32,
    /// Colunas a recalcular depois da passada principal (chunks que estavam
    /// sem carregar), em ordem de espiral, e quantas já foram.
    queue: Vec<u32>,
    pub queue_done: usize,
    cur_dy: i32,
    cur_floor: i32,
    /// Coluna em curso: vizinhas 3×3 (centro = 4), remoções e menor dy vivo.
    nb: [Nb; 9],
    cur_rm: [u64; MAX_COLUMN.div_ceil(64)],
    cur_alive: i32,
    pub reads: u64,
    pub visited: u64,
    /// Voxels pulados pela poda do subsolo morto.
    pub skipped: u64,
    pub removed: u64,
    pub unloaded: u64,
    /// Leituras de energia de uma coluna já devolvida (aproximadas). Zero
    /// numa varredura sem chunks descarregados.
    pub approx: u64,
    /// Chunks (x>>4, z>>4) que estavam sem carregar quando a casca passou.
    pub unloaded_chunks: Vec<(i32, i32)>,
    unloaded_set: std::collections::HashSet<(i32, i32), BuildFnv>,
    last_unloaded: Option<(i32, i32)>,
    /// Remoções por seção 16³ (grade densa a partir de `sec0`).
    pub sec0: [i32; 3],
    pub sec_dim: [i32; 3],
    pub sec_count: Vec<u32>,
    /// Máximo entre os pais vizinhos (ver `eval`).
    pub wide: bool,
    /// Peso da energia interpolada contra a do pai arredondado (calibrado
    /// contra o NTM no benchmark nativo).
    pub blend: f32,
    costs: Vec<Vec<f32>>,
    /// Diário (persistência): com `Some`, cada coluna terminada é anexada
    /// como registro (ver `persist.rs`); o motor esvazia a cada tick.
    pub journal: Option<crate::persist::Journal>,
    col_edges: usize,
    /// Voxels do fundo da coluna em curso pulados pela poda (energia 0, nada removido).
    col_dead: usize,
    /// Fuel estimado de trabalho que não é visita nem leitura (início de coluna).
    extra: u64,
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

impl Plan {
    pub fn new(c: [i32; 3], r: i32) -> Self {
        let r = r.clamp(1, MAX_RADIUS);
        // O centro fica no mundo: toda coluna do disco tem pelo menos um voxel.
        let c = [c[0], c[1].clamp(MIN_Y, MAX_Y), c[2]];
        let side = (2 * r + 1) as usize;
        let mut row_start = Vec::with_capacity(side + 1);
        let mut row_w = Vec::with_capacity(side);
        let mut total = 0usize;
        for dx in -r..=r {
            let w = isqrt(i64::from(r * r - dx * dx));
            row_start.push(total as u32);
            row_w.push(w);
            total += (2 * w + 1) as usize;
        }
        row_start.push(total as u32);
        let sec0 = [(c[0] - r) >> 4, (c[1] - r).max(MIN_Y) >> 4, (c[2] - r) >> 4];
        let sec_dim = [
            ((c[0] + r) >> 4) - sec0[0] + 1,
            ((c[1] + r).min(MAX_Y) >> 4) - sec0[1] + 1,
            ((c[2] + r) >> 4) - sec0[2] + 1,
        ];
        let e0 = 2.0 * r as f32;
        let mut p = Self {
            sec0,
            sec_dim,
            sec_count: vec![0; (sec_dim[0] * sec_dim[1] * sec_dim[2]) as usize],
            wide: true,
            blend: 0.3,
            costs: Vec::new(),
            journal: None,
            col_edges: 0,
            col_dead: 0,
            extra: 0,
            c,
            r,
            e0,
            // E0·scale tem que caber em u16 (R > 327).
            scale: SCALE.min(65_000.0 / e0),
            inv_r: 1.0 / r as f32,
            row_start,
            row_w,
            total,
            // Zeros: memória nova, sem custo de laço (alloc_zeroed).
            col_slot: vec![0; total],
            col_runs: vec![0; total],
            col_meta: vec![0; total],
            runs: Vec::new(),
            bufs: Vec::new(),
            free: Vec::new(),
            live: Default::default(),
            retained: 0,
            edges: Vec::new(),
            mats: Mats::new(),
            ring: Vec::new(),
            ring_pos: 0,
            ring_k: 0,
            cols_done: 0,
            queue: Vec::new(),
            queue_done: 0,
            cur_dy: 0,
            cur_floor: i32::MIN,
            nb: [NB_NONE; 9],
            cur_rm: [0; MAX_COLUMN.div_ceil(64)],
            cur_alive: i16::MAX as i32,
            reads: 0,
            visited: 0,
            skipped: 0,
            removed: 0,
            unloaded: 0,
            approx: 0,
            unloaded_chunks: Vec::new(),
            unloaded_set: Default::default(),
            last_unloaded: None,
        };
        p.advance_ring();
        p
    }

    // ------------------------------------------------------------ colunas

    /// A coluna (dx, dz) do disco, ou None fora dele.
    #[inline]
    pub fn col(&self, dx: i32, dz: i32) -> Option<ColView> {
        if dx.abs() > self.r {
            return None;
        }
        let row = (dx + self.r) as usize;
        let w = self.row_w[row];
        if dz.abs() > w {
            return None;
        }
        let h = ((self.r * self.r - dx * dx - dz * dz) as f32).sqrt() as i32;
        let lo = (-h).max(MIN_Y - self.c[1]);
        let hi = h.min(MAX_Y - self.c[1]);
        Some(ColView { ci: self.row_start[row] + (dz + w) as u32, lo, len: hi - lo + 1 })
    }

    /// (dx, dz) da coluna `ci`.
    pub fn coords(&self, ci: usize) -> (i32, i32) {
        let row = self.row_start.partition_point(|&s| s as usize <= ci) - 1;
        let dx = row as i32 - self.r;
        (dx, ci as i32 - self.row_start[row] as i32 - self.row_w[row])
    }

    fn view(&self, ci: usize) -> ColView {
        let (dx, dz) = self.coords(ci);
        self.col(dx, dz).expect("coluna do disco")
    }

    /// Faixa de dy de uma coluna dentro da esfera e do mundo.
    pub fn column(&self, dx: i32, dz: i32) -> Option<(i32, i32)> {
        self.col(dx, dz).map(|c| (c.lo, c.lo + c.len - 1))
    }

    /// Ordem da espiral: h², depois o índice (o mesmo que o counting sort antigo).
    #[inline]
    fn key_of(&self, dx: i32, dz: i32, ci: u32) -> u64 {
        ((dx * dx + dz * dz) as u64) << 32 | u64::from(ci)
    }

    pub fn key(&self, ci: usize) -> u64 {
        let (dx, dz) = self.coords(ci);
        self.key_of(dx, dz, ci as u32)
    }

    /// Colunas do anel k (k² ≤ h² < (k+1)²), na ordem da espiral.
    fn ring_cols(&self, k: i32) -> Vec<u32> {
        let mut out: Vec<(u64, u32)> = Vec::new();
        let (k2, k12) = (i64::from(k) * i64::from(k), i64::from(k + 1) * i64::from(k + 1));
        for dx in -(k + 1).min(self.r)..=(k + 1).min(self.r) {
            let dx2 = i64::from(dx) * i64::from(dx);
            let w = self.row_w[(dx + self.r) as usize];
            let lo2 = (k2 - dx2).max(0);
            let mut dz0 = isqrt(lo2);
            if i64::from(dz0) * i64::from(dz0) < lo2 {
                dz0 += 1;
            }
            if k12 - dx2 <= 0 {
                continue;
            }
            let dz1 = isqrt(k12 - dx2 - 1).min(w);
            for dz in dz0..=dz1 {
                for dz in if dz == 0 { [0, 0] } else { [-dz, dz] } {
                    if let Some(c) = self.col(dx, dz) {
                        out.push((self.key_of(dx, dz, c.ci), c.ci));
                    }
                }
                if dz == 0 {
                    out.pop();
                }
            }
        }
        out.sort_unstable_by_key(|e| e.0);
        out.into_iter().map(|e| e.1).collect()
    }

    fn ring_size(&self, k: i32) -> usize {
        let (k2, k12) = (i64::from(k) * i64::from(k), i64::from(k + 1) * i64::from(k + 1));
        let mut n = 0;
        for dx in -(k + 1).min(self.r)..=(k + 1).min(self.r) {
            let dx2 = i64::from(dx) * i64::from(dx);
            let w = self.row_w[(dx + self.r) as usize];
            let lo2 = (k2 - dx2).max(0);
            let mut dz0 = isqrt(lo2);
            if i64::from(dz0) * i64::from(dz0) < lo2 {
                dz0 += 1;
            }
            if k12 - dx2 <= 0 {
                continue;
            }
            let dz1 = isqrt(k12 - dx2 - 1).min(w);
            if dz1 >= dz0 {
                n += (2 * (dz1 - dz0 + 1) - i32::from(dz0 == 0)) as usize;
            }
        }
        n
    }

    /// Garante que `ring[ring_pos]` é a próxima coluna da passada principal.
    fn advance_ring(&mut self) {
        while self.ring_pos >= self.ring.len() && self.ring_k <= self.r {
            self.extra += 40 * self.ring_k as u64 + 200;
            self.ring = self.ring_cols(self.ring_k);
            self.ring_k += 1;
            self.ring_pos = 0;
        }
    }

    /// h² da próxima coluna da passada principal (u64::MAX se acabou): toda
    /// coluna de h² menor já está calculada.
    pub fn front_h2(&self) -> u64 {
        match self.ring.get(self.ring_pos) {
            Some(&ci) if !self.finished() => self.key(ci as usize) >> 32,
            _ => u64::MAX,
        }
    }

    fn front_key(&self) -> u64 {
        match self.ring.get(self.ring_pos) {
            Some(&ci) if !self.finished() => self.key(ci as usize),
            _ => u64::MAX,
        }
    }

    /// A coluna já foi calculada na passada principal?
    pub fn done(&self, ci: usize) -> bool {
        self.key(ci) < self.front_key()
    }

    /// Maior h² das colunas de uma seção (de um chunk), limitado a R²: a
    /// seção está decidida quando `front_h2` passa dele.
    pub fn section_h2(&self, s: [i32; 3]) -> u64 {
        let mut m = 0i64;
        for x in [s[0] * 16, s[0] * 16 + 15] {
            for z in [s[2] * 16, s[2] * 16 + 15] {
                let (dx, dz) = (i64::from(x - self.c[0]), i64::from(z - self.c[2]));
                m = m.max(dx * dx + dz * dz);
            }
        }
        m.min(i64::from(self.r) * i64::from(self.r)) as u64
    }

    pub fn sec_index(&self, x: i32, y: i32, z: i32) -> Option<usize> {
        let (a, b, c) = ((x >> 4) - self.sec0[0], (y >> 4) - self.sec0[1], (z >> 4) - self.sec0[2]);
        if a < 0 || b < 0 || c < 0 || a >= self.sec_dim[0] || b >= self.sec_dim[1] || c >= self.sec_dim[2] {
            return None;
        }
        Some(((a * self.sec_dim[1] + b) * self.sec_dim[2] + c) as usize)
    }

    fn count(&mut self, x: i32, y: i32, z: i32, delta: i32) {
        if let Some(s) = self.sec_index(x, y, z) {
            self.sec_count[s] = (self.sec_count[s] as i32 + delta) as u32;
        }
    }

    /// `count` de `n` voxels seguidos da coluna (x, z) a partir de y, uma
    /// soma por seção (restaurar o diário de R=120 são ~240 mil voxels).
    fn count_run(&mut self, x: i32, mut y: i32, z: i32, mut n: i32, delta: i32) {
        while n > 0 {
            let take = n.min(((y >> 4) + 1) * 16 - y);
            if let Some(s) = self.sec_index(x, y, z) {
                self.sec_count[s] = (self.sec_count[s] as i32 + delta * take) as u32;
            }
            y += take;
            n -= take;
        }
    }

    /// Faixas removidas da coluna `ci` (início, comprimento), em offsets.
    pub fn col_runs(&self, ci: usize) -> &[(u16, u16)] {
        let n = (self.col_meta[ci] >> 16) as usize;
        let at = self.col_runs[ci] as usize;
        &self.runs[at..at + n]
    }

    /// Energia da coluna `ci`, enquanto ela tem buffer.
    pub fn col_energy(&self, ci: usize) -> Option<&[u16]> {
        match self.col_slot[ci] {
            0 => None,
            s => Some(&self.bufs[s as usize - 1]),
        }
    }

    /// Menor dy vivo da coluna (i16::MAX: nenhum).
    pub fn alive_lo_of(&self, ci: usize) -> i32 {
        match self.col_meta[ci] & 0xffff {
            0 => i16::MAX as i32,
            a => a as i32 - ALIVE_BIAS,
        }
    }

    fn set_alive(&mut self, ci: usize, alive: i32) {
        let a = if alive >= i16::MAX as i32 { 0 } else { (alive + ALIVE_BIAS) as u32 };
        self.col_meta[ci] = (self.col_meta[ci] & 0xffff_0000) | a;
    }

    fn set_runs(&mut self, ci: usize, runs: &[(u16, u16)]) {
        self.col_runs[ci] = self.runs.len() as u32;
        self.runs.extend_from_slice(runs);
        self.col_meta[ci] = (self.col_meta[ci] & 0xffff) | ((runs.len() as u32) << 16);
    }

    /// Apaga as remoções da coluna (e as contagens das seções).
    fn clear_runs(&mut self, ci: usize, dx: i32, dz: i32, lo: i32) {
        let runs: Vec<(u16, u16)> = self.col_runs(ci).to_vec();
        let (x, z) = (self.c[0] + dx, self.c[2] + dz);
        for (start, n) in runs {
            self.count_run(x, self.c[1] + lo + start as i32, z, i32::from(n), -1);
            self.removed -= u64::from(n);
        }
        self.col_meta[ci] &= 0xffff;
    }

    /// Voxels a remover numa seção, de cima para baixo (na mesma altura, x e
    /// z crescentes): 16 baldes de altura, sem ordenação.
    pub fn section_cells(&self, s: [i32; 3]) -> Vec<[i32; 3]> {
        if self.sec_index(s[0] * 16, s[1] * 16, s[2] * 16).is_some_and(|i| self.sec_count[i] == 0) {
            return Vec::new();
        }
        let (y0, y1) = (s[1] * 16 - self.c[1], s[1] * 16 + 15 - self.c[1]);
        let mut by_y: [Vec<[i32; 3]>; 16] = Default::default();
        let mut n = 0;
        for x in s[0] * 16..s[0] * 16 + 16 {
            for z in s[2] * 16..s[2] * 16 + 16 {
                let Some(col) = self.col(x - self.c[0], z - self.c[2]) else { continue };
                for &(start, len) in self.col_runs(col.ci as usize) {
                    let (a, b) = ((col.lo + start as i32).max(y0), (col.lo + start as i32 + len as i32 - 1).min(y1));
                    for dy in a..=b {
                        by_y[(dy - y0) as usize].push([x, self.c[1] + dy, z]);
                        n += 1;
                    }
                }
            }
        }
        let mut cells = Vec::with_capacity(n);
        for level in by_y.iter_mut().rev() {
            cells.append(level);
        }
        cells
    }

    /// Voxels do disco (soma das colunas): só para relatórios.
    pub fn voxels(&self) -> usize {
        let mut n = 0;
        for dx in -self.r..=self.r {
            let w = self.row_w[(dx + self.r) as usize];
            for dz in -w..=w {
                n += self.col(dx, dz).map_or(0, |c| c.len as usize);
            }
        }
        n
    }

    // ------------------------------------------------------ buffers

    fn alloc(&mut self, ci: usize, len: usize) -> u32 {
        if self.col_slot[ci] != 0 {
            let s = self.col_slot[ci] - 1;
            let b = &mut self.bufs[s as usize];
            b.clear();
            b.resize(len, 0);
            return s;
        }
        let s = match self.free.pop() {
            Some(s) => {
                let b = &mut self.bufs[s as usize];
                b.clear();
                b.resize(len, 0);
                s
            }
            None => {
                self.bufs.push(vec![0; len]);
                (self.bufs.len() - 1) as u32
            }
        };
        self.col_slot[ci] = s + 1;
        s
    }

    fn release(&mut self, ci: usize) {
        let s = self.col_slot[ci];
        if s != 0 {
            self.col_slot[ci] = 0;
            self.free.push(s - 1);
        }
    }

    /// A coluna é pai possível do recálculo de um chunk que estava sem carregar?
    fn near_unloaded(&self, dx: i32, dz: i32) -> bool {
        if self.unloaded_set.is_empty() {
            return false;
        }
        let (x, z) = (self.c[0] + dx, self.c[2] + dz);
        let m = RELEASE as i32;
        ((x - m) >> 4..=(x + m) >> 4).any(|a| ((z - m) >> 4..=(z + m) >> 4).any(|b| self.unloaded_set.contains(&(a, b))))
    }

    /// A frente passou da coluna mais antiga com buffer: devolve (ou retém,
    /// perto de chunk descarregado, até [`RETAIN_MAX`]).
    fn release_behind(&mut self, h_front: f32) {
        while let Some(&ci) = self.live.front() {
            let (dx, dz) = self.coords(ci as usize);
            let h = ((dx * dx + dz * dz) as f32).sqrt();
            if h + RELEASE >= h_front {
                break;
            }
            self.live.pop_front();
            self.drop_or_retain(ci as usize, dx, dz);
        }
    }

    fn drop_or_retain(&mut self, ci: usize, dx: i32, dz: i32) {
        let len = self.col_energy(ci).map_or(0, <[u16]>::len);
        if self.near_unloaded(dx, dz) && self.retained + len <= RETAIN_MAX {
            self.retained += len;
        } else {
            self.release(ci);
        }
    }

    /// Devolve toda energia guardada (plano e recálculos terminados).
    pub fn release_all(&mut self) {
        self.live.clear();
        for ci in 0..self.total {
            self.release(ci);
        }
        self.retained = 0;
    }

    /// Bytes em uso pelo plano (estimativa: vetores e buffers).
    pub fn memory(&self) -> usize {
        self.total * 12
            + self.row_start.len() * 8
            + self.runs.capacity() * 4
            + self.bufs.iter().map(|b| b.capacity() * 2 + 24).sum::<usize>()
            + self.edges.capacity() * std::mem::size_of::<Edge>()
            + self.sec_count.len() * 4
            + self.ring.capacity() * 4
            + self.queue.capacity() * 4
    }

    /// Buffers de energia vivos agora.
    pub fn live_buffers(&self) -> usize {
        self.bufs.len() - self.free.len()
    }

    /// `res^(0.5 + 3.5·i/R)` com i = passo inteiro do raio (como no NTM),
    /// tabelado por material: `powf` em WebAssembly é uma rotina de software cara.
    fn step_cost(&mut self, id: u8, d: f32) -> f32 {
        let i = ((d + 0.5) as usize).min(self.r as usize);
        let id = id as usize;
        if self.costs.len() <= id {
            self.costs.resize(id + 1, Vec::new());
        }
        if self.costs[id].is_empty() {
            let res = self.mats.table[id].res;
            self.costs[id] = (0..=self.r).map(|i| res.powf(0.5 + 3.5 * i as f32 * self.inv_r)).collect();
        }
        self.costs[id][i]
    }

    /// Energia de um voxel de uma das 3×3 colunas (`k`), com `off` já
    /// dentro dela. Coluna devolvida: aproximação (vivo ⇒ energia da reta
    /// sem resistência), contada em `approx`.
    #[inline]
    fn nb_energy(&mut self, k: usize, off: i32, d_parent: f32) -> u16 {
        let n = self.nb[k];
        if n.slot != NONE {
            return self.bufs[n.slot as usize][off as usize];
        }
        self.approx += 1;
        if n.lo + off >= n.alive_lo {
            ((self.e0 * (1.0 - d_parent * self.inv_r)).max(0.0) * self.scale) as u16
        } else {
            0
        }
    }

    /// Avalia um voxel da coluna em curso (offset `off`); retorna true se houve leitura.
    fn eval_i(&mut self, off: usize, dx: i32, dy: i32, dz: i32, read: &mut impl FnMut(i32, i32, i32) -> Option<String>) -> bool {
        self.visited += 1;
        let d2 = (dx * dx + dy * dy + dz * dz) as f32;
        let center = self.nb[4].slot as usize;
        let (parent_e, d) = if d2 == 0.0 {
            (self.e0, 0.0)
        } else {
            let d = d2.sqrt();
            let f = (d - 1.0) / d;
            let (fx, fy, fz) = (dx as f32 * f, dy as f32 * f, dz as f32 * f);
            let mut pe = 0u16;
            if !(self.wide && dy < 0) {
                // Acima do centro o pai arredondado basta (no benchmark nativo
                // os dois modelos diferem do NTM em ≤ 200 blocos ali).
                let (px, py, pz) = ((fx + 0.5).floor() as i32, (fy + 0.5).floor() as i32, (fz + 0.5).floor() as i32);
                let (ox, oz) = (px - dx, pz - dz);
                if ox.abs() <= 1 && oz.abs() <= 1 {
                    let k = ((ox + 1) * 3 + oz + 1) as usize;
                    let n = self.nb[k];
                    let off = py - n.lo;
                    if n.present && off >= 0 && off < n.len {
                        pe = self.nb_energy(k, off, d - 1.0);
                    }
                }
            } else {
                let (x0, y0, z0) = (fx.floor(), fy.floor(), fz.floor());
                let (tx, ty, tz) = (fx - x0, fy - y0, fz - z0);
                let (x0, y0, z0) = (x0 as i32, y0 as i32, z0 as i32);
                // Pai arredondado: um dos 8 cantos.
                let (rx, ry, rz) = ((tx >= 0.5) as i32, (ty >= 0.5) as i32, (tz >= 0.5) as i32);
                let (mut acc, mut wsum) = (0.0f32, 0.0f32);
                // 4 colunas (x,z), todas entre as 3×3 vizinhas, 2 alturas cada.
                for ox in 0..2 {
                    let wx = if ox == 0 { 1.0 - tx } else { tx };
                    for oz in 0..2 {
                        let wz = if oz == 0 { 1.0 - tz } else { tz };
                        let (ax, az) = (x0 + ox - dx, z0 + oz - dz);
                        if ax.abs() > 1 || az.abs() > 1 {
                            continue; // peso 0 e não é o arredondado (ver docs)
                        }
                        let k = ((ax + 1) * 3 + az + 1) as usize;
                        let col = self.nb[k];
                        if !col.present {
                            continue;
                        }
                        let same = k == 4;
                        for oy in 0..2 {
                            let cy = y0 + oy;
                            let off = cy - col.lo;
                            // Só cantos já calculados: outra coluna (sempre mais perto
                            // do centro) ou, na mesma, acima deste voxel (dy < 0 aqui).
                            if off < 0 || off >= col.len || (same && cy <= dy) {
                                continue;
                            }
                            let w = wx * wz * if oy == 0 { 1.0 - ty } else { ty };
                            let rounded = (ox, oy, oz) == (rx, ry, rz);
                            if !rounded && w <= 1e-4 {
                                continue; // canto de peso 0 (às vezes ainda não calculado)
                            }
                            let ev = self.nb_energy(k, off, d - 1.0);
                            if rounded {
                                pe = ev;
                            }
                            if w > 1e-4 {
                                acc += w * ev as f32;
                                wsum += w;
                            }
                        }
                    }
                }
                if wsum > 0.0 {
                    // Vários raios do NTM passam por cada bloco: além do pai
                    // arredondado, a energia no ponto exato da reta um passo
                    // antes, interpolada (trilinear) entre os vizinhos já
                    // calculados (casca menor), misturada com peso `blend`.
                    let tri = acc / wsum;
                    pe = (self.blend * tri + (1.0 - self.blend) * pe as f32) as u16;
                }
            }
            (pe as f32 / self.scale, d)
        };
        if parent_e <= 0.0 {
            self.bufs[center][off] = 0;
            return false;
        }
        let (x, y, z) = (self.c[0] + dx, self.c[1] + dy, self.c[2] + dz);
        self.reads += 1;
        let Some(state) = read(x, y, z) else {
            // Sem chunk: a energia atravessa como ar; o chunk fica marcado.
            self.unloaded += 1;
            let key = (x >> 4, z >> 4);
            if self.last_unloaded != Some(key) {
                self.last_unloaded = Some(key);
                if self.unloaded_set.insert(key) {
                    self.unloaded_chunks.push(key);
                }
            }
            self.bufs[center][off] = (parent_e * self.scale) as u16;
            self.cur_alive = self.cur_alive.min(dy);
            return true;
        };
        // Caminho rápido: a maioria das leituras é ar (sem hash).
        let b = state.as_bytes();
        let id = if (b.len() == 13 || b.len() == 18) && b[b.len() - 3..] == *b"air" && b[b.len() - 4] != b'_' || b.len() == 18 && &b[10..14] == b"cave" {
            AIR_ID
        } else {
            self.mats.id(&state)
        };
        let mat = self.mats.table[id as usize];
        let e = match mat.kind {
            Kind::Air => parent_e,
            Kind::Liquid => parent_e,
            _ => parent_e - self.step_cost(id, d),
        };
        if e > 0.0 {
            self.bufs[center][off] = ((e * self.scale) as u32).clamp(1, u16::MAX as u32) as u16;
            self.cur_alive = self.cur_alive.min(dy);
            if mat.kind != Kind::Air {
                self.cur_rm[off >> 6] |= 1 << (off & 63);
                self.removed += 1;
                self.count(x, y, z, 1);
            }
        } else {
            self.bufs[center][off] = 0;
            if mat.kind != Kind::Air && mat.kind != Kind::Liquid {
                self.edges.push(Edge {
                    o: [dx as i16, dy as i16, dz as i16],
                    mat: id,
                });
            }
        }
        true
    }

    /// Custo estimado em fuel (≈ instruções WebAssembly) do trabalho feito até
    /// agora, para não estourar os 50 M de fuel do tick (o mod seria desligado).
    /// Calibrado pelas rampas do spike S7 (docs/explosoes.md): ~600–800 por
    /// voxel lido no jogo, conforme o terreno.
    pub fn cost(&self) -> u64 {
        self.visited * COST_VISIT + self.reads * COST_READ + self.extra
    }

    pub fn columns(&self) -> usize {
        self.total
    }

    pub fn finished(&self) -> bool {
        self.cols_done as usize >= self.total
    }

    /// Começa a coluna `ci`: buffer, vizinhas 3×3 e piso do subsolo vivo.
    fn begin_column(&mut self, ci: usize, main: bool) {
        let (dx, dz) = self.coords(ci);
        let col = self.view(ci);
        if main {
            let h = ((dx * dx + dz * dz) as f32).sqrt();
            self.release_behind(h);
        }
        let slot = self.alloc(ci, col.len as usize);
        let cur_key = self.key_of(dx, dz, ci as u32);
        // Piso pelas 3×3 vizinhas já prontas (todo pai está a ≤ 1 bloco na
        // horizontal e ≤ 2 acima).
        let mut m = i16::MAX as i32;
        for ox in -1..=1 {
            for oz in -1..=1 {
                let k = ((ox + 1) * 3 + oz + 1) as usize;
                let Some(c) = self.col(dx + ox, dz + oz) else {
                    self.nb[k] = NB_NONE;
                    continue;
                };
                let alive = self.alive_lo_of(c.ci as usize);
                let s = self.col_slot[c.ci as usize];
                self.nb[k] = Nb { present: true, lo: c.lo, len: c.len, slot: if s == 0 { NONE } else { s - 1 }, alive_lo: alive };
                if k != 4 && (!main || self.key_of(dx + ox, dz + oz, c.ci) < cur_key) {
                    m = m.min(alive);
                }
            }
        }
        self.nb[4].slot = slot;
        self.cur_floor = m;
        self.cur_alive = i16::MAX as i32;
        self.cur_rm = [0; MAX_COLUMN.div_ceil(64)];
        self.cur_dy = 0.max(col.lo);
        self.col_edges = self.edges.len();
        self.col_dead = 0;
        self.extra += col.len as u64 * 3 + COST_COLUMN;
    }

    /// Fecha a coluna `ci`: bordas expostas, faixas removidas, diário.
    fn end_column(&mut self, ci: usize, main: bool) {
        let col = self.view(ci);
        let slot = self.nb[4].slot as usize;
        // Bordas: só as expostas (o voxel de cima, na mesma coluna, tem energia).
        let mut w = self.col_edges;
        for i in self.col_edges..self.edges.len() {
            let e = self.edges[i];
            let off = e.o[1] as i32 + 1 - col.lo;
            if off >= 0 && off < col.len && self.bufs[slot][off as usize] > 0 {
                self.edges[w] = e;
                w += 1;
            }
        }
        self.edges.truncate(w);
        // Faixas removidas.
        let mut runs: Vec<(u16, u16)> = Vec::new();
        let mut off = 0usize;
        let len = col.len as usize;
        while off < len {
            if self.cur_rm[off >> 6] >> (off & 63) & 1 == 1 {
                let start = off;
                while off < len && self.cur_rm[off >> 6] >> (off & 63) & 1 == 1 {
                    off += 1;
                }
                runs.push((start as u16, (off - start) as u16));
            } else {
                off += 1;
            }
        }
        self.set_runs(ci, &runs);
        self.set_alive(ci, self.cur_alive);
        if self.journal.is_some() {
            let mut j = self.journal.take().unwrap();
            crate::persist::encode_column(self, ci, self.col_edges, self.col_dead, main, &mut j);
            self.journal = Some(j);
            self.extra += (len - self.col_dead) as u64 * 48;
        }
        if main {
            self.live.push_back(ci as u32);
            self.cols_done += 1;
            self.ring_pos += 1;
            self.advance_ring();
            if self.finished() {
                // Fim da passada principal: só fica o que o recálculo pode usar.
                while let Some(ci) = self.live.pop_front() {
                    let (dx, dz) = self.coords(ci as usize);
                    self.drop_or_retain(ci as usize, dx, dz);
                }
            }
        } else {
            self.queue_done += 1;
        }
        self.cur_floor = i32::MIN;
    }

    /// Avança a varredura (colunas em espiral; em cada uma, dy = 0, 1, 2… e
    /// depois −1, −2…) até gastar `max_cost`.
    pub fn step(&mut self, max_cost: u64, read: &mut impl FnMut(i32, i32, i32) -> Option<String>) {
        let cost0 = self.cost();
        loop {
            let main = !self.finished();
            let ci = if main {
                self.ring[self.ring_pos] as usize
            } else if self.queue_done < self.queue.len() {
                self.queue[self.queue_done] as usize
            } else {
                return;
            };
            if self.cur_floor == i32::MIN {
                self.begin_column(ci, main);
            }
            let (dx, dz) = self.coords(ci);
            let (lo, len) = (self.nb[4].lo, self.nb[4].len);
            let hi = lo + len - 1;
            loop {
                if self.cost() - cost0 >= max_cost {
                    return;
                }
                let y = self.cur_dy;
                if y >= 0 {
                    if y > hi {
                        // Fim da subida; começa a descida em −1.
                        self.cur_dy = -1;
                        continue;
                    }
                    self.cur_dy += 1;
                } else {
                    // Descida: abaixo do piso (vizinhas e a própria coluna) tudo está morto.
                    let floor = self.cur_floor.min(self.cur_alive) - 2;
                    if y < lo || y < floor {
                        if y >= lo {
                            self.skipped += (y - lo + 1) as u64;
                            self.col_dead = (y - lo + 1) as usize;
                        }
                        break;
                    }
                    self.cur_dy -= 1;
                }
                self.eval_i((y - lo) as usize, dx, y, dz, read);
            }
            self.end_column(ci, main);
        }
    }

    /// Põe as colunas de um chunk (que estava sem carregar) na fila de
    /// recálculo, mais perto do centro primeiro; devolve a posição na fila em
    /// que o chunk termina (`queue_done` ≥ ela = pronto). As marcas de remoção
    /// antigas da coluna são apagadas agora.
    pub fn reeval_begin(&mut self, chunk: (i32, i32)) -> usize {
        let mut cols: Vec<(u64, u32)> = Vec::new();
        for x in chunk.0 * 16..chunk.0 * 16 + 16 {
            for z in chunk.1 * 16..chunk.1 * 16 + 16 {
                let (dx, dz) = (x - self.c[0], z - self.c[2]);
                let Some(col) = self.col(dx, dz) else { continue };
                let ci = col.ci as usize;
                self.clear_runs(ci, dx, dz, col.lo);
                self.set_alive(ci, i16::MAX as i32);
                self.extra += col.len as u64 * 20;
                cols.push((self.key_of(dx, dz, col.ci), col.ci));
            }
        }
        cols.sort_unstable_by_key(|c| c.0);
        self.queue.extend(cols.into_iter().map(|c| c.1));
        self.queue.len()
    }

    /// Distância horizontal da coluna ao centro (para o anel de energia).
    pub(crate) fn column_h(&self, ci: usize) -> f32 {
        let (dx, dz) = self.coords(ci);
        ((dx * dx + dz * dz) as f32).sqrt()
    }

    /// Restaura a estrutura de uma coluna do diário: `alive_lo`, faixas
    /// removidas (pulo, comprimento) e bordas expostas.
    pub(crate) fn restore_column(&mut self, ci: usize, alive_lo: i32, removed: &[(u32, u32)], edges: &[(i32, Kind)]) -> Result<(), String> {
        if ci >= self.total {
            return Err("coluna fora do disco".into());
        }
        let col = self.view(ci);
        let (dx, dz) = self.coords(ci);
        let (x, z) = (self.c[0] + dx, self.c[2] + dz);
        self.clear_runs(ci, dx, dz, col.lo);
        self.set_alive(ci, alive_lo.clamp(i16::MIN as i32, i16::MAX as i32));
        let mut runs = Vec::new();
        let mut pos = 0usize;
        for &(skip, n) in removed {
            pos += skip as usize;
            if pos + n as usize > col.len as usize {
                return Err("remoção fora da coluna".into());
            }
            runs.push((pos as u16, n as u16));
            self.count_run(x, self.c[1] + col.lo + pos as i32, z, n as i32, 1);
            self.removed += u64::from(n);
            pos += n as usize;
        }
        self.set_runs(ci, &runs);
        for &(dy, kind) in edges {
            let mat = self.mats.for_kind(kind);
            self.edges.push(Edge { o: [dx as i16, dy as i16, dz as i16], mat });
        }
        Ok(())
    }

    /// Restaura a energia exata de uma coluna (registro do anel).
    pub(crate) fn restore_energy(&mut self, ci: usize, runs: &[(u32, u16)]) -> Result<(), String> {
        if ci >= self.total {
            return Err("coluna fora do disco".into());
        }
        let len = self.view(ci).len as usize;
        if runs.iter().map(|r| r.0 as usize).sum::<usize>() != len {
            return Err("energia com tamanho errado".into());
        }
        if self.col_slot[ci] == 0 {
            self.alloc(ci, len);
        }
        let s = self.col_slot[ci] as usize - 1;
        let mut off = 0usize;
        for &(n, v) in runs {
            for k in 0..n as usize {
                let e = &mut self.bufs[s][off + k];
                *e = (*e).max(v);
            }
            off += n as usize;
        }
        Ok(())
    }

    /// Chunk (x>>4, z>>4) da coluna.
    pub fn chunk_of(&self, ci: usize) -> (i32, i32) {
        let (dx, dz) = self.coords(ci);
        ((self.c[0] + dx) >> 4, (self.c[2] + dz) >> 4)
    }

    /// Marca as primeiras `n` colunas da espiral como calculadas (restauração).
    pub(crate) fn set_cols_done(&mut self, n: u32) {
        let n = (n as usize).min(self.total);
        let (mut k, mut count) = (0, 0usize);
        while k <= self.r {
            let size = self.ring_size(k);
            if count + size > n {
                break;
            }
            count += size;
            k += 1;
        }
        self.ring = if k <= self.r { self.ring_cols(k) } else { Vec::new() };
        self.ring_pos = n - count;
        self.ring_k = k + 1;
        self.cols_done = n as u32;
        self.advance_ring();
        // A frente: as colunas restauradas com energia, em ordem de espiral.
        let mut live: Vec<(u64, u32)> = (0..self.total).filter(|&ci| self.col_slot[ci] != 0).map(|ci| (self.key(ci), ci as u32)).collect();
        live.sort_unstable();
        self.live = live.into_iter().map(|c| c.1).collect();
        self.cur_floor = i32::MIN;
    }

    pub(crate) fn mark_unloaded(&mut self, key: (i32, i32)) {
        if self.unloaded_set.insert(key) {
            self.unloaded_chunks.push(key);
        }
    }

    /// O chunk tinha voxel sem carregar na passada principal?
    pub fn was_unloaded(&self, key: (i32, i32)) -> bool {
        self.unloaded_set.contains(&key)
    }

    /// Próxima coluna que a varredura vai calcular (passada principal ou fila).
    pub fn next_column(&self) -> Option<usize> {
        if !self.finished() {
            self.ring.get(self.ring_pos).map(|&c| c as usize)
        } else {
            self.queue.get(self.queue_done).map(|&c| c as usize)
        }
    }

    pub fn queue_idle(&self) -> bool {
        self.queue_done >= self.queue.len()
    }
}

/// Hash determinístico de posição → [0, 1).
pub fn rand01(x: i32, y: i32, z: i32, salt: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6b343)
        ^ (y as u32).wrapping_mul(0xd8163841)
        ^ (z as u32).wrapping_mul(0xcb1ab31f)
        ^ salt.wrapping_mul(0x9e3779b9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a2d39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}
