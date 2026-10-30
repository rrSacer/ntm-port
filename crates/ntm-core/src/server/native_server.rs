//! The server as the native bundle runs it (docs/nativo.md, stage L3 of the
//! migration): in a world from the start, with no world id, transactions,
//! probes or client channel. Each machine's state is its block entity's
//! (the host's `save_machine`, read back by [`Server::native_adopt`]); the
//! phases are [`Server::tick`]'s, on the same [`ServerHost`].
use super::*;

impl Server {
    /// A server for the native bundle, in its world.
    pub fn native(host: &mut impl ServerHost, tick: u64) -> Self {
        let mut s = Server::new(&mut crate::kv::MemKv::default(), tick);
        s.tick = tick;
        s.enter_world(host, "native".into());
        s
    }

    /// One server tick: the blocks that changed, the slots players moved,
    /// the machines and networks, the slots and states back.
    pub fn native_tick(&mut self, host: &mut impl ServerHost, tick: u64) {
        self.tick = tick;
        for e in std::mem::take(&mut self.queued) {
            self.block_event(host, e);
        }
        self.slots_in(host);
        self.heat_tick();
        self.simulate(host);
        self.fluids_tick();
        self.fraction_tick();
        self.foundry_tick(host);
        self.excavator_tick(host);
        self.drill_tick(host);
        self.bombs_tick(host);
        self.slots_out(host);
        self.native_save(host);
    }

    /// A block changed (`Level.setBlock`): a machine or cable put, taken
    /// away or replaced, as the client's events said before.
    pub fn native_block_changed(&mut self, dim: &str, pos: Pos, old: &str, new: &str) {
        let tracked = |s: &str| {
            let id = blockstate::id_of(s);
            registry::is_cable(id) || machine_from_state(s).is_some()
        };
        if blockstate::id_of(old) == blockstate::id_of(new) {
            return;
        }
        if tracked(old) {
            self.queued.push(BlockEvent::Broken(dim.into(), pos, old.into()));
        }
        if tracked(new) {
            self.queued.push(BlockEvent::Placed(dim.into(), pos, new.into()));
        }
    }

    /// A machine's block entity seen with its saved state (the world was
    /// opened again): the machine, unless it is already known. False when
    /// the state doesn't read.
    pub fn native_adopt(&mut self, dim: &str, pos: Pos, json: &str) -> bool {
        let Some(r) = self.ready() else { return false };
        let key = (dim.to_string(), pos);
        if r.world.machines.contains_key(&key) {
            return true;
        }
        let Ok(m) = serde_json::from_str::<Machine>(json) else { return false };
        r.world.machines.insert(key.clone(), m.upgraded());
        r.seen.insert(key);
        r.topo_dirty = true;
        true
    }

    /// Whether the machine at (`dim`, `pos`) is known.
    pub fn native_has(&mut self, dim: &str, pos: Pos) -> bool {
        self.ready().is_some_and(|r| r.world.machines.contains_key(&(dim.to_string(), pos)))
    }

    /// The loaded machines whose state changed since it was last kept: kept
    /// with their block entities.
    fn native_save(&mut self, host: &mut impl ServerHost) {
        let Some(r) = self.ready() else { return };
        for (key, m) in &r.world.machines {
            if r.loaded.get(key) != Some(&true) {
                continue;
            }
            let Ok(json) = serde_json::to_string(m) else { continue };
            if r.native_saved.get(key) != Some(&json) {
                host.save_machine(&key.0, key.1, &json);
                r.native_saved.insert(key.clone(), json);
            }
        }
        r.native_saved.retain(|k, _| r.world.machines.contains_key(k));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::container_bridge::{Slots, Stack};

    /// The native game as the bundle's host sees it: blocks, each block
    /// entity's slots, data and kept state.
    #[derive(Default)]
    struct World {
        blocks: BTreeMap<Pos, String>,
        slots: BTreeMap<Pos, Slots>,
        data: BTreeMap<Pos, Vec<i32>>,
        saved: BTreeMap<Pos, String>,
        kv: crate::kv::MemKv,
    }

    impl Kv for World {
        fn get(&mut self, key: &str) -> Option<Vec<u8>> {
            self.kv.get(key)
        }
        fn set(&mut self, key: &str, value: &[u8]) -> Result<(), String> {
            self.kv.set(key, value)
        }
        fn delete(&mut self, key: &str) {
            self.kv.delete(key)
        }
    }

    impl ServerHost for World {
        fn get_block(&mut self, _dim: &str, p: Pos) -> Option<String> {
            Some(self.blocks.get(&p).cloned().unwrap_or_else(|| "minecraft:air".into()))
        }
        fn set_blocks(&mut self, _dim: &str, blocks: &[(Pos, String)]) -> Result<(), String> {
            for (p, s) in blocks {
                self.blocks.insert(*p, s.clone());
            }
            Ok(())
        }
        fn tell(&mut self, _player: &str, _text: &str) {}
        fn log(&mut self, _text: &str) {}
        fn machine_slots(&mut self, _dim: &str, p: Pos) -> Option<Slots> {
            self.slots.get(&p).cloned()
        }
        fn set_machine_slot(&mut self, _dim: &str, p: Pos, slot: u32, stack: Option<Stack>) -> bool {
            let Some(s) = self.slots.get_mut(&p).and_then(|s| s.get_mut(slot as usize)) else { return false };
            *s = stack;
            true
        }
        fn set_machine_data(&mut self, _dim: &str, p: Pos, values: &[i32]) -> bool {
            self.data.insert(p, values.to_vec());
            true
        }
        fn save_machine(&mut self, _dim: &str, p: Pos, json: &str) {
            self.saved.insert(p, json.into());
        }
    }

    const OW: &str = "minecraft:overworld";
    const AT: Pos = [0, 64, 0];
    const FIREBOX: &str = "ntm:heater_firebox[facing=north,honey_level=0]";

    /// A firebox put in the world burns the coal in its block entity's slot,
    /// shows it, and keeps its state there; a server that opens the world
    /// again takes the machine back from it.
    #[test]
    fn a_native_machine_runs_on_its_block_entity_and_comes_back() {
        let mut w = World::default();
        let mut s = Server::native(&mut w, 0);
        w.blocks.insert(AT, FIREBOX.into());
        w.slots.insert(AT, vec![Some(Stack::new("minecraft:coal", 2)), None]);
        s.native_block_changed(OW, AT, "minecraft:air", FIREBOX);
        for t in 1..=40 {
            s.native_tick(&mut w, t);
        }
        assert_eq!(w.slots[&AT][0], Some(Stack::new("minecraft:coal", 1)), "um carvão queimando");
        assert!(w.data[&AT][0] > 0, "o calor na tela: {:?}", w.data[&AT]);
        assert!(w.blocks[&AT].contains("honey_level=") && !w.blocks[&AT].contains("honey_level=0"), "acesa: {}", w.blocks[&AT]);
        let saved = w.saved[&AT].clone();
        assert!(saved.contains("\"heat\""), "{saved}");

        let mut again = Server::native(&mut w, 100);
        assert!(!again.native_has(OW, AT));
        assert!(again.native_adopt(OW, AT, &saved));
        again.native_tick(&mut w, 101);
        assert!(w.data[&AT][0] > 0, "o calor voltou com o estado");

        // Broken: gone from the server.
        w.blocks.insert(AT, "minecraft:air".into());
        again.native_block_changed(OW, AT, FIREBOX, "minecraft:air");
        again.native_tick(&mut w, 102);
        assert!(!again.native_has(OW, AT));
    }
}
