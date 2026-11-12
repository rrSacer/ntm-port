//! The step of a walking player (Onda 1, slice 5d, D3): when `Block.onEntityWalking` runs in the 1.7.10.
//!
//! `Entity.moveEntity` (`sa.d(DDD)V` in the 1.7.10 client, offsets 1498-1886), after the move:
//!
//! - only if `canTriggerWalking()` (a player: yes), not `onGround && isSneaking() && instanceof EntityPlayer` (the
//!   flag computed at the start of the method) and `ridingEntity == null`;
//! - the block is `(floor x, floor(posY - 0.2 - yOffset), floor z)`, the feet less 0.2; when the block below that
//!   has render type 11, 32 or 21 (fence, wall, fence gate) it is that block one lower;
//! - `dy` counts only on a ladder (`Blocks.ladder`, `ajn.ap`); otherwise it is 0;
//! - `distanceWalkedOnStepModified += (float) sqrt(dx² + dy² + dz²) * 0.6`, in float (`Q`, `d2f` of a `double` sum);
//! - when it passes `nextStepDistance` (an int) and the block's material is not air: `nextStepDistance = (int) Q + 1`,
//!   the step sound plays and `onEntityWalking` runs on that block.
//!
//! Both counters start at 0 with the entity (a joining or respawned player) and are not saved. The port sees a
//! player's position once a tick; a jump of more than 10 blocks (`NetHandlerPlayServer.processPlayer` corrects a
//! move of more than 100 squared, and a teleport sets the position without `moveEntity`) is a teleport: no step.

/// One player's step counters, between ticks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Walk {
    pub dim: String,
    pub at: [f64; 3],
    /// `Entity.distanceWalkedOnStepModified`.
    pub walked: f32,
    /// `Entity.nextStepDistance`.
    pub next_step: i32,
}

/// What `moveEntity` needs of the world under the feet.
pub trait Ground {
    /// The block id at a cell (`minecraft:air` when none).
    fn block(&mut self, cell: [i32; 3]) -> String;
}

fn id(state: &str) -> &str {
    state.split('[').next().unwrap_or(state)
}

/// Render types 11 (`BlockFence`), 32 (`BlockWall`) and 21 (`BlockFenceGate`).
fn fence_like(id: &str) -> bool {
    id.ends_with("_fence") || id.ends_with("_wall") || id.ends_with("_fence_gate")
}

fn air(id: &str) -> bool {
    matches!(id, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
}

/// `BlockFallout` is 0.125 high (`setBlockBounds(0, 0, 0, 1, 0.125, 1)`) and never stacks; the port's fallout is a
/// snow layer (`layers` 1..8, standing `(layers - 1) / 8` above its cell). Over a fallout cell the feet are taken at
/// the Java's height: those of a player standing on it are its cell + 0.125, and a jump adds to that.
pub fn java_feet(ground: &mut impl Ground, feet: [f64; 3]) -> [f64; 3] {
    let column = feet[1].floor() as i32;
    for y in [column, column - 1] {
        let block = ground.block([feet[0].floor() as i32, y, feet[2].floor() as i32]);
        if id(&block) == "ntm:fallout" && feet[1] >= f64::from(y) {
            let layers = block.split("layers=").nth(1).and_then(|s| s.trim_end_matches(']').split(',').next()?.parse::<u32>().ok()).unwrap_or(1);
            return [feet[0], feet[1] - f64::from(layers.saturating_sub(1)) / 8.0 + 0.125, feet[2]];
        }
    }
    feet
}

/// The block a step at the feet `feet` lands on: the cell of `floor(feet.y - 0.2)`, or the fence, wall or gate below.
pub fn step_cell(ground: &mut impl Ground, feet: [f64; 3]) -> ([i32; 3], String) {
    let cell = [feet[0].floor() as i32, (feet[1] - 0.200_000_002_980_232_24).floor() as i32, feet[2].floor() as i32];
    let below = [cell[0], cell[1] - 1, cell[2]];
    let under = ground.block(below);
    if fence_like(id(&under)) {
        return (below, under);
    }
    let block = ground.block(cell);
    (cell, block)
}

/// One tick of a player's move to `to` in `dim`; `blocked`: sneaking on the ground, or riding; `teleported`: the
/// position was set this tick (`setPositionAndUpdate`, a pearl, a portal), not moved. The block a step lands on,
/// when the player stepped.
pub fn advance(walk: &mut Walk, dim: &str, to: [f64; 3], blocked: bool, teleported: bool, ground: &mut impl Ground) -> Option<([i32; 3], String)> {
    let from = std::mem::replace(&mut walk.at, to);
    if walk.dim != dim {
        walk.dim = dim.to_string();
        return None;
    }
    let (dx, mut dy, dz) = (to[0] - from[0], to[1] - from[1], to[2] - from[2]);
    if teleported || dx * dx + dy * dy + dz * dz > 100.0 || blocked {
        return None;
    }
    let feet = java_feet(ground, to);
    let (cell, block) = step_cell(ground, feet);
    if id(&block) != "minecraft:ladder" {
        dy = 0.0;
    }
    accumulate(walk, [dx, dy, dz], !air(id(&block))).then_some((cell, block))
}

/// Whether a move of the player in place was (in part) a teleport, from the engine's own step distance: its
/// `moveDist` + `flyDist` grow by 0.6 x the distance the physics moved them, so a horizontal move longer than that
/// growth / 0.6 (less a margin for the float) was not the physics': a position set this tick, with or without a
/// walk in the same tick (review 5d P3-1).
pub fn teleported_by_distance(horizontal: f64, steps_grew: f32) -> bool {
    horizontal > f64::from(steps_grew.max(0.0)) / 0.6 + 1.0e-3
}

/// The arithmetic of the step (tools/step-oracle checks it against the Java's): `Q` grows by
/// `(float) sqrt(d²) * 0.6` in double, kept as float; a step when `Q > (float) nextStepDistance` on a block that is
/// not air, and then `nextStepDistance = (int) Q + 1`.
pub fn accumulate(walk: &mut Walk, d: [f64; 3], solid: bool) -> bool {
    let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() as f32;
    walk.walked = (f64::from(walk.walked) + f64::from(length) * 0.6) as f32;
    if walk.walked > walk.next_step as f32 && solid {
        walk.next_step = walk.walked as i32 + 1;
        return true;
    }
    false
}

/// A potion `onEntityWalking` gives: `addPotionEffect(new PotionEffect(HbmPotion.radiation.id, ticks, amplifier))`;
/// `milkproof`: the effect was given `setCurativeItems(new ArrayList())` (milk does not cure it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Walked {
    pub ticks: u32,
    pub amplifier: u8,
    pub milkproof: bool,
}

/// `Block.onEntityWalking` of the radiation blocks, for a player (`creative`: in creative mode):
/// - `WasteEarth:81-93` (glowing mycelium): 30 s at amplifier 3, creative included;
/// - `BlockFallout:53-61`: 10 min at amplifier 0 with no curative items, not in creative;
/// - `BlockSellafield:44-49`: 30 s at `meta < 5 ? meta : meta * 2`, creative included.
pub fn on_entity_walking(block: &str, creative: bool) -> Option<Walked> {
    let potion = |ticks, amplifier, milkproof| Some(Walked { ticks, amplifier, milkproof });
    match id(block) {
        "ntm:waste_mycelium" => potion(30 * 20, 3, false),
        "ntm:fallout" if !creative => potion(10 * 60 * 20, 0, true),
        "ntm:sellafield" => potion(30 * 20, 0, false),
        other => {
            let level: u8 = other.strip_prefix("ntm:sellafield__")?.parse().ok()?;
            potion(30 * 20, if level < 5 { level } else { level * 2 }, false)
        }
    }
}

/// `BlockSellafield.updateTick:53-62`, a random tick of sellafield at `level` (0..5): with `rand.nextInt(meta == 0 ? 25
/// : 15) == 0` it goes down a level, and level 0 becomes `sellafield_slaked`. The block it becomes, if any. The mods
/// have no random source (crate::hash): `seed` mixes what the caller sees (the cell and the game time); the draw is the
/// Java's chance, not its sequence. The `incrementRad` of the same tick waits for the chunk radiation (slice 4).
pub fn sellafield_decay(block: &str, seed: u64) -> Option<String> {
    let level: u8 = match id(block) {
        "ntm:sellafield" => 0,
        other => other.strip_prefix("ntm:sellafield__")?.parse().ok()?,
    };
    if !crate::hash::mix(seed).is_multiple_of(if level == 0 { 25 } else { 15 }) {
        return None;
    }
    Some(match level {
        0 => "ntm:sellafield_slaked".into(),
        1 => "ntm:sellafield".into(),
        n => format!("ntm:sellafield__{}", n - 1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct World(BTreeMap<[i32; 3], String>);
    impl Ground for World {
        fn block(&mut self, cell: [i32; 3]) -> String {
            self.0.get(&cell).cloned().unwrap_or_else(|| "minecraft:air".into())
        }
    }
    fn floor(block: &str) -> World {
        let mut w = World::default();
        for x in -5..40 {
            for z in -5..40 {
                w.0.insert([x, 63, z], block.into());
            }
        }
        w
    }
    fn walk(w: &mut World, path: &[[f64; 3]], blocked: bool) -> Vec<usize> {
        let mut s = Walk { dim: "o".into(), at: path[0], ..Walk::default() };
        path.iter().enumerate().skip(1).filter_map(|(i, p)| advance(&mut s, "o", *p, blocked, false, w).map(|_| i)).collect()
    }

    /// The Java's arithmetic, compiled and run (tools/step-oracle): over long irregular walks (walking, sprinting,
    /// ladders, air) the same steps and the same float `Q`, bit for bit.
    #[test]
    fn steps_as_the_java_oracle() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../../../tools/step-oracle/fixture.json")).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let mut walk = Walk::default();
            let mut steps = Vec::new();
            for (i, m) in case["moves"].as_array().unwrap().iter().enumerate() {
                let m = m.as_array().unwrap();
                let d = [m[0].as_f64().unwrap(), m[1].as_f64().unwrap(), m[2].as_f64().unwrap()];
                if accumulate(&mut walk, d, m[3].as_bool().unwrap()) {
                    steps.push(i as u64);
                }
            }
            let expected: Vec<u64> = case["steps"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
            assert_eq!(steps, expected, "seed {}", case["seed"]);
            assert_eq!(walk.walked.to_bits(), case["walked_bits"].as_u64().unwrap() as u32);
            assert_eq!(i64::from(walk.next_step), case["next_step"].as_i64().unwrap());
        }
    }

    /// The potions of the radiation blocks: the mycelium's and the sellafield's reach creative players too, the
    /// fallout's does not and milk does not cure it; sellafield level 5 is amplifier 10.
    #[test]
    fn the_radiation_blocks_give_the_java_potions() {
        let w = |b: &str, c| on_entity_walking(b, c);
        assert_eq!(w("ntm:waste_mycelium", true), Some(Walked { ticks: 600, amplifier: 3, milkproof: false }));
        assert_eq!(w("ntm:fallout[layers=1]", false), Some(Walked { ticks: 12000, amplifier: 0, milkproof: true }));
        assert_eq!(w("ntm:fallout[layers=1]", true), None);
        let levels: Vec<_> = (0..6).map(|n| {
            let block = if n == 0 { "ntm:sellafield".to_string() } else { format!("ntm:sellafield__{n}") };
            w(&block, true).map(|p| (p.ticks, p.amplifier))
        }).collect();
        assert_eq!(levels, [0, 1, 2, 3, 4, 10].map(|a| Some((600, a))));
        assert_eq!(w("ntm:sellafield_slaked", false), None);
        assert_eq!(w("ntm:sellafield__x", false), None);
        assert_eq!(w("minecraft:stone", false), None);
    }

    /// A random tick takes sellafield down a level 1 time in 15 (1 in 25 at level 0, which becomes slaked).
    #[test]
    fn sellafield_goes_down_a_level_at_the_java_chance() {
        for (level, chance) in [(0u8, 25u64), (1, 15), (3, 15), (5, 15)] {
            let block = if level == 0 { "ntm:sellafield".to_string() } else { format!("ntm:sellafield__{level}") };
            let next = match level { 0 => "ntm:sellafield_slaked".to_string(), 1 => "ntm:sellafield".to_string(), n => format!("ntm:sellafield__{}", n - 1) };
            let n = 150_000u64;
            let hits: Vec<_> = (0..n).filter_map(|s| sellafield_decay(&block, s * 7919)).collect();
            assert!(hits.iter().all(|b| *b == next), "{level}: {:?}", hits.first());
            let (mean, sd) = (n as f64 / chance as f64, (n as f64 * (1.0 / chance as f64) * (1.0 - 1.0 / chance as f64)).sqrt());
            assert!((hits.len() as f64 - mean).abs() < 4.0 * sd, "{level}: {} against {mean}", hits.len());
        }
        assert_eq!(sellafield_decay("ntm:sellafield_slaked", 0), None);
        assert_eq!(sellafield_decay("minecraft:stone", 0), None);
    }

    /// Walking at 0.2 blocks a tick on a floor: a step when 0.12 a tick passes 1, 2, 3.. (`(int) Q + 1`).
    #[test]
    fn a_step_every_time_the_walked_distance_passes_the_next_mark() {
        let mut w = floor("minecraft:stone");
        let path: Vec<_> = (0..60).map(|i| [0.5 + 0.2 * f64::from(i), 64.0, 0.5]).collect();
        // Q after tick i: 0.12 i (float). Steps when Q > 0, 1, 2, ...: ticks 1, 9, 17, 26, 34, 42, 51, 59.
        assert_eq!(walk(&mut w, &path, false), [1, 9, 17, 26, 34, 42, 51, 59]);
    }

    /// Sneaking on the ground or riding: no step and no distance (the whole block is skipped).
    #[test]
    fn sneaking_or_riding_takes_no_step() {
        let mut w = floor("minecraft:stone");
        let path: Vec<_> = (0..60).map(|i| [0.5 + 0.2 * f64::from(i), 64.0, 0.5]).collect();
        assert!(walk(&mut w, &path, true).is_empty());
    }

    /// Standing still, or teleported (more than 10 blocks in a tick): no step.
    #[test]
    fn standing_or_teleporting_takes_no_step() {
        let mut w = floor("minecraft:stone");
        assert!(walk(&mut w, &[[0.5, 64.0, 0.5]; 30], false).is_empty());
        assert!(walk(&mut w, &[[0.5, 64.0, 0.5], [20.5, 64.0, 0.5]], false).is_empty());
    }

    /// The block is the one under the feet less 0.2: on a 0.125-high layer (the fallout) it is the block below; with
    /// the feet 0.3 above the layer's cell, it is the layer. A fence, wall or gate below that cell takes its place.
    #[test]
    fn the_step_lands_on_the_block_at_the_feet_less_a_fifth() {
        let mut w = floor("minecraft:stone");
        w.0.insert([0, 64, 0], "ntm:fallout[layers=1]".into());
        assert_eq!(step_cell(&mut w, [0.5, 64.125, 0.5]).1, "minecraft:stone");
        assert_eq!(step_cell(&mut w, [0.5, 64.3, 0.5]).1, "ntm:fallout[layers=1]");
        w.0.insert([3, 63, 0], "minecraft:oak_fence".into());
        assert_eq!(step_cell(&mut w, [3.5, 64.0, 0.5]), ([3, 63, 0], "minecraft:oak_fence".into()));
        w.0.insert([4, 62, 0], "minecraft:cobblestone_wall".into());
        w.0.insert([4, 63, 0], "minecraft:air".into());
        assert_eq!(step_cell(&mut w, [4.5, 63.9, 0.5]).0, [4, 62, 0]);
    }

    /// In the air (no block at the feet less 0.2) the distance grows but no step lands; the first tick over a block
    /// takes the step owed.
    #[test]
    fn no_step_on_air_but_the_distance_still_counts() {
        let mut w = World::default();
        w.0.insert([5, 63, 0], "minecraft:stone".into());
        let path: Vec<_> = (0..12).map(|i| [0.5 + 0.5 * f64::from(i), 64.0, 0.5]).collect();
        // Q 0.3 a tick: past 0 at tick 1, but air until x reaches 5 (tick 9, x = 5.0).
        assert_eq!(walk(&mut w, &path, false), [9]);
    }

    /// Vertical moves count only on a ladder.
    #[test]
    fn climbing_counts_only_on_a_ladder() {
        let mut w = floor("minecraft:stone");
        let up: Vec<_> = (0..20).map(|i| [0.5, 64.0 + 0.2 * f64::from(i), 0.5]).collect();
        assert!(walk(&mut w, &up, false).is_empty());
        let mut l = World::default();
        for y in 60..90 {
            l.0.insert([0, y, 0], "minecraft:ladder[facing=north]".into());
        }
        assert_eq!(walk(&mut l, &up, false), [1, 9, 17]);
        // Only `Blocks.ladder` (`ajn.ap`, offset 1656): a vine or a scaffolding climbs with no distance (review R1).
        for climbable in ["minecraft:vine[east=true]", "minecraft:scaffolding[bottom=false]"] {
            let mut v = World::default();
            for y in 60..90 {
                v.0.insert([0, y, 0], climbable.into());
            }
            assert!(walk(&mut v, &up, false).is_empty(), "{climbable}");
        }
    }

    /// A teleport (the position set, not moved) takes no step, however short (review P2-1).
    #[test]
    fn a_teleport_takes_no_step() {
        let mut w = floor("ntm:waste_mycelium");
        let mut s = Walk { dim: "o".into(), at: [0.5, 64.0, 2.5], ..Walk::default() };
        assert_eq!(advance(&mut s, "o", [0.5, 64.0, 0.5], false, true, &mut w), None);
        assert_eq!((s.walked, s.at), (0.0, [0.5, 64.0, 0.5]));
        assert!(advance(&mut s, "o", [0.5, 64.0, 0.9], false, false, &mut w).is_some(), "the next move steps");
    }

    /// A teleport leaves the step counters as they were (`setPositionAndUpdate` does not touch them, review M1): with
    /// `Q` at 0.9 and `nextStepDistance` at 1, a teleport and then 0.2 of walking steps as without the teleport.
    #[test]
    fn a_teleport_keeps_the_step_counters() {
        let mut w = floor("minecraft:stone");
        let mut s = Walk { dim: "o".into(), at: [0.5, 64.0, 0.5], walked: 0.9, next_step: 1 };
        assert_eq!(advance(&mut s, "o", [5.5, 64.0, 0.5], false, true, &mut w), None);
        assert_eq!((s.walked, s.next_step), (0.9, 1));
        assert!(advance(&mut s, "o", [5.7, 64.0, 0.5], false, false, &mut w).is_some());
    }

    /// Walking moves the player 1/0.6 of what their step distance grew; a longer move is a teleport, also when it
    /// comes in a tick of walking (review P3-1).
    #[test]
    fn a_move_longer_than_the_step_distance_is_a_teleport() {
        assert!(!teleported_by_distance(0.0, 0.0));
        assert!(!teleported_by_distance(0.2158, 0.2158 * 0.6));
        assert!(teleported_by_distance(0.3, 0.0), "a short /tp standing");
        assert!(teleported_by_distance(5.2158, 0.2158 * 0.6), "a pearl landing while walking");
        assert!(!teleported_by_distance(0.0, 0.5), "a climb grows the distance without a horizontal move");
    }

    /// The fallout at the Java's 0.125 whatever its layers (review P3-4): standing on 1 or 4 layers the step lands
    /// below it; a jump of 0.1 above standing lands on it (the Java's feet 0.225 above the cell).
    #[test]
    fn fallout_is_as_high_as_the_javas_whatever_its_layers() {
        for layers in [1, 4, 8] {
            let mut w = floor("minecraft:stone");
            w.0.insert([0, 64, 0], format!("ntm:fallout[layers={layers}]"));
            let standing = 64.0 + f64::from(layers - 1) / 8.0;
            let feet = java_feet(&mut w, [0.5, standing, 0.5]);
            assert_eq!(step_cell(&mut w, feet).0, [0, 63, 0], "{layers} standing");
            let feet = java_feet(&mut w, [0.5, standing + 0.1, 0.5]);
            assert_eq!(step_cell(&mut w, feet).0, [0, 64, 0], "{layers} jumping");
        }
        // The top of a high jump over eight layers (standing 0.875 up, then 0.35 higher): the feet are in the cell above
        // the fallout's, which is found one below them (review M2); the Java's feet 0.475 up land on the fallout.
        let mut w = floor("minecraft:stone");
        w.0.insert([0, 64, 0], "ntm:fallout[layers=8]".into());
        let feet = java_feet(&mut w, [0.5, 65.225, 0.5]);
        assert_eq!(step_cell(&mut w, feet), ([0, 64, 0], "ntm:fallout[layers=8]".into()));
        let mut w = floor("minecraft:stone");
        assert_eq!(java_feet(&mut w, [0.5, 64.0, 0.5]), [0.5, 64.0, 0.5], "no fallout, no change");
        // Walking flat on four layers (standing 0.375 above the cell): every step lands on the stone below.
        let mut w = floor("minecraft:stone");
        for x in 0..20 {
            w.0.insert([x, 64, 0], "ntm:fallout[layers=4]".into());
        }
        let mut s = Walk { dim: "o".into(), at: [0.5, 64.375, 0.5], ..Walk::default() };
        let landed: Vec<_> = (1..40).filter_map(|i| advance(&mut s, "o", [0.5 + 0.3 * f64::from(i), 64.375, 0.5], false, false, &mut w)).map(|(_, b)| b).collect();
        assert!(!landed.is_empty() && landed.iter().all(|b| b == "minecraft:stone"), "{landed:?}");
    }
}
