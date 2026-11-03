// Compiled inside the engine's existing level tests; uses its real resource
// fixture and scheduled ticks, not a second implementation of block rules.
use super::*;

#[test]
fn committed_publication_matches_vanilla_flag_three_with_observer_and_redstone() {
    let f = fixture().expect("HORIZON_RESOURCES is required for NTM equivalence");
    for old_name in ["redstone_block", "stone", "chest"] {
        let mut vanilla = World::new(f);
        let mut committed = World::new(f);
        let at = BlockPos::new(0, 1, 0);
        let old = vanilla.id(old_name, &[]);
        let observer = vanilla.id("observer", &[("facing", "west"), ("powered", "false")]);
        let lamp = vanilla.id("redstone_lamp", &[("lit", "true")]);
        for w in [&mut vanilla, &mut committed] {
            w.put(at, old);
            w.put(at.offset(1, 0, 0), observer);
            w.put(at.offset(0, 0, 1), lamp);
            w.floor(0, 3, "stone");
        }
        let air = f.behaviours.air();
        assert!(vanilla.set(at, air));
        assert!(f.behaviours.set_block(
            &mut committed.level,
            &mut committed.ticks,
            at,
            air,
            flags::UPDATE_CLIENTS | flags::UPDATE_SKIP_ALL_SIDEEFFECTS
        ));
        f.behaviours.ntm_committed_updates(
            &mut committed.level,
            &mut committed.ticks,
            at,
            old,
            air,
        );
        assert_eq!(
            committed.level.blocks, vanilla.level.blocks,
            "after publication of {old_name}"
        );
        assert_eq!(
            committed.level.events, vanilla.level.events,
            "events for {old_name}"
        );
        assert!(
            committed
                .level
                .events
                .iter()
                .all(|e| !matches!(e, LevelEvent::DropResources { .. })),
            "no second loot generation"
        );
        let mut observed_pulse = false;
        for _ in 0..8 {
            vanilla.full_tick();
            committed.full_tick();
            assert_eq!(committed.level.blocks, vanilla.level.blocks);
            assert_eq!(committed.level.events, vanilla.level.events);
            if committed.name(at.offset(1, 0, 0)).contains("powered=true") {
                observed_pulse = true;
            }
        }
        assert!(observed_pulse, "observer must react to mining {old_name}");
        assert!(
            committed.name(at.offset(0, 0, 1)).contains("lit=false"),
            "redstone neighbour must update"
        );
    }
}
