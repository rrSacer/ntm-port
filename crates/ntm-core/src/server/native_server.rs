//! The native bundle's entry points (docs/nativo.md): the world from the
//! start, each machine's state its block entity's (the host's
//! `commit_machine`, read back by [`Server::native_adopt`]), the cables found
//! from the blocks, the tick's phases.
use super::*;

/// How long each phase of a tick took, logged when the tick took 50 ms or more (`ServerHost::trace_phases`).
struct Phases {
    on: bool,
    start: std::time::Instant,
    last: std::time::Instant,
    done: Vec<(&'static str, u128)>,
}

impl Phases {
    fn new(on: bool) -> Self {
        let now = std::time::Instant::now();
        Phases { on, start: now, last: now, done: Vec::new() }
    }
    fn mark(&mut self, phase: &'static str) {
        if self.on {
            let now = std::time::Instant::now();
            self.done.push((phase, now.duration_since(self.last).as_micros()));
            self.last = now;
        }
    }
    fn report(self, host: &mut impl ServerHost) {
        let total = self.start.elapsed().as_micros();
        if self.on && total >= 50_000 {
            let mut slow: Vec<_> = self.done.into_iter().filter(|(_, us)| *us >= 1_000).collect();
            slow.sort_by_key(|p| std::cmp::Reverse(p.1));
            let parts: Vec<String> = slow.iter().map(|(p, us)| format!("{p} {:.1}", *us as f64 / 1000.0)).collect();
            host.log(&format!("slow tick phases {:.1} ms: {}", total as f64 / 1000.0, parts.join(", ")));
        }
    }
}

/// The fluid identifier (`ItemFluidIDMulti`; its type in `custom_data`
/// `ntm_fluid`).
pub const IDENTIFIER: &str = "ntm:fluid_identifier_multi";
/// The detonator (`ItemDetonator`).
pub const DETONATOR: &str = "ntm:detonator";

/// Blocks read a tick to find the cables next to adopted machines.
pub const CABLE_READS_PER_TICK: usize = 256;

/// Keep each recorded change's identity. Only enrich it with the current
/// block's properties when that block is still the event's new block type.
/// Coalescing A→air→A into A→A would lose the old BE's confirmed removal.
pub fn native_change_state(new:&str,current:Option<String>)->String {
    current.filter(|s|blockstate::id_of(s)==blockstate::id_of(new)).unwrap_or_else(||new.into())
}

impl Server {
    /// A server for the native bundle, in its world.
    pub fn native(host: &mut impl ServerHost, tick: u64) -> Self {
        crate::radiation::warm();
        let world = WorldState::load(host);
        // What an earlier version's storage held of removals and batches waits for their chunks' records to be read.
        let legacy_read = world.legacy.ids();
        let unread = obligations::legacy_homes(&world.legacy);
        let kept_rev = Some(world.stamps());
        let rbmk_stamp = world.rbmk_stamp();
        let sickness_stamp = world.sickness_stamp();
        let chunk_radiation_stamp = world.chunk_radiation_stamp();
        let annihilator = (*world.annihilator).clone();
        let annihilator_stamp = world.annihilator_stamp();
        #[cfg(debug_assertions)]
        let kept = world.kept(0);
        let broken=world.removals.iter().map(|r|(r.key.clone(),r.machine.block.clone())).collect();
        let mut bridge=container_bridge::Bridge::default();
        for r in &world.removals {if r.engine_slots{bridge.last.insert(r.key.clone(),Vec::new());}}
        Server {
            polaroid: launch_polaroid(),
            field: Default::default(),
            blasts: crate::blast::Runtime::default(),
            fluidnet: fluid_server::FluidNet::default(),
            tick,
            players: Vec::new(),
            radiation_shown: BTreeMap::new(),
            radiation_env: BTreeMap::new(),
            queued: Vec::new(),
            ready: Ready {
                world,
                broken,
                bridge,
                topo_dirty: true,
                kept_rev,
                kept_rbmk: Some(rbmk_stamp),
                kept_sickness: Some(sickness_stamp),
                kept_chunk_radiation: Some(chunk_radiation_stamp),
                annihilator,
                kept_annihilator: Some(annihilator_stamp),
                #[cfg(debug_assertions)]
                kept,
                ..Ready::default()
            },
            homes: BTreeMap::new(),
            unread,
            markers: BTreeMap::new(),
            marked: BTreeSet::new(),
            legacy_read,
            captured: None,
            taken: None,
            awaiting: None,
            taken_annihilator: None,
            awaiting_annihilator: None,
            nonce: obligations::draw_nonce(),
        }
    }

    /// One server tick: the blocks that changed, the slots players moved,
    /// the machines and networks, the slots and states back.
    pub fn native_tick(&mut self, host: &mut impl ServerHost, tick: u64) {
        self.tick = tick;
        let mut phases = Phases::new(host.trace_phases());
        self.ready.frozen.clear();
        self.ready.zirnox_radiation_rate.clear();
        self.ready.irradiated_now.clear();
        if let Some(players) = host.players() {
            self.players = players;
        }
        self.native_record_events(host);
        self.read_legacy(host);
        phases.mark("events");
        if self.ready.facts_pending{self.chunk_radiation_tick(host);self.keep_world(host);self.sync_obligations(host);return;}
        self.flush_machine_effects(host);
        self.settle_ghosts(host);
        phases.mark("effects");
        // Check accessibility every tick: a refused local step must be able to
        // recover, but must never simulate while its BE remains inaccessible.
        for (key,m) in &self.ready.world.machines {
            let loaded=host.get_block(&key.0,key.1).is_some_and(|b|blockstate::id_of(&b)==m.block);
            self.ready.loaded.insert(key.clone(),loaded);
        }
        for (key,m) in &self.ready.world.machines {
            self.ready.sound_marks.entry(key.clone()).or_insert_with(||{
                let mut mark=crate::sounds::mark(m);
                if crate::sounds::upgrade_slots(&m.block).is_some() && let Some(slots)=host.machine_slots(&key.0,key.1){crate::sounds::with_slots(&mut mark,&m.block,&slots);}
                mark
            });
        }
        self.native_find_cables(host);
        phases.mark("cables");
        self.native_redstone(host);
        phases.mark("redstone");
        self.local_steps(host);
        phases.mark("local_steps");
        self.flush_machine_effects(host);
        phases.mark("effects_after");
        for k in self.ready.world.machines.keys() {if self.ready.loaded.get(k)!=Some(&true){self.ready.frozen.insert(k.clone());}}
        for batch in self.ready.world.machine_effects.iter().chain(self.ready.world.prepared_retentions.iter().map(|p|&p.receipt)) {
            for g in &batch.origins{let k=g.key();if self.ready.world.machines.get(&k).is_some_and(|m|m.generation==g.generation){self.ready.loaded.insert(k.clone(),false);self.ready.frozen.insert(k);}}
        }
        // Failed/local-inaccessible machines cannot be a hidden destination of
        // foundry pouring, heat pulling, or other internal network transfers.
        let mut frozen=BTreeMap::new();
        for k in &self.ready.frozen{if let Some(m)=self.ready.world.machines.remove(k){frozen.insert(k.clone(),m);}}
        if !frozen.is_empty(){self.ready.topo_dirty=true;}
        self.heat_tick();
        phases.mark("heat");
        self.solar_tick(host);
        phases.mark("solar");
        self.simulate(host);
        // The RBMK's incrementRad of the steps accepted (TileEntityRBMKRod:153, RBMKNeutronHandler:305,417,421).
        self.apply_chunk_rad_pending();
        self.ash_tick();
        phases.mark("simulate");
        self.stirling_finish();
        for m in self.ready.world.machines.values_mut() {
            if let crate::machine::Data::Radioisotope(r) = &mut m.data { r.finish(); }
        }
        self.fluids_tick();
        phases.mark("fluids");
        self.fraction_tick();
        phases.mark("fraction");
        self.foundry_tick(host);
        phases.mark("foundry");
        self.bombs_tick(host);
        phases.mark("bombs");
        self.decon_tick(host);
        self.radiation_tick(host);
        phases.mark("radiation");
        if !frozen.is_empty(){self.ready.topo_dirty=true;self.ready.world.machines.append(&mut frozen);}
        // A player's EntityEffectHandler runs in NetHandlerPlayServer.processPlayer, during the network tick that
        // MinecraftServer.updateTimeLightAndEntities opens after every world's tile entities: the sickness and the death
        // see this tick's machines, ruins, decontaminators and inventory dose (PlayerTickEvent comes first), and come
        // before the radiation potion's dose (updatePotionEffects, later in the player's update).
        self.zirnox_world_effects(host);
        self.sickness_tick(host);
        // handleRadiationFX follows handleRadiationEffect in EntityEffectHandler.onUpdate:119-122.
        self.chunk_radiation_contaminate(host);
        self.zirnox_irradiation(host);
        // DigammaFlame.onEntityCollidedWithBlock (moveEntity) and a new body's maxHealth modifier.
        self.digamma_tick(host);
        phases.mark("zirnox");
        // ChunkRadiationManager.updateSystem: ServerTickEvent, Phase.END, after the worlds and the players.
        self.chunk_radiation_tick(host);
        phases.mark("chunk_radiation");
        self.native_save(host, true);
        phases.mark("save");
        self.keep_world(host);
        phases.mark("keep_world");
        // What each chunk owes, in its record within the tick that changed it.
        self.sync_obligations(host);
        phases.mark("obligations");
        phases.report(host);
    }

    /// Match the engine's native_block_claim: a client may send another
    /// hand/use before learning a server-only claim. A claimed useOn also
    /// suppresses its predicted-PASS Item.use fallback in the same hand.
    /// Keep the engine precedent's one tick of grace.
    pub fn native_claim_item(&mut self,player:&str,tick:u64,item:&str,target:Option<Pos>)->bool {
        self.ready.native_item_claims.retain(|_,(t,_,_)|tick.saturating_sub(*t)<=1);
        if self.ready.native_item_claims.get(player).is_some_and(|(t,i,p)|tick.saturating_sub(*t)<=1&&i==item&&(*p==target||(p.is_some()&&target.is_none()))){return false;}
        self.ready.native_item_claims.insert(player.into(),(tick,item.into(),target));true
    }

    /// Native chat/console entry point. Administrative commands use the
    /// engine's operator level; matching a different command never claims it.
    pub fn native_bomb_command(&mut self,host:&mut impl ServerHost,who:&PlayerInfo,permission:u8,line:&str)->Option<Result<String,String>> {
        let line=line.trim();
        let mut words=line.splitn(2,char::is_whitespace);
        if words.next()!=Some("ntmbomba"){return None;}
        if permission<2{return Some(Err("Este comando exige operador nível 2.".into()));}
        Some(self.bomb_command(host,Some(who),words.next().unwrap_or_default().trim()))
    }

    /// Record confirmed changes without simulating a machine. The native save
    /// callback also uses this when a player quits between two server ticks.
    pub fn native_record_events(&mut self,host:&mut impl ServerHost){
        for (e,generation) in std::mem::take(&mut self.queued){self.native_event(host,e,generation);}
        self.observe_removals(host);
        self.cancel_obsolete_inputs(host);
        self.checkpoint_facts(host);
    }

    /// The world's state outside the blocks, kept when it changed (the
    /// clock alone, every [`crate::persist::TICKS_EVERY`] ticks).
    pub(super) fn keep_world(&mut self, host: &mut impl ServerHost) {
        let r = &mut self.ready;
        r.world.ticks += 1;
        // Without the clock, which changes every tick.
        let clock = r.world.ticks.is_multiple_of(crate::persist::TICKS_EVERY);
        // The rest of the world only when it changed (or for the clock): a stepping reactor rewrites its own key alone.
        let written = if self.world_part_changed() || clock {
            self.write_world(host)
        } else {
            self.write_rbmk(host).and_then(|()| self.write_sickness(host)).and_then(|()| self.write_chunk_radiation(host)).and_then(|()| self.write_annihilator(host))
        };
        if let Err(e) = written {
            host.log(&format!("world state: {e}"));
        }
    }

    /// A block changed (`Level.setBlock`): a machine or cable put, taken
    /// away or replaced, as the client's events said before.
    pub fn native_block_changed(&mut self, dim: &str, pos: Pos, old: &str, new: &str) {
        let tracked = |s: &str| {
            let id = blockstate::id_of(s);
            registry::is_cable(id) || machine_from_state(s).is_some()
        };
        if blockstate::id_of(old) == blockstate::id_of(new) {
            return;
        }
        let key=(dim.to_string(),pos);
        if !tracked(old)&&!tracked(new)&&!self.ready.world.generations.contains_key(&key){return;}
        let old_generation=self.ready.world.generations.get(&key).copied().unwrap_or(0);
        let generation=old_generation.saturating_add(1);
        self.ready.world.generations.insert(key,generation);
        self.ready.facts_dirty=true;
        if tracked(old) {
            self.queued.push((BlockEvent::Broken(dim.into(), pos, old.into()),old_generation));
        }
        if tracked(new) {
            self.queued.push((BlockEvent::Placed(dim.into(), pos, new.into()),generation));
        }
    }

    /// A machine's block entity seen with its saved state (the world was
    /// opened again): the machine, unless it is already known. False when
    /// the state doesn't read.
    /// Whether the server holds a machine for the cell (a test and diagnostic question).
    pub fn native_has_machine(&mut self, dim: &str, pos: Pos) -> bool {
        self.ready().is_some_and(|r| r.world.machines.contains_key(&(dim.to_string(), pos)))
    }

    pub fn native_adopt(&mut self, dim: &str, pos: Pos, json: &str) -> bool {
        let Some(r) = self.ready() else { return false };
        let key = (dim.to_string(), pos);
        if r.world.machines.contains_key(&key) {
            return true;
        }
        let Ok(m) = serde_json::from_str::<Machine>(json) else { return false };
        let mut m = m.upgraded();
        let batches = r.world.machine_effects.iter().chain(&r.world.applied_machine_effects).chain(r.world.prepared_retentions.iter().map(|p|&p.receipt));
        // What the storage's journal says this machine became in a step whose block entities may have reached the disk
        // in different chunks (the newest, if several steps named it).
        let journal = r.world.survivors.get(&key).filter(|s|s.generation==m.generation).cloned();
        // Origins are read dependencies, not evidence of consumption: a coupled step names unchanged members too.
        // Only a removed model or an explicit replacement of this generation authorizes suppressing its old BE.
        let rewrites = |b: &CommittedEffects| b.effects.iter().any(|e|matches!(e,
            WorldEffect::Blocks{dim:target,blocks,expected,..} if target==dim
                && expected.iter().any(|g|g.key()==key&&g.generation==m.generation)
                && blocks.iter().any(|(at,state)|*at==pos&&blockstate::id_of(state)!=blockstate::id_of(&m.block))));
        if journal.is_none() && batches.clone().any(|b|rewrites(b)||b.tombstones.iter().any(|(k,old)|*k==key&&old.generation==m.generation)) {
            let taken = r.world.machine_effects.iter().any(rewrites);
            if !taken && !r.ghosts.iter().any(|(k,_)|*k==key) {
                r.ghosts.push((key.clone(), CellGuard{dim:dim.to_string(),pos,generation:m.generation,state:m.block.clone()}));
            }
            return true;
        }
        if let Some(j) = journal {
            if j.rev > m.rev {
                if let Ok(newer) = serde_json::from_str::<Machine>(&j.json) {
                    eprintln!("ntm: {dim} {pos:?}: the chunk is older than the journal (rev {} < {}); the journal's state is used", m.rev, j.rev);
                    m = newer.upgraded();
                    r.restored.insert(key.clone());
                }
            } else if j.rev < m.rev {
                eprintln!("ntm: {dim} {pos:?}: the chunk is newer than the journal (rev {} > {}); the chunk's state is kept", m.rev, j.rev);
            }
        }
        let named = batches.clone().any(|b|b.origins.iter().any(|g|g.key()==key&&g.generation==m.generation)
            || b.tombstones.iter().any(|(k,old)|*k==key&&old.generation==m.generation)
            || b.effects.iter().any(|e|matches!(e,WorldEffect::Blocks{expected,..} if expected.iter().any(|g|g.key()==key&&g.generation==m.generation))))
            || r.world.removals.iter().any(|x|x.key==key&&x.machine.generation==m.generation);
        let current=r.world.generations.get(&key).copied().unwrap_or(m.generation);
        if m.generation<current {
            if named {return false;}
            // The storage is ahead of this chunk: the break (or the replacement) that advanced the generation was never
            // saved with it, nothing was released for it, and the machine of the chunk is the one the world has.
            eprintln!("ntm: {dim} {pos:?}: the chunk predates a break that never reached it; generation {} is the machine's again", m.generation);
        }
        if r.world.generations.get(&key)!=Some(&m.generation){r.world.generations.insert(key.clone(),m.generation);}
        for cell in container_bridge::energy_port_cells(&m.block,pos,m.facing.as_deref()) { r.native_scan.push_back((dim.to_string(),cell)); }
        if let crate::machine::Data::Annihilator(a) = &mut m.data {
            for p in &a.pending {
                r.annihilator.replay_push(&key, m.generation, p);
            }
            let confirmed = r.world.annihilator.applied_seq(&key, m.generation);
            a.pending.retain(|p| p.seq > confirmed);
        }
        r.world.machines.insert(key.clone(), m);
        r.seen.insert(key.clone());
        r.topo_dirty = true;
        // Its cables aren't kept anywhere: found again from the blocks.
        r.native_scan.push_back(key);
        true
    }

    /// The cables next to the cells waiting, and next to those found, from
    /// the blocks: [`CABLE_READS_PER_TICK`] reads a tick at most. Each cell
    /// is looked around once a session (a cable put later comes as a
    /// block change).
    fn native_find_cables(&mut self, host: &mut impl ServerHost) {
        let Some(r) = self.ready() else { return };
        let mut reads = 0;
        while reads + 6 <= CABLE_READS_PER_TICK {
            let Some(key) = r.native_scan.pop_front() else { break };
            if !r.native_scanned.insert(key.clone()) {
                continue;
            }
            for d in crate::energy::DIRS {
                let n = (key.0.clone(), [key.1[0] + d[0], key.1[1] + d[1], key.1[2] + d[2]]);
                if r.world.cables.contains(&n) || r.world.machines.contains_key(&n) {
                    continue;
                }
                reads += 1;
                if host.get_block(&n.0, n.1).is_some_and(|b| registry::is_cable(blockstate::id_of(&b))) {
                    r.world.cables.insert(n.clone());
                    r.topo_dirty = true;
                    r.native_scan.push_back(n);
                }
            }
        }
    }

    /// A player used cell `cell` of the machine whose core is `core` (a
    /// right click, not sneaking): true when the machine took the use. The
    /// turbines' levers (`onLeverPull`), only while they don't run.
    pub fn native_use(&mut self, host: &mut impl ServerHost, dim: &str, core: Pos, cell: Pos, player: &str) -> bool {
        let Some(r) = self.ready() else { return false };
        let Some(m) = r.world.machines.get_mut(&(dim.to_string(), core)) else { return false };
        let Some(f) = m.facing.as_deref().and_then(registry::facing_dir) else { return false };
        // The cell in the placer's frame (container_bridge::world_cell's
        // inverse): x to their right, z away from them.
        let fwd = [-f[0], -f[2]];
        let right = [-fwd[1], fwd[0]];
        let d = [cell[0] - core[0], cell[2] - core[2]];
        let rel = [d[0] * right[0] + d[1] * right[1], cell[1] - core[1], d[0] * fwd[0] + d[1] * fwd[1]];
        let crate::machine::Data::Steam(s) = &mut m.data else { return false };
        let crate::steam::Steam::Turbine(t) = &mut **s else { return false };
        if !t.lever_cell(rel) {
            return false;
        }
        if t.on {
            host.tell(player, "Cannot change compressor setting while operational!");
        } else {
            t.lever();
        }
        true
    }

    /// A player right-clicked the power switch at `pos` without sneaking
    /// (`CableSwitch.onBlockActivated`): it flips and the nets are built again.
    /// The redstone switch takes no click. True when the switch took it.
    pub fn native_switch_use(&mut self, dim: &str, pos: Pos) -> bool {
        let Some(r) = self.ready() else { return false };
        let key = (dim.to_string(), pos);
        let Some(crate::machine::Data::CableSwitch(s)) = r.world.machines.get_mut(&key).map(|m| &mut m.data) else { return false };
        if !s.click() {
            return false;
        }
        r.topo_dirty = true;
        true
    }

    /// A player, not sneaking, used the wire (`ItemWiring.onItemUse`) on the block whose core is
    /// `core`; `start` is the endpoint the item remembers (its `x`, `y`, `z`). The first click marks an
    /// endpoint; the second links the two when they can be (same connection type, within the shorter
    /// range), tells the player why not when they cannot, and in both cases the item forgets.
    pub fn native_wire_use(&mut self, host: &mut impl ServerHost, dim: &str, core: Pos, start: Option<Pos>, player: &str) -> crate::wire::WireUse {
        use crate::{energy_network::WireKind, machine::Data, wire::WireUse};
        let Some(r) = self.ready() else { return WireUse::NotTaken };
        let end = |r: &Ready, at: Pos| {
            r.world.machines.get(&(dim.to_string(), at)).and_then(|m| match &m.data {
                Data::Wire(_) => WireKind::from_id(&m.block).map(|k| (k, at, crate::wire::meta_of(k, m.facing.as_deref()))),
                _ => None,
            })
        };
        let Some(second) = end(r, core) else { return WireUse::NotTaken };
        let Some(first_at) = start else {
            host.tell(player, "Wire start");
            return WireUse::Started(core);
        };
        let text = match end(r, first_at) {
            Some(first) => {
                let check = crate::wire::can_connect(first, second);
                if check == crate::energy_network::LinkCheck::Ok {
                    for (at, other) in [(first_at, core), (core, first_at)] {
                        if let Some(Data::Wire(w)) = r.world.machines.get_mut(&(dim.to_string(), at)).map(|m| &mut m.data) {
                            w.add_connection(other);
                        }
                    }
                    r.topo_dirty = true;
                }
                crate::wire::message(check)
            }
            None => "Wire error",
        };
        host.tell(player, text);
        WireUse::Finished(text)
    }

    /// A player used `item` on the block at `pos` (its `custom_data`'s
    /// `ntm_fluid`: the fluid identifier's type). The multi fluid
    /// identifier on a duct sets the duct's fluid
    /// (`FluidDuctBase.handleActivation`, a plain click). True when taken.
    pub fn native_use_item(&mut self, dim: &str, pos: Pos, item: &str, fluid: Option<&str>) -> bool {
        if item != IDENTIFIER {
            return false;
        }
        let Some(fluid) = fluid.filter(|f| crate::fluid::get(f).is_some_and(|t| t.id != 0)) else { return false };
        let Some(r) = self.ready() else { return false };
        let Some(m) = r.world.machines.get_mut(&(dim.to_string(), pos)) else { return false };
        let crate::machine::Data::Duct(d) = &mut m.data else { return false };
        if d.fluid != fluid {
            d.fluid = fluid.to_string();
        }
        true
    }

    /// The redstone the loaded batteries and bombs receive, one read per
    /// dimension: a battery runs in its signal's mode (`redHigh`), a bomb
    /// goes off on its rising edge. The diesel generator reads its block's
    /// signal and the turbofan its four connectors' (any of them stops it).
    fn native_redstone(&mut self, host: &mut impl ServerHost) {
        let Some(r) = self.ready() else { return };
        let mut by_dim: BTreeMap<String, Vec<Pos>> = BTreeMap::new();
        let mut cells_of: BTreeMap<Key, Vec<Pos>> = BTreeMap::new();
        for (k, m) in &r.world.machines {
            if r.loaded.get(k) != Some(&true) {
                continue;
            }
            if matches!(m.data, crate::machine::Data::Battery(_) | crate::machine::Data::Bomb(_)) || matches!(&m.data, crate::machine::Data::CableSwitch(s) if s.detector) {
                by_dim.entry(k.0.clone()).or_default().push(k.1);
            } else if let crate::machine::Data::Combustion(_) = m.data {
                let local = crate::combustion::Combustion::redstone_cells(&m.block);
                let facing = m.facing.as_deref().and_then(registry::facing_dir);
                let cells: Vec<Pos> = match (local.is_empty(), facing) {
                    (true, _) => Vec::new(),
                    (false, Some(f)) => local.iter().map(|&l| container_bridge::world_cell(k.1, f, l)).collect(),
                    (false, None) => vec![k.1],
                };
                by_dim.entry(k.0.clone()).or_default().extend(cells.iter().copied());
                cells_of.insert(k.clone(), cells);
            } else if let crate::machine::Data::Storage(_) = m.data {
                // `getRelevantMode`: any of its port cells getting a signal puts it in `redHigh`.
                let cells = crate::storage::Storage::redstone_cells(&m.block, k.1, m.facing.as_deref());
                by_dim.entry(k.0.clone()).or_default().extend(cells.iter().copied());
                cells_of.insert(k.clone(), cells);
            }
        }
        r.native_signal.clear();
        let mut seen: BTreeMap<Key, u8> = BTreeMap::new();
        for (dim, cells) in by_dim {
            for (p, s) in cells.iter().zip(host.redstone_many(&dim, &cells)) {
                let key = (dim.clone(), *p);
                let Some(s) = s else { continue };
                seen.insert(key.clone(), s);
                if matches!(r.world.machines.get(&key).map(|m| &m.data), Some(crate::machine::Data::Battery(_) | crate::machine::Data::Bomb(_))) {
                    r.native_signal.insert(key.clone(), s);
                }
                if let Some(crate::machine::Data::Battery(b)) = r.world.machines.get_mut(&key).map(|m| &mut m.data) {
                    b.powered = s > 0;
                }
                if let Some(crate::machine::Data::CableSwitch(w)) = r.world.machines.get_mut(&key).map(|m| &mut m.data)
                    && w.redstone(s > 0)
                {
                    r.topo_dirty = true;
                }
            }
        }
        for (key, cells) in cells_of {
            let powered = cells.iter().any(|p| seen.get(&(key.0.clone(), *p)).is_some_and(|s| *s > 0));
            match r.world.machines.get_mut(&key).map(|m| &mut m.data) {
                Some(crate::machine::Data::Combustion(c)) => c.set_powered(powered),
                Some(crate::machine::Data::Storage(s)) => s.powered = powered,
                _ => {}
            }
        }
    }

    /// The signal the native host read for the bomb or battery at `key`.
    pub(super) fn native_signal(&mut self, key: &Key) -> Option<u8> {
        self.ready().and_then(|r| r.native_signal.get(key).copied())
    }

    /// Whether the machine at (`dim`, `pos`) is known.
    pub fn native_has(&mut self, dim: &str, pos: Pos) -> bool {
        self.ready().is_some_and(|r| r.world.machines.contains_key(&(dim.to_string(), pos)))
    }

    /// The loaded machines whose state changed since it was last kept: kept
    /// with their block entities.
    pub(super) fn native_save(&mut self, host: &mut impl ServerHost, completed_tick: bool) {
        let tick=self.tick;
        let Some(r) = self.ready() else { return };
        for (key, m) in &mut r.world.machines {
            if r.loaded.get(key) != Some(&true) || m.generation==u64::MAX || r.world.generations.get(key).copied().unwrap_or(0)!=m.generation || !host.get_block(&key.0,key.1).is_some_and(|s|blockstate::id_of(&s)==m.block) {
                continue;
            }
            // Intermediate local transactions also save every machine. Publish
            // loop state and tick pulses only after all simulation stages finish.
            if completed_tick { m.audio = crate::sounds::machine(m); }
            let Ok(json) = serde_json::to_string(&*m) else { continue };
            // The energy network moves energy after the machines ran, so a pack in a socket or a REDD holds more or
            // less than the slot the step wrote: the slot gets the machine's copy in the same tick, with its state.
            // This runs also outside the tick's end (a hand drill on a pile): the copy is written only over the slots
            // the machine itself left, never over a player's change it has not read yet (`Storage::publishable`).
            let held = match &m.data {
                crate::machine::Data::Storage(s) => host.machine_slots(&key.0, key.1).and_then(|now| s.publishable(&now)),
                _ => None,
            };
            if r.native_saved.get(key) != Some(&json) || held.is_some() {
                // Marked kept only once written: an unreachable block entity
                // (its dimension not available now) is written again later.
                let Some(expected)=host.machine_slots(&key.0,key.1).or_else(|| container_bridge::layout(&m.block).is_none().then(Vec::new)) else{continue};
                let data=Self::machine_screen(&mut m.clone());
                let desired = held.clone().unwrap_or_else(|| expected.clone());
                if !host.commit_machine(&MachineCommit{cells:Vec::new(),guard:CellGuard::machine(key,m),expected_saved:r.native_saved.contains_key(key),dim:key.0.clone(),pos:key.1,desired,expected,data,json:json.clone()}) {continue;}
                if let crate::machine::Data::Storage(s) = &mut m.data { s.published(); }
                r.native_saved.insert(key.clone(), json);
            }
            let seed=crate::hash::mix(crate::hash::fnv64(format!("{:?}:{}",key,tick).as_bytes())) as i64;
            for cue in if completed_tick { crate::sounds::ambient(m,seed) } else { Vec::new() } {
                host.show(&key.0,&crate::blast::visuals::Show::Sound{id:cue.event,source:crate::sounds::category(cue.event).unwrap_or("block"),at:[0,1,2].map(|i|f64::from(key.1[i])+cue.offset[i]),volume:cue.volume,pitch:cue.pitch});
            }
            let mut now=crate::sounds::mark(m);
            if crate::sounds::upgrade_slots(&m.block).is_some() && let Some(slots)=host.machine_slots(&key.0,key.1){crate::sounds::with_slots(&mut now,&m.block,&slots);}
            if let Some(before)=r.sound_marks.insert(key.clone(),now.clone()) {
                for cue in crate::sounds::changed(&before,&now,completed_tick) {
                    host.show(&key.0,&crate::blast::visuals::Show::Sound {
                        id:cue.event, source:crate::sounds::category(cue.event).unwrap_or("block"),
                        at:[0,1,2].map(|i|f64::from(key.1[i])+cue.offset[i]),volume:cue.volume,pitch:cue.pitch,
                    });
                }
            }
        }
        r.sound_marks.retain(|k,_|r.world.machines.contains_key(k));
        r.native_saved.retain(|k, _| r.world.machines.contains_key(k));
    }
}

/// `MainRegistry.polaroidID` of this launch: `NTM_POLAROID` stands for the config's `generalOverride`, the draw is a
/// `new Random()`'s.
fn launch_polaroid() -> i32 {
    let general_override = std::env::var("NTM_POLAROID").ok().and_then(|v| v.parse().ok());
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as i64);
    let mut rand = crate::rbmk::jrandom::JavaRandom::new(seed);
    crate::digamma::polaroid(general_override, |n| rand.next_int(n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::Data;
    use crate::server::container_bridge::{Slots, Stack};

    /// The native game as the bundle's host sees it: blocks, each block
    /// entity's slots, data and kept state.
    #[derive(Default)]
    struct World {
        blocks: BTreeMap<Pos, String>,
        slots: BTreeMap<Pos, Slots>,
        data: BTreeMap<Pos, Vec<i32>>,
        saved: BTreeMap<Pos, String>,
        /// Its dimension cannot be reached: every block entity access fails.
        unreachable: bool,
        refused_slots: BTreeSet<(Pos, u32)>,
        refuse_after_staging:bool,
        staged_writes:usize,
        refuse_state:bool,
        told:Vec<String>,
        take_during_commit:Option<(Pos,usize)>,
        player:Vec<Stack>,
        emitted:Vec<Stack>,
        signal: BTreeMap<Pos, u8>,
        buttons: Vec<(String, String, Pos, u32)>,
        shown: Vec<crate::blast::visuals::Show>,
        kv: crate::kv::MemKv,
        /// What each chunk's record owes (`store_obligations`).
        homes: BTreeMap<[i32; 2], (Vec<crate::persist::Removal>, Vec<CommittedEffects>)>,
        /// What each chunk's record took of an earlier version's storage (`migrated`).
        marks: BTreeMap<[i32; 2], BTreeSet<crate::persist::Obligation>>,
    }

    /// The world opened again: the storage, then what each chunk's record owes, given as its chunk loads.
    fn reopen(w: &mut World, tick: u64) -> Server {
        let mut s = Server::native(w, tick);
        for (chunk, (removals, batches)) in w.homes.clone() {
            s.native_obligations_loaded(OW, chunk, removals, batches, w.marks.get(&chunk).cloned().unwrap_or_default());
        }
        s
    }

    impl Kv for World {
        fn get(&mut self, key: &str) -> Option<Vec<u8>> {
            self.kv.get(key)
        }
        fn set(&mut self, key: &str, value: &[u8]) -> Result<(), String> {
            self.kv.set(key, value)
        }
        fn delete(&mut self, key: &str) {
            self.kv.delete(key)
        }
    }

    impl ServerHost for World {
        fn store_obligations(&mut self, _: &str, chunk: [i32; 2], removals: &[crate::persist::Removal], batches: &[CommittedEffects], migrated: &BTreeSet<crate::persist::Obligation>) -> bool {
            if removals.is_empty() && batches.is_empty() { self.homes.remove(&chunk); } else { self.homes.insert(chunk, (removals.to_vec(), batches.to_vec())); }
            if migrated.is_empty() { self.marks.remove(&chunk); } else { self.marks.insert(chunk, migrated.clone()); }
            true
        }
        fn home_migrated(&mut self, _: &str, chunk: [i32; 2]) -> Option<BTreeSet<crate::persist::Obligation>> {
            Some(self.marks.get(&chunk).cloned().unwrap_or_default())
        }
        fn show(&mut self, _: &str, effect: &crate::blast::visuals::Show) { self.shown.push(effect.clone()); }
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
            if self.unreachable{return false;}
            if let Some((p,i))=self.take_during_commit.take() && let Some(v)=self.slots.get_mut(&p).and_then(|s|s[i].take()){self.player.push(v);}
            let mut staged=self.slots.clone();
            for c in cs {
                if staged.get(&c.pos).cloned().unwrap_or_default()!=c.expected{return false;}
                for (i,(a,b)) in c.expected.iter().zip(&c.desired).enumerate(){
                    if a==b{continue;}
                    if self.refused_slots.contains(&(c.pos,i as u32)){return false;}
                    staged.entry(c.pos).or_default()[i]=b.clone();self.staged_writes+=1;
                    if self.refuse_after_staging{return false;}
                }
            }
            if self.refuse_state{return false;}
            self.slots=staged;
            for c in cs{self.data.insert(c.pos,c.data.clone());self.saved.insert(c.pos,c.json.clone());}true
        }
        fn get_block(&mut self, _dim: &str, p: Pos) -> Option<String> {
            Some(self.blocks.get(&p).cloned().unwrap_or_else(|| "minecraft:air".into()))
        }
        fn set_blocks(&mut self, _dim: &str, blocks: &[(Pos, String)]) -> Result<(), String> {
            for (p, s) in blocks {
                self.blocks.insert(*p, s.clone());
            }
            Ok(())
        }
        fn tell(&mut self, _player: &str, text: &str) {self.told.push(text.into());}
        fn drop_items(&mut self,_:&str,_:Pos,items:&[(String,u32)])->bool{self.emitted.extend(items.iter().map(|(id,n)|Stack::new(id,*n)));true}
        fn log(&mut self, _text: &str) {}
        fn machine_slots(&mut self, _dim: &str, p: Pos) -> Option<Slots> {
            if self.unreachable {
                return None;
            }
            self.slots.get(&p).cloned()
        }
        fn machine_buttons(&mut self) -> Vec<(String, String, Pos, u32)> {
            std::mem::take(&mut self.buttons)
        }
        fn redstone_many(&mut self, _dim: &str, cells: &[Pos]) -> Vec<Option<u8>> {
            cells.iter().map(|p| Some(self.signal.get(p).copied().unwrap_or(0))).collect()
        }
    }

    const OW: &str = "minecraft:overworld";
    const AT: Pos = [0, 64, 0];
    const FIREBOX: &str = "ntm:heater_firebox[facing=north,honey_level=0]";

    /// Count resources on BOTH sides of the host boundary, including the
    /// player's/hopper's withdrawn input, containers, and committed tank/work.
    fn conservation_for(kind:&str) {
        use crate::{combustion::Combustion,crafter::Station,foundry::Tables,zirnox};

            for refusal in ["all","slot","staged"] {
                let partial=refusal!="all";
                for action in ["withdraw", "increment", "hopper"] {
                    let block=match kind {"zirnox"=>zirnox::BLOCK,"diesel"=>"ntm:machine_diesel","assembler"=>"ntm:machine_assembly_machine",_=>"ntm:heater_firebox"};
                    let machine_kind=match kind {"zirnox"=>Kind::Zirnox,"diesel"=>Kind::Combustion,"assembler"=>Kind::Crafter,_=>Kind::Firebox};
                    let mut m=Machine::new(machine_kind,block,Some("north".into()));
                    if let Data::Crafter(c)=&mut m.data {
                        let r=Station::Assembly.recipes().iter().find(|r|r.name=="ass.plateiron").unwrap();
                        c.recipe=Some(r.name.clone());c.progress=r.duration-1;c.power=100_000;
                    }
                    let (input,slot,count)=match kind {
                        "zirnox"=>("ntm:fluid_barrel_full__water",25,28),
                        "diesel"=>("ntm:fluid_barrel_full__diesel",0,3),
                        "assembler"=>("minecraft:iron_ingot",4,17),
                        _=>("minecraft:coal",0,2),
                    };
                    let mut w=World::default();w.blocks.insert(AT,block.into());
                    let mut slots=vec![None;count];slots[slot]=Some(Stack::new(input,2));w.slots.insert(AT,slots);
                    let mut s=Server::native(&mut w,0);assert!(s.native_adopt(OW,AT,&serde_json::to_string(&m).unwrap()));
                    if refusal=="staged"{w.refuse_after_staging=true;}
                    else if partial{w.refused_slots.insert((AT,slot as u32));}
                    else{w.refused_slots.extend((0..count).map(|i|(AT,i as u32)));}
                    s.native_tick(&mut w,1);
                    assert_eq!(w.slots[&AT][slot],Some(Stack::new(input,2)),"{kind} {action}: input refused");
                    assert!(w.slots[&AT].iter().enumerate().all(|(i,v)|i==slot||v.is_none()),"{kind}: partial refusal published a product");
                    assert!(w.saved.is_empty(),"no speculative JSON can escape");
                    if refusal=="staged"{assert!(w.staged_writes>0,"the failing host actually staged a slot write");}
                    let mut outside=Vec::new();let supplied=if action=="withdraw"{2}else{3};
                    match action {
                        "withdraw"=>outside.push(w.slots.get_mut(&AT).unwrap()[slot].take().unwrap()),
                        "increment"=>w.slots.get_mut(&AT).unwrap()[slot].as_mut().unwrap().count+=1,
                        _=>{outside.push(w.slots.get_mut(&AT).unwrap()[slot].take().unwrap());w.slots.get_mut(&AT).unwrap()[slot]=Some(Stack::new(input,1));}
                    }
                    w.unreachable=false;w.refuse_after_staging=false;w.refused_slots.clear();s.native_tick(&mut w,2);
                    let kept:&Machine=&s.ready.world.machines[&(OW.into(),AT)];
                    let all:Vec<_>=outside.iter().chain(w.slots[&AT].iter().flatten()).collect();
                    let n=|id:&str|all.iter().filter(|x|x.id==id).map(|s|s.count).sum::<u32>();
                    match &kept.data {
                        Data::Zirnox(z)=>{
                            assert_eq!(n(input)*16000+z.water.mb,supplied*16000,"water conservation {partial} {action}");
                            assert_eq!(n(input)+n("ntm:fluid_barrel_empty"),supplied,"container conservation");
                        }
                        Data::Combustion(c)=>{let Combustion::Diesel(d)=&**c else{panic!()};
                            assert_eq!(n(input)*16000+d.tank.mb,supplied*16000,"diesel conservation {partial} {action}");
                            assert_eq!(n(input)+n("ntm:fluid_barrel_empty"),supplied);
                        }
                        Data::Crafter(c)=>{
                            assert_eq!(n(input)+n("ntm:plate_iron"),supplied,"assembler conservation {partial} {action}");
                            if action=="withdraw"{assert!(c.out.is_empty());}
                        }
                        Data::Firebox(f)=>{
                            let ticks=Tables::ntm().firebox_fuels[input].ticks;
                            assert_eq!(n(input)*ticks+f.burn,supplied*ticks,"fuel/work conservation {partial} {action}");
                            if action=="withdraw" {assert_eq!(f.heat,0);}
                        }
                        _=>panic!(),
                    }
                    assert_eq!(w.saved[&AT],serde_json::to_string(kept).unwrap(),"save contains confirmed state only");
                }
            }
    }

    #[test] fn conservation_zirnox(){conservation_for("zirnox");}
    #[test] fn conservation_diesel(){conservation_for("diesel");}
    #[test] fn conservation_assembler(){conservation_for("assembler");}
    #[test] fn conservation_firebox(){conservation_for("firebox");}

    #[test]
    fn reviewer_bucket_and_three_barrel_reproductions_conserve_exactly() {
        for (input,amount,withdraw,expected_water) in [("minecraft:water_bucket",1,true,0),("ntm:fluid_barrel_full__water",3,false,16000)] {
            let mut w=World::default();w.blocks.insert(AT,crate::zirnox::BLOCK.into());w.slots.insert(AT,vec![None;28]);
            let mut s=Server::native(&mut w,0);s.native_block_changed(OW,AT,"minecraft:air",crate::zirnox::BLOCK);s.native_tick(&mut w,1);
            w.slots.get_mut(&AT).unwrap()[25]=Some(Stack::new(input,if withdraw{amount}else{amount-1}));
            w.refused_slots.insert((AT,25));s.native_tick(&mut w,2);
            assert!(w.slots[&AT][27].is_none());
            let player=if withdraw{w.slots.get_mut(&AT).unwrap()[25].take()}else{w.slots.get_mut(&AT).unwrap()[25].as_mut().unwrap().count+=1;None};
            w.refused_slots.clear();s.native_tick(&mut w,3);
            let Data::Zirnox(z)=&s.ready.world.machines[&(OW.into(),AT)].data else{panic!()};
            assert_eq!(z.water.mb,expected_water);
            if withdraw {assert_eq!(player.unwrap().id,input);assert!(w.slots[&AT][27].is_none());}
            else {assert_eq!(w.slots[&AT][25].as_ref().unwrap().count,2);assert_eq!(w.slots[&AT][27].as_ref().unwrap().count,1);}
        }
    }
    #[test]
    fn expected_inventory_conflict_discards_all_speculative_products() {
        let mut w=World::default();w.blocks.insert(AT,crate::zirnox::BLOCK.into());
        let mut slots=vec![None;28];slots[25]=Some(Stack::new("minecraft:water_bucket",1));w.slots.insert(AT,slots);
        let mut s=Server::native(&mut w,0);s.native_block_changed(OW,AT,"minecraft:air",crate::zirnox::BLOCK);
        w.take_during_commit=Some((AT,25));s.native_tick(&mut w,1);
        assert_eq!(w.player.len(),1);assert!(w.slots[&AT][27].is_none());assert!(w.saved.is_empty());
        let Data::Zirnox(z)=&s.ready.world.machines[&(OW.into(),AT)].data else{panic!()};assert_eq!(z.water.mb,0);
        s.native_tick(&mut w,2);assert!(w.slots[&AT][27].is_none());
        let Data::Zirnox(z)=&s.ready.world.machines[&(OW.into(),AT)].data else{panic!()};assert_eq!(z.water.mb,0);
    }

    #[test]
    fn hosts_without_atomic_commit_freeze_instead_of_simulating() {
        struct Unsupported(World);
        impl Kv for Unsupported {
            fn get(&mut self,k:&str)->Option<Vec<u8>>{self.0.get(k)}
            fn set(&mut self,k:&str,v:&[u8])->Result<(),String>{self.0.set(k,v)}
            fn delete(&mut self,k:&str){self.0.delete(k)}
        }
        impl ServerHost for Unsupported {
            fn get_block(&mut self,d:&str,p:Pos)->Option<String>{self.0.get_block(d,p)}
            fn machine_slots(&mut self,d:&str,p:Pos)->Option<Slots>{self.0.machine_slots(d,p)}
            fn set_blocks(&mut self,_:&str,_:&[(Pos,String)])->Result<(),String>{panic!("no speculative effect")}
            fn tell(&mut self,_:&str,_:&str){}
            fn log(&mut self,_:&str){}
        }
        let mut w=Unsupported(World::default());w.0.blocks.insert(AT,FIREBOX.into());
        w.0.slots.insert(AT,vec![Some(Stack::new("minecraft:coal",1)),None]);
        let mut s=Server::native(&mut w,0);s.native_block_changed(OW,AT,"minecraft:air",FIREBOX);
        for t in 1..40{s.native_tick(&mut w,t);}
        let Data::Firebox(f)=&s.ready.world.machines[&(OW.into(),AT)].data else{panic!()};
        assert_eq!((f.burn,f.heat),(0,0));assert_eq!(w.0.slots[&AT][0].as_ref().unwrap().count,1);
        assert!(w.0.saved.is_empty());
    }

    #[test]
    fn a_confirmed_break_never_clears_a_replacement_be_of_the_same_machine() {
        let mut w=World::default();w.blocks.insert(AT,crate::zirnox::BLOCK.into());w.slots.insert(AT,vec![None;28]);
        let mut s=Server::native(&mut w,0);s.native_block_changed(OW,AT,"minecraft:air",crate::zirnox::BLOCK);s.native_tick(&mut w,1);
        // The old BE is gone; this inventory belongs to a newly placed BE.
        w.slots.get_mut(&AT).unwrap()[25]=Some(Stack::new("minecraft:water_bucket",1));
        s.native_block_changed(OW,AT,crate::zirnox::BLOCK,&native_change_state("minecraft:air",Some(crate::zirnox::BLOCK.into())));
        s.native_block_changed(OW,AT,"minecraft:air",&native_change_state(crate::zirnox::BLOCK,Some(crate::zirnox::BLOCK.into())));
        s.native_tick(&mut w,2);
        assert_eq!(w.slots[&AT][25].as_ref().unwrap().id,"minecraft:water_bucket");
        s.native_tick(&mut w,3);
        let Data::Zirnox(z)=&s.ready.world.machines[&(OW.into(),AT)].data else{panic!()};
        assert_eq!(z.water.mb,1000);assert_eq!(w.slots[&AT][27].as_ref().unwrap().id,"minecraft:bucket");
    }

    fn armed_bomb(block:&str)->(World,Server,PlayerInfo) {
        let mut w=World::default();w.blocks.insert(AT,block.into());
        let parts=crate::bomb::bomb_type(block).unwrap().slots.iter().map(|s|Some(Stack::new(s.item,1))).collect();w.slots.insert(AT,parts);
        let m=Machine::new(Kind::Bomb,block,None);let mut s=Server::native(&mut w,0);assert!(s.native_adopt(OW,AT,&serde_json::to_string(&m).unwrap()));
        let p=PlayerInfo{id:"p".into(),name:"p".into(),dim:OW.into(),pos:[0.,64.,4.],creative:true};
        s.bomb_command(&mut w,Some(&p),"config mundo raiomax 4").unwrap();
        assert!(s.detonator(&mut w,&p,Some(AT),true).contains("ligado"));
        assert!(s.detonator(&mut w,&p,None,false).contains("solicitad"));
        (w,s,p)
    }
    #[test]
    fn a_manual_nuke_spends_no_components_or_effects_before_atomic_confirmation() {
        for partial in [false,true] {
            let (mut w,mut s,_)=armed_bomb("ntm:nuke_boy");let original=w.slots[&AT].clone();
            w.refuse_state= !partial;w.refuse_after_staging=partial;s.native_tick(&mut w,1);
            assert_eq!(w.slots[&AT],original);assert_eq!(s.ready.world.bombs.next,0);assert_eq!(s.ready.detonations.len(),1);
            assert_eq!(w.blocks[&AT],"ntm:nuke_boy");assert!(s.ready.world.bombs.queue.is_empty());
            w.refuse_state=false;w.refuse_after_staging=false;s.native_tick(&mut w,2);
            assert!(w.slots[&AT].iter().all(Option::is_none));assert_eq!(s.ready.world.bombs.next,1);
            assert!(!s.ready.world.machines.contains_key(&(OW.into(),AT)));assert!(s.ready.detonations.is_empty());
            for t in 3..6{s.native_tick(&mut w,t);}assert_eq!(s.ready.world.bombs.next,1,"the retained intent confirms exactly once");
        }
    }
    #[test]
    fn changing_a_requested_bombs_parts_cancels_even_after_a_commit_conflict() {
        for during in [false,true] {
            let (mut w,mut s,_)=armed_bomb("ntm:nuke_boy");
            if during{w.take_during_commit=Some((AT,0));}else{w.refuse_state=true;}
            s.native_tick(&mut w,1);
            let withdrawn=if during{w.player.pop().unwrap()}else{w.slots.get_mut(&AT).unwrap()[0].take().unwrap()};
            assert_eq!(w.slots[&AT].iter().flatten().map(|s|s.count).sum::<u32>()+withdrawn.count,5);
            w.refuse_state=false;s.native_tick(&mut w,2);
            assert_eq!(s.ready.world.bombs.next,0);assert!(s.ready.detonations.is_empty());
            assert!(w.told.iter().any(|t|t.contains("componentes mudaram")));
            w.slots.get_mut(&AT).unwrap()[0]=Some(withdrawn);s.native_tick(&mut w,3);
            assert_eq!(s.ready.world.bombs.next,0,"replacing a component must not resurrect the cancelled request");
            assert_eq!(w.slots[&AT].iter().flatten().map(|s|s.count).sum::<u32>(),5);
        }
    }
    #[test]
    fn a_manual_fuse_is_lit_once_only_after_its_state_commit() {
        let (mut w,mut s,_)=armed_bomb("ntm:dynamite");w.refuse_state=true;s.native_tick(&mut w,1);
        let fuse=|s:&Server|match &s.ready.world.machines[&(OW.into(),AT)].data{Data::Bomb(b)=>b.fuse,_=>panic!()};
        assert_eq!(fuse(&s),None);assert_eq!(s.ready.detonations.len(),1);
        w.refuse_state=false;s.native_tick(&mut w,2);let lit=fuse(&s).unwrap();assert_eq!(lit,crate::bomb::FUSE_TICKS-1);
        for t in 3..8{s.native_tick(&mut w,t);}assert_eq!(fuse(&s),Some(lit-5));
        assert_eq!(w.told.iter().filter(|s|s.as_str()=="Pavio aceso.").count(),1);
    }
    #[test]
    fn a_repair_gear_is_requested_from_the_hand_only_after_the_stirling_commit() {
        use crate::solar_stirling::SolarStirling;
        let block="ntm:machine_stirling";let mut w=World::default();w.blocks.insert(AT,block.into());w.slots.insert(AT,vec![]);
        let mut m=Machine::new(Kind::SolarStirling,block,Some("north".into()));let Data::SolarStirling(s)=&mut m.data else{panic!()};let SolarStirling::Stirling(s)=&mut **s else{panic!()};s.has_cog=false;
        let mut server=Server::native(&mut w,0);assert!(server.native_adopt(OW,AT,&serde_json::to_string(&m).unwrap()));
        w.refuse_state=true;assert!(!server.native_stirling_repair(&mut w,OW,AT,"ntm:gear_large__iron"));assert_eq!(server.native_stirling_gear(OW,AT),Some("ntm:gear_large__iron"));assert!(w.saved.is_empty());
        w.refuse_state=false;assert!(server.native_stirling_repair(&mut w,OW,AT,"ntm:gear_large__iron"));assert_eq!(server.native_stirling_gear(OW,AT),None);assert!(w.saved.contains_key(&AT));
    }
    #[test]
    fn native_item_claims_deduplicate_only_the_same_player_item_and_intention() {
        let mut w=World::default();let mut s=Server::native(&mut w,0);
        assert!(s.native_claim_item("p",10,DETONATOR,None));
        assert!(!s.native_claim_item("p",10,DETONATOR,None));
        assert!(!s.native_claim_item("p",11,DETONATOR,None));
        assert!(s.native_claim_item("q",11,DETONATOR,None));
        assert!(s.native_claim_item("p",11,DETONATOR,Some(AT)));
        assert!(s.native_claim_item("p",11,"other",Some(AT)));
        assert!(s.native_claim_item("p",13,DETONATOR,None));
    }

    #[test]
    fn a_native_link_ignores_its_client_item_use_fallback_without_arming() {
        let (mut w,mut s,p)=armed_bomb("ntm:dynamite");s.set_players(vec![p.clone()]);s.ready.detonations.clear();
        assert!(s.native_detonator(&mut w,&p.id,Some(AT)).unwrap().contains("ligado"));
        assert_eq!(s.native_detonator(&mut w,&p.id,None),Some(String::new()));
        s.native_tick(&mut w,1);
        assert_eq!(s.native_detonator(&mut w,&p.id,None),Some(String::new()));
        let Data::Bomb(b)=&s.ready.world.machines[&(OW.into(),AT)].data else{panic!()};assert_eq!(b.fuse,None);assert!(s.ready.detonations.is_empty());
        s.native_tick(&mut w,2);
        assert_eq!(s.native_detonator(&mut w,&p.id,None).as_deref(),Some("Acendimento solicitado."));
        s.native_tick(&mut w,3);
        let Data::Bomb(b)=&s.ready.world.machines[&(OW.into(),AT)].data else{panic!()};assert_eq!(b.fuse,Some(crate::bomb::FUSE_TICKS-1));
        assert_eq!(w.told.iter().filter(|t|t.as_str()=="Pavio aceso.").count(),1);
    }

    #[test]
    fn a_duplicated_native_click_and_a_later_request_never_relight_a_fuse() {
        let (mut w,mut s,p)=armed_bomb("ntm:dynamite");s.set_players(vec![p.clone()]);s.ready.detonations.clear();
        let first=s.native_detonator(&mut w,&p.id,None).unwrap();assert_eq!(first,"Acendimento solicitado.");
        assert_eq!(s.native_detonator(&mut w,&p.id,None),Some(String::new()));
        assert_eq!(s.ready.detonations.len(),1);s.native_tick(&mut w,1);
        let fuse=|s:&Server|match &s.ready.world.machines[&(OW.into(),AT)].data{Data::Bomb(b)=>b.fuse,_=>panic!()};
        let initial=fuse(&s).unwrap();s.native_tick(&mut w,2);
        assert_eq!(s.native_detonator(&mut w,&p.id,None).unwrap(),"Pavio já aceso.");
        assert_eq!(fuse(&s),Some(initial-1));assert!(s.ready.detonations.is_empty());
        assert_eq!(w.told.iter().filter(|t|t.as_str()=="Pavio aceso.").count(),1);
        assert_eq!(s.ready.world.bombs.next,0);
    }

    #[test]
    fn native_bomb_commands_require_operator_permission_and_keep_configuration() {
        let mut w=World::default();let mut s=Server::native(&mut w,0);
        let p=PlayerInfo{id:"p".into(),name:"p".into(),dim:OW.into(),pos:[0.,64.,0.],creative:true};
        assert!(s.native_bomb_command(&mut w,&p,4,"ntmbomba_extra status").is_none());
        assert!(s.native_bomb_command(&mut w,&p,1,"ntmbomba config mundo raiomax 12").unwrap().is_err());
        assert!(s.ready.world.bombs.cfg.is_none());
        let reply=s.native_bomb_command(&mut w,&p,2,"ntmbomba config mundo raiomax 12").unwrap().unwrap();
        assert!(reply.contains("raiomax 12"));assert_eq!(s.bomb_config(&mut w).max_radius,12);
        s.native_tick(&mut w,1);s.native_world_saved(&mut w,true);
        let mut reopened=Server::native(&mut w,2);assert_eq!(reopened.bomb_config(&mut w).max_radius,12);
        assert!(reopened.native_bomb_command(&mut w,&p,4,"ntmbomba").unwrap().unwrap().starts_with("Bombas:"));
    }

    #[test]
    fn a_clean_save_between_break_and_tick_preserves_hidden_contents_without_repeating_engine_slots() {
        let block="ntm:machine_crucible";
        let mut w=World::default();w.blocks.insert(AT,block.into());
        let mut slots=vec![None;10];slots[2]=Some(Stack::new("minecraft:iron_ingot",1));w.slots.insert(AT,slots);
        let mut m=Machine::new(Kind::Crucible,block,Some("north".into()));
        let Data::Crucible(c)=&mut m.data else{panic!()};c.waste_stack=vec![("iron".into(),144)];
        let mut s=Server::native(&mut w,0);assert!(s.native_adopt(OW,AT,&serde_json::to_string(&m).unwrap()));
        s.native_tick(&mut w,1);
        // The engine spills physical slots, then reports Broken. A save before
        // the next machine step: only the hidden molten metal is NTM's debt.
        let item=w.slots.remove(&AT).unwrap()[2].take().unwrap();w.player.push(item);
        w.saved.remove(&AT);w.blocks.insert(AT,"minecraft:air".into());
        s.native_block_changed(OW,AT,block,"minecraft:air");
        // N1 of the fresh review: the save takes the chunks only once the hidden metal is out, as a vanilla
        // container's items are out the instant its block goes.
        s.native_world_saving(&mut w);
        assert_eq!(w.emitted,vec![Stack::new("ntm:scraps__iron",2)],"put out before the save takes the chunks");
        assert!(s.ready.world.removals.is_empty()&&w.homes.is_empty(),"nothing left owing");
        s.native_world_saved(&mut w,true);
        let kept:crate::persist::Kept=serde_json::from_slice(&w.kv.map[crate::persist::KEY]).unwrap();assert!(kept.removals.is_empty());
        let mut reopened=reopen(&mut w,2);reopened.native_tick(&mut w,3);
        assert_eq!(w.player,vec![Stack::new("minecraft:iron_ingot",1)]);
        assert_eq!(w.emitted,vec![Stack::new("ntm:scraps__iron",2)]);
        assert!(!reopened.native_has(OW,AT));reopened.native_world_saved(&mut w,true);
        let mut again=reopen(&mut w,4);again.native_tick(&mut w,5);
        assert_eq!(w.emitted,vec![Stack::new("ntm:scraps__iron",2)]);
        assert!(again.ready.world.removals.is_empty());assert!(again.ready.world.machine_effects.is_empty());
    }

    /// A machine whose dimension cannot be reached stays frozen in memory but
    /// is not marked kept; its state (and a new machine's first state) is written
    /// once the block entity can be reached again.
    #[test]
    fn an_unreachable_block_entity_is_written_again_later() {
        let mut w = World::default();
        let mut s = Server::native(&mut w, 0);
        w.blocks.insert(AT, FIREBOX.into());
        w.slots.insert(AT, vec![Some(Stack::new("minecraft:coal", 2)), None]);
        w.unreachable = true;
        s.native_block_changed(OW, AT, "minecraft:air", FIREBOX);
        for t in 1..=40 {
            s.native_tick(&mut w, t);
        }
        assert!(w.saved.is_empty(), "nothing could be written");
        assert!(!s.ready().unwrap().native_saved.contains_key(&(OW.to_string(), AT)), "and nothing is marked kept");
        w.unreachable = false;
        s.native_tick(&mut w, 41);
        assert!(w.saved[&AT].contains("\"heat\""), "{:?}", w.saved.get(&AT));
        assert!(s.ready().unwrap().native_saved.contains_key(&(OW.to_string(), AT)));
    }

    /// A firebox put in the world burns the coal in its block entity's slot,
    /// shows it, and keeps its state there; a server that opens the world
    /// again takes the machine back from it.
    #[test]
    fn intermediate_machine_publications_do_not_replay_tick_audio() {
        let block = "ntm:machine_shredder";
        let mut w = World::default();
        w.blocks.insert(AT, block.into());
        let mut slots = vec![None; crate::shredder::SLOTS];
        slots[0] = Some(Stack::new("minecraft:iron_ingot", 2));
        for slot in crate::shredder::BLADES { slots[slot] = Some(Stack::new("ntm:blades_desh", 1)); }
        w.slots.insert(AT, slots);
        let mut m = Machine::new(Kind::Shredder, block, None);
        let Data::Shredder(shredder) = &mut m.data else { panic!() };
        shredder.power = crate::shredder::MAX_POWER;
        let mut server = Server::native(&mut w, 0);
        assert!(server.native_adopt(OW, AT, &serde_json::to_string(&m).unwrap()));
        server.native_tick(&mut w, 1);
        let hits = |w: &World| w.shown.iter().filter(|effect| matches!(effect,
            crate::blast::visuals::Show::Sound { id: "minecraft:entity.minecart.riding", source: "neutral", .. }
        )).count();
        assert_eq!(hits(&w), 1);
        for _ in 0..4 { server.native_save(&mut w, false); }
        assert_eq!(hits(&w), 1);
        server.native_tick(&mut w, 2);
        assert_eq!(hits(&w), 1);
    }

    #[test]
    fn a_native_machine_runs_on_its_block_entity_and_comes_back() {
        let mut w = World::default();
        let mut s = Server::native(&mut w, 0);
        w.blocks.insert(AT, FIREBOX.into());
        w.slots.insert(AT, vec![Some(Stack::new("minecraft:coal", 2)), None]);
        s.native_block_changed(OW, AT, "minecraft:air", FIREBOX);
        for t in 1..=40 {
            s.native_tick(&mut w, t);
        }
        assert_eq!(w.slots[&AT][0], Some(Stack::new("minecraft:coal", 1)), "um carvão queimando");
        assert!(w.data[&AT][0] > 0, "o calor na tela: {:?}", w.data[&AT]);
        assert!(w.blocks[&AT].contains("honey_level=") && !w.blocks[&AT].contains("honey_level=0"), "acesa: {}", w.blocks[&AT]);
        let saved = w.saved[&AT].clone();
        assert!(saved.contains("\"heat\""), "{saved}");

        let mut again = Server::native(&mut w, 100);
        assert!(!again.native_has(OW, AT));
        assert!(again.native_adopt(OW, AT, &saved));
        again.native_tick(&mut w, 101);
        assert!(w.data[&AT][0] > 0, "o calor voltou com o estado");

        // A battery adopted beside the firebox's cable line: the cables are
        // found from the blocks, and its power goes on to a furnace.
        let (bat, fur) = ([10, 64, 0], [14, 64, 0]);
        for x in 11..14 {
            w.blocks.insert([x, 64, 0], "ntm:red_cable".into());
        }
        w.blocks.insert(bat, "ntm:machine_battery[facing=north,honey_level=0]".into());
        w.blocks.insert(fur, "ntm:machine_electric_furnace_off[facing=north,honey_level=0]".into());
        let mut b = Machine::new(crate::machine::Kind::Battery, "ntm:machine_battery", Some("north".into()));
        if let Data::Battery(x) = &mut b.data {
            x.power = x.max;
            x.mode = crate::machine::BatteryMode::Output;
        }
        let f = Machine::new(crate::machine::Kind::ElectricFurnace, "ntm:machine_electric_furnace_off", Some("north".into()));
        w.slots.insert(bat, vec![None; 1]);
        w.slots.insert(fur, vec![None; 3]);
        assert!(again.native_adopt(OW, bat, &serde_json::to_string(&b).unwrap()));
        assert!(again.native_adopt(OW, fur, &serde_json::to_string(&f).unwrap()));
        for t in 110..130 {
            again.native_tick(&mut w, t);
        }
        // A redstone signal on the battery: its redHigh mode (output, by
        // default) is in force; without, redLow (here output too).
        w.signal.insert(bat, 15);
        again.native_tick(&mut w, 130);
        let Data::Battery(b) = &again.ready().unwrap().world.machines[&(OW.to_string(), bat)].data else { panic!() };
        assert!(b.powered && b.effective() == b.mode_high);
        // A menu button reaches it with no player of the old session (the
        // game checked the menu's reach): redLow output → none.
        w.buttons.push(("p".into(), OW.into(), bat, 0));
        again.native_tick(&mut w, 131);
        let Data::Battery(b) = &again.ready().unwrap().world.machines[&(OW.to_string(), bat)].data else { panic!() };
        assert_eq!(b.mode, crate::machine::BatteryMode::None, "o botão 0 avançou o redLow");
        let r = again.ready().unwrap();
        assert_eq!(r.world.cables.len(), 3, "os três cabos achados nos blocos");
        let Data::ElectricFurnace(f) = &r.world.machines[&(OW.to_string(), fur)].data else { panic!() };
        assert!(f.power > 0, "a energia chegou pelos cabos");

        // A Leviathan facing east (the placer east of it, looking west): its
        // lever, two to the placer's right (-z) and one or two towards them
        // (+x), is at (core.x + 1 or 2, core.z - 2).
        let lev = [30, 64, 0];
        let mut c = Machine::new(crate::machine::Kind::Steam, "ntm:machine_chungus", Some("east".into()));
        if let Data::Steam(s) = &mut c.data
            && let crate::steam::Steam::Turbine(t) = &mut **s
        {
            t.on = false;
        }
        assert!(again.native_adopt(OW, lev, &serde_json::to_string(&c).unwrap()));
        assert!(!again.native_use(&mut w, OW, lev, [31, 64, 0], "p"), "uma célula sem alavanca");
        assert!(again.native_use(&mut w, OW, lev, [31, 65, -2], "p"));
        let Data::Steam(s) = &again.ready().unwrap().world.machines[&(OW.to_string(), lev)].data else { panic!() };
        let crate::steam::Steam::Turbine(t) = &**s else { panic!() };
        assert_eq!(t.input.fluid, "hotsteam", "a alavanca trocou o vapor");

        // A duct typed by the fluid identifier: its fluid set; other items
        // and blocks aren't taken.
        let duct = [40, 64, 0];
        let m = Machine::new(crate::machine::Kind::Duct, "ntm:fluid_duct_neo", None);
        assert!(again.native_adopt(OW, duct, &serde_json::to_string(&m).unwrap()));
        assert!(!again.native_use_item(OW, duct, "minecraft:stick", Some("water")));
        assert!(!again.native_use_item(OW, duct, IDENTIFIER, Some("nada")));
        assert!(again.native_use_item(OW, duct, IDENTIFIER, Some("water")));
        let Data::Duct(d) = &again.ready().unwrap().world.machines[&(OW.to_string(), duct)].data else { panic!() };
        assert_eq!(d.fluid, "water");
        assert!(!again.native_use_item(OW, lev, IDENTIFIER, Some("water")), "a turbina não é duto");

        // Broken: gone from the server.
        w.blocks.insert(AT, "minecraft:air".into());
        again.native_block_changed(OW, AT, FIREBOX, "minecraft:air");
        again.native_tick(&mut w, 102);
        assert!(!again.native_has(OW, AT));
    }
}
