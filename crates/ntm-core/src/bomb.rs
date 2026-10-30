//! The NTM's bombs (docs/bombas.md): which ones, how they go off, and the
//! bomb block as a machine (its components, screen and buttons).
//!
//! A bomb is a machine of kind [`Kind::Bomb`]: its components come in and go
//! out through the same item transactions as any machine's (`TakeHeld`,
//! `Give`), so a bomb survives a crash, a rollback and a broken block the way
//! a furnace does. What makes it go off (redstone, the detonator, a fuse) and
//! the explosion itself are the server's (`server/bomb_server.rs`).
//!
//! Sources, relative to `ntm-src/src/main/java/com/hbm/`: radii in
//! `config/BombConfig.java:8-21`; components in each
//! `tileentity/bomb/TileEntityNuke*.isReady`; conventional charges in
//! `blocks/bomb/Block{TNT,Dynamite,Semtex,C4}.java` (`createExplosion`
//! strength) with the 80-tick fuse of `entity/item/EntityTNTPrimedBase:23`.

use crate::{
    machine::{Effect, Logic, remove_one},
    machine::Held,
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

/// How a bomb explodes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Blast {
    /// Vanilla's explosion (`world.createExplosion`) of this strength.
    Conventional { power: f32 },
    /// `EntityNukeExplosionMK5`: the crater engine (`crate::explosion`) of this
    /// radius; `fallout` adds the 2.5R fallout and the area radiation
    /// (`statFacNoRad` turns both off: the N2).
    Nuke { radius: i32, fallout: bool },
    /// `EntityBalefire`: a crater without slag, set on fire, radioactive.
    Balefire { radius: i32 },
}

impl Blast {
    pub fn radius(&self) -> i32 {
        match *self {
            Blast::Conventional { power } => power.ceil() as i32,
            Blast::Nuke { radius, .. } | Blast::Balefire { radius } => radius,
        }
    }
}

/// A slot of a bomb's screen: what it is, what fits.
#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub label: &'static str,
    pub item: &'static str,
}

/// A stage of a bomb: the first `slots` filled with their items make it
/// ready, with this blast. The last stage that is complete wins.
#[derive(Clone, Copy, Debug)]
pub struct Stage {
    pub slots: usize,
    pub blast: Blast,
}

#[derive(Clone, Copy, Debug)]
pub struct BombType {
    pub id: &'static str,
    pub name: &'static str,
    pub slots: &'static [Slot],
    /// Empty: a charge without components (always ready).
    pub stages: &'static [Stage],
    /// For a charge: its blast (and it burns a fuse before it).
    pub charge: Option<Blast>,
}

const LENS_EARLY: Slot = Slot { label: "Lentes explosivas (1ª geração)", item: "ntm:early_explosive_lenses" };
const LENS: Slot = Slot { label: "Lentes explosivas", item: "ntm:explosive_lenses" };
const N2: Slot = Slot { label: "Carga N²", item: "ntm:n2_charge" };

/// The fuse of a lit charge (`EntityTNTPrimedBase`: 80 ticks).
pub const FUSE_TICKS: u32 = 80;

pub const TYPES: &[BombType] = &[
    BombType {
        id: "ntm:tnt_ntm",
        name: "TNT do NTM",
        slots: &[],
        stages: &[],
        charge: Some(Blast::Conventional { power: 10.0 }),
    },
    BombType {
        id: "ntm:dynamite",
        name: "Dinamite",
        slots: &[],
        stages: &[],
        charge: Some(Blast::Conventional { power: 8.0 }),
    },
    BombType {
        id: "ntm:semtex",
        name: "Semtex",
        slots: &[],
        stages: &[],
        charge: Some(Blast::Conventional { power: 12.0 }),
    },
    BombType {
        id: "ntm:c4",
        name: "C-4",
        slots: &[],
        stages: &[],
        charge: Some(Blast::Conventional { power: 15.0 }),
    },
    // TileEntityNukeBoy.isReady: the five parts, in their slots.
    BombType {
        id: "ntm:nuke_boy",
        name: "Little Boy",
        slots: &[
            Slot { label: "Blindagem de nêutrons", item: "ntm:boy_shielding" },
            Slot { label: "Alvo de U235", item: "ntm:boy_target" },
            Slot { label: "Projétil de U235", item: "ntm:boy_bullet" },
            Slot { label: "Propelente", item: "ntm:boy_propellant" },
            Slot { label: "Ignitor", item: "ntm:boy_igniter" },
        ],
        stages: &[Stage { slots: 5, blast: Blast::Nuke { radius: 120, fallout: true } }],
        charge: None,
    },
    // TileEntityNukeGadget.isReady: wiring, four early lenses, the core.
    BombType {
        id: "ntm:nuke_gadget",
        name: "The Gadget",
        slots: &[
            Slot { label: "Fiação", item: "ntm:gadget_wireing" },
            LENS_EARLY,
            LENS_EARLY,
            LENS_EARLY,
            LENS_EARLY,
            Slot { label: "Núcleo de plutônio grande", item: "ntm:gadget_core" },
        ],
        stages: &[Stage { slots: 6, blast: Blast::Nuke { radius: 150, fallout: true } }],
        charge: None,
    },
    // TileEntityNukeMan.isReady: igniter, four early lenses, the core.
    BombType {
        id: "ntm:nuke_man",
        name: "Fat Man",
        slots: &[
            Slot { label: "Unidade de disparo", item: "ntm:man_igniter" },
            LENS_EARLY,
            LENS_EARLY,
            LENS_EARLY,
            LENS_EARLY,
            Slot { label: "Núcleo de plutônio", item: "ntm:man_core" },
        ],
        stages: &[Stage { slots: 6, blast: Blast::Nuke { radius: 175, fallout: true } }],
        charge: None,
    },
    // TileEntityNukeMike: isReady (lenses + man_core) and isFilled (+ the
    // deuterium stage). NukeMike.igniteTestBomb ignores its radius argument
    // and always uses mikeRadius (NukeMike.java:133): both stages are 250.
    BombType {
        id: "ntm:nuke_mike",
        name: "Ivy Mike",
        slots: &[
            LENS,
            LENS,
            LENS,
            LENS,
            Slot { label: "Núcleo de plutônio", item: "ntm:man_core" },
            Slot { label: "Tanque revestido de urânio", item: "ntm:mike_core" },
            Slot { label: "Tanque de deutério", item: "ntm:mike_deut" },
            Slot { label: "Unidade de resfriamento", item: "ntm:mike_cooling_unit" },
        ],
        stages: &[
            Stage { slots: 5, blast: Blast::Nuke { radius: 250, fallout: true } },
            Stage { slots: 8, blast: Blast::Nuke { radius: 250, fallout: true } },
        ],
        charge: None,
    },
    // TileEntityNukeTsar: isReady (lenses + man_core, manRadius) and
    // isFilled (+ tsar_core, tsarRadius).
    BombType {
        id: "ntm:nuke_tsar",
        name: "Tsar Bomba",
        slots: &[
            LENS,
            LENS,
            LENS,
            LENS,
            Slot { label: "Núcleo de plutônio", item: "ntm:man_core" },
            Slot { label: "Núcleo da Tsar", item: "ntm:tsar_core" },
        ],
        stages: &[
            Stage { slots: 5, blast: Blast::Nuke { radius: 175, fallout: true } },
            Stage { slots: 6, blast: Blast::Nuke { radius: 500, fallout: true } },
        ],
        charge: None,
    },
    // TileEntityNukeN2.isReady: twelve charges; statFacNoRad.
    BombType {
        id: "ntm:nuke_n2",
        name: "Mina N²",
        slots: &[N2, N2, N2, N2, N2, N2, N2, N2, N2, N2, N2, N2],
        stages: &[Stage { slots: 12, blast: Blast::Nuke { radius: 200, fallout: false } }],
        charge: None,
    },
    // TileEntityNukeBalefire.isLoaded: the egg and a spark battery
    // (destructionRange 250). Its timer screen is left out: it goes off by
    // redstone or the detonator, as the other bombs.
    BombType {
        id: "ntm:nuke_fstbmb",
        name: "Bomba de Fogo Maldito",
        slots: &[
            Slot { label: "Ovo de fogo maldito", item: "ntm:egg_balefire" },
            Slot { label: "Bateria de faísca", item: "ntm:battery_spark" },
        ],
        stages: &[Stage { slots: 2, blast: Blast::Balefire { radius: 250 } }],
        charge: None,
    },
];

pub fn bomb_type(id: &str) -> Option<&'static BombType> {
    TYPES.iter().find(|t| t.id == id)
}

/// The detonator (`ItemDetonator`): sneak + use on a bomb links it, use
/// elsewhere sets the linked bomb off.
pub const DETONATOR: &str = "ntm:detonator";

pub mod actions {
    /// Put the held item into the first free slot it fits.
    pub const INSERT: u32 = 1;
    /// Light the fuse of a charge.
    pub const IGNITE: u32 = 2;
    /// Take the last component put in out.
    pub const TAKE_LAST: u32 = 3;
    /// Take the component of slot `i` out: `TAKE + i`.
    pub const TAKE: u32 = 100;
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Bomb {
    /// The block it is (its type).
    pub id: String,
    /// One per slot of its type.
    pub slots: Vec<Option<String>>,
    /// A lit fuse: ticks left.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuse: Option<u32>,
}

impl Bomb {
    pub fn new(id: &str) -> Self {
        let n = bomb_type(id).map_or(0, |t| t.slots.len());
        Self {
            id: id.into(),
            slots: vec![None; n],
            fuse: None,
        }
    }

    pub fn kind(&self) -> Option<&'static BombType> {
        bomb_type(&self.id)
    }

    pub(crate) fn slot_ok(&self, i: usize) -> bool {
        let Some(t) = self.kind() else { return false };
        t.slots.get(i).is_some_and(|s| self.slots.get(i).and_then(Option::as_ref).is_some_and(|it| it == s.item))
    }

    /// What it would do if set off now: None while not ready.
    pub fn blast(&self) -> Option<Blast> {
        let t = self.kind()?;
        if let Some(b) = t.charge {
            return Some(b);
        }
        t.stages
            .iter()
            .rev()
            .find(|s| (0..s.slots).all(|i| self.slot_ok(i)))
            .map(|s| s.blast)
    }

    /// The fuse burnt down this tick.
    pub fn fuse_out(&self) -> bool {
        self.fuse == Some(0)
    }

    fn free_slot_for(&self, item: &str) -> Option<usize> {
        let t = self.kind()?;
        (0..t.slots.len()).find(|&i| t.slots[i].item == item && self.slots.get(i).is_some_and(Option::is_none))
    }
}

impl Logic for Bomb {
    fn tick(&mut self) -> bool {
        match &mut self.fuse {
            Some(n) if *n > 0 => {
                *n -= 1;
                true
            }
            _ => false,
        }
    }

    fn level(&self, _max: u8) -> u8 {
        0
    }

    fn view(&self) -> ViewModel {
        let Some(t) = self.kind() else {
            return ViewModel::new(self.id.clone()).text("Bomba desconhecida.", view::RED);
        };
        let mut vm = ViewModel::new(t.name);
        if let Some(b) = t.charge {
            let Blast::Conventional { power } = b else { unreachable!() };
            vm = vm.text(format!("Carga explosiva, força {power}"), view::GREY);
            vm = match self.fuse {
                Some(n) => vm.text(format!("Pavio aceso: {:.1} s", f64::from(n) / 20.0), view::RED),
                None => vm
                    .text("Redstone, detonador ou pavio.", view::GREY)
                    .button(actions::IGNITE, "Acender o pavio (4 s)"),
            };
            return vm;
        }
        // One line per kind of slot (twelve N² charges are one line): the
        // screen has room for about nine rows.
        let mut seen: Vec<&str> = Vec::new();
        for s in t.slots {
            if seen.contains(&s.label) {
                continue;
            }
            seen.push(s.label);
            let idx: Vec<usize> = (0..t.slots.len()).filter(|&i| t.slots[i].label == s.label).collect();
            let have = idx.iter().filter(|&&i| self.slot_ok(i)).count();
            let text = if idx.len() == 1 {
                format!("{}: {}", s.label, if have == 1 { "ok" } else { "vazio" })
            } else {
                format!("{}: {have}/{}", s.label, idx.len())
            };
            vm = if have > 0 { vm.item(s.item, text) } else { vm.text(text, view::GREY) };
        }
        vm = match self.blast() {
            Some(b) => vm.text(format!("Pronta (raio {})", b.radius()), view::GREEN),
            None => vm.text("Faltam componentes", view::YELLOW),
        };
        vm = vm.button(actions::INSERT, "Pôr o componente da mão");
        if self.slots.iter().any(Option::is_some) {
            vm = vm.button(actions::TAKE_LAST, "Retirar um componente");
        }
        vm
    }

    fn action(&mut self, action: u32, held: Option<&Held>, fx: &mut Vec<Effect>) {
        match action {
            actions::INSERT => {
                let Some(held) = held else {
                    fx.push(Effect::Toast("Segure um componente da bomba na mão.".into()));
                    return;
                };
                match self.free_slot_for(&held.item) {
                    Some(_) => fx.push(Effect::TakeHeld { item: held.item.clone() }),
                    None => fx.push(Effect::Toast("Esse item não cabe em nenhum lugar livre desta bomba.".into())),
                }
            }
            actions::IGNITE => {
                if self.kind().is_some_and(|t| t.charge.is_some()) && self.fuse.is_none() {
                    self.fuse = Some(FUSE_TICKS);
                }
            }
            actions::TAKE_LAST => {
                if let Some(item) = self.slots.iter_mut().rev().find_map(Option::take) {
                    fx.push(Effect::Give { item, count: 1 });
                }
            }
            a if a >= actions::TAKE => {
                let i = (a - actions::TAKE) as usize;
                if let Some(item) = self.slots.get_mut(i).and_then(Option::take) {
                    fx.push(Effect::Give { item, count: 1 });
                }
            }
            _ => {}
        }
    }

    fn consumed(&mut self, item: &str, fx: &mut Vec<Effect>) {
        match self.free_slot_for(item) {
            Some(i) => self.slots[i] = Some(item.into()),
            // Filled meanwhile: the item goes back.
            None => fx.push(Effect::Give { item: item.into(), count: 1 }),
        }
    }

    fn unconsume(&mut self, item: &str) -> bool {
        self.slots.iter_mut().rev().any(|s| {
            let mut one = s.take().map(|i| (i, 1));
            let hit = remove_one(&mut one, item);
            *s = one.map(|(i, _)| i);
            hit
        })
    }

    fn contents(&self) -> Vec<(String, u32)> {
        self.slots.iter().flatten().map(|i| (i.clone(), 1)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill(b: &mut Bomb, n: usize) {
        let t = b.kind().unwrap();
        for i in 0..n {
            let mut fx = Vec::new();
            b.consumed(t.slots[i].item, &mut fx);
            assert!(fx.is_empty());
        }
    }

    #[test]
    fn every_type_has_ntm_radii() {
        let r = |id: &str, n: usize| {
            let mut b = Bomb::new(id);
            fill(&mut b, n);
            b.blast().map(|b| b.radius())
        };
        assert_eq!(r("ntm:nuke_boy", 5), Some(120));
        assert_eq!(r("ntm:nuke_boy", 4), None);
        assert_eq!(r("ntm:nuke_gadget", 6), Some(150));
        assert_eq!(r("ntm:nuke_man", 6), Some(175));
        assert_eq!(r("ntm:nuke_mike", 5), Some(250));
        assert_eq!(r("ntm:nuke_tsar", 5), Some(175));
        assert_eq!(r("ntm:nuke_tsar", 6), Some(500));
        assert_eq!(r("ntm:nuke_n2", 12), Some(200));
        assert_eq!(r("ntm:nuke_n2", 11), None);
        assert_eq!(r("ntm:nuke_fstbmb", 2), Some(250));
        assert_eq!(r("ntm:c4", 0), Some(15));
        assert!(matches!(Bomb::new("ntm:nuke_n2").kind().unwrap().stages[0].blast, Blast::Nuke { fallout: false, .. }));
    }

    #[test]
    fn components_go_to_their_own_slots_and_back() {
        let mut b = Bomb::new("ntm:nuke_man");
        let mut fx = Vec::new();
        let held = Held { item: "ntm:man_core".into(), count: 1 };
        b.action(actions::INSERT, Some(&held), &mut fx);
        assert_eq!(fx, vec![Effect::TakeHeld { item: "ntm:man_core".into() }]);
        fx.clear();
        b.consumed("ntm:man_core", &mut fx);
        assert_eq!(b.slots[5].as_deref(), Some("ntm:man_core"));
        // A second one has no free slot.
        b.action(actions::INSERT, Some(&held), &mut fx);
        assert!(matches!(fx[0], Effect::Toast(_)));
        fx.clear();
        // Four lenses fill slots 1..=4.
        for _ in 0..4 {
            b.consumed("ntm:early_explosive_lenses", &mut fx);
        }
        assert!(b.slots[1..5].iter().all(Option::is_some));
        assert_eq!(b.contents().len(), 5);
        b.action(actions::TAKE + 5, None, &mut fx);
        assert_eq!(fx, vec![Effect::Give { item: "ntm:man_core".into(), count: 1 }]);
        assert!(b.unconsume("ntm:early_explosive_lenses"));
        assert_eq!(b.contents().len(), 3);
    }

    #[test]
    fn a_charge_burns_its_fuse() {
        let mut b = Bomb::new("ntm:dynamite");
        b.action(actions::IGNITE, None, &mut Vec::new());
        for _ in 0..FUSE_TICKS {
            assert!(!b.fuse_out());
            b.tick();
        }
        assert!(b.fuse_out());
    }
}
