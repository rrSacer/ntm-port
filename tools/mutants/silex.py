#!/usr/bin/env python3
"""Hand-picked mutants of the SILEX and of the FEL's coupling to it: each must fail its tests.
Usage: tools/mutants/silex.py REPO TARGET LOG (docs/onda-2-sublote-2.md)"""
import subprocess, sys
from pathlib import Path
repo, target, log = Path(sys.argv[1]), sys.argv[2], Path(sys.argv[3])
src = repo / 'crates/ntm-core/src'
MUTANTS = [
    ('silex.rs', 'let comp = t.items.get(key).map_or(key, String::as_str);', 'let comp = key;'),
    ('silex.rs', 'let name = t.dicts.get(name).unwrap_or(name);', ''),
    ('silex.rs', 'Some(Recipe { produced: (whole.produced / 900) * 100, ..whole })', 'Some(whole)'),
    ('silex.rs', 'Some(Recipe { produced: (whole.produced / 900) * 100, ..whole })', 'Some(Recipe { produced: whole.produced / 9, ..whole })'),
    ('silex.rs', 'let loads = CONVERSION.contains(&self.tank.fluid.as_str()) || output(&direct).is_some();', 'let loads = CONVERSION.contains(&self.tank.fluid.as_str());'),
    ('silex.rs', 'let loads = CONVERSION.contains(&self.tank.fluid.as_str()) || output(&direct).is_some();', 'let loads = output(&direct).is_some();'),
    ('silex.rs', 'let n = 50.min((MAX_FILL - self.fill).min(self.tank.mb as i32));', 'let n = 100.min((MAX_FILL - self.fill).min(self.tank.mb as i32));'),
    ('silex.rs', 'if self.fill == 0 {\n                self.current = Some(direct.clone());', 'if self.fill >= 0 {\n                self.current = Some(direct.clone());'),
    ('silex.rs', 'if self.load_delay > 20 {', 'if self.load_delay > 19 {'),
    ('silex.rs', 'if self.load_delay != 0 || self.tank.fluid != PEROXIDE ||', 'if self.load_delay != 0 ||'),
    ('silex.rs', '|| self.current.as_ref().is_some_and(|c| *c != item) {', '{'),
    ('silex.rs', 'if load <= MAX_FILL - self.fill && load <= self.tank.mb as i32 {', 'if load <= MAX_FILL - self.fill {'),
    ('silex.rs', 'if load <= MAX_FILL - self.fill && load <= self.tank.mb as i32 {', 'if load <= self.tank.mb as i32 {'),
    ('silex.rs', 'if recipe.laser > self.mode as u8 ||', 'if recipe.laser >= self.mode as u8 ||'),
    ('silex.rs', '|| self.fill < recipe.consumed ||', '|| self.fill <= recipe.consumed ||'),
    ('silex.rs', '|| self.slots[OUTPUT].is_some() {', '{'),
    ('silex.rs', 'self.progress += 1 << (self.mode as u8 - recipe.laser);', 'self.progress += 1 + (self.mode as u8 - recipe.laser) as i32;'),
    ('silex.rs', 'if self.progress >= PROCESS_TIME {', 'if self.progress > PROCESS_TIME {'),
    ('silex.rs', 'self.recipe_index %= total.max(1);', ''),
    ('silex.rs', 'if self.recipe_index < weight {', 'if self.recipe_index <= weight {'),
    ('silex.rs', 'self.recipe_index += PRIME;', 'self.recipe_index += 1;'),
    ('silex.rs', 'self.fill -= recipe.consumed;', ''),
    ('silex.rs', '&& q.count < stack_limit(&q.id)', '&& q.count <= stack_limit(&q.id)'),
    ('silex.rs', '&& q.id == out.id', ''),
    ('silex.rs', 'if !self.process() {\n            self.progress = 0;\n        }', 'self.process();'),
    ('silex.rs', 'if self.fill <= 0 {\n            self.current = None;\n        }', ''),
    ('silex.rs', '        self.mode = Wavelength::Null;\n        // The load delay', '        // The load delay'),
    ('silex.rs', '        self.shown = self.mode;\n', ''),
    ('silex.rs', '            self.shown as i32,', '            self.mode as i32,'),
    ('silex.rs', 'crate::combustion::type_from_identifier(&mut self.tank, &self.slots, IDENTIFIER, |_| true);', ''),
    ('silex.rs', 'crate::combustion::load_tank(&mut self.tank, &mut self.slots, CONTAINER_IN, CONTAINER_OUT);', ''),
    ('silex.rs', '(true, true) => slot <= CONTAINER_IN,', '(true, true) => slot <= CONTAINER_OUT,'),
    ('silex.rs', '(true, false) => slot == INPUT && output(item).is_some(),', '(true, false) => slot == INPUT,'),
    ('silex.rs', '(false, false) => slot >= QUEUE.start,', '(false, false) => slot >= OUTPUT,'),
    ('silex.rs', 'let io: Vec<usize> = std::iter::once(INPUT).chain(QUEUE).collect();', 'let io: Vec<usize> = QUEUE.collect();'),
    # The state saved by the fill's id (review P1-1), its void button (P2-1) and the shift-click's order (P3).
    ('silex.rs', 'self.recipe_index = index.max(0);', 'self.recipe_index = 0;'),
    ('silex.rs', 'let mut v = vec![self.fill, STATE_V2, self.recipe_index, fluid_id(&self.tank.fluid), self.tank.mb as i32];', 'let mut v = vec![self.fill, STATE_V2, self.recipe_index, 0, self.tank.mb as i32];'),
    ('silex.rs', 'v.extend(pack(self.current.as_deref().unwrap_or("")));', 'v.extend(pack(""));'),
    ('silex.rs', '[fill, STATE_V2, index, fluid, mb, rest @ ..] => (*fill, unpack(rest), *index, *fluid, *mb),', '[fill, STATE_V2, index, fluid, mb, ..] => (*fill, None, *index, *fluid, *mb),'),
    ('silex.rs', 'String::from_utf8(bytes.get(..len)?.to_vec()).ok()', 'String::from_utf8(bytes).ok()'),
    ('silex.rs', 'b[..c.len()].copy_from_slice(c);', ''),
    ('silex.rs', 'self.current = current.filter(|c| !c.is_empty() && self.fill > 0);', 'self.current = current;'),
    ('silex.rs', '        self.progress = 0;\n        self.load_delay = 0;\n        let (fill', '        let (fill'),
    ('silex.rs', 'if id == 0 {\n            self.void();', 'if id == 1 {\n            self.void();'),
    ('silex.rs', '        self.fill = 0;\n        self.current = None;\n    }', '        self.current = None;\n    }'),
    ('native.rs', 'rules[crate::silex::CONTAINER_IN].quick_move = Some(crate::fluid::all_full_containers());', 'rules[crate::silex::CONTAINER_IN].quick_move = Some(Vec::new());'),
    ('native.rs', '            rules[i].quick_reverse = true;\n        }\n        return rules;', '        }\n        return rules;'),
    ('server/transaction.rs', 'let probe = [p[0] + dir[0], key.1[1], p[2] + dir[2]];', 'let probe = [p[0], key.1[1], p[2]];'),
    ('server/transaction.rs', 'let valid = facing.is_some_and(|f| f == dir || f == [-dir[0], -dir[1], -dir[2]]);', 'let valid = facing.is_some_and(|f| f == dir);'),
    ('server/transaction.rs', 'let valid = facing.is_some_and(|f| f == dir || f == [-dir[0], -dir[1], -dir[2]]);', 'let valid = true;'),
    ('server/transaction.rs', 'if let Some(c) = cores.get(&i) { lasers_set.push((c.clone(), mode)); }', ''),
    ('server/transaction.rs', 'drops.push((at(i), crate::silex::BLOCK));', ''),
    ('server/transaction.rs', '&& (p[2] - c.1[2]).abs() <= 1 &&', '&& (p[2] - c.1[2]).abs() <= 0 &&'),
    ('server/transaction.rs', 'let laser = self.ready.silex_lasers.remove(k).unwrap_or_default();', 'let laser = self.ready.silex_lasers.get(k).copied().unwrap_or_default();'),
    ('fel.rs', 'if valid_rotation && i >= 5 && !spacing {', 'if valid_rotation && i >= 4 && !spacing {'),
]
results = []
for file, old, new in MUTANTS:
    path = src / file
    original = path.read_text()
    if old not in original:
        results.append((file, old[:40], 'ANCHOR MISSING')); continue
    path.write_text(original.replace(old, new, 1))
    try:
        r = subprocess.run(['/usr/local/bin/agent-run', 'env', 'QUEUE=cargo', 'CARGO_BUILD_JOBS=4', f'CARGO_TARGET_DIR={target}', 'cargo', 'test', '-p', 'ntm-core', '--lib', '--', 'silex', 'fel', 'dose_tests'],
                           cwd=repo, capture_output=True, text=True, timeout=3000)
        verdict = 'KILLED' if r.returncode != 0 and 'test result: FAILED' in r.stdout else ('SURVIVED' if r.returncode == 0 else 'BUILD ERROR')
    finally:
        path.write_text(original)
    results.append((file, old[:50].replace('\n', ' '), new[:50].replace('\n', ' '), verdict))
    log.write_text('\n'.join(map(str, results)) + '\n')
log.write_text('\n'.join(map(str, results)) + '\nDONE\n')
