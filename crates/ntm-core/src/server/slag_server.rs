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

impl Server {
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
        if same_chunk(&hit.cell)
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
        let Some(expected) = host.machine_slots(dim, src.1) else { return };
        let (desired, data) = Self::machine_output(&mut source, &expected);
        let json = serde_json::to_string(&source).expect("machine serialization");
        let mut cells = Vec::new();
        let mut born = None;
        if let Some((snapshot, n)) = fresh {
            let generation = r.world.machines.get(&above).map(|m| m.generation).unwrap_or_else(|| r.world.generations.get(&above).copied().unwrap_or(0));
            let mut m = Machine::new(Kind::Slag, "ntm:slag", None);
            m.generation = generation;
            m.data = Data::Slag(Slag::of(&mat, n));
            let first = serde_json::to_string(&m).expect("machine serialization");
            let screen = Self::machine_screen(&mut m.clone());
            let Some(shown) = desired_state(&m) else { return };
            cells.push(CellWrite {
                notify: true,
                guard: CellGuard { dim: dim.clone(), pos: above.1, generation, state: blockstate::id_of(&snapshot.state).to_string() },
                expected: snapshot,
                desired: shown.clone(),
                destroy: false,
                initial: Some(InitialState { json: first.clone(), data: screen.clone() }),
            });
            born = Some((m, first, screen, shown));
        }
        let mut commits = vec![MachineCommit { cells, guard: CellGuard::machine(src, from), expected_saved: r.native_saved.contains_key(src), dim: dim.clone(), pos: src.1, expected: expected.clone(), desired: desired.clone(), data: data.clone(), json: json.clone() }];
        let mut grown = None;
        if let Some((key, mut after)) = into {
            let Some(slots) = host.machine_slots(dim, key.1) else { return };
            let before = &r.world.machines[&key];
            let (want, screen) = Self::machine_output(&mut after, &slots);
            let first = serde_json::to_string(&after).expect("machine serialization");
            commits.push(MachineCommit { cells: Vec::new(), guard: CellGuard::machine(&key, before), expected_saved: r.native_saved.contains_key(&key), dim: key.0.clone(), pos: key.1, expected: slots, desired: want.clone(), data: screen.clone(), json: first.clone() });
            grown = Some((key, after, want, screen, first));
        }
        if !host.commit_machines(&commits) {
            return;
        }
        let r = &mut self.ready;
        r.world.machines.insert(src.clone(), source);
        r.bridge.last.insert(src.clone(), desired);
        r.bridge.data.insert(src.clone(), data);
        r.native_saved.insert(src.clone(), json);
        if let Some((key, after, want, screen, first)) = grown {
            r.world.machines.insert(key.clone(), after);
            r.bridge.last.insert(key.clone(), want);
            r.bridge.data.insert(key.clone(), screen);
            r.native_saved.insert(key, first);
        }
        if let Some((m, first, screen, shown)) = born {
            r.world.machines.insert(above.clone(), m);
            r.loaded.insert(above.clone(), true);
            r.seen.insert(above.clone());
            r.bridge.last.insert(above.clone(), Vec::new());
            r.bridge.data.insert(above.clone(), screen);
            r.native_saved.insert(above.clone(), first);
            r.shown.insert(above, shown);
        }
    }
}
