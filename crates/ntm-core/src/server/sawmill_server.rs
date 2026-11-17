//! D3 of Onda 2: the sawmill's item without its blade (`MachineSawmill.onBlockPlacedBy` / `getDrops`, the Java's
//! damage 1), across placing and breaking it. The host reads and writes the item (`minecraft:custom_data`); the model
//! keeps `has_blade`.
use super::*;
use crate::machine::Data;

impl Server {
    /// A player placed a sawmill item at `pos` (`native_place_block`, before its `Placed` event is taken): without its
    /// blade, the machine that event creates there has none.
    pub fn native_sawmill_placed(&mut self, dim: &str, pos: Pos, without_blade: bool) {
        let key = (dim.to_string(), pos);
        if without_blade {
            self.ready.sawmills_without_blade.insert(key);
        } else {
            self.ready.sawmills_without_blade.remove(&key);
        }
    }

    /// Whether the sawmill broken at `pos` drops its item without its blade: its model, still held when the loot table
    /// rolls (the `Broken` event is taken after), says so.
    /// A placement not taken yet counts: placed and broken before its `Placed` event, it still has no blade.
    pub fn native_sawmill_without_blade(&self, dim: &str, pos: Pos) -> bool {
        let key = (dim.to_string(), pos);
        self.ready.sawmills_without_blade.contains(&key)
            || matches!(self.ready.world.machines.get(&key).map(|m| &m.data), Some(Data::Sawmill(s)) if !s.has_blade)
    }

    /// Every cell `native_sawmill_without_blade` answers yes for: the host's copy for the drops it hears while the server
    /// is busy (a blast of the server's own breaking a sawmill).
    pub fn native_sawmills_without_blade(&self) -> Vec<Key> {
        let models = self.ready.world.machines.iter().filter(|(_, m)| matches!(&m.data, Data::Sawmill(s) if !s.has_blade));
        self.ready.sawmills_without_blade.iter().cloned().chain(models.map(|(k, _)| k.clone())).collect()
    }
}
