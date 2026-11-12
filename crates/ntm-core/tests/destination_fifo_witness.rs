//! The pin's writer is FIFO in batches but coalesces pending writes by (folder, chunk).
//! These are counterexamples to using enqueue order as a durability barrier, not a replacement for an engine test.
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Target {
    SourceRegion,
    DestinationEntities,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Write {
    target: Target,
    sequence: u64,
    value: u8,
    required: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
struct Writer {
    pending: BTreeMap<Target, (u64, u8)>,
    queue: VecDeque<Write>,
    disk: BTreeMap<Target, u8>,
    next: u64,
    error: bool,
    blocked: bool,
}
impl Writer {
    fn queue(&mut self, target: Target, value: u8) {
        self.next += 1;
        self.pending.insert(target, (self.next, value));
        self.queue.push_back(Write {
            target,
            sequence: self.next,
            value,
            required: false,
        });
    }
    /// Transcribes the `newest` filter and the writer's error/continue handling in pin save.rs:3159–3191.
    fn step(&mut self, fail: bool) {
        let write = self.queue.pop_front().unwrap();
        let newest = self
            .pending
            .get(&write.target)
            .is_none_or(|(sequence, _)| *sequence <= write.sequence);
        if !newest {
            return;
        }
        if fail {
            self.error = true;
            return;
        }
        self.disk.insert(write.target, write.value);
        if self
            .pending
            .get(&write.target)
            .is_some_and(|(sequence, _)| *sequence == write.sequence)
        {
            self.pending.remove(&write.target);
        }
    }
    fn queue_required(&mut self, target: Target, value: u8) {
        self.queue(target, value);
        self.queue.back_mut().unwrap().required = true;
    }
    /// Candidate semantics only: required source snapshots are never coalesced; failure prevents later writes.
    fn guarded_step(&mut self, fail: bool, drop_required: bool, ignore_failure: bool) {
        if self.blocked {
            return;
        }
        let write = self.queue.pop_front().unwrap();
        let newest = self
            .pending
            .get(&write.target)
            .is_none_or(|(seq, _)| *seq <= write.sequence);
        if (!write.required || drop_required) && !newest {
            return;
        }
        if fail {
            self.error = true;
            if write.required && !ignore_failure {
                self.blocked = true;
            }
            return;
        }
        self.disk.insert(write.target, write.value);
        if self
            .pending
            .get(&write.target)
            .is_some_and(|(seq, _)| *seq == write.sequence)
        {
            self.pending.remove(&write.target);
        }
    }
    fn after_crash(&self, target: Target) -> u8 {
        *self.disk.get(&target).unwrap_or(&0)
    }
}
#[test]
fn a_newer_source_snapshot_can_overtake_the_destination_entities() {
    let mut writer = Writer::default();
    writer.queue(Target::SourceRegion, 1); // A1: marked and consumed, queued before spawn
    writer.queue(Target::DestinationEntities, 1); // E_B from the later unload
    writer.queue(Target::SourceRegion, 1); // A2, a newer pending snapshot of the same chunk
    writer.step(false); // A1 is coalesced away because A2 is pending.
    writer.step(false); // E_B is written before A2.
    assert_eq!(
        writer.after_crash(Target::SourceRegion),
        0,
        "old source still owes the item"
    );
    assert_eq!(
        writer.after_crash(Target::DestinationEntities),
        1,
        "destination entity reached disk"
    );
    assert_eq!(
        writer.queue.len(),
        1,
        "A2 is still pending and disappears on crash"
    );
}
#[test]
fn an_error_on_the_source_does_not_stop_a_later_entity_write() {
    let mut writer = Writer::default();
    writer.queue(Target::SourceRegion, 1);
    writer.queue(Target::DestinationEntities, 1);
    writer.step(true); // The worker records an error but processes later batches.
    assert!(writer.error);
    writer.step(false);
    assert_eq!(writer.after_crash(Target::SourceRegion), 0);
    assert_eq!(writer.after_crash(Target::DestinationEntities), 1);
}

/// Exhaust all interleavings of submitting E and A2 and performing writer steps after required A1 was queued.
/// A failed required write may not let E reach disk. A crash can happen at each visited state.
fn explore_barrier(
    mutant_drop_required: bool,
    mutant_ignore_failure: bool,
) -> (usize, Option<Vec<&'static str>>) {
    use std::collections::HashSet;
    fn walk(
        writer: Writer,
        submitted: u8,
        fail_available: bool,
        path: &mut Vec<&'static str>,
        seen: &mut HashSet<(Writer, u8, bool)>,
        visited: &mut usize,
        mutant: (bool, bool),
    ) -> Option<Vec<&'static str>> {
        if !seen.insert((writer.clone(), submitted, fail_available)) {
            return None;
        }
        *visited += 1;
        if writer.after_crash(Target::DestinationEntities) == 1
            && writer.after_crash(Target::SourceRegion) == 0
        {
            return Some(path.clone());
        }
        if submitted < 2 {
            let mut next = writer.clone();
            let name = if submitted == 0 {
                next.queue(Target::DestinationEntities, 1);
                "enqueue E_B"
            } else {
                next.queue(Target::SourceRegion, 1);
                "enqueue A2"
            };
            path.push(name);
            if let Some(found) = walk(
                next,
                submitted + 1,
                fail_available,
                path,
                seen,
                visited,
                mutant,
            ) {
                return Some(found);
            }
            path.pop();
        }
        if !writer.blocked && !writer.queue.is_empty() {
            let mut next = writer.clone();
            next.guarded_step(false, mutant.0, mutant.1);
            path.push("write");
            if let Some(found) = walk(next, submitted, fail_available, path, seen, visited, mutant)
            {
                return Some(found);
            }
            path.pop();
            if fail_available && writer.queue.front().is_some_and(|w| w.required) {
                let mut next = writer;
                next.guarded_step(true, mutant.0, mutant.1);
                path.push("fail required write");
                if let Some(found) = walk(next, submitted, false, path, seen, visited, mutant) {
                    return Some(found);
                }
                path.pop();
            }
        }
        None
    }
    let mut writer = Writer::default();
    writer.queue_required(Target::SourceRegion, 1);
    let mut seen = HashSet::new();
    let mut visited = 0;
    let found = walk(
        writer,
        0,
        true,
        &mut vec![],
        &mut seen,
        &mut visited,
        (mutant_drop_required, mutant_ignore_failure),
    );
    (visited, found)
}
#[test]
fn a_protected_source_barrier_survives_all_modeled_interleavings() {
    let (visited, failed) = explore_barrier(false, false);
    assert!(failed.is_none(), "{failed:?}");
    println!("protected source: {visited} distinct states, zero source-behind-entity violations");
}
#[test]
fn mutants_coalescing_the_barrier_or_ignoring_its_failure_are_caught() {
    let (_, coalesce) = explore_barrier(true, false);
    let (_, ignore_failure) = explore_barrier(false, true);
    assert!(
        coalesce.is_some(),
        "coalescing the protected write must fail"
    );
    assert!(
        ignore_failure.is_some(),
        "continuing after its failure must fail"
    );
    println!("coalesced barrier: {coalesce:?}; ignored failure: {ignore_failure:?}");
}
