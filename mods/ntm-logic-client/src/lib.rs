//! NTM port, client side: the Horizon API under `ntm_core::client`.

use horizon_mod_sdk::{
    Mod, Registration, chat, command, game, hud, input, storage,
    ui::{self, ButtonWidget, ItemWidget, ListWidget, Screen, TextWidget, Widget},
};
use ntm_core::{
    blockstate,
    client::{Client, ClientHost, TickInput, UiWidget},
    kv::Kv,
    proto::Held,
};

/// Blocks within reach of the use button (survival's 4.5, and a margin).
const REACH: f32 = 5.0;

struct Host;

impl Kv for Host {
    fn get(&mut self, key: &str) -> Option<Vec<u8>> {
        storage::get(key)
    }
    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), String> {
        storage::set(key, value)
    }
    fn delete(&mut self, key: &str) {
        storage::delete(key)
    }
}

impl ClientHost for Host {
    fn run_quiet(&mut self, lines: &[String]) {
        command::run_quiet(lines);
    }
    fn open_screen(&mut self, title: &str, widgets: Vec<UiWidget>) {
        let widgets = widgets
            .into_iter()
            .map(|w| match w {
                UiWidget::Text { id, text, color } => Widget::Text(TextWidget { id, text, color }),
                UiWidget::Button { id, label } => Widget::Button(ButtonWidget { id, label }),
                UiWidget::Item { id, item, name } => Widget::Item(ItemWidget { id, item, name }),
                UiWidget::List { id, rows, selected } => Widget::List(ListWidget { id, rows, selected }),
            })
            .collect();
        ui::open_screen(&Screen {
            title: title.into(),
            widgets,
        });
    }
    fn close_screen(&mut self) {
        ui::close_screen();
    }
    fn chat(&mut self, text: &str) {
        chat::show(text);
    }
    fn log(&mut self, text: &str) {
        horizon_mod_sdk::info(text);
    }
}

fn held() -> Option<Held> {
    input::held().map(|h| Held {
        item: h.id,
        count: h.count,
    })
}

struct Logic {
    engine: Client,
    last_hud: Vec<String>,
}

impl Mod for Logic {
    fn init() -> Result<(Self, Registration), String> {
        // R: opens the machine looked at even with a block in the hand (the
        // use button would place it). Rebindable in Options > Mods.
        input::register_key("open", "Abrir máquina do NTM", "key.keyboard.r")?;
        // The detonator's use is ours: no placing, no swing (docs/bombas.md).
        input::claim_items(&[ntm_core::bomb::DETONATOR.to_string()]);
        Ok((
            Self {
                engine: Client::new(),
                last_hud: Vec::new(),
            },
            Registration::new().on_tick().on_frame().command("ntm"),
        ))
    }

    fn frame(&mut self, seconds: f32) {
        self.engine.frame(seconds, game::get_clock().is_some());
        if game::get_clock().is_none() && !self.last_hud.is_empty() {
            hud::clear();
            self.last_hud.clear();
        }
    }

    fn tick(&mut self, _tick: u64) {
        let (Some(clock), Some(player)) = (game::get_clock(), game::get_player()) else {
            return;
        };
        let target = game::raycast(REACH).map(|h| {
            (
                [h.block_x, h.block_y, h.block_z],
                blockstate::id_of(&h.block).to_string(),
            )
        });
        // With the detonator in hand the use button is the detonator's, not
        // the machine screen's.
        let detonator = input::held().is_some_and(|h| h.id == ntm_core::bomb::DETONATOR);
        let use_down = !detonator && input::mapping("key.use").is_some_and(|m| m.down);
        let use_clicks = if detonator { 0 } else { input::clicks_per_second().1 };
        let events = input::poll();
        let open_key = events
            .iter()
            .any(|e| matches!(e, input::Event::KeyDown(k) if k == "open"));
        let detonate = events
            .iter()
            .any(|e| matches!(e, input::Event::UseStart(u) if u.item.id == ntm_core::bomb::DETONATOR));
        if detonate {
            let sneaking = input::mapping("key.sneak").is_some_and(|m| m.down);
            let at = target.as_ref().map(|(p, _)| *p);
            horizon_mod_sdk::info(&format!("detonator used at {at:?}, sneaking {sneaking}"));
            self.engine.detonator(&mut Host, &player.dimension, at, sneaking);
        }
        self.engine.tick(
            &mut Host,
            TickInput {
                game_time: clock.game_time,
                pos: [player.x, player.y, player.z],
                dim: player.dimension,
                use_down,
                use_clicks,
                open_key,
                target,
            },
        );
        let now = self
            .engine
            .radiation
            .as_ref()
            .map_or_else(Vec::new, |r| r.lines());
        if now != self.last_hud {
            if let Some(r) = &self.engine.radiation {
                let lines: Vec<hud::Line> = now
                    .iter()
                    .map(|text| hud::Line {
                        text: text.clone(),
                        color: r.color(),
                    })
                    .collect();
                hud::set_panel(hud::Corner::TopLeft, &lines);
            } else {
                hud::clear();
            }
            self.last_hud = now;
        }
    }

    fn command(&mut self, _name: &str, args: &str) -> Result<Option<String>, String> {
        let words: Vec<&str> = args.split_whitespace().collect();
        match words.as_slice() {
            [] | ["status"] => Ok(Some(self.engine.status())),
            ["open", x, y, z] => {
                let p = |s: &str| s.parse::<i32>().map_err(|e| e.to_string());
                let dim = game::get_player().map(|p| p.dimension).ok_or("no player")?;
                self.engine.open(&mut Host, &dim, [p(x)?, p(y)?, p(z)?]);
                Ok(None)
            }
            ["run", ..] => {
                // Debugging: one line through the same quiet path as the
                // server's transactions.
                let line = args.trim_start().trim_start_matches("run").trim().to_string();
                command::run_quiet(&[line]);
                Ok(None)
            }
            ["reset"] => {
                self.engine.left = true;
                Ok(Some("Nova sessão no próximo tick".into()))
            }
            _ => Err("/ntm [status | open <x> <y> <z> | run <linha> | reset]".into()),
        }
    }

    fn ui_select(&mut self, id: u32, row: u32) {
        self.engine.select_row(&mut Host, id, row, held());
    }

    fn ui_closed(&mut self) {
        self.engine.closed(&mut Host);
    }
}

horizon_mod_sdk::export_mod!(Logic);
