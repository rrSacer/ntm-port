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
