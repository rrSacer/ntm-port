//! Uma explosão em andamento, amarrada ao mundo por traits.
//!
//! Por tick: (1) confere o que foi escrito no tick anterior (releitura) e
//! reescreve o que não pegou; (2) avança a varredura de energia com o fuel que
//! sobra; (3) grava o diário das colunas novas (persistência); (4) escreve
//! seções 16³ já decididas, mais perto de um jogador primeiro e de cima para
//! baixo, sem passar do orçamento de escritas (≤ 60 000 < 65 536 da fila global
//! do Horizon, em chamadas de ≤ 4096); (5) no fim, efeitos nas bordas.
//!
//! Todo trabalho de administração (filas de seções e de chunks, ordenação)
//! é por coluna de chunk, com conjuntos em vez de buscas lineares, limitado
//! por tick e descontado do fuel: R=500 tem ~70 mil seções e milhares de
//! chunks que podem estar sem carregar (revisão do M2, P1-1).
use crate::explosion::crater::{BuildFnv, Kind, MAX_RADIUS, MAX_Y, MIN_Y, Plan};
use crate::explosion::effects::{EdgeHit, EffectProfile, Event, Hooks};
use crate::explosion::fallout::{FalloutProfile, Sweep};
use crate::explosion::persist::{self, DATA_CHUNK, Header, JOURNAL_MAX_RADIUS, MAX_DATA_KEYS, RING, Sealed, State, Store};
use std::collections::{HashMap, HashSet, VecDeque};

type Set = HashSet<(i32, i32), BuildFnv>;

/// Fuel do hook de tick do Horizon: 50 M (horizon-mods runtime.rs:110).
/// Estourar desliga o mod; o motor planeja com estimativa e margem.
pub const FUEL_PLAN: u64 = 34_000_000;
/// Fuel estimado por escrita (string + lista), por voxel examinado numa seção
/// e por releitura de conferência (calibrados no spike S7, com o alocador pool).
const COST_WRITE: u64 = 700;
const COST_SCAN: u64 = 60;
const COST_VERIFY: u64 = 1_000;
/// Por efeito de borda (perfil, estados em texto, chunk carregado?): medido
/// no harness (`spikes/m2-fuel`, marcas de fuel): ~4,2 k, mais a escrita.
const COST_EDGE: u64 = 4_500;
/// Por seção ou coluna examinada na administração (estados, filas).
const COST_ADMIN: u64 = 400;
/// Por elemento e nível da ordenação da fila de escrita.
const COST_SORT: u64 = 120;
/// Colunas adiadas conferidas por tick.
const DEFERRED_PER_TICK: usize = 128;
/// Chunks da lista de recálculo conferidos (carregaram?) por vez.
const REEVAL_SCAN: usize = 64;
const RETRY_TICKS: u64 = 20;
/// Depois de uma retomada, a varredura espera até este tanto (60 s) pelos
/// chunks da frente: o servidor recém-iniciado ainda está carregando, e uma
/// coluna lida "sem chunk" vira ar (aproximação que, depois de retomar, não
/// tem mais a energia exata das colunas antigas para recalcular).
const RESUME_WAIT_TICKS: u64 = 1200;
/// Persistência leve: no máximo uma gravação do estado a cada tanto.
const LITE_EVERY: u64 = 20;
/// A persistência leve que falhou tenta de novo depois disto.
const PERSIST_RETRY: u64 = 200;
/// Ticks seguidos sem caber no orçamento de gravação: o diário completo
/// desiste e passa à persistência leve (a cratera não fica parada).
const LATER_MAX: u32 = 100;
/// Orçamento padrão de escritas por tick (o melhor equilíbrio medido no S7).
pub const DEFAULT_BUDGET: usize = 8192;
pub const BLOCKS_PER_CALL: usize = 4096;
pub const AIR: &str = "minecraft:air";

/// O que as explosões podem gravar no storage do mod (revisão do M2, P2-11):
/// o mod inteiro tem 4 MiB, 64 chaves e 1 MiB gravado por chamada de hook,
/// e o resto dele (o save do mundo, as caixas de correio) também grava.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Bytes por tick (por chamada do hook).
    pub tick_bytes: u64,
    /// Bytes e chaves ocupados por todas as explosões juntas.
    pub total_bytes: u64,
    pub keys: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self { tick_bytes: 512 << 10, total_bytes: 5 << 19, keys: 48 }
    }
}

/// Uma mudança de bloco (o mod converte para `world::BlockChange`).
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub state: String,
}

/// O mundo como o motor o vê (capability `world` + `players` do server mod).
pub trait World {
    /// Estado do bloco, ou `None` em chunk não carregado.
    fn block(&mut self, dim: &str, x: i32, y: i32, z: i32) -> Option<String>;
    /// No máximo [`BLOCKS_PER_CALL`] por chamada.
    fn set_blocks(&mut self, dim: &str, changes: Vec<Change>) -> Result<(), String>;
    /// Posições dos olhos dos jogadores nesta dimensão (prioridade de escrita).
    fn viewers(&mut self, dim: &str) -> Vec<[f64; 3]>;
}

/// O que detonar.
#[derive(Clone, Debug)]
pub struct Detonation {
    /// Único entre as explosões ativas; vira prefixo de chave no storage.
    pub id: String,
    pub dim: String,
    pub center: [i32; 3],
    /// 1..=[`MAX_RADIUS`].
    pub radius: i32,
    pub profile: EffectProfile,
    /// Escritas por tick (256..=60 000).
    pub budget: usize,
    /// Gravar o diário no storage para retomar após reinício.
    pub persist: bool,
    /// Acima deste raio a persistência é leve (só as seções escritas).
    pub lite_above: i32,
}

impl Detonation {
    pub fn new(id: impl Into<String>, dim: impl Into<String>, center: [i32; 3], radius: i32) -> Self {
        Self {
            id: id.into(),
            dim: dim.into(),
            center,
            radius,
            profile: EffectProfile::ntm_placeholder(),
            budget: DEFAULT_BUDGET,
            persist: true,
            lite_above: JOURNAL_MAX_RADIUS,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    /// Varredura de energia (com escritas já acontecendo atrás dela).
    Planning,
    /// Varredura pronta; terminando escritas.
    Writing,
    /// Esperando chunks que não estão carregados.
    Waiting,
    Effects,
    /// Fallout de 2,5R (segunda fase).
    Fallout,
    Done,
}

/// Progresso de uma explosão.
#[derive(Clone, Debug)]
pub struct Progress {
    pub id: String,
    pub dim: String,
    pub center: [i32; 3],
    pub radius: i32,
    pub phase: Phase,
    pub tick0: u64,
    pub columns_done: u32,
    pub columns: u32,
    pub reads: u64,
    pub removed: u64,
    pub writes: u64,
    pub effects: u64,
    pub sections_done: usize,
    pub sections: usize,
    pub deferred: usize,
    pub requeued: u64,
    pub physics: u64,
    pub unloaded_reads: u64,
    /// Ticks desde o início (ou None).
    pub first_write: Option<u64>,
    pub crater_done: Option<u64>,
    pub done: Option<u64>,
    pub persisted_bytes: usize,
    /// Fallout: colunas feitas e chunks esperando carregar.
    pub fallout_columns: u64,
    pub fallout_pending: usize,
}

impl Progress {
    pub fn line(&self) -> String {
        let t = |v: Option<u64>| v.map_or(-1, |v| v as i64);
        format!(
            "{} {:?} R={} cols={}/{} reads={} removed={} writes={} effects={} secs={}/{} deferred={} requeued={} physics={} unloaded={} first_write={} crater={} done_at={} persisted={} fallout_cols={} fallout_pending={}",
            self.id,
            self.phase,
            self.radius,
            self.columns_done,
            self.columns,
            self.reads,
            self.removed,
            self.writes,
            self.effects,
            self.sections_done,
            self.sections,
            self.deferred,
            self.requeued,
            self.physics,
            self.unloaded_reads,
            t(self.first_write),
            t(self.crater_done),
            t(self.done),
            self.persisted_bytes,
            self.fallout_columns,
            self.fallout_pending
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SecState {
    Pending = 0,
    Applied = 1,
    Done = 2,
    Deferred = 3,
}

struct Section {
    /// Coordenadas da seção (x>>4, y>>4, z>>4).
    s: [i32; 3],
    state: SecState,
    retry_at: u64,
}

/// Uma coluna de chunk da cratera: as seções dela são `start..start + n`
/// (de baixo para cima).
#[derive(Clone, Copy)]
struct Col {
    key: (i32, i32),
    start: u32,
    n: u32,
    /// Maior h² das colunas do chunk: decidida quando a frente da espiral passa dele.
    ready_h2: u64,
}

/// O que o storage pode receber agora (ver [`Limits`]).
#[derive(Clone, Copy, Debug, Default)]
struct Allow {
    /// Bytes que ainda cabem neste tick, e o máximo de um tick.
    tick: u64,
    tick_max: u64,
    /// Bytes e chaves que esta explosão pode ocupar.
    bytes: u64,
    keys: usize,
}

enum Put {
    Ok,
    /// Não cabe neste tick: tenta no próximo.
    Later,
    Fail(String),
}

/// Grava `value` em `key` dentro do que `allow` deixa; `used` é o que a
/// explosão ocupa agora (bytes, chaves) e `old` o tamanho atual da chave.
fn put(allow: &mut Allow, used: (u64, usize), store: &mut dyn Store, key: &str, value: &[u8], old: usize) -> Put {
    let len = value.len() as u64;
    if len > allow.tick_max {
        return Put::Fail(format!("{key}: {} KiB numa gravação", len >> 10));
    }
    if len > allow.tick {
        return Put::Later;
    }
    if used.0.saturating_sub(old as u64) + len > allow.bytes {
        return Put::Fail(format!("cota das explosões ({} KiB)", allow.bytes >> 10));
    }
    if old == 0 && used.1 + 1 > allow.keys {
        return Put::Fail(format!("chaves das explosões ({})", allow.keys));
    }
    match store.set(key, value) {
        Ok(()) => {
            allow.tick -= len;
            Put::Ok
        }
        // O orçamento de gravação do hook acabou (o mod gravou mais neste tick).
        Err(e) if e.contains("traffic") => Put::Later,
        Err(e) => Put::Fail(e),
    }
}

/// Adiciona ou tira `id` do índice (dois slots).
fn index_update(store: &mut dyn Store, id: &str, add: bool) -> Result<(), String> {
    let (generation, mut ids) = persist::read_index(store);
    let has = ids.iter().any(|i| i == id);
    if add == has {
        return Ok(());
    }
    if add {
        ids.push(id.to_string());
    } else {
        ids.retain(|i| i != id);
    }
    persist::write_index(store, generation, &ids).map(|_| ())
}

/// Apaga todas as chaves de `id` (o índice por último: uma queda no meio
/// deixa um id cujo estado falta, que a retomada limpa).
fn purge(store: &mut dyn Store, prefix: &str, id: &str, blocks: usize) {
    for k in 0..blocks {
        store.delete(&format!("{prefix}.d{k}"));
    }
    for k in 0..RING {
        store.delete(&format!("{prefix}.e{k}"));
    }
    persist::delete_slots(store, &format!("{prefix}.s"));
    let _ = index_update(store, id, false);
}

struct Persist {
    prefix: String,
    /// Geração do último estado gravado (0: nenhum).
    generation: u64,
    durable: bool,
    indexed: bool,
    /// Blocos fechados do diário (`<p>.d<k>`) e o bloco em curso.
    blocks: Vec<Sealed>,
    cur: Vec<u8>,
    /// Faixas de energia fechadas, por chave do anel.
    ering: [Option<(u32, Sealed)>; RING],
    /// Faixas ainda sem chave (vão na cauda do estado): as fechadas que
    /// faltam gravar e, por último, a em curso.
    ebands: Vec<(u32, Vec<u8>)>,
    /// Faixa mais nova do último estado gravado: uma faixa fechada só vai
    /// para a chave do anel quando o estado em vigor não usa mais a que
    /// está lá (a chave `b % 3` guarda `b - 3`; o estado usa as duas antes
    /// da mais nova).
    durable_top: Option<u32>,
    /// A cauda mudou desde o último estado gravado; o último cabeçalho.
    tail_dirty: bool,
    last_header: String,
    last_tick: u64,
    slot_len: [usize; 2],
    later: u32,
    /// Persistência leve que falhou: tenta de novo neste tick.
    retry_at: Option<u64>,
    warned: bool,
}

impl Persist {
    fn new(id: &str) -> Self {
        Self {
            prefix: persist::prefix(id),
            generation: 0,
            durable: false,
            indexed: false,
            blocks: Vec::new(),
            cur: Vec::new(),
            ering: [None; RING],
            ebands: Vec::new(),
            durable_top: None,
            tail_dirty: true,
            last_header: String::new(),
            last_tick: 0,
            slot_len: [0; 2],
            later: 0,
            retry_at: None,
            warned: false,
        }
    }

    /// A faixa de energia mais nova (na cauda ou numa chave).
    fn top(&self) -> Option<u32> {
        self.ebands.iter().map(|b| b.0).chain(self.ering.iter().flatten().map(|e| e.0)).max()
    }

    /// As chaves do anel que o plano ainda usa: as duas faixas antes da mais nova.
    fn ering_used(&self) -> [Option<(u32, Sealed)>; RING] {
        let top = self.top().unwrap_or(0);
        self.ering.map(|e| e.filter(|(b, _)| b + 2 >= top))
    }

    /// Bytes e chaves que ocupa no storage.
    fn used(&self) -> (u64, usize) {
        let b = self.slot_len.iter().sum::<usize>() + self.blocks.iter().map(|b| b.0 as usize).sum::<usize>() + self.ering.iter().flatten().map(|e| e.1.0 as usize).sum::<usize>();
        let k = self.slot_len.iter().filter(|l| **l > 0).count() + self.blocks.len() + self.ering.iter().flatten().count();
        (b as u64, k)
    }
}

/// O diário lido do storage, restaurado aos poucos.
struct Restoring {
    data: Vec<u8>,
    pos: usize,
    /// Colunas já vistas (a 1ª ocorrência é da passada principal).
    seen: Vec<bool>,
    energy: Vec<Vec<u8>>,
    cols_done: u32,
    done_chunks: Set,
}

/// O estado mais novo de uma explosão cujo checksum e cujos blocos conferem.
struct Loaded {
    generation: u64,
    state: State,
    len: usize,
    blocks: Vec<Vec<u8>>,
    energy: Vec<Vec<u8>>,
}

fn load(store: &mut dyn Store, prefix: &str) -> Result<Loaded, String> {
    let slots = persist::read_slots(store, &format!("{prefix}.s"));
    let mut err = String::from("sem estado");
    'slot: for (generation, payload) in slots {
        let state = match State::decode(&payload) {
            Ok(s) => s,
            Err(e) => {
                err = e;
                continue;
            }
        };
        let (mut blocks, mut energy) = (Vec::new(), Vec::new());
        if state.header.phase != "linger" && !state.header.lite {
            for (k, &(len, sum)) in state.header.blocks.iter().enumerate() {
                match store.get(&format!("{prefix}.d{k}")) {
                    Some(b) if b.len() == len as usize && persist::fnv64(&b) == sum => blocks.push(b),
                    _ => {
                        err = format!("bloco d{k} do diário ilegível");
                        continue 'slot;
                    }
                }
            }
            for (slot, e) in state.header.ering.iter().enumerate() {
                let Some((_, (len, sum))) = e else { continue };
                match store.get(&format!("{prefix}.e{slot}")) {
                    Some(b) if b.len() == *len as usize && persist::fnv64(&b) == *sum => energy.push(b),
                    _ => {
                        err = format!("faixa e{slot} ilegível");
                        continue 'slot;
                    }
                }
            }
        }
        let len = payload.len();
        return Ok(Loaded { generation, state, len, blocks, energy });
    }
    Err(err)
}

pub struct Explosion {
    pub id: String,
    pub dim: String,
    plan: Plan,
    profile: EffectProfile,
    budget: usize,
    secs: Vec<Section>,
    cols: Vec<Col>,
    col_of: HashMap<(i32, i32), u32, BuildFnv>,
    /// Colunas em ordem de `ready_h2`, e quantas já entraram na fila.
    order: Vec<u32>,
    order_pos: usize,
    /// Fila de escrita: colunas com seções prontas, por prioridade.
    pool: Vec<(f64, u32)>,
    in_pool: Vec<bool>,
    pool_dirty: bool,
    /// Colunas com seções adiadas (chunk sem carregar), conferidas em rodízio.
    deferred: VecDeque<u32>,
    in_deferred: Vec<bool>,
    /// Seções por estado.
    counts: [usize; 4],
    viewers: Vec<[f64; 3]>,
    written: Vec<(usize, Vec<[i32; 3]>)>,
    edges_done: usize,
    tick0: u64,
    started: u64,
    first_write: Option<u64>,
    crater_done: Option<u64>,
    done_tick: Option<u64>,
    writes: u64,
    effects: u64,
    requeued: u64,
    physics: u64,
    /// Chunks que estavam sem carregar e esperam carga (em ordem, e o
    /// conjunto), os postos na fila de recálculo do plano (com a posição em
    /// que terminam) e os já recalculados.
    reeval: Vec<(i32, i32)>,
    reeval_set: Set,
    reeval_pos: usize,
    reeval_started: HashMap<(i32, i32), usize, BuildFnv>,
    reeval_done: Vec<(i32, i32)>,
    reeval_done_set: Set,
    /// Fim da passada principal: os chunks sem carregar entram na lista de
    /// recálculo aos poucos (posição em `plan.unloaded_chunks`).
    marking: Option<usize>,
    persist: Option<Persist>,
    allow: Allow,
    pub max_tick_writes: usize,
    /// Retomada: até quando esperar chunks da frente (definido no 1º tick).
    resumed: bool,
    wait_until: Option<u64>,
    /// Bordas cujo chunk estava descarregado na hora do efeito (nova tentativa).
    /// (por chunk, e os chunks em rodízio para a nova tentativa).
    edges_later: HashMap<(i32, i32), Vec<u32>, BuildFnv>,
    later_keys: VecDeque<(i32, i32)>,
    edges_retry_at: u64,
    pub waited_ticks: u64,
    /// Segunda fase (fallout), depois da cratera e das bordas.
    sweep: Option<Sweep>,
    fallout_saved: u64,
    /// Retomada no meio do fallout: anel e pendentes de onde continuar
    /// depois que a cratera for reaplicada (ou terminada).
    resume_sweep: Option<(i32, Vec<(i32, i32)>)>,
    /// Fuel estimado que um tick pode gastar (ver [`Explosions::fuel`]).
    pub fuel: u64,
    /// Retomada em andamento (o diário restaurado aos poucos).
    restoring: Option<Box<Restoring>>,
    pub restore_error: Option<String>,
    /// Persistência leve (R > JOURNAL_MAX_RADIUS, ou o diário não coube):
    /// só as seções escritas, 1 bit cada.
    lite: bool,
    lite_bits: Vec<u8>,
}

fn sections_for(plan: &Plan) -> (Vec<Section>, Vec<Col>) {
    let (c, r) = (plan.c, plan.r);
    let mut secs = Vec::new();
    let mut cols = Vec::new();
    let (y0, y1) = ((c[1] - r).max(MIN_Y), (c[1] + r).min(MAX_Y));
    for sx in (c[0] - r) >> 4..=(c[0] + r) >> 4 {
        for sz in (c[2] - r) >> 4..=(c[2] + r) >> 4 {
            let nx = c[0].clamp(sx * 16, sx * 16 + 15) - c[0];
            let nz = c[2].clamp(sz * 16, sz * 16 + 15) - c[2];
            if nx * nx + nz * nz > r * r {
                continue;
            }
            let start = secs.len();
            for sy in y0 >> 4..=y1 >> 4 {
                secs.push(Section { s: [sx, sy, sz], state: SecState::Pending, retry_at: 0 });
            }
            cols.push(Col { key: (sx, sz), start: start as u32, n: (secs.len() - start) as u32, ready_h2: plan.section_h2([sx, 0, sz]) });
        }
    }
    (secs, cols)
}

impl Explosion {
    fn check(det: &Detonation) -> Result<(), String> {
        if !(1..=MAX_RADIUS).contains(&det.radius) {
            return Err(format!("raio {} fora de 1..={MAX_RADIUS}", det.radius));
        }
        if det.id.is_empty() || !det.id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-') || det.id.len() > 40 {
            return Err(format!("id inválido: {:?}", det.id));
        }
        Ok(())
    }

    /// Monta o plano (o tick da criação não avança: no Horizon, montar R=500
    /// custa ~21 M de fuel).
    pub fn new(det: Detonation, tick: u64) -> Result<Self, String> {
        Self::check(&det)?;
        let mut plan = Plan::new(det.center, det.radius);
        let lite = det.persist && det.radius > det.lite_above;
        if det.persist && !lite {
            plan.journal = Some(Default::default());
        }
        let (secs, cols) = sections_for(&plan);
        let col_of = cols.iter().enumerate().map(|(i, c)| (c.key, i as u32)).collect();
        let mut order: Vec<u32> = (0..cols.len() as u32).collect();
        order.sort_by_key(|&i| (cols[i as usize].ready_h2, i));
        let c = det.center;
        let n = cols.len();
        Ok(Self {
            persist: det.persist.then(|| Persist::new(&det.id)),
            allow: Allow { tick: u64::MAX, tick_max: u64::MAX, bytes: u64::MAX, keys: usize::MAX },
            id: det.id,
            dim: det.dim,
            plan,
            profile: det.profile,
            budget: det.budget.clamp(256, 60_000),
            counts: [secs.len(), 0, 0, 0],
            lite_bits: if lite { vec![0; secs.len().div_ceil(8)] } else { Vec::new() },
            secs,
            cols,
            col_of,
            order,
            order_pos: 0,
            pool: Vec::new(),
            in_pool: vec![false; n],
            pool_dirty: false,
            deferred: VecDeque::new(),
            in_deferred: vec![false; n],
            viewers: vec![[c[0] as f64 + 0.5, c[1] as f64, c[2] as f64 + 0.5]],
            written: Vec::new(),
            edges_done: 0,
            tick0: tick,
            started: tick,
            first_write: None,
            crater_done: None,
            done_tick: None,
            writes: 0,
            effects: 0,
            requeued: 0,
            physics: 0,
            reeval: Vec::new(),
            reeval_set: Set::default(),
            reeval_pos: 0,
            reeval_started: HashMap::default(),
            reeval_done: Vec::new(),
            reeval_done_set: Set::default(),
            marking: None,
            max_tick_writes: 0,
            resumed: false,
            wait_until: None,
            edges_later: HashMap::default(),
            later_keys: VecDeque::new(),
            edges_retry_at: 0,
            waited_ticks: 0,
            sweep: None,
            fallout_saved: 0,
            resume_sweep: None,
            fuel: FUEL_PLAN,
            restoring: None,
            restore_error: None,
            lite,
        })
    }

    fn set(&mut self, si: usize, st: SecState) {
        let old = self.secs[si].state;
        if old == st {
            return;
        }
        self.counts[old as usize] -= 1;
        self.counts[st as usize] += 1;
        self.secs[si].state = st;
        if self.lite && st == SecState::Applied {
            self.lite_bits[si >> 3] |= 1 << (si & 7);
            if let Some(p) = self.persist.as_mut() {
                p.tail_dirty = true;
            }
        }
    }

    /// Retoma a explosão do estado lido: o plano volta até a última coluna
    /// gravada; todas as seções voltam a pendentes (ver `persist`).
    fn resume_from(l: Loaded, tick: u64) -> Result<Self, String> {
        let h = &l.state.header;
        let profile = EffectProfile::decode(&h.profile)?;
        let det = Detonation {
            id: h.id.clone(),
            dim: h.dim.clone(),
            center: h.center,
            radius: h.radius,
            profile,
            budget: h.budget,
            persist: true,
            lite_above: if h.lite { 0 } else { MAX_RADIUS },
        };
        let mut ex = Explosion::new(det, tick)?;
        // Ticks relativos contam a partir da retomada (o relógio do servidor recomeça).
        ex.resumed = true;
        {
            let p = ex.persist.as_mut().expect("persistente");
            p.generation = l.generation;
            p.durable = true;
            p.indexed = true;
            p.slot_len = [l.len + 64; 2];
            p.last_header = h.encode();
            p.tail_dirty = false;
            if !h.lite {
                p.blocks = h.blocks.clone();
                p.ering = h.ering;
                p.cur = l.state.data.clone();
                p.ebands = persist::decode_bands(&l.state.energy)?;
                p.durable_top = p.top();
            }
        }
        let fallout = h.phase == "fallout";
        if fallout && h.lite && !h.crater_left {
            // Leve: a cratera acabou antes do reinício; só o fallout continua.
            ex.crater_done = Some(tick);
            for si in 0..ex.secs.len() {
                ex.set(si, SecState::Done);
            }
            ex.lite_bits = l.state.bits.clone();
            ex.lite_bits.resize(ex.secs.len().div_ceil(8), 0);
            let fp = ex.profile.fallout.clone().ok_or("fallout sem perfil")?;
            ex.sweep = Some(Sweep::resume(h.center, h.radius, fp.scale, h.fring, h.fpend.clone()));
            return Ok(ex);
        }
        if fallout {
            // A cratera é reaplicada (cobre um mundo que voltou ao último
            // autosave) ou terminada, e o fallout continua do anel salvo.
            ex.resume_sweep = Some((h.fring, h.fpend.clone()));
        }
        if h.lite {
            // Leve: o plano é refeito; as seções já escritas não são reescritas.
            // O que elas removeram virou ar, então a cratera além delas pode
            // sair um pouco maior (aceito; docs/explosoes.md §9).
            let bits = &l.state.bits;
            for si in 0..ex.secs.len() {
                if bits.get(si >> 3).is_some_and(|b| b >> (si & 7) & 1 == 1) {
                    ex.set(si, SecState::Done);
                }
            }
            ex.lite_bits = bits.clone();
            ex.lite_bits.resize(ex.secs.len().div_ceil(8), 0);
            return Ok(ex);
        }
        // Estrutura: colunas da passada principal (antes da frente) e as
        // recalculadas dos chunks já terminados; o resto é refeito. A leitura
        // é aqui; a restauração vai em passos, nos ticks seguintes (R=120 tem
        // ~45 mil colunas: num tick só, passava dos 50 M de fuel).
        let mut data = l.blocks.concat();
        data.extend_from_slice(&l.state.data);
        let mut energy = l.energy;
        energy.extend(persist::decode_bands(&l.state.energy)?.into_iter().map(|b| b.1));
        let plan = &mut ex.plan;
        // A frente da espiral primeiro: "calculada" é antes dela.
        plan.set_cols_done(h.cols_done);
        for &key in h.pending_chunks.iter().chain(&h.done_chunks) {
            plan.mark_unloaded(key);
        }
        ex.reeval = h.pending_chunks.clone();
        ex.reeval_set = ex.reeval.iter().copied().collect();
        ex.reeval_done = h.done_chunks.clone();
        ex.reeval_done_set = ex.reeval_done.iter().copied().collect();
        let seen = vec![false; ex.plan.columns()];
        let done_chunks = h.done_chunks.iter().copied().collect();
        ex.restoring = Some(Box::new(Restoring { data, pos: 0, seen, energy, cols_done: h.cols_done, done_chunks }));
        Ok(ex)
    }

    /// Um passo da retomada: registros do diário até o fuel do tick; no fim,
    /// a energia do anel e a frente.
    fn restore_step(&mut self) {
        let Some(mut r) = self.restoring.take() else { return };
        let budget = self.fuel;
        let Restoring { data, pos, seen, done_chunks, .. } = &mut *r;
        let step = persist::restore_columns_step(&mut self.plan, data, pos, budget, |plan, ci| {
            let first = !seen[ci];
            seen[ci] = true;
            let main_ok = first && plan.done(ci);
            let queue_ok = !first && done_chunks.contains(&plan.chunk_of(ci));
            main_ok || queue_ok
        });
        match step {
            Ok(false) => {
                self.restoring = Some(r);
                return;
            }
            Ok(true) => {}
            Err(e) => {
                // Diário ilegível: segue com o que foi restaurado (aproximado).
                self.restore_error = Some(e);
            }
        }
        for part in &r.energy {
            if let Err(e) = persist::restore_energy(&mut self.plan, part, |plan, ci| plan.done(ci)) {
                self.restore_error = Some(e);
            }
        }
        self.plan.set_cols_done(r.cols_done);
    }

    pub fn done(&self) -> bool {
        self.done_tick.is_some()
    }

    pub fn progress(&self) -> Progress {
        let rel = |t: Option<u64>| t.map(|t| t.saturating_sub(self.tick0));
        let deferred = self.counts[SecState::Deferred as usize];
        let pending = self.counts[SecState::Pending as usize] > 0;
        let phase = if self.done() {
            Phase::Done
        } else if self.sweep.is_some() {
            Phase::Fallout
        } else if !self.plan.finished() {
            Phase::Planning
        } else if deferred > 0 && !pending {
            Phase::Waiting
        } else if self.crater_done.is_some() {
            Phase::Effects
        } else {
            Phase::Writing
        };
        Progress {
            id: self.id.clone(),
            dim: self.dim.clone(),
            center: self.plan.c,
            radius: self.plan.r,
            phase,
            tick0: self.tick0,
            columns_done: self.plan.cols_done,
            columns: self.plan.columns() as u32,
            reads: self.plan.reads,
            removed: self.plan.removed,
            writes: self.writes,
            effects: self.effects,
            sections_done: self.counts[SecState::Done as usize],
            sections: self.secs.len(),
            deferred,
            requeued: self.requeued,
            physics: self.physics,
            unloaded_reads: self.plan.unloaded,
            first_write: rel(self.first_write),
            crater_done: rel(self.crater_done),
            done: rel(self.done_tick),
            persisted_bytes: self.persist.as_ref().map_or(0, |p| p.used().0 as usize),
            fallout_columns: self.sweep.as_ref().map_or(0, |s| s.columns),
            fallout_pending: self.sweep.as_ref().map_or(0, |s| s.pending.len()),
        }
    }

    /// Prioridade de uma coluna de chunk: perto (na horizontal) de um jogador primeiro.
    fn priority(&self, ci: u32) -> f64 {
        let (sx, sz) = self.cols[ci as usize].key;
        let cx = sx as f64 * 16.0 + 8.0;
        let cz = sz as f64 * 16.0 + 8.0;
        self.viewers.iter().map(|v| (v[0] - cx).powi(2) + (v[2] - cz).powi(2)).fold(f64::MAX, f64::min)
    }

    /// Um chunk que estava sem carregar só é escrito depois de recalculado.
    fn chunk_ready(&self, key: (i32, i32)) -> bool {
        !self.reeval_set.contains(&key) && self.reeval_started.get(&key).is_none_or(|end| self.plan.queue_done >= *end)
    }

    fn add_pool(&mut self, ci: u32) {
        if !self.in_pool[ci as usize] {
            self.in_pool[ci as usize] = true;
            self.pool.push((self.priority(ci), ci));
            self.pool_dirty = true;
        }
    }

    fn defer_col(&mut self, ci: u32) {
        if !self.in_deferred[ci as usize] {
            self.in_deferred[ci as usize] = true;
            self.deferred.push_back(ci);
        }
    }

    /// As colunas que a frente já passou entram na fila de escrita.
    fn admit(&mut self, front: u64) -> u64 {
        let mut spent = 0;
        while let Some(&ci) = self.order.get(self.order_pos) {
            if self.cols[ci as usize].ready_h2 >= front {
                break;
            }
            self.order_pos += 1;
            self.add_pool(ci);
            spent += COST_ADMIN;
        }
        spent
    }

    /// Os jogadores mudaram: prioridades refeitas.
    fn reprioritize(&mut self) -> u64 {
        for i in 0..self.pool.len() {
            self.pool[i].0 = self.priority(self.pool[i].1);
        }
        self.pool_dirty = true;
        self.pool.len() as u64 * COST_ADMIN
    }

    fn sort_pool(&mut self) -> u64 {
        if !self.pool_dirty {
            return 0;
        }
        self.pool_dirty = false;
        self.pool.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let n = self.pool.len() as u64;
        n * u64::from(64 - n.leading_zeros()) * COST_SORT
    }

    /// Colunas com seções adiadas: as que podem ser tentadas de novo voltam
    /// à fila de escrita (algumas por tick, em rodízio).
    fn deferred_step(&mut self, tick: u64, finished: bool) -> u64 {
        if !finished {
            return 0;
        }
        let mut spent = 0;
        for _ in 0..self.deferred.len().min(DEFERRED_PER_TICK) {
            let Some(ci) = self.deferred.pop_front() else { break };
            let col = self.cols[ci as usize];
            spent += COST_ADMIN * (1 + u64::from(col.n));
            let (mut any, mut due) = (false, false);
            for si in col.start..col.start + col.n {
                let s = &self.secs[si as usize];
                if s.state == SecState::Deferred {
                    any = true;
                    due |= s.retry_at <= tick;
                }
            }
            if !any {
                self.in_deferred[ci as usize] = false;
            } else if due && self.chunk_ready(col.key) {
                self.in_deferred[ci as usize] = false;
                self.add_pool(ci);
            } else {
                self.deferred.push_back(ci);
            }
        }
        spent
    }

    /// Fim da passada principal: os chunks que estavam sem carregar (e
    /// ainda não recalculados) entram na lista de recálculo, e as seções
    /// deles esperam. Aos poucos, até `max` de fuel.
    fn mark_step(&mut self, tick: u64, max: u64) -> u64 {
        let Some(mut i) = self.marking else { return 0 };
        let mut spent = 0;
        while i < self.plan.unloaded_chunks.len() && spent < max {
            let k = self.plan.unloaded_chunks[i];
            i += 1;
            spent += COST_ADMIN;
            if self.reeval_set.contains(&k) || self.reeval_done_set.contains(&k) || self.reeval_started.contains_key(&k) {
                continue;
            }
            self.reeval.push(k);
            self.reeval_set.insert(k);
            if let Some(&ci) = self.col_of.get(&k) {
                let col = self.cols[ci as usize];
                for si in col.start..col.start + col.n {
                    if self.secs[si as usize].state == SecState::Pending {
                        self.set(si as usize, SecState::Deferred);
                        self.secs[si as usize].retry_at = tick;
                    }
                }
                spent += COST_ADMIN * u64::from(col.n);
                self.defer_col(ci);
            }
        }
        self.marking = (i < self.plan.unloaded_chunks.len()).then_some(i);
        spent
    }

    /// Chunks que estavam sem carregar e agora carregaram: para a fila de
    /// recálculo (até 4 por vez, conferindo até [`REEVAL_SCAN`] em rodízio).
    fn reeval_begin_step(&mut self, world: &mut dyn World, dim: &str) -> u64 {
        let (mut begun, mut spent) = (0, 0);
        for _ in 0..self.reeval.len().min(REEVAL_SCAN) {
            if begun >= 4 || self.reeval.is_empty() {
                break;
            }
            if self.reeval_pos >= self.reeval.len() {
                self.reeval_pos = 0;
            }
            let key = self.reeval[self.reeval_pos];
            spent += COST_VERIFY;
            if world.block(dim, key.0 * 16, self.plan.c[1], key.1 * 16).is_some() {
                let end = self.plan.reeval_begin(key);
                self.reeval_started.insert(key, end);
                self.reeval.remove(self.reeval_pos);
                self.reeval_set.remove(&key);
                begun += 1;
            } else {
                self.reeval_pos += 1;
            }
        }
        spent
    }

    /// Células prontas a escrever logo (para reservar fuel), até `budget`.
    fn ready_cells(&self, budget: usize) -> usize {
        let mut n = 0;
        for &(_, ci) in self.pool.iter().take(64) {
            let col = self.cols[ci as usize];
            for si in col.start..col.start + col.n {
                let s = &self.secs[si as usize];
                if s.state == SecState::Pending
                    && let Some(i) = self.plan.sec_index(s.s[0] * 16, s.s[1] * 16, s.s[2] * 16)
                {
                    n += self.plan.sec_count[i] as usize;
                    if n >= budget {
                        return budget;
                    }
                }
            }
        }
        n
    }

    pub fn tick(&mut self, tick: u64, world: &mut dyn World, store: &mut dyn Store, hooks: &mut dyn Hooks) {
        // O tick da criação (ou da retomada) já gastou fuel montando o plano.
        if self.done() || tick == self.started {
            return;
        }
        if self.restoring.is_some() {
            self.restore_step();
            return;
        }
        if let Some(finished) = self.sweep.as_ref().map(Sweep::finished) {
            // No fallout, o que ainda falta da cratera (chunks que não estavam
            // carregados) é tentado a cada RETRY_TICKS.
            // (Seções escritas no tick anterior são conferidas logo.)
            let verify = !self.written.is_empty();
            if (self.waiting_chunks() || verify) && (finished || verify || (tick - self.started).is_multiple_of(RETRY_TICKS)) {
                self.crater_tick(tick, world, store, hooks);
            } else if !finished {
                self.fallout_tick(tick, world, store, hooks);
            }
            crate::explosion::trace(22);
            self.check_done(tick, store, hooks);
            return;
        }
        self.crater_tick(tick, world, store, hooks);
    }

    /// Só falta o que está em chunks que não estavam carregados (seções
    /// adiadas, recálculos, bordas para depois).
    fn waiting_chunks(&self) -> bool {
        self.counts[SecState::Deferred as usize] > 0
            || !self.reeval.is_empty()
            || !self.reeval_started.is_empty()
            || !self.edges_later.is_empty()
            || self.marking.is_some()
    }

    /// Nada a fazer enquanto um jogador não chegar perto dos chunks que
    /// faltam: não segura a vez das outras explosões.
    pub fn idle(&self) -> bool {
        self.crater_done.is_some()
            && self.written.is_empty()
            && self.counts[SecState::Pending as usize] == 0
            && self.sweep.as_ref().is_none_or(Sweep::finished)
            && self.waiting_chunks()
    }

    /// Fallout terminado e cratera inteira: pronta (os chunks do fallout que
    /// faltam ficam para `Linger`).
    fn check_done(&mut self, tick: u64, store: &mut dyn Store, hooks: &mut dyn Hooks) {
        let Some(sweep) = self.sweep.as_ref() else { return };
        if self.done() || !sweep.finished() || self.waiting_chunks() || !self.written.is_empty() {
            return;
        }
        if sweep.pending.is_empty() {
            self.forget(store);
        } else if !self.save_linger(store, hooks) {
            return; // não coube neste tick
        }
        self.done_tick = Some(tick);
        hooks.event(&self.id, &Event::Done { tick: tick - self.tick0 });
    }

    /// Passa a "só pendentes": o estado novo primeiro, e só depois o diário
    /// da cratera sai (uma queda no meio deixa o estado anterior inteiro).
    fn save_linger(&mut self, store: &mut dyn Store, hooks: &mut dyn Hooks) -> bool {
        let state = State { header: self.header("linger"), ..State::default() };
        let id = self.id.clone();
        let mut allow = self.allow;
        let Some(p) = self.persist.as_mut() else { return true };
        if !p.durable {
            return true; // sem persistência (falhou): só na memória
        }
        let generation = p.generation + 1;
        let bytes = persist::slot_bytes(generation, &state.encode());
        let slot = (generation % 2) as usize;
        let used = p.used();
        match put(&mut allow, used, store, &persist::slot_for(&format!("{}.s", p.prefix), generation), &bytes, p.slot_len[slot]) {
            Put::Later => return false,
            Put::Fail(reason) => hooks.event(&id, &Event::PersistFailed { reason }),
            Put::Ok => {
                p.generation = generation;
                p.slot_len[slot] = bytes.len();
                p.last_header = state.header.encode();
            }
        }
        self.allow = allow;
        // O diário da cratera sai.
        let p = self.persist.as_mut().unwrap();
        for k in 0..p.blocks.len() {
            store.delete(&format!("{}.d{k}", p.prefix));
        }
        for k in 0..RING {
            store.delete(&format!("{}.e{k}", p.prefix));
        }
        p.blocks.clear();
        p.ering = [None; RING];
        p.cur.clear();
        p.ebands.clear();
        p.durable_top = None;
        true
    }

    fn crater_tick(&mut self, tick: u64, world: &mut dyn World, store: &mut dyn Store, hooks: &mut dyn Hooks) {
        let dim = self.dim.clone();
        if self.resumed && self.wait_until.is_none() {
            self.wait_until = Some(tick + RESUME_WAIT_TICKS);
        }
        let mut fuel = self.fuel;
        if (tick - self.started) % 10 == 1 {
            let v = world.viewers(&dim);
            if !v.is_empty() && v != self.viewers {
                self.viewers = v;
                fuel = fuel.saturating_sub(self.reprioritize());
            }
        }

        // (1) Conferência das seções escritas no tick anterior.
        let mut out: Vec<Change> = Vec::new();
        for (si, cells) in std::mem::take(&mut self.written) {
            fuel = fuel.saturating_sub(cells.len() as u64 * COST_VERIFY);
            for p in &cells {
                let Some(s) = world.block(&dim, p[0], p[1], p[2]) else { continue };
                if s == AIR {
                    continue;
                }
                let id = self.plan.mats.id(&s);
                let m = self.plan.mats.table[id as usize];
                match m.kind {
                    Kind::Air => {}
                    Kind::Liquid => self.physics += 1,
                    _ if m.gravity => self.physics += 1,
                    _ => {
                        self.requeued += 1;
                        out.push(Change { x: p[0], y: p[1], z: p[2], state: AIR.into() });
                    }
                }
            }
            self.set(si, SecState::Done);
        }
        crate::explosion::trace(1);

        // Fim da passada principal: a lista de recálculo, aos poucos.
        fuel = fuel.saturating_sub(self.mark_step(tick, fuel / 4));
        // Colunas que a frente já passou: para a fila de escrita.
        fuel = fuel.saturating_sub(self.admit(self.plan.front_h2()));
        crate::explosion::trace(2);

        // Reserva de fuel só para as escritas já prontas (a varredura é o
        // gargalo: reservar o orçamento inteiro a deixaria mais lenta) e para
        // gravar o estado (checksum da cauda).
        let mut budget = self.budget.saturating_sub(out.len());
        let tail = self.persist.as_ref().map_or(0, |p| p.cur.len() + p.ebands.iter().map(|b| b.1.len()).sum::<usize>()) + self.lite_bits.len();
        let reserve = self.ready_cells(budget) as u64 * (COST_WRITE + 4 * COST_SCAN) + 2_000_000 + 2 * tail as u64 * persist::COST_BYTE;
        crate::explosion::trace(3);

        if self.plan.finished() && !self.reeval.is_empty() && (tick - self.started).is_multiple_of(5) {
            fuel = fuel.saturating_sub(self.reeval_begin_step(world, &dim));
        }

        crate::explosion::trace(4);
        // (2) Varredura de energia com o fuel que sobra.
        // Depois de retomar, espera o chunk da próxima coluna carregar.
        let waiting = self.wait_until.is_some_and(|t| tick < t)
            && self.plan.next_column().is_some_and(|ci| {
                let (cx, cz) = self.plan.chunk_of(ci);
                world.block(&dim, cx * 16, self.plan.c[1], cz * 16).is_none()
            });
        if waiting {
            self.waited_ticks += 1;
        } else if self.wait_until.is_some_and(|t| tick < t) && self.plan.finished() && self.plan.queue_idle() {
            self.wait_until = Some(tick);
        }
        if !waiting && (!self.plan.finished() || !self.plan.queue_idle()) {
            let was_finished = self.plan.finished();
            let plan_fuel = fuel.saturating_sub(reserve).max(2_000_000);
            let before = self.plan.cost();
            let mut read = |x: i32, y: i32, z: i32| world.block(&dim, x, y, z);
            self.plan.step(plan_fuel, &mut read);
            fuel = fuel.saturating_sub(self.plan.cost() - before);
            if !was_finished && self.plan.finished() {
                // Fim da passada principal: todo chunk que estava sem carregar
                // (e ainda não foi recalculado) entra na lista de espera.
                self.marking = Some(0);
                fuel = fuel.saturating_sub(self.mark_step(tick, fuel / 4));
            }
        }
        crate::explosion::trace(5);
        // Chunks recalculados por inteiro.
        if !self.reeval_started.is_empty() {
            let qd = self.plan.queue_done;
            let done: Vec<(i32, i32)> = self.reeval_started.iter().filter(|(_, end)| qd >= **end).map(|(k, _)| *k).collect();
            for key in done {
                self.reeval_started.remove(&key);
                self.reeval_done.push(key);
                self.reeval_done_set.insert(key);
            }
        }

        // (3) Diário: grava as colunas novas antes de qualquer escrita delas.
        let durable = self.save(store, hooks, tick, false);
        crate::explosion::trace(6);

        // (4) Escrita das seções prontas.
        let front = self.plan.front_h2();
        let finished = self.plan.finished();
        if finished && self.plan.queue_idle() && self.reeval.is_empty() && self.reeval_started.is_empty() && self.plan.live_buffers() > 0 {
            // Nada mais a recalcular: a energia guardada não serve mais.
            self.plan.release_all();
        }
        crate::explosion::trace(7);
        if durable {
            fuel = fuel.saturating_sub(self.admit(front));
            fuel = fuel.saturating_sub(self.deferred_step(tick, finished));
            fuel = fuel.saturating_sub(self.sort_pool());
            crate::explosion::trace(8);
            self.write_sections(tick, finished, world, &dim, &mut out, &mut budget, &mut fuel);
        }

        crate::explosion::trace(9);
        // (5) Efeitos depois da cratera inteira.
        let pending = self.counts[SecState::Pending as usize] > 0;
        if finished && self.plan.queue_idle() && !pending && self.written.is_empty() && self.crater_done.is_none() {
            self.crater_done = Some(tick);
            hooks.event(&self.id, &Event::CraterDone { tick: tick - self.tick0 });
        }
        if durable && self.crater_done.is_some() && budget > 0 {
            self.edge_effects(&mut out, budget, fuel, tick, world, hooks);
        }

        crate::explosion::trace(10);
        let n = out.len();
        self.max_tick_writes = self.max_tick_writes.max(n);
        if n > 0 && self.first_write.is_none() {
            self.first_write = Some(tick);
            hooks.event(&self.id, &Event::FirstWrite { tick: tick - self.tick0 });
        }
        while !out.is_empty() {
            let rest = out.split_off(out.len().min(BLOCKS_PER_CALL));
            let _ = world.set_blocks(&dim, std::mem::replace(&mut out, rest));
        }
        self.writes += n as u64;
        crate::explosion::trace(11);

        let all_written = self.crater_done.is_some() && self.edges_done >= self.plan.edges.len() && self.written.is_empty();
        if all_written && self.sweep.is_none() && let Some(fp) = self.profile.fallout.clone() {
            // O fallout começa mesmo com chunks da cratera esperando carregar.
            self.begin_fallout(&fp, tick, store, hooks);
        } else if all_written && self.sweep.is_none() && !self.waiting_chunks() {
            self.done_tick = Some(tick);
            hooks.event(&self.id, &Event::Done { tick: tick - self.tick0 });
            self.forget(store);
        }
    }

    /// Escreve as seções da fila (colunas por prioridade; na mesma coluna,
    /// de cima para baixo) até o orçamento de escritas ou o fuel.
    #[allow(clippy::too_many_arguments)]
    fn write_sections(&mut self, tick: u64, finished: bool, world: &mut dyn World, dim: &str, out: &mut Vec<Change>, budget: &mut usize, fuel: &mut u64) {
        let mut touched: Option<(i32, i32)> = None;
        let mut pi = 0;
        'pool: while pi < self.pool.len() {
            let ci = self.pool[pi].1;
            let col = self.cols[ci as usize];
            for k in (0..col.n).rev() {
                let si = (col.start + k) as usize;
                let st = self.secs[si].state;
                let eligible = match st {
                    SecState::Pending => true,
                    SecState::Deferred => finished && self.secs[si].retry_at <= tick && self.chunk_ready(col.key),
                    _ => false,
                };
                if !eligible {
                    if st == SecState::Deferred {
                        self.defer_col(ci);
                    }
                    continue;
                }
                *fuel = fuel.saturating_sub(COST_ADMIN);
                if *budget == 0 || *fuel < 4096 * COST_SCAN {
                    break 'pool;
                }
                let [sx, sy, sz] = self.secs[si].s;
                let empty = self.plan.sec_index(sx * 16, sy * 16, sz * 16).is_some_and(|i| self.plan.sec_count[i] == 0);
                if empty {
                    if st == SecState::Pending && self.plan.was_unloaded((sx, sz)) && !self.reeval_done_set.contains(&(sx, sz)) {
                        // "Vazia" só porque o chunk não estava carregado: espera o recálculo.
                        self.set(si, SecState::Deferred);
                        self.secs[si].retry_at = tick + RETRY_TICKS;
                        self.defer_col(ci);
                    } else {
                        self.set(si, SecState::Done);
                    }
                    continue;
                }
                // Chunk carregado? (`block` só responde em chunk carregado.)
                *fuel = fuel.saturating_sub(COST_VERIFY);
                if world.block(dim, sx * 16, sy * 16, sz * 16).is_none() {
                    self.set(si, SecState::Deferred);
                    self.secs[si].retry_at = tick + RETRY_TICKS;
                    self.defer_col(ci);
                    continue;
                }
                *fuel = fuel.saturating_sub(4096 * COST_SCAN);
                let cells = self.plan.section_cells(self.secs[si].s);
                if cells.len() as u64 * COST_WRITE > *fuel {
                    break 'pool;
                }
                if cells.len() > *budget && !out.is_empty() {
                    break 'pool; // não cabe neste tick; volta no próximo
                }
                if touched.is_some_and(|t| t != (sx, sz)) && cells.len() * 2 > *budget {
                    break 'pool; // não começa outra coluna de chunk pela metade
                }
                touched = Some((sx, sz));
                *budget = budget.saturating_sub(cells.len());
                *fuel = fuel.saturating_sub(cells.len() as u64 * COST_WRITE);
                out.extend(cells.iter().map(|p| Change { x: p[0], y: p[1], z: p[2], state: AIR.into() }));
                self.set(si, SecState::Applied);
                self.written.push((si, cells));
            }
            self.in_pool[ci as usize] = false;
            pi += 1;
        }
        self.pool.drain(..pi);
    }

    /// Cratera e bordas prontas: começa a segunda fase.
    fn begin_fallout(&mut self, fp: &FalloutProfile, tick: u64, store: &mut dyn Store, hooks: &mut dyn Hooks) {
        hooks.event(&self.id, &Event::FalloutStart { tick: tick - self.tick0 });
        self.plan.release_all();
        self.sweep = Some(match self.resume_sweep.take() {
            Some((k, pend)) => Sweep::resume(self.plan.c, self.plan.r, fp.scale, k, pend),
            None => Sweep::new(self.plan.c, self.plan.r, fp.scale),
        });
        // O diário da cratera fica até o fim: uma retomada no meio do
        // fallout reaplica a cratera (mundo que voltou ao autosave).
        self.save(store, hooks, tick, true);
        self.fallout_saved = tick;
    }

    /// Um tick do fallout: a passada principal, anel a anel.
    fn fallout_tick(&mut self, tick: u64, world: &mut dyn World, store: &mut dyn Store, hooks: &mut dyn Hooks) {
        let Some(mut sweep) = self.sweep.take() else { return };
        let Some(fp) = self.profile.fallout.as_ref() else { return };
        let dim = self.dim.clone();
        let mut out = Vec::new();
        {
            let mut read = |x: i32, y: i32, z: i32| world.block(&dim, x, y, z);
            sweep.step(fp, self.fuel, self.budget, &mut read, &mut out);
        }
        crate::explosion::trace(20);
        let n = out.len();
        self.effects += n as u64;
        self.writes += n as u64;
        self.max_tick_writes = self.max_tick_writes.max(n);
        while !out.is_empty() {
            let rest = out.split_off(out.len().min(BLOCKS_PER_CALL));
            let _ = world.set_blocks(&dim, std::mem::replace(&mut out, rest));
        }
        self.sweep = Some(sweep);
        crate::explosion::trace(21);
        if tick >= self.fallout_saved + 20 {
            self.fallout_saved = tick;
            self.save(store, hooks, tick, true);
        }
    }

    /// Os chunks do fallout que esperam carregar, para `Linger` (depois de `done`).
    fn into_linger(self) -> Option<Linger> {
        let sweep = self.sweep?;
        if sweep.pending.is_empty() {
            return None;
        }
        Some(Linger {
            id: self.id,
            dim: self.dim,
            budget: self.budget,
            tick0: self.tick0,
            profile: self.profile,
            sweep,
            persist: self.persist.filter(|p| p.durable).map(|p| (p.prefix, p.generation, p.slot_len)),
            header: String::new(),
        })
    }

    /// Efeitos nas bordas expostas, até `budget` escritas e `fuel`. As de
    /// chunk descarregado esperam por chunk (escrever num chunk descarregado
    /// seria perdido); a cada RETRY_TICKS alguns desses chunks são conferidos,
    /// em rodízio.
    fn edge_effects(&mut self, out: &mut Vec<Change>, mut budget: usize, fuel: u64, tick: u64, world: &mut dyn World, hooks: &mut dyn Hooks) {
        let dim = self.dim.clone();
        let mut spent = 0u64;
        // Primeiro os chunks que ficaram para depois e agora carregaram.
        if !self.later_keys.is_empty() && tick >= self.edges_retry_at {
            self.edges_retry_at = tick + RETRY_TICKS;
            for _ in 0..self.later_keys.len().min(256) {
                if budget < 2 || spent >= fuel {
                    break;
                }
                let Some(key) = self.later_keys.pop_front() else { break };
                spent += COST_VERIFY;
                if world.block(&dim, key.0 * 16, self.plan.c[1], key.1 * 16).is_none() {
                    self.later_keys.push_back(key);
                    continue;
                }
                let mut idxs = self.edges_later.remove(&key).unwrap_or_default();
                while let Some(&idx) = idxs.last() {
                    if budget < 2 || spent >= fuel {
                        break;
                    }
                    idxs.pop();
                    spent += self.edge_one(idx as usize, out, &mut budget, hooks);
                }
                if !idxs.is_empty() {
                    self.edges_later.insert(key, idxs);
                    self.later_keys.push_front(key);
                }
            }
        }
        // Depois as novas, que já saem em ordem de espiral (do centro para fora).
        let c = self.plan.c;
        let mut loaded: HashMap<(i32, i32), bool, BuildFnv> = Default::default();
        while budget >= 2 && spent < fuel && self.edges_done < self.plan.edges.len() {
            let idx = self.edges_done;
            self.edges_done += 1;
            let e = self.plan.edges[idx];
            let (x, y, z) = (c[0] + e.o[0] as i32, c[1] + e.o[1] as i32, c[2] + e.o[2] as i32);
            let key = (x >> 4, z >> 4);
            spent += COST_SCAN * 3;
            let ok = *loaded.entry(key).or_insert_with(|| {
                spent += COST_VERIFY;
                world.block(&dim, key.0 * 16, y, key.1 * 16).is_some()
            });
            if !ok {
                let list = self.edges_later.entry(key).or_default();
                if list.is_empty() {
                    self.later_keys.push_back(key);
                }
                list.push(idx as u32);
                continue;
            }
            spent += self.edge_one(idx, out, &mut budget, hooks);
        }
    }

    /// O efeito de uma borda (chunk carregado); devolve o fuel estimado.
    fn edge_one(&mut self, idx: usize, out: &mut Vec<Change>, budget: &mut usize, hooks: &mut dyn Hooks) -> u64 {
        let c = self.plan.c;
        let e = self.plan.edges[idx];
        let [dx, dy, dz] = [e.o[0] as i32, e.o[1] as i32, e.o[2] as i32];
        let [x, y, z] = [c[0] + dx, c[1] + dy, c[2] + dz];
        let hit = EdgeHit {
            pos: [x, y, z],
            kind: self.plan.mats.table[e.mat as usize].kind,
            f: ((dx * dx + dy * dy + dz * dz) as f32).sqrt() / self.plan.r as f32,
        };
        let eff = hooks.edge(&self.id, &hit).unwrap_or_else(|| self.profile.apply(&hit));
        let mut n = 0;
        if let Some(s) = eff.here {
            out.push(Change { x, y, z, state: s });
            n += 1;
        }
        if let Some(s) = eff.above {
            out.push(Change { x, y: y + 1, z, state: s });
            n += 1;
        }
        *budget = budget.saturating_sub(n);
        self.effects += n as u64;
        COST_EDGE + n as u64 * COST_WRITE
    }

    fn header(&self, phase: &str) -> Header {
        let linger = phase == "linger";
        let p = self.persist.as_ref();
        let mut pending = self.reeval.clone();
        pending.extend(self.reeval_started.keys().copied());
        Header {
            id: self.id.clone(),
            dim: self.dim.clone(),
            center: self.plan.c,
            radius: self.plan.r,
            budget: self.budget,
            tick0: self.tick0,
            cols_done: if linger { 0 } else { self.plan.cols_done },
            blocks: if linger { Vec::new() } else { p.map_or_else(Vec::new, |p| p.blocks.clone()) },
            pending_chunks: if linger { Vec::new() } else { pending },
            done_chunks: if linger { Vec::new() } else { self.reeval_done.clone() },
            profile: self.profile.encode(),
            ering: if linger { [None; RING] } else { p.map_or([None; RING], Persist::ering_used) },
            lite: self.lite,
            crater_left: self.lite && self.sweep.is_some() && (self.counts[SecState::Pending as usize] > 0 || self.waiting_chunks()),
            phase: phase.into(),
            fring: self.sweep.as_ref().map_or(0, Sweep::ring_now),
            fpend: self.sweep.as_ref().map_or_else(Vec::new, |s| s.pending.clone()),
        }
    }

    /// O diário completo não coube: passa à persistência leve (só as seções
    /// escritas), apagando os blocos dele.
    fn degrade(&mut self, store: &mut dyn Store, hooks: &mut dyn Hooks, reason: String) {
        if let Some(p) = self.persist.as_mut() {
            for k in 0..p.blocks.len() {
                store.delete(&format!("{}.d{k}", p.prefix));
            }
            for k in 0..RING {
                store.delete(&format!("{}.e{k}", p.prefix));
            }
            p.blocks.clear();
            p.ering = [None; RING];
            p.cur = Vec::new();
            p.ebands.clear();
            p.durable_top = None;
            p.tail_dirty = true;
            p.later = 0;
        }
        self.lite = true;
        self.plan.journal = None;
        self.lite_bits = vec![0; self.secs.len().div_ceil(8)];
        for (si, s) in self.secs.iter().enumerate() {
            if matches!(s.state, SecState::Applied | SecState::Done) {
                self.lite_bits[si >> 3] |= 1 << (si & 7);
            }
        }
        hooks.event(&self.id, &Event::PersistFailed { reason: format!("{reason}; segue com a persistência leve (só as seções escritas)") });
    }

    /// Grava o diário novo e o estado. Devolve se tudo o que foi calculado
    /// até aqui está gravado (só então as seções novas podem ser escritas
    /// no mundo). Falha de quota passa à persistência leve; a leve que falha
    /// tenta de novo mais tarde (a explosão continua sem ela).
    fn save(&mut self, store: &mut dyn Store, hooks: &mut dyn Hooks, tick: u64, force: bool) -> bool {
        let Some(p) = self.persist.as_mut() else { return true };
        if p.retry_at.is_some_and(|at| tick < at) {
            return true;
        }
        if self.lite && !force && p.durable && tick < p.last_tick + LITE_EVERY {
            return true;
        }
        if !self.lite
            && let Some(f) = self.plan.journal.as_mut().map(std::mem::take)
        {
            if !f.structure.is_empty() {
                p.cur.extend_from_slice(&f.structure);
                p.tail_dirty = true;
            }
            for (band, rec) in f.energy {
                p.tail_dirty = true;
                match p.ebands.last_mut() {
                    Some((b, buf)) if *b == band => buf.extend_from_slice(&rec),
                    _ => p.ebands.push((band, rec)),
                }
            }
        }
        let mut allow = self.allow;
        let mut failure = None;
        let mut later = false;
        if !self.lite {
            // Blocos fechados do diário: gravados uma vez só.
            while p.cur.len() >= DATA_CHUNK {
                if p.blocks.len() >= MAX_DATA_KEYS {
                    failure = Some(format!("diário passou de {MAX_DATA_KEYS} blocos"));
                    break;
                }
                let key = format!("{}.d{}", p.prefix, p.blocks.len());
                match put(&mut allow, p.used(), store, &key, &p.cur[..DATA_CHUNK], 0) {
                    Put::Ok => {
                        p.blocks.push((DATA_CHUNK as u32, persist::fnv64(&p.cur[..DATA_CHUNK])));
                        p.cur.drain(..DATA_CHUNK);
                    }
                    Put::Later => later = true,
                    Put::Fail(e) => failure = Some(e),
                }
                if later || failure.is_some() {
                    break;
                }
            }
            // Faixas de energia fechadas: a mais antiga do anel é trocada.
            while !later && failure.is_none() && p.ebands.len() > 1 && p.durable_top.is_none_or(|t| p.ebands[0].0 <= t) {
                let (band, buf) = &p.ebands[0];
                let slot = *band as usize % RING;
                let old = p.ering[slot].map_or(0, |e| e.1.0 as usize);
                match put(&mut allow, p.used(), store, &format!("{}.e{slot}", p.prefix), buf, old) {
                    Put::Ok => {
                        p.ering[slot] = Some((*band, (buf.len() as u32, persist::fnv64(buf))));
                        p.ebands.remove(0);
                        p.tail_dirty = true;
                    }
                    Put::Later => later = true,
                    Put::Fail(e) => failure = Some(e),
                }
            }
        }
        if let Some(reason) = failure {
            self.allow = allow;
            self.degrade(store, hooks, reason);
            return self.save(store, hooks, tick, true);
        }
        if later {
            return self.later(store, hooks, tick, allow);
        }
        // O estado (cabeçalho e cauda), no slot da próxima geração.
        let phase = if self.sweep.is_some() { "fallout" } else { "crater" };
        let header = self.header(phase);
        let text = header.encode();
        let p = self.persist.as_mut().unwrap();
        if p.durable && !p.tail_dirty && text == p.last_header {
            return true;
        }
        if !p.indexed {
            match index_update(store, &self.id, true) {
                Ok(()) => p.indexed = true,
                Err(e) if e.contains("traffic") => return self.later(store, hooks, tick, allow),
                Err(e) => return self.lite_failed(hooks, tick, e),
            }
        }
        let state = State {
            header,
            data: if self.lite { Vec::new() } else { p.cur.clone() },
            energy: if self.lite { Vec::new() } else { persist::encode_bands(&p.ebands) },
            bits: if self.lite { self.lite_bits.clone() } else { Vec::new() },
        };
        let generation = p.generation + 1;
        let bytes = persist::slot_bytes(generation, &state.encode_with(&text));
        let slot = (generation % 2) as usize;
        match put(&mut allow, p.used(), store, &persist::slot_for(&format!("{}.s", p.prefix), generation), &bytes, p.slot_len[slot]) {
            Put::Ok => {
                p.generation = generation;
                p.slot_len[slot] = bytes.len();
                p.durable = true;
                p.durable_top = p.top();
                p.tail_dirty = false;
                p.last_header = text;
                p.last_tick = tick;
                p.later = 0;
                p.retry_at = None;
                self.allow = allow;
                true
            }
            Put::Later => self.later(store, hooks, tick, allow),
            Put::Fail(e) if !self.lite => {
                self.allow = allow;
                self.degrade(store, hooks, e);
                self.save(store, hooks, tick, true)
            }
            Put::Fail(e) => self.lite_failed(hooks, tick, e),
        }
    }

    /// Não coube neste tick: as escritas do mundo esperam (o diário completo
    /// tem que estar gravado antes delas); depois de muito tempo, desiste
    /// para a persistência leve.
    fn later(&mut self, store: &mut dyn Store, hooks: &mut dyn Hooks, tick: u64, allow: Allow) -> bool {
        self.allow = allow;
        let p = self.persist.as_mut().unwrap();
        p.later += 1;
        if self.lite {
            return true;
        }
        if p.later >= LATER_MAX {
            self.degrade(store, hooks, format!("{LATER_MAX} ticks sem espaço para gravar"));
            return self.save(store, hooks, tick, true);
        }
        false
    }

    fn lite_failed(&mut self, hooks: &mut dyn Hooks, tick: u64, reason: String) -> bool {
        let p = self.persist.as_mut().unwrap();
        p.retry_at = Some(tick + PERSIST_RETRY);
        if !p.warned {
            p.warned = true;
            hooks.event(&self.id, &Event::PersistFailed { reason });
        }
        true
    }

    /// O estado inicial (e o índice), logo na criação: uma explosão
    /// detonada nunca fica só na memória (revisão do M2, P1-6).
    fn ensure_durable(&mut self, store: &mut dyn Store, hooks: &mut dyn Hooks, tick: u64) {
        if self.persist.as_ref().is_some_and(|p| !p.durable) {
            self.save(store, hooks, tick, true);
        }
    }

    /// Apaga as chaves da explosão (terminada ou cancelada).
    pub fn forget(&mut self, store: &mut dyn Store) {
        let Some(p) = self.persist.take() else { return };
        purge(store, &p.prefix, &self.id, p.blocks.len());
    }
}

/// Distância (blocos) de um jogador até um chunk pendente do fallout para
/// ele ser feito (10 chunks: a distância de visão padrão).
pub const LINGER_NEAR: f64 = 160.0;
/// Ticks entre conferências dos chunks pendentes.
pub const LINGER_EVERY: u64 = 40;

/// O fallout de uma explosão terminada, nos chunks que não estavam
/// carregados: feitos quando um jogador chega perto. Pesa poucos bytes (a
/// lista de chunks), sem o plano.
pub struct Linger {
    pub id: String,
    pub dim: String,
    budget: usize,
    tick0: u64,
    profile: EffectProfile,
    sweep: Sweep,
    /// Prefixo, geração e tamanho dos slots do estado.
    persist: Option<(String, u64, [usize; 2])>,
    header: String,
}

impl Linger {
    pub fn pending(&self) -> usize {
        self.sweep.pending.len()
    }

    fn used(&self) -> (u64, usize) {
        self.persist.as_ref().map_or((0, 0), |(_, _, l)| (l.iter().sum::<usize>() as u64, l.iter().filter(|l| **l > 0).count()))
    }

    /// Até `fuel` e `writes` escritas; devolve o fuel gasto.
    fn tick(&mut self, fuel: u64, writes: &mut usize, allow: &mut Allow, world: &mut dyn World, store: &mut dyn Store) -> u64 {
        let Some(fp) = self.profile.fallout.as_ref() else { return 0 };
        let viewers = world.viewers(&self.dim);
        if viewers.is_empty() {
            return 0;
        }
        let dim = self.dim.clone();
        let mut out = Vec::new();
        let before = self.sweep.pending.len();
        let used = {
            let mut read = |x: i32, y: i32, z: i32| world.block(&dim, x, y, z);
            self.sweep.pending_step(fp, &viewers, LINGER_NEAR, fuel, (*writes).min(self.budget), &mut read, &mut out)
        };
        *writes = writes.saturating_sub(out.len());
        while !out.is_empty() {
            let rest = out.split_off(out.len().min(BLOCKS_PER_CALL));
            let _ = world.set_blocks(&dim, std::mem::replace(&mut out, rest));
        }
        if self.sweep.pending.len() == before {
            return used;
        }
        let Some((prefix, generation, slot_len)) = self.persist.as_mut() else { return used };
        if self.sweep.pending.is_empty() {
            purge(store, prefix, &self.id, 0);
            return used;
        }
        let h = Header {
            id: self.id.clone(),
            dim: self.dim.clone(),
            center: self.sweep.c,
            radius: self.sweep.r,
            budget: self.budget,
            tick0: self.tick0,
            profile: self.profile.encode(),
            phase: "linger".into(),
            fring: self.sweep.ring_now(),
            fpend: self.sweep.pending.clone(),
            ..Header::default()
        };
        let text = h.encode();
        if text != self.header {
            let bytes = persist::slot_bytes(*generation + 1, &State { header: h, ..State::default() }.encode());
            let slot = ((*generation + 1) % 2) as usize;
            let total = (slot_len.iter().sum::<usize>() as u64, slot_len.iter().filter(|l| **l > 0).count());
            if let Put::Ok = put(allow, total, store, &persist::slot_for(&format!("{prefix}.s"), *generation + 1), &bytes, slot_len[slot]) {
                *generation += 1;
                slot_len[slot] = bytes.len();
                self.header = text;
            }
        }
        used
    }
}

/// O que começa num tick: uma detonação nova ou uma retomada (cada uma
/// monta um plano, que em R=500 custa ~21 M de fuel: uma por tick).
enum Start {
    New(Box<Detonation>),
    Resume(String),
}

/// As explosões ativas de um mod. Uma por vez recebe o tick (o fuel do tick é
/// dela); as outras esperam na fila.
pub struct Explosions {
    /// Fuel estimado por tick (o teto do Horizon é 50 M, e o resto do mod
    /// gasta o dele): padrão [`FUEL_PLAN`].
    pub fuel: u64,
    /// O que podem gravar no storage (o mod divide o dele).
    pub limits: Limits,
    jobs: Vec<Explosion>,
    /// Fallouts terminados com chunks que esperam um jogador chegar perto.
    lingering: Vec<Linger>,
    starts: VecDeque<Start>,
}

impl Default for Explosions {
    fn default() -> Self {
        Self { fuel: FUEL_PLAN, limits: Limits::default(), jobs: Vec::new(), lingering: Vec::new(), starts: VecDeque::new() }
    }
}

impl Explosions {
    pub fn new() -> Self {
        Self::default()
    }

    /// A explosão `id` está aqui (ativa, pendente, na fila ou retomando).
    pub fn contains(&self, id: &str) -> bool {
        self.jobs.iter().any(|j| j.id == id)
            || self.lingering.iter().any(|l| l.id == id)
            || self.starts.iter().any(|s| match s {
                Start::New(d) => d.id == id,
                Start::Resume(r) => r == id,
            })
    }

    /// A cratera de `id` ainda está sendo cavada: na fila, retomando, ou ativa sem a cratera concluída.
    pub fn digging(&self, id: &str) -> bool {
        self.jobs.iter().any(|j| j.id == id && j.crater_done.is_none() && !j.done())
            || self.starts.iter().any(|s| match s {
                Start::New(d) => d.id == id,
                Start::Resume(r) => r == id,
            })
    }

    /// Põe uma explosão na fila: o plano é montado num tick só para ele
    /// (no máximo um por tick), e o estado inicial é gravado nele.
    pub fn detonate(&mut self, det: Detonation, _tick: u64) -> Result<(), String> {
        Explosion::check(&det)?;
        if self.contains(&det.id) {
            return Err(format!("já existe a explosão {}", det.id));
        }
        self.starts.push_back(Start::New(Box::new(det)));
        Ok(())
    }

    /// A explosão `id` já está gravada (ou não tem como estar): quem a
    /// detonou pode esquecer a dele.
    pub fn durable(&self, id: &str) -> bool {
        self.lingering.iter().any(|l| l.id == id)
            || self.jobs.iter().any(|j| j.id == id && j.persist.as_ref().is_none_or(|p| p.durable || p.retry_at.is_some()))
    }

    /// Retoma todas as explosões listadas no storage; devolve as retomadas e os erros.
    pub fn resume(&mut self, store: &mut dyn Store, tick: u64, hooks: &mut dyn Hooks) -> Vec<Result<String, String>> {
        self.resume_matching(store, tick, hooks, |_| true)
    }

    /// Como [`resume`](Self::resume), só os ids que `keep` aceita (um mod com
    /// vários mundos retoma só as explosões do mundo em jogo). Só lê o
    /// índice: cada uma volta num tick dela (ver [`Explosions::tick`]).
    pub fn resume_matching(&mut self, store: &mut dyn Store, _tick: u64, _hooks: &mut dyn Hooks, keep: impl Fn(&str) -> bool) -> Vec<Result<String, String>> {
        let (_, ids) = persist::read_index(store);
        let mut out = Vec::new();
        for id in ids.into_iter().filter(|l| keep(l)) {
            if self.contains(&id) {
                continue;
            }
            self.starts.push_back(Start::Resume(id.clone()));
            out.push(Ok(id));
        }
        out
    }

    /// Descarta as retomadas ainda na cratera (fase `crater`; o fallout, `fallout` ou `linger`, é de uma
    /// `EntityFalloutRain`, que guarda seu `scale`) cujo id `keep` não aceita: o que uma entidade que não guarda seu
    /// estado deixa (`EntityNukeExplosionMK5`). Devolve os ids descartados.
    pub fn drop_craters(&mut self, store: &mut dyn Store, keep: impl Fn(&str) -> bool) -> Vec<String> {
        let mut out = Vec::new();
        let mut kept = VecDeque::new();
        for start in std::mem::take(&mut self.starts) {
            if let Start::Resume(id) = &start
                && !keep(id)
                && load(store, &persist::prefix(id)).is_ok_and(|l| l.state.header.phase == "crater")
            {
                purge(store, &persist::prefix(id), id, MAX_DATA_KEYS);
                out.push(id.clone());
                continue;
            }
            kept.push_back(start);
        }
        self.starts = kept;
        out
    }

    /// O que todas ocupam no storage (bytes, chaves), com o índice.
    pub fn stored(&self) -> (u64, usize) {
        let mut b = 0;
        let mut k = 2;
        for (bb, kk) in self.jobs.iter().filter_map(|j| j.persist.as_ref().map(Persist::used)).chain(self.lingering.iter().map(Linger::used)) {
            b += bb;
            k += kk;
        }
        (b, k)
    }

    /// O que a explosão `skip` (ou um linger, `None`) pode usar: os limites
    /// menos o que as outras ocupam.
    fn allow_for(&self, skip: Option<usize>, tick_left: u64) -> Allow {
        let (mut b, mut k) = (0u64, 2usize);
        for (i, j) in self.jobs.iter().enumerate() {
            if Some(i) != skip
                && let Some(p) = &j.persist
            {
                let (bb, kk) = p.used();
                b += bb;
                k += kk;
            }
        }
        for l in &self.lingering {
            let (bb, kk) = l.used();
            b += bb;
            k += kk;
        }
        Allow { tick: tick_left, tick_max: self.limits.tick_bytes, bytes: self.limits.total_bytes.saturating_sub(b), keys: self.limits.keys.saturating_sub(k) }
    }

    /// Avança a primeira explosão da fila; devolve o progresso final das que terminaram.
    pub fn tick(&mut self, tick: u64, world: &mut dyn World, store: &mut dyn Store, hooks: &mut dyn Hooks) -> Vec<Progress> {
        let mut tick_left = self.limits.tick_bytes;
        // Um plano novo (ou retomado) é o trabalho inteiro do tick.
        if let Some(start) = self.starts.pop_front() {
            self.start(start, tick, store, hooks);
            return Vec::new();
        }
        // A primeira que tem trabalho recebe o tick; as que só esperam chunks
        // (`idle`) são tentadas quando nenhuma tem trabalho.
        let pick = match self.jobs.iter().position(|j| !j.idle()) {
            Some(i) => Some(i),
            // Uma por tick, em rodízio (o fuel do tick é de uma só).
            None if !self.jobs.is_empty() => Some((tick % self.jobs.len() as u64) as usize),
            None => None,
        };
        if let Some(i) = pick {
            let allow = self.allow_for(Some(i), tick_left);
            let job = &mut self.jobs[i];
            job.fuel = self.fuel;
            job.allow = allow;
            job.tick(tick, world, store, hooks);
        } else if tick.is_multiple_of(LINGER_EVERY) && !self.lingering.is_empty() {
            // Sem explosão ativa: o fuel e as escritas do tick vão para os
            // fallouts pendentes, divididos (em rodízio, quem começa muda).
            let n = self.lingering.len();
            let first = (tick / LINGER_EVERY) as usize % n;
            let mut fuel = self.fuel;
            let mut writes = DEFAULT_BUDGET;
            for k in 0..n {
                let i = (first + k) % n;
                let mut allow = self.allow_for(None, tick_left);
                let used = self.lingering[i].tick(fuel, &mut writes, &mut allow, world, store);
                tick_left = allow.tick;
                fuel = fuel.saturating_sub(used);
                if fuel < 2_000_000 || writes < 64 {
                    break;
                }
            }
            self.lingering.retain(|l| l.pending() > 0);
        }
        let done: Vec<Progress> = self.jobs.iter().filter(|j| j.done()).map(Explosion::progress).collect();
        let (finished, jobs): (Vec<Explosion>, Vec<Explosion>) = std::mem::take(&mut self.jobs).into_iter().partition(Explosion::done);
        self.jobs = jobs;
        self.lingering.extend(finished.into_iter().filter_map(Explosion::into_linger));
        done
    }

    fn start(&mut self, start: Start, tick: u64, store: &mut dyn Store, hooks: &mut dyn Hooks) {
        let tick_left = self.limits.tick_bytes;
        match start {
            Start::New(det) => {
                let id = det.id.clone();
                match Explosion::new(*det, tick) {
                    Ok(mut ex) => {
                        ex.fuel = self.fuel;
                        ex.allow = self.allow_for(None, tick_left);
                        ex.ensure_durable(store, hooks, tick);
                        self.jobs.push(ex);
                    }
                    Err(reason) => hooks.event(&id, &Event::ResumeFailed { reason }),
                }
            }
            Start::Resume(id) => {
                let prefix = persist::prefix(&id);
                let loaded = load(store, &prefix);
                match loaded {
                    Ok(l) if l.state.header.phase == "linger" => {
                        let h = &l.state.header;
                        match EffectProfile::decode(&h.profile) {
                            Ok(profile) => {
                                let scale = profile.fallout.as_ref().map_or(2.5, |f| f.scale);
                                self.lingering.push(Linger {
                                    id: id.clone(),
                                    dim: h.dim.clone(),
                                    budget: h.budget,
                                    tick0: h.tick0,
                                    sweep: Sweep::resume(h.center, h.radius, scale, h.fring, h.fpend.clone()),
                                    profile,
                                    persist: Some((prefix, l.generation, [l.len + 64; 2])),
                                    header: h.encode(),
                                });
                            }
                            Err(reason) => {
                                hooks.event(&id, &Event::ResumeFailed { reason });
                                purge(store, &prefix, &id, 0);
                            }
                        }
                    }
                    Ok(l) => {
                        let blocks = l.state.header.blocks.len();
                        match Explosion::resume_from(l, tick) {
                            Ok(mut ex) => {
                                hooks.event(&id, &Event::Resumed { columns: ex.plan.cols_done });
                                ex.fuel = self.fuel;
                                self.jobs.push(ex);
                            }
                            Err(reason) => {
                                hooks.event(&id, &Event::ResumeFailed { reason });
                                purge(store, &prefix, &id, blocks);
                            }
                        }
                    }
                    Err(reason) => {
                        // Sem estado inteiro (uma queda antes da primeira
                        // gravação, ou no meio de apagar): as chaves saem.
                        hooks.event(&id, &Event::ResumeFailed { reason });
                        purge(store, &prefix, &id, MAX_DATA_KEYS);
                    }
                }
            }
        }
    }

    /// Fallouts com chunks esperando carregar.
    pub fn lingering(&self) -> impl Iterator<Item = (&str, usize)> + '_ {
        self.lingering.iter().map(|l| (l.id.as_str(), l.pending()))
    }

    /// Esquece tudo da memória sem tocar no storage (o mod saiu do mundo:
    /// as explosões dele continuam quando ele voltar).
    pub fn unload(&mut self) {
        self.jobs.clear();
        self.lingering.clear();
        self.starts.clear();
    }

    pub fn progress(&self, id: &str) -> Option<Progress> {
        self.jobs.iter().find(|j| j.id == id).map(Explosion::progress)
    }

    pub fn active(&self) -> impl Iterator<Item = Progress> + '_ {
        self.jobs.iter().map(Explosion::progress)
    }

    /// Detonações e retomadas que ainda não montaram o plano.
    pub fn queued(&self) -> usize {
        self.starts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty() && self.starts.is_empty()
    }

    /// Para uma explosão (o que já foi escrito fica) e apaga o diário.
    pub fn cancel(&mut self, id: &str, store: &mut dyn Store) -> bool {
        if let Some(i) = self.starts.iter().position(|s| matches!(s, Start::New(d) if d.id == id) || matches!(s, Start::Resume(r) if r == id)) {
            if let Some(Start::Resume(_)) = self.starts.remove(i) {
                purge(store, &persist::prefix(id), id, MAX_DATA_KEYS);
            }
            return true;
        }
        if let Some(i) = self.lingering.iter().position(|l| l.id == id) {
            let l = self.lingering.remove(i);
            if let Some((p, ..)) = l.persist {
                purge(store, &p, &l.id, 0);
            }
            return true;
        }
        let Some(i) = self.jobs.iter().position(|j| j.id == id) else { return false };
        let mut ex = self.jobs.remove(i);
        ex.forget(store);
        true
    }
}
