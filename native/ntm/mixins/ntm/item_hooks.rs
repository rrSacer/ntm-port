// Native adapter shared by consumables, equipment and stack state.
use crate::game::{
    Game,
    native_player_transactions::{
        NativeDrop, NativeDropMode, NativePlayerBenefits, NativePlayerSnapshot,
    },
};
use std::sync::Mutex;

use horizon_client::items::ItemStack;
use horizon_items::item_nbt::{item_from_nbt, item_to_nbt};
use horizon_world::{
    attachments::{AttachmentCodec, AttachmentError, AttachmentType},
    nbt::{Compound, JavaString, Tag},
};
use ntm_core::{
    item_hooks::{self, Animation, Hazard},
    radiation::Dose,
};

/// `HbmPotion.potionsickness`, the effect the herbal pill applies. Its curative
/// items are `setCurativeItems(empty)`: milk does not cure it, but the engine's
/// milk clears every effect. `herbal` in the attachment remembers how long it
/// still has to run, and the tick puts it back after anything but the antidote
/// (or its own expiry) removed it.
const SICKNESS: &str = "ntm:potion_sickness";

/// `HbmPotion.radaway`: an ordinary, curable potion in the Java (milk and the
/// antidote end it). The `radaway` counter in the attachment is the treatment;
/// this effect is its flag, so whatever clears the effect list ends the treatment.
const RADAWAY: &str = "ntm:radaway";

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn empty() -> Compound {
    Compound::new()
}
fn codec(value: &Compound) -> Result<Compound, AttachmentError> {
    if !value.is_empty() && value.get(&JavaString::from("schema")) != Some(&Tag::Int(1)) {
        return Err(AttachmentError::Decode);
    }
    for key in ["dose", "radaway", "sickness", "herbal"] {
        if let Some(value) = value.get(&JavaString::from(key)) {
            match value {
                Tag::Long(n) if *n >= 0 => {}
                _ => return Err(AttachmentError::Decode),
            }
        }
    }
    Ok(value.clone())
}
static STATE: AttachmentType<Compound> = AttachmentType {
    namespace: "ntm",
    name: "item_state",
    default: empty,
    codec: Some(AttachmentCodec {
        encode: codec,
        decode: codec,
    }),
    copy_on_death: false,
    sync: false,
};
fn number(c: &Compound, key: &str) -> u64 {
    match c.get(&JavaString::from(key)) {
        Some(Tag::Long(n)) => u64::try_from(*n).unwrap_or(0),
        _ => 0,
    }
}
fn set(c: &mut Compound, key: &str, n: u64) {
    c.insert(key.into(), Tag::Long(n.min(i64::MAX as u64) as i64));
}
fn init(s: &mut NativePlayerSnapshot, legacy: Dose) {
    if s.state.is_empty() {
        s.state.insert("schema".into(), Tag::Int(1));
        set(&mut s.state, "dose", legacy.millirad);
    }
}
fn held(s: &mut NativePlayerSnapshot, slot: usize) -> Option<&mut Option<ItemStack>> {
    if slot == crate::game::shield::OFFHAND_SLOT {
        Some(&mut s.offhand)
    } else {
        s.inventory.get_mut(slot)
    }
}
/// `InventoryPlayer.addItemStackToInventory` on a cloned inventory: merge into
/// matching stacks, then the first empty slot. `reserved` is a slot a used-up
/// stack still occupies while the item runs, so it is not empty. Returns what
/// did not fit; in creative mode the leftover vanishes, as in Java.
fn add_stack(
    game: &Game<'_>,
    slots: &mut [Option<ItemStack>],
    mut stack: ItemStack,
    reserved: Option<usize>,
) -> Option<ItemStack> {
    let max = game.items().get(&stack.item)?.max_stack;
    for s in slots.iter_mut().flatten() {
        if s.same_item_same_components(&stack) {
            let n = stack.count.min(max.saturating_sub(s.count));
            s.count += n;
            stack.count -= n;
        }
    }
    for (i, s) in slots.iter_mut().enumerate() {
        if s.is_none() && Some(i) != reserved && stack.count > 0 {
            let n = stack.count.min(max);
            let mut part = stack.clone();
            part.count = n;
            *s = Some(part);
            stack.count -= n;
        }
    }
    (stack.count > 0 && !game.creative).then_some(stack)
}
fn legacy(game: &Game<'_>) -> Option<Dose> {
    crate::game::ntm_glue::item_dose(game.native_actor_uuid().as_deref()?)
}

pub(super) fn duration(stack: &ItemStack, value: &mut i32) {
    if let Some(spec) = item_hooks::use_spec(&stack.item) {
        *value = spec.ticks;
    }
}
pub(super) fn animation(stack: &ItemStack, value: &mut horizon_world::consumable::UseAnimation) {
    use horizon_world::consumable::UseAnimation as U;
    if let Some(spec) = item_hooks::use_spec(&stack.item) {
        *value = match spec.animation {
            Animation::Eat => U::Eat,
            Animation::Drink => U::Drink,
            Animation::Bow => U::Bow,
            Animation::None => U::None,
        };
    }
}
/// `VersatileConfig.hasPotionSickness`: `isPotionActive(HbmPotion.potionsickness)`.
fn sick(game: &Game<'_>) -> bool {
    game.potions.effects.get("ntm:potion_sickness").is_some()
}

/// `onItemRightClick`'s condition for `setItemInUse`: `canEat`, the canteen's
/// cooldown, the bottle opener. A client runs the same test to predict the
/// pose; only the server edits items.
pub(super) fn start(game: &mut Game<'_>, stack: &ItemStack, canceled: &mut bool) {
    let Some(spec) = item_hooks::use_spec(&stack.item) else {
        return;
    };
    let food = game.player.as_ref().map_or(20, |p| p.health.food.food);
    let has_required = spec.requires.as_deref().is_none_or(|id| {
        game.inventory
            .iter()
            .chain(game.armor.iter())
            .flatten()
            .any(|s| &*s.item == id)
    });
    if !spec.may_start(game.creative, food, stack.damage, has_required, sick(game)) {
        *canceled = true;
        // `addChatComponentMessage`, on the server only.
        if let Some(message) = &spec.message
            && !game.native_window()
        {
            tell(game, message);
        }
        return;
    }
    // Prediction only starts a pose. Inventory/effects commit on the server.
    if game.native_window() {
        return;
    }
    if game.native_player_snapshot(&STATE).is_none() {
        *canceled = true;
    }
}
pub(super) fn use_air(game: &mut Game<'_>, stack: &ItemStack) -> bool {
    if let Some(spec) = item_hooks::use_spec(&stack.item) {
        if spec.ticks > 0 {
            return false;
        }
        complete(game, stack);
        return true;
    }
    if item_hooks::is_filter(&stack.item) {
        install_filter(game, stack);
        return true;
    }
    if instrument(game, stack) {
        return true;
    }
    if item_hooks::is_gas_mask(&stack.item) {
        return remove_filter(game, stack);
    }
    false
}

/// The hand instruments used in the air (`onItemRightClick`): the Geiger
/// counter, the dosimeter and the digamma diagnostic print their reports, the
/// survey scanner scans, the pipettes step their capacity. A client only
/// claims the click.
fn instrument(game: &mut Game<'_>, stack: &ItemStack) -> bool {
    use ntm_core::instruments as ins;
    let id: &str = &stack.item;
    if !matches!(
        id,
        "ntm:geiger_counter" | "ntm:dosimeter" | "ntm:digamma_diagnostic" | "ntm:survey_scanner" | "ntm:pipette" | "ntm:pipette_boron" | "ntm:pipette_laboratory"
    ) {
        return false;
    }
    if game.native_player_snapshot(&STATE).is_none() {
        return true;
    }
    let say = |game: &mut Game<'_>, line: &str| tell(game, line);
    match id {
        "ntm:geiger_counter" | "ntm:dosimeter" => {
            play(game, "hbm:item.techBoop");
            let buffer = rad_buffer(game);
            let lines = if id == "ntm:dosimeter" {
                ins::dosimeter_lines(buffer)
            } else {
                let seed = legacy(game).unwrap_or_default();
                let dose = read_dose(game, &seed).map_or(0, |d| d.millirad);
                let (zone, resistance) = (zone_rate(game), player_resistance(game));
                ins::geiger_lines(&ins::Readings { dose_millirad: dose, zone_millirad_s: zone, buffer_millirad_s: buffer, resistance })
            };
            lines.iter().for_each(|l| say(game, l));
        }
        "ntm:digamma_diagnostic" => {
            play(game, "hbm:item.techBoop");
            // HbmLivingProps.getDigamma: the port has no digamma, so it is 0.
            ins::diagnostic_lines(0.0).iter().for_each(|l| say(game, l));
        }
        "ntm:survey_scanner" => survey(game, say),
        _ => pipette(game, stack, say),
    }
    true
}

/// A chat line to the acting player, as a packet (`addChatMessage`): it reaches
/// a LAN guest's client, which `native_feedback` (the command capture) does not.
fn tell(game: &mut Game<'_>, line: &str) {
    if let Some(id) = game
        .native_actor_uuid()
        .and_then(|u| u128::from_str_radix(&u.replace('-', ""), 16).ok())
    {
        game.native_tell(id, horizon_client::text::Text::legacy(line));
    }
}

/// `radBuf` in milliRAD/s: the radiation server's reading of the last second.
fn rad_buffer(game: &Game<'_>) -> i64 {
    game.native_actor_uuid().map_or(0, |u| crate::game::ntm_glue::rad_buffer(&u))
}

fn zone_rate(game: &Game<'_>) -> u32 {
    match (game.native_actor_uuid(), game.player.as_ref()) {
        (Some(_), Some(p)) => crate::game::ntm_glue::zone_rate(game.dim.id(), [p.position.x, p.position.y, p.position.z]),
        _ => 0,
    }
}

/// `ItemSurveyScanner.onItemRightClick`: 11 x 11 columns five blocks apart,
/// every second block from `y + 15` down, for the ores the port has. The
/// bedrock-oil particle markers and the bedrock-ore tile are not ported.
fn survey(game: &mut Game<'_>, say: impl Fn(&mut Game<'_>, &str)) {
    let Some(p) = game.player.as_ref() else {
        return;
    };
    let [x, y, z] = [p.position.x.floor() as i32, p.position.y.floor() as i32, p.position.z.floor() as i32];
    let dim = game.dim.id().to_string();
    let (mut oil, mut bedrock_oil, mut coltan, mut depth, mut schist, mut australium) = (false, false, false, false, false, false);
    for a in -5..=5 {
        for b in -5..=5 {
            let mut i = y + 15;
            while i > 1 {
                if let Some(block) = game.native_loaded_block(&dim, [x + a * 5, i, z + b * 5]) {
                    match block.split('[').next().unwrap_or("") {
                        "ntm:ore_oil" => oil = true,
                        "ntm:ore_coltan" => coltan = true,
                        "ntm:stone_depth" | "ntm:stone_depth_nether" => depth = true,
                        "ntm:stone_gneiss" => schist = true,
                        "ntm:ore_australium" => australium = true,
                        "ntm:ore_bedrock_oil" => {
                            bedrock_oil = true;
                            break;
                        }
                        _ => {}
                    }
                }
                i -= 2;
            }
        }
    }
    for line in ntm_core::instruments::survey_messages(oil, bedrock_oil, coltan, depth, schist, australium) {
        say(game, line);
    }
}

/// `ItemPipette.onItemRightClick`: an empty pipette steps its capacity; a
/// full one says so. The `type`, `fill` and `capacity` shorts live in the
/// stack's custom data, as `initNBT` writes them.
fn pipette(game: &mut Game<'_>, stack: &ItemStack, say: impl Fn(&mut Game<'_>, &str)) {
    use ntm_core::instruments as ins;
    let short = |data: &Compound, key: &str| match data.get(&JavaString::from(key)) {
        Some(Tag::Short(n)) => Some(i32::from(*n)),
        _ => None,
    };
    let data = custom(stack).unwrap_or_default();
    let max = ins::pipette_max(&stack.item);
    let fill = short(&data, "fill").unwrap_or(0);
    if fill != 0 {
        say(game, "§ePipette not empty!");
        return;
    }
    let capacity = short(&data, "capacity").unwrap_or(max);
    let sneaking = game.keys.raw(&winit::keyboard::KeyCode::ShiftLeft);
    let next = ins::pipette_capacity(&stack.item, capacity, sneaking);
    let slot = game.held_slot();
    let done = patch_stack(game, slot, stack, |d| {
        d.insert("type".into(), Tag::Short(0));
        d.insert("fill".into(), Tag::Short(0));
        d.insert("capacity".into(), Tag::Short(next as i16));
        true
    });
    if done {
        say(game, &format!("{next}/{max}mB"));
    }
}

/// A held item used on a block, `onItemUse`: the match and the fertilizer.
/// Only the server edits stacks and blocks; a client claims the click so the
/// item is not placed or used as its template. The stack is checked against
/// the snapshot before the world is touched, and the match is spent in the
/// commit that comes before its fire, so a refused commit leaves both alone.
pub(super) fn use_on_block(
    game: &mut Game<'_>,
    stack: &ItemStack,
    pos: [i32; 3],
    face: [i32; 3],
) -> bool {
    if !matches!(&*stack.item, "ntm:matchstick" | "ntm:powder_fertilizer") {
        return false;
    }
    let Some(expected) = game.native_player_snapshot(&STATE) else {
        return true;
    };
    let creative = game.creative;
    let dim = game.dim.id().to_string();
    let held_slot = game.held_slot();
    let mut desired = expected.clone();
    match held(&mut desired, held_slot) {
        Some(slot) if slot.as_ref() == Some(stack) => {
            if !creative {
                let mut rest = stack.clone();
                rest.count -= 1;
                *slot = (rest.count > 0).then_some(rest);
            }
        }
        _ => return true,
    }
    if &*stack.item == "ntm:matchstick" {
        // ItemMatch: the cell beside the clicked face; fire if it is air, and
        // the match goes either way (not in creative).
        if !creative && !game.native_commit_player(&STATE, &expected, desired, Default::default()) {
            return true;
        }
        let at = [pos[0] + face[0], pos[1] + face[1], pos[2] + face[2]];
        if game
            .native_loaded_block(&dim, at)
            .is_some_and(|b| matches!(b.as_str(), "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"))
        {
            let _ = game.native_set_blocks(&dim, &[(at, "minecraft:fire".to_string())]);
            let pitch = 0.8 + random(game) * 0.4;
            if let Some(source) = horizon_client::settings::SOUND_SOURCES.iter().position(|s| *s == "block") {
                let center = [f64::from(at[0]) + 0.5, f64::from(at[1]) + 0.5, f64::from(at[2]) + 0.5];
                game.with_level(|_, level, _| {
                    level.native_play_sound("minecraft:item.flintandsteel.use", center, source, 1.0, pitch)
                });
            }
        }
        return true;
    }
    // ItemFertilizer: `fertilize` on the 3x3x3 around the click; it is spent
    // only if some cell was a growable target. Outside creative the stack is
    // spent first, in a confirmed commit, and only then may the world change; if
    // nothing was growable the stack is given back the same way. So a refused
    // commit never leaves a free growth behind.
    let silent = || NativePlayerBenefits {
        silent_equipment: true,
        ..Default::default()
    };
    if !creative && !game.native_commit_player(&STATE, &expected, desired, silent()) {
        return true;
    }
    let mut grew = false;
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                let cell = [pos[0] + dx, pos[1] + dy, pos[2] + dz];
                if fertilize(game, &dim, cell, dx == 0 && dy == 0 && dz == 0) {
                    grew = true;
                    // `world.playAuxSFX(2005, ...)`: the growth particles.
                    game.level_event_at(1505, cell, 15);
                }
            }
        }
    }
    if !grew
        && !creative
        && let Some(spent) = game.native_player_snapshot(&STATE)
    {
        let mut back = spent.clone();
        if let Some(slot) = held(&mut back, held_slot) {
            *slot = Some(stack.clone());
            if !game.native_commit_player(&STATE, &spent, back, silent()) {
                eprintln!("ntm item: fertilizer refund refused");
            }
        }
    }
    true
}

/// `ItemFertilizer.fertilize` on one cell: a growable block is grown when
/// `force` (the clicked cell) or its own success roll allows it, and either way
/// it counts as fertilized. The engine's `growCrop` makes that roll itself and
/// has no forced form, so the clicked cell repeats it, for the plants whose
/// roll can fail (saplings, azaleas, propagules, mushrooms and fungi), until
/// the block changes: the forced growth Java does, within a bounded number of
/// rolls. Everything else is answered by a single call.
fn fertilize(game: &mut Game<'_>, dim: &str, cell: [i32; 3], force: bool) -> bool {
    let at = horizon_core::position::BlockPos::new(cell[0], cell[1], cell[2]);
    let roll = |game: &mut Game<'_>| game.with_level(|b, level, ticks| b.grow_crop(level, ticks, at));
    let first = game.native_loaded_block(dim, cell);
    if !roll(game) {
        return false;
    }
    let rolls_can_fail = first.as_deref().is_some_and(|name| {
        ["sapling", "propagule", "azalea", "mushroom", "fungus"]
            .iter()
            .any(|plant| name.contains(plant))
    });
    if force && rolls_can_fail {
        for _ in 0..63 {
            if game.native_loaded_block(dim, cell) != first || !roll(game) {
                break;
            }
        }
    }
    true
}

/// ArmorGasMask/ItemModGasmask.onItemRightClick: sneaking with the mask in
/// hand takes its filter out. The filter goes to the inventory or, when there
/// is no room, to the ground. True when the click was claimed.
fn remove_filter(game: &mut Game<'_>, stack: &ItemStack) -> bool {
    // `keys` holds the logical sneak (the mapping of `key.sneak`, however it is
    // bound or toggled); the physical keys are `physical_keys`. A guest's turn
    // maps its sneaking into it, as `Game::use_hand` reads it.
    if !game.keys.raw(&winit::keyboard::KeyCode::ShiftLeft) {
        return false;
    }
    let installed = custom(stack)
        .and_then(|d| d.get(&JavaString::from("hfrFilter")).and_then(item_from_nbt));
    let Some(filter) = installed else {
        return false;
    };
    let Some(expected) = game.native_player_snapshot(&STATE) else {
        // A client only predicts the claim; the server edits the stacks.
        return true;
    };
    let mut desired = expected.clone();
    let held_slot = game.held_slot();
    let Some(Some(held)) = held(&mut desired, held_slot) else {
        return false;
    };
    if held != stack {
        return false;
    }
    let Some(mut data) = custom(held) else {
        return false;
    };
    data.remove(&JavaString::from("hfrFilter"));
    held.data_mut()
        .set_other("minecraft:custom_data", Some(Tag::Compound(data)));
    // `dropPlayerItemWithRandomChoice(filter, true)`: 1.7.10 ignores the flag.
    let drops: Vec<_> = add_stack(game, &mut desired.inventory, filter, None)
        .into_iter()
        .map(|d| NativeDrop::new(d, NativeDropMode::Thrown))
        .collect();
    game.native_commit_player(
        &STATE,
        &expected,
        desired,
        NativePlayerBenefits {
            drops,
            silent_equipment: true,
            ..Default::default()
        },
    );
    true
}

pub(super) fn complete(game: &mut Game<'_>, stack: &ItemStack) -> bool {
    if game
        .player
        .as_ref()
        .is_none_or(|p| p.health.dead_or_dying())
    {
        return false;
    }
    let Some(spec) = item_hooks::use_spec(&stack.item) else {
        return false;
    };
    let Some(expected) = game.native_player_snapshot(&STATE) else {
        return false;
    };
    let creative = game.creative;
    if spec.sickness_gate && sick(game) {
        return false;
    }
    // `iv_xp_empty` only fills when the player has the points.
    if spec.xp_cost > 0 && xp_total(&game.experience) < spec.xp_cost {
        return false;
    }
    let mut desired = expected.clone();
    // Only what touches the dose or the sickness timer needs the dose seed of
    // the legacy mirror (the server exists after its first tick); food, effects
    // and equipment do not wait for it.
    let dose = spec
        .dose
        .as_ref()
        .filter(|d| d.creative || !creative)
        .map(|d| d.millirad);
    let sick_ticks = spec
        .effects
        .iter()
        .find(|e| e.id == SICKNESS)
        .map(|e| e.ticks.max(0) as u64);
    if spec.radaway > 0 || spec.cooldown > 0 || dose.is_some() || sick_ticks.is_some() {
        let Some(seed) = legacy(game) else {
            return false;
        };
        init(&mut desired, seed);
    }
    if spec.cooldown > 0 && number(&desired.state, "sickness") > 0 {
        return false;
    }
    let held_slot = game.held_slot();
    let mut drops = Vec::new();
    let Some(slot) = held(&mut desired, held_slot) else {
        return false;
    };
    if slot.as_ref() != Some(stack) {
        return false;
    }
    let mut emptied = false;
    if spec.consumes(creative) {
        let mut rest = stack.clone();
        rest.count -= 1;
        *slot = (rest.count > 0).then_some(rest);
        emptied = slot.is_none();
    } else if spec.consume == item_hooks::Consume::SetDamage
        && let Some(s) = slot.as_mut()
    {
        s.damage = spec.set_damage;
    }
    // Returned items, in the order the Java adds them. A replaced slot is the
    // used-up stack's own; otherwise the emptied slot is still taken while
    // the item runs, so it is not free for the next one.
    let mut replaced = false;
    let mut gives = spec.gives.clone();
    if spec.loot.is_some() {
        // `choose(list)` on an empty list throws in the Java and nothing happens.
        let Some(part) = spec.draw(random(game)) else {
            return false;
        };
        gives.push(item_hooks::Give {
            item: part.to_string(),
            count: 1,
            replace: false,
            overflow: item_hooks::Overflow::Lost,
            creative: true,
        });
    }
    for give in &gives {
        if creative && !give.creative {
            continue;
        }
        let item = ItemStack::new(&give.item, give.count);
        if give.replace && emptied && !replaced {
            let Some(slot) = held(&mut desired, held_slot) else {
                return false;
            };
            *slot = Some(item);
            replaced = true;
            continue;
        }
        let reserved = (emptied && !replaced && held_slot != crate::game::shield::OFFHAND_SLOT)
            .then_some(held_slot);
        if let Some(rest) = add_stack(game, &mut desired.inventory, item, reserved)
            && give.overflow == item_hooks::Overflow::Thrown
        {
            drops.push(NativeDrop::new(rest, NativeDropMode::Thrown));
        }
    }
    if let Some(tier) = spec.special.as_deref().and_then(|s| s.strip_prefix("hazmat_kit:")) {
        let suffix = match tier {
            "1" => "_red",
            "2" => "_grey",
            _ => "",
        };
        // ItemStarterKit.giveHaz: `world.spawnEntityInWorld(new EntityItem(world,
        // posX, posY + eyeHeight, posZ, armor))`. It sets no pickup delay, so the
        // field's default of 0 stands, and no thrower.
        for (index, part) in ["helmet", "plate", "legs", "boots"].into_iter().enumerate() {
            let id = format!("ntm:hazmat_{part}{suffix}");
            drops.extend(
                desired.armor[index]
                    .take()
                    .map(|d| NativeDrop::new(d, NativeDropMode::World { delay: 0 })),
            );
            desired.armor[index] = Some(ItemStack::new(&id, 1));
        }
    }
    // `clearActivePotions` takes the radaway potion and the herbal sickness
    // along; a list of `removePotionEffect` takes the ones it names.
    match &spec.cure {
        Some(item_hooks::Cure::All) => {
            for key in ["radaway", "herbal"] {
                if number(&desired.state, key) > 0 {
                    set(&mut desired.state, key, 0);
                }
            }
        }
        Some(item_hooks::Cure::Effects(ids))
            if ids.iter().any(|id| id == SICKNESS) && number(&desired.state, "herbal") > 0 =>
        {
            set(&mut desired.state, "herbal", 0);
        }
        _ => {}
    }
    if let Some(ticks) = sick_ticks {
        set(&mut desired.state, "herbal", ticks);
    }
    let mut radaway_effect = None;
    if spec.radaway > 0 {
        let remaining = number(&desired.state, "radaway").saturating_add(u64::from(spec.radaway));
        set(&mut desired.state, "radaway", remaining);
        radaway_effect = Some((RADAWAY.to_string(), remaining.min(i32::MAX as u64) as i32, 0_u8));
    }
    if let Some(millirad) = dose {
        // `incrementRadiation` clamps the dose to 0..2500 RAD.
        let now = number(&desired.state, "dose") as i64;
        let next = now.saturating_add(millirad).clamp(0, ntm_core::radiation::MAX_DOSE as i64);
        set(&mut desired.state, "dose", next as u64);
    }
    if spec.cooldown > 0 {
        set(&mut desired.state, "sickness", spec.cooldown as u64);
    }
    let benefits = NativePlayerBenefits {
        food: spec.food_stats(),
        heal: spec.heal,
        effects: spec
            .effects
            .iter()
            .map(|e| (e.id.clone(), e.ticks, e.amplifier))
            .chain(radaway_effect)
            .collect(),
        cooldown: (spec.cooldown > 0).then(|| (stack.item.to_string(), spec.cooldown)),
        drops,
        ..Default::default()
    };
    let applied = game.native_commit_player(&STATE, &expected, desired, benefits);
    if applied {
        trace(game, "use", &stack.item);
        // `contaminate` feeds `radEnv` (the next `radBuf` window) whatever its
        // gates decide; the direct `incrementRadiation` of a pill does not.
        if let (Some(d), Some(uuid)) = (&spec.dose, game.native_actor_uuid())
            && !d.creative
        {
            crate::game::ntm_glue::contaminate(&uuid, d.millirad);
        }
        if let Some(sound) = spec.sound.as_deref() {
            play(game, sound);
        }
        after_commit(game, spec);
    }
    applied
}

/// What the Java does to the player that the atomic commit has no field for:
/// removing effects, experience, direct health loss and damage. They run in
/// the same server turn as the commit, in the order the Java classes use.
fn after_commit(game: &mut Game<'_>, spec: &item_hooks::UseSpec) {
    if let Some(cure) = &spec.cure {
        match cure {
            item_hooks::Cure::All => game.potions.effects.clear(),
            item_hooks::Cure::Effects(ids) => game
                .potions
                .effects
                .list
                .retain(|e| !ids.iter().any(|id| id == e.id)),
        }
        game.apply_effect_status();
    }
    if spec.xp_cost > 0 || spec.xp_gain > 0 {
        let from = game.experience.level;
        let mut e = game.experience;
        if spec.xp_cost > 0 {
            // `EnchantmentUtil.setExperience(player, total - cost)`.
            let rest = xp_total(&e) - spec.xp_cost;
            e = horizon_client::entities::Experience::default();
            e.give(rest);
        } else {
            e.give(spec.xp_gain);
        }
        game.experience = e;
        game.native_player_level_changed(from);
    }
    if spec.hp_cost > 0.0 {
        let left = game
            .player
            .as_ref()
            .map(|p| p.health.health() - spec.hp_cost);
        match left {
            Some(left) if left > 0.0 => {
                if let Some(p) = game.player.as_mut() {
                    let _ = p.set_health(left, false);
                }
            }
            // `setHealth(0)` then `onDeath`.
            Some(_) => {
                game.hurt_player_from(1.0e6, None, "minecraft:generic_kill", Some(None));
            }
            None => {}
        }
    }
    if let Some(hurt) = &spec.hurt {
        for _ in 0..hurt.times {
            // `rand.nextInt(chance) == 0`.
            if hurt.chance > 0 && (random(game) * hurt.chance as f32) as u32 != 0 {
                continue;
            }
            game.hurt_player_from(hurt.amount, None, &hurt.damage_type, Some(None));
        }
    }
    match spec.special.as_deref() {
        Some("chocolate_milk_blast") => chocolate_milk_blast(game),
        // `world.playSoundEffect(.., "hbm:entity.vomit", 1, 1)`; the vomit
        // packet of ItemLemon carries no mode and a count of 0, so it draws nothing.
        Some("vomit") => play(game, "hbm:entity.vomit"),
        Some("smoke") => smoke(game),
        // `player.setFire(5 * 20)` takes seconds: 100 seconds; `motionY = 2`.
        Some("schnitzel_vegan") => {
            if let Some(p) = game.player.as_mut() {
                p.set_remaining_fire_ticks(100 * 20);
                p.velocity.y = 2.0;
            }
        }
        // ExplosionNukeSmall.explode(PARAMS_MEDIUM): no counterpart in the port.
        // The kit's own effect is the benefit above; its ambient flag is set here.
        Some("stealth_boy") => {
            for e in game.potions.effects.list.iter_mut().filter(|e| e.id == "invisibility") {
                e.ambient = true;
            }
        }
        Some("bomb_waffle_blast") | None => {}
        Some(s) if s.starts_with("hazmat_kit:") => {}
        Some(other) => eprintln!("ntm item: unknown special {other}"),
    }
}

/// The cigarette's `vomit` packet in `smoke` mode: 30 smoke particles at
/// `posY - yOffset + eyeHeight + 1`, drifting along the view.
fn smoke(game: &mut Game<'_>) {
    use horizon_client::particles::{ParticleOptions, ParticleType};
    let Some(p) = game.player.as_ref() else {
        return;
    };
    let at = [
        p.position.x,
        p.position.y + f64::from(p.dimensions.eye_height) + 1.0,
        p.position.z,
    ];
    let Some(kind) = ParticleType::from_name("minecraft:smoke") else {
        return;
    };
    let options = ParticleOptions::simple(kind);
    game.with_level(|_, level, _| level.native_send_particles(&options, None, at, [0.1; 3], 0.01, 30));
}

/// `EnchantmentUtil.getTotalExperience`: the points in the completed levels
/// plus those on the bar. It uses the engine's bar sizes: the port's XP is the
/// 26.3 one, and the 1.7.10 `xpBarCap` describes a different bar.
fn xp_total(e: &horizon_client::entities::Experience) -> i32 {
    use horizon_client::entities::Experience as X;
    let done: i32 = (0..e.level).map(X::needed).sum();
    done + (X::needed(e.level) as f32 * e.progress) as i32
}

/// Plays the registered original event at the player, as
/// `playSoundAtEntity` does.
fn play(game: &mut Game<'_>, java: &str) {
    let Some(sound) = ntm_core::consumables::sound(java) else {
        return;
    };
    let Some(p) = game.player.as_ref() else {
        return;
    };
    let at = [p.position.x, p.position.y, p.position.z];
    let category = ntm_core::sounds::category(sound.event).unwrap_or("player");
    let Some(source) = horizon_client::settings::SOUND_SOURCES
        .iter()
        .position(|s| *s == category)
    else {
        return;
    };
    let pitch = sound.pitch + random(game) * sound.spread;
    game.with_level(|_, level, _| level.native_play_sound(sound.event, at, source, sound.volume, pitch));
}

/// A uniform number in `[0, 1)` (`Random.nextFloat`) from a splitmix of the
/// clock and the world tick: the items need chance, not a reproducible stream.
fn random(game: &Game<'_>) -> f32 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0x9E37_79B9_7F4A_7C15);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos() as u64);
    let mut z = COUNTER
        .fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed)
        .wrapping_add(nanos ^ (game.world.game_time as u64).rotate_left(32));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 24) as f32
}

/// A chocolate-milk blast waiting for the end of the server tick: where it
/// went off, and in which level.
struct Blast {
    dim: String,
    center: [f64; 3],
}
static BLASTS: Mutex<Vec<Blast>> = Mutex::new(Vec::new());

/// `ItemEnergy.onEaten` for chocolate milk: `ExplosionVNT(world, x, y + h/2, z,
/// 50)`. The blast is only queued here: the use runs inside the drinker's turn,
/// where the other players cannot be reached, so `drain` carries it out at
/// `ServerTickEnd`, when the hub is back.
fn chocolate_milk_blast(game: &mut Game<'_>) {
    let Some(p) = game.player.as_ref() else {
        return;
    };
    let center = [
        p.position.x,
        p.position.y + f64::from(p.dimensions.height) / 2.0,
        p.position.z,
    ];
    let dim = game.dim.id().to_string();
    lock(&BLASTS).push(Blast { dim, center });
}

/// The queued blasts, at the end of the server tick. Java's order for
/// `ExplosionVNT`: `BlockAllocatorStandard(32)` reads the blocks, then
/// `EntityProcessorCross(5)` and `PlayerProcessorStandard` hurt and push every
/// living player of the level in range (measuring their exposure against the
/// blocks still standing) and `BlockProcessorStandard().setNoDrop()` removes
/// them. Mobs and dropped items are not reached (no engine API for them yet;
/// see the doc).
pub(super) fn drain(game: &mut Game<'_>) {
    let blasts = std::mem::take(&mut *lock(&BLASTS));
    for blast in blasts {
        trace(game, "blast", &format!("{} {:?}", blast.dim, blast.center));
        explode(game, &blast);
    }
}

fn explode(game: &mut Game<'_>, blast: &Blast) {
    use ntm_core::blast::vanilla;
    const SIZE: f32 = 50.0;
    let Blast { dim, center } = blast;
    let (cells, _) = vanilla::allocate_vnt(*center, SIZE, 32, &mut |b| game.native_loaded_block(dim, b));
    let players: Vec<String> = game
        .native_players()
        .into_iter()
        .filter(|q| q.alive && &q.dimension == dim)
        .map(|q| q.uuid)
        .collect();
    trace(game, "blast-allocated", &format!("{} cells, {} players", cells.len(), players.len()));
    for uuid in players {
        let visited = game.native_visit_player(&uuid, |g| cross_player(g, *center, SIZE));
        trace(game, "blast-player", &format!("{uuid} visited {}", visited.is_some()));
    }
    let air: Vec<([i32; 3], String)> = cells.iter().map(|c| (*c, "minecraft:air".to_string())).collect();
    for chunk in air.chunks(ntm_core::explosion::BLOCKS_PER_CALL) {
        if let Err(e) = game.native_set_blocks(dim, chunk) {
            eprintln!("ntm item: chocolate milk blast: {e}");
            break;
        }
    }
    trace(game, "blast-removed", &format!("{} blocks", air.len()));
    // `ExplosionCreator.composeEffectStandard` is a client particle show; the
    // emitter represents it; the sound is the literal ExplosionNukeSmall event.
    crate::game::ntm_glue::in_place(game, dim, |game| {
        if let Some(source) = horizon_client::settings::SOUND_SOURCES.iter().position(|s| *s == "block") {
            game.with_level(|_, level, _| {
                level.native_play_sound("ntm:weapon.mukeexplosion", *center, source, 15.0, 1.0)
            });
        }
        if let Some(kind) = horizon_client::particles::ParticleType::from_name("minecraft:explosion_emitter") {
            let options = horizon_client::particles::ParticleOptions::simple(kind);
            game.with_level(|_, level, _| level.native_send_particles(&options, None, *center, [0.0; 3], 0.0, 1));
        }
    });
}

/// `World.getBlockDensity(node, box)` of 1.7.10: a grid over the box, stepped
/// `1 / (2 · extent + 1)` along each axis, with one ray from every point to the
/// node; the share of rays no block box stops. Each ray is the engine's
/// collision raycast (`seen_percent` over a degenerate box is one point), so
/// slabs, stairs and fences stop it by their real shape.
fn block_density(game: &Game<'_>, node: [f64; 3], min: [f64; 3], max: [f64; 3]) -> f32 {
    let step = |a: usize| 1.0 / ((max[a] - min[a]) as f32 * 2.0 + 1.0);
    let (sx, sy, sz) = (step(0), step(1), step(2));
    let (mut seen, mut total) = (0_u32, 0_u32);
    let mut x = 0.0_f32;
    while x <= 1.0 {
        let mut y = 0.0_f32;
        while y <= 1.0 {
            let mut z = 0.0_f32;
            while z <= 1.0 {
                let at = [
                    min[0] + (max[0] - min[0]) * f64::from(x),
                    min[1] + (max[1] - min[1]) * f64::from(y),
                    min[2] + (max[2] - min[2]) * f64::from(z),
                ];
                if game.seen_percent(node, at, at) > 0.5 {
                    seen += 1;
                }
                total += 1;
                z += sz;
            }
            y += sy;
        }
        x += sx;
    }
    seen as f32 / total as f32
}

/// `EntityProcessorCross.process` for the player the game is acting as: the
/// box distance, the densest of the seven nodes, then the push and the damage.
fn cross_player(game: &mut Game<'_>, center: [f64; 3], size: f32) {
    use ntm_core::blast::vanilla;
    let Some(p) = game.player.as_ref() else {
        return;
    };
    let feet = [p.position.x, p.position.y, p.position.z];
    let (width, height) = (f64::from(p.dimensions.width), f64::from(p.dimensions.height));
    let eye = f64::from(p.dimensions.eye_height);
    let min = [feet[0] - width / 2.0, feet[1], feet[2] - width / 2.0];
    let max = [feet[0] + width / 2.0, feet[1] + height, feet[2] + width / 2.0];
    let dist = vanilla::box_distance(min, max, center);
    let delta = [feet[0] - center[0], feet[1] + eye - center[1], feet[2] - center[2]];
    let length = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
    if length == 0.0 || vanilla::cross_hit(size, dist, 1.0).is_none() {
        return;
    }
    let density = vanilla::cross_nodes(center, 5.0)
        .iter()
        .map(|node| block_density(game, *node, min, max))
        .fold(0.0_f32, f32::max);
    if let Some((damage, knockback)) = vanilla::cross_hit(size, dist, density) {
        if let Some(p) = game.player.as_mut() {
            p.velocity.x += delta[0] / length * knockback;
            p.velocity.y += delta[1] / length * knockback;
            p.velocity.z += delta[2] / length * knockback;
        }
        game.hurt_player_from(damage, Some(center), "minecraft:explosion", Some(None));
    }
}

pub(super) fn tick(game: &mut Game<'_>) {
    // `ItemBDCL.onUsingTick`: the gulps between the first sip and the groan.
    // The player tick runs after the engine advanced the use, so `used` already
    // counts this tick: the tick `onUsingTick` sees is one less.
    if let Some((slot, used)) = game.using
        && game.stack_in(slot).is_some_and(|s| &*s.item == "ntm:bdcl")
        && ntm_core::consumables::bdcl_gulp_after(used)
    {
        trace(game, "gulp", &(used - 1).to_string());
        play(game, "hbm:player.gulp");
    }
    let Some(expected) = game.native_player_snapshot(&STATE) else {
        return;
    };
    let mut desired = expected.clone();
    let Some(seed) = legacy(game) else {
        return;
    };
    init(&mut desired, seed);
    let remaining = number(&desired.state, "radaway");
    if remaining > 0 {
        if game.potions.effects.get(RADAWAY).is_none() && remaining > 2 {
            // The effect list was cleared (milk, a command) with time left:
            // the treatment ends with it. Within two ticks of the end the
            // effect simply ran out.
            set(&mut desired.state, "radaway", 0);
        } else {
            let dose = number(&desired.state, "dose").saturating_sub(1_000);
            set(&mut desired.state, "dose", dose);
            set(&mut desired.state, "radaway", remaining - 1);
        }
    }
    let sickness = number(&desired.state, "sickness");
    if sickness > 0 {
        set(&mut desired.state, "sickness", sickness - 1);
    }
    // The herbal sickness follows its effect while it runs; if a milk bucket
    // wiped the effect with time still left, it comes back for that time.
    let herbal = number(&desired.state, "herbal");
    let mut benefits = NativePlayerBenefits::default();
    if herbal > 0 {
        match game.potions.effects.get(SICKNESS) {
            Some(effect) => set(&mut desired.state, "herbal", effect.duration.max(1) as u64 - 1),
            // Within a tick of the end the effect expired by itself.
            None if herbal > 2 => {
                let left = herbal - 1;
                set(&mut desired.state, "herbal", left);
                benefits.effects.push((SICKNESS.to_string(), left as i32, 0));
            }
            None => set(&mut desired.state, "herbal", 0),
        }
    }
    // `ItemCanteen.onUpdate`: every second its damage (the cooldown) drops by
    // one, for each canteen in the main inventory.
    if game.world.game_time % 20 == 0 {
        for s in desired.inventory.iter_mut().flatten() {
            if s.damage > 0
                && item_hooks::use_spec(&s.item)
                    .is_some_and(|u| u.consume == item_hooks::Consume::SetDamage)
            {
                s.damage -= 1;
            }
        }
    }
    // `ItemHot.onUpdate`: one less heat per tick, then the tag is gone.
    for s in desired.inventory.iter_mut().flatten() {
        if ntm_core::instruments::is_hot(&s.item)
            && let Some(Tag::Compound(mut data)) = s.data().and_then(|d| d.other("minecraft:custom_data")).cloned()
        {
            let heat = match data.get(&JavaString::from("heat")) {
                Some(Tag::Int(h)) => *h,
                _ => 0,
            };
            match ntm_core::instruments::cool(heat) {
                Some(h) => {
                    data.insert("heat".into(), Tag::Int(h));
                    s.data_mut().set_other("minecraft:custom_data", Some(Tag::Compound(data)));
                }
                None => s.data_mut().set_other("minecraft:custom_data", None),
            }
        }
    }
    if desired != expected {
        game.native_commit_player(&STATE, &expected, desired, benefits);
    }
    clicks(game);
}

/// `ItemGeigerCounter.onUpdate` and `ItemDosimeter.onUpdate` for each one in
/// the main inventory, every fifth tick: a click by the radiation buffer.
fn clicks(game: &mut Game<'_>) {
    use ntm_core::instruments as ins;
    if game.world.game_time % 5 != 0 {
        return;
    }
    let found: Vec<bool> = game
        .inventory
        .iter()
        .flatten()
        .filter_map(|s| match &*s.item {
            "ntm:geiger_counter" => Some(true),
            "ntm:dosimeter" => Some(false),
            _ => None,
        })
        .collect();
    if found.is_empty() {
        return;
    }
    let x = rad_buffer(game) as f32 / 1000.0;
    for geiger in found {
        let (pick, idle) = (random(game), random(game));
        let click = if geiger {
            ins::click(ins::geiger_pool, 50, x, pick, idle)
        } else {
            ins::click(ins::dosimeter_pool, 100, x, pick, idle)
        };
        if let Some(r) = click {
            play(game, &format!("hbm:item.geiger{r}"));
        }
    }
}

pub(super) fn read_dose(game: &mut Game<'_>, old: &Dose) -> Option<Dose> {
    let mut snap = game.native_player_snapshot(&STATE)?;
    init(&mut snap, old.clone());
    Some(Dose {
        millirad: number(&snap.state, "dose"),
    })
}
pub(super) fn write_dose(game: &mut Game<'_>, before: &Dose, after: &Dose) -> bool {
    let Some(expected) = game.native_player_snapshot(&STATE) else {
        return false;
    };
    let mut desired = expected.clone();
    init(&mut desired, before.clone());
    if number(&desired.state, "dose") != before.millirad {
        return false;
    }
    set(&mut desired.state, "dose", after.millirad);
    game.native_commit_player(&STATE, &expected, desired, Default::default())
}
/// The acting player's stacks with their state, for a transfer with machines (`ServerHost::player_slots`). None: no
/// acting player, or a state the codec refuses (then no transfer moves anything).
pub(super) fn transfer_snapshot(game: &mut Game<'_>) -> Option<NativePlayerSnapshot> {
    game.native_player_snapshot(&STATE)
}
/// The player's side of a transfer, published only as a whole (`native_commit_player`): the stacks compared with
/// `expected` first, every component and the state included; the hand's count changes in place, as `stackSize--`
/// does, so no equipment event replays. False changes nothing.
pub(super) fn transfer_commit(
    game: &mut Game<'_>,
    expected: &NativePlayerSnapshot,
    desired: NativePlayerSnapshot,
    drops: Vec<NativeDrop>,
) -> bool {
    let benefits = NativePlayerBenefits { drops, silent_equipment: true, ..Default::default() };
    game.native_commit_player(&STATE, expected, desired, benefits)
}
/// `HazmatRegistry.getResistance(EntityPlayer)`: the four armor pieces with
/// their cladding, plus 0.2 while Rad-X is active.
pub(super) fn player_resistance(game: &Game<'_>) -> f32 {
    // `armorInventory[0..3]` is boots to helmet, and the float narrows after each piece: the order counts.
    item_hooks::player_resistance(
        game.armor.iter().rev().flatten().map(worn_resistance),
        game.potions.effects.get("ntm:radx").is_some(),
    )
}
/// `ContaminationUtil.calculateRadiationMod` for the acting player.
pub(super) fn radiation_factor(game: &Game<'_>) -> f64 {
    f64::from(item_hooks::radiation_mod(player_resistance(game)))
}
/// `HazmatRegistry.getResistance(ItemStack)` of a worn stack: the
/// `hfr_cladding` number (`getFloat`, any numeric tag) and
/// the item in the cladding slot of `ntm_armor_mods`.
pub(super) fn worn_resistance(stack: &ItemStack) -> f64 {
    let data = match stack.data().and_then(|d| d.other("minecraft:custom_data")) {
        Some(Tag::Compound(c)) => Some(c),
        _ => None,
    };
    // NBTTagCompound.getFloat: any numeric tag (`hasKey(key, 99)`), as float; 0 for anything else.
    let hfr_cladding = match data.and_then(|c| c.get(&JavaString::from("hfr_cladding"))) {
        Some(Tag::Byte(n)) => f32::from(*n),
        Some(Tag::Short(n)) => f32::from(*n),
        Some(Tag::Int(n)) => *n as f32,
        Some(Tag::Long(n)) => *n as f32,
        Some(Tag::Float(bits)) => f32::from_bits(*bits),
        Some(Tag::Double(bits)) => f64::from_bits(*bits) as f32,
        _ => 0.0,
    };
    let slot = JavaString::from(format!("mod_slot_{}", item_hooks::CLADDING_SLOT).as_str());
    let cladding_mod = data
        .and_then(|c| match c.get(&JavaString::from("ntm_armor_mods")) {
            Some(Tag::Compound(mods)) => mods.get(&slot),
            _ => None,
        })
        .and_then(item_from_nbt);
    item_hooks::piece_resistance(&item_hooks::Piece {
        id: &stack.item,
        hfr_cladding,
        cladding_mod: cladding_mod.as_ref().map(|m| m.item.as_ref()),
    })
}

/// Atomic stack custom-data edit. The caller supplies the complete expected
/// stack; every unrelated component and custom field survives the patch.
pub(super) fn patch_stack(
    game: &mut Game<'_>,
    slot: usize,
    expected_stack: &ItemStack,
    patch: impl FnOnce(&mut Compound) -> bool,
) -> bool {
    let Some(expected) = game.native_player_snapshot(&STATE) else {
        return false;
    };
    let mut desired = expected.clone();
    let Some(Some(stack)) = held(&mut desired, slot) else {
        return false;
    };
    if stack != expected_stack {
        return false;
    }
    let mut data = match stack.data().and_then(|d| d.other("minecraft:custom_data")) {
        None => Compound::new(),
        Some(Tag::Compound(c)) => c.clone(),
        Some(_) => return false,
    };
    if !patch(&mut data) {
        return false;
    }
    stack
        .data_mut()
        .set_other("minecraft:custom_data", Some(Tag::Compound(data)));
    game.native_commit_player(&STATE, &expected, desired, Default::default())
}

fn install_filter(game: &mut Game<'_>, filter: &ItemStack) -> bool {
    let Some(expected) = game.native_player_snapshot(&STATE) else {
        return false;
    };
    let mut desired = expected.clone();
    let Some(helmet) = desired.armor[0].as_mut() else {
        return false;
    };
    let Some(mut mask) = mask_of(helmet) else {
        return false;
    };
    let mut data = match mask.data().and_then(|d| d.other("minecraft:custom_data")) {
        None => Compound::new(),
        Some(Tag::Compound(c)) => c.clone(),
        _ => return false,
    };
    let previous = match data.get(&JavaString::from("hfrFilter")) {
        None => None,
        Some(tag) => {
            let Some(stack) = item_from_nbt(tag) else {
                return false;
            };
            Some(stack)
        }
    };
    data.insert("hfrFilter".into(), item_to_nbt(filter));
    mask.data_mut()
        .set_other("minecraft:custom_data", Some(Tag::Compound(data)));
    if !put_mask(helmet, mask) {
        return false;
    }
    let Some(slot) = held(&mut desired, game.held_slot()) else {
        return false;
    };
    if slot.as_ref() != Some(filter) {
        return false;
    }
    *slot = previous;
    let applied=game.native_commit_player(
        &STATE,
        &expected,
        desired,
        NativePlayerBenefits {
            silent_equipment: true,
            ..Default::default()
        },
    );
    if applied {crate::game::ntm_sounds::player(game,"hbm:item.gasmaskScrew",1.,1.);}
    applied
}

/// Exposure-specific filter wear, in the same CAS as protection state. Gamma
/// uses HazmatRegistry independently and never spends a particle filter.
/// A stack and what `getProtectionFromItem` reads of it, owned.
struct OwnedWorn {
    id: String,
    filter: Option<String>,
    mods: Vec<OwnedWorn>,
}
impl OwnedWorn {
    fn of(stack: &ItemStack) -> Self {
        let data = custom(stack).unwrap_or_default();
        let filter = data.get(&JavaString::from("hfrFilter")).and_then(item_from_nbt).map(|f| f.item.to_string());
        let mods = match data.get(&JavaString::from("ntm_armor_mods")) {
            Some(Tag::Compound(mods)) => (0..9)
                .filter_map(|i| mods.get(&JavaString::from(format!("mod_slot_{i}").as_str())).and_then(item_from_nbt))
                .map(|m| OwnedWorn::of(&m))
                .collect(),
            _ => Vec::new(),
        };
        OwnedWorn { id: stack.item.to_string(), filter, mods }
    }
    fn view(&self) -> item_hooks::Worn<'_> {
        item_hooks::Worn { id: &self.id, filter: self.filter.as_deref(), mods: self.mods.iter().map(OwnedWorn::view).collect() }
    }
}
/// A gas or particle source meeting the acting player, as the Java writes it:
/// `if(ArmorRegistry.hasProtection(entity, 3, clazz)) ArmorUtil.damageGasMaskFilter(entity, wear)`.
/// True when the helmet protects (by itself, by a filter or by a mod); the
/// filter, when there is one, is worn in the same commit and removed past
/// 20000. A helmet that protects without an `IGasMask` filter (the rag masks,
/// goggles) wears nothing.
pub(super) fn filter_exposure(game: &mut Game<'_>, hazard: Hazard, wear: u32) -> bool {
    let Some(expected) = game.native_player_snapshot(&STATE) else {
        return false;
    };
    let Some(worn) = expected.armor[0].as_ref().map(OwnedWorn::of) else {
        return false;
    };
    if !item_hooks::has_protection(Some(&worn.view()), hazard) {
        return false;
    }
    let mut desired = expected.clone();
    let Some(helmet) = desired.armor[0].as_mut() else {
        return false;
    };
    let module = mod0(helmet);
    let module_mask = module.as_ref().filter(|m| item_hooks::is_gas_mask(&m.item));
    let target = item_hooks::filter_wear(
        item_hooks::is_gas_mask(&helmet.item),
        filter_of(helmet).is_some(),
        module_mask.is_some(),
        module_mask.and_then(filter_of).is_some(),
    );
    // ArmorUtil.damageGasMaskFilter(mask): +damage, and past the filter's max
    // damage `removeFilter(mask)` on the stack it writes to.
    let worn = |mut f: ItemStack| {
        f.damage = f.damage.saturating_add(wear);
        (f.damage <= 20_000).then_some(f)
    };
    match target {
        item_hooks::FilterWear::None => return true,
        item_hooks::FilterWear::Helmet => {
            let Some(f) = filter_of(helmet) else { return false };
            if set_filter(helmet, worn(f)).is_none() {
                return false;
            }
        }
        item_hooks::FilterWear::Mod => {
            let (Some(mut m), Some(f)) = (module.clone(), module_mask.and_then(filter_of)) else { return false };
            if set_filter(&mut m, worn(f)).is_none() || set_mod0(helmet, &m).is_none() {
                return false;
            }
        }
        item_hooks::FilterWear::CopyIntoHelmet => {
            // JAVA_FILTER_COPY_BUG: the worn copy goes into the helmet, the mod keeps its filter
            // (ArmorUtil.java:335-351); a copy past its max damage is "removed" from the
            // helmet, which had none, so nothing changes.
            let Some(f) = module_mask.and_then(filter_of) else { return false };
            if set_filter(helmet, worn(f)).is_none() {
                return false;
            }
        }
    }
    game.native_commit_player(
        &STATE,
        &expected,
        desired,
        NativePlayerBenefits {
            silent_equipment: true,
            ..Default::default()
        },
    )
}
/// `ArmorUtil.getGasMaskFilter`: the `hfrFilter` stack of a mask.
fn filter_of(stack: &ItemStack) -> Option<ItemStack> {
    custom(stack)?.get(&JavaString::from("hfrFilter")).and_then(item_from_nbt)
}
/// `installGasMaskFilter` (Some) or `removeFilter` (None) on a stack.
fn set_filter(stack: &mut ItemStack, filter: Option<ItemStack>) -> Option<()> {
    let mut data = custom(stack)?;
    match filter {
        Some(f) => {
            data.insert("hfrFilter".into(), item_to_nbt(&f));
        }
        None => {
            data.remove(&JavaString::from("hfrFilter"));
        }
    }
    stack.data_mut().set_other("minecraft:custom_data", Some(Tag::Compound(data)));
    Some(())
}
/// Write the `helmet_only` mod back into an existing `ntm_armor_mods`.
fn set_mod0(helmet: &mut ItemStack, module: &ItemStack) -> Option<()> {
    let mut data = custom(helmet)?;
    let Tag::Compound(mods) = data.get_mut(&JavaString::from("ntm_armor_mods"))? else {
        return None;
    };
    mods.insert("mod_slot_0".into(), item_to_nbt(module));
    helmet.data_mut().set_other("minecraft:custom_data", Some(Tag::Compound(data)));
    Some(())
}
/// The stack in the `helmet_only` mod slot (`ArmorModHandler.helmet_only` = 0).
fn mod0(helmet: &ItemStack) -> Option<ItemStack> {
    let data = custom(helmet)?;
    let Tag::Compound(mods) = data.get(&JavaString::from("ntm_armor_mods"))? else {
        return None;
    };
    item_from_nbt(mods.get(&JavaString::from("mod_slot_0"))?)
}
fn trace(game: &Game<'_>, event: &str, item: &str) {
    if std::env::var_os("NTM_TRACE_ITEMS").is_some() {
        eprintln!(
            "ntm item {}",
            serde_json::json!({"event":event,"item":item,"player":game.native_actor_uuid()})
        );
    }
}

/// Operator diagnostics also exercise the same exposure API environmental
/// families call. No client-side inventory edits or second effect path.
pub(super) fn command(game: &mut Game<'_>, line: &str) -> bool {
    let words: Vec<_> = line.split_whitespace().collect();
    if words.first() != Some(&"ntmitem") {
        return false;
    }
    let mut reply = String::from("false");
    match words.as_slice() {
        [_, "transaction-check"] => {
            reply = transaction_check(game).to_string();
        }
        [_, "inspect"] => {
            if let Some(snapshot) = game.native_player_snapshot(&STATE) {
                reply = horizon_world::snbt_text::to_snbt(&Tag::Compound(snapshot.state));
            }
        }
        // The operator's way to end the herbal sickness, which the tick would
        // otherwise restore after an `/effect clear`: the remembered time and the
        // effect go together.
        [_, "cure"] => {
            if let Some(expected) = game.native_player_snapshot(&STATE) {
                let mut desired = expected.clone();
                if number(&desired.state, "herbal") > 0 {
                    set(&mut desired.state, "herbal", 0);
                }
                let done = desired == expected
                    || game.native_commit_player(&STATE, &expected, desired, Default::default());
                if done {
                    game.potions.effects.list.retain(|e| e.id != SICKNESS);
                    game.apply_effect_status();
                }
                reply = done.to_string();
            }
        }
        // The reading the instruments use (`radBuf`, milliRAD/s).
        [_, "radbuf"] => reply = rad_buffer(game).to_string(),
        [_, "lens"] => {
            if let Some(line) = game.held().and_then(lens_tooltip) {
                reply = line;
            }
        }
        [_, "dose", value] => {
            if let Ok(value) = value.parse::<u64>()
                && value <= ntm_core::radiation::MAX_DOSE
                && let Some(seed) = legacy(game)
                && let Some(before) = read_dose(game, &seed)
            {
                reply = write_dose(game, &before, &Dose { millirad: value }).to_string();
            }
        }
        [_, "expose", kind, wear] => {
            if let Ok(wear) = wear.parse::<u32>() {
                let hazard = match *kind {
                    "fine" => Some(Hazard::FineParticles),
                    "lung" => Some(Hazard::LungGas),
                    "coarse" => Some(Hazard::CoarseParticles),
                    "blister" => Some(Hazard::BlisteringGas),
                    "bacteria" => Some(Hazard::Bacteria),
                    "gamma" => Some(Hazard::Gamma),
                    "co" => Some(Hazard::CarbonMonoxide),
                    "inert" => Some(Hazard::InertGas),
                    "sand" => Some(Hazard::Sand),
                    "light" => Some(Hazard::Light),
                    _ => None,
                };
                if let Some(hazard) = hazard {
                    reply = filter_exposure(game, hazard, wear).to_string();
                }
            }
        }
        [_, "charge", value] => {
            if let Ok(value) = value.parse::<i64>()
                && value >= 0
                && let Some(stack) = game.held().cloned()
            {
                reply = patch_stack(game, game.held_slot(), &stack, |data| {
                    data.insert("ntm_charge".into(), Tag::Long(value));
                    true
                })
                .to_string();
            }
        }
        _ => {}
    }
    game.native_feedback(horizon_client::text::Text::literal(reply));
    true
}
pub(super) fn equipped(game: &Game<'_>, slot: &str, new: Option<&ItemStack>) {
    trace(
        game,
        "equipment",
        &format!(
            "{}:{}",
            slot,
            new.map_or("minecraft:air", |s| s.item.as_ref())
        ),
    );
}

/// The line `ItemLens.addInformation` adds to an AMS lens tooltip: its
/// `damage` long (0 without one) against the full durability. The client
/// tooltip and `ntmitem lens` both read it here.
pub(super) fn lens_tooltip(stack: &ItemStack) -> Option<String> {
    if &*stack.item != "ntm:ams_lens" {
        return None;
    }
    let damage = match custom(stack).and_then(|d| d.get(&JavaString::from("damage")).cloned()) {
        Some(Tag::Long(n)) => n,
        _ => 0,
    };
    Some(ntm_core::consumables::lens_line(
        ntm_core::consumables::AMS_LENS_MAX,
        damage,
    ))
}

fn custom(stack: &ItemStack) -> Option<Compound> {
    match stack.data().and_then(|d| d.other("minecraft:custom_data")) {
        None => Some(Compound::new()),
        Some(Tag::Compound(c)) => Some(c.clone()),
        _ => None,
    }
}
/// The `IGasMask` that filters for a worn helmet: the helmet itself when it is
/// one (M65, the hazmat hoods), or the attachment mask installed on it.
fn mask_of(helmet: &ItemStack) -> Option<ItemStack> {
    if item_hooks::is_gas_mask(&helmet.item) && &*helmet.item != "ntm:attachment_mask" {
        return Some(helmet.clone());
    }
    let data = custom(helmet)?;
    let Tag::Compound(mods) = data.get(&JavaString::from("ntm_armor_mods"))? else {
        return None;
    };
    let mask = item_from_nbt(mods.get(&JavaString::from("mod_slot_0"))?)?;
    (&*mask.item == "ntm:attachment_mask").then_some(mask)
}
fn put_mask(helmet: &mut ItemStack, mask: ItemStack) -> bool {
    if item_hooks::is_gas_mask(&helmet.item) && &*helmet.item != "ntm:attachment_mask" {
        *helmet = mask;
        return true;
    }
    let Some(mut data) = custom(helmet) else {
        return false;
    };
    let Some(Tag::Compound(mods)) = data.get_mut(&JavaString::from("ntm_armor_mods")) else {
        return false;
    };
    mods.insert("mod_slot_0".into(), item_to_nbt(&mask));
    helmet
        .data_mut()
        .set_other("minecraft:custom_data", Some(Tag::Compound(data)));
    true
}
/// Respawn cannot reseed a new body from an older world/KV dose.
pub(super) fn cloned(game: &mut Game<'_>, player: u128, was_death: bool) {
    if !was_death {
        return;
    }
    let mut state = Compound::new();
    state.insert("schema".into(), Tag::Int(1));
    set(&mut state, "dose", 0);
    let attachments = if game.lan_acting_uuid == Some(player) {
        &mut game.player_attachments
    } else {
        game.guest_attachments.entry(player).or_default()
    };
    if let Err(error) = attachments.set(&STATE, state) {
        eprintln!("ntm item state respawn: {error:?}");
    }
}

/// Exercise the real native commit boundary without changing item counts:
/// invalid benefits refuse everything, a confirmed edit rejects its replay,
/// and restoration is another guarded transaction.
fn transaction_check(game: &mut Game<'_>) -> bool {
    let Some(expected) = game.native_player_snapshot(&STATE) else {
        return false;
    };
    let mut desired = expected.clone();
    let Some(seed) = legacy(game) else {
        return false;
    };
    init(&mut desired, seed);
    desired
        .state
        .insert("transaction_probe".into(), Tag::Long(1));
    let invalid = NativePlayerBenefits {
        effects: vec![("ntm:missing_effect".into(), 20, 0)],
        ..Default::default()
    };
    if game.native_commit_player(&STATE, &expected, desired.clone(), invalid)
        || game.native_player_snapshot(&STATE).as_ref() != Some(&expected)
    {
        return false;
    }
    if !game.native_commit_player(&STATE, &expected, desired.clone(), Default::default()) {
        return false;
    }
    let replay = game.native_commit_player(&STATE, &expected, desired.clone(), Default::default());
    let Some(current) = game.native_player_snapshot(&STATE) else {
        return false;
    };
    let unchanged = current == desired;
    let restored = game.native_commit_player(&STATE, &current, expected, Default::default());
    !replay && unchanged && restored
}
