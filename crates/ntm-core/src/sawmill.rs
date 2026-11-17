//! The sawmill (`TileEntitySawmill`, `MachineSawmill`), D3 of Onda 2: what its record keeps. The Java's NBT keeps only
//! `hasBlade` and `progress` (`readFromNBT`, 190-200). The step (recipes, overspeed and the blade's failure through the
//! ledger) comes with step 3; until then the sawmill only keeps its blade across breaking and placing its item.
use crate::machine::Logic;
use crate::view::ViewModel;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sawmill {
    /// `hasBlade`: a new sawmill has its blade; its item without one (the Java's damage 1, here
    /// `minecraft:custom_data {"ntm:no_blade":1b}`) places one without (`MachineSawmill.onBlockPlacedBy`).
    #[serde(default = "with_blade")]
    pub has_blade: bool,
    #[serde(default)]
    pub progress: i32,
}

fn with_blade() -> bool {
    true
}

impl Default for Sawmill {
    fn default() -> Self {
        Self { has_blade: true, progress: 0 }
    }
}

impl Logic for Sawmill {
    fn level(&self, _max: u8) -> u8 {
        0
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Sawmill")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_sawmill_has_its_blade_and_an_old_record_keeps_what_it_said() {
        assert!(Sawmill::default().has_blade);
        let without: Sawmill = serde_json::from_str(r#"{"has_blade":false,"progress":7}"#).unwrap();
        assert_eq!(without, Sawmill { has_blade: false, progress: 7 });
        let bare: Sawmill = serde_json::from_str("{}").unwrap();
        assert!(bare.has_blade, "a record without the field keeps the blade, as a new sawmill");
    }
}
