//! A native machine step is speculative until its whole inventory and state
//! are accepted. Networks only ever see confirmed steps.
use super::*;
use container_bridge::{Slots, Stack};

/// Item and block effects survive an unavailable level/host. Completed entries
/// are removed individually, so a partial retry never repeats an ejection.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum WorldEffect {
    Blocks {
        dim: String,
        blocks: Vec<(Pos, String)>,
    },
    Items {
        dim: String,
        pos: Pos,
        items: Vec<(String, u32)>,
    },
    PileExplosion {
        dim: String,
        pos: [f64; 3],
    },
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommittedEffects {
    pub owners: Vec<(String, Pos)>,
    pub effects: std::collections::VecDeque<WorldEffect>,
    /// Removed machines retain their contents here until every effect succeeds.
    pub tombstones: Vec<((String, Pos), Machine)>,
}

/// World effects are held until the inventory/state commit succeeds.
type AfterCommit<H> = Box<dyn FnOnce(&mut H)>;

struct Deferred<'a, H> {
    host: &'a mut H,
    slots: BTreeMap<Key, Slots>,
    effects: Vec<AfterCommit<H>>,
    critical: std::collections::VecDeque<WorldEffect>,
}
impl<H: ServerHost> Kv for Deferred<'_, H> {
    fn get(&mut self, k: &str) -> Option<Vec<u8>> {
        self.host.get(k)
    }
    fn set(&mut self, _: &str, _: &[u8]) -> Result<(), String> {
        Err("storage outside local step".into())
    }
    fn delete(&mut self, _: &str) {}
}
impl<H: ServerHost> ServerHost for Deferred<'_, H> {
    fn get_block(&mut self, d: &str, p: Pos) -> Option<String> {
        self.host.get_block(d, p)
    }
    fn block_resistance(&mut self, d: &str, p: Pos) -> Option<f32> {
        self.host.block_resistance(d, p)
    }
    fn machine_slots(&mut self, d: &str, p: Pos) -> Option<Slots> {
        self.slots.get(&(d.into(), p)).cloned()
    }
    fn machine_data(&mut self, d: &str, p: Pos) -> Option<Vec<i32>> {
        self.host.machine_data(d, p)
    }
    fn redstone_many(&mut self, d: &str, p: &[Pos]) -> Vec<Option<u8>> {
        self.host.redstone_many(d, p)
    }
    fn pile_reachable(&mut self, d: &str) -> bool {
        self.host.pile_reachable(d)
    }
    fn pile_depletion(&mut self, s: &Stack) -> f64 {
        self.host.pile_depletion(s)
    }
    fn solar_sample(&mut self, d: &str, p: Pos) -> Option<(i32, i32, bool)> {
        self.host.solar_sample(d, p)
    }
    fn set_blocks(&mut self, d: &str, b: &[(Pos, String)]) -> Result<(), String> {
        self.critical.push_back(WorldEffect::Blocks {
            dim: d.into(),
            blocks: b.to_vec(),
        });
        Ok(())
    }
    fn tell(&mut self, p: &str, t: &str) {
        let (p, t) = (p.to_string(), t.to_string());
        self.effects.push(Box::new(move |h| h.tell(&p, &t)));
    }
    fn log(&mut self, t: &str) {
        let t = t.to_string();
        self.effects.push(Box::new(move |h| h.log(&t)));
    }
    fn drop_items(&mut self, d: &str, p: Pos, i: &[(String, u32)]) -> bool {
        self.critical.push_back(WorldEffect::Items {
            dim: d.into(),
            pos: p,
            items: i.to_vec(),
        });
        true
    }
    fn pile_drop(&mut self, d: &str, p: Pos, i: &str) -> bool {
        self.drop_items(d, p, &[(i.into(), 1)])
    }
    fn pile_explode(&mut self, d: &str, p: [f64; 3]) -> bool {
        self.critical.push_back(WorldEffect::PileExplosion {
            dim: d.into(),
            pos: p,
        });
        true
    }
    fn hurt(&mut self, p: &str, a: f32, k: &str) {
        let (p, k) = (p.to_string(), k.to_string());
        self.effects.push(Box::new(move |h| h.hurt(&p, a, &k)));
    }
    fn effect(&mut self, p: &str, e: &str, s: u32, a: u8) {
        let (p, e) = (p.to_string(), e.to_string());
        self.effects.push(Box::new(move |h| h.effect(&p, &e, s, a)));
    }
    fn show(&mut self, d: &str, s: &crate::blast::visuals::Show) {
        let (d, s) = (d.to_string(), s.clone());
        self.effects.push(Box::new(move |h| h.show(&d, &s)));
    }
}

impl Server {
    pub(super) fn flush_machine_effects(&mut self, host: &mut impl ServerHost) {
        let batches = self.ready.world.machine_effects.len();
        if batches == 0 {
            return;
        }
        for _ in 0..batches {
            loop {
                let Some(batch) = self.ready.world.machine_effects.front() else {
                    return;
                };
                let Some(effect) = batch.effects.front() else {
                    self.ready.world.machine_effects.pop_front();
                    break;
                };
                let dim = match effect {
                    WorldEffect::Blocks { dim, .. }
                    | WorldEffect::Items { dim, .. }
                    | WorldEffect::PileExplosion { dim, .. } => dim,
                };
                if !host.pile_reachable(dim) {
                    self.ready.world.machine_effects.rotate_left(1);
                    break;
                }
                // Record the tombstone/intent before releasing its world effect.
                let kept = self.ready.world.kept(self.ready.world.ticks);
                if let Err(e) = host.set(crate::persist::KEY, &kept) {
                    host.log(&format!("pending machine effect: {e}"));
                    return;
                }
                let effect = self
                    .ready
                    .world
                    .machine_effects
                    .front()
                    .unwrap()
                    .effects
                    .front()
                    .unwrap();
                let done = match effect {
                    WorldEffect::Blocks { dim, blocks } => host
                        .set_machine_blocks(
                            dim,
                            blocks,
                            &self.ready.world.machine_effects.front().unwrap().owners,
                        )
                        .is_ok(),
                    WorldEffect::Items { dim, pos, items } => host.drop_items(dim, *pos, items),
                    WorldEffect::PileExplosion { dim, pos } => host.pile_explode(dim, *pos),
                };
                if !done {
                    self.ready.world.machine_effects.rotate_left(1);
                    break;
                }
                let batch = self.ready.world.machine_effects.front_mut().unwrap();
                let effect = batch.effects.pop_front().unwrap();
                // A successful spawn still exists only in the world's memory.
                // Keep its replay receipt until a flushed world save confirms
                // entities/blocks on disk; never clear it just for a KV write.
                let receipt = CommittedEffects {
                    owners: batch.owners.clone(),
                    tombstones: Vec::new(),
                    effects: std::collections::VecDeque::from([effect]),
                };
                if matches!(receipt.effects.front(), Some(WorldEffect::Blocks { .. })) {
                    // A block-role replacement can recreate its BE. The new
                    // entity needs the confirmed JSON again in this same tick.
                    for k in &receipt.owners {
                        self.ready.native_saved.remove(k);
                    }
                }
                self.ready.world.applied_machine_effects.push_back(receipt);
            }
        }
        // Persist the advanced cursor even when the queue became empty. The
        // previous pre-effect write must not make a completed batch reappear.
        let kept = self.ready.world.kept(self.ready.world.ticks);
        if host.set(crate::persist::KEY, &kept).is_ok() {
            self.ready.kept = self.ready.world.kept(0);
        } else {
            self.ready.kept = Vec::new();
        }
    }

    /// The existing engine hook reports whether world/entity writes have
    /// actually flushed. Until then replay after a crash may duplicate an
    /// effect, but must never acknowledge an entity not yet saved by the world.
    pub fn native_world_saved(&mut self, host: &mut impl ServerHost, wait: bool) {
        if !wait || self.ready.world.applied_machine_effects.is_empty() {
            return;
        }
        let applied = std::mem::take(&mut self.ready.world.applied_machine_effects);
        let kept = self.ready.world.kept(self.ready.world.ticks);
        if let Err(e) = host.set(crate::persist::KEY, &kept) {
            self.ready.world.applied_machine_effects = applied;
            host.log(&format!("machine effect receipts: {e}"));
        } else {
            self.ready.kept = self.ready.world.kept(0);
        }
    }

    /// Block events are observed facts. Broken states are handled by the same
    /// speculative group step as missing-but-loaded blocks, never as drops
    /// before an inventory commit. Placement still installs the model first.
    pub(super) fn native_event(&mut self, host: &mut impl ServerHost, e: BlockEvent) {
        match e {
            BlockEvent::Broken(d, p, s) => {
                let k = (d.clone(), p);
                if let Some(m) = self.ready.world.machines.get(&k) {
                    // Ignore delayed notifications from our own replacement.
                    if m.block == blockstate::id_of(&s) {
                        // Its BE may already be gone even if the level closes
                        // before its coupled group can commit. Keep the full
                        // confirmed state outside that missing BE meanwhile.
                        if !self.ready.world.removals.iter().any(|(key, _)| key == &k) {
                            self.ready.world.removals.push((k.clone(), m.clone()));
                        }
                        self.ready.broken.insert(k, s);
                    }
                } else {
                    self.block_event(host, BlockEvent::Broken(d, p, s));
                }
            }
            BlockEvent::Placed(d, p, s) => {
                let k = (d.clone(), p);
                if self.ready.broken.contains_key(&k) {
                    self.queued.push(BlockEvent::Placed(d, p, s));
                } else {
                    self.block_event(host, BlockEvent::Placed(d, p, s));
                }
            }
        }
    }

    /// Drilling an existing pile may eject rods. Publish the new core and
    /// members together before releasing either role changes or rod drops.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn atomic_pile_edit(
        &mut self,
        host: &mut impl ServerHost,
        dim: &str,
        at: Pos,
        face: Pos,
        held: Option<&Stack>,
        depletion: f64,
        player: &str,
    ) -> Option<PileUse> {
        let before = self.ready.clone();
        let mut observed = BTreeMap::new();
        for (k, m) in &before.world.machines {
            if k.0 == dim && matches!(m.data, crate::machine::Data::Pile(_)) {
                observed.insert(k.clone(), host.machine_slots(dim, k.1).unwrap_or_default());
            }
        }
        let mut staged = Deferred {
            host,
            slots: observed.clone(),
            effects: Vec::new(),
            critical: std::collections::VecDeque::new(),
        };
        let outcome =
            self.native_pile_use_inner(&mut staged, dim, at, face, held, depletion, player);
        let keys: Vec<_> = self
            .ready
            .world
            .machines
            .iter()
            .filter(|(k, m)| before.world.machines.get(*k) != Some(*m))
            .map(|(k, _)| k.clone())
            .collect();
        let mut commits = Vec::new();
        for k in &keys {
            let Some(expected) = observed.get(k) else {
                self.ready = before;
                return Some(PileUse::Handled);
            };
            let m = self.ready.world.machines.get_mut(k).unwrap();
            let (desired, data) = Self::machine_output(m, expected);
            commits.push(MachineCommit {
                dim: k.0.clone(),
                pos: k.1,
                expected: expected.clone(),
                desired,
                data,
                json: serde_json::to_string(m).expect("machine serialization"),
            });
        }
        if !staged.host.commit_machines(&commits) {
            self.ready = before;
            return Some(PileUse::Handled);
        }
        for c in commits {
            let k = (c.dim, c.pos);
            self.ready.bridge.last.insert(k.clone(), c.desired);
            self.ready.bridge.data.insert(k.clone(), c.data);
            self.ready.native_saved.insert(k, c.json);
        }
        if !staged.critical.is_empty() {
            self.ready
                .world
                .machine_effects
                .push_back(CommittedEffects {
                    owners: keys,
                    effects: staged.critical,
                    tombstones: Vec::new(),
                });
        }
        for effect in staged.effects {
            effect(staged.host);
        }
        self.flush_machine_effects(staged.host);
        self.native_save(staged.host);
        outcome
    }

    pub(super) fn local_steps(&mut self, host: &mut impl ServerHost) {
        let mut buttons = std::mem::take(&mut self.ready.pending_buttons);
        buttons.extend(host.machine_buttons());
        let mut groups: Vec<Vec<Key>> = Vec::new();
        let mut piles: BTreeMap<Key, Vec<Key>> = BTreeMap::new();
        for (k, m) in &self.ready.world.machines {
            if self
                .ready
                .world
                .machine_effects
                .iter()
                .any(|b| b.owners.contains(k))
            {
                continue;
            }
            if let crate::machine::Data::Pile(p) = &m.data {
                use crate::pile::{Pile, offset};
                let core = match &**p {
                    Pile::Core(_) => Some(k.clone()),
                    Pile::Member(Some(at)) => Some((k.0.clone(), *at)),
                    Pile::Control(_) => self.pile_core_at(&k.0, offset(k.1, [0, -1, 0], 1)),
                    Pile::Loader(_) | Pile::Vent(_) => {
                        let out = m
                            .facing
                            .as_deref()
                            .and_then(registry::facing_dir)
                            .unwrap_or([0, 0, -1]);
                        self.pile_core_at(&k.0, offset(k.1, out, -1))
                    }
                    _ => None,
                }
                .unwrap_or_else(|| k.clone());
                piles.entry(core).or_default().push(k.clone());
            } else if host.get_block(&k.0, k.1).is_some() {
                groups.push(vec![k.clone()]);
            }
        }
        groups.extend(piles.into_values());
        for keys in groups {
            // Isolate a coupled group. World lookups still go to the host;
            // inter-machine energy/heat/fluid exchanges run afterwards.
            let mut outside = std::mem::take(&mut self.ready.world.machines);
            for k in &keys {
                if let Some(m) = outside.remove(k) {
                    self.ready.world.machines.insert(k.clone(), m);
                }
            }
            let before = self.ready.clone();
            let lit_before = self.blasts.lit.clone();
            let mut observed = BTreeMap::new();
            let accessible = keys.iter().all(|k| {
                let Some(m) = self.ready.world.machines.get(k) else {
                    return false;
                };
                let Some(actual) = host.get_block(&k.0, k.1) else {
                    return false;
                };
                // Broken identifies the old BE generation even if a player
                // already placed the same machine id here again this tick.
                if before.broken.contains_key(k) || blockstate::id_of(&actual) != m.block {
                    observed.insert(k.clone(), Vec::new());
                    return true;
                }
                match host.machine_slots(&k.0, k.1) {
                    Some(v) => {
                        observed.insert(k.clone(), v);
                        true
                    }
                    None if container_bridge::layout(&m.block).is_none() => {
                        observed.insert(k.clone(), Vec::new());
                        true
                    }
                    None => false,
                }
            });
            let mut accepted = false;
            if accessible {
                let mut staged = Deferred {
                    host,
                    slots: observed.clone(),
                    effects: Vec::new(),
                    critical: std::collections::VecDeque::new(),
                };
                let removed: Vec<_> = keys
                    .iter()
                    .filter(|k| {
                        self.ready.broken.contains_key(*k)
                            || staged.get_block(&k.0, k.1).is_some_and(|b| {
                                blockstate::id_of(&b) != before.world.machines[*k].block
                            })
                    })
                    .cloned()
                    .collect();
                for k in removed {
                    self.block_event(
                        &mut staged,
                        BlockEvent::Broken(
                            k.0.clone(),
                            k.1,
                            before.world.machines[&k].block.clone(),
                        ),
                    );
                }
                self.slots_in(&mut staged);
                for (_, d, p, b) in &buttons {
                    if keys.contains(&(d.clone(), *p)) {
                        self.machine_button(&(d.clone(), *p), *b);
                    }
                }
                self.pile_tick(&mut staged);
                self.zirnox_tick(&mut staged);
                for m in self.ready.world.machines.values_mut() {
                    m.logic_mut().tick();
                    match &mut m.data {
                        crate::machine::Data::Crucible(c) => {
                            c.local_step(crate::foundry::Tables::ntm(), self.tick);
                        }
                        crate::machine::Data::GasCent(g) => {
                            // End-of-line materialisation changes slots; the
                            // cascade itself only transfers pseudo-fluid later.
                            let back =
                                m.facing.as_deref().and_then(registry::facing_dir).map(|d| {
                                    (
                                        keys[0].0.clone(),
                                        [
                                            keys[0].1[0] - d[0],
                                            keys[0].1[1] - d[1],
                                            keys[0].1[2] - d[2],
                                        ],
                                    )
                                });
                            let connected=back.as_ref().and_then(|k|outside.get(k)).is_some_and(|m|matches!(&m.data,crate::machine::Data::GasCent(other) if other.tank.fluid==g.tank.fluid));
                            if !connected {
                                crate::gascent::end_of_line(g);
                            }
                        }
                        _ => {}
                    }
                }
                self.excavator_tick(&mut staged);
                self.drill_tick(&mut staged);
                self.bombs_triggers(&mut staged);
                let mut commits = Vec::new();
                for k in &keys {
                    let last = &observed[k];
                    if before.broken.contains_key(k)
                        || !staged.get_block(&k.0, k.1).is_some_and(|b| {
                            blockstate::id_of(&b) == before.world.machines[k].block
                        })
                    {
                        continue;
                    }
                    let (want, data, json) = if let Some(m) = self.ready.world.machines.get_mut(k) {
                        let (want, data) = Self::machine_output(m, last);
                        (
                            want,
                            data,
                            serde_json::to_string(m).expect("machine serialization"),
                        )
                    } else {
                        (
                            vec![None; last.len()],
                            Vec::new(),
                            serde_json::to_string(&before.world.machines[k])
                                .expect("machine serialization"),
                        )
                    };
                    // Static pile members need not rewrite their BE every
                    // tick. Keep at least one accessible member in a no-op
                    // group, so an unsupported atomic host still freezes it.
                    if commits.is_empty()
                        || last != &want
                        || self.ready.native_saved.get(k) != Some(&json)
                        || self.ready.bridge.data.get(k) != Some(&data)
                    {
                        commits.push(MachineCommit {
                            dim: k.0.clone(),
                            pos: k.1,
                            expected: last.clone(),
                            desired: want,
                            data,
                            json,
                        });
                    }
                }
                accepted = staged.host.commit_machines(&commits);
                if accepted {
                    buttons.retain(|(_, d, p, _)| !keys.contains(&(d.clone(), *p)));
                    for k in &keys {
                        self.ready.broken.remove(k);
                        self.ready.world.removals.retain(|(key, _)| key != k);
                        if !self.ready.world.machines.contains_key(k) {
                            self.slots_gone(k);
                            self.ready.native_saved.remove(k);
                        }
                    }
                    for c in commits {
                        let k = (c.dim, c.pos);
                        if !self.ready.world.machines.contains_key(&k) {
                            continue;
                        }
                        self.ready.bridge.last.insert(k.clone(), c.desired);
                        self.ready.bridge.data.insert(k.clone(), c.data);
                        self.ready.native_saved.insert(k, c.json);
                    }
                    let tombstones = keys
                        .iter()
                        .filter(|k| !self.ready.world.machines.contains_key(*k))
                        .map(|k| (k.clone(), before.world.machines[k].clone()))
                        .collect();
                    if !staged.critical.is_empty() {
                        self.ready
                            .world
                            .machine_effects
                            .push_back(CommittedEffects {
                                owners: keys.clone(),
                                effects: staged.critical,
                                tombstones,
                            });
                    }
                    for effect in staged.effects {
                        effect(staged.host);
                    }
                }
            }
            if !accepted {
                self.ready = before;
                self.blasts.lit = lit_before;
                // No simulation or network exchange while the BE is missing
                // or its atomic host rejects the step. A future tick retries.
                for k in &keys {
                    self.ready.loaded.insert(k.clone(), false);
                    self.ready.frozen.insert(k.clone());
                }
            }
            self.ready.world.machines.append(&mut outside);
        }
        buttons.retain(|(_, d, p, _)| self.ready.world.machines.contains_key(&(d.clone(), *p)));
        self.ready.pending_buttons = buttons;
    }
}
