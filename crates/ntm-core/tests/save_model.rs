//! An exhaustive check of a machine's emitted items through saves, unloads and process deaths, against vanilla.
//!
//! The model has the machine's chunk A (its block entity's items and, in its record, the stacks an effect owes while the
//! chunk it drops into is not loaded), a chunk with a hopper (its record holds what the hopper took), a third chunk C,
//! the entity files of all three, the player's inventory and file, the mod's storage, and the engine's one region
//! writer: a FIFO queue whose records of one target coalesce (the newest queued is the one written) and that a death
//! drops. Items are atoms; an emission stamps its atoms with a generation.
//!
//! The mod's world keeps its storage in memory and writes it at the instant of each save, after the save has queued its
//! regions and entities and before the writer has written them (`Committed`): a death between the two finds the storage
//! of the save with the regions of the one before. Every sequence of operations up to a depth is played by it and, step
//! for step, by vanilla's world, where the machine is a dropper: after a death it comes back with the stock of the last
//! save and fires again; a shot into a chunk that is not loaded waits in its own record, as a vanilla machine's output
//! slot holds what it made, and goes out once the chunk is in. The writers of the two keep pace. In every state reached, against vanilla's in the same play:
//!
//! * (L) no atom is gone that vanilla's world still holds;
//! * (D) no atom is held more often than vanilla's world holds it.
//!
//! There are no exceptions: a loss or a copy vanilla also has is vanilla's in the very same play, and not one otherwise.
use std::collections::{BTreeMap, BTreeSet, HashMap};

type Atom = u8;
/// An atom as some copy holds it: (atom, generation of the emission that put it out: 0 for the machine's own).
type Unit = (Atom, u8);
const ATOMS: u8 = 3;
/// A stack's name: the root it was put out as (the emission's generation, a copy read again from a file), then its
/// path down the splits since (their count, and which side of each it took). A split names both halves anew, so a name
/// is never given twice, and both worlds name one stack alike.
type Id = (u16, u8, u64);

/// One name is the other's or names one of its splits.
fn related(a: Id, b: Id) -> bool {
    let shorter = a.1.min(b.1);
    let mask = (1u64 << shorter) - 1;
    a.0 == b.0 && a.2 & mask == b.2 & mask
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Policy {
    /// The dropper: its stock in its block entity, its drops vanilla's; no storage of a mod.
    Vanilla,
    /// The mod: its machine's effects, what an effect owes in the machine's record, its storage written at each save.
    Committed,
}

const A: usize = 0;
const HOPPER: usize = 1;
const C: usize = 2;
const CHUNKS: usize = 3;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Ent {
    id: Id,
    chunk: usize,
    units: Vec<Unit>,
}

/// What a chunk's record holds.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
struct Record {
    /// The machine's items in its block entity (A), or the hopper's (HOPPER).
    items: Vec<Unit>,
    /// A: the stacks an effect owes while the chunk it drops into is not loaded, with that chunk.
    owed: Vec<(usize, Vec<Unit>)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Write {
    Region(usize, Record),
    Entities(usize, Vec<Ent>),
}

impl Write {
    fn target(&self) -> (u8, usize) {
        match self {
            Write::Region(c, _) => (0, *c),
            Write::Entities(c, _) => (1, *c),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Queued {
    write: Write,
    /// The operation that queued it.
    at: u32,
}

/// The mod's storage: what its server keeps outside the blocks. It holds no item (the storage of every suite's world
/// holds none); the count of emissions stands for it, a save ahead of the regions or behind them.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
struct Storage {
    emissions: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Disk {
    region: [Record; CHUNKS],
    entities: [Vec<Ent>; CHUNKS],
    player: Vec<Unit>,
    storage: Storage,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct World {
    policy: Policy,
    disk: Disk,
    queue: Vec<Queued>,
    at: u32,
    loaded: [bool; CHUNKS],
    records: [Option<Record>; CHUNKS],
    ents: Vec<Ent>,
    player: Vec<Unit>,
    storage: Storage,
    generation: u8,
    /// The machine fired since the world opened: both worlds' machines run their step again only from what they hold
    /// after a reopen.
    fired: bool,
    shadow: Option<Box<World>>,
}

fn initial(policy: Policy) -> World {
    let machine = Record { items: (0..ATOMS).map(|a| (a, 0)).collect(), owed: Vec::new() };
    let mut region: [Record; CHUNKS] = Default::default();
    region[A] = machine.clone();
    World {
        policy,
        disk: Disk { region, entities: Default::default(), player: Vec::new(), storage: Storage::default() },
        queue: Vec::new(),
        at: 0,
        loaded: [true; CHUNKS],
        records: [Some(machine), Some(Record::default()), None],
        ents: Vec::new(),
        player: Vec::new(),
        storage: Storage::default(),
        generation: 0,
        fired: false,
        shadow: (policy != Policy::Vanilla).then(|| Box::new(initial(Policy::Vanilla))),
    }
}

impl World {
    // ------------------------------------------------------------------ the engine's writer

    fn queue(&mut self, write: Write) {
        self.queue.push(Queued { write, at: self.at });
    }
    /// The writer writes its next record, unless a newer one of the same target is queued (IOWorker's pending writes).
    /// Returns the operation that queued it and its target.
    fn write_next(&mut self) -> (u32, (u8, usize)) {
        let q = self.queue.remove(0);
        let target = q.write.target();
        if !self.queue.iter().any(|n| n.write.target() == target) {
            match q.write {
                Write::Region(c, r) => self.disk.region[c] = r,
                Write::Entities(c, e) => self.disk.entities[c] = e,
            }
        }
        (q.at, target)
    }
    /// Vanilla's writer keeps pace: everything queued before the operation the mod's writer just wrote from, and of that
    /// operation up to the same target, or all of it once the mod's is done with it.
    fn catch_up(&mut self, at: u32, target: (u8, usize), done: bool) {
        while self.queue.first().is_some_and(|q| q.at < at) {
            self.write_next();
        }
        let same: Vec<(u8, usize)> = self.queue.iter().take_while(|q| q.at == at).map(|q| q.write.target()).collect();
        let upto = if done { same.len() } else { same.iter().position(|t| *t == target).map_or(0, |i| i + 1) };
        for _ in 0..upto {
            self.write_next();
        }
    }
    fn read_region(&self, c: usize) -> Record {
        self.queue.iter().rev().find_map(|q| if let Write::Region(x, r) = &q.write { (*x == c).then(|| r.clone()) } else { None }).unwrap_or_else(|| self.disk.region[c].clone())
    }
    fn read_entities(&self, c: usize) -> Vec<Ent> {
        self.queue.iter().rev().find_map(|q| if let Write::Entities(x, e) = &q.write { (*x == c).then(|| e.clone()) } else { None }).unwrap_or_else(|| self.disk.entities[c].clone())
    }
    fn ents_of(&self, c: usize) -> Vec<Ent> {
        self.ents.iter().filter(|e| e.chunk == c).cloned().collect()
    }
    fn ent(&self, id: Id) -> Option<usize> {
        self.ents.iter().position(|e| e.id == id)
    }

    // ------------------------------------------------------------------ the game

    /// The machine fires: two atoms out of its block entity into `into`. While that chunk is not loaded they wait in A's
    /// record (the mod's effect owes them; vanilla's machine holds them in its output), and go out once it is.
    fn fire(&mut self, into: usize) {
        if !self.records[A].as_ref().is_some_and(|r| r.items.iter().any(|u| u.0 <= 1)) {
            return;
        }
        self.fired = true;
        self.generation += 1;
        self.storage.emissions = self.storage.emissions.wrapping_add(1);
        self.put_out(into, self.generation);
    }
    /// The shot of generation `generation` out of the block entity: into `into` if it is loaded, else owed in A's record.
    fn put_out(&mut self, into: usize, generation: u8) {
        let units = vec![(0, generation), (1, generation)];
        let r = self.records[A].as_mut().unwrap();
        r.items.retain(|u| u.0 > 1);
        if self.loaded[into] {
            self.ents.push(Ent { id: (u16::from(generation), 0, 0), chunk: into, units });
        } else {
            r.owed.push((into, units));
        }
    }
    /// What waits in A's record for a chunk that is loaded now, put out.
    fn pay(&mut self) {
        let Some(r) = self.records[A].as_mut().filter(|_| self.loaded[A]) else { return };
        let loaded = self.loaded;
        let (due, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut r.owed).into_iter().partition(|(c, _)| loaded[*c]);
        r.owed = kept;
        for (c, units) in due {
            self.join(Ent { id: (u16::from(units[0].1), 0, 0), chunk: c, units });
        }
    }
    /// A stack joins the world; a copy of one already here, or of one that split since (read again from a file or
    /// a record older than the stack's move), is a stack of its own.
    fn join(&mut self, mut e: Ent) {
        while self.ents.iter().any(|x| related(x.id, e.id)) {
            e.id.0 = e.id.0.wrapping_add(0x1000);
        }
        self.ents.push(e);
    }
    fn split(&mut self, id: Id) {
        let i = self.ent(id).unwrap();
        let moved = self.ents[i].units.pop().unwrap();
        let (root, len, path) = id;
        self.ents[i].id = (root, len + 1, path);
        let mut e = self.ents[i].clone();
        e.id = (root, len + 1, path | 1 << len);
        e.units = vec![moved];
        self.ents.push(e);
    }
    fn fold(&mut self, from: Id, into: Id) {
        let (i, j) = (self.ent(from).unwrap(), self.ent(into).unwrap());
        let units = std::mem::take(&mut self.ents[i].units);
        self.ents[j].units.extend(units);
        self.ents.remove(i);
    }
    fn pickup(&mut self, id: Id) {
        let e = self.ents.remove(self.ent(id).unwrap());
        self.player.extend(e.units);
    }
    fn absorb(&mut self, id: Id) {
        let e = self.ents.remove(self.ent(id).unwrap());
        self.records[HOPPER].as_mut().unwrap().items.extend(e.units);
    }
    /// The engine's unload: the chunk's entities, then its record.
    fn unload(&mut self, c: usize) {
        let e = self.ents_of(c);
        self.queue(Write::Entities(c, e));
        if let Some(r) = self.records[c].clone() {
            self.queue(Write::Region(c, r));
        }
        self.ents.retain(|e| e.chunk != c);
        self.loaded[c] = false;
        self.records[c] = None;
    }
    fn load(&mut self, c: usize) {
        self.loaded[c] = true;
        for mut e in self.read_entities(c) {
            e.chunk = c;
            self.join(e);
        }
        if c == A || c == HOPPER {
            self.records[c] = Some(self.read_region(c));
        }
    }
    /// A save: the loaded chunks' records in the order the batch takes, their entities, to the writer; the writer done
    /// when the save waits; then the player's file (in `level.dat`) and the mod's storage, written in place as of this
    /// instant, while the writer may still hold the regions.
    fn save(&mut self, wait: bool, a_first: bool) {
        let order = if a_first { [A, HOPPER] } else { [HOPPER, A] };
        for c in order {
            if let Some(r) = self.records[c].clone().filter(|_| self.loaded[c]) {
                self.queue(Write::Region(c, r));
            }
        }
        for c in 0..CHUNKS {
            if self.loaded[c] {
                let e = self.ents_of(c);
                self.queue(Write::Entities(c, e));
            }
        }
        if wait {
            while !self.queue.is_empty() {
                self.write_next();
            }
        }
        self.disk.player = self.player.clone();
        if self.policy == Policy::Committed {
            self.disk.storage = self.storage.clone();
        }
    }
    /// A death: what is not on disk is gone, and the next open starts from the files.
    fn kill(&mut self) {
        self.queue.clear();
        self.loaded = [false; CHUNKS];
        self.records = [None, None, None];
        self.ents.clear();
        self.player = self.disk.player.clone();
        self.storage = self.disk.storage.clone();
        self.fired = false;
    }

    // ------------------------------------------------------------------ what the world holds

    /// Every unit the world holds now: the loaded chunks in memory, the others as a load would read them, the player,
    /// the machine's and the hopper's records (with what A owes).
    fn units(&self) -> Vec<Unit> {
        let mut out: Vec<Unit> = Vec::new();
        for c in 0..CHUNKS {
            let ents = if self.loaded[c] { self.ents_of(c) } else { self.read_entities(c) };
            out.extend(ents.iter().flat_map(|e| e.units.iter().copied()));
        }
        out.extend(self.player.iter().copied());
        for c in [A, HOPPER] {
            let r = match &self.records[c] {
                Some(r) if self.loaded[c] => r.clone(),
                _ => self.read_region(c),
            };
            out.extend(r.items);
            out.extend(r.owed.into_iter().flat_map(|(_, u)| u));
        }
        out
    }
    fn copies(&self) -> BTreeMap<Atom, usize> {
        let mut out = BTreeMap::new();
        for (a, _) in self.units() {
            *out.entry(a).or_default() += 1;
        }
        out
    }
    /// (L) and (D) against vanilla's world in the same state of the play, and the model's own soundness.
    fn violation(&self) -> Option<String> {
        for x in std::iter::once(self).chain(self.shadow.as_deref()) {
            let clash = x.ents.iter().enumerate().any(|(i, a)| x.ents[i + 1..].iter().any(|b| related(a.id, b.id)));
            if clash {
                return Some(format!("model: two stacks' names clash: {:?}", x.ents));
            }
        }
        let vanilla = self.shadow.as_ref()?.copies();
        let here = self.copies();
        for atom in 0..ATOMS {
            let (mine, theirs) = (here.get(&atom).copied().unwrap_or(0), vanilla.get(&atom).copied().unwrap_or(0));
            if mine > theirs.max(1) {
                return Some(format!("duplicate: atom {atom} held {mine} times, vanilla {theirs}"));
            }
            if mine == 0 && theirs > 0 {
                return Some(format!("lost: atom {atom}, vanilla holds it {theirs} times"));
            }
        }
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Op {
    Fire(usize),
    Split(Id),
    Move(Id, usize),
    Fold(Id, Id),
    Pickup(Id),
    Absorb(Id),
    Unload(usize),
    Load(usize),
    Save(bool, bool),
    Write,
    Kill,
}

fn ops(w: &World) -> Vec<Op> {
    let mut out = Vec::new();
    if !w.fired && w.loaded[A] {
        out.push(Op::Fire(A));
        out.push(Op::Fire(C));
    }
    for e in &w.ents {
        if e.units.len() > 1 {
            out.push(Op::Split(e.id));
        }
        for c in 0..CHUNKS {
            if c != e.chunk && w.loaded[c] {
                out.push(Op::Move(e.id, c));
            }
        }
        for f in &w.ents {
            if f.id != e.id && f.chunk == e.chunk {
                out.push(Op::Fold(e.id, f.id));
            }
        }
        out.push(Op::Pickup(e.id));
        if e.chunk == HOPPER {
            out.push(Op::Absorb(e.id));
        }
    }
    for c in 0..CHUNKS {
        out.push(if w.loaded[c] { Op::Unload(c) } else { Op::Load(c) });
    }
    for wait in [false, true] {
        for a_first in [true, false] {
            out.push(Op::Save(wait, a_first));
        }
    }
    if !w.queue.is_empty() {
        out.push(Op::Write);
    }
    out.push(Op::Kill);
    out
}

/// Whether `op` (chosen in the mod's world) can be played in `w`: vanilla's world skips what concerns a stack only the
/// mod's has.
fn playable(w: &World, op: Op) -> bool {
    let at = |id: Id| w.ent(id).map(|i| &w.ents[i]);
    match op {
        Op::Fire(_) => !w.fired && w.loaded[A],
        Op::Split(id) => at(id).is_some_and(|e| e.units.len() > 1),
        Op::Move(id, c) => at(id).is_some_and(|e| e.chunk != c) && w.loaded[c],
        Op::Fold(i, j) => i != j && at(i).zip(at(j)).is_some_and(|(e, f)| e.chunk == f.chunk),
        Op::Pickup(id) => at(id).is_some(),
        Op::Absorb(id) => at(id).is_some_and(|e| e.chunk == HOPPER) && w.loaded[HOPPER],
        Op::Unload(c) => w.loaded[c],
        Op::Load(c) => !w.loaded[c],
        Op::Save(..) | Op::Kill => true,
        Op::Write => !w.queue.is_empty(),
    }
}

fn play(w: &mut World, op: Op) {
    match op {
        Op::Fire(c) => w.fire(c),
        Op::Split(id) => w.split(id),
        Op::Move(id, c) => {
            let i = w.ent(id).unwrap();
            w.ents[i].chunk = c;
        }
        Op::Fold(i, j) => w.fold(i, j),
        Op::Pickup(id) => w.pickup(id),
        Op::Absorb(id) => w.absorb(id),
        Op::Unload(c) => w.unload(c),
        Op::Load(c) => w.load(c),
        Op::Save(wait, a_first) => w.save(wait, a_first),
        Op::Write => {
            w.write_next();
        }
        Op::Kill => w.kill(),
    }
    // The end of the tick: what was kept for a chunk that is loaded now is put out.
    w.pay();
}

/// One operation in both worlds, with the writers keeping pace.
fn step(w: &mut World, op: Op) {
    w.at += 1;
    let mut shadow = w.shadow.take();
    if op == Op::Write {
        let (at, target) = w.write_next();
        let done = !w.queue.first().is_some_and(|q| q.at == at);
        if let Some(s) = shadow.as_mut() {
            s.catch_up(at, target, done);
        }
    } else {
        play(w, op);
    }
    if let Some(s) = shadow.as_mut() {
        s.at = w.at;
        if op != Op::Write && playable(s, op) {
            play(s, op);
        }
        s.ents.sort();
    }
    w.ents.sort();
    w.shadow = shadow;
}

struct Search {
    seen: HashMap<(u128, usize), u128>,
    states: usize,
}

/// The state with the operations of its writes numbered by their order alone.
fn canonical(w: &World) -> World {
    let mut ats: BTreeSet<u32> = BTreeSet::new();
    for x in std::iter::once(w).chain(w.shadow.as_deref()) {
        ats.extend(x.queue.iter().map(|q| q.at));
    }
    let rank: BTreeMap<u32, u32> = ats.into_iter().enumerate().map(|(i, a)| (a, i as u32 + 1)).collect();
    let renumber = |x: &mut World| {
        x.at = 0;
        for q in &mut x.queue {
            q.at = rank[&q.at];
        }
    };
    let mut out = w.clone();
    renumber(&mut out);
    if let Some(s) = out.shadow.as_mut() {
        renumber(s);
    }
    out
}

/// Two independent 64-bit hashes of the state: the memo keeps these, not the states.
fn fingerprint(w: &World) -> u128 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let c = canonical(w);
    let half = |salt: u8| {
        let mut h = DefaultHasher::new();
        salt.hash(&mut h);
        c.hash(&mut h);
        h.finish()
    };
    (u128::from(half(0)) << 64) | u128::from(half(1))
}

/// Every sequence of `depth` operations: the first violation, else how many complete sequences were covered. States
/// already explored with as many operations left are not explored again: the sequences through them are counted.
fn explore(depth: usize) -> (Result<u128, (Vec<Op>, String)>, Search) {
    fn go(w: &World, left: usize, path: &mut Vec<Op>, s: &mut Search) -> Result<u128, (Vec<Op>, String)> {
        s.states += 1;
        let key = (fingerprint(w), left);
        if let Some(n) = s.seen.get(&key) {
            return Ok(*n);
        }
        if let Some(why) = w.violation() {
            return Err((path.clone(), why));
        }
        if left == 0 {
            return Ok(1);
        }
        let mut n = 0;
        for op in ops(w) {
            let mut next = w.clone();
            step(&mut next, op);
            path.push(op);
            n += go(&next, left - 1, path, s)?;
            path.pop();
        }
        s.seen.insert(key, n);
        Ok(n)
    }
    let mut s = Search { seen: HashMap::new(), states: 0 };
    let found = go(&initial(Policy::Committed), depth, &mut Vec::new(), &mut s);
    (found, s)
}

fn depth(default: usize) -> usize {
    std::env::var("NTM_SAVE_MODEL_DEPTH").ok().and_then(|d| d.parse().ok()).unwrap_or(default)
}

#[test]
fn the_mods_world_saved_with_its_storage_holds_what_vanillas_holds() {
    let n = depth(9);
    let (found, s) = explore(n);
    match found {
        Ok(sequences) => println!("committed: depth {n}: {} states visited, {} distinct, {sequences} complete sequences", s.states, s.seen.len()),
        Err((path, why)) => panic!("{why}\n  {path:?}"),
    }
}

#[test]
fn long_random_plays_hold_what_vanillas_holds() {
    let walks: usize = std::env::var("NTM_SAVE_MODEL_WALKS").ok().and_then(|n| n.parse().ok()).unwrap_or(2_000);
    let length: usize = std::env::var("NTM_SAVE_MODEL_WALK").ok().and_then(|n| n.parse().ok()).unwrap_or(30);
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut random = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut steps = 0usize;
    for _ in 0..walks {
        let mut w = initial(Policy::Committed);
        let mut path = Vec::new();
        for _ in 0..length {
            if let Some(why) = w.violation() {
                panic!("{why}\n  {path:?}");
            }
            let choices = ops(&w);
            let op = choices[random() as usize % choices.len()];
            step(&mut w, op);
            path.push(op);
            steps += 1;
        }
        if let Some(why) = w.violation() {
            panic!("{why}\n  {path:?}");
        }
    }
    println!("committed, random: {walks} plays of up to {length} operations, {steps} operations");
}

#[test]
fn a_death_outside_a_save_brings_back_the_last_save_in_both_worlds() {
    // The common case: the machine fires, a drop is picked up, the process dies with nothing more written. Both worlds
    // come back as the last save left them, atom for atom.
    let mut w = initial(Policy::Committed);
    for op in [Op::Save(true, true), Op::Fire(A), Op::Pickup((1, 0, 0)), Op::Kill, Op::Load(A), Op::Load(HOPPER), Op::Load(C)] {
        step(&mut w, op);
    }
    let vanilla = w.shadow.as_ref().unwrap();
    assert_eq!(w.copies(), vanilla.copies());
    assert_eq!(w.records[A].as_ref().unwrap().items, (0..ATOMS).map(|a| (a, 0)).collect::<Vec<_>>(), "the machine holds its stock again");
}
