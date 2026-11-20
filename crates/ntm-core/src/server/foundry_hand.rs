//! The foundry's blocks by hand: the strand caster, the mold and the basin, the channel and the tank, the crucible
//! (`MachineStrandCaster`, `FoundryCastingBase`, `FoundryChannel`, `FoundryTank`, `MachineCrucible.onBlockActivated`). A mold in
//! hand goes into the empty mold slot, a shovel takes the molten metal as exact scraps (D4), the cast of a mold or a basin comes
//! out by a click. Like the sawmill's uses, the model and the player's stacks change in one transfer
//! (`commit_player_transfer`): a refusal moves nothing.
use super::*;
use crate::foundry::{Tables, scrap};
use crate::machine::Data;
use crate::strand_caster::{slot, stack_limit};
use container_bridge::Stack;

/// `ItemTool.getToolClasses(...).contains("shovel")`: the NTM's tools of that class (`data/tool_shovels.json`, from
/// `ItemToolAbility` SHOVEL and MINER) and the vanilla shovels.
pub fn is_shovel(item: &str) -> bool {
    static NTM: std::sync::OnceLock<std::collections::HashSet<String>> = std::sync::OnceLock::new();
    NTM.get_or_init(|| {
        #[derive(serde::Deserialize)]
        struct File {
            shovels: Vec<String>,
        }
        serde_json::from_str::<File>(include_str!("../../data/tool_shovels.json")).expect("tool_shovels.json").shovels.into_iter().collect()
    })
    .contains(item)
        || item.strip_prefix("minecraft:").is_some_and(|i| i.ends_with("_shovel"))
}

/// What a use changes: the model after it, what goes to the player, what is dropped.
struct Use {
    edits: Vec<SlotEdit>,
    drops: Vec<(Stack, TransferDrop)>,
    copies: Vec<SlotCopy>,
    sound: bool,
}

impl Server {
    /// A player used cell `at` of the foundry machine whose core is `core` (not sneaking). True when the machine takes the use;
    /// false when none applies, so the click goes on (the screen opens). Only the main hand acts (the Java reads
    /// `getHeldItem`, and 1.7.10 has no other). The slots read are the block entity's now.
    ///
    /// - strand caster: a mold in hand into the empty mold slot (any size), else a shovel: the whole pool as one exact scrap;
    /// - mold and basin: first the cast out of its slot (to the inventory), else a mold of the block's size into the empty
    ///   mold slot, else a shovel: the pool as one scrap;
    /// - channel and tank: a shovel takes the pool as one scrap (the Java takes the click even when it is empty);
    /// - crucible: a shovel takes each of its stacks, the recipe's and the waste, as one scrap each.
    ///
    /// Deviations: what does not fit is thrown from the eyes, as the sawmill's, not spawned where the Java spawns it, and only
    /// what did not fit is thrown (the Java adds to the inventory or spawns, never both).
    pub fn native_foundry_use(&mut self, host: &mut impl ServerHost, dim: &str, at: Pos, core: Pos, player: &str) -> bool {
        let key = (dim.to_string(), core);
        let Some(m) = self.ready.world.machines.get(&key).cloned() else { return false };
        if !matches!(m.data, Data::StrandCaster(_) | Data::Mold(_) | Data::Basin(_) | Data::Channel(_) | Data::FoundryTank(_) | Data::Crucible(_)) {
            return false;
        }
        let Some(hand) = host.player_slots(player) else { return false };
        if matches!(hand.hand, Some(PlayerSlot::Offhand)) {
            return false;
        }
        let held = hand.held().cloned();
        let held_id = held.as_ref().map(|h| h.stack.id.clone());
        let shovel = held_id.as_deref().is_some_and(is_shovel);
        let mold = held_id.as_deref().is_some_and(|i| Tables::ntm().mold(i).is_some());
        let takes_cast = matches!(m.data, Data::Mold(_) | Data::Basin(_));
        if !shovel && !mold && !takes_cast {
            return false;
        }
        if self.ready.broken.contains_key(&key) || !self.model_current(&key, &m) {
            return true;
        }
        if !host.get_block(dim, core).is_some_and(|b| blockstate::id_of(&b) == m.block) {
            return false;
        }
        let Some(expected) = host.machine_slots(dim, core) else { return true };
        let from = hand.hand;
        let mut machine = m.clone();
        let mut u = Use { edits: Vec::new(), drops: Vec::new(), copies: Vec::new(), sound: false };
        // All the stacks of a use in one call: separate calls would each pick the same free slot.
        let give = |u: &mut Use, stacks: Vec<Stack>| {
            let max = stacks.iter().map(|s| stack_limit(&s.id)).min().unwrap_or(64);
            let (given, left) = hand.give(&stacks, max);
            u.edits.extend(given);
            u.drops.extend(left.into_iter().map(|s| (s, TransferDrop::Thrown)));
        };
        let mut taken = true;
        match &mut machine.data {
            Data::StrandCaster(c) => {
                c.slots = expected.clone();
                if mold && c.slots[slot::MOLD].is_none() {
                    insert_mold(&mut u, &hand, held.as_ref(), from, &mut c.slots[slot::MOLD], dim, core);
                } else if shovel && c.melt.amount > 0 {
                    give(&mut u, scoop(&mut c.melt));
                } else {
                    // A shovel and an empty pool: the Java takes the click and does nothing.
                    taken = false;
                    if shovel {
                        return true;
                    }
                }
            }
            Data::Mold(c) | Data::Basin(c) => {
                c.slots = expected.clone();
                if let Some(out) = c.slots[crate::foundry::cast::OUT].take() {
                    give(&mut u, vec![out]);
                } else if mold && c.slots[crate::foundry::cast::MOLD].is_none() && held_id.as_deref().and_then(|i| Tables::ntm().mold(i)).is_some_and(|mo| mo.size == c.size) {
                    insert_mold(&mut u, &hand, held.as_ref(), from, &mut c.slots[crate::foundry::cast::MOLD], dim, core);
                } else if shovel && c.melt.amount > 0 {
                    give(&mut u, scoop(&mut c.melt));
                } else {
                    taken = false;
                    if shovel {
                        return true;
                    }
                }
            }
            Data::Channel(c) => {
                if !shovel {
                    return false;
                }
                if c.melt.amount == 0 {
                    return true;
                }
                give(&mut u, scoop(&mut c.melt));
            }
            Data::FoundryTank(c) => {
                if !shovel {
                    return false;
                }
                if c.melt.amount == 0 {
                    return true;
                }
                give(&mut u, scoop(&mut c.melt));
            }
            Data::Crucible(c) => {
                if !shovel {
                    return false;
                }
                c.slots = expected.clone();
                // Each stack of the recipe, then the waste, as one scrap (`MachineCrucible.onBlockActivated`:65-78).
                let stacks: Vec<Stack> = scrap::stacks(c.recipe_stack.iter().chain(&c.waste_stack));
                c.recipe_stack.clear();
                c.waste_stack.clear();
                give(&mut u, stacks);
            }
            _ => return false,
        }
        if !taken {
            return false;
        }
        let (desired, data) = Self::machine_output(&mut machine, &expected);
        let json = serde_json::to_string(&machine).expect("machine serialization");
        let commit = MachineCommit { cells: Vec::new(), guard: CellGuard::machine(&key, &m), expected_saved: self.ready.native_saved.contains_key(&key), dim: dim.into(), pos: core, expected, desired, data, json };
        let transfer = PlayerTransfer { player: player.into(), edits: u.edits, drops: u.drops, copies: u.copies };
        if !host.commit_player_transfer(&transfer, std::slice::from_ref(&commit)) {
            return true;
        }
        self.ready.world.machines.insert(key.clone(), machine);
        self.ready.bridge.last.insert(key.clone(), commit.desired);
        self.ready.bridge.data.insert(key.clone(), commit.data);
        self.ready.native_saved.insert(key, commit.json);
        if u.sound {
            host.show(dim, &crate::blast::visuals::Show::Sound { id: "ntm:item.upgradeplug", source: "block", at: at.map(|v| f64::from(v) + 0.5), volume: 1.0, pitch: 1.0 });
        }
        true
    }
}

/// The whole pool as one exact scrap, the pool emptied (`ItemScraps.create(new MaterialStack(type, amount))`).
fn scoop(melt: &mut crate::foundry::flow::Melt) -> Vec<Stack> {
    let out = melt.mat.as_deref().map(|m| scrap::stack(m, melt.amount)).into_iter().collect();
    *melt = Default::default();
    out
}

/// One of the stack in hand, with its data, into the empty mold slot.
fn insert_mold(u: &mut Use, hand: &PlayerSlots, held: Option<&HeldStack>, from: Option<PlayerSlot>, slot: &mut Option<Stack>, dim: &str, core: Pos) {
    let (Some(h), Some(f)) = (held, from) else { return };
    *slot = Some(Stack { count: 1, ..h.stack.clone() });
    u.edits.extend(hand.take(f, 1));
    u.copies.push(SlotCopy { dim: dim.into(), pos: core, slot: crate::strand_caster::slot::MOLD, from: f });
    u.sound = true;
}
