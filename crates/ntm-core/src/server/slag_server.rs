//! What a slag tap pours (`TileEntityFoundrySlagtap.flow`): the metal of its source goes into the slag block it hits
//! (the same material, up to sixteen blocks) and what is left into a new slag block above it, if that cell is air.
//!
//! The pour is one commit (`ServerHost::commit_machines`): the source's smaller record, the slag block's bigger one,
//! and the new block with its first record (`CellWrite::initial`) are published together, or none is. So at any point
//! the metal is in the source or in the slag, never in both and never in neither: a refused commit leaves the source
//! as it was, and nothing in memory changes before the host has accepted it.
//!
//! One chunk only: the source, the slag it fills and the cell of the new block must be in the same chunk, because a
//! chunk's record is what the world writes atomically and the records of two chunks reach the disk apart (the
//! journal that covers a step over several chunks, `Survivor`, cannot create the missing block of a pour). A slag tap
//! whose pour would leave the source's chunk pours nothing, and the source keeps its metal (`docs/content-foundry.md`).
use super::*;
use super::foundry_server::{Machines, SLAGTAP_FALL, SlagPour, below, melt_of};
use super::transaction::chunks_of;
use crate::foundry::slag::{SLAG_CAP, Slag};
use crate::machine::{Data, Machine};

fn is_air(state: &str) -> bool {
    matches!(blockstate::id_of(state), "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
}

/// The slag a pour of `left` quanta of `mat` onto `hit` fills, and what is left of the metal.
fn fill_into(m: &Machines, hit: &Key, mat: &str, left: u32) -> Option<(Key, Machine, u32)> {
    let x = m.get(hit)?;
    let Data::Slag(s) = &x.data else { return None };
    let n = s.takes(mat, left);
    if n == 0 {
        return None;
    }
    let mut after = x.clone();
    let Data::Slag(a) = &mut after.data else { return None };
    a.melt.amount += n;
    Some((hit.clone(), after, left - n))
}

/// A slag block a step makes: its cell's write (with the first record) and what the server keeps once it is published.
struct Born {
    key: Key,
    machine: Machine,
    json: String,
    screen: Vec<i32>,
    shown: String,
    cell: CellWrite,
}

/// A machine a step changes, as the host will have it.
struct Update {
    key: Key,
    machine: Machine,
    desired: container_bridge::Slots,
    data: Vec<i32>,
    json: String,
}

impl Server {
    /// A new slag of `n` quanta of `mat` in the air cell `snapshot` of `key`.
    fn born_slag(&self, key: &Key, snapshot: CellSnapshot, mat: &str, n: u32) -> Option<Born> {
        let r = &self.ready;
        let generation = r.world.machines.get(key).map(|m| m.generation).unwrap_or_else(|| r.world.generations.get(key).copied().unwrap_or(0));
        let mut m = Machine::new(Kind::Slag, "ntm:slag", None);
        m.generation = generation;
        m.data = Data::Slag(Slag::of(mat, n));
        let json = serde_json::to_string(&m).expect("machine serialization");
        let screen = Self::machine_screen(&mut m.clone());
        let shown = desired_state(&m)?;
        let cell = CellWrite {
            notify: true,
            guard: CellGuard { dim: key.0.clone(), pos: key.1, generation, state: blockstate::id_of(&snapshot.state).to_string() },
            expected: snapshot,
            desired: shown.clone(),
            destroy: false,
            initial: Some(InitialState { json: json.clone(), data: screen.clone() }),
        };
        Some(Born { key: key.clone(), machine: m, json, screen, shown, cell })
    }

    /// `after` as the commit that publishes it (with the `cells` it writes) and as the server will keep it.
    fn update_of(&self, host: &mut impl ServerHost, key: &Key, mut after: Machine, cells: Vec<CellWrite>) -> Option<(MachineCommit, Update)> {
        let r = &self.ready;
        let before = r.world.machines.get(key)?;
        let expected = host.machine_slots(&key.0, key.1)?;
        let (desired, data) = Self::machine_output(&mut after, &expected);
        let json = serde_json::to_string(&after).expect("machine serialization");
        let commit = MachineCommit { cells, guard: CellGuard::machine(key, before), expected_saved: r.native_saved.contains_key(key), dim: key.0.clone(), pos: key.1, expected, desired: desired.clone(), data: data.clone(), json: json.clone() };
        Some((commit, Update { key: key.clone(), machine: after, desired, data, json }))
    }

    /// What the server keeps after the host published a step: the changed machines, the new ones, the gone ones (their
    /// cell's generation is advanced: a later block there is another machine).
    fn settled(&mut self, updates: Vec<Update>, born: Vec<Born>, gone: &[Key]) {
        let r = &mut self.ready;
        for u in updates {
            r.world.machines.insert(u.key.clone(), u.machine);
            r.bridge.last.insert(u.key.clone(), u.desired);
            r.bridge.data.insert(u.key.clone(), u.data);
            r.native_saved.insert(u.key, u.json);
        }
        for b in born {
            r.world.machines.insert(b.key.clone(), b.machine);
            r.loaded.insert(b.key.clone(), true);
            r.seen.insert(b.key.clone());
            r.bridge.last.insert(b.key.clone(), Vec::new());
            r.bridge.data.insert(b.key.clone(), b.screen);
            r.native_saved.insert(b.key.clone(), b.json);
            r.shown.insert(b.key, b.shown);
        }
        for k in gone {
            if let Some(m) = r.world.machines.remove(k) {
                r.world.generations.insert(k.clone(), m.generation.saturating_add(1));
                r.facts_dirty = true;
            }
            r.loaded.remove(k);
            r.seen.remove(k);
            r.shown.remove(k);
            r.native_saved.remove(k);
            r.bridge.last.remove(k);
            r.bridge.data.remove(k);
            r.slag_active.remove(k);
        }
    }

    /// A slag that is a standing machine of the current model, loaded and not being broken.
    fn standing_slag(&self, k: &Key) -> Option<(&Machine, &Slag)> {
        let r = &self.ready;
        let m = r.world.machines.get(k)?;
        let Data::Slag(s) = &m.data else { return None };
        (r.loaded.get(k) == Some(&true) && !r.broken.contains_key(k) && self.model_current(k, m)).then_some((m, s))
    }

    /// The pour a source asked of a slag tap this tick, in one commit; a refusal, or nothing to pour onto, leaves
    /// everything as it was and the source holds its metal.
    pub(super) fn slag_pour(&mut self, host: &mut impl ServerHost, pour: &SlagPour) {
        let SlagPour { src, tap } = pour;
        let r = &self.ready;
        let (Some(from), Some(tap_m)) = (r.world.machines.get(src), r.world.machines.get(tap)) else { return };
        if r.loaded.get(src) != Some(&true) || r.broken.contains_key(src) || !self.model_current(src, from) {
            return;
        }
        let Data::Slagtap(o) = &tap_m.data else { return };
        let behind = tap_m.facing.as_deref().and_then(registry::facing_dir).is_some_and(|f| src.1 == [tap.1[0] - f[0], tap.1[1] - f[1], tap.1[2] - f[2]]);
        let mut source = from.clone();
        let Some((melt, _)) = melt_of(&mut source.data) else { return };
        let (Some(mat), q) = (melt.mat.clone(), melt.amount) else { return };
        if q == 0 || !behind || !o.passes(&mat) {
            return;
        }
        let dim = &src.0;
        let Some(hit) = below(&r.world.machines, host, dim, [tap.1[0], tap.1[1] - 1, tap.1[2]], SLAGTAP_FALL - 1) else { return };
        let here = chunks_of(std::iter::once(src));
        let same_chunk = |k: &Key| chunks_of(std::iter::once(k)) == here;
        // Into the slag it hit, if it is the same material.
        let mut left = q;
        let mut into = None;
        // The slag it hit must be a standing, loaded machine of the current model, as the source is.
        if same_chunk(&hit.cell)
            && self.standing_slag(&hit.cell).is_some()
            && let Some((key, after, rest)) = fill_into(&r.world.machines, &hit.cell, &mat, left)
        {
            left = rest;
            into = Some((key, after));
        }
        // What is left, into a new block above the hit, if that cell is air (`above.isReplaceable`; the other
        // replaceable blocks, plants and snow, are not: `docs/content-foundry.md`).
        let above = (dim.clone(), [hit.cell.1[0], hit.cell.1[1] + 1, hit.cell.1[2]]);
        let mut fresh = None;
        if left > 0 && same_chunk(&above) && !r.world.machines.contains_key(&above) {
            fresh = host.cell_snapshot(dim, above.1).filter(|s| is_air(&s.state) && s.entity.is_none()).map(|expected| {
                let n = left.min(SLAG_CAP);
                left -= n;
                (expected, n)
            });
        }
        if left == q {
            return;
        }
        let Some((melt, _)) = melt_of(&mut source.data) else { return };
        melt.amount = left;
        if left == 0 {
            melt.mat = None;
        }
        let born = fresh.and_then(|(snapshot, n)| self.born_slag(&above, snapshot, &mat, n));
        let cells = born.iter().map(|b| b.cell.clone()).collect();
        let Some((first, source_update)) = self.update_of(host, src, source, cells) else { return };
        let mut commits = vec![first];
        let mut updates = vec![source_update];
        if let Some((key, after)) = into.clone() {
            let Some((c, u)) = self.update_of(host, &key, after, Vec::new()) else { return };
            commits.push(c);
            updates.push(u);
        }
        if !host.commit_machines(&commits) {
            return;
        }
        // Scheduled to flow on the next tick: the slag it filled and the one it made (`scheduleBlockUpdate(.., 1)`).
        let r = &mut self.ready;
        r.slag_active.extend(into.map(|(k, _)| k));
        r.slag_active.extend(born.iter().map(|b| b.key.clone()));
        self.settled(updates, born.into_iter().collect(), &[]);
    }

    /// The slag blocks scheduled for this tick flow, each in one commit (`BlockDynamicSlag.updateTick`): into the air under
    /// it, whole; else into the slag under it of the same material; else, from a fifth of its capacity, to the air at its
    /// four sides. Nothing crosses the chunk's border, and a refusal changes nothing.
    pub(super) fn slag_flow(&mut self, host: &mut impl ServerHost) {
        for key in std::mem::take(&mut self.ready.slag_active) {
            self.slag_step(host, &key);
        }
    }

    fn slag_step(&mut self, host: &mut impl ServerHost, key: &Key) {
        let Some((m, s)) = self.standing_slag(key) else { return };
        let amount = s.melt.amount;
        let dim = &key.0;
        let here = chunks_of(std::iter::once(key));
        let same_chunk = |k: &Key| chunks_of(std::iter::once(k)) == here;
        let r = &self.ready;
        let at = |dx: i32, dy: i32, dz: i32| (dim.clone(), [key.1[0] + dx, key.1[1] + dy, key.1[2] + dz]);
        let air = |host: &mut dyn ServerHost, k: &Key| {
            (same_chunk(k) && !r.world.machines.contains_key(k)).then(|| host.cell_snapshot(&k.0, k.1)).flatten().filter(|c| is_air(&c.state) && c.entity.is_none())
        };
        let below = at(0, -1, 0);
        let Some(me) = host.cell_snapshot(dim, key.1) else { return };
        let gone_cell = |me: CellSnapshot| CellWrite {
            notify: true,
            guard: CellGuard::machine(key, m),
            expected: me,
            desired: "minecraft:air".into(),
            destroy: false,
            initial: None,
        };
        // One emptied by the step before (it gave all it had to the slag under it) goes: no metal is in play, so this is a plain
        // structural write.
        let Some(mat) = s.melt.mat.clone().filter(|_| amount > 0) else {
            if host.commit_structural(&[gone_cell(me)], &[], std::slice::from_ref(key)) {
                self.settled(Vec::new(), Vec::new(), std::slice::from_ref(key));
            }
            return;
        };
        // Into the air under it, whole; the cell it leaves is air (no break, no drop).
        if let Some(snapshot) = air(host, &below) {
            let Some(born) = self.born_slag(&below, snapshot, &mat, amount) else { return };
            let cells = [gone_cell(me), born.cell.clone()];
            if host.commit_structural(&cells, &[], &[key.clone(), below.clone()]) {
                self.settled(Vec::new(), vec![born], std::slice::from_ref(key));
            }
            return;
        }
        // Into the slag under it, the same material, up to its capacity.
        if let Some((bm, bs)) = self.standing_slag(&below).filter(|_| same_chunk(&below))
            && bs.melt.mat.as_deref() == Some(&mat)
            && bs.melt.amount < SLAG_CAP
        {
            let t = amount.min(SLAG_CAP - bs.melt.amount);
            let mut lower = bm.clone();
            let Data::Slag(l) = &mut lower.data else { return };
            l.melt.amount += t;
            let mut upper = m.clone();
            let Data::Slag(u) = &mut upper.data else { return };
            u.melt.amount -= t;
            // Both machines in one commit; if the upper one is emptied it stays, empty, to be removed on the next tick (a
            // cell written to air in a machine commit is a break to the server: its scrap would be dropped).
            let emptied = u.melt.amount == 0;
            if emptied {
                u.melt.mat = None;
            }
            let Some((c1, u1)) = self.update_of(host, key, upper, Vec::new()) else { return };
            let Some((c2, u2)) = self.update_of(host, &below, lower, Vec::new()) else { return };
            if host.commit_machines(&[c1, c2]) {
                self.ready.slag_active.insert(below);
                if emptied {
                    self.ready.slag_active.insert(key.clone());
                }
                self.settled(vec![u1, u2], Vec::new(), &[]);
            }
            return;
        }
        // To the air at its four sides, north, south, east, west (`toSpread = max(amount / (count * 2), 1)`).
        let sides = [at(0, 0, -1), at(0, 0, 1), at(1, 0, 0), at(-1, 0, 0)];
        let free: Vec<(Key, CellSnapshot)> = sides.into_iter().filter_map(|k| air(host, &k).map(|c| (k, c))).collect();
        if amount < SLAG_CAP / 5 || free.is_empty() {
            return;
        }
        let spread = (amount / (free.len() as u32 * 2)).max(1);
        let mut upper = m.clone();
        let Data::Slag(u) = &mut upper.data else { return };
        let mut born = Vec::new();
        for (k, snapshot) in free {
            let Some(b) = self.born_slag(&k, snapshot, &mat, spread) else { return };
            u.melt.amount -= spread;
            born.push(b);
        }
        let cells = born.iter().map(|b| b.cell.clone()).collect();
        let Some((commit, update)) = self.update_of(host, key, upper, cells) else { return };
        if host.commit_machines(std::slice::from_ref(&commit)) {
            self.ready.slag_active.extend(born.iter().map(|b| b.key.clone()));
            self.settled(vec![update], born, &[]);
        }
    }
}
