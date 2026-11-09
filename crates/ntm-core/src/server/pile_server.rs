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
    /// A rod went from the hand into a loader, both in the one transfer (`ServerHost::commit_player_transfer`): the
    /// caller has nothing left to debit.
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
            m.rev=self.ready.world.machines.get(&key).map_or(0,|m|m.rev);
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
                    if expected.first().is_none_or(Option::is_some){return Some(PileUse::Handled);}
                    // The rod leaves the hand in the loader's own commit (`stackSize--`, creative too, as
                    // BlockPileDevice does): a refusal on either side moves nothing.
                    let Some(hand)=host.player_slots(player) else{return Some(PileUse::Handled)};
                    let Some(edit)=hand.held().filter(|s|s.stack==*h).and_then(|_|hand.take(hand.hand?,1)) else{return Some(PileUse::Handled)};
                    let mut stack=h.clone();stack.count=1;l.slots[0]=Some(stack);
                    let (desired,data)=Self::machine_output(&mut machine,&expected);
                    let json=serde_json::to_string(&machine).expect("machine serialization");
                    let c=MachineCommit{cells:Vec::new(),guard:CellGuard::machine(&key,&machine),expected_saved:self.ready.native_saved.contains_key(&key),dim:dim.into(),pos:at,expected,desired,data,json};
                    let copies=vec![SlotCopy{dim:dim.into(),pos:at,slot:0,from:edit.slot}];
                    let transfer=PlayerTransfer{player:player.into(),edits:vec![edit],drops:Vec::new(),copies};
                    if host.commit_player_transfer(&transfer,std::slice::from_ref(&c)){
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
        /// Items applied by id once (`replays_by_id`), as the native host's ledger does.
        by_id:Option<BTreeSet<u64>>,
        /// What each chunk's record owes (`store_obligations`), as the native host's chunk attachments hold it.
        homes:BTreeMap<[i32;2],(Vec<crate::persist::Removal>,Vec<CommittedEffects>)>,
        /// What each chunk's record took of an earlier version's storage (`migrated`).
        marks:BTreeMap<[i32;2],BTreeSet<crate::persist::Obligation>>,
        /// The acting player's stacks (`player_slots`), as the native host reads them.
        hands:Option<PlayerSlots>,
        /// The next transfer is refused after its preflight (nothing may change on either side).
        refuse_player:bool,
    }
    /// Puts `stack` in the acting player's hand (slot 0): the stack a use hands the server.
    fn hold(h:&mut Host,stack:&Stack){
        let held=HeldStack{stack:stack.clone(),encoded:format!("{stack:?}")};
        h.hands=Some(PlayerSlots{hand:Some(PlayerSlot::Inventory(0)),inventory:vec![Some(held),None],..Default::default()});
    }
    /// The world opened again from the host's records: the storage, then what each chunk's record owes, as the
    /// native host gives it when the chunk loads (before its block entities are adopted).
    fn reopen(h:&mut Host,tick:u64)->Server{
        let mut s=Server::native(h,tick);
        let chunks:BTreeSet<[i32;2]>=h.homes.keys().chain(h.marks.keys()).copied().collect();
        for chunk in chunks{
            let (removals,batches)=h.homes.get(&chunk).cloned().unwrap_or_default();
            s.native_obligations_loaded(DIM,chunk,removals,batches,h.marks.get(&chunk).cloned().unwrap_or_default());
        }
        s
    }
    /// The removals, batches and prepared retentions the storage holds, as written.
    fn stored(h:&Host)->usize{
        h.kv.map.get(crate::persist::KEY).and_then(|v|serde_json::from_slice::<crate::persist::Kept>(v).ok()).map_or(0,|k|k.removals.len()+k.machine_effects.len()+k.prepared_retentions.len())
    }
    /// How many of the pending entries a record holds (removals and batches).
    fn owed_in_records(h:&Host)->usize{h.homes.values().map(|(r,b)|r.len()+b.len()).sum()}
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
        fn store_obligations(&mut self,_:&str,chunk:[i32;2],removals:&[crate::persist::Removal],batches:&[CommittedEffects],migrated:&BTreeSet<crate::persist::Obligation>)->bool{
            if removals.is_empty()&&batches.is_empty(){self.homes.remove(&chunk);}else{self.homes.insert(chunk,(removals.to_vec(),batches.to_vec()));}
            if migrated.is_empty(){self.marks.remove(&chunk);}else{self.marks.insert(chunk,migrated.clone());}
            true
        }
        fn home_loaded(&mut self,dim:&str,_:[i32;2])->bool{!self.inactive.contains(dim)}
        fn home_migrated(&mut self,dim:&str,chunk:[i32;2])->Option<BTreeSet<crate::persist::Obligation>>{
            (!self.inactive.contains(dim)).then(||self.marks.get(&chunk).cloned().unwrap_or_default())
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

        fn commit_formation(&mut self,cells:&[CellWrite],wear_held:bool,owners:&[(String,Pos)])->bool{
            if self.refuse_formation||!self.commit_structural(cells,&[],owners){return false;}
            for c in cells{if let Some(i)=&c.initial{self.first_json.insert(c.guard.pos,i.json.clone());}}
            if wear_held{self.worn+=1;}
            true
        }
        fn player_slots(&mut self,_:&str)->Option<PlayerSlots>{self.hands.clone()}
        fn commit_player_transfer(&mut self,t:&PlayerTransfer,cs:&[MachineCommit])->bool{
            let Some(mut hands)=self.hands.clone() else{return false};
            if self.refuse_player||!t.drops.is_empty()||t.edits.iter().any(|e|hands.get(e.slot).map(|s|s.encoded.clone())!=e.expected){return false;}
            if self.refuse_commit || cs.iter().any(|c|self.inactive.contains(&c.dim)||self.unloaded.contains(&c.pos)||self.slots.get(&c.pos).cloned().unwrap_or_default()!=c.expected){return false;}
            for e in &t.edits{
                let new=e.desired.clone().map(|stack|HeldStack{encoded:format!("{stack:?}"),stack});
                match e.slot{PlayerSlot::Inventory(i)=>hands.inventory[i]=new,PlayerSlot::Offhand=>hands.offhand=new,PlayerSlot::Carried=>hands.carried=new}
            }
            self.hands=Some(hands);
            self.commit_machines(cs)
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
        fn replays_by_id(&self)->bool{self.by_id.is_some()}
        fn apply_items(&mut self,id:u64,dim:&str,p:Pos,items:&[(String,u32)],_:&[(String,Pos)])->bool{
            if self.by_id.as_ref().is_some_and(|ids|ids.contains(&id)){return true;}
            let done=self.drop_items(dim,p,items);
            if done && let Some(ids)=&mut self.by_id{ids.insert(id);}
            done
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
            hold(&mut h, &held);
            assert_eq!(
                s.native_pile_use(&mut h, DIM, loader, [0, 0, -1], Some(&held), 0., "p"),
                Some(PileUse::ConsumeRod)
            );
            assert_eq!(h.hands.as_ref().unwrap().held().unwrap().stack.count, 11, "one rod left the hand");
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
    /// Undoing a channel leaves the core a new state that only the core's block entity holds; the batch of
    /// effects journals that state with a revision, so a chunk saved before it can be brought up to it.
    #[test]
    fn an_undo_journals_the_state_it_leaves_in_the_core() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        core(&mut s).channels[0].load(Rod::of("ntm:pile_rod__nu", 40.).unwrap());
        let before = s.ready.world.machines[&(DIM.into(), AT)].rev;
        let tool = Stack::new("ntm:hand_drill_desh", 1);
        assert_eq!(s.native_pile_use(&mut h, DIM, [-1, 64, 0], [0, 0, -1], Some(&tool), 0., "p"), Some(PileUse::WearDrill));
        let journaled = s.ready.world.survivors.get(&(DIM.to_string(), AT)).cloned().expect("the core is journaled");
        let now = &s.ready.world.machines[&(DIM.into(), AT)];
        assert_eq!((journaled.rev, now.rev), (before + 1, before + 1));
        let saved: crate::machine::Machine = serde_json::from_str(&journaled.json).unwrap();
        let Data::Pile(p) = &saved.data else { panic!() };
        let Pile::Core(Some(c)) = &**p else { panic!() };
        assert!(c.channels.is_empty(), "the journal holds the state the step left: the channel is gone");
        assert_eq!(s.ready.native_saved.get(&(DIM.into(), AT)), Some(&journaled.json), "and the block entity got the same");
    }
    /// Cost of a pile step with and without the journal (run with `--ignored --nocapture`, in release).
    #[test]
    #[ignore = "timing, not a check"]
    fn timing_of_pile_steps_and_of_an_undo() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        let mut worst = std::time::Duration::ZERO;
        let all = std::time::Instant::now();
        for t in 0..300 {
            let one = std::time::Instant::now();
            s.native_tick(&mut h, 10 + t);
            worst = worst.max(one.elapsed());
        }
        let mean = all.elapsed() / 300;
        let mut undo = std::time::Duration::ZERO;
        for n in 0..20 {
            if core(&mut s).channels.is_empty() {
                drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
            }
            core(&mut s).channels[0].load(Rod::of("ntm:pile_rod__nu", 40.).unwrap());
            let tool = Stack::new("ntm:hand_drill_desh", 1);
            let one = std::time::Instant::now();
            s.native_pile_use(&mut h, DIM, [-1, 64, 0], [0, 0, -1], Some(&tool), 0., "p");
            undo += one.elapsed();
            s.native_world_saved(&mut h, true);
            let _ = n;
        }
        eprintln!("TIMING tick mean {:?} worst {:?}; undo mean {:?}", mean, worst, undo / 20);
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
    /// The meltdown of `formed()`'s pile, its blast tick and then its sweep tick, with `items` effects ahead of the
    /// sweep, on a host that keeps items by id or not. What the world ends as, and the storage writes it took.
    fn melt_on(items: usize, by_id: bool) -> Host {
        let items_first = items > 0;
        let (mut s, mut h) = formed();
        if by_id {
            h.by_id = Some(BTreeSet::new());
        }
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
                owners: vec![(DIM.into(), AT)],
                tombstones: vec![],
                effects: (0..items).map(|_| WorldEffect::Items { dim: DIM.into(), pos: [0, 70, 0], items: vec![("ntm:pile_rod__zr".into(), 1)] }).collect(),
                ..Default::default()
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
    /// R6-2 / F1: what a meltdown owes (the rods, the sweep of its 124 members) is in its chunk's record within the
    /// tick, never in the storage, whatever effects wait ahead of it; the storage is written a bounded number of times.
    #[test]
    fn a_meltdown_keeps_what_it_owes_in_its_chunks_record_never_in_the_storage() {
        let plain = melt_on(0, false);
        let ahead = melt_on(10, false);
        let by_id = melt_on(10, true);
        assert_eq!(plain.drops.len(), 1);
        assert_eq!(ahead.drops.iter().filter(|d| d.1 == "ntm:pile_rod__zr").count(), 10);
        assert_eq!(ahead.drops, by_id.drops, "a host keeping items by id ends the same");
        assert_eq!(ahead.blocks, plain.blocks);
        for h in [&plain, &ahead, &by_id] {
            assert!(h.pending_items_at_sets.iter().all(|n| *n == 0), "{:?}", h.pending_items_at_sets);
            assert!(h.sweep_sets <= 3, "{}", h.sweep_sets);
            assert_eq!(owed_in_records(h), 0, "all paid: the records owe nothing");
            let kept: crate::persist::Kept = serde_json::from_slice(&h.kv.map[crate::persist::KEY]).unwrap();
            assert!(kept.machine_effects.is_empty() && kept.removals.is_empty());
        }
    }
    /// T01: the rod leaves the hand in the loader's own commit. A refusal on either side (the player's stacks, the
    /// loader's slots) moves nothing; an accepted one moves exactly one rod, the rest of the stack in hand.
    #[test]
    fn a_rod_from_the_hand_moves_in_one_transfer_or_not_at_all() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        let loader = [-1, 64, -1];
        device(&mut s, &mut h, loader, "ntm:pile_device__loader", "north");
        let rods = Stack::new("ntm:pile_rod__nu", 3);
        let use_loader = |s: &mut Server, h: &mut Host| s.native_pile_use(h, DIM, loader, [0, 0, -1], Some(&rods), 0., "p");
        let (slots, hands, saved) = (h.slots[&loader].clone(), h.hands.clone(), s.ready.world.machines[&(DIM.into(), loader)].clone());
        // No transfer host: nothing moves, the use is only consumed.
        assert_eq!(use_loader(&mut s, &mut h), Some(PileUse::Handled));
        assert_eq!((&h.slots[&loader], &h.hands), (&slots, &hands));
        hold(&mut h, &rods);
        let hands = h.hands.clone();
        for refuse in [|h: &mut Host| h.refuse_player = true, |h: &mut Host| h.refuse_commit = true] {
            refuse(&mut h);
            assert_eq!(use_loader(&mut s, &mut h), Some(PileUse::Handled));
            assert_eq!((&h.slots[&loader], &h.hands), (&slots, &hands), "a refusal publishes neither side");
            assert_eq!(s.ready.world.machines[&(DIM.into(), loader)], saved, "nor changes the model");
            (h.refuse_player, h.refuse_commit) = (false, false);
        }
        // The hand read differs from the stack the use was given (another stack in hand): nothing moves.
        hold(&mut h, &Stack::new("ntm:pile_rod__nu", 2));
        assert_eq!(use_loader(&mut s, &mut h), Some(PileUse::Handled));
        assert_eq!(h.slots[&loader], slots);
        hold(&mut h, &rods);
        assert_eq!(use_loader(&mut s, &mut h), Some(PileUse::ConsumeRod));
        assert_eq!(h.slots[&loader][0], Some(Stack::new("ntm:pile_rod__nu", 1)));
        assert_eq!(h.hands.as_ref().unwrap().held().unwrap().stack.count, 2, "one rod left the hand");
        // A full loader takes nothing more.
        assert_eq!(use_loader(&mut s, &mut h), Some(PileUse::Handled));
        assert_eq!(h.hands.as_ref().unwrap().held().unwrap().stack.count, 2);
    }
    #[test]
    fn native_loader_reads_the_signal_of_its_own_cell_only() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        let loader = [-1, 64, -1];
        device(&mut s, &mut h, loader, "ntm:pile_device__loader", "north");
        let rod = Stack::new("ntm:pile_rod__nu", 1);
        hold(&mut h, &rod);
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
        assert_eq!(owed_in_records(&h),1,"the rest of the batch is in the core's record");
        assert_eq!(stored(&h),0,"and not in the storage");
        let mut reopened=reopen(&mut h,5);reopened.native_tick(&mut h,6);
        assert_eq!(h.drops.len(),1);
        h.drop_budget=None;for t in 7..11{reopened.native_tick(&mut h,t);}
        assert_eq!(h.drops,vec![([-1,64,5],"ntm:pile_rod__zr".into()),([-1,64,5],"ntm:pile_rod__nu".into())]);
        assert!(reopened.ready.world.machine_effects.is_empty());
    }
    /// A batch lives in its home chunk's record while it owes, leaves it when paid, and the storage holds it at no
    /// point: an unsaved spawn is the chunk's, as a dropper's shot is.
    #[test]
    fn a_batch_is_in_its_chunks_record_until_paid_and_never_in_the_storage() {
        let mut h=Host::default();let mut s=Server::native(&mut h,0);
        s.ready.world.machine_effects.push_back(CommittedEffects{origins:vec![],owners:vec![(DIM.into(),AT)],tombstones:vec![],effects:std::collections::VecDeque::from([WorldEffect::Items{dim:DIM.into(),pos:AT,items:vec![("ntm:pile_rod__zr".into(),1)]}]),..Default::default()});
        h.refuse_items=true;s.native_tick(&mut h,1);
        assert_eq!(owed_in_records(&h),1,"waiting, the batch is in the record of its owner's chunk");
        s.native_world_saved(&mut h,false);
        assert_eq!(stored(&h),0,"the storage holds nothing of it");
        h.refuse_items=false;s.native_tick(&mut h,2);assert_eq!(h.drops.len(),1);
        s.flush_machine_effects(&mut h);assert_eq!(h.drops.len(),1,"no repeat in memory");
        s.native_world_saved(&mut h,true);
        assert_eq!(owed_in_records(&h),0,"paid, the record owes nothing");
        let mut reopened=reopen(&mut h,0);reopened.native_tick(&mut h,1);
        assert_eq!(h.drops.len(),1,"a reopen cannot eject again");
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
        assert_eq!(stored(&h),0,"the storage holds nothing of the core");
        let pending:Vec<CommittedEffects>=h.homes.values().flat_map(|(_,b)|b.clone()).collect();assert!(!pending.is_empty());
        let rods:usize=pending.iter().flat_map(|b|&b.tombstones).filter_map(|(_,m)|match &m.data{Data::Pile(p)=>match &**p{Pile::Core(Some(c))=>Some(c.channels.iter().map(|c|c.rods.iter().flatten().count()).sum::<usize>()),_=>None},_=>None}).sum();
        assert_eq!(rods,1,"the pending tombstone retains every rod");
        let mut again=reopen(&mut h,2);
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
        assert!(h.drops.is_empty());assert_eq!(stored(&h),0);
        assert_eq!(h.homes.values().map(|(r,_)|r.len()).sum::<usize>(),1,"the removal is in the core's record");
        let mut reopened=reopen(&mut h,5);assert_eq!(core(&mut reopened).channels[0].rods.iter().flatten().count(),1);
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
        hold(&mut h, &rod);
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
    /// A pile across a chunk border, saved formed on one side only (a death between the two chunks' records, or a
    /// chunk written before the formation): reopened, it neither holds a half formed pile nor stops the tick. The core
    /// with a brick among its cells dismantles, its rod dropped once; members whose core is a brick revert.
    #[test]
    fn a_pile_saved_formed_on_one_side_of_a_border_reopens_whole_or_as_bricks() {
        let other_side = |p: &Pos| p[0] >= 1;
        for core_side_written in [true, false] {
            let (mut s, mut h) = formed();
            drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
            core(&mut s).channels[0].load(Rod::of("ntm:pile_rod__nu", 0.).unwrap());
            s.native_tick(&mut h, 6);
            s.native_world_saved(&mut h, true);
            let core_json = h.saved[&AT].clone();
            assert!(core_json.contains(r#""rods":[{"kind":3"#), "the rod is in the core's saved state: {core_json}");
            let written = |p: &Pos| other_side(p) != core_side_written;
            for (p, _) in h.saved.clone() {
                if !written(&p) {
                    h.blocks.insert(p, pile::BRICK.into());
                }
            }
            h.drops.clear();
            let mut reopened = Server::native(&mut h, 0);
            for (p, json) in h.saved.clone() {
                if written(&p) {
                    assert!(reopened.native_adopt(DIM, p, &json));
                }
            }
            for tick in 10..14 {
                reopened.native_tick(&mut h, tick);
                replay(&mut reopened, &mut h);
            }
            assert!(reopened.ready.world.machines.is_empty(), "core side written: {core_side_written}");
            assert!(h.saved.keys().all(|p| h.blocks[p] == pile::BRICK), "every cell is a brick again");
            let rods = if core_side_written { 1 } else { 0 };
            assert_eq!(h.drops.len(), rods, "the core's rod, once, only where the core was written");
        }
    }
    /// Every rod of the world by kind: dropped, in slots, in the cores the server holds.
    fn rods_by_kind(s: &Server, h: &Host) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::<String, usize>::new();
        for (_, id) in &h.drops {
            *counts.entry(id.clone()).or_default() += 1;
        }
        for i in h.slots.values().flatten().flatten() {
            *counts.entry(i.id.clone()).or_default() += i.count as usize;
        }
        for m in s.ready.world.machines.values() {
            if let Data::Pile(p) = &m.data && let Pile::Core(Some(c)) = &**p {
                for r in c.channels.iter().flat_map(|c| c.rods.iter().flatten()) {
                    *counts.entry(r.id()).or_default() += 1;
                }
            }
        }
        counts
    }
    /// R6-2 of the sixth review: a core broken while a member's chunk is out puts its rods out at once, as Java's
    /// `breakBlock` does; nothing waits holding them, neither in a record nor in the storage.
    #[test]
    fn a_break_with_a_member_out_puts_the_rods_out_at_once() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        core(&mut s).channels[0].load(Rod::of("ntm:pile_rod__zr", 0.).unwrap());
        s.native_tick(&mut h, 5);
        h.unloaded.insert([2, 66, 4]);
        h.blocks.insert(AT, "minecraft:air".into());
        s.native_block_changed(DIM, AT, pile::CORE, "minecraft:air");
        s.native_tick(&mut h, 6);
        s.native_world_saved(&mut h, true);
        assert_eq!(h.drops, vec![([-1, 64, 5], "ntm:pile_rod__zr".to_string())], "the rod is out");
        assert_eq!((stored(&h), owed_in_records(&h)), (0, 0), "nothing waits");
        // The member comes in: it turns back to brick, nothing more goes out.
        h.unloaded.clear();
        for t in 7..10 {
            s.native_tick(&mut h, t);
            replay(&mut s, &mut h);
        }
        assert_eq!(h.blocks[&[2, 66, 4]], pile::BRICK);
        assert_eq!(h.drops.len(), 1);
    }
    /// R6-2: a transfer, then a break with a member out, then a save whose storage is written while no region is: the
    /// world opened again is the old records', whichever storage it finds, as vanilla's would be (5 Ra-226Be + 1 Zr).
    fn removal_stock(new_store: bool) {
        let (mut s, mut h) = formed();
        h.by_id = Some(BTreeSet::new());
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        for _ in 0..5 {
            core(&mut s).channels[0].load(Rod::of("ntm:pile_rod__ra226be", 0.).unwrap());
        }
        let loader = [-1, 64, -1];
        device(&mut s, &mut h, loader, "ntm:pile_device__loader", "north");
        let incoming = Stack::new("ntm:pile_rod__zr", 1);
        hold(&mut h, &incoming);
        assert_eq!(s.native_pile_use(&mut h, DIM, loader, [0, 0, -1], Some(&incoming), 0., "p"), Some(PileUse::ConsumeRod));
        s.native_tick(&mut h, 5);
        s.native_world_saved(&mut h, true);
        // The save-base: its records (block entities, chunk records) and its storage.
        let (old_kv, old_blocks, old_slots, old_saved, old_homes) = (h.kv.clone(), h.blocks.clone(), h.slots.clone(), h.saved.clone(), h.homes.clone());
        s.native_pile_use(&mut h, DIM, loader, [0, 0, -1], None, 0., "p");
        for t in 10..30 {
            s.native_tick(&mut h, t);
        }
        assert_eq!(h.drops.len(), 1, "first the full channel ejects its old rod");
        h.blocks.insert(AT, "minecraft:air".into());
        s.native_block_changed(DIM, AT, pile::CORE, "minecraft:air");
        h.unloaded.insert([2, 66, 4]);
        s.native_tick(&mut h, 31);
        s.native_world_saved(&mut h, false);
        assert_eq!(stored(&h), 0, "the storage of this save holds no removal");
        // The death: no record of this save reached the disk; the storage did, or not.
        h.blocks = old_blocks;
        h.slots = old_slots;
        h.saved = old_saved;
        h.homes = old_homes;
        h.drops.clear();
        h.unloaded.clear();
        h.changes.clear();
        h.by_id = Some(BTreeSet::new());
        if !new_store {
            h.kv = old_kv;
        }
        let mut reopened = reopen(&mut h, 0);
        for (p, json) in h.saved.clone() {
            reopened.native_adopt(DIM, p, &json);
        }
        for t in 40..45 {
            reopened.native_tick(&mut h, t);
            replay(&mut reopened, &mut h);
        }
        assert_eq!(
            rods_by_kind(&reopened, &h),
            BTreeMap::from([("ntm:pile_rod__ra226be".into(), 5), ("ntm:pile_rod__zr".into(), 1)]),
            "new storage: {new_store}"
        );
        assert!(h.drops.is_empty(), "nothing is put out for a break the records never saw");
    }
    #[test]
    fn an_old_storage_with_old_records_is_the_save_base() {
        removal_stock(false);
    }
    #[test]
    fn a_new_storage_with_old_records_is_the_save_base_too() {
        removal_stock(true);
    }
    /// F1 of the fresh review: a removal saved pending, paid after that save, then a save whose records and entities
    /// reach the disk and whose storage does not: the rod is out once, as vanilla's spilled chest is.
    #[test]
    fn a_removal_paid_after_its_save_is_not_paid_again_with_the_older_storage() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        core(&mut s).channels[0].load(Rod::of("ntm:pile_rod__nu", 100.).unwrap());
        s.native_tick(&mut h, 3);
        h.blocks.insert(AT, "minecraft:air".into());
        s.native_block_changed(DIM, AT, pile::CORE, "minecraft:air");
        // A save between the break and the next tick: the rod is out before it takes the chunks (N1 of the fresh
        // review), as a vanilla container's items are.
        s.native_world_saving(&mut h);
        s.native_world_saved(&mut h, true);
        assert_eq!((h.drops.len(), owed_in_records(&h)), (1, 0));
        let old_kv = h.kv.clone();
        for tick in 5..9 {
            s.native_tick(&mut h, tick);
            replay(&mut s, &mut h);
        }
        assert_eq!(h.drops.len(), 1);
        assert_eq!(owed_in_records(&h), 0, "paid: the record owes nothing");
        // The next save's records and entities reach the disk, its storage does not.
        h.kv = old_kv;
        let mut reopened = reopen(&mut h, 0);
        for (p, json) in h.saved.clone() {
            if h.blocks.get(&p).is_some_and(|b| b.starts_with("ntm:pile_block__")) {
                reopened.native_adopt(DIM, p, &json);
            }
        }
        for tick in 1..5 {
            reopened.native_tick(&mut h, tick);
            replay(&mut reopened, &mut h);
        }
        assert_eq!(h.drops.len(), 1, "the older storage names no removal: the rod is out once");
    }
    /// R6-1 of the sixth review: the storage set back by a death, while a chunk's record still names a batch the
    /// newer session paid; a server opened from that storage names its batches anew, never as the count alone says.
    #[test]
    fn a_server_opened_from_a_storage_that_went_back_names_its_batches_anew() {
        let mut h = Host::default();
        let first = |h: &mut Host| {
            let mut s = Server::native(h, 0);
            s.ready.world.machine_effects.push_back(CommittedEffects { owners: vec![(DIM.into(), AT)], ..Default::default() });
            s.name_batches();
            s.ready.world.machine_effects[0].serial
        };
        let (a, b) = (first(&mut h), first(&mut h));
        assert_eq!(*Server::native(&mut h, 0).ready.world.effect_serial, 0, "the same storage, the same count");
        assert_ne!(a, b, "yet the names differ: the record's paid id is never taken for a new one");
        assert!(a != 0 && b != 0);
    }


    /// Where a death cuts the first save of a world opened from an earlier version's storage.
    #[derive(Clone, Copy, Debug)]
    enum Cut {
        /// A save that does not wait, its storage written while none of its records is (`HORIZON_WRITER_HOLD`,
        /// `kv_committed`): new storage, old records.
        StorageOnly,
        /// A save whose records and entities reach the disk and whose storage does not: old storage, new records.
        RecordsOnly,
        /// A waited save, all of it.
        Whole,
    }
    /// What the host's disk holds of the world: the records (blocks, block entities, what each chunk owes and took),
    /// the entities put out, the storage.
    #[derive(Clone)]
    struct Disk {
        blocks: BTreeMap<Pos, String>,
        saved: BTreeMap<Pos, String>,
        homes: BTreeMap<[i32; 2], (Vec<crate::persist::Removal>, Vec<CommittedEffects>)>,
        marks: BTreeMap<[i32; 2], BTreeSet<crate::persist::Obligation>>,
        drops: Vec<(Pos, String)>,
        kv: MemKv,
    }
    fn disk(h: &Host) -> Disk {
        Disk { blocks: h.blocks.clone(), saved: h.saved.clone(), homes: h.homes.clone(), marks: h.marks.clone(), drops: h.drops.clone(), kv: h.kv.clone() }
    }
    /// A death: the host holds what its disk holds.
    fn die(h: &mut Host, d: &Disk) {
        (h.blocks, h.saved, h.homes, h.marks, h.drops, h.kv) =
            (d.blocks.clone(), d.saved.clone(), d.homes.clone(), d.marks.clone(), d.drops.clone(), d.kv.clone());
        h.changes.clear();
    }
    /// The world opened from the disk, its pile's block entities adopted, played until nothing is left to put out:
    /// every rod by kind, and what the storage holds.
    fn opened(h: &mut Host) -> (BTreeMap<String, usize>, usize) {
        h.refuse_items = false;
        let mut s = reopen(h, 0);
        for (p, json) in h.saved.clone() {
            if h.blocks.get(&p).is_some_and(|b| b.starts_with("ntm:pile_block__")) {
                s.native_adopt(DIM, p, &json);
            }
        }
        for t in 1..8 {
            s.native_tick(h, t);
            replay(&mut s, h);
        }
        (rods_by_kind(&s, h), stored(h))
    }
    /// The world as an earlier version left it: a storage holding a batch that puts out one Zr rod at the core.
    fn legacy_batch() -> Host {
        let mut h = Host::default();
        let mut old = WorldState::default();
        old.machine_effects.push_back(CommittedEffects {
            serial: 77,
            owners: vec![(DIM.into(), AT)],
            effects: std::collections::VecDeque::from([WorldEffect::Items { dim: DIM.into(), pos: AT, items: vec![("ntm:pile_rod__zr".into(), 1)] }]),
            ..Default::default()
        });
        h.kv.set(crate::persist::KEY, &old.kept_as_an_earlier_version(0)).unwrap();
        h
    }
    /// The world as an earlier version left it: a pile whose core, holding one Zr rod, was broken, its removal still
    /// in the storage (the format R5 wrote).
    fn legacy_removal() -> Host {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        core(&mut s).channels[0].load(Rod::of("ntm:pile_rod__zr", 0.).unwrap());
        s.native_tick(&mut h, 3);
        let machine = s.ready.world.machines[&(DIM.to_string(), AT)].clone();
        let mut old = WorldState::default();
        old.removals.push(crate::persist::Removal { key: (DIM.into(), AT), machine, engine_slots: false });
        h.blocks.insert(AT, "minecraft:air".into());
        h.saved.remove(&AT);
        h.homes.clear();
        h.kv = MemKv::default();
        h.kv.set(crate::persist::KEY, &old.kept_as_an_earlier_version(0)).unwrap();
        h
    }
    /// R7-1 and N2 of the reviews: the world opened from `h`, its earlier version's obligation played (or held, when
    /// `executed` is false: the drop is refused and the batch waits in the core's record), then a save cut by `cut`
    /// and a death. Each of the next two openings holds one rod, as the earlier version's world did, and the storage
    /// keeps the obligation only until a waited save wrote the record that took it.
    fn migration(mut h: Host, executed: bool, cut: Cut) {
        let before = disk(&h);
        let mut s = reopen(&mut h, 0);
        for (p, json) in h.saved.clone() {
            if h.blocks.get(&p).is_some_and(|b| b.starts_with("ntm:pile_block__")) {
                s.native_adopt(DIM, p, &json);
            }
        }
        h.refuse_items = !executed;
        for t in 1..4 {
            s.native_tick(&mut h, t);
            replay(&mut s, &mut h);
        }
        let zr = |h: &Host| h.drops.iter().filter(|(_, id)| id == "ntm:pile_rod__zr").count();
        assert_eq!(zr(&h), usize::from(executed), "{cut:?}: played once, or held");
        assert_eq!(stored(&h), 1, "{cut:?}: the storage keeps it until a waited save wrote its record");
        assert!(h.marks.values().any(|m| !m.is_empty()), "{cut:?}: the record says it took it");
        s.native_world_saving(&mut h);
        s.native_world_saved(&mut h, matches!(cut, Cut::Whole | Cut::RecordsOnly));
        let after = match cut {
            Cut::StorageOnly => {
                assert_eq!(stored(&h), 1, "a save that does not wait retires nothing");
                Disk { kv: h.kv.clone(), ..before.clone() }
            }
            Cut::RecordsOnly => Disk { kv: before.kv.clone(), ..disk(&h) },
            Cut::Whole => {
                assert_eq!(stored(&h), 0, "the waited save wrote the record first");
                disk(&h)
            }
        };
        for opening in 0..2 {
            die(&mut h, &after);
            let (rods, _) = opened(&mut h);
            assert_eq!(rods, BTreeMap::from([("ntm:pile_rod__zr".into(), 1)]), "{cut:?}, executed {executed}, opening {opening}");
        }
        // Opened once more and saved with waiting: the storage keeps none of it, and the world still holds one rod.
        die(&mut h, &after);
        h.refuse_items = false;
        let mut s = reopen(&mut h, 0);
        for (p, json) in h.saved.clone() {
            if h.blocks.get(&p).is_some_and(|b| b.starts_with("ntm:pile_block__")) {
                s.native_adopt(DIM, p, &json);
            }
        }
        for t in 1..8 {
            s.native_tick(&mut h, t);
            replay(&mut s, &mut h);
        }
        s.native_world_saving(&mut h);
        s.native_world_saved(&mut h, true);
        assert_eq!(stored(&h), 0, "{cut:?}: retired once its record is on the disk");
        let saved = disk(&h);
        die(&mut h, &saved);
        assert_eq!(opened(&mut h).0, BTreeMap::from([("ntm:pile_rod__zr".into(), 1)]), "{cut:?}: and after it");
    }
    #[test]
    fn a_legacy_batch_still_held_survives_a_save_that_does_not_wait() {
        migration(legacy_batch(), false, Cut::StorageOnly);
    }
    #[test]
    fn a_legacy_batch_played_survives_a_save_that_does_not_wait() {
        migration(legacy_batch(), true, Cut::StorageOnly);
    }
    #[test]
    fn a_legacy_batch_held_or_played_is_not_played_again_with_its_old_storage() {
        migration(legacy_batch(), false, Cut::RecordsOnly);
        migration(legacy_batch(), true, Cut::RecordsOnly);
    }
    #[test]
    fn a_legacy_batch_leaves_the_storage_with_a_waited_save() {
        migration(legacy_batch(), false, Cut::Whole);
        migration(legacy_batch(), true, Cut::Whole);
    }
    #[test]
    fn a_legacy_removal_survives_a_save_that_does_not_wait() {
        migration(legacy_removal(), true, Cut::StorageOnly);
        migration(legacy_removal(), false, Cut::StorageOnly);
    }
    /// N2 of the fresh review: the removal paid, its records and entities on the disk, the storage the old one: one
    /// rod, not two (the core's record says it took it).
    #[test]
    fn a_legacy_removal_paid_with_new_records_and_its_old_storage_is_paid_once() {
        migration(legacy_removal(), true, Cut::RecordsOnly);
        migration(legacy_removal(), false, Cut::RecordsOnly);
    }
    #[test]
    fn a_legacy_removal_leaves_the_storage_with_a_waited_save() {
        migration(legacy_removal(), true, Cut::Whole);
        migration(legacy_removal(), false, Cut::Whole);
    }
    /// A record read back with its marker: the storage need not keep the obligation, even past a save that does not
    /// wait; a marker the storage read with the world no longer names goes with the record's next write.
    #[test]
    fn a_marker_read_from_the_disk_retires_the_legacy_and_goes_once_the_storage_forgets_it() {
        let mut h = legacy_batch();
        let after = {
            let mut s = reopen(&mut h, 0);
            s.native_tick(&mut h, 1);
            s.native_world_saving(&mut h);
            s.native_world_saved(&mut h, true);
            disk(&h)
        };
        // The storage of the old version comes back with the records written: the marker is read from the disk.
        die(&mut h, &Disk { kv: legacy_batch().kv, ..after });
        let mut s = reopen(&mut h, 0);
        s.native_tick(&mut h, 1);
        assert_eq!(h.drops.len(), 1, "not played again");
        assert_eq!(s.ready.world.legacy.len(), 0, "the record read from the disk took it");
        s.native_world_saving(&mut h);
        s.native_world_saved(&mut h, false);
        assert_eq!(stored(&h), 0, "retired even by a save that does not wait");
        assert!(h.marks.values().any(|m| !m.is_empty()), "kept while the storage read with the world named it");
        let mut s = reopen(&mut h, 0);
        s.native_tick(&mut h, 1);
        s.native_tick(&mut h, 2);
        assert!(h.marks.values().all(BTreeSet::is_empty), "gone once the storage no longer names it");
        assert_eq!(h.drops.len(), 1);
    }
    /// R7-2: a command's break the end of a tick has not taken, then the core's chunk unloads. The unload hook runs
    /// with the chunk still in place (the engine's order, E2): the rod is out before the chunk's record and entities
    /// are written, as vanilla's chest spilled.
    #[test]
    fn a_break_not_yet_taken_when_its_chunk_unloads_is_out_before_the_chunk_goes() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        core(&mut s).channels[0].load(Rod::of("ntm:pile_rod__zr", 0.).unwrap());
        s.native_tick(&mut h, 5);
        s.native_world_saving(&mut h);
        s.native_world_saved(&mut h, true);
        h.blocks.insert(AT, "minecraft:air".into());
        h.saved.remove(&AT);
        s.native_block_changed(DIM, AT, pile::CORE, "minecraft:air");
        s.native_chunk_unloading(&mut h);
        assert_eq!(h.drops.iter().filter(|(_, id)| id == "ntm:pile_rod__zr").count(), 1, "out in the unload hook");
        assert!(s.ready.world.removals.is_empty() && owed_in_records(&h) == 0 && stored(&h) == 0);
        // The unload wrote the chunk's record and its entities; the process dies.
        let written = disk(&h);
        die(&mut h, &written);
        assert_eq!(opened(&mut h).0, BTreeMap::from([("ntm:pile_rod__zr".into(), 1)]));
    }
    /// N1 of the fresh review: a break with no tick before the save. The save's hook before it takes the chunks puts
    /// the core's rods out; a death at any cut of it finds them where vanilla's spilled chest's items are.
    #[test]
    fn a_break_with_no_tick_before_the_save_is_out_before_the_save_takes_the_chunks() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        core(&mut s).channels[0].load(Rod::of("ntm:pile_rod__zr", 0.).unwrap());
        s.native_tick(&mut h, 5);
        h.blocks.insert(AT, "minecraft:air".into());
        h.saved.remove(&AT);
        s.native_block_changed(DIM, AT, pile::CORE, "minecraft:air");
        s.native_world_saving(&mut h);
        assert_eq!(h.drops.len(), 1, "out before the chunks are taken");
        assert!(s.ready.world.removals.is_empty() && owed_in_records(&h) == 0);
        s.native_world_saved(&mut h, false);
        let written = disk(&h);
        die(&mut h, &written);
        assert_eq!(opened(&mut h).0, BTreeMap::from([("ntm:pile_rod__zr".into(), 1)]));
    }

    /// A session saved only by saves that do not wait keeps no receipt for ever: those a save took are confirmed once
    /// the region writer has put everything on the disk, and those applied after it stay for the next save.
    #[test]
    fn a_save_that_does_not_wait_confirms_its_receipts_once_the_writer_is_done() {
        let mut h = legacy_batch();
        let mut s = reopen(&mut h, 0);
        s.native_tick(&mut h, 1);
        assert_eq!(h.drops.len(), 1);
        let receipts = |s: &Server| s.ready.world.applied_machine_effects.len();
        assert_eq!(receipts(&s), 1, "the drop's receipt");
        s.native_world_saving(&mut h);
        s.native_world_saved(&mut h, false);
        assert_eq!(receipts(&s), 1, "kept while the save's writes may still be queued");
        // A receipt of after the save.
        s.ready.world.applied_machine_effects.push_back(CommittedEffects { serial: 9, ..Default::default() });
        s.native_writes_finished();
        assert_eq!(receipts(&s), 1, "the save's is confirmed, the later one stays");
        assert_eq!(s.ready.world.applied_machine_effects[0].serial, 9);
        s.native_writes_finished();
        assert_eq!(receipts(&s), 1, "nothing more to confirm before another save");
        s.native_world_saving(&mut h);
        s.native_world_saved(&mut h, true);
        assert_eq!(receipts(&s), 0, "a waited save confirms at once");
    }
}
