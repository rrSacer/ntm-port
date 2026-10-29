//! The foundry on the server (docs/maquinas.md, "Fundição"): what needs
//! the blocks around a machine. Each loaded crucible pulls heat from the
//! machine right under its core (a firebox: the NTM's proxies don't pass
//! heat), melts and alloys, and pours its two streams straight down from
//! two blocks beside its core: the alloy's outputs on its facing side, the
//! rest on the other (`TileEntityCrucible.java:165-224`). A stream falls up
//! to 6 blocks onto the first block in its way; if that one takes no metal
//! (or there is none), nothing pours and nothing is lost.
use super::*;
use crate::foundry::{Acceptor, HeatSource, Tables, crucible::Stream};
use crate::machine::Data;

/// How far a crucible's stream falls (`pourFullStack(..., 6, ...)`).
const FALL: i32 = 6;

/// The machines molten metal can be poured into.
fn acceptor(data: &mut Data) -> Option<&mut dyn Acceptor> {
    match data {
        Data::Crucible(c) => Some(c),
        _ => None,
    }
}

impl Server {
    pub(super) fn foundry_tick(&mut self, host: &mut impl ServerHost) {
        let tick = self.tick;
        let t = Tables::ntm();
        let Session::Ready(r) = &mut self.session else { return };
        let crucibles: Vec<(Key, Option<String>)> = r
            .world
            .machines
            .iter()
            .filter(|(k, m)| matches!(m.data, Data::Crucible(_)) && r.loaded.get(*k) == Some(&true))
            .map(|(k, m)| (k.clone(), m.facing.clone()))
            .collect();
        for (key, facing) in crucibles {
            let Some(mut m) = r.world.machines.remove(&key) else { continue };
            let Data::Crucible(c) = &mut m.data else {
                r.world.machines.insert(key, m);
                continue;
            };
            // Heat from the machine right under the core.
            let below = (key.0.clone(), [key.1[0], key.1[1] - 1, key.1[2]]);
            let mut changed = match r.world.machines.get_mut(&below).map(|b| &mut b.data) {
                Some(Data::Firebox(f)) => c.step(t, Some(f as &mut dyn HeatSource), tick),
                _ => c.step(t, None, tick),
            };
            // The streams.
            if let Some(d) = facing.as_deref().and_then(registry::facing_dir) {
                for (stream, s) in [(Stream::Recipe, 1), (Stream::Waste, -1)] {
                    let x = key.1[0] + 2 * d[0] * s;
                    let z = key.1[2] + 2 * d[2] * s;
                    let Some(target) = (0..=FALL).map(|k| (key.0.clone(), [x, key.1[1] - k, z])).find(|p| {
                        r.world.machines.contains_key(p) || host.get_block(&p.0, p.1).is_none_or(|b| blockstate::id_of(&b) != "minecraft:air")
                    }) else {
                        continue;
                    };
                    if let Some(a) = r.world.machines.get_mut(&target).and_then(|m| acceptor(&mut m.data))
                        && c.pour_stream(t, stream, a).is_some()
                    {
                        changed = true;
                    }
                }
            }
            r.world.machines.insert(key, m);
            if changed {
                r.dirty = true;
            }
        }
    }
}
