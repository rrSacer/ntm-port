// Native adapter shared by consumables, equipment and stack state.
use crate::game::{
    Game,
    native_player_transactions::{NativePlayerBenefits, NativePlayerSnapshot},
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
/// Plan counted insertion on a cloned inventory. Full inventory refuses the
/// complete use, so kit/container overflow never deletes or duplicates items.
fn insert(game: &Game<'_>, slots: &mut [Option<ItemStack>], mut stack: ItemStack) -> bool {
    let Some(info) = game.items().get(&stack.item) else {
        return false;
    };
    for s in slots.iter_mut().flatten() {
        if s.same_item_same_components(&stack) {
            let n = stack.count.min(info.max_stack.saturating_sub(s.count));
            s.count += n;
            stack.count -= n;
        }
    }
    for s in slots.iter_mut() {
        if s.is_none() && stack.count > 0 {
            let n = stack.count.min(info.max_stack);
            let mut part = stack.clone();
            part.count = n;
            *s = Some(part);
            stack.count -= n;
        }
    }
    stack.count == 0
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
    if let Some(spec) = item_hooks::use_spec(&stack.item) {
        *value = match spec.animation {
            Animation::Eat => horizon_world::consumable::UseAnimation::Eat,
            Animation::Drink | Animation::Inject | Animation::Smoke => {
                horizon_world::consumable::UseAnimation::Drink
            }
            Animation::None => horizon_world::consumable::UseAnimation::None,
        };
    }
}
pub(super) fn start(game: &mut Game<'_>, stack: &ItemStack, canceled: &mut bool) {
    if item_hooks::use_spec(&stack.item).is_none() {
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
    if &*stack.item == "ntm:gas_mask_filter" {
        install_filter(game, stack);
        return true;
    }
    false
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
    let mut desired = expected.clone();
    let Some(seed) = legacy(game) else {
        return false;
    };
    init(&mut desired, seed);
    if spec.cooldown > 0 && number(&desired.state, "sickness") > 0 {
        return false;
    }
    let Some(slot) = held(&mut desired, game.held_slot()) else {
        return false;
    };
    if slot.as_ref() != Some(stack) {
        return false;
    }
    let mut rest = stack.clone();
    rest.count -= 1;
    *slot = (rest.count > 0).then_some(rest);
    if let Some(id) = spec.container
        && !insert(game, &mut desired.inventory, ItemStack::new(id, 1))
    {
        return false;
    }
    if &*stack.item == "ntm:hazmat_kit" {
        for (index, id) in [
            "ntm:hazmat_helmet",
            "ntm:hazmat_plate",
            "ntm:hazmat_legs",
            "ntm:hazmat_boots",
        ]
        .into_iter()
        .enumerate()
        {
            if let Some(old) = desired.armor[index].take()
                && !insert(game, &mut desired.inventory, old)
            {
                return false;
            }
            desired.armor[index] = Some(ItemStack::new(id, 1));
        }
    }
    if spec.radaway > 0 {
        let remaining = number(&desired.state, "radaway").saturating_add(u64::from(spec.radaway));
        set(&mut desired.state, "radaway", remaining);
    }
    if spec.cooldown > 0 {
        set(&mut desired.state, "sickness", spec.cooldown as u64);
    }
    let benefits = NativePlayerBenefits {
        food: spec.food,
        effects: spec
            .effects
            .iter()
            .map(|e| (e.id.into(), e.ticks, e.amplifier))
            .collect(),
        cooldown: (spec.cooldown > 0).then(|| (stack.item.to_string(), spec.cooldown)),
        ..Default::default()
    };
    let applied = game.native_commit_player(&STATE, &expected, desired, benefits);
    if applied {
        trace(game, "use", &stack.item);
    }
    applied
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
    if desired != expected {
        game.native_commit_player(&STATE, &expected, desired, Default::default());
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
    item_hooks::dose_factor(game.armor.iter().flatten().map(|s| s.item.as_ref()))
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
