// The NTM bundle's edges in the game (docs/nativo.md of ntm-port): a
// machine's block entity to and from ntm_core::native's plain data. Its
// state is a JSON string under "ntm" in the block entity's compound.
use horizon_client::containers::{ModBlockEntity, ModContainer};
use horizon_client::items::ItemStack;
use horizon_world::nbt::{Compound, JavaString, Tag};
use ntm_core::server::container_bridge::Stack;

const STATE: &str = "ntm";

/// A new machine block's block entity, with its new state.
pub(super) fn entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if entity.is_some() {
        return;
    }
    if let Some(state) = ntm_core::native::new_state(block, None) {
        let mut nbt = Compound::new();
        nbt.insert(JavaString::from(STATE), Tag::String(JavaString::from(state.as_str())));
        *entity = Some((block.to_string(), nbt));
    }
}

pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if ntm_core::native::handles(block) {
        *ticks = true;
    }
}

pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if let Some(c) = ntm_core::native::container(block) {
        *container = Some(ModContainer {
            slots: c.slots,
            faces: c.faces,
            inputs: c.inputs,
            outputs: c.outputs,
            menu: c.menu.map(str::to_string),
        });
    }
}

/// A slot as ntm-core sees it: id, count and damage (an empty slot, or air,
/// is none).
fn stack(s: &Option<ItemStack>) -> Option<Stack> {
    let s = s.as_ref().filter(|s| s.count > 0 && &*s.item != "minecraft:air")?;
    let mut t = Stack::new(&s.item, s.count);
    t.damage = s.damage;
    Some(t)
}

/// A server tick: the state, the slots and the menu's data through
/// ntm-core. A slot ntm-core left as it was keeps the game's stack, with
/// all its components.
pub(super) fn tick(entity: &mut ModBlockEntity) {
    let block = entity.id.clone();
    let Some(c) = ntm_core::native::container(&block) else { return };
    if entity.items.len() < c.slots {
        return;
    }
    let mut state = match entity.nbt.get(&JavaString::from(STATE)) {
        Some(Tag::String(s)) => s.to_string_lossy(),
        _ => String::new(),
    };
    let before: Vec<Option<Stack>> = entity.items.iter().map(stack).collect();
    let mut items = before.clone();
    if !ntm_core::native::tick(&block, &mut state, &mut items, &mut entity.data) {
        return;
    }
    entity.nbt.insert(JavaString::from(STATE), Tag::String(JavaString::from(state.as_str())));
    for (i, (new, old)) in items.iter().zip(&before).enumerate() {
        if new != old {
            entity.items[i] = new.as_ref().map(|s| {
                let mut it = ItemStack::new(&s.id, s.count);
                it.damage = s.damage;
                it
            });
        }
    }
}
