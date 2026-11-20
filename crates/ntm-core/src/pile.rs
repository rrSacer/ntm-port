//! Chicago Pile MK2, translated from ItemPileRodMK2 and TileEntityPileCore.
//! Geometry and simulation are independent of the host. In particular a control
//! channel uses the Java vertical-index calculation, including its axial case.
use crate::{
    Pos,
    fluid::{FluidNode, Tank},
    machine::Logic,
    server::container_bridge::{Slots, Slotted},
    view::ViewModel,
};
use serde::{Deserialize, Serialize};

pub const BRICK: &str = "ntm:pile_brick";
pub const CORE: &str = "ntm:pile_block__core";
pub const ROD_NAMES: [&str; 9] = [
    "ra226be",
    "po210be",
    "zr",
    "nu",
    "pu239",
    "rgp",
    "waste",
    "thorium",
    "thorium_fuel",
];
pub const MAX_HEAT: f64 = 800.0;

#[derive(Clone, Copy, Debug)]
pub struct RodSpec {
    pub source: f64,
    pub reaction: f64,
    pub life: f64,
    pub heat: f64,
    pub product: u8,
}
pub const RODS: [RodSpec; 9] = [
    RodSpec {
        source: 1.,
        reaction: 0.,
        life: 0.,
        heat: 0.,
        product: 0,
    },
    RodSpec {
        source: 1.,
        reaction: 0.,
        life: 0.,
        heat: 0.,
        product: 0,
    },
    RodSpec {
        source: 0.,
        reaction: 0.,
        life: 0.,
        heat: 0.,
        product: 2,
    },
    RodSpec {
        source: 0.,
        reaction: 1.,
        life: 25000.,
        heat: 0.25,
        product: 4,
    },
    RodSpec {
        source: 0.,
        reaction: 1.,
        life: 500.,
        heat: 0.5,
        product: 5,
    },
    RodSpec {
        source: 0.,
        reaction: 1.,
        life: 1000.,
        heat: 0.5,
        product: 6,
    },
    RodSpec {
        source: 0.,
        reaction: 1.,
        life: 0.,
        heat: 1.5,
        product: 6,
    },
    RodSpec {
        source: 0.,
        reaction: 1.,
        life: 35000.,
        heat: 0.25,
        product: 8,
    },
    RodSpec {
        source: 0.,
        reaction: 1.,
        life: 2000.,
        heat: 0.5,
        product: 6,
    },
];
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rod {
    pub kind: u8,
    pub depletion: f64,
}
impl Rod {
    pub fn of(id: &str, depletion: f64) -> Option<Self> {
        let name = id.strip_prefix("ntm:pile_rod__")?;
        Some(Self {
            kind: ROD_NAMES.iter().position(|n| *n == name)? as u8,
            depletion: if depletion.is_finite() {
                depletion.max(0.)
            } else {
                0.
            },
        })
    }
    pub fn id(&self) -> String {
        format!("ntm:pile_rod__{}", ROD_NAMES[self.kind as usize])
    }
    pub fn react(&mut self, input: f64) -> (f64, f64) {
        let s = RODS[self.kind as usize];
        let x = input.max(0.);
        let squirt = (x + 1. / ((x + 2.) * (x + 2.))).sqrt() - 1. / (x + 2.);
        let out = s.source + squirt * s.reaction;
        let heat = out * s.heat;
        if s.life > 0. {
            self.depletion += out;
            if self.depletion >= s.life {
                self.kind = s.product;
                self.depletion = 0.;
            }
        }
        (out, heat)
    }
}
pub fn offset(p: Pos, d: Pos, n: i32) -> Pos {
    std::array::from_fn(|i| p[i] + d[i] * n)
}
pub fn right(d: Pos) -> Pos {
    [-d[2], 0, d[0]]
}
fn opposite(d: Pos) -> Pos {
    d.map(|v| -v)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub at: Pos,
    pub inward: Pos,
    pub up: i32,
    pub down: i32,
    pub left: i32,
    pub right: i32,
    pub depth: i32,
}
impl Shape {
    pub fn height(&self) -> i32 {
        self.up + self.down + 1
    }
    pub fn width(&self) -> i32 {
        self.left + self.right + 1
    }
    pub fn cells(&self) -> Vec<(Pos, &'static str)> {
        let mut cells = Vec::new();
        let left = opposite(right(self.inward));
        for h in -self.down..=self.up {
            for v in -self.left..=self.right {
                for d in 0..self.depth {
                    let p = offset(
                        offset(offset(self.at, [0, 1, 0], h), left, -v),
                        self.inward,
                        d,
                    );
                    let edges = i32::from(h == -self.down || h == self.up)
                        + i32::from(v == -self.left || v == self.right)
                        + i32::from(d == 0 || d == self.depth - 1);
                    cells.push((
                        p,
                        if p == self.at {
                            "core"
                        } else if edges > 1 {
                            "edge"
                        } else {
                            "dummy"
                        },
                    ));
                }
            }
        }
        cells
    }
    pub fn contains(&self, p: Pos) -> bool {
        let delta = std::array::from_fn::<_, 3, _>(|i| p[i] - self.at[i]);
        let r = right(self.inward);
        let w = delta[0] * r[0] + delta[2] * r[2];
        let d = delta[0] * self.inward[0] + delta[2] * self.inward[2];
        (-self.down..=self.up).contains(&delta[1])
            && (-self.left..=self.right).contains(&w)
            && (0..self.depth).contains(&d)
    }
    /// Java probes up/left first, caps height/width to 15, and probes 15
    /// blocks behind the face (therefore its effective depth cap is 16).
    pub fn probe(
        at: Pos,
        inward: Pos,
        mut brick: impl FnMut(Pos) -> bool,
    ) -> Result<Self, &'static str> {
        if inward[1] != 0 || !crate::energy::DIRS.contains(&inward) {
            return Err("Use a horizontal face.");
        }
        if !brick(at) {
            return Err("Graphite block missing.");
        }
        let mut count = |d, max| {
            let mut n = 0;
            for i in 1..=max {
                if !brick(offset(at, d, i)) {
                    break;
                }
                n = i;
            }
            n
        };
        let up = count([0, 1, 0], 14);
        let down = count([0, -1, 0], 14 - up);
        let l = opposite(right(inward));
        let left = count(l, 14);
        let right = count(opposite(l), 14 - left);
        let depth = count(inward, 15) + 1;
        if up + down + 1 < 5 {
            return Err("Height too low (<5).");
        }
        if left + right + 1 < 5 {
            return Err("Width too low (<5).");
        }
        if depth < 5 {
            return Err("Depth too low (<5).");
        }
        if up == 0 || down == 0 || left == 0 || right == 0 {
            return Err("Core cannot be on an edge.");
        }
        let shape = Self {
            at,
            inward,
            up,
            down,
            left,
            right,
            depth,
        };
        if shape.cells().iter().any(|(p, _)| !brick(*p)) {
            return Err("Graphite block missing.");
        }
        Ok(shape)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelType {
    Fuel,
    Ventilation,
    Control,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    pub entry: Pos,
    pub inward: Pos,
    pub kind: ChannelType,
    pub rods: Vec<Option<Rod>>,
    pub heat: f64,
    pub incoming: f64,
    #[serde(skip)]
    pub outgoing: f64,
    #[serde(deserialize_with = "saved_air")]
    pub air: u32,
    pub control: f64,
}
/// A saved channel never holds more than the vent can fill (1000 mB).
fn saved_air<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    Ok(<u32 as serde::Deserialize>::deserialize(d)?.min(1000))
}
impl Channel {
    pub fn new(entry: Pos, inward: Pos, kind: ChannelType, length: usize) -> Self {
        Self {
            entry,
            inward,
            kind,
            rods: vec![None; length],
            heat: 0.,
            incoming: 0.,
            outgoing: 0.,
            air: 0,
            control: 1.,
        }
    }
    pub fn length(&self) -> i32 {
        self.rods.len() as i32
    }
    pub fn cells(&self) -> Vec<(Pos, &'static str)> {
        (0..self.length())
            .map(|i| {
                (
                    offset(self.entry, self.inward, i),
                    if i == 0 {
                        match self.kind {
                            ChannelType::Fuel => "fuel_in",
                            ChannelType::Ventilation => "air_in",
                            ChannelType::Control => "control",
                        }
                    } else if i == self.length() - 1 {
                        match self.kind {
                            ChannelType::Fuel => "fuel_out",
                            ChannelType::Ventilation => "air_out",
                            ChannelType::Control => "control",
                        }
                    } else {
                        "channel"
                    },
                )
            })
            .collect()
    }
    /// Pushes rods along the channel. Empty loader strokes do not eject.
    pub fn load(&mut self, rod: Rod) -> Option<Rod> {
        let mut next = Some(rod);
        for slot in &mut self.rods {
            std::mem::swap(slot, &mut next);
            next.as_ref()?;
        }
        next
    }
    pub fn exit(&self) -> Pos {
        offset(self.entry, self.inward, self.length())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Core {
    pub shape: Shape,
    pub channels: Vec<Channel>,
    #[serde(skip)]
    pub highest_heat: f64,
}
impl Core {
    pub fn new(shape: Shape) -> Self {
        Self {
            shape,
            channels: vec![],
            highest_heat: 0.,
        }
    }
    pub fn cells(&self) -> Vec<(Pos, &'static str)> {
        let roles: std::collections::BTreeMap<_, _> =
            self.channels.iter().flat_map(Channel::cells).collect();
        self.shape
            .cells()
            .into_iter()
            .map(|(p, role)| (p, roles.get(&p).copied().unwrap_or(role)))
            .collect()
    }
    pub fn channel_type(&self, d: Pos) -> ChannelType {
        if d[1] != 0 {
            ChannelType::Control
        } else if (d[0] != 0) == (self.shape.inward[0] != 0) {
            ChannelType::Fuel
        } else {
            ChannelType::Ventilation
        }
    }
    pub fn drill(&mut self, at: Pos, inward: Pos) -> Result<Vec<(Pos, Rod)>, &'static str> {
        if !crate::energy::DIRS.contains(&inward) {
            return Err("Invalid channel direction.");
        }
        if at == self.shape.at {
            return Err("Cannot intersect core.");
        }
        let kind = self.channel_type(inward);
        if let Some(i) = self
            .channels
            .iter()
            .position(|c| c.entry == at && c.inward == inward)
        {
            let c = self.channels.remove(i);
            let exit = c.exit();
            return Ok(c.rods.into_iter().flatten().map(|r| (exit, r)).collect());
        }
        let length = match kind {
            ChannelType::Fuel => self.shape.depth,
            ChannelType::Ventilation => self.shape.width(),
            ChannelType::Control => self.shape.height(),
        };
        let roles: std::collections::BTreeMap<_, _> = self.cells().into_iter().collect();
        for i in 0..length {
            match roles.get(&offset(at, inward, i)).copied() {
                Some("dummy") => {}
                Some("core") => return Err("Cannot intersect core."),
                Some("edge") => return Err("Cannot drill along edge."),
                Some(_) => return Err("Cannot intersect channel."),
                None => return Err("Foreign block in reactor."),
            }
        }
        self.channels
            .push(Channel::new(at, inward, kind, length as usize));
        Ok(Vec::new())
    }
    fn segment(&self, c: &Channel) -> usize {
        // ForgeDirection.getRotation(UP) leaves UP/DOWN axial. The Java
        // control channels therefore all have index `left`; preserve it.
        let r = right(c.inward);
        ((c.entry[0] - self.shape.at[0]) * r[0]
            + (c.entry[2] - self.shape.at[2]) * r[2]
            + self.shape.left) as usize
    }
    /// Reaction → intra/inter segment flux → ventilation → passive cooling
    /// → strict >800°C failure. Input flux is carried to the next tick.
    pub fn step(&mut self) -> bool {
        for c in &mut self.channels {
            if c.kind != ChannelType::Fuel {
                continue;
            }
            c.outgoing = 0.;
            let len = c.rods.len() as f64;
            for r in c.rods.iter_mut().flatten() {
                let (out, heat) = r.react(c.incoming / len);
                c.outgoing += out;
                c.heat += heat;
            }
            c.incoming = 0.;
        }
        let mut segments: Vec<Option<(ChannelType, Vec<usize>)>> =
            vec![None; self.shape.width() as usize];
        for kind in [ChannelType::Fuel, ChannelType::Control] {
            for (i, c) in self
                .channels
                .iter()
                .enumerate()
                .filter(|(_, c)| c.kind == kind)
            {
                if let Some(seg) = segments.get_mut(self.segment(c)) {
                    match seg {
                        None => *seg = Some((kind, vec![i])),
                        Some((k, ids)) if *k == kind => ids.push(i),
                        _ => {}
                    }
                }
            }
        }
        for (from, seg) in segments.iter().enumerate() {
            let Some((ChannelType::Fuel, ids)) = seg else {
                continue;
            };
            let outgoing: f64 = ids.iter().map(|i| self.channels[*i].outgoing).sum();
            for i in ids {
                self.channels[*i].incoming += outgoing;
            }
            // Java propagates between segments only from indices 1..width-1.
            // A reverse-drilled channel in an asymmetric pile can map to an
            // edge index despite its physical entry being inside the face.
            if from == 0 || from + 1 == segments.len() {
                continue;
            }
            for step in [-1_i32, 1] {
                let mut j = from as i32 + step;
                let mut mult = 1.;
                while j >= 1 && j < (segments.len() - 1) as i32 {
                    if let Some((kind, ids)) = &segments[j as usize] {
                        if *kind == ChannelType::Control {
                            let sum: f64 = ids.iter().map(|i| self.channels[*i].control).sum();
                            mult *= (sum / f64::from(self.shape.depth - 1)).clamp(0., 0.5);
                        } else {
                            for i in ids {
                                self.channels[*i].incoming += outgoing * mult;
                            }
                        }
                    }
                    j += step;
                }
            }
        }
        let vents: Vec<_> = self
            .channels
            .iter_mut()
            .filter(|c| c.kind == ChannelType::Ventilation && c.air > 0)
            .map(|c| {
                let cap = f64::from(c.air) / 1000.;
                c.air = c.air.saturating_sub((cap * 5.).ceil() as u32);
                (c.entry[1], cap)
            })
            .collect();
        self.highest_heat = 0.;
        for c in &mut self.channels {
            if c.kind != ChannelType::Fuel {
                continue;
            }
            for (y, cap) in &vents {
                if (c.entry[1] - y).abs() <= 1 {
                    c.heat *= 1. - cap * 0.05;
                }
            }
            c.heat = (c.heat * 0.999).max(20.);
            self.highest_heat = self.highest_heat.max(c.heat);
        }
        self.highest_heat > MAX_HEAT
    }
    pub fn blast_center(&self) -> [f64; 3] {
        let fuels: Vec<_> = self
            .channels
            .iter()
            .filter(|c| c.kind == ChannelType::Fuel)
            .collect();
        let mut p = [0., f64::from(self.shape.at[1] + self.shape.up), 0.];
        for c in &fuels {
            for k in [0, 2] {
                p[k] +=
                    f64::from(c.entry[k]) + 0.5 + f64::from(c.inward[k] * (c.length() - 1)) / 2.;
            }
        }
        for k in [0, 2] {
            p[k] /= fuels.len().max(1) as f64;
        }
        p
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Loader {
    pub slots: Slots,
    pub loading: bool,
    pub level: f64,
    pub delay: u32,
    pub was_redstone: bool,
}
impl Default for Loader {
    fn default() -> Self {
        Self {
            slots: vec![None],
            loading: false,
            level: 0.,
            delay: 0,
            was_redstone: false,
        }
    }
}
impl Loader {
    pub fn stroke(&mut self, redstone: bool) -> bool {
        if redstone && !self.was_redstone && self.delay == 0 && self.level <= 0. {
            self.loading = true;
        }
        self.was_redstone = redstone;
        if self.delay > 0 {
            self.delay -= 1;
            return false;
        }
        if self.loading {
            self.level += 1. / 7.;
            if self.level >= 1. {
                self.level = 1.;
                self.loading = false;
                self.delay = 5;
            }
            false
        } else {
            let load = self.level == 1.;
            self.level = (self.level - 1. / 7.).max(0.);
            load
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Control {
    pub level: f64,
    pub target: f64,
    pub was_redstone: bool,
}
impl Control {
    pub fn step(&mut self, can_move: bool, redstone: bool) {
        if can_move {
            let d = self.target - self.level;
            self.level += d.clamp(-1. / 60., 1. / 60.);
        }
        if redstone != self.was_redstone {
            self.target = if redstone { 1. } else { 0. };
        }
        self.was_redstone = redstone;
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Pile {
    Core(Option<Core>),
    Member(Option<Pos>),
    Loader(Loader),
    Vent(Tank),
    Control(Control),
}
impl Pile {
    pub fn of(block: &str) -> Option<Self> {
        Some(match block {
            CORE => Self::Core(None),
            "ntm:pile_device__loader" => Self::Loader(Loader::default()),
            "ntm:pile_device__vent" => Self::Vent(Tank::typed("air", 4000).with_pressure(VENT_PRESSURE)),
            "ntm:pile_device__control" => Self::Control(Control::default()),
            b if b.starts_with("ntm:pile_block__") => Self::Member(None),
            _ => return None,
        })
    }
}
impl Logic for Pile {
    fn level(&self, _: u8) -> u8 {
        0
    }
    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        if matches!(self, Self::Vent(_)) {
            Some(self)
        } else {
            None
        }
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Chicago Pile MK2")
    }
}
/// `TileEntityPileVent`'s `compair = new FluidTank(Fluids.AIR, 4_000).withPressure(1)` (`:28`): the
/// ventilation takes compressed air, 1 PU.
pub const VENT_PRESSURE: u8 = 1;

/// The ventilation takes air at 1 PU (the NTM) and also at 0 PU: a divergence kept so that worlds and
/// proofs that feed it from a plain barrel keep working (the same as the hydrotreater's and the
/// vacuum distiller's, `docs/onda-2-grupo-c.md`). Any other pressure is refused.
fn vent_takes(pressure: u8) -> bool {
    pressure == 0 || pressure == VENT_PRESSURE
}

impl FluidNode for Pile {
    fn fluid_demand_at(&self, f: &str, pressure: u8) -> u64 {
        if vent_takes(pressure) { self.fluid_demand(f) } else { 0 }
    }
    fn fluid_give_at(&mut self, f: &str, pressure: u8, n: u64) {
        if vent_takes(pressure) {
            self.fluid_give(f, n);
        }
    }
    fn fluid_types(&self) -> Vec<String> {
        if matches!(self, Self::Vent(_)) {
            vec!["air".into()]
        } else {
            vec![]
        }
    }
    fn fluid_demand(&self, f: &str) -> u64 {
        if let Self::Vent(t) = self
            && f == "air"
        {
            u64::from(t.free())
        } else {
            0
        }
    }
    fn fluid_offer(&self, _: &str) -> u64 {
        0
    }
    fn fluid_take(&mut self, _: &str, _: u64) {}
    fn fluid_give(&mut self, f: &str, n: u64) {
        if let Self::Vent(t) = self
            && f == "air"
        {
            t.fill(f, n.min(u64::from(t.free())) as u32);
        }
    }
}
impl Slotted for Pile {
    /// Its record keeps its slots (the model's JSON): a restore from the journal has them.
    fn slots_recorded(&self) -> bool {
        true
    }
    fn slots_mut(&mut self) -> &mut Slots {
        if let Self::Loader(l) = self {
            &mut l.slots
        } else {
            unreachable!("only loaders have slots")
        }
    }
    fn screen_data(&self) -> Vec<i32> {
        vec![]
    }
    fn held_contents(&self) -> Vec<(String, u32)> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn shape() -> Shape {
        Shape {
            at: [0, 64, 0],
            inward: [0, 0, 1],
            left: 2,
            right: 2,
            up: 2,
            down: 2,
            depth: 5,
        }
    }
    fn fuel(x: i32, y: i32) -> Channel {
        Channel::new([x, y, 0], [0, 0, 1], ChannelType::Fuel, 5)
    }
    fn rod(kind: u8) -> Rod {
        Rod {
            kind,
            depletion: 0.,
        }
    }
    #[test]
    fn forge_rotation_and_asymmetric_dimensions_match_the_source_matrix() {
        assert_eq!(right([0, 0, -1]), [1, 0, 0]);
        assert_eq!(right([1, 0, 0]), [0, 0, 1]);
        let expected = Shape {
            inward: [0, 0, -1],
            left: 1,
            right: 3,
            ..shape()
        };
        let bricks: std::collections::BTreeSet<_> =
            expected.cells().iter().map(|(p, _)| *p).collect();
        assert!(bricks.contains(&[-1, 64, 0]));
        assert!(bricks.contains(&[3, 64, 0]));
        assert_eq!(
            Shape::probe(expected.at, expected.inward, |p| bricks.contains(&p)).unwrap(),
            expected
        );
    }
    #[test]
    fn all_nine_rods_react_and_transmute_at_the_exact_lifetime() {
        for i in 0..9 {
            let mut r = rod(i);
            let s = RODS[i as usize];
            if s.life > 0. {
                r.depletion = s.life - 0.1;
            }
            let (out, heat) = r.react(4.);
            let squirt = (4_f64 + 1. / 36.).sqrt() - 1. / 6.;
            assert!((out - (s.source + squirt * s.reaction)).abs() < 1e-12);
            assert_eq!(heat, out * s.heat);
            if s.life > 0. {
                assert_eq!((r.kind, r.depletion), (s.product, 0.));
            } else {
                assert_eq!(r.kind, i);
            }
        }
    }
    #[test]
    fn fuels_need_neutrons_and_sources_and_waste_never_deplete() {
        for i in [2, 3, 4, 5, 6, 7, 8] {
            assert_eq!(rod(i).react(0.), (0., 0.));
        }
        for i in [0, 1] {
            let mut r = rod(i);
            assert_eq!(r.react(1e9), (1., 0.));
            assert_eq!(r.depletion, 0.);
        }
        let mut r = rod(6);
        for _ in 0..100 {
            r.react(100.);
        }
        assert_eq!(r.kind, 6);
        assert_eq!(r.depletion, 0.);
    }
    #[test]
    fn formation_rotates_in_all_four_directions_and_marks_only_edges() {
        for d in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
            let expected = Shape {
                inward: d,
                ..shape()
            };
            let bricks: std::collections::BTreeSet<_> =
                expected.cells().iter().map(|(p, _)| *p).collect();
            let found = Shape::probe(expected.at, d, |p| bricks.contains(&p)).unwrap();
            assert_eq!(found, expected);
            assert_eq!(found.cells().len(), 125);
            assert_eq!(
                found.cells().iter().filter(|(_, r)| *r == "core").count(),
                1
            );
            assert_eq!(
                found.cells().iter().filter(|(_, r)| *r == "edge").count(),
                44
            );
            assert!(found.cells().iter().all(|(p, _)| found.contains(*p)));
        }
    }
    #[test]
    fn formation_rejects_holes_small_sizes_and_core_on_an_edge() {
        let s = shape();
        let mut bricks: std::collections::BTreeSet<_> = s.cells().iter().map(|(p, _)| *p).collect();
        bricks.remove(&[0, 64, 2]);
        assert!(Shape::probe(s.at, s.inward, |p| bricks.contains(&p)).is_err());
        assert!(Shape::probe([2, 64, 0], s.inward, |p| s.contains(p)).is_err());
        assert!(Shape::probe(s.at, [0, 1, 0], |_| true).is_err());
        assert!(Shape::probe(s.at, s.inward, |p| s.contains(p) && p[1] > 62).is_err());
    }
    #[test]
    fn java_dimension_probe_keeps_the_effective_sixteen_block_depth() {
        let s = Shape {
            up: 7,
            down: 7,
            left: 7,
            right: 7,
            depth: 16,
            ..shape()
        };
        assert_eq!(Shape::probe(s.at, s.inward, |p| s.contains(p)).unwrap(), s);
    }
    #[test]
    fn drilling_rejects_edges_core_intersections_and_can_undo_with_ejection() {
        let mut c = Core::new(shape());
        assert!(c.drill([0, 64, 0], [0, 0, 1]).is_err());
        assert!(c.drill([2, 64, 0], [0, 0, 1]).is_err());
        c.drill([-1, 64, 0], [0, 0, 1]).unwrap();
        assert!(c.drill([-2, 64, 2], [1, 0, 0]).is_err());
        c.channels[0].load(rod(3));
        let dropped = c.drill([-1, 64, 0], [0, 0, 1]).unwrap();
        assert_eq!(dropped, vec![([-1, 64, 5], rod(3))]);
        assert!(c.channels.is_empty());
    }
    #[test]
    fn pushing_only_ejects_when_full_and_preserves_internal_depletion() {
        let mut c = fuel(-1, 64);
        let mut first = rod(3);
        first.depletion = 42.;
        assert_eq!(c.load(first.clone()), None);
        for _ in 0..4 {
            assert_eq!(c.load(rod(2)), None);
        }
        assert_eq!(c.load(rod(4)), Some(first));
        assert_eq!(c.rods[0], Some(rod(4)));
    }
    #[test]
    fn flux_is_delayed_and_summed_within_and_between_slices() {
        let mut c = Core::new(shape());
        let mut a = fuel(-1, 63);
        a.load(rod(0));
        let mut b = fuel(-1, 65);
        b.load(rod(3));
        let mut d = fuel(1, 64);
        d.load(rod(3));
        c.channels = vec![a, b, d];
        c.step();
        assert_eq!(c.channels[1].rods[0].as_ref().unwrap().depletion, 0.);
        assert_eq!(c.channels[1].incoming, 1.);
        assert_eq!(c.channels[2].incoming, 1.);
        c.step();
        assert!(c.channels[1].rods[0].as_ref().unwrap().depletion > 0.);
    }
    #[test]
    fn asymmetric_reverse_channel_at_an_edge_index_has_only_intra_segment_flux() {
        let mut c = Core::new(Shape {
            inward: [0, 0, -1],
            left: 1,
            right: 3,
            ..shape()
        });
        c.drill([1, 64, -4], [0, 0, 1]).unwrap();
        c.drill([2, 64, 0], [0, 0, -1]).unwrap();
        c.channels[0].load(rod(0));
        assert_eq!(c.segment(&c.channels[0]), 0);
        c.step();
        assert_eq!(c.channels[0].incoming, 1.);
        assert_eq!(c.channels[1].incoming, 0.);
    }
    #[test]
    fn control_reduces_only_cross_slice_flux_and_preserves_java_axial_index() {
        let mut c = Core::new(shape());
        let mut a = fuel(-1, 64);
        a.load(rod(0));
        c.channels = vec![
            a,
            fuel(1, 64),
            Channel::new([0, 66, 2], [0, -1, 0], ChannelType::Control, 5),
        ];
        c.step();
        assert_eq!(c.channels[0].incoming, 1.);
        assert_eq!(c.channels[1].incoming, 0.25);
        c.channels[2].control = 0.;
        c.step();
        assert_eq!(c.channels[1].incoming, 0.);
        let before = c.segment(&c.channels[2]);
        c.channels[2].entry[0] = 1;
        assert_eq!(c.segment(&c.channels[2]), before);
    }
    #[test]
    fn air_cools_only_nearby_rows_then_passive_cooling_is_applied() {
        let mut c = Core::new(Shape {
            up: 3,
            down: 3,
            ..shape()
        });
        let mut a = fuel(-1, 64);
        a.heat = 500.;
        let mut b = fuel(1, 66);
        b.heat = 500.;
        let mut v = Channel::new([-2, 64, 2], [1, 0, 0], ChannelType::Ventilation, 5);
        v.air = 1000;
        c.channels = vec![a, b, v];
        c.step();
        assert!((c.channels[0].heat - 474.525).abs() < 1e-10);
        assert_eq!(c.channels[1].heat, 499.5);
        assert_eq!(c.channels[2].air, 995);
        c.channels[2].air = 1;
        c.step();
        assert_eq!(c.channels[2].air, 0);
    }
    #[test]
    fn meltdown_is_strictly_after_cooling_and_greater_than_eight_hundred() {
        let mut c = Core::new(shape());
        c.channels = vec![fuel(-1, 64)];
        c.channels[0].heat = 800. / 0.999;
        assert!(!c.step());
        c.channels[0].heat = 801. / 0.999;
        assert!(c.step());
        assert_eq!(c.blast_center(), [-0.5, 66., 2.5]);
    }
    #[test]
    fn loader_strokes_and_controls_keep_the_java_timings() {
        let mut l = Loader::default();
        let mut moves = 0;
        for t in 0..30 {
            if l.stroke(t == 0) {
                moves += 1;
            }
        }
        assert_eq!(moves, 1);
        assert_eq!(l.level, 0.);
        assert!(!l.loading);
        let mut c = Control::default();
        c.step(true, true);
        assert_eq!(c.level, 0.);
        assert_eq!(c.target, 1.);
        for _ in 0..60 {
            c.step(true, true);
        }
        assert_eq!(c.level, 1.);
        c.step(true, false);
        assert_eq!(c.target, 0.);
        c.step(false, false);
        assert_eq!(c.level, 1.);
    }
    #[test]
    fn persistence_keeps_flux_heat_life_air_and_controls_but_not_outgoing() {
        let mut c = Core::new(shape());
        let mut a = fuel(-1, 64);
        a.load(rod(3));
        a.incoming = 12.5;
        a.outgoing = 8.;
        a.heat = 345.;
        a.rods[0].as_mut().unwrap().depletion = 23.;
        let mut v = Channel::new([-2, 63, 2], [1, 0, 0], ChannelType::Ventilation, 5);
        v.air = 321;
        let mut control = Channel::new([0, 66, 2], [0, -1, 0], ChannelType::Control, 5);
        control.control = 0.42;
        c.channels = vec![a, v, control];
        let text = serde_json::to_string(&c).unwrap();
        let loaded: Core = serde_json::from_str(&text).unwrap();
        c.channels[0].outgoing = 0.;
        assert_eq!(c, loaded);
    }
    #[test]
    fn a_corrupted_saved_air_level_is_capped_to_the_vent_capacity() {
        let mut v = Channel::new([-2, 63, 2], [1, 0, 0], ChannelType::Ventilation, 5);
        v.air = 4_000_000_000;
        let loaded: Channel = serde_json::from_str(&serde_json::to_string(&v).unwrap()).unwrap();
        assert_eq!(loaded.air, 1000);
    }
    /// `TileEntityPileVent.java:28`: the ventilation's tank is at 1 PU; it takes air at 1 PU and at 0 PU, nothing else.
    #[test]
    fn the_vent_takes_air_at_one_pu_and_at_zero_but_no_other_pressure() {
        let mut v = Pile::of("ntm:pile_device__vent").unwrap();
        let Pile::Vent(t) = &v else { panic!() };
        assert_eq!((t.fluid.as_str(), t.cap, t.pressure), ("air", 4_000, VENT_PRESSURE));
        for (p, takes) in [(0u8, true), (1, true), (2, false), (3, false), (5, false)] {
            assert_eq!(v.fluid_demand_at("air", p) > 0, takes, "demand @{p}");
        }
        assert_eq!(v.fluid_demand_at("water", 1), 0);
        v.fluid_give_at("air", 2, 500);
        v.fluid_give_at("water", 1, 500);
        assert!(matches!(&v, Pile::Vent(t) if t.mb == 0));
        v.fluid_give_at("air", 1, 700);
        v.fluid_give_at("air", 0, 300);
        v.fluid_give_at("air", 5, 900);
        assert!(matches!(&v, Pile::Vent(t) if t.mb == 1_000));
        assert_eq!(v.fluid_offer_at("air", 1), 0, "the vent never sends");
    }

    /// A vent saved before pressure has its tank at 0: it opens and takes the same air.
    #[test]
    fn an_old_vent_without_a_pressure_still_takes_air() {
        let old: Pile = serde_json::from_str(r#"{"Vent":{"fluid":"air","mb":10,"cap":4000}}"#).unwrap();
        assert!(matches!(&old, Pile::Vent(t) if t.pressure == 0 && t.mb == 10));
        assert!(old.fluid_demand_at("air", 1) > 0 && old.fluid_demand_at("air", 0) > 0);
    }
}
