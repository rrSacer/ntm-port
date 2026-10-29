//! Persistência de uma explosão em andamento no `storage` do mod.
//!
//! Sem ela, um reinício do servidor no meio da explosão perde o plano e
//! recalcular do zero daria uma cratera maior: o que já foi removido virou ar,
//! e ar não gasta energia. Por isso o plano grava um **diário por coluna**:
//! quando a varredura termina uma coluna, a energia (u16 em runs), as faixas
//! removidas e as bordas (dy + tipo de material) dela viram um registro
//! compacto. Uma seção só é escrita no mundo depois que as colunas dela estão
//! no diário gravado (o motor grava o diário antes das escritas do tick), então
//! a retomada nunca precisa recalcular por cima de blocos já removidos.
//!
//! Chaves (`<p>` = `nx.<id>`):
//! - `<p>.s0`/`<p>.s1`: o **estado**, gravado alternando entre as duas com
//!   geração e checksum (como o save do M1, `ntm-core/src/persist.rs`): o
//!   cabeçalho em texto e a cauda que muda a cada tick (o bloco do diário em
//!   curso, a energia da faixa em curso, os bits da persistência leve);
//! - `<p>.d0`, `<p>.d1`…: blocos fechados do diário, de [`DATA_CHUNK`]
//!   bytes, gravados uma vez só (o estado guarda o tamanho e o checksum);
//! - `<p>.e0..e2`: faixas de energia fechadas (idem);
//! - `nx.index0`/`nx.index1`: a lista de ids ativos, também em dois slots.
//!
//! O `storage::set` do Horizon reescreve o arquivo sem rename atômico: uma
//! queda no meio deixa metade. Por isso nada que o estado em vigor usa é
//! reescrito: a gravação nova vai para o outro slot, e a leitura fica com a
//! geração mais nova cujo checksum confere (e cujos blocos conferem).
//! Limites do Horizon: 4 MiB e 64 chaves por mod, 1 MiB gravado por chamada
//! de hook (horizon-mods runtime.rs:124-134).
//!
//! Na retomada todas as seções voltam a pendentes: reescrever ar sobre ar é
//! barato (~0,6 µs no host) e cobre também um mundo que voltou ao último
//! autosave depois de uma queda.
use crate::crater::{Kind, Plan};

/// Tamanho de cada bloco do diário no storage.
pub const DATA_CHUNK: usize = 96 * 1024;
/// Chaves de dados permitidas por explosão (o mod tem 64 no total).
pub const MAX_DATA_KEYS: usize = 40;
/// Base das duas chaves do índice (`nx.index0`, `nx.index1`).
pub const INDEX_KEY: &str = "nx.index";
/// Até este raio o diário é completo (R=150: ~930 KB de pico). Acima, a
/// persistência é leve: o diário de R=500 passaria de 10 MB, e a pasta do mod
/// tem 4 MB para tudo.
pub const JOURNAL_MAX_RADIUS: i32 = 150;

/// O storage do mod (capability `storage`): o mod implementa com
/// `horizon_mod_sdk::storage`.
pub trait Store {
    fn get(&mut self, key: &str) -> Option<Vec<u8>>;
    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), String>;
    fn delete(&mut self, key: &str);
}

pub(crate) fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

pub(crate) struct Reader<'a> {
    pub b: &'a [u8],
    pub p: usize,
}

impl Reader<'_> {
    pub fn varint(&mut self) -> Result<u64, String> {
        let mut v = 0u64;
        let mut shift = 0;
        loop {
            let b = *self.b.get(self.p).ok_or("diário truncado")?;
            self.p += 1;
            v |= ((b & 0x7f) as u64) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
            shift += 7;
            if shift > 63 {
                return Err("varint inválido".into());
            }
        }
    }
    fn u16(&mut self) -> Result<u16, String> {
        let s = self.b.get(self.p..self.p + 2).ok_or("diário truncado")?;
        self.p += 2;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }
    fn u8(&mut self) -> Result<u8, String> {
        let b = *self.b.get(self.p).ok_or("diário truncado")?;
        self.p += 1;
        Ok(b)
    }
}

fn zigzag(v: i32) -> u64 {
    ((v << 1) ^ (v >> 31)) as u32 as u64
}

fn unzigzag(v: u64) -> i32 {
    let v = v as u32;
    ((v >> 1) as i32) ^ -((v & 1) as i32)
}

pub(crate) fn kind_from(b: u8) -> Kind {
    use Kind::*;
    [Air, Liquid, Rock, Soil, Sand, Wood, Planks, Leaves, Plant, Other, Wall]
        .get(b as usize)
        .copied()
        .unwrap_or(Other)
}

/// Largura (em blocos de raio horizontal) de uma faixa do anel de energia.
pub const BAND: f32 = 2.0;
/// Faixas guardadas no anel: a atual e as duas anteriores. Os pais e os cantos
/// da interpolação de um voxel ficam a ≤ 1,5 bloco na horizontal, em colunas
/// mais perto do centro: só a frente da espiral precisa de energia exata.
pub const RING: usize = 3;

/// O que a varredura produziu desde a última gravação.
#[derive(Default)]
pub struct Journal {
    /// Registros de estrutura (só anexados).
    pub structure: Vec<u8>,
    /// Registros de energia por faixa (só colunas da passada principal).
    pub energy: Vec<(u32, Vec<u8>)>,
}

pub(crate) fn band_of(h: f32) -> u32 {
    (h / BAND) as u32
}

/// Anexa os registros da coluna `ci` (bordas expostas a partir de `edges_from`).
pub(crate) fn encode_column(plan: &Plan, ci: usize, edges_from: usize, dead: usize, main: bool, j: &mut Journal) {
    let energy = plan.col_energy(ci).unwrap_or(&[]);
    let len = energy.len();
    // O fundo podado (`dead` voxels) tem energia 0 e nada removido: não é varrido.
    let dead = dead.min(len);
    let out = &mut j.structure;
    put_varint(out, ci as u64);
    put_varint(out, zigzag(plan.alive_lo_of(ci)));
    // Remoções em faixas (pulo, comprimento).
    let runs = plan.col_runs(ci);
    put_varint(out, runs.len() as u64);
    let mut last_end = 0usize;
    for &(start, n) in runs {
        put_varint(out, (start as usize - last_end) as u64);
        put_varint(out, n as u64);
        last_end = start as usize + n as usize;
    }
    // O plano só guarda bordas expostas (as outras nunca recebem efeito).
    let edges = &plan.edges[edges_from..];
    put_varint(out, edges.len() as u64);
    for e in edges {
        put_varint(out, zigzag(e.o[1] as i32));
        out.push(plan.mats.table[e.mat as usize].kind as u8);
    }
    if main {
        let band = band_of(plan.column_h(ci));
        let mut rec = Vec::new();
        put_varint(&mut rec, ci as u64);
        let mut runs: Vec<(u32, u16)> = if dead > 0 { vec![(dead as u32, 0)] } else { Vec::new() };
        for &v in &energy[dead..] {
            match runs.last_mut() {
                Some(r) if r.1 == v => r.0 += 1,
                _ => runs.push((1, v)),
            }
        }
        put_varint(&mut rec, runs.len() as u64);
        for (n, v) in runs {
            put_varint(&mut rec, n as u64);
            rec.extend_from_slice(&v.to_le_bytes());
        }
        match j.energy.last_mut() {
            Some((b, buf)) if *b == band => buf.extend_from_slice(&rec),
            _ => j.energy.push((band, rec)),
        }
    }
}

/// Lê registros de estrutura a partir de `*pos` e restaura os que `keep`
/// aceita, até gastar `budget` de fuel estimado; true quando acabou.
pub(crate) fn restore_columns_step(plan: &mut Plan, data: &[u8], pos: &mut usize, budget: u64, mut keep: impl FnMut(&Plan, usize) -> bool) -> Result<bool, String> {
    let mut r = Reader { b: data, p: *pos };
    let mut spent = 0u64;
    let mut removed = Vec::new();
    let mut edges = Vec::new();
    while r.p < data.len() {
        if spent >= budget {
            *pos = r.p;
            return Ok(false);
        }
        let ci = r.varint()? as usize;
        let alive = unzigzag(r.varint()?);
        removed.clear();
        for _ in 0..r.varint()? {
            let skip = r.varint()? as u32;
            removed.push((skip, r.varint()? as u32));
        }
        edges.clear();
        for _ in 0..r.varint()? {
            let dy = unzigzag(r.varint()?);
            edges.push((dy, kind_from(r.u8()?)));
        }
        spent += COST_RECORD + COST_RUN * removed.len() as u64 + COST_RUN * edges.len() as u64;
        if keep(plan, ci) {
            plan.restore_column(ci, alive, &removed, &edges)?;
        }
    }
    *pos = r.p;
    Ok(true)
}

/// Fuel estimado por registro do diário restaurado e por faixa/borda dele.
const COST_RECORD: u64 = 4_000;
const COST_RUN: u64 = 600;

/// Lê registros de energia (anel) e restaura os das colunas que `keep` aceita.
pub(crate) fn restore_energy(plan: &mut Plan, data: &[u8], mut keep: impl FnMut(&Plan, usize) -> bool) -> Result<usize, String> {
    let mut r = Reader { b: data, p: 0 };
    let mut n = 0;
    while r.p < data.len() {
        let ci = r.varint()? as usize;
        let mut runs = Vec::new();
        for _ in 0..r.varint()? {
            let len = r.varint()? as u32;
            runs.push((len, r.u16()?));
        }
        if keep(plan, ci) {
            plan.restore_energy(ci, &runs)?;
            n += 1;
        }
    }
    Ok(n)
}

/// Um bloco gravado uma vez só: tamanho e checksum.
pub type Sealed = (u32, u64);

/// Cabeçalho `chave=valor` por linha (texto: fácil de inspecionar no disco).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Header {
    pub id: String,
    pub dim: String,
    pub center: [i32; 3],
    pub radius: i32,
    pub budget: usize,
    pub tick0: u64,
    /// Colunas da espiral cujos registros estão no diário gravado.
    pub cols_done: u32,
    /// Blocos fechados do diário (`<p>.d<k>`).
    pub blocks: Vec<Sealed>,
    /// Chunks que estavam sem carregar e ainda não foram recalculados.
    pub pending_chunks: Vec<(i32, i32)>,
    /// Chunks que estavam sem carregar e já foram recalculados (registros no diário).
    pub done_chunks: Vec<(i32, i32)>,
    pub profile: String,
    /// Faixa fechada guardada em cada chave do anel `<p>.e0..e2` (ou nenhuma).
    pub ering: [Option<(u32, Sealed)>; RING],
    /// Faixa da energia que está na cauda do estado.
    pub cband: Option<u32>,
    /// Persistência leve (R > [`JOURNAL_MAX_RADIUS`], ou o diário não coube):
    /// só as seções escritas (bits na cauda); a retomada recalcula o plano.
    pub lite: bool,
    /// Leve, já no fallout, com seções da cratera ainda por escrever: a
    /// retomada refaz o plano em vez de dar a cratera por pronta.
    pub crater_left: bool,
    /// `crater`, `fallout` (passada principal) ou `linger` (só pendentes).
    pub phase: String,
    /// Anel do fallout em curso e chunks do fallout que esperam carregar.
    pub fring: i32,
    pub fpend: Vec<(i32, i32)>,
}

fn chunks_str(v: &[(i32, i32)]) -> String {
    v.iter().map(|(a, b)| format!("{a},{b}")).collect::<Vec<_>>().join(";")
}

fn chunks_parse(s: &str) -> Vec<(i32, i32)> {
    s.split(';')
        .filter_map(|p| {
            let (a, b) = p.split_once(',')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .collect()
}

fn sealed_str((len, sum): Sealed) -> String {
    format!("{len}:{sum:x}")
}

fn sealed_parse(s: &str) -> Option<Sealed> {
    let (a, b) = s.split_once(':')?;
    Some((a.parse().ok()?, u64::from_str_radix(b, 16).ok()?))
}

impl Header {
    pub fn encode(&self) -> String {
        let ering: Vec<String> = self.ering.iter().map(|e| e.map_or("-".into(), |(b, s)| format!("{b}@{}", sealed_str(s)))).collect();
        format!(
            "v=2\nid={}\ndim={}\ncenter={},{},{}\nradius={}\nbudget={}\ntick0={}\ncols_done={}\nblocks={}\npending_chunks={}\ndone_chunks={}\nering={}\ncband={}\nlite={}\ncrater_left={}\nphase={}\nfring={}\nfpend={}\nprofile={}\n",
            self.id,
            self.dim,
            self.center[0],
            self.center[1],
            self.center[2],
            self.radius,
            self.budget,
            self.tick0,
            self.cols_done,
            self.blocks.iter().map(|b| sealed_str(*b)).collect::<Vec<_>>().join(","),
            chunks_str(&self.pending_chunks),
            chunks_str(&self.done_chunks),
            ering.join(","),
            self.cband.map_or("-".into(), |b| b.to_string()),
            u8::from(self.lite),
            u8::from(self.crater_left),
            if self.phase.is_empty() { "crater" } else { &self.phase },
            self.fring,
            chunks_str(&self.fpend),
            self.profile.replace('\n', "\\n"),
        )
    }

    pub fn decode(text: &str) -> Result<Self, String> {
        let mut h = Header::default();
        let mut version = 0;
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let num = |v: &str| v.parse::<i64>().map_err(|e| format!("{k}: {e}"));
            match k {
                "v" => version = num(v)?,
                "id" => h.id = v.into(),
                "dim" => h.dim = v.into(),
                "center" => {
                    let p: Vec<i32> = v.split(',').filter_map(|x| x.parse().ok()).collect();
                    h.center = [*p.first().ok_or("center")?, *p.get(1).ok_or("center")?, *p.get(2).ok_or("center")?];
                }
                "radius" => h.radius = num(v)? as i32,
                "budget" => h.budget = num(v)? as usize,
                "tick0" => h.tick0 = num(v)? as u64,
                "cols_done" => h.cols_done = num(v)? as u32,
                "blocks" => h.blocks = v.split(',').filter(|b| !b.is_empty()).map(|b| sealed_parse(b).ok_or(format!("blocks: {b}"))).collect::<Result<_, _>>()?,
                "pending_chunks" => h.pending_chunks = chunks_parse(v),
                "done_chunks" => h.done_chunks = chunks_parse(v),
                "profile" => h.profile = v.replace("\\n", "\n"),
                "lite" => h.lite = v == "1",
                "crater_left" => h.crater_left = v == "1",
                "phase" => h.phase = v.into(),
                "fring" => h.fring = num(v)? as i32,
                "fpend" => h.fpend = chunks_parse(v),
                "cband" => h.cband = v.parse().ok(),
                "ering" => {
                    for (i, e) in v.split(',').take(RING).enumerate() {
                        h.ering[i] = e.split_once('@').and_then(|(b, s)| Some((b.parse().ok()?, sealed_parse(s)?)));
                    }
                }
                _ => {}
            }
        }
        if version != 2 {
            return Err(format!("versão de cabeçalho {version}"));
        }
        Ok(h)
    }
}

/// O estado gravado: cabeçalho e cauda.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub header: Header,
    /// O bloco do diário em curso (depois dos fechados).
    pub data: Vec<u8>,
    /// Energia da faixa `header.cband`.
    pub energy: Vec<u8>,
    /// Persistência leve: 1 bit por seção escrita.
    pub bits: Vec<u8>,
}

impl State {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = self.header.encode().into_bytes();
        out.push(0);
        for part in [&self.data, &self.energy, &self.bits] {
            put_varint(&mut out, part.len() as u64);
            out.extend_from_slice(part);
        }
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let nul = b.iter().position(|&c| c == 0).ok_or("estado sem cauda")?;
        let header = Header::decode(std::str::from_utf8(&b[..nul]).map_err(|e| e.to_string())?)?;
        let mut r = Reader { b, p: nul + 1 };
        let mut part = || -> Result<Vec<u8>, String> {
            let n = r.varint()? as usize;
            let s = r.b.get(r.p..r.p + n).ok_or("cauda truncada")?.to_vec();
            r.p += n;
            Ok(s)
        };
        Ok(Self { header, data: part()?, energy: part()?, bits: part()? })
    }
}

/// FNV-1a de 64 bits (o checksum dos slots e dos blocos).
pub fn fnv64(b: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &c in b {
        h = (h ^ u64::from(c)).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Fuel estimado por byte de checksum/cópia na gravação (laço simples em wasm).
pub const COST_BYTE: u64 = 6;

fn slot_key(base: &str, gen: u64) -> String {
    format!("{base}{}", gen % 2)
}

/// O valor de um slot: `NX2 <geração> <fnv> <tamanho>\n` e os bytes.
pub fn slot_bytes(gen: u64, payload: &[u8]) -> Vec<u8> {
    let mut out = format!("NX2 {gen} {:016x} {}\n", fnv64(payload), payload.len()).into_bytes();
    out.extend_from_slice(payload);
    out
}

fn slot_decode(b: &[u8]) -> Option<(u64, &[u8])> {
    let nl = b.iter().position(|&c| c == b'\n')?;
    let mut f = std::str::from_utf8(&b[..nl]).ok()?.split(' ');
    if f.next()? != "NX2" {
        return None;
    }
    let gen: u64 = f.next()?.parse().ok()?;
    let sum = u64::from_str_radix(f.next()?, 16).ok()?;
    let len: usize = f.next()?.parse().ok()?;
    let payload = &b[nl + 1..];
    (payload.len() == len && fnv64(payload) == sum).then_some((gen, payload))
}

/// Chave em que a geração `gen` vai (a outra fica com a anterior).
pub fn slot_for(base: &str, gen: u64) -> String {
    slot_key(base, gen)
}

/// Os slots inteiros de `base`, do mais novo para o mais velho.
pub fn read_slots(store: &mut dyn Store, base: &str) -> Vec<(u64, Vec<u8>)> {
    let mut out: Vec<(u64, Vec<u8>)> = (0..2)
        .filter_map(|i| store.get(&slot_key(base, i)))
        .filter_map(|b| slot_decode(&b).map(|(g, p)| (g, p.to_vec())))
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out
}

pub fn delete_slots(store: &mut dyn Store, base: &str) {
    for i in 0..2 {
        store.delete(&slot_key(base, i));
    }
}

/// O índice (ids ativos) e a geração dele.
pub fn read_index(store: &mut dyn Store) -> (u64, Vec<String>) {
    match read_slots(store, INDEX_KEY).into_iter().next() {
        Some((g, b)) => (g, String::from_utf8_lossy(&b).lines().filter(|l| !l.is_empty()).map(str::to_string).collect()),
        None => (0, Vec::new()),
    }
}

/// Grava o índice com `ids` (no slot da próxima geração); vazio apaga.
pub fn write_index(store: &mut dyn Store, gen: u64, ids: &[String]) -> Result<usize, String> {
    if ids.is_empty() {
        delete_slots(store, INDEX_KEY);
        return Ok(0);
    }
    let text: String = ids.iter().map(|i| format!("{i}\n")).collect();
    let b = slot_bytes(gen + 1, text.as_bytes());
    store.set(&slot_for(INDEX_KEY, gen + 1), &b)?;
    Ok(b.len())
}

pub fn prefix(id: &str) -> String {
    format!("nx.{id}")
}
