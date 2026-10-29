//! The client mod's engine (docs/fundacao.md): sessions, the mailbox, the
//! machine screens and running the server's vanilla commands, over a
//! [`ClientHost`] the mod implements with the Horizon API.

use crate::{
    Pos,
    hash::Entropy,
    kv::{Kv, keys},
    proto::{C2S, Held, Mailbox, S2C},
    registry,
    view::{self, ViewModel, Widget},
};

/// A `ui` widget as the client mod hands it to the game.
#[derive(Clone, Debug, PartialEq)]
pub enum UiWidget {
    Text { id: u32, text: String, color: u32 },
    Button { id: u32, label: String },
    Item { id: u32, item: String, name: String },
    /// A `ui` list: a pick is `select(id, row)`.
    List { id: u32, rows: Vec<String>, selected: Option<u32> },
}

pub trait ClientHost: Kv {
    /// `command.run-quiet`: at most 256 lines a call.
    fn run_quiet(&mut self, lines: &[String]);
    fn open_screen(&mut self, title: &str, widgets: Vec<UiWidget>);
    fn close_screen(&mut self);
    fn chat(&mut self, text: &str);
    fn log(&mut self, text: &str);
}

/// What the client mod read this tick.
#[derive(Clone, Debug, Default)]
pub struct TickInput {
    /// `game.get-clock().game-time`: per world, so a jump means another world.
    pub game_time: u64,
    pub dim: String,
    /// The player's feet: the server picks this client's player by it.
    pub pos: [f64; 3],
    /// The use button (`input.mapping("key.use")`) is down.
    pub use_down: bool,
    /// Use clicks in the last second (`input.clicks-per-second`): catches a
    /// click shorter than a tick.
    pub use_clicks: u32,
    /// The mod's own "open" key went down this tick: opens the machine looked
    /// at whatever the hand holds (the use button would place a held block).
    pub open_key: bool,
    /// The block under the crosshair within reach: position and id.
    pub target: Option<(Pos, String)>,
}

/// Widget ids of texts and items (buttons use their action).
const TEXT_IDS: u32 = 10_000;
/// `command.run-quiet` lines one hook call may run: the game cuts the rest
/// off without a word, so a `Run` that doesn't fit whole waits a tick.
pub const RUN_BUDGET: usize = 256;

pub struct Client {
    pub radiation: Option<crate::radiation::Reading>,
    mail: Option<Mailbox<C2S, S2C>>,
    pub entropy: Entropy,
    /// The machine whose screen is open.
    screen: Option<(String, Pos)>,
    /// Asked to open, waiting for its view.
    opening: Option<(String, Pos)>,
    pub wid: Option<String>,
    last_time: Option<u64>,
    /// Game-time jumps seen (lag, the server's corrections), logged sparsely.
    time_jumps: u32,
    use_was_down: bool,
    use_clicks: u32,
    /// Errors logged, so a failing write doesn't log every tick.
    errors: u32,
    /// Left the world (title screen): the next tick starts a session.
    pub left: bool,
    /// Frames in a row with no world: a few happen while one loads.
    no_world_frames: u32,
    pub ran: u64,
    /// `Run`s received, each run whole and in order, `RUN_BUDGET` a tick.
    runs: std::collections::VecDeque<Vec<String>>,
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    pub fn new() -> Self {
        Self {
            mail: None,
            radiation: None,
            entropy: Entropy::default(),
            screen: None,
            opening: None,
            wid: None,
            last_time: None,
            time_jumps: 0,
            use_was_down: false,
            use_clicks: 0,
            errors: 0,
            left: true,
            no_world_frames: 0,
            ran: 0,
            runs: std::collections::VecDeque::new(),
        }
    }

    pub fn session(&self) -> Option<u64> {
        self.mail.as_ref().map(|m| m.sid)
    }

    /// Every frame: feeds the entropy pool (frame times jitter).
    pub fn frame(&mut self, seconds: f32, in_world: bool) {
        self.entropy.feed(u64::from(seconds.to_bits()));
        if in_world {
            self.no_world_frames = 0;
        } else {
            self.no_world_frames += 1;
            if self.no_world_frames >= 20 {
                self.left = true;
            }
        }
    }

    fn start_session(&mut self, host: &mut impl ClientHost, input: &TickInput, why: &str) {
        let sid = self.entropy.draw() | 1;
        let inst = self.entropy.draw();
        let mut mail = Mailbox::with_instance(keys::C2S, keys::S2C, sid, inst);
        mail.send(C2S::Hello {
            protocol: crate::PROTOCOL,
            dim: input.dim.clone(),
            pos: input.pos,
        });
        // Lines of the old session were for its server (or world).
        self.runs.clear();
        self.mail = Some(mail);
        self.wid = None;
        self.radiation = None;
        if self.screen.take().is_some() {
            host.close_screen();
        }
        self.opening = None;
        host.log(&format!("session {sid:x} ({why})"));
    }

    pub fn tick(&mut self, host: &mut impl ClientHost, input: TickInput) {
        self.entropy.feed(input.game_time);
        // The game time is no sign of another world: the client's runs on
        // its own ticks and the server's `SetTime` overwrites it, so a slow
        // server sets it back and a stalled client jumps it ahead.
        if let Some(t) = self.last_time
            && (input.game_time < t || input.game_time > t + 100)
        {
            self.time_jumps += 1;
            if self.time_jumps.is_power_of_two() {
                host.log(&format!(
                    "game time {t} -> {} ({}x): same session",
                    input.game_time, self.time_jumps
                ));
            }
        }
        self.last_time = Some(input.game_time);
        if self.left || self.mail.is_none() {
            self.left = false;
            self.start_session(host, &input, "entered a world");
        }
        let polled = loop {
            let Some(m) = self.mail.as_mut() else { return };
            let had = m.peer;
            let polled = m.poll(host);
            // Another server: in single player the server mod starts anew
            // with each world opened (and with a restart of the mod). It
            // never hears a session it doesn't know, so a new one says hello.
            if polled.new_peer && had.is_some() {
                self.start_session(host, &input, "the server changed");
                continue;
            }
            break polled;
        };
        for m in polled.msgs {
            self.message(host, m);
        }
        self.run_queued(host);

        let pressed = (input.use_down && !self.use_was_down)
            || input.use_clicks > self.use_clicks
            || input.open_key;
        self.use_was_down = input.use_down;
        self.use_clicks = input.use_clicks;
        // The use button reaches us only while no screen is open, so a press
        // opens even when we think ours still is (a screen closed by the
        // game says nothing).
        if pressed
            && let Some((pos, id)) = &input.target
            && registry::machine_ids().any(|m| m == id)
        {
            let key = (input.dim.clone(), *pos);
            self.send(C2S::Open {
                dim: key.0.clone(),
                pos: key.1,
            });
            self.opening = Some(key);
        }
        self.flush(host);
    }

    fn send(&mut self, m: C2S) {
        if let Some(mail) = self.mail.as_mut() {
            mail.send(m);
        }
    }

    pub fn flush(&mut self, host: &mut impl ClientHost) {
        if let Some(mail) = self.mail.as_mut()
            && let Err(e) = mail.flush(host)
        {
            self.errors += 1;
            if self.errors.is_power_of_two() {
                host.log(&format!("c2s ({}x): {e}", self.errors));
            }
        }
    }

    fn message(&mut self, host: &mut impl ClientHost, m: S2C) {
        match m {
            S2C::Radiation { reading } => self.radiation = Some(reading),
            S2C::World { wid } => {
                host.log(&format!("world {wid}"));
                self.wid = Some(wid);
            }
            S2C::View { dim, pos, vm } => {
                let key = (dim, pos);
                if self.screen.as_ref() == Some(&key) || self.opening.as_ref() == Some(&key) {
                    let (title, widgets) = render(&vm, &key);
                    host.open_screen(&title, widgets);
                    self.screen = Some(key);
                    self.opening = None;
                }
            }
            S2C::Close => {
                self.opening = None;
                if self.screen.take().is_some() {
                    host.close_screen();
                }
            }
            S2C::Run { lines, .. } => {
                if !lines.is_empty() {
                    self.runs.push_back(lines);
                }
            }
            S2C::Toast { text } => host.chat(&text),
        }
    }

    /// The queued `Run`s that fit this call's budget, each whole. A `Run`
    /// longer than the budget (the server never sends one) goes in pieces
    /// over several ticks rather than never.
    fn run_queued(&mut self, host: &mut impl ClientHost) {
        let mut budget = RUN_BUDGET;
        while let Some(front) = self.runs.front_mut() {
            if front.len() <= budget {
                let lines = self.runs.pop_front().unwrap_or_default();
                budget -= lines.len();
                self.ran += lines.len() as u64;
                host.run_quiet(&lines);
            } else if budget == RUN_BUDGET {
                let rest = front.split_off(RUN_BUDGET);
                let head = std::mem::replace(front, rest);
                self.ran += head.len() as u64;
                host.run_quiet(&head);
                break;
            } else {
                break;
            }
        }
    }

    /// `Run`s waiting for a tick with budget.
    pub fn queued_runs(&self) -> usize {
        self.runs.len()
    }

    /// A button of our screen (`ui-events.select`).
    /// A row of a list of our screen (`ui-events.select(id, row)`): the
    /// action is the list's own plus the row (`view::LIST_ACTIONS`).
    pub fn select_row(&mut self, host: &mut impl ClientHost, id: u32, row: u32, held: Option<Held>) {
        let id = if id == view::LIST_ACTIONS { id + row.min(view::LIST_MAX as u32 - 1) } else { id };
        self.select(host, id, held);
    }

    pub fn select(&mut self, host: &mut impl ClientHost, id: u32, held: Option<Held>) {
        let Some((dim, pos)) = self.screen.clone() else {
            return;
        };
        if id >= TEXT_IDS {
            return;
        }
        self.send(C2S::Action {
            dim,
            pos,
            action: id,
            held,
        });
        self.flush(host);
    }

    /// The player left our screen (`ui-events.closed`).
    pub fn closed(&mut self, host: &mut impl ClientHost) {
        if self.screen.take().is_some() {
            self.send(C2S::Close);
            self.flush(host);
        }
    }

    /// The detonator was used (`input` use-start on `ntm:detonator`):
    /// sneaking links it to the bomb looked at, else it sets its bomb off.
    pub fn detonator(&mut self, host: &mut impl ClientHost, dim: &str, target: Option<Pos>, sneaking: bool) {
        self.send(C2S::Detonator {
            dim: dim.into(),
            target,
            link: sneaking,
        });
        self.flush(host);
    }

    /// `/ntm open x y z`: the screen of a machine without the use button.
    pub fn open(&mut self, host: &mut impl ClientHost, dim: &str, pos: Pos) {
        self.opening = Some((dim.into(), pos));
        self.send(C2S::Open {
            dim: dim.into(),
            pos,
        });
        self.flush(host);
    }

    pub fn status(&self) -> String {
        format!(
            "NTM cliente: sessão {} | mundo {} | servidor {} | tela {:?} | comandos rodados {}",
            self.session().map_or("-".into(), |s| format!("{s:x}")),
            self.wid.as_deref().unwrap_or("?"),
            match self.mail.as_ref().and_then(|m| m.peer) {
                Some(p) => format!("{p:x}"),
                None => "ausente".into(),
            },
            self.screen.as_ref().map(|s| s.1),
            self.ran
        )
    }
}

/// A view-model as `ui` widgets.
pub fn render(vm: &ViewModel, key: &(String, Pos)) -> (String, Vec<UiWidget>) {
    let mut out = Vec::new();
    for (i, w) in vm.widgets.iter().enumerate().take(63) {
        let id = TEXT_IDS + i as u32;
        out.push(match w {
            Widget::Text { text, color } => UiWidget::Text {
                id,
                text: text.clone(),
                color: *color,
            },
            Widget::Bar {
                label,
                value,
                max,
                unit,
            } => UiWidget::Text {
                id,
                text: view::bar_text(label, *value, *max, unit),
                color: 0,
            },
            Widget::Button { action, label } => UiWidget::Button {
                id: *action,
                label: label.clone(),
            },
            Widget::List { action, rows, selected } => UiWidget::List {
                id: *action,
                rows: rows.clone(),
                selected: *selected,
            },
            Widget::Item { item, name } => UiWidget::Item {
                id,
                item: item.clone(),
                name: name.clone(),
            },
        });
    }
    let p = key.1;
    out.push(UiWidget::Text {
        id: TEXT_IDS + 63,
        text: format!("{} {} {}", p[0], p[1], p[2]),
        color: view::GREY,
    });
    (vm.title.clone(), out)
}
