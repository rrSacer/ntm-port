//! The RBMK crane (`TileEntityCraneConsole`, T11) and what it carries rods between: the storage column's queue
//! (`TileEntityRBMKStorage`) and the columns it loads and unloads (`IRBMKLoadable`). Pure: the server finds the
//! columns, reads the players' keys and publishes a transfer as one commit of both blocks.
use crate::rbmk::persist;

/// The storage column's block.
pub const STORAGE: &str = "ntm:rbmk_storage";

/// `TileEntityRBMKStorage.updateEntity` every 10 ticks: one ascending pass, each successor into the gap before it
/// (an item moves one place a pass; a queue with gaps closes over several).
pub fn storage_pass<T>(slots: &mut [Option<T>]) {
    for i in 0..slots.len().saturating_sub(1) {
        if slots[i].is_none() && slots[i + 1].is_some() {
            slots[i] = slots[i + 1].take();
        }
    }
}

/// Which slot a column is loaded into and unloaded from (`IRBMKLoadable`): the channels' one slot; the storage takes
/// in at 11 and gives out from 0. None: not loadable.
pub fn loadable_slots(block: &str) -> Option<(usize, usize)> {
    if persist::is_channel(block) {
        Some((0, 0))
    } else if block == STORAGE {
        Some((11, 0))
    } else {
        None
    }
}

/// `canLoad`: the slot it loads into is empty (and, for a channel, there is something to load).
pub fn can_load<T>(block: &str, slots: &[Option<T>]) -> bool {
    loadable_slots(block).is_some_and(|(into, _)| slots.get(into).is_some_and(Option::is_none))
}

/// `canUnload`: the slot it gives from holds something.
pub fn can_unload<T>(block: &str, slots: &[Option<T>]) -> bool {
    loadable_slots(block).is_some_and(|(_, from)| slots.get(from).is_some_and(Option::is_some))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One pass moves each item one place down into a gap, ascending: an item lands where the one before it left.
    #[test]
    fn the_storage_closes_its_gaps_one_place_a_pass() {
        let mut s = [None, Some(1), None, Some(2), Some(3), None, None, None, None, None, None, Some(4)];
        storage_pass(&mut s);
        // 1 into 0; 2 into 2, and then 3 into 3 (each successor into the gap the step before left); 4 into 10.
        assert_eq!(s, [Some(1), None, Some(2), Some(3), None, None, None, None, None, None, Some(4), None]);
        storage_pass(&mut s);
        assert_eq!(s, [Some(1), Some(2), Some(3), None, None, None, None, None, None, Some(4), None, None]);
        let mut full = [Some(1); 12];
        storage_pass(&mut full);
        assert_eq!(full, [Some(1); 12]);
        storage_pass::<i32>(&mut []);
    }

    /// A channel loads into and unloads from its slot; the storage loads at 11 and unloads from 0; other columns are not
    /// loadable.
    #[test]
    fn what_the_crane_may_load_and_unload() {
        let empty: Vec<Option<i32>> = vec![None];
        assert!(can_load("ntm:rbmk_rod_reasim_mod", &empty) && !can_unload("ntm:rbmk_rod", &empty));
        assert!(!can_load("ntm:rbmk_rod", &[Some(1)]) && can_unload("ntm:rbmk_rod", &[Some(1)]));
        let mut st: Vec<Option<i32>> = vec![None; 12];
        st[0] = Some(1);
        assert!(can_load(STORAGE, &st) && can_unload(STORAGE, &st));
        st[11] = Some(2);
        assert!(!can_load(STORAGE, &st));
        st[0] = None;
        assert!(!can_unload(STORAGE, &st), "only slot 0 gives out");
        assert!(!can_load("ntm:rbmk_blank", &empty) && !can_unload("ntm:rbmk_boiler", &[Some(1)]));
        assert!(!can_load::<i32>("ntm:rbmk_rod", &[]), "no slot: nothing");
    }
}
