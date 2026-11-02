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
        self.ready.zirnox_radiation_rate.clear();
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
                if z.step() && self.zirnox_meltdown(host, &key, m.facing.as_deref()) {
                    m.block = zirnox::DESTROYED.into();
                    **z = Zirnox::of(zirnox::DESTROYED);
                    self.ready.native_saved.remove(&key);
                    self.ready.shown.remove(&key);
                    self.ready.topo_dirty = true;
                }
            }
            self.ready.world.machines.insert(key, m);
        }
        let blasts = std::mem::take(&mut self.ready.world.zirnox.blasts);
        for (dim, at) in blasts {
            if !host.pile_reachable(&dim) || !host.zirnox_explode(&dim, at) {
                self.ready.world.zirnox.blasts.push((dim, at));
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
        // Remove rods before the vanilla explosion can spill the inventory.
        // Failed writes leave the live state pending for the next tick.
        for i in 0..28 {
            if !host.set_machine_slot(&key.0, key.1, i, None) {
                host.log("zirnox failure: inventory unreachable, retrying");
                return false;
            }
            // Keep the bridge aware even if the subsequent block batch fails;
            // slots_out then restores every cleared slot before saving state.
            if let Some(last) = self
                .ready
                .bridge
                .last
                .get_mut(key)
                .and_then(|v| v.get_mut(i as usize))
            {
                *last = None;
            }
        }
        if let Err(e) = host.set_blocks(&key.0, &blocks) {
            host.log(&format!("zirnox failure: {e}"));
            return false;
        }
        let at = [
            f64::from(key.1[0]),
            f64::from(key.1[1]) + 3.,
            f64::from(key.1[2]),
        ];
        self.ready.world.zirnox.blasts.push((key.0.clone(), at));
        self.slots_gone(key);
        host.show(
            &key.0,
            &Show::Sound {
                id: "minecraft:entity.generic.explode",
                source: "block",
                at,
                volume: 10.,
                pitch: 1.,
            },
        );
        let s = seed(&key.0, key.1, self.ready.world.ticks);
        self.ready
            .world
            .zirnox
            .debris
            .extend(Debris::spawn(&key.0, key.1, s));
        self.ready.world.zirnox.waste.push(Waste {
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
            for i in 1..len as usize {
                let at = [0, 1, 2].map(|k| (center[k] + delta[k] * i as f64 / len).floor() as i32);
                if let Some(r) = host.block_resistance(&key.0, at) {
                    resistance += f64::from(r.max(0.));
                }
            }
            let rad = if z.on_fire { 500000. } else { 75000. };
            let amount = (rad / resistance.max(1.) / (len * len) * 1000.) as u64;
            crate::radiation::add_dose(
                self.ready.world.radiation.entry(p.id.clone()).or_default(),
                amount,
            );
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
            host.show(
                &key.0,
                &Show::Sound {
                    id: "minecraft:block.fire.ambient",
                    source: "block",
                    at: center,
                    volume: 1.,
                    pitch: 0.65,
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
                    if host.set_blocks(&d.dim, &changes).is_ok() {
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
                for p in self.players.iter().filter(|p| {
                    p.dim == d.dim
                        && !p.creative
                        && (0..3).all(|k| (p.pos[k] - d.pos[k]).abs() <= 2.5)
                }) {
                    self.ready.world.zirnox.irradiate(&p.id, 1200, level);
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
        // WasteEarth.onEntityWalking: glowing mycelium gives radiation
        // amplifier 3 for 30 seconds. Source spreading is off by default.
        for p in self.players.iter().filter(|p| !p.creative) {
            let below = [
                p.pos[0].floor() as i32,
                (p.pos[1] - 0.05).floor() as i32,
                p.pos[2].floor() as i32,
            ];
            if host
                .get_block(&p.dim, below)
                .is_some_and(|b| blockstate::id_of(&b) == "ntm:waste_mycelium")
            {
                self.ready.world.zirnox.irradiate(&p.id, 600, 3);
            }
        }
        for (id, (remaining, level)) in &mut self.ready.world.zirnox.irradiated {
            if let Some(p) = self.players.iter().find(|p| p.id == *id) {
                if !p.creative {
                    crate::radiation::add_dose(
                        self.ready.world.radiation.entry(id.clone()).or_default(),
                        (u64::from(*level) + 1) * 50,
                    );
                    let rate = self
                        .ready
                        .zirnox_radiation_rate
                        .entry(id.clone())
                        .or_default();
                    *rate = rate.saturating_add((u32::from(*level) + 1) * 1000);
                }
                *remaining = remaining.saturating_sub(1);
            }
        }
        self.ready
            .world
            .zirnox
            .irradiated
            .retain(|_, (n, _)| *n > 0);
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
        _ => match crate::explosion::crater::classify(block).kind {
            crate::explosion::crater::Kind::Wood => "ntm:waste_log",
            crate::explosion::crater::Kind::Planks => "ntm:waste_planks",
            _ => return None,
        },
    };
    Some(out.into())
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
        refuse_data: bool,
        refuse_blocks: bool,
        refuse_blast: bool,
        blasts: Vec<(String, [f64; 3])>,
        buttons: Vec<(String, String, Pos, u32)>,
        shows: Vec<String>,
        hurt: Vec<(String, f32)>,
        carried: BTreeMap<String, Vec<Stack>>,
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
                self.blocks.insert((d.into(), *p), b.clone());
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
        fn set_machine_slot(&mut self, d: &str, p: Pos, i: u32, s: Option<Stack>) -> bool {
            if self.refuse_slots {
                return false;
            }
            if let Some(c) = self
                .slots
                .get_mut(&(d.into(), p))
                .and_then(|v| v.get_mut(i as usize))
            {
                *c = s;
                true
            } else {
                false
            }
        }
        fn machine_buttons(&mut self) -> Vec<(String, String, Pos, u32)> {
            std::mem::take(&mut self.buttons)
        }
        fn machine_data(&mut self, d: &str, p: Pos) -> Option<Vec<i32>> {
            self.data.get(&(d.into(), p)).cloned()
        }
        fn set_machine_data(&mut self, d: &str, p: Pos, v: &[i32]) -> bool {
            if self.refuse_data {
                false
            } else {
                self.data.insert((d.into(), p), v.to_vec());
                true
            }
        }
        fn save_machine(&mut self, d: &str, p: Pos, j: &str) -> bool {
            if self.refuse_save {
                false
            } else {
                self.saved.insert((d.into(), p), j.into());
                true
            }
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
        fn hurt(&mut self, id: &str, n: f32, _: &str) {
            self.hurt.push((id.into(), n));
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
        assert_eq!(zirnox::life(z(&s).slots[0].as_ref().unwrap()), before + 3);
        h.refuse_slots = false;
        h.refuse_data = false;
        h.refuse_save = true;
        s.native_tick(&mut h, 5);
        assert_eq!(
            zirnox::life(h.slots[&key()][0].as_ref().unwrap()),
            before + 4
        );
        assert_eq!(h.saved[&key()], saved);
        h.refuse_save = false;
        s.native_tick(&mut h, 6);
        assert_ne!(h.saved[&key()], saved);
        assert_eq!(
            zirnox::life(h.slots[&key()][0].as_ref().unwrap()),
            before + 5
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
        assert!(s.ready.bridge.pending.is_empty());
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
        assert!(!z(&s).destroyed);
        assert!(h.blasts.is_empty());
        h.refuse_blocks = false;
        h.refuse_blast = true;
        s.native_tick(&mut h, 3);
        assert!(z(&s).destroyed);
        assert!(h.slots[&key()].iter().all(Option::is_none));
        assert_eq!(s.ready.world.zirnox.debris.len(), 72);
        assert_eq!(s.ready.world.zirnox.blasts.len(), 1);
        // Native notifications from our replacement arrive next tick.
        s.native_block_changed(DIM, AT, zirnox::BLOCK, zirnox::DESTROYED);
        h.refuse_blast = false;
        s.native_tick(&mut h, 4);
        assert!(z(&s).destroyed);
        assert_eq!(h.blasts, vec![(DIM.into(), [0., 67., 0.])]);
        assert!(s.ready.world.zirnox.blasts.is_empty());
        let back = WorldState::load(&mut h);
        assert_eq!(back.zirnox.debris.len(), 72);
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
        s.native_tick(&mut h, 2);
        assert!(s.ready.world.radiation["same"].millirad > 0);
        assert!(!s.ready.world.radiation.contains_key("other"));
        assert_eq!(h.hurt.len(), 1);
        zm(&mut s).on_fire = false;
        s.native_tick(&mut h, 3);
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
    fn extraction_contract_is_ready_for_conditional_engine_hook() {
        assert!(!zirnox::can_extract(0, &zirnox::table().fuels[0].item));
        assert!(zirnox::can_extract(23, &zirnox::table().fuels[0].output));
        assert!(!zirnox::can_extract(24, "minecraft:bucket"));
        let c = crate::native::container(zirnox::BLOCK).unwrap();
        assert_eq!(c.inputs, (0..24).collect::<Vec<_>>());
        assert!(c.outputs.is_empty());
        assert!(c.faces.iter().all(|v| v.len() == 24));
    }
}
