//! The gas centrifuge (docs/maquinas.md), `tileentity/machine/
//! TileEntityMachineGasCent.java` and `inventory/recipes/
//! GasCentrifugeRecipes.java:24-34, 94-96` of the NTM.
//!
//! A real tank (2 000 mB, UF6 by default) is converted into a "pseudo
//! fluid" input tank (8 000): NUF6 from UF6, PF6 from PuF6, MUD from WATZ.
//! Each stage consumes some of it, puts its products out and passes the
//! rest, as the next pseudo fluid, to an output tank (8 000). Every 10
//! ticks the output goes into the gas centrifuge **behind** this one (a
//! cascade: the enrichment needs several). At the end of a LEUF6 line, 600
//! mB become 6 uranium fuel nuggets and a fluorite. 150 ticks at 200 HE/t
//! a stage; with the speed upgrade 80 ticks at 300 HE/t, and only with it
//! HEUF6 goes on. The cascade itself is the server's (it needs the
//! neighbour): `server/fluid_server.rs`.

use crate::{
    energy::{EnergyNode, Role},
    fluid::{self, FluidNode, Tank},
    machine::{Effect, Logic},
    proto::Held,
    server::container_bridge::{Slots, Slotted, Stack},
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pseudo {
    #[default]
    None,
    Nuf6,
    Leuf6,
    Meuf6,
    Heuf6,
    Pf6,
    Mud,
    MudHeavy,
}

/// (consumed, produced, next, needs the speed upgrade, outputs).
type Stage = (u32, u32, Pseudo, bool, &'static [(&'static str, u32)]);

impl Pseudo {
    pub fn stage(self) -> Stage {
        use Pseudo::*;
        match self {
            None => (0, 0, None, false, &[]),
            Heuf6 => (300, 0, None, true, &[("ntm:nugget_u238", 2), ("ntm:nugget_u235", 1), ("ntm:fluorite", 1)]),
            Meuf6 => (200, 100, Heuf6, false, &[("ntm:nugget_u238", 1)]),
            Leuf6 => (300, 200, Meuf6, false, &[("ntm:nugget_u238", 1), ("ntm:fluorite", 1)]),
            Nuf6 => (400, 300, Leuf6, false, &[("ntm:nugget_u238", 1)]),
            Pf6 => (300, 0, None, false, &[("ntm:nugget_pu238", 1), ("ntm:nugget_pu_mix", 2), ("ntm:fluorite", 1)]),
            MudHeavy => (500, 0, None, false, &[("ntm:powder_iron", 1), ("ntm:dust", 1), ("ntm:nuclear_waste_tiny", 1)]),
            Mud => (1000, 500, MudHeavy, false, &[("ntm:powder_lead", 1), ("ntm:dust", 1)]),
        }
    }

    /// `fluidConversions`: the pseudo fluid of a real one.
    pub fn of(fluid: &str) -> Self {
        match fluid {
            "uf6" => Pseudo::Nuf6,
            "puf6" => Pseudo::Pf6,
            "watz" => Pseudo::Mud,
            _ => Pseudo::None,
        }
    }

    fn label(self) -> &'static str {
        use Pseudo::*;
        match self {
            None => "nenhum",
            Nuf6 => "UF6 natural",
            Leuf6 => "UF6 pouco enriquecido",
            Meuf6 => "UF6 médio",
            Heuf6 => "UF6 muito enriquecido",
            Pf6 => "PF6",
            Mud => "Lama",
            MudHeavy => "Lama pesada",
        }
    }
}

pub const MAX_POWER: u64 = 100_000;
pub const SPEED: u32 = 150;
pub const SPEED_UPGRADED: u32 = 80;
pub const PSEUDO_CAP: u32 = 8_000;
pub const UPGRADE: &str = "ntm:upgrade_gc_speed";

pub mod actions {
    pub const POUR: u32 = 1;
    pub const TYPE_FROM_HAND: u32 = 2;
    pub const TAKE_OUT: u32 = 3;
    pub const UPGRADE: u32 = 4;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GasCent {
    pub tank: Tank,
    pub input: (Pseudo, u32),
    pub output: (Pseudo, u32),
    pub power: u64,
    pub progress: u32,
    /// The 4 output slots, as (item, count).
    #[serde(default)]
    pub out: Vec<(String, u32)>,
    #[serde(default)]
    pub upgrade: bool,
    /// The engine's slots (`ContainerMachineGasCent`): 4 outputs, battery,
    /// fluid identifier, speed upgrade; a mirror of `out` and `upgrade`
    /// (the battery and the identifier aren't used yet).
    #[serde(skip)]
    pub slots: Slots,
}

impl Default for GasCent {
    fn default() -> Self {
        Self {
            tank: Tank::typed("uf6", 2_000),
            input: (Pseudo::Nuf6, 0),
            output: (Pseudo::Leuf6, 0),
            power: 0,
            progress: 0,
            out: Vec::new(),
            upgrade: false,
            slots: Vec::new(),
        }
    }
}

impl GasCent {
    /// Whether `items` all fit the 4 output slots (64 a stack).
    pub fn fits(&self, items: &[(&str, u32)]) -> bool {
        let mut out = self.out.clone();
        for &(i, n) in items {
            if let Some(o) = out.iter_mut().find(|o| o.0 == i) {
                if o.1 + n > 64 {
                    return false;
                }
                o.1 += n;
            } else if out.len() < 4 {
                out.push((i.into(), n));
            } else {
                return false;
            }
        }
        true
    }

    pub fn put(&mut self, items: &[(&str, u32)]) {
        for &(i, n) in items {
            match self.out.iter_mut().find(|o| o.0 == i) {
                Some(o) => o.1 += n,
                None => self.out.push((i.into(), n)),
            }
        }
    }

    /// Sets the tank's fluid (an identifier): the pseudo tanks follow.
    pub fn set_type(&mut self, f: &str) {
        if self.tank.fluid != f {
            self.tank.set_type(f);
            let p = Pseudo::of(f);
            self.input = (p, 0);
            self.output = (p.stage().2, 0);
        }
    }

    fn speed(&self) -> u32 {
        if self.upgrade { SPEED_UPGRADED } else { SPEED }
    }

    fn can_enrich(&self) -> bool {
        let (consumed, produced, _, high, items) = self.input.0.stage();
        self.power > 0
            && !items.is_empty()
            && self.input.1 >= consumed
            && self.output.1 + produced <= PSEUDO_CAP
            && (!high || self.upgrade)
            && self.fits(items)
    }
}

impl EnergyNode for GasCent {
    fn role(&self) -> Role {
        Role::Consumer
    }
    fn offer(&self) -> u64 {
        0
    }
    fn demand(&self) -> u64 {
        MAX_POWER.saturating_sub(self.power)
    }
    fn give(&mut self, he: u64) {
        self.power += he;
    }
    fn take(&mut self, _he: u64) {}
}

impl FluidNode for GasCent {
    fn fluid_offer(&self, _f: &str) -> u64 {
        0
    }
    fn fluid_demand(&self, f: &str) -> u64 {
        if self.tank.fluid == f { u64::from(self.tank.free()) } else { 0 }
    }
    fn fluid_types(&self) -> Vec<String> {
        vec![self.tank.fluid.clone()]
    }
    fn fluid_give(&mut self, f: &str, mb: u64) {
        self.tank.fill(f, mb as u32);
    }
    fn fluid_take(&mut self, _f: &str, _mb: u64) {}
}

impl Logic for GasCent {
    fn tick(&mut self) -> bool {
        let before = (self.tank.mb, self.input.1, self.progress);
        // `attemptConversion`: the real fluid into the pseudo input tank.
        if self.input.0 == Pseudo::of(&self.tank.fluid) && self.input.0 != Pseudo::None && self.input.1 < PSEUDO_CAP {
            let n = (PSEUDO_CAP - self.input.1).min(self.tank.mb);
            self.tank.drain(n);
            self.input.1 += n;
        }
        if self.can_enrich() {
            self.progress += 1;
            let cost = if self.upgrade { 300 } else { 200 };
            if self.power < cost {
                self.power = 0;
                self.progress = 0;
            } else {
                self.power -= cost;
            }
            if self.progress >= self.speed() {
                let (consumed, produced, next, _, items) = self.input.0.stage();
                self.progress = 0;
                self.input.1 -= consumed;
                if self.output.0 == Pseudo::None || self.output.0 == next {
                    self.output = (next, self.output.1 + produced);
                }
                self.put(items);
            }
        } else {
            self.progress = 0;
        }
        (self.tank.mb, self.input.1, self.progress) != before
    }

    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        Some(self)
    }

    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        Some(self)
    }

    fn level(&self, max: u8) -> u8 {
        if self.progress == 0 { 0 } else { crate::machine::fill_level(u64::from(self.progress), u64::from(self.speed()), max) }
    }

    fn view(&self) -> ViewModel {
        let n: u32 = self.out.iter().map(|o| o.1).sum();
        let mut vm = ViewModel::new("Centrífuga de gás")
            .bar("Energia", self.power, MAX_POWER, "HE")
            .text(self.tank.line(), view::GREY)
            .text(format!("Entrada: {} {} mB", self.input.0.label(), self.input.1), view::GREY)
            .text(format!("Saída: {} {} mB", self.output.0.label(), self.output.1), view::GREY)
            .bar("Progresso", u64::from(self.progress), u64::from(self.speed()), "t");
        if n > 0 {
            vm = vm.text(format!("Pronto: {n} item(ns)"), view::GREEN);
        }
        vm.button(actions::POUR, "Despejar UF6/PuF6 da mão")
            .button(actions::TAKE_OUT, "Retirar os produtos")
            .button(
                actions::UPGRADE,
                if self.upgrade { "Upgrade de velocidade: instalado" } else { "Instalar o upgrade de velocidade da mão" },
            )
    }

    fn action(&mut self, action: u32, held: Option<&Held>, fx: &mut Vec<Effect>) {
        let toast = |fx: &mut Vec<Effect>, t: &str| fx.push(Effect::Toast(t.into()));
        match action {
            actions::POUR | actions::TYPE_FROM_HAND => match held.and_then(|h| fluid::contents(&h.item).map(|c| (h, c))) {
                Some((_, c)) if Pseudo::of(&c.fluid) == Pseudo::None => toast(fx, "A centrífuga de gás só processa UF6, PuF6 e lama do Watz."),
                Some((h, c)) => {
                    // An identifier's job: a container of another fluid sets the type.
                    if self.tank.fluid != c.fluid && self.tank.mb == 0 {
                        self.set_type(&c.fluid);
                    }
                    if action == actions::POUR {
                        if self.tank.fluid == c.fluid && self.tank.free() >= c.mb {
                            fx.push(Effect::TakeHeld { item: h.item.clone() });
                        } else {
                            toast(fx, "Não cabe: o tanque tem outro fluido ou está cheio.");
                        }
                    }
                }
                None => toast(fx, "Segure um recipiente cheio de UF6, PuF6 ou lama."),
            },
            actions::TAKE_OUT => {
                for (i, n) in std::mem::take(&mut self.out) {
                    fx.push(Effect::Give { item: i, count: n });
                }
            }
            actions::UPGRADE => match held {
                _ if self.upgrade => {
                    self.upgrade = false;
                    fx.push(Effect::Give { item: UPGRADE.into(), count: 1 });
                }
                Some(h) if h.item == UPGRADE => fx.push(Effect::TakeHeld { item: UPGRADE.into() }),
                _ => toast(fx, "Segure o upgrade de velocidade da centrífuga de gás."),
            },
            _ => {}
        }
    }

    fn consumed(&mut self, item: &str, fx: &mut Vec<Effect>) {
        if item == UPGRADE && !self.upgrade {
            self.upgrade = true;
            return;
        }
        match fluid::contents(item) {
            Some(c) if self.tank.fluid == c.fluid && self.tank.free() >= c.mb => {
                self.tank.fill(&c.fluid, c.mb);
                fx.push(Effect::Give { item: c.empty, count: 1 });
            }
            _ => fx.push(Effect::Give { item: item.into(), count: 1 }),
        }
    }

    fn unconsume(&mut self, item: &str) -> bool {
        if item == UPGRADE && self.upgrade {
            self.upgrade = false;
            return true;
        }
        match fluid::contents(item) {
            Some(c) if self.tank.fluid == c.fluid && self.tank.mb >= c.mb => {
                self.tank.drain(c.mb);
                true
            }
            _ => false,
        }
    }

    fn contents(&self) -> Vec<(String, u32)> {
        let mut c = self.out.clone();
        if self.upgrade {
            c.push((UPGRADE.into(), 1));
        }
        c
    }
}

/// Every 10 ticks, `from` (the centrifuge in front) passes its pseudo
/// output into `to` (the one behind it) when both run the same real fluid
/// (`attemptTransfer`). Returns whether it was a cascade.
pub fn cascade(from: &mut GasCent, to: &mut GasCent) -> bool {
    if to.tank.fluid != from.tank.fluid {
        return false;
    }
    if to.input.0 != from.output.0 && from.output.0 != Pseudo::None {
        to.input = (from.output.0, 0);
        to.output = (from.output.0.stage().2, 0);
    }
    if to.input.1 < PSEUDO_CAP && from.output.1 > 0 {
        let n = (PSEUDO_CAP - to.input.1).min(from.output.1);
        from.output.1 -= n;
        to.input.1 += n;
    }
    true
}

/// The end of a LEUF6 line with nothing behind: 600 mB of the output
/// become 6 uranium fuel nuggets and a fluorite.
pub fn end_of_line(g: &mut GasCent) -> bool {
    const FUEL: &[(&str, u32)] = &[("ntm:nugget_uranium_fuel", 6), ("ntm:fluorite", 1)];
    if g.input.0 == Pseudo::Leuf6 && g.output.1 >= 600 && g.fits(FUEL) {
        g.output.1 -= 600;
        g.put(FUEL);
        return true;
    }
    false
}

/// Slots of the engine's screen.
pub mod slot {
    pub const OUT: std::ops::Range<usize> = 0..4;
    pub const BATTERY: usize = 4;
    pub const IDENTIFIER: usize = 5;
    pub const UPGRADE: usize = 6;
    pub const COUNT: usize = 7;
}

impl Slotted for GasCent {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn sync_in(&mut self) {
        self.out = self.slots[slot::OUT].iter().flatten().filter(|s| s.count > 0).map(|s| (s.id.clone(), s.count)).collect();
        self.upgrade = self.slots.get(slot::UPGRADE).is_some_and(|s| s.as_ref().is_some_and(|s| s.id == UPGRADE));
    }
    /// The outputs stay in the slots they are in; a new one takes the first
    /// free slot.
    fn sync_out(&mut self) {
        self.slots.resize(slot::COUNT, None);
        let mut left = self.out.clone();
        for i in slot::OUT {
            self.slots[i] = self.slots[i].take().and_then(|s| {
                let k = left.iter().position(|o| o.0 == s.id)?;
                let (id, n) = left.remove(k);
                Some(Stack { id, count: n, damage: s.damage })
            });
        }
        for i in slot::OUT {
            if self.slots[i].is_none() && !left.is_empty() {
                let (id, n) = left.remove(0);
                self.slots[i] = Some(Stack::new(&id, n));
            }
        }
        let other = self.slots[slot::UPGRADE].as_ref().is_some_and(|s| s.id != UPGRADE);
        if !other {
            self.slots[slot::UPGRADE] = self.upgrade.then(|| Stack::new(UPGRADE, 1));
        }
    }
    fn migrates(&self) -> bool {
        true
    }
    /// Progress and the time a stage takes, energy and its maximum, the
    /// fluid tank (mB, capacity), the pseudo input and output tanks (type,
    /// mB) and their capacity.
    fn screen_data(&self) -> Vec<i32> {
        vec![
            self.progress as i32,
            self.speed() as i32,
            self.power.min(MAX_POWER) as i32,
            MAX_POWER as i32,
            self.tank.mb as i32,
            self.tank.cap as i32,
            self.input.0 as i32,
            self.input.1 as i32,
            self.output.0 as i32,
            self.output.1 as i32,
            PSEUDO_CAP as i32,
        ]
    }
    fn save_state(&self) -> Vec<i32> {
        vec![self.progress as i32]
    }
    fn load_state(&mut self, v: &[i32]) {
        if let [progress, ..] = v {
            self.progress = (*progress).max(0) as u32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(g: &mut GasCent, ticks: u32) {
        for _ in 0..ticks {
            g.power = MAX_POWER;
            g.tick();
        }
    }

    #[test]
    fn one_centrifuge_takes_uf6_one_stage() {
        let mut g = GasCent::default();
        g.tank.fill("uf6", 2000);
        run(&mut g, SPEED);
        assert_eq!(g.out, vec![("ntm:nugget_u238".to_string(), 1)]);
        assert_eq!((g.input.0, g.input.1), (Pseudo::Nuf6, 2000 - 400));
        assert_eq!(g.output, (Pseudo::Leuf6, 300));
        // A lone LEUF6 stage turns its output into fuel.
        let mut lone = GasCent { input: (Pseudo::Leuf6, 0), output: (Pseudo::Meuf6, 600), ..Default::default() };
        assert!(end_of_line(&mut lone));
        assert_eq!(lone.out, vec![("ntm:nugget_uranium_fuel".to_string(), 6), ("ntm:fluorite".to_string(), 1)]);
    }

    #[test]
    fn a_cascade_enriches_to_u235_only_with_the_upgrade() {
        let mut line: Vec<GasCent> = (0..4).map(|_| GasCent::default()).collect();
        line[0].tank.fill("uf6", 2000);
        for t in 0..6000u32 {
            for g in &mut line {
                g.power = MAX_POWER;
                g.tick();
            }
            if t % 10 == 0 {
                for i in 0..3 {
                    let (a, b) = line.split_at_mut(i + 1);
                    cascade(&mut a[i], &mut b[0]);
                }
            }
            if t % 500 == 0 {
                line[0].tank.fill("uf6", 2000);
            }
        }
        assert_eq!(line[3].input.0, Pseudo::Heuf6);
        assert!(line[3].out.is_empty(), "HEUF6 precisa do upgrade");
        line[3].upgrade = true;
        run(&mut line[3], 2 * SPEED_UPGRADED);
        assert!(line[3].out.iter().any(|o| o.0 == "ntm:nugget_u235"), "{:?}", line[3].out);
    }

    #[test]
    fn puf6_gives_plutonium_and_other_fluids_are_refused() {
        let mut g = GasCent::default();
        g.set_type("puf6");
        g.tank.fill("puf6", 2000);
        run(&mut g, SPEED);
        assert!(g.out.iter().any(|o| o.0 == "ntm:nugget_pu238"));
        let mut fx = Vec::new();
        let h = Held { item: "ntm:fluid_tank_full__water".into(), count: 1 };
        g.action(actions::POUR, Some(&h), &mut fx);
        assert!(matches!(fx[0], Effect::Toast(_)));
    }
}
