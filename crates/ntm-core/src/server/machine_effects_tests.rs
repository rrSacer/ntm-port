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
    blocks: BTreeMap<Pos, String>,
    saved: BTreeMap<Pos, String>,
    refuse_commit: bool,
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
    fn get_block(&mut self, _: &str, pos: Pos) -> Option<String> {
        Some(
            self.blocks
                .get(&pos)
                .cloned()
                .unwrap_or_else(|| "minecraft:air".into()),
        )
    }
    fn commit_machine(&mut self, commit: &MachineCommit) -> bool {
        if self.refuse_commit {
            return false;
        }
        self.order.push("commit");
        self.saved.insert(commit.pos, commit.json.clone());
        true
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

fn synthetic_machine(host: &mut Host, server: &mut Server, pos: Pos, facing: Option<&str>) -> Key {
    use crate::machine::{Data, Kind};
    let key = (DIM.to_string(), pos);
    let mut machine = Machine::new(Kind::Inert, "ntm:effect_probe", facing.map(str::to_string));
    machine.data = Data::Inert {
        name: "__effect_probe_armed".into(),
    };
    let json = serde_json::to_string(&machine).unwrap();
    host.blocks.insert(pos, machine.block.clone());
    host.saved.insert(pos, json.clone());
    server.ready.native_saved.insert(key.clone(), json);
    server.ready.loaded.insert(key.clone(), true);
    server.ready.world.machines.insert(key.clone(), machine);
    key
}
fn probe_name(server: &Server, key: &Key) -> String {
    let crate::machine::Data::Inert { name } = &server.ready.world.machines[key].data else {
        panic!()
    };
    name.clone()
}
fn phase(machine: &mut Machine) -> Vec<crate::machine::MachineEffect> {
    let crate::machine::Data::Inert { name } = &mut machine.data else {
        panic!()
    };
    if name.as_str() == "__effect_probe_spent" {
        return vec![];
    }
    *name = "__effect_probe_spent".into();
    vec![crate::machine::MachineEffect::Explode {
        offset: [0., 1., 0.],
        power: 5.,
        fire: false,
        breaks: false,
    }]
}
#[test]
fn a_common_step_publishes_its_model_and_critical_effects_only_after_acceptance() {
    let mut host = Host::default();
    let mut server = Server::native(&mut host, 0);
    let key = synthetic_machine(&mut host, &mut server, CORE, Some("north"));
    let before = server.ready.world.machines[&key].clone();
    host.refuse_commit = true;
    server.local_steps(&mut host);
    server.sync_obligations(&mut host);
    assert_eq!(server.ready.world.machines[&key], before);
    assert!(server.ready.world.machine_effects.is_empty() && host.homes.is_empty());
    assert!(host.blasts.is_empty() && host.items.is_empty());
    host.refuse_commit = false;
    server.local_steps(&mut host);
    assert_eq!(probe_name(&server, &key), "__effect_probe_spent");
    assert_eq!(server.ready.world.machine_effects.len(), 1);
    let batch = &server.ready.world.machine_effects[0];
    assert_eq!(batch.owners, [key.clone()]);
    assert_eq!(batch.home(), Some((DIM.into(), 0, 0)));
    assert_eq!(batch.origins, [CellGuard::machine(&key, &before)]);
    assert_eq!(batch.effects, VecDeque::from([blast(), drop()]));
    assert!(
        host.blasts.is_empty() && host.items.is_empty(),
        "committed, owed, not published before the commit"
    );
    assert_eq!(
        serde_json::from_str::<Machine>(&host.saved[&CORE]).unwrap(),
        server.ready.world.machines[&key]
    );
    server.local_steps(&mut host);
    assert_eq!(
        server.ready.world.machine_effects.len(),
        1,
        "the waiting batch freezes the consumer"
    );
    server.sync_obligations(&mut host);
    assert_eq!(host.homes[&[0, 0]].len(), 1);
    server.flush_machine_effects(&mut host);
    assert_eq!(host.order, ["commit", "explosion attempt", "items"]);
}
#[test]
fn relative_outputs_rotate_about_the_owners_core_and_never_another_machine() {
    use crate::machine::MachineEffect;
    for (facing, expected) in [
        (None, [17, 65, -3]),
        (Some("north"), [17, 65, -3]),
        (Some("east"), [18, 65, 2]),
    ] {
        let key = (DIM.to_string(), CORE);
        let drop = MachineEffect::Drop {
            offset: [2, 1, 3],
            item: "probe".into(),
            count: 7,
        }
        .in_world(&key, facing);
        assert_eq!(
            drop,
            WorldEffect::Items {
                dim: DIM.into(),
                pos: expected,
                items: vec![("probe".into(), 7)]
            }
        );
        let blast = MachineEffect::Explode {
            offset: [2., 1., 3.],
            power: 5.,
            fire: false,
            breaks: false,
        }
        .in_world(&key, facing);
        assert_eq!(
            blast,
            WorldEffect::Explosion {
                dim: DIM.into(),
                at: [
                    f64::from(expected[0]) + 0.5,
                    65.,
                    f64::from(expected[2]) + 0.5
                ],
                power: 5.,
                fire: false,
                breaks: false
            }
        );
    }
    let mut host = Host::default();
    let mut server = Server::native(&mut host, 0);
    synthetic_machine(&mut host, &mut server, [-160, 64, 0], Some("west"));
    let owner = synthetic_machine(&mut host, &mut server, CORE, Some("north"));
    assert!(server.publish_machine_effects(&mut host, &owner, phase));
    let batch = &server.ready.world.machine_effects[0];
    assert_eq!(batch.owners, [owner]);
    assert_eq!(batch.home(), Some((DIM.into(), 0, 0)));
    assert_eq!(batch.effects, VecDeque::from([blast()]));
}
#[test]
fn a_post_step_phase_refused_at_its_first_commit_publishes_neither_state_nor_obligation() {
    let mut host = Host::default();
    let mut server = Server::native(&mut host, 0);
    let key = synthetic_machine(&mut host, &mut server, CORE, Some("north"));
    let before = server.ready.world.machines[&key].clone();
    host.refuse_commit = true;
    assert!(!server.publish_machine_effects(&mut host, &key, phase));
    server.native_save(&mut host, false);
    server.native_world_saving(&mut host);
    assert_eq!(server.ready.world.machines[&key], before);
    assert_eq!(
        serde_json::from_str::<Machine>(&host.saved[&CORE]).unwrap(),
        before
    );
    assert!(host.blasts.is_empty() && host.homes.is_empty());
    assert!(server.ready.world.machine_effects.is_empty());
    host.refuse_commit = false;
    assert!(server.publish_machine_effects(&mut host, &key, phase));
    // The first commit was accepted. Refusal of a redundant native_save cannot restore the earlier BE.
    host.refuse_commit = true;
    server.native_save(&mut host, false);
    assert_eq!(probe_name(&server, &key), "__effect_probe_spent");
    assert_eq!(
        serde_json::from_str::<Machine>(&host.saved[&CORE]).unwrap(),
        server.ready.world.machines[&key]
    );
    server.sync_obligations(&mut host);
    assert_eq!(host.homes[&[0, 0]].len(), 1);
    host.refuse_commit = false;
    server.native_world_saving(&mut host);
    assert_eq!(host.blasts, [blast()]);
    assert!(host.homes.is_empty());
    assert!(server.publish_machine_effects(&mut host, &key, phase));
    server.native_world_saving(&mut host);
    assert_eq!(
        host.blasts.len(),
        1,
        "a completed phase has nothing new to publish"
    );
}
#[test]
fn a_save_between_this_tick_and_the_next_keeps_the_post_phase_batch_with_its_model() {
    let mut host = Host {
        refuse_blast: true,
        ..Default::default()
    };
    let mut server = Server::native(&mut host, 0);
    let key = synthetic_machine(&mut host, &mut server, CORE, Some("north"));
    assert!(server.publish_machine_effects(&mut host, &key, phase));
    server.native_save(&mut host, true);
    server.sync_obligations(&mut host);
    server.native_world_saving(&mut host);
    // Capture immediately, with no next tick. Same home record contains the committed model and its debt.
    let saved_model = host.saved[&CORE].clone();
    let saved_batches = host.homes.get(&[0, 0]).cloned().unwrap_or_default();
    assert_eq!(
        saved_batches.len(),
        1,
        "PublishNextTick loses the debt in this cut"
    );
    assert_eq!(
        serde_json::from_str::<Machine>(&saved_model).unwrap(),
        server.ready.world.machines[&key]
    );
    for t in [1, 2] {
        let mut reopened = Server::native(&mut host, t);
        reopened.native_obligations_loaded(
            DIM,
            [0, 0],
            vec![],
            saved_batches.clone(),
            BTreeSet::new(),
        );
        assert!(reopened.native_adopt(DIM, CORE, &saved_model));
        assert_eq!(probe_name(&reopened, &key), "__effect_probe_spent");
        reopened.native_settle(&mut host);
        assert!(host.blasts.is_empty());
    }
    let mut reopened = Server::native(&mut host, 3);
    reopened.native_obligations_loaded(DIM, [0, 0], vec![], saved_batches, BTreeSet::new());
    host.refuse_blast = false;
    reopened.native_settle(&mut host);
    assert_eq!(host.blasts, [blast()]);
    assert!(host.homes.is_empty());
}
#[test]
fn the_post_phase_publisher_rejects_stale_or_replaced_owners_before_running_the_phase() {
    for stale in [false, true] {
        let mut host = Host::default();
        let mut server = Server::native(&mut host, 0);
        let key = synthetic_machine(&mut host, &mut server, CORE, None);
        if stale {
            server.ready.world.generations.insert(key.clone(), 1);
        } else {
            host.blocks.insert(CORE, "minecraft:stone".into());
        }
        assert!(
            !server
                .publish_machine_effects(&mut host, &key, |_| panic!("stale phase must not run"))
        );
        assert!(server.ready.world.machine_effects.is_empty());
    }
}
