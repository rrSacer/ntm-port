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
    PileExplosion {
        dim: String,
        pos: [f64; 3],
    },
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommittedEffects {
    pub owners: Vec<(String, Pos)>,
    #[serde(default)]
    pub origins: Vec<CellGuard>,
    pub effects: std::collections::VecDeque<WorldEffect>,
    /// Removed machines retain their contents here until every effect succeeds.
    pub tombstones: Vec<((String, Pos), Machine)>,
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
        if self.resource && !self.cells.contains_key(&(d.into(),p)) {
            let snapshot=self.host.cell_snapshot(d,p)?;
            if !self.world_writes.iter().any(|w|w.guard.dim==d&&w.guard.pos==p){
                let guard=CellGuard{dim:d.into(),pos:p,generation:self.generations.get(&(d.into(),p)).copied().unwrap_or(0),state:snapshot.state.clone()};
                self.world_writes.push(CellWrite{notify:true,guard,desired:snapshot.state.clone(),expected:snapshot,destroy:false});
            }
            return Some(self.world_writes.iter().find(|w|w.guard.dim==d&&w.guard.pos==p).unwrap().expected.state.clone());
        }
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
        if self.resource {
            for (pos,desired) in b {
                if let Some(previous)=self.world_writes.iter_mut().find(|w|w.guard.dim==d&&w.guard.pos==*pos){previous.desired=desired.clone();previous.destroy=blockstate::id_of(desired)=="minecraft:air" && blockstate::id_of(&previous.expected.state)!="minecraft:air";continue;}
                let Some(expected)=self.host.cell_snapshot(d,*pos) else{self.valid=false;return Err("unavailable consumed cell".into())};
                let guard=CellGuard{dim:d.into(),pos:*pos,generation:self.generations.get(&(d.into(),*pos)).copied().unwrap_or(0),state:expected.state.clone()};
                let destroy=blockstate::id_of(desired)=="minecraft:air" && blockstate::id_of(&expected.state)!="minecraft:air";
                self.world_writes.push(CellWrite{notify:true,guard,expected,desired:desired.clone(),destroy});
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
    fn pile_explode(&mut self, d: &str, p: [f64; 3]) -> bool {
        self.critical.push_back(WorldEffect::PileExplosion {
            dim: d.into(),
            pos: p,
        });
        true
    }
    fn hurt(&mut self, p: &str, a: f32, k: &str) {
        let (p, k) = (p.to_string(), k.to_string());
        self.effects.push((self.origins.clone(),Box::new(move |h| h.hurt(&p, a, &k))));
    }
    fn effect(&mut self, p: &str, e: &str, s: u32, a: u8) {
        let (p, e) = (p.to_string(), e.to_string());
        self.effects.push((self.origins.clone(),Box::new(move |h| h.effect(&p, &e, s, a))));
    }
    fn show(&mut self, d: &str, s: &crate::blast::visuals::Show) {
        let (d, s) = (d.to_string(), s.clone());
        self.effects.push((self.origins.clone(),Box::new(move |h| h.show(&d, &s))));
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
            if !(self.current_guard(g)&&actual==prepared.source.expected){
                // Consumed or genuinely ambiguous: keep the old physical
                // retention. An intact source still owns its items, so emits none.
                self.ready.world.machine_effects.push_back(prepared.receipt);
            }
        }
        if self.ready.world.prepared_retentions==before.prepared_retentions && self.ready.world.machine_effects==before.machine_effects{return true;}
        match host.set(crate::persist::KEY,&self.ready.world.kept(self.ready.world.ticks)){
            Ok(())=>{self.ready.kept=self.ready.world.kept(0);true}
            Err(e)=>{self.ready.world.prepared_retentions=before.prepared_retentions;self.ready.world.machine_effects=before.machine_effects;host.log(&format!("retention recovery checkpoint: {e}"));false}
        }
    }

    pub(super) fn track_effect_cells(&mut self) {
        let guards:Vec<_>=self.ready.world.machine_effects.iter().chain(&self.ready.world.applied_machine_effects).flat_map(|b|b.origins.iter().chain(b.effects.iter().flat_map(|e|match e{WorldEffect::Blocks{expected,..}=>expected.as_slice(),_=>&[]}))).cloned().collect();
        for g in guards{self.ready.world.generations.entry(g.key()).or_insert(g.generation);}
        for p in &self.ready.world.prepared_retentions{let g=&p.source.guard;self.ready.world.generations.entry(g.key()).or_insert(g.generation);}
    }

    pub(super) fn flush_machine_effects(&mut self, host: &mut impl ServerHost) {
        // External facts must invalidate old writes before the first retry.
        if !self.queued.is_empty(){self.native_record_events(host);}
        if self.ready.facts_pending || !self.recover_retentions(host){return;}
        let batches=self.ready.world.machine_effects.len();
        if batches==0{return;}
        for _ in 0..batches {
            while let Some(batch)=self.ready.world.machine_effects.front() {
                let Some(mut effect)=batch.effects.front().cloned() else{self.ready.world.machine_effects.pop_front();break};
                let dim=match &effect{WorldEffect::Blocks{dim,..}|WorldEffect::Items{dim,..}|WorldEffect::EncodedItems{dim,..}|WorldEffect::PileExplosion{dim,..}=>dim};
                if !host.pile_reachable(dim){self.ready.world.machine_effects.rotate_left(1);break;}
                let mut wait=false;
                let mut physical=false;
                let mut cells=Vec::new();
                if let WorldEffect::Blocks{dim,blocks,expected,retained}=&mut effect {
                    let mut keep=Vec::new();let mut guards=Vec::new();
                    for (pos,desired) in &*blocks {
                        let Some(g)=expected.iter().find(|g|g.pos==*pos&&g.dim==*dim) else{continue};
                        if !self.current_guard(g){continue;}
                        match host.get_block(dim,*pos){
                            None=>{wait=true;keep.push((*pos,desired.clone()));guards.push(g.clone());}
                            Some(actual) if blockstate::id_of(&actual)==blockstate::id_of(&g.state) || blockstate::id_of(&actual)==blockstate::id_of(desired)=>{
                                let Some(snapshot)=host.cell_snapshot(dim,*pos) else{wait=true;keep.push((*pos,desired.clone()));guards.push(g.clone());continue};
                                // Refresh only a still-existing source. A journal
                                // replay whose source is already gone retains its
                                // old physical drop rather than silently losing it.
                                if blockstate::id_of(&actual)==blockstate::id_of(&g.state){
                                    retained.retain(|d|d.origin.key()!=g.key());
                                    if blockstate::id_of(&actual)!=blockstate::id_of(desired)&&!snapshot.items.is_empty(){retained.push(RetainedDrop{origin:g.clone(),items:snapshot.items.clone()});}
                                }
                                cells.push(CellWrite{notify:false,guard:g.clone(),expected:snapshot,desired:desired.clone(),destroy:false});
                                physical=true;keep.push((*pos,desired.clone()));guards.push(g.clone());
                            }
                            Some(_)=>{},
                        }
                    }
                    *blocks=keep;*expected=guards;
                    physical|=!retained.is_empty();
                }else{
                    // Confirmed items and explosions belong to the old origin,
                    // independently of whatever occupies its position now.
                    physical=true;
                }
                if wait{self.ready.world.machine_effects.front_mut().unwrap().effects[0]=effect;self.ready.world.machine_effects.rotate_left(1);break;}
                self.ready.world.machine_effects.front_mut().unwrap().effects[0]=effect.clone();
                if let Err(e)=host.set(crate::persist::KEY,&self.ready.world.kept(self.ready.world.ticks)){host.log(&format!("pending machine effect: {e}"));return;}
                let done=match &effect {
                    WorldEffect::Blocks{retained,..}=>cells.is_empty()&&retained.is_empty()||host.commit_structural(&cells,retained,&self.ready.world.machine_effects.front().unwrap().owners),
                    WorldEffect::Items{dim,pos,items}=>host.drop_items(dim,*pos,items),
                    WorldEffect::EncodedItems{dim,pos,items}=>host.drop_encoded_items(dim,*pos,items),
                    WorldEffect::PileExplosion{dim,pos}=>host.pile_explode(dim,*pos),
                };
                if !done{
                    // A rejected atomic publication emitted no item. Discard
                    // its captured inventory; the next attempt observes it anew.
                    if let Some(WorldEffect::Blocks{retained,..})=self.ready.world.machine_effects.front_mut().and_then(|b|b.effects.front_mut()){retained.clear();}
                    self.ready.world.machine_effects.rotate_left(1);break;
                }
                let batch=self.ready.world.machine_effects.front_mut().unwrap();
                let effect=batch.effects.pop_front().unwrap();
                if physical {
                    let receipt=CommittedEffects{origins:batch.origins.clone(),owners:batch.owners.clone(),tombstones:Vec::new(),effects:std::collections::VecDeque::from([effect])};
                    if matches!(receipt.effects.front(),Some(WorldEffect::Blocks{..})){
                        for g in &receipt.origins {
                            if self.ready.world.machines.get(&g.key()).is_some_and(|m|m.generation==g.generation){self.ready.native_saved.remove(&g.key());}
                        }
                    }
                    self.ready.world.applied_machine_effects.push_back(receipt);
                }
            }
        }
        if host.set(crate::persist::KEY,&self.ready.world.kept(self.ready.world.ticks)).is_ok(){self.ready.kept=self.ready.world.kept(0);}else{self.ready.kept=Vec::new();}
    }

    /// The existing engine hook reports whether world/entity writes have
    /// actually flushed. Until then replay after a crash may duplicate an
    /// effect, but must never acknowledge an entity not yet saved by the world.
    pub fn native_world_saved(&mut self, host: &mut impl ServerHost, wait: bool) {
        let applied = wait.then(|| std::mem::take(&mut self.ready.world.applied_machine_effects));
        if self.ready.world.kept(0) == self.ready.kept {
            return;
        }
        let kept = self.ready.world.kept(self.ready.world.ticks);
        if let Err(e) = host.set(crate::persist::KEY, &kept) {
            if let Some(applied) = applied {
                self.ready.world.applied_machine_effects = applied;
            }
            host.log(&format!("machine effect receipts/removals: {e}"));
        } else {
            self.ready.kept = self.ready.world.kept(0);
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
            generations:before.world.generations.clone(),
            origins:before.world.machines.iter().filter(|(k,_)|k.0==dim).map(|(k,m)|CellGuard::machine(k,m)).collect(),
            valid:true,
            resource:false,world_writes:Vec::new(),
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
        for k in &keys {
            let Some(expected) = observed.get(k) else {
                self.ready = before;
                return Some(PileUse::Handled);
            };
            let m = self.ready.world.machines.get_mut(k).unwrap();
            let (desired, data) = Self::machine_output(m, expected);
            commits.push(MachineCommit {
                cells: Vec::new(),
                guard:CellGuard::machine(k,&before.world.machines[k]),
                expected_saved:before.native_saved.contains_key(k),
                dim: k.0.clone(),
                pos: k.1,
                expected: expected.clone(),
                desired,
                data,
                json: serde_json::to_string(m).expect("machine serialization"),
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
            self.ready
                .world
                .machine_effects
                .push_back(CommittedEffects {
                    origins:staged.origins.clone(),
                    owners: keys,
                    effects: staged.critical,
                    tombstones: Vec::new(),
                });
        }
        for (guards,effect) in staged.effects {
            if guards.iter().all(|g|self.current_guard(g)){effect(staged.host);}
        }
        self.track_effect_cells();
        self.flush_machine_effects(staged.host);
        self.native_save(staged.host);
        outcome
    }

    pub(super) fn local_steps(&mut self, host: &mut impl ServerHost) {
        self.validate_detonations(host);
        let mut buttons = std::mem::take(&mut self.ready.pending_buttons);
        for (player,dim,pos,button) in host.machine_buttons(){
            if let Some(m)=self.ready.world.machines.get(&(dim.clone(),pos)){
                buttons.push(generation::Guarded{guard:CellGuard::machine(&(dim.clone(),pos),m),value:(player,dim,pos,button)});
            }
        }
        buttons.retain(|i|self.current_guard(&i.guard)&&!self.ready.broken.contains_key(&i.guard.key()));
        let mut groups: Vec<Vec<Key>> = Vec::new();
        let mut piles: BTreeMap<Key, Vec<Key>> = BTreeMap::new();
        for (k, m) in &self.ready.world.machines {
            if self
                .ready
                .world
                .machine_effects
                .iter().chain(self.ready.world.prepared_retentions.iter().map(|p|&p.receipt))
                .any(|b| b.origins.iter().any(|g|g.key()==*k&&g.generation==m.generation))
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
            let mut journaled=false;
            if accessible {
                let mut staged = Deferred {
                    host,
                    slots: observed.clone(),
                    effects: Vec::new(),
                    critical: std::collections::VecDeque::new(),
                    cells:before.world.machines.iter().map(|(k,m)|(k.clone(),CellGuard::machine(k,m))).collect(),
            vacant:before.broken.keys().cloned().collect(),
            generations:before.world.generations.clone(),
                    origins:before.world.machines.iter().map(|(k,m)|CellGuard::machine(k,m)).collect(),
                    valid:true,
                    resource:before.world.machines.values().any(|m|matches!(m.data,crate::machine::Data::Excavator(_)|crate::machine::Data::Drill(_))),world_writes:Vec::new(),
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
                for input in &buttons {
                    let (_,d,p,b)=&input.value;
                    if keys.contains(&(d.clone(), *p)) {
                        self.machine_button(&(d.clone(), *p), *b);
                    }
                }
                self.pile_tick(&mut staged);
                self.zirnox_tick(&mut staged);
                self.requested_detonations(&mut staged);
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
                let preparations:Vec<_>=staged.world_writes.iter().filter(|w|blockstate::id_of(&w.expected.state)!=blockstate::id_of(&w.desired)&&!w.expected.items.is_empty()).map(|w|{
                    let mut origins=staged.origins.clone();origins.push(w.guard.clone());
                    PreparedRetention{source:w.clone(),receipt:CommittedEffects{owners:keys.clone(),origins,tombstones:Vec::new(),effects:std::collections::VecDeque::from([WorldEffect::EncodedItems{dim:w.guard.dim.clone(),pos:w.guard.pos,items:w.expected.items.clone()}])}}
                }).collect();
                if staged.valid && !preparations.is_empty(){
                    // Only confirmed world state is journaled. The speculative
                    // machine's new state belongs solely to its atomic BE commit.
                    let mut durable=before.world.clone();
                    for p in &preparations{durable.generations.entry(p.source.guard.key()).or_insert(p.source.guard.generation);}
                    durable.prepared_retentions.extend(preparations.clone());
                    match staged.host.set(crate::persist::KEY,&durable.kept(durable.ticks)){
                        Ok(())=>{journaled=true;self.ready.world.prepared_retentions.extend(preparations.clone());}
                        Err(e)=>{staged.valid=false;staged.host.log(&format!("retention preparation: {e}"));}
                    }
                }
                accepted = staged.valid && staged.host.commit_machines(&commits);
                if accepted {
                    for p in &preparations{
                        self.ready.world.prepared_retentions.retain(|other|other!=p);
                        self.ready.world.applied_machine_effects.push_back(p.receipt.clone());
                    }
                    for w in &staged.world_writes {
                        if w.desired!=w.expected.state{self.ready.world.generations.entry(w.guard.key()).or_insert(w.guard.generation);}
                    }
                    buttons.retain(|i| !keys.contains(&i.guard.key()));
                    for k in &keys {
                        self.ready.broken.remove(k);
                        self.ready.world.removals.retain(|r| &r.key != k);
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
                                origins:staged.origins.clone(),
                                owners: keys.clone(),
                                effects: staged.critical,
                                tombstones,
                            });
                    }
                    for (guards,effect) in staged.effects {
                        if guards.iter().all(|g|self.current_guard(g)){effect(staged.host);}
                    }
                }
            }
            if !accepted {
                self.ready = before;
                self.blasts.lit = lit_before;
                if journaled && let Err(e)=host.set(crate::persist::KEY,&self.ready.world.kept(self.ready.world.ticks)){
                    self.ready.facts_pending=true;host.log(&format!("discard refused retention preparation: {e}"));
                }
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
