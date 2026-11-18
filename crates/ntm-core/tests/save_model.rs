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
//!
//! The machine can also be broken: vanilla's dropper spills its stock at once, and so does the mod's (a pile's core
//! puts its rods out at once, as Java's `breakBlock` does, whatever members are out). A break by a command between two
//! ticks (`Command`) is seen by the mod only when the next tick ends, a save is about to take the chunks
//! (`world_saving`) or a chunk leaves while still in place (the engine's unload order): its stock is out then, before
//! anything is taken. And an owed shot paid leaves its name in the machine's record, where a later shot of the same
//! name would be taken for it and not put out: the mod names its shots anew in every session.
//!
//! A world can also open from an earlier version's storage that holds a removal (the machine broken, its stock not
//! out) or a batch (a shot owed): vanilla's world holds the same atoms in A's entities or A's record. The mod plays
//! neither before A's record is read: a record that took it (`migrated`) says what is left of it; otherwise the mod
//! takes it up and A's record says so. The storage keeps it until a waited save wrote that record before it, or the
//! record was read from the disk with the marker.
//!
//! The check is not empty: seven wrong mods, kept here only to be caught, are caught within a few operations: one that
//! keeps what an effect owes in its storage (`InStorage`), one that keeps a removal there (`RemovalInStorage`), one
//! that names its shots by a count kept with the storage, which a death sets back (`CounterIds`), one that takes a
//! command's break only after the save took the chunks (`PublishAfterCapture`), one that takes it only after the chunk
//! left (`UnloadWithoutHome`), one whose storage forgets the earlier version's obligation at a save that does not wait
//! (`RetireEarly`), one that takes it up again from a storage that came back although A's record took it
//! (`NoMarker`).
use std::collections::{BTreeMap, BTreeSet, HashMap};

type Atom = u8;
/// An atom as some copy holds it: (atom, generation of the emission that put it out: 0 for the machine's own).
type Unit = (Atom, u8);
/// The machine's stock: two shots of two atoms each (a session fires once, so the second shot is a later session's).
const ATOMS: u8 = 4;
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
    /// A wrong mod, to show the check finds it: what an effect owes is kept in its storage, written at each save.
    InStorage,
    /// A wrong mod: a removal waits for its group (the hopper's chunk, a member, loaded), kept in its storage (R6-2,
    /// F1 of the reviews).
    RemovalInStorage,
    /// A wrong mod: a shot is named by the storage's count of emissions, which a death sets back (R6-1).
    CounterIds,
    /// A wrong mod: a command's break is taken after the save took the chunks (F-novo-1, N1).
    PublishAfterCapture,
    /// A wrong mod: a command's break is taken after its chunk left (R7-2).
    UnloadWithoutHome,
    /// A wrong mod: the earlier version's obligation leaves the storage at any save once A's record took it in memory
    /// (R7-1).
    RetireEarly,
    /// A wrong mod: the earlier version's obligation is taken up whatever A's record says it took (F-novo-2, N2).
    NoMarker,
}

/// An earlier version's obligation in its storage: a removal holding the machine's stock, or a shot owed into a chunk.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Legacy {
    Removal(Vec<Unit>),
    Batch(usize, Vec<Unit>, u8),
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
    /// A: the stacks an effect owes while the chunk it drops into is not loaded, with that chunk and the shot's name.
    owed: Vec<(usize, Vec<Unit>, u8)>,
    /// A: the names of the owed shots paid (the ledger's `applied`): a shot of the same name is taken for one of them.
    paid: Vec<u8>,
    /// A: the earlier version's obligation was taken from its storage (`migrated: X`).
    migrated: bool,
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
/// holds none); the count of emissions stands for it, a save ahead of the regions or behind them. Only `InStorage`'s
/// holds what an effect owes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
struct Storage {
    emissions: u8,
    owed: Vec<(usize, Vec<Unit>, u8)>,
    /// `RemovalInStorage`'s removal.
    removed: Option<Vec<Unit>>,
    /// An earlier version's obligation, kept as it was read.
    legacy: Option<Legacy>,
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
    /// A command's break the mod has not taken yet: the stock, in memory only, with the name its stack takes, given at
    /// the command as vanilla's spill gets it then (a name the model's two worlds play their operations by).
    pending: Option<(Id, Vec<Unit>)>,
    /// A's record was read for the earlier version's obligation this session.
    read: bool,
    /// The marker was given to A's record this session (in memory, or written by its unload).
    marked: bool,
    shadow: Option<Box<World>>,
}

fn initial(policy: Policy) -> World {
    let machine = Record { items: (0..ATOMS).map(|a| (a, 0)).collect(), ..Default::default() };
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
        pending: None,
        read: false,
        marked: false,
        shadow: (policy != Policy::Vanilla).then(|| Box::new(initial(Policy::Vanilla))),
    }
}

/// A world opened from an earlier version's storage holding `legacy`, and vanilla's world holding the same atoms where
/// it keeps them: a shot owed into C (out of reach) in A's record, as a dropper's output; a removal's stock in A's
/// record too, the machine whole, broken the instant the world reads that record (`settle`), as the mod takes the
/// removal up then. Vanilla's world keeps no storage: its copy of `legacy` only says which machine that is.
fn initial_legacy(policy: Policy, removal: bool) -> World {
    let mut w = initial(policy);
    let (stock, legacy) = if removal {
        (Vec::new(), Legacy::Removal((0..ATOMS).map(|a| (a, 0)).collect()))
    } else {
        ((2..ATOMS).map(|a| (a, 0)).collect(), Legacy::Batch(C, vec![(0, 1), (1, 1)], 1))
    };
    let set = |x: &mut World| {
        x.generation = 1;
        x.disk.region[A].items = stock.clone();
        x.records[A] = Some(x.disk.region[A].clone());
        if !removal {
            x.loaded[C] = false;
        }
    };
    set(&mut w);
    w.storage.legacy = Some(legacy.clone());
    w.disk.storage = w.storage.clone();
    let vanilla = w.shadow.as_mut().unwrap();
    set(vanilla);
    match legacy {
        Legacy::Removal(units) => {
            vanilla.disk.region[A].items = units;
            vanilla.storage.legacy = Some(Legacy::Removal(Vec::new()));
            vanilla.disk.storage = vanilla.storage.clone();
        }
        Legacy::Batch(c, units, name) => vanilla.disk.region[A].owed = vec![(c, units, name)],
    }
    vanilla.records[A] = Some(vanilla.disk.region[A].clone());
    w
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
        // Vanilla's machine standing for an earlier version's removal is a broken one: it never fires.
        let removed = self.policy == Policy::Vanilla && matches!(self.storage.legacy, Some(Legacy::Removal(_)));
        if removed || !self.records[A].as_ref().is_some_and(|r| !r.items.is_empty()) {
            return;
        }
        self.fired = true;
        self.generation += 1;
        self.storage.emissions = self.storage.emissions.wrapping_add(1);
        // The mod draws a nonce each session: its names never come back (`generation` stands for them, never set back).
        let name = if self.policy == Policy::CounterIds { self.storage.emissions } else { self.generation };
        self.put_out(into, self.generation, name);
    }
    /// The shot of generation `generation`, named `name`, out of the block entity: into `into` if it is loaded, else
    /// owed in A's record. A name A's record knows as paid is taken for that one: nothing goes out.
    fn put_out(&mut self, into: usize, generation: u8, name: u8) {
        let r = self.records[A].as_mut().unwrap();
        // The two lowest atoms of the stock go out: (0, 1), then (2, 3).
        let mut shot: Vec<Atom> = r.items.iter().map(|u| u.0).collect();
        shot.sort_unstable();
        shot.truncate(2);
        r.items.retain(|u| !shot.contains(&u.0));
        let units: Vec<Unit> = shot.iter().map(|a| (*a, generation)).collect();
        if r.paid.contains(&name) {
            return;
        }
        if self.loaded[into] {
            self.ents.push(Ent { id: (u16::from(generation), 0, 0), chunk: into, units });
        } else if self.policy == Policy::InStorage {
            self.storage.owed.push((into, units, name));
        } else {
            r.owed.push((into, units, name));
        }
    }
    /// A command breaks the machine between two ticks: vanilla's spills at once; the mod's stock waits in memory until
    /// it is taken (`settle`).
    fn command_break(&mut self) {
        if self.policy == Policy::Vanilla {
            return self.break_machine();
        }
        let units = std::mem::take(&mut self.records[A].as_mut().unwrap().items);
        let id = self.free_name((0xB000, 0, 0));
        self.pending = Some((id, units));
    }
    /// What the mod has not taken yet, taken as the end of a tick takes it: A's record read for the earlier version's
    /// obligation (taken up, or retired when the record took it), and a command's break spilled while A is in.
    fn settle(&mut self) {
        if !self.loaded[A] {
            return;
        }
        if self.policy == Policy::Vanilla {
            // The machine standing for an earlier version's removal breaks as the world reads its record, unless that
            // record is one written after it broke.
            if !std::mem::replace(&mut self.read, true)
                && matches!(self.storage.legacy, Some(Legacy::Removal(_)))
                && self.records[A].as_ref().is_some_and(|r| !r.items.is_empty())
            {
                self.break_machine();
            }
            return;
        }
        if !self.read {
            self.read = true;
            let took = self.records[A].as_ref().unwrap().migrated;
            if took && self.policy != Policy::NoMarker {
                // The record came from the disk with its marker: the storage need not keep it.
                self.storage.legacy = None;
            } else if let Some(legacy) = self.storage.legacy.clone() {
                self.marked = true;
                let r = self.records[A].as_mut().unwrap();
                r.migrated = true;
                match legacy {
                    Legacy::Removal(units) => self.join(Ent { id: (0xB000, 0, 0), chunk: A, units }),
                    Legacy::Batch(c, units, name) => r.owed.push((c, units, name)),
                }
            }
        }
        if let Some((id, units)) = self.pending.take() {
            self.ents.push(Ent { id, chunk: A, units });
        }
    }
    /// The machine is broken: its stock spills in its chunk at once, vanilla's and the mod's alike
    /// (`RemovalInStorage`'s removal waits for its group in the storage, `pay`).
    fn break_machine(&mut self) {
        let r = self.records[A].as_mut().unwrap();
        let items = std::mem::take(&mut r.items);
        if self.policy == Policy::RemovalInStorage {
            self.storage.removed = Some(items);
        } else {
            self.join(Ent { id: (0xB000, 0, 0), chunk: A, units: items });
        }
    }
    /// What waits in A's record for a chunk that is loaded now, put out.
    fn pay(&mut self) {
        self.settle();
        let loaded = self.loaded;
        // `RemovalInStorage`: a removal whose group is in (the hopper's chunk, a member) spills in its chunk.
        if loaded[A] && loaded[HOPPER] && let Some(units) = self.storage.removed.take().filter(|u| !u.is_empty()) {
            self.join(Ent { id: (0xB000, 0, 0), chunk: A, units });
        }
        let owed = if self.policy == Policy::InStorage {
            &mut self.storage.owed
        } else {
            let Some(r) = self.records[A].as_mut().filter(|_| loaded[A]) else { return };
            &mut r.owed
        };
        let (due, kept): (Vec<_>, Vec<_>) = std::mem::take(owed).into_iter().partition(|(c, ..)| loaded[*c]);
        *owed = kept;
        for (c, units, name) in due {
            // The ledger keeps the name of what it paid (`ChunkLedger::take_owed`), in A's record.
            if self.policy != Policy::InStorage && let Some(r) = self.records[A].as_mut() && !r.paid.contains(&name) {
                r.paid.push(name);
            }
            self.join(Ent { id: (u16::from(units[0].1), 0, 0), chunk: c, units });
        }
    }
    /// A stack joins the world; a copy of one already here, or of one that split since (read again from a file or
    /// a record older than the stack's move), is a stack of its own.
    fn join(&mut self, mut e: Ent) {
        e.id = self.free_name(e.id);
        self.ents.push(e);
    }
    /// `id`, or the next root no stack here has, nor the one a command's break holds for its spill.
    fn free_name(&self, mut id: Id) -> Id {
        while self.ents.iter().map(|x| x.id).chain(self.pending.as_ref().map(|p| p.0)).any(|x| related(x, id)) {
            id.0 = id.0.wrapping_add(0x1000);
        }
        id
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
    /// The engine's unload: the chunk's entities, then its record. The mod hears it with the chunk still in place and
    /// takes what it has not taken first (`UnloadWithoutHome` only after the chunk left).
    fn unload(&mut self, c: usize) {
        if self.policy != Policy::UnloadWithoutHome {
            self.settle();
        }
        let e = self.ents_of(c);
        self.queue(Write::Entities(c, e));
        if let Some(r) = self.records[c].clone() {
            self.queue(Write::Region(c, r));
        }
        self.ents.retain(|e| e.chunk != c);
        self.loaded[c] = false;
        self.records[c] = None;
        if self.policy == Policy::UnloadWithoutHome {
            self.settle();
        }
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
        // `world_saving`: before anything is taken (`PublishAfterCapture` only after).
        if self.policy != Policy::PublishAfterCapture {
            self.settle();
        }
        // A's record with its marker is in this save (loaded) or queued by its unload before it.
        let captured = self.marked && self.storage.legacy.is_some();
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
        if self.policy == Policy::PublishAfterCapture {
            self.settle();
        }
        if wait {
            while !self.queue.is_empty() {
                self.write_next();
            }
        }
        // Written before this storage: the earlier version's obligation need not be kept any longer.
        if captured && (wait || self.policy == Policy::RetireEarly) {
            self.storage.legacy = None;
        }
        self.disk.player = self.player.clone();
        if self.policy != Policy::Vanilla {
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
        self.pending = None;
        self.read = false;
        self.marked = false;
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
            out.extend(r.owed.into_iter().flat_map(|(_, u, _)| u));
        }
        out.extend(self.storage.owed.iter().flat_map(|(_, u, _)| u.iter().copied()));
        out.extend(self.storage.removed.iter().flatten().copied());
        out.extend(self.pending.iter().flat_map(|p| p.1.iter().copied()));
        // The earlier version's obligation, while A's record is not read: taken up then unless the record took it.
        let took = match &self.records[A] {
            Some(r) if self.loaded[A] => r.migrated,
            _ => self.read_region(A).migrated,
        };
        if !self.read && (!took || self.policy == Policy::NoMarker) {
            match &self.storage.legacy {
                Some(Legacy::Removal(u) | Legacy::Batch(_, u, _)) => out.extend(u.iter().copied()),
                None => {}
            }
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
    Break,
    /// A command breaks the machine between two ticks.
    Command,
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
    if breakable(w) {
        out.push(Op::Break);
        out.push(Op::Command);
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

/// Whether the machine stands with a stock to spill.
fn breakable(w: &World) -> bool {
    w.loaded[A]
        && w.storage.removed.is_none()
        && w.pending.is_none()
        && w.records[A].as_ref().is_some_and(|r| !r.items.is_empty())
}

/// Whether `op` (chosen in the mod's world) can be played in `w`: vanilla's world skips what concerns a stack only the
/// mod's has.
fn playable(w: &World, op: Op) -> bool {
    let at = |id: Id| w.ent(id).map(|i| &w.ents[i]);
    match op {
        Op::Fire(_) => !w.fired && w.loaded[A],
        Op::Break | Op::Command => breakable(w),
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
        Op::Break => w.break_machine(),
        Op::Command => w.command_break(),
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
    // The end of the tick: what was kept for a chunk that is loaded now is put out. A command runs between ticks.
    if op != Op::Command {
        w.pay();
    }
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
fn explore(policy: Policy, depth: usize) -> (Result<u128, (Vec<Op>, String)>, Search) {
    explore_from(initial(policy), depth)
}

/// `explore` from `start`.
fn explore_from(start: World, depth: usize) -> (Result<u128, (Vec<Op>, String)>, Search) {
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
    let found = go(&start, depth, &mut Vec::new(), &mut s);
    (found, s)
}

fn depth(default: usize) -> usize {
    std::env::var("NTM_SAVE_MODEL_DEPTH").ok().and_then(|d| d.parse().ok()).unwrap_or(default)
}

#[test]
fn the_mods_world_saved_with_its_storage_holds_what_vanillas_holds() {
    let n = depth(9);
    let (found, s) = explore(Policy::Committed, n);
    match found {
        Ok(sequences) => println!("committed: depth {n}: {} states visited, {} distinct, {sequences} complete sequences", s.states, s.seen.len()),
        Err((path, why)) => panic!("{why}\n  {path:?}"),
    }
}

#[test]
fn long_random_plays_hold_what_vanillas_holds() {
    let walks: usize = std::env::var("NTM_SAVE_MODEL_WALKS").ok().and_then(|n| n.parse().ok()).unwrap_or(2_000);
    let length: usize = std::env::var("NTM_SAVE_MODEL_WALK").ok().and_then(|n| n.parse().ok()).unwrap_or(30);
    let mut seed: u64 = std::env::var("NTM_SAVE_MODEL_SEED").ok().and_then(|n| n.parse().ok()).unwrap_or(0x9e37_79b9_7f4a_7c15);
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

#[test]
fn a_mod_that_keeps_what_it_owes_in_its_storage_is_caught() {
    // Its storage is written at the save's instant, the machine's record later by the writer: a death between the two
    // finds the owed shot in the storage and the stock it was taken from in the record.
    let (found, s) = explore(Policy::InStorage, 6);
    let (path, why) = found.expect_err("a storage that holds items must be caught");
    println!("in storage: caught after {} states: {why}\n  {path:?}", s.states);
    assert!(why.starts_with("duplicate") || why.starts_with("lost"), "{why}");
}

#[test]
fn a_mod_that_keeps_a_removal_in_its_storage_is_caught() {
    // R6-2 / F1: the storage is written at the save's instant, the machine's record later; a death between the two finds
    // the removal with its stock in the storage and the machine whole in the record.
    let (found, s) = explore(Policy::RemovalInStorage, 7);
    let (path, why) = found.expect_err("a removal kept in the storage must be caught");
    println!("removal in storage: caught after {} states: {why}\n  {path:?}", s.states);
    assert!(why.starts_with("duplicate") || why.starts_with("lost"), "{why}");
}

#[test]
fn a_mod_whose_shot_names_go_back_with_its_storage_is_caught() {
    // R6-1: a shot paid from what A owed leaves its name in A's record; the storage goes back with a death, and the
    // next shot gets that name again: taken for the paid one, it is not put out.
    // The world opens with C out of reach. The shot owed in A; C in, paid, its name in A's record; A's record written
    // by its unload (its entities, then its record); a death sets the storage's count back; the next session's shot
    // takes that name and is not put out: 8 operations.
    let mut start = initial(Policy::CounterIds);
    start.loaded[C] = false;
    if let Some(vanilla) = start.shadow.as_mut() {
        vanilla.loaded[C] = false;
    }
    let (found, s) = explore_from(start, 9);
    let (path, why) = found.expect_err("names that go back with the storage must be caught");
    println!("counter ids: caught after {} states: {why}\n  {path:?}", s.states);
    assert!(why.starts_with("duplicate") || why.starts_with("lost"), "{why}");
}

/// Both starts from an earlier version's storage: a removal (the stock spilled in vanilla's world) and a shot owed
/// into C. Every play holds what vanilla's holds, through saves that wait or not, unloads, writes and deaths, two
/// openings and more.
#[test]
fn a_world_opened_from_an_earlier_storage_holds_what_vanillas_holds() {
    let n = depth(8);
    for removal in [true, false] {
        let (found, s) = explore_from(initial_legacy(Policy::Committed, removal), n);
        match found {
            Ok(sequences) => println!(
                "legacy {}: depth {n}: {} states visited, {} distinct, {sequences} complete sequences",
                if removal { "removal" } else { "batch" },
                s.states,
                s.seen.len()
            ),
            Err((path, why)) => panic!("legacy removal {removal}: {why}\n  {path:?}"),
        }
    }
}

fn caught(start: World, depth: usize, what: &str) {
    let (found, s) = explore_from(start, depth);
    let (path, why) = found.expect_err(what);
    println!("{what}: caught after {} states: {why}\n  {path:?}", s.states);
    assert!(why.starts_with("duplicate") || why.starts_with("lost"), "{why}");
}

#[test]
fn a_mod_that_takes_a_commands_break_after_the_save_took_the_chunks_is_caught() {
    // A command breaks the machine, a waited save takes the chunks before the mod spills its stock, a death: the
    // save's entities lack it, its record the machine; vanilla's entities hold it.
    caught(initial(Policy::PublishAfterCapture), 4, "a break taken after the capture must be caught");
}

#[test]
fn a_mod_that_takes_a_commands_break_after_its_chunk_left_is_caught() {
    // A command breaks the machine, its chunk leaves with its entities and record written without the stock, which the
    // mod holds in memory only; a death loses it; vanilla's entities hold it.
    caught(initial(Policy::UnloadWithoutHome), 6, "a break taken after its chunk left must be caught");
}

#[test]
fn a_mod_whose_storage_forgets_an_earlier_obligation_at_a_save_that_does_not_wait_is_caught() {
    // R7-1: the obligation taken up into A's record in memory, a save that does not wait writes a storage without it
    // while the writer still holds A's record; a death finds neither.
    for removal in [true, false] {
        caught(initial_legacy(Policy::RetireEarly, removal), 4, "a storage retiring before the record is written must be caught");
    }
}

#[test]
fn a_mod_that_takes_up_an_earlier_obligation_its_record_took_is_caught() {
    // F-novo-2 / N2: A's record with the marker and the spilled stock reach the disk (its unload), the storage is the
    // old one; opened again, a mod that does not read the marker spills the stock a second time.
    caught(initial_legacy(Policy::NoMarker, true), 7, "an obligation taken up twice must be caught");
}

#[path = "common/world_effects_order_model.rs"]
mod world_effects_order_model;
