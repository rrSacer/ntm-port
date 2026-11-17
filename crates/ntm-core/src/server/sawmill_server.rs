//! D3 of Onda 2: the sawmill's item without its blade (`MachineSawmill.onBlockPlacedBy` / `getDrops`, the Java's
//! damage 1), across placing and breaking it. The host reads and writes the item (`minecraft:custom_data`); the model
//! keeps `has_blade`.
use super::*;
use crate::machine::{Data, MachineEffect};

impl Server {
    /// `finish` of every loaded sawmill (`TileEntitySawmill.updateEntity` after `tryPullHeat`, which `heat_tick` did).
    /// The tick that breaks the blade (128-147) is committed with its two effects as one batch, before this tick's
    /// native_save, in this order (R4: the blast must not kill the blade it throws): the blast at
    /// [x+0.5, y+1, z+0.5], power 5, no fire, no broken block; the blade, one `ntm:sawblade` at the cell in front of
    /// the core one up (the Java's `EntitySawblade` at core + dir, y+1, which gives the blade back when used; the entity
    /// and its flight need the entity API, so the item lies there).
    pub(super) fn sawmill_finish(&mut self, host: &mut impl ServerHost) {
        let r = &mut self.ready;
        let mut failing = Vec::new();
        for (k, m) in &mut r.world.machines {
            if r.loaded.get(k) == Some(&true)
                && let Data::Sawmill(s) = &mut m.data
            {
                if s.fails_on_finish() {
                    failing.push(k.clone());
                } else {
                    s.finish();
                }
            }
        }
        for key in failing {
            let published = self.publish_machine_effects(host, &key, |m| {
                let Data::Sawmill(s) = &mut m.data else { return Vec::new() };
                s.finish();
                vec![
                    MachineEffect::Explode { offset: [0., 1., 0.], power: 5., fire: false, breaks: false },
                    MachineEffect::Drop { offset: [0, 1, 1], item: crate::sawmill::BLADE.into(), count: 1 },
                ]
            });
            // Refused: the live model keeps its blade and count and breaks on the next hot tick. Only the heat of this
            // tick is spent, as `finish` spends it every tick (it is never saved).
            if !published
                && let Some(Data::Sawmill(s)) = self.ready.world.machines.get_mut(&key).map(|m| &mut m.data)
            {
                s.heat = 0;
                host.log(&format!("ntm: the failing sawmill at {:?} was not committed; it breaks on its next hot tick", key.1));
            }
        }
    }

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
