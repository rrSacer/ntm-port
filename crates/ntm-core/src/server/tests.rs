//! The native server against a fake world: blocks, chunks that aren't
//! loaded, each machine block entity's slots and menu data, menu buttons,
//! redstone, items dropped, and the world's storage.

use super::*;
use crate::{kv::MemKv, machine::Data};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

const OW: &str = "minecraft:overworld";

#[derive(Clone)]
struct World {
    blocks: HashMap<(String, Pos), String>,
    /// Chunks (dimension, cx, cz) not loaded.
    unloaded: BTreeSet<(String, i32, i32)>,
    /// `set_blocks` of the server, applied after its tick returns.
    writes: Vec<(String, Pos, String)>,
    /// Items dropped: "<dimension> <x> <y> <z> <item> <count>".
    dropped: Vec<String>,
    /// The machine block entities: their slots and menu data. Only the
    /// blocks a test gives slots have them.
    engine: BTreeMap<(String, Pos), (container_bridge::Slots, Vec<i32>)>,
    /// Menu buttons pressed, taken on the next tick.
    presses: Vec<(String, Pos, u32)>,
    /// The redstone signal at cells (0 elsewhere).
    signal: BTreeMap<(String, Pos), u8>,
    /// What each machine kept with its block entity.
    saved: BTreeMap<(String, Pos), String>,
    kv: MemKv,
    /// What each player carries, by id.
    carried: BTreeMap<String, Vec<(String, u32)>>,
    /// Damage done: (player, amount, type).
    hurts: Vec<(String, f32, String)>,
    /// Effects given: (player, effect, seconds, amplifier).
    effects: Vec<(String, String, u32, u8)>,
    /// The values each player's client was sent.
    values: BTreeMap<(String, String), i64>,
}

impl World {
    fn new() -> Self {
        let mut w = World {
            blocks: HashMap::new(),
            unloaded: BTreeSet::new(),
            writes: Vec::new(),
            dropped: Vec::new(),
            engine: BTreeMap::new(),
            presses: Vec::new(),
            signal: BTreeMap::new(),
            saved: BTreeMap::new(),
            kv: MemKv::default(),
            carried: BTreeMap::new(),
            hurts: Vec::new(),
            effects: Vec::new(),
            values: BTreeMap::new(),
        };
        for x in -32..32 {
            for z in -32..32 {
                w.blocks.insert((OW.into(), [x, -64, z]), "minecraft:bedrock".into());
                for y in -63..-50 {
                    w.blocks.insert((OW.into(), [x, y, z]), "minecraft:deepslate".into());
                }
            }
        }
        w
    }
    /// The server's `set_blocks` of its last tick, made now.
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
        self.blocks.get(&(dim.to_string(), p)).cloned().unwrap_or_else(|| "minecraft:air".into())
    }
    fn set(&mut self, dim: &str, p: Pos, s: &str) {
        self.blocks.insert((dim.into(), p), s.into());
    }
}

struct SHost {
    world: Rc<RefCell<World>>,
    told: Vec<String>,
    /// Blocks read so far.
    reads: u64,
}

impl Kv for SHost {
    fn get(&mut self, key: &str) -> Option<Vec<u8>> {
        self.world.borrow_mut().kv.get(key)
    }
    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), String> {
        self.world.borrow_mut().kv.set(key, value)
    }
    fn delete(&mut self, key: &str) {
        self.world.borrow_mut().kv.delete(key)
    }
}

impl ServerHost for SHost {
    fn get_block(&mut self, dim: &str, p: Pos) -> Option<String> {
        self.reads += 1;
        let w = self.world.borrow();
        w.loaded(dim, p).then(|| w.get(dim, p))
    }
    fn set_blocks(&mut self, dim: &str, blocks: &[(Pos, String)]) -> Result<(), String> {
        // As the game: made after the tick (`apply_writes`).
        let mut w = self.world.borrow_mut();
        w.writes.extend(blocks.iter().map(|(p, s)| (dim.to_string(), *p, s.clone())));
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
    fn save_machine(&mut self, dim: &str, p: Pos, json: &str) {
        self.world.borrow_mut().saved.insert((dim.to_string(), p), json.into());
    }
    fn redstone_many(&mut self, dim: &str, cells: &[Pos]) -> Vec<Option<u8>> {
        let w = self.world.borrow();
        cells.iter().map(|p| w.loaded(dim, *p).then(|| w.signal.get(&(dim.to_string(), *p)).copied().unwrap_or(0))).collect()
    }
    fn inventory(&mut self, id: &str) -> Option<Vec<(String, u32)>> {
        Some(self.world.borrow().carried.get(id).cloned().unwrap_or_default())
    }
    fn hurt(&mut self, id: &str, amount: f32, kind: &str) {
        self.world.borrow_mut().hurts.push((id.into(), amount, kind.into()));
    }
    fn effect(&mut self, id: &str, effect: &str, seconds: u32, amplifier: u8) {
        self.world.borrow_mut().effects.push((id.into(), effect.into(), seconds, amplifier));
    }
    fn set_player_value(&mut self, id: &str, key: &str, value: i64) {
        self.world.borrow_mut().values.insert((id.into(), key.into()), value);
    }
    fn drop_items(&mut self, dim: &str, p: Pos, items: &[(String, u32)]) {
        let mut w = self.world.borrow_mut();
        for (item, n) in items {
            w.dropped.push(format!("{dim} {} {} {} {item} {n}", p[0], p[1], p[2]));
        }
    }
}

/// One game: a world and the server.
struct Game {
    tick: u64,
    world: Rc<RefCell<World>>,
    server: Server,
    sh: SHost,
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
    /// The server opening `world`: the machines come back from what their
    /// block entities kept, as their first ticks adopt them.
    fn new(world: Rc<RefCell<World>>) -> Self {
        let mut sh = SHost { world: world.clone(), told: vec![], reads: 0 };
        let mut server = Server::native(&mut sh, 0);
        server.set_players(vec![player()]);
        let saved: Vec<((String, Pos), String)> = world.borrow().saved.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        for ((dim, p), json) in saved {
            assert!(server.native_adopt(&dim, p, &json), "{json}");
        }
        Self { tick: 0, world, server, sh }
    }
    /// One tick: the server's, then its block writes.
    fn step(&mut self) {
        self.tick += 1;
        self.server.native_tick(&mut self.sh, self.tick);
        self.world.borrow_mut().apply_writes();
    }
    fn run(&mut self, n: usize) {
        for _ in 0..n {
            self.step();
        }
    }
    /// A block put in the overworld, as the game's block change says it.
    fn place(&mut self, p: Pos, state: &str) {
        let old = self.world.borrow().get(OW, p);
        self.world.borrow_mut().set(OW, p, state);
        self.server.native_block_changed(OW, p, &old, state);
    }
    /// A block broken: air in its place.
    fn break_block(&mut self, p: Pos) {
        self.place(p, "minecraft:air");
    }
    fn ready(&mut self) -> &mut Ready {
        &mut self.server.ready
    }
    fn machine(&mut self, p: Pos) -> Machine {
        self.ready().world.machines[&(OW.to_string(), p)].clone()
    }
    /// The duct at `p` typed `fluid` by the fluid identifier in a hand.
    fn type_duct(&mut self, p: Pos, fluid: &str) {
        assert!(self.server.native_use_item(OW, p, native_server::IDENTIFIER, Some(fluid)), "{p:?} {fluid}");
    }
    /// What the electric furnace at `p` holds out.
    fn output(&mut self, p: Pos) -> Option<(String, u32)> {
        let Data::ElectricFurnace(f) = self.machine(p).data else { panic!() };
        f.output
    }
}

fn game() -> Game {
    let mut g = Game::new(Rc::new(RefCell::new(World::new())));
    g.run(2);
    g
}

#[test]
fn breaking_a_machine_drops_what_it_holds_outside_its_slots_once() {
    let mut g = game();
    let at = [0, 70, 0];
    g.place(at, "ntm:machine_wood_burner[facing=north,honey_level=0]");
    g.run(2);
    {
        let r = g.ready();
        let Data::WoodBurner(b) = &mut r.world.machines.get_mut(&(OW.to_string(), at)).unwrap().data else {
            panic!()
        };
        b.fuel = Some(("minecraft:oak_log".into(), 4));
        b.on = false;
    }
    g.break_block(at);
    g.run(400);
    let dropped = g.world.borrow().dropped.clone();
    assert_eq!(dropped, ["minecraft:overworld 0 70 0 minecraft:oak_log 4"]);
    assert!(g.ready().world.machines.is_empty());
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

/// The world's state outside the blocks (a bomb's queue, the doses, the
/// clock) comes back when the server opens the world again.
#[test]
fn the_world_keeps_its_state_outside_the_blocks() {
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w.clone());
    g.run(5);
    g.ready().world.bombs.next = 7;
    g.ready().world.radiation.insert("p1".into(), crate::radiation::Dose { millirad: 42 });
    g.run(crate::persist::TICKS_EVERY as usize);
    drop(g);
    let mut g = Game::new(w);
    let r = g.ready();
    assert_eq!((r.world.bombs.next, r.world.radiation["p1"].millirad), (7, 42));
    assert_eq!(r.world.ticks, crate::persist::TICKS_EVERY, "the clock kept at its period");
}

/// What a player carries gives a dose once a second past the joining
/// grace, sickness acts at its bands, and the client reads the dose and
/// the rate; a creative player gets no dose but reads the rate.
#[test]
fn radiation_from_what_a_player_carries() {
    use crate::radiation::{GRACE_TICKS, KEY_DOSE, KEY_RATE};
    let mut g = game();
    let mut p = player();
    p.creative = false;
    g.server.set_players(vec![p]);
    g.world.borrow_mut().carried.insert("p1".into(), vec![("ntm:ingot_uranium".into(), 64), ("minecraft:dirt".into(), 3)]);
    let value = |g: &Game, k: &str| g.world.borrow().values.get(&("p1".to_string(), k.to_string())).copied();
    g.run(40);
    assert_eq!(value(&g, KEY_RATE), Some(64 * 350), "a taxa aparece já na carência");
    assert_eq!(value(&g, KEY_DOSE), Some(0), "nada de dose na carência");
    g.run(GRACE_TICKS as usize + 20 * 10);
    let dose = value(&g, KEY_DOSE).unwrap();
    assert!((9 * 64 * 350..=12 * 64 * 350).contains(&dose), "{dose}");
    // 200 RAD: nausea and weakness, every second.
    g.ready().world.radiation.get_mut("p1").unwrap().millirad = 300_000;
    g.run(20);
    let effects = g.world.borrow().effects.clone();
    assert!(effects.iter().any(|e| e.1 == "weakness" && e.3 == 0), "{effects:?}");
    // 1000 RAD: death, and the dose starts again.
    g.ready().world.radiation.get_mut("p1").unwrap().millirad = 999_999;
    g.run(40);
    assert!(g.world.borrow().hurts.iter().any(|h| h.2 == "minecraft:generic_kill"), "{:?}", g.world.borrow().hurts);
    assert!(value(&g, KEY_DOSE).unwrap() < 999_999);
    // Creative: the rate, no dose.
    g.server.set_players(vec![player()]);
    let before = g.ready().world.radiation["p1"].millirad;
    g.run(100);
    assert_eq!(g.ready().world.radiation["p1"].millirad, before);
    assert_eq!(value(&g, KEY_RATE), Some(64 * 350));
}

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
mod oil_b_tests;
mod oil_c_tests;
mod oil_d_tests;
mod heater_tests;
mod steam_tests;
