//! An exhaustive check of how a machine's emitted items survive saves, unloads and process deaths, against vanilla.
//!
//! The model has the origin chunk A (a machine's block entity with items, its record R_A), a chunk with a hopper
//! (its record holds what the hopper took), a third chunk, the entity files of all three, the player's inventory and
//! file (written in place by a save, apart from the writer), the host's write-ahead storage (KV), and the engine's one
//! region writer: a FIFO queue whose records of one
//! chunk coalesce (a region record is skipped only for a newer one of its chunk queued with no entity batch between
//! them; entity records are all written) and that a kill drops. Items are atoms; an emission stamps its atoms with a generation, and what a ledger
//! puts back into the world on its own carries a mark of its own.
//!
//! Every sequence of operations up to a depth is played twice: by the policy under test and, step for step, by
//! vanilla (the same machine and items, no ledger, no replay, the engine's own write order). The writers of the two
//! keep pace: when the policy's writer has written what an operation queued, vanilla's has too. In every state reached:
//!
//! * (I), for the ledgers that keep only the applied effect: no entity file or other chunk's record holds an emitted
//!   atom while the origin's record on disk does not hold the effect (a replay would put it out again). The player's
//!   file is left out: a save that does not wait writes it ahead of the writer, as vanilla's does;
//! * (D) no atom is held more than once where vanilla's world holds it fewer times (by the block entity and an emission,
//!   by an emission and what a ledger put out again, by two files);
//! * (L) no atom is gone that vanilla's world still holds.
//!
//! (I) is not asked of an emission part of which went through the player's inventory: its record then waits for the
//! save, as vanilla's does (written earlier, it would lose what the player holds where vanilla keeps it); (L) and (D) are,
//! and those states are counted.
//! A state reached by a death between two writes of one save or unload, or a disk in the middle of one, is vanilla's
//! window between a save's regions and its entities; copies of one generation are one stack read again from a file it
//! left after the file was written, the engine's own for any entity that crosses chunks. What such a state shows is
//! counted apart, by kind, never hidden; a ledger's own duplicate is two generations of one atom. `Tracking` models the rules of the ledger that followed emitted stacks; `EndOfTick` keeps only the applied
//! effect and queues the record at the end of the tick that applied it; `Late` queues it only before a write that
//! may hold the emission's items, followed by what holds them (see `World::flush_origin`).
use std::collections::{BTreeMap, BTreeSet, HashMap};

type Atom = u8;
/// An atom as some copy holds it: (atom, generation of the emission that put it out: 0 for the block entity's own;
/// `RECREATED` added for what a ledger put out again on its own).
type Unit = (Atom, u8);
/// Added to the generation of what the tracking ledger creates (a respawn, a bump).
const RECREATED: u8 = 100;
/// A stack's name: the root it was put out as (the emission's generation, a ledger's respawn, a copy read again from a
/// file), then its path down the splits since (their count, and which side of each it took). A split names both halves
/// anew, so a name is never given twice, and both worlds name one stack alike.
type Id = (u16, u8, u64);

/// One name is the other's or names one of its splits: two stacks that both lived would name a split alike.
fn related(a: Id, b: Id) -> bool {
    let shorter = a.1.min(b.1);
    let mask = (1u64 << shorter) - 1;
    a.0 == b.0 && a.2 & mask == b.2 & mask
}
const ATOMS: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Policy {
    Vanilla,
    Tracking,
    EndOfTick,
    Late,
}

const A: usize = 0;
/// The chunk with a hopper, whose record holds what it took.
const HOPPER: usize = 1;
const CHUNKS: usize = 3;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Ent {
    /// Named by its lineage (the emission, then its splits), so that both worlds call one stack the same.
    id: Id,
    chunk: usize,
    units: Vec<Unit>,
    /// Tracking: the stack of the ledger it belongs to (saved with the entity).
    tag: Option<u8>,
}

/// What a chunk's record holds.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
struct Record {
    /// The machine's items still in its block entity (A), or the hopper's (HOPPER).
    items: Vec<Unit>,
    /// The effect is applied in this record (its atoms left the block entity).
    applied: bool,
    /// Tracking: the stacks the ledger follows and what each holds; the stacks that folded into another.
    owed: BTreeMap<u8, Vec<Unit>>,
    gone: BTreeSet<u8>,
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
    /// Entity batches queued before it.
    epoch: u32,
    /// The operation that queued it.
    at: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Disk {
    region: [Record; CHUNKS],
    entities: [Vec<Ent>; CHUNKS],
    player: Vec<Unit>,
    /// The effect is named by the write-ahead storage: a reopened world replays it where the record lacks it.
    intent: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct World {
    policy: Policy,
    disk: Disk,
    queue: Vec<Queued>,
    epoch: u32,
    /// The operation being played.
    at: u32,
    /// The operation whose write the writer wrote last.
    last_at: Option<u32>,
    loaded: [bool; CHUNKS],
    /// The records in memory: of A and of the hopper's chunk while loaded, and (tracking) A's while its debt is followed.
    records: [Option<Record>; CHUNKS],
    ents: Vec<Ent>,
    player: Vec<Unit>,
    next_tag: u8,
    respawns: u32,
    generation: u8,
    /// EndOfTick: the origin's record as last queued.
    flushed: Option<Record>,
    emitted: bool,
    /// Late: the origin's record gained the effect since it was last queued; the emission's entities followed, and
    /// the chunks that may hold its atoms otherwise (where a stack of it folded, the hopper that took one).
    due: bool,
    drops: BTreeSet<Id>,
    holders: BTreeSet<usize>,
    /// Late: part of the due emission went through the player's inventory, whose file only a save writes: the record
    /// waits for the save, as vanilla's; and the process died while it did.
    picked: bool,
    /// The emissions (by generation) part of which went through the player's inventory.
    picked_gens: BTreeSet<u8>,
    /// The process died between two writes of one operation.
    died_mid_write: bool,
    /// Vanilla's world, played step for step.
    shadow: Option<Box<World>>,
}

fn initial(policy: Policy) -> World {
    let machine = Record { items: (0..ATOMS).map(|a| (a, 0)).collect(), ..Default::default() };
    let mut region: [Record; CHUNKS] = Default::default();
    region[A] = machine.clone();
    World {
        policy,
        disk: Disk { region, entities: Default::default(), player: Vec::new(), intent: false },
        queue: Vec::new(),
        epoch: 0,
        at: 0,
        last_at: None,
        loaded: [true; CHUNKS],
        records: [Some(machine.clone()), Some(Record::default()), None],
        ents: Vec::new(),
        player: Vec::new(),
        next_tag: 1,
        respawns: 0,
        generation: 0,
        flushed: Some(machine),
        emitted: false,
        due: false,
        drops: BTreeSet::new(),
        holders: BTreeSet::new(),
        picked: false,
        picked_gens: BTreeSet::new(),
        died_mid_write: false,
        shadow: (policy != Policy::Vanilla).then(|| Box::new(initial(Policy::Vanilla))),
    }
}

impl World {
    // ------------------------------------------------------------------ the engine's writer

    fn queue(&mut self, write: Write) {
        if matches!(write, Write::Entities(..)) {
            self.epoch += 1;
        }
        self.queue.push(Queued { write, epoch: self.epoch, at: self.at });
    }
    /// The writer writes its next record, unless it is a region record and a newer one of its chunk is queued with no
    /// entity batch between them; an entity record is always written (a region record written before it may rely on
    /// it). Returns the operation that queued it and its target.
    fn write_next(&mut self) -> (u32, (u8, usize)) {
        let q = self.queue.remove(0);
        let target = q.write.target();
        self.last_at = Some(q.at);
        let superseded = matches!(q.write, Write::Region(..)) && self.queue.iter().any(|n| n.write.target() == target && n.epoch == q.epoch);
        if !superseded {
            match q.write {
                Write::Region(c, r) => self.disk.region[c] = r,
                Write::Entities(c, e) => self.disk.entities[c] = e,
            }
        }
        (q.at, target)
    }
    /// Vanilla's writer keeps pace with the policy's, which just wrote `target` of operation `at`: everything queued
    /// before that operation, and of that operation up to the same target, or all of it once the policy's is done.
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
    /// Between two writes of one operation: the disk shows part of it.
    fn mid_write(&self) -> bool {
        self.queue.first().is_some_and(|q| Some(q.at) == self.last_at)
    }
    /// What a load reads: the newest queued record, else the disk.
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
    fn has_record(c: usize) -> bool {
        c == A || c == HOPPER
    }

    // ------------------------------------------------------------------ the game

    /// The machine's effect: its write-ahead intent, then two atoms out of the block entity into one entity at A.
    fn emit(&mut self) {
        if self.policy != Policy::Vanilla {
            self.disk.intent = true;
        }
        self.apply_effect();
    }
    fn apply_effect(&mut self) {
        let r = self.records[A].as_mut().unwrap();
        if r.applied {
            return;
        }
        r.items.retain(|u| u.0 > 1);
        r.applied = true;
        self.generation += 1;
        let units = vec![(0, self.generation), (1, self.generation)];
        let tag = (self.policy == Policy::Tracking).then(|| {
            let t = self.next_tag;
            self.next_tag += 1;
            r.owed.insert(t, units.clone());
            t
        });
        let id = (u16::from(self.generation), 0, 0);
        self.ents.push(Ent { id, chunk: A, units, tag });
        self.emitted = true;
        if self.policy == Policy::Late {
            self.due = true;
            self.drops.insert(id);
        }
    }
    /// The stack splits in two (fragments of one emission: two stacks of one ledger).
    fn split(&mut self, id: Id) {
        let i = self.ent(id).unwrap();
        let moved = self.ents[i].units.pop().unwrap();
        let (root, len, path) = id;
        self.ents[i].id = (root, len + 1, path);
        let mut e = self.ents[i].clone();
        e.id = (root, len + 1, path | 1 << len);
        e.units = vec![moved];
        if let Some(old) = e.tag {
            let t = self.next_tag;
            self.next_tag += 1;
            e.tag = Some(t);
            if let Some(r) = self.records[A].as_mut().filter(|r| r.owed.contains_key(&old)) {
                r.owed.get_mut(&old).unwrap().retain(|u| *u != moved);
                r.owed.insert(t, vec![moved]);
            }
        }
        if self.drops.remove(&id) {
            self.drops.insert(self.ents[i].id);
            self.drops.insert(e.id);
        }
        self.ents.push(e);
    }
    fn fold(&mut self, from: Id, into: Id) {
        let (i, j) = (self.ent(from).unwrap(), self.ent(into).unwrap());
        let units = std::mem::take(&mut self.ents[i].units);
        let (gone, kept, chunk) = (self.ents[i].tag, self.ents[j].tag, self.ents[i].chunk);
        self.ents[j].units.extend(units.iter().copied());
        self.ents.remove(i);
        if let (Some(g), Some(k)) = (gone, kept)
            && let Some(r) = self.records[A].as_mut()
            && r.owed.contains_key(&g)
        {
            r.owed.remove(&g);
            r.gone.insert(g);
            r.owed.entry(k).or_default().extend(units);
        }
        // Late: the stack is gone and no pickup took it: its atoms are in a stack of that chunk now.
        if self.drops.remove(&from) {
            self.holders.insert(chunk);
        }
    }
    fn pickup(&mut self, id: Id) {
        let e = self.ents.remove(self.ent(id).unwrap());
        self.player.extend(e.units.iter().copied());
        if let Some(t) = e.tag
            && let Some(r) = self.records[A].as_mut()
        {
            r.owed.remove(&t);
        }
        // Late: the pickup is told apart; its items are in the player's file only once a save writes it, so the
        // record waits for that save (written earlier, it would lose them where vanilla keeps them in the machine).
        if self.drops.remove(&id) {
            self.picked = true;
            self.picked_gens.extend(e.units.iter().map(|u| u.1));
        }
    }
    /// The hopper takes a stack lying in its chunk into its block entity.
    fn absorb(&mut self, id: Id) {
        let e = self.ents.remove(self.ent(id).unwrap());
        self.records[HOPPER].as_mut().unwrap().items.extend(e.units.iter().copied());
        if let Some(t) = e.tag
            && let Some(r) = self.records[A].as_mut()
        {
            r.owed.remove(&t);
        }
        if self.drops.remove(&id) {
            self.holders.insert(HOPPER);
        }
    }
    /// Late: the chunks that may hold the emission's atoms: the origin, where its stacks lie, where some went.
    fn holding(&self) -> BTreeSet<usize> {
        let mut out: BTreeSet<usize> = self.holders.clone();
        out.insert(A);
        out.extend(self.ents.iter().filter(|e| self.drops.contains(&e.id)).map(|e| e.chunk));
        out
    }
    /// Late: the origin's record goes to the writer now, then the records and the entities of every loaded chunk that
    /// may hold the emission's atoms, so that a death after them keeps the atoms where they are; nothing is due any more.
    fn flush_origin(&mut self) {
        let holding: Vec<usize> = self.holding().into_iter().filter(|c| self.loaded[*c]).collect();
        if let Some(r) = self.records[A].clone().filter(|_| self.loaded[A]) {
            self.queue(Write::Region(A, r));
        }
        for &c in &holding {
            if let Some(r) = self.records[c].clone().filter(|_| c != A) {
                self.queue(Write::Region(c, r));
            }
        }
        for &c in &holding {
            let e = self.ents_of(c);
            self.queue(Write::Entities(c, e));
        }
        self.due = false;
        self.drops.clear();
        self.holders.clear();
        self.picked = false;
    }
    fn unload(&mut self, c: usize) {
        // Late: before the engine queues what leaves, the origin's record when that may hold the emission's atoms:
        // the origin itself, a chunk where one of its stacks lies, or one that took some (a fold, the hopper).
        if self.due && !self.picked && self.holding().contains(&c) {
            self.flush_origin();
        }
        let e = self.ents_of(c);
        // The engine's unload: the chunk's entities, then its region record.
        self.queue(Write::Entities(c, e));
        if let Some(r) = self.records[c].clone() {
            self.queue(Write::Region(c, r));
        }
        self.ents.retain(|e| e.chunk != c);
        self.loaded[c] = false;
        // The tracking ledger stays in memory while a debt of it is followed, here or away (as `LEDGERS`).
        let followed = c == A && self.policy == Policy::Tracking && self.records[A].as_ref().is_some_and(|r| !r.owed.is_empty());
        if !followed {
            self.records[c] = None;
            if c == A {
                self.flushed = None;
            }
        }
    }
    fn load(&mut self, c: usize) {
        self.loaded[c] = true;
        // A copy of a stack already here, or of one that split since (it crossed chunks after its file was written),
        // is a stack of its own.
        for mut e in self.read_entities(c) {
            e.chunk = c;
            while self.ents.iter().any(|x| related(x.id, e.id)) {
                e.id.0 = e.id.0.wrapping_add(0x1000);
            }
            self.ents.push(e);
        }
        if Self::has_record(c) && self.records[c].is_none() {
            self.records[c] = Some(self.read_region(c));
            if c == A {
                self.flushed = self.records[A].clone();
            }
        }
        // A reopened world replays the effect its record does not hold. Vanilla has no such storage, but its machine,
        // back with what it held, runs its step again.
        let again = if self.policy == Policy::Vanilla { self.emitted } else { self.disk.intent };
        if c == A && again && !self.records[A].as_ref().unwrap().applied {
            self.apply_effect();
        }
    }
    /// A save: the loaded chunks' records in the order the batch takes (`a_first`) and their entities, to the writer;
    /// then the player's file (in `level.dat`), written in place: after the writer is done when the save waits, at
    /// once when it does not.
    fn save(&mut self, wait: bool, a_first: bool) {
        let order = if a_first { [A, HOPPER] } else { [HOPPER, A] };
        for c in order {
            if let Some(r) = self.records[c].clone().filter(|_| self.loaded[c]) {
                self.queue(Write::Region(c, r));
                if c == A {
                    self.flushed = self.records[A].clone();
                    self.due = false;
                    self.drops.clear();
                    self.holders.clear();
                    self.picked = false;
                }
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
        if wait {
            // Tracking: a confirmed save lets go of the stacks whose entities are all loaded, and the engine writes
            // again what that changed.
            let followed: BTreeSet<u8> = self.records[A].as_ref().map(|r| r.owed.keys().copied().collect()).unwrap_or_default();
            let all_here = followed.iter().all(|t| self.ents.iter().any(|e| e.tag == Some(*t)));
            if self.policy == Policy::Tracking && !followed.is_empty() && all_here && self.loaded[A] {
                self.records[A].as_mut().unwrap().owed.clear();
                for e in &mut self.ents {
                    e.tag = None;
                }
                self.save(true, a_first);
            }
        }
    }
    /// Tracking: the ledger's pass, with the ledger in memory only: copies of a folded stack go, a tagged copy no
    /// ledger owes is let go, a followed stack is bumped to what is owed, and one that no loaded entity and no
    /// unloaded holder's file shows is spawned again at A.
    fn reconcile(&mut self) {
        if self.policy != Policy::Tracking {
            return;
        }
        let Some(r) = self.records[A].clone() else { return };
        self.ents.retain(|e| !e.tag.is_some_and(|t| r.gone.contains(&t)));
        for e in &mut self.ents {
            if e.tag.is_some_and(|t| !r.owed.contains_key(&t)) {
                e.tag = None;
            }
        }
        for (tag, owed) in &r.owed {
            let held: Vec<Atom> = self.ents.iter().filter(|e| e.tag == Some(*tag)).flat_map(|e| e.units.iter().map(|u| u.0)).collect();
            let missing: Vec<Unit> = owed.iter().filter(|u| !held.contains(&u.0)).map(|u| (u.0, u.1 % RECREATED + RECREATED)).collect();
            if missing.is_empty() || !self.loaded[A] {
                continue;
            }
            if let Some(e) = self.ents.iter_mut().find(|e| e.tag == Some(*tag)) {
                e.units.extend(missing);
                continue;
            }
            let shown = (0..CHUNKS).any(|c| !self.loaded[c] && self.read_entities(c).iter().any(|e| e.tag == Some(*tag)));
            if !shown {
                self.respawns += 1;
                self.ents.push(Ent { id: (0x0100 + self.respawns as u16, 0, 0), chunk: A, units: missing, tag: Some(*tag) });
            }
        }
    }
    /// EndOfTick: the end of the tick: the origin's record, if it changed, goes to the writer.
    fn flush_records(&mut self) {
        if self.policy == Policy::EndOfTick && self.loaded[A] && self.records[A] != self.flushed {
            self.flushed = self.records[A].clone();
            let r = self.records[A].clone().unwrap();
            self.queue(Write::Region(A, r));
        }
    }
    fn kill(&mut self) {
        self.died_mid_write |= self.mid_write();
        self.queue.clear();
        self.loaded = [false; CHUNKS];
        self.records = [None, None, None];
        self.flushed = None;
        self.ents.clear();
        self.player = self.disk.player.clone();
        self.due = false;
        self.drops.clear();
        self.holders.clear();
        self.picked = false;
    }

    // ------------------------------------------------------------------ what the world holds

    /// Every unit the world holds now: the loaded chunks in memory, the others as a load would read them, the
    /// player, the machine's and the hopper's block entities.
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
        }
        out
    }
    /// The copies of each atom, by the generation each holds.
    fn copies(&self) -> BTreeMap<Atom, Vec<u8>> {
        let mut out: BTreeMap<Atom, Vec<u8>> = BTreeMap::new();
        for (a, g) in self.units() {
            out.entry(a).or_default().push(g);
        }
        out
    }
    /// (D) and (L), against vanilla's world in the same state of the play: more than one copy of an atom where vanilla
    /// holds fewer, or none where vanilla holds one.
    fn against_vanilla(&self) -> Option<(&'static str, String)> {
        let vanilla = self.shadow.as_ref()?.copies();
        let here = self.copies();
        for atom in 0..ATOMS {
            let (mine, theirs) = (here.get(&atom).map_or(0, Vec::len), vanilla.get(&atom).map_or(0, Vec::len));
            if mine > 1 && mine > theirs {
                // Copies of one generation are one stack read again from a file that an entity left after it was
                // written: the engine's own for any entity that crosses chunks.
                let one = here[&atom].iter().all(|g| *g == here[&atom][0]);
                let kind = if one { "crossing" } else { "duplicate" };
                return Some((kind, format!("atom {atom} held {:?} (vanilla: {:?})", here[&atom], vanilla.get(&atom))));
            }
            if mine == 0 && theirs > 0 {
                return Some(("lost", format!("atom {atom} lost, vanilla holds it ({:?})", vanilla[&atom])));
            }
        }
        None
    }
    /// (I): an emitted atom in an entity file or the hopper's record while the origin's record on disk does not hold the
    /// effect. The player's file is not one: a save that does not wait writes it at once, ahead of the writer, as
    /// vanilla's does (a pickup split between the player's file and the entity's); (L) and (D) still hold there.
    fn unordered(&self) -> Option<(&'static str, String)> {
        if !matches!(self.policy, Policy::EndOfTick | Policy::Late) || self.disk.region[A].applied {
            return None;
        }
        let emitted = self.disk.entities.iter().flatten().flat_map(|e| e.units.iter()).chain(&self.disk.region[HOPPER].items).find(|u| u.1 > 0 && !self.picked_gens.contains(&u.1));
        emitted.map(|u| ("unordered", format!("{u:?} on disk before the record that emitted it")))
    }
    /// The player's file holds an emitted atom while the origin's record on disk does not hold the effect: vanilla's
    /// split of a pickup, where (L) and (D) are still checked.
    fn player_ahead(&self) -> bool {
        matches!(self.policy, Policy::EndOfTick | Policy::Late) && !self.disk.region[A].applied && self.disk.player.iter().any(|u| u.1 > 0)
    }
    /// A file other than the player's holds an atom of an emission that went partly through the player's inventory
    /// while the origin's record on disk does not hold the effect: vanilla's timing, where (L) and (D) are still checked.
    fn picked_ahead(&self) -> bool {
        matches!(self.policy, Policy::EndOfTick | Policy::Late)
            && !self.disk.region[A].applied
            && self.disk.entities.iter().flatten().flat_map(|e| e.units.iter()).chain(&self.disk.region[HOPPER].items).any(|u| u.1 > 0 && self.picked_gens.contains(&u.1))
    }
    /// The first property this state breaks, and what of vanilla's it is, if it is: its window between two writes of
    /// one save or unload (a death between them, or a disk in the middle of one), or an entity's copies after a crossing.
    fn violation(&self) -> Option<(&'static str, String, Option<&'static str>)> {
        // The model's own soundness: no stack's name is another's or names one of its splits.
        let clash = self.ents.iter().enumerate().any(|(i, a)| self.ents[i + 1..].iter().any(|b| related(a.id, b.id)));
        if clash {
            return Some(("model", format!("two stacks' names clash: {:?}", self.ents), None));
        }
        let (kind, why) = self.unordered().or_else(|| self.against_vanilla())?;
        let vanillas = if self.died_mid_write || self.mid_write() {
            Some(match kind {
                "lost" => "window: lost",
                "unordered" => "window: unordered",
                "crossing" => "window: crossing",
                _ => "window: duplicate",
            })
        } else {
            (kind == "crossing").then_some("crossing")
        };
        Some((kind, why, vanillas))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Op {
    Emit,
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
    if !w.emitted && w.loaded[A] && w.records[A].is_some() {
        out.push(Op::Emit);
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
            // Tracked items fold only into items of their own ledger; vanilla items into any.
            if f.id != e.id && f.chunk == e.chunk && f.tag.is_some() == e.tag.is_some() {
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
    if w.emitted {
        out.push(Op::Kill);
    }
    out
}

/// Whether `op` (chosen in the policy's world) can be played in `w`: vanilla's world skips what concerns a stack
/// only the policy's has (one a replay or a respawn put out).
fn playable(w: &World, op: Op) -> bool {
    let at = |id: Id| w.ent(id).map(|i| &w.ents[i]);
    match op {
        Op::Emit => !w.emitted && w.loaded[A] && w.records[A].is_some(),
        Op::Split(id) => at(id).is_some_and(|e| e.units.len() > 1),
        Op::Move(id, c) => at(id).is_some_and(|e| e.chunk != c) && w.loaded[c],
        Op::Fold(i, j) => i != j && at(i).zip(at(j)).is_some_and(|(e, f)| e.chunk == f.chunk && e.tag.is_some() == f.tag.is_some()),
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
        Op::Emit => w.emit(),
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
}

/// One operation in both worlds, then the end of its tick: the ledger's pass and the queued record.
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
        s.reconcile();
        s.flush_records();
        s.ents.sort();
    }
    w.reconcile();
    w.flush_records();
    w.ents.sort();
    w.shadow = shadow;
}

struct Search {
    /// The states explored, by a 128-bit fingerprint and the operations left, with how many complete sequences start
    /// there: a state explored once is the same subtree every time it is reached again.
    seen: HashMap<(u128, usize), u128>,
    states: usize,
    /// States that show what vanilla's world shows in the same play (its window between two writes of one operation,
    /// an entity's copies after a crossing), by kind, with a first example.
    window: BTreeMap<&'static str, (usize, Vec<Op>, String)>,
    /// States checked with the player's file ahead of the origin's record, and with another file ahead of it holding an
    /// emission that went partly through the player.
    player_ahead: usize,
    picked_ahead: usize,
}

impl Search {
    fn distinct(&self) -> usize {
        self.seen.len()
    }
}

/// The state with the operations of its writes numbered by their order alone: two states that differ only in when
/// they happened explore the same.
fn canonical(w: &World) -> World {
    let mut ats: BTreeSet<u32> = BTreeSet::new();
    for x in std::iter::once(w).chain(w.shadow.as_deref()) {
        ats.extend(x.queue.iter().map(|q| q.at));
        ats.extend(x.last_at);
    }
    let rank: BTreeMap<u32, u32> = ats.into_iter().enumerate().map(|(i, a)| (a, i as u32 + 1)).collect();
    let renumber = |x: &mut World| {
        x.at = 0;
        x.last_at = x.last_at.map(|a| rank[&a]);
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

/// Every sequence of `depth` operations from the initial state; the first counterexample outside vanilla's window
/// whose path `wanted` accepts, and how many complete sequences were covered. States already explored with as many
/// operations left are not explored again: the sequences through them are counted once more.
fn explore_where(policy: Policy, depth: usize, wanted: fn(&[Op]) -> bool) -> (Option<(Vec<Op>, String)>, Search, u128) {
    fn go(w: &World, left: usize, path: &mut Vec<Op>, s: &mut Search, wanted: fn(&[Op]) -> bool) -> Result<u128, (Vec<Op>, String)> {
        s.states += 1;
        let key = (fingerprint(w), left);
        if let Some(n) = s.seen.get(&key) {
            return Ok(*n);
        }
        if let Some((kind, why, vanillas)) = w.violation() {
            let Some(vanillas) = vanillas else {
                return if wanted(path) { Err((path.clone(), format!("{kind}: {why}"))) } else { Ok(1) };
            };
            let e = s.window.entry(vanillas).or_insert_with(|| (0, path.clone(), why));
            e.0 += 1;
            s.seen.insert(key, 1);
            return Ok(1);
        }
        s.player_ahead += usize::from(w.player_ahead());
        s.picked_ahead += usize::from(w.picked_ahead());
        if left == 0 {
            return Ok(1);
        }
        let mut n = 0;
        for op in ops(w) {
            let mut next = w.clone();
            step(&mut next, op);
            path.push(op);
            n += go(&next, left - 1, path, s, wanted)?;
            path.pop();
        }
        s.seen.insert(key, n);
        Ok(n)
    }
    let mut s = Search { seen: HashMap::new(), states: 0, window: BTreeMap::new(), player_ahead: 0, picked_ahead: 0 };
    match go(&initial(policy), depth, &mut Vec::new(), &mut s, wanted) {
        Ok(n) => (None, s, n),
        Err(found) => (Some(found), s, 0),
    }
}

fn depth(default: usize) -> usize {
    std::env::var("NTM_SAVE_MODEL_DEPTH").ok().and_then(|d| d.parse().ok()).unwrap_or(default)
}

#[test]
fn the_tracking_ledger_returns_a_folded_copy_picked_up_in_its_holder_before_its_origin_loads() {
    // R4-1 of the fourth review: a fold, a death, a reopen in the old holder, its copy picked up, the origin loaded.
    let r4 = |p: &[Op]| {
        let kill = p.iter().position(|o| *o == Op::Kill);
        kill.is_some_and(|k| {
            let after = &p[k..];
            let pickup = after.iter().position(|o| matches!(o, Op::Pickup(_)));
            let origin = after.iter().position(|o| *o == Op::Load(A));
            p[..k].iter().any(|o| matches!(o, Op::Fold(..))) && pickup.zip(origin).is_some_and(|(p, o)| p < o)
        })
    };
    let (found, s, _) = explore_where(Policy::Tracking, depth(14), r4);
    let (path, why) = found.expect("the tracking rules let a folded copy come back");
    println!("tracking, R4-1: {why} after {} states\n  {path:?}", s.states);
}

#[test]
fn a_record_queued_at_the_end_of_the_tick_loses_what_vanilla_keeps() {
    // The record of the tick that applied the effect reaches the disk before any file holds the items: a death
    // before the next save loses them, where vanilla's world comes back with them in the machine.
    let (found, s, _) = explore_where(Policy::EndOfTick, 4, |_| true);
    let (path, why) = found.expect("the end-of-tick record loses the emission at a plain death");
    println!("end of tick: {why} after {} states\n  {path:?}", s.states);
    assert!(why.starts_with("lost") && path.contains(&Op::Kill), "{why}");
}

#[test]
fn a_record_queued_before_what_may_hold_the_items_is_no_worse_than_vanilla() {
    let n = depth(9);
    let (found, s, sequences) = explore_where(Policy::Late, n, |_| true);
    println!("late: depth {n}: {} states visited, {} distinct states, {sequences} complete sequences covered", s.states, s.distinct());
    println!("  the player's file ahead of the origin's record, (L) and (D) holding: {} states", s.player_ahead);
    println!("  an emission partly picked up, its other files ahead of the origin's record, (L) and (D) holding: {} states", s.picked_ahead);
    for (kind, (count, path, why)) in &s.window {
        println!("  vanilla's {kind}: {count} states, e.g. {why}\n    {path:?}");
    }
    assert!(found.is_none(), "{found:?}");
}

#[test]
fn long_random_plays_keep_the_late_record_no_worse_than_vanilla() {
    // Plays far longer than the exhaustive search can reach, chosen at random: cheap and deep, they catch what the
    // search would find only at a depth it cannot afford, the model's own faults among them.
    let walks: usize = std::env::var("NTM_SAVE_MODEL_WALKS").ok().and_then(|n| n.parse().ok()).unwrap_or(2_000);
    let length: usize = std::env::var("NTM_SAVE_MODEL_WALK").ok().and_then(|n| n.parse().ok()).unwrap_or(30);
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut random = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let (mut steps, mut vanillas): (usize, BTreeMap<&'static str, usize>) = (0, BTreeMap::new());
    for _ in 0..walks {
        let mut w = initial(Policy::Late);
        let mut path = Vec::new();
        for _ in 0..length {
            if let Some((kind, why, vanilla)) = w.violation() {
                let vanilla = vanilla.unwrap_or_else(|| panic!("{kind}: {why}\n  {path:?}"));
                *vanillas.entry(vanilla).or_default() += 1;
                break;
            }
            let choices = ops(&w);
            let op = choices[random() as usize % choices.len()];
            step(&mut w, op);
            path.push(op);
            steps += 1;
        }
        if let Some((kind, why, None)) = w.violation() {
            panic!("{kind}: {why}\n  {path:?}");
        }
    }
    println!("late, random: {walks} plays of up to {length} operations, {steps} operations; vanilla's: {vanillas:?}");
}
