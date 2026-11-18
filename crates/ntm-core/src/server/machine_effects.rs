//! Local machine outputs, translated once at their core into the durable world-effect queue.
use super::*;
use crate::machine::MachineEffect;

impl MachineEffect {
    pub(super) fn in_world(self, key: &Key, facing: Option<&str>) -> WorldEffect {
        let forward = facing.and_then(registry::facing_dir).unwrap_or([0, 0, -1]);
        // [right, up, forward]: when north, right is east. Y is the Java block Y, not Y + 0.5.
        let right = [-forward[2], 0, forward[0]];
        match self {
            MachineEffect::Explode {
                offset,
                power,
                fire,
                breaks,
            } => {
                let at = [
                    f64::from(key.1[0])
                        + 0.5
                        + offset[0] * f64::from(right[0])
                        + offset[2] * f64::from(forward[0]),
                    f64::from(key.1[1]) + offset[1],
                    f64::from(key.1[2])
                        + 0.5
                        + offset[0] * f64::from(right[2])
                        + offset[2] * f64::from(forward[2]),
                ];
                WorldEffect::Explosion {
                    dim: key.0.clone(),
                    at,
                    power,
                    fire,
                    breaks,
                }
            }
            MachineEffect::Drop {
                offset,
                item,
                count,
            } => {
                let pos = [
                    key.1[0] + offset[0] * right[0] + offset[2] * forward[0],
                    key.1[1] + offset[1],
                    key.1[2] + offset[0] * right[2] + offset[2] * forward[2],
                ];
                WorldEffect::Items {
                    dim: key.0.clone(),
                    pos,
                    items: vec![(item, count)],
                }
            }
        }
    }
}

impl Server {
    /// Runs a phase outside `Logic::tick` against a copy. It returns local outputs and may change the model, but
    /// cannot publish to the world. The host must accept its whole MachineCommit before either model or batch moves.
    /// Call in the same tick as the phase, before native_save/sync_obligations. A later native_save is redundant;
    /// its refusal cannot leave the effects with the old BE. On refusal the live model stays intact for retry.
    pub fn publish_machine_effects(
        &mut self,
        host: &mut impl ServerHost,
        key: &(String, Pos),
        phase: impl FnOnce(&mut Machine) -> Vec<MachineEffect>,
    ) -> bool {
        let Some(before) = self.ready.world.machines.get(key) else {
            return false;
        };
        if self.ready.loaded.get(key) != Some(&true)
            || !self.model_current(key, before)
            || self.ready.broken.contains_key(key)
            || !host
                .get_block(&key.0, key.1)
                .is_some_and(|s| blockstate::id_of(&s) == before.block)
        {
            return false;
        }
        let origin = CellGuard::machine(key, before);
        let Some(expected) = host.machine_slots(&key.0, key.1).or_else(|| {
            container_bridge::layout(&before.block)
                .is_none()
                .then(Vec::new)
        }) else {
            return false;
        };
        let mut machine = before.clone();
        let local = phase(&mut machine);
        // A phase may change state and contents, not the identity/generation of its machine.
        if CellGuard::machine(key, &machine) != origin
            || machine.facing != before.facing
            || machine.kind() != before.kind()
        {
            return false;
        }
        let effects = local
            .into_iter()
            .map(|e| e.in_world(key, machine.facing.as_deref()))
            .collect::<std::collections::VecDeque<_>>();
        if !effects.is_empty() {
            machine.rev += 1;
        }
        let (desired, data) = Self::machine_output(&mut machine, &expected);
        let json = serde_json::to_string(&machine).expect("machine serialization");
        let commit = MachineCommit {
            cells: Vec::new(),
            guard: origin.clone(),
            expected_saved: self.ready.native_saved.contains_key(key),
            dim: key.0.clone(),
            pos: key.1,
            expected,
            desired,
            data,
            json,
        };
        if !host.commit_machine(&commit) {
            return false;
        }
        self.ready
            .world
            .machines
            .insert(key.clone(), machine.clone());
        self.ready.bridge.last.insert(key.clone(), commit.desired);
        self.ready.bridge.data.insert(key.clone(), commit.data);
        self.ready
            .native_saved
            .insert(key.clone(), commit.json.clone());
        if !effects.is_empty() {
            self.journal_survivor(Survivor {
                dim: key.0.clone(),
                pos: key.1,
                generation: machine.generation,
                rev: machine.rev,
                json: commit.json,
            });
            let batch = CommittedEffects {
                serial: self.batch_serial(),
                owners: vec![key.clone()],
                origins: vec![origin],
                effects,
                ..Default::default()
            };
            self.ready.world.machine_effects.push_back(batch);
        }
        true
    }
}
