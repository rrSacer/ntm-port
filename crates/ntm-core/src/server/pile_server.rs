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
    fn pile_core_at(&self, dim: &str, p: Pos) -> Option<Key> {
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
        host.set_blocks(dim, &blocks)?;
        for (p, role) in cells {
            let id = format!("ntm:pile_block__{role}");
            let key = (dim.to_string(), p);
            let mut m = Machine::new(Kind::Pile, &id, Some("north".into()));
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
        if !host.pile_enter(dim) {
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
    pub fn native_pile_use(
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
                    Ok(shape) => match self.pile_write(host, dim, Core::new(shape)) {
                        Ok(()) => PileUse::WearDrill,
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
        if let Some(Data::Pile(p)) = self.ready.world.machines.get_mut(&key).map(|m| &mut m.data)
            && let Pile::Loader(l) = &mut **p
        {
            if l.level <= 0. && !l.loading {
                if let Some(h) = held
                    && Rod::of(&h.id, depletion).is_some()
                    && l.slots[0].is_none()
                {
                    let mut stack = h.clone();
                    stack.count = 1;
                    if host.set_machine_slot(dim, at, 0, Some(stack.clone())) {
                        l.slots[0] = Some(stack);
                        return Some(PileUse::ConsumeRod);
                    }
                    return Some(PileUse::Handled);
                }
                l.loading = true;
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
        // Drops and explosions only exist in the dimension the host has in
        // place, so each dimension with piles is brought in place in turn (and
        // the host's own restored at the end). One the host cannot bring in
        // place freezes, like unloaded cells, instead of losing rods.
        let dims: BTreeSet<String> = self
            .ready
            .world
            .machines
            .iter()
            .filter(|(_, m)| matches!(&m.data, Data::Pile(_)))
            .map(|(k, _)| k.0.clone())
            .collect();
        for dim in dims {
            if host.pile_enter(&dim) {
                self.pile_tick_dim(host, &dim);
            }
        }
        host.pile_restore();
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
        in_place: String,
        home: Option<String>,
        blast_dims: Vec<String>,
        drop_dims: Vec<String>,
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
        fn tell(&mut self, _: &str, _: &str) {}
        fn log(&mut self, _: &str) {}
        fn machine_slots(&mut self, _: &str, p: Pos) -> Option<Vec<Option<Stack>>> {
            self.slots.get(&p).cloned()
        }
        fn set_machine_slot(&mut self, _: &str, p: Pos, i: u32, s: Option<Stack>) -> bool {
            if let Some(slot) = self.slots.get_mut(&p).and_then(|v| v.get_mut(i as usize)) {
                *slot = s;
                true
            } else {
                false
            }
        }
        fn save_machine(&mut self, _: &str, p: Pos, json: &str) {
            self.saved.insert(p, json.into());
        }
        fn pile_drop(&mut self, _: &str, p: Pos, id: &str) {
            self.drop_dims.push(self.in_place.clone());
            self.drops.push((p, id.into()));
        }
        fn pile_explode(&mut self, _: &str, at: [f64; 3]) -> bool {
            self.blast_dims.push(self.in_place.clone());
            self.blasts.push(at);
            true
        }
        fn pile_enter(&mut self, dim: &str) -> bool {
            if self.inactive.contains(dim) {
                return false;
            }
            if self.home.is_none() {
                self.home = Some(self.in_place.clone());
            }
            self.in_place = dim.into();
            true
        }
        fn pile_restore(&mut self) {
            if let Some(home) = self.home.take() {
                self.in_place = home;
            }
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
        h.in_place = DIM.into();
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
            Some(PileUse::WearDrill)
        );
        replay(&mut s, &mut h);
        s.native_tick(&mut h, 1);
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
        assert_eq!(h.blocks[&[1, 64, 2]], pile::BRICK);
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
        assert_eq!(h.drops.len(), 1);
        assert_eq!(h.blasts.len(), 1);
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
    fn native_pile_simulates_when_another_level_ticked_last() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        let c = core(&mut s);
        c.channels[0].load(Rod::of("ntm:pile_rod__waste", 0.).unwrap());
        c.channels[0].heat = 802.;
        // The Nether is the level in place at the end of the server tick.
        h.in_place = "minecraft:the_nether".into();
        s.native_tick(&mut h, 8);
        // The Overworld pile still melted, with its level in place for the blast and drops...
        assert_eq!(h.blast_dims, vec![DIM.to_string()]);
        assert_eq!(h.drop_dims, vec![DIM.to_string()]);
        // ...and the host's own level was put back afterwards.
        assert_eq!(h.in_place, "minecraft:the_nether");
    }
    #[test]
    fn native_member_break_dismantles_at_once_with_part_of_the_pile_unloaded() {
        let (mut s, mut h) = formed();
        drill(&mut s, &mut h, [-1, 64, 0], [0, 0, -1]);
        core(&mut s).channels[0].load(Rod::of("ntm:pile_rod__nu", 100.).unwrap());
        h.unloaded.insert([2, 66, 4]);
        let old = h.blocks[&[1, 64, 2]].clone();
        h.blocks.insert([1, 64, 2], "minecraft:air".into());
        s.native_block_changed(DIM, [1, 64, 2], &old, "minecraft:air");
        s.native_tick(&mut h, 5);
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
