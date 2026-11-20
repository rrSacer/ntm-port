//! The battery socket and the REDD (`ntm:machine_battery_socket`,
//! `ntm:machine_battery_redd`; `TileEntityBatterySocket`, `TileEntityBatteryREDD`
//! over `TileEntityBatteryBase`, `MachineBatterySocket`, `MachineBatteryREDD`).
//!
//! The rules are `energy_network` (proven against the Java): the socket has no
//! store, its pack's charge is its power; the REDD has an unbounded store
//! (`BigInteger`) and two slots, one it draws from and one it charges. This
//! module is the machine around them: the redstone modes, the screen, the
//! ports, the persistence, and the pack in a slot as an item stack (its
//! `charge` is the stack's custom data, `ItemBatteryPack`'s NBT `charge`).
//!
//! Both are conductors (`IEnergyConductorMK2`): all their ports are one node,
//! so cables at any side are one network whatever the mode. The network here
//! has no priorities (as the battery, `crate::machine::Battery`): the setting
//! is kept and shown, not applied.

use crate::{
    Pos,
    energy::{EnergyNode, Role},
    energy_network::{
        BatteryBase, BatteryControl, BatteryRedd, BatterySocket, BigInt, ConnectionPriority, Dir, EnergyEntity,
        ItemSlot, MODE_BUFFER, MODE_INPUT, MODE_NONE, MODE_OUTPUT, PACKS, SlotItem, comparator_power,
    },
    machine::{CREATIVE_BATTERY, Logic},
    server::container_bridge::{Slots, Slotted, Stack},
    view::{self, ViewModel},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const SOCKET: &str = "ntm:machine_battery_socket";
pub const REDD: &str = "ntm:machine_battery_redd";

/// `TileEntityBatteryREDD.getMaxPower`: `Long.MAX_VALUE / 100`.
pub const REDD_MAX: i64 = i64::MAX / 100;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Body {
    #[default]
    Socket,
    Redd,
}

impl Body {
    pub fn of(block: &str) -> Option<Body> {
        match block {
            SOCKET => Some(Body::Socket),
            REDD => Some(Body::Redd),
            _ => None,
        }
    }

    /// `TileEntityBatterySocket()` is `super(1)`, the REDD's `super(2)`.
    fn slot_count(self) -> usize {
        match self {
            Body::Socket => 1,
            Body::Redd => 2,
        }
    }
}

/// `ItemBatteryPack` and `ItemBattery` by metadata: which `IBatteryItem` the stack is.
pub fn slot_item(id: &str) -> SlotItem {
    if id == CREATIVE_BATTERY {
        SlotItem::Creative
    } else if let Some(i) = PACKS.iter().position(|p| p.id == id) {
        SlotItem::Pack(i as i32)
    } else {
        SlotItem::Other
    }
}

/// The items `isItemValidForSlot` takes (`stack.getItem() instanceof IBatteryItem`).
pub fn battery_items() -> Vec<String> {
    PACKS.iter().map(|p| p.id.to_string()).chain([CREATIVE_BATTERY.to_string()]).collect()
}

/// The NBT long `charge` of a stack (`None`: it has no tag yet).
pub fn charge_of(stack: &Stack) -> Option<i64> {
    let fields = crate::snbt::fields(stack.custom.as_deref()?)?;
    fields.get("charge")?.trim_end_matches(['l', 'L']).parse().ok()
}

pub(crate) fn item_slot(stack: &Option<Stack>) -> Option<ItemSlot> {
    stack.as_ref().map(|s| ItemSlot::new(slot_item(&s.id), charge_of(s)))
}

/// What the energy code did to the stack's tag goes back into the stack.
pub(crate) fn write_back(stack: &mut Option<Stack>, slot: Option<&ItemSlot>) {
    let (Some(stack), Some(slot)) = (stack.as_mut(), slot) else { return };
    if let Some(charge) = slot.tag()
        && charge_of(stack) != Some(charge)
    {
        stack.custom = Some(crate::snbt::with_field(stack.custom.as_deref(), "charge", &format!("{charge}L")));
    }
}

/// `power.toByteArray()`, as the Java saves it.
mod java_bytes {
    use super::*;

    pub fn serialize<S: Serializer>(v: &BigInt, s: S) -> Result<S::Ok, S::Error> {
        v.to_java_bytes().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<BigInt, D::Error> {
        let bytes = Vec::<u8>::deserialize(d)?;
        BigInt::from_java_bytes(&bytes).ok_or_else(|| serde::de::Error::custom("power: not a BigInteger"))
    }
}

fn high_mode() -> i16 {
    MODE_OUTPUT
}

fn low_priority() -> u8 {
    ConnectionPriority::Low.ordinal() as u8
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Storage {
    pub body: Body,
    /// `redLow` and `redHigh`: the mode without and with a redstone signal.
    #[serde(default)]
    pub red_low: i16,
    #[serde(default = "high_mode")]
    pub red_high: i16,
    /// `priority.ordinal()`.
    #[serde(default = "low_priority")]
    pub priority: u8,
    /// `lastRedstone`: the comparator signal of the last tick.
    #[serde(default)]
    pub last_redstone: u8,
    /// The REDD's store (`power`, saved as `toByteArray`).
    #[serde(default = "BigInt::zero", with = "java_bytes")]
    pub power: BigInt,
    /// Any of its ports gets a redstone signal (the native host reads it): `redHigh` is in force.
    #[serde(skip)]
    pub powered: bool,
    /// `delta` and `log` of the REDD (not saved by the Java).
    #[serde(skip, default = "BigInt::zero")]
    pub delta: BigInt,
    #[serde(skip)]
    pub log: [Option<BigInt>; 20],
    /// The socket's `delta` and `log`, longs.
    #[serde(skip)]
    pub socket_delta: i64,
    #[serde(skip)]
    pub socket_log: [i64; 20],
    /// The engine's slots, as the bridge reads and writes them: the socket's pack, the REDD's two (the one it
    /// draws from and the one it charges).
    #[serde(skip)]
    pub slots: Slots,
    /// The machine's own copy of them. The energy network moves energy after the machines ran, outside the step
    /// that writes the slots, so between the step and the end of the tick this copy is the truth: the tick's end
    /// writes it to the slots (`publishable`, `published`) in the same commit as the machine's state, so no player
    /// ever sees a slot behind the network. `sync_in` adopts a slot a player (or a hopper) changed.
    #[serde(skip)]
    pub kept: Slots,
    /// What `sync_out` last put in `slots`: a slot that differs was changed by someone else.
    #[serde(skip)]
    sent: Slots,
    /// `sync_in` has read the engine's slots since this machine was read from a save: until then `kept` is not
    /// what the slots hold and must not be written over them.
    #[serde(skip)]
    synced: bool,
}


impl Storage {
    pub fn new(body: Body) -> Self {
        Self {
            body,
            red_low: MODE_INPUT,
            red_high: MODE_OUTPUT,
            priority: low_priority(),
            last_redstone: 0,
            power: BigInt::zero(),
            powered: false,
            delta: BigInt::zero(),
            log: Default::default(),
            socket_delta: 0,
            socket_log: [0; 20],
            slots: vec![None; body.slot_count()],
            kept: vec![None; body.slot_count()],
            sent: vec![None; body.slot_count()],
            synced: true,
        }
    }

    fn fit_slots(&mut self) {
        let n = self.body.slot_count();
        self.slots.resize(n, None);
        self.kept.resize(n, None);
        self.sent.resize(n, None);
    }

    fn base(&self) -> BatteryBase {
        let mut base = BatteryBase::default();
        base.red_low = self.red_low;
        base.red_high = self.red_high;
        base.priority = ConnectionPriority::from_index_safely(i32::from(self.priority));
        base.last_redstone = self.last_redstone;
        base
    }

    fn keep_base(&mut self, base: &BatteryBase) {
        self.red_low = base.red_low;
        self.red_high = base.red_high;
        self.priority = base.priority.ordinal() as u8;
        self.last_redstone = base.last_redstone;
    }

    /// The pure socket over this machine's state; its `meta` (the orientation) only places ports, so it is not set.
    fn socket(&self) -> BatterySocket {
        let mut s = BatterySocket::new(10);
        s.base = self.base();
        s.slot = self.kept.first().and_then(item_slot);
        s
    }

    fn redd(&self) -> BatteryRedd {
        let mut r = BatteryRedd::new(10);
        r.base = self.base();
        r.power = self.power.clone();
        r.slots = [self.kept.first().and_then(item_slot), self.kept.get(1).and_then(item_slot)];
        r
    }

    /// The tags the energy code wrote go back into the stacks.
    fn commit_socket(&mut self, s: &BatterySocket) {
        self.fit_slots();
        write_back(&mut self.kept[0], s.slot.as_ref());
    }

    fn commit_redd(&mut self, r: &BatteryRedd) {
        self.fit_slots();
        self.power = r.power.clone();
        write_back(&mut self.kept[0], r.slots[0].as_ref());
        write_back(&mut self.kept[1], r.slots[1].as_ref());
    }

    /// `getRelevantMode(false)` with the redstone the host last read.
    pub fn mode(&self) -> i16 {
        if self.powered { self.red_high } else { self.red_low }
    }

    /// The charge, the capacity and the rate the screens show.
    fn socket_figures(&self) -> Option<(i64, i64)> {
        let s = self.socket();
        s.slot.as_ref().filter(|p| p.is_battery()).map(|p| (p.get_charge(), p.max_charge()))
    }

    /// `getPower` / `getMaxPower` of the machine: the pack's, or the REDD's window of the store.
    pub fn power_figures(&self) -> (i64, i64) {
        match self.body {
            Body::Socket => self.socket_figures().unwrap_or((0, 0)),
            Body::Redd => {
                let r = self.redd();
                (r.power(), r.max_power())
            }
        }
    }

    /// The cells (and the way out of each) its power connects through: the socket has eight, the REDD six, all
    /// on the edge of its volume (`getConPos`: the cell outside and the direction, here the cell it joins and the
    /// face it joins by).
    pub fn port_cells(block: &str, core: Pos, facing: Option<&str>) -> Vec<(Pos, Pos)> {
        let Some(body) = Body::of(block) else { return Vec::new() };
        let Some(dir) = java_dir(facing) else { return Vec::new() };
        let meta = 10 + dir.ordinal() as i32;
        let cons = match body {
            Body::Socket => BatterySocket::new(meta).connection_positions(core),
            Body::Redd => BatteryRedd::new(meta).connection_positions(core),
        };
        cons.into_iter()
            .map(|(at, d)| {
                let o = d.offset();
                ([at[0] - o[0], at[1] - o[1], at[2] - o[2]], o)
            })
            .collect()
    }

    /// `getPortPos`: the cells that read redstone (every cell of the socket, the REDD's six).
    pub fn redstone_cells(block: &str, core: Pos, facing: Option<&str>) -> Vec<Pos> {
        let Some(body) = Body::of(block) else { return Vec::new() };
        let Some(dir) = java_dir(facing) else { return Vec::new() };
        let meta = 10 + dir.ordinal() as i32;
        match body {
            Body::Socket => BatterySocket::new(meta).port_positions(core),
            Body::Redd => BatteryRedd::new(meta).port_positions(core),
        }
    }

    /// The screen's long, as two `i32`s (the high half then the low).
    fn pair(v: i64) -> [i32; 2] {
        [(v >> 32) as i32, v as i32]
    }

    /// `updateEntity` of `TileEntityBatteryBase` and its two bodies. Whether what is saved changed.
    fn step(&mut self) -> bool {
        self.fit_slots();
        let before = (self.red_low, self.red_high, self.priority, self.last_redstone, self.power.clone());
        let kept_before = self.kept.clone();
        match self.body {
            Body::Socket => {
                let mut s = self.socket();
                let prev = s.power();
                s.update(self.powered);
                let comp = comparator_power(s.power(), s.max_power());
                s.base.last_redstone = comp;
                // `(getPower() + prevPower) / 2`; the log of 20 averages and the difference to its oldest.
                let avg = s.power().wrapping_add(prev) / 2;
                self.socket_delta = avg.wrapping_sub(self.socket_log[0]);
                self.socket_log.rotate_left(1);
                self.socket_log[19] = avg;
                self.keep_base(&s.base);
                self.commit_socket(&s);
            }
            Body::Redd => {
                let mut r = self.redd();
                r.delta = self.delta.clone();
                r.log = self.log.clone();
                // The comparator is read by the base class, before the REDD moves its items.
                let comp = comparator_power(r.power(), r.max_power());
                r.update(self.powered);
                r.base.last_redstone = comp;
                self.delta = r.delta.clone();
                self.log = r.log.clone();
                self.keep_base(&r.base);
                self.commit_redd(&r);
            }
        }
        before != (self.red_low, self.red_high, self.priority, self.last_redstone, self.power.clone()) || kept_before != self.kept
    }

    /// The machine's copy of its slots, to be written to the engine's `now` at the end of the tick (after the
    /// network moved energy). Only when the engine's slots are still exactly what this machine last put there (or
    /// read from them): anything else is a player's (or a hopper's) change this machine has not seen yet, and
    /// writing the copy over it would give a taken battery back or take a put one away. `None`: nothing to write.
    pub fn publishable(&self, now: &Slots) -> Option<Slots> {
        if !self.synced {
            return None;
        }
        let n = self.body.slot_count();
        // The item data as the engine writes it back, canonical: the same data this machine wrote (`slots_in` does the same).
        let fit = |s: &Slots| {
            let mut s = s.clone();
            s.resize(n, None);
            for stack in s.iter_mut().flatten() {
                if let Some(c) = stack.custom.as_deref().and_then(crate::snbt::canonical) {
                    stack.custom = Some(c);
                }
            }
            s
        };
        let (now, kept) = (fit(now), fit(&self.kept));
        (now == fit(&self.sent) && now != kept).then_some(kept)
    }

    /// The engine took what `publishable` gave: it is what the slots hold now.
    pub fn published(&mut self) {
        self.sync_out();
    }

    /// The rotor speed the client animates (`getSpeed`), 0 for the socket.
    pub fn speed(&self) -> f32 {
        match self.body {
            Body::Socket => 0.0,
            Body::Redd => self.redd().speed(),
        }
    }
}

/// The Java direction a block placed `facing` has (its metadata minus 10 is `ForgeDirection.ordinal`).
pub fn java_dir(facing: Option<&str>) -> Option<Dir> {
    match facing? {
        "north" => Some(Dir::North),
        "south" => Some(Dir::South),
        "west" => Some(Dir::West),
        "east" => Some(Dir::East),
        _ => None,
    }
}

impl Logic for Storage {
    fn tick(&mut self) -> bool {
        self.step()
    }

    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        Some(self)
    }

    fn level(&self, _max: u8) -> u8 {
        0
    }

    fn view(&self) -> ViewModel {
        let (power, max) = self.power_figures();
        let title = match self.body {
            Body::Socket => "Soquete de bateria",
            Body::Redd => "FEnSU",
        };
        let mut v = ViewModel::new(title);
        match self.body {
            Body::Socket => v = v.bar("Energia", power.max(0) as u64, max.max(0) as u64, "HE"),
            Body::Redd => v = v.text(format!("{} HE", self.power), view::GREEN),
        }
        v.text(format!("Modo: {} (com sinal: {})", mode_label(self.red_low), mode_label(self.red_high)), view::GREY)
    }
}

fn mode_label(mode: i16) -> &'static str {
    match mode {
        MODE_INPUT => "entrada",
        MODE_BUFFER => "buffer",
        MODE_OUTPUT => "saída",
        MODE_NONE => "desligada",
        _ => "?",
    }
}

impl EnergyNode for Storage {
    fn role(&self) -> Role {
        Role::Storage
    }

    /// What `PowerNetMK2.update` would take from it: its power, up to its provider speed, in the modes that
    /// make it a provider.
    fn offer(&self) -> u64 {
        let mode = self.mode();
        if mode != MODE_OUTPUT && mode != MODE_BUFFER {
            return 0;
        }
        match self.body {
            Body::Socket => {
                let mut s = self.socket();
                s.base.relevant_mode(self.powered);
                s.power().min(s.provider_speed()).max(0) as u64
            }
            // No `getProviderSpeed` of its own: its `getMaxPower`.
            Body::Redd => {
                let r = self.redd();
                r.power().min(r.provider_speed()).max(0) as u64
            }
        }
    }

    /// `min(max - power, receiverSpeed)`, in the modes that make it a receiver.
    fn demand(&self) -> u64 {
        let mode = self.mode();
        if mode != MODE_INPUT && mode != MODE_BUFFER {
            return 0;
        }
        match self.body {
            Body::Socket => {
                let mut s = self.socket();
                s.base.relevant_mode(self.powered);
                s.max_power().wrapping_sub(s.power()).min(s.receiver_speed()).max(0) as u64
            }
            Body::Redd => {
                let r = self.redd();
                r.max_power().wrapping_sub(r.power()).min(r.receiver_speed()).max(0) as u64
            }
        }
    }

    fn give(&mut self, he: u64) {
        let he = he.min(i64::MAX as u64) as i64;
        match self.body {
            Body::Socket => {
                let mut s = self.socket();
                s.transfer_power(he);
                self.commit_socket(&s);
            }
            Body::Redd => {
                let mut r = self.redd();
                r.transfer_power(he);
                self.commit_redd(&r);
            }
        }
    }

    fn take(&mut self, he: u64) {
        let he = he.min(i64::MAX as u64) as i64;
        match self.body {
            Body::Socket => {
                let mut s = self.socket();
                s.use_power(he);
                self.commit_socket(&s);
            }
            Body::Redd => {
                let mut r = self.redd();
                r.use_power(he);
                self.commit_redd(&r);
            }
        }
    }
}

/// The screens (`horizon_screen/machine_battery_socket.json`, `..._redd.json`).
///
/// Socket: [fill, fill max, redLow, redHigh, priority, charge (2), max (2), delta (2), has a pack (11)]; the bar
/// is the 52 pixels of `GUIBatterySocket` (`power * 52 / maxPower`), kept as `2p - 1` of 104 so each pixel
/// shows exactly. REDD: [redLow, redHigh, priority, power (2), delta (2), the wheel's speed].
impl Slotted for Storage {
    fn slots_mut(&mut self) -> &mut Slots {
        self.fit_slots();
        &mut self.slots
    }

    /// The slots were just read from the engine: what a player put in or took out is the truth, the rest is
    /// the machine's own copy (which the network may have moved energy in since the last write).
    fn sync_in(&mut self) {
        self.synced = true;
        self.fit_slots();
        for i in 0..self.slots.len() {
            if self.slots[i] != self.sent[i] {
                self.kept[i] = self.slots[i].clone();
                self.sent[i] = self.slots[i].clone();
            }
        }
    }

    fn sync_out(&mut self) {
        self.fit_slots();
        self.slots = self.kept.clone();
        self.sent = self.kept.clone();
    }

    fn screen_data(&self) -> Vec<i32> {
        let mut v = Vec::new();
        match self.body {
            Body::Socket => {
                let figures = self.socket_figures();
                let pixels = figures.map_or(0, |(power, max)| {
                    // `GUIBatterySocket`: a charge above `Long.MAX_VALUE / 100` is scaled down before the ratio.
                    let (power, max) = if power > i64::MAX / 100 { (power / 100, max / 100) } else { (power, max) };
                    let max = max.max(1);
                    ((i128::from(power) * 52 / i128::from(max)) as i64).clamp(0, 52)
                });
                v.extend([(2 * pixels - 1).max(0) as i32, 104, i32::from(self.red_low), i32::from(self.red_high), i32::from(self.priority)]);
                let (power, max) = figures.unwrap_or((0, 0));
                v.extend(Self::pair(power));
                v.extend(Self::pair(max));
                v.extend(Self::pair(self.socket_delta));
                v.push(i32::from(figures.is_some()));
            }
            Body::Redd => {
                v.extend([i32::from(self.red_low), i32::from(self.red_high), i32::from(self.priority)]);
                v.extend(Self::pair(self.power.saturating_long()));
                v.extend(Self::pair(self.delta.saturating_long()));
                // The wheel's turn per tick, whole degrees (`getSpeed`: 0 to 15).
                v.push(self.speed().round().clamp(0.0, 15.0) as i32);
            }
        }
        v
    }

    /// `receiveControl`: 0 the mode without redstone, 1 the one with it, 2 the priority.
    fn button(&mut self, id: u32) -> bool {
        let mut base = self.base();
        base.receive_control(BatteryControl { low: id == 0, high: id == 1, priority: id == 2 });
        self.keep_base(&base);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::{self, Dir as BarDir};
    use crate::server::container_bridge::world_cell;

    const LEAD: &str = "ntm:battery_pack__battery_lead";
    const LEAD_MAX: i64 = 1_000 * 20 * 60 * 15;

    fn pack(id: &str, charge: Option<i64>) -> Option<Stack> {
        let mut s = Stack::new(id, 1);
        if let Some(c) = charge {
            s.custom = Some(format!("{{charge:{c}L}}"));
        }
        Some(s)
    }

    fn socket(item: Option<Stack>, mode: i16, powered: bool) -> Storage {
        let mut s = Storage::new(Body::Socket);
        s.kept = vec![item];
        s.red_low = mode;
        s.red_high = mode;
        s.powered = powered;
        s
    }

    fn charge(s: &Storage) -> Option<i64> {
        s.kept[0].as_ref().and_then(charge_of)
    }

    /// The volume of the block in the placer's frame, as `MultiblockHandlerXR` fills it
    /// (`getDimensions` with the placer looking away from the Java direction).
    fn volume(body: Body, core: Pos, facing: &str) -> std::collections::BTreeSet<Pos> {
        let f = crate::registry::facing_dir(facing).unwrap();
        let (min, max) = match body {
            Body::Socket => ([-1, 0, 0], [0, 1, 1]),
            Body::Redd => ([-4, 0, -2], [4, 9, 2]),
        };
        let mut out = std::collections::BTreeSet::new();
        for x in min[0]..=max[0] {
            for y in min[1]..=max[1] {
                for z in min[2]..=max[2] {
                    out.insert(world_cell(core, f, [x, y, z]));
                }
            }
        }
        out
    }

    #[test]
    fn the_ports_sit_on_the_edge_of_the_volume_and_open_outwards_for_every_facing() {
        let core = [100, 70, 100];
        for facing in ["north", "south", "east", "west"] {
            for (block, body, count) in [(SOCKET, Body::Socket, 8), (REDD, Body::Redd, 6)] {
                let cells = volume(body, core, facing);
                assert!(cells.contains(&core), "{block} {facing}: the core is in its volume");
                let ports = Storage::port_cells(block, core, Some(facing));
                assert_eq!(ports.len(), count, "{block} {facing}");
                for (cell, face) in &ports {
                    assert!(cells.contains(cell), "{block} {facing}: port {cell:?} inside");
                    let out = [cell[0] + face[0], cell[1] + face[1], cell[2] + face[2]];
                    assert!(!cells.contains(&out), "{block} {facing}: the face of {cell:?} leaves the volume");
                    assert_eq!(face[1], 0, "horizontal");
                }
                let distinct: std::collections::BTreeSet<_> = ports.iter().collect();
                assert_eq!(distinct.len(), count, "no port twice");
                // `getPortPos`, the redstone cells, are cells of the volume too.
                for cell in Storage::redstone_cells(block, core, Some(facing)) {
                    assert!(cells.contains(&cell), "{block} {facing}: redstone cell {cell:?}");
                }
            }
        }
        // Without a facing there is no geometry.
        assert!(Storage::port_cells(SOCKET, core, None).is_empty());
        assert!(Storage::redstone_cells(REDD, core, None).is_empty());
        assert!(Storage::port_cells("ntm:machine_battery", core, Some("north")).is_empty());
    }

    #[test]
    fn the_socket_ports_are_the_javas_eight() {
        // `MachineBatterySocket.getAllPorts` for a block placed facing north (`dir` north: `rotate` east):
        // cells {0,0}, {0,1} (z+1, the back), {1,0}, {1,1}; two ports on each of the four sides.
        let ports = Storage::port_cells(SOCKET, [0, 70, 0], Some("north"));
        let mut cells: Vec<_> = ports.iter().map(|(c, f)| (c[0], c[2], f[0], f[2])).collect();
        cells.sort();
        assert_eq!(
            cells,
            vec![(0, 0, -1, 0), (0, 0, 0, -1), (0, 1, -1, 0), (0, 1, 0, 1), (1, 0, 0, -1), (1, 0, 1, 0), (1, 1, 0, 1), (1, 1, 1, 0)]
        );
    }

    #[test]
    fn the_modes_make_a_socket_a_provider_a_receiver_both_or_neither() {
        let full = pack(LEAD, Some(LEAD_MAX / 2));
        // discharge rate 1000, charge rate 10 000
        for (mode, offer, demand) in [(MODE_INPUT, 0, 10_000), (MODE_BUFFER, 1_000, 10_000), (MODE_OUTPUT, 1_000, 0), (MODE_NONE, 0, 0)] {
            let s = socket(full.clone(), mode, false);
            assert_eq!((s.offer(), s.demand()), (offer, demand), "mode {mode}");
        }
        // The redstone picks the mode: low without a signal, high with one.
        let mut s = socket(full.clone(), MODE_INPUT, false);
        s.red_high = MODE_OUTPUT;
        assert_eq!((s.offer(), s.demand()), (0, 10_000));
        s.powered = true;
        assert_eq!((s.offer(), s.demand()), (1_000, 0));
        // Nothing in the slot, or something that is no battery: nothing moves.
        for item in [None, Some(Stack::new("ntm:ingot_steel", 1))] {
            let s = socket(item, MODE_BUFFER, false);
            assert_eq!((s.offer(), s.demand()), (0, 0));
        }
    }

    #[test]
    fn a_pack_gives_and_takes_only_what_it_has_room_for_and_the_stack_keeps_it() {
        let mut s = socket(pack(LEAD, Some(500)), MODE_BUFFER, false);
        assert_eq!(s.offer(), 500, "a pack gives what it has when that is under its rate");
        s.take(200);
        assert_eq!(charge(&s), Some(300));
        s.give(10_000);
        assert_eq!(charge(&s), Some(10_300));
        // The stack's custom data is the NBT: other keys stay.
        s.kept[0].as_mut().unwrap().custom = Some("{charge:10300L,note:\"kept\"}".into());
        s.give(1);
        let custom = s.kept[0].as_ref().unwrap().custom.clone().unwrap();
        assert!(custom.contains("charge:10301l") && custom.contains("note"), "{custom}");
        // Full: it takes the room, no more.
        let mut full = socket(pack(LEAD, Some(LEAD_MAX - 5)), MODE_INPUT, false);
        assert_eq!(full.demand(), 5);
        full.give(5);
        assert_eq!((charge(&full), full.demand()), (Some(LEAD_MAX), 0));
    }

    #[test]
    fn a_pack_with_no_tag_gets_one_set_to_zero_as_getcharge_does() {
        let mut s = socket(pack(LEAD, None), MODE_INPUT, false);
        assert!(s.tick(), "the stack changed");
        assert_eq!(s.kept[0].as_ref().unwrap().custom.as_deref(), Some("{charge:0l}"));
        assert!(!s.tick(), "and then nothing does");
    }

    #[test]
    fn the_creative_battery_never_runs_down_and_never_fills() {
        let mut s = socket(Some(Stack::new(CREATIVE_BATTERY, 1)), MODE_BUFFER, false);
        let rate = i64::MAX / 100;
        assert_eq!(s.offer(), rate as u64);
        s.take(1_000_000);
        s.give(1_000_000);
        assert_eq!(s.kept[0].as_ref().unwrap().custom, None, "nothing is written to it");
        assert_eq!(s.offer(), rate as u64);
    }

    #[test]
    fn the_comparator_reads_the_pack_and_the_screen_has_its_figures() {
        let mut s = socket(pack(LEAD, Some(LEAD_MAX / 2)), MODE_INPUT, false);
        s.tick();
        assert_eq!(s.last_redstone, 8, "round(7.5)");
        let json = serde_json::to_string(&crate::machine::Machine::new(crate::machine::Kind::Storage, SOCKET, Some("north".into()))).unwrap();
        assert_eq!(crate::native::comparator_signal(&json), 0);
        let mut m: crate::machine::Machine = serde_json::from_str(&json).unwrap();
        if let crate::machine::Data::Storage(x) = &mut m.data {
            x.last_redstone = 11;
        }
        assert_eq!(crate::native::comparator_signal(&serde_json::to_string(&m).unwrap()), 11);
        let d = s.screen_data();
        assert_eq!(d.len(), 12);
        assert_eq!(screens::format("{5.6:si} / {7.8:si} HE", &d), "9M / 18M HE");
        assert_eq!(d[11], 1, "has a pack");
        assert_eq!(socket(None, MODE_INPUT, false).screen_data()[11], 0);
    }

    #[test]
    fn the_bar_shows_the_javas_pixels() {
        // `p = power * 52 / maxPower`, drawn bottom up.
        for p in 0..=52i64 {
            let c = (p * LEAD_MAX + 51) / 52;
            assert_eq!(c * 52 / LEAD_MAX, p, "the test's charge for {p} pixels");
            let d = socket(pack(LEAD, Some(c)), MODE_INPUT, false).screen_data();
            let f = screens::fill(d[0], 0, d[1]);
            let shown = f.and_then(|f| screens::bar_part([62.0, 17.0, 34.0, 52.0], BarDir::Up, f)).map_or(0.0, |part| part[3]);
            assert_eq!(shown as i64, p, "{p} pixels");
        }
        // A charge past `Long.MAX_VALUE / 100` is scaled before the ratio (the creative battery: half the maximum).
        let d = socket(Some(Stack::new(CREATIVE_BATTERY, 1)), MODE_INPUT, false).screen_data();
        let f = screens::fill(d[0], 0, d[1]).unwrap();
        assert_eq!(screens::bar_part([62.0, 17.0, 34.0, 52.0], BarDir::Up, f).unwrap()[3], 26.0);
    }

    #[test]
    fn the_buttons_step_the_modes_and_the_priority_as_receivecontrol() {
        let mut s = Storage::new(Body::Socket);
        assert_eq!((s.red_low, s.red_high, s.priority), (MODE_INPUT, MODE_OUTPUT, 1), "redLow 0, redHigh 2, LOW");
        for expected in [1, 2, 3, 0, 1] {
            s.button(0);
            assert_eq!(s.red_low, expected);
        }
        for expected in [3, 0, 1] {
            s.button(1);
            assert_eq!(s.red_high, expected);
        }
        for expected in [2, 3, 1, 2] {
            s.button(2);
            assert_eq!(s.priority, expected, "LOW, NORMAL, HIGH and back to LOW");
        }
        assert_eq!((s.red_low, s.red_high), (1, 1), "the priority button touches no mode");
        // An out of range priority (LOWEST, HIGHEST) is put back to LOW by the tick.
        for bad in [0u8, 4] {
            s.priority = bad;
            assert!(s.tick());
            assert_eq!(s.priority, 1);
        }
    }

    #[test]
    fn the_redd_keeps_a_store_no_long_could_and_loads_its_pack() {
        let mut r = Storage::new(Body::Redd);
        r.red_low = MODE_BUFFER;
        r.kept = vec![pack(LEAD, Some(5_000)), None];
        r.tick();
        // slot 0 feeds it: min(max - 0, discharge rate 1000, charge)
        assert_eq!(r.power, BigInt::from_i64(1_000));
        assert_eq!(charge_of(r.kept[0].as_ref().unwrap()), Some(4_000));
        // It takes more than a long holds.
        for _ in 0..200 {
            r.give(REDD_MAX as u64);
        }
        let mut expected = BigInt::from_i64(1_000);
        for _ in 0..200 {
            expected = expected.add_long(REDD_MAX);
        }
        assert_eq!(r.power, expected, "200 transfers of the largest rate: more than a long");
        assert!(r.power.saturating_long() == i64::MAX);
        // What it offers is the window of the store (half of `max`): the rest stays.
        assert_eq!(r.offer(), (REDD_MAX / 2) as u64, "in buffer mode");
        let before = r.power.clone();
        r.take(r.offer());
        assert_eq!(before.subtract_long(REDD_MAX / 2), r.power);
    }

    #[test]
    fn the_redd_charges_the_pack_in_slot_one_from_its_store() {
        let mut r = Storage::new(Body::Redd);
        r.power = BigInt::from_i64(2_000_000);
        r.kept = vec![None, pack(LEAD, Some(0))];
        r.tick();
        // `chargeItemsFromTE`: the pack's charge rate, 10 000
        assert_eq!(charge_of(r.kept[1].as_ref().unwrap()), Some(10_000));
        assert_eq!(r.power, BigInt::from_i64(1_990_000));
        // A creative battery in slot 1 takes everything within the window... and the REDD keeps nothing.
        r.kept[1] = Some(Stack::new(CREATIVE_BATTERY, 1));
        r.tick();
        assert_eq!(r.power, BigInt::zero());
    }

    #[test]
    fn the_redd_saves_its_power_as_java_bytes_and_nothing_else_of_its_log() {
        let mut r = Storage::new(Body::Redd);
        let mut huge = BigInt::from_i64(i64::MAX);
        for _ in 0..4 {
            huge = huge.add(&huge.clone());
        }
        r.power = huge.clone();
        r.red_low = MODE_NONE;
        r.priority = 3;
        r.last_redstone = 4;
        let json = serde_json::to_string(&r).unwrap();
        let back: Storage = serde_json::from_str(&json).unwrap();
        assert_eq!((back.power.clone(), back.red_low, back.red_high, back.priority, back.last_redstone), (huge.clone(), MODE_NONE, MODE_OUTPUT, 3, 4));
        // The Java's `power.toByteArray()`: big endian, two's complement, as `BigInteger`.
        assert!(json.contains(&format!("{:?}", huge.to_java_bytes()).replace(' ', "")), "{json}");
        // The screen shows a long; the store can be larger.
        let d = back.screen_data();
        assert_eq!(screens::format("{3.4:int}", &d), "9,223,372,036,854,775,807", "saturated at the largest long");
        // An old save with only the body.
        let old: Storage = serde_json::from_str(r#"{"body":"Redd"}"#).unwrap();
        assert_eq!((old.power, old.red_low, old.red_high, old.priority), (BigInt::zero(), 0, 2, 1));
        assert!(serde_json::from_str::<Storage>(r#"{"body":"Redd","power":[]}"#).is_err());
    }

    #[test]
    fn the_redd_log_gives_the_delta_over_twenty_ticks() {
        let mut r = Storage::new(Body::Redd);
        r.red_low = MODE_BUFFER;
        for _ in 0..25 {
            r.give(100);
            r.tick();
        }
        // avg is the mean of the power before and after the tick (no items: the same power); the log is
        // 20 averages long: delta = newest - oldest of those it still holds at the moment, as the Java.
        assert_eq!(r.delta, BigInt::from_i64(2_000), "the newest average less the one 20 ticks before: 2500 - 500");
        assert!(r.log.iter().all(Option::is_some));
    }

    #[test]
    fn a_socket_delta_is_the_same_log_in_longs() {
        let mut s = socket(pack(LEAD, Some(0)), MODE_INPUT, false);
        for _ in 0..22 {
            s.give(100);
            s.tick();
        }
        assert_eq!(s.socket_delta, 2000, "the newest average less the one 20 ticks before: 2200 - 200");
        assert_eq!(s.socket_log[19], 2200);
    }

    #[test]
    fn the_redd_wheel_turns_only_with_a_great_store_and_never_past_fifteen() {
        let mut r = Storage::new(Body::Redd);
        assert_eq!(r.screen_data()[7], 0);
        r.power = BigInt::from_i64(1_000_000_000_000);
        // (ln(5e10) * 0.05)^5 = about 2.8 degrees a tick
        assert!((r.speed() - 2.85).abs() < 0.1, "{}", r.speed());
        assert_eq!(r.screen_data()[7], 3);
        r.power = BigInt::from_i64(i64::MAX);
        assert_eq!(r.screen_data()[7], 15);
        assert_eq!(Storage::new(Body::Socket).speed(), 0.0);
    }

    #[test]
    fn a_saved_machine_takes_its_slots_as_the_engine_has_them() {
        // The copy is not saved: the slots are, in the same commit as the state, and are what the machine reads.
        let mut s = Storage::new(Body::Socket);
        s.kept = vec![pack(LEAD, Some(900))];
        let json = serde_json::to_string(&s).unwrap();
        assert!(!json.contains("charge") && !json.contains("kept"), "{json}");
        let load = |slot: Option<Stack>| {
            let mut s = serde_json::from_str::<Storage>(&json).unwrap();
            assert_eq!(s.publishable(&vec![slot.clone()]), None, "nothing to be written over the slots before they are read");
            *s.slots_mut() = vec![slot];
            s.sync_in();
            s.kept[0].clone()
        };
        // Whatever is in the slot, exactly: a pack swapped for another of the same kind, with another charge, is that one.
        assert_eq!(load(pack(LEAD, Some(500))), pack(LEAD, Some(500)));
        assert_eq!(load(pack(LEAD, Some(0))), pack(LEAD, Some(0)));
        assert_eq!(load(pack("ntm:battery_pack__battery_lithium", Some(1))), pack("ntm:battery_pack__battery_lithium", Some(1)));
        assert_eq!(load(None), None);
        // An old save with the copy in it still reads.
        let old: Storage = serde_json::from_str(r#"{"body":"Socket","kept":[{"id":"x","count":1}]}"#).unwrap();
        assert!(old.kept.is_empty());
    }

    #[test]
    fn the_slots_get_the_copy_at_the_end_of_the_tick_and_a_pack_taken_then_is_what_the_network_left() {
        let mut s = Storage::new(Body::Socket);
        s.red_low = MODE_OUTPUT;
        s.kept = vec![pack(LEAD, Some(5_000))];
        s.sync_out();
        // The network took what the socket gave.
        let gave = s.offer();
        s.take(gave);
        assert_eq!(charge(&s), Some(5_000 - gave as i64));
        assert_eq!(s.slots[0].as_ref().and_then(charge_of), Some(5_000), "the slot is the step's");
        let publish = s.publishable(&s.slots.clone()).unwrap();
        assert_eq!(publish[0].as_ref().and_then(charge_of), Some(5_000 - gave as i64));
        s.published();
        assert_eq!(s.slots[0].as_ref().and_then(charge_of), Some(5_000 - gave as i64));
        // The next step reads the slots it just wrote: no change from outside, the copy stays.
        s.sync_in();
        assert_eq!(charge(&s), Some(5_000 - gave as i64));
    }

    #[test]
    fn the_copy_is_never_written_over_a_slot_a_player_changed_since_the_machine_wrote_it() {
        let mut s = Storage::new(Body::Socket);
        s.red_low = MODE_OUTPUT;
        s.kept = vec![pack(LEAD, Some(5_000))];
        s.sync_out();
        let written = s.slots.clone();
        let gave = s.offer();
        s.take(gave);
        // The slots are as the machine left them: the network's move is to be written.
        assert!(s.publishable(&written).is_some());
        // A player took the pack out: giving the copy back would duplicate it.
        assert_eq!(s.publishable(&vec![None]), None);
        // A player put another in: writing None over it would take it away.
        let mut empty = Storage::new(Body::Socket);
        empty.synced = true;
        assert_eq!(empty.publishable(&vec![pack(LEAD, Some(9))]), None);
        // Nothing to write when they already are the copy.
        assert_eq!(empty.publishable(&vec![None]), None);
        // The REDD's two slots, each as it is: a change in either holds the copy back.
        let mut r = Storage::new(Body::Redd);
        r.kept = vec![pack(LEAD, Some(5)), None];
        r.sync_out();
        let base = r.slots.clone();
        r.kept[0] = pack(LEAD, Some(4));
        assert!(r.publishable(&base).is_some());
        let mut other = base.clone();
        other[1] = pack(LEAD, Some(1));
        assert_eq!(r.publishable(&other), None);
    }

    #[test]
    fn the_engines_spelling_of_the_item_data_is_the_same_data() {
        // The engine writes `custom_data` back as `{charge: 14925000L}`; the machine wrote `{charge:14925000l}`.
        let mut s = Storage::new(Body::Socket);
        s.red_low = MODE_OUTPUT;
        s.kept = vec![pack(LEAD, Some(5_000))];
        s.sync_out();
        let gave = s.offer();
        assert_eq!(gave, 1_000);
        s.take(gave);
        let mut engine = s.slots.clone();
        engine[0].as_mut().unwrap().custom = Some("{charge: 5000L}".into());
        assert!(s.publishable(&engine).is_some(), "the slot is the machine's own: the copy is written");
        engine[0].as_mut().unwrap().custom = Some("{charge: 4999L}".into());
        assert_eq!(s.publishable(&engine), None, "another charge is a player's");
    }

    #[test]
    fn what_a_player_changes_in_a_slot_wins_over_the_copy_and_the_rest_does_not() {
        let mut s = Storage::new(Body::Socket);
        s.kept = vec![pack(LEAD, Some(100))];
        s.sync_out();
        // The network moved energy after the write.
        s.give(50);
        assert_eq!(charge(&s), Some(150));
        // The engine's slot is the old write: nobody touched it, the copy stays.
        s.sync_in();
        assert_eq!(charge(&s), Some(150));
        s.sync_out();
        assert_eq!(s.slots[0].as_ref().and_then(charge_of), Some(150));
        // A player takes the pack out and puts another in: the slot is the truth.
        *s.slots_mut() = vec![pack("ntm:battery_pack__capacitor_gold", Some(7))];
        s.sync_in();
        assert_eq!(s.kept[0], pack("ntm:battery_pack__capacitor_gold", Some(7)));
        s.sync_out();
        assert_eq!(s.slots[0], pack("ntm:battery_pack__capacitor_gold", Some(7)));
    }

    #[test]
    fn the_redd_reads_its_comparator_before_it_moves_its_items() {
        let mut r = Storage::new(Body::Redd);
        r.kept = vec![Some(Stack::new(CREATIVE_BATTERY, 1)), None];
        r.tick();
        assert_eq!(r.last_redstone, 0, "the store was empty when the base class read it");
        r.tick();
        assert_eq!(r.last_redstone, 8, "the window is half of the maximum: round(7.5)");
    }

    #[test]
    fn something_that_is_no_battery_in_the_slot_is_no_pack_on_the_screen() {
        let s = socket(Some(Stack::new("ntm:ingot_steel", 1)), MODE_INPUT, false);
        let d = s.screen_data();
        assert_eq!((d[0], d[11]), (0, 0));
        assert_eq!(s.power_figures(), (0, 0));
    }

    #[test]
    fn a_big_integer_clamps_to_a_long_for_the_screen() {
        for v in [0, 1, -1, i64::MAX, i64::MIN, i64::MAX - 1, i64::MIN + 1] {
            assert_eq!(BigInt::from_i64(v).saturating_long(), v);
        }
        let over = BigInt::from_i64(i64::MAX).add_long(1);
        assert_eq!(over.saturating_long(), i64::MAX);
        assert_eq!(over.negate().saturating_long(), i64::MIN);
        assert_eq!(over.negate().add_long(-1).saturating_long(), i64::MIN);
        let mut big = BigInt::from_i64(i64::MAX);
        for _ in 0..3 {
            big = big.add(&big.clone());
        }
        assert_eq!((big.saturating_long(), big.negate().saturating_long()), (i64::MAX, i64::MIN));
    }

    #[test]
    fn every_battery_item_is_taken_and_nothing_else() {
        let items = battery_items();
        assert_eq!(items.len(), 13);
        for id in &items {
            assert_ne!(slot_item(id), SlotItem::Other, "{id}");
        }
        assert_eq!(slot_item("ntm:ingot_steel"), SlotItem::Other);
        assert_eq!(slot_item("ntm:battery_pack__battery_quantum"), SlotItem::Pack(5));
        assert_eq!(slot_item("ntm:battery_pack__capacitor_spark"), SlotItem::Pack(11));
    }
}
