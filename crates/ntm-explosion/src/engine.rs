//! Uma explosão em andamento, amarrada ao mundo por traits.
//!
//! Por tick: (1) confere o que foi escrito no tick anterior (releitura) e
//! reescreve o que não pegou; (2) avança a varredura de energia com o fuel que
//! sobra; (3) grava o diário das colunas novas (persistência); (4) escreve
//! seções 16³ já decididas, mais perto de um jogador primeiro e de cima para
//! baixo, sem passar do orçamento de escritas (≤ 60 000 < 65 536 da fila global
//! do Horizon, em chamadas de ≤ 4096); (5) no fim, efeitos nas bordas.
use crate::crater::{Kind, MAX_RADIUS, MIN_Y, MAX_Y, Plan};
use crate::effects::{EdgeHit, EffectProfile, Event, Hooks};
use crate::fallout::{FalloutProfile, Sweep};
use crate::persist::{self, DATA_CHUNK, Header, INDEX_KEY, JOURNAL_MAX_RADIUS, Journal, MAX_DATA_KEYS, RING, Store};

/// Fuel do hook de tick do Horizon: 50 M (horizon-mods runtime.rs:110).
/// Estourar desliga o mod; o motor planeja com estimativa e margem.
pub const FUEL_PLAN: u64 = 34_000_000;
/// Fuel estimado por escrita (string + lista), por voxel examinado numa seção
/// e por releitura de conferência (calibrados no spike S7, com o alocador pool).
const COST_WRITE: u64 = 700;
const COST_SCAN: u64 = 60;
const COST_VERIFY: u64 = 1_000;
const RETRY_TICKS: u64 = 20;
/// Depois de uma retomada, a varredura espera até este tanto (60 s) pelos
/// chunks da frente: o servidor recém-iniciado ainda está carregando, e uma
/// coluna lida "sem chunk" vira ar (aproximação que, depois de retomar, não
/// tem mais a energia exata das colunas antigas para recalcular).
const RESUME_WAIT_TICKS: u64 = 1200;
/// Orçamento padrão de escritas por tick (o melhor equilíbrio medido no S7).
pub const DEFAULT_BUDGET: usize = 8192;
pub const BLOCKS_PER_CALL: usize = 4096;
pub const AIR: &str = "minecraft:air";

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
    Pending,
    Applied,
    Done,
    Deferred,
}

struct Section {
    /// Coordenadas da seção (x>>4, y>>4, z>>4).
    s: [i32; 3],
    /// Maior h² das colunas da seção: decidida quando a frente da espiral passa dele.
    ready_h2: u64,
    state: SecState,
    retry_at: u64,
}

struct Persist {
    prefix: String,
    /// Bloco do diário em curso (o último `d<k>`), e quantos já fecharam.
    cur: Vec<u8>,
    closed: usize,
    dirty: bool,
    header: String,
    bytes: usize,
    failed: bool,
    /// Anel de energia: faixa em curso, o buffer dela e a faixa de cada chave.
    eband: Option<u32>,
    ebuf: Vec<u8>,
    edirty: bool,
    ring: [Option<u32>; RING],
}

/// O diário lido do storage, restaurado aos poucos.
struct Restoring {
    data: Vec<u8>,
    pos: usize,
    /// Colunas já vistas (a 1ª ocorrência é da passada principal).
    seen: Vec<bool>,
    energy: Vec<Vec<u8>>,
    cols_done: u32,
    done_chunks: Vec<(i32, i32)>,
}

pub struct Explosion {
    pub id: String,
    pub dim: String,
    plan: Plan,
    profile: EffectProfile,
    budget: usize,
    secs: Vec<Section>,
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
    /// Chunks que estavam sem carregar e esperam carga, e os já postos na fila
    /// de recálculo do plano (com a posição em que terminam).
    reeval: Vec<(i32, i32)>,
    reeval_started: Vec<((i32, i32), usize)>,
    reeval_done: Vec<(i32, i32)>,
    persist: Option<Persist>,
    pub max_tick_writes: usize,
    /// Retomada: até quando esperar chunks da frente (definido no 1º tick).
    resumed: bool,
    wait_until: Option<u64>,
    /// Bordas cujo chunk estava descarregado na hora do efeito (nova tentativa).
    edges_later: Vec<usize>,
    edges_retry_at: u64,
    pub waited_ticks: u64,
    /// Segunda fase (fallout), depois da cratera e das bordas.
    sweep: Option<Sweep>,
    fallout_saved: u64,
    /// Retomada de um fallout com diário: anel e pendentes de onde continuar
    /// depois que a cratera for reaplicada.
    resume_sweep: Option<(i32, Vec<(i32, i32)>)>,
    /// Fuel estimado que um tick pode gastar (ver [`Explosions::fuel`]).
    pub fuel: u64,
    /// Retomada em andamento (o diário restaurado aos poucos).
    restoring: Option<Box<Restoring>>,
    pub restore_error: Option<String>,
    /// Persistência leve (R > JOURNAL_MAX_RADIUS): só as seções escritas.
    lite: bool,
    lite_bits: Vec<u8>,
    lite_saved: u64,
}

fn sections_for(plan: &Plan) -> Vec<Section> {
    let (c, r) = (plan.c, plan.r);
    let mut secs = Vec::new();
    let (y0, y1) = ((c[1] - r).max(MIN_Y), (c[1] + r).min(MAX_Y));
    for sx in (c[0] - r) >> 4..=(c[0] + r) >> 4 {
        for sz in (c[2] - r) >> 4..=(c[2] + r) >> 4 {
            let nx = c[0].clamp(sx * 16, sx * 16 + 15) - c[0];
            let nz = c[2].clamp(sz * 16, sz * 16 + 15) - c[2];
            if nx * nx + nz * nz > r * r {
                continue;
            }
            let ready_h2 = plan.section_h2([sx, 0, sz]);
            for sy in y0 >> 4..=y1 >> 4 {
                secs.push(Section { s: [sx, sy, sz], ready_h2, state: SecState::Pending, retry_at: 0 });
            }
        }
    }
    secs
}

impl Explosion {
    /// Monta o plano (o tick da criação não avança: no Horizon, montar R=150
    /// custa ~21 M de fuel).
    pub fn new(det: Detonation, tick: u64) -> Result<Self, String> {
        if !(1..=MAX_RADIUS).contains(&det.radius) {
            return Err(format!("raio {} fora de 1..={MAX_RADIUS}", det.radius));
        }
        if det.id.is_empty() || det.id.contains(['.', '\n', ' ']) {
            return Err(format!("id inválido: {:?}", det.id));
        }
        let mut plan = Plan::new(det.center, det.radius);
        let lite = det.persist && det.radius > det.lite_above;
        if det.persist && !lite {
            plan.journal = Some(Journal::default());
        }
        let secs = sections_for(&plan);
        let c = det.center;
        Ok(Self {
            persist: det.persist.then(|| Persist {
                prefix: persist::prefix(&det.id),
                cur: Vec::new(),
                closed: 0,
                dirty: !lite,
                header: String::new(),
                bytes: 0,
                failed: false,
                eband: None,
                ebuf: Vec::new(),
                edirty: false,
                ring: [None; RING],
            }),
            id: det.id,
            dim: det.dim,
            plan,
            profile: det.profile,
            budget: det.budget.clamp(256, 60_000),
            secs,
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
            reeval_started: Vec::new(),
            reeval_done: Vec::new(),
            max_tick_writes: 0,
            resumed: false,
            wait_until: None,
            edges_later: Vec::new(),
            edges_retry_at: 0,
            waited_ticks: 0,
            sweep: None,
            fallout_saved: 0,
            resume_sweep: None,
            fuel: FUEL_PLAN,
            restoring: None,
            restore_error: None,
            lite,
            lite_bits: Vec::new(),
            lite_saved: 0,
        })
    }

    /// Retoma a explosão `id` do storage: o plano volta até a última coluna
    /// gravada; todas as seções voltam a pendentes (ver `persist`).
    pub fn resume(store: &mut dyn Store, id: &str, tick: u64) -> Result<Self, String> {
        let prefix = persist::prefix(id);
        let text = store.get(&format!("{prefix}.h")).ok_or("sem cabeçalho")?;
        let h = Header::decode(&String::from_utf8_lossy(&text))?;
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
        if h.phase == "fallout" && !h.lite {
            // Com diário, a cratera é reaplicada (cobre um mundo que voltou ao
            // último autosave) e o fallout continua do anel salvo.
            ex.resume_sweep = Some((h.fring, h.fpend.clone()));
        } else if h.phase == "fallout" {
            // Leve: a cratera acabou antes do reinício; só o fallout continua.
            ex.crater_done = Some(tick);
            for s in &mut ex.secs {
                s.state = SecState::Done;
            }
            let fp = ex.profile.fallout.clone().ok_or("fallout sem perfil")?;
            ex.sweep = Some(Sweep::resume(h.center, h.radius, fp.scale, h.fring, h.fpend.clone()));
            if let Some(p) = ex.persist.as_mut() {
                p.header = String::from_utf8_lossy(&text).to_string();
            }
            return Ok(ex);
        }
        if h.lite {
            // Leve: o plano é refeito; as seções já escritas não são reescritas.
            // O que elas removeram virou ar, então a cratera além delas pode
            // sair um pouco maior (aceito; docs/explosoes.md §9).
            let bits = store.get(&format!("{prefix}.w")).unwrap_or_default();
            for (i, s) in ex.secs.iter_mut().enumerate() {
                if bits.get(i >> 3).is_some_and(|b| b >> (i & 7) & 1 == 1) {
                    s.state = SecState::Done;
                }
            }
            ex.lite_bits = bits;
            return Ok(ex);
        }
        // Estrutura: colunas da passada principal (antes da frente) e as
        // recalculadas dos chunks já terminados; o resto é refeito. A leitura
        // é aqui; a restauração vai em passos, nos ticks seguintes (R=120 tem
        // ~45 mil colunas: num tick só, passava dos 50 M de fuel).
        let mut data = Vec::new();
        let mut cur = Vec::new();
        for k in 0..h.data_keys {
            let part = store.get(&format!("{prefix}.d{k}")).ok_or(format!("falta {prefix}.d{k}"))?;
            if k + 1 == h.data_keys {
                cur = part.clone();
            }
            data.extend_from_slice(&part);
        }
        let mut eband = None;
        let mut ebuf = Vec::new();
        let mut energy = Vec::new();
        for (slot, band) in h.ring.iter().enumerate() {
            let Some(band) = band else { continue };
            let part = store.get(&format!("{prefix}.e{slot}")).ok_or(format!("falta {prefix}.e{slot}"))?;
            if eband.is_none_or(|b| *band > b) {
                eband = Some(*band);
                ebuf = part.clone();
            }
            energy.push(part);
        }
        let plan = &mut ex.plan;
        // A frente da espiral primeiro: "calculada" é antes dela.
        plan.set_cols_done(h.cols_done);
        for &key in h.pending_chunks.iter().chain(&h.done_chunks) {
            plan.mark_unloaded(key);
        }
        ex.reeval = h.pending_chunks.clone();
        ex.reeval_done = h.done_chunks.clone();
        if let Some(p) = ex.persist.as_mut() {
            p.closed = h.data_keys.saturating_sub(1);
            p.bytes = data.len().saturating_sub(cur.len());
            p.cur = cur;
            p.ring = h.ring;
            p.eband = eband;
            p.ebuf = ebuf;
            // O cabeçalho só é regravado depois de restaurar.
            p.header = String::from_utf8_lossy(&text).to_string();
        }
        let seen = vec![false; ex.plan.columns()];
        ex.restoring = Some(Box::new(Restoring { data, pos: 0, seen, energy, cols_done: h.cols_done, done_chunks: h.done_chunks }));
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
        let count = |st: SecState| self.secs.iter().filter(|s| s.state == st).count();
        let rel = |t: Option<u64>| t.map(|t| t.saturating_sub(self.tick0));
        let deferred = count(SecState::Deferred);
        let pending = self.secs.iter().any(|s| s.state == SecState::Pending);
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
            sections_done: count(SecState::Done),
            sections: self.secs.len(),
            deferred,
            requeued: self.requeued,
            physics: self.physics,
            unloaded_reads: self.plan.unloaded,
            first_write: rel(self.first_write),
            crater_done: rel(self.crater_done),
            done: rel(self.done_tick),
            persisted_bytes: self.persist.as_ref().map_or(0, |p| p.bytes + p.cur.len() + p.ebuf.len() + p.header.len()) + self.lite_bits.len(),
            fallout_columns: self.sweep.as_ref().map_or(0, |s| s.columns),
            fallout_pending: self.sweep.as_ref().map_or(0, |s| s.pending.len()),
        }
    }

    fn priority(&self, s: &Section) -> f64 {
        // Coluna de chunk mais perto (na horizontal) de um jogador primeiro.
        let cx = s.s[0] as f64 * 16.0 + 8.0;
        let cz = s.s[2] as f64 * 16.0 + 8.0;
        self.viewers.iter().map(|v| (v[0] - cx).powi(2) + (v[2] - cz).powi(2)).fold(f64::MAX, f64::min)
    }

    /// Um chunk que estava sem carregar só é escrito depois de recalculado.
    fn chunk_ready(&self, key: (i32, i32)) -> bool {
        if self.reeval.contains(&key) {
            return false;
        }
        match self.reeval_started.iter().find(|(k, _)| *k == key) {
            Some((_, end)) => self.plan.queue_done >= *end,
            None => true,
        }
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
            self.check_done(tick, store, hooks);
            return;
        }
        self.crater_tick(tick, world, store, hooks);
    }

    /// Só falta o que está em chunks que não estavam carregados (seções
    /// adiadas, recálculos, bordas para depois).
    fn waiting_chunks(&self) -> bool {
        self.secs.iter().any(|s| s.state == SecState::Deferred)
            || !self.reeval.is_empty()
            || !self.reeval_started.is_empty()
            || !self.edges_later.is_empty()
    }

    /// Nada a fazer enquanto um jogador não chegar perto dos chunks que
    /// faltam: não segura a vez das outras explosões.
    pub fn idle(&self) -> bool {
        self.crater_done.is_some()
            && self.written.is_empty()
            && !self.secs.iter().any(|s| s.state == SecState::Pending)
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
        let pending = sweep.pending.len();
        self.done_tick = Some(tick);
        hooks.event(&self.id, &Event::Done { tick: tick - self.tick0 });
        if pending == 0 {
            self.forget(store);
        } else {
            self.drop_journal(store);
            self.save_fallout(store, hooks, "linger");
        }
    }

    fn crater_tick(&mut self, tick: u64, world: &mut dyn World, store: &mut dyn Store, hooks: &mut dyn Hooks) {
        let dim = self.dim.clone();
        if self.resumed && self.wait_until.is_none() {
            self.wait_until = Some(tick + RESUME_WAIT_TICKS);
        }
        if (tick - self.started) % 10 == 1 {
            let v = world.viewers(&dim);
            if !v.is_empty() {
                self.viewers = v;
            }
        }
        let mut fuel = self.fuel;

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
            self.secs[si].state = SecState::Done;
        }

        // Reserva de fuel só para as escritas já prontas (a varredura é o
        // gargalo: reservar o orçamento inteiro a deixaria mais lenta).
        let mut budget = self.budget.saturating_sub(out.len());
        let front_before = self.plan.front_h2();
        let mut ready_cells = 0usize;
        for sec in &self.secs {
            if sec.state == SecState::Pending && sec.ready_h2 < front_before {
                let [sx, sy, sz] = sec.s;
                if let Some(i) = self.plan.sec_index(sx * 16, sy * 16, sz * 16) {
                    ready_cells += self.plan.sec_count[i] as usize;
                    if ready_cells >= budget {
                        break;
                    }
                }
            }
        }
        let reserve = ready_cells.min(budget) as u64 * (COST_WRITE + 4 * COST_SCAN) + 2_000_000;

        // Chunks que estavam sem carregar e agora carregaram: para a fila de
        // recálculo (até 4 a cada 5 ticks; mais perto do centro primeiro).
        if self.plan.finished() && !self.reeval.is_empty() && (tick - self.started).is_multiple_of(5) {
            let mut begun = 0;
            let mut i = 0;
            while i < self.reeval.len() && begun < 4 {
                let key = self.reeval[i];
                if world.block(&dim, key.0 * 16, self.plan.c[1], key.1 * 16).is_some() {
                    let end = self.plan.reeval_begin(key);
                    self.reeval_started.push((key, end));
                    self.reeval.remove(i);
                    begun += 1;
                } else {
                    i += 1;
                }
            }
        }

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
                for &k in &self.plan.unloaded_chunks {
                    if !self.reeval.contains(&k) && !self.reeval_done.contains(&k) && !self.reeval_started.iter().any(|(s, _)| *s == k) {
                        self.reeval.push(k);
                    }
                }
                let reeval = self.reeval.clone();
                for s in &mut self.secs {
                    if reeval.contains(&(s.s[0], s.s[2])) && s.state == SecState::Pending {
                        s.state = SecState::Deferred;
                        s.retry_at = tick;
                    }
                }
            }
        }
        // Chunks recalculados por inteiro.
        let qd = self.plan.queue_done;
        let mut i = 0;
        while i < self.reeval_started.len() {
            if qd >= self.reeval_started[i].1 {
                let (key, _) = self.reeval_started.swap_remove(i);
                self.reeval_done.push(key);
            } else {
                i += 1;
            }
        }

        // (3) Diário: grava as colunas novas antes de qualquer escrita delas.
        self.save(store, hooks, tick);

        // (4) Escrita das seções prontas.
        let front = self.plan.front_h2();
        let finished = self.plan.finished();
        if finished && self.plan.queue_idle() && self.reeval.is_empty() && self.reeval_started.is_empty() && self.plan.live_buffers() > 0 {
            // Nada mais a recalcular: a energia guardada não serve mais.
            self.plan.release_all();
        }
        let mut ready: Vec<(f64, i32, usize)> = self
            .secs
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                (s.state == SecState::Pending && s.ready_h2 < front)
                    || (s.state == SecState::Deferred && s.retry_at <= tick && finished && self.chunk_ready((s.s[0], s.s[2])))
            })
            .map(|(i, s)| (self.priority(s), -s.s[1], i))
            .collect();
        // Mais perto de um jogador primeiro; na mesma coluna, de cima para baixo.
        ready.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut touched: Option<(i32, i32)> = None;
        for (_, _, si) in ready {
            if budget == 0 || fuel < 4096 * COST_SCAN {
                break;
            }
            let [sx, sy, sz] = self.secs[si].s;
            let empty = self.plan.sec_index(sx * 16, sy * 16, sz * 16).is_some_and(|i| self.plan.sec_count[i] == 0);
            if empty {
                if self.secs[si].state == SecState::Pending && self.plan.was_unloaded((sx, sz)) && !self.reeval_done.contains(&(sx, sz)) {
                    // "Vazia" só porque o chunk não estava carregado: espera o recálculo.
                    self.secs[si].state = SecState::Deferred;
                    self.secs[si].retry_at = tick + RETRY_TICKS;
                } else {
                    self.secs[si].state = SecState::Done;
                }
                continue;
            }
            // Chunk carregado? (`block` só responde em chunk carregado.)
            fuel = fuel.saturating_sub(COST_VERIFY);
            if world.block(&dim, sx * 16, sy * 16, sz * 16).is_none() {
                self.secs[si].state = SecState::Deferred;
                self.secs[si].retry_at = tick + RETRY_TICKS;
                continue;
            }
            fuel = fuel.saturating_sub(4096 * COST_SCAN);
            let cells = self.plan.section_cells(self.secs[si].s);
            if cells.len() as u64 * COST_WRITE > fuel {
                break;
            }
            if cells.len() > budget && !out.is_empty() {
                break; // não cabe neste tick; volta no próximo
            }
            if touched.is_some_and(|t| t != (sx, sz)) && cells.len() * 2 > budget {
                break; // não começa outra coluna de chunk pela metade
            }
            touched = Some((sx, sz));
            budget = budget.saturating_sub(cells.len());
            fuel = fuel.saturating_sub(cells.len() as u64 * COST_WRITE);
            out.extend(cells.iter().map(|p| Change { x: p[0], y: p[1], z: p[2], state: AIR.into() }));
            self.secs[si].state = SecState::Applied;
            self.written.push((si, cells));
        }

        // (5) Efeitos depois da cratera inteira.
        let pending = self.secs.iter().any(|s| s.state == SecState::Pending);
        if finished && self.plan.queue_idle() && !pending && self.written.is_empty() && self.crater_done.is_none() {
            self.crater_done = Some(tick);
            hooks.event(&self.id, &Event::CraterDone { tick: tick - self.tick0 });
        }
        if self.crater_done.is_some() && budget > 0 {
            let budget = budget.min((fuel / (COST_WRITE + 200)) as usize);
            self.edge_effects(&mut out, budget, tick, world, hooks);
        }

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

    /// Cratera e bordas prontas: o diário da cratera sai do storage e começa
    /// a segunda fase.
    fn begin_fallout(&mut self, fp: &FalloutProfile, tick: u64, store: &mut dyn Store, hooks: &mut dyn Hooks) {
        hooks.event(&self.id, &Event::FalloutStart { tick: tick - self.tick0 });
        self.plan.release_all();
        self.sweep = Some(match self.resume_sweep.take() {
            Some((k, pend)) => Sweep::resume(self.plan.c, self.plan.r, fp.scale, k, pend),
            None => Sweep::new(self.plan.c, self.plan.r, fp.scale),
        });
        // O diário da cratera fica até o fim: uma retomada no meio do
        // fallout reaplica a cratera (mundo que voltou ao autosave).
        self.save_fallout(store, hooks, "fallout");
        self.fallout_saved = tick;
    }

    /// Apaga o diário da cratera (fica só o cabeçalho, para os pendentes).
    fn drop_journal(&mut self, store: &mut dyn Store) {
        if let Some(p) = self.persist.as_mut() {
            for k in 0..=p.closed {
                store.delete(&format!("{}.d{k}", p.prefix));
            }
            for k in 0..RING {
                store.delete(&format!("{}.e{k}", p.prefix));
            }
            store.delete(&format!("{}.w", p.prefix));
            p.closed = 0;
            p.cur.clear();
            p.ring = [None; RING];
            p.header.clear();
        }
    }

    fn fallout_header(&self, phase: &str) -> Header {
        let mut h = self.header();
        h.phase = phase.into();
        if phase == "linger" {
            h.data_keys = 0;
            h.cols_done = 0;
            h.ring = [None; RING];
            h.pending_chunks.clear();
            h.done_chunks.clear();
        }
        h
    }

    fn save_fallout(&mut self, store: &mut dyn Store, hooks: &mut dyn Hooks, phase: &str) {
        let header = self.fallout_header(phase).encode();
        let Some(p) = self.persist.as_mut() else { return };
        if p.failed || header == p.header {
            return;
        }
        match store.set(&format!("{}.h", p.prefix), header.as_bytes()) {
            Ok(()) => p.header = header,
            Err(reason) => {
                p.failed = true;
                hooks.event(&self.id, &Event::PersistFailed { reason });
            }
        }
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
        let n = out.len();
        self.effects += n as u64;
        self.writes += n as u64;
        self.max_tick_writes = self.max_tick_writes.max(n);
        while !out.is_empty() {
            let rest = out.split_off(out.len().min(BLOCKS_PER_CALL));
            let _ = world.set_blocks(&dim, std::mem::replace(&mut out, rest));
        }
        self.sweep = Some(sweep);
        if tick >= self.fallout_saved + 20 {
            self.fallout_saved = tick;
            self.save_fallout(store, hooks, "fallout");
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
            prefix: self.persist.as_ref().map(|p| p.prefix.clone()),
            header: self.persist.map(|p| p.header).unwrap_or_default(),
        })
    }

    fn edge_effects(&mut self, out: &mut Vec<Change>, mut budget: usize, tick: u64, world: &mut dyn World, hooks: &mut dyn Hooks) {
        let c = self.plan.c;
        let r = self.plan.r as f32;
        let dim = self.dim.clone();
        // Chunk carregado? (uma leitura por chunk por tick; escrever num chunk
        // descarregado seria perdido).
        let mut loaded: std::collections::HashMap<(i32, i32), bool> = Default::default();
        // Primeiro as que ficaram para depois, a cada RETRY_TICKS.
        let mut queue: Vec<usize> = Vec::new();
        if !self.edges_later.is_empty() && tick >= self.edges_retry_at {
            queue = std::mem::take(&mut self.edges_later);
            self.edges_retry_at = tick + RETRY_TICKS;
        }
        let mut qi = 0;
        // Depois as novas, que já saem em ordem de espiral (do centro para fora).
        while budget >= 2 {
            let idx = if qi < queue.len() {
                qi += 1;
                queue[qi - 1]
            } else if self.edges_done < self.plan.edges.len() {
                self.edges_done += 1;
                self.edges_done - 1
            } else {
                break;
            };
            let e = self.plan.edges[idx];
            let [dx, dy, dz] = [e.o[0] as i32, e.o[1] as i32, e.o[2] as i32];
            let [x, y, z] = [c[0] + dx, c[1] + dy, c[2] + dz];
            let key = (x >> 4, z >> 4);
            let ok = *loaded.entry(key).or_insert_with(|| world.block(&dim, key.0 * 16, y, key.1 * 16).is_some());
            if !ok {
                self.edges_later.push(idx);
                continue;
            }
            let hit = EdgeHit {
                pos: [x, y, z],
                kind: self.plan.mats.table[e.mat as usize].kind,
                f: ((dx * dx + dy * dy + dz * dz) as f32).sqrt() / r,
            };
            let eff = hooks.edge(&self.id, &hit).unwrap_or_else(|| self.profile.apply(&hit));
            if let Some(s) = eff.here {
                out.push(Change { x, y, z, state: s });
                budget -= 1;
                self.effects += 1;
            }
            if let Some(s) = eff.above {
                out.push(Change { x, y: y + 1, z, state: s });
                budget -= 1;
                self.effects += 1;
            }
        }
        // O que sobrou da fila de nova tentativa volta para ela.
        self.edges_later.extend_from_slice(&queue[qi..]);
    }

    fn header(&self) -> Header {
        let mut pending = self.reeval.clone();
        pending.extend(self.reeval_started.iter().map(|(k, _)| *k));
        Header {
            id: self.id.clone(),
            dim: self.dim.clone(),
            center: self.plan.c,
            radius: self.plan.r,
            budget: self.budget,
            tick0: self.tick0,
            cols_done: self.plan.cols_done,
            data_keys: self.persist.as_ref().map_or(0, |p| p.closed + 1),
            pending_chunks: pending,
            done_chunks: self.reeval_done.clone(),
            profile: self.profile.encode(),
            ring: self.persist.as_ref().map_or([None; RING], |p| p.ring),
            lite: self.lite,
            phase: if self.sweep.is_some() { "fallout" } else { "crater" }.into(),
            fring: self.sweep.as_ref().map_or(0, Sweep::ring_now),
            fpend: self.sweep.as_ref().map_or_else(Vec::new, |s| s.pending.clone()),
        }
    }

    /// Grava o diário novo e o cabeçalho. Falha (quota) desliga a
    /// persistência, mas não a explosão.
    fn save(&mut self, store: &mut dyn Store, hooks: &mut dyn Hooks, tick: u64) {
        let Some(p) = self.persist.as_ref() else { return };
        if p.failed {
            return;
        }
        if self.lite && tick >= self.lite_saved + 20 {
            // Leve: 1 bit por seção já escrita, no máximo uma gravação por segundo.
            let mut bits = vec![0u8; self.secs.len().div_ceil(8)];
            for (i, s) in self.secs.iter().enumerate() {
                if matches!(s.state, SecState::Applied | SecState::Done) {
                    bits[i >> 3] |= 1 << (i & 7);
                }
            }
            if bits != self.lite_bits {
                self.lite_saved = tick;
                if let Err(reason) = store.set(&format!("{}.w", p.prefix), &bits) {
                    self.persist.as_mut().unwrap().failed = true;
                    hooks.event(&self.id, &Event::PersistFailed { reason });
                    return;
                }
                self.lite_bits = bits;
            }
        }
        let first = p.header.is_empty();
        let fresh = self.plan.journal.as_mut().map(std::mem::take).unwrap_or_default();
        let id = self.id.clone();
        let p = self.persist.as_mut().unwrap();
        let mut result = Ok(());
        if first {
            // Registra a explosão no índice.
            let mut index = String::from_utf8_lossy(&store.get(INDEX_KEY).unwrap_or_default()).to_string();
            if !index.lines().any(|l| l == id) {
                index.push_str(&id);
                index.push('\n');
                result = store.set(INDEX_KEY, index.as_bytes());
            }
        }
        // Estrutura: blocos de até DATA_CHUNK; só o último é regravado.
        if !fresh.structure.is_empty() {
            p.cur.extend_from_slice(&fresh.structure);
            p.dirty = true;
        }
        while result.is_ok() && p.cur.len() >= DATA_CHUNK {
            let rest = p.cur.split_off(DATA_CHUNK);
            result = store.set(&format!("{}.d{}", p.prefix, p.closed), &p.cur);
            p.bytes += p.cur.len();
            p.closed += 1;
            p.cur = rest;
            p.dirty = true;
        }
        if result.is_ok() && p.closed >= MAX_DATA_KEYS {
            result = Err(format!("diário passou de {MAX_DATA_KEYS} blocos"));
        }
        if result.is_ok() && p.dirty {
            result = store.set(&format!("{}.d{}", p.prefix, p.closed), &p.cur);
            p.dirty = false;
        }
        // Energia: uma chave do anel por faixa; ao mudar de faixa, a mais
        // antiga é sobrescrita.
        for (band, rec) in fresh.energy {
            if result.is_err() {
                break;
            }
            if p.eband != Some(band) {
                if let Some(old) = p.eband.filter(|_| p.edirty) {
                    result = store.set(&format!("{}.e{}", p.prefix, old as usize % RING), &p.ebuf);
                }
                p.eband = Some(band);
                p.ring[band as usize % RING] = Some(band);
                p.ebuf.clear();
            }
            p.ebuf.extend_from_slice(&rec);
            p.edirty = true;
        }
        if result.is_ok() && p.edirty {
            let band = p.eband.unwrap_or(0);
            result = store.set(&format!("{}.e{}", p.prefix, band as usize % RING), &p.ebuf);
            p.edirty = false;
        }
        let header = self.header().encode();
        let p = self.persist.as_mut().unwrap();
        if result.is_ok() && header != p.header {
            result = store.set(&format!("{}.h", p.prefix), header.as_bytes());
            p.header = header;
        }
        if let Err(reason) = result {
            p.failed = true;
            self.plan.journal = None;
            hooks.event(&id, &Event::PersistFailed { reason });
        }
    }

    /// Apaga as chaves da explosão (terminada ou cancelada).
    pub fn forget(&mut self, store: &mut dyn Store) {
        let Some(p) = self.persist.take() else { return };
        for k in 0..=p.closed {
            store.delete(&format!("{}.d{k}", p.prefix));
        }
        for k in 0..RING {
            store.delete(&format!("{}.e{k}", p.prefix));
        }
        store.delete(&format!("{}.w", p.prefix));
        forget_header(store, &p.prefix, &self.id);
    }
}

/// Apaga o cabeçalho e tira o id do índice.
fn forget_header(store: &mut dyn Store, prefix: &str, id: &str) {
    store.delete(&format!("{prefix}.h"));
    let index = String::from_utf8_lossy(&store.get(INDEX_KEY).unwrap_or_default()).to_string();
    let rest: String = index.lines().filter(|l| *l != id).map(|l| format!("{l}\n")).collect();
    if rest.is_empty() {
        store.delete(INDEX_KEY);
    } else {
        let _ = store.set(INDEX_KEY, rest.as_bytes());
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
    prefix: Option<String>,
    header: String,
}

impl Linger {
    pub fn pending(&self) -> usize {
        self.sweep.pending.len()
    }

    fn tick(&mut self, fuel: u64, world: &mut dyn World, store: &mut dyn Store) {
        let Some(fp) = self.profile.fallout.as_ref() else { return };
        let viewers = world.viewers(&self.dim);
        if viewers.is_empty() {
            return;
        }
        let dim = self.dim.clone();
        let mut out = Vec::new();
        let changed = {
            let mut read = |x: i32, y: i32, z: i32| world.block(&dim, x, y, z);
            self.sweep.pending_step(fp, &viewers, LINGER_NEAR, fuel, self.budget, &mut read, &mut out)
        };
        while !out.is_empty() {
            let rest = out.split_off(out.len().min(BLOCKS_PER_CALL));
            let _ = world.set_blocks(&dim, std::mem::replace(&mut out, rest));
        }
        if !changed {
            return;
        }
        let Some(prefix) = self.prefix.clone() else { return };
        if self.sweep.pending.is_empty() {
            forget_header(store, &prefix, &self.id);
            return;
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
        }
        .encode();
        if h != self.header && store.set(&format!("{prefix}.h"), h.as_bytes()).is_ok() {
            self.header = h;
        }
    }
}

/// As explosões ativas de um mod. Uma por vez recebe o tick (o fuel do tick é
/// dela); as outras esperam na fila.
pub struct Explosions {
    /// Fuel estimado por tick (o teto do Horizon é 50 M, e o resto do mod
    /// gasta o dele): padrão [`FUEL_PLAN`].
    pub fuel: u64,
    jobs: Vec<Explosion>,
    /// Fallouts terminados com chunks que esperam um jogador chegar perto.
    lingering: Vec<Linger>,
}

impl Default for Explosions {
    fn default() -> Self {
        Self { fuel: FUEL_PLAN, jobs: Vec::new(), lingering: Vec::new() }
    }
}

impl Explosions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Começa uma explosão (o plano é montado agora; avança a partir do próximo tick).
    pub fn detonate(&mut self, det: Detonation, tick: u64) -> Result<Progress, String> {
        if self.jobs.iter().any(|j| j.id == det.id) {
            return Err(format!("já existe a explosão {}", det.id));
        }
        let ex = Explosion::new(det, tick)?;
        let p = ex.progress();
        self.jobs.push(ex);
        Ok(p)
    }

    /// Retoma todas as explosões listadas no storage; devolve as retomadas e os erros.
    pub fn resume(&mut self, store: &mut dyn Store, tick: u64, hooks: &mut dyn Hooks) -> Vec<Result<String, String>> {
        self.resume_matching(store, tick, hooks, |_| true)
    }

    /// Como [`resume`](Self::resume), só os ids que `keep` aceita (um mod com
    /// vários mundos retoma só as explosões do mundo em jogo).
    pub fn resume_matching(&mut self, store: &mut dyn Store, tick: u64, hooks: &mut dyn Hooks, keep: impl Fn(&str) -> bool) -> Vec<Result<String, String>> {
        let index = String::from_utf8_lossy(&store.get(INDEX_KEY).unwrap_or_default()).to_string();
        let mut out = Vec::new();
        for id in index.lines().filter(|l| !l.is_empty() && keep(l)) {
            if self.jobs.iter().any(|j| j.id == id) || self.lingering.iter().any(|l| l.id == id) {
                continue;
            }
            let prefix = persist::prefix(id);
            if let Some(h) = store.get(&format!("{prefix}.h")).and_then(|t| Header::decode(&String::from_utf8_lossy(&t)).ok())
                && h.phase == "linger"
            {
                let profile = match EffectProfile::decode(&h.profile) {
                    Ok(p) => p,
                    Err(e) => {
                        out.push(Err(format!("{id}: {e}")));
                        continue;
                    }
                };
                let scale = profile.fallout.as_ref().map_or(2.5, |f| f.scale);
                self.lingering.push(Linger {
                    id: id.to_string(),
                    dim: h.dim.clone(),
                    budget: h.budget,
                    tick0: h.tick0,
                    sweep: Sweep::resume(h.center, h.radius, scale, h.fring, h.fpend.clone()),
                    profile,
                    prefix: Some(prefix),
                    header: h.encode(),
                });
                out.push(Ok(id.to_string()));
                continue;
            }
            match Explosion::resume(store, id, tick) {
                Ok(ex) => {
                    hooks.event(id, &Event::Resumed { columns: ex.plan.cols_done });
                    out.push(Ok(id.to_string()));
                    self.jobs.push(ex);
                }
                Err(e) => out.push(Err(format!("{id}: {e}"))),
            }
        }
        out
    }

    /// Avança a primeira explosão da fila; devolve o progresso final das que terminaram.
    pub fn tick(&mut self, tick: u64, world: &mut dyn World, store: &mut dyn Store, hooks: &mut dyn Hooks) -> Vec<Progress> {
        // A primeira que tem trabalho recebe o tick; as que só esperam chunks
        // (`idle`) são tentadas quando nenhuma tem trabalho.
        if let Some(job) = self.jobs.iter_mut().find(|j| !j.idle()) {
            job.fuel = self.fuel;
            job.tick(tick, world, store, hooks);
        } else if !self.jobs.is_empty() {
            // Uma por tick, em rodízio (o fuel do tick é de uma só).
            let n = self.jobs.len();
            let job = &mut self.jobs[(tick % n as u64) as usize];
            job.fuel = self.fuel;
            job.tick(tick, world, store, hooks);
        } else if tick.is_multiple_of(LINGER_EVERY) {
            // Sem explosão ativa: o fuel do tick vai para os fallouts pendentes.
            for l in &mut self.lingering {
                l.tick(self.fuel, world, store);
            }
            self.lingering.retain(|l| l.pending() > 0);
        }
        let done: Vec<Progress> = self.jobs.iter().filter(|j| j.done()).map(Explosion::progress).collect();
        let (finished, jobs): (Vec<Explosion>, Vec<Explosion>) = std::mem::take(&mut self.jobs).into_iter().partition(Explosion::done);
        self.jobs = jobs;
        self.lingering.extend(finished.into_iter().filter_map(Explosion::into_linger));
        done
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
    }

    pub fn progress(&self, id: &str) -> Option<Progress> {
        self.jobs.iter().find(|j| j.id == id).map(Explosion::progress)
    }

    pub fn active(&self) -> impl Iterator<Item = Progress> + '_ {
        self.jobs.iter().map(Explosion::progress)
    }

    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    /// Para uma explosão (o que já foi escrito fica) e apaga o diário.
    pub fn cancel(&mut self, id: &str, store: &mut dyn Store) -> bool {
        if let Some(i) = self.lingering.iter().position(|l| l.id == id) {
            let l = self.lingering.remove(i);
            if let Some(p) = l.prefix {
                forget_header(store, &p, &l.id);
            }
            return true;
        }
        let Some(i) = self.jobs.iter().position(|j| j.id == id) else { return false };
        let mut ex = self.jobs.remove(i);
        ex.forget(store);
        true
    }
}
