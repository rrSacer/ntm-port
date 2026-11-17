#!/usr/bin/env python3
"""Run through agent-run. Every mutant must compile and fail its named oracle; restore sources even on failure."""
import json
import os
import shutil
import signal
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
TRANSACTION = 'crates/ntm-core/src/server/transaction.rs'
STIRLING = 'crates/ntm-core/src/server/solar_server.rs'
SAWMILL = 'crates/ntm-core/src/server/sawmill_server.rs'
MUTANTS = {
    '1': [
        ('ExplosionAcknowledgedOnRefusal', TRANSACTION,
         'WorldEffect::Explosion{dim,at,power,fire,breaks}=>host.explode(dim,*at,*power,*fire,*breaks),',
         'WorldEffect::Explosion{dim,at,power,fire,breaks}=>{host.explode(dim,*at,*power,*fire,*breaks);true},',
         'a_refused_generic_explosion_keeps_the_cursor_and_blocks_later_items'),
        ('ExplosionFlagsLost', TRANSACTION,
         'host.explode(dim,*at,*power,*fire,*breaks)',
         'host.explode(dim,*at,15.,true,true)',
         'a_refused_generic_explosion_keeps_the_cursor_and_blocks_later_items'),
        ('ExplosionRepeatedOnItemRetry', TRANSACTION,
         'let effect=batch.effects.pop_front().unwrap();',
         'let effect=if matches!(batch.effects.front(),Some(WorldEffect::Explosion{..})){batch.effects.front().unwrap().clone()}else{batch.effects.pop_front().unwrap()};',
         'an_accepted_generic_explosion_is_not_repeated_when_its_item_waits'),
    ],
    '2': [
        ('HostWithoutLoadedMarker', 'crates/ntm-core/tests/common/world_effects_order_model.rs',
         '            self.state.destination.seen = true;', '',
         'saved_outputs_survive_an_older_homes_explosion_replay'),
        ('LoadedHomeNotTracked', 'crates/ntm-core/src/server/obligations.rs',
         'self.homes.entry(home.clone()).or_insert(0);', '',
         'a_reopened_home_is_written_empty_when_its_last_batch_is_paid'),
        ('ReplayWithoutOrderGuard', TRANSACTION,
         'if later_seen {', 'if later_seen && false {',
         'replay_skips_an_old_explosion_when_a_later_destination_proves_the_order'),
    ],
    '3': [
        ('PublishBeforeCommit', TRANSACTION,
         '                accepted = staged.valid && staged.host.commit_machines(&commits);',
         '                for effect in &staged.critical { if let WorldEffect::Explosion{dim,at,power,fire,breaks}=effect { staged.host.explode(dim,*at,*power,*fire,*breaks); } }\n                accepted = staged.valid && staged.host.commit_machines(&commits);',
         'a_common_step_publishes_its_model_and_critical_effects_only_after_acceptance'),
        ('CommitWithoutBatch', TRANSACTION,
         '                    if !staged.critical.is_empty() {',
         '                    if false && !staged.critical.is_empty() {',
         'a_common_step_publishes_its_model_and_critical_effects_only_after_acceptance'),
        ('PostPhaseWithoutCommit', 'crates/ntm-core/src/server/machine_effects.rs',
         '        if !host.commit_machine(&commit) {\n            return false;\n        }',
         '        host.commit_machine(&commit);',
         'a_post_step_phase_refused_at_its_first_commit_publishes_neither_state_nor_obligation'),
        ('PublishNextTick', 'crates/ntm-core/src/server/machine_effects.rs',
         '            self.ready.world.machine_effects.push_back(batch);',
         '            DELAYED.with(|pending| pending.borrow_mut().push(batch));',
         'a_save_between_this_tick_and_the_next_keeps_the_post_phase_batch_with_its_model'),
    ],
    '5': [
        ('StirlingFailsInPlace', STIRLING,
         '                if s.fails_on_finish() {', '                if false && s.fails_on_finish() {',
         'a_failing_stirling_commits_its_lost_gear_with_exactly_one_blast'),
        ('StirlingPublishBeforeCommit', STIRLING,
         '        for key in failing {\n',
         '        for key in failing {\n            host.explode(&key.0, [f64::from(key.1[0]) + 0.5, f64::from(key.1[1]) + 1., f64::from(key.1[2]) + 0.5], 5., false, false);\n',
         'a_refused_stirling_failure_publishes_nothing_and_fails_on_the_next_hot_tick'),
        ('StirlingRefusalLosesGear', STIRLING,
         '                s.heat = 0;\n', '                s.finish();\n',
         'a_refused_stirling_failure_publishes_nothing_and_fails_on_the_next_hot_tick'),
        ('StirlingPublishNextTick', 'crates/ntm-core/src/server/machine_effects.rs',
         '            self.ready.world.machine_effects.push_back(batch);',
         '            DELAYED.with(|pending| pending.borrow_mut().push(batch));',
         'a_save_right_after_the_stirling_failure_keeps_the_blast_with_the_lost_gear'),
    ],
    'd3': [
        ('SawmillPlacementIgnored', 'crates/ntm-core/src/server/transaction.rs',
         '                    self.sawmill_word(host,&k,generation);\n', '',
         'a_sawmill_placed_from_its_item_without_blade_has_none_and_drops_that_variant'),
        ('SawmillWordForAnyPlacement', 'crates/ntm-core/src/server/sawmill_server.rs',
         '        if self.ready.sawmills_without_blade.get(key) != Some(&Some(generation)) {',
         '        if !self.ready.sawmills_without_blade.contains_key(key) {',
         'a_word_is_taken_by_one_placement_only'),
        ('SawmillPendingNotCounted', 'crates/ntm-core/src/server/sawmill_server.rs',
         '        self.ready.sawmills_without_blade.contains_key(&key)\n            || matches!',
         '        false\n            || matches!',
         'a_sawmill_placed_from_its_item_without_blade_has_none_and_drops_that_variant'),
    ],
    # The independent review of the sawmill (P1-1, P2-1 and its P3s).
    'review': [
        ('LateWordLost', SAWMILL,
         '        if self.ready.sawmills_placed_now.contains(&key) {', '        if false && self.ready.sawmills_placed_now.contains(&key) {',
         'a_word_given_after_its_placed_event_still_takes_the_blade'),
        ('SpentWordKept', 'crates/ntm-core/src/server/native_server.rs',
         '        self.ready.sawmills_without_blade.retain(|k,g|g.is_some_and(|g|waiting.contains(&(k.clone(),g))));', '',
         'a_word_whose_placement_was_undone_is_spent'),
        ('CopyLateOnFailure', SAWMILL,
         '            } else if published && fails {\n                host.sawmill_blade(&key.0, key.1, true);\n', '            } else if published && fails {\n',
         'a_failing_sawmill_commits_its_lost_blade_with_the_blast_then_the_blade'),
        ('CopyLateOnRepair', SAWMILL,
         '            host.sawmill_blade(dim, core, false);\n', '',
         'a_blade_in_hand_goes_back_into_a_sawmill_without_one_in_one_transfer'),
        ('GivePilesOntoNamed', 'crates/ntm-core/src/server.rs',
         '                let same = same.filter(|&i| self.inventory[i].as_ref().is_none_or(|h| h.plain));\n', '',
         'the_outputs_never_pile_onto_a_stack_with_other_components'),
        ('SpinReadsSpentHeat', 'crates/ntm-core/src/sawmill.rs',
         '        u8::from(self.spinning).min(max)', '        u8::from(self.has_blade && self.heat >= 100).min(max)',
         'heat_saws_an_input_in_600_progress_and_spends_it'),
    ],
    'saw': [
        ('SawmillFailsInPlace', SAWMILL,
         '                if s.fails_on_finish() {', '                if false && s.fails_on_finish() {',
         'a_failing_sawmill_commits_its_lost_blade_with_the_blast_then_the_blade'),
        ('BladeBeforeBlast', SAWMILL,
         '                vec![\n                    MachineEffect::Explode { offset: [0., 1., 0.], power: 5., fire: false, breaks: false },\n                    MachineEffect::Drop { offset: [0, 1, 1], item: crate::sawmill::BLADE.into(), count: 1 },\n                ]',
         '                vec![\n                    MachineEffect::Drop { offset: [0, 1, 1], item: crate::sawmill::BLADE.into(), count: 1 },\n                    MachineEffect::Explode { offset: [0., 1., 0.], power: 5., fire: false, breaks: false },\n                ]',
         'a_failing_sawmill_commits_its_lost_blade_with_the_blast_then_the_blade'),
        ('SawmillRefusalLosesBlade', SAWMILL,
         '                s.heat = 0;\n', '                s.finish();\n',
         'a_refused_sawmill_failure_publishes_nothing_and_breaks_on_the_next_hot_tick'),
        ('SawmillPublishNextTick', 'crates/ntm-core/src/server/machine_effects.rs',
         '            self.ready.world.machine_effects.push_back(batch);',
         '            DELAYED.with(|pending| pending.borrow_mut().push(batch));',
         'a_save_right_after_the_sawmill_failure_keeps_the_blast_and_the_blade_with_the_lost_blade'),
    ],
    'hand': [
        ('RepairWithoutDebit', SAWMILL,
         '            next.has_blade = true;\n            edits.extend(from.and_then(|f| hand.take(f, 1)));',
         '            next.has_blade = true;',
         'a_blade_in_hand_goes_back_into_a_sawmill_without_one_in_one_transfer'),
        ('CollectThrowsTheWholeStack', SAWMILL,
         '            drops = left.into_iter().map(|s| (s, TransferDrop::Thrown)).collect();',
         '            drops = out.iter().cloned().chain(left).map(|s| (s, TransferDrop::Thrown)).collect();',
         'the_outputs_go_to_the_player_and_what_does_not_fit_is_thrown_once'),
        ('InsertKeepsTheHand', SAWMILL,
         '            edits.extend(hand.take(f, 1));\n            copies.push',
         '            copies.push',
         'one_of_the_stack_in_hand_goes_into_an_empty_input_only_when_it_has_an_output'),
    ],
    # The second review of the sawmill (P1 of step 3c, the residual P2 of the words).
    'review2': [
        ('RestoredSlotsOverwritten', 'crates/ntm-core/src/server/container_bridge.rs',
         '                if let Some(n) = slotted(&mut m.data)\n                    && !n.slots_recorded()\n',
         '                if let Some(n) = slotted(&mut m.data)\n                    && false && !n.slots_recorded()\n',
         'a_sawmill_restored_from_the_journal_keeps_the_log_in_its_block_entity'),
    ],
    # Step 3c: the sawmill's slots are its block entity's, open to hoppers by the Java's rules.
    'hopper': [
        ('InsertIntoABusySawmill', 'crates/ntm-core/src/sawmill.rs',
         'slot == 0 && slots.iter().all(Option::is_none) && item.count == 1',
         'slot == 0 && item.count == 1',
         'hoppers_follow_the_javas_slot_rules'),
        ('InsertAnyCount', 'crates/ntm-core/src/sawmill.rs',
         ' && item.count == 1 && output(', ' && output(',
         'hoppers_follow_the_javas_slot_rules'),
        ('ExtractTheInput', 'crates/ntm-core/src/sawmill.rs',
         '        SlotAction::Extract => slot > 0,', '        SlotAction::Extract => true,',
         'hoppers_follow_the_javas_slot_rules'),
        ('NoSawmillPolicy', 'crates/ntm-core/src/native.rs',
         '    if block == crate::sawmill::BLOCK {\n        return crate::sawmill::allows(slots, slot, item, action);\n    }\n', '',
         'hoppers_follow_the_javas_slot_rules'),
        ('SawmillNotSlotted', 'crates/ntm-core/src/server/container_bridge.rs',
         '        Data::Sawmill(s) => Some(s),\n', '',
         'a_log_in_the_block_entity_saws_into_its_slots'),
        ('SlotsInTheRecord', 'crates/ntm-core/src/sawmill.rs',
         '#[serde(default = "empty_slots", skip_serializing)]', '#[serde(default = "empty_slots")]',
         'a_log_in_the_block_entity_saws_into_its_slots'),
        ('UseReadsTheModel', SAWMILL,
         '        next.slots = expected.clone();\n', '',
         'a_use_reads_the_block_entity_a_hopper_changed'),
        ('SawnOutsideTheCommit', SAWMILL,
         '                } else if s.saws_on_finish() {', '                } else if false && s.saws_on_finish() {',
         'a_log_in_the_block_entity_saws_into_its_slots'),
        ('InputWithoutItsData', SAWMILL,
         '            copies.push(SlotCopy { dim: dim.into(), pos: core, slot: 0, from: f });\n', '',
         'one_of_the_stack_in_hand_goes_into_an_empty_input_only_when_it_has_an_output'),
    ],
}


def main():
    backup_root = pathlib.Path(os.environ['CARGO_TARGET_DIR']) / 'world-effects-mutant-backups'
    if sys.argv[1] == '--restore':
        for backup in backup_root.rglob('*'):
            if backup.is_file():
                (ROOT / backup.relative_to(backup_root)).write_bytes(backup.read_bytes())
        shutil.rmtree(backup_root, ignore_errors=True)
        print('Interrupted mutant sources restored; discard the interrupted result.', flush=True)
        return
    if backup_root.exists():
        raise RuntimeError('Interrupted mutant: inspect changes, run this script --restore, then rerun the slice.')
    def interrupted(signum, _frame):
        raise RuntimeError(f'Interrupted by signal {signum}; discard this run')
    signal.signal(signal.SIGTERM, interrupted)
    results = []
    # Every anchor first: a stale one stops the slice before any source is touched.
    for name, relative, before, _after, _oracle in MUTANTS[sys.argv[1]]:
        assert (ROOT / relative).read_text().count(before) == 1, (name, 'stale anchor')
    for name, relative, before, after, oracle in MUTANTS[sys.argv[1]]:
        path = ROOT / relative
        original = path.read_text()
        assert original.count(before) == 1, (name, original.count(before))
        # Repeat only once: a timeout or compile error is never accepted as a kill.
        if name == 'ExplosionRepeatedOnItemRetry':
            after = 'let effect=if batch.done==0 && matches!(batch.effects.front(),Some(WorldEffect::Explosion{..})){batch.effects.front().unwrap().clone()}else{batch.effects.pop_front().unwrap()};'
        backup = backup_root / relative
        backup.parent.mkdir(parents=True, exist_ok=True)
        backup.write_text(original)
        extra_originals = {}
        try:
            changed = original.replace(before, after)
            if name.endswith('PublishNextTick'):
                # A real delayed queue, drained only at the beginning of the next native tick.
                changed += '\nstd::thread_local! { pub(super) static DELAYED: std::cell::RefCell<Vec<CommittedEffects>> = const { std::cell::RefCell::new(Vec::new()) }; }\n'
                native = ROOT / 'crates/ntm-core/src/server/native_server.rs'
                extra_originals[native] = native.read_text()
                native_backup = backup_root / native.relative_to(ROOT)
                native_backup.parent.mkdir(parents=True, exist_ok=True)
                native_backup.write_text(extra_originals[native])
                needle = '    pub fn native_tick(&mut self, host: &mut impl ServerHost, tick: u64) {'
                assert extra_originals[native].count(needle) == 1
                native.write_text(extra_originals[native].replace(needle, needle + '\n        super::machine_effects::DELAYED.with(|pending| self.ready.world.machine_effects.extend(pending.borrow_mut().drain(..)));'))
            path.write_text(changed)
            command = ['cargo', 'test', '--release', '--test', 'save_model', oracle, '--', '--nocapture'] if name == 'HostWithoutLoadedMarker' else ['cargo', 'test', '--lib', oracle, '--', '--nocapture']
            run = subprocess.run(command, cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=300)
            killed = run.returncode == 101 and 'test result: FAILED.' in run.stdout and 'could not compile' not in run.stdout
            results.append({'mutant': name, 'oracle': oracle, 'killed': killed, 'exit': run.returncode})
            if killed:
                results[-1]['failure'] = run.stdout[-1800:]
            print(json.dumps(results[-1]), flush=True)
            if killed and name == 'ReplayWithoutOrderGuard':
                model = subprocess.run(['cargo', 'test', '--release', '--test', 'save_model', 'saved_outputs_survive_an_older_homes_explosion_replay', '--', '--nocapture'], cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=300)
                results[-1]['model_killed'] = model.returncode == 101 and 'test result: FAILED.' in model.stdout and 'could not compile' not in model.stdout
                results[-1]['model_failure'] = model.stdout[-1800:]
                print(json.dumps(results[-1]), flush=True)
                if not results[-1]['model_killed']:
                    print(model.stdout, flush=True)
                    raise RuntimeError('replay mutant survived the production-backed save verifier')
            if not killed:
                print(run.stdout, flush=True)
                raise RuntimeError(f'{name} not killed by a test assertion')
        finally:
            path.write_text(original)
            for extra, previous in extra_originals.items():
                extra.write_text(previous)
            shutil.rmtree(backup_root)
    print(json.dumps({'slice': sys.argv[1], 'results': results}), flush=True)


if __name__ == '__main__':
    main()
