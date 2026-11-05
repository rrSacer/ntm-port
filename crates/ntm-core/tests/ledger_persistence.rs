//! What a ledger keeps about stacks that left the world, against entity files older than its record.
use ntm_core::{
    Pos,
    kv::{Kv, MemKv},
    ledger::{ChunkLedger, State, entity_tag, parse_tag, retained_id},
    server::{Server, ServerHost},
};
use std::collections::BTreeSet;

const DIM: &str = "minecraft:overworld";

#[derive(Default)]
struct Host {
    kv: MemKv,
    ledger: ChunkLedger,
}
impl Kv for Host {
    fn get(&mut self, k: &str) -> Option<Vec<u8>> {
        self.kv.get(k)
    }
    fn set(&mut self, k: &str, v: &[u8]) -> Result<(), String> {
        self.kv.set(k, v)
    }
    fn delete(&mut self, k: &str) {
        self.kv.delete(k)
    }
}
impl ServerHost for Host {
    fn get_block(&mut self, _: &str, _: Pos) -> Option<String> {
        None
    }
    fn set_blocks(&mut self, _: &str, _: &[(Pos, String)]) -> Result<(), String> {
        Ok(())
    }
    fn tell(&mut self, _: &str, _: &str) {}
    fn log(&mut self, _: &str) {}
    // As the native host: the ledger forgets what the storage no longer names.
    fn ledger_prune(&mut self, live: &BTreeSet<u64>) {
        self.ledger.prune(live);
    }
}

/// Two spawned stacks of 32 of one ledger, from cells the storage names no receipt for (a player's break).
fn two_stacks() -> (ChunkLedger, u64, u64) {
    let mut l = ChunkLedger::default();
    let (x, y) = (retained_id(DIM, [130, 100, 8], 1), retained_id(DIM, [140, 100, 8], 1));
    l.apply(x, [130, 100, 8], vec!["{id:'ntm:niter',count:32}".into()]);
    l.apply(y, [140, 100, 8], vec!["{id:'ntm:niter',count:32}".into()]);
    for e in &mut l.entries {
        e.state = State::Spawned;
        e.live = 32;
    }
    (l, x, y)
}

/// What an older entity file adds back on reload: every stack it shows that the ledger does not hold gone.
fn returned(l: &ChunkLedger, file: &[(String, u32)]) -> u32 {
    file.iter().filter(|(tag, _)| parse_tag(tag).is_some_and(|(id, idx, _)| !l.is_gone(id, idx))).map(|(_, n)| n).sum()
}

#[test]
fn a_confirmed_save_keeps_a_fold_while_an_older_holder_file_may_show_it() {
    let mut h = Host::default();
    let mut s = Server::native(&mut h, 0);
    let (mut l, x, y) = two_stacks();
    // X was seen in chunk (64,0), whose entity file kept it; then it folded into Y, which holds 64 now.
    l.entries[0].see([64, 0]);
    l.entries.iter_mut().find(|e| e.id == y).unwrap().live = 64;
    l.retire(x, 0);
    let older = vec![(entity_tag(x, 0, [8, 0]), 32)];
    // The process dies; a reopen with (64,0) away releases Y at a confirmed save, which prunes the ledger.
    l.release(y, 0);
    h.ledger = l;
    s.native_world_saved(&mut h, true);
    let reloaded = ChunkLedger::from_json(&h.ledger.to_json());
    assert_eq!(64 + returned(&reloaded, &older), 64, "the folded stack does not come back: {:?}", reloaded.gone);
    // Once (64,0) is known not to show it, the marker goes; until then it stays.
    let mut kept = reloaded.clone();
    assert!(!kept.forget_gone(|_, _, c| c != [64, 0]));
    assert!(kept.is_gone(x, 0));
    assert!(kept.forget_gone(|_, _, _| true));
    assert!(!kept.is_gone(x, 0));
}

#[test]
fn a_fold_marker_of_an_older_record_still_goes_with_its_receipt() {
    let id = retained_id(DIM, [130, 100, 8], 1);
    let old = format!(r#"{{"gone":["{id:016x}:0"]}}"#);
    let mut l = ChunkLedger::from_json(&old);
    assert!(l.is_gone(id, 0));
    assert!(l.gone[&(id, 0)].is_empty(), "no holders were kept then");
    assert!(!l.forget_gone(|_, _, _| true), "without holders, files cannot say");
    assert!(l.prune(&BTreeSet::new()));
    assert!(!l.is_gone(id, 0));
}

/// The rule the ledger used before: what the stacks still here gained, spent on the departed in order.
fn by_gain(departed: &[(u64, u32, u32)], gained: u32) -> (Vec<u64>, Vec<u64>) {
    let (mut gone, mut taken, mut left) = (Vec::new(), Vec::new(), gained);
    for (id, _, n) in departed {
        if left >= *n {
            left -= n;
            gone.push(*id);
        } else {
            taken.push(*id);
        }
    }
    (gone, taken)
}

#[test]
fn a_pickup_beside_a_fold_of_other_items_is_told_apart_by_the_merge_itself() {
    // N (16 niter) is picked up while D1 (32 diamonds) folds into D2 (32): in one pass.
    let mut l = ChunkLedger::default();
    let (n, d1, d2) = (retained_id(DIM, [129, 100, 8], 1), retained_id(DIM, [131, 100, 8], 1), retained_id(DIM, [133, 100, 8], 1));
    l.apply(n, [129, 100, 8], vec!["{id:'ntm:niter',count:16}".into()]);
    l.apply(d1, [131, 100, 8], vec!["{id:'minecraft:diamond',count:32}".into()]);
    l.apply(d2, [133, 100, 8], vec!["{id:'minecraft:diamond',count:32}".into()]);
    // The old rule credits the pickup with the fold: N gone, D1 taken.
    assert_eq!(by_gain(&[(n, 0, 16), (d1, 0, 32)], 32), (vec![n], vec![d1]));
    // The engine says which entity was emptied into another: D1's (entity 12) into D2's.
    let folded = BTreeSet::from([12u32]);
    l.settle(&[(n, 0, vec![11]), (d1, 0, vec![12])], &folded);
    assert!(l.is_gone(d1, 0) && !l.is_gone(n, 0));
    // An entity file older than the record shows all three; D2 is restored to the 64 it holds.
    let older = vec![(entity_tag(n, 0, [8, 0]), 16), (entity_tag(d1, 0, [8, 0]), 32)];
    let diamonds: u32 = 64 + older.iter().filter(|(t, _)| t.starts_with(&format!("{d1:016x}"))).filter(|(t, _)| parse_tag(t).is_some_and(|(i, x, _)| !l.is_gone(i, x))).map(|(_, c)| c).sum::<u32>();
    assert_eq!(diamonds, 64, "the folded diamonds do not come back");
    assert_eq!(returned(&l, &older), 16, "the picked up niter is a plain item there, as in vanilla");
}

#[test]
fn a_pickup_and_a_fold_of_the_same_items_in_one_pass_are_each_their_own() {
    // N (16) is picked up while D1 (32) folds into D2 (32), all niter.
    let (mut l, d1, _d2) = two_stacks();
    let n = retained_id(DIM, [129, 100, 8], 1);
    l.apply(n, [129, 100, 8], vec!["{id:'ntm:niter',count:16}".into()]);
    assert_eq!(by_gain(&[(n, 0, 16), (d1, 0, 32)], 32), (vec![n], vec![d1]), "the old rule credits N");
    l.settle(&[(n, 0, vec![21]), (d1, 0, vec![22])], &BTreeSet::from([22u32]));
    assert!(l.is_gone(d1, 0) && !l.is_gone(n, 0));
}
