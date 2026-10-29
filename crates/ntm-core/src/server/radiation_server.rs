//! Platform-independent server hooks for inventory radiation.
use super::*;
use crate::radiation::{self, CELLS, COMMANDS_PER_TICK, Mode, Reading, Sample};

impl Server {
    /// Called after machine requests have been dispatched. Radiation yields
    /// while the world-id probe is active and on ticks machine transactions
    /// go out, and uses only 16 lines/tick of the consumer's 64.
    pub(super) fn radiation_tick(&mut self, host: &mut impl ServerHost) {
        let tick = self.tick;
        // Flush a trailing dose change even when exposure has already stopped.
        if self.radiation.dose_dirty && tick >= self.radiation.next_dose_save {
            if let Some(r) = self.ready() {
                r.urgent = true;
            }
            self.radiation.dose_dirty = false;
            self.radiation.next_dose_save = tick + radiation::DOSE_SAVE_TICKS;
        }
        if self.players.len() != 1 {
            if tick.is_multiple_of(20) {
                self.mail.send_latest(
                    S2C::Radiation {
                        reading: Reading {
                            status: "Radiação M1: disponível em single player".into(),
                            ..Reading::default()
                        },
                    },
                    "radiation",
                );
            }
            return;
        }
        let player = self.players[0].clone();
        if self
            .radiation
            .joined
            .as_ref()
            .is_none_or(|(id, _)| id != &player.id)
        {
            self.radiation.joined = Some((player.id.clone(), tick));
        }
        if self
            .probes
            .iter()
            .any(|p| !p.late && !matches!(p.purpose, Purpose::Radiation { .. }))
        {
            return;
        }
        // Machines first: their lines leave this tick, radiation's the next.
        if self
            .ready()
            .is_some_and(|r| !r.to_publish.is_empty() || !r.verify_publish.is_empty())
        {
            return;
        }
        // Lines leave only once their probe is stored (a crash never leaves
        // cells nobody knows about).
        if !self.radiation.commands.is_empty() && self.probes_durable {
            let n = self.radiation.commands.len().min(COMMANDS_PER_TICK);
            let lines = self.radiation.commands.drain(..n).collect();
            let tx = self.take_id();
            let seq = self.mail.send(S2C::Run { tx, lines });
            if self.radiation.commands.is_empty()
                && let Some(p) = self.probes.iter_mut().find(|p| {
                    p.external && !p.late && p.seq == 0 && matches!(p.purpose, Purpose::Radiation { .. })
                })
            {
                // Its last lines: its deadline counts from their ack.
                p.seq = seq;
            }
            return;
        }
        if tick < self.radiation.next_sample
            || self
                .probes
                .iter()
                // A late one holds the next back for a while: a stalled
                // client never piles up columns under the player.
                .any(|p| {
                    matches!(p.purpose, Purpose::Radiation { .. })
                        && (!p.late
                            || tick + crate::probes::LATE_TICKS
                                < p.late_until + RADIATION_LATE_TICKS)
                })
        {
            return;
        }
        let (x, z) = (player.pos[0].floor() as i32, player.pos[2].floor() as i32);
        let mut found = None;
        // Once, not per cell: with many probes and retired cells, building
        // it per cell overran the tick's fuel.
        let busy = self.busy_cells();
        'search: for dx in 0..4 {
            for dz in 0..4 {
                let positions = worldid::column(&player.dim, x + dx, z + dz);
                let positions: [Pos; CELLS] = positions[..CELLS].try_into().unwrap();
                if positions.iter().any(|p| busy.contains(&(player.dim.clone(), *p))) {
                    continue;
                }
                let states: Option<Vec<_>> = positions
                    .iter()
                    .map(|p| host.get_block(&player.dim, *p))
                    .collect();
                if let Some(states) = states
                    && states
                        .iter()
                        .all(|s| radiation::safe_cell(s) && s.len() <= 40)
                {
                    found = Some((positions, states));
                    break 'search;
                }
            }
        }
        // In id mode, a tag sample now and then: the game may know them now.
        let kind = match self.radiation.mode {
            Mode::Ids if tick >= self.radiation.retry_tags_at => {
                self.radiation.retry_tags_at = tick + radiation::TAG_RETRY_TICKS;
                Mode::Tags
            }
            m => m,
        };
        self.radiation.next_sample = tick + kind.period();
        let Some((positions, states)) = found else {
            self.radiation_reading(&player.id, 0, "Geiger: aguardando terreno carregado");
            return;
        };
        let id = self.take_id();
        let with_states: [(Pos, String); CELLS] =
            std::array::from_fn(|i| (positions[i], states[i].clone()));
        let lines = match kind {
            Mode::Tags => radiation::sample_lines(id, &player.dim, &with_states),
            Mode::Ids => radiation::ids_sample_lines(id, &player.dim, &with_states),
        };
        let sending = lines.len().div_ceil(COMMANDS_PER_TICK) as u64;
        self.radiation.commands = lines.into();
        self.radiation.kinds.retain(|(i, _)| self.probes.iter().any(|p| p.id == *i));
        self.radiation.kinds.push((id, kind));
        let cells = positions
            .into_iter()
            .zip(states)
            .map(|(pos, state)| Cell {
                pos,
                restore: Restore::State(state),
                seen: None,
            })
            .collect();
        let mut probe = Probe::new(
            id,
            self.wid().map(str::to_string),
            &player.dim,
            cells,
            Purpose::Radiation { player: player.id },
        );
        // Commands are dispatched gradually here; the common engine reads
        // and restores cells, but never sends lines of its own for it.
        probe.external = true;
        // Until its last lines are acked; then `PROBE_TICKS` (run_probes).
        probe.deadline = tick + sending + crate::probes::UNACKED_TICKS;
        self.probes.push(probe);
    }

    pub(super) fn radiation_done(
        &mut self,
        host: &mut impl ServerHost,
        id: u32,
        player: &str,
        levels: &[Option<u8>],
    ) {
        self.radiation.commands.clear();
        if self.players.len() != 1 || self.players[0].id != player {
            return;
        }
        let kind = self
            .radiation
            .kinds
            .iter()
            .find(|(i, _)| *i == id)
            .map_or(Mode::Tags, |(_, k)| *k);
        let rate = match radiation::read(id, levels) {
            Sample::Rate(rate) => {
                if kind == Mode::Tags {
                    if self.radiation.mode == Mode::Ids {
                        host.log("radiation: the game knows the mod's tags again; tag samples");
                    }
                    self.radiation.mode = Mode::Tags;
                    self.radiation.tag_misses = 0;
                }
                rate
            }
            Sample::TagsUnknown => {
                self.radiation.tag_misses += 1;
                let switch = self.radiation.mode == Mode::Tags
                    && self.radiation.tag_misses >= radiation::TAG_MISSES;
                if switch {
                    host.log(
                        "radiation: the game doesn't know the mod's item tags in commands; \
                         sampling by item ids (one sample every 3 s)",
                    );
                    self.radiation.mode = Mode::Ids;
                    self.radiation.retry_tags_at = self.tick + radiation::TAG_RETRY_TICKS;
                    self.radiation.next_sample = self.tick;
                }
                self.radiation.last_dose = None;
                self.radiation_reading(player, 0, "Geiger: tags do mod desconhecidas; trocando para leitura por ids");
                return;
            }
            Sample::Incomplete => {
                self.radiation.last_dose = None;
                self.radiation_reading(player, 0, "Geiger: sem confirmação (comandos ligados?)");
                host.log("radiation probe timed out or incomplete; no dose applied");
                return;
            }
        };
        let grace = self
            .radiation
            .joined
            .as_ref()
            .is_none_or(|(_, t)| self.tick.saturating_sub(*t) < radiation::GRACE_TICKS);
        // One confirmed sample stands for the ticks since the last one
        // (its period at least, if that one had no rate), up to a cap.
        let ticks = radiation::exposure_ticks(kind, self.radiation.last_dose, self.tick);
        self.radiation.last_dose = (!grace).then_some(self.tick);
        let mut lines = Vec::new();
        let mut changed = false;
        let mut crossed_threshold = false;
        if let Some(r) = self.ready() {
            let dose = r.world.radiation.entry(player.to_string()).or_default();
            if !grace {
                let old = dose.clone();
                radiation::add_dose(dose, u64::from(rate) * ticks / 20);
                crossed_threshold =
                    old.millirad / 200_000 != dose.millirad / 200_000 || dose.millirad >= 1_000_000;
                lines = radiation::effect_lines(dose);
                changed = old != *dose;
                r.dirty |= changed;
                if crossed_threshold {
                    r.urgent = true;
                }
            }
        }
        self.radiation.dose_dirty |= changed;
        if crossed_threshold {
            self.radiation.dose_dirty = false;
            self.radiation.next_dose_save = self.tick + radiation::DOSE_SAVE_TICKS;
        }
        if !lines.is_empty() {
            host.log(&format!(
                "radiation sickness: {} command(s) for {player}",
                lines.len()
            ));
            let tx = self.take_id();
            self.mail.send(S2C::Run { tx, lines });
        }
        // The fallout zones of nukes add to what the Geiger shows; their dose
        // is the server's (`bomb_server.rs`, every second).
        self.radiation_reading(
            player,
            rate.saturating_add(self.area_rate(player)),
            if grace {
                "Proteção inicial (10 s)"
            } else if self.radiation.mode == Mode::Ids {
                "Modo compatível: leitura por ids a cada 3 s"
            } else {
                ""
            },
        );
    }

    fn radiation_reading(&mut self, player: &str, rate: u32, status: &str) {
        let dose = self
            .ready()
            .and_then(|r| r.world.radiation.get(player))
            .map_or(0, |d| d.millirad);
        self.mail.send_latest(
            S2C::Radiation {
                reading: Reading {
                    dose_millirad: dose,
                    rate_millirad_s: rate,
                    status: status.into(),
                },
            },
            "radiation",
        );
    }
}
