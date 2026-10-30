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
    /// A rebuild under way: its shape, the fluids left to build (their
    /// ducts and machines) and those built. The old networks run meanwhile.
    pending: Option<Rebuild>,
}

type Members = BTreeMap<Pos, Vec<crate::energy::Port>>;

struct Rebuild {
    sig: u64,
    todo: Vec<((String, String), BTreeSet<Pos>, Members)>,
    done: Vec<(String, String, Vec<Vec<Pos>>)>,
}

/// Cells and machines a tick's share of a rebuild walks (at least one
/// fluid a tick): a world with many machines that take any fluid has a few
/// hundred per fluid, and building every fluid at once took 30M of the
/// tick's 50M of fuel.
pub const REBUILD_CELLS_PER_TICK: usize = 600;

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
        if self.tick.is_multiple_of(10) || self.fluidnet.sig == 0 && self.fluidnet.pending.is_none() {
            self.fluid_shape();
        }
        self.fluid_rebuild_step();
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
        // A fluid's networks join only the machines that hold it, take any
        // fluid or would take it now (an empty barrel): one with nothing to
        // do with it never passes it on (machines only touch, ducts carry).
        // Built with every machine of the dimension, each fluid's networks
        // walked them all: 32M of fuel in a world of 28 machines.
        let mut users: BTreeMap<String, BTreeMap<Pos, Vec<crate::energy::Port>>> = BTreeMap::new();
        let mut kinds: BTreeSet<(String, String)> = BTreeSet::new();
        let mut any: BTreeMap<String, BTreeSet<Pos>> = BTreeMap::new();
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
            // A multiblock's proxy cells carry its fluids as ducts (container_bridge).
            let proxies = super::container_bridge::fluid_proxy_cells(&m.block, *pos, m.facing.as_deref());
            let Some(node) = m.logic_mut().fluid() else { continue };
            feed(at.rotate_left(7) ^ crate::hash::fnv64(dim.as_bytes()));
            if node.accepts_any() && !proxies.is_empty() {
                feed(0x5a5a_a5a5);
                any.entry(dim.clone()).or_default().extend(proxies.iter().copied());
            }
            for t in node.fluid_types() {
                feed(crate::hash::fnv64(t.as_bytes()));
                if !proxies.is_empty() {
                    ducts.entry((dim.clone(), t.clone())).or_default().extend(proxies.iter().copied());
                }
                kinds.insert((dim.clone(), t));
            }
            users.entry(dim.clone()).or_default().insert(*pos, ports);
        }
        // Machines that take any fluid: their proxies in every network of
        // their dimension.
        for (dim, f) in &kinds {
            if let Some(cells) = any.get(dim) {
                ducts.entry((dim.clone(), f.clone())).or_default().extend(cells.iter().copied());
            }
        }
        // Who is in each fluid's networks, read at every check (cheap) and
        // part of the shape: an empty barrel set to take joins at the next.
        let mut members: Vec<((String, String), Members)> = Vec::new();
        for (dim, f) in &kinds {
            let mut m: BTreeMap<Pos, Vec<crate::energy::Port>> = BTreeMap::new();
            for (p, ports) in users.get(dim).into_iter().flatten() {
                let Some(node) = r.world.machines.get_mut(&(dim.clone(), *p)).and_then(|m| m.logic_mut().fluid()) else { continue };
                if node.accepts_any() || node.fluid_demand(f) > 0 || node.fluid_offer(f) > 0 || node.fluid_types().contains(f) {
                    feed(((p[0] as u64) ^ ((p[1] as u64) << 21) ^ ((p[2] as u64) << 42)).rotate_left(13) ^ crate::hash::fnv64(f.as_bytes()));
                    m.insert(*p, ports.clone());
                }
            }
            members.push(((dim.clone(), f.clone()), m));
        }
        let sig = h | 1;
        let target = self.fluidnet.pending.as_ref().map_or(self.fluidnet.sig, |p| p.sig);
        if sig != target {
            let todo = members
                .into_iter()
                .map(|((dim, f), machines)| {
                    let cables = ducts.remove(&(dim.clone(), f.clone())).unwrap_or_default();
                    ((dim, f), cables, machines)
                })
                .collect();
            self.fluidnet.pending = Some(Rebuild { sig, todo, done: Vec::new() });
        }
    }

    /// Builds the networks of a few fluids of the pending rebuild; when all
    /// are built, they replace the old ones.
    fn fluid_rebuild_step(&mut self) {
        let Some(p) = &mut self.fluidnet.pending else { return };
        let mut walked = 0;
        while let Some(((dim, f), cables, machines)) = p.todo.pop() {
            walked += cables.len() + machines.len();
            let nets = Topology::build(&cables, &machines).networks;
            p.done.push((dim, f, nets));
            if walked >= REBUILD_CELLS_PER_TICK {
                break;
            }
        }
        if p.todo.is_empty() {
            let p = self.fluidnet.pending.take().expect("pending");
            self.fluidnet.sig = p.sig;
            self.fluidnet.nets = p.done;
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
