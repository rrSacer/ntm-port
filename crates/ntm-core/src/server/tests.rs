//! The two engines together over one storage folder, against a fake world
//! that runs vanilla commands as the game does: the integrated server's
//! quiet-command consumer (64 lines a tick, batches cut anywhere), the
//! scoreboard and `execute store`, chunks that aren't loaded (a `setblock`
//! or `summon` there fails), `clear` over the whole inventory, and storage
//! that can refuse writes.

use super::*;
use crate::{
    client::{Client, ClientHost, TickInput, UiWidget},
    kv::MemKv,
    machine::{Data, actions},
};
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    rc::Rc,
};

const OW: &str = "minecraft:overworld";
const NETHER: &str = "minecraft:the_nether";

#[derive(Clone)]
struct World {
    blocks: HashMap<(String, Pos), String>,
    /// `data storage ntmlogic:world` as flat keys.
    storage: BTreeMap<String, String>,
    scores: BTreeMap<(String, String), i64>,
    objectives: BTreeSet<String>,
    /// The quiet commands waiting, oldest first.
    queue: VecDeque<String>,
    /// Lines the consumer runs a tick (`lan_quiet`: 64 a player).
    per_tick: usize,
    held: Option<(String, u32)>,
    inventory: BTreeMap<String, u32>,
    dropped: Vec<String>,
    effects: Vec<String>,
    /// Off: commands do nothing (a world without cheats).
    cheats: bool,
    /// The client's player's dimension: lines without `execute in` run there.
    player_dim: String,
    gamemode: String,
    /// On: the client's lines wait (a lagging or frozen game).
    paused: bool,
    /// Chunks (dimension, cx, cz) not loaded.
    unloaded: BTreeSet<(String, i32, i32)>,
    /// `run-quiet` lines this hook call: the client mod cuts past 256.
    hook_used: usize,
    /// Each hook call's lines, in order.
    hooks: Vec<Vec<String>>,
    /// Every line the consumer ran, in order.
    ran: Vec<String>,
    /// `set-blocks` of the server mod, applied after its hook returns.
    writes: Vec<(String, Pos, String)>,
    /// The item tags the content mod ships (`ntm:rad/bit_K`), read from
    /// `content/`: a line naming another tag doesn't parse.
    tags: BTreeMap<String, BTreeSet<String>>,
    /// Command storage ints other than `ntmlogic:world`: "<id> <path>".
    nbt: BTreeMap<String, i64>,
    /// The engine's machine block entities (patch `ntm-visual`): their
    /// slots and values, saved with the world (a test's autosave copies
    /// them with it). Only the blocks a test gives slots have them.
    engine: BTreeMap<(String, Pos), (container_bridge::Slots, Vec<i32>)>,
    /// Screen buttons pressed on engine screens, taken on the next tick.
    presses: Vec<(String, Pos, u32)>,
    /// Item ids the game doesn't have (an older content mod): a line
    /// naming one doesn't parse.
    unknown_items: BTreeSet<String>,
}

impl World {
    fn new() -> Self {
        let mut w = World {
            blocks: HashMap::new(),
            storage: BTreeMap::new(),
            scores: BTreeMap::new(),
            objectives: BTreeSet::new(),
            queue: VecDeque::new(),
            per_tick: 64,
            held: None,
            inventory: BTreeMap::new(),
            dropped: Vec::new(),
            effects: Vec::new(),
            cheats: true,
            player_dim: OW.into(),
            gamemode: "survival".into(),
            paused: false,
            unloaded: BTreeSet::new(),
            hook_used: 0,
            hooks: Vec::new(),
            ran: Vec::new(),
            writes: Vec::new(),
            tags: content_tags(),
            nbt: BTreeMap::new(),
            engine: BTreeMap::new(),
            presses: Vec::new(),
            unknown_items: BTreeSet::new(),
        };
        for x in -32..32 {
            for z in -32..32 {
                w.blocks.insert((OW.into(), [x, -64, z]), "minecraft:bedrock".into());
                for y in -63..-50 {
                    w.blocks.insert((OW.into(), [x, y, z]), "minecraft:deepslate".into());
                }
                w.blocks.insert((NETHER.into(), [x, 0, z]), "minecraft:bedrock".into());
                for y in 1..12 {
                    w.blocks.insert((NETHER.into(), [x, y, z]), "minecraft:netherrack".into());
                }
            }
        }
        w
    }
    /// The server mod's `set-blocks` of its last hook, made now.
    fn apply_writes(&mut self) {
        for (d, p, s) in std::mem::take(&mut self.writes) {
            if self.loaded(&d, p) {
                self.set(&d, p, &s);
            }
        }
    }
    fn loaded(&self, dim: &str, p: Pos) -> bool {
        !self.unloaded.contains(&(dim.to_string(), p[0] >> 4, p[2] >> 4))
    }
    fn get(&self, dim: &str, p: Pos) -> String {
        self.blocks
            .get(&(dim.to_string(), p))
            .cloned()
            .unwrap_or_else(|| "minecraft:air".into())
    }
    fn set(&mut self, dim: &str, p: Pos, s: &str) {
        self.blocks.insert((dim.into(), p), s.into());
    }
    fn score(&self, holder: &str, obj: &str) -> Option<i64> {
        self.scores.get(&(holder.to_string(), obj.to_string())).copied()
    }

    /// The consumer: `per_tick` queued lines (nothing while paused).
    fn run_commands(&mut self) {
        if self.paused {
            return;
        }
        for _ in 0..self.per_tick {
            let Some(line) = self.queue.pop_front() else { break };
            self.ran.push(line.clone());
            if self.cheats && self.parses(&line) {
                let dim = self.player_dim.clone();
                self.exec(&dim, &line);
            }
        }
    }

    fn count(&self, pred: &str) -> u32 {
        let matches = |i: &str| match pred.strip_prefix('#') {
            Some(tag) => {
                let id = i.split('[').next().unwrap();
                self.tags[tag].contains(id)
            }
            None => item_matches(i, pred),
        };
        let held = self
            .held
            .as_ref()
            .filter(|(i, _)| matches(i))
            .map_or(0, |(_, n)| *n);
        held + self
            .inventory
            .iter()
            .filter(|(i, _)| matches(i))
            .map(|(_, n)| *n)
            .sum::<u32>()
    }

    /// Whether the game parses `line`: an unknown item tag is an error,
    /// and then nothing of the line runs (its `store` neither).
    fn parses(&self, line: &str) -> bool {
        line.split(' ')
            .filter_map(|w| w.strip_prefix("#ntm:"))
            .all(|t| self.tags.contains_key(&format!("ntm:{t}")))
            && !line.split(' ').any(|w| self.unknown_items.contains(w))
    }

    /// Removes up to `n` matching items, the hand first, as `clear`.
    fn take(&mut self, pred: &str, mut n: u32) -> u32 {
        let mut taken = 0;
        if let Some((i, c)) = &mut self.held
            && item_matches(i, pred)
        {
            let t = n.min(*c);
            *c -= t;
            n -= t;
            taken += t;
            if *c == 0 {
                self.held = None;
            }
        }
        for (i, c) in self.inventory.iter_mut() {
            if n == 0 {
                break;
            }
            if item_matches(i, pred) {
                let t = n.min(*c);
                *c -= t;
                n -= t;
                taken += t;
            }
        }
        self.inventory.retain(|_, c| *c > 0);
        taken
    }

    /// One command: (success, result).
    fn exec(&mut self, dim: &str, line: &str) -> (bool, i64) {
        let w: Vec<&str> = line.split(' ').collect();
        let rest = |i: usize| w[i..].join(" ");
        let pos = |i: usize| -> Pos {
            [w[i].parse().unwrap(), w[i + 1].parse().unwrap(), w[i + 2].parse().unwrap()]
        };
        match w[0] {
            "execute" => {
                let mut dim = dim.to_string();
                let mut stores: Vec<(bool, String, String)> = Vec::new();
                // `store result storage <id> <path> int <scale>`.
                let mut nbt_stores: Vec<(String, f64)> = Vec::new();
                let mut i = 1;
                while i < w.len() {
                    match w[i] {
                        "in" => {
                            dim = w[i + 1].into();
                            i += 2;
                        }
                        "store" if w[i + 2] == "storage" => {
                            assert_eq!((w[i + 1], w[i + 5]), ("result", "int"));
                            nbt_stores.push((format!("{} {}", w[i + 3], w[i + 4]), w[i + 6].parse().unwrap()));
                            i += 7;
                        }
                        "store" => {
                            stores.push((w[i + 1] == "success", w[i + 3].into(), w[i + 4].into()));
                            i += 5;
                        }
                        iu @ ("if" | "unless") => {
                            let (cond, used) = match w[i + 1] {
                                "score" => {
                                    assert_eq!(w[i + 4], "matches");
                                    let v = self.score(w[i + 2], w[i + 3]);
                                    (v.is_some_and(|v| range(w[i + 5], v)), 6)
                                }
                                "block" => {
                                    let p = pos(i + 2);
                                    let want = w[i + 5];
                                    let here = self.get(&dim, p);
                                    let is = self.loaded(&dim, p)
                                        && if want.contains('[') {
                                            here == want
                                        } else {
                                            blockstate::id_of(&here) == want
                                        };
                                    (is, 6)
                                }
                                "items" => {
                                    assert_eq!(
                                        (w[i + 2], w[i + 3], w[i + 4]),
                                        ("entity", "@s", "weapon.mainhand")
                                    );
                                    let pred = w[i + 5];
                                    let ok = self
                                        .held
                                        .as_ref()
                                        .is_some_and(|(s, n)| *n > 0 && item_matches(s, pred));
                                    (ok, 6)
                                }
                                "data" => {
                                    let path = w[i + 4];
                                    let ok = if let Some(nbt) = path.strip_prefix('{') {
                                        let (k, v) = nbt.trim_end_matches('}').split_once(':').unwrap();
                                        self.storage.get(k).map(String::as_str) == Some(v)
                                    } else {
                                        self.storage.contains_key(path)
                                    };
                                    (ok, 5)
                                }
                                "entity" => {
                                    let mode = w[i + 2]
                                        .strip_prefix("@s[gamemode=")
                                        .and_then(|m| m.strip_suffix(']'))
                                        .expect("entity selector");
                                    (self.gamemode == mode, 3)
                                }
                                other => panic!("execute {iu} {other}: {line}"),
                            };
                            if cond != (iu == "if") {
                                return (false, 0);
                            }
                            i += used;
                        }
                        "run" => {
                            let (ok, res) = self.exec(&dim, &rest(i + 1));
                            for (success, h, o) in stores {
                                let v = if success { i64::from(ok) } else { i64::from(res as i32) };
                                self.scores.insert((h, o), v);
                            }
                            for (key, scale) in nbt_stores {
                                // Java's (int) of a double: it saturates.
                                let v = (f64::from(res as i32) * scale) as i32;
                                self.nbt.insert(key, i64::from(v));
                            }
                            return (ok, res);
                        }
                        other => panic!("execute subcommand {other}: {line}"),
                    }
                }
                (true, 1)
            }
            "scoreboard" => match (w[1], w[2]) {
                ("objectives", "add") => (self.objectives.insert(w[3].into()), 0),
                ("players", "set") => {
                    let v: i64 = w[5].parse().unwrap();
                    self.scores.insert((w[3].into(), w[4].into()), v);
                    (true, v)
                }
                ("players", "reset") => {
                    self.scores.remove(&(w[3].to_string(), w[4].to_string()));
                    (true, 0)
                }
                ("players", "operation") => {
                    let b = self.score(w[6], w[7]).unwrap_or(0);
                    let a = self.scores.entry((w[3].into(), w[4].into())).or_default();
                    // The game's scores are 32-bit and wrap.
                    let (x, y) = (*a as i32, b as i32);
                    *a = i64::from(match w[5] {
                        "=" => y,
                        "+=" => x.wrapping_add(y),
                        "*=" => x.wrapping_mul(y),
                        "%=" => x.rem_euclid(y),
                        "/=" => x.div_euclid(y),
                        op => panic!("score operation {op}"),
                    });
                    (true, *a)
                }
                other => panic!("scoreboard {other:?}"),
            },
            "setblock" => {
                let p = pos(1);
                if !self.loaded(dim, p) {
                    return (false, 0); // argument.pos.unloaded
                }
                self.set(dim, p, w[4]);
                (true, 1)
            }
            "clear" => {
                assert_eq!(w[1], "@s");
                let n: u32 = w[3].parse().unwrap();
                let got = if n == 0 { self.count(w[2]) } else { self.take(w[2], n) };
                (got > 0, i64::from(got))
            }
            "give" => {
                let n: u32 = w[3].parse().unwrap();
                *self.inventory.entry(w[2].into()).or_default() += n;
                (true, i64::from(n))
            }
            "summon" => {
                let p = [
                    w[2].parse::<f64>().unwrap().floor() as i32,
                    w[3].parse::<f64>().unwrap().floor() as i32,
                    w[4].parse::<f64>().unwrap().floor() as i32,
                ];
                if !self.loaded(dim, p) {
                    return (false, 0);
                }
                self.dropped.push(format!("{dim} {line}"));
                (true, 1)
            }
            "#" => (false, 0), // a line the game cut: it doesn't parse
            "effect" | "damage" | "playsound" | "particle" => {
                self.effects.push(line.into());
                (true, 1)
            }
            "data" if w[1] == "get" => {
                assert_eq!(w[2], "storage");
                match self.nbt.get(&format!("{} {}", w[3], w[4])) {
                    Some(v) => (true, *v),
                    None => (false, 0), // a missing path is an error
                }
            }
            "data" if w[1] == "remove" => {
                assert_eq!(w[2], "storage");
                let prefix = format!("{} {}", w[3], w[4]);
                let before = self.nbt.len();
                self.nbt.retain(|k, _| k != &prefix && !k.starts_with(&format!("{prefix}.")));
                (self.nbt.len() < before, 1)
            }
            "data" => {
                assert_eq!(w[1], "merge");
                let nbt = rest(4);
                for kv in nbt.trim_matches(|c| c == '{' || c == '}').split(',') {
                    let (k, v) = kv.split_once(':').unwrap();
                    self.storage.insert(k.into(), v.into());
                }
                (true, 1)
            }
            other => panic!("command {other}: {line}"),
        }
    }
}

/// `matches` ranges: `5`, `1..`, `..-1`, `..2147483647`, `0..3`.
fn range(r: &str, v: i64) -> bool {
    match r.split_once("..") {
        Some((a, b)) => {
            (a.is_empty() || v >= a.parse::<i64>().unwrap())
                && (b.is_empty() || v <= b.parse::<i64>().unwrap())
        }
        None => v == r.parse::<i64>().unwrap(),
    }
}

/// `stack` (`id` or `id[minecraft:custom_data={k:v}]`, as `give` wrote it)
/// against a predicate: `id`, `id[!minecraft:custom_data]` or
/// `id[minecraft:custom_data~{k:v}]`.
/// The `ntm` item tags of the content mod, as the game loads them.
fn content_tags() -> BTreeMap<String, BTreeSet<String>> {
    static TAGS: std::sync::OnceLock<BTreeMap<String, BTreeSet<String>>> = std::sync::OnceLock::new();
    TAGS.get_or_init(read_content_tags).clone()
}

fn read_content_tags() -> BTreeMap<String, BTreeSet<String>> {
    let dir = format!("{}/../../content/data/ntm/tags/item/rad", env!("CARGO_MANIFEST_DIR"));
    let mut out = BTreeMap::new();
    for e in std::fs::read_dir(&dir).unwrap() {
        let name = e.unwrap().file_name().into_string().unwrap();
        let Some(stem) = name.strip_suffix(".json") else { continue };
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(format!("{dir}/{name}")).unwrap()).unwrap();
        let ids = v["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect();
        out.insert(format!("ntm:rad/{stem}"), ids);
    }
    out
}

fn item_matches(stack: &str, pred: &str) -> bool {
    let (sid, sdata) = match stack.split_once("[minecraft:custom_data=") {
        Some((id, d)) => (id, Some(d.trim_end_matches(']'))),
        None => (stack, None),
    };
    match pred.split_once('[') {
        None => sid == pred,
        Some((pid, rest)) => {
            let rest = rest.trim_end_matches(']');
            if rest == "!minecraft:custom_data" {
                sid == pid && sdata.is_none()
            } else if let Some(want) = rest.strip_prefix("minecraft:custom_data~") {
                sid == pid && sdata == Some(want)
            } else {
                panic!("predicate {pred}")
            }
        }
    }
}

struct SHost {
    kv: Rc<RefCell<MemKv>>,
    world: Rc<RefCell<World>>,
    told: Vec<String>,
    /// Blocks read so far: what the game's fuel mostly pays for.
    reads: u64,
}
impl Kv for SHost {
    fn get(&mut self, key: &str) -> Option<Vec<u8>> {
        self.kv.get(key)
    }
    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), String> {
        self.kv.set(key, value)
    }
    fn delete(&mut self, key: &str) {
        self.kv.delete(key)
    }
}
impl ServerHost for SHost {
    fn get_block(&mut self, dim: &str, p: Pos) -> Option<String> {
        self.reads += 1;
        let w = self.world.borrow();
        w.loaded(dim, p).then(|| w.get(dim, p))
    }
    fn set_blocks(&mut self, dim: &str, blocks: &[(Pos, String)]) -> Result<(), String> {
        // As the game: made after the hook returns (`apply_writes`).
        let mut w = self.world.borrow_mut();
        w.writes
            .extend(blocks.iter().map(|(p, s)| (dim.to_string(), *p, s.clone())));
        Ok(())
    }
    fn tell(&mut self, _player: &str, text: &str) {
        self.told.push(text.into());
    }
    fn log(&mut self, text: &str) {
        eprintln!("[server] {text}");
    }
    fn machine_slots(&mut self, dim: &str, p: Pos) -> Option<container_bridge::Slots> {
        self.world.borrow().engine.get(&(dim.to_string(), p)).map(|e| e.0.clone())
    }
    fn set_machine_slot(&mut self, dim: &str, p: Pos, slot: u32, stack: Option<container_bridge::Stack>) -> bool {
        match self.world.borrow_mut().engine.get_mut(&(dim.to_string(), p)) {
            Some(e) if (slot as usize) < e.0.len() => {
                e.0[slot as usize] = stack;
                true
            }
            _ => false,
        }
    }
    fn machine_buttons(&mut self) -> Vec<(String, String, Pos, u32)> {
        std::mem::take(&mut self.world.borrow_mut().presses).into_iter().map(|(d, p, b)| ("p1".to_string(), d, p, b)).collect()
    }
    fn machine_data(&mut self, dim: &str, p: Pos) -> Option<Vec<i32>> {
        self.world.borrow().engine.get(&(dim.to_string(), p)).map(|e| e.1.clone())
    }
    fn set_machine_data(&mut self, dim: &str, p: Pos, values: &[i32]) -> bool {
        match self.world.borrow_mut().engine.get_mut(&(dim.to_string(), p)) {
            Some(e) => {
                e.1 = values.to_vec();
                true
            }
            None => false,
        }
    }
}

#[derive(Default)]
struct Screen {
    title: String,
    widgets: Vec<UiWidget>,
}
struct CHost {
    kv: Rc<RefCell<MemKv>>,
    world: Rc<RefCell<World>>,
    screen: Option<Screen>,
    chat: Vec<String>,
}
impl Kv for CHost {
    fn get(&mut self, key: &str) -> Option<Vec<u8>> {
        self.kv.get(key)
    }
    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), String> {
        self.kv.set(key, value)
    }
    fn delete(&mut self, key: &str) {
        self.kv.delete(key)
    }
}
impl ClientHost for CHost {
    fn run_quiet(&mut self, lines: &[String]) {
        // As the client mod's host: 256 lines a hook call, the rest cut.
        let mut w = self.world.borrow_mut();
        let room = 256 - w.hook_used.min(256);
        // And each line cut at 256 characters (`quiet::MAX_LINE`): a cut
        // line fails to parse.
        let taken: Vec<String> = lines
            .iter()
            .take(room)
            .map(|l| {
                if l.len() > crate::token::MAX_LINE {
                    "# cut".to_string()
                } else {
                    l.clone()
                }
            })
            .collect();
        w.hook_used += taken.len();
        if let Some(h) = w.hooks.last_mut() {
            h.extend(taken.iter().cloned());
        }
        w.queue.extend(taken);
    }
    fn open_screen(&mut self, title: &str, widgets: Vec<UiWidget>) {
        self.screen = Some(Screen {
            title: title.into(),
            widgets,
        });
    }
    fn close_screen(&mut self) {
        self.screen = None;
    }
    fn chat(&mut self, text: &str) {
        self.chat.push(text.into());
    }
    fn log(&mut self, text: &str) {
        eprintln!("[client] {text}");
    }
}

/// One single-player game: a world, the shared folder, both mods.
struct Game {
    tick: u64,
    world: Rc<RefCell<World>>,
    kv: Rc<RefCell<MemKv>>,
    server: Server,
    client: Client,
    sh: SHost,
    ch: CHost,
    game_time: u64,
    use_down: bool,
    target: Option<(Pos, String)>,
    /// Who the server sees online; the first is the client's by default.
    players: Vec<PlayerInfo>,
}

fn player() -> PlayerInfo {
    PlayerInfo {
        id: "p1".into(),
        name: "Steve".into(),
        dim: OW.into(),
        pos: [0.5, 70.0, 0.5],
        creative: true,
    }
}

impl Game {
    fn new(world: Rc<RefCell<World>>, kv: Rc<RefCell<MemKv>>, seed: u64) -> Self {
        let mut sh = SHost {
            kv: kv.clone(),
            world: world.clone(),
            told: vec![],
            reads: 0,
        };
        let server = Server::new(&mut sh, 0);
        let mut client = Client::new();
        client.entropy.feed(seed);
        Self {
            tick: 0,
            world: world.clone(),
            kv: kv.clone(),
            server,
            client,
            sh,
            ch: CHost {
                kv,
                world,
                screen: None,
                chat: vec![],
            },
            game_time: seed * 1000,
            use_down: false,
            target: None,
            players: vec![player()],
        }
    }
    /// One tick: the client mod, the consumer, the server mod.
    fn step(&mut self) {
        self.client_step();
        self.world.borrow_mut().run_commands();
        self.server.tick(&mut self.sh, self.tick, self.players.clone());
        self.world.borrow_mut().apply_writes();
    }
    fn client_step(&mut self) {
        self.tick += 1;
        self.game_time += 1;
        self.client_hook();
    }
    /// The client mod's tick hook alone, at `self.game_time` as the
    /// client's level shows it (lag tests drive both clocks themselves).
    fn client_hook(&mut self) {
        self.client.frame(0.016, true);
        {
            let mut w = self.world.borrow_mut();
            w.hook_used = 0;
            w.hooks.push(Vec::new());
        }
        let dim = self.world.borrow().player_dim.clone();
        let pos = self
            .players
            .iter()
            .find(|p| p.dim == dim)
            .map_or([0.5, 70.0, 0.5], |p| p.pos);
        self.client.tick(
            &mut self.ch,
            TickInput {
                game_time: self.game_time,
                dim,
                pos,
                use_down: self.use_down,
                use_clicks: 0,
                open_key: false,
                target: self.target.clone(),
            },
        );
    }
    /// The server alone ticks (the client mod got no tick).
    fn server_step(&mut self) {
        self.tick += 1;
        self.world.borrow_mut().run_commands();
        self.server.tick(&mut self.sh, self.tick, self.players.clone());
        self.world.borrow_mut().apply_writes();
    }
    fn run(&mut self, n: usize) {
        for _ in 0..n {
            self.step();
        }
    }
    fn place(&mut self, p: Pos, state: &str) {
        self.world.borrow_mut().set(OW, p, state);
        self.server.block_placed(OW, p, state);
    }
    fn open(&mut self, p: Pos) {
        let id = BlockState::parse(&self.world.borrow().get(OW, p)).unwrap().id;
        self.target = Some((p, id));
        self.use_down = true;
        self.run(1);
        self.use_down = false;
        self.run(2);
    }
    fn held_now(&self) -> Option<Held> {
        // `input.held` gives the id alone, never the components.
        self.world.borrow().held.clone().map(|(item, count)| Held {
            item: blockstate::id_of(&item).to_string(),
            count,
        })
    }
    fn click(&mut self, action: u32) {
        let held = self.held_now();
        self.client.select(&mut self.ch, action, held);
        self.run(2);
    }
    fn screen_text(&self) -> String {
        let s = self.ch.screen.as_ref().expect("a screen");
        let mut t = s.title.clone();
        for w in &s.widgets {
            if let UiWidget::Text { text, .. } | UiWidget::Button { label: text, .. } = w {
                t.push('\n');
                t.push_str(text);
            }
        }
        t
    }
    fn ready(&mut self) -> &mut Ready {
        let Session::Ready(r) = &mut self.server.session else {
            panic!("not ready")
        };
        r
    }
    fn machine(&mut self, p: Pos) -> Machine {
        self.ready().world.machines[&(OW.to_string(), p)].clone()
    }
    fn held(&self) -> Option<(String, u32)> {
        self.world.borrow().held.clone()
    }
    fn hold(&self, item: &str, n: u32) {
        self.world.borrow_mut().held = Some((item.into(), n));
    }
    fn inv(&self, item: &str) -> u32 {
        self.world.borrow().inventory.get(item).copied().unwrap_or(0)
    }
    /// A burner at the origin, its screen open.
    fn burner(&mut self) -> Pos {
        let at = [0, 70, 0];
        self.place(at, "ntm:machine_wood_burner[facing=north,honey_level=0]");
        self.run(2);
        self.open(at);
        at
    }
    /// Items the burner took: waiting plus the one burning.
    fn fuel_taken(&mut self, at: Pos) -> u32 {
        let Data::WoodBurner(b) = self.machine(at).data else { panic!() };
        b.fuel.map_or(0, |f| f.1) + u32::from(b.max_burn > 0)
    }
    /// A furnace at `at` holding `n` iron ingots of output, its screen open.
    fn furnace_with_output(&mut self, at: Pos, n: u32) {
        if !self.ready().world.machines.contains_key(&(OW.to_string(), at)) {
            self.place(at, "ntm:machine_electric_furnace_off[facing=north,honey_level=0]");
            self.run(2);
        }
        let r = self.ready();
        let Data::ElectricFurnace(f) = &mut r.world.machines.get_mut(&(OW.to_string(), at)).unwrap().data else {
            panic!()
        };
        f.output = Some(("minecraft:iron_ingot".into(), n));
        r.urgent = true;
        self.run(1);
        self.open(at);
    }
    fn saved(&mut self) -> WorldState {
        let wid = self.server.wid().unwrap().to_string();
        let (_, d) = persist::load(&mut *self.kv.borrow_mut(), &wid).expect("a save");
        WorldState::from_save(d)
    }
    fn output(&mut self, at: Pos) -> Option<(String, u32)> {
        let Data::ElectricFurnace(f) = self.machine(at).data else { panic!() };
        f.output
    }
    fn scores_left(&self) -> usize {
        self.world
            .borrow()
            .scores
            .keys()
            .filter(|(h, o)| o == tx::OBJECTIVE && h.starts_with("#t"))
            .count()
    }
}

fn game() -> Game {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w, kv, 1);
    g.run(10);
    g
}

fn column_untouched(w: &World) -> bool {
    (-8..8).all(|x| {
        (-8..8).all(|z| {
            w.get(OW, [x, -64, z]) == "minecraft:bedrock"
                && (-63..-54).all(|y| w.get(OW, [x, y, z]) == "minecraft:deepslate")
        })
    })
}

/// No token left anywhere.
fn no_tokens(w: &World) -> bool {
    !w.blocks.values().any(|s| s.starts_with(crate::registry::TOKEN))
}

/// No token left but at the cells of probes and transactions in flight (a
/// radiation sample may be filling its column at any moment).
fn no_orphan_tokens(g: &Game) -> bool {
    let mut held: BTreeSet<(String, Pos)> = g
        .server
        .probes()
        .iter()
        .flat_map(|p| p.cells.iter().map(|c| (p.dim.clone(), c.pos)).collect::<Vec<_>>())
        .collect();
    for t in g.server.txs() {
        held.extend(t.cells.iter().map(|c| (c.dim.clone(), c.pos)));
    }
    !g.world
        .borrow()
        .blocks
        .iter()
        .any(|(k, s)| s.starts_with(crate::registry::TOKEN) && !held.contains(k))
}

// --------------------------------------------------------------- basics

#[test]
fn world_identity_is_found_kept_and_distinct() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w1 = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w1.clone(), kv.clone(), 1);
    g.run(10);
    let wid = g.server.wid().expect("identified").to_string();
    assert_eq!(wid.len(), 10);
    assert_eq!(g.client.wid.as_deref(), Some(wid.as_str()));
    assert!(column_untouched(&w1.borrow()), "the column was put back");
    let mut g = Game::new(w1.clone(), kv.clone(), 2);
    g.run(10);
    assert_eq!(g.server.wid(), Some(wid.as_str()));
    let w2 = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w2, kv.clone(), 3);
    g.run(10);
    assert_ne!(g.server.wid().unwrap(), wid);
}

#[test]
fn no_cheats_no_identity_and_a_hint() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    w.borrow_mut().cheats = false;
    let mut g = Game::new(w.clone(), kv, 1);
    g.run(400);
    assert_eq!(g.server.wid(), None);
    assert!(g.sh.told.iter().any(|t| t.contains("comandos")), "{:?}", g.sh.told);
    assert!(column_untouched(&w.borrow()));
}

#[test]
fn battery_screen_button_and_persistence() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w.clone(), kv.clone(), 1);
    g.run(10);
    let bat = [3, 70, 0];
    g.place(bat, "ntm:machine_battery[facing=east,honey_level=0]");
    g.run(2);
    g.open(bat);
    let text = g.screen_text();
    assert!(text.starts_with("Bateria"), "{text}");
    assert!(text.contains("Energia [----------] 0/1M HE"), "{text}");
    g.click(actions::BATTERY_MODE);
    assert!(g.screen_text().contains("Saída"), "{}", g.screen_text());
    for _ in 0..5 {
        g.click(actions::BATTERY_DEBUG_FILL);
    }
    assert!(g.screen_text().contains("[#####-----] 500k/1M"), "{}", g.screen_text());
    g.run(3);
    assert_eq!(w.borrow().get(OW, bat), "ntm:machine_battery[facing=east,honey_level=2]");
    drop(g);
    let mut g = Game::new(w.clone(), kv.clone(), 9);
    g.run(10);
    let Data::Battery(b) = g.machine(bat).data else { panic!() };
    assert_eq!((b.power, b.mode), (500_000, crate::machine::BatteryMode::Output));
}

#[test]
fn burner_takes_fuel_from_the_hand_and_charges_a_battery_through_a_cable() {
    let mut g = game();
    // The burner faces north: its port is its back, to the south (+z).
    let (burner, cable, bat) = ([0, 70, 0], [0, 70, 1], [0, 70, 2]);
    g.place(burner, "ntm:machine_wood_burner[facing=north,honey_level=0]");
    g.place(cable, "ntm:red_cable");
    g.place(bat, "ntm:machine_battery[facing=north,honey_level=0]");
    g.run(2);
    g.hold("minecraft:coal", 3);
    g.open(burner);
    assert!(g.screen_text().contains("Queimador"));
    g.click(actions::BURNER_FUEL);
    g.run(5);
    assert_eq!(g.held(), Some(("minecraft:coal".into(), 2)), "one coal taken");
    assert_eq!(g.fuel_taken(burner), 1);
    assert_eq!(
        g.world.borrow().get(OW, burner).split('[').next(),
        Some("ntm:machine_wood_burner"),
        "the machine block is never borrowed"
    );
    g.run(100);
    let Data::Battery(b) = g.machine(bat).data else { panic!() };
    assert!(b.power > 5_000, "battery got {}", b.power);
    assert!(no_orphan_tokens(&g));
    assert!(g.server.txs().is_empty());
    assert_eq!(g.scores_left(), 1, "kept while a crash could roll it back");
    g.run(RETAIN_TICKS as usize + 100);
    assert_eq!(g.scores_left(), 0, "then reset");
}

#[test]
fn a_take_whose_item_left_the_hand_fails_cleanly() {
    let mut g = game();
    let at = g.burner();
    g.world.borrow_mut().held = None;
    let stale = Some(Held {
        item: "minecraft:coal".into(),
        count: 1,
    });
    g.client.select(&mut g.ch, actions::BURNER_FUEL, stale);
    g.run(10);
    assert_eq!(g.fuel_taken(at), 0);
    assert!(g.ch.chat.iter().any(|c| c.contains("não está mais")), "{:?}", g.ch.chat);
    assert!(g.server.txs().is_empty());
}

#[test]
fn breaking_a_machine_drops_its_contents_once() {
    let mut g = game();
    let at = g.burner();
    {
        let r = g.ready();
        let Data::WoodBurner(b) = &mut r.world.machines.get_mut(&(OW.to_string(), at)).unwrap().data else {
            panic!()
        };
        b.fuel = Some(("minecraft:oak_log".into(), 4));
        b.on = false;
    }
    g.world.borrow_mut().set(OW, at, "minecraft:air");
    g.server.block_broken(OW, at, "ntm:machine_wood_burner[facing=north,honey_level=0]");
    g.run(400);
    let dropped = g.world.borrow().dropped.clone();
    assert_eq!(dropped.len(), 1, "{dropped:?}");
    assert!(dropped[0].contains("oak_log") && dropped[0].contains("count:4"));
    assert!(g.ch.screen.is_none(), "the screen closed with the machine");
}

#[test]
fn a_vanished_machine_is_dropped_only_after_a_second_look() {
    let mut g = game();
    let bat = [3, 70, 0];
    g.place(bat, "ntm:machine_battery[facing=east,honey_level=0]");
    g.run(2);
    g.world.borrow_mut().set(OW, bat, "minecraft:stone");
    g.run(LOAD_CHECK as usize + 1);
    assert_eq!(g.ready().world.machines.len(), 1, "kept after one look");
    g.run(2 * LOAD_CHECK as usize);
    assert!(g.ready().world.machines.is_empty());
}

#[test]
fn switching_worlds_keeps_each_worlds_machines() {
    let mut g = game();
    g.place([3, 70, 0], "ntm:machine_battery[facing=east,honey_level=0]");
    g.run(3);
    let wid1 = g.server.wid().unwrap().to_string();
    let w2 = Rc::new(RefCell::new(World::new()));
    g.world = w2.clone();
    g.sh.world = w2.clone();
    g.ch.world = w2.clone();
    // As the game: the server mod starts anew with the world opened.
    g.server = Server::new(&mut g.sh, 0);
    g.game_time = 777_000;
    g.run(15);
    let wid2 = g.server.wid().unwrap().to_string();
    assert_ne!(wid1, wid2);
    assert!(g.ready().world.machines.is_empty());
    let (_, saved) = persist::load(&mut *g.kv.borrow_mut(), &wid1).expect("world 1 saved");
    assert_eq!(saved.machines.len(), 1);
}

#[test]
fn electric_press_with_a_stamp_from_the_hand_and_its_wear_round_trip() {
    use crate::press::actions as pa;
    let mut g = game();
    let (press, bat) = ([0, 70, 0], [1, 70, 0]);
    g.place(press, "ntm:machine_epress[facing=north,honey_level=0]");
    g.place(bat, "ntm:machine_battery[facing=north,honey_level=0]");
    g.run(2);
    g.open(bat);
    for _ in 0..10 {
        g.click(actions::BATTERY_DEBUG_FILL);
    }
    g.client.closed(&mut g.ch);
    g.open(press);
    g.hold("ntm:stamp_iron_plate", 1);
    g.click(pa::INSERT);
    g.run(5);
    assert_eq!(g.held(), None, "the stamp left the hand");
    for _ in 0..3 {
        g.hold("ntm:ingot_lead", 1);
        g.click(pa::INSERT);
        g.run(5);
    }
    g.run(120);
    let Data::ElectricPress(p) = g.machine(press).data else { panic!() };
    assert_eq!(p.core.output, Some(("ntm:plate_lead".into(), 3)), "{p:?}");
    assert_eq!(p.core.stamp.as_ref().unwrap().used, 3);
    g.click(pa::TAKE_OUTPUT);
    g.click(pa::TAKE_STAMP);
    g.run(5);
    let tagged = "ntm:stamp_iron_plate[minecraft:custom_data={ntm_wear:1}]";
    assert_eq!(g.inv("ntm:plate_lead"), 3);
    assert_eq!(g.inv(tagged), 1);
    // Back in: the wear comes back rounded up, never repaired.
    g.world.borrow_mut().inventory.remove(tagged);
    g.hold(tagged, 1);
    g.click(pa::INSERT);
    g.run(5);
    assert_eq!(g.held(), None);
    let Data::ElectricPress(p) = g.machine(press).data else { panic!() };
    assert_eq!(p.core.stamp.as_ref().unwrap().used, 11);
    // Broken with its stamp: the stamp drops with its wear.
    g.world.borrow_mut().set(OW, press, "minecraft:air");
    g.server.block_broken(OW, press, "ntm:machine_epress[facing=north,honey_level=0]");
    g.run(5);
    let dropped = g.world.borrow().dropped.join("\n");
    assert!(
        dropped.contains("id:\"ntm:stamp_iron_plate\",count:1,components:{\"minecraft:custom_data\":{ntm_wear:1}}"),
        "{dropped}"
    );
}

#[test]
fn burner_press_runs_on_coal_from_the_hand() {
    use crate::press::actions as pa;
    let mut g = game();
    let press = [0, 70, 0];
    g.place(press, "ntm:machine_press[facing=north,honey_level=0]");
    g.run(2);
    g.open(press);
    for item in ["ntm:stamp_iron_wire", "minecraft:coal", "ntm:ingot_red_copper"] {
        g.hold(item, 1);
        g.click(pa::INSERT);
        g.run(5);
        assert_eq!(g.held(), None, "{item} taken");
    }
    g.run(200);
    let Data::BurnerPress(p) = g.machine(press).data else { panic!() };
    assert_eq!(p.core.output, Some(("ntm:wire_fine__red_copper".into(), 8)), "{p:?}");
}

#[test]
fn single_player_without_its_client_mod_pauses_instead_of_mixing_worlds() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w.clone(), kv.clone(), 1);
    g.run(10);
    assert!(g.server.wid().is_some());
    let mut sh = SHost { kv: kv.clone(), world: w.clone(), told: vec![], reads: 0 };
    let mut server = Server::new(&mut sh, 0);
    for t in 1..=(NO_CLIENT_TICKS + 20) {
        server.tick(&mut sh, t, vec![player()]);
        w.borrow_mut().apply_writes();
    }
    assert_eq!(server.wid(), None);
    assert_eq!(sh.told.iter().filter(|t| t.contains("mod cliente")).count(), 1);
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let mut sh = SHost { kv: kv.clone(), world: w.clone(), told: vec![], reads: 0 };
    let mut server = Server::new(&mut sh, 0);
    for t in 1..=(NO_CLIENT_TICKS + 1) {
        server.tick(&mut sh, t, vec![player()]);
    }
    assert_eq!(server.wid(), Some(DEDICATED));
}

#[test]
fn energy_follows_the_ntm_ports() {
    let mut g = game();
    let (fw, bat, fe, burner) = ([-1, 70, 0], [0, 70, 0], [1, 70, 0], [0, 70, -1]);
    g.place(fw, "ntm:machine_electric_furnace_off[facing=north,honey_level=0]");
    g.place(bat, "ntm:machine_battery[facing=north,honey_level=0]");
    g.place(fe, "ntm:machine_electric_furnace_off[facing=north,honey_level=0]");
    g.place(burner, "ntm:machine_wood_burner[facing=south,honey_level=0]");
    g.run(2);
    {
        let r = g.ready();
        let nets: Vec<Vec<Pos>> = r.topo.values().flat_map(|t| t.networks.clone()).collect();
        assert_eq!(nets, vec![vec![fw, bat], vec![bat, fe]], "the burner's front joins nothing");
        let Data::Battery(b) = &mut r.world.machines.get_mut(&(OW.to_string(), bat)).unwrap().data else {
            panic!()
        };
        b.power = 100_000;
    }
    g.run(20);
    let Data::ElectricFurnace(w) = g.machine(fw).data else { panic!() };
    let Data::ElectricFurnace(e) = g.machine(fe).data else { panic!() };
    assert!(w.power > 0 && e.power > 0);
}

#[test]
fn forget_deletes_another_worlds_save_only() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w1 = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w1, kv.clone(), 1);
    g.run(10);
    g.place([3, 70, 0], "ntm:machine_battery[facing=east,honey_level=0]");
    g.run(3);
    let old = g.server.wid().unwrap().to_string();
    let w2 = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w2, kv.clone(), 2);
    g.run(10);
    let here = g.server.wid().unwrap().to_string();
    assert!(g.server.forget(&mut g.sh, &here).is_err());
    assert!(g.server.forget(&mut g.sh, "x y").is_err());
    g.server.forget(&mut g.sh, &old).unwrap();
    assert!(persist::load(&mut *kv.borrow_mut(), &old).is_none());
    assert!(g.server.forget(&mut g.sh, &old).is_err());
}

// --------------------------------------------- first review (still hold)

/// First review, finding 1: the client runs the take long after it was
/// asked.
#[test]
fn review_1_a_late_take_still_credits_the_machine() {
    let mut g = game();
    let at = g.burner();
    g.hold("minecraft:coal", 3);
    g.world.borrow_mut().paused = true;
    g.click(actions::BURNER_FUEL);
    g.run(3000);
    assert_eq!(g.held(), Some(("minecraft:coal".into(), 3)));
    g.world.borrow_mut().paused = false;
    g.run(200);
    assert_eq!(g.held(), Some(("minecraft:coal".into(), 2)));
    assert_eq!(g.fuel_taken(at), 1);
    assert!(no_orphan_tokens(&g));
}

/// First review, finding 1: the client's session ends between running
/// the lines and the server reading the answer.
#[test]
fn review_1_a_session_change_mid_transaction_keeps_the_credit() {
    let mut g = game();
    let at = g.burner();
    g.hold("minecraft:coal", 2);
    g.click(actions::BURNER_FUEL);
    g.world.borrow_mut().paused = true;
    g.run(1);
    g.client.left = true;
    g.world.borrow_mut().paused = false;
    g.run(300);
    assert_eq!(g.held(), Some(("minecraft:coal".into(), 1)));
    assert_eq!(g.fuel_taken(at), 1);
}

/// First review, finding 4: the column goes under the client's player,
/// not a guest's; nothing is left behind.
#[test]
fn review_4_the_column_is_the_clients() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w.clone(), kv, 1);
    let mut guest = player();
    guest.id = "guest".into();
    guest.dim = NETHER.into();
    g.players = vec![guest, player()];
    g.run(15);
    assert!(g.server.wid().is_some());
    assert!(column_untouched(&w.borrow()));
    assert!(no_orphan_tokens(&g));
}

/// First review, finding 5: an explosion removes a burning burner: the
/// look changing in the window never brings it back.
#[test]
fn review_5_a_look_change_never_rebuilds_a_vanished_machine() {
    let mut g = game();
    let at = g.burner();
    g.hold("minecraft:coal", 1);
    g.click(actions::BURNER_FUEL);
    g.run(5);
    g.client.closed(&mut g.ch);
    g.world.borrow_mut().set(OW, at, "minecraft:air");
    for _ in 0..400 {
        g.step();
        assert_eq!(g.world.borrow().get(OW, at), "minecraft:air", "rebuilt at tick {}", g.tick);
    }
    assert!(g.ready().world.machines.is_empty());
}

/// First review, finding 8: a copy of the world drops nothing of the
/// machines it never had.
#[test]
fn review_8_a_machine_never_seen_here_drops_nothing() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let original = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(original.clone(), kv.clone(), 1);
    g.run(10);
    let copy = Rc::new(RefCell::new(World::new()));
    copy.borrow_mut().storage = original.borrow().storage.clone();
    g.furnace_with_output([0, 70, 0], 64);
    drop(g);
    let mut g = Game::new(copy.clone(), kv, 2);
    g.run(10 + 3 * LOAD_CHECK as usize);
    assert!(g.ready().world.machines.is_empty());
    assert!(copy.borrow().dropped.is_empty());
}

/// First review, finding 9: an id column left by a crash is put back.
#[test]
fn review_9_an_orphan_id_column_is_put_back_after_a_crash() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    {
        let mut g = Game::new(w.clone(), kv.clone(), 1);
        g.run(10);
    }
    let mut g = Game::new(w.clone(), kv.clone(), 2);
    for _ in 0..10 {
        g.client_step();
        g.world.borrow_mut().run_commands();
        if !column_untouched(&w.borrow()) {
            break;
        }
        g.server.tick(&mut g.sh, g.tick, vec![player()]);
        g.world.borrow_mut().apply_writes();
    }
    assert!(!column_untouched(&w.borrow()), "the column holds tokens");
    drop(g);
    let mut g = Game::new(w.clone(), kv, 3);
    g.run(15);
    assert!(g.server.wid().is_some());
    assert!(column_untouched(&w.borrow()), "the orphan column was put back");
}

/// First review, finding 12: an item id from the client never reaches a
/// command line.
#[test]
fn review_12_malformed_item_ids_build_no_lines() {
    let mut g = game();
    g.burner();
    let evil = Held {
        item: "minecraft:oak_log run setblock 0 0 0 minecraft:tnt".into(),
        count: 1,
    };
    g.client.select(&mut g.ch, actions::BURNER_FUEL, Some(evil));
    g.run(5);
    assert!(g.world.borrow().ran.iter().all(|l| !l.contains("tnt")));
    assert!(g.server.txs().is_empty());
}

/// First review, finding 13: the test button needs creative; a far
/// machine is not used.
#[test]
fn review_13_test_button_and_reach() {
    let mut g = game();
    g.players[0].creative = false;
    let bat = [3, 70, 0];
    g.place(bat, "ntm:machine_battery[facing=east,honey_level=0]");
    g.run(2);
    g.open(bat);
    let test_button = |g: &Game| {
        g.ch.screen.as_ref().is_some_and(|s| {
            s.widgets
                .iter()
                .any(|w| matches!(w, UiWidget::Button { label, .. } if label.contains("+10%")))
        })
    };
    assert!(g.ch.screen.is_some());
    assert!(!test_button(&g), "the test button is hidden outside creative");
    g.click(actions::BATTERY_DEBUG_FILL);
    let Data::Battery(b) = g.machine(bat).data else { panic!() };
    assert_eq!(b.power, 0);
    g.players[0].creative = true;
    g.run(10);
    assert!(test_button(&g), "and shown in creative");
    g.players[0].creative = false;
    let far = [40, 70, 0];
    g.place(far, "ntm:machine_battery[facing=east,honey_level=0]");
    g.run(2);
    g.client.open(&mut g.ch, OW, far);
    g.run(3);
    assert!(g.ch.screen.as_ref().is_none_or(|s| !s
        .widgets
        .iter()
        .any(|w| matches!(w, UiWidget::Text { text, .. } if text == "40 70 0"))));
}

// ------------------------------------------------------ second review

/// Second review, finding 1: the integrated server runs 64 quiet lines a
/// tick, cutting a batch anywhere. Every cut point ahead of a take: the
/// coal taken is exactly the coal credited.
#[test]
fn review_2nd_1_a_take_cut_anywhere_never_credits_without_taking() {
    for filler in 50..70 {
        let mut g = game();
        let at = g.burner();
        g.hold("minecraft:coal", 3);
        let lines: Vec<String> = (0..filler)
            .map(|i| format!("scoreboard players set #f{i} other 0"))
            .collect();
        let tx = g.server.next_id;
        g.server.mail.send(S2C::Run { tx, lines });
        g.click(actions::BURNER_FUEL);
        g.run(300);
        let taken = 3 - g.held().map_or(0, |h| h.1);
        assert_eq!(taken, 1, "filler {filler}");
        assert_eq!(g.fuel_taken(at), 1, "filler {filler}");
    }
}

/// Second review, finding 1: the hand empties between the check of a
/// take and its clear (cut into two ticks): the machine is credited exactly
/// what the commands took, at every cut point.
#[test]
fn review_2nd_1_the_hand_emptying_mid_take_never_dupes() {
    for filler in 56..68 {
        let mut g = game();
        let at = g.burner();
        g.hold("minecraft:coal", 3);
        let lines: Vec<String> = (0..filler)
            .map(|i| format!("scoreboard players set #f{i} other 0"))
            .collect();
        let tx = g.server.next_id;
        g.server.mail.send(S2C::Run { tx, lines });
        let held = g.held_now();
        g.client.select(&mut g.ch, actions::BURNER_FUEL, held);
        let mut moved = 0;
        for _ in 0..300 {
            g.step();
            let checked = g
                .world
                .borrow()
                .ran
                .iter()
                .any(|l| l.contains("if items entity @s weapon.mainhand minecraft:coal"));
            if checked && moved == 0 && g.held().is_some() {
                // The player puts the coal away right after the check ran.
                moved = g.held().map_or(0, |h| h.1);
                g.world.borrow_mut().held = None;
            }
        }
        let took = 3 - moved - g.held().map_or(0, |h| h.1);
        assert_eq!(g.fuel_taken(at), took, "filler {filler}: credited what was taken");
    }
}

/// Second review, finding 2: two takes on one machine, the first late
/// past every timeout: both are taken and both credited.
#[test]
fn review_2nd_2_two_takes_under_long_lag_credit_both() {
    let mut g = game();
    let at = g.burner();
    g.hold("minecraft:coal", 5);
    g.world.borrow_mut().paused = true;
    g.click(actions::BURNER_FUEL);
    g.run(1250);
    g.click(actions::BURNER_FUEL);
    g.run(20);
    g.world.borrow_mut().paused = false;
    g.run(600);
    assert_eq!(g.held(), Some(("minecraft:coal".into(), 3)));
    assert_eq!(g.fuel_taken(at), 2);
}

/// Second review, finding 2: two outputs taken under long lag (5 then 7):
/// the player gets 12, never more, however long it waits.
#[test]
fn review_2nd_2_two_gives_under_long_lag_give_each_once() {
    let mut g = game();
    let at = [0, 70, 0];
    g.furnace_with_output(at, 5);
    g.world.borrow_mut().paused = true;
    g.click(actions::FURNACE_TAKE);
    g.run(1250);
    g.furnace_with_output(at, 7);
    g.click(actions::FURNACE_TAKE);
    g.world.borrow_mut().paused = false;
    g.run(15_000);
    assert_eq!(g.inv("minecraft:iron_ingot"), 12);
    assert!(g.server.txs().is_empty());
}

/// Second review, finding 3: the give runs but its answer can't be
/// written (the answer cell's chunk unloaded). It is never given again.
#[test]
fn review_2nd_3_a_give_whose_answer_fails_is_never_repeated() {
    let mut g = game();
    let at = [0, 70, 0];
    g.furnace_with_output(at, 5);
    g.world.borrow_mut().paused = true;
    g.click(actions::FURNACE_TAKE);
    g.run(2);
    // The player wanders off: the chunk with the machine and the answer
    // cell unloads, then the lines run.
    g.world.borrow_mut().unloaded.insert((OW.into(), 0, 0));
    g.world.borrow_mut().paused = false;
    g.run(2000);
    assert_eq!(g.inv("minecraft:iron_ingot"), 5, "given while away");
    g.world.borrow_mut().unloaded.clear();
    g.run(15_000);
    assert_eq!(g.inv("minecraft:iron_ingot"), 5, "never twice");
    assert!(g.server.txs().is_empty());
}

/// Second review, finding 4: the game stops right after the save that
/// took the output, before any line ran: the give survives in the save
/// and runs once on the next start.
#[test]
fn review_2nd_4_a_crash_after_the_save_loses_nothing() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w.clone(), kv.clone(), 1);
    g.run(10);
    let at = [0, 70, 0];
    g.furnace_with_output(at, 5);
    g.client.select(&mut g.ch, actions::FURNACE_TAKE, None);
    g.client_step(); // the action reaches the server's folder
    g.server.tick(&mut g.sh, g.tick, g.players.clone()); // saved; lines out
    g.world.borrow_mut().apply_writes();
    assert_eq!(g.saved().txs.len(), 1);
    drop(g); // crash: the client never ran them
    let mut g = Game::new(w.clone(), kv.clone(), 2);
    g.run(400);
    assert_eq!(g.inv("minecraft:iron_ingot"), 5);
    assert_eq!(g.output(at), None);
}

/// Second review, finding 4: the game stops after the give ran but before
/// the server read the answer: the score in the world says it ran.
#[test]
fn review_2nd_4_a_crash_after_the_give_ran_never_gives_again() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w.clone(), kv.clone(), 1);
    g.run(10);
    let at = [0, 70, 0];
    g.furnace_with_output(at, 5);
    g.client.select(&mut g.ch, actions::FURNACE_TAKE, None);
    g.client_step();
    g.server.tick(&mut g.sh, g.tick, g.players.clone());
    g.world.borrow_mut().apply_writes();
    g.client_step(); // the client takes the lines
    g.world.borrow_mut().run_commands(); // and they run
    assert_eq!(g.inv("minecraft:iron_ingot"), 5);
    drop(g); // crash before the server reads the answer
    let mut g = Game::new(w.clone(), kv.clone(), 2);
    g.run(400);
    assert_eq!(g.inv("minecraft:iron_ingot"), 5, "the give ran once");
    assert!(g.server.txs().is_empty());
}

/// Second review, finding 5: while the save fails, nothing that moves
/// items leaves; once it works, the give goes, once.
#[test]
fn review_2nd_5_a_failing_save_holds_every_item_line() {
    let mut g = game();
    let at = [0, 70, 0];
    g.furnace_with_output(at, 5);
    g.kv.borrow_mut().refuse.insert(keys::JOURNAL.into());
    g.click(actions::FURNACE_TAKE);
    g.run(300);
    assert_eq!(g.inv("minecraft:iron_ingot"), 0);
    assert!(g.world.borrow().ran.iter().all(|l| !l.contains("give @s minecraft:iron_ingot")));
    assert!(g.sh.told.iter().any(|t| t.contains("save")), "{:?}", g.sh.told);
    g.kv.borrow_mut().refuse.clear();
    g.run(300);
    assert_eq!(g.inv("minecraft:iron_ingot"), 5);
    g.run(3000);
    assert_eq!(g.inv("minecraft:iron_ingot"), 5);
}

/// Second review, finding 6: a late drop doesn't destroy the block the
/// player put where the machine was.
#[test]
fn review_2nd_6_a_late_drop_leaves_the_players_block() {
    let mut g = game();
    let at = g.burner();
    {
        let r = g.ready();
        let Data::WoodBurner(b) = &mut r.world.machines.get_mut(&(OW.to_string(), at)).unwrap().data else {
            panic!()
        };
        b.fuel = Some(("minecraft:oak_log".into(), 4));
        b.on = false;
    }
    g.world.borrow_mut().paused = true;
    g.world.borrow_mut().set(OW, at, "minecraft:air");
    g.server.block_broken(OW, at, "ntm:machine_wood_burner[facing=north,honey_level=0]");
    g.run(5);
    g.world.borrow_mut().set(OW, at, "minecraft:chest[facing=north]");
    g.world.borrow_mut().paused = false;
    g.run(400);
    assert_eq!(g.world.borrow().get(OW, at), "minecraft:chest[facing=north]");
    assert!(g.world.borrow().dropped.iter().any(|d| d.contains("oak_log")));
    assert!(no_orphan_tokens(&g));
}

/// Second review, finding 7: a transaction left in another world never
/// holds back the same coordinates in this one.
#[test]
fn review_2nd_7_another_worlds_transaction_never_blocks_this_one() {
    let mut g = game();
    let at = [0, 70, 0];
    g.furnace_with_output(at, 5);
    g.world.borrow_mut().paused = true;
    g.click(actions::FURNACE_TAKE);
    g.run(3);
    let wid_a = g.server.wid().unwrap().to_string();
    let w2 = Rc::new(RefCell::new(World::new()));
    g.world = w2.clone();
    g.sh.world = w2.clone();
    g.ch.world = w2.clone();
    g.server = Server::new(&mut g.sh, 0);
    g.game_time = 999_000;
    g.run(15);
    assert_ne!(g.server.wid().unwrap(), wid_a);
    g.furnace_with_output(at, 7);
    g.click(actions::FURNACE_TAKE);
    g.run(200);
    assert_eq!(g.inv("minecraft:iron_ingot"), 7);
    let (_, a) = persist::load(&mut *g.kv.borrow_mut(), &wid_a).unwrap();
    assert_eq!(a.txs.len(), 1, "world A's give waits for world A");
}

/// The client never got the give (its session ended first): it goes to
/// the next session, once.
#[test]
fn a_give_the_client_never_received_goes_to_the_next_session_once() {
    let mut g = game();
    let at = [0, 70, 0];
    g.furnace_with_output(at, 5);
    g.client.select(&mut g.ch, actions::FURNACE_TAKE, None);
    g.server_step();
    g.server_step();
    g.client.left = true;
    g.run(1000);
    assert_eq!(g.inv("minecraft:iron_ingot"), 5);
    g.run(3000);
    assert_eq!(g.inv("minecraft:iron_ingot"), 5);
}

/// Crash: the world comes back from its autosave, before a give that
/// went through (no items, no score); the mod's save is newer. The give is
/// noticed as forgotten and the furnace goes back to before it.
#[test]
fn a_crash_rollback_past_a_give_puts_the_output_back() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w.clone(), kv.clone(), 1);
    g.run(10);
    let at = [0, 70, 0];
    g.furnace_with_output(at, 5);
    let autosave = w.borrow().clone();
    g.click(actions::FURNACE_TAKE);
    g.run(20);
    assert_eq!(g.inv("minecraft:iron_ingot"), 5);
    drop(g);
    *w.borrow_mut() = autosave; // the crash
    let mut g = Game::new(w.clone(), kv.clone(), 2);
    g.run(400);
    // The world never gave them: the furnace has them back.
    assert_eq!(g.inv("minecraft:iron_ingot"), 0);
    assert_eq!(g.output(at), Some(("minecraft:iron_ingot".into(), 5)));
    g.run(400);
    assert_eq!(g.inv("minecraft:iron_ingot"), 0, "never given late either");
}

/// Crash: the world comes back from before a take (the item back in the
/// hand): the machine gives up the item it was credited.
#[test]
fn a_crash_rollback_past_a_take_takes_it_back_from_the_machine() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w.clone(), kv.clone(), 1);
    g.run(10);
    let at = [0, 70, 0];
    g.place(at, "ntm:machine_electric_furnace_off[facing=north,honey_level=0]");
    g.run(2);
    g.open(at);
    g.hold("minecraft:raw_iron", 3);
    let autosave = w.borrow().clone();
    g.click(actions::FURNACE_INSERT);
    g.run(20);
    let Data::ElectricFurnace(f) = g.machine(at).data else { panic!() };
    assert_eq!(f.input, Some(("minecraft:raw_iron".into(), 1)));
    drop(g);
    *w.borrow_mut() = autosave; // the crash: 3 raw iron in the hand again
    let mut g = Game::new(w.clone(), kv.clone(), 2);
    g.run(400);
    let Data::ElectricFurnace(f) = g.machine(at).data else { panic!() };
    assert_eq!(f.input, None, "the forgotten take was undone");
    assert_eq!(g.held(), Some(("minecraft:raw_iron".into(), 3)));
}

/// No crash: a world that saved its scores confirms every kept
/// transaction; nothing is undone.
#[test]
fn a_clean_reload_confirms_and_undoes_nothing() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w.clone(), kv.clone(), 1);
    g.run(10);
    let at = [0, 70, 0];
    g.furnace_with_output(at, 5);
    g.click(actions::FURNACE_TAKE);
    g.run(20);
    drop(g);
    let mut g = Game::new(w.clone(), kv.clone(), 2);
    g.run(400);
    assert_eq!(g.inv("minecraft:iron_ingot"), 5);
    assert!(g.ready().verify.is_empty(), "checked");
    assert_eq!(g.ready().world.applied.len(), 1, "still kept");
}

/// Powers the furnace at `at` fully.
fn charge_furnace(g: &mut Game, at: Pos) {
    let r = g.ready();
    let Data::ElectricFurnace(f) = &mut r.world.machines.get_mut(&(OW.to_string(), at)).unwrap().data else {
        panic!()
    };
    f.power = crate::machine::ElectricFurnace::MAX;
}

/// Third check, finding (a): raw iron went in, part of it smelted, then a
/// crash rolls the world back past the insert (the hand has it again). The
/// furnace goes back to before the insert: no ingot made of it remains.
#[test]
fn a_rollback_past_an_insert_undoes_what_was_smelted_of_it() {
    for smelt_ticks in [40u64, 150, 400] {
        let kv = Rc::new(RefCell::new(MemKv::default()));
        let w = Rc::new(RefCell::new(World::new()));
        let mut g = Game::new(w.clone(), kv.clone(), 1);
        g.run(10);
        let at = [0, 70, 0];
        g.place(at, "ntm:machine_electric_furnace_off[facing=north,honey_level=0]");
        g.run(2);
        charge_furnace(&mut g, at);
        g.open(at);
        g.hold("minecraft:raw_iron", 3);
        let autosave = w.borrow().clone();
        g.click(actions::FURNACE_INSERT);
        g.click(actions::FURNACE_INSERT);
        g.run(smelt_ticks as usize);
        drop(g);
        *w.borrow_mut() = autosave;
        let mut g = Game::new(w.clone(), kv.clone(), 2);
        g.run(400);
        let Data::ElectricFurnace(f) = g.machine(at).data else { panic!() };
        assert_eq!((f.input.clone(), f.output.clone()), (None, None), "after {smelt_ticks} ticks");
        assert_eq!(g.held(), Some(("minecraft:raw_iron".into(), 3)));
        assert_eq!(g.inv("minecraft:iron_ingot"), 0);
    }
}

/// Third check, finding (a): the ingot was even taken out (a later give,
/// forgotten too): everything goes back to before the insert.
#[test]
fn a_rollback_past_an_insert_and_its_output_taken_out_undoes_both() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w.clone(), kv.clone(), 1);
    g.run(10);
    let at = [0, 70, 0];
    g.place(at, "ntm:machine_electric_furnace_off[facing=north,honey_level=0]");
    g.run(2);
    charge_furnace(&mut g, at);
    g.open(at);
    g.hold("minecraft:raw_iron", 1);
    let autosave = w.borrow().clone();
    g.click(actions::FURNACE_INSERT);
    g.run(120);
    g.click(actions::FURNACE_TAKE);
    g.run(20);
    assert_eq!(g.inv("minecraft:iron_ingot"), 1);
    drop(g);
    *w.borrow_mut() = autosave;
    let mut g = Game::new(w.clone(), kv.clone(), 2);
    g.run(400);
    let Data::ElectricFurnace(f) = g.machine(at).data else { panic!() };
    assert_eq!((f.input, f.output), (None, None));
    assert_eq!(g.held(), Some(("minecraft:raw_iron".into(), 1)));
    assert_eq!(g.inv("minecraft:iron_ingot"), 0);
}

/// Third check, finding (a): the same with a press, its stamp and plates.
#[test]
fn a_rollback_past_press_inserts_undoes_the_plates() {
    use crate::press::actions as pa;
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w.clone(), kv.clone(), 1);
    g.run(10);
    let press = [0, 70, 0];
    g.place(press, "ntm:machine_epress[facing=north,honey_level=0]");
    g.run(2);
    {
        let r = g.ready();
        let Data::ElectricPress(p) = &mut r.world.machines.get_mut(&(OW.to_string(), press)).unwrap().data else {
            panic!()
        };
        p.power = crate::press::ElectricPress::MAX;
    }
    g.open(press);
    g.world.borrow_mut().inventory.insert("ntm:ingot_lead".into(), 2);
    g.hold("ntm:stamp_iron_plate", 1);
    let autosave = w.borrow().clone();
    g.click(pa::INSERT);
    g.run(5);
    for _ in 0..2 {
        let lead = g.world.borrow_mut().inventory.remove("ntm:ingot_lead");
        if let Some(n) = lead {
            g.hold("ntm:ingot_lead", n);
        }
        g.click(pa::INSERT);
        g.run(5);
        let left = g.world.borrow_mut().held.take();
        if let Some((i, n)) = left {
            g.world.borrow_mut().inventory.insert(i, n);
        }
    }
    g.run(100);
    let Data::ElectricPress(p) = g.machine(press).data else { panic!() };
    assert!(p.core.output.is_some(), "plates were pressed: {p:?}");
    drop(g);
    *w.borrow_mut() = autosave;
    let mut g = Game::new(w.clone(), kv.clone(), 2);
    g.run(400);
    let Data::ElectricPress(p) = g.machine(press).data else { panic!() };
    assert_eq!((p.core.stamp.clone(), p.core.input.clone(), p.core.output.clone()), (None, None, None));
}

/// Third check, finding (b): the kept transactions age out, and the game
/// stops before the client gets the resets: the next run sends them.
#[test]
fn resets_survive_a_stop_before_the_client_has_them() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w.clone(), kv.clone(), 1);
    g.run(10);
    g.furnace_with_output([0, 70, 0], 5);
    g.click(actions::FURNACE_TAKE);
    g.run(20);
    assert_eq!(g.scores_left(), 1);
    // Old enough, with the client gone quiet: the resets can't arrive.
    g.world.borrow_mut().paused = true;
    g.run(RETAIN_TICKS as usize + 50);
    assert!(g.ready().world.applied.is_empty());
    drop(g); // stops before they ran
    w.borrow_mut().queue.clear();
    w.borrow_mut().paused = false;
    let left = w
        .borrow()
        .scores
        .keys()
        .filter(|(h, o)| o == tx::OBJECTIVE && h.starts_with("#t"))
        .count();
    assert_eq!(left, 1, "the score is still in the world");
    let mut g = Game::new(w.clone(), kv.clone(), 2);
    g.run(220);
    assert_eq!(g.scores_left(), 0, "sent again from the save");
    assert!(g.ready().world.resets.is_empty());
}

/// Third check, finding (c): the answer is read and committed, and the
/// game stops before its `set-blocks` (made after the hook) reach the
/// world: the token is put back on the next run.
#[test]
fn an_answer_token_whose_restore_was_cut_off_is_put_back() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w.clone(), kv.clone(), 1);
    g.run(10);
    g.furnace_with_output([0, 70, 0], 5);
    g.client.select(&mut g.ch, actions::FURNACE_TAKE, None);
    let mut crashed = false;
    for _ in 0..50 {
        g.client_step();
        g.world.borrow_mut().run_commands();
        let had_tx = !g.server.txs().is_empty();
        g.server.tick(&mut g.sh, g.tick, g.players.clone());
        if had_tx && g.server.txs().is_empty() {
            // Resolved and saved this hook; its restore is still queued.
            g.world.borrow_mut().writes.clear();
            crashed = true;
            break;
        }
        g.world.borrow_mut().apply_writes();
    }
    assert!(crashed);
    assert!(!no_tokens(&w.borrow()), "the token is left");
    drop(g);
    let mut g = Game::new(w.clone(), kv, 2);
    g.run(60);
    assert!(no_tokens(&w.borrow()), "put back from the save");
}

mod radiation_tests;
mod lag_tests;

mod bomb_tests;

mod fluid_tests;

mod crafter_tests;

mod review_m2_tests;

mod shredder_tests;

mod blastfurnace_tests;

mod foundry_tests;

mod slot_items_tests;

mod anvil_tests;

mod press_slots_tests;

mod purex_tests;

mod bedrock_tests;

mod crystallizer_tests;

mod stage3_tests;

mod stage4_tests;

mod excavator_tests;

mod oil_tests;

/// A world saved at the end of the bedrock chain's stage 4 (25 machines, 24
/// transactions kept for checking) loaded after crashes that left world-id
/// columns behind. In the game such a load ran out of the tick's fuel and
/// the mod stopped, leaving more probes: the world stayed stuck. The tick
/// it loads in does nothing else, and every tick after reads a bounded
/// number of blocks while the checks and the old columns are worked off.
#[test]
fn a_world_loaded_after_crashes_spreads_its_first_work() {
    let save = include_bytes!("tests/fixtures/bedrock_stage4.save");
    let (_, _, data) = persist::decode(save).expect("the fixture decodes");
    let applied = data.applied.len();
    assert!(applied >= 20, "{applied}");
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let wid = {
        let mut g = Game::new(w.clone(), kv.clone(), 1);
        g.run(10);
        g.server.wid().unwrap().to_string()
    };
    let bytes = persist::encode(&wid, 5_000, &data).unwrap();
    kv.borrow_mut().map.insert(crate::kv::keys::world(&wid), bytes);
    // A crash in the middle of identifying the world: its column stays.
    for seed in 2..3 {
        let mut g = Game::new(w.clone(), kv.clone(), seed);
        for _ in 0..10 {
            g.client_step();
            g.world.borrow_mut().run_commands();
            if !column_untouched(&w.borrow()) {
                break;
            }
            g.server.tick(&mut g.sh, g.tick, vec![player()]);
            g.world.borrow_mut().apply_writes();
        }
    }
    let mut g = Game::new(w.clone(), kv.clone(), 9);
    let left = g.server.probes.len();
    assert!(left >= 1, "probes left by the crash: {left}");
    let mut most = 0;
    let mut entered = None;
    for t in 0..400 {
        let before = g.sh.reads;
        let was = g.server.wid().is_some();
        let sent = if was { g.ready().verify_publish.len() } else { 0 };
        g.run(1);
        most = most.max(g.sh.reads - before);
        if was {
            let now = g.ready().verify_publish.len();
            assert!(now <= sent + ANSWER_CELLS_PER_TICK as usize, "{} checks in one tick", now - sent);
        }
        if !was && g.server.wid().is_some() {
            entered = Some(t);
            // The load is all that tick did: no machine ran.
            assert_eq!(g.ready().world.ticks, data.ticks, "o tick da carga não roda as máquinas");
        }
    }
    assert!(entered.is_some(), "the world loads");
    // A chunk's bottom (256) for each of 4 answer cells, the old probes' 40
    // cells and the identify search (64 columns of 10) at most.
    assert!(most <= 4 * 256 + 40 + 640, "{most} blocks read in a tick");
    let r = g.ready();
    assert!(r.verify.is_empty() || r.verify_publish.len() >= applied.min(24), "checks sent: {} of {}", r.verify_publish.len(), r.verify.len());
    assert!(column_untouched(&w.borrow()), "the old columns were put back");
}

mod oil_b_tests;

mod oil_c_tests;

mod oil_d_tests;
