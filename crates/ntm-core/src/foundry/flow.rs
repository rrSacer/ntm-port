//! Where molten metal goes after the crucible, stage 2 (docs/maquinas.md,
//! "Fundição"): channels, tanks and outlets
//! (`tileentity/machine/TileEntityFoundry{Base,Channel,Tank,Outlet}.java`).
//! Each holds one material (`Melt`); the server moves it between them
//! (`server::foundry_server`). The NTM's shuffles and 1-in-5 swaps take a
//! hash of the position and the tick here (deterministic, as the rest of
//! the port).

use super::{Tables, quanta_name};
use crate::{
    machine::{Effect, Logic, fill_level},
    proto::Held,
    server::container_bridge::{Slots, Slotted},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

/// A channel holds two ingots (`INGOT.q(2)`), a tank four blocks.
pub const CHANNEL_CAP: u32 = 144;
pub const TANK_CAP: u32 = 2_592;
/// Ticks between a channel's moves; a tank waits 5..=10.
pub const CHANNEL_EVERY: u8 = 5;

/// One material in a foundry block (`TileEntityFoundryBase`: `type`, `amount`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Melt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mat: Option<String>,
    #[serde(default)]
    pub amount: u32,
}

impl Melt {
    /// `standardCheck`: not another material, not full.
    pub fn check(&self, mat: &str, cap: u32) -> bool {
        !(self.mat.as_deref().is_some_and(|m| m != mat) && self.amount > 0) && self.amount < cap
    }

    /// `standardAdd`: what doesn't fit comes back.
    pub fn add(&mut self, mat: &str, q: u32, cap: u32) -> u32 {
        self.mat = Some(mat.to_string());
        let n = q.min(cap.saturating_sub(self.amount));
        self.amount += n;
        q - n
    }

    /// The values a block without a screen sends for its model
    /// (`horizon_machine` `synced`): quanta, capacity, molten colour.
    pub fn synced(&self, t: &Tables, cap: u32) -> Vec<i32> {
        vec![self.amount as i32, cap as i32, self.mat.as_deref().map_or(0, |m| t.color(m)) as i32]
    }

    fn line(&self) -> String {
        match &self.mat {
            Some(m) if self.amount > 0 => format!("{m}: {}", quanta_name(self.amount)),
            _ => "vazio".into(),
        }
    }

    /// As scraps, a whole ingot's worth each (see `Crucible::contents`).
    fn scraps(&self) -> Vec<(String, u32)> {
        match &self.mat {
            Some(m) if self.amount >= 72 => vec![(format!("ntm:scraps__{m}"), self.amount / 72)],
            _ => Vec::new(),
        }
    }
}

/// `TileEntityFoundryChannel`: passes its metal on every 5 ticks.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    #[serde(flatten)]
    pub melt: Melt,
    /// The side it last got metal from (index in [`SIDES`], +1; 0 none):
    /// tried last.
    #[serde(default)]
    pub last_flow: u8,
    #[serde(default)]
    pub next: u8,
    /// No slots: the engine's block entity only carries its values.
    #[serde(skip)]
    pub slots: Slots,
}

/// `TileEntityFoundryTank`: fills the tank under it first.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Tank {
    #[serde(flatten)]
    pub melt: Melt,
    #[serde(default)]
    pub next: u8,
    #[serde(skip)]
    pub slots: Slots,
}

/// `TileEntityFoundryOutlet`: takes metal from behind and pours it into
/// what is under it (up to 4 blocks down). A material filter, which may be
/// inverted. Left out: the redstone that closes it (the block's template
/// has no signal state).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Outlet {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    #[serde(default)]
    pub invert: bool,
    #[serde(skip)]
    pub slots: Slots,
}

/// The four sides, in the NTM's `ForgeDirection` order 2..=5 (north,
/// south, west, east).
pub const SIDES: [[i32; 3]; 4] = [[0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];

/// The NTM's shuffled side order, from a hash; `last` (the side it got
/// metal from, 1..=4) goes last.
pub fn side_order(pos: [i32; 3], tick: u64, last: u8) -> Vec<usize> {
    let mut v: Vec<(u64, usize)> = (0..4).map(|i| (mix(pos, tick, i as u64), i)).collect();
    v.sort();
    let mut order: Vec<usize> = v.into_iter().map(|x| x.1).collect();
    if last > 0
        && let Some(i) = order.iter().position(|&s| s == usize::from(last - 1))
    {
        let s = order.remove(i);
        order.push(s);
    }
    order
}

/// A number in `0..n` from a hash (the NTM's `rand.nextInt(n)`).
pub fn roll(pos: [i32; 3], tick: u64, salt: u64, n: u64) -> u64 {
    mix(pos, tick, salt) % n.max(1)
}

/// A 1-in-`n` chance from a hash (the NTM's `rand.nextInt(n) == 0`).
pub fn one_in(pos: [i32; 3], tick: u64, salt: u64, n: u64) -> bool {
    mix(pos, tick, salt).is_multiple_of(n)
}

fn mix(pos: [i32; 3], tick: u64, salt: u64) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for v in [pos[0] as u64, pos[1] as u64, pos[2] as u64, tick, salt] {
        h = (h ^ v).wrapping_mul(0x0100_0000_01b3);
        h ^= h >> 29;
    }
    h
}

/// Moves between two neighbours of the same kind (`updateEntity`'s second
/// pass): swap the fill states (1 in 5, or always with a single quantum in
/// a channel) or give half the difference.
pub fn level(a: &mut Melt, b: &mut Melt, swap: bool) {
    if !(b.mat.is_none() || b.mat == a.mat || b.amount == 0) {
        return;
    }
    b.mat = a.mat.clone();
    if swap {
        std::mem::swap(&mut a.amount, &mut b.amount);
    } else if a.amount > b.amount {
        let d = (a.amount - b.amount) / 2;
        a.amount -= d;
        b.amount += d;
    }
}

impl Logic for Channel {
    fn level(&self, max: u8) -> u8 {
        fill_level(u64::from(self.melt.amount), u64::from(CHANNEL_CAP), max)
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Canal de fundição").text(self.melt.line(), view::GREY)
    }
    fn contents(&self) -> Vec<(String, u32)> {
        self.melt.scraps()
    }
}

impl Logic for Tank {
    fn level(&self, max: u8) -> u8 {
        fill_level(u64::from(self.melt.amount), u64::from(TANK_CAP), max)
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Tanque de fundição").text(self.melt.line(), view::GREY)
    }
    fn contents(&self) -> Vec<(String, u32)> {
        self.melt.scraps()
    }
}

/// Blocks without slots on the engine's block entity: their values go
/// to the model (`horizon_machine` `synced`): quanta, capacity, colour.
impl Slotted for Channel {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        self.melt.synced(Tables::ntm(), CHANNEL_CAP)
    }
}

impl Slotted for Tank {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        self.melt.synced(Tables::ntm(), TANK_CAP)
    }
}

/// The outlet holds no metal: its filter's colour (0: none), inverted.
impl Slotted for Outlet {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        vec![self.filter.as_deref().map_or(0, |m| Tables::ntm().color(m)) as i32, i32::from(self.invert)]
    }
}

pub mod actions {
    /// The filter: the material of the scraps in the hand.
    pub const FILTER_HELD: u32 = 1;
    pub const FILTER_CLEAR: u32 = 2;
    pub const FILTER_INVERT: u32 = 3;
}

impl Outlet {
    /// `canAcceptPartialFlow`'s filter rule.
    pub fn passes(&self, mat: &str) -> bool {
        self.filter.as_deref().is_none_or(|f| (f == mat) != self.invert)
    }
}

impl Logic for Outlet {
    fn level(&self, _max: u8) -> u8 {
        0
    }
    fn view(&self) -> ViewModel {
        let f = match &self.filter {
            None => "sem filtro".to_string(),
            Some(m) if self.invert => format!("tudo menos {m}"),
            Some(m) => format!("só {m}"),
        };
        ViewModel::new("Saída de fundição")
            .text(format!("Filtro: {f}"), view::GREY)
            .button(actions::FILTER_HELD, "Filtrar pelo material da sucata na mão")
            .button(actions::FILTER_INVERT, "Inverter o filtro")
            .button(actions::FILTER_CLEAR, "Limpar o filtro")
    }
    fn action(&mut self, action: u32, held: Option<&Held>, _fx: &mut Vec<Effect>) {
        match action {
            actions::FILTER_HELD => {
                if let Some(m) = held.and_then(|h| h.item.strip_prefix("ntm:scraps__")) {
                    self.filter = Some(m.to_string());
                }
            }
            actions::FILTER_CLEAR => self.filter = None,
            actions::FILTER_INVERT => self.invert = !self.invert,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(mat: &str, q: u32) -> Melt {
        Melt { mat: Some(mat.into()), amount: q }
    }

    #[test]
    fn one_material_per_block_up_to_its_capacity() {
        let mut c = Melt::default();
        assert!(c.check("iron", CHANNEL_CAP));
        assert_eq!(c.add("iron", 100, CHANNEL_CAP), 0);
        assert!(!c.check("copper", CHANNEL_CAP), "outro material");
        assert_eq!(c.add("iron", 100, CHANNEL_CAP), 56);
        assert!(!c.check("iron", CHANNEL_CAP), "cheio");
        // Empty but still typed: another material may come in.
        let e = Melt { mat: Some("iron".into()), amount: 0 };
        assert!(e.check("copper", CHANNEL_CAP));
    }

    #[test]
    fn neighbours_level_or_swap() {
        let (mut a, mut b) = (m("iron", 100), Melt::default());
        level(&mut a, &mut b, false);
        assert_eq!((a.amount, b.amount, b.mat.as_deref()), (50, 50, Some("iron")));
        let (mut a, mut b) = (m("iron", 100), m("iron", 10));
        level(&mut a, &mut b, true);
        assert_eq!((a.amount, b.amount), (10, 100));
        // Another material in the neighbour: nothing.
        let (mut a, mut b) = (m("iron", 100), m("copper", 10));
        level(&mut a, &mut b, false);
        assert_eq!((a.amount, b.amount), (100, 10));
    }

    #[test]
    fn the_side_order_is_shuffled_with_the_last_source_last() {
        let o = side_order([1, 2, 3], 10, 2);
        assert_eq!(o.len(), 4);
        assert_eq!(*o.last().unwrap(), 1);
        let mut s = o.clone();
        s.sort();
        assert_eq!(s, vec![0, 1, 2, 3]);
        // Different ticks, different orders (sometimes).
        assert!((0..20).any(|t| side_order([1, 2, 3], t, 0) != side_order([1, 2, 3], 0, 0)));
        // About one in five.
        let n = (0..1000).filter(|t| one_in([5, 6, 7], *t, 1, 5)).count();
        assert!((150..250).contains(&n), "{n}");
    }

    #[test]
    fn the_outlet_filter() {
        let mut o = Outlet::default();
        assert!(o.passes("iron"));
        o.action(actions::FILTER_HELD, Some(&Held { item: "ntm:scraps__steel".into(), count: 1 }), &mut Vec::new());
        assert!(o.passes("steel") && !o.passes("iron"));
        o.action(actions::FILTER_INVERT, None, &mut Vec::new());
        assert!(!o.passes("steel") && o.passes("iron"));
        o.action(actions::FILTER_CLEAR, None, &mut Vec::new());
        assert!(o.passes("steel"));
    }
}
