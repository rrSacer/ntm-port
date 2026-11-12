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
    emitted_ids: Vec<u64>,
    tagged_at_destination: bool,
    destroy_items_on_blast: bool,
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
        pos == DEST
            && (self.destination.knows(id)
                || self.tagged_at_destination && id == effect_id(SERIAL, 1))
    }
    fn explode(&mut self, _: &str, _: [f64; 3], _: f32, _: bool, _: bool) -> bool {
        self.explosions += 1;
        if self.destroy_items_on_blast {
            self.physical_items = 0;
        }
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
            self.emitted_ids.push(id);
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
        emitted_ids: vec![],
        tagged_at_destination: false,
        destroy_items_on_blast: false,
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
fn owed_drain_from_an_old_home_reemits_even_if_the_destination_is_marked() {
    let id = effect_id(SERIAL, 1);
    let mut old_source = ChunkLedger::default();
    assert!(old_source.owe(id, DEST, vec!["{id:'ntm:sawblade',count:1}".into()]));
    let mut region_b = ChunkLedger::default();
    region_b.applied.insert(id);
    // A is old with one owed item. Its vanilla counterpart has one in the old output slot.
    // Both old and new destination entity files are possible with the same region marker.
    for saved_destination_items in [0, 1] {
        let mut source = old_source.clone();
        let due = source.take_owed(|_| true);
        assert_eq!(due.len(), 1);
        assert!(region_b.knows(id));
        let vanilla_total = 1 + saved_destination_items;
        assert_eq!(
            saved_destination_items + due.len(),
            vanilla_total,
            "paying the old debt matches the vanilla cut"
        );
        assert_ne!(
            saved_destination_items, vanilla_total,
            "skipping the debt loses the old source's copy"
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
        emitted_ids: vec![],
        tagged_at_destination: false,
        destroy_items_on_blast: false,
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
            emitted_ids: vec![],
            tagged_at_destination: false,
            destroy_items_on_blast: false,
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

#[test]
fn a_later_receipt_does_not_suppress_an_earlier_owed_item() {
    let mut destination = ChunkLedger::default();
    destination.applied.insert(effect_id(SERIAL, 1));
    let mut host = CutHost {
        kv: MemKv::default(),
        destination,
        physical_items: 1,
        explosions: 0,
        reemit_old_debt: true,
        emitted_ids: vec![],
        tagged_at_destination: false,
        destroy_items_on_blast: false,
    };
    let mut batch = old_home_batch();
    batch.effects = VecDeque::from([
        WorldEffect::Items {
            dim: DIM.into(),
            pos: SOURCE,
            items: vec![("ntm:iron".into(), 1)],
        },
        WorldEffect::Items {
            dim: DIM.into(),
            pos: DEST,
            items: vec![("ntm:sawblade".into(), 1)],
        },
    ]);
    let mut server = Server::native(&mut host, 0);
    server.native_obligations_loaded(DIM, [0, 0], vec![], vec![batch], BTreeSet::new());
    server.native_settle(&mut host);
    assert_eq!(
        host.emitted_ids,
        [effect_id(SERIAL, 0), effect_id(SERIAL, 1)],
        "both old-source items are still owed"
    );
    assert_eq!(host.physical_items, 3);
    // The equivalent older vanilla source has both its items; the saved destination contains the later item too.
    assert_eq!(
        host.physical_items, 3,
        "the old source and saved destination match the vanilla cut"
    );
}

#[test]
fn a_tag_lost_on_pickup_leaves_no_destination_evidence_to_stop_a_second_blast() {
    // Save 1: old source still owes [explosion, item], old destination region has no marker, entity file is empty.
    // The first explosion and item spawn happen; the player picks up the entity. Player data alone reaches disk
    // before a crash. Entity attachments are not copied into ItemStack by ItemEntity.playerTouch.
    let saved_player_items = 1;
    let mut host = CutHost {
        kv: MemKv::default(),
        destination: ChunkLedger::default(),
        physical_items: 0,
        explosions: 0,
        reemit_old_debt: true,
        emitted_ids: vec![],
        tagged_at_destination: false,
        destroy_items_on_blast: false,
    };
    let mut server = Server::native(&mut host, 0);
    server.native_obligations_loaded(DIM, [0, 0], vec![], vec![old_home_batch()], BTreeSet::new());
    server.native_settle(&mut host);
    assert_eq!(
        host.physical_items + saved_player_items,
        2,
        "the old vanilla source plus the saved player item"
    );
    assert_eq!(
        host.explosions, 1,
        "with neither a region marker nor a remaining tagged entity, the earlier blast is repeated"
    );
    // An identical disk view can also come from a player who had the same item before the still-pending blast.
    // This witness does not model movement, merge, or an in-game save; it exercises the real server's replay decision
    // with a host limited to the two proposed proofs (destination marker or still-existing entity tag).
}

#[test]
fn a_saved_tagged_entity_can_prove_the_blast_when_its_region_is_old() {
    let mut host = CutHost {
        kv: MemKv::default(),
        destination: ChunkLedger::default(),
        physical_items: 1,
        explosions: 0,
        reemit_old_debt: true,
        emitted_ids: vec![],
        tagged_at_destination: true,
        destroy_items_on_blast: true,
    };
    let mut server = Server::native(&mut host, 0);
    server.native_obligations_loaded(DIM, [0, 0], vec![], vec![old_home_batch()], BTreeSet::new());
    server.native_settle(&mut host);
    assert_eq!(host.explosions, 0);
    assert_eq!(
        host.physical_items, 2,
        "saved tagged output plus the old vanilla source"
    );
}

#[test]
fn a_merged_item_that_loses_its_entity_tag_can_be_destroyed_on_replay() {
    // The emitted item merged into an older untagged ItemEntity, which reached disk; B's region did not.
    let mut host = CutHost {
        kv: MemKv::default(),
        destination: ChunkLedger::default(),
        physical_items: 2,
        explosions: 0,
        reemit_old_debt: true,
        emitted_ids: vec![],
        tagged_at_destination: false,
        destroy_items_on_blast: true,
    };
    let mut server = Server::native(&mut host, 0);
    server.native_obligations_loaded(DIM, [0, 0], vec![], vec![old_home_batch()], BTreeSet::new());
    server.native_settle(&mut host);
    assert_eq!(host.explosions, 1);
    assert_eq!(
        host.physical_items, 1,
        "the blast destroyed the two merged saved items before replaying one"
    );
    assert_ne!(
        host.physical_items, 3,
        "vanilla holds the two saved items plus one from old source"
    );
}

#[test]
fn a_tagged_output_saved_after_moving_away_is_not_evidence_at_its_origin() {
    // The output moved to chunk C and C's entities were saved; B's old region and entity file carry no proof.
    let mut host = CutHost {
        kv: MemKv::default(),
        destination: ChunkLedger::default(),
        physical_items: 1,
        explosions: 0,
        reemit_old_debt: true,
        emitted_ids: vec![],
        tagged_at_destination: false,
        destroy_items_on_blast: false,
    };
    let mut server = Server::native(&mut host, 0);
    server.native_obligations_loaded(DIM, [0, 0], vec![], vec![old_home_batch()], BTreeSet::new());
    server.native_settle(&mut host);
    assert_eq!(
        host.physical_items, 2,
        "the moved saved item and old source match vanilla"
    );
    assert_eq!(
        host.explosions, 1,
        "an origin-only tag search cannot prove the earlier blast"
    );
}

/// A small exhaustive cut model, not a model of all engine scheduling. Source A is always the old pending record.
/// Each history is built by spawn then a real-world-style transfer; each mask independently selects old/new B region,
/// B entity file, C entity file and player file. This deliberately over-approximates writer order and must not be
/// presented as a game proof. Each case runs the actual Server replay through the synthetic host above.
#[test]
fn tag_and_marker_cut_matrix_covers_pickup_merge_and_movement() {
    #[derive(Clone, Copy)]
    enum Transfer {
        Stay,
        Pickup,
        Merge,
        Move,
    }
    let mut cases = 0;
    let mut losses = 0;
    let mut repeated_blasts_over_saved_output = 0;
    for transfer in [
        Transfer::Stay,
        Transfer::Pickup,
        Transfer::Merge,
        Transfer::Move,
    ] {
        for mask in 0u8..16 {
            let marker_new = mask & 1 != 0;
            let b_new = mask & 2 != 0;
            let c_new = mask & 4 != 0;
            let player_new = mask & 8 != 0;
            let b_items = if b_new {
                match transfer {
                    Transfer::Stay => 1,
                    Transfer::Merge => 3,
                    _ => 0,
                }
            } else {
                0
            };
            let b_tag = b_new && matches!(transfer, Transfer::Stay);
            let c_items = usize::from(c_new && matches!(transfer, Transfer::Move));
            let player_items = usize::from(player_new && matches!(transfer, Transfer::Pickup));
            // The output is physically present in a saved file even when its entity tag was discarded by merge/pickup.
            let output_saved = b_new && matches!(transfer, Transfer::Stay | Transfer::Merge)
                || c_items > 0
                || player_items > 0;
            let mut destination = ChunkLedger::default();
            if marker_new {
                destination.applied.insert(effect_id(SERIAL, 1));
            }
            let mut host = CutHost {
                kv: MemKv::default(),
                destination,
                physical_items: b_items,
                explosions: 0,
                reemit_old_debt: true,
                emitted_ids: vec![],
                tagged_at_destination: b_tag,
                destroy_items_on_blast: true,
            };
            let vanilla = 1 + b_items + c_items + player_items; // old source plus identical saved destination files
            let mut server = Server::native(&mut host, 0);
            server.native_obligations_loaded(
                DIM,
                [0, 0],
                vec![],
                vec![old_home_batch()],
                BTreeSet::new(),
            );
            server.native_settle(&mut host);
            let actual = host.physical_items + c_items + player_items;
            assert!(
                actual <= vanilla,
                "candidate adds no items beyond the vanilla cut"
            );
            losses += usize::from(actual < vanilla);
            repeated_blasts_over_saved_output += usize::from(output_saved && host.explosions > 0);
            if marker_new || b_tag {
                assert_eq!(host.explosions, 0);
                assert_eq!(actual, vanilla);
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 64);
    assert!(losses > 0, "the merged, untagged saved entity exposes loss");
    assert!(
        repeated_blasts_over_saved_output > 0,
        "pickup or move can lose origin evidence"
    );
    println!(
        "tag/marker cut matrix: {cases} cases, {losses} losses, {repeated_blasts_over_saved_output} repeated blasts over a saved output"
    );
}
