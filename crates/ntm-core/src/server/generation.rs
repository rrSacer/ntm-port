//! One cell identity for machine steps, inputs and delayed cell mutations.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CellGuard {
    pub dim: String,
    pub pos: Pos,
    pub generation: u64,
    /// Block id: property changes do not create another block entity.
    pub state: String,
}
impl CellGuard {
    pub fn machine(key: &Key, m: &Machine) -> Self {
        Self {
            dim: key.0.clone(),
            pos: key.1,
            generation: m.generation,
            state: m.block.clone(),
        }
    }
    pub fn key(&self) -> Key {
        (self.dim.clone(), self.pos)
    }
}

impl Server {
    pub(super) fn current_guard(&self, g: &CellGuard) -> bool {
        g.generation != u64::MAX
            && self
                .ready
                .world
                .generations
                .get(&g.key())
                .copied()
                .unwrap_or(0)
                == g.generation
    }
    pub(super) fn model_current(&self, key: &Key, m: &Machine) -> bool {
        self.current_guard(&CellGuard::machine(key, m))
    }
    pub(super) fn remember_removal(&mut self, key: &Key) {
        let Some(m) = self.ready.world.machines.get(key) else {
            return;
        };
        if !self
            .ready
            .world
            .removals
            .iter()
            .any(|r| r.key == *key && r.machine.generation == m.generation)
        {
            self.ready.facts_dirty = true;
            self.ready.world.removals.push(crate::persist::Removal {
                key: key.clone(),
                machine: m.clone(),
                engine_slots: self.ready.bridge.last.contains_key(key)
                    || container_bridge::layout(&m.block).is_some(),
            });
        }
        self.ready.broken.insert(key.clone(), m.block.clone());
    }
    /// Positive absence is a fact, independent of the rest of a coupled group.
    /// A pending owned role transition still expects the old physical block.
    pub(super) fn observe_removals(&mut self, host: &mut impl ServerHost) {
        let absent: Vec<_> = self
            .ready
            .world
            .machines
            .iter()
            .filter_map(|(k, m)| {
                if self.ready.broken.contains_key(k) {
                    return None;
                }
                let actual = host.get_block(&k.0, k.1)?;
                let expected = self
                    .ready
                    .world
                    .machine_effects
                    .iter()
                    .flat_map(|b| &b.effects)
                    .find_map(|e| match e {
                        WorldEffect::Blocks { expected, .. } => expected
                            .iter()
                            .find(|g| g.key() == *k && g.generation == m.generation)
                            .map(|g| g.state.as_str()),
                        _ => None,
                    })
                    .unwrap_or(&m.block);
                (blockstate::id_of(&actual) != blockstate::id_of(expected)).then_some((k.clone(), m.generation))
            })
            .collect();
        for (k, generation) in absent {
            if host.trace_effects() {
                let held = host.get_block(&k.0, k.1);
                host.log(&format!("observe_removals: {k:?} generation {generation} is absent: world holds {held:?}"));
            }
            if self.ready.world.generations.get(&k).copied().unwrap_or(0) == generation {
                self.ready
                    .world
                    .generations
                    .insert(k.clone(), generation.saturating_add(1));
            }
            self.remember_removal(&k);
        }
    }
    pub(super) fn checkpoint_facts(&mut self, host: &mut impl ServerHost) -> bool {
        if !self.ready.facts_dirty && !self.ready.facts_pending {
            return true;
        }
        if !self.world_changed() {
            self.ready.facts_pending = false;
            self.ready.facts_dirty = false;
            return true;
        }
        match self.write_world(host) {
            Ok(()) => {
                self.ready.facts_pending = false;
                self.ready.facts_dirty = false;
                true
            }
            Err(e) => {
                self.ready.facts_pending = true;
                host.log(&format!("machine generation/removal checkpoint: {e}"));
                false
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Guarded<T> {
    pub guard: CellGuard,
    pub value: T,
}

impl Server {
    /// Menu producers bind the click to the BE that supplied its saved state.
    pub fn native_menu_button(
        &mut self,
        player: &str,
        dim: &str,
        pos: Pos,
        button: u32,
        generation: u64,
    ) -> bool {
        let key = (dim.to_string(), pos);
        let Some(m) = self.ready.world.machines.get(&key) else {
            return false;
        };
        if m.generation != generation || !self.model_current(&key, m) {
            return false;
        }
        self.ready.pending_buttons.push(Guarded {
            guard: CellGuard::machine(&key, m),
            value: (player.into(), dim.into(), pos, button),
        });
        true
    }
    pub(super) fn cancel_obsolete_inputs(&mut self, host: &mut impl ServerHost) {
        let cancelled: Vec<_> = self
            .ready
            .detonations
            .iter()
            .filter(|(k, i)| !self.current_guard(&i.guard) || self.ready.broken.contains_key(*k))
            .map(|(k, _)| k.clone())
            .collect();
        for k in cancelled {
            if let Some(i) = self.ready.detonations.remove(&k) {
                host.tell(
                    &i.value.0,
                    "Detonação cancelada: os componentes mudaram ou a bomba foi removida.",
                );
            }
        }
        let buttons = std::mem::take(&mut self.ready.pending_buttons);
        self.ready.pending_buttons = buttons
            .into_iter()
            .filter(|i| {
                self.current_guard(&i.guard) && !self.ready.broken.contains_key(&i.guard.key())
            })
            .collect();
    }
}
