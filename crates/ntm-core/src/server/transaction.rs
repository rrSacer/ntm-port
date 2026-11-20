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
        #[serde(default)]
        expected: Vec<CellGuard>,
        #[serde(default)]
        retained:Vec<RetainedDrop>,
    },
    Items {
        dim: String,
        pos: Pos,
        items: Vec<(String, u32)>,
    },
    EncodedItems { dim: String, pos: Pos, items: Vec<String> },
    /// Generic native explosion; unlike `PileExplosion`, its Java flags are explicit.
    Explosion { dim: String, at: [f64; 3], power: f32, fire: bool, breaks: bool },
    PileExplosion {
        dim: String,
        pos: [f64; 3],
    },
    /// Entities a step puts into the world (debris, `rbmk-plano` T20), owed in the cell `pos` like items: spawned
    /// once by the effect's id (`ServerHost::apply_entities`), or owed in an owner's chunk record until `pos` loads.
    Entities { dim: String, pos: Pos, entities: Vec<EntitySpawn> },
}

/// One entity of a [`WorldEffect::Entities`]: its kind, exact position and compound (SNBT, as `/summon` takes it).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EntitySpawn {
    pub kind: String,
    pub pos: [f64; 3],
    pub nbt: String,
}
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommittedEffects {
    /// Names this batch for good (0: not named yet): with `done` it gives each item effect a stable id.
    #[serde(default)]
    pub serial: u64,
    /// Effects of the batch already applied: the id of the front effect is `(serial, done)`.
    #[serde(default)]
    pub done: u32,
    pub owners: Vec<(String, Pos)>,
    /// Read guards of the step, including unchanged participants. These do not prove consumption.
    #[serde(default)]
    pub origins: Vec<CellGuard>,
    pub effects: std::collections::VecDeque<WorldEffect>,
    /// Proof of consumed model generations, retaining their contents through save confirmation.
    pub tombstones: Vec<((String, Pos), Machine)>,
    /// Runtime only: the batch came back from the storage of an earlier session, so the world's files may be
    /// older than it. Each chunk says by its ledger whether it already holds an effect.
    #[serde(skip)]
    pub replayed: bool,
}

/// The chunks a set of cells is in.
pub(super) fn chunks_of<'a>(keys:impl Iterator<Item=&'a Key>)->BTreeSet<(String,i32,i32)>{
    keys.map(|k|(k.0.clone(),k.1[0]>>4,k.1[2]>>4)).collect()
}

/// A machine's state as a batch of world effects left it.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Survivor {
    pub dim: String,
    pub pos: Pos,
    pub generation: u64,
    pub rev: u64,
    pub json: String,
}

/// What a save took with its chunks of the session's receipts and survivors: the first `receipts` receipts, and each
/// survivor as it stood.
#[derive(Clone, Debug, Default)]
pub(super) struct Taken {
    receipts: usize,
    survivors: Vec<((String, Pos), u64, u64)>,
}

/// A lossless retention journal written before consuming a BE's cell.
/// Recovery can prove the original source is still intact and discard it.
#[derive(Clone,Debug,PartialEq,serde::Serialize,serde::Deserialize)]
pub struct PreparedRetention {
    pub source:CellWrite,
    pub receipt:CommittedEffects,
}

/// World effects are held until the inventory/state commit succeeds.
type AfterCommit<H> = Box<dyn FnOnce(&mut H)>;

struct Deferred<'a, H> {
    host: &'a mut H,
    slots: BTreeMap<Key, Slots>,
    effects: Vec<(Vec<CellGuard>,AfterCommit<H>)>,
    generations:BTreeMap<Key,u64>,
    cells:BTreeMap<Key,CellGuard>,
    vacant:BTreeSet<Key>,
    origins:Vec<CellGuard>,
    valid:bool,
    resource:bool,
    world_writes:Vec<CellWrite>,
    critical: std::collections::VecDeque<WorldEffect>,
    /// Doses a step wrote, by player: what a later read in the same step sees. The player's own storage takes them
    /// after the commit, each write expecting the one before it.
    doses: BTreeMap<String, crate::radiation::Dose>,
    /// The same for the radiation potion in the player's storage.
    irradiations: BTreeMap<String, crate::zirnox::StoredPotion>,
    /// The same for the digamma (`HbmLivingProps.digamma`).
    digammas: BTreeMap<String, f32>,
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
    fn trace_effects(&self)->bool{self.host.trace_effects()}
    fn get_block(&mut self, d: &str, p: Pos) -> Option<String> {
        if self.resource && !self.cells.contains_key(&(d.into(),p)) {
            let snapshot=self.host.cell_snapshot(d,p)?;
            if !self.world_writes.iter().any(|w|w.guard.dim==d&&w.guard.pos==p){
                let guard=CellGuard{dim:d.into(),pos:p,generation:self.generations.get(&(d.into(),p)).copied().unwrap_or(0),state:snapshot.state.clone()};
                self.world_writes.push(CellWrite{notify:true,initial:None,guard,desired:snapshot.state.clone(),expected:snapshot,destroy:false});
            }
            return Some(self.world_writes.iter().find(|w|w.guard.dim==d&&w.guard.pos==p).unwrap().expected.state.clone());
        }
        self.host.get_block(d, p)
    }
    fn fel_opaque(&mut self, d: &str, p: Pos) -> Option<bool> {
        self.host.fel_opaque(d, p)
    }
    /// The block as this step reads it (`get_block`, guarded alike), with the host's fact of whether it is opaque.
    fn block_opaque(&mut self, d: &str, p: Pos) -> Option<(String, bool)> {
        let b = self.get_block(d, p)?;
        let (_, opaque) = self.host.block_opaque(d, p)?;
        Some((b, opaque))
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
    /// A read of the world's recipes, which no step changes: the autocrafter's.
    fn crafting_matches(&mut self, grid: &[Option<Stack>]) -> Vec<crate::autocrafter::CraftMatch> {
        self.host.crafting_matches(grid)
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
        if self.resource {
            for (pos,desired) in b {
                if let Some(previous)=self.world_writes.iter_mut().find(|w|w.guard.dim==d&&w.guard.pos==*pos){previous.desired=desired.clone();previous.destroy=blockstate::id_of(desired)=="minecraft:air" && blockstate::id_of(&previous.expected.state)!="minecraft:air";continue;}
                let Some(expected)=self.host.cell_snapshot(d,*pos) else{self.valid=false;return Err("unavailable consumed cell".into())};
                let guard=CellGuard{dim:d.into(),pos:*pos,generation:self.generations.get(&(d.into(),*pos)).copied().unwrap_or(0),state:expected.state.clone()};
                let destroy=blockstate::id_of(desired)=="minecraft:air" && blockstate::id_of(&expected.state)!="minecraft:air";
                self.world_writes.push(CellWrite{notify:true,initial:None,guard,expected,desired:desired.clone(),destroy});
            }
            return Ok(());
        }
        let mut expected=Vec::new();
        for (pos,_) in b {
            let Some(state)=self.host.get_block(d,*pos) else{self.valid=false;return Err("unavailable effect destination".into())};
            let key=(d.to_string(),*pos);
            let current=self.generations.get(&key).copied().unwrap_or(0);
            let generation=self.cells.get(&key).map_or(current,|origin|{
                if origin.generation==current || self.vacant.contains(&key)&&blockstate::id_of(&state)=="minecraft:air"&&origin.generation.checked_add(1)==Some(current){current}else{origin.generation}
            });
            expected.push(CellGuard{dim:d.into(),pos:*pos,generation,state:blockstate::id_of(&state).into()});
        }
        self.critical.push_back(WorldEffect::Blocks {dim:d.into(),blocks:b.to_vec(),expected,retained:Vec::new()});
        Ok(())
    }
    fn tell(&mut self, p: &str, t: &str) {
        let (p, t) = (p.to_string(), t.to_string());
        self.effects.push((self.origins.clone(),Box::new(move |h| h.tell(&p, &t))));
    }
    fn log(&mut self, t: &str) {
        let t = t.to_string();
        self.effects.push((self.origins.clone(),Box::new(move |h| h.log(&t))));
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
    fn spawn_entities(&mut self, d: &str, p: Pos, entities: &[EntitySpawn]) -> bool {
        self.critical.push_back(WorldEffect::Entities { dim: d.into(), pos: p, entities: entities.to_vec() });
        true
    }
    fn explode(&mut self, dim: &str, at: [f64; 3], power: f32, fire: bool, breaks: bool) -> bool {
        self.critical.push_back(WorldEffect::Explosion { dim: dim.into(), at, power, fire, breaks });
        true
    }
    fn pile_explode(&mut self, d: &str, p: [f64; 3]) -> bool {
        self.critical.push_back(WorldEffect::PileExplosion {
            dim: d.into(),
            pos: p,
        });
        true
    }
    fn fel_fire(&mut self, p: &str, ticks: u32) {
        let p = p.to_string();
        self.effects.push((self.origins.clone(), Box::new(move |h| h.fel_fire(&p, ticks))));
    }
    fn hurt(&mut self, p: &str, a: f32, k: &str) {
        let (p, k) = (p.to_string(), k.to_string());
        self.effects.push((self.origins.clone(),Box::new(move |h| h.hurt(&p, a, &k))));
    }
    fn effect(&mut self, p: &str, e: &str, t: u32, a: u8) {
        let (p, e) = (p.to_string(), e.to_string());
        self.effects.push((self.origins.clone(),Box::new(move |h| h.effect(&p, &e, t, a))));
    }
    fn show(&mut self, d: &str, s: &crate::blast::visuals::Show) {
        let (d, s) = (d.to_string(), s.clone());
        self.effects.push((self.origins.clone(),Box::new(move |h| h.show(&d, &s))));
    }
    fn radiation_factor(&mut self, id: &str) -> f64 {
        self.host.radiation_factor(id)
    }
    fn player_dose(&mut self, id: &str, legacy: &crate::radiation::Dose) -> Option<crate::radiation::Dose> {
        match self.doses.get(id) {
            Some(staged) => Some(staged.clone()),
            None => self.host.player_dose(id, legacy),
        }
    }
    fn commit_player_dose(&mut self, id: &str, expected: &crate::radiation::Dose, desired: &crate::radiation::Dose) -> bool {
        self.doses.insert(id.into(), desired.clone());
        let (id, expected, desired) = (id.to_string(), expected.clone(), desired.clone());
        self.effects.push((self.origins.clone(),Box::new(move |h| {
            if !h.commit_player_dose(&id, &expected, &desired) {
                h.log(&format!("ntm: the dose of {id} changed under a machine step; its write was dropped"));
            }
        })));
        true
    }
    fn player_irradiation(&mut self, id: &str, legacy: &crate::radiation::Dose) -> Option<crate::zirnox::StoredPotion> {
        match self.irradiations.get(id) {
            Some(staged) => Some(*staged),
            None => self.host.player_irradiation(id, legacy),
        }
    }
    fn commit_player_irradiation(&mut self, id: &str, legacy: &crate::radiation::Dose, expected: crate::zirnox::StoredPotion, desired: crate::zirnox::StoredPotion) -> bool {
        self.irradiations.insert(id.into(), desired);
        let (id, legacy) = (id.to_string(), legacy.clone());
        self.effects.push((self.origins.clone(),Box::new(move |h| {
            if !h.commit_player_irradiation(&id, &legacy, expected, desired) {
                h.log(&format!("ntm: the radiation potion of {id} changed under a machine step; its write was dropped"));
            }
        })));
        true
    }
    fn player_digamma(&mut self, id: &str, legacy: &crate::radiation::Dose) -> Option<f32> {
        match self.digammas.get(id) {
            Some(staged) => Some(*staged),
            None => self.host.player_digamma(id, legacy),
        }
    }
    fn commit_player_digamma(&mut self, id: &str, legacy: &crate::radiation::Dose, expected: f32, desired: f32) -> bool {
        self.digammas.insert(id.into(), desired);
        let (id, legacy) = (id.to_string(), legacy.clone());
        self.effects.push((self.origins.clone(),Box::new(move |h| {
            if !h.commit_player_digamma(&id, &legacy, expected, desired) {
                h.log(&format!("ntm: the digamma of {id} changed under a machine step; its write was dropped"));
            }
        })));
        true
    }
    fn max_health(&mut self, id: &str) -> Option<f64> {
        self.host.max_health(id)
    }
    fn worn_armor(&mut self, id: &str) -> Option<[Option<String>; 4]> {
        self.host.worn_armor(id)
    }
    fn effect_state(&mut self, id: &str, effect: &str) -> Option<Option<(u32, u8)>> {
        self.host.effect_state(id, effect)
    }
    fn remove_effect(&mut self, id: &str, effect: &str) {
        let (id, effect) = (id.to_string(), effect.to_string());
        self.effects.push((self.origins.clone(),Box::new(move |h| h.remove_effect(&id, &effect))));
    }
}

impl Server {
    fn recover_retentions(&mut self,host:&mut impl ServerHost)->bool{
        if self.ready.world.prepared_retentions.is_empty(){return true;}
        let before=self.ready.world.clone();
        for _ in 0..before.prepared_retentions.len(){
            let prepared=self.ready.world.prepared_retentions.pop_front().unwrap();
            let g=&prepared.source.guard;
            if g.generation==u64::MAX{self.ready.world.prepared_retentions.push_back(prepared);continue;}
            let Some(actual)=host.cell_snapshot(&g.dim,g.pos)else{self.ready.world.prepared_retentions.push_back(prepared);continue};
            // Whatever the cell became, its contents are not this journal's to put in the world: the step that
            // consumes a cell puts them in the ledger of its chunk in the same record, and an intact cell still owns them.
            let _ = (actual, &prepared);
        }
        if self.ready.world.prepared_retentions==before.prepared_retentions && self.ready.world.machine_effects==before.machine_effects{return true;}
        match self.write_world(host){
            Ok(())=>true,
            Err(e)=>{*self.ready.world.prepared_retentions=(*before.prepared_retentions).clone();*self.ready.world.machine_effects=(*before.machine_effects).clone();host.log(&format!("retention recovery checkpoint: {e}"));false}
        }
    }

    /// Whether the kept world changed since it was last written: a part's stamp moved. Debug builds hold the answer
    /// against the whole state serialized: a change that left the stamps behind would leave the storage behind the
    /// world.
    pub(super) fn world_changed(&self) -> bool {
        self.world_part_changed() || self.rbmk_changed() || self.sickness_changed() || self.chunk_radiation_changed() || self.annihilator_changed()
    }

    /// Whether the RBMK layers changed since they were last written.
    pub(super) fn rbmk_changed(&self) -> bool {
        self.ready.kept_rbmk != Some(self.ready.world.rbmk_stamp())
    }

    /// Whether the kept world but the RBMK layers changed since it was last written.
    pub(super) fn world_part_changed(&self) -> bool {
        let moved = self.ready.kept_rev != Some(self.ready.world.stamps());
        #[cfg(debug_assertions)]
        if !moved {
            assert!(self.ready.world.kept(0) == self.ready.kept, "the kept world changed without moving its stamps");
        }
        moved
    }

    /// The kept world to the host's storage, with its clock.
    pub(super) fn write_world(&mut self, host: &mut impl ServerHost) -> Result<(), String> {
        host.set(crate::persist::KEY, &self.ready.world.kept(self.ready.world.ticks))?;
        self.ready.kept_rev = Some(self.ready.world.stamps());
        self.write_rbmk(host)?;
        self.write_sickness(host)?;
        self.write_chunk_radiation(host)?;
        self.write_annihilator(host)?;
        #[cfg(debug_assertions)]
        {
            self.ready.kept = self.ready.world.kept(0);
        }
        Ok(())
    }

    /// The RBMK layers to the host's storage, under their own key: a reactor that steps rewrites only them.
    pub(super) fn write_rbmk(&mut self, host: &mut impl ServerHost) -> Result<(), String> {
        if self.rbmk_changed() {
            host.set(crate::persist::RBMK_KEY, &self.ready.world.kept_rbmk())?;
            host.set(crate::persist::RBMK_RAYS_KEY, &self.ready.world.kept_rbmk_rays())?;
            self.ready.kept_rbmk = Some(self.ready.world.rbmk_stamp());
        }
        Ok(())
    }

    /// Whether the sickness random changed since it was last written.
    pub(super) fn sickness_changed(&self) -> bool {
        self.ready.kept_sickness != Some(self.ready.world.sickness_stamp())
    }

    /// The sickness random to the host's storage, under its own key: a tick with someone sick rewrites only it.
    pub(super) fn write_sickness(&mut self, host: &mut impl ServerHost) -> Result<(), String> {
        if self.sickness_changed() {
            host.set(crate::persist::SICKNESS_KEY, &self.ready.world.kept_sickness())?;
            self.ready.kept_sickness = Some(self.ready.world.sickness_stamp());
        }
        Ok(())
    }

    /// Whether the chunks' saved radiation changed since it was last written.
    pub(super) fn chunk_radiation_changed(&self) -> bool {
        self.ready.kept_chunk_radiation != Some(self.ready.world.chunk_radiation_stamp())
    }

    /// The chunks' saved radiation to the host's storage, under its own key: it changes when a chunk is saved.
    pub(super) fn write_chunk_radiation(&mut self, host: &mut impl ServerHost) -> Result<(), String> {
        if self.chunk_radiation_changed() {
            host.set(crate::persist::CHUNK_RADIATION_KEY, &self.ready.world.kept_chunk_radiation())?;
            self.ready.kept_chunk_radiation = Some(self.ready.world.chunk_radiation_stamp());
        }
        Ok(())
    }

    pub(super) fn annihilator_changed(&self) -> bool {
        self.ready.kept_annihilator != Some(self.ready.world.annihilator_stamp())
    }

    pub(super) fn write_annihilator(&mut self, host: &mut impl ServerHost) -> Result<(), String> {
        if self.annihilator_changed() {
            host.set(crate::persist::ANNIHILATOR_KEY, &self.ready.world.kept_annihilator())?;
            self.ready.kept_annihilator = Some(self.ready.world.annihilator_stamp());
        }
        Ok(())
    }

    fn prune_annihilator_pending(&mut self) {
        let world_ann = &self.ready.world.annihilator;
        for (k, m) in &mut self.ready.world.machines {
            if let crate::machine::Data::Annihilator(a) = &mut m.data {
                let confirmed = world_ann.applied_seq(k, m.generation);
                a.pending.retain(|p| p.seq > confirmed);
            }
        }
    }

    /// The SILEXes of the step's group take the wavelength their FEL gave them (NULL for none) before they tick.
    fn silex_lasers_in(&mut self, keys: &[Key]) {
        for k in keys {
            let laser = self.ready.silex_lasers.remove(k).unwrap_or_default();
            if let Some(crate::machine::Data::Silex(s)) = self.ready.world.machines.get_mut(k).map(|m| &mut m.data) {
                s.mode = laser;
            }
        }
    }

    /// Stage FEL beam block effects with the machine step. A world cell the host cannot read freezes the step.
    /// `outside` holds the other machines: the SILEXes the beam may reach.
    fn fel_tick(&mut self, host: &mut impl ServerHost, keys: &[Key], removed: &BTreeSet<Key>, outside: &BTreeMap<Key, crate::machine::Machine>) -> bool {
        use crate::fel::{BeamCell, BeamEffect, Wavelength};
        let silexes: Vec<(Key, Option<[i32; 3]>)> = outside
            .iter()
            .filter(|(_, m)| matches!(m.data, crate::machine::Data::Silex(_)))
            .map(|(k, m)| (k.clone(), m.facing.as_deref().and_then(registry::facing_dir)))
            .collect();
        let mut victims = Vec::new();
        let mut lasers_set = Vec::new();
        for key in keys {
            if removed.contains(key) { continue; }
            let Some(machine) = self.ready.world.machines.get_mut(key) else { continue };
            let Some(dir) = machine.facing.as_deref().and_then(registry::facing_dir) else { continue };
            let crate::machine::Data::Fel(fel) = &mut machine.data else { continue };
            let previous_distance = fel.distance;
            let Some(mode) = fel.begin_tick() else { continue };
            let at = |i: i32| [key.1[0] + dir[0]*i, key.1[1]+1, key.1[2] + dir[2]*i];
            let mut missing = false;
            let mut random = crate::rbmk::jrandom::JavaRandom::new((self.tick as i64) ^ i64::from(key.1[0]).wrapping_mul(341873128712) ^ i64::from(key.1[2]).wrapping_mul(132897987541));
            let lasers = &self.ready.silex_lasers;
            let mut cores = BTreeMap::new();
            let effects = fel.trace_beam(mode, |i| {
                let p = at(i);
                let Some(block) = host.get_block(&key.0, p) else { missing = true; return BeamCell::Transparent };
                let id = blockstate::id_of(&block);
                // `b == ModBlocks.machine_silex`: a part of a SILEX (its 3x3x3 round the core's bottom centre). Its
                // `TileEntitySILEX` is looked for one cell on, at the FEL's height (`TileEntityFEL:156`): there or not
                // at all, and with it the rotation check (`rotationIsValid`: along the beam either way) and its mode.
                if matches!(id, "ntm:machine_silex" | "ntm:dummy_machine")
                    && silexes.iter().any(|(c, _)| c.0 == key.0 && (p[0] - c.1[0]).abs() <= 1 && (p[2] - c.1[2]).abs() <= 1 && (0..=2).contains(&(p[1] - c.1[1])))
                {
                    let probe = [p[0] + dir[0], key.1[1], p[2] + dir[2]];
                    let core = silexes.iter().find(|(c, _)| c.0 == key.0 && c.1 == probe).map(|(c, facing)| {
                        cores.insert(i, c.clone());
                        let valid = facing.is_some_and(|f| f == dir || f == [-dir[0], -dir[1], -dir[2]]);
                        (valid, lasers.get(c).copied().unwrap_or_default())
                    });
                    return BeamCell::Silex { core };
                }
                let Some(opaque) = host.fel_opaque(&key.0, p) else { missing = true; return BeamCell::Transparent };
                if !opaque && id != "minecraft:tnt" { return BeamCell::Transparent; }
                let Some(resistance) = host.block_resistance(&key.0, p) else { missing = true; return BeamCell::Transparent };
                BeamCell::Obstacle { liquid: matches!(id, "minecraft:water" | "minecraft:lava"), resistance }
            }, || random.next_int(5) == 0);
            if missing { return false; }
            let mut blocks = Vec::new();
            let mut drops: Vec<(Pos, &str)> = Vec::new();
            for effect in effects {
                match effect {
                    BeamEffect::RemoveLiquid { at: i } => blocks.push((at(i), "minecraft:air".into())),
                    BeamEffect::Ignite { at: i, digamma: false } => blocks.push((at(i), "minecraft:fire".into())),
                    // TileEntityFEL:187-190: the burnt block becomes digamma fire (matter with polaroid 11), the one
                    // under it digamma ash.
                    BeamEffect::Ignite { at: i, digamma: true } => {
                        blocks.push((at(i), crate::digamma::drx_block(self.polaroid).into()));
                        let [x, y, z] = at(i);
                        blocks.push(([x, y - 1, z], crate::digamma::ASH_BLOCK.into()));
                    }
                    BeamEffect::SetSilex { at: i, mode } => {
                        if let Some(c) = cores.get(&i) { lasers_set.push((c.clone(), mode)); }
                    }
                    // `silexBlock.breakBlock(...)` (the whole SILEX goes, its items spilled) and a SILEX item at the
                    // struck cell.
                    BeamEffect::BreakSilex { at: i } => {
                        let Some(c) = cores.get(&i) else { continue };
                        for dx in -1..=1 { for dy in 0..=2 { for dz in -1..=1 {
                            let cell = [c.1[0] + dx, c.1[1] + dy, c.1[2] + dz];
                            if host.get_block(&key.0, cell).is_some_and(|b| matches!(blockstate::id_of(&b), "ntm:machine_silex" | "ntm:dummy_machine")) {
                                blocks.push((cell, "minecraft:air".into()));
                            }
                        }}}
                        drops.push((at(i), crate::silex::BLOCK));
                    }
                }
            }
            if !blocks.is_empty() && host.set_blocks(&key.0, &blocks).is_err() { return false; }
            for (p, item) in drops {
                if !host.drop_items(&key.0, p, &[(item.to_string(), 1)]) { return false; }
            }
            victims.extend(self.players.iter().filter(|p| p.dim == key.0 && crate::fel::beam_intersects_player(key.1, dir, previous_distance, p.pos)).map(|p| (p.clone(), mode)));
        }
        self.ready.silex_lasers.extend(lasers_set);
        for (p, mode) in victims {
            let (id, creative) = (p.id.clone(), p.creative);
            match mode {
                Wavelength::Visible => { host.effect(&id, "minecraft:blindness", 60*60*65536, 0); host.fel_fire(&id, 200); }
                Wavelength::Ir | Wavelength::Uv => host.fel_fire(&id, 200),
                Wavelength::Gamma => {
                    self.note_contamination(&id, 25_000);
                    if !creative && !self.in_grace(&id) { self.expose_player(host, &id, 25_000); }
                }
                Wavelength::Drx => { self.digamma_dose(host, &p, crate::digamma::FEL_DRX); }
                Wavelength::Null => {}
            }
        }
        true
    }

    /// The radiation a drain released this tick (`FT_VentRadiation.onFluidRelease`) goes to its chunk.
    fn drain_tick(&mut self, keys: &[Key], removed_keys: &BTreeSet<Key>) {
        for k in keys {
            if removed_keys.contains(k) || self.ready.broken.contains_key(k) {
                continue;
            }
            let Some(crate::machine::Data::Drain(d)) = self.ready.world.machines.get_mut(k).map(|m| &mut m.data) else { continue };
            let rad = std::mem::take(&mut d.rad);
            if rad > 0.0 {
                self.ready.chunk_rad_pending.push((k.0.clone(), k.1, rad));
            }
        }
    }

    fn annihilator_tick(&mut self, keys: &[Key], removed_keys: &BTreeSet<Key>) {
        let recipes = crate::annihilator::recipes();
        for k in keys {
            if removed_keys.contains(k) || self.ready.broken.contains_key(k) {
                continue;
            }
            let Some(m) = self.ready.world.machines.get_mut(k) else { continue };
            let crate::machine::Data::Annihilator(a) = &mut m.data else { continue };
            let confirmed = self.ready.world.annihilator.applied_seq(k, m.generation);
            let rads = a.step_world(
                &mut self.ready.annihilator,
                k,
                m.generation,
                m.facing.as_deref(),
                confirmed,
                recipes,
            );
            for (pos, amount) in rads {
                self.ready.chunk_rad_pending.push((k.0.clone(), pos, amount));
            }
        }
    }

    pub(super) fn track_effect_cells(&mut self) {
        let guards:Vec<_>=self.ready.world.machine_effects.iter().chain(&self.ready.world.applied_machine_effects).flat_map(|b|b.origins.iter().chain(b.effects.iter().flat_map(|e|match e{WorldEffect::Blocks{expected,..}=>expected.as_slice(),_=>&[]}))).cloned().collect();
        // Only the cells not named yet: an entry the map already has would stamp it every tick for nothing.
        let mut new:Vec<_>=guards.iter().filter(|g|!self.ready.world.generations.contains_key(&g.key())).map(|g|(g.key(),g.generation)).collect();
        new.extend(self.ready.world.prepared_retentions.iter().map(|p|&p.source.guard).filter(|g|!self.ready.world.generations.contains_key(&g.key())).map(|g|(g.key(),g.generation)));
        for (k,generation) in new{self.ready.world.generations.entry(k).or_insert(generation);}
    }

    /// Keeps the state a machine was left in by a journaled step: the one of the highest revision.
    pub(super) fn journal_survivor(&mut self,s:Survivor){
        let key=(s.dim.clone(),s.pos);
        if self.ready.world.survivors.get(&key).is_none_or(|old|old.generation!=s.generation||old.rev<=s.rev){self.ready.world.survivors.insert(key,s);}
    }

    /// Gives every queued batch that has none its serial (the counter is kept with the journal).
    pub(super) fn name_batches(&mut self){
        for i in 0..self.ready.world.machine_effects.len(){
            if self.ready.world.machine_effects[i].serial==0{
                // A count kept with the storage may go back with it while a chunk's record still names a batch: the
                // server's nonce keeps every name its own (R6-1 of the sixth review).
                let serial=self.batch_serial();
                self.ready.world.machine_effects[i].serial=serial;
            }
        }
    }

    /// Machines an older chunk showed although a receipt consumed them: the break is carried through, the block and
    /// its block entity go and nothing is released (the receipt did that).
    pub(super) fn settle_ghosts(&mut self, host: &mut impl ServerHost) {
        for (key,g) in std::mem::take(&mut self.ready.ghosts) {
            let Some(actual)=host.get_block(&key.0,key.1) else{self.ready.ghosts.push((key,g));continue};
            if blockstate::id_of(&actual)!=blockstate::id_of(&g.state){continue;}
            let Some(snapshot)=host.cell_snapshot(&key.0,key.1) else{self.ready.ghosts.push((key,g));continue};
            let cell=CellWrite{notify:false,initial:None,guard:g.clone(),expected:snapshot,desired:"minecraft:air".into(),destroy:true};
            if !host.commit_structural(&[cell],&[],std::slice::from_ref(&key)){self.ready.ghosts.push((key,g));}
        }
    }

    pub(super) fn flush_machine_effects(&mut self, host: &mut impl ServerHost) {
        // External facts must invalidate old writes before the first retry.
        if !self.queued.is_empty(){self.native_record_events(host);}
        if self.ready.facts_pending || !self.recover_retentions(host){return;}
        let batches=self.ready.world.machine_effects.len();
        if batches==0{return;}
        // No journal is written ahead of an effect: a batch is in its home chunk's record (`obligations`), given it
        // within the tick, and no save comes between two effects of a tick. Every queued batch is named first, so the
        // ids of a batch's effects are the same in whatever order the batches go out.
        self.name_batches();
        for _ in 0..batches {
            while let Some(batch)=self.ready.world.machine_effects.front() {
                // What a chunk owes is played only while it is loaded: its record then follows every step.
                if let Some(h)=batch.home()&&!host.home_loaded(&h.0,[h.1,h.2]){self.ready.world.machine_effects.rotate_left(1);break;}
                let Some(mut effect)=batch.effects.front().cloned() else{self.ready.world.machine_effects.pop_front();break};
                // A later item/entity effect in its destination's record proves every earlier effect ran in order.
                // The home's older record may still owe the explosion, which has no persistent mark of its own.
                // Advance without a new receipt: replaying it now could destroy the very item that proves it ran.
                let later_seen = batch.replayed && matches!(effect, WorldEffect::Blocks { .. } | WorldEffect::Explosion { .. } | WorldEffect::PileExplosion { .. }) && batch.effects.iter().enumerate().skip(1).any(|(k, later)| {
                    let (dim, pos) = match later {
                        WorldEffect::Items { dim, pos, .. } | WorldEffect::EncodedItems { dim, pos, .. }
                        | WorldEffect::Entities { dim, pos, .. } => (dim, pos),
                        _ => return false,
                    };
                    host.effect_seen(crate::ledger::effect_id(batch.serial, batch.done + k as u32), dim, *pos)
                });
                if later_seen {
                    let batch = self.ready.world.machine_effects.front_mut().unwrap();
                    batch.effects.pop_front();
                    batch.done += 1;
                    continue;
                }
                let dim=match &effect{WorldEffect::Blocks{dim,..}|WorldEffect::Items{dim,..}|WorldEffect::EncodedItems{dim,..}|WorldEffect::Explosion{dim,..}|WorldEffect::PileExplosion{dim,..}|WorldEffect::Entities{dim,..}=>dim};
                if !host.pile_reachable(dim){self.ready.world.machine_effects.rotate_left(1);break;}
                let (effect_id,replayed)={let b=self.ready.world.machine_effects.front().unwrap();(crate::ledger::effect_id(b.serial,b.done),b.replayed)};
                let mut wait=false;
                let mut physical=false;
                let mut cells=Vec::new();
                if let WorldEffect::Blocks{dim,blocks,expected,retained}=&mut effect {
                    let mut keep=Vec::new();let mut guards=Vec::new();
                    for (pos,desired) in &*blocks {
                        let Some(g)=expected.iter().find(|g|g.pos==*pos&&g.dim==*dim) else{continue};
                        // The chunk's own record says whether it already holds this effect (it was saved after it).
                        if host.effect_seen(effect_id,dim,*pos){continue;}
                        // A chunk older than the storage still has the very machine the effect consumed (its block entity
                        // is of the same generation), whatever the generations since say: its record does not hold the
                        // effect, so the effect is due there. A machine placed anew has a generation of its own.
                        let stale_chunk=replayed&&host.get_block(dim,*pos).is_some_and(|b|blockstate::id_of(&b)==blockstate::id_of(&g.state))&&host.cell_generation(dim,*pos)==Some(g.generation);
                        if !self.current_guard(g)&&!stale_chunk{continue;}
                        match host.get_block(dim,*pos){
                            None=>{wait=true;keep.push((*pos,desired.clone()));guards.push(g.clone());}
                            Some(actual) if blockstate::id_of(&actual)==blockstate::id_of(&g.state) || blockstate::id_of(&actual)==blockstate::id_of(desired)=>{
                                let Some(snapshot)=host.cell_snapshot(dim,*pos) else{wait=true;keep.push((*pos,desired.clone()));guards.push(g.clone());continue};
                                // Refresh only a still-existing source. A journal
                                // replay whose source is already gone retains its
                                // old physical drop rather than silently losing it.
                                if blockstate::id_of(&actual)==blockstate::id_of(&g.state){
                                    retained.retain(|d|d.origin.key()!=g.key());
                                    if blockstate::id_of(&actual)!=blockstate::id_of(desired)&&!snapshot.items.is_empty(){retained.push(RetainedDrop{origin:g.clone(),items:snapshot.items.clone(),id:None});}
                                }
                                cells.push(CellWrite{notify:false,initial:None,guard:g.clone(),expected:snapshot,desired:desired.clone(),destroy:false});
                                physical=true;keep.push((*pos,desired.clone()));guards.push(g.clone());
                            }
                            Some(_)=>{},
                        }
                    }
                    // A source may have been consumed elsewhere while another chunk kept this effect waiting.
                    // Its old snapshot no longer belongs to this effect, even when it was retained on an earlier pass.
                    retained.retain(|d| guards.contains(&d.origin));
                    *blocks=keep;*expected=guards;
                    physical|=!retained.is_empty();
                }else{
                    // Confirmed items and explosions belong to the old origin,
                    // independently of whatever occupies its position now.
                    physical=true;
                }
                if wait{self.ready.world.machine_effects.front_mut().unwrap().effects[0]=effect;self.ready.world.machine_effects.rotate_left(1);break;}
                self.ready.world.machine_effects.front_mut().unwrap().effects[0]=effect.clone();
                if host.trace_effects(){
                    let b=self.ready.world.machine_effects.front().unwrap();
                    let kind=match &effect{WorldEffect::Blocks{blocks,..}=>format!("blocks {}",blocks.len()),WorldEffect::Items{pos,items,..}=>format!("items {pos:?} {items:?}"),WorldEffect::EncodedItems{pos,items,..}=>format!("encoded {pos:?} {}",items.len()),WorldEffect::Explosion{at,power,fire,breaks,..}=>format!("explosion {at:?} power={power} fire={fire} breaks={breaks}"),WorldEffect::PileExplosion{..}=>"explosion".into(),WorldEffect::Entities{pos,entities,..}=>format!("entities {pos:?} {}",entities.len())};
                    host.log(&format!("effect {}.{} {kind} owners={} origins={}",b.serial,b.done,b.owners.len(),b.origins.len()));
                }
                let done=match &effect {
                    WorldEffect::Blocks{retained,..}=>cells.is_empty()&&retained.is_empty()||{
                        let applied=host.commit_structural(&cells,retained,&self.ready.world.machine_effects.front().unwrap().owners);
                        if applied{let at:Vec<_>=cells.iter().map(|c|(c.guard.dim.clone(),c.guard.pos)).collect();host.effect_applied(effect_id,&at);}
                        applied
                    },
                    WorldEffect::Items{dim,pos,items}=>host.apply_items(effect_id,dim,*pos,items,&self.ready.world.machine_effects.front().unwrap().owners),
                    WorldEffect::EncodedItems{dim,pos,items}=>host.apply_encoded_items(effect_id,dim,*pos,items,&self.ready.world.machine_effects.front().unwrap().owners),
                    WorldEffect::Explosion{dim,at,power,fire,breaks}=>host.explode(dim,*at,*power,*fire,*breaks),
                    WorldEffect::PileExplosion{dim,pos}=>host.pile_explode(dim,*pos),
                    WorldEffect::Entities{dim,pos,entities}=>host.apply_entities(effect_id,dim,*pos,entities,&self.ready.world.machine_effects.front().unwrap().owners),
                };
                if !done{
                    // A rejected atomic publication emitted no item. Discard
                    // its captured inventory; the next attempt observes it anew.
                    if let Some(WorldEffect::Blocks{retained,..})=self.ready.world.machine_effects.front_mut().and_then(|b|b.effects.front_mut()){retained.clear();}
                    self.ready.world.machine_effects.rotate_left(1);break;
                }
                let batch=self.ready.world.machine_effects.front_mut().unwrap();
                let effect=batch.effects.pop_front().unwrap();
                let (serial,done)=(batch.serial,batch.done);
                batch.done+=1;
                if physical {
                    // The receipt keeps the effect's place in its batch, so a replay derives the same id.
                    let receipt=CommittedEffects{serial,done,origins:batch.origins.clone(),owners:batch.owners.clone(),tombstones:Vec::new(),effects:std::collections::VecDeque::from([effect]),replayed:false};
                    if matches!(receipt.effects.front(),Some(WorldEffect::Blocks{..})){
                        for g in &receipt.origins {
                            if self.ready.world.machines.get(&g.key()).is_some_and(|m|m.generation==g.generation){self.ready.native_saved.remove(&g.key());}
                        }
                    }
                    self.ready.world.applied_machine_effects.push_back(receipt);
                }
            }
        }
        if self.write_world(host).is_ok(){self.prune_ledger(host);}else{self.ready.kept_rev=None;}
    }

    /// Every effect id the stored state still names: batches waiting, receipts waiting for a flushed save,
    /// retentions prepared. The ledgers keep a retired id only while it is here.
    pub(super) fn live_effect_ids(&self)->std::collections::BTreeSet<u64>{
        let mut live=std::collections::BTreeSet::new();
        let batches=self.ready.world.machine_effects.iter().chain(&self.ready.world.applied_machine_effects).chain(self.ready.world.prepared_retentions.iter().map(|p|&p.receipt));
        for b in batches{
            if b.serial!=0{for k in 0..b.effects.len(){live.insert(crate::ledger::effect_id(b.serial,b.done+k as u32));}}
            for e in &b.effects{if let WorldEffect::Blocks{retained,..}=e{for d in retained{live.insert(crate::ledger::retained_id(&d.origin.dim,d.origin.pos,d.origin.generation));}}}
        }
        for p in &self.ready.world.prepared_retentions{let g=&p.source.guard;live.insert(crate::ledger::retained_id(&g.dim,g.pos,g.generation));}
        live
    }

    pub(super) fn prune_ledger(&self,host:&mut impl ServerHost){host.ledger_prune(&self.live_effect_ids());}

    /// The existing engine hook reports whether world/entity writes have
    /// actually flushed. Until then replay after a crash may duplicate an
    /// effect, but must never acknowledge an entity not yet saved by the world.
    pub fn native_world_saved(&mut self, host: &mut impl ServerHost, wait: bool) {
        // What each chunk owes was in its record before the save took the chunks (`native_world_saving`); nothing is
        // given to a record past that. A waited save wrote those records first: the earlier version's removals and
        // batches they took need not be kept any longer.
        self.retire_legacy(wait);
        // The receipts and survivors the save took are confirmed once its writes are on the disk: now when it waited,
        // else when the writer is done with them (`native_writes_finished`). A save without the hook before it takes
        // all of them.
        let taken = self.taken.take().unwrap_or_else(|| self.taking());
        let taken_annihilator = self.taken_annihilator.take().unwrap_or_else(|| self.ready.annihilator.clone());
        if wait {
            if *self.ready.world.annihilator != taken_annihilator {
                *self.ready.world.annihilator = taken_annihilator;
            }
            self.awaiting_annihilator = None;
            self.prune_annihilator_pending();
        } else {
            self.awaiting_annihilator = Some(taken_annihilator);
        }
        // Written at every save, the clock with it: what runs on it (a balefire's damage) comes back as of
        // this save, not as of the last keep of the clock alone.
        if let Err(e) = self.write_world(host) {
            host.log(&format!("machine effect receipts/removals: {e}"));
            return;
        }
        if wait {
            self.confirm(&taken);
            self.awaiting = None;
        } else {
            self.awaiting = Some(taken);
        }
        self.prune_ledger(host);
    }

    /// The session's receipts and survivors as they stand: what a save about to take the chunks takes with them.
    pub(super) fn taking(&self) -> Taken {
        Taken {
            receipts: self.ready.world.applied_machine_effects.len(),
            survivors: self.ready.world.survivors.iter().map(|(k, s)| (k.clone(), s.generation, s.rev)).collect(),
        }
    }

    /// The region writer has put on the disk everything queued so far: the last save that did not wait is as good as
    /// one that did for the receipts and survivors it took (the earlier version's storage keeps its own rule,
    /// `retire_legacy`). Without that, a session saved only by autosaves would keep every receipt.
    pub fn native_writes_finished(&mut self) {
        if let Some(taken) = self.awaiting.take() {
            self.confirm(&taken);
        }
        if let Some(taken) = self.awaiting_annihilator.take() {
            if *self.ready.world.annihilator != taken {
                *self.ready.world.annihilator = taken;
            }
            self.prune_annihilator_pending();
        }
    }

    fn confirm(&mut self, taken: &Taken) {
        if taken.receipts > 0 {
            let n = taken.receipts.min(self.ready.world.applied_machine_effects.len());
            self.ready.world.applied_machine_effects.drain(..n);
        }
        if !taken.survivors.is_empty() {
            let survivors = &mut self.ready.world.survivors;
            for (key, generation, rev) in &taken.survivors {
                if survivors.get(key).is_some_and(|s| s.generation == *generation && s.rev == *rev) {
                    survivors.remove(key);
                }
            }
        }
    }

    /// Block events are observed facts. Broken states are handled by the same
    /// speculative group step as missing-but-loaded blocks, never as drops
    /// before an inventory commit. Placement still installs the model first.
    pub(super) fn native_event(&mut self, host: &mut impl ServerHost, e: BlockEvent,generation:u64) {
        match e {
            BlockEvent::Broken(d,p,s)=>{
                let k=(d.clone(),p);
                if self.ready.world.machines.get(&k).is_some_and(|m|m.generation==generation){
                    self.remember_removal(&k);
                }else if !self.ready.world.machines.contains_key(&k){
                    self.block_event(host,BlockEvent::Broken(d,p,s));
                }
            }
            BlockEvent::Placed(d,p,s)=>{
                let k=(d.clone(),p);
                if self.ready.world.generations.get(&k).copied().unwrap_or(0)!=generation{return;}
                if self.ready.broken.contains_key(&k){
                    self.queued.push((BlockEvent::Placed(d,p,s),generation));
                }else{
                    self.block_event(host,BlockEvent::Placed(d,p,s));
                    if let Some(m)=self.ready.world.machines.get_mut(&k){m.generation=generation;}
                    self.sawmill_word(host,&k,generation);
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
            cells:before.world.machines.iter().map(|(k,m)|(k.clone(),CellGuard::machine(k,m))).collect(),
            vacant:before.broken.keys().cloned().collect(),
            generations:(*before.world.generations).clone(),
            origins:before.world.machines.iter().filter(|(k,_)|k.0==dim).map(|(k,m)|CellGuard::machine(k,m)).collect(),
            valid:true,
            resource:false,world_writes:Vec::new(),doses:BTreeMap::new(),irradiations:BTreeMap::new(),digammas:BTreeMap::new(),
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
        staged.origins=keys.iter().filter_map(|k|before.world.machines.get(k).map(|m|CellGuard::machine(k,m))).collect();
        for (guards,_) in &mut staged.effects{*guards=staged.origins.clone();}
        if keys.iter().any(|k| {
            before.broken.contains_key(k) || before.world.machines.get(k).is_some_and(|m|!self.model_current(k,m))
                || before.world.machines.get(k).is_some_and(|m| match &m.data {
                    crate::machine::Data::Pile(p) => match &**p {
                        crate::pile::Pile::Core(Some(c)) => before
                            .broken
                            .keys()
                            .any(|b| b.0 == k.0 && c.shape.contains(b.1)),
                        _ => false,
                    },
                    _ => false,
                })
        }) {
            self.ready = before;
            return Some(PileUse::Handled);
        }
        let mut commits = Vec::new();
        // A tool use that leaves world effects owing, or changes machines of more than one chunk, journals what it
        // leaves in them: their block entities reach the disk in records of their own chunks.
        let journal=!staged.critical.is_empty()||chunks_of(keys.iter()).len()>1;
        let mut survivors=Vec::new();
        for k in &keys {
            let Some(expected) = observed.get(k) else {
                self.ready = before;
                return Some(PileUse::Handled);
            };
            let m = self.ready.world.machines.get_mut(k).unwrap();
            let (desired, data) = Self::machine_output(m, expected);
            let mut json=serde_json::to_string(m).expect("machine serialization");
            if journal&&(self.ready.native_saved.get(k)!=Some(&json)||&desired!=expected){
                m.rev+=1;
                json=serde_json::to_string(m).expect("machine serialization");
                survivors.push(Survivor{dim:k.0.clone(),pos:k.1,generation:m.generation,rev:m.rev,json:json.clone()});
            }
            commits.push(MachineCommit {
                cells: Vec::new(),
                guard:CellGuard::machine(k,&before.world.machines[k]),
                expected_saved:before.native_saved.contains_key(k),
                dim: k.0.clone(),
                pos: k.1,
                expected: expected.clone(),
                desired,
                data,
                json,
            });
        }
        if !staged.valid || !staged.host.commit_machines(&commits) {
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
            let owners=self.owners_home_first(keys,&before.world.machines);
            self.ready
                .world
                .machine_effects
                .push_back(CommittedEffects {
                    origins:staged.origins.clone(),
                    owners,
                    effects: staged.critical,
                    tombstones: Vec::new(),
                ..Default::default()});
            self.name_batches();
        }
        for s in survivors{self.journal_survivor(s);}
        for (guards,effect) in staged.effects {
            if guards.iter().all(|g|self.current_guard(g)){effect(staged.host);}
        }
        self.track_effect_cells();
        self.flush_machine_effects(staged.host);
        self.native_save(staged.host, false);
        // A tool use comes outside the tick: what it leaves owing is in its chunk's record at once.
        self.sync_obligations(staged.host);
        outcome
    }

    pub(super) fn local_steps(&mut self, host: &mut impl ServerHost) {
        self.steps(host, false);
    }

    /// The steps of the groups whose machine, or pile's core, went (`native_settle`): their contents put out now.
    pub(super) fn removal_steps(&mut self, host: &mut impl ServerHost) {
        self.steps(host, true);
    }

    /// A group whose machine, or pile's core, went: the step that puts its contents out.
    fn group_removed(&self, keys: &[Key]) -> bool {
        keys.iter().any(|k| {
            self.ready.broken.contains_key(k)
                && (keys.len() == 1
                    || self.ready.world.machines.get(k).is_some_and(|m| {
                        matches!(&m.data, crate::machine::Data::Pile(p) if matches!(&**p, crate::pile::Pile::Core(_)))
                    }))
        })
    }

    fn steps(&mut self, host: &mut impl ServerHost, removed_only: bool) {
        if !removed_only {
            self.validate_detonations(host);
        }
        let mut buttons = std::mem::take(&mut self.ready.pending_buttons);
        for (player,dim,pos,button) in host.machine_buttons(){
            if let Some(m)=self.ready.world.machines.get(&(dim.clone(),pos)){
                buttons.push(generation::Guarded{guard:CellGuard::machine(&(dim.clone(),pos),m),value:(player,dim,pos,button)});
            }
        }
        buttons.retain(|i|self.current_guard(&i.guard)&&!self.ready.broken.contains_key(&i.guard.key()));
        let mut groups: Vec<Vec<Key>> = Vec::new();
        let mut piles: BTreeMap<Key, Vec<Key>> = BTreeMap::new();
        // Machines that wait for a batch of their own: no step takes them.
        let mut waiting: BTreeSet<Key> = BTreeSet::new();
        for (k, m) in &self.ready.world.machines {
            if self
                .ready
                .world
                .machine_effects
                .iter().chain(self.ready.world.prepared_retentions.iter().map(|p|&p.receipt))
                .any(|b| b.origins.iter().any(|g|g.key()==*k&&g.generation==m.generation))
            {
                waiting.insert(k.clone());
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
            } else if matches!(m.data, crate::machine::Data::Rbmk(_)) {
                // An RBMK reactor is one coupled group (T01), its columns found below.
            } else if host.get_block(&k.0, k.1).is_some() {
                groups.push(vec![k.clone()]);
            }
        }
        groups.extend(piles.into_values());
        groups.extend(self.rbmk_groups(host, &waiting));
        if removed_only {
            groups.retain(|keys| self.group_removed(keys));
        }
        for mut keys in groups {
            // A pile whose core is broken goes at once, as Java's `breakBlock` does: the core's rods leave from the
            // core's chunk now, as a vanilla container's items spill. Members out of reach stay out of the step and turn
            // back to bricks once they come in (the orphan sweep): no removal waits holding the rods.
            let core_broken = keys.iter().any(|k| {
                self.ready.broken.contains_key(k)
                    && host.get_block(&k.0, k.1).is_some()
                    && self.ready.world.machines.get(k).is_some_and(|m| {
                        matches!(&m.data, crate::machine::Data::Pile(p) if matches!(&**p, crate::pile::Pile::Core(_)))
                    })
            });
            if core_broken {
                keys.retain(|k| host.get_block(&k.0, k.1).is_some());
            }
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
            let radiation_env_before = keys.iter().any(|k| before.world.machines.get(k).is_some_and(|m| matches!(m.data, crate::machine::Data::Fel(_)))).then(|| self.radiation_env.clone());
            let mut observed = BTreeMap::new();
            let accessible = keys.iter().all(|k| {
                let Some(m) = self.ready.world.machines.get(k) else {
                    return false;
                };
                if !before.broken.contains_key(k)&&!self.model_current(k,m){return false;}
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
                    cells:before.world.machines.iter().map(|(k,m)|(k.clone(),CellGuard::machine(k,m))).collect(),
            vacant:before.broken.keys().cloned().collect(),
            generations:(*before.world.generations).clone(),
                    origins:before.world.machines.iter().map(|(k,m)|CellGuard::machine(k,m)).collect(),
                    valid:true,
                    resource:before.world.machines.values().any(|m|matches!(m.data,crate::machine::Data::Excavator(_)|crate::machine::Data::Drill(_))),world_writes:Vec::new(),doses:BTreeMap::new(),irradiations:BTreeMap::new(),digammas:BTreeMap::new(),
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
                let removed_keys: BTreeSet<Key> = removed.iter().cloned().collect();
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
                for input in &buttons {
                    let (_,d,p,b)=&input.value;
                    if keys.contains(&(d.clone(), *p)) {
                        self.machine_button(&(d.clone(), *p), *b);
                    }
                }
                self.pile_tick(&mut staged);
                self.zirnox_tick(&mut staged);
                if !self.rbmk_tick(&mut staged, &removed_keys) {
                    staged.valid = false;
                }
                self.requested_detonations(&mut staged);
                self.silex_lasers_in(&keys);
                for (key, m) in &mut self.ready.world.machines {
                    m.logic_mut().tick();
                    staged.critical.extend(m.logic_mut().world_effects().into_iter().map(|e| e.in_world(key, m.facing.as_deref())));
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
                if !self.fel_tick(&mut staged, &keys, &removed_keys, &outside) {
                    staged.valid = false;
                }
                self.annihilator_tick(&keys, &removed_keys);
                self.drain_tick(&keys, &removed_keys);
                self.excavator_tick(&mut staged);
                self.drill_tick(&mut staged);
                self.bombs_triggers(&mut staged);
                let mut commits = Vec::new();
                let mut changed_keys:Vec<Key>=Vec::new();
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
                    let changed=last != &want
                        || self.ready.native_saved.get(k) != Some(&json)
                        || self.ready.bridge.data.get(k) != Some(&data);
                    if changed{changed_keys.push(k.clone());}
                    // Static pile members need not rewrite their BE every
                    // tick. Keep at least one accessible member in a no-op
                    // group, so an unsupported atomic host still freezes it.
                    if commits.is_empty() || changed {
                        commits.push(MachineCommit {
                cells: Vec::new(),
                            guard:CellGuard::machine(k,&before.world.machines[k]),
                            expected_saved:before.native_saved.contains_key(k),
                            dim: k.0.clone(),
                            pos: k.1,
                            expected: last.clone(),
                            desired: want,
                            data,
                            json,
                        });
                    }
                }
                if !staged.world_writes.is_empty(){
                    if let Some(c)=commits.first_mut(){c.cells=staged.world_writes.clone();}else{staged.valid=false;}
                }
                // A step that leaves world effects owing, or changes machines of more than one chunk, journals what it
                // leaves in the machines that changed: their block entities reach the disk in records of their own chunks,
                // and a save can end between two of them. The consumed cells' contents need no journal of their own:
                // the host puts them in the ledger of their chunk in the very step that consumes the cell.
                let mut survivors=Vec::new();
                if !staged.critical.is_empty()||chunks_of(changed_keys.iter()).len()>1{
                    for c in &mut commits{
                        let k=(c.dim.clone(),c.pos);
                        if changed_keys.contains(&k)&&let Some(m)=self.ready.world.machines.get_mut(&k){
                            m.rev+=1;
                            c.json=serde_json::to_string(m).expect("machine serialization");
                            survivors.push(Survivor{dim:k.0.clone(),pos:k.1,generation:m.generation,rev:m.rev,json:c.json.clone()});
                        }
                    }
                }
                accepted = staged.valid && staged.host.commit_machines(&commits);
                if accepted {
                    for s in survivors{self.journal_survivor(s);}
                    for w in &staged.world_writes {
                        if w.desired!=w.expected.state&&!self.ready.world.generations.contains_key(&w.guard.key()){self.ready.world.generations.insert(w.guard.key(),w.guard.generation);}
                    }
                    buttons.retain(|i| !keys.contains(&i.guard.key()));
                    for k in &keys {
                        self.ready.broken.remove(k);
                        if self.ready.world.removals.iter().any(|r| &r.key == k) {
                            self.ready.world.removals.retain(|r| &r.key != k);
                        }
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
                    let tombstones: Vec<_> = keys
                        .iter()
                        .filter(|k| !self.ready.world.machines.contains_key(*k))
                        .map(|k| (k.clone(), before.world.machines[k].clone()))
                        .collect();
                    if !staged.critical.is_empty() {
                        if staged.host.trace_effects(){staged.host.log(&format!("batch created owners={} origins={} critical={} tombstones={}",keys.len(),staged.origins.len(),staged.critical.len(),tombstones.len()));}
                        let owners=self.owners_home_first(keys.clone(),&before.world.machines);
                        self.ready
                            .world
                            .machine_effects
                            .push_back(CommittedEffects {
                                origins:staged.origins.clone(),
                                owners,
                                effects: staged.critical,
                                tombstones,
                            ..Default::default()});
                        self.name_batches();
                    }
                    for (guards,effect) in staged.effects {
                        if guards.iter().all(|g|self.current_guard(g)){effect(staged.host);}
                    }
                }
            }
            if !accepted {
                self.ready = before;
                self.blasts.lit = lit_before;
                if let Some(env) = radiation_env_before { self.radiation_env = env; }
                // No simulation or network exchange while the BE is missing
                // or its atomic host rejects the step. A future tick retries.
                for k in &keys {
                    self.ready.loaded.insert(k.clone(), false);
                    self.ready.frozen.insert(k.clone());
                }
            }
            self.ready.world.machines.append(&mut outside);
        }
        buttons.retain(|i| self.ready.world.machines.get(&i.guard.key()).is_some_and(|m|m.generation==i.guard.generation));
        self.ready.pending_buttons = buttons;
        self.track_effect_cells();
    }
}

#[cfg(test)]
mod dose_tests {
    use super::*;
    use crate::radiation::Dose;

    #[derive(Default)]
    struct FelWorld { unavailable: bool, stone: bool, writes: Vec<(Pos, String)>, fire: Vec<(String, u32)>, effects: Vec<(String, String, u32)>, digamma: BTreeMap<String, f32>, parts: BTreeMap<Pos, String>, dropped: Vec<(Pos, String, u32)> }
    impl Kv for FelWorld {
        fn get(&mut self, _: &str) -> Option<Vec<u8>> { None }
        fn set(&mut self, _: &str, _: &[u8]) -> Result<(), String> { Ok(()) }
        fn delete(&mut self, _: &str) {}
    }
    impl ServerHost for FelWorld {
        fn get_block(&mut self, _: &str, p: Pos) -> Option<String> {
            if let Some(b) = self.parts.get(&p) { return Some(b.clone()); }
            if self.unavailable { None } else if p == [0, 65, -5] { Some(if self.stone { "minecraft:stone" } else { "minecraft:water" }.into()) } else { Some("minecraft:air".into()) }
        }
        fn player_digamma(&mut self, id: &str, _: &Dose) -> Option<f32> { Some(self.digamma.get(id).copied().unwrap_or(0.0)) }
        fn commit_player_digamma(&mut self, id: &str, _: &Dose, expected: f32, desired: f32) -> bool {
            if self.digamma.get(id).copied().unwrap_or(0.0) != expected { return false; }
            self.digamma.insert(id.into(), desired);
            true
        }
        fn fel_opaque(&mut self, _: &str, p: Pos) -> Option<bool> { Some(self.parts.get(&p).map_or(p == [0, 65, -5], |b| b != "minecraft:air")) }
        fn drop_items(&mut self, _: &str, p: Pos, items: &[(String, u32)]) -> bool {
            self.dropped.extend(items.iter().map(|(i, n)| (p, i.clone(), *n)));
            true
        }
        fn block_resistance(&mut self, _: &str, _: Pos) -> Option<f32> { Some(1.0) }
        fn set_blocks(&mut self, _: &str, blocks: &[(Pos, String)]) -> Result<(), String> { self.writes.extend_from_slice(blocks); Ok(()) }
        fn tell(&mut self, _: &str, _: &str) {}
        fn log(&mut self, _: &str) {}
        fn fel_fire(&mut self, id: &str, ticks: u32) { self.fire.push((id.into(), ticks)); }
        fn effect(&mut self, id: &str, effect: &str, ticks: u32, _: u8) { self.effects.push((id.into(), effect.into(), ticks)); }
    }
    /// A FEL facing north at (0, 64, 0) with a DRX crystal and the power of one tick, and a SILEX facing `silex_facing` whose
    /// core is at (0, 64, z): its 27 cells are SILEX parts.
    fn fel_and_silex(silex_facing: &str, z: i32) -> (FelWorld, Server, Key, Key, BTreeMap<Key, Machine>) {
        let mut host = FelWorld::default();
        let core = ("minecraft:overworld".to_string(), [0, 64, z]);
        for dx in -1..=1 { for dy in 0..=2 { for dz in -1..=1 {
            let cell = [dx, 64 + dy, z + dz];
            host.parts.insert(cell, if cell == core.1 { crate::silex::BLOCK } else { "ntm:dummy_machine" }.into());
        }}}
        // No water in the way (FelWorld's at z -5) unless the SILEX is there.
        host.parts.entry([0, 65, -5]).or_insert_with(|| "minecraft:air".into());
        let server = Server::native(&mut host, 0);
        let key = ("minecraft:overworld".to_string(), [0, 64, 0]);
        let mut fel = Machine::new(Kind::Fel, crate::fel::BLOCK, Some("north".into()));
        let crate::machine::Data::Fel(f) = &mut fel.data else { panic!() };
        f.on = true; f.power = 11_250;
        f.slots[1] = Some(crate::server::container_bridge::Stack::new("ntm:laser_crystal_bismuth", 1));
        let mut server = server;
        server.ready.world.machines.insert(key.clone(), fel);
        let mut outside = BTreeMap::new();
        outside.insert(core.clone(), Machine::new(Kind::Silex, crate::silex::BLOCK, Some(silex_facing.into())));
        (host, server, key, core, outside)
    }

    #[test]
    fn a_silex_along_the_beam_takes_its_wavelength_and_lets_it_through() {
        for facing in ["north", "south"] {
            let (mut host, mut server, key, core, outside) = fel_and_silex(facing, -9);
            assert!(server.fel_tick(&mut host, std::slice::from_ref(&key), &BTreeSet::new(), &outside));
            assert_eq!(server.ready.silex_lasers.get(&core), Some(&crate::fel::Wavelength::Visible), "{facing}");
            assert!(host.writes.is_empty() && host.dropped.is_empty());
            let crate::machine::Data::Fel(f) = &server.ready.world.machines[&key].data else { panic!() };
            assert!(!f.missing_valid_silex);
            assert_eq!(f.distance, crate::fel::RANGE, "the beam goes through and on");
            // The SILEX's step takes it, once.
            server.ready.world.machines.insert(core.clone(), outside[&core].clone());
            server.silex_lasers_in(std::slice::from_ref(&core));
            let crate::machine::Data::Silex(s) = &server.ready.world.machines[&core].data else { panic!() };
            assert_eq!(s.mode, crate::fel::Wavelength::Visible);
            assert!(server.ready.silex_lasers.is_empty());
        }
    }

    #[test]
    fn a_silex_across_the_beam_or_too_near_is_broken_and_dropped() {
        for (facing, z) in [("east", -9), ("north", -4), ("north", -5)] {
            let (mut host, mut server, key, core, outside) = fel_and_silex(facing, z);
            assert!(server.fel_tick(&mut host, std::slice::from_ref(&key), &BTreeSet::new(), &outside));
            assert!(server.ready.silex_lasers.is_empty(), "{facing} {z}");
            let mut cleared: Vec<Pos> = host.writes.iter().filter(|(_, b)| b == "minecraft:air").map(|(p, _)| *p).collect();
            cleared.sort();
            let mut all: Vec<Pos> = host.parts.iter().filter(|(_, b)| *b != "minecraft:air").map(|(p, _)| *p).collect();
            all.sort();
            assert_eq!(cleared, all, "every part of it goes");
            let struck = [0, 65, z + 1];
            assert_eq!(host.dropped, vec![(struck, crate::silex::BLOCK.to_string(), 1)]);
            let _ = core;
        }
    }

    #[test]
    fn another_machines_part_is_an_obstacle_not_a_freeze() {
        let mut host = FelWorld::default();
        host.parts.insert([0, 65, -5], "minecraft:air".into());
        host.parts.insert([0, 65, -7], "ntm:dummy_machine".into());
        let mut server = Server::native(&mut host, 0);
        let key = ("minecraft:overworld".to_string(), [0, 64, 0]);
        let mut fel = Machine::new(Kind::Fel, crate::fel::BLOCK, Some("north".into()));
        let crate::machine::Data::Fel(f) = &mut fel.data else { panic!() };
        f.on = true; f.power = 11_250;
        f.slots[1] = Some(crate::server::container_bridge::Stack::new("ntm:laser_crystal_bismuth", 1));
        server.ready.world.machines.insert(key.clone(), fel);
        assert!(server.fel_tick(&mut host, std::slice::from_ref(&key), &BTreeSet::new(), &BTreeMap::new()));
        let crate::machine::Data::Fel(f) = &server.ready.world.machines[&key].data else { panic!() };
        assert_eq!(f.distance, 7, "it stops the beam");
    }

    #[test]
    fn fel_server_tick_spends_energy_and_stages_the_struck_block() {
        let mut host = FelWorld::default();
        let mut server = Server::native(&mut host, 0);
        let key = ("minecraft:overworld".to_string(), [0, 64, 0]);
        let mut machine = Machine::new(Kind::Fel, crate::fel::BLOCK, Some("north".into()));
        let crate::machine::Data::Fel(f) = &mut machine.data else { panic!() };
        f.on = true; f.power = 4_000;
        f.slots[1] = Some(crate::server::container_bridge::Stack::new("ntm:laser_crystal_co2", 1));
        server.ready.world.machines.insert(key.clone(), machine);
        assert!(server.fel_tick(&mut host, std::slice::from_ref(&key), &BTreeSet::new(), &BTreeMap::new()));
        let crate::machine::Data::Fel(f) = &server.ready.world.machines[&key].data else { panic!() };
        assert_eq!((f.power, f.distance), (250, 5));
        assert_eq!(host.writes, vec![([0, 65, -5], "minecraft:air".into())]);
    }

    #[test]
    fn drx_beam_leaves_digamma_fire_on_ash_and_doses_the_players_in_it() {
        for (polaroid, left) in [(1, "ntm:fire_digamma"), (11, "ntm:digamma_matter")] {
            let mut host = FelWorld { stone: true, ..Default::default() };
            let mut server = Server::native(&mut host, 0);
            server.polaroid = polaroid;
            server.blasts.entered.insert("in".into(), 0);
            server.blasts.entered.insert("out".into(), 0);
            let key = ("minecraft:overworld".to_string(), [0, 64, 0]);
            let mut machine = Machine::new(Kind::Fel, crate::fel::BLOCK, Some("north".into()));
            let crate::machine::Data::Fel(f) = &mut machine.data else { panic!() };
            f.on = true; f.distance = 5;
            f.slots[1] = Some(crate::server::container_bridge::Stack::new("ntm:laser_crystal_digamma", 1));
            server.ready.world.machines.insert(key.clone(), machine);
            server.set_players([("in", [0.5, 64.0, -2.0]), ("out", [0.5, 64.0, 3.0])].into_iter().map(|(id, pos)| PlayerInfo { id: id.into(), name: id.into(), dim: key.0.clone(), pos, creative: false }).collect());
            let mut ticks = 0;
            while host.writes.is_empty() {
                server.tick = 200 + ticks;
                let crate::machine::Data::Fel(f) = &mut server.ready.world.machines.get_mut(&key).unwrap().data else { panic!() };
                f.power = 303_750;
                assert!(server.fel_tick(&mut host, std::slice::from_ref(&key), &BTreeSet::new(), &BTreeMap::new()));
                ticks += 1;
                assert!(ticks < 100);
            }
            assert_eq!(host.writes, vec![([0, 65, -5], left.into()), ([0, 64, -5], "ntm:ash_digamma".into())]);
            // 0.1F a tick, summed in float.
            let mut expected = 0.0f32;
            for _ in 0..ticks { expected = crate::digamma::increment(expected, 0.1); }
            assert_eq!(host.digamma["in"], expected);
            assert!(!host.digamma.contains_key("out"));
        }
    }

    #[test]
    fn visible_beam_blinds_and_ignites_players_in_its_previous_bounds() {
        let mut host = FelWorld::default();
        let mut server = Server::native(&mut host, 0);
        let key = ("minecraft:overworld".to_string(), [0, 64, 0]);
        let mut machine = Machine::new(Kind::Fel, crate::fel::BLOCK, Some("north".into()));
        let crate::machine::Data::Fel(f) = &mut machine.data else { panic!() };
        f.on = true; f.power = 20_000; f.distance = 7;
        f.slots[1] = Some(crate::server::container_bridge::Stack::new("ntm:laser_crystal_bismuth", 1));
        server.ready.world.machines.insert(key.clone(), machine);
        server.set_players(vec![PlayerInfo { id:"inside".into(), name:"inside".into(), dim:key.0.clone(), pos:[0.5,64.0,-2.0], creative:false }, PlayerInfo { id:"outside".into(), name:"outside".into(), dim:key.0.clone(), pos:[5.0,64.0,-2.0], creative:false }]);
        assert!(server.fel_tick(&mut host, &[key], &BTreeSet::new(), &BTreeMap::new()));
        assert_eq!(host.fire, vec![("inside".into(), 200)]);
        assert_eq!(host.effects, vec![("inside".into(), "minecraft:blindness".into(), 60*60*65536)]);
    }

    #[test]
    fn gamma_beam_records_environment_and_respects_creative_and_join_grace() {
        let mut host = FelWorld::default();
        let mut server = Server::native(&mut host, 200);
        server.tick = 200;
        server.blasts.entered.insert("normal".into(), 0);
        server.blasts.entered.insert("creative".into(), 0);
        server.blasts.entered.insert("new".into(), 199);
        let key = ("minecraft:overworld".to_string(), [0, 64, 0]);
        let mut machine = Machine::new(Kind::Fel, crate::fel::BLOCK, Some("north".into()));
        let crate::machine::Data::Fel(f) = &mut machine.data else { panic!() };
        f.on = true; f.power = 200_000; f.distance = 7;
        f.slots[1] = Some(crate::server::container_bridge::Stack::new("ntm:laser_crystal_dnt", 1));
        server.ready.world.machines.insert(key.clone(), machine);
        server.set_players([("normal", false), ("creative", true), ("new", false)].into_iter().map(|(id, creative)| PlayerInfo { id:id.into(), name:id.into(), dim:key.0.clone(), pos:[0.5,64.0,-2.0], creative }).collect());
        assert!(server.fel_tick(&mut host, &[key], &BTreeSet::new(), &BTreeMap::new()));
        assert_eq!(server.player_dose("normal").millirad, 25_000);
        assert_eq!(server.player_dose("creative").millirad, 0);
        assert_eq!(server.player_dose("new").millirad, 0);
        for id in ["normal", "creative", "new"] { assert_eq!(server.radiation_env[id], 25_000); }
    }

    /// A player's dose storage that refuses a write expecting another dose, and the armor's factor.
    #[derive(Default)]
    struct Attached {
        doses: BTreeMap<String, u64>,
        writes: Vec<(u64, u64)>,
        logs: Vec<String>,
        irradiation: BTreeMap<String, crate::zirnox::StoredPotion>,
        removed: Vec<String>,
        digamma: BTreeMap<String, f32>,
        digamma_writes: Vec<(f32, f32)>,
    }
    impl Kv for Attached {
        fn get(&mut self, _: &str) -> Option<Vec<u8>> { None }
        fn set(&mut self, _: &str, _: &[u8]) -> Result<(), String> { Ok(()) }
        fn delete(&mut self, _: &str) {}
    }
    impl ServerHost for Attached {
        fn get_block(&mut self, _: &str, _: Pos) -> Option<String> { None }
        fn set_blocks(&mut self, _: &str, _: &[(Pos, String)]) -> Result<(), String> { Ok(()) }
        fn tell(&mut self, _: &str, _: &str) {}
        fn log(&mut self, text: &str) { self.logs.push(text.into()); }
        fn radiation_factor(&mut self, _: &str) -> f64 { 0.25 }
        fn player_dose(&mut self, id: &str, legacy: &Dose) -> Option<Dose> {
            Some(self.doses.get(id).map_or_else(|| legacy.clone(), |&millirad| Dose { millirad }))
        }
        fn player_irradiation(&mut self, id: &str, _: &Dose) -> Option<crate::zirnox::StoredPotion> {
            Some(self.irradiation.get(id).copied().unwrap_or_default())
        }
        fn commit_player_irradiation(&mut self, id: &str, _: &Dose, expected: crate::zirnox::StoredPotion, desired: crate::zirnox::StoredPotion) -> bool {
            if self.irradiation.get(id).copied().unwrap_or_default() != expected { return false; }
            self.irradiation.insert(id.into(), desired);
            true
        }
        fn effect_state(&mut self, _: &str, effect: &str) -> Option<Option<(u32, u8)>> { Some((effect == "seen").then_some((5, 1))) }
        fn remove_effect(&mut self, id: &str, effect: &str) { self.removed.push(format!("{id} {effect}")); }
        fn player_digamma(&mut self, id: &str, _: &Dose) -> Option<f32> { Some(self.digamma.get(id).copied().unwrap_or(0.0)) }
        fn commit_player_digamma(&mut self, id: &str, _: &Dose, expected: f32, desired: f32) -> bool {
            if self.digamma.get(id).copied().unwrap_or(0.0) != expected { return false; }
            self.digamma_writes.push((expected, desired));
            self.digamma.insert(id.into(), desired);
            true
        }
        fn commit_player_dose(&mut self, id: &str, expected: &Dose, desired: &Dose) -> bool {
            if self.doses.get(id).copied().unwrap_or(0) != expected.millirad { return false; }
            self.writes.push((expected.millirad, desired.millirad));
            self.doses.insert(id.into(), desired.millirad);
            true
        }
    }
    fn staged(host: &mut Attached) -> Deferred<'_, Attached> {
        Deferred { host, slots: BTreeMap::new(), effects: Vec::new(), generations: BTreeMap::new(), cells: BTreeMap::new(),
            vacant: BTreeSet::new(), origins: Vec::new(), valid: true, resource: false, world_writes: Vec::new(),
            critical: std::collections::VecDeque::new(), doses: BTreeMap::new(), irradiations: BTreeMap::new(), digammas: BTreeMap::new() }
    }

    /// A machine step's dose writes reach the player's storage after the commit, in order, each expecting the one
    /// before it; a read in the step sees the step's own write; the armor's factor is the host's.
    #[test]
    fn a_machine_steps_doses_reach_the_players_storage_in_order() {
        let mut host = Attached::default();
        host.doses.insert("p".into(), 1_000);
        let mut step = staged(&mut host);
        assert_eq!(step.radiation_factor("p"), 0.25);
        for add in [500, 700] {
            let expected = step.player_dose("p", &Dose::default()).unwrap();
            let desired = Dose { millirad: expected.millirad + add };
            assert!(step.commit_player_dose("p", &expected, &desired));
        }
        assert_eq!(step.player_dose("p", &Dose::default()).unwrap().millirad, 2_200);
        let effects = std::mem::take(&mut step.effects);
        assert_eq!(host.doses["p"], 1_000, "nothing reaches the storage before the commit");
        for (_, effect) in effects { effect(&mut host); }
        assert_eq!(host.writes, [(1_000, 1_500), (1_500, 2_200)]);
        assert!(host.logs.is_empty());
    }

    /// The radiation potion goes the way of the dose: a read in the step sees the step's write, the storage takes
    /// the writes in order after the commit, and so does an effect's removal; the effect check reads the host.
    #[test]
    fn a_machine_steps_radiation_potion_reaches_the_players_storage_in_order() {
        let mut host = Attached::default();
        let mut step = staged(&mut host);
        let legacy = Dose::default();
        let at = |potion| crate::zirnox::StoredPotion { potion, migrated: (0, 0), milkproof: false };
        assert!(step.commit_player_irradiation("p", &legacy, at((0, 0)), at((600, 3))));
        assert_eq!(step.player_irradiation("p", &legacy), Some(at((600, 3))));
        assert!(step.commit_player_irradiation("p", &legacy, at((600, 3)), at((1200, 4))));
        step.remove_effect("p", "ntm:radiation");
        assert_eq!(step.effect_state("p", "seen"), Some(Some((5, 1))));
        let effects = std::mem::take(&mut step.effects);
        assert!(host.irradiation.is_empty() && host.removed.is_empty(), "nothing before the commit");
        for (_, effect) in effects { effect(&mut host); }
        assert_eq!(host.irradiation["p"], at((1200, 4)));
        assert_eq!(host.removed, ["p ntm:radiation"]);
        assert!(host.logs.is_empty());
    }

    /// The digamma goes the way of the dose: staged, read back in the step, written in order after the commit, and a
    /// write that finds another value in the storage is dropped and logged.
    #[test]
    fn a_machine_steps_digamma_reaches_the_players_storage_in_order() {
        let mut host = Attached::default();
        host.digamma.insert("p".into(), 1.0);
        let mut step = staged(&mut host);
        let legacy = Dose::default();
        for _ in 0..2 {
            let before = step.player_digamma("p", &legacy).unwrap();
            assert!(step.commit_player_digamma("p", &legacy, before, crate::digamma::increment(before, 0.1)));
        }
        assert!(step.commit_player_digamma("q", &legacy, 0.5, 0.6));
        assert_eq!(step.player_digamma("p", &legacy), Some(1.0f32 + 0.1 + 0.1));
        let effects = std::mem::take(&mut step.effects);
        assert_eq!(host.digamma["p"], 1.0, "nothing reaches the storage before the commit");
        for (_, effect) in effects { effect(&mut host); }
        assert_eq!(host.digamma_writes, [(1.0, 1.1), (1.1, 1.0f32 + 0.1 + 0.1)]);
        assert!(!host.digamma.contains_key("q"));
        assert_eq!(host.logs, ["ntm: the digamma of q changed under a machine step; its write was dropped"]);
    }

    /// Dropped with the step: a step that is not committed leaves the storage as it was.
    #[test]
    fn an_uncommitted_steps_dose_never_reaches_the_storage() {
        let mut host = Attached::default();
        let mut step = staged(&mut host);
        assert!(step.commit_player_dose("p", &Dose::default(), &Dose { millirad: 9 }));
        drop(step);
        assert!(host.writes.is_empty() && host.doses.is_empty());
    }
}
