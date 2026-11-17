//! The native server against a fake world: blocks, chunks that aren't
//! loaded, each machine block entity's slots and menu data, menu buttons,
//! redstone, items dropped, and the world's storage.

use super::*;
use crate::{kv::MemKv, machine::Data};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

const OW: &str = "minecraft:overworld";

/// What a chunk's record owes: removals and batches.
type Owed = (Vec<crate::persist::Removal>, Vec<CommittedEffects>);

#[derive(Clone)]
struct World {
    /// `calculateRadiationMod` of the players (`ServerHost::radiation_factor`): 1 without armor.
    radiation_factor: f64,
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
    model_entities: BTreeSet<Key>,
    /// Menu buttons pressed, taken on the next tick.
    presses: Vec<(String, Pos, u32)>,
    /// The redstone signal at cells (0 elsewhere).
    signal: BTreeMap<(String, Pos), u8>,
    sun: BTreeMap<(String, Pos), (i32,i32,bool)>,
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
    /// Particles and sounds shown: (dimension, what).
    shown: Vec<(String, crate::blast::visuals::Show)>,
    /// The acting player's stacks (`player_slots`), for transfers.
    hands: Option<PlayerSlots>,
    /// Machine commits and transfers refused after their checks (nothing may change).
    refuse_commits: bool,
    /// Entities spawned by effects (`apply_entities`), and the ledger of the ids applied: a known id adds nothing.
    entities: Vec<(String, EntitySpawn)>,
    /// Each chunk's ledger (`crate::ledger::ChunkLedger`): entities owed and ids applied, written with the chunk, as
    /// the native host's.
    ledgers: BTreeMap<(String, [i32; 2]), crate::ledger::ChunkLedger>,
    /// What each chunk's record owes (`store_obligations`): removals and batches.
    homes: BTreeMap<(String, [i32; 2]), Owed>,
    /// A host without entities: `apply_entities` refuses.
    no_entities: bool,
    /// The whole stacks transfers copied into machines (`SlotCopy`), as the player held them.
    copied: Vec<String>,
    /// The storage's writes fail (I/O).
    kv_fails: bool,
}

impl World {
    fn new() -> Self {
        let mut w = World {
            radiation_factor: 1.0,
            blocks: HashMap::new(),
            unloaded: BTreeSet::new(),
            writes: Vec::new(),
            dropped: Vec::new(),
            engine: BTreeMap::new(),
            model_entities: BTreeSet::new(),
            presses: Vec::new(),
            signal: BTreeMap::new(),
            sun: BTreeMap::new(),
            saved: BTreeMap::new(),
            kv: MemKv::default(),
            carried: BTreeMap::new(),
            hurts: Vec::new(),
            effects: Vec::new(),
            values: BTreeMap::new(),
            shown: Vec::new(),
            hands: None,
            refuse_commits: false,
            entities: Vec::new(),
            ledgers: BTreeMap::new(),
            homes: BTreeMap::new(),
            copied: Vec::new(),
            no_entities: false,
            kv_fails: false,
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
        if self.world.borrow().kv_fails {
            return Err("disk full".into());
        }
        self.world.borrow_mut().kv.set(key, value)
    }
    fn delete(&mut self, key: &str) {
        self.world.borrow_mut().kv.delete(key)
    }
}

impl ServerHost for SHost {
        fn radiation_factor(&mut self, _id: &str) -> f64 {
            self.world.borrow().radiation_factor
        }
        fn commit_structural(&mut self,cells:&[CellWrite],drops:&[RetainedDrop],owners:&[(String,Pos)])->bool{
            if !drops.is_empty(){return false;}
            if cells.iter().any(|c|self.cell_snapshot(&c.guard.dim,c.guard.pos).as_ref()!=Some(&c.expected)){return false;}
            let dims:BTreeSet<_>=cells.iter().map(|c|c.guard.dim.clone()).collect();
            if dims.len()>1{return false;}
            let Some(dim)=dims.first()else{return true};
            let blocks:Vec<_>=cells.iter().map(|c|(c.guard.pos,c.desired.clone())).collect();
            self.set_machine_blocks(dim,&blocks,owners).is_ok()
        }

    fn commit_machine(&mut self,c:&MachineCommit)->bool{self.commit_machines(std::slice::from_ref(c))}
    fn player_slots(&mut self,_:&str)->Option<PlayerSlots>{self.world.borrow().hands.clone()}
    /// Once by id, as the native host's ledger: a known id adds nothing; an unloaded cell waits.
    /// As the glue (`ledger_add_entities_here`): a kind it cannot summon is refused; an id a loaded record knows adds
    /// nothing; the destination loaded, the entities come out at once and no id is kept (the batch leaves its home's
    /// record in the same tick); else they are owed in a loaded owner's record.
    fn apply_entities(&mut self,id:u64,dim:&str,pos:Pos,entities:&[EntitySpawn],owners:&[(String,Pos)])->bool{
        let mut w=self.world.borrow_mut();
        if w.no_entities||entities.iter().any(|e|!matches!(e.kind.as_str(),"minecraft:item"|"minecraft:armor_stand")){return false;}
        let here=[pos[0]>>4,pos[2]>>4];
        let mut chunks=vec![here];
        for (d,p) in owners{let c=[p[0]>>4,p[2]>>4];if d==dim&&!chunks.contains(&c){chunks.push(c);}}
        let loaded:Vec<[i32;2]>=chunks.into_iter().filter(|c|w.loaded(dim,[c[0]*16,0,c[1]*16])).collect();
        if loaded.iter().any(|c|w.ledgers.get(&(dim.to_string(),*c)).is_some_and(|l|l.knows(id))){return true;}
        if loaded.contains(&here){for e in entities{w.entities.push((dim.into(),e.clone()));}return true;}
        let Some(owner)=loaded.first().copied()else{return false};
        let owed=entities.iter().map(|e|(crate::ledger::OwedEntity{kind:e.kind.clone(),pos:e.pos},e.nbt.clone())).collect();
        w.ledgers.entry((dim.into(),owner)).or_default().owe_entities(id,pos,owed);
        true
    }
    fn store_obligations(&mut self,dim:&str,chunk:[i32;2],removals:&[crate::persist::Removal],batches:&[CommittedEffects],_:&BTreeSet<crate::persist::Obligation>)->bool{
        let mut w=self.world.borrow_mut();
        if !w.loaded(dim,[chunk[0]*16,0,chunk[1]*16]){return false;}
        if removals.is_empty()&&batches.is_empty(){w.homes.remove(&(dim.into(),chunk));}else{w.homes.insert((dim.into(),chunk),(removals.to_vec(),batches.to_vec()));}
        true
    }
    fn home_loaded(&mut self,dim:&str,chunk:[i32;2])->bool{self.world.borrow().loaded(dim,[chunk[0]*16,0,chunk[1]*16])}
    /// Both sides checked before either changes: the player's expected stacks, then the machines' (`commit_machines`
    /// checks every one before it writes).
    fn commit_player_transfer(&mut self,t:&PlayerTransfer,cs:&[MachineCommit])->bool{
        let Some(mut hands)=self.world.borrow().hands.clone() else{return false};
        if !t.publishable(){return false;}
        if t.edits.iter().any(|e|hands.get(e.slot).map(|s|s.encoded.clone())!=e.expected){return false;}
        // A copy names the stack the player held and lands on a slot of the same item, as the glue checks.
        let copied:Option<Vec<String>>=t.copies.iter().map(|c|{
            let from=hands.get(c.from)?;
            let into=cs.iter().find(|m|m.dim==c.dim&&m.pos==c.pos)?.desired.get(c.slot)?.as_ref()?;
            (into.id==from.stack.id).then(||from.encoded.clone())
        }).collect();
        let Some(copied)=copied else{return false};
        if !self.commit_machines(cs){return false;}
        for e in &t.edits{
            let new=e.desired.clone().map(|stack|HeldStack{encoded:format!("{stack:?}"),stack});
            match e.slot{PlayerSlot::Inventory(i)=>hands.inventory[i]=new,PlayerSlot::Offhand=>hands.offhand=new,PlayerSlot::Carried=>hands.carried=new}
        }
        let mut w=self.world.borrow_mut();
        w.hands=Some(hands);
        w.copied.extend(copied);
        for (d,_) in &t.drops{w.dropped.push(format!("player {} {}",d.id,d.count));}
        true
    }
    fn commit_machines(&mut self,cs:&[MachineCommit])->bool{
        let mut w=self.world.borrow_mut();
        if w.refuse_commits{return false;}
        if cs.iter().flat_map(|c|&c.cells).any(|c|!w.loaded(&c.guard.dim,c.guard.pos)||w.get(&c.guard.dim,c.guard.pos)!=c.expected.state){return false;}
        if cs.iter().any(|c|!w.loaded(&c.dim,c.pos)||w.engine.get(&(c.dim.clone(),c.pos)).map(|e|e.0.clone()).unwrap_or_default()!=c.expected){return false;}
        for c in cs {for cell in &c.cells{w.set(&cell.guard.dim,cell.guard.pos,&cell.desired);if blockstate::id_of(&cell.expected.state)!=blockstate::id_of(&cell.desired){w.engine.remove(&cell.guard.key());w.saved.remove(&cell.guard.key());}}}
        for c in cs {w.engine.insert((c.dim.clone(),c.pos),(c.desired.clone(),c.data.clone()));w.saved.insert((c.dim.clone(),c.pos),c.json.clone());}true
    }

    fn solar_sample(&mut self, dim: &str, p: Pos) -> Option<(i32,i32,bool)> { self.world.borrow().sun.get(&(dim.into(),p)).copied() }
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
    fn machine_buttons(&mut self) -> Vec<(String, String, Pos, u32)> {
        std::mem::take(&mut self.world.borrow_mut().presses).into_iter().map(|(d, p, b)| ("p1".to_string(), d, p, b)).collect()
    }
    fn machine_data(&mut self, dim: &str, p: Pos) -> Option<Vec<i32>> {
        self.world.borrow().engine.get(&(dim.to_string(), p)).map(|e| e.1.clone())
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
    fn effect(&mut self, id: &str, effect: &str, ticks: u32, amplifier: u8) {
        self.world.borrow_mut().effects.push((id.into(), effect.into(), ticks, amplifier));
    }
    fn world_seed(&mut self) -> i64 {
        WORLD_SEED
    }
    fn show(&mut self, dim: &str, show: &crate::blast::visuals::Show) {
        self.world.borrow_mut().shown.push((dim.into(), show.clone()));
    }
    /// The y above each column's highest block, tall grass and ferns aside (a motion-blocking heightmap), looked for
    /// from y 80 down: the tests' surfaces are below it.
    fn surface_heights(&mut self, dim: &str, columns: &[(i32, i32)]) -> Vec<Option<i32>> {
        let w = self.world.borrow();
        let key = &mut (dim.to_string(), [0, 0, 0]);
        columns
            .iter()
            .map(|&(x, z)| {
                (-64..=80).rev().find_map(|y| {
                    key.1 = [x, y, z];
                    w.blocks
                        .get(key)
                        .filter(|b| !matches!(crate::blockstate::id_of(b), "minecraft:air" | "minecraft:short_grass" | "minecraft:fern"))
                        .map(|_| y + 1)
                })
            })
            .collect()
    }
    fn set_player_value(&mut self, id: &str, key: &str, value: i64) {
        self.world.borrow_mut().values.insert((id.into(), key.into()), value);
    }
    fn drop_items(&mut self, dim: &str, p: Pos, items: &[(String, u32)]) -> bool {
        let mut w = self.world.borrow_mut();
        for (item, n) in items {
            w.dropped.push(format!("{dim} {} {} {} {item} {n}", p[0], p[1], p[2]));
        }
        true
    }
}

/// One game: a world and the server.
struct Game {
    tick: u64,
    world: Rc<RefCell<World>>,
    server: Server,
    sh: SHost,
}

/// The test world's seed (`ServerHost::world_seed`): what the sickness random is seeded from.
const WORLD_SEED: i64 = 4242;

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
        // What each loaded chunk's record owes comes with it, before its block entities are adopted.
        let homes: Vec<_> = world.borrow().homes.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        for ((dim, chunk), (removals, batches)) in homes {
            if world.borrow().loaded(&dim, [chunk[0] * 16, 0, chunk[1] * 16]) {
                server.native_obligations_loaded(&dim, chunk, removals, batches, BTreeSet::new());
            }
        }
        for ((dim, p), json) in saved {
            assert!(server.native_adopt(&dim, p, &json), "{json}");
        }
        Self { tick: 0, world, server, sh }
    }
    /// One tick: the server's, then its block writes.
    fn step(&mut self) {
        self.server.native_record_events(&mut self.sh);
        let confirmed=self.server.ready.bridge.last.clone();
        for (k,m) in &mut self.server.ready.world.machines {
            let mut w=self.world.borrow_mut();
            if w.model_entities.contains(k) && w.engine.get(k).map(|e|&e.0)!=confirmed.get(k) {
                w.model_entities.remove(k);
            }
            if !w.engine.contains_key(k) || w.model_entities.contains(k) {
                w.model_entities.insert(k.clone());
                let n=crate::native::block_slots(&m.block).len();
                let (slots,data)=Server::machine_output(m,&vec![None;n]);
                w.engine.insert(k.clone(),(slots,data));
            }
        }
        self.tick += 1;
        self.server.native_tick(&mut self.sh, self.tick);
        self.world.borrow_mut().apply_writes();
        // The owed entities whose cell is loaded come out (the glue's `ledger_tick`).
        let mut w = self.world.borrow_mut();
        let keys: Vec<_> = w.ledgers.keys().cloned().collect();
        for key in keys {
            let mut l = w.ledgers.remove(&key).unwrap();
            let due = l.take_owed_entities(|e| w.loaded(&key.0, e.at));
            for (e, nbt) in due {
                w.entities.push((key.0.clone(), EntitySpawn { kind: e.kind, pos: e.pos, nbt }));
            }
            w.ledgers.insert(key, l);
        }
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
        // Model-seeded legacy fixtures still have real BE slots now. Simulate
        // the engine spilling those slots on break, as the native host does.
        let key=(OW.to_string(),p);
        if self.world.borrow().model_entities.contains(&key) {
            let mut m=self.server.ready.world.machines[&key].clone();
            let n=crate::native::block_slots(&m.block).len();
            let slots=Server::machine_output(&mut m,&vec![None;n]).0;
            self.world.borrow_mut().engine.remove(&key);
            for s in slots.into_iter().flatten(){self.world.borrow_mut().dropped.push(format!("{OW} {} {} {} {} {}",p[0],p[1],p[2],s.id,s.count));}
            self.world.borrow_mut().model_entities.remove(&key);
        }
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
fn a_confirmed_loaded_removal_becomes_a_tombstone() {
    let mut g = game();
    let bat = [3, 70, 0];
    g.place(bat, "ntm:machine_battery[facing=east,honey_level=0]");
    g.run(2);
    g.world.borrow_mut().set(OW, bat, "minecraft:stone");
    g.run(LOAD_CHECK as usize + 1);
    assert!(g.ready().world.machines.is_empty(), "an accessible replacement confirms removal");
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
        // A new battery only takes (Java `redLow = 0`): it gives with a redstone signal.
        b.mode = crate::machine::BatteryMode::Output;
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
    assert_eq!(*g.ready().world.sickness, None, "nobody sick yet: no random kept");
    g.run(GRACE_TICKS as usize + 20 * 10);
    let dose = value(&g, KEY_DOSE).unwrap();
    assert!((9 * 64 * 350..=12 * 64 * 350).contains(&dose), "{dose}");
    // 300 RAD, nothing carried: every tick nausea one time in 300 and weakness one in 500, for 100 ticks each
    // (EntityEffectHandler:250-252), drawn from the world's sickness random.
    g.world.borrow_mut().carried.clear();
    g.ready().world.radiation.get_mut("p1").unwrap().millirad = 300_000;
    g.world.borrow_mut().effects.clear();
    g.run(6000);
    let effects = g.world.borrow().effects.clone();
    assert!(effects.iter().all(|e| (e.1 == "nausea" || e.1 == "weakness") && e.2 == 100 && e.3 == 0), "{effects:?}");
    let count = |name: &str| effects.iter().filter(|e| e.1 == name).count();
    assert!(count("nausea") > count("weakness") && count("weakness") > 0, "{effects:?}");
    assert!(g.ready().world.sickness.is_some(), "the random is kept once someone was sick");
    // 1000 RAD: death, and the dose starts again.
    g.world.borrow_mut().carried.insert("p1".into(), vec![("ntm:ingot_uranium".into(), 64)]);
    g.ready().world.radiation.get_mut("p1").unwrap().millirad = 999_999;
    g.run(40);
    assert!(g.world.borrow().hurts.iter().any(|h| h.2 == "ntm:radiation"), "{:?}", g.world.borrow().hurts);
    assert!(value(&g, KEY_DOSE).unwrap() < 999_999);
    // Creative: the rate, no dose.
    g.server.set_players(vec![player()]);
    let before = g.ready().world.radiation["p1"].millirad;
    g.run(100);
    assert_eq!(g.ready().world.radiation["p1"].millirad, before);
    assert_eq!(value(&g, KEY_RATE), Some(64 * 350));
}

/// Slice 5a review: the sickness random is the world seed's. The first sick tick draws a 200-RAD band's two rolls
/// from `java.util.Random(WORLD_SEED)`; another seed (0 among them) leaves another state.
#[test]
fn the_sickness_random_is_seeded_from_the_world_seed() {
    use crate::rbmk::jrandom::JavaRandom;
    let mut g = game();
    let mut p = player();
    p.creative = false;
    g.server.set_players(vec![p]);
    g.ready().world.radiation.insert("p1".into(), crate::radiation::Dose { millirad: 300_000 });
    g.run(1);
    let mut expected = JavaRandom::new(WORLD_SEED);
    expected.next_int(300);
    expected.next_int(500);
    let kept = (*g.ready().world.sickness).clone().expect("drawn once");
    assert_eq!(kept.rng().state(), expected.state());
    let mut other = JavaRandom::new(0);
    other.next_int(300);
    other.next_int(500);
    assert_ne!(kept.rng().state(), other.state());
}

/// Slice 5a review: a blast's effects are given in seconds (`visuals::Show::Effect`) and reach the host in ticks.
#[test]
fn a_blasts_effect_seconds_reach_the_host_as_ticks() {
    let mut g = game();
    let show = crate::blast::visuals::Show::Effect { player: "p1".into(), id: "nausea", seconds: 8, amplifier: 0 };
    super::bomb_server::show(&mut g.sh, OW, vec![show]);
    assert_eq!(g.world.borrow().effects, [("p1".to_string(), "nausea".to_string(), 160, 0)]);
}

/// Slice 5a review: creative players are skipped (`handleRadiationEffect` returns for them), and there is no joining
/// grace: a player sick from their first tick rolls at once.
#[test]
fn sickness_skips_creative_players_and_has_no_joining_grace() {
    let mut g = game();
    g.server.set_players(vec![player()]);
    g.ready().world.radiation.insert("p1".into(), crate::radiation::Dose { millirad: 900_000 });
    g.run(3000);
    assert!(g.world.borrow().effects.is_empty(), "creative: {:?}", g.world.borrow().effects);
    assert_eq!(*g.ready().world.sickness, None, "nothing drawn for a creative player");
    // A player who joins now, sick: their first tick is well inside radiation::GRACE_TICKS of joining, and the
    // sickness draws on it.
    let mut p = player();
    p.id = "p2".into();
    p.creative = false;
    g.server.set_players(vec![player(), p]);
    g.ready().world.radiation.insert("p2".into(), crate::radiation::Dose { millirad: 900_000 });
    g.run(1);
    assert!(g.ready().world.sickness.is_some(), "drawn on the first tick");
    g.run(2000);
    let effects = g.world.borrow().effects.clone();
    assert!(!effects.is_empty() && effects.iter().all(|e| e.0 == "p2"), "{effects:?}");
}

/// Slice 5a review P2: the sickness random has its own key. A tick whose only change is a sick player's rolls writes
/// that key alone, a few bytes; the world's key moves only with the rest of the world (its doses every second, the
/// clock).
#[test]
fn a_sick_tick_rewrites_only_the_sickness_key() {
    let mut g = game();
    let mut p = player();
    p.creative = false;
    g.server.set_players(vec![p]);
    g.ready().world.radiation.insert("p1".into(), crate::radiation::Dose { millirad: 300_000 });
    g.run(5);
    let (mut own, mut world_moved) = (0, 0);
    for _ in 0..60 {
        let before = g.world.borrow().kv.written;
        g.run(1);
        let w = g.world.borrow();
        let sickness = w.kv.map.get(crate::persist::SICKNESS_KEY).map_or(0, Vec::len) as u64;
        let world = w.kv.map.get(crate::persist::KEY).map_or(0, Vec::len) as u64;
        let written = w.kv.written - before;
        if written == sickness {
            own += 1;
        } else {
            // Anything more is the world's whole key, written because the rest of the world moved.
            assert!(written >= world, "{written} B written: neither the sickness key ({sickness} B) alone nor the world's");
            world_moved += 1;
        }
    }
    assert!(own >= 50 && world_moved <= 10, "{own} ticks with the sickness key alone, {world_moved} with the world");
}

/// Slice 5b review P1: a player's EntityEffectHandler runs after the tile entities of the tick (processPlayer, in the
/// network tick). At 1000.2 RAD on a decontaminator, the decontaminator's 0.5 RAD comes first: the player lives on
/// 999.7 RAD.
#[test]
fn a_decontaminator_takes_its_half_rad_before_the_death_check() {
    let mut g = game();
    let mut p = player();
    p.creative = false;
    g.server.set_players(vec![p]);
    g.place([0, 69, 0], crate::radiation::DECON);
    g.ready().world.radiation.insert("p1".into(), crate::radiation::Dose { millirad: 1_000_200 });
    g.run(1);
    assert!(!g.world.borrow().hurts.iter().any(|h| h.2 == crate::radiation::RADIATION_DAMAGE), "{:?}", g.world.borrow().hurts);
    assert_eq!(g.ready().world.radiation["p1"].millirad, 999_700);
}

/// Slice 5b review P2-4: one threshold. Just under 1000 RAD the player rolls the 800 band and lives; at 1000 they die
/// in that tick and nothing is drawn.
#[test]
fn just_under_a_thousand_rad_rolls_and_a_thousand_kills_without_drawing() {
    let mut g = game();
    let mut p = player();
    p.creative = false;
    g.server.set_players(vec![p]);
    g.ready().world.radiation.insert("p1".into(), crate::radiation::Dose { millirad: 999_999 });
    g.run(1);
    assert!(g.world.borrow().hurts.is_empty());
    let drawn = (*g.ready().world.sickness).clone().expect("the 800 band drew its five rolls");
    g.ready().world.radiation.insert("p1".into(), crate::radiation::Dose { millirad: 1_000_000 });
    g.run(1);
    assert_eq!(g.world.borrow().hurts.iter().filter(|h| h.2 == crate::radiation::RADIATION_DAMAGE).count(), 1);
    assert_eq!(g.ready().world.radiation["p1"].millirad, 0);
    assert_eq!(*g.ready().world.sickness, Some(drawn), "the death's tick drew nothing");
}

mod bomb_tests;
mod ash_tests;
mod chunk_radiation_tests;
mod combustion_tests;
mod solar_stirling_tests;
mod sawmill_tests;
mod fluid_tests;
mod crafter_tests;
mod review_m2_tests;
mod shredder_tests;
mod brick_furnace_tests;
mod furnaces_tests;
mod blastfurnace_tests;
mod foundry_tests;
mod slot_items_tests;
mod rbmk_tests;
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
