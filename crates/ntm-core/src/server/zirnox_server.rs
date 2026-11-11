//! ZIRNOX world effects use existing native blocks, explosion, particles,
//! radiation dose and fluid/slot bridges; no second network or explosion engine.
use super::*;
use crate::{
    blast::visuals::Show,
    machine::Data,
    zirnox::{self, Debris, Waste, Zirnox},
};

fn particle(host: &mut impl ServerHost, dim: &str, id: &'static str, at: [f64; 3], count: i32) {
    host.show(
        dim,
        &Show::Particles {
            id,
            color: [1.; 4],
            at,
            spread: [0.05; 3],
            speed: 0.,
            count,
        },
    );
}
fn seed(dim: &str, p: Pos, t: u64) -> u64 {
    crate::hash::mix(
        crate::hash::fnv64(dim.as_bytes())
            ^ t
            ^ (p[0] as u64).rotate_left(17)
            ^ (p[1] as u64).rotate_left(31)
            ^ (p[2] as u64).rotate_left(47),
    )
}
fn roll(s: u64, n: u64) -> u64 {
    crate::hash::mix(s) % n
}

impl Server {
    pub(super) fn zirnox_tick(&mut self, host: &mut impl ServerHost) {
        let keys: Vec<_> = self
            .ready
            .world
            .machines
            .iter()
            .filter(|(_, m)| matches!(m.data, Data::Zirnox(_)))
            .map(|(k, _)| k.clone())
            .collect();
        for key in keys {
            if !host.pile_reachable(&key.0) {
                self.ready.loaded.insert(key.clone(), false);
                continue;
            }
            let Some(mut m) = self.ready.world.machines.remove(&key) else {
                continue;
            };
            // An externally removed core must not react or fail before the
            // reconciliation sweep notices it.
            if !host
                .get_block(&key.0, key.1)
                .is_some_and(|s| blockstate::id_of(&s) == m.block)
            {
                self.ready.loaded.insert(key.clone(), false);
                self.ready.world.machines.insert(key, m);
                continue;
            }
            self.ready.loaded.insert(key.clone(), true);
            let Data::Zirnox(z) = &mut m.data else {
                unreachable!()
            };
            if z.destroyed {
                self.zirnox_ruin(host, &key, z);
            } else if host.machine_slots(&key.0, key.1).is_some() {
                let cells = Zirnox::redstone_cells(key.1, m.facing.as_deref());
                // Read the received signal at the reactor's own surface cells,
                // exactly as ReactorZirnox.onNeighborBlockChange does.
                let levels = host.redstone_many(&key.0, &cells);
                if levels.iter().any(|v| v.is_some_and(|n| n > 0)) {
                    z.set_powered(true);
                } else if levels.iter().all(Option::is_some) {
                    z.set_powered(false);
                }
                if z.step() && self.zirnox_meltdown(host, &key, m.facing.as_deref(),m.generation) {
                    m.block = zirnox::DESTROYED.into();
                    **z = Zirnox::of(zirnox::DESTROYED);
                    self.ready.native_saved.remove(&key);
                    self.ready.shown.remove(&key);
                    self.ready.topo_dirty = true;
                }
            }
            self.ready.world.machines.insert(key, m);
        }
    }
    /// `addPotionEffect(new PotionEffect(HbmPotion.radiation.id, duration, amplifier))` on player `id`. The potion
    /// lives in the player's own storage (Onda 1, slice 5c, D3) and shows as the `ntm:radiation` effect; a host
    /// without player storage keeps it in the world's map. An entry of the world's map still waiting for the
    /// storage (a world from before 5c) joins it on the way, once ([`crate::zirnox::absorb`]).
    pub(super) fn irradiate_player(&mut self, host: &mut impl ServerHost, id: &str, duration: u32, amplifier: u8) {
        let legacy = self.player_dose(id);
        if let Some(stored) = host.player_irradiation(id, &legacy) {
            let pending = self.ready.world.zirnox.irradiated.get(id).copied();
            let mut base = stored;
            // Cured earlier in the tick (the milk drunk before the step on the mycelium): the new potion starts afresh.
            if self.potion_cured(host, id, stored.potion) {
                base.potion = (0, 0);
            }
            let (mut desired, taken) = crate::zirnox::absorb(base, pending);
            crate::zirnox::combine(&mut desired.potion, duration, amplifier);
            if host.commit_player_irradiation(id, &legacy, stored, desired) {
                if taken {
                    self.ready.world.zirnox.irradiated.remove(id);
                }
                self.ready.irradiated_now.insert(id.into());
                host.effect(id, crate::radiation::RADIATION_EFFECT, desired.potion.0, desired.potion.1);
                return;
            }
        }
        // No storage, or it changed under the write: the world's map keeps it until the storage takes it.
        self.ready.world.zirnox.irradiate(id, duration, amplifier);
    }
    /// Whether the player is trusted to have the potion's effect: not on the tick the potion was given, nor on their
    /// first tick in the world (the effect reaches them at the end of that tick).
    fn potion_effect_settled(&self, id: &str) -> bool {
        self.blasts.entered.get(id).is_some_and(|t| *t != self.tick) && !self.ready.irradiated_now.contains(id)
    }
    /// Whether something took the potion's effect from player `id` with time left in their storage (`potion`):
    /// milk, the antidote, an iodine pill, a med bag, `/effect clear`.
    fn potion_cured(&self, host: &mut impl ServerHost, id: &str, potion: crate::zirnox::Irradiation) -> bool {
        potion.0 > 0
            && self.potion_effect_settled(id)
            && host.effect_state(id, crate::radiation::RADIATION_EFFECT) == Some(None)
    }
    /// `TileEntityDecon` and the like: `removePotionEffect(HbmPotion.radiation.id)`. A world's entry still waiting
    /// is ended too, and marked as taken in.
    pub(super) fn end_irradiation(&mut self, host: &mut impl ServerHost, id: &str) {
        let legacy = self.player_dose(id);
        let pending = self.ready.world.zirnox.irradiated.get(id).copied();
        if let Some(stored) = host.player_irradiation(id, &legacy) {
            let desired = crate::zirnox::StoredPotion { potion: (0, 0), migrated: pending.unwrap_or(stored.migrated) };
            if desired != stored {
                if !host.commit_player_irradiation(id, &legacy, stored, desired) {
                    return;
                }
                if stored.potion.0 > 0 {
                    host.remove_effect(id, crate::radiation::RADIATION_EFFECT);
                }
            }
        }
        if pending.is_some() {
            self.ready.world.zirnox.irradiated.remove(id);
        }
    }
    /// The radiation potion (`HbmPotion.radiation`): glowing mycelium under the feet gives it, and every tick
    /// (`updatePotionEffects`) it doses its players and counts down. It runs every tick, with or without ZIRNOX
    /// debris or effects in flight. From the player's storage, the effect is a flag: put back every tick with the
    /// time left and one tick to spare (the engine counts its effects down before this step), and gone with time
    /// left (milk, the antidote, an iodine pill, a med bag, `/effect clear`) it ends the potion, as
    /// `removePotionEffect` does in the Java. A stronger or longer effect than the storage's (`/effect give`) is an
    /// `addPotionEffect` of its own. The potion is written before its dose: a refused write gives no dose.
    pub(super) fn zirnox_irradiation(&mut self, host: &mut impl ServerHost) {
        // WasteEarth.onEntityWalking: glowing mycelium gives radiation
        // amplifier 3 for 30 seconds. Source spreading is off by default.
        let walking: Vec<String> = self
            .players
            .iter()
            .filter(|p| !p.creative)
            .filter(|p| {
                let below = [p.pos[0].floor() as i32, (p.pos[1] - 0.05).floor() as i32, p.pos[2].floor() as i32];
                host.get_block(&p.dim, below).is_some_and(|b| blockstate::id_of(&b) == "ntm:waste_mycelium")
            })
            .map(|p| p.id.clone())
            .collect();
        for id in walking {
            self.irradiate_player(host, &id, 600, 3);
        }
        for p in self.players.clone() {
            let legacy = self.player_dose(&p.id);
            let stored = host.player_irradiation(&p.id, &legacy);
            // Read first: a mutable borrow stamps the kept part, which an idle world must not rewrite.
            let pending = self.ready.world.zirnox.irradiated.get(&p.id).copied();
            let (mut desired, taken) = match stored {
                Some(stored) => {
                    let mut base = stored;
                    // Read every tick, acted on once settled: the reading before is what "grew" is measured against.
                    let read = host.effect_state(&p.id, crate::radiation::RADIATION_EFFECT);
                    let before = read.and_then(|now| self.ready.radiation_effect_read.insert(p.id.clone(), now.unwrap_or((0, 0))));
                    let settled = self.potion_effect_settled(&p.id);
                    if stored.potion.0 > 0 && settled && read == Some(None) {
                        base.potion = (0, 0);
                    } else if settled
                        && let (Some(Some((ticks, amplifier))), Some((was, was_level))) = (read, before)
                        && (amplifier > was_level || amplifier == was_level && ticks > was)
                        && (amplifier > base.potion.1 || amplifier == base.potion.1 && ticks > base.potion.0)
                    {
                        // An effect that grew without the storage (`/effect give`): the Java's potion is that effect.
                        crate::zirnox::combine(&mut base.potion, ticks, amplifier);
                    }
                    crate::zirnox::absorb(base, pending)
                }
                None => {
                    let mut potion = (0, 0);
                    if let Some((n, a)) = pending {
                        crate::zirnox::combine(&mut potion, n, a);
                    }
                    (crate::zirnox::StoredPotion { potion, migrated: (0, 0) }, pending.is_some())
                }
            };
            let potion = desired.potion;
            if potion.0 > 0 {
                desired.potion.0 -= 1;
            }
            if desired.potion.0 == 0 {
                desired.potion = (0, 0);
            }
            match stored {
                Some(stored) => {
                    if desired != stored && !host.commit_player_irradiation(&p.id, &legacy, stored, desired) {
                        // Refused: this tick gives no dose and counts nothing; a later tick tries again.
                        continue;
                    }
                    if taken {
                        self.ready.world.zirnox.irradiated.remove(&p.id);
                    }
                    if desired.potion.0 > 0 {
                        host.effect(&p.id, crate::radiation::RADIATION_EFFECT, desired.potion.0 + 1, desired.potion.1);
                    }
                }
                None if desired.potion.0 > 0 => {
                    self.ready.world.zirnox.irradiated.insert(p.id.clone(), desired.potion);
                }
                None => {
                    if taken {
                        self.ready.world.zirnox.irradiated.remove(&p.id);
                    }
                }
            }
            if potion.0 > 0 && !p.creative {
                // `HbmPotion.radiation` doses through `contaminate`, which skips the first 200 ticks of a player
                // (`ContaminationUtil:287`); the effect still counts down and the Geiger still hears it.
                if !self.in_grace(&p.id)
                    && let Some(expected) = host.player_dose(&p.id, &legacy)
                {
                    let mut dose = expected.clone();
                    crate::radiation::add_dose(&mut dose, ((u64::from(potion.1) + 1) as f64 * 50.0 * host.radiation_factor(&p.id)) as u64);
                    if host.commit_player_dose(&p.id, &expected, &dose) {
                        self.ready.world.radiation.insert(p.id.clone(), dose);
                    }
                }
                let rate = self.ready.zirnox_radiation_rate.entry(p.id.clone()).or_default();
                *rate = rate.saturating_add((u32::from(potion.1) + 1) * 1000);
            }
        }
    }
    pub(super) fn zirnox_world_effects(&mut self, host: &mut impl ServerHost) {
        if self.ready.world.machine_effects.iter().any(|b|b.owners.iter().any(|k| self.ready.world.machines.get(k).is_some_and(|m|matches!(m.data,Data::Zirnox(_))))) {return;}

        // Read first: taking stamps the kept part, which an idle world must not rewrite.
        if !self.ready.world.zirnox.blasts.is_empty() {
            let blasts = std::mem::take(&mut self.ready.world.zirnox.blasts);
            for blast in blasts {
                let (dim,at)=blast.location();
                if !host.pile_reachable(dim) || !host.zirnox_explode(dim, at) {
                    self.ready.world.zirnox.blasts.push(blast);
                }
            }
        }
        self.zirnox_waste(host);
        self.zirnox_debris(host);
    }
    fn zirnox_meltdown(
        &mut self,
        host: &mut impl ServerHost,
        key: &Key,
        facing: Option<&str>,
        generation:u64,
    ) -> bool {
        let f = facing.unwrap_or("north");
        // Clear the tall live casing; ruined geometry is only its 5x2x5
        // base. Strict native writes have no item drops or neighbour cascade.
        let dir = registry::facing_dir(f).unwrap_or([0, 0, -1]);
        let mut blocks = Vec::new();
        for y in 0..=4 {
            for x in -2_i32..=2 {
                for z in -2_i32..=2 {
                    if y <= 1 || (x.abs() <= 1 && z.abs() <= 1) || (x.abs() == 2 && z == 0) {
                        let p = container_bridge::world_cell(key.1, dir, [x, y, z]);
                        let state = if p == key.1 {
                            format!("{}[facing={f},honey_level=0]", zirnox::DESTROYED)
                        } else if y <= 1 {
                            "ntm:dummy_zirnox_destroyed".into()
                        } else {
                            "minecraft:air".into()
                        };
                        blocks.push((p, state));
                    }
                }
            }
        }
        if let Err(e) = host.set_blocks(&key.0, &blocks) {
            host.log(&format!("zirnox failure: {e}"));
            return false;
        }
        // ZirnoxDestroyed.onBlockAdded: each new ruined cell can hiss as it
        // appears, before the dedicated reactor explosion event.
        for (pos,state) in &blocks {
            if !matches!(blockstate::id_of(state),"ntm:zirnox_destroyed"|"ntm:dummy_zirnox_destroyed"){continue;}
            let mut random=crate::bedrock::JavaRandom::new(seed(&key.0,*pos,self.ready.world.ticks) as i64);
            if random.next_int(4)==0 {
                host.show(&key.0,&Show::Sound{id:"minecraft:block.fire.ambient",source:"block",at:pos.map(|n|f64::from(n)+0.5),volume:1.+random.next_float(),pitch:random.next_float()*0.7+0.3});
            }
        }
        let at = [
            f64::from(key.1[0]),
            f64::from(key.1[1]) + 3.,
            f64::from(key.1[2]),
        ];
        let origin=CellGuard{dim:key.0.clone(),pos:key.1,generation,state:zirnox::BLOCK.into()};
        self.ready.world.zirnox.blasts.push(zirnox::CommittedBlast::Guarded{origin:origin.clone(),at});
        host.show(
            &key.0,
            &Show::Sound {
                id: "ntm:block.rbmk_explosion",
                source: "block",
                at: [at[0],at[1]-1.,at[2]],
                volume: 10.,
                pitch: 1.,
            },
        );
        let s = seed(&key.0, key.1, self.ready.world.ticks);
        let mut debris = Debris::spawn(&key.0, key.1, s);
        for d in &mut debris{d.origin=Some(origin.clone());}
        for d in &debris {
            if let Some(n) = self.ready.world.zirnox.debris_created.get_mut(usize::from(d.kind)) {
                *n = n.saturating_add(1);
            }
        }
        if host.trace_effects(){
            host.log(&format!("ZIRNOX trace creation {}",serde_json::json!({"tick":self.tick,"world_tick":self.ready.world.ticks,"seed":s,"origin":origin,"debris":debris})));
        }
        self.ready.world.zirnox.debris.extend(debris);
        self.ready.world.zirnox.waste.push(Waste {
            origin:Some(origin),
            dim: key.0.clone(),
            center: key.1,
            cursor: 0,
            seed: s,
        });
        host.log(
            "ZIRNOX meltdown: explosion 12; 72 simulated debris; waste radius 35; radioactive ruin",
        );
        true
    }
    fn zirnox_ruin(&mut self, host: &mut impl ServerHost, key: &Key, z: &mut Zirnox) {
        let s = seed(&key.0, key.1, self.ready.world.ticks);
        if roll(s, 5000) == 0 {
            z.on_fire = false;
        }
        let center = key.1.map(|n| f64::from(n) + 0.5);
        for p in self
            .players
            .clone()
            .into_iter()
            .filter(|p| p.dim == key.0 && !p.creative)
        {
            if (0..3).any(|i| (p.pos[i] - center[i]).abs() > 100.) {
                continue;
            }
            let eye = [p.pos[0], p.pos[1] + 1.62, p.pos[2]];
            let delta = [0, 1, 2].map(|i| eye[i] - center[i]);
            let len = delta.iter().map(|v| v * v).sum::<f64>().sqrt().max(0.001);
            let mut resistance = 0.;
            // Java's i < len includes floor(len) for fractional distances.
            for i in (1_u32..).take_while(|&i| f64::from(i) < len) {
                let at = [0, 1, 2].map(|k| (center[k] + delta[k] * f64::from(i) / len).floor() as i32);
                if let Some(r) = host.block_resistance(&key.0, at) {
                    resistance += f64::from(r.max(0.));
                }
            }
            let rad = if z.on_fire { 500000. } else { 75000. };
            let amount = (rad / resistance.max(1.) / (len * len) * 1000.) as u64;
            // `TileEntityZirnoxDestroyed.radiate:89` doses through `contaminate`: nothing in the first 200 ticks of a
            // player (`ContaminationUtil:287`).
            if !self.in_grace(&p.id) {
                self.expose_player(host,&p.id,amount);
            }
            let rate = self
                .ready
                .zirnox_radiation_rate
                .entry(p.id.clone())
                .or_default();
            *rate = rate.saturating_add(amount.saturating_mul(20).min(u64::from(u32::MAX)) as u32);
            if z.on_fire && len < 5. {
                host.hurt(&p.id, 2., "minecraft:on_fire");
            }
        }
        if z.on_fire && self.tick.is_multiple_of(50) {
            particle(
                host,
                &key.0,
                "minecraft:flame",
                [center[0], center[1] + 1.25, center[2]],
                12,
            );
            let mut sound_random=crate::bedrock::JavaRandom::new(seed(&key.0,key.1,self.ready.world.ticks) as i64);
            host.show(
                &key.0,
                &Show::Sound {
                    id: "minecraft:block.fire.ambient",
                    source: "block",
                    at: center,
                    volume: 1.+sound_random.next_float(),
                    pitch: sound_random.next_float()*0.7+0.3,
                },
            );
        }
        // Source schedules every 100..119 ticks on all ruined cells.
        if self.tick.is_multiple_of(110) {
            for x in -2..=2 {
                for y in 0..=1 {
                    for zz in -2..=2 {
                        let above = [key.1[0] + x, key.1[1] + y + 1, key.1[2] + zz];
                        let Some(b) = host.get_block(&key.0, above) else {
                            continue;
                        };
                        let r = seed(&key.0, above, self.ready.world.ticks);
                        if matches!(blockstate::id_of(&b), "ntm:foam_layer" | "ntm:block_foam")
                            && roll(r, 25) == 0
                        {
                            z.on_fire = false;
                        }
                        if blockstate::id_of(&b) == "minecraft:air"
                            && (roll(r, 10) == 0 || roll(r ^ 1, 10) == 0)
                        {
                            // gas_meltdown is absent from this content catalog; retain
                            // emission events and show its plume through native smoke.
                            z.gas_emissions = z.gas_emissions.saturating_add(1);
                            particle(
                                host,
                                &key.0,
                                "minecraft:large_smoke",
                                above.map(|v| f64::from(v) + 0.5),
                                2,
                            );
                        }
                    }
                }
            }
        }
    }
    fn zirnox_waste(&mut self, host: &mut impl ServerHost) {
        if self.ready.world.zirnox.waste.is_empty() {
            return;
        }
        let mut jobs = std::mem::take(&mut self.ready.world.zirnox.waste);
        for job in &mut jobs {
            if !host.pile_reachable(&job.dim) {
                continue;
            }
            if job
                .center
                .iter()
                .any(|n| n.checked_add(35).is_none() || n.checked_sub(35).is_none())
            {
                job.cursor = 70 * 70 * 70;
                continue;
            }
            let start = job.cursor;
            let end = job.cursor.saturating_add(4096).min(70 * 70 * 70);
            let mut changes = Vec::new();
            for i in start..end {
                let local = [
                    (i / 4900) as i32 - 35,
                    ((i / 70) % 70) as i32 - 35,
                    (i % 70) as i32 - 35,
                ];
                let r = crate::hash::mix(job.seed ^ u64::from(i));
                if local.iter().map(|n| n * n).sum::<i32>() >= 612 + (r % 122) as i32 {
                    continue;
                }
                let p = [0, 1, 2].map(|k| job.center[k] + local[k]);
                let Some(b) = host.get_block(&job.dim, p) else {
                    continue;
                };
                if let Some(next) = waste_block(&b, r) {
                    changes.push((p, next));
                }
            }
            if changes.is_empty() || host.set_blocks(&job.dim, &changes).is_ok() {
                job.cursor = end;
            } else {
                host.log("zirnox waste: block batch refused, retrying");
            }
        }
        jobs.retain(|j| j.cursor < 70 * 70 * 70);
        self.ready.world.zirnox.waste = jobs;
    }
    fn zirnox_debris(&mut self, host: &mut impl ServerHost) {
        if self.ready.world.zirnox.debris.is_empty() {
            return;
        }
        let mut debris = std::mem::take(&mut self.ready.world.zirnox.debris);
        for d in &mut debris {
            if !host.pile_reachable(&d.dim) {
                continue;
            }
            if !d
                .pos
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 30_000_000.)
                || !d.velocity.iter().all(|v| v.is_finite() && v.abs() < 100.)
            {
                d.removed = true;
                continue;
            }
            let cell = d.pos.map(|n| n.floor() as i32);
            if host.get_block(&d.dim, cell).is_none() {
                continue;
            }
            if !d.pos.iter().chain(&d.velocity).all(|v| v.is_finite()) {
                d.removed = true;
                continue;
            }
            d.velocity[1] -= 0.04;
            let next = [0, 1, 2].map(|i| d.pos[i] + d.velocity[i]);
            let cell = next.map(|n| n.floor() as i32);
            let solid = host.get_block(&d.dim, cell).is_some_and(|b| {
                !matches!(
                    crate::explosion::crater::classify(&b).kind,
                    crate::explosion::crater::Kind::Air | crate::explosion::crater::Kind::Liquid
                )
            });
            if solid {
                if matches!(d.kind, 4 | 5) && d.velocity[1] > 0. {
                    let mut changes = Vec::new();
                    for x in -1_i32..=1 {
                        for y in -1_i32..=1 {
                            for z in -1_i32..=1 {
                                let n = x.abs() + y.abs() + z.abs();
                                if n <= 1
                                    || roll(
                                        seed(
                                            &d.dim,
                                            [cell[0] + x, cell[1] + y, cell[2] + z],
                                            u64::from(d.age),
                                        ),
                                        n as u64,
                                    ) == 0
                                {
                                    let p = [cell[0] + x, cell[1] + y, cell[2] + z];
                                    if host.get_block(&d.dim, p).is_some() {
                                        changes.push((p, "minecraft:air".into()));
                                    }
                                }
                            }
                        }
                    }
                    let removed=host.trace_effects().then(||changes.iter().filter_map(|(p,_)|host.get_block(&d.dim,*p).filter(|b|blockstate::id_of(b)!="minecraft:air").map(|b|(*p,b))).collect::<Vec<_>>());
                    if host.set_blocks(&d.dim, &changes).is_ok() {
                        if let Some(removed)=removed{host.log(&format!("ZIRNOX trace debris impact {}",serde_json::json!({"tick":self.tick,"world_tick":self.ready.world.ticks,"kind":d.kind,"age":d.age,"pos":d.pos,"velocity":d.velocity,"removed":removed})));}
                        d.removed = true;
                        continue;
                    }
                }
                d.velocity[0] *= 0.85;
                d.velocity[2] *= 0.85;
                d.velocity[1] *= -0.5;
            } else {
                d.pos = next;
            }
            if matches!(d.kind, 1 | 3) {
                let level = if d.kind == 1 { 7 } else { 4 };
                let near: Vec<String> = self
                    .players
                    .iter()
                    .filter(|p| p.dim == d.dim && !p.creative && (0..3).all(|k| (p.pos[k] - d.pos[k]).abs() <= 2.5))
                    .map(|p| p.id.clone())
                    .collect();
                for id in near {
                    self.irradiate_player(host, &id, 1200, level);
                }
            }
            if self.tick.is_multiple_of(5) {
                particle(
                    host,
                    &d.dim,
                    if matches!(d.kind, 1 | 3) {
                        "minecraft:ash"
                    } else {
                        "minecraft:smoke"
                    },
                    d.pos,
                    1,
                );
            }
            d.age = d.age.saturating_add(1);
        }
        debris.retain(|d| !d.removed && (d.permanent || d.age < d.lifetime));
        self.ready.world.zirnox.debris = debris;
    }
}

/// ExplosionNukeGeneric.wasteDest, rather than nuclear fallout rain.
fn waste_block(block: &str, r: u64) -> Option<String> {
    let b = blockstate::id_of(block);
    let out = match b {
        "minecraft:iron_door"
        | "minecraft:brown_mushroom_block"
        | "minecraft:red_mushroom_block" => "minecraft:air",
        "minecraft:mushroom_stem" => "ntm:waste_log",
        "minecraft:grass_block" => "ntm:waste_earth",
        "minecraft:mycelium" => "ntm:waste_mycelium",
        "minecraft:sand" if r % 20 == 1 => "ntm:waste_trinitite",
        "minecraft:red_sand" if r % 20 == 1 => "ntm:waste_trinitite_red",
        "minecraft:clay" => "minecraft:terracotta",
        "minecraft:mossy_cobblestone" => "minecraft:coal_ore",
        "minecraft:coal_ore" if matches!(r % 10, 1..=3) => "minecraft:diamond_ore",
        "minecraft:coal_ore" if r % 10 == 9 => "minecraft:emerald_ore",
        b if b.starts_with("minecraft:") && b.ends_with("_door") => "minecraft:air",
        "ntm:ore_uranium" if r % 250 == 1 => "ntm:ore_schrabidium",
        "ntm:ore_nether_uranium" if r % 250 == 1 => "ntm:ore_nether_schrabidium",
        "ntm:ore_gneiss_uranium" if r % 250 == 1 => "ntm:ore_gneiss_schrabidium",
        "ntm:ore_uranium" => "ntm:ore_uranium_scorched",
        "ntm:ore_nether_uranium" => "ntm:ore_nether_uranium_scorched",
        "ntm:ore_gneiss_uranium" => "ntm:ore_gneiss_uranium_scorched",
        _ => waste_wood(b)?,
    };
    Some(out.into())
}

/// wasteDest's opaque wooden blocks, not the bomb crater's broader burnable
/// category (which also contains fences, gates and inventory-bearing chests).
fn waste_wood(block: &str) -> Option<&'static str> {
    let name = block.strip_prefix("minecraft:")?;
    let plain = name.strip_prefix("stripped_").unwrap_or(name);
    let species = ["oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "pale_oak", "crimson", "warped", "bamboo"];
    for wood in species {
        if ["_log", "_wood", "_stem", "_hyphae"].iter().any(|suffix| plain.strip_suffix(suffix) == Some(wood)) {
            return Some("ntm:waste_log");
        }
        if name.strip_suffix("_planks") == Some(wood) {
            return Some("ntm:waste_planks");
        }
    }
    matches!(name, "bamboo_block" | "stripped_bamboo_block" | "bamboo_mosaic" | "bookshelf" | "chiseled_bookshelf" | "crafting_table" | "note_block" | "jukebox" | "loom" | "cartography_table" | "fletching_table" | "smithing_table" | "barrel" | "beehive" | "bee_nest").then_some("ntm:waste_planks")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        fluidbox::FluidStore,
        kv::MemKv,
        server::container_bridge::{Slots, Stack},
    };
    const DIM: &str = "minecraft:overworld";
    const AT: Pos = [0, 64, 0];
    #[derive(Default)]
    struct Host {
        kv: MemKv,
        blocks: BTreeMap<Key, String>,
        slots: BTreeMap<Key, Slots>,
        saved: BTreeMap<Key, String>,
        data: BTreeMap<Key, Vec<i32>>,
        signal: BTreeSet<Key>,
        reads: Vec<Key>,
        frozen: BTreeSet<String>,
        refuse_slots: bool,
        refuse_save: bool,
        save_attempts: usize,
        refuse_data: bool,
        refuse_blocks: bool,
        refuse_blast: bool,
        blasts: Vec<(String, [f64; 3])>,
        buttons: Vec<(String, String, Pos, u32)>,
        shows: Vec<String>,
        hurt: Vec<(String, f32)>,
        /// The radiation deaths (`ntm:radiation`), apart from the ruin's own damage.
        radiation_deaths: Vec<String>,
        /// Every dose written through the host, in order, as (player, the dose read, the dose written).
        doses: Vec<(String, u64, u64)>,
        /// The player's own dose storage (the engine's player attachment): it wins over the world's mirror and
        /// refuses a write that expected another dose.
        attached: BTreeMap<String, u64>,
        /// The armor's share of the radiation that gets through (`ContaminationUtil.calculateRadiationMod`).
        factor: Option<f64>,
        /// The radiation potion in the player's storage (`Some` map: the host has one).
        irradiation: Option<BTreeMap<String, crate::zirnox::StoredPotion>>,
        refuse_irradiation: bool,
        /// The status effects the players have: (player, effect) → (ticks, amplifier).
        effects: BTreeMap<(String, String), (u32, u8)>,
        /// Effects given this tick: the engine queues them and applies them at the end of the tick.
        queued: Vec<((String, String), (u32, u8))>,
        carried: BTreeMap<String, Vec<Stack>>,
        shields: BTreeMap<Key, f32>,
    }
    impl Kv for Host {
        fn get(&mut self, k: &str) -> Option<Vec<u8>> {
            self.kv.get(k)
        }
        fn set(&mut self, k: &str, v: &[u8]) -> Result<(), String> {
            self.kv.set(k, v)
        }
        fn delete(&mut self, k: &str) {
            self.kv.delete(k)
        }
    }
    impl ServerHost for Host {
        fn player_dose(&mut self, id: &str, legacy: &crate::radiation::Dose) -> Option<crate::radiation::Dose> {
            Some(self.attached.get(id).map_or_else(|| legacy.clone(), |&millirad| crate::radiation::Dose { millirad }))
        }
        fn commit_player_dose(&mut self, id: &str, expected: &crate::radiation::Dose, desired: &crate::radiation::Dose) -> bool {
            if self.attached.get(id).is_some_and(|&d| d != expected.millirad) {
                return false;
            }
            self.doses.push((id.into(), expected.millirad, desired.millirad));
            self.attached.insert(id.into(), desired.millirad);
            true
        }
        fn radiation_factor(&mut self, _: &str) -> f64 {
            self.factor.unwrap_or(1.0)
        }
        fn player_irradiation(&mut self, id: &str, _: &crate::radiation::Dose) -> Option<crate::zirnox::StoredPotion> {
            self.irradiation.as_ref().map(|m| m.get(id).copied().unwrap_or_default())
        }
        fn commit_player_irradiation(&mut self, id: &str, _: &crate::radiation::Dose, expected: crate::zirnox::StoredPotion, desired: crate::zirnox::StoredPotion) -> bool {
            let refuse = self.refuse_irradiation;
            let Some(m) = self.irradiation.as_mut() else { return false };
            if refuse || m.get(id).copied().unwrap_or_default() != expected {
                return false;
            }
            m.insert(id.into(), desired);
            true
        }
        fn effect_state(&mut self, id: &str, effect: &str) -> Option<Option<(u32, u8)>> {
            Some(self.effects.get(&(id.into(), effect.into())).copied())
        }
        fn effect(&mut self, id: &str, effect: &str, ticks: u32, amplifier: u8) {
            self.queued.push(((id.into(), effect.into()), (ticks, amplifier)));
        }
        fn remove_effect(&mut self, id: &str, effect: &str) {
            self.effects.remove(&(id.into(), effect.into()));
        }
        fn commit_structural(&mut self,cells:&[CellWrite],drops:&[RetainedDrop],owners:&[(String,Pos)])->bool{
            if !drops.is_empty(){return false;}
            if cells.iter().any(|c|self.cell_snapshot(&c.guard.dim,c.guard.pos).as_ref()!=Some(&c.expected)){return false;}
            let dims:BTreeSet<_>=cells.iter().map(|c|c.guard.dim.clone()).collect();
            if dims.len()>1{return false;}
            let Some(dim)=dims.first()else{return true};
            let blocks:Vec<_>=cells.iter().map(|c|(c.guard.pos,c.desired.clone())).collect();
            self.set_machine_blocks(dim,&blocks,owners).is_ok()
        }

        fn commit_machine(&mut self,c:&MachineCommit)->bool{self.commit_machines(std::slice::from_ref(c))}
        fn commit_machines(&mut self,cs:&[MachineCommit])->bool{
            self.save_attempts+=1;
            if self.refuse_slots || self.refuse_data || self.refuse_save || cs.iter().any(|c|self.frozen.contains(&c.dim)||self.slots.get(&(c.dim.clone(),c.pos)).cloned().unwrap_or_default()!=c.expected){return false;}
            for c in cs {let k=(c.dim.clone(),c.pos);self.slots.insert(k.clone(),c.desired.clone());self.data.insert(k.clone(),c.data.clone());self.saved.insert(k,c.json.clone());}true
        }

        fn get_block(&mut self, d: &str, p: Pos) -> Option<String> {
            (!self.frozen.contains(d)).then(|| {
                self.blocks
                    .get(&(d.into(), p))
                    .cloned()
                    .unwrap_or("minecraft:air".into())
            })
        }
        fn set_blocks(&mut self, d: &str, b: &[(Pos, String)]) -> Result<(), String> {
            if self.refuse_blocks {
                return Err("refused".into());
            }
            for (p, b) in b {
                let key=(d.to_string(),*p);
                if self.blocks.get(&key).is_some_and(|old|blockstate::id_of(old)!=blockstate::id_of(b)) && self.slots.contains_key(&key) {
                    // The native engine recreates the BE on a role/id change.
                    self.saved.remove(&key);self.data.remove(&key);
                    self.slots.insert(key.clone(),vec![None;crate::native::block_slots(blockstate::id_of(b)).len()]);
                }
                self.blocks.insert(key, b.clone());
            }
            Ok(())
        }
        fn tell(&mut self, _: &str, _: &str) {}
        fn log(&mut self, _: &str) {}
        fn pile_reachable(&mut self, d: &str) -> bool {
            !self.frozen.contains(d)
        }
        fn machine_slots(&mut self, d: &str, p: Pos) -> Option<Slots> {
            if self.frozen.contains(d) {
                None
            } else {
                self.slots.get(&(d.into(), p)).cloned()
            }
        }
        fn machine_buttons(&mut self) -> Vec<(String, String, Pos, u32)> {
            std::mem::take(&mut self.buttons)
        }
        fn machine_data(&mut self, d: &str, p: Pos) -> Option<Vec<i32>> {
            self.data.get(&(d.into(), p)).cloned()
        }
        fn redstone_many(&mut self, d: &str, c: &[Pos]) -> Vec<Option<u8>> {
            c.iter()
                .map(|p| {
                    let k = (d.into(), *p);
                    self.reads.push(k.clone());
                    Some(if self.signal.contains(&k) { 15 } else { 0 })
                })
                .collect()
        }
        fn zirnox_explode(&mut self, d: &str, p: [f64; 3]) -> bool {
            if self.refuse_blast {
                false
            } else {
                self.blasts.push((d.into(), p));
                true
            }
        }
        fn show(&mut self, d: &str, _: &Show) {
            self.shows.push(d.into());
        }
        fn inventory_stacks(&mut self, id: &str) -> Option<Vec<Stack>> {
            self.carried.get(id).cloned()
        }
        fn hurt(&mut self, id: &str, n: f32, kind: &str) {
            if kind == crate::radiation::RADIATION_DAMAGE {
                self.radiation_deaths.push(id.into());
            } else {
                self.hurt.push((id.into(), n));
            }
        }
        fn block_resistance(&mut self, dim: &str, at: Pos) -> Option<f32> {
            Some(self.shields.get(&(dim.into(), at)).copied().unwrap_or(0.))
        }
    }
    fn key() -> Key {
        (DIM.into(), AT)
    }
    fn install(h: &mut Host, s: &mut Server, dim: &str, p: Pos) {
        let block = format!("{}[facing=north,honey_level=0]", zirnox::BLOCK);
        h.blocks.insert((dim.into(), p), block.clone());
        h.slots.insert((dim.into(), p), vec![None; 28]);
        s.native_block_changed(dim, p, "minecraft:air", &block);
    }
    fn z(s: &Server) -> &Zirnox {
        let Data::Zirnox(z) = &s.ready.world.machines[&key()].data else {
            panic!()
        };
        z
    }
    fn zm(s: &mut Server) -> &mut Zirnox {
        let Data::Zirnox(z) = &mut s.ready.world.machines.get_mut(&key()).unwrap().data else {
            panic!()
        };
        z
    }
    fn prepare() -> (Host, Server) {
        let mut h = Host::default();
        let mut s = Server::native(&mut h, 0);
        install(&mut h, &mut s, DIM, AT);
        h.signal.insert(key());
        h.slots.get_mut(&key()).unwrap()[0] = Some(Stack::new(&zirnox::table().fuels[0].item, 1));
        s.native_tick(&mut h, 1);
        (h, s)
    }
    #[test]
    fn runs_from_real_slots_containers_menu_buttons_and_persists_strictly_after_reopening() {
        let (mut h, mut s) = prepare();
        h.slots.get_mut(&key()).unwrap()[24] =
            Some(Stack::new("ntm:fluid_barrel_full__carbondioxide", 1));
        h.slots.get_mut(&key()).unwrap()[25] = Some(Stack::new("ntm:fluid_barrel_full__water", 1));
        s.native_tick(&mut h, 2);
        assert_eq!((z(&s).co2.mb, z(&s).water.mb), (16000, 16000));
        assert!(z(&s).heat > 0);
        let life = zirnox::life(h.slots[&key()][0].as_ref().unwrap());
        assert_eq!(life, 2);
        let saved = h.saved[&key()].clone();
        let mut reopened = Server::native(&mut h, 100);
        assert!(reopened.native_adopt(DIM, AT, &saved));
        reopened.native_tick(&mut h, 101);
        assert!(
            zirnox::life(h.slots[&key()][0].as_ref().unwrap()) > life,
            "must actually run after adoption"
        );
        assert!(z(&reopened).heat > z(&s).heat);
        h.signal.clear();
        reopened.native_tick(&mut h, 102);
        assert!(!z(&reopened).on);
        let life = zirnox::life(h.slots[&key()][0].as_ref().unwrap());
        reopened.native_tick(&mut h, 103);
        assert_eq!(zirnox::life(h.slots[&key()][0].as_ref().unwrap()), life);
        h.buttons.push(("p".into(), DIM.into(), AT, 0));
        reopened.native_tick(&mut h, 104);
        assert!(z(&reopened).on);
        h.buttons.push(("p".into(), DIM.into(), AT, 1));
        reopened.native_tick(&mut h, 105);
        assert_eq!(z(&reopened).co2.mb, 15000);
    }
    #[test]
    fn reads_own_surface_only_and_runs_in_two_open_dimensions_freezing_closed_ones() {
        let (mut h, mut s) = prepare();
        h.signal.clear();
        let outside = [3, 64, 0];
        h.signal.insert((DIM.into(), outside));
        let other = "minecraft:the_nether";
        let p = [40, 64, 0];
        install(&mut h, &mut s, other, p);
        h.signal.insert((other.into(), p));
        h.slots.get_mut(&(other.into(), p)).unwrap()[0] =
            Some(Stack::new(&zirnox::table().fuels[0].item, 1));
        h.reads.clear();
        s.native_tick(&mut h, 2);
        assert!(!z(&s).on);
        assert!(!h.reads.contains(&(DIM.into(), outside)));
        assert_eq!(
            zirnox::life(h.slots[&(other.into(), p)][0].as_ref().unwrap()),
            1
        );
        h.frozen.insert(other.into());
        s.native_tick(&mut h, 3);
        assert_eq!(
            zirnox::life(h.slots[&(other.into(), p)][0].as_ref().unwrap()),
            1
        );
        h.frozen.clear();
        s.native_tick(&mut h, 4);
        assert!(zirnox::life(h.slots[&(other.into(), p)][0].as_ref().unwrap()) > 1);
    }
    #[test]
    fn redstone_on_a_box_corner_outside_the_casing_starts_the_reactor() {
        let (mut h, mut s) = prepare();
        h.signal.clear();
        s.native_tick(&mut h, 2);
        assert!(!z(&s).on);
        let corner = [AT[0]+2, AT[1]+3, AT[2]+2];
        h.blocks.insert((DIM.into(), corner), "minecraft:stone".into());
        h.signal.insert((DIM.into(), corner));
        h.reads.clear();
        s.native_tick(&mut h, 3);
        assert!(h.reads.contains(&(DIM.into(), corner)));
        assert!(z(&s).on && z(&s).powered);
    }
    #[test]
    fn failed_slot_data_and_state_writes_are_retried_without_resetting_life() {
        let (mut h, mut s) = prepare();
        let before = zirnox::life(h.slots[&key()][0].as_ref().unwrap());
        h.refuse_slots = true;
        h.refuse_data = true;
        let saved = h.saved[&key()].clone();
        for t in 2..5 {
            s.native_tick(&mut h, t);
        }
        assert_eq!(
            h.saved[&key()],
            saved,
            "state cannot be saved ahead of its slots"
        );
        assert_eq!(zirnox::life(z(&s).slots[0].as_ref().unwrap()), before, "refused steps must not age or consume rods");
        h.refuse_slots = false;
        h.refuse_data = false;
        h.refuse_save = true;
        s.native_tick(&mut h, 5);
        assert_eq!(
            zirnox::life(h.slots[&key()][0].as_ref().unwrap()),
            before
        );
        assert_eq!(h.saved[&key()], saved);
        h.refuse_save = false;
        s.native_tick(&mut h, 6);
        assert_ne!(h.saved[&key()], saved);
        assert_eq!(
            zirnox::life(h.slots[&key()][0].as_ref().unwrap()),
            before + 1
        );
    }
    #[test]
    fn removing_a_rod_during_a_refused_life_write_does_not_recreate_it() {
        let (mut h, mut s) = prepare();
        assert_eq!(zirnox::life(h.slots[&key()][0].as_ref().unwrap()), 1);
        h.refuse_slots = true;
        s.native_tick(&mut h, 2);
        let removed = h.slots.get_mut(&key()).unwrap()[0].take().unwrap();
        assert_eq!(zirnox::life(&removed), 1);
        h.refuse_slots = false;
        s.native_tick(&mut h, 3);
        assert!(h.slots[&key()][0].is_none());
        assert!(z(&s).slots[0].is_none());

    }
    #[test]
    fn refused_screen_b_returning_to_confirmed_a_unblocks_stable_saves() {
        let mut h = Host { refuse_save: true, ..Default::default() };
        let mut s = Server::native(&mut h, 0);
        install(&mut h, &mut s, DIM, AT);
        s.native_tick(&mut h, 1); // A: off, cold, no rods.
        assert_eq!(h.save_attempts, 1);
        h.refuse_data = true;
        h.buttons.push(("p".into(), DIM.into(), AT, 0));
        s.native_tick(&mut h, 2); // B: on; screen write refused.
        assert!(!z(&s).on, "refused button step rolls back");
        h.refuse_data = false;
        h.refuse_save = false;
        h.buttons.push(("p".into(), DIM.into(), AT, 0));
        for t in 3..20 { s.native_tick(&mut h, t); }
        assert!(!z(&s).on);

        assert!(h.save_attempts >= 3, "healthy ticks retry and accept the JSON save");
        let back: crate::machine::Machine = serde_json::from_str(&h.saved[&key()]).unwrap();
        let Data::Zirnox(z) = back.data else { panic!() };
        assert!(!z.on);
    }
    #[test]
    fn a_refused_button_is_retried_once_with_the_machine_step() {
        let mut h=Host::default();let mut s=Server::native(&mut h,0);
        install(&mut h,&mut s,DIM,AT);s.native_tick(&mut h,1);
        h.buttons.push(("p".into(),DIM.into(),AT,0));h.refuse_data=true;
        s.native_tick(&mut h,2);assert!(!z(&s).on);
        h.refuse_data=false;s.native_tick(&mut h,3);assert!(z(&s).on);
        for t in 4..10{s.native_tick(&mut h,t);assert!(z(&s).on,"confirmed button must not toggle twice");}
    }
    #[test]
    fn life_conversion_breeding_and_real_outward_steam_network_work() {
        let (mut h, mut s) = prepare();
        let f = &zirnox::table().fuels[9];
        let mut rod = Stack::new(&f.item, 1);
        rod.custom = Some("{life:20000}".into());
        h.slots.get_mut(&key()).unwrap()[1] = Some(rod);
        zm(&mut s).heat = 50000;
        zm(&mut s).co2.mb = 14000;
        zm(&mut s).water.mb = 500;
        let barrel = [3, 65, 0];
        let bk = (DIM.into(), barrel);
        h.blocks
            .insert(bk.clone(), "ntm:barrel_steel[facing=north]".into());
        h.slots.insert(bk.clone(), vec![None; 6]);
        s.native_block_changed(
            DIM,
            barrel,
            "minecraft:air",
            "ntm:barrel_steel[facing=north]",
        );
        for t in 2..20 {
            s.native_tick(&mut h, t);
        }
        assert_eq!(h.slots[&key()][1], Some(Stack::new(&f.output, 1)));
        let Data::FluidStore(FluidStore { tank, .. }) = &s.ready.world.machines[&bk].data else {
            panic!()
        };
        assert_eq!(tank.fluid, "superhotsteam");
        assert!(tank.mb > 0);
        assert!(z(&s).water.mb < 500);
        assert_eq!(z(&s).co2.mb, 14000);
    }
    #[test]
    fn failure_clears_inventory_keeps_ruin_and_retries_explosion_in_original_dimension() {
        let (mut h, mut s) = prepare();
        zm(&mut s).heat = 100001;
        h.refuse_blocks = true;
        s.native_tick(&mut h, 2);
        assert!(z(&s).destroyed, "inventory/state committed before deferred casing replacement");
        assert!(h.slots[&key()].iter().all(Option::is_none));
        assert!(!s.ready.world.machine_effects.is_empty());
        assert!(h.blasts.is_empty());
        h.refuse_blocks = false;
        h.refuse_blast = true;
        s.native_tick(&mut h, 3);
        assert!(z(&s).destroyed);
        assert!(h.slots[&key()].iter().all(Option::is_none));
        assert_eq!(s.ready.world.zirnox.debris.len(), 72);
        assert_eq!(s.ready.world.zirnox.debris_created, [20, 10, 10, 10, 20, 2]);
        assert_eq!(s.ready.world.zirnox.blasts.len(), 1);
        let kept:Machine=serde_json::from_str(&h.saved[&key()]).unwrap();
        assert_eq!(kept.block,zirnox::DESTROYED,"the recreated ruin BE receives confirmed state in the failure tick");
        // Native notifications from our replacement arrive next tick.
        s.native_block_changed(DIM, AT, zirnox::BLOCK, zirnox::DESTROYED);
        h.refuse_blast = false;
        s.native_tick(&mut h, 4);
        assert!(z(&s).destroyed);
        assert_eq!(h.blasts, vec![(DIM.into(), [0., 67., 0.])]);
        assert!(s.ready.world.zirnox.blasts.is_empty());
        let back = WorldState::load(&mut h);
        assert_eq!(back.zirnox.debris.len(), 72);
        assert_eq!(back.zirnox.debris_created, [20, 10, 10, 10, 20, 2]);
        assert!(!back.zirnox.waste.is_empty());
    }
    #[test]
    fn ruined_reactor_irradiates_only_its_dimension_and_fire_can_be_extinguished() {
        let (mut h, mut s) = prepare();
        zm(&mut s).destroyed = true;
        zm(&mut s).on_fire = true;
        s.ready.world.machines.get_mut(&key()).unwrap().block = zirnox::DESTROYED.into();
        h.blocks.insert(key(), zirnox::DESTROYED.into());
        s.set_players(vec![
            PlayerInfo {
                id: "same".into(),
                name: "same".into(),
                dim: DIM.into(),
                pos: [2., 64., 0.],
                creative: false,
            },
            PlayerInfo {
                id: "other".into(),
                name: "other".into(),
                dim: "minecraft:the_nether".into(),
                pos: [2., 64., 0.],
                creative: false,
            },
        ]);
        s.blasts.entered.insert("same".into(), 0);s.blasts.entered.insert("other".into(), 0);
        s.native_tick(&mut h, 202);
        // The ruin wrote its dose, over 1000 RAD, to the player's storage; the death read it and wrote 0.
        let same: Vec<_> = h.doses.iter().filter(|(id, ..)| id == "same").map(|d| (d.1, d.2)).collect();
        assert!(matches!(same[..], [(0, given), (read, 0)] if given >= 1_000_000 && read == given), "{:?}", h.doses);
        assert!(!h.doses.iter().any(|(id, ..)| id == "other"));
        assert!(!s.ready.world.radiation.contains_key("other"));
        assert_eq!(h.hurt.len(), 1);
        // The ruin, a tile entity, took "same" over 1000 RAD: radiation kills them in that same tick, the player's
        // update coming after the tile entities (EntityEffectHandler, slice 5b), and the tick ends at 0.
        assert_eq!(h.radiation_deaths, ["same"]);
        assert_eq!(s.ready.world.radiation["same"].millirad, 0);
        zm(&mut s).on_fire = false;
        s.native_tick(&mut h, 203);
        assert_eq!(h.hurt.len(), 1);
        assert!(!z(&s).on_fire);
    }
    #[test]
    fn player_inventory_nbt_drives_the_real_radiation_rate() {
        let (mut h, mut s) = prepare();
        let mut rod = Stack::new("ntm:rod_zirnox__uranium_fuel", 1);
        rod.custom = Some("{life:6250}".into());
        h.carried.insert("player".into(), vec![rod]);
        s.set_players(vec![PlayerInfo {
            id: "player".into(),
            name: "player".into(),
            dim: DIM.into(),
            pos: [0., 64., 0.],
            creative: true,
        }]);
        s.native_tick(&mut h, 20);
        assert_eq!(s.radiation_shown["player"].1, 37875);
        h.carried.get_mut("player").unwrap()[0].custom = Some("{life:200000}".into());
        s.native_tick(&mut h, 40);
        assert_eq!(s.radiation_shown["player"].1, 150000);
    }
    #[test]
    fn ruin_shielding_includes_the_last_fractional_sample_but_not_the_eye() {
        for (eye_x, wall_x, expected) in [(21., 20, 11897), (21., 19, 11897), (20.5, 20, 1250000)] {
            let (mut h, mut s) = prepare();
            zm(&mut s).destroyed = true;
            zm(&mut s).on_fire = true;
            s.ready.world.machines.get_mut(&key()).unwrap().block = zirnox::DESTROYED.into();
            h.blocks.insert(key(), zirnox::DESTROYED.into());
            h.shields.insert((DIM.into(), [wall_x, 64, 0]), 100.);
            s.set_players(vec![PlayerInfo { id: "p".into(), name: "p".into(), dim: DIM.into(), pos: [eye_x, 62.88, 0.5], creative: false }]);
            s.blasts.entered.insert("p".into(), 0);
            s.native_tick(&mut h, 202);
            // Below 1000 RAD the tick ends at the ruin's dose; from 1000 the player's update, after the ruin, kills in the
            // same tick: its write read the ruin's dose and left 0.
            // The ruin's dose reaches the player's own storage (through the machine step's host) once per tick.
            if expected < 1_000_000 {
                assert_eq!(s.ready.world.radiation["p"].millirad, expected, "eye={eye_x}, wall={wall_x}");
                assert_eq!(h.doses, [("p".to_string(), 0, expected)], "eye={eye_x}, wall={wall_x}");
                assert_eq!(h.attached["p"], expected);
                assert!(h.radiation_deaths.is_empty());
            } else {
                assert_eq!(h.doses, [("p".to_string(), 0, expected), ("p".to_string(), expected, 0)], "eye={eye_x}, wall={wall_x}");
                assert_eq!(h.attached["p"], 0);
                assert_eq!(h.radiation_deaths, ["p"]);
                assert_eq!(s.ready.world.radiation["p"].millirad, 0);
            }
        }
    }
    /// The ruin's dose is given inside the machine step: it reaches the player's own storage, not only the world's
    /// mirror, so a reopened world (an empty mirror) still has it, and the armor's factor applies to it.
    #[test]
    fn the_ruins_dose_reaches_the_players_storage_and_survives_reopening() {
        for factor in [None, Some(0.5)] {
            let (mut h, mut s) = prepare();
            h.factor = factor;
            zm(&mut s).destroyed = true;
            zm(&mut s).on_fire = false;
            s.ready.world.machines.get_mut(&key()).unwrap().block = zirnox::DESTROYED.into();
            h.blocks.insert(key(), zirnox::DESTROYED.into());
            s.set_players(vec![PlayerInfo { id: "p".into(), name: "p".into(), dim: DIM.into(), pos: [10.5, 62.88, 0.5], creative: false }]);
            s.blasts.entered.insert("p".into(), 0);
            h.attached.insert("p".into(), 3_000);
            s.native_tick(&mut h, 202);
            let given = h.attached["p"] - 3_000;
            assert!(given > 0, "{:?}", h.doses);
            assert_eq!(h.doses.len(), 1);
            assert_eq!(s.ready.world.radiation["p"].millirad, h.attached["p"]);
            if let Some(f) = factor {
                let (mut h1, mut s1) = prepare();
                zm(&mut s1).destroyed = true;
                zm(&mut s1).on_fire = false;
                s1.ready.world.machines.get_mut(&key()).unwrap().block = zirnox::DESTROYED.into();
                h1.blocks.insert(key(), zirnox::DESTROYED.into());
                s1.set_players(vec![PlayerInfo { id: "p".into(), name: "p".into(), dim: DIM.into(), pos: [10.5, 62.88, 0.5], creative: false }]);
                s1.blasts.entered.insert("p".into(), 0);
                s1.native_tick(&mut h1, 202);
                assert_eq!(given, (h1.attached["p"] as f64 * f) as u64, "the armor's factor applies to the ruin");
            }
            let mut reopened = Server::native(&mut h, 3);
            reopened.ready.world.radiation.clear();
            assert_eq!(reopened.update_player_dose(&mut h, "p", |d| d.millirad), Some(3_000 + given));
        }
    }
    /// Slice 5b review R-2: the radiation potion's dose (`updatePotionEffects`) comes after the death check of the
    /// player's `LivingUpdateEvent`. At 999.9 RAD with the potion the player lives through the tick (the potion takes
    /// them over 1000) and dies in the next one.
    #[test]
    fn the_radiation_potion_doses_after_the_death_check() {
        let (mut h, mut s) = prepare();
        s.set_players(vec![PlayerInfo { id: "p".into(), name: "p".into(), dim: "minecraft:the_nether".into(), pos: [0., 64., 0.], creative: false }]);
        s.blasts.entered.insert("p".into(), 0);
        h.attached.insert("p".into(), 999_900);
        s.ready.world.zirnox.irradiate("p", 600, 3);
        s.native_tick(&mut h, 202);
        assert!(h.radiation_deaths.is_empty());
        assert_eq!(h.attached["p"], 1_000_100, "the potion's 200 millirad came after the check");
        s.native_tick(&mut h, 203);
        assert_eq!(h.radiation_deaths, ["p"]);
        assert_eq!(h.attached["p"], 200, "the death left 0 and the potion gave its tick");
    }
    /// Review R2-1: the ruin and the radiation potion dose through `ContaminationUtil.contaminate`, which skips a
    /// player's first 200 ticks (`:287`, `ticksExisted < 200`). A player who joins next to a burning ruin, or respawns
    /// there, takes nothing for 200 ticks and the dose after; the potion still counts down meanwhile.
    #[test]
    fn the_ruin_and_the_potion_spare_a_players_first_200_ticks() {
        let (mut h, mut s) = prepare();
        zm(&mut s).destroyed = true;
        zm(&mut s).on_fire = true;
        s.ready.world.machines.get_mut(&key()).unwrap().block = zirnox::DESTROYED.into();
        h.blocks.insert(key(), zirnox::DESTROYED.into());
        let near = PlayerInfo { id: "p".into(), name: "p".into(), dim: DIM.into(), pos: [30.5, 62.88, 0.5], creative: false };
        s.set_players(vec![near.clone()]);
        s.ready.world.zirnox.irradiate("p", 600, 3);
        h.attached.insert("p".into(), 0);
        let joined = 10;
        for tick in joined..joined + crate::radiation::GRACE_TICKS {
            s.native_tick(&mut h, tick);
        }
        assert_eq!(h.attached["p"], 0, "nothing in the first 200 ticks: {:?}", h.doses);
        assert_eq!(s.ready.world.zirnox.irradiated["p"].0, 600 - 200, "the potion counted down all along");
        assert!(s.rad_buf("p") > 0, "the Geiger heard the ruin all along");
        s.native_tick(&mut h, joined + crate::radiation::GRACE_TICKS);
        let first = h.attached["p"];
        assert!(first > 200, "the ruin and the potion dose on tick 200: {first}");
        // Dead, the player leaves the list (the host lists the living); back, the 200 ticks start again.
        s.set_players(Vec::new());
        s.native_tick(&mut h, joined + crate::radiation::GRACE_TICKS + 1);
        s.set_players(vec![near]);
        let back = joined + crate::radiation::GRACE_TICKS + 2;
        for tick in back..back + crate::radiation::GRACE_TICKS {
            s.native_tick(&mut h, tick);
        }
        assert_eq!(h.attached["p"], first, "a respawn is spared its first 200 ticks too");
        s.native_tick(&mut h, back + crate::radiation::GRACE_TICKS);
        assert!(h.attached["p"] > first);
    }
    const RAD: &str = crate::radiation::RADIATION_EFFECT;
    /// One engine tick: the effects count down first (`tick_effects`, an effect at 0 is gone), then the NTM's server
    /// step, then the queued effects reach the players by the engine's `update` (a higher amplifier replaces, the
    /// same amplifier keeps the longer duration).
    fn tick(h: &mut Host, s: &mut Server, tick: u64) {
        h.effects.retain(|_, (n, _)| {
            *n -= 1;
            *n > 0
        });
        s.native_tick(h, tick);
        for (k, (n, a)) in std::mem::take(&mut h.queued) {
            let e = h.effects.entry(k).or_insert((0, 0));
            if e.0 == 0 || a > e.1 || a == e.1 && n > e.0 {
                *e = (n, a);
            }
        }
    }
    fn potion(h: &Host) -> crate::zirnox::Irradiation {
        h.irradiation.as_ref().unwrap().get("p").copied().unwrap_or_default().potion
    }
    /// A player standing on glowing mycelium, past their first 200 ticks, with a host that keeps the potion in
    /// the player's storage.
    fn on_mycelium(h: &mut Host, s: &mut Server) {
        h.irradiation = Some(BTreeMap::new());
        h.attached.insert("p".into(), 0);
        h.blocks.insert((DIM.into(), [0, 99, 0]), "ntm:waste_mycelium".into());
        s.set_players(vec![PlayerInfo { id: "p".into(), name: "p".into(), dim: DIM.into(), pos: [0.5, 100., 0.5], creative: false }]);
        s.blasts.entered.insert("p".into(), 0);
    }
    fn off_mycelium(s: &mut Server) {
        s.set_players(vec![PlayerInfo { id: "p".into(), name: "p".into(), dim: DIM.into(), pos: [0.5, 100., 40.5], creative: false }]);
    }

    /// Slice 5c, D3: the radiation potion lives in the player's storage and shows as the `ntm:radiation` effect.
    /// `WasteEarth.onEntityWalking` gives it (600 ticks, amplifier 3); `updatePotionEffects` doses 0.2 RAD that same
    /// tick and counts down. The world's map stays empty.
    #[test]
    fn the_radiation_potion_lives_in_the_players_storage() {
        let (mut h, mut s) = prepare();
        on_mycelium(&mut h, &mut s);
        tick(&mut h, &mut s, 300);
        assert_eq!(potion(&h), (599, 3));
        assert_eq!(h.effects[&("p".into(), RAD.into())], (600, 3), "shown with the time left and a tick to spare");
        assert_eq!(h.attached["p"], 200);
        assert!(s.ready.world.zirnox.irradiated.is_empty());
        off_mycelium(&mut s);
        for t in 301..311 {
            tick(&mut h, &mut s, t);
        }
        assert_eq!(potion(&h), (589, 3));
        assert_eq!(h.effects[&("p".into(), RAD.into())], (590, 3), "put back with the time left and a tick to spare");
        assert_eq!(h.attached["p"], 2_200);
        assert!(s.ready.world.zirnox.irradiated.is_empty());
    }

    /// Whatever takes the effect away with time left (milk, the antidote, an iodine pill, a med bag, `/effect
    /// clear`) ends the potion, as `removePotionEffect` does: no dose from the next tick on.
    #[test]
    fn taking_the_effect_away_ends_the_potion() {
        let (mut h, mut s) = prepare();
        on_mycelium(&mut h, &mut s);
        tick(&mut h, &mut s, 300);
        off_mycelium(&mut s);
        tick(&mut h, &mut s, 301);
        let dose = h.attached["p"];
        h.effects.clear();
        tick(&mut h, &mut s, 302);
        assert_eq!(potion(&h), (0, 0));
        assert_eq!(h.attached["p"], dose, "no dose once cured");
        tick(&mut h, &mut s, 303);
        assert!(h.effects.is_empty(), "not put back");
        assert_eq!(h.attached["p"], dose);
    }

    /// On the tick it is given, and on a player's first tick in the world, the effect is not there yet (it reaches
    /// the player at the end of the tick, or was not saved with them): that is no cure.
    #[test]
    fn a_missing_effect_is_no_cure_on_the_tick_it_is_given_or_on_joining() {
        let (mut h, mut s) = prepare();
        on_mycelium(&mut h, &mut s);
        tick(&mut h, &mut s, 300);
        h.effects.clear();
        // Joins again: the storage holds the potion, the effect is not there.
        s.set_players(Vec::new());
        tick(&mut h, &mut s, 301);
        off_mycelium(&mut s);
        tick(&mut h, &mut s, 302);
        assert_eq!(potion(&h), (598, 3), "the first tick back is trusted");
        assert_eq!(h.effects[&("p".into(), RAD.into())], (599, 3), "and the effect is back");
    }

    /// Milk drunk before the step on the mycelium, in the same tick: the old potion is gone and the new one starts
    /// afresh (600 ticks), not summed with what the old one had left.
    #[test]
    fn a_potion_given_right_after_a_cure_starts_afresh() {
        let (mut h, mut s) = prepare();
        on_mycelium(&mut h, &mut s);
        h.irradiation.as_mut().unwrap().insert("p".into(), crate::zirnox::StoredPotion { potion: (900, 3), migrated: (0, 0) });
        h.effects.insert(("p".into(), RAD.into()), (900, 3));
        tick(&mut h, &mut s, 300);
        assert_eq!(potion(&h), (899, 3), "still standing: the longer one stays");
        h.effects.clear();
        tick(&mut h, &mut s, 301);
        assert_eq!(potion(&h), (599, 3));
        assert_eq!(h.effects[&("p".into(), RAD.into())], (600, 3));
    }

    /// The decontaminator (`TileEntityDecon:27`) ends the potion in the storage and takes the effect.
    #[test]
    fn the_decontaminator_ends_the_potion_in_the_storage() {
        let (mut h, mut s) = prepare();
        on_mycelium(&mut h, &mut s);
        tick(&mut h, &mut s, 300);
        off_mycelium(&mut s);
        h.blocks.insert((DIM.into(), [0, 99, 40]), crate::radiation::DECON.into());
        tick(&mut h, &mut s, 301);
        assert_eq!(potion(&h), (0, 0));
        assert!(h.effects.is_empty());
    }

    /// The one-time move of a world from before 5c: a `zirnox.irradiated` entry joins the player's storage (by
    /// `PotionEffect.combine`) once the storage takes the write, and leaves the world's map then. A refused write
    /// keeps the entry and gives no dose; a player who is not in the world keeps it until they come. Saved and opened again, the
    /// storage still has it and the map does not.
    #[test]
    fn the_worlds_potions_move_to_the_players_storage_once() {
        let (mut h, mut s) = prepare();
        h.irradiation = Some(BTreeMap::new());
        h.attached.insert("p".into(), 0);
        s.ready.world.zirnox.irradiate("p", 300, 2);
        s.ready.world.zirnox.irradiate("away", 900, 5);
        s.blasts.entered.insert("p".into(), 0);
        s.set_players(vec![PlayerInfo { id: "p".into(), name: "p".into(), dim: DIM.into(), pos: [0.5, 100., 40.5], creative: false }]);
        h.refuse_irradiation = true;
        tick(&mut h, &mut s, 300);
        assert_eq!(s.ready.world.zirnox.irradiated["p"], (300, 2), "a refused write keeps the entry as it was");
        assert_eq!(h.attached["p"], 0, "and gives no dose");
        h.refuse_irradiation = false;
        tick(&mut h, &mut s, 301);
        assert_eq!(potion(&h), (299, 2));
        assert!(!s.ready.world.zirnox.irradiated.contains_key("p"));
        assert_eq!(s.ready.world.zirnox.irradiated["away"], (900, 5), "waits for its player");
        assert_eq!(h.attached["p"], 150, "one dose for the one tick written");
        s.native_save(&mut h, true);
        s.keep_world(&mut h);
        let back = WorldState::load(&mut h);
        assert!(!back.zirnox.irradiated.contains_key("p"));
        assert_eq!(back.zirnox.irradiated["away"], (900, 5));
        let mut reopened = Server::native(&mut h, 302);
        reopened.set_players(vec![PlayerInfo { id: "p".into(), name: "p".into(), dim: DIM.into(), pos: [0.5, 100., 40.5], creative: false }]);
        reopened.blasts.entered.insert("p".into(), 0);
        tick(&mut h, &mut reopened, 302);
        assert_eq!(potion(&h), (298, 2), "the storage carried it through the reopening");
    }

    /// Creative players: the potion still counts down (`updatePotionEffects`), `contaminate` gives no dose.
    #[test]
    fn a_creative_players_potion_counts_down_without_dose() {
        let (mut h, mut s) = prepare();
        on_mycelium(&mut h, &mut s);
        tick(&mut h, &mut s, 300);
        s.set_players(vec![PlayerInfo { id: "p".into(), name: "p".into(), dim: DIM.into(), pos: [0.5, 100., 40.5], creative: true }]);
        tick(&mut h, &mut s, 301);
        assert_eq!(potion(&h), (598, 3));
        assert_eq!(h.attached["p"], 200);
    }
    /// Runs the player off the mycelium until their potion and its effect are gone; the dose it gave.
    fn run_out(h: &mut Host, s: &mut Server, from: u64, cut: Option<(u64, crate::zirnox::Irradiation)>) -> u64 {
        let start = h.attached["p"];
        for t in from..from + 2000 {
            if let Some((at, entry)) = cut
                && t == at
            {
                // A save cut between the player's file and the world's storage: the world opens with its old entry.
                s.ready.world.zirnox.irradiate("p", entry.0, entry.1);
            }
            tick(h, s, t);
            if potion(h).0 == 0 && h.effects.is_empty() && s.ready.world.zirnox.irradiated.is_empty() {
                break;
            }
        }
        h.attached["p"] - start
    }
    fn migrating() -> (Host, Server) {
        let (mut h, mut s) = prepare();
        h.irradiation = Some(BTreeMap::new());
        h.attached.insert("p".into(), 0);
        s.blasts.entered.insert("p".into(), 0);
        off_mycelium(&mut s);
        s.ready.world.zirnox.irradiate("p", 600, 3);
        (h, s)
    }

    /// Review P3-2: a potion moved from the world's map doses every one of its ticks (600 x 0.2 RAD), with an engine
    /// that counts its effects down before the server step: the effect keeps a tick to spare and is not taken for a
    /// cure on its last tick.
    #[test]
    fn a_migrated_potion_doses_all_its_ticks() {
        let (mut h, mut s) = migrating();
        assert_eq!(run_out(&mut h, &mut s, 300, None), 120_000);
        assert_eq!(h.irradiation.as_ref().unwrap()["p"].migrated, (600, 3));
    }

    /// Review P2-1: a save cut between the player's file and the world's storage leaves the old entry in the world
    /// with the player's storage already past it; the mark taken in with the potion drops it, so its ticks are not
    /// dosed twice.
    #[test]
    fn a_save_cut_during_the_migration_doses_nothing_twice() {
        for at in [301, 400, 850] {
            let (mut h, mut s) = migrating();
            assert_eq!(run_out(&mut h, &mut s, 300, Some((at, (600, 3)))), 120_000, "cut at tick {at}");
        }
    }

    /// The decontaminator ends a world's entry before the storage took it in (it runs before the potion's step):
    /// the entry is marked, so the same entry found again after a save cut does not bring the potion back.
    #[test]
    fn a_decontaminated_worlds_potion_does_not_come_back_after_a_cut() {
        let (mut h, mut s) = migrating();
        h.blocks.insert((DIM.into(), [0, 99, 40]), crate::radiation::DECON.into());
        tick(&mut h, &mut s, 300);
        assert_eq!(potion(&h), (0, 0));
        assert!(s.ready.world.zirnox.irradiated.is_empty());
        h.blocks.remove(&(DIM.into(), [0, 99, 40]));
        assert_eq!(run_out(&mut h, &mut s, 301, Some((301, (600, 3)))), 0);
    }

    /// Re-review RR-1: a guest the engine has not loaded yet keeps its effects still (they don't count down), and
    /// the NTM sees the guest already: an effect held still is no application, and the potion gives exactly its ticks.
    #[test]
    fn an_effect_held_still_is_no_application() {
        let (mut h, mut s) = prepare();
        h.irradiation = Some(BTreeMap::new());
        h.attached.insert("p".into(), 0);
        s.blasts.entered.insert("p".into(), 0);
        off_mycelium(&mut s);
        h.irradiation.as_mut().unwrap().insert("p".into(), crate::zirnox::StoredPotion { potion: (30, 3), migrated: (0, 0) });
        h.effects.insert(("p".into(), RAD.into()), (31, 3));
        for t in 300..400 {
            // No engine tick for this guest: the effect stays, the queue does not reach it.
            s.native_tick(&mut h, t);
            h.queued.clear();
        }
        assert_eq!(potion(&h), (0, 0));
        assert_eq!(h.attached["p"], 30 * 200);
    }

    /// Review P3-4: a refused write gives no dose and counts nothing; the potion still gives its 600 doses.
    #[test]
    fn refused_writes_give_no_dose_and_lose_no_tick() {
        let (mut h, mut s) = migrating();
        tick(&mut h, &mut s, 300);
        h.refuse_irradiation = true;
        for t in 301..306 {
            tick(&mut h, &mut s, t);
        }
        assert_eq!(h.attached["p"], 200, "only the first tick's dose");
        h.refuse_irradiation = false;
        assert_eq!(200 + run_out(&mut h, &mut s, 306, None), 120_000);
    }

    /// Review P3-3: `/effect give @s ntm:radiation 20 9` is a real potion in the Java (`performEffect` doses it):
    /// an effect stronger or longer than the storage's is taken in as an application.
    #[test]
    fn a_given_radiation_effect_doses_as_the_potion() {
        let (mut h, mut s) = prepare();
        h.irradiation = Some(BTreeMap::new());
        h.attached.insert("p".into(), 0);
        s.blasts.entered.insert("p".into(), 0);
        off_mycelium(&mut s);
        tick(&mut h, &mut s, 300);
        h.effects.insert(("p".into(), RAD.into()), (400, 9));
        tick(&mut h, &mut s, 301);
        assert_eq!(potion(&h), (398, 9));
        assert_eq!(h.attached["p"], 500);
        assert_eq!(run_out(&mut h, &mut s, 302, None), 398 * 500);
    }
    #[test]
    fn extraction_contract_exposes_conditional_rod_outputs() {
        assert!(!zirnox::can_extract(0, &zirnox::table().fuels[0].item));
        assert!(zirnox::can_extract(23, &zirnox::table().fuels[0].output));
        assert!(!zirnox::can_extract(24, "minecraft:bucket"));
        let c = crate::native::container(zirnox::BLOCK).unwrap();
        assert_eq!(c.inputs, (0..24).collect::<Vec<_>>());
        assert_eq!(c.outputs, (0..24).collect::<Vec<_>>());
        assert!(c.faces.iter().all(|v| v.len() == 24));
    }
    #[test]
    fn waste_preserves_nonopaque_wood_and_transforms_logs_planks_and_mushrooms() {
        let (mut h, mut s) = prepare();
        let cases = [("oak_fence", "minecraft:oak_fence"), ("oak_fence_gate", "minecraft:oak_fence_gate"), ("chest", "minecraft:chest"), ("oak_log", "ntm:waste_log"), ("oak_planks", "ntm:waste_planks"), ("mushroom_stem", "ntm:waste_log"), ("brown_mushroom_block", "minecraft:air")];
        for (i, (before, _)) in cases.iter().enumerate() {
            h.blocks.insert((DIM.into(), [15, 64, i as i32]), format!("minecraft:{before}"));
        }
        s.ready.world.zirnox.waste.push(Waste { origin:None,dim: DIM.into(), center: AT, cursor: 0, seed: 42 });
        for t in 2..90 { s.native_tick(&mut h, t); }
        for (i, (_, after)) in cases.iter().enumerate() {
            assert_eq!(&h.blocks[&(DIM.into(), [15, 64, i as i32])], after);
        }
    }
}
