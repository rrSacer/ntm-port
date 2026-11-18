#!/usr/bin/env python3
"""Hand-picked mutants of the autocrafter (`crates/ntm-core/src/autocrafter.rs`): each must fail its tests.
Usage: tools/mutants/autocrafter.py REPO TARGET LOG (docs/onda-2-sublote-2.md)"""
import subprocess, sys
from pathlib import Path
repo, target, log = Path(sys.argv[1]), sys.argv[2], Path(sys.argv[3])
src = repo / 'crates/ntm-core/src'
MUTANTS = [
    ('autocrafter.rs', 'if clicked.contains(&true) || self.recipe_index >= self.recipes.len() {', 'if self.recipe_index >= self.recipes.len() {'),
    # The third review: a copy put again, the hand's rule, the vanilla families, exact by damage.
    ('autocrafter.rs', 'self.slots[i].as_ref().is_some_and(|s| mode_of(s).is_none())\n                    || ', ''),
    ('autocrafter.rs', '(_, true) => can_insert(slots, slot, item),', '(_, true) => manual || can_insert(slots, slot, item),'),
    ('autocrafter.rs', '(true, true) if slot == OUTPUT || slot == BATTERY => true,', '(true, true) if slot == BATTERY => true,'),
    ('autocrafter.rs', '        _ if wood("_planks") => "planks",\n', ''),
    ('autocrafter.rs', '        _ if colored("_wool") => "wool",\n', ''),
    ('autocrafter.rs', '        _ if DYES.contains(&path) => "dye",\n', ''),
    ('autocrafter.rs', '(id.starts_with("ntm:") && id.contains("__")) || vanilla_family(id).is_some()', 'id.starts_with("ntm:") && id.contains("__")'),
    ('autocrafter.rs', 'vanilla_family(id).unwrap_or_else(|| id.split_once("__").map_or(id, |(b, _)| b))', 'id.split_once("__").map_or(id, |(b, _)| b)'),
    ('autocrafter.rs', 'EXACT => input_id == filter_id && input.damage == filter.damage,', 'EXACT => input_id == filter_id,'),
    ('autocrafter.rs', 'self.slots[i] = template[i].clone().map(|s| with_mode(s, mode.as_deref()));', 'self.slots[i] = template[i].clone();'),
    ('autocrafter.rs', 'if self.recipes.is_empty() || self.power < CONSUMPTION {', 'if self.recipes.is_empty() || self.power < 1 {'),
    ('autocrafter.rs', '        self.power -= CONSUMPTION;\n', ''),
    ('autocrafter.rs', 'Some(o) => same_item(o, &made.result) && o.count + made.result.count <= made.max_stack,', 'Some(o) => same_item(o, &made.result) && o.count <= made.max_stack,'),
    ('autocrafter.rs', 'a.id == b.id && a.damage == b.damage && a.custom == b.custom', 'a.id == b.id && a.damage == b.damage'),
    ('autocrafter.rs', 'self.slots[i] = made.remainders.get(k).cloned().flatten();', 'self.slots[i] = None;'),
    ('autocrafter.rs', '        self.inputs_seen = None;\n        self.inputs_match.clear();\n', ''),
    ('autocrafter.rs', '        if self.recipe_index >= self.recipes.len() {\n            self.recipe_index = 0;\n        }\n        self.show_result();', '        if self.recipe_index > self.recipes.len() {\n            self.recipe_index = 0;\n        }\n        self.show_result();'),
    ('autocrafter.rs', 'for prefix in ["ingot", "block", "dust", "nugget", "plate"] {', 'for prefix in ["block", "dust", "nugget", "plate"] {'),
    ('autocrafter.rs', '        Some(EXACT) if bedrock_grade(id).is_some() => BEDROCK.into(),\n', ''),
    ('autocrafter.rs', 'WILDCARD => base(input_id) == base(filter_id),', 'WILDCARD => input_id == filter_id,'),
    ('autocrafter.rs', 'held.as_ref().is_some_and(|h| h.count + item.count > INPUT_MAX) || item.count > INPUT_MAX', 'held.as_ref().is_some_and(|h| h.count + item.count > INPUT_MAX)'),
    ('autocrafter.rs', '        if other.count < size {\n            return false;', '        if other.count <= size {\n            return false;'),
    ('autocrafter.rs', '    if slot == OUTPUT {\n        return true;\n    }', '    if slot == OUTPUT {\n        return false;\n    }'),
    ('autocrafter.rs', 'if slot >= SLOTS || slot <= RESULT {', 'if slot >= SLOTS || slot < RESULT {'),
    ('autocrafter.rs', '        MAX_POWER.saturating_sub(self.power)', '        0'),
    ('autocrafter.rs', 'vec![self.power as i32, self.recipe_index as i32]', 'vec![self.power as i32, 0]'),
    ('autocrafter.rs', 'if item.count > 1 && has_container_item(&item.id) {', 'if item.count > 2 && has_container_item(&item.id) {'),
    ('autocrafter.rs', '        if let Some(n) = names.iter().find(|n| n.starts_with(prefix)) {', '        if let Some(n) = names.iter().rev().find(|n| n.starts_with(prefix)) {'),
    ('server/transaction.rs', '    fn crafting_matches(&mut self, grid: &[Option<Stack>]) -> Vec<crate::autocrafter::CraftMatch> {\n        self.host.crafting_matches(grid)\n    }\n', ''),
    ('server/container_bridge.rs', '                    if let Data::Autocrafter(a) = data {\n                        a.refresh(|grid| host.crafting_matches(grid));\n                    }', ''),
]
results = []
for file, old, new in MUTANTS:
    path = src / file
    original = path.read_text()
    if old not in original:
        results.append((file, old[:40], 'ANCHOR MISSING')); continue
    path.write_text(original.replace(old, new, 1))
    try:
        r = subprocess.run(['/usr/local/bin/agent-run', 'env', 'QUEUE=cargo', 'CARGO_BUILD_JOBS=4', f'CARGO_TARGET_DIR={target}', 'cargo', 'test', '-p', 'ntm-core', '--lib', '--', 'autocrafter'],
                           cwd=repo, capture_output=True, text=True, timeout=3000)
        verdict = 'KILLED' if r.returncode != 0 and 'test result: FAILED' in r.stdout else ('SURVIVED' if r.returncode == 0 else 'BUILD ERROR')
    finally:
        path.write_text(original)
    results.append((file, old[:50].replace('\n', ' '), new[:50].replace('\n', ' '), verdict))
    log.write_text('\n'.join(map(str, results)) + '\n')
log.write_text('\n'.join(map(str, results)) + '\nDONE\n')
