//! The screens' sessions and commands on the server (RBMK T06, `screens::control`). A screen that opens gets a
//! session and the block's first snapshot (the menu's extra data); a command the session accepts waits, guarded by the
//! block entity's generation, for its machine's next transaction, where it is applied before the machine's own step
//! (as a menu button is). The 1.7.10 applies `receiveControl` when the packet arrives, between two ticks: here it is at
//! most the next tick's start. While a screen is open its block's snapshot goes to the player whenever it changes.
use super::*;
use crate::machine::Data;
use crate::screens::control::{Command, Fields, Grant, Refusal, Snapshot};

/// The fields a machine's screen shows, for the machines that take commands.
fn screen_fields(m: &Machine) -> Option<Fields> {
    match &m.data {
        Data::Rbmk(c) if crate::rbmk::screen::takes_commands(&m.block) => crate::rbmk::screen::snapshot(&m.block, c),
        Data::RbmkConsole(c) => Some(crate::rbmk::console::snapshot(c)),
        _ => None,
    }
}

impl Server {
    /// A player opens the screen of the machine at `pos` (`native_menu_extra_data`): a new session (the player's
    /// earlier one ends) and the machine's snapshot now, encoded for the client. None for a machine that takes no
    /// command.
    pub fn native_screen_open(&mut self, player: &str, dim: &str, pos: Pos) -> Option<Vec<u8>> {
        let key = (dim.to_string(), pos);
        let m = self.ready.world.machines.get(&key)?;
        if !self.model_current(&key, m) || self.ready.broken.contains_key(&key) {
            return None;
        }
        let fields = screen_fields(m)?;
        let generation = m.generation;
        let session = self.screens.open(player, dim, pos, generation);
        let snapshot = Snapshot { session, generation, pos, tick: self.ready.world.ticks as i64, fields };
        let bytes = Grant { session, snapshot: snapshot.clone() }.encode().ok()?;
        self.screen_sent.insert(player.to_string(), snapshot);
        Some(bytes)
    }

    /// A command from `player`'s client (`native_net_from_client`), the player standing at `at` in `player_dim`: taken
    /// (it waits for its machine's next transaction) or refused whole, and why.
    pub fn native_screen_command(&mut self, player: &str, player_dim: &str, at: [f64; 3], data: &[u8]) -> Result<(), Refusal> {
        let cmd = Command::decode(data).map_err(|e| Refusal::Invalid(format!("{e:?}")))?;
        let dim = self.screens.current(player).map(|s| s.dim.clone()).ok_or(Refusal::NoSession)?;
        let key = (dim, cmd.pos);
        let machine = self
            .ready
            .world
            .machines
            .get(&key)
            .filter(|m| self.model_current(&key, m) && !self.ready.broken.contains_key(&key))
            .cloned();
        self.screens.accept(player, player_dim, at, &cmd, machine.as_ref().map(|m| m.generation))?;
        let m = machine.expect("a session's generation is a machine's");
        if let Data::RbmkConsole(c) = &m.data {
            return self.console_command(&key, c, &cmd.fields);
        }
        // What the Java's `receiveControl` would throw on is refused before it waits (D2).
        if let Data::Rbmk(c) = &m.data {
            crate::rbmk::screen::receive(&m.block, &mut c.clone(), &cmd.fields)?;
        } else {
            return Err(Refusal::Invalid(format!("{} takes no command", m.block)));
        }
        // Into the column's queue: one of each kind (T06 review P3(a)), what the commands in order would do.
        let mut queue: Vec<crate::rbmk::screen::Pending> = Vec::new();
        self.ready.pending_commands.retain(|g| {
            if g.value.0 == key {
                queue.push(g.value.1.clone());
                false
            } else {
                true
            }
        });
        for p in crate::rbmk::screen::pending_of(&m.block, &cmd.fields) {
            crate::rbmk::screen::coalesce(&mut queue, p);
        }
        let guard = generation::CellGuard::machine(&key, &m);
        self.ready.pending_commands.extend(queue.into_iter().map(|p| generation::Guarded { guard: guard.clone(), value: (key.clone(), p) }));
        Ok(())
    }

    /// A console's command (T10, `TileEntityRBMKConsole.receiveControl`): refused whole on what the Java would throw on
    /// (D2), else its screens wait in the console's own queue and what it does to the columns of its window waits in
    /// each column's queue, guarded by that column's generation, for that column's transaction (the level of a manual
    /// rod as its own screen's `level`, a group set outright, a boiler's compressor). A cell with no column, a column of
    /// another kind or one not loaded is left as the Java's `getTileStandard` leaves it: untouched.
    fn console_command(&mut self, key: &Key, c: &crate::rbmk::console::Console, fields: &Fields) -> Result<(), Refusal> {
        use crate::rbmk::{console::Edit, persist, screen::Pending};
        use crate::screens::control::Value;
        let mut after = c.state.clone();
        let edits = crate::rbmk::console::receive(&mut after, fields)?;
        let mut waiting: Vec<(Key, Pending)> = Vec::new();
        if let Some(p) = crate::rbmk::screen::console_pending(&c.state, &after) {
            waiting.push((key.clone(), p));
        }
        let [tx, ty, tz] = c.state.target;
        for e in edits {
            let (at, kind) = match &e {
                Edit::Target { at, .. } | Edit::Color { at, .. } | Edit::Compress { at } => (*at, &e),
            };
            let ck = (key.0.clone(), [tx + at.0, ty, tz + at.1]);
            let Some(cm) = self.ready.world.machines.get(&ck) else { continue };
            if self.ready.loaded.get(&ck) != Some(&true) || !matches!(cm.data, Data::Rbmk(_)) {
                continue;
            }
            let p = match kind {
                Edit::Target { level, .. } if persist::is_manual_control(&cm.block) => {
                    let mut f = Fields::default();
                    f.set("level", Value::Double(*level));
                    Pending::Set("level", f)
                }
                Edit::Color { color, .. } if persist::is_manual_control(&cm.block) => Pending::Groups([Some(*color); 6]),
                Edit::Compress { .. } if cm.block == "ntm:rbmk_boiler" => Pending::Compress(1),
                _ => continue,
            };
            waiting.push((ck, p));
        }
        for (k, p) in waiting {
            let Some(target) = self.ready.world.machines.get(&k).cloned() else { continue };
            let mut queue: Vec<Pending> = Vec::new();
            self.ready.pending_commands.retain(|g| {
                if g.value.0 == k {
                    queue.push(g.value.1.clone());
                    false
                } else {
                    true
                }
            });
            crate::rbmk::screen::coalesce(&mut queue, p);
            let guard = generation::CellGuard::machine(&k, &target);
            self.ready.pending_commands.extend(queue.into_iter().map(|p| generation::Guarded { guard: guard.clone(), value: (k.clone(), p) }));
        }
        Ok(())
    }

    /// The player left: their session ends.
    pub fn native_screen_left(&mut self, player: &str) {
        self.screens.close(player);
        self.screen_sent.remove(player);
    }

    /// The snapshots to send now: each open session's machine, where it changed since the player was last told (its
    /// fields; the tick alone is no change). A session whose machine went stays quiet.
    pub fn native_screen_snapshots(&mut self) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        let players: Vec<String> = self.screen_sent.keys().cloned().collect();
        for player in players {
            let Some(s) = self.screens.current(&player).cloned() else { continue };
            let key = (s.dim.clone(), s.pos);
            let Some(m) = self.ready.world.machines.get(&key).filter(|m| m.generation == s.generation) else { continue };
            let Some(fields) = screen_fields(m) else { continue };
            let last = &self.screen_sent[&player];
            if last.session == s.id && last.fields == fields {
                continue;
            }
            let snapshot = Snapshot { session: s.id, generation: s.generation, pos: s.pos, tick: self.ready.world.ticks as i64, fields };
            if let Ok(bytes) = snapshot.encode() {
                out.push((player.clone(), bytes));
                self.screen_sent.insert(player, snapshot);
            }
        }
        out
    }
}
