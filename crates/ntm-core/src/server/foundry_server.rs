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
//! - A slag tap is an outlet that pours onto the ground, up to 15 blocks down; what it pours is a block of slag
//!   (`slag_server`), made in one commit with the source's loss.
//! - Molds take metal from above and from the sides, basins from above
//!   (`crate::foundry::cast`; they cool on their own tick).
use super::*;
use crate::blast::visuals::Show;
use crate::foundry::{
    Acceptor, Tables,
    crucible::Stream,
    flow::{self, CHANNEL_CAP, CHANNEL_EVERY, Melt, SIDES, TANK_CAP},
};
use crate::machine::Data;
use std::collections::BTreeMap;

/// How far a crucible's stream falls (`pourFullStack(..., 6, ...)`), and
/// an outlet's (from `y − 0.125` to `y − 3.875`).
const CRUCIBLE_FALL: i32 = 6;
const OUTLET_FALL: i32 = 4;
/// The slag tap's ray, from `y − 0.125` to `y + 0.125 − 15` (`TileEntityFoundrySlagtap`).
pub(super) const SLAGTAP_FALL: i32 = 15;

pub(super) type Machines = BTreeMap<Key, crate::machine::Machine>;

fn add(p: Pos, d: [i32; 3]) -> Pos {
    [p[0] + d[0], p[1] + d[1], p[2] + d[2]]
}

/// Where a stream landed: the machine (`key`, its core) and the block
/// struck (`cell`, the core itself or one of the cells of its multiblock).
#[derive(Clone, Debug)]
pub(super) struct Hit {
    pub(super) key: Key,
    pub(super) cell: Key,
}

/// What a tick of the foundry carries between its blocks: the tables, and the pours into a slag tap that its sources
/// asked for. A pour is not made where it is asked for: it changes the source, a block and a record, which only the
/// server's commit makes together (`slag_server`).
struct Cx<'a> {
    t: &'a Tables,
    slag: Vec<SlagPour>,
}

/// A source (a channel or a tank) that handed its metal to a slag tap this tick.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct SlagPour {
    pub(super) src: Key,
    pub(super) tap: Key,
}

/// The face a falling stream strikes (`mop.sideHit` of a ray from above).
const TOP: [i32; 3] = [0, 1, 0];

/// The core of the multiblock whose pour cell is `cell` (the block a jet
/// struck): `MachineStrandCaster.findCore` for the four cells that are
/// `moltenMetal` proxies. The core is at most one block aside and two below.
pub(super) fn pour_core(m: &Machines, cell: &Key) -> Option<Key> {
    for dx in -1..=1 {
        for dz in -1..=1 {
            let core = (cell.0.clone(), [cell.1[0] + dx, cell.1[1] - 2, cell.1[2] + dz]);
            let Some(x) = m.get(&core) else { continue };
            if x.block == crate::strand_caster::BLOCK
                && x.facing.as_deref().is_some_and(|f| crate::strand_caster::takes_pour_at(core.1, f, cell.1, TOP))
            {
                return Some(core);
            }
        }
    }
    None
}

/// The first block a stream from `from` hits within `fall` blocks down
/// (a machine, a cell of one, or anything that isn't air).
pub(super) fn below(m: &Machines, host: &mut impl ServerHost, dim: &str, from: Pos, fall: i32) -> Option<Hit> {
    let cell = (0..=fall).map(|k| (dim.to_string(), [from[0], from[1] - k, from[2]])).find(|p| {
        m.contains_key(p) || host.get_block(&p.0, p.1).is_none_or(|b| blockstate::id_of(&b) != "minecraft:air")
    })?;
    let key = if m.contains_key(&cell) { cell.clone() } else { pour_core(m, &cell).unwrap_or_else(|| cell.clone()) };
    Some(Hit { key, cell })
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
        // An empty channel's network type is none (`node.type = null` at
        // amount 0, `TileEntityFoundryChannel.java:37`).
        if c.melt.amount > 0 && c.melt.mat.as_deref().is_some_and(|x| x != mat) {
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
fn accept_pour(m: &mut Machines, t: &Tables, hit: &Hit, mat: &str, q: u32) -> Option<u32> {
    let key = &hit.key;
    if matches!(m.get(key).map(|x| &x.data), Some(Data::Channel(_))) && !network_takes(m, key, mat) {
        return None;
    }
    let facing = m.get(key)?.facing.clone();
    match &mut m.get_mut(key)?.data {
        Data::Channel(c) if c.melt.check(mat, CHANNEL_CAP) => Some(c.melt.add(mat, q, CHANNEL_CAP)),
        Data::FoundryTank(c) if c.melt.check(mat, TANK_CAP) => Some(c.melt.add(mat, q, TANK_CAP)),
        Data::Crucible(c) => c.pour(t, mat, q),
        Data::Mold(c) | Data::Basin(c) => c.accept(t, mat, q),
        // The caster takes a jet on one of its four funnel cells (`hit.cell`, face UP).
        Data::StrandCaster(c) => c.pour(t, &crate::strand_caster::Jet { core: key.1, facing: facing.as_deref()?, cell: hit.cell.1, face: TOP }, mat, q),
        _ => None,
    }
}

/// Metal flowing into `key` from its side `from` (the direction from `key`
/// to the source): `canAcceptPartialFlow` + `flow`.
fn accept_flow(m: &mut Machines, host: &mut impl ServerHost, cx: &mut Cx, key: &Key, from: [i32; 3], mat: &str, q: u32) -> Option<u32> {
    let t = cx.t;
    let x = m.get(key)?;
    let (facing, outlet) = (x.facing.clone(), matches!(x.data, Data::Outlet(_)));
    // A slag tap takes metal from behind, if anything is under it to pour onto (`canAcceptPartialFlow`); the pour
    // itself is the server's. The source keeps its metal until then, so the flow ends here, as it does when the tap
    // takes it all.
    if let Data::Slagtap(o) = &x.data {
        let behind = facing.as_deref().and_then(registry::facing_dir).is_some_and(|f| from == [-f[0], -f[1], -f[2]]);
        if !o.passes(mat) || !behind {
            return None;
        }
        below(m, host, &key.0, [key.1[0], key.1[1] - 1, key.1[2]], SLAGTAP_FALL - 1)?;
        let src = (key.0.clone(), add(key.1, from));
        let pour = SlagPour { src, tap: key.clone() };
        if !cx.slag.contains(&pour) {
            cx.slag.push(pour);
        }
        return Some(q);
    }
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
    hit: Hit,
}

impl Acceptor for Target<'_> {
    fn pour(&mut self, t: &Tables, mat: &str, q: u32) -> Option<u32> {
        accept_pour(self.m, t, &self.hit, mat, q)
    }
}

pub(super) fn melt_of(d: &mut Data) -> Option<(&mut Melt, u32)> {
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
        let mut cx = Cx { t, slag: Vec::new() };
        let r = &mut self.ready;
        let mine: Vec<(Key, u8)> = r
            .world
            .machines
            .iter()
            .filter(|(k, _)| r.loaded.get(*k) == Some(&true))
            .filter_map(|(k, m)| match m.data {
                Data::Crucible(_) => Some((k.clone(), 0)),
                Data::Channel(_) => Some((k.clone(), 1)),
                Data::FoundryTank(_) => Some((k.clone(), 2)),
                Data::ArcFurnace(_) | Data::Electrolyser(_) | Data::RotaryFurnace(_) => Some((k.clone(), 3)),
                _ => None,
            })
            .collect();
        for (key, kind) in mine {
            match kind {
                0 => crucible_tick(&mut r.world.machines, host, t, &key, tick),
                1 => channel_tick(&mut r.world.machines, host, &mut cx, &key, tick),
                2 => tank_tick(&mut r.world.machines, host, &mut cx, &key, tick),
                _ => molten_tick(&mut r.world.machines, host, t, &key),
            };
        }
        for pour in cx.slag {
            self.slag_pour(host, &pour);
        }
    }
}

/// The molten metal of the arc furnace (from a spout 3 blocks ahead of its
/// core, 72 quanta a tick, the lid open: `TileEntityMachineArcFurnaceLarge`
/// :166-183) and the electrolyser (its left stack 6 blocks behind, its right
/// 6 ahead, 24 a tick each: `TileEntityElectrolyser`:201-245), onto the
/// first block up to 6 below; nothing pours with no foundry block there.
fn molten_tick(m: &mut Machines, host: &mut impl ServerHost, t: &Tables, key: &Key) -> bool {
    let Some(mut me) = m.remove(key) else { return false };
    let Some(d) = me.facing.as_deref().and_then(registry::facing_dir) else {
        m.insert(key.clone(), me);
        return false;
    };
    let at = |k: i32, up: i32| [key.1[0] + d[0] * k, key.1[1] + up, key.1[2] + d[2] * k];
    let mut changed = false;
    match &mut me.data {
        Data::ArcFurnace(f) => {
            if let Some(target) = below(m, host, &key.0, at(3, 1), 6) {
                changed |= f.pour(|mat, q| accept_pour(m, t, &target, mat, q));
            }
        }
        Data::Electrolyser(e) => {
            for (right, k) in [(false, -6), (true, 6)] {
                if let Some(target) = below(m, host, &key.0, at(k, 2), 6) {
                    changed |= e.pour(right, |mat, q| accept_pour(m, t, &target, mat, q));
                }
            }
        }
        Data::RotaryFurnace(r) => {
            let rot = [d[2], 0, -d[0]];
            let spout = [key.1[0] + rot[0] * 3, key.1[1] + 1, key.1[2] + rot[2] * 3];
            if let Some(target) = below(m, host, &key.0, spout, CRUCIBLE_FALL) {
                changed |= r.pour(|mat, q| accept_pour(m, t, &target, mat, q));
            }
        }
        _ => {}
    }
    m.insert(key.clone(), me);
    changed
}

fn crucible_tick(m: &mut Machines, host: &mut impl ServerHost, t: &Tables, key: &Key, _tick: u64) -> bool {
    let Some(mut me) = m.remove(key) else { return false };
    let facing = me.facing.clone();
    let Data::Crucible(c) = &mut me.data else {
        m.insert(key.clone(), me);
        return false;
    };
    // Any heat source under it: a firebox, the oven, a heater.
    let under = super::oil_server::heat_source_under(m, key);
    let mut changed = match under.as_ref().and_then(|u| m.get_mut(u)).and_then(|b| super::oil_server::heat_source(&mut b.data)) {
        Some(s) => { c.pull_heat(Some(s)); true },
        None => { c.pull_heat(None); true },
    };
    if c.fade_jets() {
        changed = true;
    }
    if let Some(d) = facing.as_deref().and_then(registry::facing_dir) {
        for (stream, s) in [(Stream::Recipe, 1), (Stream::Waste, -1)] {
            let from = [key.1[0] + 2 * d[0] * s, key.1[1], key.1[2] + 2 * d[2] * s];
            let Some(target) = below(m, host, &key.0, from, CRUCIBLE_FALL) else { continue };
            let landing = target.cell.clone();
            if c.pour_stream(t, stream, &mut Target { m, hit: target }).is_some() {
                changed = true;
                // The stream's splash where it lands (ParticleFoundry's
                // drops), one per tick it pours.
                host.show(&landing.0, &Show::Particles {
                    id: "minecraft:falling_lava",
                    color: [1.; 4],
                    at: [f64::from(landing.1[0]) + 0.5, f64::from(landing.1[1]) + 1.0, f64::from(landing.1[2]) + 0.5],
                    spread: [0.2, 0.0, 0.2],
                    speed: 0.0,
                    count: 1,
                });
            }
        }
    }
    m.insert(key.clone(), me);
    changed
}

fn channel_tick(m: &mut Machines, host: &mut impl ServerHost, cx: &mut Cx, key: &Key, tick: u64) -> bool {
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
            if let Some(left) = accept_flow(m, host, cx, &n, back, &mat, c.melt.amount) {
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

fn tank_tick(m: &mut Machines, host: &mut impl ServerHost, cx: &mut Cx, key: &Key, tick: u64) -> bool {
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
                if let Some(left) = accept_flow(m, host, cx, &n, SIDES[s].map(|v| -v), &mat, c.melt.amount) {
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
