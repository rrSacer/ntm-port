//! Door, seal and lock logic, checked against the Java numbers
//! (`ntm-src/src/main/java/com/hbm/tileentity/DoorDecl.java` and the
//! classes listed in `doors_security.rs`).

use ntm_core::doors_security::*;
use std::collections::BTreeSet;

/// One row per `DoorDecl`, as written in the Java: id, `timeToOpen`, the
/// ranges, `getDimensions`, `getExtraDimensions` (count), `getBlockOffset`,
/// `remoteControllable`, `neverDemandRedstone`.
#[test]
fn every_door_decl_matches_the_java_numbers() {
    type Row = (
        DoorKind,
        &'static str,
        u32,
        &'static [[i32; 6]],
        [i32; 6],
        usize,
        i32,
        bool,
        bool,
    );
    let rows: [Row; 14] = [
        (
            DoorKind::TransitionSeal,
            "ntm:transition_seal",
            480,
            &[[-9, 2, 0, 20, 20, 1]],
            [23, 0, 0, 0, 13, 12],
            0,
            0,
            false,
            false,
        ),
        (
            DoorKind::VaultDoor,
            "ntm:vault_door",
            120,
            &[[-1, 1, 0, 3, 3, 2]],
            [4, 0, 0, 0, 2, 2],
            1,
            0,
            false,
            false,
        ),
        (
            DoorKind::FireDoor,
            "ntm:fire_door",
            160,
            &[[-1, 0, 0, 3, 4, 1]],
            [2, 0, 0, 0, 2, 1],
            0,
            0,
            false,
            false,
        ),
        (
            DoorKind::SlidingBlastDoor,
            "ntm:sliding_blast_door",
            24,
            &[[-2, 0, 0, 4, 5, 1]],
            [3, 0, 0, 0, 3, 3],
            0,
            0,
            false,
            false,
        ),
        (
            DoorKind::SlidingSealDoor,
            "ntm:sliding_seal_door",
            20,
            &[[0, 0, 0, 1, 2, 2]],
            [1, 0, 0, 0, 0, 0],
            0,
            0,
            false,
            false,
        ),
        (
            DoorKind::SecureAccessDoor,
            "ntm:secure_access_door",
            120,
            &[[-2, 1, 0, 4, 5, 1]],
            [4, 0, 0, 0, 2, 2],
            0,
            0,
            false,
            false,
        ),
        (
            DoorKind::RoundAirlockDoor,
            "ntm:round_airlock_door",
            60,
            &[[0, 0, 0, -2, 4, 2], [0, 0, 0, 3, 4, 2]],
            [3, 0, 0, 0, 2, 1],
            0,
            0,
            false,
            false,
        ),
        (
            DoorKind::QeSlidingDoor,
            "ntm:qe_sliding_door",
            10,
            &[[0, 0, 0, 2, 2, 2]],
            [1, 0, 0, 0, 1, 0],
            0,
            0,
            false,
            false,
        ),
        (
            DoorKind::QeContainment,
            "ntm:qe_containment",
            160,
            &[[-1, 0, 0, 3, 3, 1]],
            [2, 0, 0, 0, 1, 1],
            0,
            0,
            false,
            false,
        ),
        (
            DoorKind::WaterDoor,
            "ntm:water_door",
            60,
            &[[1, 0, 0, -3, 3, 2]],
            [2, 0, 0, 0, 1, 1],
            0,
            0,
            false,
            true,
        ),
        (
            DoorKind::SiloHatch,
            "ntm:silo_hatch",
            60,
            &[
                [1, 0, 1, -3, 3, 0],
                [0, 0, 1, -3, 3, 0],
                [-1, 0, 1, -3, 3, 0],
            ],
            [0, 0, 2, 2, 2, 2],
            0,
            2,
            true,
            false,
        ),
        (
            DoorKind::SiloHatchLarge,
            "ntm:silo_hatch_large",
            60,
            &[
                [2, 0, 1, -3, 3, 0],
                [1, 0, 2, -5, 3, 0],
                [0, 0, 2, -5, 3, 0],
                [-1, 0, 2, -5, 3, 0],
                [-2, 0, 1, -3, 3, 0],
            ],
            [0, 0, 3, 3, 3, 3],
            0,
            3,
            true,
            false,
        ),
        (
            DoorKind::LargeVehicleDoor,
            "ntm:large_vehicle_door",
            60,
            &[[0, 0, 0, -4, 6, 2], [0, 0, 0, 4, 6, 2]],
            [5, 0, 0, 0, 3, 3],
            0,
            0,
            false,
            false,
        ),
        (
            DoorKind::CargoDoor,
            "ntm:cargo_door",
            60,
            &[[-1, -1, 0, 3, 3, 1]],
            [2, 0, 0, 0, 1, 1],
            0,
            0,
            false,
            false,
        ),
    ];
    for (kind, id, time, ranges, dims, extra, offset, remote, never) in rows {
        let s = spec(kind);
        assert_eq!(s.kind, kind, "{id}");
        assert_eq!(s.id, id);
        assert_eq!(s.time_to_open, time, "{id} timeToOpen");
        assert_eq!(s.open_ranges, ranges, "{id} open ranges");
        assert_eq!(s.dimensions, dims, "{id} dimensions");
        assert_eq!(s.extra_dimensions.len(), extra, "{id} extra dimensions");
        assert_eq!(s.block_offset, offset, "{id} block offset");
        assert_eq!(s.remote_controllable, remote, "{id} remote");
        assert_eq!(s.never_demand_redstone, never, "{id} never demand redstone");
    }
}

#[test]
fn vault_extra_box_is_a_pocket_one_row_in_front() {
    let vault = spec(DoorKind::VaultDoor);
    // Java: {0, 0, 1, -1, 2, 2} → z from -1 to -1, y 0, x -2..=2 (five cells).
    let cells = footprint((0, 0, 0), vault.extra_dimensions[0], Facing::South);
    let expected: Vec<Cell> = (-2..=2).map(|x| (x, 0, -1)).collect();
    assert_eq!(cells, expected);
}

#[test]
fn vault_footprint_is_five_by_five() {
    let vault = spec(DoorKind::VaultDoor);
    for facing in [Facing::South, Facing::East] {
        assert_eq!(
            footprint((0, 0, 0), vault.dimensions, facing).len(),
            25,
            "{facing:?}"
        );
    }
}

#[test]
fn rotate_dims_swaps_like_the_java_table() {
    let d = [1, 2, 3, 4, 5, 6];
    assert_eq!(rotate_dims(d, Facing::South), d);
    assert_eq!(rotate_dims(d, Facing::North), [1, 2, 4, 3, 6, 5]);
    assert_eq!(rotate_dims(d, Facing::East), [1, 2, 6, 5, 3, 4]);
    assert_eq!(rotate_dims(d, Facing::West), [1, 2, 5, 6, 4, 3]);
}

#[test]
fn range_time_windows() {
    let vault = spec(DoorKind::VaultDoor);
    assert_eq!(range_time(vault, 0), 0.0);
    assert_eq!(range_time(vault, 60), 0.5);
    assert_eq!(range_time(vault, 500), 1.0);

    let water = spec(DoorKind::WaterDoor);
    assert_eq!(range_time(water, 35), 0.0);
    assert!((range_time(water, 37) - 0.4).abs() < 1e-6);
    assert_eq!(range_time(water, 40), 1.0);
}

#[test]
fn water_door_window_exposes_the_expected_cells() {
    let water = spec(DoorKind::WaterDoor);
    // Fully open: 3 steps × 3 wide = 9 cells, minus the core (0, 0, 0) that the walk passes.
    assert_eq!(moving_cells(water, Facing::South, 40, true).len(), 8);
    // At 37 ticks the fraction is 0.4: steps 0 and 1 (fractions 0 and 0.25) pass, step 2 (0.5) stops.
    // Step 1 reaches the core, so it contributes two cells: 3 + 2 = 5.
    assert_eq!(moving_cells(water, Facing::South, 37, true).len(), 5);
}

#[test]
fn cargo_door_full_open_leaves_out_the_core() {
    let cargo = spec(DoorKind::CargoDoor);
    let cells = moving_cells(cargo, Facing::South, 60, true);
    assert_eq!(cells.len(), 8);
    assert!(!cells.contains(&(0, 0, 0)));
}

#[test]
fn silo_hatch_quirk_at_exactly_twenty_ticks() {
    let silo = spec(DoorKind::SiloHatch);
    // Java: getNormTime(t, 20, 20) is 0/0 = NaN at t == 20, so every step passes.
    assert!(range_time(silo, 20).is_nan());
    assert_eq!(moving_cells(silo, Facing::South, 19, true).len(), 9);
    assert_eq!(moving_cells(silo, Facing::South, 20, true).len(), 26);
    assert_eq!(moving_cells(silo, Facing::South, 21, true).len(), 26);
}

#[test]
fn closing_walks_back_from_the_far_end() {
    let vault = spec(DoorKind::VaultDoor);
    // At 0 ticks left, the fraction is 0: closing keeps the steps at or above 0,
    // so every step still counts. At 60 (fraction 0.5) the first step is cut.
    assert_eq!(moving_cells(vault, Facing::South, 0, false).len(), 9);
    assert_eq!(moving_cells(vault, Facing::South, 60, false).len(), 6);
}

#[test]
fn padlock_and_counterfeit_keys() {
    let mut lock = Lock::default();
    assert!(
        !lock.apply_padlock(0, 0.1),
        "a padlock without pins does nothing"
    );
    assert!(lock.apply_padlock(9, 0.02));
    assert!(lock.locked);
    assert_eq!(lock.pins, 9);
    assert!(
        !lock.apply_padlock(4, 0.1),
        "a locked lock takes no second padlock"
    );

    assert_eq!(counterfeit_keys(&lock), Ok([9, 9]));
    let hard = Lock {
        cheesable: false,
        ..lock
    };
    assert_eq!(
        counterfeit_keys(&hard),
        Err(CounterfeitRefusal::TooElaborate)
    );
    assert_eq!(
        counterfeit_keys(&Lock::default()),
        Err(CounterfeitRefusal::NotLocked)
    );
}

#[test]
fn pins_and_red_key_open_locks() {
    let lock = Lock {
        pins: 42,
        locked: true,
        ..Lock::default()
    };
    assert!(lock.can_access(Some(Holding::Key(42))));
    assert!(!lock.can_access(Some(Holding::Key(41))));
    assert!(!lock.can_access(Some(Holding::Nothing)));
    assert!(!lock.can_access(None));
    assert!(lock.can_access(Some(Holding::RedKey)));
    assert!(
        Lock::default().can_access(None),
        "an unlocked lock opens for anyone"
    );
}

#[test]
fn lock_pick_chance_follows_the_modifier_and_the_jacket() {
    assert_eq!(pick_chance(0.1, false), 10.0);
    assert_eq!(pick_chance(0.1, true), 1000.0);
    let lock = Lock {
        locked: true,
        ..Lock::default()
    };
    assert!(lock.can_access(Some(Holding::Pick {
        jacket: false,
        roll: 9.99
    })));
    assert!(!lock.can_access(Some(Holding::Pick {
        jacket: false,
        roll: 10.0
    })));
    let unbreakable = Lock {
        locked: true,
        modifier: 0.0,
        ..Lock::default()
    };
    assert!(!unbreakable.can_access(Some(Holding::Pick {
        jacket: true,
        roll: 0.0
    })));
}

#[test]
fn locked_door_refuses_a_click_with_wrong_hand_but_still_uses_it() {
    let mut door = Door::new(spec(DoorKind::VaultDoor));
    door.lock = Lock {
        pins: 42,
        locked: true,
        ..Lock::default()
    };
    assert!(
        door.try_toggle(Some(Holding::Nothing)),
        "Java returns true even when the lock refuses"
    );
    assert_eq!(door.state, DoorState::Closed);
    assert!(
        !door.try_toggle(None),
        "no player: a locked door stays shut"
    );
    assert!(door.try_toggle(Some(Holding::Key(42))));
    assert_eq!(door.state, DoorState::Opening);
}

#[test]
fn vault_opens_in_120_ticks_and_closes_in_120() {
    let mut door = Door::new(spec(DoorKind::VaultDoor));
    door.try_toggle(Some(Holding::Nothing));
    for _ in 0..119 {
        door.tick(Facing::South);
    }
    assert_eq!(door.state, DoorState::Opening);
    door.tick(Facing::South);
    assert_eq!(door.state, DoorState::Open);
    assert_eq!(door.open_ticks, 120);

    assert!(door.try_toggle(Some(Holding::Nothing)));
    assert_eq!(door.state, DoorState::Closing);
    for _ in 0..120 {
        door.tick(Facing::South);
    }
    assert_eq!(door.state, DoorState::Closed);
    assert_eq!(door.open_ticks, 0);
}

#[test]
fn powered_door_opens_and_an_unpowered_open_one_closes_on_its_own() {
    let mut door = Door::new(spec(DoorKind::CargoDoor));
    let source = (5, 0, 5);
    door.update_redstone(source, true);
    assert_eq!(door.redstone, 1);
    assert!(
        !door.try_toggle(Some(Holding::Nothing)),
        "a powered closed door does not move"
    );
    door.tick(Facing::South);
    assert_eq!(door.state, DoorState::Opening);

    door.state = DoorState::Open;
    door.open_ticks = 60;
    door.update_redstone(source, false);
    assert_eq!(door.redstone, -1);
    door.tick(Facing::South);
    assert_eq!(door.state, DoorState::Closing);
    assert_eq!(door.redstone, 0, "the tick clears the -1");
}

#[test]
fn locked_door_ignores_redstone() {
    let mut door = Door::new(spec(DoorKind::CargoDoor));
    door.lock = Lock {
        pins: 7,
        locked: true,
        ..Lock::default()
    };
    door.state = DoorState::Open;
    door.open_ticks = 60;
    door.update_redstone((1, 1, 1), true);
    door.update_redstone((1, 1, 1), false);
    door.tick(Facing::South);
    assert_eq!(
        door.state,
        DoorState::Open,
        "passcode -1 does not match the pins"
    );
}

#[test]
fn only_remote_kinds_take_a_bomb() {
    assert!(!Door::new(spec(DoorKind::VaultDoor)).bomb_trigger());
    let mut silo = Door::new(spec(DoorKind::SiloHatch));
    assert!(silo.bomb_trigger());
    assert_eq!(silo.state, DoorState::Opening);
}

#[test]
fn ultra_larp_mode_only_lets_water_door_answer_players() {
    assert!(!Door::new(spec(DoorKind::VaultDoor)).player_can_click(true));
    assert!(Door::new(spec(DoorKind::WaterDoor)).player_can_click(true));
    assert!(Door::new(spec(DoorKind::VaultDoor)).player_can_click(false));
}

#[test]
fn redstone_count_goes_from_minus_one_to_zero_and_back() {
    let mut door = Door::new(spec(DoorKind::CargoDoor));
    door.update_redstone((0, 0, 0), true);
    door.update_redstone((1, 0, 0), true);
    assert_eq!(door.redstone, 2);
    door.update_redstone((0, 0, 0), false);
    door.update_redstone((1, 0, 0), false);
    assert_eq!(door.redstone, -1);
    door.update_redstone((2, 0, 0), false);
    assert_eq!(
        door.redstone, -1,
        "a source that was never on changes nothing"
    );
}

#[test]
fn blast_group_opens_neighbours_and_steps_the_dummies() {
    let mut group = BlastDoorGroup::new();
    group.add((0, 0, 0));
    group.add((1, 0, 0));
    group.toggle((0, 0, 0));
    assert_eq!(
        group.doors[&(1, 0, 0)].state,
        BlastState::Moving,
        "the neighbour opens with it"
    );

    let first = group.tick((0, 0, 0), false, false);
    group.tick((1, 0, 0), false, false);
    assert_eq!(
        first,
        vec![Dummy {
            place: false,
            dy: 1
        }]
    );
    for _ in 1..19 {
        group.tick((0, 0, 0), false, false);
        group.tick((1, 0, 0), false, false);
    }
    let at_twenty = group.tick((0, 0, 0), false, false);
    group.tick((1, 0, 0), false, false);
    assert_eq!(
        at_twenty,
        vec![
            Dummy {
                place: false,
                dy: 1
            },
            Dummy {
                place: false,
                dy: 2
            }
        ]
    );

    for _ in 21..100 {
        group.tick((0, 0, 0), false, false);
        group.tick((1, 0, 0), false, false);
    }
    assert_eq!(
        group.doors[&(0, 0, 0)].state,
        BlastState::Moving,
        "still moving at 99 ticks"
    );
    group.tick((0, 0, 0), false, false);
    group.tick((1, 0, 0), false, false);
    assert_eq!(group.doors[&(0, 0, 0)].state, BlastState::Open);
    assert_eq!(group.doors[&(1, 0, 0)].state, BlastState::Open);
}

#[test]
fn blast_group_closing_places_from_the_top_down() {
    let mut group = BlastDoorGroup::new();
    group.add((0, 0, 0));
    group.doors.get_mut(&(0, 0, 0)).unwrap().state = BlastState::Open;
    group.toggle((0, 0, 0));
    let mut seen = Vec::new();
    for _ in 0..20 {
        seen.extend(group.tick((0, 0, 0), false, false));
    }
    assert_eq!(seen, vec![Dummy { place: true, dy: 5 }]);
}

#[test]
fn blast_door_locked_ignores_the_base_sensor_but_not_the_top_one() {
    let mut group = BlastDoorGroup::new();
    group.add((0, 0, 0));
    group.apply_padlock((0, 0, 0), 3, 0.1);
    group.tick((0, 0, 0), true, false);
    assert_eq!(group.doors[&(0, 0, 0)].state, BlastState::Closed);
    group.tick((0, 0, 0), false, true);
    assert_eq!(group.doors[&(0, 0, 0)].state, BlastState::Moving);
}

#[test]
fn blast_lock_gives_neighbours_the_old_modifier() {
    let mut group = BlastDoorGroup::new();
    group.add((0, 0, 0));
    group.add((1, 0, 0));
    assert!(group.apply_padlock((0, 0, 0), 9, 0.02));
    let a = &group.doors[&(0, 0, 0)].lock;
    let b = &group.doors[&(1, 0, 0)].lock;
    assert_eq!((a.pins, a.locked, a.modifier), (9, true, 0.02));
    assert_eq!((b.pins, b.locked), (9, true));
    assert_eq!(
        b.modifier, 0.1,
        "Java sets the modifier after lock(), so the neighbour keeps the old one"
    );
}

#[test]
fn blast_neighbours_with_another_lock_stay_shut() {
    let mut group = BlastDoorGroup::new();
    group.add((0, 0, 0));
    group.add((1, 0, 0));
    group.doors.get_mut(&(1, 0, 0)).unwrap().lock = Lock {
        pins: 5,
        locked: true,
        ..Lock::default()
    };
    group.toggle((0, 0, 0));
    assert_eq!(group.doors[&(0, 0, 0)].state, BlastState::Moving);
    assert_eq!(group.doors[&(1, 0, 0)].state, BlastState::Closed);
}

#[test]
fn blast_click_with_a_lock_tool_is_left_to_the_item() {
    let mut group = BlastDoorGroup::new();
    group.add((0, 0, 0));
    assert!(!group.click((0, 0, 0), Holding::Nothing, true, false));
    assert_eq!(group.doors[&(0, 0, 0)].state, BlastState::Closed);
    assert!(group.click((0, 0, 0), Holding::Nothing, false, false));
    assert_eq!(group.doors[&(0, 0, 0)].state, BlastState::Moving);
}

#[test]
fn seal_frame_is_a_ring_around_the_square() {
    let controller = (0, 0, 0);
    // Facing meta 0: the square sits at x+1 .. x+2, so the ring is x 0..=2, z -1..=1.
    let mut ring: BTreeSet<Cell> = BTreeSet::new();
    for x in 0..=2 {
        ring.insert((x, 0, -1));
        ring.insert((x, 0, 1));
    }
    for z in -1..=1 {
        ring.insert((0, 0, z));
        ring.insert((2, 0, z));
    }
    assert_eq!(frame_size(controller, 0, |c| ring.contains(&c)), 1);
    ring.remove(&(2, 0, 1));
    assert_eq!(
        frame_size(controller, 0, |c| ring.contains(&c)),
        0,
        "a missing corner closes nothing"
    );
}

#[test]
fn seal_square_and_redstone() {
    let controller = (0, 0, 0);
    assert_eq!(seal_interior(controller, 0, 1), vec![(1, 0, 0)]);
    assert_eq!(
        seal_close_cells(controller, 0, 1, |_| true),
        vec![(1, 0, 0)]
    );
    assert!(seal_is_closed(controller, 0, 1, |c| c == (1, 0, 0)));
    assert!(!seal_is_closed(controller, 0, 1, |_| false));

    assert_eq!(
        seal_redstone(0, true),
        SealRedstone {
            meta: 4,
            toggle: true
        }
    );
    assert_eq!(
        seal_redstone(4, true),
        SealRedstone {
            meta: 4,
            toggle: false
        }
    );
    assert_eq!(
        seal_redstone(4, false),
        SealRedstone {
            meta: 0,
            toggle: false
        }
    );
    assert_eq!(
        seal_redstone(0, false),
        SealRedstone {
            meta: 0,
            toggle: false
        }
    );
}

#[test]
fn hatch_needs_its_controller_and_a_frame() {
    assert!(hatch_survives(true, 1));
    assert!(!hatch_survives(false, 1));
    assert!(!hatch_survives(true, 0));
}

#[test]
fn keyhole_orientation_by_side() {
    assert_eq!(keyhole_orientation(2), Some(1));
    assert_eq!(keyhole_orientation(5), Some(2));
    assert_eq!(keyhole_orientation(3), Some(3));
    assert_eq!(keyhole_orientation(4), Some(0));
    assert_eq!(keyhole_orientation(0), None);
    assert_eq!(keyhole_orientation(1), None);
}
