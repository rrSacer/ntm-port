//! Pile MK2 world integration. Geometry lives in the core's block entity;
//! members keep only its position. Writes are registered immediately, so a
//! delayed native block notification cannot replace newly formed state.
use super::*;
use crate::{
    machine::Data,
    pile::{self, ChannelType, Core, Pile, Rod, Shape},
    server::container_bridge::Stack,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PileUse {
    Handled,
    ConsumeRod,
    WearDrill,
}
fn state(role: &str) -> String {
    format!("ntm:pile_block__{role}[facing=north,honey_level=0]")
}

impl Server {
    pub(super) fn pile_core_at(&self, dim: &str, p: Pos) -> Option<Key> {
        self.ready.world.machines.iter().find_map(|(k, m)| {
            if k.0 == dim
                && let Data::Pile(pile) = &m.data
                && let Pile::Core(Some(c)) = &**pile
                && c.shape.contains(p)
            {
                Some(k.clone())
            } else {
                None
            }
        })
    }
    /// Set all roles, preserving core state; the vanilla block entities are
    /// created by set_blocks and carry the complete state on the same tick.
    fn pile_write(
        &mut self,
        host: &mut impl ServerHost,
        dim: &str,
        core: Core,
    ) -> Result<(), String> {
        let cells = core.cells();
        let blocks: Vec<_> = cells.iter().map(|(p, r)| (*p, state(r))).collect();
        let owners:Vec<_>=cells.iter().map(|(p,_)|(dim.to_string(),*p)).collect();
        host.set_machine_blocks(dim, &blocks,&owners)?;
        for (p, role) in cells {
            let id = format!("ntm:pile_block__{role}");
            let key = (dim.to_string(), p);
            let mut m = Machine::new(Kind::Pile, &id, Some("north".into()));
            m.generation=self.ready.world.machines.get(&key).map(|m|m.generation).unwrap_or_else(||self.ready.world.generations.get(&key).copied().unwrap_or(0));
            m.data = Data::Pile(Box::new(if p == core.shape.at {
                Pile::Core(Some(core.clone()))
            } else {
                Pile::Member(Some(core.shape.at))
            }));
            self.ready.world.machines.insert(key.clone(), m);
            self.ready.loaded.insert(key.clone(), true);
            self.ready.seen.insert(key.clone());
            self.ready.shown.insert(key, state(role));
        }
        self.ready.topo_dirty = true;
        Ok(())
    }
    /// A pile's formation in one step: every cell's new block, its block entity
    /// with the machine's first JSON, and the held drill's wear. The model is
    /// recorded only after the host confirmed the publication, so a refusal
    /// leaves the bricks, the model and the tool as they were.
    fn pile_form(
        &mut self,
        host: &mut impl ServerHost,
        dim: &str,
        core: Core,
        held: Option<&Stack>,
    ) -> Result<(), String> {
        let wear = held.is_some_and(|h| h.id == "ntm:hand_drill");
        let cells = core.cells();
        let owners: Vec<_> = cells.iter().map(|(p, _)| (dim.to_string(), *p)).collect();
        let mut writes = Vec::new();
        let mut formed = Vec::new();
        for (p, role) in cells {
            let id = format!("ntm:pile_block__{role}");
            let key = (dim.to_string(), p);
            let Some(expected) = host.cell_snapshot(dim, p) else {
                return Err(format!("pile formation: {p:?} is not loaded"));
            };
            if blockstate::id_of(&expected.state) != pile::BRICK || expected.entity.is_some() {
                return Err(format!("pile formation: {p:?} is not a plain brick"));
            }
            let generation = self
                .ready
                .world
                .machines
                .get(&key)
                .map(|m| m.generation)
                .unwrap_or_else(|| self.ready.world.generations.get(&key).copied().unwrap_or(0));
            let mut m = Machine::new(Kind::Pile, &id, Some("north".into()));
            m.generation = generation;
            m.data = Data::Pile(Box::new(if p == core.shape.at {
                Pile::Core(Some(core.clone()))
            } else {
                Pile::Member(Some(core.shape.at))
            }));
            let json = serde_json::to_string(&m).map_err(|e| e.to_string())?;
            let data = Self::machine_screen(&mut m.clone());
            writes.push(CellWrite {
                notify: false,
                guard: CellGuard {
                    dim: dim.to_string(),
                    pos: p,
                    generation,
                    state: pile::BRICK.to_string(),
                },
                desired: state(role),
                expected,
                destroy: false,
                initial: Some(InitialState { json: json.clone(), data }),
            });
            formed.push((key, m, json, state(role)));
        }
        if !host.commit_formation(&writes, wear, &owners) {
            return Err("pile formation refused: nothing changed".into());
        }
        for (key, m, json, shown) in formed {
            self.ready.world.machines.insert(key.clone(), m);
            self.ready.loaded.insert(key.clone(), true);
            self.ready.seen.insert(key.clone());
            self.ready.native_saved.insert(key.clone(), json);
            self.ready.shown.insert(key, shown);
        }
        self.ready.topo_dirty = true;
        Ok(())
    }
    fn pile_eject(host: &mut impl ServerHost, dim: &str, at: Pos, rod: Rod) {
        host.pile_drop(dim, at, &rod.id());
    }
    /// BlockPile.breakBlock restores even the removed/replaced cell to brick.
    /// The existing native change hook has no cause, so commands have the
    /// same result. Rod drops lose depletion, as in Java. Nothing happens
    /// (false) outside the host's active dimension, where drops and explosions
    /// are unavailable: the pile stays intact until it can be handled.
    pub(super) fn pile_destroy(&mut self, host: &mut impl ServerHost, dim: &str, c: &Core) -> bool {
        self.pile_dismantle(host, dim, c, false)
    }
    /// `core_only`: the meltdown reverts just the core block (Java `destroy`).
    fn pile_dismantle(&mut self, host: &mut impl ServerHost, dim: &str, c: &Core, core_only: bool) -> bool {
        if !host.pile_reachable(dim) {
            return false;
        }
        let cells: Vec<_> = c.cells().into_iter().filter(|(p, _)| !core_only || *p == c.shape.at).collect();
        let mut changes = Vec::new();
        for (at, _) in cells {
            let key = (dim.to_string(), at);
            if host.get_block(dim, at).is_some() {
                changes.push((at, pile::BRICK.to_string()));
            }
            self.ready.world.machines.remove(&key);
            self.ready.loaded.remove(&key);
            self.ready.shown.remove(&key);
            self.ready.native_saved.remove(&key);
            self.ready.seen.remove(&key);
        }
        if let Err(e) = host.set_blocks(dim, &changes) {
            host.log(&format!("pile dismantling: {e}"));
        }
        for c in &c.channels {
            if c.kind == ChannelType::Fuel {
                for r in c.rods.iter().flatten() {
                    Self::pile_eject(host, dim, c.exit(), r.clone());
                }
            }
        }
        self.ready.topo_dirty = true;
        true
    }
    /// None: unrelated block. Handled consumes the click only; a rod/tool
    /// action is returned only after the state/slot write succeeded.
    #[allow(clippy::too_many_arguments)]
    pub fn native_pile_use(&mut self,host:&mut impl ServerHost,dim:&str,at:Pos,face:Pos,held:Option<&Stack>,depletion:f64,player:&str)->Option<PileUse>{
        let key=(dim.to_string(),at);
        if self.ready.broken.contains_key(&key)||self.ready.world.machines.get(&key).is_some_and(|m|!self.model_current(&key,m)){return Some(PileUse::Handled);}
        let drill=held.is_some_and(|h|matches!(h.id.as_str(),"ntm:hand_drill"|"ntm:hand_drill_desh"));
        if drill && host.get_block(dim,at).is_some_and(|b|blockstate::id_of(&b)!=pile::BRICK){
            return self.atomic_pile_edit(host,dim,at,face,held,depletion,player);
        }
        self.native_pile_use_inner(host,dim,at,face,held,depletion,player)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn native_pile_use_inner(
        &mut self,
        host: &mut impl ServerHost,
        dim: &str,
        mut at: Pos,
        face: Pos,
        held: Option<&Stack>,
        depletion: f64,
        player: &str,
    ) -> Option<PileUse> {
        let actual = host.get_block(dim, at)?;
        let id = blockstate::id_of(&actual);
        if id != pile::BRICK
            && !id.starts_with("ntm:pile_block__")
            && !id.starts_with("ntm:pile_device__")
        {
            return None;
        }
        let drill =
            held.is_some_and(|h| matches!(h.id.as_str(), "ntm:hand_drill" | "ntm:hand_drill_desh"));
        let mut inward = face.map(|v| -v);
        if drill {
            if id == pile::BRICK {
                let formed = Shape::probe(at, inward, |p| {
                    host.get_block(dim, p)
                        .is_some_and(|b| blockstate::id_of(&b) == pile::BRICK)
                });
                return Some(match formed {
                    // The drill's wear is part of the formation's commit: nothing
                    // is left for the caller to apply afterwards.
                    Ok(shape) => match self.pile_form(host, dim, Core::new(shape), held) {
                        Ok(()) => PileUse::Handled,
                        Err(e) => {
                            host.log(&e);
                            PileUse::Handled
                        }
                    },
                    Err(e) => {
                        host.tell(player, e);
                        // BlockPileBrick reports horizontal validation errors
                        // as a handled tool use, which also wears the drill.
                        if inward[1] == 0 {
                            PileUse::WearDrill
                        } else {
                            PileUse::Handled
                        }
                    }
                });
            }
            if id.starts_with("ntm:pile_device__") {
                if id == "ntm:pile_device__control" {
                    at[1] -= 1;
                    inward = [0, -1, 0];
                } else {
                    let d = blockstate::BlockState::parse(&actual)
                        .and_then(|b| b.get("facing").and_then(registry::facing_dir))
                        .unwrap_or([0, 0, -1]);
                    at = pile::offset(at, d, -1);
                    inward = d.map(|v| -v);
                }
            }
            let Some(key) = self.pile_core_at(dim, at) else {
                host.tell(player, "No core found.");
                return Some(PileUse::Handled);
            };
            let Data::Pile(p) = &self.ready.world.machines[&key].data else {
                unreachable!()
            };
            let Pile::Core(Some(c)) = &**p else {
                unreachable!()
            };
            let mut core = c.clone();
            // Validate loaded ownership before drilling. No partial writes.
            if core.cells().iter().any(|(p, r)| {
                host.get_block(dim, *p)
                    .is_none_or(|b| blockstate::id_of(&b) != format!("ntm:pile_block__{r}"))
            }) {
                host.tell(player, "Foreign or unloaded block in reactor.");
                return Some(PileUse::Handled);
            }
            let undone = core
                .channels
                .iter()
                .any(|c| c.entry == at && c.inward == inward);
            return Some(match core.drill(at, inward) {
                Err(e) => {
                    host.tell(player, e);
                    PileUse::Handled
                }
                Ok(drops) => match self.pile_write(host, dim, core) {
                    Err(e) => {
                        host.log(&e);
                        PileUse::Handled
                    }
                    Ok(()) => {
                        host.show(
                            dim,
                            &crate::blast::visuals::Show::Sound {
                                id: "minecraft:entity.item.break",
                                source: "block",
                                at: at.map(|v| f64::from(v) + 0.5),
                                volume: 1.,
                                pitch: if undone { 0.75 } else { 1.25 },
                            },
                        );
                        for (p, r) in drops {
                            Self::pile_eject(host, dim, p, r);
                        }
                        PileUse::WearDrill
                    }
                },
            });
        }
        let key = (dim.to_string(), at);
        if let Some(mut machine) = self.ready.world.machines.get(&key).cloned()
            && let Data::Pile(p) = &mut machine.data
            && let Pile::Loader(l) = &mut **p
        {
            if l.level <= 0. && !l.loading {
                if let Some(h) = held
                    && Rod::of(&h.id, depletion).is_some()
                    && l.slots[0].is_none()
                {
                    let Some(expected)=host.machine_slots(dim,at) else{return Some(PileUse::Handled)};
                    // The hand is debited by the caller only after this atomic
                    // slot/state commit, so refusal cannot create or lose a rod.
                    if expected.first().is_none_or(Option::is_some){return Some(PileUse::Handled);}
                    let mut stack=h.clone();stack.count=1;l.slots[0]=Some(stack);
                    let (desired,data)=Self::machine_output(&mut machine,&expected);
                    let json=serde_json::to_string(&machine).expect("machine serialization");
                    let c=MachineCommit{cells:Vec::new(),guard:CellGuard::machine(&key,&machine),expected_saved:self.ready.native_saved.contains_key(&key),dim:dim.into(),pos:at,expected,desired,data,json};
                    if host.commit_machine(&c){
                        self.ready.world.machines.insert(key.clone(),machine);
                        self.ready.bridge.last.insert(key.clone(),c.desired);
                        self.ready.bridge.data.insert(key.clone(),c.data);
                        self.ready.native_saved.insert(key,c.json);
                        host.show(dim,&crate::blast::visuals::Show::Sound{id:"ntm:item.upgradeplug",source:"player",at:at.map(|v|f64::from(v)+0.5),volume:1.,pitch:1.});
                        return Some(PileUse::ConsumeRod);
                    }
                    return Some(PileUse::Handled);
                }
                l.loading = true;
                self.ready.world.machines.insert(key,machine);
            }
            return Some(PileUse::Handled);
        }
        if held.is_none()
            && let Some(key) = self.pile_core_at(dim, at)
            && let Data::Pile(p) = &self.ready.world.machines[&key].data
            && let Pile::Core(Some(c)) = &**p
        {
            host.tell(
                player,
                &format!(
                    "Chicago Pile: {:.0} / 800°C; {} channels",
                    c.highest_heat,
                    c.channels.len()
                ),
            );
        }
        // Bare bricks must still allow placing more bricks/devices.
        (id != pile::BRICK && held.is_none()).then_some(PileUse::Handled)
    }
    pub(super) fn pile_tick(&mut self, host: &mut impl ServerHost) {
        // A dimension the host cannot reach (not open) freezes, like unloaded
        // cells, instead of losing rods: drops and blasts go through the host,
        // which switches levels itself only when one is really needed.
        let dims: BTreeSet<String> = self
            .ready
            .world
            .machines
            .iter()
            .filter(|(_, m)| matches!(&m.data, Data::Pile(_)))
            .map(|(k, _)| k.0.clone())
            .collect();
        for dim in dims {
            if host.pile_reachable(&dim) {
                self.pile_tick_dim(host, &dim);
            }
        }
    }
    fn pile_tick_dim(&mut self, host: &mut impl ServerHost, dim: &str) {
        let live: BTreeSet<String> = BTreeSet::from([dim.to_string()]);
        let devices:Vec<_>=self.ready.world.machines.iter().filter(|(k,m)|live.contains(&k.0) && self.ready.loaded.get(*k)==Some(&true) && matches!(&m.data,Data::Pile(p) if matches!(&**p,Pile::Loader(_)|Pile::Vent(_)|Pile::Control(_)))).map(|(k,_)|k.clone()).collect();
        // Loader and control read the signal their own cell receives, one batch
        // per dimension (the vent has no redstone input).
        let mut wanted: BTreeMap<String, Vec<Pos>> = BTreeMap::new();
        for key in &devices {
            if matches!(&self.ready.world.machines[key].data, Data::Pile(p) if matches!(&**p, Pile::Loader(_) | Pile::Control(_))) {
                wanted.entry(key.0.clone()).or_default().push(key.1);
            }
        }
        let mut powered: BTreeSet<Key> = BTreeSet::new();
        for (dim, cells) in wanted {
            for (cell, level) in cells.iter().zip(host.redstone_many(&dim, &cells)) {
                if level.unwrap_or(0) > 0 {
                    powered.insert((dim.clone(), *cell));
                }
            }
        }
        for key in devices {
            let Some(mut m) = self.ready.world.machines.remove(&key) else {
                continue;
            };
            let outward = m
                .facing
                .as_deref()
                .and_then(registry::facing_dir)
                .unwrap_or([0, 0, -1]);
            let Data::Pile(p) = &mut m.data else {
                unreachable!()
            };
            let at = if matches!(&**p, Pile::Control(_)) {
                pile::offset(key.1, [0, -1, 0], 1)
            } else {
                pile::offset(key.1, outward, -1)
            };
            let ck = self.pile_core_at(&key.0, at);
            let channel = ck
                .as_ref()
                .filter(|k| self.ready.loaded.get(*k) == Some(&true))
                .and_then(|k| self.ready.world.machines.get_mut(k))
                .and_then(|m| {
                    if let Data::Pile(p) = &mut m.data
                        && let Pile::Core(Some(c)) = &mut **p
                    {
                        c.channels.iter_mut().find(|c| c.entry == at)
                    } else {
                        None
                    }
                });
            let red = powered.contains(&key);
            match &mut **p {
                Pile::Loader(l) => {
                    if l.stroke(red)
                        && let Some(c) = channel.filter(|c| c.kind == ChannelType::Fuel)
                        && let Some(stack) = l.slots[0].as_ref()
                        && let Some(rod) = Rod::of(&stack.id, host.pile_depletion(stack))
                    {
                        // Slots and core are changed together before native_save.
                        if let Some(r) = c.load(rod) {
                            Self::pile_eject(host, &key.0, c.exit(), r);
                        }
                        l.slots[0] = None;
                    }
                }
                Pile::Vent(t) => {
                    if let Some(c) = channel.filter(|c| c.kind == ChannelType::Ventilation) {
                        let n = t.mb.min(1000u32.saturating_sub(c.air));
                        c.air += n;
                        t.drain(n);
                    }
                }
                Pile::Control(control) => {
                    let can = channel.is_some_and(|c| {
                        if c.kind == ChannelType::Control {
                            c.control = control.level;
                            true
                        } else {
                            false
                        }
                    });
                    control.step(can, red);
                }
                _ => {}
            }
            self.ready.world.machines.insert(key, m);
        }
        let cores: Vec<_> = self
            .ready
            .world
            .machines
            .iter()
            .filter(|(k, m)| {
                live.contains(&k.0)
                    && self.ready.loaded.get(*k) == Some(&true)
                    && matches!(&m.data,Data::Pile(p) if matches!(&**p,Pile::Core(Some(_))))
            })
            .map(|(k, _)| k.clone())
            .collect();
        let mut melted = false;
        for key in cores {
            let Data::Pile(p) = &self.ready.world.machines[&key].data else {
                continue;
            };
            let Pile::Core(Some(c)) = &**p else {
                continue;
            };
            let c = c.clone();
            let mut foreign = false;
            let mut unloaded = false;
            for (at, role) in c.cells() {
                match host.get_block(&key.0, at) {
                    None => unloaded = true,
                    Some(b) => {
                        foreign |= blockstate::id_of(&b) != format!("ntm:pile_block__{role}")
                    }
                }
            }
            if unloaded {
                continue;
            }
            if foreign {
                self.pile_destroy(host, &key.0, &c);
                continue;
            }
            let Data::Pile(p) = &mut self.ready.world.machines.get_mut(&key).expect("core").data
            else {
                unreachable!()
            };
            let Pile::Core(Some(c)) = &mut **p else {
                unreachable!()
            };
            if c.step() {
                let c = c.clone();
                let center = c.blast_center();
                // As in Java only the core reverts; the explosion runs against
                // the standing members and the orphan sweep reverts survivors
                // on the next tick.
                self.pile_dismantle(host, &key.0, &c, true);
                melted = true;
                // The engine supplies real fire, block destruction and entity
                // damage; EntityBulletBaseMK4's 15 custom debris are unavailable.
                if !host.pile_explode(&key.0, center) {
                    host.log("pile meltdown: explosion not applied (chunk not loaded)");
                }
                host.log("pile meltdown: 15 EntityBulletBaseMK4 debris not emitted (custom NTM projectile unavailable)");
            }
        }
        // A member whose core is definitely gone reverts after reopening,
        // while an unloaded core is left alone until it can be adopted.
        let orphaned: Vec<_> = self
            .ready
            .world
            .machines
            .iter()
            .filter_map(|(k, m)| {
                if !melted
                    && live.contains(&k.0)
                    && let Data::Pile(p) = &m.data
                    && let Pile::Member(Some(core)) = &**p
                    && !self
                        .ready
                        .world
                        .machines
                        .contains_key(&(k.0.clone(), *core))
                    && host
                        .get_block(&k.0, *core)
                        .is_some_and(|b| blockstate::id_of(&b) != pile::CORE)
                {
                    Some(k.clone())
                } else {
                    None
                }
            })
            .collect();
        for key in orphaned {
            if host
                .get_block(&key.0, key.1)
                .is_some_and(|b| blockstate::id_of(&b).starts_with("ntm:pile_block__"))
            {
                let _ = host.set_blocks(&key.0, &[(key.1, pile::BRICK.into())]);
            }
            self.ready.world.machines.remove(&key);
            self.ready.loaded.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kv::MemKv;
    #[derive(Default)]
    struct Host {
        blocks: BTreeMap<Pos, String>,
        slots: BTreeMap<Pos, Vec<Option<Stack>>>,
        saved: BTreeMap<Pos, String>,
        changes: Vec<(Pos, String, String)>,
        drops: Vec<(Pos, String)>,
        signal: BTreeMap<Pos, u8>,
        kv: MemKv,
        blasts: Vec<[f64; 3]>,
        unloaded: BTreeSet<Pos>,
        inactive: BTreeSet<String>,
        refuse_commit:bool,
        refuse_items:bool,
        drop_budget:Option<usize>,
        /// The next formation is refused after its preflight (nothing may change).
        refuse_formation:bool,
        /// The drill's wear applied with a formation.
        worn:usize,
        /// First JSON published with a formation's block entities.
        first_json:BTreeMap<Pos,String>,
        /// Journal writes of the last sweep tick (`melt`).
        sweep_sets:u64,
        /// For each journal write, how many `Items` effects it still held pending.
        pending_items_at_sets:Vec<usize>,
    }
    impl Kv for Host {
        fn get(&mut self, k: &str) -> Option<Vec<u8>> {
            self.kv.get(k)
        }
        fn set(&mut self, k: &str, v: &[u8]) -> Result<(), String> {
            if k == crate::persist::KEY
                && let Ok(kept) = serde_json::from_slice::<crate::persist::Kept>(v)
            {
                let items = kept.machine_effects.iter().flat_map(|b| &b.effects).filter(|e| matches!(e, WorldEffect::Items { .. })).count();
                self.pending_items_at_sets.push(items);
            }
            self.kv.set(k, v)
        }
        fn delete(&mut self, k: &str) {
            self.kv.delete(k)
        }
    }
    impl ServerHost for Host {
        fn commit_structural(&mut self,cells:&[CellWrite],drops:&[RetainedDrop],owners:&[(String,Pos)])->bool{
            if !drops.is_empty(){return false;}
            if cells.iter().any(|c|self.cell_snapshot(&c.guard.dim,c.guard.pos).as_ref()!=Some(&c.expected)){return false;}
            let dims:BTreeSet<_>=cells.iter().map(|c|c.guard.dim.clone()).collect();
            if dims.len()>1{return false;}
            let Some(dim)=dims.first()else{return true};
            let blocks:Vec<_>=cells.iter().map(|c|(c.guard.pos,c.desired.clone())).collect();
            self.set_machine_blocks(dim,&blocks,owners).is_ok()
        }

        fn commit_formation(&mut self,cells:&[CellWrite],wear_held:bool,owners:&[(String,Pos)])->bool{
            if self.refuse_formation||!self.commit_structural(cells,&[],owners){return false;}
            for c in cells{if let Some(i)=&c.initial{self.first_json.insert(c.guard.pos,i.json.clone());}}
            if wear_held{self.worn+=1;}
            true
        }
        fn commit_machine(&mut self,c:&MachineCommit)->bool{self.commit_machines(std::slice::from_ref(c))}
        fn commit_machines(&mut self,cs:&[MachineCommit])->bool{
            if self.refuse_commit || cs.iter().any(|c|self.inactive.contains(&c.dim)||self.unloaded.contains(&c.pos)||self.slots.get(&c.pos).cloned().unwrap_or_default()!=c.expected){return false;}
            for c in cs {self.slots.insert(c.pos,c.desired.clone());self.saved.insert(c.pos,c.json.clone());}true
        }

        fn get_block(&mut self, _: &str, p: Pos) -> Option<String> {
            (!self.unloaded.contains(&p)).then(|| {
                self.blocks
                    .get(&p)
                    .cloned()
                    .unwrap_or("minecraft:air".into())
            })
        }
        fn set_blocks(&mut self, _: &str, blocks: &[(Pos, String)]) -> Result<(), String> {
            for (p, s) in blocks {
                let old = self
                    .blocks
                    .insert(*p, s.clone())
                    .unwrap_or("minecraft:air".into());
                self.changes.push((*p, old, s.clone()));
            }
            Ok(())
        }
        fn set_machine_blocks(&mut self,dim:&str,blocks:&[(Pos,String)],owners:&[(String,Pos)])->Result<(),String>{
            let first=self.changes.len();self.set_blocks(dim,blocks)?;
            let events=self.changes.split_off(first);
            self.changes.extend(events.into_iter().filter(|(p,_,_)|!owners.contains(&(dim.to_string(),*p))));Ok(())
        }
        fn tell(&mut self, _: &str, _: &str) {}
        fn log(&mut self, _: &str) {}
        fn machine_slots(&mut self, _: &str, p: Pos) -> Option<Vec<Option<Stack>>> {
            self.slots.get(&p).cloned()
        }
        fn drop_items(&mut self,_:&str,p:Pos,items:&[(String,u32)])->bool{
            if self.refuse_items{return false;}
            if let Some(budget)=&mut self.drop_budget {if *budget==0{return false;}*budget-=1;}
            for (id,n) in items {for _ in 0..*n{self.drops.push((p,id.clone()));}}true
        }
        fn pile_explode(&mut self, _: &str, at: [f64; 3]) -> bool {
            self.blasts.push(at);
            true
        }
        fn pile_reachable(&mut self, dim: &str) -> bool {
            !self.inactive.contains(dim)
        }
        fn redstone_many(&mut self, _: &str, cells: &[Pos]) -> Vec<Option<u8>> {
            cells
                .iter()
                .map(|p| Some(self.signal.get(p).copied().unwrap_or(0)))
                .collect()
        }
    }
    const DIM: &str = "minecraft:overworld";
    const AT: Pos = [0, 64, 0];
    fn core(s: &mut Server) -> &mut Core {
        let Data::Pile(p) = &mut s
            .ready
            .world
            .machines
            .get_mut(&(DIM.into(), AT))
            .unwrap()
            .data
        else {
            panic!()
        };
        let Pile::Core(Some(c)) = &mut **p else {
            panic!()
        };
        c
    }
    fn replay(s: &mut Server, h: &mut Host) {
        for (p, old, new) in std::mem::take(&mut h.changes) {
            s.native_block_changed(DIM, p, &old, &new);
        }
    }
    fn formed() -> (Server, Host) {
        let mut h = Host::default();
        let mut s = Server::native(&mut h, 0);
        for x in -2..=2 {
            for y in 62..=66 {
                for z in 0..5 {
                    h.blocks.insert([x, y, z], pile::BRICK.into());
                }
            }
        }
        let tool = Stack::new("ntm:hand_drill", 1);
        assert_eq!(
            s.native_pile_use(&mut h, DIM, AT, [0, 0, -1], Some(&tool), 0., "p"),
            Some(PileUse::Handled)
        );
        assert_eq!(h.worn, 1, "the wear was part of the formation");
        replay(&mut s, &mut h);
        s.native_tick(&mut h, 1);
        s.native_world_saved(&mut h,true);
        (s, h)
    }
    fn drill(s: &mut Server, h: &mut Host, p: Pos, face: Pos) {
        let tool = Stack::new("ntm:hand_drill_desh", 1);
        assert_eq!(
            s.native_pile_use(h, DIM, p, face, Some(&tool), 0., "p"),
            Some(PileUse::WearDrill)
        );
        replay(s, h);
        s.native_tick(h, 2);
        s.native_world_saved(h,true);
    }
    fn bricks(h: &mut Host) {
        for x in -2..=2 {
            for y in 62..=66 {
                for z in 0..5 {
                    h.blocks.insert([x, y, z], pile::BRICK.into());
                }
            }
        }
    }
    /// The formation is one step: the bricks, the model, every block entity's first JSON and the
    /// drill's wear all come together, and a refusal at the host leaves each as it was.
    #[test]
    fn formation_publishes_blocks_first_state_and_wear_together_or_not_at_all() {
        let tool = Stack::new("ntm:hand_drill", 1);
        let mut h = Host::default();
        let mut s = Server::native(&mut h, 0);
        bricks(&mut h);
        let bricks_before = h.blocks.clone();
        h.refuse_formation = true;
        assert_eq!(
            s.native_pile_use(&mut h, DIM, AT, [0, 0, -1], Some(&tool), 0., "p"),
            Some(PileUse::Handled)
        );
        assert_eq!(h.blocks, bricks_before, "no brick changed");
        assert!(s.ready.world.machines.is_empty(), "no model");
        assert_eq!((h.worn, h.first_json.len()), (0, 0), "no wear, no state");
        h.refuse_formation = false;
        assert_eq!(
            s.native_pile_use(&mut h, DIM, AT, [0, 0, -1], Some(&tool), 0., "p"),
            Some(PileUse::Handled)
        );
        assert_eq!(h.worn, 1);
        assert_eq!(s.ready.world.machines.len(), 125);
        assert_eq!(h.first_json.len(), 125, "each cell got its JSON with the cell");
        for (k, m) in &s.ready.world.machines {
            assert_eq!(s.ready.native_saved.get(k), h.first_json.get(&k.1), "{k:?}");
            assert!(h.blocks[&k.1].starts_with(&m.block));
        }
        // The desh drill does not wear.
        let mut h2 = Host::default();
        let mut s2 = Server::native(&mut h2, 0);
        bricks(&mut h2);
        let desh = Stack::new("ntm:hand_drill_desh", 1);
        s2.native_pile_use(&mut h2, DIM, AT, [0, 0, -1], Some(&desh), 0., "p");
        assert_eq!((h2.worn, s2.ready.world.machines.len()), (0, 125));
    }
    /// A cell that is not a plain brick (or not loaded) refuses the whole formation before any
    /// write: the wear stays unspent.
    #[test]
    fn formation_with_an_unloaded_cell_changes_nothing() {
        let tool = Stack::new("ntm:hand_drill", 1);
        let mut h = Host::default();
        let mut s = Server::native(&mut h, 0);
        bricks(&mut h);
        h.unloaded.insert([1, 65, 2]);
        let before = h.blocks.clone();
        s.native_pile_use(&mut h, DIM, AT, [0, 0, -1], Some(&tool), 0., "p");
        assert_eq!(h.blocks, before);
        assert_eq!((h.worn, s.ready.world.machines.len(), h.first_json.len()), (0, 0, 0));
    }
    fn device(s: &mut Server, h: &mut Host, p: Pos, id: &str, facing: &str) {
        let state = format!("{id}[facing={facing},honey_level=0]");
        h.blocks.insert(p, state.clone());
        s.native_block_changed(DIM, p, "minecraft:air", &state);
        if id.ends_with("loader") {
            h.slots.insert(p, vec![None]);
        }
        s.native_tick(h, 3);
    }
    #[test]
    fn native_formation_and_delayed_notifications_keep_channels_and_bindings() {
        let (mut s, mut h) = formed();
        assert_eq!(s.ready.world.machines.len(), 125);
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        assert_eq!(core(&mut s).channels.len(), 1);
        s.native_tick(&mut h, 4);
        assert_eq!(core(&mut s).channels.len(), 1);
        let Data::Pile(p) = &s.ready.world.machines[&(DIM.into(), [-1, 64, 2])].data else {
            panic!()
        };
        assert_eq!(**p, Pile::Member(Some(AT)));
    }
    #[test]
    fn native_loader_consumes_one_rod_then_pushes_and_ejects_at_the_far_end() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        let loader = [-1, 64, -1];
        device(&mut s, &mut h, loader, "ntm:pile_device__loader", "north");
        for n in 0..6 {
            let held = Stack::new(
                if n == 0 {
                    "ntm:pile_rod__pu239"
                } else {
                    "ntm:pile_rod__zr"
                },
                12,
            );
            assert_eq!(
                s.native_pile_use(&mut h, DIM, loader, [0, 0, -1], Some(&held), 0., "p"),
                Some(PileUse::ConsumeRod)
            );
            assert_eq!(h.slots[&loader][0].as_ref().unwrap().count, 1);
            assert_eq!(
                s.native_pile_use(&mut h, DIM, loader, [0, 0, -1], None, 0., "p"),
                Some(PileUse::Handled)
            );
            for t in 0..25 {
                s.native_tick(&mut h, 10 + n * 25 + t);
            }
            assert!(h.slots[&loader][0].is_none());
        }
        assert_eq!(h.drops, vec![([-1, 64, 5], "ntm:pile_rod__pu239".into())]);
    }
    #[test]
    fn native_undo_and_member_break_restore_bricks_and_eject_once() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        core(&mut s).channels[0].load(Rod::of("ntm:pile_rod__nu", 40.).unwrap());
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        assert_eq!(h.drops.len(), 1);
        assert!(core(&mut s).channels.is_empty());
        h.blocks.insert([1, 64, 2], "minecraft:stone".into());
        s.native_tick(&mut h, 5);
        assert!(!s.ready.world.machines.contains_key(&(DIM.into(), AT)));
        assert_eq!(h.blocks[&AT], pile::BRICK);
        assert_eq!(h.blocks[&[1, 64, 2]], "minecraft:stone", "a replacement belongs to its own generation");
        assert_eq!(h.drops.len(), 1);
    }
    #[test]
    fn native_player_core_break_restores_its_cell_and_every_member() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        core(&mut s).channels[0].load(Rod::of("ntm:pile_rod__nu", 100.).unwrap());
        h.blocks.insert(AT, "minecraft:air".into());
        s.native_block_changed(DIM, AT, pile::CORE, "minecraft:air");
        s.native_tick(&mut h, 5);
        assert_eq!(h.drops.len(), 1);
        assert_eq!(h.blocks[&AT], pile::BRICK);
        assert_eq!(h.blocks[&[1, 64, 2]], pile::BRICK);
    }
    #[test]
    fn native_control_redstone_and_front_vent_tank_reach_the_correct_channels() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        drill(&mut s, &mut h, [0, 66, 2], [0, 1, 0]);
        drill(&mut s, &mut h, [-2, 63, 3], [-1, 0, 0]);
        let ctrl = [0, 67, 2];
        let vent = [-3, 63, 3];
        device(&mut s, &mut h, ctrl, "ntm:pile_device__control", "north");
        device(&mut s, &mut h, vent, "ntm:pile_device__vent", "west");
        let Data::Pile(p) = &mut s
            .ready
            .world
            .machines
            .get_mut(&(DIM.into(), vent))
            .unwrap()
            .data
        else {
            panic!()
        };
        let Pile::Vent(t) = &mut **p else { panic!() };
        t.fill("air", 4000);
        h.signal.insert(ctrl, 15);
        for t in 10..75 {
            s.native_tick(&mut h, t);
        }
        let c = core(&mut s);
        assert_eq!(
            c.channels
                .iter()
                .find(|c| c.kind == ChannelType::Control)
                .unwrap()
                .control,
            1.
        );
        assert!(
            c.channels
                .iter()
                .find(|c| c.kind == ChannelType::Ventilation)
                .unwrap()
                .air
                > 0
        );
        let spec = registry::spec("ntm:pile_device__vent").unwrap();
        assert_eq!(
            registry::fluid_ports(spec, Some("west")),
            vec![vec![[-1, 0, 0]]]
        );
    }
    #[test]
    fn native_unloaded_member_freezes_reaction_without_dismantling() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        let c = core(&mut s);
        c.channels[0].load(Rod::of("ntm:pile_rod__nu", 0.).unwrap());
        c.channels[0].incoming = 100.;
        h.unloaded.insert([1, 65, 3]);
        let before = core(&mut s).clone();
        s.native_tick(&mut h, 6);
        assert_eq!(*core(&mut s), before);
        h.unloaded.clear();
        s.native_tick(&mut h, 7);
        assert!(core(&mut s).channels[0].rods[0].as_ref().unwrap().depletion > 0.);
    }
    #[test]
    fn native_meltdown_uses_the_engine_blast_and_never_drops_rods_twice() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        let c = core(&mut s);
        c.channels[0].load(Rod::of("ntm:pile_rod__waste", 0.).unwrap());
        c.channels[0].heat = 802.;
        s.native_tick(&mut h, 8);
        assert_eq!(h.blasts, vec![[-0.5, 66., 2.5]]);
        assert_eq!(h.drops.len(), 1);
        // Java destroy(): only the core turns to brick before the blast; the
        // surviving members revert on the following tick (orphan sweep).
        assert_eq!(h.blocks[&AT], pile::BRICK);
        assert!(h.blocks[&[1, 64, 2]].starts_with("ntm:pile_block__"));
        replay(&mut s, &mut h);
        s.native_tick(&mut h, 9);
        assert_eq!(h.blocks[&[1, 64, 2]], pile::BRICK);
        // Every standing member, not only one: the sweep leaves no `pile_block` behind.
        assert!(h.blocks.values().all(|b| !b.starts_with("ntm:pile_block__")), "{:?}", h.blocks.iter().filter(|(_, b)| b.starts_with("ntm:pile_block__")).count());
        assert_eq!(h.drops.len(), 1);
        assert_eq!(h.blasts.len(), 1);
    }
    /// The meltdown of `formed()`'s pile, its blast tick and then its sweep tick, journaled before
    /// every effect (`strict`) or as the server does. What the world ends as, and the writes it took.
    fn melt(strict: bool, items_first: bool) -> Host {
        let (mut s, mut h) = formed();
        s.ready.strict_journal = strict;
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        let c = core(&mut s);
        c.channels[0].load(Rod::of("ntm:pile_rod__waste", 0.).unwrap());
        c.channels[0].heat = 802.;
        s.native_tick(&mut h, 8);
        replay(&mut s, &mut h);
        if items_first {
            // An effect that must not repeat, ahead of the sweep's.
            s.ready.world.machine_effects.push_front(CommittedEffects {
                origins: vec![],
                owners: vec![],
                tombstones: vec![],
                effects: std::collections::VecDeque::from([WorldEffect::Items { dim: DIM.into(), pos: [0, 70, 0], items: vec![("ntm:pile_rod__zr".into(), 1)] }]),
            });
        }
        let before = h.kv.sets;
        h.pending_items_at_sets.clear();
        s.native_tick(&mut h, 9);
        assert!(h.blocks.values().all(|b| !b.starts_with("ntm:pile_block__")));
        assert!(s.ready.world.machine_effects.is_empty());
        h.sweep_sets = h.kv.sets - before;
        h
    }
    #[test]
    fn the_meltdown_sweep_journals_once_and_ends_as_a_write_per_effect() {
        let strict = melt(true, false);
        let fast = melt(false, false);
        // The same world: blocks, drops, blast, and the journal the server ends with.
        assert_eq!(fast.blocks, strict.blocks);
        assert_eq!(fast.drops, strict.drops);
        assert_eq!(fast.blasts, strict.blasts);
        assert_eq!(fast.kv.map["world"], strict.kv.map["world"]);
        // 124 members: a write before each of them, against one before the first and one at the end.
        assert!(strict.sweep_sets > 100, "{}", strict.sweep_sets);
        assert!(fast.sweep_sets <= 3, "{}", fast.sweep_sets);
        assert!(fast.kv.written * 20 < strict.kv.written, "{} against {} bytes", fast.kv.written, strict.kv.written);
    }
    #[test]
    fn an_effect_that_must_not_repeat_is_journaled_before_the_next_one_goes_out() {
        let strict = melt(true, true);
        let fast = melt(false, true);
        assert_eq!(fast.blocks, strict.blocks);
        assert_eq!(fast.drops, strict.drops);
        assert_eq!(fast.kv.map["world"], strict.kv.map["world"]);
        assert_eq!(fast.drops.iter().filter(|d| d.1 == "ntm:pile_rod__zr").count(), 1);
        // Items (write, final), then the first block after them (write, because the drop went out), then the final.
        assert!(fast.sweep_sets <= 5, "{}", fast.sweep_sets);
        // Only the first write still holds the items; every later one, the first block's included, has them done.
        assert_eq!(fast.pending_items_at_sets.first(), Some(&1), "{:?}", fast.pending_items_at_sets);
        assert!(fast.pending_items_at_sets.iter().skip(1).all(|n| *n == 0), "{:?}", fast.pending_items_at_sets);
    }
    #[test]
    fn native_loader_reads_the_signal_of_its_own_cell_only() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        let loader = [-1, 64, -1];
        device(&mut s, &mut h, loader, "ntm:pile_device__loader", "north");
        let rod = Stack::new("ntm:pile_rod__nu", 1);
        assert_eq!(
            s.native_pile_use(&mut h, DIM, loader, [0, 0, -1], Some(&rod), 0., "p"),
            Some(PileUse::ConsumeRod)
        );
        // The outer neighbour used to be read; its signal no longer counts.
        h.signal.insert([-1, 64, -2], 15);
        for t in 10..40 {
            s.native_tick(&mut h, t);
        }
        assert!(h.slots[&loader][0].is_some());
        h.signal.insert(loader, 15);
        for t in 40..70 {
            s.native_tick(&mut h, t);
        }
        assert!(h.slots[&loader][0].is_none());
        assert_eq!(core(&mut s).channels[0].rods.iter().flatten().count(), 1);
    }
    #[test]
    fn native_pile_outside_the_active_dimension_freezes_without_losing_rods() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        let c = core(&mut s);
        c.channels[0].load(Rod::of("ntm:pile_rod__waste", 0.).unwrap());
        c.channels[0].heat = 802.;
        h.inactive.insert(DIM.into());
        s.native_tick(&mut h, 8);
        // No explosion, no drop, no brick: everything waits for the dimension.
        assert!(h.blasts.is_empty() && h.drops.is_empty());
        assert!(h.blocks[&AT].starts_with("ntm:pile_block__"));
        assert_eq!(core(&mut s).channels[0].heat, 802.);
        // A broken member in that dimension does not dismantle it either.
        let old = h.blocks[&[1, 64, 2]].clone();
        h.blocks.insert([1, 64, 2], "minecraft:air".into());
        s.native_block_changed(DIM, [1, 64, 2], &old, "minecraft:air");
        s.native_tick(&mut h, 9);
        assert!(h.drops.is_empty() && s.ready.world.machines.contains_key(&(DIM.into(), AT)));
        h.inactive.clear();
        s.native_tick(&mut h, 10);
        assert_eq!(h.drops.len(), 1);
        assert!(!s.ready.world.machines.contains_key(&(DIM.into(), AT)));
        assert_eq!(h.blocks[&AT], pile::BRICK);
    }
    #[test]
    fn loader_core_and_ejection_conserve_rods_across_refusal_and_player_withdrawal() {
        for withdraw in [false,true] {
            let (mut s,mut h)=formed();drill(&mut s,&mut h,[-1,64,0],[0,0,-1]);
            let at=[-1,64,-1];device(&mut s,&mut h,at,"ntm:pile_device__loader","north");
            for r in &mut core(&mut s).channels[0].rods{*r=Rod::of("ntm:pile_rod__zr",0.);}
            let held=core(&mut s).channels[0].rods.len();
            h.slots.get_mut(&at).unwrap()[0]=Some(Stack::new("ntm:pile_rod__zr",1));
            let Data::Pile(p)=&mut s.ready.world.machines.get_mut(&(DIM.into(),at)).unwrap().data else{panic!()};
            let Pile::Loader(l)=&mut **p else{panic!()};l.level=1.;l.loading=false;l.delay=0;
            h.refuse_commit=true;s.native_tick(&mut h,100);
            assert!(h.drops.is_empty());assert!(h.slots[&at][0].is_some());
            assert_eq!(core(&mut s).channels[0].rods.iter().flatten().count(),held);
            let player=if withdraw {h.slots.get_mut(&at).unwrap()[0].take()}else{None};
            h.refuse_commit=false;h.refuse_items=true;s.native_tick(&mut h,101);
            let pending:usize=s.ready.world.machine_effects.iter().flat_map(|b|&b.effects).map(|e|match e {WorldEffect::Items{items,..}=>items.iter().map(|i|i.1 as usize).sum(),_=>0}).sum();
            assert_eq!(core(&mut s).channels[0].rods.iter().flatten().count()+usize::from(player.is_some())+pending,held+1);
            h.refuse_items=false;for t in 102..106{s.native_tick(&mut h,t);}
            assert_eq!(h.drops.len(),usize::from(!withdraw));
            assert_eq!(core(&mut s).channels[0].rods.iter().flatten().count()+usize::from(player.is_some())+h.drops.len(),held+1);
        }
    }
    #[test]
    fn drilling_out_a_channel_does_not_eject_before_the_core_commit() {
        let (mut s,mut h)=formed();drill(&mut s,&mut h,[-1,64,0],[0,0,-1]);
        core(&mut s).channels[0].load(Rod::of("ntm:pile_rod__zr",0.).unwrap());
        let tool=Stack::new("ntm:hand_drill_desh",1);h.refuse_commit=true;
        assert_eq!(s.native_pile_use(&mut h,DIM,[-1,64,0],[0,0,-1],Some(&tool),0.,"p"),Some(PileUse::Handled));
        assert!(h.drops.is_empty());assert_eq!(core(&mut s).channels[0].rods.iter().flatten().count(),1);
        h.refuse_commit=false;
        assert_eq!(s.native_pile_use(&mut h,DIM,[-1,64,0],[0,0,-1],Some(&tool),0.,"p"),Some(PileUse::WearDrill));
        assert_eq!(h.drops.len(),1);assert!(core(&mut s).channels.is_empty());
    }
    #[test]
    fn a_partial_ejection_retry_and_clean_reopen_never_reemit_the_first_rod() {
        let (mut s,mut h)=formed();drill(&mut s,&mut h,[-1,64,0],[0,0,-1]);
        core(&mut s).channels[0].load(Rod::of("ntm:pile_rod__nu",0.).unwrap());
        core(&mut s).channels[0].load(Rod::of("ntm:pile_rod__zr",0.).unwrap());
        h.blocks.insert(AT,"minecraft:air".into());s.native_block_changed(DIM,AT,pile::CORE,"minecraft:air");
        h.drop_budget=Some(1);s.native_tick(&mut h,5);
        assert_eq!(h.drops,vec![([-1,64,5],"ntm:pile_rod__zr".into())]);
        let pending:usize=s.ready.world.machine_effects.iter().flat_map(|b|&b.effects).map(|e|match e{WorldEffect::Items{items,..}=>items.iter().map(|i|i.1 as usize).sum(),_=>0}).sum();
        assert_eq!(pending+h.drops.len(),2,"remaining intents plus world items conserve both rods");
        s.native_world_saved(&mut h,true);
        let mut reopened=Server::native(&mut h,5);reopened.native_tick(&mut h,6);
        assert_eq!(h.drops.len(),1);
        h.drop_budget=None;for t in 7..11{reopened.native_tick(&mut h,t);}
        assert_eq!(h.drops,vec![([-1,64,5],"ntm:pile_rod__zr".into()),([-1,64,5],"ntm:pile_rod__nu".into())]);
        assert!(reopened.ready.world.machine_effects.is_empty());
    }
    #[test]
    fn effect_receipts_clear_only_after_a_flushed_world_save() {
        let mut h=Host::default();let mut s=Server::native(&mut h,0);
        s.ready.world.machine_effects.push_back(CommittedEffects{origins:vec![],owners:vec![],tombstones:vec![],effects:std::collections::VecDeque::from([WorldEffect::Items{dim:DIM.into(),pos:AT,items:vec![("ntm:pile_rod__zr".into(),1)]}]),..Default::default()});
        s.flush_machine_effects(&mut h);assert_eq!(h.drops.len(),1);
        s.flush_machine_effects(&mut h);assert_eq!(h.drops.len(),1,"no repeat in memory");
        s.native_world_saved(&mut h,false);
        let unflushed=WorldState::load(&mut h);assert_eq!(unflushed.machine_effects.len(),1,"unflushed effects replay rather than lose an unsaved spawn");
        h.kv.refuse.insert(crate::persist::KEY.into());s.native_world_saved(&mut h,true);
        assert!(!s.ready.world.applied_machine_effects.is_empty(),"failed receipt write remains retryable");
        h.kv.refuse.clear();s.native_world_saved(&mut h,true);
        assert!(WorldState::load(&mut h).machine_effects.is_empty());
        let mut reopened=Server::native(&mut h,0);reopened.native_tick(&mut h,1);
        assert_eq!(h.drops.len(),1,"clean reopen cannot eject again");
    }
    #[test]
    fn a_confirmed_core_tombstone_retries_without_duplicate_rods_even_after_reopening() {
        let (mut s,mut h)=formed();drill(&mut s,&mut h,[-1,64,0],[0,0,-1]);
        core(&mut s).channels[0].load(Rod::of("ntm:pile_rod__nu",100.).unwrap());
        h.blocks.insert(AT,"minecraft:air".into());s.native_block_changed(DIM,AT,pile::CORE,"minecraft:air");
        h.refuse_commit=true;s.native_tick(&mut h,1);
        assert!(h.drops.is_empty());assert_eq!(core(&mut s).channels[0].rods.iter().flatten().count(),1);
        h.refuse_commit=false;h.refuse_items=true;s.native_tick(&mut h,2);
        assert!(h.drops.is_empty());
        let pending=WorldState::load(&mut h);assert!(!pending.machine_effects.is_empty());
        let rods:usize=pending.machine_effects.iter().flat_map(|b|&b.tombstones).filter_map(|(_,m)|match &m.data{Data::Pile(p)=>match &**p{Pile::Core(Some(c))=>Some(c.channels.iter().map(|c|c.rods.iter().flatten().count()).sum::<usize>()),_=>None},_=>None}).sum();
        assert_eq!(rods,1,"the pending tombstone retains every rod");
        let mut again=Server::native(&mut h,2);
        for (p,json) in h.saved.clone(){assert!(again.native_adopt(DIM,p,&json));}
        for t in 3..6{again.native_tick(&mut h,t);}assert!(h.drops.is_empty());
        h.refuse_items=false;for t in 6..10{again.native_tick(&mut h,t);}
        assert_eq!(h.drops.iter().filter(|(_,id)|id=="ntm:pile_rod__nu").count(),1);
        assert!(again.ready.world.machine_effects.is_empty());
    }
    #[test]
    fn a_confirmed_break_in_an_inaccessible_level_keeps_its_tombstone_across_reopen() {
        let (mut s,mut h)=formed();drill(&mut s,&mut h,[-1,64,0],[0,0,-1]);
        core(&mut s).channels[0].load(Rod::of("ntm:pile_rod__zr",0.).unwrap());
        h.inactive.insert(DIM.into());h.blocks.insert(AT,"minecraft:air".into());
        s.native_block_changed(DIM,AT,pile::CORE,"minecraft:air");s.native_tick(&mut h,5);
        assert!(h.drops.is_empty());assert_eq!(WorldState::load(&mut h).removals.len(),1);
        let mut reopened=Server::native(&mut h,5);assert_eq!(core(&mut reopened).channels[0].rods.iter().flatten().count(),1);
        reopened.native_tick(&mut h,6);assert!(h.drops.is_empty());
        h.inactive.clear();for t in 7..10{reopened.native_tick(&mut h,t);}
        assert_eq!(h.drops.len(),1);assert!(reopened.ready.world.removals.is_empty());
    }
    #[test]
    fn native_core_break_in_a_frozen_dimension_keeps_its_rods_until_it_can_be_entered() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        core(&mut s).channels[0].load(Rod::of("ntm:pile_rod__nu", 100.).unwrap());
        h.inactive.insert(DIM.into());
        h.blocks.insert(AT, "minecraft:air".into());
        s.native_block_changed(DIM, AT, pile::CORE, "minecraft:air");
        s.native_tick(&mut h, 5);
        assert!(h.drops.is_empty());
        assert!(s.ready.world.machines.contains_key(&(DIM.into(), AT)));
        h.inactive.clear();
        s.native_tick(&mut h, 6);
        assert_eq!(h.drops.len(), 1);
        assert!(!s.ready.world.machines.contains_key(&(DIM.into(), AT)));
        assert_eq!(h.blocks[&[1, 64, 2]], pile::BRICK);
    }
    #[test]
    fn native_member_break_waits_for_the_whole_atomic_group_to_be_accessible() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        core(&mut s).channels[0].load(Rod::of("ntm:pile_rod__nu", 100.).unwrap());
        h.unloaded.insert([2, 66, 4]);
        let old = h.blocks[&[1, 64, 2]].clone();
        h.blocks.insert([1, 64, 2], "minecraft:air".into());
        s.native_block_changed(DIM, [1, 64, 2], &old, "minecraft:air");
        s.native_tick(&mut h, 5);
        assert!(h.drops.is_empty(),"an inaccessible member cannot be committed as a tombstone");
        assert!(s.ready.world.machines.contains_key(&(DIM.into(),AT)));
        h.unloaded.clear();s.native_tick(&mut h,6);
        assert_eq!(h.drops.len(), 1);
        assert!(!s.ready.world.machines.contains_key(&(DIM.into(), AT)));
        assert_eq!(h.blocks[&[1, 64, 2]], pile::BRICK);
        assert_eq!(h.blocks[&AT], pile::BRICK);
    }
    #[test]
    fn native_vent_with_a_full_channel_never_underflows() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-2, 63, 3], [-1, 0, 0]);
        let vent = [-3, 63, 3];
        device(&mut s, &mut h, vent, "ntm:pile_device__vent", "west");
        let Data::Pile(p) = &mut s.ready.world.machines.get_mut(&(DIM.into(), vent)).unwrap().data else { panic!() };
        let Pile::Vent(t) = &mut **p else { panic!() };
        t.fill("air", 4000);
        for t in 10..30 {
            s.native_tick(&mut h, t);
        }
        let air = core(&mut s).channels[0].air;
        assert!(air <= 1000, "{air}");
    }
    #[test]
    fn native_reopen_keeps_channel_reaction_and_depletion() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        let c = core(&mut s);
        c.channels[0].load(Rod::of("ntm:pile_rod__pu239", 499.9).unwrap());
        c.channels[0].incoming = 50.;
        s.native_tick(&mut h, 6);
        assert_eq!(core(&mut s).channels[0].rods[0].as_ref().unwrap().kind, 5);
        let expected = core(&mut s).clone();
        s.native_world_saved(&mut h,true);
        let mut reloaded = Server::native(&mut h, 0);
        for (p, json) in h.saved.clone() {
            assert!(reloaded.native_adopt(DIM, p, &json));
        }
        assert_eq!(
            core(&mut reloaded).channels,
            expected
                .channels
                .iter()
                .cloned()
                .map(|mut c| {
                    c.outgoing = 0.;
                    c
                })
                .collect::<Vec<_>>()
        );
        reloaded.native_tick(&mut h, 7);
        assert!(
            core(&mut reloaded).channels[0].rods[0]
                .as_ref()
                .unwrap()
                .depletion
                > 0.
        );
    }
    #[test]
    fn native_reopen_mid_stroke_loads_exactly_one_rod() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        let loader = [-1, 64, -1];
        device(&mut s, &mut h, loader, "ntm:pile_device__loader", "north");
        let rod = Stack::new("ntm:pile_rod__nu", 1);
        assert_eq!(
            s.native_pile_use(&mut h, DIM, loader, [0, 0, -1], Some(&rod), 0., "p"),
            Some(PileUse::ConsumeRod)
        );
        s.native_pile_use(&mut h, DIM, loader, [0, 0, -1], None, 0., "p");
        for tick in 10..14 {
            s.native_tick(&mut h, tick);
        }
        s.native_world_saved(&mut h,true);
        let mut reopened = Server::native(&mut h, 0);
        for (p, json) in h.saved.clone() {
            assert!(reopened.native_adopt(DIM, p, &json));
        }
        for tick in 20..50 {
            reopened.native_tick(&mut h, tick);
        }
        assert_eq!(
            core(&mut reopened).channels[0]
                .rods
                .iter()
                .flatten()
                .count(),
            1
        );
        assert!(h.slots[&loader][0].is_none());
        assert!(h.drops.is_empty());
    }
}
