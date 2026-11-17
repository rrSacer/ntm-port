//! The power switch and the redstone power switch (`ntm:cable_switch`,
//! `ntm:cable_detector`): `CableSwitch.java`, `CableDetector.java` and
//! `TileEntityCableSwitch.java`.
//!
//! One block entity (`TileEntityCableSwitch`, a `TileEntityCableBaseNT`) whose
//! block metadata is the state: 0 off, 1 on. Only the on state has a node in
//! the power net (`shouldCreateNode`: `getBlockMetadata() == 1`): on, it is a
//! cable open on all six faces; off, it joins nothing and the net is split
//! there. The switch flips on a plain right click (`onBlockActivated`, not
//! sneaking); the detector, whatever its name, measures nothing: redstone
//! power at the block (`isBlockIndirectlyGettingPowered`) sets its state when
//! a neighbour changes (`onNeighborBlockChange`), and it takes no click.
//! Both play `hbm:block.reactorStart` (pitch 1.0 on, 0.85 off), a sound the
//! catalog leaves out (`excluded_by_license`).

use crate::{
    energy_network::{detector_meta_after, switch_joins_net, switch_toggled},
    machine::Logic,
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub const SWITCH: &str = "ntm:cable_switch";
pub const DETECTOR: &str = "ntm:cable_detector";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CableSwitch {
    /// The redstone one (`CableDetector`): driven by the signal, not by a click.
    #[serde(default)]
    pub detector: bool,
    /// Block metadata 1.
    #[serde(default)]
    pub on: bool,
    /// The redstone seen at the last look: the NTM acts when a neighbour
    /// changes, not on a level, so a block put beside a powered one stays off
    /// until the signal changes (the first look, also after a reload, only
    /// takes the level).
    #[serde(skip)]
    pub seen: Option<bool>,
}

impl CableSwitch {
    pub fn new(block: &str) -> Self {
        Self { detector: block == DETECTOR, on: false, seen: None }
    }

    /// The block metadata: 0 off, 1 on.
    pub fn meta(&self) -> u8 {
        u8::from(self.on)
    }

    /// Whether it is a node of the power net now (`shouldCreateNode`).
    pub fn joins_net(&self) -> bool {
        switch_joins_net(self.meta())
    }

    /// `CableSwitch.onBlockActivated` (not sneaking): flips the state. The
    /// detector has no click.
    pub fn click(&mut self) -> bool {
        if self.detector {
            return false;
        }
        self.on = switch_toggled(self.meta()) == 1;
        true
    }

    /// `CableDetector.onNeighborBlockChange`, with the redstone now at the
    /// block: true when the state changed. The switch ignores redstone.
    pub fn redstone(&mut self, powered: bool) -> bool {
        if !self.detector {
            return false;
        }
        let before = self.seen.replace(powered);
        if before.is_none_or(|b| b == powered) {
            return false;
        }
        match detector_meta_after(powered, self.meta()) {
            Some(meta) => {
                self.on = meta == 1;
                true
            }
            None => false,
        }
    }
}

impl Logic for CableSwitch {
    fn level(&self, max: u8) -> u8 {
        if self.on { max } else { 0 }
    }

    fn view(&self) -> ViewModel {
        ViewModel::new(if self.detector { "Interruptor de redstone" } else { "Interruptor de energia" })
            .text(if self.on { "Ligado" } else { "Desligado" }, view::GREY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_starts_off_and_a_click_flips_the_switch() {
        let mut s = CableSwitch::new(SWITCH);
        assert_eq!((s.detector, s.on, s.meta(), s.joins_net()), (false, false, 0, false));
        assert!(s.click());
        assert_eq!((s.on, s.meta(), s.joins_net(), s.level(5)), (true, 1, true, 5));
        assert!(s.click());
        assert_eq!((s.on, s.meta(), s.joins_net(), s.level(5)), (false, 0, false, 0));
    }

    #[test]
    fn the_detector_takes_no_click_and_the_switch_no_redstone() {
        let mut d = CableSwitch::new(DETECTOR);
        assert!(d.detector);
        assert!(!d.click());
        assert!(!d.on);
        let mut s = CableSwitch::new(SWITCH);
        assert!(!s.redstone(false));
        assert!(!s.redstone(true));
        assert!(!s.on);
        assert_eq!(s.seen, None, "the switch looks at nothing");
    }

    /// `onNeighborBlockChange` only runs when a neighbour changes: the
    /// level at the first look is a baseline, not a change.
    #[test]
    fn the_detector_follows_the_redstone_after_its_first_look() {
        let mut d = CableSwitch::new(DETECTOR);
        assert!(!d.redstone(true), "put beside a powered block: no change yet");
        assert!(!d.on);
        assert!(!d.redstone(true), "still powered");
        assert!(!d.redstone(false) && !d.on, "off and unpowered: nothing");
        assert!(d.redstone(true), "a rising signal turns it on");
        assert!(d.on && d.joins_net());
        assert!(!d.redstone(true));
        assert!(d.redstone(false), "a falling signal turns it off");
        assert!(!d.on && !d.joins_net());
    }

    #[test]
    fn an_on_detector_after_a_reload_waits_for_a_change() {
        let mut d = CableSwitch::new(DETECTOR);
        d.on = true;
        let text = serde_json::to_string(&d).unwrap();
        let mut back: CableSwitch = serde_json::from_str(&text).unwrap();
        assert!(back.on && back.seen.is_none());
        assert!(!back.redstone(false), "unpowered at the first look: it stays on until a neighbour changes");
        assert!(back.on);
        assert!(!back.redstone(true), "powered while on: nothing");
        assert!(back.redstone(false), "then a falling signal turns it off");
    }

    #[test]
    fn the_state_survives_a_save_and_old_ones_open() {
        let mut s = CableSwitch::new(SWITCH);
        s.click();
        let back: CableSwitch = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
        let bare: CableSwitch = serde_json::from_str("{}").unwrap();
        assert_eq!(bare, CableSwitch::default());
    }

    #[test]
    fn the_screen_text_names_the_state() {
        let mut s = CableSwitch::new(DETECTOR);
        assert!(format!("{:?}", s.view()).contains("Desligado"));
        s.seen = Some(false);
        s.redstone(true);
        assert!(format!("{:?}", s.view()).contains("Ligado"));
    }
}
