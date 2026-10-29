//! The server mod's engine (docs/fundacao.md): world identity, persistence,
//! machines, energy, screens, item transactions and probes, over a
//! [`ServerHost`] the mod implements with the Horizon API.

use crate::{
    Pos,
    blockstate::{self, BlockState},
    energy::{self, EnergyNode, Topology},
    hash::{Entropy, fnv64},
    kv::{Kv, keys},
    machine::{Effect, Kind, Machine, actions},
    persist::{self, Saver, WorldInfo, WorldState, Worlds},
    probes::{self, Probe},
    proto::{C2S, Held, Mailbox, S2C},
    registry,
    token::{self, Cell, Restore},
    tx::{self, Applied, Outcome, Remembered, Tx},
    view::ViewModel,
    worldid::{self, Wid},
};
use std::collections::{BTreeMap, BTreeSet};

pub use crate::probes::Purpose;

/// What the engine needs from the game.
pub trait ServerHost: Kv {
    fn get_block(&mut self, dim: &str, p: Pos) -> Option<String>;
    fn set_blocks(&mut self, dim: &str, blocks: &[(Pos, String)]) -> Result<(), String>;
    fn tell(&mut self, player: &str, text: &str);
    fn log(&mut self, text: &str);
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerInfo {
    pub id: String,
    pub name: String,
    pub dim: String,
    pub pos: [f64; 3],
    pub creative: bool,
}

/// Ticks between saves of a world whose only change is energy and
/// progress; a change that moves items or blocks saves at once.
pub const SAVE_EVERY: u64 = 200;
/// Ticks a world-id or radiation probe waits before it is reported failed.
pub const PROBE_TICKS: u64 = probes::ACTIVE_TICKS;
/// Ticks before a transaction still unanswered is published again; the
/// wait doubles each time, up to [`TX_RESEND_MAX`]. The lines are safe to
/// repeat; the waits only keep a lagging client's queue short.
pub const TX_RESEND: u64 = 60;
pub const TX_RESEND_MAX: u64 = 2400;
/// Answer cells a transaction keeps (the newest replace the oldest).
pub const TX_CELLS: usize = 4;
/// Ticks of play a transaction that went through is kept (and checked at
/// each world entry): longer than vanilla's autosave, so a crash that rolls
/// the world back past it is noticed and undone.
pub const RETAIN_TICKS: u64 = 18_000;
/// Ticks a radiation sample that timed out holds back the next one; its
/// column stays watched for [`probes::LATE_TICKS`] (lines may still be
/// queued at a stalled client) and the next sample takes another column.
pub const RADIATION_LATE_TICKS: u64 = 600;
/// Ticks after the client acked a batch of resets before it leaves the
/// save: by then the consumer (64 lines a tick) has run it.
pub const RESET_SETTLE: u64 = 200;
/// Ticks a cell of a finished transaction is still watched for a late
/// token to put back.
pub const RETIRED_TICKS: u64 = 12_000;
/// Ticks without a client before the world is taken for a dedicated
/// server's only one.
pub const NO_CLIENT_TICKS: u64 = 600;
/// Ticks between checks of whether a machine's chunk is loaded.
pub const LOAD_CHECK: u64 = 20;
/// Ticks between screen refreshes.
pub const VIEW_EVERY: u64 = 5;
/// How far (blocks) the player may be from a machine they use.
pub const REACH: f64 = 8.0;
/// The world of a server no client mod shares a folder with.
pub const DEDICATED: &str = "server";

#[derive(Clone, Debug)]
enum BlockEvent {
    Placed(String, Pos, String),
    Broken(String, Pos, String),
}

type Key = (String, Pos);

#[derive(Debug)]
struct Ready {
    wid: Wid,
    world: WorldState,
    generation: u64,
    /// Energy or progress changed: saved within [`SAVE_EVERY`].
    dirty: bool,
    /// Items, blocks or transactions changed: saved this tick, before
    /// anything that depends on it is published.
    urgent: bool,
    next_save: u64,
    saver: Saver,
    topo: BTreeMap<String, Topology>,
    topo_dirty: bool,
    /// Chunk loaded, as last checked.
    loaded: BTreeMap<Key, bool>,
    /// The state last written or seen, per machine.
    shown: BTreeMap<Key, String>,
    /// Machines and cables whose block was missing at the last check.
    suspect: BTreeMap<Key, u64>,
    /// Machines whose block this session has seen in the world: only those
    /// drop their contents when they vanish.
    seen: BTreeSet<Key>,
    viewer: Option<(Key, Option<ViewModel>)>,
    /// Transactions whose new answer cell waits for the save before their
    /// lines leave (`tx.id`).
    to_publish: BTreeSet<u64>,
    /// The batch of `world.resets` sent: (client, seq, lines, tick acked).
    /// Received isn't run (the consumer's queue may be lost with the game):
    /// the batch leaves the save [`RESET_SETTLE`] ticks after the ack.
    resets_sent: Option<(u64, u64, usize, Option<u64>)>,
    /// Machines put back to a snapshot this session (the forgotten
    /// transaction's id): only an older one may replace it.
    restored: BTreeMap<Key, u64>,
    /// Where the next answer cell is looked for.
    cursor: usize,
    /// Applied transactions to check against the world this session, and
    /// those whose check waits for the save before it leaves.
    verify: BTreeSet<u64>,
    verify_publish: BTreeSet<u64>,
}

#[derive(Debug)]
enum Session {
    /// No player: nothing to do.
    Idle,
    /// Players, no world id yet.
    Identifying,
    Ready(Box<Ready>),
}

pub struct Server {
    radiation: crate::radiation::Runtime,
    /// Bombs and explosions (`server/bomb_server.rs`, docs/bombas.md).
    blasts: crate::blast::Runtime,
    /// Fluid networks (`server/fluid_server.rs`, docs/fluidos.md).
    fluidnet: fluid_server::FluidNet,
    mail: Mailbox<S2C, C2S>,
    worlds: Worlds,
    entropy: Entropy,
    tick: u64,
    last_tick: Option<u64>,
    players: Vec<PlayerInfo>,
    players_since: u64,
    session: Session,
    queued: Vec<BlockEvent>,
    /// Probes in flight (world id, radiation), of any world.
    probes: Vec<Probe>,
    probe_store: probes::Store,
    /// The last store of the probes succeeded: their lines may leave.
    probes_durable: bool,
    next_id: u32,
    warned_no_client: bool,
    warned_full: bool,
    warned_save: bool,
    id_attempts: u32,
    next_id_try: u64,
    /// Where the client's player is (`Hello`, `Open`).
    client_at: Option<(String, [f64; 3])>,
    /// Errors from `new`, which has no log: logged at the first tick.
    notes: Vec<String>,
    /// The machine an action or a break starts from: the snapshot the
    /// transactions it makes keep.
    snapshot: Option<(Key, Machine)>,
}

impl Server {
    pub fn new(kv: &mut impl Kv, tick: u64) -> Self {
        let mut notes = Vec::new();
        crate::radiation::warm();
        let mut worlds = Worlds::load(kv);
        worlds.boot += 1;
        if let Err(e) = worlds.store(kv) {
            notes.push(format!("worlds: {e}"));
        }
        let mut entropy = Entropy::default();
        entropy.feed(worlds.boot);
        entropy.feed(tick);
        for key in [keys::C2S, keys::S2C, keys::JOURNAL, probes::KEYS[0], probes::KEYS[1]] {
            if let Some(b) = kv.get(key) {
                entropy.feed(fnv64(&b));
            }
        }
        let sid = entropy.draw() | 1;
        let inst = entropy.draw();
        let mut mail = Mailbox::with_instance(keys::S2C, keys::C2S, sid, inst);
        let _ = mail.flush(kv);
        // Probes a previous run left: only their cells are put back now.
        let (mut left, probe_store) = probes::Store::load(kv);
        for p in &mut left {
            p.late = true;
            p.late_until = tick + probes::LATE_TICKS;
            p.sent_to = None;
            // Never sent again: the lines only weigh on each store.
            p.lines = Vec::new();
        }
        if !left.is_empty() {
            notes.push(format!("{} probes from the last run", left.len()));
        }
        let next_id = left.iter().map(|p| p.id).max().unwrap_or(0).wrapping_add(1).max(1);
        Self {
            radiation: crate::radiation::Runtime::default(),
            blasts: crate::blast::Runtime::default(),
            fluidnet: fluid_server::FluidNet::default(),
            mail,
            worlds,
            entropy,
            tick,
            last_tick: None,
            players: Vec::new(),
            players_since: tick,
            session: Session::Idle,
            queued: Vec::new(),
            probes: left,
            probe_store,
            probes_durable: true,
            next_id,
            warned_no_client: false,
            warned_full: false,
            warned_save: false,
            id_attempts: 0,
            next_id_try: 0,
            client_at: None,
            notes,
            snapshot: None,
        }
    }

    pub fn wid(&self) -> Option<&str> {
        match &self.session {
            Session::Ready(r) => Some(&r.wid),
            _ => None,
        }
    }

    fn ready(&mut self) -> Option<&mut Ready> {
        match &mut self.session {
            Session::Ready(r) => Some(r),
            _ => None,
        }
    }

    pub fn block_placed(&mut self, dim: &str, pos: Pos, state: &str) {
        self.queued
            .push(BlockEvent::Placed(dim.into(), pos, state.into()));
    }
    pub fn block_broken(&mut self, dim: &str, pos: Pos, state: &str) {
        self.queued
            .push(BlockEvent::Broken(dim.into(), pos, state.into()));
    }

    /// The probes in flight (tests, status).
    pub fn probes(&self) -> &[Probe] {
        &self.probes
    }

    /// The item transactions of the world in play (tests, status).
    pub fn txs(&self) -> &[Tx] {
        match &self.session {
            Session::Ready(r) => &r.world.txs,
            _ => &[],
        }
    }

    /// Once per server tick, with the players online now.
    pub fn tick(&mut self, host: &mut impl ServerHost, tick: u64, players: Vec<PlayerInfo>) {
        for n in std::mem::take(&mut self.notes) {
            host.log(&n);
        }
        self.tick = tick;
        let jumped = self
            .last_tick
            .is_some_and(|t| tick < t || tick > t + 100);
        self.last_tick = Some(tick);
        for p in &players {
            self.entropy.feed(p.pos[0].to_bits() ^ p.pos[2].to_bits().rotate_left(17));
            self.entropy.feed_bytes(p.id.as_bytes());
        }
        if self.players.is_empty() && !players.is_empty() {
            self.players_since = tick;
        }
        self.players = players;

        let polled = self.mail.poll(host);
        if polled.new_peer || jumped {
            // A client session started: a world was entered (or the client
            // mod restarted). Whatever world this was, it is saved and the
            // new one identified.
            self.session_ended();
            self.leave_world(host);
            if !self.players.is_empty() {
                self.session = Session::Identifying;
                self.id_attempts = 0;
                self.next_id_try = tick;
            }
        }
        if self.players.is_empty() {
            self.leave_world(host);
        } else if matches!(self.session, Session::Idle) {
            self.session = Session::Identifying;
        }
        for m in polled.msgs {
            self.message(host, m);
        }

        if matches!(self.session, Session::Identifying) {
            self.identify(host);
        }
        self.run_probes(host);
        if self.ready().is_some() {
            for e in std::mem::take(&mut self.queued) {
                self.block_event(host, e);
            }
            self.radiation_tick(host);
            self.simulate(host);
            self.fluids_tick();
            self.bombs_tick(host);
            self.run_txs(host);
        } else if self.queued.len() > 4096 {
            self.queued.drain(..2048);
        }
        // What leaves depends on what is durable: the world's save (with
        // its transactions) and the probes are written first, and only what
        // they made durable is published.
        let saved = self.save_urgent(host);
        let probes_stored = self.store_probes(host);
        self.probes_durable = probes_stored;
        if probes_stored {
            self.publish_probes();
        }
        if saved {
            self.publish_txs(host);
        }
        if let Err(e) = self.mail.flush(host) {
            host.log(&format!("s2c: {e}"));
        }
    }

    /// The client's session ended: its probes' lines may or may not run.
    /// The radiation sample's too (its seq meant the old session's ack).
    fn session_ended(&mut self) {
        let tick = self.tick;
        for p in &mut self.probes {
            if p.sent() && !p.late {
                p.late = true;
                p.late_until = tick + probes::LATE_TICKS;
                p.sent_to = None;
                p.lines = Vec::new();
            }
        }
    }

    /// Saves and forgets the world in play, if any.
    fn leave_world(&mut self, host: &mut impl ServerHost) {
        if let Session::Ready(r) = &mut self.session {
            let wid = r.wid.clone();
            if let Err(e) = save(host, r, true) {
                host.log(&format!("save of world {wid} failed: {e}"));
            }
            host.log(&format!("left world {wid}"));
        }
        self.radiation = crate::radiation::Runtime::default();
        self.session = Session::Idle;
    }

    /// The online player who is the client's: the one nearest where the
    /// client said its player is, in its dimension.
    fn client_player(&self) -> Option<&PlayerInfo> {
        let Some((dim, at)) = &self.client_at else {
            return self.players.first();
        };
        let d2 = |p: &PlayerInfo| (0..3).map(|i| (p.pos[i] - at[i]).powi(2)).sum::<f64>();
        self.players
            .iter()
            .filter(|p| &p.dim == dim)
            .min_by(|a, b| d2(a).total_cmp(&d2(b)))
            .or(self.players.first())
    }

    /// Every cell some probe or transaction (of any world) holds, or a
    /// finished one is still watched at.
    fn busy_cells(&self) -> BTreeSet<Key> {
        let mut out: BTreeSet<Key> = self
            .probes
            .iter()
            .flat_map(|q| q.positions().map(|c| (q.dim.clone(), c)).collect::<Vec<_>>())
            .collect();
        if let Session::Ready(r) = &self.session {
            for t in r.world.txs.iter().chain(r.world.applied.iter().map(|a| &a.tx)) {
                out.extend(t.cells.iter().map(|c| (c.dim.clone(), c.pos)));
            }
            out.extend(r.world.retired.iter().map(|(c, _)| (c.dim.clone(), c.pos)));
        }
        out
    }

    fn identify(&mut self, host: &mut impl ServerHost) {
        if self
            .probes
            .iter()
            .any(|p| p.purpose == Purpose::WorldId && !p.late && p.wid.is_none())
        {
            return;
        }
        if self.mail.peer.is_none() {
            if self.tick.saturating_sub(self.players_since) < NO_CLIENT_TICKS {
                return;
            }
            if host.get(keys::C2S).is_some() {
                if !self.warned_no_client {
                    self.warned_no_client = true;
                    host.log("the client mod is silent: machines paused, nothing saved");
                    self.tell_all(
                        host,
                        "NTM: o mod cliente ntmlogic não respondeu. As máquinas do NTM \
                         ficam paradas e nada é salvo até ele voltar (Opções > Mods).",
                    );
                }
                return;
            }
            host.log("no client mod shares this server's storage: one world");
            self.enter_world(host, DEDICATED.into());
            return;
        }
        if self.tick < self.next_id_try {
            return;
        }
        let Some(p) = self.client_player().cloned() else {
            return;
        };
        let busy = self.busy_cells();
        let (bx, bz) = (p.pos[0].floor() as i32, p.pos[2].floor() as i32);
        let mut cells = None;
        // 64 columns: late radiation samples may hold some for a while.
        'search: for dx in 0..8 {
            for dz in 0..8 {
                let col = worldid::column(&p.dim, bx + dx, bz + dz);
                if col.iter().any(|c| busy.contains(&(p.dim.clone(), *c))) {
                    continue;
                }
                let states: Option<Vec<String>> =
                    col.iter().map(|c| host.get_block(&p.dim, *c)).collect();
                if let Some(states) = states
                    && states.iter().all(|s| worldid::safe_to_borrow(s))
                {
                    cells = Some(col.into_iter().zip(states).collect::<Vec<_>>());
                    break 'search;
                }
            }
        }
        let Some(cells) = cells else {
            self.next_id_try = self.tick + 40;
            return;
        };
        let proposal = worldid::fresh(self.entropy.draw());
        let lines = token::in_dim(&p.dim, worldid::lines(&proposal, &cells));
        let id = self.take_id();
        let cells = cells
            .into_iter()
            .map(|(pos, s)| Cell {
                pos,
                restore: Restore::State(s),
                seen: None,
            })
            .collect();
        let mut probe = Probe::new(id, None, &p.dim, cells, Purpose::WorldId);
        probe.lines = lines;
        self.probes.push(probe);
    }

    fn take_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        id
    }

    fn enter_world(&mut self, host: &mut impl ServerHost, wid: Wid) {
        let (generation, mut world) = match persist::load(host, &wid) {
            Some((g, d)) => (g, WorldState::from_save(d)),
            None => (0, WorldState::default()),
        };
        if world.salt.is_empty() {
            world.salt = format!("{:06x}", self.entropy.draw() & 0xff_ffff);
        }
        host.log(&format!(
            "world {wid}: {} machines, {} cables, {} transactions (save generation {generation})",
            world.machines.len(),
            world.cables.len(),
            world.txs.len()
        ));
        let bytes = self.worlds.worlds.get(&wid).map_or(0, |w| w.bytes);
        self.worlds.worlds.insert(
            wid.clone(),
            WorldInfo {
                seen: self.worlds.boot,
                machines: world.machines.len(),
                bytes,
            },
        );
        if let Err(e) = self.worlds.store(host) {
            host.log(&format!("worlds: {e}"));
        }
        self.recover_columns(host, &wid);
        self.mail.send(S2C::World { wid: wid.clone() });
        self.radiation.next_sample = self.tick + crate::radiation::SAMPLE_TICKS;
        let pending = !world.txs.is_empty();
        let verify: BTreeSet<u64> = world.applied.iter().map(|a| a.tx.id).collect();
        self.session = Session::Ready(Box::new(Ready {
            wid,
            world,
            generation,
            dirty: false,
            urgent: pending,
            next_save: self.tick + SAVE_EVERY,
            saver: Saver::default(),
            topo: BTreeMap::new(),
            topo_dirty: true,
            loaded: BTreeMap::new(),
            shown: BTreeMap::new(),
            suspect: BTreeMap::new(),
            seen: BTreeSet::new(),
            viewer: None,
            to_publish: BTreeSet::new(),
            resets_sent: None,
            restored: BTreeMap::new(),
            cursor: 0,
            verify,
            verify_publish: BTreeSet::new(),
        }));
    }

    /// World-id columns an earlier session or run left in the world: put
    /// back only when its tokens spell this world's id.
    fn recover_columns(&mut self, host: &mut impl ServerHost, wid: &str) {
        let mut done = Vec::new();
        for (i, p) in self.probes.iter().enumerate() {
            if p.purpose != Purpose::WorldId || p.sent_to.is_some() {
                continue;
            }
            let levels: Vec<Option<u8>> = p
                .cells
                .iter()
                .map(|c| host.get_block(&p.dim, c.pos).and_then(|s| token::token_level(&s)))
                .collect();
            if worldid::decode(&levels).as_deref() == Some(wid) {
                let writes: Vec<(Pos, String)> = p
                    .cells
                    .iter()
                    .filter_map(|c| match &c.restore {
                        Restore::State(s) => Some((c.pos, s.clone())),
                        Restore::Machine => None,
                    })
                    .collect();
                if let Err(e) = host.set_blocks(&p.dim, &writes) {
                    host.log(&format!("column restore: {e}"));
                }
                host.log(&format!("put back a world-id column left at {:?}", p.cells[0].pos));
                done.push(i);
            }
        }
        for i in done.into_iter().rev() {
            self.probes.remove(i);
        }
    }

    // ------------------------------------------------------------ probes

    /// Whether probe `p` is read now: its world is the one in play (a
    /// world-id probe: it went to the client of this session).
    fn in_context(&self, p: &Probe) -> bool {
        match (&p.wid, p.sent_to) {
            (Some(w), _) => self.wid() == Some(w.as_str()),
            (None, Some(peer)) => Some(peer) == self.mail.peer,
            (None, None) => !p.late && matches!(self.session, Session::Identifying),
        }
    }

    fn run_probes(&mut self, host: &mut impl ServerHost) {
        let tick = self.tick;
        // Late probes out of context past their watch go.
        let ctx: Vec<bool> = self.probes.iter().map(|p| self.in_context(p)).collect();
        let mut i = 0;
        self.probes.retain(|p| {
            let keep = ctx[i] || !p.late || tick < p.late_until;
            i += 1;
            keep
        });

        let mut finished = Vec::new();
        for i in 0..self.probes.len() {
            if !self.in_context(&self.probes[i]) {
                continue;
            }
            if !self.probes[i].sent() {
                if !self.probes[i].late && !self.probes[i].admitted {
                    self.try_admit(i);
                }
                if !self.probes[i].late {
                    continue;
                }
            }
            // The client has the lines: from now on, they run at the
            // consumer's pace, whatever the client's.
            let acked = self.probes[i].seq != 0 && self.mail.acked(self.probes[i].seq);
            let p = &mut self.probes[i];
            if acked && !p.acked && !p.late {
                p.acked = true;
                p.deadline = p.deadline.min(tick + PROBE_TICKS);
            }
            for c in p.cells.iter_mut().filter(|c| c.seen.is_none()) {
                if let Some(level) = host.get_block(&p.dim, c.pos).and_then(|s| token::token_level(&s)) {
                    c.seen = Some(level);
                }
            }
            let complete = p.cells.iter().all(|c| c.seen.is_some());
            if complete || (p.late && tick >= p.late_until) {
                finished.push(i);
            } else if !p.late && p.sent() && tick >= p.deadline {
                p.late = true;
                p.late_until = tick + probes::LATE_TICKS;
                p.lines = Vec::new();
                let p = p.clone();
                self.probe_timed_out(host, &p);
            } else if p.late && p.cells.iter().any(|c| c.seen.is_some()) {
                // A late column filling in: put back the cells as they show.
                let p = p.clone();
                self.restore_seen(host, &p);
                for c in &mut self.probes[i].cells {
                    if c.seen.is_some() {
                        c.restore = Restore::State(String::new());
                    }
                }
            }
        }
        for i in finished.into_iter().rev() {
            let p = self.probes.remove(i);
            self.finish_probe(host, p);
        }
    }

    /// Admits probe `i` when its cells are free among the probes and
    /// transactions of its world; it is published once stored.
    fn try_admit(&mut self, i: usize) {
        let positions: Vec<Pos> = self.probes[i].positions().collect();
        let dim = self.probes[i].dim.clone();
        let wid = self.probes[i].wid.clone();
        let others: Vec<&Probe> = self
            .probes
            .iter()
            .enumerate()
            .filter(|(j, o)| *j != i && (o.wid.is_none() || wid.is_none() || o.wid == wid))
            .map(|(_, p)| p)
            .collect();
        let tx_busy = match &self.session {
            Session::Ready(r) => r
                .world
                .txs
                .iter()
                .flat_map(|t| &t.cells)
                .any(|c| c.dim == dim && positions.contains(&c.pos)),
            _ => false,
        };
        if probes::admit(&others, &dim, &positions) && !tx_busy {
            self.probes[i].admitted = true;
        }
    }

    /// Sends the admitted probes' lines: called once they are stored.
    fn publish_probes(&mut self) {
        let Some(peer) = self.mail.peer else { return };
        let tick = self.tick;
        for p in &mut self.probes {
            if !p.admitted || p.sent() {
                continue;
            }
            p.seq = self.mail.send(S2C::Run {
                tx: p.id,
                lines: p.lines.clone(),
            });
            p.sent_to = Some(peer);
            p.admitted = false;
            p.acked = false;
            p.deadline = tick + probes::UNACKED_TICKS;
        }
    }

    /// Puts back the cells of `p` that show a token.
    fn restore_seen(&mut self, host: &mut impl ServerHost, p: &Probe) {
        let writes: Vec<(Pos, String)> = p
            .cells
            .iter()
            .filter(|c| c.seen.is_some())
            .filter_map(|c| match &c.restore {
                Restore::State(s) if !s.is_empty() => Some((c.pos, s.clone())),
                _ => None,
            })
            .collect();
        if !writes.is_empty()
            && let Err(e) = host.set_blocks(&p.dim, &writes)
        {
            host.log(&format!("probe restore: {e}"));
        }
    }

    fn probe_timed_out(&mut self, host: &mut impl ServerHost, p: &Probe) {
        match &p.purpose {
            Purpose::WorldId => self.world_id_failed(host),
            Purpose::Radiation { player } => {
                let levels: Vec<Option<u8>> = p.cells.iter().map(|c| c.seen).collect();
                self.radiation_done(host, p.id, player, &levels);
            }
        }
    }

    fn world_id_failed(&mut self, host: &mut impl ServerHost) {
        self.id_attempts += 1;
        self.next_id_try = self.tick + 100 * u64::from(self.id_attempts.min(12));
        if self.id_attempts == 2 {
            self.tell_all(
                host,
                "NTM: não consegui identificar este mundo. As máquinas do NTM \
                 precisam de comandos ligados (cheats, ou Abrir para LAN > \
                 Permitir comandos).",
            );
        }
    }

    fn finish_probe(&mut self, host: &mut impl ServerHost, p: Probe) {
        self.restore_seen(host, &p);
        if p.late {
            return;
        }
        let levels: Vec<Option<u8>> = p.cells.iter().map(|c| c.seen).collect();
        match p.purpose.clone() {
            Purpose::Radiation { player } => self.radiation_done(host, p.id, &player, &levels),
            Purpose::WorldId => match worldid::decode(&levels) {
                Some(wid) if matches!(self.session, Session::Identifying) => {
                    self.enter_world(host, wid)
                }
                Some(_) => {}
                None => self.world_id_failed(host),
            },
        }
    }

    fn tell_all(&mut self, host: &mut impl ServerHost, text: &str) {
        for p in &self.players {
            host.tell(&p.id, text);
        }
    }

    // ------------------------------------------------------ transactions

    /// A new transaction in the world in play: saved before its lines go.
    fn new_tx(&mut self, dim: &str, pos: Pos, kind: tx::Kind) {
        let key = (dim.to_string(), pos);
        let snap = self
            .snapshot
            .as_ref()
            .filter(|(k, _)| *k == key)
            .map(|(_, m)| m.clone());
        let Some(r) = self.ready() else { return };
        let before = snap.or_else(|| r.world.machines.get(&key).cloned()).map(Box::new);
        let id = r.world.next_tx;
        r.world.next_tx += 1;
        let mut t = Tx {
            id,
            dim: dim.into(),
            pos,
            kind,
            name: String::new(),
            before,
            cells: Vec::new(),
            sent_at: None,
            sends: 0,
        };
        t.name = t.holder(&r.world.salt);
        if !t.fits(&r.world.salt)
            && let tx::Kind::Drop { item, count } = &t.kind
        {
            // A drop whose line can't fit a quiet line: to the player.
            t.kind = tx::Kind::Give {
                item: item.clone(),
                count: *count,
            };
        }
        r.world.txs.push(t);
        r.urgent = true;
    }

    /// An answer cell under the client's player: a block the bottom row of
    /// its chunk holds that can be borrowed and given back exactly, not in
    /// use by any probe or transaction.
    fn answer_cell(&mut self, host: &mut impl ServerHost) -> Option<tx::Cell> {
        let p = self.client_player()?.clone();
        let busy = self.busy_cells();
        let y = worldid::min_y(&p.dim);
        let (cx, cz) = ((p.pos[0].floor() as i32) >> 4, (p.pos[2].floor() as i32) >> 4);
        let start = self.ready()?.cursor;
        for k in 0..256 {
            let idx = (start + k) % 256;
            let pos = [cx * 16 + (idx % 16) as i32, y, cz * 16 + (idx / 16) as i32];
            if busy.contains(&(p.dim.clone(), pos)) {
                continue;
            }
            let Some(state) = host.get_block(&p.dim, pos) else {
                continue;
            };
            // Short states only: the answer lines must fit a quiet line.
            if worldid::safe_to_borrow(&state)
                && token::token_level(&state).is_none()
                && state.len() <= 40
            {
                self.ready()?.cursor = idx + 1;
                return Some(tx::Cell {
                    dim: p.dim,
                    pos,
                    orig: state,
                });
            }
        }
        None
    }

    /// Answers read, outcomes applied, transactions due prepared.
    fn run_txs(&mut self, host: &mut impl ServerHost) {
        let tick = self.tick;
        if let Some(r) = self.ready() {
            r.world.ticks += 1;
        }
        self.run_applied(host);
        // Late tokens at the cells of finished transactions.
        if let Some(r) = self.ready() {
            let now = r.world.ticks;
            let retired = std::mem::take(&mut r.world.retired);
            let before = retired.len();
            let mut keep = Vec::new();
            for (c, until) in retired {
                let token = host
                    .get_block(&c.dim, c.pos)
                    .is_some_and(|s| token::token_level(&s).is_some());
                // Queries still queued may write it again: watched to the end.
                if token {
                    let _ = host.set_blocks(&c.dim, &[(c.pos, c.orig.clone())]);
                }
                if until > now {
                    keep.push((c, until));
                }
            }
            if let Some(r) = self.ready() {
                if keep.len() != before {
                    r.urgent = true;
                }
                r.world.retired = keep;
            }
        }

        // Answers.
        let txs = self.ready().map(|r| r.world.txs.clone()).unwrap_or_default();
        let mut outcomes = Vec::new();
        for t in &txs {
            let mut outcome = None;
            let mut writes: Vec<&tx::Cell> = Vec::new();
            for c in &t.cells {
                let Some(level) = host
                    .get_block(&c.dim, c.pos)
                    .and_then(|s| token::token_level(&s))
                else {
                    continue;
                };
                // A token at a cell this tx borrowed: its answer, or one it
                // can't read; either way the cell is put back.
                writes.push(c);
                if outcome.is_none() {
                    outcome = t.read(level);
                }
            }
            for c in writes {
                if let Err(e) = host.set_blocks(&c.dim, &[(c.pos, c.orig.clone())]) {
                    host.log(&format!("answer cell restore: {e}"));
                }
            }
            if let Some(o) = outcome {
                outcomes.push((t.id, o));
            }
        }
        for (id, o) in outcomes {
            self.resolve(host, id, o);
        }

        // Resets the client has: out of the save.
        let peer = self.mail.peer;
        let sent = self.ready().and_then(|r| r.resets_sent);
        let acked_now = sent.is_some_and(|(p, seq, _, at)| {
            at.is_none() && Some(p) == peer && self.mail.acked(seq)
        });
        if let Some(r) = self.ready()
            && let Some((p, seq, n, at)) = r.resets_sent
        {
            if Some(p) != peer {
                r.resets_sent = None; // a new session: sent again
            } else if acked_now {
                r.resets_sent = Some((p, seq, n, Some(tick)));
            } else if at.is_some_and(|a| tick >= a + RESET_SETTLE) {
                let n = n.min(r.world.resets.len());
                r.world.resets.drain(..n);
                r.urgent = true;
                r.resets_sent = None;
            }
        }

        // Due: never sent in this run, or unanswered for a while; none while
        // the kept ones are being checked against the world (a rollback may
        // still put a machine back, and these would act on the new state).
        if self.mail.peer.is_none() || self.ready().is_some_and(|r| !r.verify.is_empty()) {
            return;
        }
        let due: Vec<u64> = match &self.session {
            Session::Ready(r) => r
                .world
                .txs
                .iter()
                .filter(|t| !r.to_publish.contains(&t.id))
                .filter(|t| {
                    let wait = (TX_RESEND << t.sends.min(6)).min(TX_RESEND_MAX);
                    t.sent_at.is_none_or(|s| tick >= s + wait)
                })
                .map(|t| t.id)
                .collect(),
            _ => return,
        };
        for id in due {
            // The cell it has, while that one is loaded and still ours; a
            // new one under the player otherwise.
            let newest = self
                .ready()
                .and_then(|r| r.world.txs.iter().find(|t| t.id == id))
                .and_then(|t| t.cells.last().cloned());
            let usable = newest.is_some_and(|c| {
                host.get_block(&c.dim, c.pos)
                    .is_some_and(|s| s == c.orig || token::token_level(&s).is_some())
            });
            let cell = if usable { None } else { self.answer_cell(host) };
            let Some(r) = self.ready() else { return };
            let Some(t) = r.world.txs.iter_mut().find(|t| t.id == id) else {
                continue;
            };
            if let Some(c) = cell {
                t.cells.push(c);
                if t.cells.len() > TX_CELLS {
                    let old = t.cells.remove(0);
                    let until = r.world.ticks + RETIRED_TICKS;
                    r.world.retired.push((old, until));
                }
            }
            r.to_publish.insert(id);
            r.urgent = true;
        }
    }

    /// Applied transactions: checked against the world after each entry
    /// (undone if it forgot them), dropped once old enough.
    fn run_applied(&mut self, host: &mut impl ServerHost) {
        let tick = self.tick;
        let Some(r) = self.ready() else { return };
        // Old enough: the world surely saved them; their scores go.
        let now = r.world.ticks;
        let (old, keep): (Vec<Applied>, Vec<Applied>) = std::mem::take(&mut r.world.applied)
            .into_iter()
            .partition(|a| now >= a.at + RETAIN_TICKS && !r.verify.contains(&a.tx.id));
        r.world.applied = keep;
        if !old.is_empty() {
            r.urgent = true;
        }
        let salt = r.world.salt.clone();
        for a in old {
            r.world.resets.extend(a.tx.reset_lines(&salt));
        }

        // Answers to the checks.
        let checks: Vec<Applied> = r
            .world
            .applied
            .iter()
            .filter(|a| r.verify.contains(&a.tx.id))
            .cloned()
            .collect();
        let mut answers = Vec::new();
        for a in &checks {
            for c in &a.tx.cells {
                let Some(level) = host
                    .get_block(&c.dim, c.pos)
                    .and_then(|s| token::token_level(&s))
                else {
                    continue;
                };
                let _ = host.set_blocks(&c.dim, &[(c.pos, c.orig.clone())]);
                if let Some(m) = Applied::read(level) {
                    answers.push((a.clone(), m));
                    break;
                }
            }
        }
        for (a, m) in answers {
            let Some(r) = self.ready() else { return };
            r.verify.remove(&a.tx.id);
            r.verify_publish.remove(&a.tx.id);
            for c in &a.tx.cells {
                r.world.retired.push((c.clone(), r.world.ticks + RETIRED_TICKS));
            }
            if let Some(kept) = r.world.applied.iter_mut().find(|x| x.tx.id == a.tx.id) {
                kept.tx.cells.clear();
            }
            r.urgent = true;
            if m == Remembered::No {
                r.world.applied.retain(|x| x.tx.id != a.tx.id);
                self.undo(host, &a.tx);
            }
        }

        // Checks due.
        if self.mail.peer.is_none() {
            return;
        }
        let due: Vec<u64> = match &self.session {
            Session::Ready(r) => r
                .world
                .applied
                .iter()
                .filter(|a| r.verify.contains(&a.tx.id) && !r.verify_publish.contains(&a.tx.id))
                .filter(|a| {
                    let wait = (TX_RESEND << a.tx.sends.min(6)).min(TX_RESEND_MAX);
                    a.tx.sent_at.is_none_or(|s| tick >= s + wait)
                })
                .map(|a| a.tx.id)
                .collect(),
            _ => return,
        };
        for id in due {
            let newest = self
                .ready()
                .and_then(|r| r.world.applied.iter().find(|a| a.tx.id == id))
                .and_then(|a| a.tx.cells.last().cloned());
            let usable = newest.is_some_and(|c| {
                host.get_block(&c.dim, c.pos)
                    .is_some_and(|s| s == c.orig || token::token_level(&s).is_some())
            });
            let cell = if usable { None } else { self.answer_cell(host) };
            let Some(r) = self.ready() else { return };
            let Some(a) = r.world.applied.iter_mut().find(|a| a.tx.id == id) else {
                continue;
            };
            if let Some(c) = cell {
                a.tx.cells.push(c);
            }
            if a.tx.cells.is_empty() {
                continue;
            }
            r.verify_publish.insert(id);
            r.urgent = true;
        }
    }

    /// The world came back without a transaction that had gone through (a
    /// crash rolled it back). The machine goes back to how it was right
    /// before that transaction, which undoes the transaction and everything
    /// that came of it: what it smelted or pressed, outputs taken out since
    /// (those transactions are later, so the world forgot them too), and the
    /// later transactions still pending on it. Without a snapshot or without
    /// its block in the world, the transaction's own effect is undone.
    fn undo(&mut self, host: &mut impl ServerHost, t: &Tx) {
        host.log(&format!(
            "the world forgot transaction {} ({:?}): undoing the machine's side",
            t.id, t.kind
        ));
        let key = (t.dim.clone(), t.pos);
        let block_here = t.before.as_ref().is_some_and(|m| {
            host.get_block(&t.dim, t.pos)
                .is_some_and(|s| blockstate::id_of(&s) == m.block)
        });
        if let Some(before) = t.before.clone().filter(|_| block_here) {
            let Some(r) = self.ready() else { return };
            if r.restored.get(&key).is_some_and(|&k| k <= t.id) {
                return; // already back to an older state
            }
            r.restored.insert(key.clone(), t.id);
            r.world.machines.insert(key.clone(), *before);
            r.seen.insert(key.clone());
            r.loaded.remove(&key);
            r.shown.remove(&key);
            r.topo_dirty = true;
            r.urgent = true;
            // What came later on this machine never happened in this world.
            let later: Vec<Tx> = r
                .world
                .txs
                .iter()
                .filter(|x| x.dim == key.0 && x.pos == key.1 && x.id > t.id)
                .cloned()
                .collect();
            r.world
                .txs
                .retain(|x| !(x.dim == key.0 && x.pos == key.1 && x.id > t.id));
            let salt = r.world.salt.clone();
            for x in later {
                r.to_publish.remove(&x.id);
                for c in &x.cells {
                    r.world.retired.push((c.clone(), r.world.ticks + RETIRED_TICKS));
                }
                r.world.resets.extend(x.reset_lines(&salt));
            }
            return;
        }
        match &t.kind {
            tx::Kind::Take { item, .. } => {
                let undone = self
                    .ready()
                    .and_then(|r| r.world.machines.get_mut(&key))
                    .is_some_and(|m| m.logic_mut().unconsume(item));
                if !undone {
                    host.log(&format!("{item} was already used by the machine"));
                }
            }
            tx::Kind::Give { item, count } => self.new_tx(
                &t.dim,
                t.pos,
                tx::Kind::Give {
                    item: item.clone(),
                    count: *count,
                },
            ),
            tx::Kind::Drop { item, count } => self.new_tx(
                &t.dim,
                t.pos,
                tx::Kind::Drop {
                    item: item.clone(),
                    count: *count,
                },
            ),
        }
    }

    /// Applies a transaction's answer.
    fn resolve(&mut self, host: &mut impl ServerHost, id: u64, outcome: Outcome) {
        let Some(r) = self.ready() else { return };
        let Some(i) = r.world.txs.iter().position(|t| t.id == id) else {
            return;
        };
        let t = r.world.txs.remove(i);
        r.to_publish.remove(&id);
        r.urgent = true;
        for c in &t.cells {
            r.world.retired.push((c.clone(), r.world.ticks + RETIRED_TICKS));
        }
        let key = (t.dim.clone(), t.pos);
        let mut fx = Vec::new();
        let mut follow = None;
        match (&t.kind, outcome) {
            (tx::Kind::Take { item, .. }, Outcome::Ok(tag)) => match r.world.machines.get_mut(&key) {
                Some(m) => m.logic_mut().consumed_tagged(item, tag, &mut fx),
                None => {
                    // The machine went away meanwhile: the item goes back.
                    let item = match tag {
                        0 => item.clone(),
                        b => format!("{item}[minecraft:custom_data={{{}:{b}}}]", crate::press::WEAR_KEY),
                    };
                    follow = Some(tx::Kind::Give { item, count: 1 });
                }
            },
            (tx::Kind::Take { .. }, Outcome::Failed) => {
                fx.push(Effect::Toast("O item não está mais na sua mão.".into()))
            }
            (tx::Kind::Give { .. }, Outcome::Ok(_)) | (tx::Kind::Drop { .. }, Outcome::Ok(_)) => {}
            (tx::Kind::Give { item, count }, Outcome::Failed) => {
                host.log(&format!("give of {count} {item} failed: the item is unknown"));
            }
            (tx::Kind::Drop { item, count }, Outcome::Failed) => {
                // The drop couldn't be summoned (the place unloaded): to the
                // player instead.
                follow = Some(tx::Kind::Give {
                    item: item.clone(),
                    count: *count,
                });
            }
        }
        match outcome {
            // Kept, to be checked against the world if it comes back older.
            Outcome::Ok(_) => {
                let at = r.world.ticks;
                let mut kept = t.clone();
                kept.cells.clear();
                kept.sent_at = None;
                kept.sends = 0;
                r.world.applied.push(Applied { tx: kept, at });
            }
            Outcome::Failed => {
                let lines = t.reset_lines(&r.world.salt);
                r.world.resets.extend(lines);
            }
        }
        if let Some(kind) = follow {
            self.new_tx(&t.dim, t.pos, kind);
        }
        self.effects(host, &t.dim, t.pos, fx);
        self.push_view(true);
    }

    /// Sends the lines of the transactions due, now that the save holding
    /// them (and their answer cells) succeeded; then the resets of the
    /// finished ones.
    fn publish_txs(&mut self, _host: &mut impl ServerHost) {
        if self.mail.peer.is_none() {
            return;
        }
        let tick = self.tick;
        let Some(r) = self.ready() else { return };
        let salt = r.world.salt.clone();
        let mut runs = Vec::new();
        for id in std::mem::take(&mut r.to_publish) {
            let Some(t) = r.world.txs.iter_mut().find(|t| t.id == id) else {
                continue;
            };
            let mut lines = t.act_lines(&salt);
            if let Some(c) = t.cells.last() {
                lines.extend(t.query_lines(&salt, c));
            }
            t.sent_at = Some(tick);
            t.sends += 1;
            runs.push((id, lines));
        }
        for id in std::mem::take(&mut r.verify_publish) {
            let Some(a) = r.world.applied.iter_mut().find(|a| a.tx.id == id) else {
                continue;
            };
            let Some(c) = a.tx.cells.last() else { continue };
            let lines = a.verify_lines(&salt, c);
            a.tx.sent_at = Some(tick);
            a.tx.sends += 1;
            runs.push((id, lines));
        }
        // Resets: one batch at a time, dropped from the save once acked.
        if r.resets_sent.is_none() && !r.world.resets.is_empty() {
            let n = r.world.resets.len().min(200);
            let lines: Vec<String> = r.world.resets[..n].to_vec();
            let peer = self.mail.peer.unwrap_or(0);
            let seq = self.mail.send(S2C::Run { tx: 0, lines });
            if let Some(r) = self.ready() {
                r.resets_sent = Some((peer, seq, n, None));
            }
        }
        for (id, lines) in runs {
            if let Some(l) = lines.iter().find(|l| l.len() > token::MAX_LINE) {
                // The game would cut it: never sent (a bug, not a state).
                _host.log(&format!("transaction {id}: a line of {} chars not sent: {l}", l.len()));
                continue;
            }
            self.mail.send(S2C::Run {
                tx: id as u32,
                lines,
            });
        }
    }

    fn effects(&mut self, host: &mut impl ServerHost, dim: &str, pos: Pos, fx: Vec<Effect>) {
        for e in fx {
            match e {
                Effect::TakeHeld { item } if token::valid_id(&item) => self.new_tx(
                    dim,
                    pos,
                    tx::Kind::Take {
                        item,
                        tagged: false,
                    },
                ),
                Effect::TakeTagged { item, .. } if token::valid_id(&item) => {
                    self.new_tx(dim, pos, tx::Kind::Take { item, tagged: true })
                }
                Effect::Give { item, count } if token::valid_item(&item) => {
                    self.new_tx(dim, pos, tx::Kind::Give { item, count })
                }
                Effect::Toast(text) => {
                    self.mail.send_latest(S2C::Toast { text }, "toast");
                }
                other => host.log(&format!("refused an effect with a malformed item: {other:?}")),
            }
        }
    }

    // ---------------------------------------------------------- messages

    /// Whether the client's player may use the machine at `key`.
    fn within_reach(&self, key: &Key) -> bool {
        let Some(p) = self.client_player() else {
            return false;
        };
        let c = [
            f64::from(key.1[0]) + 0.5,
            f64::from(key.1[1]) + 0.5,
            f64::from(key.1[2]) + 0.5,
        ];
        p.dim == key.0 && (0..3).map(|i| (p.pos[i] - c[i]).powi(2)).sum::<f64>() <= REACH * REACH
    }

    fn message(&mut self, host: &mut impl ServerHost, m: C2S) {
        match m {
            C2S::Hello { dim, pos, .. } => {
                if !dim.is_empty() {
                    self.client_at = Some((dim, pos));
                }
            }
            C2S::Open { dim, pos } => {
                let key = (dim.clone(), pos);
                if self.ready().is_none() {
                    self.mail.send_latest(
                        S2C::Toast {
                            text: "NTM: o mundo ainda está carregando.".into(),
                        },
                        "toast",
                    );
                    return;
                }
                if !self.within_reach(&key) {
                    self.mail.send(S2C::Close);
                    return;
                }
                let Some(r) = self.ready() else { return };
                if !r.world.machines.contains_key(&key) {
                    if let Some(state) = host.get_block(&dim, pos)
                        && let Some(m) = machine_from_state(&state)
                    {
                        r.world.machines.insert(key.clone(), m);
                        r.seen.insert(key.clone());
                        r.urgent = true;
                        r.topo_dirty = true;
                        r.loaded.insert(key.clone(), true);
                    } else {
                        self.mail.send(S2C::Close);
                        return;
                    }
                }
                r.viewer = Some((key, None));
                self.push_view(true);
            }
            C2S::Action {
                dim,
                pos,
                action,
                held,
            } => self.action(host, &dim, pos, action, held.as_ref()),
            C2S::Close => {
                if let Some(r) = self.ready() {
                    r.viewer = None;
                }
            }
            C2S::Detonator { dim, target, link } => self.detonator(host, dim, target, link),
        }
    }

    fn action(&mut self, host: &mut impl ServerHost, dim: &str, pos: Pos, action: u32, held: Option<&Held>) {
        let key = (dim.to_string(), pos);
        if !self.within_reach(&key) {
            self.mail.send(S2C::Close);
            return;
        }
        let creative = self.client_player().is_some_and(|p| p.creative);
        let Some(r) = self.ready() else { return };
        let Some(m) = r.world.machines.get_mut(&key) else {
            self.mail.send(S2C::Close);
            return;
        };
        let snapshot = (key.clone(), m.clone());
        let mut fx = Vec::new();
        if m.kind() == Kind::Battery && action == actions::BATTERY_DEBUG_FILL && !creative {
            fx.push(Effect::Toast("Esse botão de teste só funciona no modo criativo.".into()));
        } else {
            m.logic_mut().action(action, held, &mut fx);
            r.urgent = true;
        }
        self.snapshot = Some(snapshot);
        self.effects(host, dim, pos, fx);
        self.snapshot = None;
        self.push_view(true);
    }

    /// Sends the open screen when it changed (or `force`).
    fn push_view(&mut self, force: bool) {
        let creative = self.client_player().is_some_and(|p| p.creative);
        let Some(r) = self.ready() else { return };
        let Some(((dim, pos), last)) = &mut r.viewer else {
            return;
        };
        let Some(m) = r.world.machines.get(&(dim.clone(), *pos)) else {
            r.viewer = None;
            self.mail.send(S2C::Close);
            return;
        };
        let mut vm = m.logic().view();
        if !creative {
            // The test button works only in creative: it isn't shown elsewhere.
            vm.widgets.retain(|w| {
                !matches!(w, crate::view::Widget::Button { action, .. } if *action == actions::BATTERY_DEBUG_FILL)
            });
        }
        let pending = r
            .world
            .txs
            .iter()
            .filter(|t| t.dim == *dim && t.pos == *pos)
            .count();
        if pending > 0 {
            vm = vm.text(
                format!("Aguardando o cliente: {pending} transação(ões)"),
                crate::view::GREY,
            );
        }
        if !force && last.as_ref() == Some(&vm) {
            return;
        }
        *last = Some(vm.clone());
        let msg = S2C::View {
            dim: dim.clone(),
            pos: *pos,
            vm,
        };
        self.mail.send_latest(msg, "view");
    }

    // ------------------------------------------------------------- world

    /// A machine's contents dropped at `pos`, a transaction per stack.
    fn drop_contents(&mut self, dim: &str, pos: Pos, m: &Machine) {
        self.snapshot = Some(((dim.to_string(), pos), m.clone()));
        for (item, count) in m.logic().contents() {
            if count > 0 && token::valid_item(&item) {
                self.new_tx(dim, pos, tx::Kind::Drop { item, count });
            }
        }
        self.snapshot = None;
    }

    fn block_event(&mut self, host: &mut impl ServerHost, e: BlockEvent) {
        if let BlockEvent::Placed(dim, pos, state) = &e
            && registry::is_bomb(blockstate::id_of(state))
        {
            self.bomb_placed((dim.clone(), *pos));
        }
        let Some(r) = self.ready() else { return };
        match e {
            BlockEvent::Placed(dim, pos, state) => {
                let id = blockstate::id_of(&state);
                let key = (dim, pos);
                if let Some(m) = machine_from_state(&state) {
                    r.world.machines.insert(key.clone(), m);
                    r.loaded.insert(key.clone(), true);
                    r.seen.insert(key.clone());
                    r.shown.insert(key, state.clone());
                    r.urgent = true;
                    r.topo_dirty = true;
                } else if registry::is_cable(id) {
                    r.world.cables.insert(key);
                    r.urgent = true;
                    r.topo_dirty = true;
                }
            }
            BlockEvent::Broken(dim, pos, _state) => {
                let key = (dim.clone(), pos);
                if let Some(m) = r.world.machines.remove(&key) {
                    r.loaded.remove(&key);
                    r.shown.remove(&key);
                    r.seen.remove(&key);
                    r.urgent = true;
                    r.topo_dirty = true;
                    host.log(&format!("machine {} broken at {pos:?}", m.block));
                    self.drop_contents(&dim, pos, &m);
                    self.push_view(false);
                } else if r.world.cables.remove(&key) {
                    r.urgent = true;
                    r.topo_dirty = true;
                }
            }
        }
    }

    /// Machines, networks, looks, screens, saves: one tick of a world.
    fn simulate(&mut self, host: &mut impl ServerHost) {
        let tick = self.tick;
        let mut removed: Vec<(Key, Machine, bool)> = Vec::new();
        let Some(r) = self.ready() else { return };

        // Loaded chunks, and what the world shows, a machine every 20 ticks.
        for (i, (key, m)) in r.world.machines.iter().enumerate() {
            let known = r.loaded.get(key).copied();
            if known.is_some() && !(tick + i as u64).is_multiple_of(LOAD_CHECK) {
                continue;
            }
            let actual = host.get_block(&key.0, key.1);
            r.loaded.insert(key.clone(), actual.is_some());
            let Some(actual) = actual else { continue };
            let id = blockstate::id_of(&actual);
            if id == m.block {
                r.shown.insert(key.clone(), actual);
                r.suspect.remove(key);
                r.seen.insert(key.clone());
            } else if token::token_level(&actual).is_some() {
                // A token from an earlier version's probe: the machine back.
                r.shown.remove(key);
            } else if r.suspect.get(key).is_some_and(|t| tick >= t + LOAD_CHECK) {
                removed.push((key.clone(), m.clone(), r.seen.contains(key)));
            } else {
                r.suspect.entry(key.clone()).or_insert(tick);
                r.loaded.insert(key.clone(), false);
            }
        }
        // Cables, the same way (they hold nothing).
        let mut gone_cables = Vec::new();
        for (i, key) in r.world.cables.iter().enumerate() {
            if !(tick + i as u64).is_multiple_of(LOAD_CHECK) {
                continue;
            }
            let Some(actual) = host.get_block(&key.0, key.1) else {
                continue;
            };
            if registry::is_cable(blockstate::id_of(&actual)) {
                r.suspect.remove(key);
            } else if r.suspect.get(key).is_some_and(|t| tick >= t + LOAD_CHECK) {
                gone_cables.push(key.clone());
            } else {
                r.suspect.entry(key.clone()).or_insert(tick);
            }
        }
        for key in gone_cables {
            r.world.cables.remove(&key);
            r.suspect.remove(&key);
            r.urgent = true;
            r.topo_dirty = true;
        }
        for (key, m, _) in &removed {
            r.world.machines.remove(key);
            r.loaded.remove(key);
            r.shown.remove(key);
            r.suspect.remove(key);
            host.log(&format!("machine {} at {:?} is gone from the world", m.block, key.1));
            r.urgent = true;
            r.topo_dirty = true;
        }

        // Machine ticks.
        let mut any = false;
        for (key, m) in r.world.machines.iter_mut() {
            if r.loaded.get(key) == Some(&true) {
                any |= m.logic_mut().tick();
            }
        }

        // Energy: a machine may be in several networks (one per separate
        // port): they move energy one after the other.
        if r.topo_dirty {
            r.topo = build_topology(&r.world);
            r.topo_dirty = false;
        }
        for (dim, t) in &r.topo {
            for net in &t.networks {
                let mut group: Vec<&mut dyn EnergyNode> = r
                    .world
                    .machines
                    .iter_mut()
                    .filter(|(k, _)| k.0 == *dim && net.binary_search(&k.1).is_ok())
                    .filter(|(k, _)| r.loaded.get(*k) == Some(&true))
                    .filter_map(|(_, m)| m.logic_mut().energy())
                    .collect();
                any |= energy::distribute(&mut group) > 0;
            }
        }
        if any {
            r.dirty = true;
        }

        // Looks: what changed, written only where the machine still is.
        let mut writes: BTreeMap<String, Vec<(Pos, String)>> = BTreeMap::new();
        let mut recheck = Vec::new();
        for (key, m) in &r.world.machines {
            if r.loaded.get(key) != Some(&true) {
                continue;
            }
            let Some(want) = desired_state(m) else { continue };
            if r.shown.get(key).is_some_and(|s| same_look(s, &want)) {
                continue;
            }
            match host.get_block(&key.0, key.1) {
                Some(actual)
                    if blockstate::id_of(&actual) == m.block
                        || token::token_level(&actual).is_some() =>
                {
                    writes.entry(key.0.clone()).or_default().push((key.1, want.clone()));
                    r.shown.insert(key.clone(), want);
                }
                _ => recheck.push(key.clone()),
            }
        }
        for key in recheck {
            r.loaded.remove(&key);
            r.shown.remove(&key);
        }
        for (dim, blocks) in writes {
            for chunk in blocks.chunks(4096) {
                if let Err(e) = host.set_blocks(&dim, chunk) {
                    host.log(&format!("set-blocks: {e}"));
                }
            }
        }

        // Saves of energy and progress.
        if r.saver.busy() {
            if let Err(e) = r.saver.finish(host) {
                host.log(&format!("save: {e}"));
            }
        } else if r.dirty && !r.urgent && tick >= r.next_save {
            r.next_save = tick + SAVE_EVERY;
            if let Err(e) = save(host, r, false) {
                host.log(&format!("save: {e}"));
            }
        }

        let viewer_gone = r
            .viewer
            .as_ref()
            .is_some_and(|(k, _)| !r.world.machines.contains_key(k));
        if viewer_gone || tick.is_multiple_of(VIEW_EVERY) {
            self.push_view(false);
        }
        for (key, m, seen) in removed {
            if seen {
                self.drop_contents(&key.0, key.1, &m);
            } else if !m.logic().contents().is_empty() {
                host.log(&format!(
                    "machine {} at {:?} was never seen in this world: its contents are not dropped",
                    m.block, key.1
                ));
            }
        }
    }

    /// The save of a change that moved items, blocks or transactions, now.
    /// True when nothing waits for a save: what depends on it may leave.
    fn save_urgent(&mut self, host: &mut impl ServerHost) -> bool {
        let Some(r) = self.ready() else { return true };
        if !r.urgent {
            return true;
        }
        let result = save(host, r, false);
        let (wid, bytes) = (r.wid.clone(), r.saver.pending_len() as u64);
        match result {
            Ok(()) => {
                self.warned_save = false;
                let info = self.worlds.worlds.entry(wid).or_default();
                let changed = info.bytes.abs_diff(bytes) > 4096 || info.bytes == 0;
                info.bytes = bytes;
                if changed && let Err(e) = self.worlds.store(host) {
                    host.log(&format!("worlds: {e}"));
                }
                let explosions = self.blasts.nukes.stored();
                if self.worlds.nearly_full(explosions) && !self.warned_full {
                    self.warned_full = true;
                    let (used, n) = self.worlds.usage();
                    let (used, n) = (used + explosions.0, n + explosions.1);
                    host.log(&format!("storage nearly full: {} KiB, {n} keys", used >> 10));
                    self.tell_all(
                        host,
                        "NTM: a pasta de dados do mod está quase cheia (muitos mundos com \
                         máquinas). Saves novos podem falhar; veja /ntmlogic forget.",
                    );
                }
                true
            }
            Err(e) => {
                // Nothing that moves items leaves until the save succeeds.
                host.log(&format!("urgent save of {wid}: {e}"));
                if !self.warned_save {
                    self.warned_save = true;
                    self.tell_all(
                        host,
                        &format!("NTM: o save das máquinas falhou ({e}); as trocas de itens ficam paradas."),
                    );
                }
                false
            }
        }
    }

    /// Writes the probes when they changed: true when they are durable.
    fn store_probes(&mut self, host: &mut impl ServerHost) -> bool {
        match self.probe_store.store(host, &self.probes) {
            Ok(()) => true,
            Err(e) => {
                host.log(&format!("probes: {e}"));
                false
            }
        }
    }

    /// `/ntmlogic status`.
    pub fn status(&self) -> String {
        let session = match &self.session {
            Session::Idle => "sem jogador".to_string(),
            Session::Identifying => format!("identificando (tentativa {})", self.id_attempts + 1),
            Session::Ready(r) => format!(
                "mundo {} | {} máquinas, {} cabos, {} redes, {} redes de fluido, {} transações | save generation {}",
                r.wid,
                r.world.machines.len(),
                r.world.cables.len(),
                r.topo.values().map(|t| t.networks.len()).sum::<usize>(),
                self.fluidnet.networks(),
                r.world.txs.len(),
                r.generation
            ),
        };
        let late = self.probes.iter().filter(|p| p.late).count();
        format!(
            "NTM lógica: {session} | cliente {} | sondas {} ({late} tardias) | boot {}",
            if self.mail.peer.is_some() { "conectado" } else { "ausente" },
            self.probes.len(),
            self.worlds.boot
        )
    }

    /// `/ntmlogic forget <wid>`: deletes another world's save.
    pub fn forget(&mut self, host: &mut impl ServerHost, wid: &str) -> Result<String, String> {
        if self.wid() == Some(wid) {
            return Err("não dá para esquecer o mundo em jogo".into());
        }
        if wid.is_empty() || !wid.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(format!("id de mundo inválido: {wid}"));
        }
        let known = self.worlds.worlds.remove(wid).is_some();
        let key = keys::world(wid);
        let had = host.get(&key).is_some();
        host.delete(&key);
        if host
            .get(keys::JOURNAL)
            .and_then(|b| persist::decode(&b))
            .is_some_and(|(w, _, _)| w == wid)
        {
            host.delete(keys::JOURNAL);
        }
        self.probes.retain(|p| p.wid.as_deref() != Some(wid));
        self.worlds.store(host)?;
        if !known && !had {
            return Err(format!("nenhum save do mundo {wid}"));
        }
        host.log(&format!("forgot world {wid}"));
        Ok(format!("save do mundo {wid} apagado"))
    }

    /// `/ntmlogic save`.
    pub fn save_now(&mut self, host: &mut impl ServerHost) -> Result<String, String> {
        let Session::Ready(r) = &mut self.session else {
            return Err("nenhum mundo identificado".into());
        };
        save(host, r, true)?;
        Ok(format!("mundo {} salvo (generation {})", r.wid, r.generation))
    }
}

/// Encodes and writes a world; `now` does both steps at once.
fn save(kv: &mut impl Kv, r: &mut Ready, now: bool) -> Result<(), String> {
    if r.wid.is_empty() {
        return Ok(());
    }
    let bytes = persist::encode(&r.wid, r.generation + 1, &r.world.to_save())?;
    r.saver.finish(kv)?;
    if now {
        r.saver.now(kv, &r.wid, bytes)?;
    } else {
        r.saver.begin(kv, &r.wid, bytes)?;
    }
    r.generation += 1;
    r.dirty = false;
    r.urgent = false;
    Ok(())
}

fn build_topology(w: &WorldState) -> BTreeMap<String, Topology> {
    type Ports = BTreeMap<Pos, Vec<crate::energy::Port>>;
    let mut dims: BTreeMap<&str, (BTreeSet<Pos>, Ports)> = BTreeMap::new();
    for (d, p) in &w.cables {
        dims.entry(d).or_default().0.insert(*p);
    }
    for ((d, p), m) in &w.machines {
        let Some(spec) = registry::spec(&m.block) else { continue };
        let ports = registry::ports(spec, m.facing.as_deref());
        if m.kind() != Kind::Inert && !ports.is_empty() {
            dims.entry(d).or_default().1.insert(*p, ports);
        }
    }
    dims.into_iter()
        .map(|(d, (c, m))| (d.to_string(), Topology::build(&c, &m)))
        .collect()
}

/// A new machine for a placed (or found) machine block.
pub fn machine_from_state(state: &str) -> Option<Machine> {
    let s = BlockState::parse(state)?;
    let spec = registry::spec(&s.id)?;
    let facing = spec.facing.and_then(|f| s.get(f)).map(str::to_string);
    Some(Machine::new(spec.kind, &s.id, facing))
}

/// The state that shows `m` now.
pub fn desired_state(m: &Machine) -> Option<String> {
    let spec = registry::spec(&m.block)?;
    let mut s = BlockState::new(&m.block);
    if let (Some(prop), Some(f)) = (spec.facing, &m.facing) {
        s = s.with(prop, f);
    }
    if let Some((prop, max)) = spec.level {
        s = s.with(prop, m.logic().level(max));
    }
    Some(s.to_string())
}

/// Whether `actual` (as the world reports it, every property) already
/// shows `want` (only the properties the logic sets).
fn same_look(actual: &str, want: &str) -> bool {
    // Sem os dois `parse` (~3 k fuel cada, por máquina carregada a cada tick).
    blockstate::shows(actual, want)
}

mod bomb_server;
mod fluid_server;
mod radiation_server;

#[cfg(test)]
mod tests;
