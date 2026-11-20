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
    ('port: the fluid ducts reach a port from any side', 'crates/ntm-core/src/server/fluid_server.rs', 'faces.insert(p, vec![vec![d]]);', 'let _ = (p, d);', 'reaches_a'),
    ('port: a caster port faces the wrong way', 'crates/ntm-core/src/server/container_bridge.rs', '(cell(1, rot, 1, back), rot),', '(cell(1, rot, 1, back), anti),', 'directed_ports'),
    ('port: the rotary steam ports face the front', 'crates/ntm-core/src/server/container_bridge.rs', '(cell(2, anti, 1, back), back),', '(cell(2, anti, 1, back), d),', 'directed_ports'),
    ('stack: every output stacks to 64', SC, '    .copied()\n    .unwrap_or(64)', '    .copied()\n    .map(|_| 64)\n    .unwrap_or(64)', 'an_output_that'),
    ('stack: the cast deposits up to 64', SC, 'remaining.min(stack_limit(&out_id).saturating_sub(s.count))', 'remaining.min(64u32.saturating_sub(s.count))', 'an_output_that'),
    ('rotary claims its record keeps the slots', RF, '    fn slots_recorded(&self) -> bool {\n        false', '    fn slots_recorded(&self) -> bool {\n        true', 'say_their_records'),
    ('hand: a shovel takes only the whole ingots of the pool', 'crates/ntm-core/src/server/foundry_hand.rs', 'scrap::stack(m, melt.amount)', 'scrap::stack(m, melt.amount / 72 * 72)', 'a_shovel_takes'),
    ('hand: a shovel leaves the pool', 'crates/ntm-core/src/server/foundry_hand.rs', '    *melt = Default::default();\n', '', 'a_shovel_takes'),
    ('hand: a mold goes in even into a taken slot', 'crates/ntm-core/src/server/foundry_hand.rs', 'if mold && c.slots[slot::MOLD].is_none() {', 'if mold {', 'a_mold_in_hand'),
    ('hand: the whole stack of molds goes in, not one', 'crates/ntm-core/src/server/foundry_hand.rs', 'Some(Stack { count: 1, ..h.stack.clone() })', 'Some(h.stack.clone())', 'a_mold_in_hand'),
    ('hand: a shovel with an empty pool opens the screen', 'crates/ntm-core/src/server/foundry_hand.rs', 'taken = false;\n                    if shovel {\n                        return true;', 'taken = false;\n                    if shovel {\n                        return false;', 'a_shovel_takes'),
    ('hand: the scrap that does not fit is lost', 'crates/ntm-core/src/server/foundry_hand.rs', 'u.drops.extend(left.into_iter().map(|s| (s, TransferDrop::Thrown)));', 'let _ = left;', 'does_not_fit'),
    ('hand: any item ending in _shovel is a shovel', 'crates/ntm-core/src/server/foundry_hand.rs', '    .contains(item)\n        || item.strip_prefix("minecraft:")', '    .contains(item)\n        || item.ends_with("_shovel")\n        || item.strip_prefix("minecraft:")', 'the_shovels_come'),
    ('hand: the off hand acts too', 'crates/ntm-core/src/server/foundry_hand.rs', 'if matches!(hand.hand, Some(PlayerSlot::Offhand)) {\n            return false;\n        }', '', 'only_the_main_hand'),
    ('hand: a mold of any size goes into a mold block', 'crates/ntm-core/src/server/foundry_hand.rs', 'mo.size == c.size', 'true', 'a_basin_takes'),
    ('hand: the cast stays in the mold block', 'crates/ntm-core/src/server/foundry_hand.rs', 'if let Some(out) = c.slots[crate::foundry::cast::OUT].take() {', 'if let Some(out) = None::<Stack> {', 'a_mold_blocks_click'),
    ('hand: the crucible takes only the waste', 'crates/ntm-core/src/server/foundry_hand.rs', 'scrap::stacks(c.recipe_stack.iter().chain(&c.waste_stack))', 'scrap::stacks(c.waste_stack.iter())', 'each_stack_of_a_crucible'),
    ('hand: the crucible keeps its stacks', 'crates/ntm-core/src/server/foundry_hand.rs', '                c.waste_stack.clear();\n', '', 'each_stack_of_a_crucible'),
    ('hand: an empty channel opens the screen with a shovel', 'crates/ntm-core/src/server/foundry_hand.rs', '                if c.melt.amount == 0 {\n                    return true;\n                }\n                give(&mut u, scoop(&mut c.melt));\n            }\n            Data::FoundryTank', '                if c.melt.amount == 0 {\n                    return false;\n                }\n                give(&mut u, scoop(&mut c.melt));\n            }\n            Data::FoundryTank', 'a_shovel_takes_a_channels'),
    ('hand: the crucible gives each scrap in a call of its own (the same free slot)', 'crates/ntm-core/src/server/foundry_hand.rs', 'let (given, left) = hand.give(&stacks, max);\n            u.edits.extend(given);\n            u.drops.extend(left.into_iter().map(|s| (s, TransferDrop::Thrown)));', 'for s in &stacks {\n                let (given, left) = hand.give(std::slice::from_ref(s), max);\n                u.edits.extend(given);\n                u.drops.extend(left.into_iter().map(|s| (s, TransferDrop::Thrown)));\n            }', 'each_stack_of_a_crucible'),
    ('slag: it takes any material', 'crates/ntm-core/src/foundry/slag.rs', 'if self.melt.mat.as_deref() == Some(mat) { q.min(', 'if true { q.min(', 'slag'),
    ('slag: it takes more than its capacity', 'crates/ntm-core/src/foundry/slag.rs', 'q.min(SLAG_CAP.saturating_sub(self.melt.amount))', 'q', 'slag'),
    ('slag D4: a broken slag gives whole ingots only', 'crates/ntm-core/src/foundry/slag.rs', '    fn scraps(&self) -> Vec<(String, u32)> {\n        self.melt.molten()', '    fn scraps(&self) -> Vec<(String, u32)> {\n        self.melt.molten().into_iter().map(|(m, q)| (m, q / 72 * 72)).collect()', 'slag'),
    ('slag: the source is not reduced (the metal twice)', 'crates/ntm-core/src/server/slag_server.rs', '        melt.amount = left;\n        if left == 0 {', '        melt.amount = q;\n        if left == 0 {', 'slag'),
    ('slag: the source is emptied whatever was poured (the rest lost)', 'crates/ntm-core/src/server/slag_server.rs', '        melt.amount = left;\n        if left == 0 {', '        melt.amount = 0;\n        if left == 0 {', 'slag'),
    ('slag: a refused commit is poured in memory anyway', 'crates/ntm-core/src/server/slag_server.rs', 'if !host.commit_machines(&commits) {\n            return;', 'if !host.commit_machines(&commits) {\n            self.ready.world.machines.insert(src.clone(), source.clone());', 'slag'),
    ('slag: no block above for what the hit cannot take', 'crates/ntm-core/src/server/slag_server.rs', 'if left > 0 && same_chunk(&above) && !r.world.machines.contains_key(&above) {', 'if false {', 'slag'),
    ('slag: any block above is air', 'crates/ntm-core/src/server/slag_server.rs', 'matches!(blockstate::id_of(state), "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")', 'true', 'slag'),
    ('slag: the new block is not part of the commit', 'crates/ntm-core/src/server/slag_server.rs', 'vec![MachineCommit { cells, guard:', 'vec![MachineCommit { cells: Vec::new(), guard:', 'slag'),
    ('slag: the slag filled is not in the commit', 'crates/ntm-core/src/server/slag_server.rs', '        if let Some((key, mut after)) = into {', '        if let Some((key, mut after)) = into.filter(|_| false) {', 'slag'),
    ('slag: a pour leaves the source\'s chunk (new block)', 'crates/ntm-core/src/server/slag_server.rs', 'if left > 0 && same_chunk(&above) &&', 'if left > 0 &&', 'slag'),
    ('slag: a pour leaves the source\'s chunk (slag filled)', 'crates/ntm-core/src/server/slag_server.rs', 'if same_chunk(&hit.cell)\n            && let Some', 'if true\n            && let Some', 'slag'),
    ('slag: the ray stops at cave air', 'crates/ntm-core/src/server/foundry_server.rs', '!matches!(blockstate::id_of(&b), "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")', '!matches!(blockstate::id_of(&b), "minecraft:air" | "minecraft:void_air")', 'the_ray_passes'),
    ('own: every cell of a commit is the server\'s own (a mined cable stays a ghost)', 'crates/ntm-core/src/server.rs', '        .filter(|c| c.initial.is_some())\n', '', 'only_the_cells_born'),
    ('flow: a slag over air does not fall', 'crates/ntm-core/src/server/slag_server.rs', 'if let Some(snapshot) = air(host, &below) {', 'if let Some(snapshot) = air(host, &below).filter(|_| false) {', 'slag'),
    ('flow: a falling slag leaves half behind', 'crates/ntm-core/src/server/slag_server.rs', 'self.born_slag(&below, snapshot, &mat, amount)', 'self.born_slag(&below, snapshot, &mat, amount / 2)', 'slag'),
    ('flow: the slag below takes more than its capacity', 'crates/ntm-core/src/server/slag_server.rs', 'let t = amount.min(SLAG_CAP - bs.melt.amount);', 'let t = amount;', 'slag'),
    ('flow: the slag below may be another material', 'crates/ntm-core/src/server/slag_server.rs', '&& bs.melt.mat.as_deref() == Some(&mat)\n', '\n', 'slag'),
    ('flow: the emptied upper is not scheduled to go', 'crates/ntm-core/src/server/slag_server.rs', '                if emptied {\n                    self.ready.slag_active.insert(key.clone());\n                }\n', '', 'slag'),
    ('flow: the threshold is a quarter, not a fifth', 'crates/ntm-core/src/server/slag_server.rs', 'if amount < SLAG_CAP / 5 ||', 'if amount < SLAG_CAP / 4 ||', 'slag'),
    ('flow: the spread ignores the count', 'crates/ntm-core/src/server/slag_server.rs', '(amount / (free.len() as u32 * 2)).max(1)', '(amount / 8).max(1)', 'slag'),
    ('flow: the spread crosses the chunk border', 'crates/ntm-core/src/server/slag_server.rs', '(same_chunk(k) && !r.world.machines.contains_key(k))', '(!r.world.machines.contains_key(k))', 'slag'),
    ('flow: a reopened empty slag is not scheduled', 'crates/ntm-core/src/server/native_server.rs', '            r.slag_active.insert(key.clone());\n        }\n        r.world.machines.insert(key.clone(), m);', '        }\n        r.world.machines.insert(key.clone(), m);', 'reopened_between'),
    ('fuel: the eternal coal has no container (it is used up)', 'crates/ntm-core/data/machines/foundry.json', '"container": "ntm:coal_eternal"\n}', '"container": null\n}', 'the_eternal_coal'),
    ('slag tap: it takes metal from any side', 'crates/ntm-core/src/server/foundry_server.rs', 'if !o.passes(mat) || !behind {\n            return None;\n        }\n        below(m, host, &key.0, [key.1[0], key.1[1] - 1, key.1[2]], SLAGTAP_FALL', 'if !o.passes(mat) {\n            return None;\n        }\n        below(m, host, &key.0, [key.1[0], key.1[1] - 1, key.1[2]], SLAGTAP_FALL', 'slag'),
    ('slag tap: it ignores its filter', 'crates/ntm-core/src/server/foundry_server.rs', 'if !o.passes(mat) || !behind {\n            return None;\n        }\n        below(m, host, &key.0, [key.1[0], key.1[1] - 1, key.1[2]], SLAGTAP_FALL', 'if !behind {\n            return None;\n        }\n        below(m, host, &key.0, [key.1[0], key.1[1] - 1, key.1[2]], SLAGTAP_FALL', 'slag'),
    ('slag tap: the ray reaches 16 blocks', 'crates/ntm-core/src/server/foundry_server.rs', 'pub(super) const SLAGTAP_FALL: i32 = 15;', 'pub(super) const SLAGTAP_FALL: i32 = 16;', 'slag'),
    ('slag tap: the ray reaches 14 blocks', 'crates/ntm-core/src/server/foundry_server.rs', 'pub(super) const SLAGTAP_FALL: i32 = 15;', 'pub(super) const SLAGTAP_FALL: i32 = 14;', 'slag'),
    ('redstone: the slag tap ignores it', 'crates/ntm-core/src/server/foundry_server.rs', 'if !o.passes(mat) || !behind || closed(o, host, key) {\n            return None;\n        }\n        below(', 'if !o.passes(mat) || !behind {\n            return None;\n        }\n        below(', 'redstone_closes'),
    ('redstone: the outlet ignores it', 'crates/ntm-core/src/server/foundry_server.rs', 'if !o.passes(mat) || !behind || closed(o, host, key) {\n            return None;\n        }\n        let target', 'if !o.passes(mat) || !behind {\n            return None;\n        }\n        let target', 'redstone_closes'),
    ('redstone: no reading is open', 'crates/ntm-core/src/server/foundry_server.rs', '.is_none_or(|s| o.closed(s))', '.is_some_and(|s| o.closed(s))', 'redstone_closes'),
    ('redstone: the inversion is ignored', FL, 'self.invert_redstone ^ (signal > 0)', 'signal > 0', 'redstone_closes'),
    ('redstone: the inversion is not kept', FL, 'i32::from(self.invert) | i32::from(self.invert_redstone) << 1', 'i32::from(self.invert)', 'redstone_closes'),
    ('rotary progress ignores the process speed', RF, 'self.progress += process_speed / r.duration.max(1) as f32;', 'self.progress += 1.0 / r.duration.max(1) as f32;', 'rotary'),
]


def main(log, only=()):
    env = dict(os.environ, QUEUE='cargo', SKIP_UPSTREAM_CHECK='1')
    env.setdefault('CARGO_BUILD_JOBS', '6')
    out = open(log, 'w')
    killed = 0
    for name, path, old, new, flt in MUTANTS:
        if only and not any(o in name for o in only):
            continue
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
    summary = f'{killed} of {len([m for m in MUTANTS if not only or any(o in m[0] for o in only)])} killed'
    print(summary)
    out.write(summary + '\n')


if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2:])
