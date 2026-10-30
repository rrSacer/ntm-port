//! The presses (`TileEntityMachinePress`, `TileEntityMachineEPress`): a
//! stamp, an input, an output; the burner press burns solid fuel (200 burn
//! ticks a pressing) and ramps its speed up, the electric one draws HE.
//!
//! Stamp wear: the server can't read an item's components, so a worn stamp
//! carries its wear out of a press as `minecraft:custom_data={ntm_wear:B}`,
//! `B` a bucket of 0..=6, and the take probe reads the bucket back with
//! component predicates (`Effect::TakeTagged`). Wear going out rounds up,
//! so taking a stamp out and back in never repairs it.

use crate::{
    energy::{EnergyNode, Role},
    server::container_bridge::{Slots, Slotted, Stack},
    machine::{Effect, Logic, burn_ticks},
    proto::Held,
    recipes::{self, PressRecipe},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

/// The custom-data key of a stamp's wear bucket.
pub const WEAR_KEY: &str = "ntm_wear";
/// Buckets 1..=6 (0: fresh, no tag).
pub const WEAR_BUCKETS: u8 = 6;
const STACK: u32 = 64;

pub mod actions {
    pub const INSERT: u32 = 1;
    pub const TAKE_OUTPUT: u32 = 2;
    pub const TAKE_STAMP: u32 = 3;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stamp {
    pub item: String,
    /// Pressings done.
    pub used: u32,
}

fn bucket_step(durability: u32) -> u32 {
    durability.div_ceil(u32::from(WEAR_BUCKETS)).max(1)
}

/// The bucket a stamp leaves with (rounded up).
pub fn wear_bucket(durability: u32, used: u32) -> u8 {
    if durability == 0 || used == 0 {
        return 0;
    }
    used.div_ceil(bucket_step(durability))
        .min(u32::from(WEAR_BUCKETS)) as u8
}

/// The wear a stamp of `bucket` comes back in with.
pub fn wear_of_bucket(durability: u32, bucket: u8) -> u32 {
    if durability == 0 {
        return 0;
    }
    (u32::from(bucket) * bucket_step(durability)).min(durability - 1)
}

/// The item string a stamp goes out as: tagged when worn.
pub fn stamp_item(s: &Stamp) -> String {
    let durability = recipes::press().stamp(&s.item).map_or(0, |k| k.durability);
    match wear_bucket(durability, s.used) {
        0 => s.item.clone(),
        b => format!("{}[minecraft:custom_data={{{WEAR_KEY}:{b}}}]", s.item),
    }
}

/// Stamp, input, output, and the ram: what both presses share.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PressCore {
    pub stamp: Option<Stamp>,
    pub input: Option<(String, u32)>,
    pub output: Option<(String, u32)>,
    /// The ram's travel, 0..=max_press.
    pub press: u32,
    pub retracting: bool,
    pub delay: u32,
}

impl PressCore {
    fn recipe(&self) -> Option<&'static PressRecipe> {
        let stamp = self.stamp.as_ref()?;
        let kind = recipes::press().stamp(&stamp.item)?;
        let (input, n) = self.input.as_ref()?;
        let r = recipes::press().recipe(input, &kind.kind)?;
        (*n >= r.input_count).then_some(r)
    }

    /// `canProcess` without the fuel or power check.
    fn can_process(&self) -> bool {
        let Some(r) = self.recipe() else { return false };
        match &self.output {
            None => true,
            Some((o, n)) => *o == r.output && n + r.count <= STACK,
        }
    }

    /// One pressing: output, input, stamp wear.
    fn press_one(&mut self) {
        let Some(r) = self.recipe() else { return };
        match &mut self.output {
            Some((_, n)) => *n += r.count,
            None => self.output = Some((r.output.clone(), r.count)),
        }
        if let Some((_, n)) = &mut self.input {
            *n -= r.input_count;
            if *n == 0 {
                self.input = None;
            }
        }
        if let Some(s) = &mut self.stamp {
            let durability = recipes::press().stamp(&s.item).map_or(0, |k| k.durability);
            if durability != 0 {
                s.used += 1;
                if s.used >= durability {
                    self.stamp = None;
                }
            }
        }
    }

    /// The ram moves `speed` toward the end (or back): true when a pressing
    /// was made. `retract_speed` is the speed back.
    fn ram(&mut self, can: bool, speed: u32, retract_speed: u32, max: u32, delay: u32) -> bool {
        if self.delay > 0 {
            self.delay -= 1;
            return false;
        }
        if self.retracting {
            self.press = self.press.saturating_sub(retract_speed);
            if self.press == 0 {
                self.retracting = false;
                self.delay = delay;
            }
        } else if can {
            self.press += speed;
            if self.press >= max {
                self.press_one();
                self.retracting = true;
                self.delay = delay;
                return true;
            }
        } else if self.press > 0 {
            self.retracting = true;
        }
        false
    }

    fn view_slots(&self, mut vm: ViewModel) -> ViewModel {
        let p = recipes::press();
        vm = match &self.stamp {
            Some(s) => {
                let d = p.stamp(&s.item).map_or(0, |k| k.durability);
                let left = if d == 0 {
                    "infinito".into()
                } else {
                    format!("{}/{d}", d - s.used)
                };
                vm.item(&s.item, format!("Molde: {left} usos"))
            }
            None => vm.text("Sem molde", view::GREY),
        };
        vm = match &self.input {
            Some((i, n)) => vm.item(i, format!("Entrada: {n}")),
            None => vm.text("Entrada vazia", view::GREY),
        };
        match &self.output {
            Some((i, n)) => vm.item(i, format!("Saída: {n}")),
            None => vm.text("Saída vazia", view::GREY),
        }
    }

    /// The insert button: a stamp, a press input, or (`fuel`) a fuel.
    fn insert(&self, held: Option<&Held>, fuel: Option<&Option<(String, u32)>>, fx: &mut Vec<Effect>) {
        let Some(h) = held else {
            fx.push(Effect::Toast("Segure um molde, um material ou combustível.".into()));
            return;
        };
        let p = recipes::press();
        let take = Effect::TakeHeld {
            item: h.item.clone(),
        };
        if p.stamp(&h.item).is_some() {
            if self.stamp.is_some() {
                fx.push(Effect::Toast("Já há um molde: retire-o antes.".into()));
            } else {
                fx.push(Effect::TakeTagged {
                    item: h.item.clone(),
                    key: WEAR_KEY.into(),
                    buckets: WEAR_BUCKETS,
                });
            }
        } else if self.as_input(&h.item, fuel.is_some()) {
            match &self.input {
                Some((i, n)) if *i != h.item || *n >= STACK => {
                    fx.push(Effect::Toast("A entrada está ocupada.".into()))
                }
                _ => fx.push(take),
            }
        } else if let Some(slot) = fuel
            && burn_ticks(&h.item) > 0
        {
            match slot {
                Some((i, n)) if *i != h.item || *n >= STACK => {
                    fx.push(Effect::Toast("O compartimento de combustível está ocupado.".into()))
                }
                _ => fx.push(take),
            }
        } else {
            fx.push(Effect::Toast("Esta prensa não aceita esse item.".into()));
        }
    }

    /// Whether `item` goes to the input. An item that is also fuel (a
    /// jungle log) is an input only when the stamp in the press presses
    /// it; otherwise the burner press burns it.
    fn as_input(&self, item: &str, has_fuel: bool) -> bool {
        let p = recipes::press();
        if !p.is_input(item) {
            return false;
        }
        if !has_fuel || burn_ticks(item) == 0 {
            return true;
        }
        self.stamp.as_ref().and_then(|s| p.stamp(&s.item)).is_some_and(|k| p.recipe(item, &k.kind).is_some())
    }

    /// An item the probe took: where it goes (false: nowhere, give it back).
    fn accept(&mut self, item: &str, tag: u8, fuel: Option<&mut Option<(String, u32)>>) -> bool {
        let p = recipes::press();
        if let Some(k) = p.stamp(item) {
            if self.stamp.is_some() {
                return false;
            }
            self.stamp = Some(Stamp {
                item: item.into(),
                used: wear_of_bucket(k.durability, tag),
            });
            return true;
        }
        if self.as_input(item, fuel.is_some()) {
            return add_to(&mut self.input, item);
        }
        match fuel {
            Some(slot) if burn_ticks(item) > 0 => add_to(slot, item),
            _ => false,
        }
    }

    fn take_action(&mut self, action: u32, fx: &mut Vec<Effect>) {
        match action {
            actions::TAKE_OUTPUT => {
                if let Some((item, count)) = self.output.take() {
                    fx.push(Effect::Give { item, count });
                }
            }
            actions::TAKE_STAMP => {
                if let Some(s) = self.stamp.take() {
                    self.press = 0;
                    self.retracting = false;
                    fx.push(Effect::Give {
                        item: stamp_item(&s),
                        count: 1,
                    });
                }
            }
            _ => {}
        }
    }

    fn contents(&self) -> Vec<(String, u32)> {
        let mut out: Vec<(String, u32)> = self.stamp.iter().map(|s| (stamp_item(s), 1)).collect();
        out.extend(self.input.iter().chain(&self.output).cloned());
        out
    }

    fn level(&self, active: bool, max_press: u32, max: u8) -> u8 {
        if !active {
            return 0;
        }
        let l = 1 + u64::from(self.press.min(max_press)) * u64::from(max - 1) / u64::from(max_press);
        l as u8
    }
}

impl PressCore {
    /// One `item` out again (a take the world forgot): the stamp, or one
    /// of the input.
    fn unaccept(&mut self, item: &str) -> bool {
        if self.stamp.as_ref().is_some_and(|s| s.item == item) {
            self.stamp = None;
            return true;
        }
        crate::machine::remove_one(&mut self.input, item)
    }
}

fn add_to(slot: &mut Option<(String, u32)>, item: &str) -> bool {
    match slot {
        None => {
            *slot = Some((item.into(), 1));
            true
        }
        Some((i, n)) if i == item && *n < STACK => {
            *n += 1;
            true
        }
        _ => false,
    }
}

fn give_back(item: &str, tag: u8, fx: &mut Vec<Effect>) {
    let item = match tag {
        0 => item.to_string(),
        b => format!("{item}[minecraft:custom_data={{{WEAR_KEY}:{b}}}]"),
    };
    fx.push(Effect::Give { item, count: 1 });
}

fn buttons(vm: ViewModel) -> ViewModel {
    vm.button(actions::INSERT, "Pôr item da mão (1)")
        .button(actions::TAKE_OUTPUT, "Retirar a saída")
        .button(actions::TAKE_STAMP, "Retirar o molde")
}

// ----------------------------------------------------------- burner press

/// `machine_press`: 200 burn ticks a pressing; the speed climbs by one a
/// tick while it can work (to `max_speed`) and falls by one otherwise.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BurnerPress {
    pub core: PressCore,
    pub speed: u32,
    /// Burn ticks stored.
    pub burn: u32,
    pub fuel: Option<(String, u32)>,
    /// The engine's slots (`ContainerMachinePress`: fuel, stamp, input,
    /// output), a mirror of the fields.
    #[serde(skip)]
    pub slots: Slots,
}

/// Burn ticks one pressing costs.
pub const BURN_PER_OP: u32 = 200;

impl Logic for BurnerPress {
    fn tick(&mut self) -> bool {
        let p = recipes::press();
        let before = (self.core.press, self.speed, self.burn, self.core.delay);
        let can = self.burn >= BURN_PER_OP && self.core.can_process();
        if (can || self.core.retracting) && self.burn >= BURN_PER_OP {
            self.speed = (self.speed + 1).min(p.max_speed);
        } else {
            self.speed = self.speed.saturating_sub(1);
        }
        let step = self.speed * p.progress_at_max / p.max_speed;
        if self.core.ram(can, step, step, p.max_press, 5) {
            self.burn -= BURN_PER_OP;
        }
        if self.burn < BURN_PER_OP
            && let Some((item, n)) = &mut self.fuel
        {
            self.burn += burn_ticks(item);
            *n -= 1;
            if *n == 0 {
                self.fuel = None;
            }
        }
        before != (self.core.press, self.speed, self.burn, self.core.delay)
    }
    fn level(&self, max: u8) -> u8 {
        self.core
            .level(self.speed > 0, recipes::press().max_press, max)
    }
    fn view(&self) -> ViewModel {
        let p = recipes::press();
        let mut vm = ViewModel::new("Prensa a combustível")
            .bar("Prensa", u64::from(self.core.press), u64::from(p.max_press), "")
            .bar("Velocidade", u64::from(self.speed), u64::from(p.max_speed), "")
            .text(
                format!(
                    "Combustível: {} prensagens ({} t)",
                    self.burn / BURN_PER_OP,
                    self.burn
                ),
                if self.burn >= BURN_PER_OP { view::GREEN } else { view::GREY },
            );
        if let Some((i, n)) = &self.fuel {
            vm = vm.item(i, format!("Combustível guardado: {n}"));
        }
        buttons(self.core.view_slots(vm))
    }
    fn action(&mut self, action: u32, held: Option<&Held>, fx: &mut Vec<Effect>) {
        match action {
            actions::INSERT => self.core.insert(held, Some(&self.fuel), fx),
            a => self.core.take_action(a, fx),
        }
    }
    fn unconsume(&mut self, item: &str) -> bool {
        self.core.unaccept(item) || crate::machine::remove_one(&mut self.fuel, item)
    }
    fn consumed(&mut self, item: &str, fx: &mut Vec<Effect>) {
        self.consumed_tagged(item, 0, fx)
    }
    fn consumed_tagged(&mut self, item: &str, tag: u8, fx: &mut Vec<Effect>) {
        if !self.core.accept(item, tag, Some(&mut self.fuel)) {
            give_back(item, tag, fx);
        }
    }
    fn contents(&self) -> Vec<(String, u32)> {
        let mut c = self.core.contents();
        c.extend(self.fuel.iter().cloned());
        c
    }
}

// --------------------------------------------------------- electric press

/// `machine_epress` without upgrades: 100 HE every working tick, 56 travel
/// a tick down and 25 back (`45` and `20` times 1.25), 5 ticks' pause.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ElectricPress {
    pub core: PressCore,
    pub power: u64,
    /// The engine's slots (`ContainerMachineEPress`: battery, stamp, input,
    /// output, upgrade); the battery and the upgrade aren't used yet.
    #[serde(skip)]
    pub slots: Slots,
}

impl ElectricPress {
    pub const MAX: u64 = 50_000;
    pub const DOWN: u32 = 56;
    pub const UP: u32 = 25;
    fn active(&self) -> bool {
        let per = recipes::press().epress_energy;
        self.power >= per && (self.core.can_process() || self.core.retracting || self.core.delay > 0)
    }
}

impl Logic for ElectricPress {
    fn tick(&mut self) -> bool {
        if !self.active() {
            return false;
        }
        self.power -= recipes::press().epress_energy;
        let can = self.core.can_process();
        self.core
            .ram(can, Self::DOWN, Self::UP, recipes::press().max_press, 5);
        true
    }
    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        Some(self)
    }
    fn level(&self, max: u8) -> u8 {
        self.core
            .level(self.active(), recipes::press().max_press, max)
    }
    fn view(&self) -> ViewModel {
        let p = recipes::press();
        let vm = ViewModel::new("Prensa elétrica")
            .bar("Energia", self.power, Self::MAX, "HE")
            .bar("Prensa", u64::from(self.core.press), u64::from(p.max_press), "");
        buttons(self.core.view_slots(vm))
    }
    fn action(&mut self, action: u32, held: Option<&Held>, fx: &mut Vec<Effect>) {
        match action {
            actions::INSERT => self.core.insert(held, None, fx),
            a => self.core.take_action(a, fx),
        }
    }
    fn unconsume(&mut self, item: &str) -> bool {
        self.core.unaccept(item)
    }
    fn consumed(&mut self, item: &str, fx: &mut Vec<Effect>) {
        self.consumed_tagged(item, 0, fx)
    }
    fn consumed_tagged(&mut self, item: &str, tag: u8, fx: &mut Vec<Effect>) {
        if !self.core.accept(item, tag, None) {
            give_back(item, tag, fx);
        }
    }
    fn contents(&self) -> Vec<(String, u32)> {
        self.core.contents()
    }
}

impl EnergyNode for ElectricPress {
    fn role(&self) -> Role {
        Role::Consumer
    }
    fn offer(&self) -> u64 {
        0
    }
    fn demand(&self) -> u64 {
        Self::MAX - self.power.min(Self::MAX)
    }
    fn give(&mut self, he: u64) {
        self.power = (self.power + he).min(Self::MAX);
    }
    fn take(&mut self, _he: u64) {}
}

// ------------------------------------------------------- the engine's slots

/// Slot `i` of a press's engine slots.
pub mod slot {
    pub const FUEL: usize = 0;
    pub const BATTERY: usize = 0;
    pub const STAMP: usize = 1;
    pub const INPUT: usize = 2;
    pub const OUTPUT: usize = 3;
    pub const UPGRADE: usize = 4;
}

fn pile(s: &Option<Stack>) -> Option<(String, u32)> {
    s.as_ref().filter(|s| s.count > 0).map(|s| (s.id.clone(), s.count))
}

fn stack(p: &Option<(String, u32)>) -> Option<Stack> {
    p.as_ref().map(|(i, n)| Stack::new(i, *n))
}

impl PressCore {
    /// Stamp (its wear as the item's damage), input and output from the
    /// slots.
    fn sync_in(&mut self, slots: &Slots) {
        self.stamp = slots.get(slot::STAMP).cloned().flatten().map(|s| Stamp { item: s.id, used: s.damage });
        self.input = slots.get(slot::INPUT).and_then(pile);
        self.output = slots.get(slot::OUTPUT).and_then(pile);
    }

    fn sync_out(&self, slots: &mut Slots, len: usize) {
        slots.resize(len, None);
        slots[slot::STAMP] = self.stamp.as_ref().map(|s| Stack { id: s.item.clone(), count: 1, damage: s.used, custom: None });
        slots[slot::INPUT] = stack(&self.input);
        slots[slot::OUTPUT] = stack(&self.output);
    }

    fn save_state(&self) -> Vec<i32> {
        vec![self.press as i32, i32::from(self.retracting), self.delay as i32]
    }

    fn load_state(&mut self, v: &[i32]) {
        if let [press, retracting, delay, ..] = v {
            self.press = (*press).clamp(0, recipes::press().max_press as i32) as u32;
            self.retracting = *retracting != 0;
            self.delay = (*delay).max(0) as u32;
        }
    }
}

impl Slotted for BurnerPress {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn sync_in(&mut self) {
        self.core.sync_in(&self.slots);
        self.fuel = self.slots.get(slot::FUEL).and_then(pile);
    }
    fn sync_out(&mut self) {
        self.core.sync_out(&mut self.slots, 4);
        self.slots[slot::FUEL] = stack(&self.fuel);
    }
    fn migrates(&self) -> bool {
        true
    }
    /// The ram, its maximum, the speed, its maximum, the burn ticks stored
    /// and what a pressing costs.
    fn screen_data(&self) -> Vec<i32> {
        let p = recipes::press();
        vec![self.core.press as i32, p.max_press as i32, self.speed as i32, p.max_speed as i32, self.burn as i32, BURN_PER_OP as i32]
    }
    fn save_state(&self) -> Vec<i32> {
        let mut v = self.core.save_state();
        v.extend([self.speed as i32, self.burn as i32]);
        v
    }
    fn load_state(&mut self, v: &[i32]) {
        self.core.load_state(v);
        if let [_, _, _, speed, burn, ..] = v {
            self.speed = (*speed).clamp(0, recipes::press().max_speed as i32) as u32;
            self.burn = (*burn).max(0) as u32;
        }
    }
}

impl Slotted for ElectricPress {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn sync_in(&mut self) {
        self.core.sync_in(&self.slots);
    }
    fn sync_out(&mut self) {
        self.core.sync_out(&mut self.slots, 5);
    }
    fn migrates(&self) -> bool {
        true
    }
    /// The ram, its maximum, the energy and its maximum.
    fn screen_data(&self) -> Vec<i32> {
        vec![self.core.press as i32, recipes::press().max_press as i32, self.power.min(i32::MAX as u64) as i32, Self::MAX as i32]
    }
    fn save_state(&self) -> Vec<i32> {
        let mut v = self.core.save_state();
        v.push(self.power.min(Self::MAX) as i32);
        v
    }
    fn load_state(&mut self, v: &[i32]) {
        self.core.load_state(v);
        if let [_, _, _, power, ..] = v {
            self.power = (*power).clamp(0, Self::MAX as i32) as u64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loaded() -> PressCore {
        PressCore {
            stamp: Some(Stamp {
                item: "ntm:stamp_iron_plate".into(),
                used: 0,
            }),
            input: Some(("ntm:ingot_lead".into(), 3)),
            ..Default::default()
        }
    }

    #[test]
    fn wear_buckets_round_up_and_back() {
        assert_eq!(wear_bucket(64, 0), 0);
        assert_eq!(wear_bucket(64, 1), 1);
        assert_eq!(wear_bucket(64, 11), 1);
        assert_eq!(wear_bucket(64, 12), 2);
        assert_eq!(wear_bucket(64, 63), 6);
        for used in 0..64 {
            let back = wear_of_bucket(64, wear_bucket(64, used));
            assert!(back >= used, "used {used} came back as {back}");
            assert!(back < 64);
        }
        assert_eq!(
            stamp_item(&Stamp { item: "ntm:stamp_iron_plate".into(), used: 30 }),
            "ntm:stamp_iron_plate[minecraft:custom_data={ntm_wear:3}]"
        );
    }

    #[test]
    fn burner_press_ramps_up_and_presses_with_fuel() {
        let mut p = BurnerPress {
            core: loaded(),
            fuel: Some(("minecraft:coal".into(), 1)),
            ..Default::default()
        };
        let mut ticks = 0;
        while p.core.output.is_none() && ticks < 2000 {
            p.tick();
            ticks += 1;
        }
        assert_eq!(p.core.output, Some(("ntm:plate_lead".into(), 1)));
        assert_eq!(p.core.input, Some(("ntm:ingot_lead".into(), 2)));
        assert_eq!(p.core.stamp.as_ref().unwrap().used, 1);
        assert_eq!(p.burn, 1600 - 200);
        // Ramp: speed 1, 2, ... adds speed*25/400 a tick: from cold, 200 of
        // travel takes about 90 ticks.
        assert!(ticks > 50, "{ticks}");
        // No fuel at all: it never starts.
        let mut cold = BurnerPress { core: loaded(), ..Default::default() };
        for _ in 0..500 {
            cold.tick();
        }
        assert_eq!(cold.core.output, None);
        assert_eq!(cold.level(5), 0);
    }

    /// fbef4942's stalled burner press in game: a flat stamp has no recipe
    /// for an iron ingot (the plate stamp has), so `canProcess` is false and
    /// the speed never climbs however hot it is, as in the NTM
    /// (`TileEntityMachinePress`:70-81).
    #[test]
    fn a_burner_press_without_a_recipe_stays_still_however_hot() {
        let with = |stamp: &str| BurnerPress {
            core: PressCore {
                stamp: Some(Stamp { item: stamp.into(), used: 0 }),
                input: Some(("minecraft:iron_ingot".into(), 16)),
                ..Default::default()
            },
            fuel: Some(("minecraft:coal".into(), 15)),
            ..Default::default()
        };
        let mut flat = with("ntm:stamp_iron_flat");
        for _ in 0..400 {
            flat.tick();
        }
        assert_eq!((flat.speed, flat.core.press, flat.core.output.clone()), (0, 0, None));
        assert_eq!(flat.burn, 1_600, "o carvão queimou uma vez e o calor ficou");
        let mut plate = with("ntm:stamp_iron_plate");
        for _ in 0..400 {
            plate.tick();
        }
        assert!(plate.speed > 0);
        assert!(plate.core.output.as_ref().is_some_and(|o| o.0 == "ntm:plate_iron" && o.1 > 0), "{:?}", plate.core.output);
    }

    #[test]
    fn epress_presses_on_power_and_wears_the_stamp_out() {
        let mut p = ElectricPress {
            core: loaded(),
            power: ElectricPress::MAX,
            ..Default::default()
        };
        p.core.input = Some(("ntm:ingot_lead".into(), 64));
        p.core.stamp.as_mut().unwrap().used = 62;
        for _ in 0..200 {
            p.tick();
        }
        // Two pressings left on the stamp: then it breaks.
        assert_eq!(p.core.output, Some(("ntm:plate_lead".into(), 2)));
        assert_eq!(p.core.stamp, None);
        assert!(p.power < ElectricPress::MAX);
        // No power: nothing moves.
        let mut dead = ElectricPress { core: loaded(), power: 0, ..Default::default() };
        assert!(!dead.tick());
    }

    #[test]
    fn insert_routes_stamps_inputs_and_fuel() {
        let mut p = BurnerPress::default();
        let mut fx = vec![];
        let held = |item: &str| Held { item: item.into(), count: 1 };
        p.action(actions::INSERT, Some(&held("ntm:stamp_iron_wire")), &mut fx);
        assert!(matches!(&fx[0], Effect::TakeTagged { buckets: 6, .. }));
        p.consumed_tagged("ntm:stamp_iron_wire", 2, &mut fx);
        assert_eq!(p.core.stamp.as_ref().unwrap().used, 22);
        fx.clear();
        p.action(actions::INSERT, Some(&held("minecraft:coal")), &mut fx);
        assert_eq!(fx, vec![Effect::TakeHeld { item: "minecraft:coal".into() }]);
        p.consumed("minecraft:coal", &mut fx);
        p.consumed("ntm:ingot_tungsten", &mut fx);
        assert_eq!(p.fuel, Some(("minecraft:coal".into(), 1)));
        assert_eq!(p.core.input, Some(("ntm:ingot_tungsten".into(), 1)));
        fx.clear();
        p.action(actions::TAKE_STAMP, None, &mut fx);
        assert_eq!(
            fx,
            vec![Effect::Give {
                item: "ntm:stamp_iron_wire[minecraft:custom_data={ntm_wear:2}]".into(),
                count: 1
            }]
        );
        // The electric press takes no fuel.
        let e = ElectricPress::default();
        let mut fx = vec![];
        e.core.insert(Some(&held("minecraft:coal")), None, &mut fx);
        assert!(matches!(&fx[0], Effect::Toast(_)));
    }

    #[test]
    fn fuel_that_is_also_an_input_follows_the_stamp() {
        let held = |item: &str| Held { item: item.into(), count: 1 };
        // A plate stamp: a jungle log burns.
        let mut p = BurnerPress { core: loaded(), ..Default::default() };
        p.core.input = None;
        let mut fx = vec![];
        p.action(actions::INSERT, Some(&held("minecraft:jungle_log")), &mut fx);
        assert_eq!(fx, vec![Effect::TakeHeld { item: "minecraft:jungle_log".into() }]);
        p.consumed("minecraft:jungle_log", &mut fx);
        assert_eq!((p.fuel.clone(), p.core.input.clone()), (Some(("minecraft:jungle_log".into(), 1)), None));
        // A flat stamp: it is pressed into resin.
        p.core.stamp = Some(Stamp { item: "ntm:stamp_iron_flat".into(), used: 0 });
        p.consumed("minecraft:jungle_log", &mut fx);
        assert_eq!(p.core.input, Some(("minecraft:jungle_log".into(), 1)));
        assert_eq!(p.core.recipe().map(|r| r.output.as_str()), Some("ntm:ball_resin"));
    }

    #[test]
    fn a_desh_stamp_never_wears_and_a_stone_one_lasts_32() {
        let mut p = ElectricPress { core: loaded(), power: ElectricPress::MAX, ..Default::default() };
        p.core.stamp = Some(Stamp { item: "ntm:stamp_desh_wire".into(), used: 0 });
        p.core.input = Some(("minecraft:gold_ingot".into(), 40));
        for _ in 0..40 {
            p.core.output = None;
            p.core.press_one();
        }
        assert_eq!(p.core.stamp, Some(Stamp { item: "ntm:stamp_desh_wire".into(), used: 0 }));
        assert_eq!(stamp_item(p.core.stamp.as_ref().unwrap()), "ntm:stamp_desh_wire");
        p.core.stamp = Some(Stamp { item: "ntm:stamp_stone_wire".into(), used: 0 });
        p.core.input = Some(("ntm:ingot_graphite".into(), 40));
        let mut made = 0;
        while p.core.stamp.is_some() {
            p.core.output = None;
            p.core.press_one();
            made += 1;
        }
        assert_eq!(made, 32);
        assert_eq!(p.core.output, Some(("ntm:wire_fine__carbon".into(), 8)));
    }
}
