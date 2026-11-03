// ContainerArmorTable: one physical armor stack and nine views of its nested
// mods, plus four views of the player's worn armor. Views never own items.
use crate::game::{
    Game,
    native_player_transactions::{NativePlayerBenefits, NativePlayerSnapshot},
};
use horizon_client::{
    containers::{ModSlotFilter, SlotAction, SlotOrigin, SlotPolicy},
    items::ItemStack,
    menu::{Env, Input, Kind, Menu},
    text::Text,
};
use horizon_items::item_nbt::{item_from_nbt, item_to_nbt};
use horizon_world::{
    attachments::{AttachmentCodec, AttachmentError, AttachmentType},
    nbt::{Compound, JavaString, Tag},
};
use std::{collections::HashMap, sync::Mutex};
const BLOCK: &str = "ntm:machine_armor_table";
const MENU: &str = "ntm:armor_table";
const N: usize = 14;
fn empty() -> Compound {
    Compound::new()
}
fn codec(c: &Compound) -> Result<Compound, AttachmentError> {
    if let Some(t) = c.get(&JavaString::from("armor"))
        && item_from_nbt(t).is_none_or(|s| s.count != 1 || mods(&s).is_none())
    {
        return Err(AttachmentError::Decode);
    }
    Ok(c.clone())
}
static STATE: AttachmentType<Compound> = AttachmentType {
    namespace: "ntm",
    name: "armor_table",
    default: empty,
    codec: Some(AttachmentCodec {
        encode: codec,
        decode: codec,
    }),
    copy_on_death: true,
    sync: false,
};
type Sessions = HashMap<u128, (i32, Menu)>;
static OPEN: Mutex<Option<Sessions>> = Mutex::new(None);
static ARMOR: Mutex<Vec<(String, usize)>> = Mutex::new(Vec::new());
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}
fn armor_index(game: &Game<'_>, stack: &ItemStack) -> Option<usize> {
    match game.items().get(&stack.item)?.equip_slot.as_deref()? {
        "head" => Some(0),
        "chest" => Some(1),
        "legs" => Some(2),
        "feet" => Some(3),
        _ => None,
    }
}
fn custom(stack: &ItemStack) -> Option<Compound> {
    match stack.data().and_then(|d| d.other("minecraft:custom_data")) {
        None => Some(Compound::new()),
        Some(Tag::Compound(c)) => Some(c.clone()),
        _ => None,
    }
}
fn mods(stack: &ItemStack) -> Option<Vec<Option<ItemStack>>> {
    let data = custom(stack)?;
    let c = match data.get(&JavaString::from("ntm_armor_mods")) {
        None => return Some(vec![None; 9]),
        Some(Tag::Compound(c)) => c,
        _ => return None,
    };
    (0..9)
        .map(|i| {
            let key = JavaString::from(format!("mod_slot_{i}").as_str());
            match c.get(&key) {
                None => Some(None),
                Some(t) => Some(Some(item_from_nbt(t)?)),
            }
        })
        .collect()
}
fn set_mods(armor: &mut ItemStack, slots: &[Option<ItemStack>]) -> Option<()> {
    let mut data = custom(armor)?;
    let mut c = match data.get(&JavaString::from("ntm_armor_mods")) {
        None => Compound::new(),
        Some(Tag::Compound(c)) => c.clone(),
        _ => return None,
    };
    for (i, slot) in slots.iter().take(9).enumerate() {
        let key = JavaString::from(format!("mod_slot_{i}").as_str());
        if let Some(s) = slot {
            c.insert(key, item_to_nbt(s));
        } else {
            c.remove(&key);
        }
    }
    if c.is_empty() {
        data.remove(&JavaString::from("ntm_armor_mods"));
    } else {
        data.insert("ntm_armor_mods".into(), Tag::Compound(c));
    }
    armor
        .data_mut()
        .set_other("minecraft:custom_data", Some(Tag::Compound(data)));
    Some(())
}
fn table_armor(s: &NativePlayerSnapshot) -> Option<ItemStack> {
    s.state
        .get(&JavaString::from("armor"))
        .and_then(item_from_nbt)
}
fn view(s: &NativePlayerSnapshot) -> Option<Vec<Option<ItemStack>>> {
    let armor = table_armor(s);
    let mut v = armor.as_ref().map_or_else(|| Some(vec![None; 9]), mods)?;
    v.push(armor);
    v.extend(s.armor.iter().cloned());
    Some(v)
}
fn env<'a>(game: &'a Game<'_>) -> Env<'a> {
    Env {
        items: game.items(),
        recipes: game.recipes(),
        brewing: &game.stations().brewing,
        banners: &game.stations().banners,
        station_recipes: &game.stations().recipes,
        enchantments: &game.stations().enchantments,
        names: &game.pack.names,
        trade: None,
        maps: None,
    }
}
fn policy(
    slots: &[Option<ItemStack>],
    slot: usize,
    item: &ItemStack,
    action: SlotAction,
    _origin: SlotOrigin,
) -> bool {
    if action == SlotAction::Extract {
        return true;
    }
    if slot < 9 {
        return slot == 0
            && &*item.item == "ntm:attachment_mask"
            && slots.get(9).and_then(Option::as_ref).is_some_and(|armor| {
                lock(&ARMOR)
                    .iter()
                    .any(|(id, i)| id == armor.item.as_ref() && *i == 0)
            });
    }
    true
}
pub(super) fn filters(game: &Game<'_>) -> Vec<ModSlotFilter> {
    let armor: Vec<_> = game
        .items()
        .items
        .values()
        .filter_map(|i| {
            let stack = ItemStack::new(&i.id, 1);
            Some((i.id.to_string(), armor_index(game, &stack)?))
        })
        .collect();
    *lock(&ARMOR) = armor.clone();
    (0..N)
        .map(|slot| ModSlotFilter {
            max: Some(1),
            accept: Some(if slot < 9 {
                if slot == 0 {
                    vec!["ntm:attachment_mask".into()]
                } else {
                    Vec::new()
                }
            } else {
                armor
                    .iter()
                    .filter(|(_, index)| slot == 9 || *index == slot - 10)
                    .map(|(id, _)| id.clone())
                    .collect()
            }),
            policy: Some(SlotPolicy(policy)),
            ..Default::default()
        })
        .collect()
}
fn model(game: &Game<'_>) -> Menu {
    let mut m = Menu::new(Kind::NativeContainer(N));
    m.mod_slot_filters = filters(game);
    m.creative = game.creative;
    m.selected = game.selected;
    m
}
fn load(game: &Game<'_>, m: &mut Menu, s: &NativePlayerSnapshot) -> Option<()> {
    let v = view(s)?;
    if v.iter()
        .flatten()
        .any(|s| s.count != 1 || game.items().get(&s.item).is_none())
    {
        return None;
    }
    let e = env(game);
    for (i, stack) in v.into_iter().enumerate() {
        m.set_slot(&e, i, stack);
    }
    for (i, stack) in s.inventory.iter().enumerate() {
        m.set_player_item(i, stack.clone());
    }
    m.set_player_item(40, s.offhand.clone());
    m.carried = s.carried.clone();
    m.dropped.clear();
    m.drops.clear();
    Some(())
}
pub(super) fn open(game: &mut Game<'_>, pos: [i32; 3]) -> bool {
    if !game
        .native_loaded_block(game.dim.id(), pos)
        .is_some_and(|b| ntm_core::blockstate::id_of(&b) == BLOCK)
    {
        return false;
    }
    if let Some(uuid) = game.native_acting_player().map(|p| p.uuid) {
        let old = lock(&OPEN).as_mut().and_then(|s| s.remove(&uuid)).is_some();
        if old {
            close_items(game);
        }
    }
    let Some(snapshot) = game.native_player_snapshot(&STATE) else {
        return true;
    };
    let Some(items) = view(&snapshot) else {
        return true;
    };
    if items
        .iter()
        .flatten()
        .any(|s| s.count != 1 || game.items().get(&s.item).is_none())
    {
        return true;
    }
    let Some(conn) = game.lan_acting else {
        return true;
    };
    let Some(mut host) = game.lan.take() else {
        return true;
    };
    game.guest_close_all(&mut host, conn);
    if let Some(guest) = host.hub.guest_mut(conn) {
        guest.native_menu = items.clone();
    }
    host.hub.open_container(
        conn,
        MENU,
        horizon_client::lan::data::component_nbt(
            &Text::tr("container.ntm.armor_table", &[]).component(),
        ),
        (pos[0], pos[1], pos[2]),
        horizon_client::lan::menu::Slots::Native,
        items,
    );
    let window = host
        .hub
        .guest(conn)
        .and_then(|g| g.menu.as_ref())
        .map(|m| m.window);
    game.lan = Some(host);
    if let Some(window) = window {
        lock(&OPEN)
            .get_or_insert_with(Default::default)
            .insert(snapshot.uuid, (window, model(game)));
    }
    true
}
fn move_to(
    game: &Game<'_>,
    m: &mut Menu,
    source: usize,
    destinations: impl IntoIterator<Item = usize>,
) -> bool {
    let Some(stack) = m.slot(source).cloned() else {
        return false;
    };
    let current: Vec<_> = (0..N).map(|i| m.slot(i).cloned()).collect();
    for to in destinations {
        if m.slot(to).is_some() {
            continue;
        }
        if to < N
            && !m.mod_slot_filters[to].allows(
                &current,
                to,
                &stack,
                SlotAction::Insert,
                SlotOrigin::Manual,
            )
        {
            continue;
        }
        let e = env(game);
        m.set_slot(&e, to, Some(stack));
        m.set_slot(&e, source, None);
        return true;
    }
    false
}
fn quick(game: &Game<'_>, m: &mut Menu, slot: usize) {
    let Some(stack) = m.slot(slot).cloned() else {
        return;
    };
    if slot < 10 {
        if slot == 9
            && let Some(index) = armor_index(game, &stack)
            && move_to(game, m, slot, [10 + index])
        {
            return;
        }
        move_to(game, m, slot, (N..N + 36).rev());
    } else if armor_index(game, &stack).is_some() {
        move_to(game, m, slot, [9]);
    } else if &*stack.item == "ntm:attachment_mask" {
        move_to(game, m, slot, [0]);
    }
}
pub(super) fn clicked(
    game: &mut Game<'_>,
    uuid: u128,
    window: i32,
    slot: i32,
    button: i32,
    mode: i32,
) {
    let current = game
        .lan
        .as_ref()
        .and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid))
        .and_then(|g| g.menu.as_ref())
        .is_some_and(|m| m.window == window && m.mod_type.is_some_and(|t| t.id == MENU));
    if !current {
        return;
    }
    game.native_visit_player(&horizon_protocol::crypto::uuid_string(uuid), |game| {
        let Some(expected) = game.native_player_snapshot(&STATE) else {
            return;
        };
        let mut sessions = lock(&OPEN);
        let Some((id, m)) = sessions.as_mut().and_then(|s| s.get_mut(&uuid)) else {
            return;
        };
        if *id != window {
            return;
        }
        if load(game, m, &expected).is_none() {
            return;
        }
        let before = table_armor(&expected);
        if mode == 1 {
            if let Ok(slot) = usize::try_from(slot)
                && slot < N + 36
            {
                quick(game, m, slot);
            }
        } else {
            let input = match mode {
                0 => Input::Pickup,
                2 => Input::Swap,
                3 => Input::Clone,
                4 => Input::Throw,
                5 => Input::QuickCraft,
                6 => Input::PickupAll,
                _ => return,
            };
            if m.click(&env(game), slot, button, input).is_err() {
                return;
            }
        }
        let mut desired = expected.clone();
        let mut armor = m.slot(9).cloned();
        if armor == before
            && let Some(a) = armor.as_mut()
        {
            let slots: Vec<_> = (0..9).map(|i| m.slot(i).cloned()).collect();
            if set_mods(a, &slots).is_none() {
                return;
            }
        }
        if let Some(armor) = armor {
            desired.state.insert("armor".into(), item_to_nbt(&armor));
        } else {
            desired.state.remove(&JavaString::from("armor"));
        }
        for (i, to) in desired.inventory.iter_mut().enumerate() {
            *to = m.player_item(i).cloned();
        }
        for (i, to) in desired.armor.iter_mut().enumerate() {
            *to = m.slot(10 + i).cloned();
        }
        desired.offhand = m.player_item(40).cloned();
        desired.carried = m.carried.clone();
        let benefits = NativePlayerBenefits {
            drops: m.dropped.clone(),
            ..Default::default()
        };
        if !game.native_commit_player(&STATE, &expected, desired.clone(), benefits) {
            *m = model(game);
            return;
        }
        let Some(items) = view(&desired) else {
            return;
        };
        if let Some(conn) = game.lan_acting
            && let Some(host) = game.lan.as_mut()
        {
            host.hub.set_native_menu(conn, items);
        }
    });
}
pub(super) fn closed(game: &mut Game<'_>, uuid: u128, window: i32) {
    let mut sessions = lock(&OPEN);
    let ours = sessions
        .as_ref()
        .and_then(|s| s.get(&uuid))
        .is_some_and(|(w, _)| *w == window);
    if ours && let Some(s) = sessions.as_mut() {
        s.remove(&uuid);
    }
    drop(sessions);
    if !ours {
        return;
    }
    game.native_visit_player(&horizon_protocol::crypto::uuid_string(uuid), close_items);
}
fn close_items(game: &mut Game<'_>) {
    let Some(expected) = game.native_player_snapshot(&STATE) else {
        return;
    };
    let Some(mut armor) = table_armor(&expected) else {
        return;
    };
    let Some(mods) = mods(&armor) else {
        return;
    };
    let mut drops: Vec<_> = mods.into_iter().flatten().collect();
    if set_mods(&mut armor, &std::array::from_fn::<_, 9, _>(|_| None)).is_none() {
        return;
    }
    drops.push(armor);
    let mut desired = expected.clone();
    desired.state.remove(&JavaString::from("armor"));
    game.native_commit_player(
        &STATE,
        &expected,
        desired,
        NativePlayerBenefits {
            drops,
            ..Default::default()
        },
    );
}
pub(super) fn tick(game: &mut Game<'_>) {
    let Some(player) = game.native_acting_player() else {
        return;
    };
    let open = game
        .lan
        .as_ref()
        .and_then(|h| h.hub.guests.iter().find(|g| g.uuid == player.uuid))
        .and_then(|g| g.menu.as_ref())
        .is_some_and(|m| m.mod_type.is_some_and(|t| t.id == MENU));
    if !open {
        close_items(game);
    }
}
pub(super) fn screen() -> horizon_client::mod_api::NativeScreen {
    horizon_client::mod_api::NativeScreen {
        width: 198.,
        height: 222.,
        slots: vec![
            Some([48., 27.]),
            Some([84., 27.]),
            Some([120., 27.]),
            Some([156., 45.]),
            Some([156., 81.]),
            Some([120., 99.]),
            Some([84., 99.]),
            Some([48., 99.]),
            Some([30., 63.]),
            Some([66., 63.]),
            Some([5., 36.]),
            Some([5., 54.]),
            Some([5., 72.]),
            Some([5., 90.]),
        ],
        inventory_x: 30.,
        inventory_y: 140.,
        own_labels: true,
        own_background: true,
        hide_inventory: false,
    }
}
pub(super) fn draw(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, [x, y]: [f32; 2]) {
    let sprite = "ntm:gui/machine/gui_armor_modifier";
    game.native_sprite(
        batch,
        sprite,
        [0., 0., 176., 222.],
        [x + 22., y, 176., 222.],
    );
    game.native_sprite(
        batch,
        sprite,
        [176., 96., 22., 100.],
        [x, y + 31., 22., 100.],
    );
    if let Some(entity) = game
        .container
        .and_then(|p| game.world.block_entities.get(&p))
    {
        let slots = entity.slots();
        if slots.get(9).is_some_and(Option::is_some) {
            game.native_sprite(
                batch,
                sprite,
                [176., 74., 22., 22.],
                [x + 63., y + 60., 22., 22.],
            );
        }
        let positions = screen().slots;
        for (i, slot) in slots.iter().take(9).enumerate() {
            if slot.is_some()
                && let Some([sx, sy]) = positions[i]
            {
                let applicable = i == 0
                    && slots
                        .get(9)
                        .and_then(Option::as_ref)
                        .is_some_and(|a| armor_index(game, a) == Some(0));
                game.native_sprite(
                    batch,
                    sprite,
                    [176., if applicable { 34. } else { 16. }, 18., 18.],
                    [x + sx - 1., y + sy - 1., 18., 18.],
                );
            }
        }
    }
    let title = game.translate("container.ntm.armor_table", "");
    game.native_text(
        batch,
        &title,
        [x + 110. - game.native_text_width(&title) / 2., y + 6.],
        0x404040,
        false,
    );
    let inventory = game.translate("container.inventory", "");
    game.native_text(batch, &inventory, [x + 30., y + 128.], 0x404040, false);
}
