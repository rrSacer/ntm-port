//! Counterexamples for a marker kept in the chunk record while entities are in a separate file.
//! These tests document unsafe candidate designs; passing them does not approve a host implementation.
use ntm_core::{
    Pos,
    kv::{Kv, MemKv},
    ledger::{ChunkLedger, effect_id},
    server::{CommittedEffects, Server, ServerHost, WorldEffect},
};
use std::collections::{BTreeSet, VecDeque};

const DIM: &str = "minecraft:overworld";
const SOURCE: Pos = [15, 64, 0];
const DEST: Pos = [16, 65, 0];
const SERIAL: u64 = 700;

/// The destination region has reached disk with its applied marker, while the entities file is still the previous one.
/// The source's older record still owes the batch. This is possible with the existing independent writers.
struct CutHost {
    kv: MemKv,
    destination: ChunkLedger,
    physical_items: usize,
    explosions: usize,
    reemit_old_debt: bool,
}
impl Kv for CutHost {
    fn get(&mut self, k: &str) -> Option<Vec<u8>> {
        self.kv.get(k)
    }
    fn set(&mut self, k: &str, v: &[u8]) -> Result<(), String> {
        self.kv.set(k, v)
    }
    fn delete(&mut self, k: &str) {
        self.kv.delete(k);
    }
}
impl ServerHost for CutHost {
    fn get_block(&mut self, _: &str, _: Pos) -> Option<String> {
        Some("minecraft:air".into())
    }
    fn set_blocks(&mut self, _: &str, _: &[(Pos, String)]) -> Result<(), String> {
        Ok(())
    }
    fn tell(&mut self, _: &str, _: &str) {}
    fn log(&mut self, _: &str) {}
    fn effect_seen(&mut self, id: u64, _: &str, pos: Pos) -> bool {
        pos == DEST && self.destination.knows(id)
    }
    fn explode(&mut self, _: &str, _: [f64; 3], _: f32, _: bool, _: bool) -> bool {
        self.explosions += 1;
        true
    }
    fn apply_items(
        &mut self,
        id: u64,
        _: &str,
        _: Pos,
        _: &[(String, u32)],
        _: &[(String, Pos)],
    ) -> bool {
        if self.reemit_old_debt || !self.destination.knows(id) {
            self.physical_items += 1;
        }
        true
    }
}
fn old_home_batch() -> CommittedEffects {
    CommittedEffects {
        serial: SERIAL,
        owners: vec![(DIM.into(), SOURCE)],
        effects: VecDeque::from([
            WorldEffect::Explosion {
                dim: DIM.into(),
                at: [15.5, 65., 0.5],
                power: 5.,
                fire: false,
                breaks: false,
            },
            WorldEffect::Items {
                dim: DIM.into(),
                pos: DEST,
                items: vec![("ntm:sawblade".into(), 1)],
            },
        ]),
        ..Default::default()
    }
}
#[test]
fn region_marker_before_entity_file_hides_what_old_home_still_owes() {
    let id = effect_id(SERIAL, 1);
    let mut destination = ChunkLedger::default();
    destination.applied.insert(id); // destination region from the second save
    let mut host = CutHost {
        kv: MemKv::default(),
        destination,
        physical_items: 0,
        explosions: 0,
        reemit_old_debt: false,
    }; // old entity file
    let mut server = Server::native(&mut host, 0);
    server.native_obligations_loaded(DIM, [0, 0], vec![], vec![old_home_batch()], BTreeSet::new());
    server.native_settle(&mut host);
    let vanilla_chest_items = 1; // The corresponding source chunk is also old in the control.
    assert_eq!(
        host.explosions, 0,
        "the destination marker suppresses the earlier explosion"
    );
    assert_eq!(
        host.physical_items, 0,
        "the same marker suppresses the item although its file is old"
    );
    assert_ne!(
        host.physical_items, vanilla_chest_items,
        "a naive durable marker loses an item relative to vanilla"
    );
}
#[test]
fn owed_drain_cannot_decide_from_the_region_marker_alone() {
    let id = effect_id(SERIAL, 1);
    let mut old_source = ChunkLedger::default();
    assert!(old_source.owe(id, DEST, vec!["{id:'ntm:sawblade',count:1}".into()]));
    let mut region_b = ChunkLedger::default();
    region_b.applied.insert(id);
    // Two legal cuts have identical source and destination region records but different entity files.
    for (old_entity_file_count, correct_to_spawn) in [(0, true), (1, false)] {
        let mut source = old_source.clone();
        let due = source.take_owed(|_| true);
        assert_eq!(due.len(), 1);
        assert!(region_b.knows(id));
        assert_eq!(correct_to_spawn, old_entity_file_count == 0);
        // Always spawning duplicates the second cut; always skipping loses the first against the old vanilla chest.
        let always_spawn = old_entity_file_count + due.len();
        let always_skip = old_entity_file_count;
        assert_ne!(
            if correct_to_spawn {
                always_skip
            } else {
                always_spawn
            },
            1
        );
    }
}

#[test]
fn a_newer_destination_and_older_home_may_contain_two_vanilla_copies() {
    let id = effect_id(SERIAL, 1);
    let mut destination = ChunkLedger::default();
    destination.applied.insert(id);
    let mut host = CutHost {
        kv: MemKv::default(),
        destination,
        physical_items: 1, // This time both region B and entities B reached disk.
        explosions: 0,
        reemit_old_debt: false,
    };
    let mut server = Server::native(&mut host, 0);
    server.native_obligations_loaded(DIM, [0, 0], vec![], vec![old_home_batch()], BTreeSet::new());
    server.native_settle(&mut host);
    // The same torn records in the vanilla control: the old chest still holds one and the new entity file one.
    let vanilla_chest_and_ground = 2;
    assert_eq!(
        host.physical_items, 1,
        "R3 idempotence suppresses the output owed by the older home"
    );
    assert_eq!(
        host.explosions, 0,
        "the earlier explosion is correctly not repeated over the saved output"
    );
    assert_ne!(
        host.physical_items, vanilla_chest_and_ground,
        "the saved vanilla copy in the old chest is missing from the mod"
    );
}

#[test]
fn replaying_the_old_source_item_without_repeating_explosion_matches_both_cuts() {
    for existing_in_saved_entities in [0, 1] {
        let mut destination = ChunkLedger::default();
        destination.applied.insert(effect_id(SERIAL, 1));
        let mut host = CutHost {
            kv: MemKv::default(),
            destination,
            physical_items: existing_in_saved_entities,
            explosions: 0,
            reemit_old_debt: true,
        };
        let mut server = Server::native(&mut host, 0);
        server.native_obligations_loaded(
            DIM,
            [0, 0],
            vec![],
            vec![old_home_batch()],
            BTreeSet::new(),
        );
        server.native_settle(&mut host);
        assert_eq!(
            host.explosions, 0,
            "the marker remains evidence for the previous explosion"
        );
        assert_eq!(
            host.physical_items,
            1 + existing_in_saved_entities,
            "old vanilla source plus its saved destination file"
        );
    }
}
