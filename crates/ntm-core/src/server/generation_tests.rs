#![allow(dead_code)]
use crate as ntm_core;
use ntm_core::{
    Pos, blockstate,
    kv::{Kv, MemKv},
    machine::{Data, Kind, Machine},
    persist::WorldState,
    server::{
        MachineCommit, PlayerInfo, Server, ServerHost,
        container_bridge::{Slots, Stack},
    },
    zirnox,
};
use std::collections::{BTreeMap, BTreeSet};
const D: &str = "minecraft:overworld";
const P: Pos = [0, 64, 0];
#[derive(Default)]
struct Host {
    kv: MemKv,
    blocks: BTreeMap<Pos, String>,
    slots: BTreeMap<Pos, Slots>,
    data: BTreeMap<Pos, Vec<i32>>,
    saved: BTreeMap<Pos, String>,
    refuse: bool,
    refuse_blocks: bool,
    unloaded: BTreeSet<Pos>,
    drops: Vec<Stack>,
    told: Vec<String>,
    commit_calls: usize,
    insert_on_commit: Option<(Pos, usize)>,
    refuse_at: Option<Pos>,
    change_cell: Option<(Pos, String)>,
    encoded: BTreeMap<Pos, Vec<String>>,
    retained: Vec<String>,
    supply_on_structural: Option<Stack>,
    structural_rejections: usize,
    crash_before_commit: bool,
    crash_after_commit: bool,
    refuse_preparation: bool,
    /// The ids of the effects the host's ledger already holds: it survives a "crash" of the server.
    ledger: BTreeSet<u64>,
    blocks_done: BTreeSet<(u64, Pos)>,
    pruned: usize,
}
impl Kv for Host {
    fn get(&mut self, k: &str) -> Option<Vec<u8>> {
        self.kv.get(k)
    }
    fn set(&mut self, k: &str, v: &[u8]) -> Result<(), String> {
        if self.refuse_preparation
            && k == crate::persist::KEY
            && serde_json::from_slice::<crate::persist::Kept>(v)
                .is_ok_and(|kept| !kept.prepared_retentions.is_empty())
        {
            return Err("refused retention journal".into());
        }
        self.kv.set(k, v)
    }
    fn delete(&mut self, k: &str) {
        self.kv.delete(k)
    }
}
impl ServerHost for Host {
    fn get_block(&mut self, _: &str, p: Pos) -> Option<String> {
        (!self.unloaded.contains(&p)).then(|| {
            self.blocks
                .get(&p)
                .cloned()
                .unwrap_or("minecraft:air".into())
        })
    }
    fn cell_generation(&mut self, _: &str, p: Pos) -> Option<u64> {
        let saved = self.saved.get(&p)?;
        serde_json::from_str::<Machine>(saved).ok().map(|m| m.generation)
    }
    fn effect_seen(&mut self, id: u64, _: &str, p: Pos) -> bool {
        self.blocks_done.contains(&(id, p))
    }
    fn effect_applied(&mut self, id: u64, cells: &[(String, Pos)]) {
        self.blocks_done.extend(cells.iter().map(|(_, p)| (id, *p)));
    }
    fn cell_snapshot(&mut self, d: &str, p: Pos) -> Option<super::CellSnapshot> {
        let state = self.get_block(d, p)?;
        let mut items = self.encoded.get(&p).cloned().unwrap_or_default();
        items.extend(
            self.slots
                .get(&p)
                .into_iter()
                .flatten()
                .flatten()
                .map(|i| format!("{{id:'{}',count:{}}}", i.id, i.count)),
        );
        Some(super::CellSnapshot {
            state,
            entity: (!items.is_empty()).then(|| format!("{items:?}")),
            items,
        })
    }
    fn drop_encoded_items(&mut self, _: &str, _: Pos, items: &[String]) -> bool {
        self.retained.extend_from_slice(items);
        true
    }
    fn commit_structural(
        &mut self,
        cells: &[super::CellWrite],
        drops: &[super::RetainedDrop],
        _: &[(String, Pos)],
    ) -> bool {
        let inserted = self.supply_on_structural.take();
        if let Some(item) = &inserted {
            self.slots.get_mut(&P).unwrap()[25] = Some(item.clone());
        }
        if cells
            .iter()
            .any(|c| self.cell_snapshot(&c.guard.dim, c.guard.pos).as_ref() != Some(&c.expected))
        {
            self.structural_rejections += 1;
            if inserted.is_some() {
                assert_eq!(blockstate::id_of(&self.blocks[&P]), zirnox::BLOCK);
                assert_eq!(self.slots[&P][25], inserted);
                assert!(self.retained.is_empty(), "refusal must not publish a drop");
            }
            return false;
        }
        // One fake host batch, with preflight before all mutation/drop release.
        if self.refuse_blocks || cells.iter().any(|c| self.unloaded.contains(&c.guard.pos)) {
            return false;
        }
        let blocks: Vec<_> = cells
            .iter()
            .map(|c| (c.guard.pos, c.desired.clone()))
            .collect();
        if self.set_blocks(D, &blocks).is_err() {
            return false;
        }
        for d in drops {
            self.retained.extend_from_slice(&d.items);
        }
        true
    }
    fn machine_slots(&mut self, _: &str, p: Pos) -> Option<Slots> {
        if self.unloaded.contains(&p) {
            None
        } else {
            self.slots.get(&p).cloned()
        }
    }
    fn machine_data(&mut self, _: &str, p: Pos) -> Option<Vec<i32>> {
        self.data.get(&p).cloned()
    }
    fn commit_machine(&mut self, c: &MachineCommit) -> bool {
        self.commit_machines(std::slice::from_ref(c))
    }
    fn commit_machines(&mut self, cs: &[MachineCommit]) -> bool {
        self.commit_calls += 1;
        if self.crash_before_commit && cs.iter().any(|c| !c.cells.is_empty()) {
            panic!("simulated crash before atomic commit");
        }
        if let Some((p, i)) = self.insert_on_commit.take() {
            self.slots.get_mut(&p).unwrap()[i].as_mut().unwrap().count += 1;
        }
        if let Some((p, state)) = self.change_cell.take() {
            self.blocks.insert(p, state);
        }
        if cs
            .iter()
            .flat_map(|c| &c.cells)
            .any(|w| self.cell_snapshot(&w.guard.dim, w.guard.pos).as_ref() != Some(&w.expected))
        {
            return false;
        }
        if self.refuse_blocks
            && cs
                .iter()
                .flat_map(|c| &c.cells)
                .any(|w| w.desired != w.expected.state)
        {
            return false;
        }
        if self.refuse
            || cs.iter().any(|c| self.refuse_at == Some(c.pos))
            || cs.iter().any(|c| {
                self.unloaded.contains(&c.pos)
                    || self.slots.get(&c.pos).cloned().unwrap_or_default() != c.expected
            })
        {
            return false;
        }
        for w in cs.iter().flat_map(|c| &c.cells) {
            self.blocks.insert(w.guard.pos, w.desired.clone());
            if blockstate::id_of(&w.expected.state) != blockstate::id_of(&w.desired) {
                self.encoded.remove(&w.guard.pos);
                self.slots.remove(&w.guard.pos);
                self.saved.remove(&w.guard.pos);
            }
        }
        for w in cs.iter().flat_map(|c| &c.cells) {
            if blockstate::id_of(&w.expected.state) != blockstate::id_of(&w.desired) {
                self.retained.extend_from_slice(&w.expected.items);
            }
        }
        for c in cs {
            self.slots.insert(c.pos, c.desired.clone());
            self.data.insert(c.pos, c.data.clone());
            self.saved.insert(c.pos, c.json.clone());
        }
        if self.crash_after_commit && cs.iter().any(|c| !c.cells.is_empty()) {
            panic!("simulated crash after atomic commit");
        }
        true
    }
    fn set_blocks(&mut self, _: &str, b: &[(Pos, String)]) -> Result<(), String> {
        if self.refuse_blocks || b.iter().any(|(p, _)| self.unloaded.contains(p)) {
            return Err("refused".into());
        }
        for (p, s) in b {
            let old = self
                .blocks
                .get(p)
                .map(|s| blockstate::id_of(s).to_string())
                .unwrap_or_default();
            if old != blockstate::id_of(s) {
                self.slots.remove(p);
                self.saved.remove(p);
                self.data.remove(p);
                if let Some(c) = ntm_core::native::container(blockstate::id_of(s)) {
                    self.slots.insert(*p, vec![None; c.slots]);
                }
            }
            self.blocks.insert(*p, s.clone());
        }
        Ok(())
    }
    fn tell(&mut self, _: &str, t: &str) {
        self.told.push(t.into());
    }
    fn log(&mut self, _: &str) {}
    fn redstone_many(&mut self, _: &str, c: &[Pos]) -> Vec<Option<u8>> {
        vec![Some(0); c.len()]
    }
    fn drop_items(&mut self, _: &str, _: Pos, i: &[(String, u32)]) -> bool {
        self.drops
            .extend(i.iter().map(|(id, n)| Stack::new(id, *n)));
        true
    }
    fn apply_items(&mut self, id: u64, d: &str, p: Pos, i: &[(String, u32)]) -> bool {
        // As the chunk ledger does: an effect already known adds nothing.
        !self.ledger.insert(id) || self.drop_items(d, p, i)
    }
    fn ledger_prune(&mut self, live: &BTreeSet<u64>) {
        let before = self.ledger.len();
        self.ledger.retain(|id| live.contains(id));
        self.pruned += before - self.ledger.len();
    }
    fn zirnox_explode(&mut self, _: &str, _: [f64; 3]) -> bool {
        true
    }
    fn pile_explode(&mut self, _: &str, _: [f64; 3]) -> bool {
        true
    }
}
fn install(h: &mut Host, block: &str, n: usize) -> Server {
    h.blocks.insert(P, block.into());
    h.slots.insert(P, vec![None; n]);
    let mut s = Server::native(h, 0);
    s.native_block_changed(D, P, "minecraft:air", block);
    s.native_tick(h, 1);
    s
}

#[test]
fn a_waiting_structural_effect_does_not_release_contents_of_a_source_consumed_elsewhere() {
    use super::{CellGuard, CommittedEffects, WorldEffect};
    let mut h = Host::default();
    let mut s = Server::native(&mut h, 0);
    let other = [16, 64, 0];
    h.blocks.insert(P, "minecraft:chest".into());
    h.blocks.insert(other, "minecraft:stone".into());
    h.encoded.insert(P, vec!["{id:'minecraft:diamond',count:7}".into()]);
    s.ready.world.generations.insert((D.into(), P), 0);
    h.unloaded.insert(other);
    s.ready.world.machine_effects.push_back(CommittedEffects {
        effects: std::collections::VecDeque::from([WorldEffect::Blocks {
            dim: D.into(),
            blocks: vec![(P, "minecraft:air".into()), (other, "minecraft:air".into())],
            expected: vec![
                CellGuard { dim: D.into(), pos: P, generation: 0, state: "minecraft:chest".into() },
                CellGuard { dim: D.into(), pos: other, generation: 0, state: "minecraft:stone".into() },
            ],
            retained: Vec::new(),
        }]),
        ..Default::default()
    });
    s.flush_machine_effects(&mut h);
    assert!(h.retained.is_empty(), "the second chunk keeps the whole effect waiting");
    // The player breaks the chest while the other chunk is absent: those seven diamonds are already theirs.
    s.native_block_changed(D, P, "minecraft:chest", "minecraft:air");
    h.blocks.insert(P, "minecraft:air".into());
    h.encoded.remove(&P);
    h.unloaded.clear();
    s.flush_machine_effects(&mut h);
    assert!(h.retained.is_empty(), "the stale snapshot must not emit the same diamonds again");
    assert_eq!(h.blocks[&other], "minecraft:air", "the still-valid part completes");
}

fn z(h: &Host) -> ntm_core::zirnox::Zirnox {
    let m: Machine = serde_json::from_str(&h.saved[&P]).unwrap();
    let Data::Zirnox(z) = m.data else { panic!() };
    *z
}
fn conservation() {
    for (input, n, withdraw, water) in [
        ("minecraft:water_bucket", 1, true, 0),
        ("ntm:fluid_barrel_full__water", 2, false, 16000),
    ] {
        let mut h = Host::default();
        let mut s = install(&mut h, zirnox::BLOCK, 28);
        h.slots.get_mut(&P).unwrap()[25] = Some(Stack::new(input, n));
        h.refuse = true;
        s.native_tick(&mut h, 2);
        assert_eq!(z(&h).water.mb, 0);
        assert!(h.slots[&P][27].is_none());
        if withdraw {
            h.slots.get_mut(&P).unwrap()[25] = None;
        } else {
            h.slots.get_mut(&P).unwrap()[25].as_mut().unwrap().count += 1;
        }
        h.refuse = false;
        s.native_tick(&mut h, 3);
        assert_eq!(z(&h).water.mb, water);
        println!(
            "conservation {input} withdrawn={withdraw}: water={}, slots={:?}",
            water,
            &h.slots[&P][25..28]
        );
    }
    let mut h = Host::default();
    let mut s = install(&mut h, "ntm:machine_diesel", 3);
    h.slots.get_mut(&P).unwrap()[0] = Some(Stack::new("ntm:fluid_barrel_full__diesel", 1));
    h.refuse = true;
    s.native_tick(&mut h, 2);
    h.slots.get_mut(&P).unwrap()[0] = None;
    h.refuse = false;
    s.native_tick(&mut h, 3);
    let m: Machine = serde_json::from_str(&h.saved[&P]).unwrap();
    let Data::Combustion(c) = m.data else {
        panic!()
    };
    let ntm_core::combustion::Combustion::Diesel(d) = *c else {
        panic!()
    };
    assert_eq!(d.tank.mb, 0);
    println!(
        "diesel withdraw: tank={}, no empty={}",
        d.tank.mb,
        h.slots[&P][1].is_none()
    );
    let mut h = Host::default();
    let mut s = install(&mut h, zirnox::BLOCK, 28);
    h.slots.get_mut(&P).unwrap()[25] = Some(Stack::new("ntm:fluid_barrel_full__water", 2));
    h.insert_on_commit = Some((P, 25));
    s.native_tick(&mut h, 2);
    assert_eq!(z(&h).water.mb, 0);
    assert!(h.slots[&P][27].is_none());
    s.native_tick(&mut h, 3);
    assert_eq!(z(&h).water.mb, 16000);
    assert_eq!(h.slots[&P][25].as_ref().unwrap().count, 2);
    println!("increment during CAS: no product until next healthy step");
}
fn bomb() -> (Host, Server, PlayerInfo) {
    let mut h = Host::default();
    let mut s = install(&mut h, "ntm:nuke_boy", 5);
    h.slots.get_mut(&P).unwrap().clone_from(
        &ntm_core::bomb::bomb_type("ntm:nuke_boy")
            .unwrap()
            .slots
            .iter()
            .map(|x| Some(Stack::new(x.item, 1)))
            .collect(),
    );
    s.native_tick(&mut h, 2);
    let who = PlayerInfo {
        id: "p".into(),
        name: "p".into(),
        dim: D.into(),
        pos: [0., 64., 4.],
        creative: true,
    };
    s.set_players(vec![who.clone()]);
    s.bomb_command(&mut h, Some(&who), "config mundo raiomax 4")
        .unwrap();
    assert!(s.detonator(&mut h, &who, Some(P), true).contains("ligado"));
    assert!(s.detonator(&mut h, &who, None, false).contains("solicitad"));
    (h, s, who)
}
fn replaced_bomb() {
    let (mut h, mut s, _) = bomb();
    let parts = h.slots[&P].clone();
    h.blocks.insert(P, "ntm:nuke_boy".into());
    h.slots.insert(P, parts);
    h.saved.remove(&P);
    s.native_block_changed(D, P, "ntm:nuke_boy", "minecraft:air");
    s.native_block_changed(D, P, "minecraft:air", "ntm:nuke_boy");
    s.native_tick(&mut h, 3);
    s.native_tick(&mut h, 4);
    let w = WorldState::load(&mut h);
    assert_eq!(w.bombs.next, 0, "R10: old request must not fire a new BE");
    println!(
        "requested bomb broken+replaced: detonations={}, actual block={}, messages={:?}",
        w.bombs.next, h.blocks[&P], h.told
    );
}
fn delayed_block_effect() {
    let (mut h, mut s, _) = bomb();
    h.refuse_blocks = true;
    s.native_tick(&mut h, 3);
    println!(
        "pending bomb clear: effects={}",
        WorldState::load(&mut h).machine_effects.len()
    );
    h.blocks.insert(P, zirnox::BLOCK.into());
    let mut slots = vec![None; 28];
    slots[25] = Some(Stack::new("minecraft:water_bucket", 1));
    h.slots.insert(P, slots);
    h.saved.remove(&P);
    s.native_block_changed(D, P, "ntm:nuke_boy", "minecraft:air");
    s.native_block_changed(D, P, "minecraft:air", zirnox::BLOCK);
    h.refuse_blocks = false;
    s.native_tick(&mut h, 4);
    let pending = WorldState::load(&mut h).machine_effects.len();
    println!(
        "replacement while block effect pending: block={}, slot_exists={}, effects={}",
        h.blocks[&P],
        h.slots.contains_key(&P),
        pending
    );
}

fn missing_core_tombstone() {
    use ntm_core::pile::{Core, Pile, Rod, Shape};
    let mut h = Host::default();
    let mut s = Server::native(&mut h, 0);
    let shape = Shape {
        at: P,
        inward: [0, 0, 1],
        up: 2,
        down: 2,
        left: 2,
        right: 2,
        depth: 5,
    };
    let mut core = Core::new(shape);
    core.drill([-1, 64, 0], [0, 0, 1]).unwrap();
    core.channels[0].load(Rod::of("ntm:pile_rod__zr", 0.).unwrap());
    for (at, role) in core.cells() {
        let block = format!("ntm:pile_block__{role}");
        let mut m = Machine::new(Kind::Pile, &block, Some("north".into()));
        m.data = Data::Pile(Box::new(if at == P {
            Pile::Core(Some(core.clone()))
        } else {
            Pile::Member(Some(P))
        }));
        h.blocks.insert(at, block);
        h.slots.insert(at, Vec::new());
        assert!(s.native_adopt(D, at, &serde_json::to_string(&m).unwrap()));
    }
    s.native_tick(&mut h, 1);
    assert!(h.saved.contains_key(&P));
    // The world proves core absence, but a different group member is inaccessible.
    h.blocks.insert(P, "minecraft:air".into());
    h.saved.remove(&P);
    h.slots.remove(&P);
    h.unloaded.insert([1, 65, 3]);
    s.native_tick(&mut h, 2);
    s.native_world_saved(&mut h, true);
    let kept = WorldState::load(&mut h);
    println!(
        "missing loaded core + inaccessible member: tombstones={}, pending_effects={}, runtime_has_core={}",
        kept.removals.len(),
        kept.machine_effects.len(),
        s.native_has(D, P)
    );
    assert_eq!(
        kept.removals.len(),
        1,
        "R9: absence must be durable before group availability"
    );
    let mut reopened = Server::native(&mut h, 3);
    for (at, j) in h.saved.clone() {
        reopened.native_adopt(D, at, &j);
    }
    h.unloaded.clear();
    for t in 4..8 {
        reopened.native_tick(&mut h, t);
    }
    println!(
        "clean reopen after absent core: core={}, rods_ejected={}",
        reopened.native_has(D, P),
        h.drops.len()
    );
}

fn deferred_pile_edit_overwrites_replacement() {
    use ntm_core::pile::{Core, Pile, Rod, Shape};
    let mut h = Host::default();
    let mut s = Server::native(&mut h, 0);
    let shape = Shape {
        at: P,
        inward: [0, 0, 1],
        up: 2,
        down: 2,
        left: 2,
        right: 2,
        depth: 5,
    };
    let mut core = Core::new(shape);
    core.drill([-1, 64, 0], [0, 0, 1]).unwrap();
    core.channels[0].load(Rod::of("ntm:pile_rod__zr", 0.).unwrap());
    for (at, role) in core.cells() {
        let block = format!("ntm:pile_block__{role}");
        let mut m = Machine::new(Kind::Pile, &block, Some("north".into()));
        m.data = Data::Pile(Box::new(if at == P {
            Pile::Core(Some(core.clone()))
        } else {
            Pile::Member(Some(P))
        }));
        h.blocks.insert(at, block);
        h.slots.insert(at, Vec::new());
        assert!(s.native_adopt(D, at, &serde_json::to_string(&m).unwrap()));
    }
    s.native_tick(&mut h, 1);
    h.refuse_blocks = true;
    let at = [-1, 64, 0];
    let old = h.blocks[&at].clone();
    let action = s.native_pile_use(
        &mut h,
        D,
        at,
        [0, 0, -1],
        Some(&Stack::new("ntm:hand_drill", 1)),
        0.,
        "p",
    );
    assert!(action.is_some());
    assert_eq!(WorldState::load(&mut h).machine_effects.len(), 1);
    h.blocks.insert(at, zirnox::BLOCK.into());
    let mut slots = vec![None; 28];
    slots[25] = Some(Stack::new("minecraft:water_bucket", 1));
    h.slots.insert(at, slots);
    h.saved.remove(&at);
    s.native_block_changed(D, at, &old, "minecraft:air");
    s.native_block_changed(D, at, "minecraft:air", zirnox::BLOCK);
    h.refuse_blocks = false;
    s.native_tick(&mut h, 2);
    println!(
        "delayed pile edit (NO EXPLOSION): replacement block={}, full bucket survives={}, action={:?}, drops={:?}",
        h.blocks[&at],
        h.slots
            .get(&at)
            .is_some_and(|v| v.iter().flatten().any(|x| x.id == "minecraft:water_bucket")),
        action,
        h.drops
    );
    assert_eq!(blockstate::id_of(&h.blocks[&at]), zirnox::BLOCK);
    assert!(
        h.slots
            .get(&at)
            .is_some_and(|v| v.iter().flatten().any(|x| x.id == "minecraft:water_bucket"))
    );
}

fn network_does_not_publish_refused_input() {
    use ntm_core::fluidbox::Mode;
    let mut h = Host::default();
    let mut s = Server::native(&mut h, 0);
    let target = [1, 64, 0];
    for (at, mode) in [(P, Mode::Output), (target, Mode::Input)] {
        let mut m = Machine::new(Kind::FluidStore, "ntm:barrel_steel", Some("north".into()));
        let Data::FluidStore(b) = &mut m.data else {
            panic!()
        };
        b.mode = mode;
        b.tank.fluid = "water".into();
        let json = serde_json::to_string(&m).unwrap();
        h.blocks.insert(at, "ntm:barrel_steel".into());
        h.slots.insert(at, vec![None; 6]);
        h.saved.insert(at, json.clone());
        s.native_adopt(D, at, &json);
    }
    h.slots.get_mut(&P).unwrap()[2] = Some(Stack::new("ntm:fluid_barrel_full__water", 1));
    h.refuse_at = Some(P);
    for t in 1..15 {
        s.native_tick(&mut h, t);
    }
    let total = |h: &Host| {
        h.saved
            .values()
            .map(|j| {
                let m: Machine = serde_json::from_str(j).unwrap();
                if let Data::FluidStore(b) = m.data {
                    b.tank.mb
                } else {
                    0
                }
            })
            .sum::<u32>()
    };
    assert_eq!(total(&h), 0);
    assert_eq!(h.slots[&P][2].as_ref().unwrap().count, 1);
    assert!(h.slots[&P][3].is_none());
    h.refuse_at = None;
    for t in 15..30 {
        s.native_tick(&mut h, t);
    }
    assert_eq!(total(&h), 16000);
    assert!(h.slots[&P][2].is_none());
    let m: Machine = serde_json::from_str(&h.saved[&target]).unwrap();
    let Data::FluidStore(b) = m.data else {
        panic!()
    };
    assert!(b.tank.mb > 0);
    println!(
        "fluid network: rejected source emitted 0; healthy source+sink total={}, sink={}",
        total(&h),
        b.tank.mb
    );
}

#[test]
fn old_cell_retry_preserves_a_replacement_and_ejects_the_old_rod_once() {
    deferred_pile_edit_overwrites_replacement();
}
#[test]
fn positive_absence_is_durable_even_with_an_inaccessible_group_member() {
    missing_core_tombstone();
}
#[test]
fn a_break_cancels_the_old_generation_detonation() {
    replaced_bomb();
}
#[test]
fn existing_slot_and_network_conservation_remains_atomic() {
    conservation();
    network_does_not_publish_refused_input();
}

#[test]
fn pending_nuke_and_fuse_requests_do_not_survive_a_replacement_even_after_refusal() {
    for block in ["ntm:nuke_boy", "ntm:dynamite"] {
        for refused in [false, true] {
            let mut h = Host::default();
            let mut s = install(
                &mut h,
                block,
                ntm_core::bomb::bomb_type(block).unwrap().slots.len(),
            );
            let parts: Slots = ntm_core::bomb::bomb_type(block)
                .unwrap()
                .slots
                .iter()
                .map(|x| Some(Stack::new(x.item, 1)))
                .collect();
            h.slots.insert(P, parts.clone());
            let p = PlayerInfo {
                id: "p".into(),
                name: "p".into(),
                dim: D.into(),
                pos: [0., 64., 4.],
                creative: true,
            };
            s.set_players(vec![p.clone()]);
            s.detonator(&mut h, &p, Some(P), true);
            s.detonator(&mut h, &p, None, false);
            if refused {
                h.refuse = true;
                s.native_tick(&mut h, 2);
                h.refuse = false;
            }
            h.saved.remove(&P);
            h.slots.insert(P, parts.clone());
            s.native_block_changed(D, P, block, "minecraft:air");
            s.native_block_changed(D, P, "minecraft:air", block);
            for t in 3..7 {
                s.native_tick(&mut h, t);
            }
            assert_eq!(s.ready.world.bombs.next, 0);
            assert_eq!(h.slots[&P], parts);
            let Data::Bomb(b) = &s.ready.world.machines[&(D.into(), P)].data else {
                panic!()
            };
            assert_eq!(b.fuse, None);
            assert!(s.ready.detonations.is_empty());
            assert!(
                !h.told
                    .iter()
                    .any(|t| t == "Detonada." || t == "Pavio aceso.")
            );
        }
    }
}

#[test]
fn a_menu_intention_cannot_toggle_a_new_generation() {
    let mut h = Host::default();
    let mut s = install(&mut h, zirnox::BLOCK, 28);
    let generation = s.ready.world.machines[&(D.into(), P)].generation;
    assert!(s.native_menu_button("p", D, P, 0, generation));
    h.saved.remove(&P);
    h.slots.insert(P, vec![None; 28]);
    s.native_block_changed(D, P, zirnox::BLOCK, "minecraft:air");
    s.native_block_changed(D, P, "minecraft:air", zirnox::BLOCK);
    for t in 2..5 {
        s.native_tick(&mut h, t);
    }
    assert!(!z(&h).on);
    assert!(s.ready.pending_buttons.is_empty());
    assert!(!s.native_menu_button("p", D, P, 0, generation));
}

#[test]
fn a_committed_bomb_still_explodes_once_after_replacement_before_admission() {
    let (mut h, mut s, _) = bomb();
    s.local_steps(&mut h);
    s.flush_machine_effects(&mut h);
    assert_eq!(s.ready.world.bombs.next, 1);
    assert_eq!(s.ready.world.bombs.queue.len(), 1);
    assert!(!s.ready.world.bombs.queue[0].started);
    h.blocks.insert(P, zirnox::BLOCK.into());
    h.slots.insert(P, vec![None; 28]);
    h.saved.remove(&P);
    s.native_block_changed(D, P, "ntm:nuke_boy", "minecraft:air");
    s.native_block_changed(D, P, "minecraft:air", zirnox::BLOCK);
    s.ready.world.bombs.cfg.as_mut().unwrap().fuel = 200;
    for t in 4..9 {
        s.native_tick(&mut h, t);
    }
    assert_eq!(s.ready.world.bombs.next, 1);
    assert!(s.ready.world.bombs.queue.is_empty());
    assert_eq!(
        s.blasts.log.len(),
        1,
        "physical explosion of the consumed generation, exactly once"
    );
}

#[test]
fn a_refused_zirnox_ruin_write_preserves_a_new_zirnox_inventory() {
    let mut h = Host::default();
    let mut s = install(&mut h, zirnox::BLOCK, 28);
    if let Data::Zirnox(state) = &mut s.ready.world.machines.get_mut(&(D.into(), P)).unwrap().data {
        state.heat = zirnox::MAX + 100;
    }
    h.refuse_blocks = true;
    s.native_tick(&mut h, 2);
    assert!(!s.ready.world.machine_effects.is_empty());
    h.blocks.insert(P, zirnox::BLOCK.into());
    h.slots.insert(P, vec![None; 28]);
    h.slots.get_mut(&P).unwrap()[25] = Some(Stack::new("minecraft:water_bucket", 1));
    h.saved.remove(&P);
    s.native_block_changed(D, P, zirnox::BLOCK, "minecraft:air");
    s.native_block_changed(D, P, "minecraft:air", zirnox::BLOCK);
    h.refuse_blocks = false;
    for t in 3..7 {
        s.native_tick(&mut h, t);
    }
    assert_eq!(blockstate::id_of(&h.blocks[&P]), zirnox::BLOCK);
    let full = h.slots[&P][25].as_ref().map_or(0, |x| x.count);
    assert_eq!(full * 1000 + z(&h).water.mb, 1000);
    assert!(!z(&h).destroyed);
}

/// The break reached the storage but not the chunk, and nothing was released for it: the machine of the chunk is the
/// world's again, with the generation it has.
#[test]
fn a_break_that_never_reached_the_chunk_leaves_the_machine_of_the_chunk_alive() {
    let mut h = Host::default();
    let mut s = install(&mut h, zirnox::BLOCK, 28);
    let key = (D.to_string(), P);
    let saved = serde_json::to_string(&s.ready.world.machines[&key]).unwrap();
    let generation = s.ready.world.machines[&key].generation;
    s.ready.world.machines.remove(&key);
    s.ready.world.generations.insert(key.clone(), generation + 2);
    assert!(s.native_adopt(D, P, &saved));
    assert!(s.native_has_machine(D, P), "alive");
    assert_eq!(s.ready.world.generations[&key], generation, "the generation of the chunk");
}

/// The break reached the storage with its receipt (the contents were released), not the chunk: the block goes.
#[test]
fn a_machine_a_receipt_consumed_is_removed_from_an_older_chunk() {
    use super::{CommittedEffects, WorldEffect};
    let mut h = Host::default();
    let mut s = install(&mut h, zirnox::BLOCK, 28);
    let key = (D.to_string(), P);
    let guard = super::CellGuard::machine(&key, &s.ready.world.machines[&key]);
    let saved = serde_json::to_string(&s.ready.world.machines[&key]).unwrap();
    s.ready.world.machines.remove(&key);
    s.ready.world.generations.insert(key.clone(), 1);
    s.ready.world.applied_machine_effects.push_back(CommittedEffects {
        serial: 4,
        origins: vec![guard],
        owners: vec![key.clone()],
        effects: std::collections::VecDeque::from([WorldEffect::Items { dim: D.into(), pos: P, items: vec![("ntm:pile_rod__zr".into(), 1)] }]),
        ..Default::default()
    });
    assert!(s.native_adopt(D, P, &saved), "told as handled");
    assert!(!s.native_has_machine(D, P));
    s.native_tick(&mut h, 2);
    assert_eq!(blockstate::id_of(&h.blocks[&P]), "minecraft:air", "the break is carried through");
    assert!(h.retained.is_empty(), "and releases nothing: the receipt did that");
}

#[test]
fn an_older_chunk_does_not_bring_back_a_machine_a_receipt_consumed() {
    use super::{CommittedEffects, WorldEffect};
    let mut h = Host::default();
    let mut s = install(&mut h, zirnox::BLOCK, 28);
    let key = (D.to_string(), P);
    let guard = super::CellGuard::machine(&key, &s.ready.world.machines[&key]);
    let saved = serde_json::to_string(&s.ready.world.machines[&key]).unwrap();
    s.ready.world.machines.remove(&key);
    // The receipt was replayed and kept for the next confirmed save: it has left the pending list.
    s.ready.world.applied_machine_effects.push_back(CommittedEffects {
        origins: vec![guard.clone()],
        owners: vec![key.clone()],
        effects: std::collections::VecDeque::from([WorldEffect::Blocks { retained: Vec::new(), dim: D.into(), blocks: vec![(P, zirnox::DESTROYED.into())], expected: vec![guard] }]),
        ..Default::default()
    });
    assert!(s.native_adopt(D, P, &saved), "told as handled");
    assert!(!s.ready.world.machines.contains_key(&key), "but the stale machine is not back");
}

/// An older chunk with the very machine a receipt consumed: the effect is due there, whatever the storage's
/// generations say; a chunk whose record already holds the effect is left alone.
fn old_chunk_replay(seen: bool) -> (Host, Server) {
    use super::{CommittedEffects, WorldEffect};
    let mut h = Host::default();
    let mut s = install(&mut h, zirnox::BLOCK, 28);
    let key = (D.to_string(), P);
    let old = super::CellGuard::machine(&key, &s.ready.world.machines[&key]);
    h.saved.insert(P, serde_json::to_string(&s.ready.world.machines[&key]).unwrap());
    s.ready.world.machines.remove(&key);
    s.ready.world.effect_serial = 7;
    s.ready.world.generations.insert(key, old.generation + 3);
    s.ready.world.applied_machine_effects.push_back(CommittedEffects {
        serial: 7,
        origins: vec![old.clone()],
        owners: vec![(D.into(), P)],
        effects: std::collections::VecDeque::from([WorldEffect::Blocks { retained: Vec::new(), dim: D.into(), blocks: vec![(P, zirnox::DESTROYED.into())], expected: vec![old] }]),
        ..Default::default()
    });
    if seen {
        h.blocks_done.insert((ntm_core::ledger::effect_id(7, 0), P));
    }
    h.set(ntm_core::persist::KEY, &s.ready.world.kept(0)).unwrap();
    let mut reopened = Server::native(&mut h, 1);
    assert!(reopened.native_adopt(D, P, &h.saved[&P].clone()));
    reopened.native_tick(&mut h, 2);
    reopened.native_tick(&mut h, 3);
    (h, reopened)
}

#[test]
fn a_chunk_older_than_the_storage_gets_the_effect_it_missed() {
    let (h, _) = old_chunk_replay(false);
    assert_eq!(blockstate::id_of(&h.blocks[&P]), zirnox::DESTROYED, "the missed effect was applied");
    assert!(h.blocks_done.contains(&(ntm_core::ledger::effect_id(7, 0), P)), "and the chunk now records it");
}

/// A machine that outlives a receipt's step comes back in the state the journal gives it when its chunk is older.
fn journal_adopt(chunk_rev: u64, journal_rev: u64) -> Option<String> {
    use super::Survivor;
    let mut h = Host::default();
    let s = install(&mut h, zirnox::BLOCK, 28);
    let key = (D.to_string(), P);
    let mut chunk = s.ready.world.machines[&key].clone();
    chunk.rev = chunk_rev;
    chunk.facing = Some("north".into());
    let mut journaled = chunk.clone();
    journaled.rev = journal_rev;
    journaled.facing = Some("south".into());
    let guard = super::CellGuard::machine(&key, &chunk);
    let mut s = s;
    s.ready.world.machines.remove(&key);
    let _ = guard;
    s.ready.world.survivors.insert(key.clone(), Survivor { dim: D.into(), pos: P, generation: chunk.generation, rev: journal_rev, json: serde_json::to_string(&journaled).unwrap() });
    assert!(s.native_adopt(D, P, &serde_json::to_string(&chunk).unwrap()));
    s.ready.world.machines.get(&key).and_then(|m| m.facing.clone())
}

#[test]
fn a_surviving_machine_in_an_older_chunk_gets_the_state_the_journal_gives_it() {
    assert_eq!(journal_adopt(0, 1).as_deref(), Some("south"), "the chunk is older: the journal's state");
    assert_eq!(journal_adopt(1, 1).as_deref(), Some("north"), "the same revision: nothing to do");
    assert_eq!(journal_adopt(2, 1).as_deref(), Some("north"), "a chunk newer than the journal is kept");
}

#[test]
fn a_chunk_that_holds_the_effect_is_not_given_it_twice() {
    let (h, _) = old_chunk_replay(true);
    assert_eq!(blockstate::id_of(&h.blocks[&P]), zirnox::BLOCK, "untouched");
}

#[test]
fn receipt_replay_checks_its_cell_generation_and_keeps_old_drops_separate() {
    use super::{CellGuard, CommittedEffects, WorldEffect};
    let mut h = Host::default();
    let mut s = install(&mut h, zirnox::BLOCK, 28);
    let old = super::CellGuard::machine(&(D.into(), P), &s.ready.world.machines[&(D.into(), P)]);
    s.ready
        .world
        .applied_machine_effects
        .push_back(CommittedEffects {
            origins: vec![old.clone()],
            owners: vec![(D.into(), P)],
            tombstones: vec![],
            effects: std::collections::VecDeque::from([WorldEffect::Blocks {
                retained: Vec::new(),
                dim: D.into(),
                blocks: vec![(P, zirnox::DESTROYED.into())],
                expected: vec![old],
            }]),
        ..Default::default()});
    s.ready.world.generations.insert((D.into(), P), 10);
    let mut fresh = Machine::new(Kind::Zirnox, zirnox::BLOCK, None);
    fresh.generation = 10;
    h.saved.insert(P, serde_json::to_string(&fresh).unwrap());
    h.slots.get_mut(&P).unwrap()[25] = Some(Stack::new("minecraft:water_bucket", 1));
    s.ready.world.machine_effects.push_back(CommittedEffects {
        origins: vec![CellGuard {
            dim: D.into(),
            pos: P,
            generation: 1,
            state: zirnox::BLOCK.into(),
        }],
        owners: vec![(D.into(), P)],
        tombstones: vec![],
        effects: std::collections::VecDeque::from([WorldEffect::Items {
            dim: D.into(),
            pos: P,
            items: vec![("ntm:pile_rod__zr".into(), 1)],
        }]),
    ..Default::default()});
    h.set(ntm_core::persist::KEY, &s.ready.world.kept(0))
        .unwrap();
    let mut reopened = Server::native(&mut h, 1);
    assert!(reopened.native_adopt(D, P, &h.saved[&P]));
    reopened.native_tick(&mut h, 2);
    assert_eq!(blockstate::id_of(&h.blocks[&P]), zirnox::BLOCK);
    assert_eq!(z(&h).water.mb, 1000);
    assert_eq!(h.drops, vec![Stack::new("ntm:pile_rod__zr", 1)]);
    reopened.native_world_saved(&mut h, true);
    let mut again = Server::native(&mut h, 3);
    again.native_tick(&mut h, 4);
    assert_eq!(h.drops.len(), 1);
}

#[test]
fn an_inaccessible_cell_is_not_a_removal_and_exhausted_generations_freeze() {
    let mut h = Host::default();
    let mut s = install(&mut h, zirnox::BLOCK, 28);
    h.unloaded.insert(P);
    s.native_tick(&mut h, 2);
    assert!(s.ready.world.removals.is_empty());
    h.unloaded.clear();
    s.ready.world.generations.insert((D.into(), P), u64::MAX);
    s.ready
        .world
        .machines
        .get_mut(&(D.into(), P))
        .unwrap()
        .generation = u64::MAX;
    h.slots.get_mut(&P).unwrap()[25] = Some(Stack::new("minecraft:water_bucket", 1));
    s.native_tick(&mut h, 3);
    assert_eq!(h.slots[&P][25].as_ref().unwrap().count, 1);
    assert_eq!(z(&h).water.mb, 0);
}

fn resource_machine(block: &str) -> (Host, Server, Pos) {
    let mut h = Host::default();
    let mut s = install(&mut h, block, crate::native::block_slots(block).len());
    let target = [P[0], P[1] - 1, P[2]];
    let m = s.ready.world.machines.get_mut(&(D.into(), P)).unwrap();
    match &mut m.data {
        Data::Excavator(e) => {
            e.enabled = true;
            e.power = crate::excavator::MAX_POWER;
            e.worked = 100;
            h.slots.get_mut(&P).unwrap()[crate::excavator::slot::DRILL] =
                Some(Stack::new("ntm:drillbit__ferro", 1));
            let target = [P[0], P[1] - 4, P[2]];
            h.blocks.insert(target, "minecraft:iron_ore".into());
            return (h, s, target);
        }
        Data::Drill(d) => {
            d.power = d.shape.spec().max_power;
            d.ticks = d.delay() - 1;
            if d.tanks.len() > 2 {
                d.tanks[2].mb = 1000;
            }
            h.blocks.insert(target, crate::oil_drill::OIL.into());
        }
        _ => panic!(),
    }
    (h, s, target)
}
fn resource_total(s: &Server, _h: &Host) -> u64 {
    let m = &s.ready.world.machines[&(D.into(), P)];
    match &m.data {
        Data::Excavator(e) => e
            .slots
            .iter()
            .flatten()
            .filter(|i| i.id == "minecraft:iron_ore")
            .map(|i| u64::from(i.count))
            .sum(),
        Data::Drill(d) => u64::from(d.tanks[0].mb),
        _ => panic!(),
    }
}
#[test]
fn resource_consumption_compares_the_world_even_when_a_deposit_is_not_drained() {
    for block in [
        "ntm:machine_excavator",
        "ntm:machine_well",
        "ntm:machine_fracking_tower",
    ] {
        let (mut h, mut s, target) = resource_machine(block);
        h.change_cell = Some((target, "minecraft:air".into()));
        s.native_tick(&mut h, 2);
        assert_eq!(
            resource_total(&s, &h),
            0,
            "stale world read credited resource for {block}"
        );
        assert_eq!(h.blocks[&target], "minecraft:air");
        // The player's extracted resource is the only product; retry observes air.
        s.native_tick(&mut h, 3);
        assert_eq!(
            resource_total(&s, &h),
            0,
            "retry duplicated the player resource for {block}"
        );
    }
}
#[test]
fn refused_world_commit_never_credits_mining_and_retry_preserves_retained_be_items() {
    let (mut h, mut s, target) = resource_machine("ntm:machine_excavator");
    h.blocks.insert(target, "minecraft:chest".into());
    let nbt = "{id:'minecraft:diamond',count:7,components:{'minecraft:custom_name':'Tesouro'}}"
        .to_string();
    h.encoded.insert(target, vec![nbt.clone()]);
    h.refuse_blocks = true;
    s.native_tick(&mut h, 2);
    assert_eq!(h.blocks[&target], "minecraft:chest");
    assert!(h.retained.is_empty());
    h.refuse_blocks = false;
    s.native_tick(&mut h, 3);
    assert_eq!(h.blocks[&target], "minecraft:air");
    assert_eq!(h.retained, vec![nbt]);
    s.native_tick(&mut h, 4);
    assert_eq!(h.retained.len(), 1);
}

#[test]
fn refused_consumed_cells_and_player_breaks_conserve_all_three_machine_products() {
    for block in [
        "ntm:machine_excavator",
        "ntm:machine_well",
        "ntm:machine_fracking_tower",
    ] {
        let (mut h, mut s, target) = resource_machine(block);
        if let Data::Drill(d) = &mut s.ready.world.machines.get_mut(&(D.into(), P)).unwrap().data {
            let seed = (0..10000)
                .find(|seed| {
                    let mut test = (**d).clone();
                    test.rolls = *seed;
                    test.suck(crate::oil_drill::OIL)
                })
                .expect("draining draw");
            d.rolls = seed;
        }
        h.refuse_blocks = true;
        s.native_tick(&mut h, 2);
        assert_eq!(
            resource_total(&s, &h),
            0,
            "refused world mutation credited {block}"
        );
        assert_ne!(h.blocks[&target], "minecraft:air");
        // The player mines the whole remaining source. It is counted once,
        // and neither the refused result nor its resource debit may survive.
        let player_resource = 1;
        h.blocks.insert(target, "minecraft:air".into());
        h.refuse_blocks = false;
        for tick in 3..8 {
            s.native_tick(&mut h, tick);
        }
        assert_eq!(
            player_resource + resource_total(&s, &h),
            1,
            "resource duplicated for {block}"
        );
    }
}

#[test]
fn all_three_mining_fixtures_produce_with_a_healthy_atomic_host() {
    for block in [
        "ntm:machine_excavator",
        "ntm:machine_well",
        "ntm:machine_fracking_tower",
    ] {
        let (mut h, mut s, _) = resource_machine(block);
        s.native_tick(&mut h, 2);
        assert!(resource_total(&s, &h) > 0, "dead fixture for {block}");
    }
}

#[test]
fn late_screen_and_hopper_inputs_are_retained_atomically_when_a_condemned_be_is_replaced() {
    for source in ["screen", "hopper"] {
        let mut h = Host::default();
        let mut s = install(&mut h, zirnox::BLOCK, 28);
        let Data::Zirnox(z) = &mut s.ready.world.machines.get_mut(&(D.into(), P)).unwrap().data
        else {
            panic!()
        };
        z.heat = zirnox::MAX + 100;
        h.refuse_blocks = true;
        s.native_tick(&mut h, 2);
        assert_eq!(blockstate::id_of(&h.blocks[&P]), zirnox::BLOCK);
        assert!(h.retained.is_empty());
        h.refuse_blocks = false;
        if source == "screen" {
            h.slots.get_mut(&P).unwrap()[25] = Some(Stack::new("minecraft:water_bucket", 1));
        } else {
            h.supply_on_structural = Some(Stack::new("minecraft:water_bucket", 1));
        }
        s.native_tick(&mut h, 3);
        if source == "hopper" {
            assert!(
                h.structural_rejections > 0,
                "stale BE snapshot must refuse publication before the retry"
            );
        }
        for t in 4..7 {
            s.native_tick(&mut h, t);
        }
        assert_eq!(blockstate::id_of(&h.blocks[&P]), zirnox::DESTROYED);
        let total = h
            .retained
            .iter()
            .filter(|i| i.contains("minecraft:water_bucket"))
            .count();
        assert_eq!(
            total, 1,
            "late {source} input must survive as one old-generation drop"
        );
        assert!(
            h.slots
                .get(&P)
                .into_iter()
                .flatten()
                .flatten()
                .all(|i| i.id != "minecraft:water_bucket")
        );
        s.native_world_saved(&mut h, true);
        let mut reopened = Server::native(&mut h, 20);
        for (at, json) in h.saved.clone() {
            reopened.native_adopt(D, at, &json);
        }
        reopened.native_tick(&mut h, 21);
        assert_eq!(
            h.retained
                .iter()
                .filter(|i| i.contains("minecraft:water_bucket"))
                .count(),
            1,
            "clean reopen repeats late {source} input"
        );
    }
}

#[test]
fn a_refused_structural_capture_does_not_duplicate_a_withdrawn_item_on_retry() {
    let mut h = Host::default();
    let mut s = install(&mut h, zirnox::BLOCK, 28);
    let Data::Zirnox(z) = &mut s.ready.world.machines.get_mut(&(D.into(), P)).unwrap().data else {
        panic!()
    };
    z.heat = zirnox::MAX + 100;
    h.refuse_blocks = true;
    s.native_tick(&mut h, 2);
    h.slots.get_mut(&P).unwrap()[25] = Some(Stack::new("minecraft:water_bucket", 1));
    s.native_tick(&mut h, 3);
    assert!(h.retained.is_empty());
    assert!(
        s.ready
            .world
            .machine_effects
            .iter()
            .flat_map(|b| &b.effects)
            .all(|e| !matches!(e,super::WorldEffect::Blocks{retained,..} if !retained.is_empty()))
    );
    // Clean reopen while the refused generation still owns its real input.
    s.native_world_saved(&mut h, true);
    let player = h.slots.get_mut(&P).unwrap()[25].take().unwrap();
    let mut reopened = Server::native(&mut h, 10);
    h.refuse_blocks = false;
    for t in 11..15 {
        reopened.native_tick(&mut h, t);
    }
    assert_eq!(blockstate::id_of(&h.blocks[&P]), zirnox::DESTROYED);
    assert_eq!(player.count, 1);
    assert_eq!(
        h.retained
            .iter()
            .filter(|i| i.contains("minecraft:water_bucket"))
            .count(),
        0
    );
}

fn retained_excavator() -> (Host, Server, Pos) {
    let (mut h, s, target) = resource_machine("ntm:machine_excavator");
    h.blocks.insert(target, "minecraft:chest".into());
    h.encoded
        .insert(target, vec!["{id:'minecraft:diamond',count:7}".into()]);
    (h, s, target)
}
/// A cell's contents leave in the very publication that consumes the cell (the host's ledger holds them in the
/// record of its chunk), so no journal of the storage waits for them and none brings them back.
#[test]
fn a_crash_before_the_atomic_commit_leaves_the_cell_and_the_miner_as_they_were() {
    let (mut h, mut s, target) = retained_excavator();
    h.crash_before_commit = true;
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.native_tick(&mut h, 2)))
            .is_err()
    );
    assert!(WorldState::load(&mut h).prepared_retentions.is_empty(), "no journal of the contents");
    assert_eq!(h.blocks[&target], "minecraft:chest");
    assert!(h.retained.is_empty());
    h.crash_before_commit = false;
    let mut reopened = Server::native(&mut h, 10);
    for (at, json) in h.saved.clone() {
        reopened.native_adopt(D, at, &json);
    }
    assert!(reopened.native_has_machine(D, P), "the miner is back");
    reopened.native_tick(&mut h, 11);
    assert!(h.retained.len() <= 1, "mined again at most once");
}
#[test]
fn a_crash_after_the_atomic_commit_does_not_put_the_contents_in_the_world_twice() {
    let (mut h, mut s, target) = retained_excavator();
    h.crash_after_commit = true;
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.native_tick(&mut h, 2)))
            .is_err()
    );
    assert_eq!(h.blocks[&target], "minecraft:air");
    // The host's record of the chunk holds the contents (`retained`); the storage never names them.
    assert_eq!(h.retained.len(), 1);
    assert!(WorldState::load(&mut h).prepared_retentions.is_empty());
    assert!(WorldState::load(&mut h).applied_machine_effects.iter().all(|b| b.effects.iter().all(|e| !matches!(e, super::WorldEffect::EncodedItems { .. }))));
    h.crash_after_commit = false;
    let mut reopened = Server::native(&mut h, 10);
    for (at, json) in h.saved.clone() {
        reopened.native_adopt(D, at, &json);
    }
    assert!(reopened.native_has_machine(D, P), "the miner is back");
    reopened.native_tick(&mut h, 11);
    reopened.native_tick(&mut h, 12);
    assert_eq!(h.retained.len(), 1, "no replay");
    reopened.native_world_saved(&mut h, true);
    let mut clean = Server::native(&mut h, 20);
    for (at, json) in h.saved.clone() {
        clean.native_adopt(D, at, &json);
    }
    clean.native_tick(&mut h, 21);
    assert_eq!(h.retained.len(), 1);
}
#[test]
fn refusing_the_atomic_mining_commit_discards_preparation_without_a_spawn() {
    let (mut h, mut s, target) = retained_excavator();
    h.refuse = true;
    s.native_tick(&mut h, 2);
    assert!(h.retained.is_empty());
    assert_eq!(h.blocks[&target], "minecraft:chest");
    assert!(WorldState::load(&mut h).prepared_retentions.is_empty());
    assert_eq!(h.encoded[&target].len(), 1);
}

fn isotope(h: &Host) -> crate::radioisotopes::Radioisotope {
    let m:Machine=serde_json::from_str(&h.saved[&P]).unwrap();
    let Data::Radioisotope(r)=m.data else{panic!()};*r
}
#[test]
fn radioisotopes_refused_commit_never_consumes_items_life_or_energy() {
    use crate::radioisotopes::{RTG,RADGEN,Radioisotope,DEPLETION};
    for block in [RTG,RADGEN] {for withdraw in [false,true] {
        let mut h=Host::default();let mut s=install(&mut h,block,if block==RTG {15}else{24});
        let item=if block==RTG {"ntm:pellet_rtg_lead"}else{"ntm:gem_rad"};
        let input=Stack::new(item,1);h.slots.get_mut(&P).unwrap()[0]=Some(input.clone());
        h.refuse=true;s.native_tick(&mut h,2);
        assert_eq!(h.slots[&P][0],Some(input.clone()));
        match isotope(&h) {Radioisotope::Rtg(r)=>{assert_eq!(r.power+r.pending,0);assert_eq!(r.heat,0)},Radioisotope::Radgen(r)=>{assert_eq!(r.power+r.pending,0);assert!(r.queues.iter().all(|q|q.processing.is_none()));}}
        if withdraw {h.slots.get_mut(&P).unwrap()[0]=None;} else if block==RTG {
            h.slots.get_mut(&P).unwrap()[1]=Some(input.clone());
        } else {h.slots.get_mut(&P).unwrap()[0].as_mut().unwrap().count+=1;}
        h.refuse=false;s.native_tick(&mut h,3);
        let external=usize::from(withdraw);
        match isotope(&h) {
            Radioisotope::Rtg(r)=>{
                assert_eq!(r.power,if withdraw {0}else{3000});
                let items=h.slots[&P].iter().flatten().collect::<Vec<_>>();assert_eq!(items.len()+external,if withdraw {1}else{2});
                for s in items {assert_eq!(crate::radioisotopes::life(s),21599);assert!(s.custom.as_ref().unwrap().contains(DEPLETION));}
            },
            Radioisotope::Radgen(r)=>{
                let active=r.queues.iter().filter(|q|q.processing.is_some()).count();
                let slotted=h.slots[&P].iter().flatten().map(|s|s.count as usize).sum::<usize>();
                assert_eq!(slotted+active+external,if withdraw {1}else{2});assert_eq!(r.power,if withdraw {0}else{25000});
                assert_eq!(r.queues[0].progress,if withdraw {0}else{1});
            }
        }
    }}
}
#[test]
fn isotope_conflict_in_expected_rolls_back_and_clean_reopen_resumes_strictly() {
    use crate::radioisotopes::{RTG,RADGEN,Radioisotope};
    for block in [RTG,RADGEN] {
        let mut h=Host::default();let mut s=install(&mut h,block,if block==RTG {15}else{24});
        let item=if block==RTG {"ntm:pellet_rtg_radium"}else{"ntm:gem_rad"};
        h.slots.get_mut(&P).unwrap()[0]=Some(Stack::new(item,1));
        h.insert_on_commit=Some((P,0));s.native_tick(&mut h,2);
        assert_eq!(h.slots[&P][0].as_ref().unwrap().count,2);
        match isotope(&h) {Radioisotope::Rtg(r)=>assert_eq!(r.power,0),Radioisotope::Radgen(r)=>assert_eq!(r.power,0)}
        s.native_tick(&mut h,3);s.native_tick(&mut h,4);
        let before=isotope(&h);let slots=h.slots[&P].clone();let json=h.saved[&P].clone();
        let mut reopened=Server::native(&mut h,4);assert!(reopened.native_adopt(D,P,&json));
        assert_eq!(h.slots[&P],slots);reopened.native_tick(&mut h,5);
        match (before,isotope(&h)) {
            (Radioisotope::Rtg(a),Radioisotope::Rtg(b))=>{
                assert!(b.power>a.power);let remaining=crate::radioisotopes::life(h.slots[&P][0].as_ref().unwrap());
                assert_eq!(remaining,11_520_000_000-3);
            },
            (Radioisotope::Radgen(a),Radioisotope::Radgen(b))=>{assert!(b.power>a.power);assert!(b.queues[0].progress>a.queues[0].progress);assert_eq!(b.queues[0].processing,a.queues[0].processing);},
            _=>panic!()
        }
    }
}
#[test]
fn radgen_back_port_is_exact_and_follows_all_four_facings() {
    use crate::{radioisotopes::RADGEN,energy::Role,machine::BatteryMode};
    for face in ["north","south","east","west"] {
        let dir=crate::registry::facing_dir(face).unwrap();
        let port=[P[0]-dir[0]*3,P[1],P[2]-dir[2]*3];
        assert_eq!(super::container_bridge::energy_port_cells(RADGEN,P,Some(face)),vec![port]);
        let mut h=Host::default();let mut s=install(&mut h,RADGEN,24);
        let key=(D.to_string(),P);
        s.ready.world.machines.get_mut(&key).unwrap().facing=Some(face.into());
        let out=[P[0]-dir[0]*4,P[1],P[2]-dir[2]*4];
        let bp=[out[0]-dir[0],out[1],out[2]-dir[2]];
        h.blocks.insert(out,"ntm:red_cable".into());h.blocks.insert(bp,"ntm:machine_battery".into());h.slots.insert(bp,vec![None;2]);
        s.native_block_changed(D,out,"minecraft:air","ntm:red_cable");s.native_block_changed(D,bp,"minecraft:air","ntm:machine_battery");
        h.slots.get_mut(&P).unwrap()[0]=Some(Stack::new("ntm:gem_rad",1));
        s.native_tick(&mut h,2);
        let m=s.ready.world.machines.get_mut(&(D.into(),bp)).unwrap();let Data::Battery(b)=&mut m.data else{panic!()};b.mode=BatteryMode::Input;
        assert_eq!(m.logic_mut().energy().unwrap().role(),Role::Storage);
        s.native_tick(&mut h,3);
        let Data::Battery(b)=&s.ready.world.machines[&(D.into(),bp)].data else{panic!()};assert!(b.power>0,"{face}");
    }
}

/// Item effects carry ids that survive a crash: two identical effects in one batch both apply, a replay
/// of either after the server forgot it applies nothing, and a retired id goes once storage stops naming it.
#[test]
fn item_effects_apply_once_across_a_replay_and_identical_ones_both_apply() {
    use super::{CommittedEffects, WorldEffect};
    let mut h = Host::default();
    let mut s = install(&mut h, zirnox::BLOCK, 28);
    let rod = || WorldEffect::Items { dim: D.into(), pos: P, items: vec![("ntm:pile_rod__zr".into(), 1)] };
    s.ready.world.machine_effects.push_back(CommittedEffects {
        origins: vec![],
        owners: vec![(D.into(), P)],
        effects: std::collections::VecDeque::from([rod(), rod()]),
        ..Default::default()
    });
    for t in 2..6 {
        s.native_tick(&mut h, t);
    }
    assert_eq!(h.drops.len(), 2, "both identical effects were applied");
    assert_eq!(h.ledger.len(), 2);
    // The crash: storage still names both (receipts waiting for a flushed save), the server forgets them.
    let kept = s.ready.world.kept(0);
    assert!(!s.ready.world.applied_machine_effects.is_empty());
    h.set(ntm_core::persist::KEY, &kept).unwrap();
    let mut reopened = Server::native(&mut h, 3);
    let saved = h.saved[&P].clone();
    assert!(reopened.native_adopt(D, P, &saved));
    for t in 4..8 {
        reopened.native_tick(&mut h, t);
    }
    assert!(reopened.ready.world.machine_effects.is_empty(), "the replay ran");
    assert_eq!(h.drops.len(), 2, "the replay found both ids and added nothing");
    // A flushed save acknowledges the receipts; storage stops naming the ids and the ledger forgets them.
    reopened.native_world_saved(&mut h, true);
    assert_eq!(h.pruned, 2);
    assert!(h.ledger.is_empty());
}
