//! Chunk records written by earlier versions of the ledger, read now: settled once, and the same when read again.
use ntm_core::ledger::{ChunkLedger, effect_id, entity_tag, retained_id};
use std::collections::BTreeSet;

const DIM: &str = "minecraft:overworld";

/// A record of the version that followed its drops (R3): a stack still pending, one spawned and followed into other
/// chunks (64 items after folds), a paid effect, and a folded stack with the chunks it was seen in.
fn r3_record() -> (String, u64, u64, u64, u64) {
    let (pending, spawned, paid, folded) = (retained_id(DIM, [130, 100, 8], 1), effect_id(9, 0), effect_id(9, 1), retained_id(DIM, [140, 100, 8], 1));
    let json = format!(
        concat!(
            r#"{{"entries":["#,
            r#"{{"id":"{p:016x}","idx":0,"at":[130,100,8],"item":"{{id:'ntm:niter',count:32}}","state":"Pending"}},"#,
            r#"{{"id":"{s:016x}","idx":0,"at":[131,100,9],"item":"{{id:'ntm:niter',count:16}}","state":"Spawned","live":64,"last":[50,0],"seen":[[31,0],[50,0]]}}"#,
            r#"],"applied":["{a:016x}"],"gone":["{f:016x}:0@31,0"]}}"#
        ),
        p = pending,
        s = spawned,
        a = paid,
        f = folded
    );
    (json, pending, spawned, paid, folded)
}

#[test]
fn a_followed_stack_no_file_shows_comes_back_with_what_it_held() {
    // R5-1 of the fifth review: the stack held 64 after its folds; C, where it was, has no entities.
    let (json, pending, spawned, paid, folded) = r3_record();
    let mut ledger = ChunkLedger::from_json(&json);
    assert!(ledger.has_legacy());
    let mut asked = Vec::new();
    let copies = ledger.legacy(|e| {
        asked.push((e.tag(), e.holders()));
        false
    });
    assert_eq!(asked, vec![(entity_tag(spawned, 0, [8, 0]), vec![[8, 0], [31, 0], [50, 0]])], "every chunk it was seen in is looked at");
    assert_eq!(copies, vec![((folded, 0), vec![[31, 0]])], "the folded stack's copies are the host's to remove");
    let dropped = ledger.take_owed(|_| true);
    assert_eq!(dropped, vec![([130, 100, 8], "{id:'ntm:niter',count:32}".to_string()), ([131, 100, 9], "{id:'ntm:niter',count:64}".to_string())]);
    assert!(ledger.knows(pending) && ledger.knows(spawned) && ledger.knows(paid), "a replay of any of its effects adds nothing");
}

#[test]
fn a_followed_stack_a_file_shows_stays_where_it_is() {
    let (json, _, spawned, _, _) = r3_record();
    let mut ledger = ChunkLedger::from_json(&json);
    ledger.legacy(|_| true);
    assert_eq!(ledger.take_owed(|_| true).len(), 1, "only the pending stack is dropped");
    assert!(ledger.knows(spawned));
}

#[test]
fn settling_an_old_record_twice_gives_the_same_record() {
    // One-shot and idempotent: read again (after a death before the save, or after it), nothing more is owed or dropped.
    let (json, ..) = r3_record();
    let settle = |text: &str| {
        let mut l = ChunkLedger::from_json(text);
        if l.has_legacy() {
            l.legacy(|_| false);
        }
        l
    };
    let once = settle(&json);
    let again = settle(&json);
    assert_eq!(once, again, "the same old record settles the same way");
    let written = once.to_json();
    assert!(!written.contains("Spawned") && !written.contains("gone") && !written.contains("seen"), "{written}");
    let reread = settle(&written);
    assert_eq!(reread, once, "a settled record is settled");
    assert!(!reread.has_legacy());
}

#[test]
fn records_of_the_versions_since_read_as_they_are() {
    // R4: only applied ids; R2: entries without a state are pending.
    let id = effect_id(4, 0);
    let r4 = ChunkLedger::from_json(&format!(r#"{{"applied":["{id:016x}"]}}"#));
    assert!(r4.knows(id) && r4.owed.is_empty() && !r4.has_legacy());
    let mut r2 = ChunkLedger::from_json(&format!(r#"{{"entries":[{{"id":"{id:016x}","idx":0,"at":[-1,64,17],"item":"{{id:'ntm:niter',count:1}}"}}]}}"#));
    assert!(!r2.has_legacy());
    assert_eq!(r2.take_owed(|_| true).len(), 1);
    let empty: BTreeSet<u64> = BTreeSet::new();
    r2.prune(&empty);
    assert!(r2.is_empty());
}
