//! The foundry on the server (docs/maquinas.md, "Fundição"): what needs
//! the blocks around a machine.
//!
//! - Each loaded crucible pulls heat from the machine right under its core
//!   (a firebox: the NTM's proxies don't pass heat), melts and alloys, and
//!   pours its two streams straight down from two blocks beside its core:
//!   the alloy's outputs on its facing side, the rest on the other
//!   (`TileEntityCrucible.java:165-224`). A stream falls up to 6 blocks onto
//!   the first block in its way; if that one takes no metal (or there is
//!   none), nothing pours and nothing is lost.
//! - Channels pass their metal on every 5 ticks: all of it into a side
//!   neighbour that takes it and isn't a channel, else levelling with the
//!   neighbouring channels. Tanks fill the tank under them first, then a
//!   side neighbour, then level with tanks (`TileEntityFoundry{Channel,
//!   Tank}.java`).
//! - An outlet takes metal from behind only and pours it into what is
//!   under it, up to 4 blocks down (`TileEntityFoundryOutlet.java`).
//! - Molds take metal from above and from the sides, basins from above
//!   (`crate::foundry::cast`; they cool on their own tick).
use super::*;
use crate::foundry::{
    Acceptor, HeatSource, Tables,
    crucible::Stream,
    flow::{self, CHANNEL_CAP, CHANNEL_EVERY, Melt, SIDES, TANK_CAP},
};
use crate::machine::Data;
use std::collections::BTreeMap;

/// How far a crucible's stream falls (`pourFullStack(..., 6, ...)`), and
/// an outlet's (from `y − 0.125` to `y − 3.875`).
const CRUCIBLE_FALL: i32 = 6;
const OUTLET_FALL: i32 = 4;

type Machines = BTreeMap<Key, crate::machine::Machine>;

fn add(p: Pos, d: [i32; 3]) -> Pos {
    [p[0] + d[0], p[1] + d[1], p[2] + d[2]]
}

/// The first block a stream from `from` hits within `fall` blocks down
/// (a machine, or anything that isn't air).
fn below(m: &Machines, host: &mut impl ServerHost, dim: &str, from: Pos, fall: i32) -> Option<Key> {
    (0..=fall).map(|k| (dim.to_string(), [from[0], from[1] - k, from[2]])).find(|p| {
        m.contains_key(p) || host.get_block(&p.0, p.1).is_none_or(|b| blockstate::id_of(&b) != "minecraft:air")
    })
}

/// No channel connected to `key` holds another material (the
/// `FoundryNetwork` check of `canAcceptPartialPour`).
fn network_takes(m: &Machines, key: &Key, mat: &str) -> bool {
    let mut seen = std::collections::BTreeSet::new();
    let mut todo = vec![key.clone()];
    while let Some(k) = todo.pop() {
        if !seen.insert(k.clone()) {
            continue;
        }
        let Some(Data::Channel(c)) = m.get(&k).map(|x| &x.data) else { continue };
        if c.melt.mat.as_deref().is_some_and(|x| x != mat) {
            return false;
        }
        for d in SIDES {
            todo.push((k.0.clone(), add(k.1, d)));
        }
    }
    true
}

/// Metal falling onto `key` from above (`canAcceptPartialPour` + `pour`):
/// what is left, or `None` when it takes none.
fn accept_pour(m: &mut Machines, t: &Tables, key: &Key, mat: &str, q: u32) -> Option<u32> {
    if matches!(m.get(key).map(|x| &x.data), Some(Data::Channel(_))) && !network_takes(m, key, mat) {
        return None;
    }
    match &mut m.get_mut(key)?.data {
        Data::Channel(c) if c.melt.check(mat, CHANNEL_CAP) => Some(c.melt.add(mat, q, CHANNEL_CAP)),
        Data::FoundryTank(c) if c.melt.check(mat, TANK_CAP) => Some(c.melt.add(mat, q, TANK_CAP)),
        Data::Crucible(c) => c.pour(t, mat, q),
        Data::Mold(c) | Data::Basin(c) => c.accept(t, mat, q),
        _ => None,
    }
}

/// Metal flowing into `key` from its side `from` (the direction from `key`
/// to the source): `canAcceptPartialFlow` + `flow`.
fn accept_flow(m: &mut Machines, host: &mut impl ServerHost, t: &Tables, key: &Key, from: [i32; 3], mat: &str, q: u32) -> Option<u32> {
    let x = m.get(key)?;
    let (facing, outlet) = (x.facing.clone(), matches!(x.data, Data::Outlet(_)));
    if outlet {
        let Some(Data::Outlet(o)) = m.get(key).map(|x| &x.data) else { return None };
        // From behind only (the source is against its facing).
        let behind = facing.as_deref().and_then(registry::facing_dir).is_some_and(|f| from == [-f[0], -f[1], -f[2]]);
        if !o.passes(mat) || !behind {
            return None;
        }
        let target = below(m, host, &key.0, [key.1[0], key.1[1] - 1, key.1[2]], OUTLET_FALL - 1)?;
        return accept_pour(m, t, &target, mat, q);
    }
    match &mut m.get_mut(key)?.data {
        Data::Channel(c) if c.melt.check(mat, CHANNEL_CAP) => Some(c.melt.add(mat, q, CHANNEL_CAP)),
        Data::FoundryTank(c) if c.melt.check(mat, TANK_CAP) => Some(c.melt.add(mat, q, TANK_CAP)),
        // A mold block takes metal from the sides too; a basin doesn't.
        Data::Mold(c) => c.accept(t, mat, q),
        _ => None,
    }
}

/// A crucible stream's target as an [`Acceptor`].
struct Target<'a> {
    m: &'a mut Machines,
    key: Key,
}

impl Acceptor for Target<'_> {
    fn pour(&mut self, t: &Tables, mat: &str, q: u32) -> Option<u32> {
        accept_pour(self.m, t, &self.key, mat, q)
    }
}

fn melt_of(d: &mut Data) -> Option<(&mut Melt, u32)> {
    match d {
        Data::Channel(c) => Some((&mut c.melt, CHANNEL_CAP)),
        Data::FoundryTank(c) => Some((&mut c.melt, TANK_CAP)),
        _ => None,
    }
}

impl Server {
    pub(super) fn foundry_tick(&mut self, host: &mut impl ServerHost) {
        let tick = self.tick;
        let t = Tables::ntm();
        let Session::Ready(r) = &mut self.session else { return };
        let mine: Vec<(Key, u8)> = r
            .world
            .machines
            .iter()
            .filter(|(k, _)| r.loaded.get(*k) == Some(&true))
            .filter_map(|(k, m)| match m.data {
                Data::Crucible(_) => Some((k.clone(), 0)),
                Data::Channel(_) => Some((k.clone(), 1)),
                Data::FoundryTank(_) => Some((k.clone(), 2)),
                _ => None,
            })
            .collect();
        let mut changed = false;
        for (key, kind) in mine {
            changed |= match kind {
                0 => crucible_tick(&mut r.world.machines, host, t, &key, tick),
                1 => channel_tick(&mut r.world.machines, host, t, &key, tick),
                _ => tank_tick(&mut r.world.machines, host, t, &key, tick),
            };
        }
        if changed {
            r.dirty = true;
        }
    }
}

fn crucible_tick(m: &mut Machines, host: &mut impl ServerHost, t: &Tables, key: &Key, tick: u64) -> bool {
    let Some(mut me) = m.remove(key) else { return false };
    let facing = me.facing.clone();
    let Data::Crucible(c) = &mut me.data else {
        m.insert(key.clone(), me);
        return false;
    };
    let under = (key.0.clone(), [key.1[0], key.1[1] - 1, key.1[2]]);
    let mut changed = match m.get_mut(&under).map(|b| &mut b.data) {
        Some(Data::Firebox(f)) => c.step(t, Some(f as &mut dyn HeatSource), tick),
        _ => c.step(t, None, tick),
    };
    // The jets show what poured this tick only.
    c.jets = [0; 2];
    if let Some(d) = facing.as_deref().and_then(registry::facing_dir) {
        for (stream, s) in [(Stream::Recipe, 1), (Stream::Waste, -1)] {
            let from = [key.1[0] + 2 * d[0] * s, key.1[1], key.1[2] + 2 * d[2] * s];
            let Some(target) = below(m, host, &key.0, from, CRUCIBLE_FALL) else { continue };
            if c.pour_stream(t, stream, &mut Target { m, key: target }).is_some() {
                changed = true;
            }
        }
    }
    m.insert(key.clone(), me);
    changed
}

fn channel_tick(m: &mut Machines, host: &mut impl ServerHost, t: &Tables, key: &Key, tick: u64) -> bool {
    let Some(mut me) = m.remove(key) else { return false };
    let Data::Channel(c) = &mut me.data else {
        m.insert(key.clone(), me);
        return false;
    };
    let mut changed = false;
    c.next = c.next.saturating_sub(1);
    if c.next == 0
        && c.melt.amount > 0
        && let Some(mat) = c.melt.mat.clone()
    {
        c.next = CHANNEL_EVERY;
        let order = flow::side_order(key.1, tick, c.last_flow);
        // All of it into a neighbour that takes it and isn't a channel.
        let mut done = false;
        for &s in &order {
            let n = (key.0.clone(), add(key.1, SIDES[s]));
            if matches!(m.get(&n).map(|x| &x.data), Some(Data::Channel(_))) {
                continue;
            }
            let back = SIDES[s].map(|v| -v);
            if let Some(left) = accept_flow(m, host, t, &n, back, &mat, c.melt.amount) {
                c.melt.amount = left;
                if left == 0 {
                    c.melt.mat = None;
                }
                done = true;
                changed = true;
                break;
            }
        }
        // Else level with the channels around.
        if !done {
            for &s in &order {
                let n = (key.0.clone(), add(key.1, SIDES[s]));
                let Some(Data::Channel(o)) = m.get_mut(&n).map(|x| &mut x.data) else { continue };
                if !(o.melt.mat.is_none() || o.melt.mat == c.melt.mat || o.melt.amount == 0) {
                    continue;
                }
                let swap = flow::one_in(key.1, tick, s as u64, 5) || c.melt.amount == 1;
                let before = (c.melt.amount, o.melt.amount);
                flow::level(&mut c.melt, &mut o.melt, swap);
                // It came from here: tried last when it moves on.
                o.last_flow = (s ^ 1) as u8 + 1;
                changed |= before != (c.melt.amount, o.melt.amount);
            }
        }
    }
    if c.melt.amount == 0 {
        c.last_flow = 0;
        c.next = CHANNEL_EVERY;
    }
    m.insert(key.clone(), me);
    changed
}

fn tank_tick(m: &mut Machines, host: &mut impl ServerHost, t: &Tables, key: &Key, tick: u64) -> bool {
    let Some(mut me) = m.remove(key) else { return false };
    let Data::FoundryTank(c) = &mut me.data else {
        m.insert(key.clone(), me);
        return false;
    };
    if c.melt.mat.is_none() {
        c.melt.amount = 0;
    }
    let mut changed = false;
    c.next = c.next.saturating_sub(1);
    if c.next == 0
        && c.melt.amount > 0
        && let Some(mat) = c.melt.mat.clone()
    {
        c.next = 5 + flow::roll(key.1, tick, 7, 6) as u8;
        // The tank under it first.
        let under = (key.0.clone(), [key.1[0], key.1[1] - 1, key.1[2]]);
        let mut done = false;
        if let Some(Data::FoundryTank(o)) = m.get_mut(&under).map(|x| &mut x.data)
            && (o.melt.mat.is_none() || o.melt.mat.as_deref() == Some(&mat))
            && o.melt.amount < TANK_CAP
        {
            o.melt.mat = Some(mat.clone());
            let n = c.melt.amount.min(TANK_CAP - o.melt.amount);
            c.melt.amount -= n;
            o.melt.amount += n;
            done = true;
        }
        let order = flow::side_order(key.1, tick, 0);
        if !done {
            for &s in &order {
                let n = (key.0.clone(), add(key.1, SIDES[s]));
                if matches!(m.get(&n).map(|x| &x.data), Some(Data::Channel(_))) {
                    continue;
                }
                if let Some(left) = accept_flow(m, host, t, &n, SIDES[s].map(|v| -v), &mat, c.melt.amount) {
                    c.melt.amount = left;
                    if left == 0 {
                        c.melt.mat = None;
                    }
                    done = true;
                    break;
                }
            }
        }
        if !done {
            for &s in &order {
                let n = (key.0.clone(), add(key.1, SIDES[s]));
                if let Some(d) = m.get_mut(&n).map(|x| &mut x.data)
                    && let Data::FoundryTank(_) = d
                    && let Some((o, _)) = melt_of(d)
                {
                    flow::level(&mut c.melt, o, flow::one_in(key.1, tick, s as u64, 5));
                }
            }
        }
        changed = true;
    }
    m.insert(key.clone(), me);
    changed
}
