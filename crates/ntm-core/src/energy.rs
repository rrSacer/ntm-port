//! HE networks (docs/fundacao.md, "Energia"), as the NTM's `TilePort`
//! nodes (`UniNodespace`) join them: every cable is a node open on all six
//! faces, every machine has one or more **port** nodes, each open on some
//! faces. Two nodes side by side join when each is open toward the other
//! (`checkConnection`): a cable and a port facing it, or two ports facing
//! each other. A network is a connected set of nodes; a machine is in every
//! network one of its ports is in. A machine whose single port is open all
//! round (the electric furnace and press, `TilePortShapes.around`) joins
//! everything around it, so a row of them conducts; one whose ports are
//! separate (the battery's four sides) never passes energy across itself.
//!
//! Each tick a network moves energy from what offers it to what wants it,
//! in a fixed order: providers to consumers, providers' surplus to storage,
//! storage to consumers' remaining demand. Within a group the share is
//! proportional to demand, so no machine starves another by position.

use crate::Pos;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Generates: only offers.
    Provider,
    /// Only takes.
    Consumer,
    /// Takes and offers (batteries), as its mode allows this tick.
    Storage,
}

/// A machine as a network node.
pub trait EnergyNode {
    fn role(&self) -> Role;
    /// HE it can give this tick.
    fn offer(&self) -> u64;
    /// HE it can take this tick.
    fn demand(&self) -> u64;
    fn give(&mut self, he: u64);
    fn take(&mut self, he: u64);
}

/// The six directions, as offsets.
pub const DIRS: [Pos; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];

fn add(p: Pos, d: Pos) -> Pos {
    [p[0] + d[0], p[1] + d[1], p[2] + d[2]]
}
fn neg(d: Pos) -> Pos {
    [-d[0], -d[1], -d[2]]
}

/// A machine's port: the faces (directions) it is open on.
pub type Port = Vec<Pos>;

/// Which machines form one network, cached until cables or machines change.
#[derive(Clone, Debug, Default)]
pub struct Topology {
    /// Each network's machine positions, sorted, deduplicated: a machine
    /// may be in several.
    pub networks: Vec<Vec<Pos>>,
}

/// A node of the graph: a cable, or port `i` of the machine at a position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Node {
    Cable(Pos),
    Port(Pos, usize),
}

impl Topology {
    /// Flood fill over nodes joined face to face. `machines` holds each
    /// machine's ports (none: not on any network).
    pub fn build(cables: &BTreeSet<Pos>, machines: &BTreeMap<Pos, Vec<Port>>) -> Self {
        Self::build_at(cables, machines, &BTreeMap::new())
    }

    /// A port may sit on a multiblock's edge instead of its core. It keeps
    /// its owner's identity and outward faces; it isn't a six-face cable.
    pub fn build_at(cables: &BTreeSet<Pos>, machines: &BTreeMap<Pos, Vec<Port>>, positions: &BTreeMap<(Pos, usize), Pos>) -> Self {
        Self::build_linked(cables, machines, positions, &BTreeMap::new())
    }

    /// As [`Topology::build_at`], with wire links: `links` holds, for a machine, the machines
    /// its first port is wired to (the pylons and connectors, `TileEntityPylonBase.connected`).
    /// A link joins two first ports at any distance, and only when each lists the other (`checkConnection`).
    pub fn build_linked(
        cables: &BTreeSet<Pos>,
        machines: &BTreeMap<Pos, Vec<Port>>,
        positions: &BTreeMap<(Pos, usize), Pos>,
        links: &BTreeMap<Pos, Vec<Pos>>,
    ) -> Self {
        Self::build_conducting(cables, machines, positions, links, &BTreeSet::new())
    }

    /// As [`Topology::build_linked`], with `conductors`: machines whose ports are one node
    /// (`IEnergyConductorMK2`, the battery socket and the REDD), so what reaches one port reaches them all.
    pub fn build_conducting(
        cables: &BTreeSet<Pos>,
        machines: &BTreeMap<Pos, Vec<Port>>,
        positions: &BTreeMap<(Pos, usize), Pos>,
        links: &BTreeMap<Pos, Vec<Pos>>,
        conductors: &BTreeSet<Pos>,
    ) -> Self {
        let mut ports_at: BTreeMap<Pos, Vec<(Pos,usize)>> = BTreeMap::new();
        for (&core,ports) in machines {
            for i in 0..ports.len() { ports_at.entry(positions.get(&(core,i)).copied().unwrap_or(core)).or_default().push((core,i)); }
        }
        let open = |p: Pos, i: usize, d: Pos| machines.get(&p).is_some_and(|ports| ports[i].contains(&d));
        // The nodes joined to `n`.
        let next = |n: Node| -> Vec<Node> {
            let (p, faces): (Pos, Vec<Pos>) = match n {
                Node::Cable(p) => (p, DIRS.to_vec()),
                Node::Port(p, i) => (positions.get(&(p,i)).copied().unwrap_or(p), machines[&p][i].clone()),
            };
            let mut out = Vec::new();
            if let Node::Port(p, _) = n
                && conductors.contains(&p)
            {
                out.extend((0..machines[&p].len()).map(|j| Node::Port(p, j)));
            }
            if let Node::Port(p, 0) = n {
                for q in links.get(&p).into_iter().flatten() {
                    if machines.get(q).is_some_and(|ports| !ports.is_empty()) {
                        out.push(Node::Port(*q, 0));
                    }
                }
            }
            for d in faces {
                let q = add(p, d);
                if cables.contains(&q) {
                    out.push(Node::Cable(q));
                }
                if let Some(ports) = ports_at.get(&q) {
                    for &(core,j) in ports {
                        if open(core, j, neg(d)) { out.push(Node::Port(core, j)); }
                    }
                }
            }
            out
        };
        let mut seen: BTreeSet<Node> = BTreeSet::new();
        let mut networks = Vec::new();
        for (&p, ports) in machines {
            for i in 0..ports.len() {
                let start = Node::Port(p, i);
                if !seen.insert(start) {
                    continue;
                }
                let mut net = BTreeSet::new();
                let mut queue = VecDeque::from([start]);
                while let Some(n) = queue.pop_front() {
                    if let Node::Port(q, _) = n {
                        net.insert(q);
                    }
                    for m in next(n) {
                        if seen.insert(m) {
                            queue.push_back(m);
                        }
                    }
                }
                if net.len() > 1 {
                    networks.push(net.into_iter().collect());
                }
            }
        }
        Self { networks }
    }
}

/// One network's tick: moves energy between `nodes`. Returns the HE moved.
/// Offers and demands are read once, before anything moves, so no node
/// passes on in the same tick what it just received, and none takes more
/// than its rate allows across the three steps.
pub fn distribute(nodes: &mut [&mut dyn EnergyNode]) -> u64 {
    let mut offer: Vec<u64> = nodes.iter().map(|n| n.offer()).collect();
    let mut demand: Vec<u64> = nodes.iter().map(|n| n.demand()).collect();
    let idx = |role: Role| -> Vec<usize> {
        (0..nodes.len()).filter(|&i| nodes[i].role() == role).collect()
    };
    let providers = idx(Role::Provider);
    let consumers = idx(Role::Consumer);
    let storage = idx(Role::Storage);
    let mut moved = 0;
    for (from, to) in [
        (&providers, &consumers),
        (&providers, &storage),
        (&storage, &consumers),
        // `PowerNetMK2.update` pools every provider and every receiver, and a battery in buffer mode
        // is in both sets: one holding charge feeds another with room. A lone one would only pass
        // energy to itself.
        (&storage, &storage),
    ] {
        if from == to && from.len() < 2 {
            continue;
        }
        let amount = transfer(&mut offer, &mut demand, from, to, nodes);
        moved += amount;
    }
    moved
}

/// Moves min(offer, demand) from `from` to `to`, each side's share
/// proportional to its own remaining offer or demand.
fn transfer(
    offer: &mut [u64],
    demand: &mut [u64],
    from: &[usize],
    to: &[usize],
    nodes: &mut [&mut dyn EnergyNode],
) -> u64 {
    let total_offer: u64 = from.iter().map(|&i| offer[i]).sum();
    let total_demand: u64 = to.iter().map(|&i| demand[i]).sum();
    let amount = total_offer.min(total_demand);
    if amount == 0 {
        return 0;
    }
    for (i, part) in share(demand, to, total_demand, amount) {
        demand[i] -= part;
        nodes[i].give(part);
    }
    for (i, part) in share(offer, from, total_offer, amount) {
        offer[i] -= part;
        nodes[i].take(part);
    }
    amount
}

/// Splits `amount` over `who` by weight (`weights[i]` of `total`), the
/// rounding remainder to the first with room, so the sum is exact.
fn share(weights: &[u64], who: &[usize], total: u64, amount: u64) -> Vec<(usize, u64)> {
    let mut parts: Vec<(usize, u64, u64)> = who
        .iter()
        .map(|&i| {
            let w = weights[i];
            let part = (u128::from(amount) * u128::from(w) / u128::from(total.max(1))) as u64;
            (i, part, w)
        })
        .collect();
    let mut rest = amount - parts.iter().map(|p| p.1).sum::<u64>();
    for p in &mut parts {
        let extra = (p.2 - p.1).min(rest);
        p.1 += extra;
        rest -= extra;
    }
    parts
        .into_iter()
        .filter(|p| p.1 > 0)
        .map(|(i, part, _)| (i, part))
        .collect()
}

/// The network a cable or machine at `p` would join: for the screens.
pub fn network_of(topology: &Topology, p: Pos) -> Option<usize> {
    topology.networks.iter().position(|n| n.binary_search(&p).is_ok())
}

/// Sizes per network.
pub fn summary(topology: &Topology) -> BTreeMap<usize, usize> {
    topology
        .networks
        .iter()
        .enumerate()
        .map(|(i, n)| (i, n.len()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct N {
        role: Role,
        power: u64,
        max: u64,
        rate: u64,
    }
    impl EnergyNode for N {
        fn role(&self) -> Role {
            self.role
        }
        fn offer(&self) -> u64 {
            if self.role == Role::Consumer {
                0
            } else {
                self.power.min(self.rate)
            }
        }
        fn demand(&self) -> u64 {
            if self.role == Role::Provider {
                0
            } else {
                (self.max - self.power).min(self.rate)
            }
        }
        fn give(&mut self, he: u64) {
            self.power += he;
        }
        fn take(&mut self, he: u64) {
            self.power -= he;
        }
    }

    fn all() -> Vec<Port> {
        vec![DIRS.to_vec()]
    }
    /// The battery's four sides, each its own port.
    fn sides() -> Vec<Port> {
        vec![vec![[1, 0, 0]], vec![[-1, 0, 0]], vec![[0, 0, 1]], vec![[0, 0, -1]]]
    }

    #[test]
    fn topology_through_cables() {
        let cables: BTreeSet<Pos> = [[1, 0, 0], [2, 0, 0]].into();
        let machines: BTreeMap<Pos, Vec<Port>> = [
            ([0, 0, 0], all()),
            ([3, 0, 0], all()),
            ([10, 0, 0], all()),
            ([10, 1, 0], all()),
        ]
        .into();
        let t = Topology::build(&cables, &machines);
        assert_eq!(
            t.networks,
            vec![vec![[0, 0, 0], [3, 0, 0]], vec![[10, 0, 0], [10, 1, 0]]]
        );
    }

    #[test]
    fn ports_join_only_face_to_face_and_separate_ports_do_not_pass_through() {
        // furnace (all round) | battery (four sides) | furnace: two networks,
        // the battery in both, the furnaces never together.
        let machines: BTreeMap<Pos, Vec<Port>> =
            [([0, 0, 0], all()), ([1, 0, 0], sides()), ([2, 0, 0], all())].into();
        let t = Topology::build(&BTreeSet::new(), &machines);
        assert_eq!(t.networks, vec![vec![[0, 0, 0], [1, 0, 0]], vec![[1, 0, 0], [2, 0, 0]]]);
        // A row of all-round machines conducts, as in the NTM.
        let row: BTreeMap<Pos, Vec<Port>> =
            [([0, 0, 0], all()), ([1, 0, 0], all()), ([2, 0, 0], all())].into();
        assert_eq!(Topology::build(&BTreeSet::new(), &row).networks.len(), 1);
        // A burner open only at its back (-x): a battery at its front (+x)
        // is not joined; one at its back is.
        let back: Vec<Port> = vec![vec![[-1, 0, 0]]];
        let front: BTreeMap<Pos, Vec<Port>> = [([0, 0, 0], back.clone()), ([1, 0, 0], sides())].into();
        assert!(Topology::build(&BTreeSet::new(), &front).networks.is_empty());
        let behind: BTreeMap<Pos, Vec<Port>> = [([0, 0, 0], back), ([-1, 0, 0], sides())].into();
        assert_eq!(Topology::build(&BTreeSet::new(), &behind).networks.len(), 1);
        // Above the battery (no port up) a cable joins nothing.
        let cables: BTreeSet<Pos> = [[1, 1, 0], [1, 2, 0]].into();
        let up: BTreeMap<Pos, Vec<Port>> = [([1, 0, 0], sides()), ([1, 3, 0], all())].into();
        assert!(Topology::build(&cables, &up).networks.is_empty());
    }

    #[test]
    fn energy_is_conserved_and_ordered() {
        let mut generation = N { role: Role::Provider, power: 100, max: 100, rate: 100 };
        let mut con = N { role: Role::Consumer, power: 0, max: 1000, rate: 30 };
        let mut bat = N { role: Role::Storage, power: 0, max: 1000, rate: 50 };
        let moved = distribute(&mut [&mut generation, &mut con, &mut bat]);
        assert_eq!((generation.power, con.power, bat.power, moved), (20, 30, 50, 80));
        // No generation and a full consumer: nothing moves.
        con.max = 30;
        generation.power = 0;
        let moved = distribute(&mut [&mut generation, &mut con, &mut bat]);
        assert_eq!((con.power, bat.power, moved), (30, 50, 0));
        // Room again: the battery feeds the consumer.
        con.max = 1000;
        let moved = distribute(&mut [&mut generation, &mut con, &mut bat]);
        assert_eq!((con.power, bat.power, moved), (60, 20, 30));
    }

    /// Two batteries that both give and take exchange energy through the cable, as the Java's pooled
    /// `PowerNetMK2` does (a full one feeds an empty one); a battery in input mode only takes.
    #[test]
    fn batteries_exchange_energy_with_each_other() {
        let mut a = N { role: Role::Storage, power: 1000, max: 1000, rate: 50 };
        let mut b = N { role: Role::Storage, power: 0, max: 1000, rate: 50 };
        distribute(&mut [&mut a, &mut b]);
        assert_eq!((a.power, b.power), (950, 50));
        for _ in 0..40 {
            distribute(&mut [&mut a, &mut b]);
        }
        // Nothing is lost on the way, and the empty one kept what it got: as in the pooled Java net,
        // two batteries with equal offer and room weigh the same and stop trading.
        assert_eq!(a.power + b.power, 1000);
        assert!(b.power >= 50, "{} {}", a.power, b.power);
        // One battery alone moves nothing.
        let mut lone = N { role: Role::Storage, power: 500, max: 1000, rate: 50 };
        assert_eq!(distribute(&mut [&mut lone]), 0);
        assert_eq!(lone.power, 500);
    }

    #[test]
    fn proportional_share_sums_exactly() {
        let mut generation = N { role: Role::Provider, power: 10, max: 10, rate: 10 };
        let mut a = N { role: Role::Consumer, power: 0, max: 100, rate: 7 };
        let mut b = N { role: Role::Consumer, power: 0, max: 100, rate: 7 };
        let mut c = N { role: Role::Consumer, power: 0, max: 100, rate: 7 };
        distribute(&mut [&mut generation, &mut a, &mut b, &mut c]);
        assert_eq!(a.power + b.power + c.power, 10);
        assert_eq!(generation.power, 0);
    }

    #[test]
    fn a_wire_link_joins_two_first_ports_at_any_distance_and_only_when_listed() {
        let a = [0, 70, 0];
        let b = [90, 70, 40];
        let machines: BTreeMap<Pos, Vec<Port>> = [(a, vec![vec![]]), (b, vec![vec![]])].into_iter().collect();
        let none = BTreeMap::new();
        assert!(Topology::build_linked(&BTreeSet::new(), &machines, &BTreeMap::new(), &none).networks.is_empty());
        let links: BTreeMap<Pos, Vec<Pos>> = [(a, vec![b]), (b, vec![a])].into_iter().collect();
        let t = Topology::build_linked(&BTreeSet::new(), &machines, &BTreeMap::new(), &links);
        assert_eq!(t.networks, vec![vec![a, b]]);
        // A link to a machine that is not there, or with no port, joins nothing.
        let ghost: BTreeMap<Pos, Vec<Pos>> = [(a, vec![[5, 5, 5]])].into_iter().collect();
        assert!(Topology::build_linked(&BTreeSet::new(), &machines, &BTreeMap::new(), &ghost).networks.is_empty());
    }

    /// A wire passes energy on: a battery on a cable on one pylon reaches another across the gap.
    #[test]
    fn a_link_carries_the_net_across_and_a_pylon_conducts_to_its_cables() {
        let (p1, p2) = ([10, 70, 0], [60, 70, 0]);
        let (bat, load) = ([8, 70, 0], [62, 70, 0]);
        let cables: BTreeSet<Pos> = [[9, 70, 0], [61, 70, 0]].into_iter().collect();
        let horiz = vec![[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];
        let machines: BTreeMap<Pos, Vec<Port>> = [
            (p1, vec![horiz.clone()]),
            (p2, vec![horiz]),
            (bat, vec![vec![[1, 0, 0]]]),
            (load, vec![vec![[-1, 0, 0]]]),
        ]
        .into_iter()
        .collect();
        let links: BTreeMap<Pos, Vec<Pos>> = [(p1, vec![p2]), (p2, vec![p1])].into_iter().collect();
        let t = Topology::build_linked(&cables, &machines, &BTreeMap::new(), &links);
        assert_eq!(t.networks, vec![vec![bat, p1, p2, load]]);
        let without = Topology::build_linked(&cables, &machines, &BTreeMap::new(), &BTreeMap::new());
        assert!(without.networks.iter().all(|n| !(n.contains(&bat) && n.contains(&load))));
    }
}
