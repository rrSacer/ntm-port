//! Fluid networks on the server (docs/fluidos.md): per dimension and fluid
//! type, the machines that ducts of that type join (a bus), and the pairs
//! of machines that touch (a port on a port), rebuilt when ducts, machines
//! or tank types change; then, each tick, `fluid::distribute` in each.
use super::*;
use crate::{energy::Port, fluid, machine::Data};

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

type Members = BTreeMap<Pos, Vec<Port>>;
/// Each machine's proxy cells, by its core.
type Proxies = BTreeMap<Pos, Vec<Pos>>;
/// A fluid to build: (dimension, fluid), its ducts, machines and proxies.
type Job = ((String, String), BTreeSet<Pos>, Members, Proxies);

struct Rebuild {
    sig: u64,
    /// (fluid, its ducts, its machines, their proxy cells).
    todo: Vec<Job>,
    done: Vec<(String, String, Vec<Vec<Pos>>)>,
}

/// Cells and machines a tick's share of a rebuild walks (at least one
/// fluid a tick): a world with many machines that take any fluid has a few
/// hundred per fluid, and building every fluid at once took 30M of the
/// tick's 50M of fuel.
pub const REBUILD_CELLS_PER_TICK: usize = 600;

impl Server {
    pub(super) fn fluids_tick(&mut self) {
        // The networks are checked for changes every 10 ticks (a duct put
        // down or retyped joins within half a second); between checks the
        // cached ones move fluid.
        if self.tick.is_multiple_of(10) || self.fluidnet.sig == 0 && self.fluidnet.pending.is_none() {
            self.fluid_shape();
        }
        self.fluid_rebuild_step();
        let r = &mut self.ready;
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
                fluid::distribute(f, &mut group);
            }
        }
        if self.tick.is_multiple_of(10) {
            self.gascent_cascades();
        }
    }

    /// What shapes the networks (loaded ducts and their type, loaded fluid
    /// machines and the types they hold), hashed without building strings;
    /// the networks are rebuilt when it changed.
    fn fluid_shape(&mut self) {
        let r = &mut self.ready;
        let mut h: u64 = 0x6e74_6d66;
        let mut feed = |v: u64| h = crate::hash::mix(h ^ v);
        let mut ducts: BTreeMap<(String, String), BTreeSet<Pos>> = BTreeMap::new();
        // A fluid's networks join only the machines that hold it, take any
        // fluid or would take it now (an empty barrel): one with nothing to
        // do with it never passes it on (machines only touch, ducts carry).
        // Built with every machine of the dimension, each fluid's networks
        // walked them all: 32M of fuel in a world of 28 machines.
        let mut users: BTreeMap<String, BTreeMap<Pos, Vec<Port>>> = BTreeMap::new();
        let mut proxies_of: BTreeMap<String, BTreeMap<Pos, Vec<Pos>>> = BTreeMap::new();
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
            // A multiblock's proxy cells are more of its ports
            // (container_bridge): a duct or a machine touching one reaches it.
            let proxies = super::container_bridge::fluid_proxy_cells(&m.block, *pos, m.facing.as_deref());
            let Some(node) = m.logic_mut().fluid() else { continue };
            feed(at.rotate_left(7) ^ crate::hash::fnv64(dim.as_bytes()));
            if node.accepts_any() {
                feed(0x5a5a_a5a5);
            }
            for t in node.fluid_types() {
                feed(crate::hash::fnv64(t.as_bytes()));
                kinds.insert((dim.clone(), t));
            }
            users.entry(dim.clone()).or_default().insert(*pos, ports);
            if !proxies.is_empty() {
                proxies_of.entry(dim.clone()).or_default().insert(*pos, proxies);
            }
        }
        // Who is in each fluid's networks, read at every check (cheap) and
        // part of the shape: an empty barrel set to take joins at the next.
        let mut members: Vec<((String, String), Members)> = Vec::new();
        for (dim, f) in &kinds {
            let mut m: BTreeMap<Pos, Vec<Port>> = BTreeMap::new();
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
                    let all = proxies_of.get(&dim);
                    let proxies = machines.keys().filter_map(|p| all.and_then(|a| a.get(p)).map(|c| (*p, c.clone()))).collect();
                    ((dim, f), cables, machines, proxies)
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
        while let Some(((dim, f), cables, machines, proxies)) = p.todo.pop() {
            walked += cables.len() + machines.len() + proxies.values().map(Vec::len).sum::<usize>();
            let nets = fluid_networks(&cables, &machines, &proxies);
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
        let r = &mut self.ready;
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
            let cascaded = match back {
                Some(b) => {
                    let mut bm = r.world.machines.remove(b).expect("checked");
                    let Data::GasCent(gb) = &mut bm.data else { unreachable!() };
                    let cascaded = crate::gascent::cascade(ga, gb);
                    r.world.machines.insert(b.clone(), bm);
                    cascaded
                }
                None => false,
            };
            if !cascaded {
                crate::gascent::end_of_line(ga);
            }
            r.world.machines.insert(key, a);
        }
    }
}

/// One fluid's networks: each bus of ducts with every machine touching it
/// (by a port at its core or by one of its proxy cells), and each pair of
/// machines touching each other. A machine passes nothing on between two
/// others that touch it, as in the NTM, where machines only touch and ducts
/// carry: built as one graph, a barrel on one port of the cracker filled
/// another barrel on its other port.
pub fn fluid_networks(ducts: &BTreeSet<Pos>, machines: &Members, proxies: &Proxies) -> Vec<Vec<Pos>> {
    use crate::energy::DIRS;
    let add = |p: Pos, d: Pos| [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
    // Every cell a machine answers at, and the machine.
    let mut owner: BTreeMap<Pos, Pos> = BTreeMap::new();
    for &core in machines.keys() {
        owner.insert(core, core);
        for &c in proxies.get(&core).into_iter().flatten() {
            owner.entry(c).or_insert(core);
        }
    }
    // A machine's core is open where its ports are; its proxies all round.
    let open = |cell: Pos, d: Pos| match machines.get(&cell) {
        Some(ports) => ports.iter().any(|port| port.contains(&d)),
        None => true,
    };
    let mut nets: Vec<Vec<Pos>> = Vec::new();
    // Buses.
    let mut seen: BTreeSet<Pos> = BTreeSet::new();
    for &start in ducts {
        if !seen.insert(start) {
            continue;
        }
        let mut members: BTreeSet<Pos> = BTreeSet::new();
        let mut queue = std::collections::VecDeque::from([start]);
        while let Some(q) = queue.pop_front() {
            for d in DIRS {
                let n = add(q, d);
                if ducts.contains(&n) {
                    if seen.insert(n) {
                        queue.push_back(n);
                    }
                } else if let Some(&m) = owner.get(&n)
                    && open(n, [-d[0], -d[1], -d[2]])
                {
                    members.insert(m);
                }
            }
        }
        if members.len() > 1 {
            nets.push(members.into_iter().collect());
        }
    }
    // Machines touching.
    let mut pairs: BTreeSet<(Pos, Pos)> = BTreeSet::new();
    for (&cell, &a) in &owner {
        for d in DIRS {
            let n = add(cell, d);
            if let Some(&b) = owner.get(&n)
                && a != b
                && open(cell, d)
                && open(n, [-d[0], -d[1], -d[2]])
            {
                pairs.insert((a.min(b), a.max(b)));
            }
        }
    }
    nets.extend(pairs.into_iter().map(|(a, b)| vec![a, b]));
    nets
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::energy::DIRS;

    fn all_round() -> Vec<Port> {
        vec![DIRS.to_vec()]
    }

    /// A barrel on each of two ports of a machine: each reaches the machine,
    /// never the other barrel (in game the oil of one filled the other).
    #[test]
    fn two_barrels_on_one_machine_never_reach_each_other() {
        let core = [0, 0, 0];
        let proxies = BTreeMap::from([(core, vec![[-1, 0, 0], [1, 0, 0]])]);
        let (a, b) = ([-2, 0, 0], [2, 0, 0]);
        let machines: Members = [(core, all_round()), (a, all_round()), (b, all_round())].into_iter().collect();
        let nets = fluid_networks(&BTreeSet::new(), &machines, &proxies);
        assert!(nets.contains(&vec![a, core]) && nets.contains(&vec![core, b]), "{nets:?}");
        assert!(!nets.iter().any(|n| n.contains(&a) && n.contains(&b)), "{nets:?}");
    }

    /// A duct carries: every machine on it shares one network, the machine
    /// by its proxy cell; a closed side of a core takes no duct.
    #[test]
    fn a_duct_joins_what_touches_it() {
        let core = [0, 0, 0];
        let proxies = BTreeMap::from([(core, vec![[1, 0, 0]])]);
        let (a, shut) = ([4, 0, 0], [0, 0, 2]);
        let ducts: BTreeSet<Pos> = [[2, 0, 0], [3, 0, 0], [0, 0, 1]].into_iter().collect();
        let machines: Members = [(core, vec![vec![[0, -1, 0]]]), (a, all_round()), (shut, vec![vec![[1, 0, 0]]])].into_iter().collect();
        let nets = fluid_networks(&ducts, &machines, &proxies);
        assert!(nets.contains(&vec![core, a]), "{nets:?}");
        assert!(!nets.iter().any(|n| n.contains(&shut)), "{nets:?}");
    }
}
