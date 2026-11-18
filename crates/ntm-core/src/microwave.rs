//! The microwave (`ntm:machine_microwave`, `MachineMicrowave.java`, `TileEntityMicrowave.java`, `RenderMicrowave.java`).
//!
//! A single block that cooks food with the furnace's recipes (`FurnaceRecipes.smelting()`, the port's `furnace_recipes.json`): one
//! input, one output, a battery slot, 50 000 HE of buffer. Every tick it can process (a speed above 0, 50 HE, an input
//! with a smelting result of which the input or the result is food, room in the output) it takes 50 HE and its
//! progress grows by `speed * 2`; at 300 it makes one result. Two buttons raise and lower the speed, 0 to 5.
//!
//! At the top speed (5) the Java breaks the block and explodes (`ExplosionVNT` of 5, with damage). The port has no explosion a machine
//! may ask for outright (only the ones recorded as world effects), so a microwave at 5 does not run at all
//! (it neither processes nor takes power): a nominal gap (docs/onda-2-sumidouros.md).
//!
//! The battery slot is a `Slot` whose `isItemValidForSlot` is `false` (it only takes slot 0), so a hand or a hopper puts nothing there;
//! `chargeTEFromItems` still reads whatever is in it (a command can put a battery there).

use crate::{
    energy::{EnergyNode, Role},
    energy_network::charge_te_from_items,
    machine::Logic,
    recipes,
    server::container_bridge::{Slots, Slotted},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub const BLOCK: &str = "ntm:machine_microwave";

/// `maxPower`, `consumption`, `maxTime` and `maxSpeed`.
pub const MAX_POWER: u64 = 50_000;
pub const CONSUMPTION: u64 = 50;
pub const MAX_TIME: i32 = 300;
pub const MAX_SPEED: i32 = 5;

pub const INPUT: usize = 0;
pub const OUTPUT: usize = 1;
pub const BATTERY: usize = 2;
pub const SLOTS: usize = 3;

/// A stack's `getMaxStackSize`: 64 for every food of the table.
const STACK: u32 = 64;

/// The items that are `ItemFood`, among those the furnace's recipes use: the raw and cooked meats and fish, the potatoes, the kelp and
/// the glyphid meat.
pub const FOODS: &[&str] = &[
    "minecraft:beef", "minecraft:cooked_beef", "minecraft:porkchop", "minecraft:cooked_porkchop", "minecraft:mutton", "minecraft:cooked_mutton",
    "minecraft:chicken", "minecraft:cooked_chicken", "minecraft:rabbit", "minecraft:cooked_rabbit", "minecraft:cod", "minecraft:cooked_cod",
    "minecraft:salmon", "minecraft:cooked_salmon", "minecraft:potato", "minecraft:baked_potato", "minecraft:kelp", "minecraft:dried_kelp",
    "ntm:glyphid_meat", "ntm:glyphid_meat_grilled",
];

pub fn is_food(item: &str) -> bool {
    FOODS.contains(&item)
}

/// `isItemValidForSlot`: only slot 0, and only what has a smelting result.
pub fn accepts(slot: usize, item: &str) -> bool {
    slot == INPUT && recipes::furnace().smelt(item).is_some()
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Microwave {
    pub power: u64,
    /// 0 to 5 (saved as `speed`).
    pub speed: i32,
    /// The progress: not saved by the Java (`readFromNBT` reads only `power` and `speed`).
    #[serde(skip)]
    pub time: i32,
    #[serde(skip)]
    pub slots: Slots,
}

impl Microwave {
    fn fit(&mut self) {
        self.slots.resize(SLOTS, None);
    }

    /// `canProcess`.
    pub fn can_process(&self) -> bool {
        if self.speed == 0 || self.power < CONSUMPTION {
            return false;
        }
        let Some(input) = self.slots.get(INPUT).and_then(Option::as_ref) else { return false };
        let Some(smelt) = recipes::furnace().smelt(&input.id) else { return false };
        if !is_food(&input.id) && !is_food(&smelt.output) {
            return false;
        }
        match self.slots.get(OUTPUT).and_then(Option::as_ref) {
            None => true,
            Some(out) => out.id == smelt.output && out.damage == 0 && smelt.count + out.count <= STACK,
        }
    }

    /// `process`: one result in the output, one input less.
    fn process(&mut self) {
        let Some(input) = self.slots[INPUT].as_ref() else { return };
        let Some(smelt) = recipes::furnace().smelt(&input.id) else { return };
        let (id, count) = (smelt.output.clone(), smelt.count);
        match self.slots[OUTPUT].as_mut() {
            None => self.slots[OUTPUT] = Some(crate::server::container_bridge::Stack::new(&id, count)),
            Some(out) => out.count += count,
        }
        if let Some(i) = self.slots[INPUT].as_mut() {
            i.count -= 1;
            if i.count == 0 {
                self.slots[INPUT] = None;
            }
        }
    }

    /// `updateEntity` (server side). True when what is saved or shown changed.
    pub fn step(&mut self) -> bool {
        self.fit();
        let before = (self.power, self.time, self.slots.clone());
        // `chargeTEFromItems(slots, 2, power, maxPower)`.
        let battery = crate::storage::item_slot(&self.slots[BATTERY]);
        self.power = charge_te_from_items(battery.as_ref(), self.power as i64, MAX_POWER as i64).max(0) as u64;
        crate::storage::write_back(&mut self.slots[BATTERY], battery.as_ref());
        if self.can_process() {
            // The Java explodes here (`speed >= maxSpeed`): see the module's note.
            if self.speed >= MAX_SPEED {
                return before != (self.power, self.time, self.slots.clone());
            }
            if self.time >= MAX_TIME {
                self.process();
                self.time = 0;
            }
            if self.can_process() {
                self.power -= CONSUMPTION;
                self.time += self.speed * 2;
            }
        }
        before != (self.power, self.time, self.slots.clone())
    }
}

impl Logic for Microwave {
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
        ViewModel::new("Micro-ondas")
            .bar("Energia", self.power, MAX_POWER, "HE")
            .text(format!("Velocidade: {} / {}", self.speed, MAX_SPEED), view::GREY)
    }
}

impl EnergyNode for Microwave {
    fn role(&self) -> Role {
        Role::Consumer
    }
    fn offer(&self) -> u64 {
        0
    }
    /// A receiver takes what its buffer has room for.
    fn demand(&self) -> u64 {
        MAX_POWER.saturating_sub(self.power)
    }
    fn give(&mut self, he: u64) {
        self.power = self.power.saturating_add(he).min(MAX_POWER);
    }
    fn take(&mut self, _he: u64) {}
}

/// `i * value / max`, as `getPowerScaled` and the other scaled getters do.
fn scaled(value: i64, max: i64, px: i64) -> i64 {
    (value * px / max.max(1)).max(0)
}

/// The bars' values as the screen draws them. `bar_value` keeps a bar of `px` pixels exact: `2p - 1` of `2 * px`.
fn bar_value(pixels: i64) -> i32 {
    (2 * pixels - 1).max(0) as i32
}

/// The native screen (`horizon_screen/machine_microwave.json`, `GUIMicrowave`): data
/// [power bar (34 px), its max, progress bar (22 px), its max, speed bar (34 px), its max, power, max power, speed, spin];
/// button 0 raises the speed and button 1 lowers it (`handleButtonPacket`), both clamped to 0..5.
impl Slotted for Microwave {
    fn slots_mut(&mut self) -> &mut Slots {
        self.fit();
        &mut self.slots
    }

    fn screen_data(&self) -> Vec<i32> {
        let power = scaled(self.power as i64, MAX_POWER as i64, 34);
        // `getProgressScaled(23)`, at most 22.
        let progress = scaled(i64::from(self.time), i64::from(MAX_TIME), 23).min(22);
        let speed = scaled(i64::from(self.speed), i64::from(MAX_SPEED), 34);
        let spin = if self.time > 0 { self.speed } else { 0 };
        vec![
            bar_value(power), 68, bar_value(progress), 44, bar_value(speed), 68,
            self.power.min(i32::MAX as u64) as i32, MAX_POWER as i32, self.speed, spin,
        ]
    }

    fn button(&mut self, id: u32) -> bool {
        match id {
            0 => self.speed += 1,
            1 => self.speed -= 1,
            _ => {}
        }
        self.speed = self.speed.clamp(0, MAX_SPEED);
        false
    }
}

/// `getAccessibleSlotsFromSide`: the bottom reaches the output (slot 1), every other side the input (slot 0);
/// `canExtractItem` is slot 1 only.
pub fn container() -> crate::native::Container {
    crate::native::Container {
        slots: SLOTS,
        faces: [vec![OUTPUT], vec![INPUT], vec![INPUT], vec![INPUT], vec![INPUT], vec![INPUT]],
        inputs: vec![INPUT],
        outputs: vec![OUTPUT],
        menu: Some("minecraft:generic_9x3"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::container_bridge::Stack;

    fn with(item: &str, n: u32, speed: i32) -> Microwave {
        let mut m = Microwave { power: MAX_POWER, speed, ..Default::default() };
        m.slots = vec![Some(Stack::new(item, n)), None, None];
        m
    }

    #[test]
    fn a_raw_meat_cooks_at_the_progress_the_speed_gives() {
        // speed 1: +2 a tick, 300 = 150 ticks, then one is made on the next tick.
        let mut m = with("minecraft:beef", 2, 1);
        let mut ticks = 0;
        while m.slots[OUTPUT].is_none() && ticks < 400 {
            m.step();
            ticks += 1;
        }
        assert_eq!(ticks, 151, "150 ticks to reach 300 and the tick that processes");
        assert_eq!(m.slots[OUTPUT], Some(Stack::new("minecraft:cooked_beef", 1)));
        assert_eq!(m.slots[INPUT].as_ref().map(|s| s.count), Some(1));
        assert_eq!(m.time, 2, "it goes on in the same tick (`canProcess` again)");
        // 150 ticks of 50 HE, then the one after the process
        assert_eq!(MAX_POWER - m.power, 151 * 50, "50 HE a working tick (`consumption`)");
    }

    #[test]
    fn a_higher_speed_is_faster_and_costs_the_same_per_tick() {
        // ceil(300 / (2 * speed)) ticks of 50 HE to reach 300, and the tick that processes: 150, 75, 50 and 38 (a 304) plus one
        for (speed, ticks) in [(1, 151), (2, 76), (3, 51), (4, 39)] {
            let mut m = with("minecraft:chicken", 1, speed);
            let mut n = 0;
            while m.slots[OUTPUT].is_none() {
                m.step();
                n += 1;
                assert!(n < 500);
            }
            assert_eq!(n, ticks, "speed {speed}");
        }
    }

    #[test]
    fn only_food_cooks() {
        // iron ore smelts in the furnace but is not food, nor is its ingot.
        let mut m = with("minecraft:iron_ore", 1, 3);
        assert!(!m.can_process());
        assert!(!m.step());
        // a glyphid's meat is food: input or output is enough
        assert!(with("ntm:glyphid_meat", 1, 1).can_process());
        assert!(with("minecraft:potato", 1, 1).can_process());
    }

    #[test]
    fn it_stops_without_speed_power_or_room_and_keeps_its_progress() {
        let mut m = with("minecraft:beef", 1, 0);
        assert!(!m.can_process(), "speed 0");
        m.speed = 2;
        m.power = CONSUMPTION - 1;
        assert!(!m.can_process(), "power under the 50 it takes");
        m.power = CONSUMPTION;
        assert!(m.can_process(), "exactly 50 is enough");
        // another result in the output: not the same item
        m.slots[OUTPUT] = Some(Stack::new("minecraft:cooked_chicken", 1));
        assert!(!m.can_process());
        // the same item up to the stack
        m.slots[OUTPUT] = Some(Stack::new("minecraft:cooked_beef", 63));
        assert!(m.can_process());
        m.slots[OUTPUT] = Some(Stack::new("minecraft:cooked_beef", 64));
        assert!(!m.can_process(), "64 + 1 does not fit");
        // an output with damage is not the same item
        m.slots[OUTPUT] = Some(Stack { damage: 1, ..Stack::new("minecraft:cooked_beef", 1) });
        assert!(!m.can_process());
        // nothing to cook
        m.slots[INPUT] = None;
        m.slots[OUTPUT] = None;
        assert!(!m.can_process());
    }

    #[test]
    fn the_results_pile_up_and_the_input_runs_out() {
        let mut m = with("minecraft:cod", 2, 4);
        for _ in 0..200 {
            m.step();
        }
        assert_eq!(m.slots[OUTPUT], Some(Stack::new("minecraft:cooked_cod", 2)));
        assert_eq!(m.slots[INPUT], None);
        let power = m.power;
        m.step();
        assert_eq!(m.power, power, "idle: it takes nothing");
    }

    #[test]
    fn at_the_top_speed_it_does_not_run_where_the_java_blows_up() {
        let mut m = with("minecraft:beef", 3, MAX_SPEED);
        assert!(m.can_process());
        for _ in 0..50 {
            assert!(!m.step());
        }
        assert_eq!((m.time, m.power), (0, MAX_POWER));
        assert_eq!(m.slots[OUTPUT], None);
    }

    #[test]
    fn the_buttons_step_the_speed_inside_its_bounds() {
        let mut m = Microwave::default();
        for expected in [1, 2, 3, 4, 5, 5] {
            m.button(0);
            assert_eq!(m.speed, expected);
        }
        for expected in [4, 3, 2, 1, 0, 0] {
            m.button(1);
            assert_eq!(m.speed, expected);
        }
        m.button(7);
        assert_eq!(m.speed, 0);
    }

    #[test]
    fn a_battery_in_the_slot_charges_it_and_a_creative_one_fills_it() {
        let mut m = Microwave { slots: vec![None, None, Some(Stack::new(crate::machine::CREATIVE_BATTERY, 1))], ..Default::default() };
        m.step();
        assert_eq!(m.power, MAX_POWER);
        // a charged lead battery gives its rate (1 000 HE a tick)
        let mut pack = Stack::new("ntm:battery_pack__battery_lead", 1);
        pack.custom = Some("{charge:5000l}".into());
        let mut lead = Microwave { slots: vec![None, None, Some(pack)], ..Default::default() };
        lead.step();
        assert_eq!(lead.power, 1_000);
        assert_eq!(crate::storage::charge_of(lead.slots[BATTERY].as_ref().unwrap()), Some(4_000));
    }

    #[test]
    fn the_slots_take_what_the_java_takes() {
        assert!(accepts(INPUT, "minecraft:beef") && accepts(INPUT, "minecraft:iron_ore"));
        assert!(!accepts(INPUT, "minecraft:stick"), "no smelting result");
        assert!(!accepts(OUTPUT, "minecraft:beef") && !accepts(BATTERY, "minecraft:beef"));
        let c = container();
        assert_eq!((c.slots, c.inputs, c.outputs), (3, vec![INPUT], vec![OUTPUT]));
        assert_eq!(c.faces[0], vec![OUTPUT], "the bottom gives the output");
        assert!(c.faces[1..].iter().all(|f| *f == vec![INPUT]));
    }

    #[test]
    fn the_bars_show_the_javas_pixels() {
        use crate::screens::{self, Dir};
        let draw = |v: i32, max: i32, rect: [f32; 4]| -> f32 {
            screens::fill(v, 0, max).and_then(|f| screens::bar_part(rect, Dir::Up, f)).map_or(0.0, |p| p[3])
        };
        for p in 0..=34i64 {
            let power = (p * MAX_POWER as i64 + 34) / 34;
            let mut m = Microwave { power: power as u64, ..Default::default() };
            assert_eq!(scaled(m.power as i64, MAX_POWER as i64, 34), p, "the test's power for {p}");
            let d = m.screen_data();
            assert_eq!(draw(d[0], d[1], [8.0, 17.0, 16.0, 34.0]) as i64, p, "power bar {p}");
            m.power = 0;
        }
        for speed in 0..=5 {
            let m = Microwave { speed, ..Default::default() };
            let d = m.screen_data();
            let k = scaled(i64::from(speed), 5, 34);
            assert_eq!(draw(d[4], d[5], [62.0, 26.0, 4.0, 34.0]) as i64, k, "speed bar {speed}");
        }
        for t in [0, 1, 13, 150, 299, 300, 600] {
            let m = Microwave { time: t, ..Default::default() };
            let d = m.screen_data();
            let want = scaled(i64::from(t), 300, 23).min(22);
            let got = screens::fill(d[2], 0, d[3]).and_then(|f| screens::bar_part([104.0, 34.0, 22.0, 16.0], Dir::Right, f)).map_or(0.0, |p| p[2]);
            assert_eq!(got as i64, want, "progress {t}");
        }
    }

    #[test]
    fn it_takes_what_its_buffer_has_room_for_and_saves_power_and_speed_only() {
        let mut m = Microwave { power: 49_990, speed: 3, time: 120, ..Default::default() };
        assert_eq!((m.demand(), m.offer()), (10, 0));
        m.give(500);
        assert_eq!(m.power, MAX_POWER);
        let back: Microwave = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!((back.power, back.speed, back.time), (MAX_POWER, 3, 0));
    }
}
