//! A chunk record written by the ledger of the earlier version (which kept and followed the owed stacks) read now.
use ntm_core::ledger::{ChunkLedger, effect_id, retained_id};
use std::collections::BTreeSet;

const DIM: &str = "minecraft:overworld";

/// A record as that version wrote it: a stack still pending (its chunk's entities were not in yet), one spawned and
/// followed into two other chunks, a paid effect, and a folded stack with the chunks it was seen in.
fn old_record() -> (String, u64, u64, u64, u64) {
    let (pending, spawned, paid, folded) = (retained_id(DIM, [130, 100, 8], 1), effect_id(9, 0), effect_id(9, 1), retained_id(DIM, [140, 100, 8], 1));
    let json = format!(
        concat!(
            r#"{{"entries":["#,
            r#"{{"id":"{p:016x}","idx":0,"at":[130,100,8],"item":"{{id:'ntm:niter',count:32}}","state":"Pending"}},"#,
            r#"{{"id":"{p:016x}","idx":1,"at":[130,100,8],"item":"{{id:'minecraft:diamond',count:3}}"}},"#,
            r#"{{"id":"{s:016x}","idx":0,"at":[131,100,9],"item":"{{id:'ntm:rod_empty',count:1}}","state":"Spawned","live":10,"last":[9,0],"seen":[[9,0],[9,1]]}}"#,
            r#"],"applied":["{a:016x}"],"gone":["{f:016x}:0@8,0;9,0","{f:016x}:1"]}}"#
        ),
        p = pending,
        s = spawned,
        a = paid,
        f = folded
    );
    (json, pending, spawned, paid, folded)
}

#[test]
fn an_old_record_loads_and_drops_once_what_it_never_put_into_the_world() {
    let (json, pending, spawned, paid, folded) = old_record();
    let mut ledger = ChunkLedger::from_json(&json);
    assert_eq!(ledger.owed.len(), 3, "every stack it kept is read: {ledger:?}");
    assert!(ledger.knows(pending) && ledger.knows(spawned) && ledger.knows(paid), "a replay of any of its effects adds nothing");
    // The two stacks it never spawned are dropped, with their items exactly; the spawned one is in the world's files.
    let owed = ledger.take_owed();
    assert_eq!(owed, vec![([130, 100, 8], "{id:'ntm:niter',count:32}".to_string()), ([130, 100, 8], "{id:'minecraft:diamond',count:3}".to_string())]);
    assert_eq!(ledger.applied, BTreeSet::from([pending, spawned, paid]));
    assert!(ledger.take_owed().is_empty(), "settled once");
    // Written again in the new form, nothing of the old one comes back: a second load owes nothing.
    let written = ledger.to_json();
    assert!(!written.contains("entries") && !written.contains("gone"), "{written}");
    let reloaded = ChunkLedger::from_json(&written);
    assert!(reloaded.owed.is_empty() && reloaded.knows(pending) && !reloaded.knows(folded));
}

#[test]
fn an_old_record_whose_stacks_all_reached_the_world_owes_nothing_and_keeps_its_ids() {
    let id = effect_id(4, 0);
    let json = format!(r#"{{"entries":[{{"id":"{id:016x}","idx":0,"at":[-1,64,17],"item":"{{id:'ntm:niter',count:1}}","state":"Spawned"}}]}}"#);
    let mut ledger = ChunkLedger::from_json(&json);
    assert!(ledger.take_owed().is_empty());
    assert_eq!(ledger.applied, BTreeSet::from([id]));
    // The record of the previous form with only paid ids reads as it is.
    let paid = ChunkLedger::from_json(&format!(r#"{{"applied":["{id:016x}"]}}"#));
    assert!(paid.owed.is_empty() && paid.knows(id));
}
