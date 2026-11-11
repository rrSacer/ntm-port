#!/usr/bin/env python3
"""Run through agent-run. Every mutant must compile and fail its named oracle; restore sources even on failure."""
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
TRANSACTION = 'crates/ntm-core/src/server/transaction.rs'
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
}


def main():
    results = []
    for name, relative, before, after, oracle in MUTANTS[sys.argv[1]]:
        path = ROOT / relative
        original = path.read_text()
        assert original.count(before) == 1, (name, original.count(before))
        # The repeat mutant deliberately loops; timeout is NOT accepted as a kill.
        if name == 'ExplosionRepeatedOnItemRetry':
            after = 'let effect=if batch.done==0 && matches!(batch.effects.front(),Some(WorldEffect::Explosion{..})){batch.effects.front().unwrap().clone()}else{batch.effects.pop_front().unwrap()};'
        try:
            path.write_text(original.replace(before, after))
            run = subprocess.run(['cargo', 'test', '--lib', oracle, '--', '--nocapture'], cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=300)
            killed = run.returncode == 101 and 'test result: FAILED.' in run.stdout and 'could not compile' not in run.stdout
            results.append({'mutant': name, 'oracle': oracle, 'killed': killed, 'exit': run.returncode})
            print(json.dumps(results[-1]), flush=True)
            if not killed:
                print(run.stdout, flush=True)
                raise RuntimeError(f'{name} not killed by a test assertion')
        finally:
            path.write_text(original)
    print(json.dumps({'slice': sys.argv[1], 'results': results}), flush=True)


if __name__ == '__main__':
    main()
