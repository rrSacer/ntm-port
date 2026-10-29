//! The NTM's recipe machines (docs/maquinas.md): the assembly machine, the
//! chemical plant and the centrifuge, one logic over recipe tables.
//!
//! Sources, relative to `ntm-src/src/main/java/com/hbm/`:
//! - assembly machine `tileentity/machine/TileEntityMachineAssemblyMachine
//!   .java` (`module/machine/ModuleMachineAssembler.java`): 12 item inputs,
//!   1 fluid in, 1 item out, 1 fluid out, 4 000 mB tanks; HE/t and ticks per
//!   recipe; buffer `max(power·100, 100 000)`;
//! - chemical plant `TileEntityMachineChemicalPlant.java`: 3/3 items, 3/3
//!   tanks of 24 000 mB, container slots;
//! - centrifuge `TileEntityMachineCentrifuge.java:54-56`: one input picks
//!   its recipe, 4 outputs, 200 ticks at 200 HE/t, 100 000 HE.
//!
//! Recipes come from `data/machines/*.json` (`tools/machine-gen/build.py`
//! from the content agent's extraction). Items move only through item
//! transactions (`Effect::TakeHeld`, `Effect::Give`). Upgrades, blueprint
//! pools and pressure are left out (docs/maquinas.md).

use crate::{
    energy::{EnergyNode, Role},
    fluid::{self, FluidNode, Tank},
    machine::{Effect, Logic, fill_level},
    proto::Held,
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Station {
    Assembly,
    ChemicalPlant,
    Centrifuge,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Ingredient {
    /// Any of these (OreDict alternatives), the first the representative.
    pub ids: Vec<String>,
    pub n: u32,
    pub label: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Recipe {
    pub name: String,
    pub label: String,
    pub duration: u32,
    pub power: u64,
    #[serde(rename = "in")]
    pub items_in: Vec<Ingredient>,
    #[serde(rename = "fin")]
    pub fluids_in: Vec<(String, u32)>,
    /// (item, count, chance).
    #[serde(rename = "out")]
    pub items_out: Vec<(String, u32, f32)>,
    #[serde(rename = "fout")]
    pub fluids_out: Vec<(String, u32)>,
}

#[derive(Deserialize)]
struct Table {
    recipes: Vec<Recipe>,
}

fn table(json: &str) -> Vec<Recipe> {
    serde_json::from_str::<Table>(json).expect("data/machines").recipes
}

impl Station {
    pub fn of(block: &str) -> Option<Self> {
        match block {
            "ntm:machine_assembly_machine" => Some(Station::Assembly),
            "ntm:machine_chemical_plant" => Some(Station::ChemicalPlant),
            "ntm:machine_centrifuge" => Some(Station::Centrifuge),
            _ => None,
        }
    }

    pub fn recipes(self) -> &'static [Recipe] {
        static A: OnceLock<Vec<Recipe>> = OnceLock::new();
        static C: OnceLock<Vec<Recipe>> = OnceLock::new();
        static F: OnceLock<Vec<Recipe>> = OnceLock::new();
        match self {
            Station::Assembly => A.get_or_init(|| table(include_str!("../data/machines/assembly.json"))),
            Station::ChemicalPlant => C.get_or_init(|| table(include_str!("../data/machines/chemical_plant.json"))),
            Station::Centrifuge => F.get_or_init(|| table(include_str!("../data/machines/centrifuge.json"))),
        }
    }

    fn title(self) -> &'static str {
        match self {
            Station::Assembly => "Montadora",
            Station::ChemicalPlant => "Planta química",
            Station::Centrifuge => "Centrífuga",
        }
    }

    /// (input tanks, output tanks, capacity).
    fn tanks(self) -> (usize, usize, u32) {
        match self {
            Station::Assembly => (1, 1, 4_000),
            Station::ChemicalPlant => (3, 3, 24_000),
            Station::Centrifuge => (0, 0, 0),
        }
    }

    /// The recipe is picked from the input (the centrifuge), not a list.
    fn picks_by_input(self) -> bool {
        self == Station::Centrifuge
    }
}

pub mod actions {
    pub const PICK_SCREEN: u32 = 1;
    pub const INSERT: u32 = 2;
    pub const TAKE_OUT: u32 = 3;
    pub const TAKE_IN: u32 = 4;
    pub const FLUID_SCREEN: u32 = 5;
    pub const BACK: u32 = 6;
    pub const POUR: u32 = 7;
    pub const FILL: u32 = 8;
    /// The recipe list: `LIST + row` (a `ui` list widget).
    pub const LIST: u32 = crate::view::LIST_ACTIONS;
}

/// Items one "put from hand" takes at most (one transaction each).
pub const INSERT_MAX: u32 = 16;
/// Items an output or input may pile up to.
pub const STACK: u32 = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Screen {
    #[default]
    Machine,
    Pick,
    Fluids,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Crafter {
    pub station: Station,
    /// The recipe's `name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe: Option<String>,
    /// What went in, by item id.
    #[serde(default)]
    pub items: Vec<(String, u32)>,
    #[serde(default)]
    pub out: Vec<(String, u32)>,
    #[serde(default)]
    pub tin: Vec<Tank>,
    #[serde(default)]
    pub tout: Vec<Tank>,
    pub power: u64,
    pub progress: u32,
    /// Recipes done (the chance outputs' dice).
    #[serde(default)]
    pub done: u64,
    #[serde(default)]
    pub screen: Screen,
}

fn add(list: &mut Vec<(String, u32)>, item: &str, n: u32) {
    match list.iter_mut().find(|(i, _)| i == item) {
        Some(e) => e.1 += n,
        None => list.push((item.into(), n)),
    }
}

fn count(list: &[(String, u32)], ids: &[String]) -> u32 {
    list.iter().filter(|(i, _)| ids.contains(i)).map(|(_, n)| n).sum()
}

/// Takes `n` of any of `ids` out of `list`.
fn remove(list: &mut Vec<(String, u32)>, ids: &[String], mut n: u32) {
    for e in list.iter_mut() {
        if n == 0 {
            break;
        }
        if ids.contains(&e.0) {
            let t = e.1.min(n);
            e.1 -= t;
            n -= t;
        }
    }
    list.retain(|e| e.1 > 0);
}

impl Crafter {
    pub fn new(station: Station) -> Self {
        let (i, o, cap) = station.tanks();
        Self {
            station,
            recipe: None,
            items: Vec::new(),
            out: Vec::new(),
            tin: (0..i).map(|_| Tank::new(cap)).collect(),
            tout: (0..o).map(|_| Tank::new(cap)).collect(),
            power: 0,
            progress: 0,
            done: 0,
            screen: Screen::Machine,
        }
    }

    pub fn current(&self) -> Option<&'static Recipe> {
        let name = self.recipe.as_deref()?;
        self.station.recipes().iter().find(|r| r.name == name)
    }

    pub(crate) fn max_power(&self) -> u64 {
        match self.station {
            Station::Centrifuge => 100_000,
            _ => self.current().map_or(100_000, |r| (r.power * 100).max(100_000)),
        }
    }

    /// Picks `r`: the tanks take its fluids (a tank whose type changes is
    /// emptied, as `FluidTank.setTankType`); progress restarts.
    pub fn set_recipe(&mut self, r: &Recipe) {
        if self.recipe.as_deref() == Some(r.name.as_str()) {
            return;
        }
        self.recipe = Some(r.name.clone());
        self.progress = 0;
        for (t, f) in self.tin.iter_mut().zip(r.fluids_in.iter().map(|f| f.0.as_str()).chain(std::iter::repeat(""))) {
            t.set_type(f);
            // The assembly machine sizes its tanks to the recipe.
            if self.station == Station::Assembly {
                t.cap = 4_000.max(r.fluids_in.first().map_or(0, |f| f.1 * 2));
            }
        }
        for (t, f) in self.tout.iter_mut().zip(r.fluids_out.iter().map(|f| f.0.as_str()).chain(std::iter::repeat(""))) {
            t.set_type(f);
            if self.station == Station::Assembly {
                t.cap = 4_000.max(r.fluids_out.first().map_or(0, |f| f.1 * 2));
            }
        }
    }

    /// What the current recipe still lacks to run once, for the screen.
    fn missing(&self, r: &Recipe) -> Vec<String> {
        let mut out = Vec::new();
        for i in &r.items_in {
            let have = count(&self.items, &i.ids);
            if have < i.n {
                out.push(format!("{}× {}", i.n - have, i.label));
            }
        }
        for (f, mb) in &r.fluids_in {
            let have = self.tin.iter().filter(|t| t.fluid == *f).map(|t| t.mb).sum::<u32>();
            if have < *mb {
                out.push(format!("{} mB {}", mb - have, fluid::label(f)));
            }
        }
        out
    }

    fn can_run(&self, r: &Recipe) -> bool {
        r.items_in.iter().all(|i| count(&self.items, &i.ids) >= i.n)
            && r.fluids_in.iter().all(|(f, mb)| self.tin.iter().any(|t| t.fluid == *f && t.mb >= *mb))
            && r.items_out.iter().all(|(i, n, _)| self.out.iter().find(|o| o.0 == *i).map_or(0, |o| o.1) + n <= STACK)
            && r.fluids_out.iter().all(|(f, mb)| self.tout.iter().any(|t| t.fluid == *f && t.free() >= *mb))
            && (self.out.len() < 6 || r.items_out.iter().all(|(i, ..)| self.out.iter().any(|o| o.0 == *i)))
    }

    fn finish(&mut self, r: &Recipe) {
        for i in &r.items_in {
            remove(&mut self.items, &i.ids, i.n);
        }
        for (f, mb) in &r.fluids_in {
            if let Some(t) = self.tin.iter_mut().find(|t| t.fluid == *f && t.mb >= *mb) {
                t.drain(*mb);
            }
        }
        self.done += 1;
        for (k, (i, n, chance)) in r.items_out.iter().enumerate() {
            // `ChanceOutput`: a fixed hash of (recipes done, output) instead
            // of the world's random.
            let roll = ntm_explosion::crater::rand01(self.done as i32, k as i32, (self.done >> 31) as i32, 77);
            if *chance >= 1.0 || roll < *chance {
                add(&mut self.out, i, *n);
            }
        }
        for (f, mb) in &r.fluids_out {
            if let Some(t) = self.tout.iter_mut().find(|t| t.fluid == *f) {
                t.fill(f, *mb);
            }
        }
    }

    /// The recipe that takes `item` as an input (the centrifuge's choice).
    /// The centrifuge's recipe by what it holds, as INSERT picks it
    /// (`TileEntityMachineCentrifuge` reads its input slot each tick): for
    /// items the engine's slots put in, which skip INSERT.
    pub(crate) fn pick_by_items(&mut self) {
        if !self.station.picks_by_input() {
            return;
        }
        let Some(first) = self.items.first().map(|i| i.0.clone()) else { return };
        let takes = |r: &Recipe| r.items_in.iter().any(|i| i.ids.contains(&first));
        if self.current().is_some_and(takes) {
            return;
        }
        if let Some(r) = self.recipe_for(&first) {
            self.set_recipe(r);
        }
    }

    fn recipe_for(&self, item: &str) -> Option<&'static Recipe> {
        self.station.recipes().iter().find(|r| r.items_in.iter().any(|i| i.ids.iter().any(|x| x == item)))
    }

    /// How many of `item` may still go in for the current recipe.
    fn room_for(&self, item: &str) -> u32 {
        let Some(r) = self.current() else { return 0 };
        r.items_in
            .iter()
            .filter(|i| i.ids.iter().any(|x| x == item))
            .map(|i| (i.n * 2).max(STACK).saturating_sub(count(&self.items, &i.ids)))
            .max()
            .unwrap_or(0)
    }
}

impl EnergyNode for Crafter {
    fn role(&self) -> Role {
        Role::Consumer
    }
    fn offer(&self) -> u64 {
        0
    }
    fn demand(&self) -> u64 {
        self.max_power().saturating_sub(self.power)
    }
    fn give(&mut self, he: u64) {
        self.power += he;
    }
    fn take(&mut self, _he: u64) {}
}

impl FluidNode for Crafter {
    fn fluid_offer(&self, f: &str) -> u64 {
        self.tout.iter().filter(|t| t.fluid == f).map(|t| u64::from(t.mb)).sum()
    }
    fn fluid_demand(&self, f: &str) -> u64 {
        // Only the fluids the recipe takes (each input tank is typed by it).
        self.tin.iter().filter(|t| !t.fluid.is_empty() && t.fluid == f).map(|t| u64::from(t.free())).sum()
    }
    fn fluid_types(&self) -> Vec<String> {
        self.tin.iter().chain(&self.tout).filter(|t| !t.fluid.is_empty()).map(|t| t.fluid.clone()).collect()
    }
    fn fluid_give(&mut self, f: &str, mut mb: u64) {
        for t in self.tin.iter_mut().filter(|t| t.fluid == f) {
            mb -= u64::from(t.fill(f, mb.min(u64::from(u32::MAX)) as u32));
        }
    }
    fn fluid_take(&mut self, f: &str, mut mb: u64) {
        for t in self.tout.iter_mut().filter(|t| t.fluid == f) {
            mb -= u64::from(t.drain(mb.min(u64::from(u32::MAX)) as u32));
        }
    }
}

impl Logic for Crafter {
    fn tick(&mut self) -> bool {
        let Some(r) = self.current() else {
            return false;
        };
        if !self.can_run(r) || self.power < r.power {
            if self.progress > 0 && !self.can_run(r) {
                self.progress = 0;
                return true;
            }
            return false;
        }
        self.power -= r.power;
        self.progress += 1;
        if self.progress >= r.duration.max(1) {
            self.progress = 0;
            self.finish(r);
        }
        true
    }

    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        Some(self)
    }

    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        if self.tin.is_empty() && self.tout.is_empty() { None } else { Some(self) }
    }

    fn level(&self, max: u8) -> u8 {
        match self.current() {
            Some(r) if self.progress > 0 => fill_level(u64::from(self.progress), u64::from(r.duration), max),
            _ => 0,
        }
    }

    fn view(&self) -> ViewModel {
        let st = self.station;
        let r = self.current();
        match self.screen {
            Screen::Pick => {
                let recipes = st.recipes();
                let rows: Vec<String> = recipes.iter().take(view::LIST_MAX).map(|r| r.label.clone()).collect();
                let selected = r.and_then(|r| recipes.iter().position(|x| x.name == r.name)).map(|i| i as u32);
                ViewModel::new(format!("{}: receita", st.title()))
                    .list(actions::LIST, rows, selected)
                    .button(actions::BACK, "Voltar")
            }
            Screen::Fluids => {
                let mut vm = ViewModel::new(format!("{}: fluidos", st.title()));
                for t in &self.tin {
                    vm = vm.text(format!("Entrada {}", t.line()), view::GREY);
                }
                for t in &self.tout {
                    vm = vm.text(format!("Saída {}", t.line()), view::GREY);
                }
                vm.button(actions::POUR, "Despejar o recipiente cheio da mão")
                    .button(actions::FILL, "Encher o recipiente vazio da mão")
                    .button(actions::BACK, "Voltar")
            }
            Screen::Machine => {
                let mut vm = ViewModel::new(st.title()).bar("Energia", self.power, self.max_power(), "HE");
                match r {
                    Some(r) => {
                        vm = match r.items_out.first() {
                            Some((i, ..)) => vm.item(i, r.label.clone()),
                            None => vm.text(r.label.clone(), view::GREEN),
                        };
                        let miss = self.missing(r);
                        vm = if miss.is_empty() {
                            vm.bar("Progresso", u64::from(self.progress), u64::from(r.duration), "t")
                        } else {
                            vm.text(format!("Falta: {}", miss.join(", ")), view::YELLOW)
                        };
                    }
                    None => {
                        vm = vm.text(
                            if st.picks_by_input() { "Ponha um item que a centrífuga processe." } else { "Escolha uma receita." },
                            view::YELLOW,
                        )
                    }
                }
                let out: u32 = self.out.iter().map(|o| o.1).sum();
                if out > 0 {
                    vm = vm.text(format!("Pronto: {out} item(ns)"), view::GREEN);
                }
                if !st.picks_by_input() {
                    vm = vm.button(actions::PICK_SCREEN, "Escolher receita");
                }
                vm = vm.button(actions::INSERT, "Pôr itens da mão").button(actions::TAKE_OUT, "Retirar o que ficou pronto");
                if !self.tin.is_empty() {
                    vm = vm.button(actions::FLUID_SCREEN, "Fluidos e recipientes");
                } else {
                    vm = vm.button(actions::TAKE_IN, "Retirar as entradas");
                }
                vm
            }
        }
    }

    fn action(&mut self, action: u32, held: Option<&Held>, fx: &mut Vec<Effect>) {
        let toast = |fx: &mut Vec<Effect>, t: &str| fx.push(Effect::Toast(t.into()));
        match action {
            actions::PICK_SCREEN => self.screen = Screen::Pick,
            actions::FLUID_SCREEN => self.screen = Screen::Fluids,
            actions::BACK => self.screen = Screen::Machine,
            a if (actions::LIST..actions::LIST + view::LIST_MAX as u32).contains(&a) => {
                if let Some(r) = self.station.recipes().get((a - actions::LIST) as usize) {
                    self.set_recipe(r);
                    self.screen = Screen::Machine;
                }
            }
            actions::INSERT => {
                let Some(h) = held else { return toast(fx, "Segure um item da receita na mão.") };
                if self.station.picks_by_input()
                    && self.items.is_empty()
                    && let Some(r) = self.recipe_for(&h.item)
                {
                    self.set_recipe(r);
                }
                let n = self.room_for(&h.item).min(h.count).min(INSERT_MAX);
                if n == 0 {
                    return toast(fx, "Esse item não entra na receita escolhida (ou já há bastante).");
                }
                for _ in 0..n {
                    fx.push(Effect::TakeHeld { item: h.item.clone() });
                }
            }
            actions::TAKE_OUT => {
                for (i, n) in std::mem::take(&mut self.out) {
                    fx.push(Effect::Give { item: i, count: n });
                }
            }
            actions::TAKE_IN => {
                for (i, n) in std::mem::take(&mut self.items) {
                    fx.push(Effect::Give { item: i, count: n });
                }
                self.progress = 0;
            }
            actions::POUR => match held.and_then(|h| fluid::contents(&h.item).map(|c| (h, c))) {
                Some((h, c)) if self.tin.iter().any(|t| t.fluid == c.fluid && t.free() >= c.mb) => fx.push(Effect::TakeHeld { item: h.item.clone() }),
                Some(_) => toast(fx, "A receita não usa esse fluido, ou o tanque está cheio."),
                None => toast(fx, "Segure um recipiente cheio de fluido."),
            },
            actions::FILL => match held.filter(|h| fluid::is_empty_container(&h.item)) {
                Some(h) => {
                    let ok = self.tout.iter().any(|t| fluid::fill_container(&h.item, &t.fluid).is_some_and(|(_, mb)| t.mb >= mb));
                    if ok {
                        fx.push(Effect::TakeHeld { item: h.item.clone() });
                    } else {
                        toast(fx, "Nenhuma saída com fluido bastante para esse recipiente.");
                    }
                }
                None => toast(fx, "Segure um recipiente vazio."),
            },
            _ => {}
        }
    }

    fn consumed(&mut self, item: &str, fx: &mut Vec<Effect>) {
        if let Some(c) = fluid::contents(item).filter(|c| self.tin.iter().any(|t| t.fluid == c.fluid)) {
            match self.tin.iter_mut().find(|t| t.fluid == c.fluid && t.free() >= c.mb) {
                Some(t) => {
                    t.fill(&c.fluid, c.mb);
                    fx.push(Effect::Give { item: c.empty, count: 1 });
                }
                None => fx.push(Effect::Give { item: item.into(), count: 1 }),
            }
            return;
        }
        if fluid::is_empty_container(item) {
            for t in &mut self.tout {
                if let Some((full, mb)) = fluid::fill_container(item, &t.fluid).filter(|(_, mb)| t.mb >= *mb) {
                    t.drain(mb);
                    fx.push(Effect::Give { item: full, count: 1 });
                    return;
                }
            }
            fx.push(Effect::Give { item: item.into(), count: 1 });
            return;
        }
        if self.room_for(item) > 0 {
            add(&mut self.items, item, 1);
        } else {
            // The recipe changed meanwhile: it goes back.
            fx.push(Effect::Give { item: item.into(), count: 1 });
        }
    }

    fn unconsume(&mut self, item: &str) -> bool {
        if count(&self.items, &[item.to_string()]) > 0 {
            remove(&mut self.items, &[item.to_string()], 1);
            return true;
        }
        match fluid::contents(item) {
            Some(c) => match self.tin.iter_mut().find(|t| t.fluid == c.fluid && t.mb >= c.mb) {
                Some(t) => {
                    t.drain(c.mb);
                    true
                }
                None => false,
            },
            None => false,
        }
    }

    fn contents(&self) -> Vec<(String, u32)> {
        self.items.iter().chain(&self.out).cloned().collect()
    }
}
