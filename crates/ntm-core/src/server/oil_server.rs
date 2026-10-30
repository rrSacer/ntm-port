//! The oil chain's world (docs/maquinas.md, "Petróleo"): fractioning towers
//! stacked one on another (`TileEntityMachineFractionTower`: the one below
//! pushes its input up and pulls the fractions down, each tick), and the
//! heat boilers and cokers take from the firebox under them.
use super::*;
use crate::foundry::HeatSource;
use crate::fraction::{FractionTower, STACK_DY};
use crate::machine::Data;

impl Server {
    pub(super) fn fraction_tick(&mut self) {
        let Session::Ready(r) = &mut self.session else { return };
        let mut keys: Vec<Key> = r
            .world
            .machines
            .iter()
            .filter(|(k, m)| r.loaded.get(*k) == Some(&true) && matches!(m.data, Data::FractionTower(_)))
            .map(|(k, _)| k.clone())
            .collect();
        // From the bottom of each stack up, as the NTM's lower tower acts.
        keys.sort_by_key(|k| k.1[1]);
        let mut changed = false;
        for key in keys {
            let above = (key.0.clone(), [key.1[0], key.1[1] + STACK_DY, key.1[2]]);
            let Some(mut upper) = r.world.machines.remove(&above) else { continue };
            if let (Some(Data::FractionTower(low)), Data::FractionTower(high)) = (r.world.machines.get_mut(&key).map(|m| &mut m.data), &mut upper.data) {
                changed |= FractionTower::stack(low, high);
            }
            r.world.machines.insert(above, upper);
        }
        if changed {
            r.dirty = true;
        }
    }

    /// Boilers and cokers take heat from the source under them (a firebox:
    /// `tryPullHeat`), before they run.
    pub(super) fn heat_tick(&mut self) {
        let Session::Ready(r) = &mut self.session else { return };
        let keys: Vec<Key> = r
            .world
            .machines
            .iter()
            .filter(|(k, m)| r.loaded.get(*k) == Some(&true) && matches!(m.data, Data::Boiler(_) | Data::Coker(_)))
            .map(|(k, _)| k.clone())
            .collect();
        for key in keys {
            let under = (key.0.clone(), [key.1[0], key.1[1] - 1, key.1[2]]);
            let source = match r.world.machines.get(&under).map(|m| &m.data) {
                Some(Data::Firebox(f)) => Some(f.heat_stored()),
                _ => None,
            };
            let taken = match r.world.machines.get_mut(&key).map(|m| &mut m.data) {
                Some(Data::Boiler(b)) => b.pull_heat(source),
                Some(Data::Coker(c)) => c.pull_heat(source),
                _ => 0,
            };
            if taken > 0
                && let Some(Data::Firebox(f)) = r.world.machines.get_mut(&under).map(|m| &mut m.data)
            {
                f.use_heat(taken);
            }
        }
    }
}
