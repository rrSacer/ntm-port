//! The continuous caster (`ntm:machine_strand_caster`, docs/content-foundry.md):
//! `tileentity/machine/TileEntityMachineStrandCaster.java` and
//! `blocks/machine/MachineStrandCaster.java`.
//!
//! This file holds what a jet of molten metal needs to find it: the four
//! funnel cells above its core (`getMetalPourPos`, `MachineStrandCaster.fillSpace`
//! makes them the `moltenMetal` proxies) and the face rule of
//! `canAcceptPartialPour`.

use crate::Pos;
use crate::registry::facing_dir;

pub const BLOCK: &str = "ntm:machine_strand_caster";

/// `getMetalPourPos` (`TileEntityMachineStrandCaster.java:204-209`): with
/// `rot = dir.getRotation(UP)`, the cells at `y + 2` over `core + rot - dir`,
/// `core - dir`, `core + rot` and `core`.
pub fn pour_cells(core: Pos, facing: &str) -> Vec<Pos> {
    let Some(d) = facing_dir(facing) else { return Vec::new() };
    let rot = [-d[2], 0, d[0]];
    let at = |r: i32, k: i32| [core[0] + rot[0] * r - d[0] * k, core[1] + 2, core[2] + rot[2] * r - d[2] * k];
    vec![at(1, 1), at(0, 1), at(1, 0), at(0, 0)]
}

/// The geometry half of `canAcceptPartialPour`
/// (`TileEntityMachineStrandCaster.java:211-224`): only the face UP, and only
/// one of the four funnel cells.
pub fn takes_pour_at(core: Pos, facing: &str, cell: Pos, face: [i32; 3]) -> bool {
    face == [0, 1, 0] && pour_cells(core, facing).contains(&cell)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_distinct_funnel_cells_two_above_the_core() {
        for facing in ["north", "south", "east", "west"] {
            let core = [10, 64, -3];
            let cells = pour_cells(core, facing);
            assert_eq!(cells.len(), 4, "{facing}");
            assert!(cells.iter().all(|c| c[1] == 66), "{facing}");
            assert!(cells.contains(&[10, 66, -3]), "the cell over the core: {facing}");
            let mut sorted = cells.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), 4, "{facing}");
        }
    }

    #[test]
    fn funnel_cells_follow_dir_and_its_rotation_up() {
        // North: dir = (0,0,-1), rot = dir.getRotation(UP) = (1,0,0).
        let core = [0, 64, 0];
        let mut cells = pour_cells(core, "north");
        cells.sort();
        assert_eq!(cells, vec![[0, 66, 0], [0, 66, 1], [1, 66, 0], [1, 66, 1]]);
        // East: dir = (1,0,0), rot = (0,0,1).
        let mut cells = pour_cells(core, "east");
        cells.sort();
        assert_eq!(cells, vec![[-1, 66, 0], [-1, 66, 1], [0, 66, 0], [0, 66, 1]]);
    }

    #[test]
    fn only_the_face_up_of_a_funnel_cell_takes_a_pour() {
        let core = [0, 64, 0];
        assert!(takes_pour_at(core, "north", [1, 66, 1], [0, 1, 0]));
        assert!(takes_pour_at(core, "north", [0, 66, 0], [0, 1, 0]));
        for face in [[0, -1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
            assert!(!takes_pour_at(core, "north", [1, 66, 1], face), "{face:?}");
        }
        // The funnel is 3 wide: the third column and the layer under are not pour cells.
        assert!(!takes_pour_at(core, "north", [-1, 66, 0], [0, 1, 0]));
        assert!(!takes_pour_at(core, "north", [1, 65, 1], [0, 1, 0]));
        assert!(!takes_pour_at(core, "bogus", [0, 66, 0], [0, 1, 0]));
    }
}
