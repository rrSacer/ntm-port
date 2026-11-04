// Native adapter shared by consumables, equipment and stack state.
use crate::game::{
    Game,
    native_player_transactions::{
        NativeDrop, NativeDropMode, NativePlayerBenefits, NativePlayerSnapshot,
    },
};
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

fn empty() -> Compound {
    Compound::new()
}
fn codec(value: &Compound) -> Result<Compound, AttachmentError> {
    if !value.is_empty() && value.get(&JavaString::from("schema")) != Some(&Tag::Int(1)) {
        return Err(AttachmentError::Decode);
    }
    for key in ["dose", "radaway", "sickness"] {
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
            game.native_feedback(horizon_client::text::Text::literal(message.clone()));
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
    if matches!(&*stack.item, "ntm:gas_mask_m65" | "ntm:attachment_mask") {
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
    let say = |game: &mut Game<'_>, line: &str| game.native_feedback(horizon_client::text::Text::legacy(line));
    match id {
        "ntm:geiger_counter" | "ntm:dosimeter" => {
            play(game, "hbm:item.techBoop");
            let buffer = rad_buffer(game);
            let lines = if id == "ntm:dosimeter" {
                ins::dosimeter_lines(buffer)
            } else {
                let Some(seed) = legacy(game) else {
                    return true;
                };
                let dose = read_dose(game, &seed).map_or(0, |d| d.millirad);
                let (zone, resistance) = (zone_rate(game), -radiation_factor(game).log10());
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
    let sneaking = game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft);
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
/// item is not placed or used as its template.
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
    let used = if &*stack.item == "ntm:matchstick" {
        // ItemMatch: the cell beside the clicked face; fire if it is air, and
        // the match goes either way (not in creative).
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
        true
    } else {
        // ItemFertilizer: bone meal on the 3x3x3 around the click; it is spent
        // only if something grew.
        let mut grew = false;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let cell = horizon_core::position::BlockPos::new(pos[0] + dx, pos[1] + dy, pos[2] + dz);
                    grew |= game.with_level(|b, level, ticks| {
                        b.use_bone_meal(level, ticks, cell, horizon_core::geometry::Direction::Up)
                    });
                }
            }
        }
        let _ = game.process_level_events();
        grew
    };
    if used && !creative {
        let held_slot = game.held_slot();
        let mut desired = expected.clone();
        if let Some(slot) = held(&mut desired, held_slot)
            && slot.as_ref() == Some(stack)
        {
            let mut rest = stack.clone();
            rest.count -= 1;
            *slot = (rest.count > 0).then_some(rest);
            game.native_commit_player(&STATE, &expected, desired, Default::default());
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
    if !game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft) {
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
    if spec.radaway > 0 || spec.cooldown > 0 || dose.is_some() {
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
    } else if spec.consume == item_hooks::Consume::SetDamage {
        if let Some(s) = slot.as_mut() {
            s.damage = spec.set_damage;
        }
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
    if spec.radaway > 0 {
        let remaining = number(&desired.state, "radaway").saturating_add(u64::from(spec.radaway));
        set(&mut desired.state, "radaway", remaining);
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
            .collect(),
        cooldown: (spec.cooldown > 0).then(|| (stack.item.to_string(), spec.cooldown)),
        drops,
        ..Default::default()
    };
    let applied = game.native_commit_player(&STATE, &expected, desired, benefits);
    if applied {
        trace(game, "use", &stack.item);
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
        let mut e = game.experience.clone();
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

/// Plays the vanilla event that stands for a Java sound, at the player, as
/// `playSoundAtEntity` does.
fn play(game: &mut Game<'_>, java: &str) {
    let Some(sound) = ntm_core::consumables::sound(java) else {
        return;
    };
    let Some(p) = game.player.as_ref() else {
        return;
    };
    let at = [p.position.x, p.position.y, p.position.z];
    let Some(source) = horizon_client::settings::SOUND_SOURCES
        .iter()
        .position(|s| *s == "player")
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

/// `ItemEnergy.onEaten` for chocolate milk: `ExplosionVNT(world, x, y + h/2, z,
/// 50)` with `BlockAllocatorStandard(32)`, `BlockProcessorStandard().setNoDrop()`,
/// `EntityProcessorCross(5)` and `PlayerProcessorStandard`, in that order:
/// allocate, hurt and push the entities, then remove the blocks. The rays are
/// the NTM crater module's vanilla allocation (`blast::vanilla`), the blocks
/// go out as air in batches with no drops, and every living player of the level
/// in range takes the Cross damage and push. Mobs and dropped items are not
/// reached (no engine API for them yet; see the doc).
fn chocolate_milk_blast(game: &mut Game<'_>) {
    use ntm_core::blast::vanilla;
    const SIZE: f32 = 50.0;
    let Some(p) = game.player.as_ref() else {
        return;
    };
    let center = [
        p.position.x,
        p.position.y + f64::from(p.dimensions.height) / 2.0,
        p.position.z,
    ];
    let dim = game.dim.id().to_string();
    let (cells, _) = vanilla::allocate_vnt(center, SIZE, 32, &mut |b| game.native_loaded_block(&dim, b));
    // The drinker, then the other living players of the level, as the engine's
    // batch-by-UUID helpers reach them.
    cross_player(game, center, SIZE, &dim);
    let me = game.native_actor_uuid();
    let others: Vec<String> = game
        .native_players()
        .into_iter()
        .filter(|q| q.alive && q.dimension == dim && Some(&q.uuid) != me.as_ref())
        .map(|q| q.uuid)
        .collect();
    for uuid in others {
        game.native_visit_player(&uuid, |g| cross_player(g, center, SIZE, &dim));
    }
    let air: Vec<([i32; 3], String)> = cells.iter().map(|c| (*c, "minecraft:air".to_string())).collect();
    for chunk in air.chunks(ntm_core::explosion::BLOCKS_PER_CALL) {
        if let Err(e) = game.native_set_blocks(&dim, chunk) {
            eprintln!("ntm item: chocolate milk blast: {e}");
            break;
        }
    }
    // `ExplosionCreator.composeEffectStandard` is a client particle show; the
    // vanilla explosion sound and emitter stand for it.
    if let Some(source) = horizon_client::settings::SOUND_SOURCES.iter().position(|s| *s == "block") {
        game.with_level(|_, level, _| {
            level.native_play_sound("minecraft:entity.generic.explode", center, source, 4.0, 0.7)
        });
    }
    if let Some(kind) = horizon_client::particles::ParticleType::from_name("minecraft:explosion_emitter") {
        let options = horizon_client::particles::ParticleOptions::simple(kind);
        game.with_level(|_, level, _| level.native_send_particles(&options, None, center, [0.0; 3], 0.0, 1));
    }
}

/// `EntityProcessorCross.process` for the player the game is acting as: the
/// box distance, the densest of the seven nodes, then the push and the damage.
fn cross_player(game: &mut Game<'_>, center: [f64; 3], size: f32, dim: &str) {
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
    if length == 0.0 {
        return;
    }
    let density = vanilla::cross_nodes(center, 5.0)
        .iter()
        .map(|node| vanilla::exposure(*node, feet, &mut |b| game.native_loaded_block(dim, b)))
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
        let dose = number(&desired.state, "dose").saturating_sub(1_000);
        set(&mut desired.state, "dose", dose);
        set(&mut desired.state, "radaway", remaining - 1);
    }
    let sickness = number(&desired.state, "sickness");
    if sickness > 0 {
        set(&mut desired.state, "sickness", sickness - 1);
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
        game.native_commit_player(&STATE, &expected, desired, Default::default());
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
pub(super) fn radiation_factor(game: &Game<'_>) -> f64 {
    // HazmatRegistry.getResistance: the armor plus 0.2 while Rad-X is active.
    item_hooks::dose_factor(
        game.armor.iter().flatten().map(|s| s.item.as_ref()),
        game.potions.effects.get("ntm:radx").is_some(),
    )
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

/// Exposure-specific filter wear, in the same CAS as protection state. Gamma
/// uses HazmatRegistry independently and never spends a particle filter.
pub(super) fn filter_exposure(game: &mut Game<'_>, hazard: Hazard, wear: u32) -> bool {
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
    let Some(Tag::Compound(mut data)) = mask
        .data()
        .and_then(|d| d.other("minecraft:custom_data"))
        .cloned()
    else {
        return false;
    };
    let Some(mut filter) = data
        .get(&JavaString::from("hfrFilter"))
        .and_then(item_from_nbt)
    else {
        return false;
    };
    if !item_hooks::filter_protects(&mask.item, &filter.item, hazard) {
        return false;
    }
    filter.damage = filter.damage.saturating_add(wear);
    if filter.damage > 20_000 {
        data.remove(&JavaString::from("hfrFilter"));
    } else {
        data.insert("hfrFilter".into(), item_to_nbt(&filter));
    }
    mask.data_mut()
        .set_other("minecraft:custom_data", Some(Tag::Compound(data)));
    if !put_mask(helmet, mask) {
        return false;
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

fn custom(stack: &ItemStack) -> Option<Compound> {
    match stack.data().and_then(|d| d.other("minecraft:custom_data")) {
        None => Some(Compound::new()),
        Some(Tag::Compound(c)) => Some(c.clone()),
        _ => None,
    }
}
fn mask_of(helmet: &ItemStack) -> Option<ItemStack> {
    if &*helmet.item == "ntm:gas_mask_m65" {
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
    if &*helmet.item == "ntm:gas_mask_m65" {
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
