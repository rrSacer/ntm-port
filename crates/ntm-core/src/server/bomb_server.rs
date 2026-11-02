//! The bombs on the server (docs/bombas.md): what sets them off (redstone,
//! a fuse, the detonator), the explosions (`crate::blast`), their damage,
//! radiation and effects, and `/ntmbomba`.
use super::*;
use crate::{
    blast::{self, Config, PULSE_EVERY, PULSE_MAX, PULSE_TICKS, Pulse, Queued, Recorder, Zone, vanilla, visuals},
    bomb::{Blast, FUSE_TICKS},
    machine::Data,
    radiation,
};
use std::cell::RefCell;

/// The engine's view of the world and the storage, over the server's host
/// (one `RefCell`: the engine asks for both at once).
struct Engine<'a, H: ServerHost> {
    host: RefCell<&'a mut H>,
    players: &'a [PlayerInfo],
}

impl<H: ServerHost> crate::explosion::World for &Engine<'_, H> {
    fn block(&mut self, dim: &str, x: i32, y: i32, z: i32) -> Option<String> {
        self.host.borrow_mut().get_block(dim, [x, y, z])
    }
    fn set_blocks(&mut self, dim: &str, changes: Vec<crate::explosion::Change>) -> Result<(), String> {
        let blocks: Vec<(Pos, String)> = changes.into_iter().map(|c| ([c.x, c.y, c.z], c.state)).collect();
        self.host.borrow_mut().set_blocks(dim, &blocks)
    }
    fn viewers(&mut self, dim: &str) -> Vec<[f64; 3]> {
        self.players.iter().filter(|p| p.dim == dim).map(|p| [p.pos[0], p.pos[1] + 1.62, p.pos[2]]).collect()
    }
}

impl<H: ServerHost> crate::explosion::Store for &Engine<'_, H> {
    fn get(&mut self, key: &str) -> Option<Vec<u8>> {
        self.host.borrow_mut().get(key)
    }
    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), String> {
        self.host.borrow_mut().set(key, value)
    }
    fn delete(&mut self, key: &str) {
        self.host.borrow_mut().delete(key)
    }
}

/// What a blast or a fuse shows, sent through the host: effects to their
/// players, the rest to the dimension.
fn show(host: &mut impl ServerHost, dim: &str, shows: Vec<visuals::Show>) {
    for s in shows {
        match s {
            visuals::Show::Effect { player, id, seconds, amplifier } => host.effect(&player, id, seconds, amplifier),
            other => host.show(dim, &other),
        }
    }
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

fn center(p: Pos) -> [f64; 3] {
    [f64::from(p[0]) + 0.5, f64::from(p[1]) + 0.5, f64::from(p[2]) + 0.5]
}

fn eye(p: &PlayerInfo) -> [f64; 3] {
    [p.pos[0], p.pos[1] + 1.62, p.pos[2]]
}

/// Explosion resistance along the line from `a` to `b`, sampled at up to
/// 48 points and scaled to the line's length (`EntityNukeExplosionMK5
/// .radiate` sums it block by block).
fn shielding(host: &mut impl ServerHost, dim: &str, a: [f64; 3], b: [f64; 3]) -> f64 {
    let len = dist(a, b);
    let n = (len as usize).clamp(1, 48);
    let mut sum = 0.0;
    for i in 1..n {
        let t = i as f64 / n as f64;
        let p = [0, 1, 2].map(|k| (a[k] + (b[k] - a[k]) * t).floor() as i32);
        if let Some(s) = host.get_block(dim, p) {
            sum += f64::from(vanilla::resistance(&s).max(0.0));
        }
    }
    sum * len / n as f64
}

/// Whether a block stands between `a` and `b` (`Library.isObstructed`),
/// sampled at up to 24 points.
fn obstructed(host: &mut impl ServerHost, dim: &str, a: [f64; 3], b: [f64; 3]) -> bool {
    let len = dist(a, b);
    let n = (len as usize).clamp(1, 24);
    (1..n).any(|i| {
        let t = i as f64 / n as f64;
        let p = [0, 1, 2].map(|k| (a[k] + (b[k] - a[k]) * t).floor() as i32);
        host.get_block(dim, p).is_some_and(|s| vanilla::resistance(&s) > 0.5)
    })
}

/// The world's tag in its explosions' ids (`w<world>n<number>`): the
/// host's storage is the world's own.
const WORLD: &str = "native";

/// `lit=true` in a bomb's state: redstone powers it.
/// The lamp template has `lit` alone, so the state ends with it.
const BOMB_LIT: &str = "lit=true]";
#[cfg(test)]
pub(super) const BOMB_LIT_FOR_TESTS: &str = BOMB_LIT;

impl Server {
    /// The settings in force: the world's, else the global ones.
    pub fn bomb_config(&mut self, host: &mut impl ServerHost) -> Config {
        if self.blasts.global.is_none() {
            self.blasts.global = Some(Config::load(host.get(blast::config::KEY)));
        }
        let global = self.blasts.global.clone().unwrap_or_default();
        self.ready().and_then(|r| r.world.bombs.cfg.clone()).unwrap_or(global)
    }

    /// A bomb block was placed: redstone already on it sets it off (its
    /// `lit` is a rising edge from here).
    pub(super) fn bomb_placed(&mut self, key: Key) {
        self.blasts.lit.insert(key, false);
    }

    /// Each tick of a world in play: bombs, explosions, damage, zones.
    pub(super) fn bombs_tick(&mut self, host: &mut impl ServerHost) {
        let tick = self.tick;
        if self.blasts.wid.is_none() {
            // The world's explosions come back (its storage holds only its).
            self.blasts.wid = Some(WORLD.into());
            let prefix = format!("w{WORLD}n");
            let mut rec = Recorder::default();
            let resumed = {
                let engine = Engine { host: RefCell::new(&mut *host), players: &[] };
                self.blasts.nukes.resume_matching(&mut &engine, tick, &mut rec, |id| id.starts_with(&prefix))
            };
            for r in resumed {
                host.log(&format!("explosion resumed: {r:?}"));
            }
        }
        for p in &self.players {
            self.blasts.entered.entry(p.id.clone()).or_insert(tick);
        }
        let online: Vec<String> = self.players.iter().map(|p| p.id.clone()).collect();
        self.blasts.entered.retain(|id, _| online.contains(id));
        self.blasts.nukes.fuel = u64::from(self.bomb_config(host).fuel) * 1_000_000;
        self.bombs_admit(host);
        self.bomb_pulses(host);
        // The explosions (one at a time gets the tick's fuel; none in the
        // tick a charge went off, which spent it).
        let mut rec = Recorder::default();
        let finished = if self.blasts.charged_at == Some(tick) {
            Vec::new()
        } else {
            let players = self.players.clone();
            let engine = Engine { host: RefCell::new(&mut *host), players: &players };
            self.blasts.nukes.tick(tick, &mut &engine, &mut &engine, &mut rec)
        };
        for p in finished {
            host.log(&format!("explosion done: {}", p.line()));
        }
        for (id, e) in rec.0 {
            match e {
                crate::explosion::Event::CraterDone { tick } => host.log(&format!("{id}: crater done after {tick} ticks")),
                crate::explosion::Event::FalloutStart { tick } => host.log(&format!("{id}: fallout starts after {tick} ticks")),
                crate::explosion::Event::ResumeFailed { reason } => host.log(&format!("{id}: not resumed ({reason}); its keys were removed")),
                crate::explosion::Event::PersistFailed { reason } => {
                    host.log(&format!("{id}: explosion not saved: {reason}"));
                }
                _ => {}
            }
        }
        if tick.is_multiple_of(20) {
            self.area_dose();
        }
        if tick.is_multiple_of(200)
            && let Some(r) = self.ready()
        {
            let now = r.world.ticks;
            r.world.bombs.zones.retain(|z| !z.spent(now));
        }
    }

    /// Redstone and fuses: which loaded bombs go off this tick.
    pub(super) fn bombs_triggers(&mut self, host: &mut impl ServerHost) {
        let Some(r) = self.ready() else { return };
        let bombs: Vec<(Key, String, bool, Option<Blast>, bool)> = r
            .world
            .machines
            .iter()
            .filter(|(k, _)| r.loaded.get(*k) == Some(&true))
            .filter_map(|(k, m)| match &m.data {
                Data::Bomb(b) => Some((k.clone(), m.block.clone(), b.fuse_out(), b.blast(), b.kind().is_some_and(|t| t.charge.is_some()) && b.fuse.is_none())),
                _ => None,
            })
            .collect();
        for (key, block, fuse_out, blast, unlit_charge) in bombs {
            let Some(state) = host.get_block(&key.0, key.1) else { continue };
            if blockstate::id_of(&state) != block {
                continue; // gone: the reconciliation decides
            }
            // The native host's signal when it reads one (the nukes on beehive
            // have no `lit`); else the lamp's `lit`, a substring, not a parse:
            // this runs for every loaded bomb, every tick.
            let lit = match self.native_signal(&key) {
                Some(s) => s > 0,
                None => state.ends_with(BOMB_LIT) || (!state.ends_with("]") && state.contains(BOMB_LIT)),
            };
            let before = self.blasts.lit.insert(key.clone(), lit);
            let rising = lit && before == Some(false);
            if fuse_out {
                if let Some(b) = blast {
                    self.trigger(host, key, b, "pavio");
                }
            } else if rising && unlit_charge {
                self.light_fuse(host, &key);
            } else if rising && !unlit_charge && let Some(b) = blast.filter(|b| !matches!(b, Blast::Conventional { .. })) {
                self.trigger(host, key, b, "redstone");
            }
        }
    }

    fn light_fuse(&mut self, host: &mut impl ServerHost, key: &Key) {
        let cfg = self.bomb_config(host);
        let Some(r) = self.ready() else { return };
        if let Some(Data::Bomb(b)) = r.world.machines.get_mut(key).map(|m| &mut m.data)
            && b.fuse.is_none()
        {
            b.fuse = Some(FUSE_TICKS);
            let vis = visuals::Optimized;
            show(host, &key.0, vis.fuse(&cfg, &key.0, key.1));
        }
    }

    /// Sets the bomb at `key` off: it leaves the world now, and its blast
    /// joins the queue in the same save (revisão do M2, P1-2 e P1-6).
    pub(super) fn trigger(&mut self, host: &mut impl ServerHost, key: Key, blast: Blast, why: &str) {
        let Some(r) = self.ready() else { return };
        // The bomb is spent: its block and its components go with it.
        let name = r.world.machines.remove(&key).map(|m| m.block).unwrap_or_default();
        r.loaded.remove(&key);
        r.shown.remove(&key);
        r.seen.remove(&key);
        r.suspect.remove(&key);
        r.topo_dirty = true;
        let n = r.world.bombs.next;
        r.world.bombs.next += 1;
        let (dim, pos) = key.clone();
        r.world.bombs.queue.push(Queued { n, dim: dim.clone(), pos, blast, name, why: why.into(), started: false });
        self.blasts.lit.remove(&key);
        if why != "teste"
            && let Err(e) = host.set_blocks(&dim, &[(pos, "minecraft:air".into())])
        {
            host.log(&format!("bomb block: {e}"));
        }
    }

    /// The queue: blasts go off in order while the tick's fuel lasts (a
    /// vanilla explosion of power 15 takes 12 M); nukes handed to the
    /// engine leave it once the engine has them in the folder.
    fn bombs_admit(&mut self, host: &mut impl ServerHost) {
        let budget = u64::from(self.bomb_config(host).fuel) * 1_000_000;
        let Some(r) = self.ready() else { return };
        if r.world.bombs.queue.is_empty() {
            return;
        }
        let queue = r.world.bombs.queue.clone();
        let mut spent = 0;
        let mut keep = Vec::new();
        for mut q in queue {
            let id = format!("w{WORLD}n{}", q.n);
            if q.started {
                if !self.blasts.nukes.durable(&id) {
                    if !self.blasts.nukes.contains(&id) {
                        // Lost with a restart before it was in the folder.
                        self.hand_to_engine(host, &q, &id);
                    }
                    keep.push(q);
                }
                continue;
            }
            let cost = q.cost();
            if spent > 0 && spent + cost > budget {
                keep.push(q);
                continue;
            }
            spent += cost;
            self.fire(host, &q, &id);
            if !matches!(q.blast, Blast::Conventional { .. }) {
                q.started = true;
                keep.push(q);
            }
        }
        if let Some(r) = self.ready() {
            r.world.bombs.queue = keep;
        }
    }

    fn hand_to_engine(&mut self, host: &mut impl ServerHost, q: &Queued, id: &str) {
        let cfg = self.bomb_config(host);
        let (radius, fallout) = match q.blast {
            Blast::Nuke { radius, fallout } => (radius.min(cfg.max_radius), fallout),
            Blast::Balefire { radius } => (radius.min(cfg.max_radius), false),
            Blast::Conventional { .. } => return,
        };
        let mut det = crate::explosion::Detonation::new(id, q.dim.clone(), q.pos, radius);
        det.budget = cfg.budget;
        det.profile = blast::profile(q.blast, fallout && cfg.fallout);
        match self.blasts.nukes.detonate(det, self.tick) {
            Ok(()) => host.log(&format!("{} at {:?} ({}): explosion {id}, radius {radius}", q.name, q.pos, q.why)),
            Err(e) => host.log(&format!("{} at {:?}: {e}", q.name, q.pos)),
        }
    }

    /// A queued blast goes off: the explosion, its effects, damage and radiation.
    fn fire(&mut self, host: &mut impl ServerHost, q: &Queued, id: &str) {
        let cfg = self.bomb_config(host);
        let tick = self.tick;
        let Some(r) = self.ready() else { return };
        let now = r.world.ticks;
        let (dim, pos, name, why) = (q.dim.clone(), q.pos, q.name.clone(), q.why.clone());
        let players: Vec<PlayerInfo> = self.players.iter().filter(|p| p.dim == dim).cloned().collect();
        let blast = match q.blast {
            Blast::Nuke { radius, fallout } => Blast::Nuke { radius: radius.min(cfg.max_radius), fallout },
            Blast::Balefire { radius } => Blast::Balefire { radius: radius.min(cfg.max_radius) },
            b => b,
        };
        let vis = visuals::Optimized;
        show(host, &dim, vis.detonation(&cfg, &visuals::Shot { dim: &dim, pos, blast, players: &players }));
        let c = center(pos);
        match blast {
            Blast::Conventional { power } => {
                self.blasts.charged_at = Some(tick);
                // Damage first, over the blocks as they are (vanilla's order),
                // through what the blast sees of each player.
                if cfg.damage {
                    for p in players.iter().filter(|p| !p.creative) {
                        let d = dist(c, eye(p));
                        if d >= 2.0 * f64::from(power) {
                            continue;
                        }
                        let seen = vanilla::exposure(c, p.pos, &mut |b| host.get_block(&dim, b));
                        let dmg = vanilla::damage(power, d, seen);
                        if dmg >= 1.0 {
                            host.hurt(&p.id, dmg, "minecraft:explosion");
                        }
                    }
                }
                let (cells, reads) = vanilla::explode(c, power, &mut |p| host.get_block(&dim, p));
                let writes: Vec<(Pos, String)> = cells.iter().map(|p| (*p, "minecraft:air".to_string())).collect();
                for chunk in writes.chunks(crate::explosion::BLOCKS_PER_CALL) {
                    if let Err(e) = host.set_blocks(&dim, chunk) {
                        host.log(&format!("charge: {e}"));
                    }
                }
                host.log(&format!("{name} at {pos:?} ({why}): power {power}, {} blocks, {reads} reads", cells.len()));
            }
            Blast::Nuke { radius, .. } | Blast::Balefire { radius } => {
                self.hand_to_engine(host, q, id);
                let fallout = matches!(blast, Blast::Nuke { fallout: true, .. });
                if cfg.damage {
                    self.blasts.pulses.push(Pulse { dim: dim.clone(), center: c, reach: 2.0 * f64::from(radius), next: tick, until: tick + PULSE_TICKS });
                }
                let radioactive = fallout || matches!(blast, Blast::Balefire { .. });
                if cfg.radiation && radioactive {
                    self.radiation_pulse(host, &dim, c, radius, &players);
                    let zone = Zone {
                        dim: dim.clone(),
                        center: pos,
                        radius: (2.5 * radius as f32) as i32,
                        peak: (u64::from(cfg.zone_peak) * radius as u64 / 100).min(u64::from(u32::MAX)) as u32,
                        t0: now,
                        half_life: cfg.zone_half_life,
                    };
                    if let Some(r) = self.ready() {
                        r.world.bombs.zones.push(zone);
                    }
                }
            }
        }
        self.blasts.log.push(format!("{name} {pos:?} {why}"));
        if self.blasts.log.len() > 8 {
            self.blasts.log.remove(0);
        }
    }

    /// The first ticks' radiation of `EntityNukeExplosionMK5.radiate`:
    /// 2.5 M/(5t+1) RAD for t < 10, over the shielding and the distance².
    fn radiation_pulse(&mut self, host: &mut impl ServerHost, dim: &str, c: [f64; 3], radius: i32, players: &[PlayerInfo]) {
        const TOTAL: f64 = 3_780_000.0; // Σ 2 500 000/(5t+1), t = 0..9
        let reach = 2.0 * f64::from(radius);
        for p in players.iter().filter(|p| !p.creative) {
            let e = eye(p);
            let len = dist(c, e).max(1.0);
            if len > reach {
                continue;
            }
            let res = shielding(host, dim, c, e).max(1.0);
            let millirad = (TOTAL / (res * len * len) * 1000.0) as u64;
            if millirad == 0 {
                continue;
            }
            if let Some(r) = self.ready() {
                radiation::add_dose(r.world.radiation.entry(p.id.clone()).or_default(), millirad);
            }
            host.log(&format!("blast radiation: {} RAD to {}", millirad / 1000, p.name));
        }
    }

    /// Blast damage while the craters are dug.
    fn bomb_pulses(&mut self, host: &mut impl ServerHost) {
        let tick = self.tick;
        self.blasts.pulses.retain(|p| tick < p.until);
        let due: Vec<Pulse> = self.blasts.pulses.iter().filter(|p| tick >= p.next).cloned().collect();
        for p in &mut self.blasts.pulses {
            if tick >= p.next {
                p.next = tick + PULSE_EVERY;
            }
        }
        for p in due {
            for pl in self.players.iter().filter(|x| x.dim == p.dim && !x.creative).cloned().collect::<Vec<_>>() {
                let d = dist(p.center, eye(&pl));
                if d > p.reach || obstructed(host, &p.dim, p.center, eye(&pl)) {
                    continue;
                }
                let dmg = PULSE_MAX * (p.reach - d) / p.reach;
                if dmg >= 1.0 {
                    host.hurt(&pl.id, dmg as f32, "minecraft:explosion");
                }
            }
        }
    }

    /// One second of the fallout zones' radiation, for each player in one:
    /// the server knows where they are, so this needs no sample from the
    /// client (`radiation_done` only shows the rate in the Geiger).
    fn area_dose(&mut self) {
        // A carência de entrada da amostra do inventário vale aqui também,
        // por jogador: desde quando cada um está no mundo.
        let tick = self.tick;
        let entered = &self.blasts.entered;
        let in_grace = |id: &str| entered.get(id).is_none_or(|t| tick.saturating_sub(*t) < radiation::GRACE_TICKS);
        let doses: Vec<(String, u32)> = self
            .players
            .iter()
            .filter(|p| !in_grace(&p.id))
            .map(|p| (p.id.clone(), self.area_rate(&p.id)))
            .filter(|(_, r)| *r > 0)
            .collect();
        let Some(r) = self.ready() else { return };
        for (id, rate) in doses {
            let dose = r.world.radiation.entry(id).or_default();
            let before = dose.millirad;
            radiation::add_dose(dose, u64::from(rate));
            // A sickness threshold (every 200 RAD) is saved at once, as the
            // inventory's dose does.
            if before / 200_000 != dose.millirad / 200_000 {
            }
        }
    }

    /// Area radiation at the player's place now (milliRAD/s).
    pub(super) fn area_rate(&self, player: &str) -> u32 {
        let r = &self.ready;
        // Criativo e espectador não se contaminam (`ContaminationUtil`;
        // `PlayerInfo.creative` cobre os dois, ver o server mod).
        let Some(p) = self.players.iter().find(|p| p.id == player && !p.creative) else { return 0 };
        let now = r.world.ticks;
        r.world.bombs.zones.iter().map(|z| z.rate(&p.dim, p.pos, now)).fold(0u32, u32::saturating_add)
    }

    /// The detonator used by player `id` in the native game, on block
    /// `target` or in the air: on a bomb it links (NTM's sneaking click; the
    /// hook doesn't say whether the player sneaks), anywhere else it sets the
    /// linked bomb off. What the player is told; None when `id` isn't in
    /// the world.
    pub fn native_detonator(&mut self, host: &mut impl ServerHost, id: &str, target: Option<Pos>) -> Option<String> {
        let p = self.players.iter().find(|p| p.id == id)?.clone();
        let on_bomb = target.filter(|t| matches!(self.ready.world.machines.get(&(p.dim.clone(), *t)).map(|m| &m.data), Some(Data::Bomb(_))));
        Some(self.detonator(host, &p, on_bomb, on_bomb.is_some()))
    }

    /// The detonator of `player`: link it (sneaking, on a bomb) or set its
    /// bomb off. What the player is told.
    pub fn detonator(&mut self, host: &mut impl ServerHost, player: &PlayerInfo, target: Option<Pos>, link: bool) -> String {
        let dim = player.dim.clone();
        host.log(&format!("detonator: {dim} {target:?} link={link}"));
        let toast = |_: &mut Server, text: &str| text.to_string();
        if link {
            let Some(pos) = target else {
                return toast(self, "Agache e use o detonador numa bomba do NTM para ligá-lo.");
            };
            let key = (dim.clone(), pos);
            let r = &mut self.ready;
            if !r.world.machines.contains_key(&key)
                && let Some(state) = host.get_block(&dim, pos)
                && let Some(m) = machine_from_state(&state)
            {
                r.world.machines.insert(key.clone(), m);
                r.seen.insert(key.clone());
                r.loaded.insert(key.clone(), true);
            }
            let name = match r.world.machines.get(&key).map(|m| &m.data) {
                Some(Data::Bomb(b)) => b.kind().map_or("bomba", |t| t.name),
                _ => return toast(self, "Isso não é uma bomba do NTM."),
            };
            r.world.bombs.links.insert(player.id.clone(), key.clone());
            let text = format!("Detonador ligado: {name} em {} {} {}.", pos[0], pos[1], pos[2]);
            return toast(self, &text);
        }
        let r = &mut self.ready;
        let Some(key) = r.world.bombs.links.get(&player.id).cloned() else {
            return toast(self, "Detonador sem bomba: agache e use-o numa bomba para ligá-lo.");
        };
        if key.0 != player.dim {
            return toast(self, "A bomba ligada está em outra dimensão.");
        }
        let Some(Data::Bomb(b)) = r.world.machines.get(&key).map(|m| m.data.clone()) else {
            r.world.bombs.links.remove(&player.id);
            return toast(self, "A bomba ligada não existe mais.");
        };
        let block = r.world.machines.get(&key).map(|m| m.block.clone()).unwrap_or_default();
        let Some(state) = host.get_block(&key.0, key.1) else {
            return toast(self, "O chunk da bomba não está carregado.");
        };
        if blockstate::id_of(&state) != block {
            // Another block took its place (and the reconciliation will see
            // to the record): nothing goes off (revisão do M2, P2-10).
            return toast(self, "A bomba ligada não está mais lá.");
        }
        match b.blast() {
            None => toast(self, "A bomba não está pronta: faltam componentes."),
            Some(Blast::Conventional { .. }) => {
                self.light_fuse(host, &key);
                toast(self, "Pavio aceso.")
            }
            Some(blast) => {
                self.trigger(host, key, blast, "detonador");
                toast(self, "Detonada.")
            }
        }
    }

    /// `/ntmbomba` (server command): status, settings, test blasts.
    pub fn bomb_command(&mut self, host: &mut impl ServerHost, who: Option<&PlayerInfo>, args: &str) -> Result<String, String> {
        let words: Vec<&str> = args.split_whitespace().collect();
        let usage = "/ntmbomba status | config [global|mundo] [<chave> <valor> | limpar] | teste <raio> [n2|balefire|forca] | cancelar";
        match words.as_slice() {
            [] | ["status"] => {
                let cfg = self.bomb_config(host);
                let mut out = vec![format!("Bombas: {}", cfg.describe())];
                for p in self.blasts.nukes.active() {
                    out.push(p.line());
                }
                if let Some(r) = self.ready()
                    && !r.world.bombs.queue.is_empty()
                {
                    out.push(format!("{} detonação(ões) na fila", r.world.bombs.queue.len()));
                }
                for (id, n) in self.blasts.nukes.lingering() {
                    out.push(format!("{id}: fallout esperando {n} chunk(s)"));
                }
                if let Some(r) = self.ready() {
                    let now = r.world.ticks;
                    for z in &r.world.bombs.zones {
                        out.push(format!("zona {} {:?} raio {}: {:.3} RAD/s no centro", z.dim, z.center, z.radius, f64::from(z.rate(&z.dim, [f64::from(z.center[0]), 0.0, f64::from(z.center[2])], now)) / 1000.0));
                    }
                }
                out.extend(self.blasts.log.iter().map(|l| format!("detonada: {l}")));
                Ok(out.join("\n"))
            }
            ["config"] => {
                let cfg = self.bomb_config(host);
                let local = self.ready().is_some_and(|r| r.world.bombs.cfg.is_some());
                Ok(format!("{} ({})", cfg.describe(), if local { "do mundo" } else { "global" }))
            }
            ["config", scope @ ("global" | "mundo"), rest @ ..] => {
                let global = *scope == "global";
                let mut cfg = if global {
                    self.bomb_config(host);
                    self.blasts.global.clone().unwrap_or_default()
                } else {
                    self.bomb_config(host)
                };
                match rest {
                    [] => return Ok(cfg.describe()),
                    ["limpar"] if !global => {
                        let r = self.ready().ok_or("nenhum mundo identificado")?;
                        r.world.bombs.cfg = None;
                        return Ok("o mundo volta a usar a configuração global".into());
                    }
                    [k, v] => cfg.set(k, v)?,
                    _ => return Err(usage.into()),
                }
                if global {
                    host.set(blast::config::KEY, &serde_json::to_vec(&cfg).map_err(|e| e.to_string())?)?;
                    self.blasts.global = Some(cfg.clone());
                } else {
                    let r = self.ready().ok_or("nenhum mundo identificado")?;
                    r.world.bombs.cfg = Some(cfg.clone());
                }
                Ok(cfg.describe())
            }
            ["teste", r, kind @ ..] => {
                let who = who.ok_or("só um jogador pode testar")?;
                if !who.creative {
                    return Err("o teste só funciona no modo criativo".into());
                }
                let radius: i32 = r.parse().map_err(|e| format!("{r}: {e}"))?;
                // Range first: nothing is spent or allocated for a bad one
                // (revisão do M2, P1-13).
                let max = if kind == ["forca"] { vanilla::MAX_POWER as i32 } else { crate::explosion::MAX_RADIUS };
                if !(1..=max).contains(&radius) {
                    return Err(format!("{radius} fora de 1..={max}"));
                }
                let blast = match kind {
                    [] => Blast::Nuke { radius, fallout: true },
                    ["n2"] => Blast::Nuke { radius, fallout: false },
                    ["balefire"] => Blast::Balefire { radius },
                    ["forca"] => Blast::Conventional { power: radius as f32 },
                    _ => return Err(usage.into()),
                };
                let pos = [who.pos[0].floor() as i32, who.pos[1].floor() as i32, who.pos[2].floor() as i32];
                self.ready().ok_or("nenhum mundo identificado")?;
                self.trigger(host, (who.dim.clone(), pos), blast, "teste");
                Ok(format!("detonada em {pos:?}"))
            }
            ["cancelar"] => {
                let ids: Vec<String> = self.blasts.nukes.active().map(|p| p.id).chain(self.blasts.nukes.lingering().map(|(id, _)| id.to_string())).collect();
                let engine = Engine { host: RefCell::new(&mut *host), players: &[] };
                for id in &ids {
                    self.blasts.nukes.cancel(id, &mut &engine);
                }
                self.blasts.pulses.clear();
                if let Some(r) = self.ready() {
                    r.world.bombs.queue.clear();
                }
                Ok(format!("{} explosão(ões) cancelada(s)", ids.len()))
            }
            _ => Err(usage.into()),
        }
    }
}
