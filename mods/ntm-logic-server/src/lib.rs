//! NTM port, server side: the Horizon API under `ntm_core::server`.

// The pool allocator (docs/explosoes.md §10): every block state the host
// returns is allocated in the guest, and dlmalloc's path dominated the fuel
// of a `get-block` (measured: docs/bombas.md, "Pool"). Only in WebAssembly:
// the crate is also an rlib, and native tests run on several threads.
#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOC: ntm_core::Pool = ntm_core::Pool::new();

use horizon_mod_sdk::{
    server::{
        Argument, Args, BlockChange, BlockPos, GameMode, Player, Registration, ServerMod, players,
        world,
    },
    storage,
};
use ntm_core::{
    Pos,
    kv::Kv,
    server::{PlayerInfo, Server, ServerHost},
};

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

impl ServerHost for Host {
    fn get_block(&mut self, dim: &str, p: Pos) -> Option<String> {
        world::get_block(dim, p[0], p[1], p[2])
    }
    fn set_blocks(&mut self, dim: &str, blocks: &[(Pos, String)]) -> Result<(), String> {
        let changes: Vec<BlockChange> = blocks
            .iter()
            .map(|(p, s)| BlockChange {
                x: p[0],
                y: p[1],
                z: p[2],
                state: s.clone(),
            })
            .collect();
        world::set_blocks(dim, &changes)
    }
    fn tell(&mut self, player: &str, text: &str) {
        players::tell(player, text);
    }
    fn log(&mut self, text: &str) {
        horizon_mod_sdk::info(text);
    }
    // Capability `machines` (Horizon patch ntm-visual, docs/visual-fiel.md):
    // only with the cargo feature, since a Horizon without it refuses the mod.
    #[cfg(feature = "machines")]
    fn machine_slots(&mut self, dim: &str, p: Pos) -> Option<ntm_core::server::container_bridge::Slots> {
        let slots = horizon_mod_sdk::server::machines::get_slots(dim, p[0], p[1], p[2])?;
        Some(
            slots
                .into_iter()
                .map(|s| s.map(|s| ntm_core::server::container_bridge::Stack { id: s.id, count: s.count, damage: s.damage, custom: None }))
                .collect(),
        )
    }
    #[cfg(feature = "machines")]
    fn set_machine_slot(
        &mut self,
        dim: &str,
        p: Pos,
        slot: u32,
        stack: Option<ntm_core::server::container_bridge::Stack>,
    ) -> bool {
        let stack = stack.map(|s| horizon_mod_sdk::server::ItemStack { id: s.id, count: s.count, damage: s.damage });
        horizon_mod_sdk::server::machines::set_slot(dim, p[0], p[1], p[2], slot, stack.as_ref())
    }
    #[cfg(feature = "machines")]
    fn set_machine_data(&mut self, dim: &str, p: Pos, values: &[i32]) -> bool {
        horizon_mod_sdk::server::machines::set_data(dim, p[0], p[1], p[2], values)
    }
    #[cfg(feature = "machines")]
    fn machine_data(&mut self, dim: &str, p: Pos) -> Option<Vec<i32>> {
        horizon_mod_sdk::server::machines::get_data(dim, p[0], p[1], p[2])
    }
    #[cfg(feature = "machines")]
    fn machine_buttons(&mut self) -> Vec<(String, String, Pos, u32)> {
        horizon_mod_sdk::server::machines::poll_buttons()
            .into_iter()
            .map(|b| (b.player, b.dimension, [b.x, b.y, b.z], b.button))
            .collect()
    }
}

fn info(p: &Player) -> PlayerInfo {
    PlayerInfo {
        id: p.id.clone(),
        name: p.name.clone(),
        dim: p.dimension.clone(),
        pos: [p.x, p.y, p.z],
        // Espectador conta como criativo: imune a dano e radiação das bombas
        // (a amostra do inventário zera os dois modos).
        creative: matches!(p.game_mode, GameMode::Creative | GameMode::Spectator),
    }
}

struct Logic {
    engine: Server,
}

impl ServerMod for Logic {
    fn init() -> Result<(Self, Registration), String> {
        let engine = Server::new(&mut Host, 0);
        Ok((
            Self { engine },
            Registration::new()
                .on_tick()
                .on_blocks()
                .command_with(
                    "ntmlogic",
                    [Argument::word("what").optional(), Argument::word("wid").optional()],
                )
                .command_with("ntmbomba", [Argument::text("args").optional()]),
        ))
    }

    fn tick(&mut self, tick: u64) {
        let online = players::online().iter().map(info).collect();
        self.engine.tick(&mut Host, tick, online);
    }

    fn block_placed(&mut self, player: &Player, pos: BlockPos, state: &str) {
        self.engine
            .block_placed(&player.dimension, [pos.x, pos.y, pos.z], state);
    }

    fn block_broken(&mut self, player: &Player, pos: BlockPos, state: &str) {
        self.engine
            .block_broken(&player.dimension, [pos.x, pos.y, pos.z], state);
    }

    fn command(&mut self, player: &Player, name: &str, args: &Args) -> Result<Option<String>, String> {
        if name == "ntmbomba" {
            let who = info(player);
            return self
                .engine
                .bomb_command(&mut Host, Some(&who), args.string("args").unwrap_or(""))
                .map(Some);
        }
        match args.string("what").unwrap_or("status") {
            "status" => Ok(Some(self.engine.status())),
            "save" => self.engine.save_now(&mut Host).map(Some),
            "forget" => {
                let wid = args.string("wid").ok_or("/ntmlogic forget <id do mundo>")?;
                self.engine.forget(&mut Host, wid).map(Some)
            }
            other => Err(format!("/ntmlogic status|save|forget <wid> ({other}?)")),
        }
    }
}

horizon_mod_sdk::export_server_mod!(Logic);
