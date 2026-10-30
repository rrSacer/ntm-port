//! The native bundle's entry points (docs/nativo.md): the world from the
//! start, each machine's state its block entity's (the host's
//! `save_machine`, read back by [`Server::native_adopt`]), the cables found
//! from the blocks, the tick's phases.
use super::*;

/// The fluid identifier (`ItemFluidIDMulti`; its type in `custom_data`
/// `ntm_fluid`).
pub const IDENTIFIER: &str = "ntm:fluid_identifier_multi";
/// The detonator (`ItemDetonator`).
pub const DETONATOR: &str = "ntm:detonator";

/// Blocks read a tick to find the cables next to adopted machines.
pub const CABLE_READS_PER_TICK: usize = 256;

impl Server {
    /// A server for the native bundle, in its world.
    pub fn native(host: &mut impl ServerHost, tick: u64) -> Self {
        crate::radiation::warm();
        let world = WorldState::load(host);
        let kept = world.kept(0);
        Server {
            blasts: crate::blast::Runtime::default(),
            fluidnet: fluid_server::FluidNet::default(),
            tick,
            players: Vec::new(),
            radiation_shown: BTreeMap::new(),
            queued: Vec::new(),
            ready: Ready {
                world,
                topo_dirty: true,
                kept,
                ..Ready::default()
            },
        }
    }

    /// One server tick: the blocks that changed, the slots players moved,
    /// the machines and networks, the slots and states back.
    pub fn native_tick(&mut self, host: &mut impl ServerHost, tick: u64) {
        self.tick = tick;
        if let Some(players) = host.players() {
            self.players = players;
        }
        for e in std::mem::take(&mut self.queued) {
            self.block_event(host, e);
        }
        self.native_find_cables(host);
        self.native_redstone(host);
        self.slots_in(host);
        self.heat_tick();
        self.solar_tick(host);
        self.simulate(host);
        self.stirling_finish();
        self.fluids_tick();
        self.fraction_tick();
        self.foundry_tick(host);
        self.excavator_tick(host);
        self.drill_tick(host);
        self.bombs_tick(host);
        self.radiation_tick(host);
        self.slots_out(host);
        self.native_save(host);
        self.keep_world(host);
    }

    /// The world's state outside the blocks, kept when it changed (the
    /// clock alone, every [`crate::persist::TICKS_EVERY`] ticks).
    fn keep_world(&mut self, host: &mut impl ServerHost) {
        let r = &mut self.ready;
        r.world.ticks += 1;
        // Compared without the clock, which changes every tick.
        let now = r.world.kept(0);
        if now != r.kept || r.world.ticks.is_multiple_of(crate::persist::TICKS_EVERY) {
            match host.set(crate::persist::KEY, &r.world.kept(r.world.ticks)) {
                Ok(()) => r.kept = now,
                Err(e) => host.log(&format!("world state: {e}")),
            }
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
        if tracked(old) {
            self.queued.push(BlockEvent::Broken(dim.into(), pos, old.into()));
        }
        if tracked(new) {
            self.queued.push(BlockEvent::Placed(dim.into(), pos, new.into()));
        }
    }

    /// A machine's block entity seen with its saved state (the world was
    /// opened again): the machine, unless it is already known. False when
    /// the state doesn't read.
    pub fn native_adopt(&mut self, dim: &str, pos: Pos, json: &str) -> bool {
        let Some(r) = self.ready() else { return false };
        let key = (dim.to_string(), pos);
        if r.world.machines.contains_key(&key) {
            return true;
        }
        let Ok(m) = serde_json::from_str::<Machine>(json) else { return false };
        r.world.machines.insert(key.clone(), m.upgraded());
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
            if matches!(m.data, crate::machine::Data::Battery(_) | crate::machine::Data::Bomb(_)) {
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
            }
        }
        for (key, cells) in cells_of {
            let powered = cells.iter().any(|p| seen.get(&(key.0.clone(), *p)).is_some_and(|s| *s > 0));
            if let Some(crate::machine::Data::Combustion(c)) = r.world.machines.get_mut(&key).map(|m| &mut m.data) {
                c.set_powered(powered);
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
    fn native_save(&mut self, host: &mut impl ServerHost) {
        let Some(r) = self.ready() else { return };
        for (key, m) in &r.world.machines {
            if r.loaded.get(key) != Some(&true) {
                continue;
            }
            let Ok(json) = serde_json::to_string(m) else { continue };
            if r.native_saved.get(key) != Some(&json) {
                host.save_machine(&key.0, key.1, &json);
                r.native_saved.insert(key.clone(), json);
            }
        }
        r.native_saved.retain(|k, _| r.world.machines.contains_key(k));
    }
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
        signal: BTreeMap<Pos, u8>,
        buttons: Vec<(String, String, Pos, u32)>,
        kv: crate::kv::MemKv,
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
        fn get_block(&mut self, _dim: &str, p: Pos) -> Option<String> {
            Some(self.blocks.get(&p).cloned().unwrap_or_else(|| "minecraft:air".into()))
        }
        fn set_blocks(&mut self, _dim: &str, blocks: &[(Pos, String)]) -> Result<(), String> {
            for (p, s) in blocks {
                self.blocks.insert(*p, s.clone());
            }
            Ok(())
        }
        fn tell(&mut self, _player: &str, _text: &str) {}
        fn log(&mut self, _text: &str) {}
        fn machine_slots(&mut self, _dim: &str, p: Pos) -> Option<Slots> {
            self.slots.get(&p).cloned()
        }
        fn set_machine_slot(&mut self, _dim: &str, p: Pos, slot: u32, stack: Option<Stack>) -> bool {
            let Some(s) = self.slots.get_mut(&p).and_then(|s| s.get_mut(slot as usize)) else { return false };
            *s = stack;
            true
        }
        fn set_machine_data(&mut self, _dim: &str, p: Pos, values: &[i32]) -> bool {
            self.data.insert(p, values.to_vec());
            true
        }
        fn save_machine(&mut self, _dim: &str, p: Pos, json: &str) {
            self.saved.insert(p, json.into());
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

    /// A firebox put in the world burns the coal in its block entity's slot,
    /// shows it, and keeps its state there; a server that opens the world
    /// again takes the machine back from it.
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
