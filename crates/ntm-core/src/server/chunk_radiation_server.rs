//! The world's radiation by chunk in the server (Onda 1, 4a): `ChunkRadiationHandlerSimple` through
//! `ChunkRadiationManager`, its arithmetic in [`crate::chunk_radiation`].
//!
//! - Each world's map is the Java's: a chunk loaded is put in with what its save holds (`receiveChunkLoad`); one that
//!   unloads stays (the `receiveChunkUnload` bug) and its save takes the map's value (`receiveChunkSave`, which the
//!   unload's save calls); every 20 server ticks from the server's start (`eggTimer`, `Phase.END`) the map spreads
//!   and decays.
//! - The chunks' saves are the NTM storage (`persist::CHUNK_RADIATION_KEY`), not their NBT: the engine gives a mod no
//!   data of its own in a chunk. Two differences follow. Every save writes every loaded chunk's value, where the Java
//!   writes it with the chunks the save writes (those `isModified`, which `setRadiation` marks and the spread does
//!   not, and those that unload): no value is lost, only some written that the Java might not have rewritten. And the
//!   storage is committed after the chunks: a cut between the two leaves the chunks of this save with the values of
//!   the last commit (docs/onda-1-protecao.md, 4a).
use super::*;
use crate::chunk_radiation::{self, ChunkMap};

/// `RadiationConfig.hellRad`: the least RAD/s a player in the Nether takes (`AMBIENT_00_nether`, 0.1 by default).
pub const HELL_RAD: f64 = 0.1;

/// `ntm:rad_fog_<n>`: the fog's 25 quads as mod particles, each of its own size.
pub const FOG_PARTICLES: [&str; 25] = [
    "ntm:rad_fog_0", "ntm:rad_fog_1", "ntm:rad_fog_2", "ntm:rad_fog_3", "ntm:rad_fog_4", "ntm:rad_fog_5", "ntm:rad_fog_6",
    "ntm:rad_fog_7", "ntm:rad_fog_8", "ntm:rad_fog_9", "ntm:rad_fog_10", "ntm:rad_fog_11", "ntm:rad_fog_12",
    "ntm:rad_fog_13", "ntm:rad_fog_14", "ntm:rad_fog_15", "ntm:rad_fog_16", "ntm:rad_fog_17", "ntm:rad_fog_18",
    "ntm:rad_fog_19", "ntm:rad_fog_20", "ntm:rad_fog_21", "ntm:rad_fog_22", "ntm:rad_fog_23", "ntm:rad_fog_24",
];

/// A chunk the probe read: where, its raw value (None: not in the map), the next update's, whether it is loaded.
pub type Probed = ([i32; 2], Option<f32>, f32, bool);

/// The world's radiation by chunk in memory: each world's map as the Java holds it, the chunks loaded, the update's
/// timer and the fractions of a milliRAD `contaminate` left. Not part of `Ready`: every group of machines copies
/// `Ready` for its transaction; the RBMK's writes reach it through `Ready::chunk_rad_pending`.
#[derive(Clone, Debug, Default)]
pub(super) struct Field {
    /// Each world's map (`SimpleRadiationPerWorld.radiation`).
    pub(super) maps: BTreeMap<String, ChunkMap>,
    /// The chunks loaded, by world (the hooks of loading and unloading): the ones a save writes.
    pub(super) loaded: BTreeMap<String, BTreeSet<[i32; 2]>>,
    /// `ChunkRadiationManager.eggTimer`: server ticks since the last update, counted from the server's start.
    pub(super) timer: u32,
    /// The fractions of a milliRAD the field's `contaminate` left, by player: into `radEnv` and into the dose.
    pub(super) env_carry: BTreeMap<String, f64>,
    pub(super) dose_carry: BTreeMap<String, f64>,
}

impl Server {
    /// A chunk of `dim` loaded (`ChunkDataEvent.Load`): its saved value into the map, over what the map had.
    pub fn native_chunk_radiation_loaded(&mut self, dim: &str, chunk: [i32; 2]) {
        let saved = self.ready.world.chunk_radiation.get(dim, chunk);
        self.field.maps.entry(dim.to_string()).or_default().put(chunk, saved);
        self.field.loaded.entry(dim.to_string()).or_default().insert(chunk);
    }

    /// A chunk of `dim` unloaded: its save takes the map's value (0 when it has none); the map keeps it.
    pub fn native_chunk_radiation_unloaded(&mut self, dim: &str, chunk: [i32; 2]) {
        let Some(loaded) = self.field.loaded.get_mut(dim) else { return };
        if !loaded.remove(&chunk) {
            return;
        }
        self.save_chunk_radiation(dim, chunk);
    }

    /// The field's value at a chunk as `getRadiation` reads it (clamped to 0..100 000; 0 in a world with none).
    pub fn native_chunk_radiation(&self, dim: &str, chunk: [i32; 2]) -> f32 {
        self.field.maps.get(dim).map_or(0.0, |m| chunk_radiation::get(m, chunk))
    }

    /// A save takes the chunks: each loaded chunk's value into its save.
    pub(super) fn save_loaded_chunk_radiation(&mut self) {
        let loaded: Vec<(String, [i32; 2])> = self
            .field
            .loaded
            .iter()
            .flat_map(|(dim, chunks)| chunks.iter().map(move |c| (dim.clone(), *c)))
            .collect();
        for (dim, chunk) in loaded {
            self.save_chunk_radiation(&dim, chunk);
        }
    }

    /// `receiveChunkSave`: the map's value, as it is (0 when it has none), into the chunk's save. The saved part's stamp
    /// moves only when the value does.
    fn save_chunk_radiation(&mut self, dim: &str, chunk: [i32; 2]) {
        let rad = self.field.maps.get(dim).and_then(|m| m.get(chunk)).unwrap_or(0.0);
        if self.ready.world.chunk_radiation.differs(dim, chunk, rad) {
            self.ready.world.chunk_radiation.set(dim, chunk, rad);
        }
    }

    /// `EntityEffectHandler.handleRadiationFX:262-279`, every tick, for each player: the field's value where they stand
    /// (`getRadiation` at their block, clamped), at least `hellRad` in the Nether, goes through
    /// `ContaminationUtil.contaminate(.., RADIATION, CREATIVE, rad / 20F)`:
    /// - `radEnv` takes the amount before any gate (the Geiger hears it: [`Server::note_contamination`]);
    /// - a creative player takes no dose (`:284`), nor one in their first 200 ticks (`:287`);
    /// - the dose takes the amount times `calculateRadiationMod` (the armor, `ServerHost::radiation_factor`).
    ///
    /// `isRadImmune` (`:268`) holds a player only under `HbmPotion.mutation`, which the port has not. The Java's float
    /// sums are the port's milliRAD; the fractions of a milliRAD each tick leaves are carried to the next, so a weak
    /// field doses over time as the Java's does instead of nothing.
    pub(super) fn chunk_radiation_contaminate(&mut self, host: &mut impl ServerHost) {
        for p in self.players.clone() {
            let chunk = [(p.pos[0].floor() as i32) >> 4, (p.pos[2].floor() as i32) >> 4];
            let mut rad = self.native_chunk_radiation(&p.dim, chunk);
            if p.dim == "minecraft:the_nether" && HELL_RAD > 0.0 && f64::from(rad) < HELL_RAD {
                rad = HELL_RAD as f32;
            }
            if rad <= 0.0 {
                continue;
            }
            let amount = f64::from(rad / 20.0) * 1000.0;
            let env = self.field.env_carry.entry(p.id.clone()).or_insert(0.0);
            *env += amount;
            let whole = env.trunc();
            *env -= whole;
            self.note_contamination(&p.id, whole as i64);
            if p.creative || self.in_grace(&p.id) {
                continue;
            }
            // calculateRadiationMod is a float: amount * mod in float, as the Java's `amount * mod`.
            let factor = host.radiation_factor(&p.id) as f32;
            let dosed = f64::from(rad / 20.0 * factor) * 1000.0;
            let carry = self.field.dose_carry.get(&p.id).copied().unwrap_or(0.0) + dosed;
            let whole = carry.trunc();
            if whole == 0.0 {
                self.field.dose_carry.insert(p.id.clone(), carry);
                continue;
            }
            if self.update_player_dose(host, &p.id, |dose| crate::radiation::add_dose(dose, whole as u64)).is_some() {
                self.field.dose_carry.insert(p.id.clone(), carry - whole);
            }
        }
        self.field.env_carry.retain(|id, _| self.players.iter().any(|p| &p.id == id));
        self.field.dose_carry.retain(|id, _| self.players.iter().any(|p| &p.id == id));
    }

    /// `ChunkRadiationManager.updateSystem`, at the end of a server tick: `updateSystem` every 20 ticks, with its
    /// fog; `handleWorldDestruction` every tick (`worldRadEffects`).
    pub(super) fn chunk_radiation_tick(&mut self, host: &mut impl ServerHost) {
        self.field.timer += 1;
        if self.field.timer >= 20 {
            let dims: Vec<String> = self.field.maps.keys().cloned().collect();
            for dim in dims {
                let mut wrote = Vec::new();
                if let Some(map) = self.field.maps.get_mut(&dim) {
                    chunk_radiation::update_system_with(map, |coord, rad| wrote.push((coord, rad)));
                }
                self.chunk_fog(host, &dim, &wrote);
            }
            self.field.timer = 0;
        }
        self.world_destruction(host);
    }

    /// The world's draws for one purpose this tick (`world.rand`; the mod has none: a hash of the world, the tick
    /// and the purpose).
    fn field_random(&self, dim: &str, purpose: &str) -> crate::rbmk::jrandom::JavaRandom {
        let key = format!("{dim}|{}|{purpose}", self.tick);
        crate::rbmk::jrandom::JavaRandom::new(crate::hash::mix(crate::hash::fnv64(key.as_bytes())) as i64)
    }

    /// `updateSystem:104-117`, the fog: each share written over `fogRad`, in 1 of `fogCh`, and when the chunk that
    /// gave it exists, a fog at a random column of that chunk (not the one that got the share), `getHeightValue` plus
    /// 0 to 4 above. `ParticleRadiationFog` is drawn as its 25 quads, each a mod particle (`ntm:rad_fog_<n>`, of
    /// that quad's size) at its place (`chunk_radiation::fog_quads`).
    fn chunk_fog(&mut self, host: &mut impl ServerHost, dim: &str, wrote: &[([i32; 2], f32)]) {
        let mut rand = self.field_random(dim, "fog");
        for &(coord, rad) in wrote {
            if !(rad > chunk_radiation::FOG_RAD && rand.next_int(chunk_radiation::FOG_CHANCE) == 0) {
                continue;
            }
            if !self.field.loaded.get(dim).is_some_and(|l| l.contains(&coord)) {
                continue;
            }
            let x = coord[0] * 16 + rand.next_int(16);
            let z = coord[1] * 16 + rand.next_int(16);
            let Some(height) = host.surface_heights(dim, &[(x, z)]).first().copied().flatten() else {
                continue;
            };
            let y = height + rand.next_int(5);
            for (n, (at, _)) in chunk_radiation::fog_quads().iter().enumerate() {
                host.show(
                    dim,
                    &crate::blast::visuals::Show::Particles {
                        id: FOG_PARTICLES[n],
                        color: [1.0; 4],
                        at: [f64::from(x) + at[0], f64::from(y) + at[1], f64::from(z) + at[2]],
                        spread: [0.0; 3],
                        speed: 0.0,
                        count: 1,
                    },
                );
            }
        }
    }

    /// `handleWorldDestruction:190-255`, every tick, each world: five entries drawn from its map (in the map's
    /// order); each one at 10 RAD/s or more whose chunk is loaded goes ten times over its 16 × 16 columns, a column
    /// in three at its height or the block below, grass to dead earth, tall grass to air, leaves to dead leaves (6 in
    /// 7) or air. The heights are read at each pass: a pass sees what the one before it changed.
    pub(super) fn world_destruction(&mut self, host: &mut impl ServerHost) {
        let dims: Vec<String> = self.field.maps.keys().cloned().collect();
        for dim in dims {
            let entries: Vec<([i32; 2], f32)> = self.field.maps[&dim].iter().collect();
            if entries.is_empty() {
                continue;
            }
            let mut rand = self.field_random(&dim, "destruction");
            for _ in 0..5 {
                let (coords, value) = entries[rand.next_int(entries.len() as i32) as usize];
                for _ in 0..10 {
                    if value < 10.0 {
                        continue;
                    }
                    if !self.field.loaded.get(&dim).is_some_and(|l| l.contains(&coords)) {
                        continue;
                    }
                    let heights = host.surface_heights(&dim, &chunk_radiation::chunk_columns(coords));
                    chunk_radiation::destruction_pass(coords, &heights, &mut rand, |at, rand| {
                        let Some(block) = host.get_block(&dim, at) else { return };
                        if let Some(new) = chunk_radiation::destroyed(&block, || rand.next_int(7))
                            && let Err(e) = host.set_blocks(&dim, &[(at, new.to_string())])
                        {
                            host.log(&format!("world destruction at {at:?}: {e}"));
                        }
                    });
                }
            }
        }
    }

    /// The raw value the world's map holds at a chunk (None: not in the map), for the in-game proofs.
    pub fn native_chunk_radiation_raw(&self, dim: &str, chunk: [i32; 2]) -> Option<f32> {
        self.field.maps.get(dim).and_then(|m| m.get(chunk))
    }

    /// The in-game proofs' probe: each chunk within `r` of `center`, its raw value (None: not in the map), the one the
    /// next update would give it (an update of a copy of the map) and whether it is loaded (a save writes it), with the
    /// ticks since the last update.
    pub fn native_chunk_radiation_probe(&self, dim: &str, center: [i32; 2], r: i32) -> (u32, Vec<Probed>) {
        let map = self.field.maps.get(dim).cloned().unwrap_or_default();
        let mut next = map.clone();
        chunk_radiation::update_system(&mut next);
        let mut out = Vec::new();
        for x in center[0] - r..=center[0] + r {
            for z in center[1] - r..=center[1] + r {
                let loaded = self.field.loaded.get(dim).is_some_and(|l| l.contains(&[x, z]));
                out.push(([x, z], map.get([x, z]), next.get([x, z]).unwrap_or(0.0), loaded));
            }
        }
        (self.field.timer, out)
    }

    /// `CommandRadiation` (`/ntmrad`): `set <amount>` puts the sender's chunk at the amount (0 to 100 000, through
    /// `setRadiation`, only in a loaded chunk); `clear` empties the sender's world's map (`clearSystem`). The Java's
    /// replies.
    pub fn native_radiation_command(&mut self, dim: &str, at: [f64; 3], line: &str) -> Option<Result<String, String>> {
        let mut words = line.split_whitespace();
        if words.next() != Some("ntmrad") {
            return None;
        }
        let args: Vec<&str> = words.collect();
        const USAGE: &str = "/ntmrad <set/clear>";
        Some(match args.as_slice() {
            ["clear"] => {
                if let Some(map) = self.field.maps.get_mut(dim) {
                    map.clear();
                }
                Ok("Cleared radiation data!".into())
            }
            ["set", amount] => {
                // CommandBase.parseDoubleBounded(sender, s, 0D, 100_000D), then (float).
                let Ok(amount) = amount.parse::<f64>() else {
                    return Some(Err(format!("'{amount}' is not a valid number")));
                };
                if amount < 0.0 {
                    return Some(Err(format!("The number you have entered ({amount}) is too small, it must be at least 0.0")));
                }
                if amount > 100_000.0 {
                    return Some(Err(format!("The number you have entered ({amount}) is too big, it must be at most 100000.0")));
                }
                let chunk = [(at[0].floor() as i32) >> 4, (at[2].floor() as i32) >> 4];
                self.set_chunk_radiation(dim, chunk, amount as f32);
                Ok("Radiation set.".into())
            }
            _ => Err(USAGE.into()),
        })
    }

    /// The world's map, for the tests.
    #[cfg(test)]
    pub(crate) fn native_chunk_field_for_test(&mut self, dim: &str) -> &mut crate::chunk_radiation::ChunkMap {
        self.field.maps.entry(dim.to_string()).or_default()
    }

    /// `incrementRad` at a block: the field's value of its chunk plus `rad`, through `setRadiation` (only in a loaded
    /// chunk, clamped). The world's sources call it (4d).
    pub fn native_increment_radiation(&mut self, dim: &str, pos: [i32; 3], rad: f32) {
        let chunk = [pos[0] >> 4, pos[2] >> 4];
        if self.field.loaded.get(dim).is_some_and(|l| l.contains(&chunk)) {
            chunk_radiation::increment(self.field.maps.entry(dim.to_string()).or_default(), chunk, rad);
        }
    }

    /// The RBMK's writes of the steps accepted this tick, in order.
    pub(super) fn apply_chunk_rad_pending(&mut self) {
        for (dim, pos, rad) in std::mem::take(&mut self.ready.chunk_rad_pending) {
            self.native_increment_radiation(&dim, pos, rad);
        }
    }

    /// `setRadiation`: clamped to 0..100 000, only in a loaded chunk (`world.blockExists`).
    pub(super) fn set_chunk_radiation(&mut self, dim: &str, chunk: [i32; 2], rad: f32) {
        if self.field.loaded.get(dim).is_some_and(|l| l.contains(&chunk)) {
            chunk_radiation::set(self.field.maps.entry(dim.to_string()).or_default(), chunk, rad);
        }
    }
}
