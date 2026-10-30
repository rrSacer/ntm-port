//! Batch 4's world inputs. Mirrors and boilers run in the world's stable
//! machine order: a mirror after its boiler leaves heat for the next tick.
use super::*;
use crate::{machine::Data, solar_stirling::SolarStirling};

impl Server {
    pub(super) fn solar_tick(&mut self, host: &mut impl ServerHost) {
        let r = &mut self.ready;
        let keys: Vec<Key> = r
            .world
            .machines
            .iter()
            .filter(|(k, m)| {
                r.loaded.get(*k) == Some(&true) && matches!(m.data, Data::SolarStirling(_))
            })
            .map(|(k, _)| k.clone())
            .collect();
        for key in keys {
            let Some(Data::SolarStirling(s)) = r.world.machines.get_mut(&key).map(|m| &mut m.data)
            else {
                continue;
            };
            match &mut **s {
                SolarStirling::Boiler(b) => b.step(),
                SolarStirling::Mirror(m) => {
                    let sun = m.sun(key.1[1], host.solar_sample(&key.0, key.1));
                    let target = (key.0.clone(), [m.target[0], m.target[1] - 1, m.target[2]]);
                    if sun > 0
                        && r.loaded.get(&target) == Some(&true)
                        && let Some(Data::SolarStirling(s)) =
                            r.world.machines.get_mut(&target).map(|m| &mut m.data)
                        && let SolarStirling::Boiler(b) = &mut **s
                    {
                        b.heat = b.heat.wrapping_add(sun);
                    }
                }
                SolarStirling::Stirling(_) => {}
            }
        }
    }
    pub(super) fn stirling_finish(&mut self) {
        let r = &mut self.ready;
        for (k, m) in &mut r.world.machines {
            if r.loaded.get(k) == Some(&true)
                && let Data::SolarStirling(s) = &mut m.data
                && let SolarStirling::Stirling(s) = &mut **s
            {
                s.finish();
            }
        }
    }
    /// The mirror tool stores the core one block up, even when a dummy was clicked.
    pub fn native_solar_target(&self, dim: &str, core: Pos) -> Option<Pos> {
        let m = self.ready.world.machines.get(&(dim.into(), core))?;
        matches!(&m.data, Data::SolarStirling(s) if matches!(&**s, SolarStirling::Boiler(_)))
            .then_some([core[0], core[1] + 1, core[2]])
    }
    pub fn native_mirror_link(
        &mut self,
        dim: &str,
        at: Pos,
        target: Pos,
    ) -> Option<Result<(), &'static str>> {
        let m = self.ready.world.machines.get_mut(&(dim.into(), at))?;
        let Data::SolarStirling(s) = &mut m.data else {
            return None;
        };
        let SolarStirling::Mirror(m) = &mut **s else {
            return None;
        };
        Some(m.link(at, target))
    }
    pub fn native_stirling_gear(&self, dim: &str, core: Pos) -> Option<&'static str> {
        let m = self.ready.world.machines.get(&(dim.into(), core))?;
        let Data::SolarStirling(s) = &m.data else {
            return None;
        };
        let SolarStirling::Stirling(s) = &**s else {
            return None;
        };
        if s.has_cog { None } else { s.shape.gear() }
    }
    /// Called only after taking one matching gear from the acting hand.
    pub fn native_stirling_repair(&mut self, dim: &str, core: Pos, gear: &str) -> bool {
        if self.native_stirling_gear(dim, core) != Some(gear) {
            return false;
        }
        let Data::SolarStirling(s) = &mut self
            .ready
            .world
            .machines
            .get_mut(&(dim.into(), core))
            .expect("checked")
            .data
        else {
            unreachable!()
        };
        let SolarStirling::Stirling(s) = &mut **s else {
            unreachable!()
        };
        s.has_cog = true;
        true
    }
}
