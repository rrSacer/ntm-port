//! Fluid networks on the server (docs/fluidos.md): per dimension and fluid
//! type, the machines that ducts of that type (or their own touching ports)
//! join, rebuilt when ducts, machines or tank types change; then, each tick,
//! `fluid::distribute` in each network.
use super::*;
use crate::{energy::Topology, fluid, machine::Data};

/// The fluid networks of the world in play, and what they were built from.
#[derive(Default)]
pub struct FluidNet {
    sig: u64,
    /// (dimension, fluid, machines of each network).
    nets: Vec<(String, String, Vec<Vec<Pos>>)>,
}

impl FluidNet {
    pub fn networks(&self) -> usize {
        self.nets.iter().map(|n| n.2.len()).sum()
    }

}

impl Server {
    pub(super) fn fluids_tick(&mut self) {
        // The networks are checked for changes every 10 ticks (a duct put
        // down or retyped joins within half a second); between checks the
        // cached ones move fluid.
        if self.tick.is_multiple_of(10) || self.fluidnet.sig == 0 {
            self.fluid_shape();
        }
        let Session::Ready(r) = &mut self.session else { return };
        let mut moved = 0;
        for (dim, f, nets) in &self.fluidnet.nets {
            for net in nets {
                let mut group: Vec<&mut dyn fluid::FluidNode> = r
                    .world
                    .machines
                    .iter_mut()
                    .filter(|(k, _)| k.0 == *dim && net.binary_search(&k.1).is_ok())
                    .filter(|(k, _)| r.loaded.get(*k) == Some(&true))
                    .filter_map(|(_, m)| m.logic_mut().fluid())
                    .collect();
                moved += fluid::distribute(f, &mut group);
            }
        }
        if moved > 0 {
            r.dirty = true;
        }
        if self.tick.is_multiple_of(10) {
            self.gascent_cascades();
        }
    }

    /// What shapes the networks (loaded ducts and their type, loaded fluid
    /// machines and the types they hold), hashed without building strings;
    /// the networks are rebuilt when it changed.
    fn fluid_shape(&mut self) {
        let Session::Ready(r) = &mut self.session else { return };
        let mut h: u64 = 0x6e74_6d66;
        let mut feed = |v: u64| h = crate::hash::mix(h ^ v);
        let mut ducts: BTreeMap<(String, String), BTreeSet<Pos>> = BTreeMap::new();
        let mut users: BTreeMap<String, BTreeMap<Pos, Vec<crate::energy::Port>>> = BTreeMap::new();
        let mut kinds: BTreeSet<(String, String)> = BTreeSet::new();
        for ((dim, pos), m) in r.world.machines.iter_mut() {
            if r.loaded.get(&(dim.clone(), *pos)) != Some(&true) {
                continue;
            }
            let at = (pos[0] as u64) ^ ((pos[1] as u64) << 21) ^ ((pos[2] as u64) << 42);
            if let Data::Duct(d) = &m.data {
                if !d.fluid.is_empty() {
                    feed(at ^ crate::hash::fnv64(d.fluid.as_bytes()) ^ crate::hash::fnv64(dim.as_bytes()));
                    ducts.entry((dim.clone(), d.fluid.clone())).or_default().insert(*pos);
                    kinds.insert((dim.clone(), d.fluid.clone()));
                }
                continue;
            }
            let Some(spec) = registry::spec(&m.block) else { continue };
            let ports = registry::fluid_ports(spec, m.facing.as_deref());
            if ports.is_empty() {
                continue;
            }
            let Some(node) = m.logic_mut().fluid() else { continue };
            feed(at.rotate_left(7) ^ crate::hash::fnv64(dim.as_bytes()));
            for t in node.fluid_types() {
                feed(crate::hash::fnv64(t.as_bytes()));
                kinds.insert((dim.clone(), t));
            }
            users.entry(dim.clone()).or_default().insert(*pos, ports);
        }
        let sig = h | 1;
        if sig != self.fluidnet.sig {
            self.fluidnet.sig = sig;
            self.fluidnet.nets = kinds
                .into_iter()
                .map(|(dim, f)| {
                    let cables = ducts.get(&(dim.clone(), f.clone())).cloned().unwrap_or_default();
                    let machines = users.get(&dim).cloned().unwrap_or_default();
                    let nets = Topology::build(&cables, &machines).networks;
                    (dim, f, nets)
                })
                .collect();
        }
    }

    /// Each loaded gas centrifuge passes its output to the one behind it, or
    /// ends a LEUF6 line (`TileEntityMachineGasCent.java:217-231`).
    fn gascent_cascades(&mut self) {
        let Session::Ready(r) = &mut self.session else { return };
        let cents: Vec<(Key, Option<String>)> = r
            .world
            .machines
            .iter()
            .filter(|(k, m)| matches!(m.data, Data::GasCent(_)) && r.loaded.get(*k) == Some(&true))
            .map(|(k, m)| (k.clone(), m.facing.clone()))
            .collect();
        for (key, facing) in cents {
            let behind = facing.as_deref().and_then(registry::facing_dir).map(|d| (key.0.clone(), [key.1[0] - d[0], key.1[1] - d[1], key.1[2] - d[2]]));
            let back = behind.as_ref().filter(|b| r.loaded.get(*b) == Some(&true) && matches!(r.world.machines.get(*b).map(|m| &m.data), Some(Data::GasCent(_))));
            let Some(mut a) = r.world.machines.remove(&key) else { continue };
            let Data::GasCent(ga) = &mut a.data else { continue };
            let changed = match back {
                Some(b) => {
                    let mut bm = r.world.machines.remove(b).expect("checked");
                    let Data::GasCent(gb) = &mut bm.data else { unreachable!() };
                    let was = (ga.output.1, gb.input.1);
                    let cascaded = crate::gascent::cascade(ga, gb);
                    let changed = (ga.output.1, gb.input.1) != was;
                    r.world.machines.insert(b.clone(), bm);
                    if cascaded { changed } else { crate::gascent::end_of_line(ga) }
                }
                None => crate::gascent::end_of_line(ga),
            };
            r.world.machines.insert(key, a);
            if changed {
                r.dirty = true;
            }
        }
    }
}
