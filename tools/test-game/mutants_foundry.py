#!/usr/bin/env python3
"""Manual mutation runs for the foundry front: each mutant edits one source line, runs the tests that must kill it
and restores the file (also on error). Usage: mutants_foundry.py LOGFILE; run from the repository root through agent-run
(QUEUE=cargo, CARGO_TARGET_DIR outside the tree). The result of every mutant is printed and kept in LOGFILE."""
import os
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..'))
os.chdir(ROOT)
SC, RF, FL = 'crates/ntm-core/src/strand_caster.rs', 'crates/ntm-core/src/rotary_furnace.rs', 'crates/ntm-core/src/foundry/flow.rs'
# (name, file, old, new, test filter)
MUTANTS = [
    ('F1 the face of the jet is not checked', SC, 'face == [0, 1, 0] && pour_cells', 'pour_cells', 'strand_caster'),
    ('F1 the resolution misses a column of the funnel', 'crates/ntm-core/src/server/foundry_server.rs', 'for dz in -1..=1 {', 'for dz in 0..=0 {', 'a_jet_on_a_funnel_cell'),
    ('F4 the limit is ten molds', SC, 'm.cost * 9)', 'm.cost * 10)', 'strand_caster'),
    ('F4 a partial batch goes after 100 ticks', SC, 'IDLE_FLUSH: u64 = 200', 'IDLE_FLUSH: u64 = 100', 'strand_caster'),
    ('F4 an overflow keeps the old progress', SC, '        self.last_progress = self.now;\n        Some(q - required)', '        Some(q - required)', 'strand_caster'),
    ('F4 a cast uses no water', SC, 'self.water.mb = self.water.mb.saturating_sub(used);', '', 'strand_caster'),
    ('F5 the excess is lost (no effect)', SC, 'self.pending.push(MachineEffect::DropStack', 'let _ = (MachineEffect::DropStack', 'strand_caster'),
    ('F5 the excess is rounded to ingots', SC, 'scrap::stack(mat, self.melt.amount - cap).custom', 'scrap::stack(mat, (self.melt.amount - cap) / 72 * 72).custom', 'strand_caster'),
    ('F5 the effects are not drained', SC, 'std::mem::take(&mut self.pending)', 'self.pending.clone()', 'strand_caster'),
    ('P1-1 the caster claims its record keeps the slots', SC, '    fn slots_recorded(&self) -> bool {\n        false', '    fn slots_recorded(&self) -> bool {\n        true', 'restored_from_the_journal'),
    ('D4 the pool is rounded to ingots on break', FL, 'vec![(m.clone(), self.amount)]', 'vec![(m.clone(), self.amount / 72 * 72)]', 'a_broken'),
    ('D4 the crucible ignores a scrap\'s amount', 'crates/ntm-core/src/foundry/mod.rs', 'Some((mat, q)) => self.converted(&mat, q).into_iter().collect(),', 'Some((mat, _)) => self.converted(&mat, 72).into_iter().collect(),', 'a_scrap_melts'),
    ('R8 the rotary furnace\'s molten output becomes a scrap', RF, 'impl Logic for RotaryFurnace {\n    fn tick(&mut self) -> bool {', 'impl Logic for RotaryFurnace {\n    fn scraps(&self) -> Vec<(String, u32)> { self.output.iter().cloned().collect() }\n    fn tick(&mut self) -> bool {', 'r8'),
    ('rotary steamUseMult uses 5 instead of 10', RF, 'let steam_use_mult = 10.0 * process_speed.log10() + 1.0;', 'let steam_use_mult = 5.0 * process_speed.log10() + 1.0;', 'rotary'),
    ('rotary canProcess ignores the steam already used', RF, '        if self.steam_used > 100 {\n            return false;\n        }\n', '', 'rotary'),
    ('rotary canProcess ignores the room for the spent steam', RF, 'if (self.spent_steam.free() as f32) < need_steam / 100.0 {', 'if false {', 'rotary'),
    ('rotary consumes one of an ingredient, not its count', RF, 's.count -= g.n;', 's.count -= 1;', 'rotary'),
    ('rotary returns a spent steam per 50 used, not 100', RF, 'let steam_return = self.steam_used / 100;', 'let steam_return = self.steam_used / 50;', 'rotary'),
    ('rotary burns a fuel for its whole time, not half', RF, 'burn_time(&s.id) / 2;', 'burn_time(&s.id);', 'rotary'),
    ('rotary pours 36 quanta a tick', RF, 'pub const POUR: u32 = 72;', 'pub const POUR: u32 = 36;', 'rotary'),
    ('rotary spout one block closer', 'crates/ntm-core/src/server/foundry_server.rs', 'rot[0] * 3, key.1[1] + 1, key.1[2] + rot[2] * 3]', 'rot[0] * 2, key.1[1] + 1, key.1[2] + rot[2] * 2]', 'rotary'),
    ('rotary progress ignores the process speed', RF, 'self.progress += process_speed / r.duration.max(1) as f32;', 'self.progress += 1.0 / r.duration.max(1) as f32;', 'rotary'),
]


def main(log):
    env = dict(os.environ, QUEUE='cargo', SKIP_UPSTREAM_CHECK='1')
    env.setdefault('CARGO_BUILD_JOBS', '6')
    out = open(log, 'w')
    killed = 0
    for name, path, old, new, flt in MUTANTS:
        src = open(path).read()
        if old not in src:
            line = f'NOT-APPLIED  {name}'
        else:
            open(path, 'w').write(src.replace(old, new, 1))
            try:
                p = subprocess.run(['/usr/local/bin/agent-run', 'cargo', 'test', '-p', 'ntm-core', '--lib', flt],
                                   capture_output=True, text=True, env=env, timeout=1800)
                text = p.stdout + p.stderr
                if 'could not compile' in text:
                    line = f'COMPILE-ERROR {name}'
                elif ' FAILED' in text:
                    killed += 1
                    failing = sorted({l.split()[1] for l in text.splitlines() if l.startswith('test ') and l.endswith('FAILED')})
                    line = f'KILLED       {name}  <- {", ".join(f.rsplit("::", 1)[-1] for f in failing)[:150]}'
                else:
                    line = f'SURVIVED     {name}'
            finally:
                open(path, 'w').write(src)
        print(line, flush=True)
        out.write(line + '\n')
        out.flush()
    summary = f'{killed} of {len(MUTANTS)} killed'
    print(summary)
    out.write(summary + '\n')


if __name__ == '__main__':
    main(sys.argv[1])
