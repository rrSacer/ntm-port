//! Synthetic consumers of the common machine-effect transaction, without changing a production machine.
use super::*;
use crate::kv::MemKv;
use std::collections::VecDeque;

const DIM: &str = "minecraft:overworld";
const CORE: Pos = [15, 64, 0];
const DEST: Pos = [16, 65, 0];

#[derive(Default)]
struct Host {
    kv: MemKv,
    refuse_blast: bool,
    refuse_items: bool,
    blasts: Vec<WorldEffect>,
    items: Vec<(String, u32)>,
    order: Vec<&'static str>,
    seen: BTreeSet<(u64, String, Pos)>,
    homes: BTreeMap<[i32; 2], Vec<CommittedEffects>>,
    empty_homes: Vec<[i32; 2]>,
}
impl Kv for Host {
    fn get(&mut self, key: &str) -> Option<Vec<u8>> {
        self.kv.get(key)
    }
    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), String> {
        self.kv.set(key, value)
    }
    fn delete(&mut self, key: &str) {
        self.kv.delete(key);
    }
}
impl ServerHost for Host {
    fn get_block(&mut self, _: &str, _: Pos) -> Option<String> {
        Some("minecraft:air".into())
    }
    fn set_blocks(&mut self, _: &str, _: &[(Pos, String)]) -> Result<(), String> {
        Ok(())
    }
    fn tell(&mut self, _: &str, _: &str) {}
    fn log(&mut self, _: &str) {}
    fn explode(&mut self, dim: &str, at: [f64; 3], power: f32, fire: bool, breaks: bool) -> bool {
        self.order.push("explosion attempt");
        if self.refuse_blast {
            return false;
        }
        self.blasts.push(WorldEffect::Explosion {
            dim: dim.into(),
            at,
            power,
            fire,
            breaks,
        });
        true
    }
    fn apply_items(
        &mut self,
        id: u64,
        dim: &str,
        pos: Pos,
        items: &[(String, u32)],
        _: &[Key],
    ) -> bool {
        if self.refuse_items {
            return false;
        }
        self.order.push("items");
        if self.seen.insert((id, dim.into(), pos)) {
            self.items.extend_from_slice(items);
        }
        true
    }
    fn store_obligations(
        &mut self,
        _: &str,
        chunk: [i32; 2],
        _: &[crate::persist::Removal],
        batches: &[CommittedEffects],
        _: &BTreeSet<crate::persist::Obligation>,
    ) -> bool {
        if batches.is_empty() {
            self.empty_homes.push(chunk);
            self.homes.remove(&chunk);
        } else {
            self.homes.insert(chunk, batches.to_vec());
        }
        true
    }
    fn effect_seen(&mut self, id: u64, dim: &str, pos: Pos) -> bool {
        self.seen.contains(&(id, dim.into(), pos))
    }
}
fn blast() -> WorldEffect {
    WorldEffect::Explosion {
        dim: DIM.into(),
        at: [15.5, 65., 0.5],
        power: 5.,
        fire: false,
        breaks: false,
    }
}
fn drop() -> WorldEffect {
    WorldEffect::Items {
        dim: DIM.into(),
        pos: DEST,
        items: vec![("ntm:sawblade".into(), 1)],
    }
}
fn batch() -> CommittedEffects {
    CommittedEffects {
        serial: 700,
        owners: vec![(DIM.into(), CORE)],
        effects: VecDeque::from([blast(), drop()]),
        ..Default::default()
    }
}
#[test]
fn a_legacy_pile_explosion_still_deserializes() {
    let old = r#"{"PileExplosion":{"dim":"minecraft:overworld","pos":[1.5,65.0,2.5]}}"#;
    assert_eq!(
        serde_json::from_str::<WorldEffect>(old).unwrap(),
        WorldEffect::PileExplosion {
            dim: DIM.into(),
            pos: [1.5, 65., 2.5]
        }
    );
    let original = batch();
    assert_eq!(
        serde_json::from_slice::<CommittedEffects>(&serde_json::to_vec(&original).unwrap())
            .unwrap(),
        original
    );
}
#[test]
fn a_refused_generic_explosion_keeps_the_cursor_and_blocks_later_items() {
    let mut host = Host {
        refuse_blast: true,
        ..Default::default()
    };
    let mut server = Server::native(&mut host, 0);
    server.ready.world.machine_effects.push_back(batch());
    server.flush_machine_effects(&mut host);
    assert_eq!(server.ready.world.machine_effects[0], batch());
    assert!(host.blasts.is_empty() && host.items.is_empty());
    assert!(server.ready.world.applied_machine_effects.is_empty());
    host.refuse_blast = false;
    server.flush_machine_effects(&mut host);
    assert_eq!(host.blasts, vec![blast()]);
    assert_eq!(host.items, vec![("ntm:sawblade".into(), 1)]);
    assert_eq!(
        host.order,
        ["explosion attempt", "explosion attempt", "items"]
    );
    assert!(server.ready.world.machine_effects.is_empty());
    server.flush_machine_effects(&mut host);
    assert_eq!(host.blasts.len(), 1);
}
#[test]
fn an_accepted_generic_explosion_is_not_repeated_when_its_item_waits() {
    let mut host = Host {
        refuse_items: true,
        ..Default::default()
    };
    let mut server = Server::native(&mut host, 0);
    server.ready.world.machine_effects.push_back(batch());
    server.flush_machine_effects(&mut host);
    assert_eq!(server.ready.world.machine_effects[0].done, 1);
    assert_eq!(
        server.ready.world.machine_effects[0].effects,
        VecDeque::from([drop()])
    );
    server.flush_machine_effects(&mut host);
    assert_eq!(host.blasts, vec![blast()]);
    host.refuse_items = false;
    server.flush_machine_effects(&mut host);
    assert_eq!(host.order, ["explosion attempt", "items"]);
    assert_eq!(server.ready.world.applied_machine_effects.len(), 2);
}

#[test]
fn replay_skips_an_old_explosion_when_a_later_destination_proves_the_order() {
    for later in [
        drop(),
        WorldEffect::EncodedItems {
            dim: DIM.into(),
            pos: DEST,
            items: vec![],
        },
        WorldEffect::Entities {
            dim: DIM.into(),
            pos: DEST,
            entities: vec![],
        },
    ] {
        let mut host = Host {
            refuse_blast: true,
            ..Default::default()
        };
        let mut server = Server::native(&mut host, 0);
        let mut old = batch();
        old.done = 3;
        old.effects = VecDeque::from([blast(), blast(), later]);
        server.ready.world.machine_effects.push_back(old.clone());
        server.flush_machine_effects(&mut host);
        assert_eq!(
            server.ready.world.machine_effects[0], old,
            "save 1: explosion refused, whole batch owed"
        );
        // Save 2 wrote the later effect's chunk but not the home: R3. Its stable index is 3 + 2.
        host.seen
            .insert((crate::ledger::effect_id(old.serial, 5), DIM.into(), DEST));
        host.refuse_blast = false;
        let mut reopened = Server::native(&mut host, 1);
        reopened.native_obligations_loaded(DIM, [0, 0], vec![], vec![old], BTreeSet::new());
        reopened.flush_machine_effects(&mut host);
        assert!(
            host.blasts.is_empty(),
            "the later record proves both earlier explosions ran"
        );
        assert!(
            reopened
                .ready
                .world
                .applied_machine_effects
                .iter()
                .all(|b| b
                    .effects
                    .iter()
                    .all(|e| !matches!(e, WorldEffect::Explosion { .. }))),
            "skipped effects get no new receipt"
        );
    }
}

#[test]
fn a_later_unseen_effect_or_a_new_batch_does_not_skip_the_explosion() {
    for replayed in [false, true] {
        let mut host = Host::default();
        let mut server = Server::native(&mut host, 0);
        let mut batch = batch();
        batch.replayed = replayed;
        if !replayed {
            host.seen
                .insert((crate::ledger::effect_id(batch.serial, 1), DIM.into(), DEST));
        } else {
            // A different id, even in the same cell, proves nothing about this batch's order.
            host.seen
                .insert((crate::ledger::effect_id(batch.serial, 9), DIM.into(), DEST));
        }
        server.ready.world.machine_effects.push_back(batch);
        server.flush_machine_effects(&mut host);
        assert_eq!(host.blasts, vec![blast()]);
    }
}

#[test]
fn a_reopened_home_is_written_empty_when_its_last_batch_is_paid() {
    let mut host = Host::default();
    host.homes.insert([0, 0], vec![batch()]);
    let mut server = Server::native(&mut host, 0);
    server.native_obligations_loaded(
        DIM,
        [0, 0],
        vec![],
        host.homes[&[0, 0]].clone(),
        BTreeSet::new(),
    );
    server.native_settle(&mut host);
    assert!(server.ready.world.machine_effects.is_empty());
    assert!(
        host.homes.is_empty(),
        "the loaded record must lose its old batch on full payment"
    );
    assert_eq!(
        host.empty_homes,
        [[0, 0]],
        "write the empty record even when no pending batch remains"
    );
    let mut reopened = Server::native(&mut host, 1);
    for (chunk, batches) in host.homes.clone() {
        reopened.native_obligations_loaded(DIM, chunk, vec![], batches, BTreeSet::new());
    }
    reopened.native_settle(&mut host);
    assert_eq!(host.blasts.len(), 1, "a second reopen owes no explosion");
}
