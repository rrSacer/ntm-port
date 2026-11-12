//! The fluid blocks (docs/fluidos.md): ducts and storage (barrel, tank).
//!
//! A duct (`fluid_duct_neo`, `TileEntityPipeBaseNT`) carries one fluid type,
//! set in its screen (the NTM sets it with an identifier); the network
//! only joins ducts of that type. A barrel/tank (`TileEntityBarrel`,
//! `TileEntityMachineFluidTank`) holds one tank and has the NTM's modes:
//! input, buffer (low priority, both ways), output, off. Containers go in
//! and out through item transactions: a full one from the hand pours in and
//! the empty one comes back; an empty one is filled.

use crate::{
    fluid::{self, FluidNode, PRIORITY_LOW, Tank},
    machine::{Effect, Logic, fill_level},
    machine::Held,
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub mod actions {
    /// Duct and storage: set the type to the fluid the held container holds.
    pub const TYPE_FROM_HAND: u32 = 1;
    pub const PAGE_BACK: u32 = 2;
    pub const PAGE_NEXT: u32 = 3;
    /// Duct: set the type to fluid `id`: `PICK + id`.
    pub const PICK: u32 = 1000;
    /// Storage: the mode.
    pub const MODE: u32 = 10;
    /// Storage: pour the held full container in / fill the held empty one.
    pub const POUR: u32 = 11;
    pub const FILL: u32 = 12;
}

/// Fluids a duct screen lists per page.
const PAGE: usize = 5;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Duct {
    /// `string_id`; empty: none yet (joins nothing).
    #[serde(default)]
    pub fluid: String,
    #[serde(default)]
    pub page: u16,
}

impl Logic for Duct {
    fn level(&self, _max: u8) -> u8 {
        0
    }

    fn view(&self) -> ViewModel {
        let types: Vec<&fluid::FluidType> = fluid::types().iter().filter(|f| f.id != 0).collect();
        let pages = types.len().div_ceil(PAGE);
        let page = (self.page as usize).min(pages - 1);
        let mut vm = ViewModel::new("Duto de fluido").text(
            if self.fluid.is_empty() { "Fluido: nenhum".to_string() } else { format!("Fluido: {}", fluid::label(&self.fluid)) },
            if self.fluid.is_empty() { view::YELLOW } else { view::GREEN },
        );
        vm = vm.button(actions::TYPE_FROM_HAND, "Usar o fluido do recipiente na mão");
        for f in &types[page * PAGE..((page + 1) * PAGE).min(types.len())] {
            vm = vm.button(actions::PICK + u32::from(f.id), fluid::label(&f.string_id));
        }
        vm.button(actions::PAGE_BACK, format!("◀ página {}/{pages}", page + 1)).button(actions::PAGE_NEXT, "▶")
    }

    fn action(&mut self, action: u32, held: Option<&Held>, fx: &mut Vec<Effect>) {
        let pages = (fluid::types().len() - 1).div_ceil(PAGE) as u16;
        match action {
            actions::TYPE_FROM_HAND => match held.and_then(|h| fluid::contents(&h.item)) {
                Some(c) => self.fluid = c.fluid,
                None => fx.push(Effect::Toast("Segure um recipiente cheio do fluido.".into())),
            },
            actions::PAGE_BACK => self.page = (self.page + pages - 1) % pages,
            actions::PAGE_NEXT => self.page = (self.page + 1) % pages,
            a if a >= actions::PICK => {
                if let Some(f) = fluid::types().iter().find(|f| u32::from(f.id) == a - actions::PICK && f.id != 0) {
                    self.fluid = f.string_id.clone();
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Only takes from the network.
    #[default]
    Input,
    /// Takes and gives, at low priority.
    Buffer,
    /// Only gives.
    Output,
    Off,
}

impl Mode {
    fn next(self) -> Self {
        match self {
            Mode::Input => Mode::Buffer,
            Mode::Buffer => Mode::Output,
            Mode::Output => Mode::Off,
            Mode::Off => Mode::Input,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Mode::Input => "Entrada",
            Mode::Buffer => "Buffer",
            Mode::Output => "Saída",
            Mode::Off => "Desligado",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FluidStore {
    pub tank: Tank,
    pub mode: Mode,
    /// mB a tick it moves at most (0: no limit): the floor of the big tank's
    /// `max(500, fill/100)` out and `max(500, room/100)` in
    /// (`TileEntityMachineFluidTank.java:49-50`; the orbus has 1 000).
    #[serde(default)]
    pub rate_floor: u32,
    /// `ContainerBarrel` / `ContainerMachineFluidTank`: the identifier and
    /// its output, a full container in and its empty out, an empty in and
    /// its full out.
    #[serde(skip, default = "store_slots")]
    pub slots: crate::server::container_bridge::Slots,
}

fn store_slots() -> crate::server::container_bridge::Slots {
    vec![None; 6]
}

impl FluidStore {
    pub fn new(cap: u32, rate_floor: u32) -> Self {
        Self { tank: Tank::new(cap), mode: Mode::Input, rate_floor, slots: store_slots() }
    }

    /// One container from slot `from` to slot `to` (`loadTank` and
    /// `unloadTank`): `turn` gives what it becomes, when it can.
    fn swap(&mut self, from: usize, to: usize, turn: impl Fn(&mut Tank, &str) -> Option<String>) -> bool {
        use crate::server::container_bridge::Stack;
        let Some(id) = self.slots[from].as_ref().map(|s| s.id.clone()) else { return false };
        let room = |out: &Option<Stack>, item: &str| out.as_ref().is_none_or(|o| o.id == item && o.count < 64);
        // Tried on a copy: the tank changes only when the result fits.
        let mut tank = self.tank.clone();
        let Some(result) = turn(&mut tank, &id) else { return false };
        if !room(&self.slots[to], &result) {
            return false;
        }
        self.tank = tank;
        if let Some(s) = &mut self.slots[from] {
            s.count -= 1;
            if s.count == 0 {
                self.slots[from] = None;
            }
        }
        match &mut self.slots[to] {
            Some(o) => o.count += 1,
            None => self.slots[to] = Some(Stack::new(&result, 1)),
        }
        true
    }

    /// What it gives a tick: `getProviderSpeed`, `max(floor, fill / 100)`.
    fn send_rate(&self) -> u64 {
        if self.rate_floor == 0 { u64::MAX } else { u64::from(self.rate_floor.max(self.tank.mb / 100)) }
    }

    /// What it takes a tick: `getReceiverSpeed`, `max(floor, (max - fill) / 100)` (`TileEntityMachineFluidTank.java:49`,
    /// `TileEntityMachineOrbus.java`): the room it has left, not what it holds.
    fn receive_rate(&self) -> u64 {
        if self.rate_floor == 0 { u64::MAX } else { u64::from(self.rate_floor.max(self.tank.free() / 100)) }
    }
}

impl FluidNode for FluidStore {
    fn fluid_offer(&self, f: &str) -> u64 {
        if matches!(self.mode, Mode::Buffer | Mode::Output) && self.tank.fluid == f { u64::from(self.tank.mb).min(self.send_rate()) } else { 0 }
    }
    fn fluid_demand(&self, f: &str) -> u64 {
        if matches!(self.mode, Mode::Buffer | Mode::Input) && self.tank.accepts(f) { u64::from(self.tank.free()).min(self.receive_rate()) } else { 0 }
    }
    fn fluid_priority(&self) -> u8 {
        if self.mode == Mode::Buffer { PRIORITY_LOW } else { fluid::PRIORITY_NORMAL }
    }
    fn fluid_types(&self) -> Vec<String> {
        if self.tank.fluid.is_empty() { Vec::new() } else { vec![self.tank.fluid.clone()] }
    }
    fn fluid_give(&mut self, f: &str, mb: u64) {
        self.tank.fill(f, mb as u32);
    }
    fn fluid_take(&mut self, _f: &str, mb: u64) {
        self.tank.drain(mb as u32);
    }
}

/// The native screen (`horizon_machine/barrel_steel.json`, `GUIBarrel`):
/// containers poured in from slot 2 (empty out at 3), filled from slot 4
/// (full out at 5); data [mB, capacity, colour, mode]; button 0 the mode.
impl crate::server::container_bridge::Slotted for FluidStore {
    fn slots_mut(&mut self) -> &mut crate::server::container_bridge::Slots {
        &mut self.slots
    }
    fn sync_in(&mut self) {
        self.swap(2, 3, |t, item| {
            let c = fluid::contents(item).filter(|c| t.accepts(&c.fluid) && t.free() >= c.mb)?;
            t.fill(&c.fluid, c.mb);
            Some(c.empty)
        });
        self.swap(4, 5, |t, item| {
            let (full, mb) = fluid::fill_container(item, &t.fluid).filter(|(_, mb)| t.mb >= *mb)?;
            t.drain(mb);
            Some(full)
        });
    }
    fn screen_data(&self) -> Vec<i32> {
        let mode = match self.mode {
            Mode::Input => 0,
            Mode::Buffer => 1,
            Mode::Output => 2,
            Mode::Off => 3,
        };
        vec![self.tank.mb as i32, self.tank.cap as i32, fluid::get(&self.tank.fluid).map_or(0, |f| f.color as i32), mode]
    }
    fn button(&mut self, id: u32) -> bool {
        if id == 0 {
            self.mode = self.mode.next();
        }
        false
    }
}

impl Logic for FluidStore {
    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(self)
    }

    fn level(&self, max: u8) -> u8 {
        fill_level(u64::from(self.tank.mb), u64::from(self.tank.cap), max)
    }

    fn view(&self) -> ViewModel {
        let title = if self.tank.cap > 100_000 { "Tanque de fluido" } else { "Barril de fluido" };
        ViewModel::new(title)
            .bar(&if self.tank.fluid.is_empty() { "Vazio".to_string() } else { fluid::label(&self.tank.fluid) }, u64::from(self.tank.mb), u64::from(self.tank.cap), "mB")
            .button(actions::MODE, format!("Modo: {}", self.mode.label()))
            .button(actions::POUR, "Despejar o recipiente cheio da mão")
            .button(actions::FILL, "Encher o recipiente vazio da mão")
            .button(actions::TYPE_FROM_HAND, "Tipo pelo recipiente da mão")
    }

    fn action(&mut self, action: u32, held: Option<&Held>, fx: &mut Vec<Effect>) {
        let toast = |fx: &mut Vec<Effect>, t: &str| fx.push(Effect::Toast(t.into()));
        match action {
            actions::MODE => self.mode = self.mode.next(),
            actions::TYPE_FROM_HAND => match held.and_then(|h| fluid::contents(&h.item)) {
                Some(c) => self.tank.set_type(&c.fluid),
                None => toast(fx, "Segure um recipiente cheio do fluido."),
            },
            actions::POUR => match held.and_then(|h| fluid::contents(&h.item).map(|c| (h, c))) {
                Some((h, c)) if self.tank.accepts(&c.fluid) && self.tank.free() >= c.mb => fx.push(Effect::TakeHeld { item: h.item.clone() }),
                Some(_) => toast(fx, "Não cabe: outro fluido no tanque, ou sem espaço."),
                None => toast(fx, "Segure um recipiente cheio de fluido."),
            },
            actions::FILL => match held.filter(|h| fluid::is_empty_container(&h.item)) {
                Some(h) => match fluid::fill_container(&h.item, &self.tank.fluid) {
                    Some((_, mb)) if self.tank.mb >= mb => fx.push(Effect::TakeHeld { item: h.item.clone() }),
                    Some(_) => toast(fx, "Fluido insuficiente no tanque."),
                    None => toast(fx, "Esse recipiente não leva este fluido."),
                },
                None => toast(fx, "Segure um recipiente vazio."),
            },
            _ => {}
        }
    }

    fn consumed(&mut self, item: &str, fx: &mut Vec<Effect>) {
        if let Some(c) = fluid::contents(item) {
            // A full container: in, and its empty one back.
            if self.tank.accepts(&c.fluid) && self.tank.free() >= c.mb {
                self.tank.fill(&c.fluid, c.mb);
                fx.push(Effect::Give { item: c.empty, count: 1 });
            } else {
                fx.push(Effect::Give { item: item.into(), count: 1 });
            }
        } else if let Some((full, mb)) = fluid::fill_container(item, &self.tank.fluid).filter(|(_, mb)| self.tank.mb >= *mb) {
            self.tank.drain(mb);
            fx.push(Effect::Give { item: full, count: 1 });
        } else {
            // Emptied meanwhile: the container goes back as it came.
            fx.push(Effect::Give { item: item.into(), count: 1 });
        }
    }

    fn unconsume(&mut self, item: &str) -> bool {
        // The world forgot a pour: take its fluid out again, if still there.
        match fluid::contents(item) {
            Some(c) if self.tank.fluid == c.fluid && self.tank.mb >= c.mb => {
                self.tank.drain(c.mb);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held(item: &str) -> Held {
        Held { item: item.into(), count: 1 }
    }

    #[test]
    fn a_barrel_takes_full_containers_and_fills_empty_ones() {
        let mut b = FluidStore::new(16_000, 0);
        let mut fx = Vec::new();
        b.action(actions::POUR, Some(&held("ntm:fluid_tank_full__water")), &mut fx);
        assert_eq!(fx, vec![Effect::TakeHeld { item: "ntm:fluid_tank_full__water".into() }]);
        fx.clear();
        b.consumed("ntm:fluid_tank_full__water", &mut fx);
        assert_eq!(b.tank.mb, 1000);
        assert_eq!(fx, vec![Effect::Give { item: "ntm:fluid_tank_empty".into(), count: 1 }]);
        fx.clear();
        b.action(actions::FILL, Some(&held("minecraft:bucket")), &mut fx);
        assert_eq!(fx, vec![Effect::TakeHeld { item: "minecraft:bucket".into() }]);
        fx.clear();
        b.consumed("minecraft:bucket", &mut fx);
        assert_eq!(fx, vec![Effect::Give { item: "minecraft:water_bucket".into(), count: 1 }]);
        assert_eq!(b.tank.mb, 0);
        // Another fluid doesn't go in while the tank has water typed in it.
        b.tank.fill("water", 10);
        fx.clear();
        b.action(actions::POUR, Some(&held("ntm:cell_uf6")), &mut fx);
        assert!(matches!(fx[0], Effect::Toast(_)));
        assert!(!b.unconsume("ntm:fluid_tank_full__water"));
    }

    #[test]
    fn modes_decide_what_the_network_sees() {
        let mut b = FluidStore::new(16_000, 0);
        b.tank.fill("water", 4000);
        assert_eq!((b.fluid_offer("water"), b.fluid_demand("water")), (0, 12_000));
        b.mode = Mode::Buffer;
        assert_eq!((b.fluid_offer("water"), b.fluid_demand("water"), b.fluid_priority()), (4000, 12_000, PRIORITY_LOW));
        b.mode = Mode::Output;
        assert_eq!((b.fluid_offer("water"), b.fluid_demand("water")), (4000, 0));
        let mut t = FluidStore::new(256_000, 500);
        t.mode = Mode::Output;
        t.tank.fill("oil", 200_000);
        assert_eq!(t.fluid_offer("oil"), 2000, "max(500, fill/100)");
    }

    /// The speed in is the room left over a hundred (`getReceiverSpeed`), the speed out what it holds over a hundred.
    #[test]
    fn the_big_tanks_take_by_their_room_and_give_by_their_content() {
        let mut t = FluidStore::new(512_000, 1_000);
        t.mode = Mode::Buffer;
        // Empty: it takes max(1 000, 512 000 / 100) = 5 120, gives nothing.
        assert_eq!((t.fluid_demand("oil"), t.fluid_offer("oil")), (5_120, 0));
        t.tank.fill("oil", 412_000);
        // 100 000 of room: max(1 000, 1 000); 412 000 held: max(1 000, 4 120).
        assert_eq!((t.fluid_demand("oil"), t.fluid_offer("oil")), (1_000, 4_120));
        t.tank.fill("oil", 99_500);
        // 500 of room: the floor, then what is left of it.
        assert_eq!(t.fluid_demand("oil"), 500);
        let mut small = FluidStore::new(256_000, 500);
        assert_eq!(small.fluid_demand("water"), 2_560, "max(500, 256 000 / 100)");
        small.tank.fill("water", 255_900);
        assert_eq!(small.fluid_demand("water"), 100, "100 of room");
    }

    #[test]
    fn a_duct_takes_its_type_from_the_screen_or_the_hand() {
        let mut d = Duct::default();
        let mut fx = Vec::new();
        d.action(actions::TYPE_FROM_HAND, Some(&held("ntm:cell_uf6")), &mut fx);
        assert_eq!(d.fluid, "uf6");
        d.action(actions::PICK + 1, None, &mut fx);
        assert_eq!(d.fluid, "water");
        d.action(actions::PAGE_BACK, None, &mut fx);
        assert!(d.page > 0, "volta para a última página");
        assert!(d.view().widgets.len() <= 9, "cabe na tela");
    }
}
