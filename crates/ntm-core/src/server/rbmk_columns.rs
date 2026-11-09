//! The RBMK columns as blocks (`rbmk-plano` T02): their height, dummies, lids and dials.
//!
//! - A column is an engine column (rbmk-t02-engine): its core at the bottom and the run of `ntm:dummy_rbmk` above it,
//!   2..16 cells. Its height is the world's, each one as tall as it was placed (D5): the dial `dialColumnHeight` sets
//!   only the height of the columns placed from then on (`ServerHost::set_column_height`), and `/ntmrbmk migrate`
//!   rebuilds standing ones to it, column by column, each in one step of its own chunk.
//! - The dials are Java's game rules (`RBMKDials`), one set for the save, kept with the world's state; the server
//!   creates them as `createDials` does when it starts, and `/ntmrbmk dial` sets one as `/gamerule` does, then
//!   refreshes the cache (`ModEventHandler.commandEvent`).
//! - A dummy whose run no longer reaches a core goes, as `BlockDummyable.destroyIfOrphan` takes it away, the next above
//!   it then: a column broken any other way than by a player (a command, an explosion) leaves no dummy behind.
//! - Lids (`ItemRBMKLid`, `RBMKBase.onScrew`, `RBMKBase.breakBlock`): one goes on a column that has none, the hand
//!   giving it in the same transfer; a screwdriver takes it off as an item, owed in the column's own chunk record with
//!   the change; a broken column drops it. Both drops are where Java puts them: at the top as the dial reads now
//!   (`getColumnHeight`), not as the column stands.
use super::*;
use crate::{
    machine::Data,
    rbmk::{
        dials::{DialState, Dials, KEYS},
        persist::{self, Lid},
    },
};

/// The most cells a column has (`dialColumnHeight` clamps to 2..16).
pub const TALLEST: i32 = 16;

/// The core of the column one of whose cells is at `pos` (`BlockDummyable.findCore` down its run of dummies): None
/// when the cell is no column's, its run reaches no core, or a cell on the way is unknown.
pub(super) fn column_core(host: &mut impl ServerHost, dim: &str, pos: Pos) -> Option<Pos> {
    for dy in 0..TALLEST {
        let at = [pos[0], pos[1] - dy, pos[2]];
        let b = host.get_block(dim, at)?;
        let id = blockstate::id_of(&b);
        if persist::is_column(id) {
            return (dy == 0 || standing_height(host, dim, at) > dy).then_some(at);
        }
        if id != persist::DUMMY {
            return None;
        }
    }
    None
}

/// How tall the column whose core is at `core` stands: the core and the run of dummies right above it, as the
/// engine reads it (`mod_multiblocks::standing`); an unknown cell ends the run.
pub(super) fn standing_height(host: &mut impl ServerHost, dim: &str, core: Pos) -> i32 {
    1 + (1..TALLEST)
        .take_while(|dy| host.get_block(dim, [core[0], core[1] + dy, core[2]]).is_some_and(|b| blockstate::id_of(&b) == persist::DUMMY))
        .count() as i32
}

/// What one column's rebuilding to a height does: the cells it writes, or why it cannot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rebuild {
    Unchanged,
    Cells(Vec<(Pos, String)>),
    /// A cell the taller column needs is not free (air) or not known.
    Blocked(Pos),
}

/// The cells that make the column at `core`, `from` cells high, `to` cells high: dummies added above it into air, or
/// its top dummies taken away.
pub(super) fn rebuild(host: &mut impl ServerHost, dim: &str, core: Pos, from: i32, to: i32) -> Rebuild {
    let at = |dy: i32| [core[0], core[1] + dy, core[2]];
    if from == to {
        return Rebuild::Unchanged;
    }
    if to < from {
        return Rebuild::Cells((to..from).map(|dy| (at(dy), "minecraft:air".to_string())).collect());
    }
    let mut cells = Vec::new();
    for dy in from..to {
        match host.get_block(dim, at(dy)) {
            Some(b) if is_air(&b) => cells.push((at(dy), persist::DUMMY.to_string())),
            _ => return Rebuild::Blocked(at(dy)),
        }
    }
    Rebuild::Cells(cells)
}

/// Air, by the block's id.
fn is_air(state: &str) -> bool {
    matches!(blockstate::id_of(state), "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
}

impl Server {
    /// The dials as the server reads them (`RBMKDials.getGameRule`): created when it started.
    pub(super) fn rbmk_dials(&mut self) -> Dials {
        *self.ready.rbmk_dials.get(KEYS[0].name)
    }

    /// `createDials` as the server starts (`MainRegistry.serverStart`): the cache from the rules kept, then, unless
    /// `dialSaveDials` is true, every rule back to its default (the cache keeps the old values until a refresh, D2);
    /// the engine places columns as tall as the cache says.
    pub(super) fn rbmk_start(&mut self, host: &mut impl ServerHost) {
        let mut d = DialState::default();
        d.rules = (*self.ready.world.rbmk_rules).clone();
        d.create(false);
        if d.rules != *self.ready.world.rbmk_rules {
            *self.ready.world.rbmk_rules = d.rules.clone();
        }
        self.ready.rbmk_dials = d;
        let height = self.rbmk_dials().column_height + 1;
        host.set_column_height(height);
    }

    /// `/ntmrbmk ...` (operator level 2, as `/gamerule`): `dial [rule [value]]` reads or sets a dial; `migrate x y z
    /// [x2 y2 z2]` rebuilds the columns of a cell or of a box to the dial's height. None: not this command.
    pub fn native_rbmk_command(&mut self, host: &mut impl ServerHost, who: &PlayerInfo, permission: u8, line: &str) -> Option<Result<String, String>> {
        let mut words = line.split_whitespace();
        if words.next() != Some("ntmrbmk") {
            return None;
        }
        if permission < 2 {
            return Some(Err("Este comando exige operador nível 2.".into()));
        }
        let args: Vec<&str> = words.collect();
        Some(match args.first().copied() {
            Some("dial") => self.rbmk_dial_command(host, &args[1..]),
            Some("migrate") => self.rbmk_migrate_command(host, &who.dim, &args[1..]),
            _ => Err("Uso: /ntmrbmk dial [regra [valor]] | /ntmrbmk migrate x y z [x2 y2 z2]".into()),
        })
    }

    fn rbmk_dial_command(&mut self, host: &mut impl ServerHost, args: &[&str]) -> Result<String, String> {
        let rules = &self.ready.world.rbmk_rules;
        match args {
            [] => Ok(KEYS.iter().map(|k| format!("{} = {}", k.rule, rules.string(k.rule))).collect::<Vec<_>>().join("\n")),
            [rule] => KEYS.iter().find(|k| k.rule == *rule).map(|k| format!("{} = {}", k.rule, rules.string(k.rule))).ok_or_else(|| format!("Regra desconhecida: {rule}")),
            [rule, value] => {
                // `CommandGameRule`: only a rule the world has (`createDials` made the 25).
                let Some(key) = KEYS.iter().find(|k| k.rule == *rule) else { return Err(format!("Regra desconhecida: {rule}")) };
                self.ready.world.rbmk_rules.set(key.rule, value);
                self.ready.rbmk_dials.rules = (*self.ready.world.rbmk_rules).clone();
                self.ready.rbmk_dials.refresh();
                let height = self.rbmk_dials().column_height + 1;
                host.set_column_height(height);
                Ok(format!("{} = {value}; colunas novas: {height} blocos (as de pé mudam só com /ntmrbmk migrate)", key.rule))
            }
            _ => Err("Uso: /ntmrbmk dial [regra [valor]]".into()),
        }
    }

    fn rbmk_migrate_command(&mut self, host: &mut impl ServerHost, dim: &str, args: &[&str]) -> Result<String, String> {
        let n: Vec<i32> = args.iter().map(|a| a.parse::<i32>()).collect::<Result<_, _>>().map_err(|_| "Coordenadas inteiras".to_string())?;
        let (lo, hi) = match n[..] {
            [x, y, z] => ([x, y, z], [x, y, z]),
            [a, b, c, d, e, f] => ([a.min(d), b.min(e), c.min(f)], [a.max(d), b.max(e), c.max(f)]),
            _ => return Err("Uso: /ntmrbmk migrate x y z [x2 y2 z2]".into()),
        };
        let volume = (0..3).map(|i| i64::from(hi[i]) - i64::from(lo[i]) + 1).product::<i64>();
        if volume > 32 * 32 * 32 {
            return Err("Caixa grande demais (32768 blocos no máximo)".into());
        }
        let to = self.rbmk_dials().column_height + 1;
        let mut cores = BTreeSet::new();
        for x in lo[0]..=hi[0] {
            for z in lo[2]..=hi[2] {
                for y in lo[1]..=hi[1] {
                    if let Some(core) = column_core(host, dim, [x, y, z]) {
                        cores.insert(core);
                    }
                }
            }
        }
        let (mut done, mut same, mut refused) = (0, 0, Vec::new());
        for core in cores {
            match self.rbmk_rebuild(host, dim, core, to) {
                Ok(true) => done += 1,
                Ok(false) => same += 1,
                Err(why) => refused.push(format!("[{}, {}, {}]: {why}", core[0], core[1], core[2])),
            }
        }
        let mut out = format!("{to} blocos: {done} refeitas, {same} já nessa altura, {} recusadas", refused.len());
        for r in refused.iter().take(8) {
            out.push_str(&format!("\n  {r}"));
        }
        Ok(out)
    }

    /// Rebuilds the column at `core` to `to` cells (D5's explicit migration): its cells, all in the column's chunk,
    /// written in one step. Ok(false): already that tall.
    pub(super) fn rbmk_rebuild(&mut self, host: &mut impl ServerHost, dim: &str, core: Pos, to: i32) -> Result<bool, String> {
        let from = standing_height(host, dim, core);
        match rebuild(host, dim, core, from, to) {
            Rebuild::Unchanged => Ok(false),
            Rebuild::Blocked(p) => Err(format!("célula ocupada ou não carregada em [{}, {}, {}]", p[0], p[1], p[2])),
            Rebuild::Cells(cells) => host.set_blocks(dim, &cells).map(|()| true),
        }
    }

    /// A block changed: when it was a column's cell, the cell above may be an orphaned dummy now
    /// (`BlockDummyable.onNeighborBlockChange`).
    pub(super) fn rbmk_note_change(&mut self, dim: &str, pos: Pos, old: &str, new: &str) {
        let (old, new) = (blockstate::id_of(old), blockstate::id_of(new));
        if old != new && (old == persist::DUMMY || persist::is_column(old)) {
            self.ready.rbmk_orphans.push((dim.to_string(), [pos[0], pos[1] + 1, pos[2]]));
        }
    }

    /// The orphaned dummies go, each to air (its removal notes the one above it: the run goes up to its end). A dummy
    /// with a cell below it not loaded stays (`checkChunksExist`).
    pub(super) fn rbmk_orphans(&mut self, host: &mut impl ServerHost) {
        while let Some((dim, pos)) = self.ready.rbmk_orphans.pop() {
            let Some(b) = host.get_block(&dim, pos) else { continue };
            if blockstate::id_of(&b) != persist::DUMMY {
                continue;
            }
            if host.get_block(&dim, [pos[0], pos[1] - 1, pos[2]]).is_none() {
                continue;
            }
            if column_core(host, &dim, pos).is_none() {
                if let Err(e) = host.set_blocks(&dim, &[(pos, "minecraft:air".into())]) {
                    host.log(&format!("rbmk orphan dummy at {dim} {pos:?}: {e}"));
                }
                self.ready.rbmk_orphans.push((dim, [pos[0], pos[1] + 1, pos[2]]));
            }
        }
    }

    /// Where a lid comes off a column at `core` (`RBMKBase.onScrew` and `breakBlock`): its top as the dial reads now.
    pub(super) fn rbmk_lid_drop_at(&mut self, core: Pos) -> Pos {
        [core[0], core[1] + self.rbmk_dials().column_height, core[2]]
    }

    /// A lid in hand on an RBMK column (`ItemRBMKLid.onItemUse`, reached from `RBMKBase.openInv` when the column has
    /// none): it goes on, one of the stack given in the same transfer (survival only), with the lid's sound. True:
    /// the use was the lid's (done, or refused as a whole). False: the click goes on (a column that has a lid opens
    /// its menu, as `openInv` does).
    pub fn native_rbmk_lid(&mut self, host: &mut impl ServerHost, dim: &str, at: Pos, core: Pos, player: &str) -> bool {
        let key = (dim.to_string(), core);
        let Some(m) = self.ready.world.machines.get(&key).cloned() else { return false };
        let Data::Rbmk(c) = &m.data else { return false };
        let Some(hand) = host.player_slots(player) else { return false };
        let Some(lid) = hand.held().and_then(|h| Lid::of_item(&h.stack.id)) else { return false };
        if c.has_lid(&m.block) || self.ready.broken.contains_key(&key) || !self.model_current(&key, &m) {
            return false;
        }
        let Some(expected) = host.machine_slots(dim, core) else { return true };
        let from = hand.hand.expect("a held stack has a hand");
        let edits = if hand.creative { Vec::new() } else { hand.take(from, 1).into_iter().collect() };
        let mut machine = m.clone();
        let Data::Rbmk(col) = &mut machine.data else { unreachable!() };
        col.lid = lid;
        let (desired, data) = Self::machine_output(&mut machine, &expected);
        let json = serde_json::to_string(&machine).expect("machine serialization");
        let commit = MachineCommit { cells: Vec::new(), guard: CellGuard::machine(&key, &m), expected_saved: self.ready.native_saved.contains_key(&key), dim: dim.into(), pos: core, expected, desired, data, json };
        let transfer = PlayerTransfer { player: player.into(), edits, drops: Vec::new(), copies: Vec::new() };
        if !host.commit_player_transfer(&transfer, std::slice::from_ref(&commit)) {
            return true;
        }
        self.rbmk_published(key, machine, commit);
        // `Blocks.glass.stepSound.func_150496_b()` is `step.stone`; `concrete_smooth`'s, a stone's, `dig.stone`.
        let id = if lid == Lid::Glass { "minecraft:block.stone.step" } else { "minecraft:block.stone.place" };
        host.show(dim, &crate::blast::visuals::Show::Sound { id, source: "block", at: at.map(|v| f64::from(v) + 0.5), volume: 1., pitch: 0.8 });
        true
    }

    /// A screwdriver on an RBMK column (`RBMKBase.onScrew`, a sneaking use: a plain one opens the menu first): a lid
    /// that comes off comes off as an item at the top, owed in the column's chunk record with the change, and the
    /// tool wears in the same publication. True: the screw was the column's (the tool wears).
    pub fn native_rbmk_screw(&mut self, host: &mut impl ServerHost, dim: &str, core: Pos, tool: &str) -> bool {
        let key = (dim.to_string(), core);
        let Some(m) = self.ready.world.machines.get(&key).cloned() else { return false };
        let Data::Rbmk(c) = &m.data else { return false };
        // `hasLid() && isLidRemovable()`: a control rod's own lid never comes off.
        let Some(item) = c.lid.item().filter(|_| !persist::has_own_lid(&m.block)) else { return false };
        if self.ready.broken.contains_key(&key) || !self.model_current(&key, &m) {
            return false;
        }
        let Some(expected) = host.machine_slots(dim, core) else { return false };
        let mut top = self.rbmk_lid_drop_at(core);
        let Some(top_state) = host.get_block(dim, top).or_else(|| {
            top = core;
            host.get_block(dim, core)
        }) else {
            return false;
        };
        let mut machine = m.clone();
        let Data::Rbmk(col) = &mut machine.data else { unreachable!() };
        col.lid = Lid::None;
        let (desired, data) = Self::machine_output(&mut machine, &expected);
        let json = serde_json::to_string(&machine).expect("machine serialization");
        let commit = MachineCommit { cells: Vec::new(), guard: CellGuard::machine(&key, &m), expected_saved: self.ready.native_saved.contains_key(&key), dim: dim.into(), pos: core, expected, desired, data, json };
        let origin = CellGuard { dim: dim.into(), pos: top, generation: 0, state: blockstate::id_of(&top_state).into() };
        let drop = RetainedDrop { origin, items: vec![format!("{{id:\"{item}\",count:1}}")], id: Some(self.batch_serial()) };
        if !host.commit_lid_removal(&commit, &drop, tool) {
            return false;
        }
        self.rbmk_published(key, machine, commit);
        true
    }

    fn rbmk_published(&mut self, key: Key, machine: Machine, commit: MachineCommit) {
        self.ready.world.machines.insert(key.clone(), machine);
        self.ready.bridge.last.insert(key.clone(), commit.desired);
        self.ready.bridge.data.insert(key.clone(), commit.data);
        self.ready.native_saved.insert(key, commit.json);
    }
}
