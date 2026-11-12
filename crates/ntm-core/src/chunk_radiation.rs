//! The world's radiation by chunk (Onda 1, slice 4a): `ChunkRadiationHandlerSimple`, the 1.7.10 default
//! (`ChunkRadiationManager.proxy`).
//!
//! - Per world, a `HashMap<ChunkCoordIntPair, Float>`. `getRadiation` reads it clamped to 0..100 000; `setRadiation`
//!   writes it clamped, only in a loaded chunk; `incrementRad`/`decrementRad` go through both.
//! - `updateSystem`, every 20 server ticks: the map is copied (`new HashMap<>(radiation)`) and cleared, and each chunk
//!   of the copy that is not 0 gives 0.6 of its value to itself, 0.075 to each side and 0.025 to each corner. A
//!   chunk of the copy that already got something adds and decays (`× 0.99 − 0.05`, through the swapped
//!   `clamp_float(0F, x, maxRad)`, which keeps `max(0, x)` with no ceiling); one outside the copy is *set* to the
//!   share, the last one written winning. Both depend on the order the copy is walked, which is the Java's
//!   `HashMap` order: [`ChunkMap`] keeps it.
//! - A chunk that loads is put in with the value its save holds (0 when none, `getFloat` of a missing key), over what
//!   the map had. One that unloads stays in the map, spreading and decaying (`receiveChunkUnload` removes the `Chunk`
//!   from a map keyed by `ChunkCoordIntPair`: nothing), and its save keeps the value it left with. The spread also
//!   puts in chunks never loaded. A chunk at 0 leaves the map at the next update (it is cleared and only what a
//!   chunk not at 0 spreads goes back in), so the map holds the chunks not at 0 and the ring around them.
//! - Where the save is: the NTM storage (`persist::CHUNK_RADIATION_KEY`), not the chunk's NBT (`server::chunk_radiation`).
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_RAD: f32 = 100_000.0;

/// `ChunkCoordIntPair.hashCode` (`agu`, bytecode of the 1.7.10 client).
pub fn hash_code(c: [i32; 2]) -> i32 {
    let i = 1_664_525i32.wrapping_mul(c[0]).wrapping_add(1_013_904_223);
    let j = 1_664_525i32.wrapping_mul(c[1] ^ -559_038_737).wrapping_add(1_013_904_223);
    i ^ j
}

/// `HashMap.hash`: the high half spread over the low.
fn spread(h: i32) -> u32 {
    let h = h as u32;
    h ^ (h >> 16)
}

/// `HashMap.tableSizeFor`.
fn table_size_for(cap: usize) -> usize {
    cap.max(1).next_power_of_two()
}

type Entry = ([i32; 2], f32);

/// A `HashMap.TreeNode`'s links. The bin's walk is its `next` chain (`Bin::list`); the tree only says where a new
/// node goes in it and which node `moveRootToFront` puts first.
#[derive(Clone, Debug, PartialEq)]
struct Node {
    key: [i32; 2],
    hash: u32,
    parent: Option<usize>,
    left: Option<usize>,
    right: Option<usize>,
    red: bool,
}

/// One bin: its nodes in `next` order, and the red-black tree a bin of 9 or more becomes in a table of 64 or more.
#[derive(Clone, Debug, Default, PartialEq)]
struct Bin {
    list: Vec<Entry>,
    tree: Option<(Vec<Node>, usize)>,
}

/// A `java.util.HashMap<ChunkCoordIntPair, Float>`: its buckets, in the order the Java walks them, and its capacity
/// rules (16 to start, doubled past 0.75 full or when a bin of 8 grows under 64; `clear` keeps the table). A bin of 8
/// that grows at 64 and over becomes a tree (`treeifyBin`), which walks in another order: the root first, and each new
/// node after its parent. `ChunkCoordIntPair` is not `Comparable`, so the tree orders by hash and, for two equal full
/// hashes, by `System.identityHashCode`, which no run repeats: the port puts the new one left, as the Java does when
/// its identity hash is the lower.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChunkMap {
    buckets: Vec<Bin>,
    /// The table size the first put makes (`threshold` before the table exists).
    initial: usize,
    size: usize,
}

impl ChunkMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// `new HashMap<>(other)`: sized for it (`size / 0.75 + 1`, to a power of two), filled in its order.
    pub fn copy_of(other: &ChunkMap) -> Self {
        let mut map = Self::default();
        if other.size > 0 {
            map.initial = table_size_for((other.size as f32 / 0.75 + 1.0) as usize);
        }
        for (c, v) in other.iter() {
            map.put(c, v);
        }
        map
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    fn index(&self, c: [i32; 2]) -> usize {
        (spread(hash_code(c)) as usize) & (self.buckets.len() - 1)
    }

    pub fn get(&self, c: [i32; 2]) -> Option<f32> {
        if self.buckets.is_empty() {
            return None;
        }
        self.buckets[self.index(c)].list.iter().find(|(k, _)| *k == c).map(|(_, v)| *v)
    }

    pub fn contains(&self, c: [i32; 2]) -> bool {
        self.get(c).is_some()
    }

    /// `HashMap.putVal`.
    pub fn put(&mut self, c: [i32; 2], v: f32) {
        if self.buckets.is_empty() {
            self.resize();
        }
        let i = self.index(c);
        let cap = self.buckets.len();
        let bin = &mut self.buckets[i];
        if let Some(entry) = bin.list.iter_mut().find(|(k, _)| *k == c) {
            entry.1 = v;
            return;
        }
        if bin.tree.is_some() {
            put_tree_val(bin, c, v);
        } else {
            let count = bin.list.len();
            bin.list.push((c, v));
            // binCount reached TREEIFY_THRESHOLD - 1: treeifyBin, which resizes a table under 64 instead.
            if count >= 8 {
                if cap < 64 {
                    self.resize();
                } else {
                    treeify(bin);
                }
            }
        }
        self.size += 1;
        if self.size as f32 > self.buckets.len() as f32 * 0.75 {
            self.resize();
        }
    }

    /// `HashMap.resize`: the table doubles; each bin splits in order, into the same index and that plus the old size
    /// (`TreeNode.split` for a tree: a half of 6 or fewer goes back to a list, a bigger one is a tree again, rebuilt
    /// when the other half is not empty).
    fn resize(&mut self) {
        let old = std::mem::take(&mut self.buckets);
        let bit = old.len();
        let cap = if old.is_empty() { if self.initial > 0 { self.initial } else { 16 } } else { bit * 2 };
        self.buckets = vec![Bin::default(); cap];
        for (j, bin) in old.into_iter().enumerate() {
            let was_tree = bin.tree.is_some();
            let (lo, hi): (Vec<Entry>, Vec<Entry>) =
                bin.list.iter().partition(|(c, _)| spread(hash_code(*c)) as usize & bit == 0);
            let (lo_empty, hi_empty) = (lo.is_empty(), hi.is_empty());
            for (at, half, other_empty) in [(j, lo, hi_empty), (j + bit, hi, lo_empty)] {
                let target = &mut self.buckets[at];
                target.list = half;
                if !was_tree || target.list.len() <= 6 {
                    continue;
                }
                if other_empty {
                    target.tree = bin.tree.clone();
                } else {
                    treeify(target);
                }
            }
        }
    }

    /// `HashMap.clear`: the table stays.
    pub fn clear(&mut self) {
        for b in &mut self.buckets {
            *b = Bin::default();
        }
        self.size = 0;
    }

    #[cfg(test)]
    fn trees(&self) -> usize {
        self.buckets.iter().filter(|b| b.tree.is_some()).count()
    }

    /// The Java's walk: bins by index, each in its `next` order.
    pub fn iter(&self) -> impl Iterator<Item = ([i32; 2], f32)> + '_ {
        self.buckets.iter().flat_map(|b| b.list.iter().copied())
    }
}

/// The tree's direction from `p` for a key of hash `h` (`putTreeVal`/`treeify`: by hash, then `tieBreakOrder`).
fn dir(p: &Node, h: u32) -> bool {
    // true: left. `(int)` hashes compare signed.
    (p.hash as i32) > (h as i32) || p.hash == h
}

/// `TreeNode.treeify`: each node of the list, in order, into a red-black tree; then `moveRootToFront`.
fn treeify(bin: &mut Bin) {
    let mut nodes: Vec<Node> = Vec::with_capacity(bin.list.len());
    let mut root = 0;
    for (n, (c, _)) in bin.list.iter().enumerate() {
        let h = spread(hash_code(*c));
        nodes.push(Node { key: *c, hash: h, parent: None, left: None, right: None, red: false });
        if n == 0 {
            continue;
        }
        let mut p = root;
        loop {
            let left = dir(&nodes[p], h);
            let next = if left { nodes[p].left } else { nodes[p].right };
            match next {
                Some(q) => p = q,
                None => {
                    nodes[n].parent = Some(p);
                    if left {
                        nodes[p].left = Some(n);
                    } else {
                        nodes[p].right = Some(n);
                    }
                    root = balance_insertion(&mut nodes, root, n);
                    break;
                }
            }
        }
    }
    move_root_to_front(&mut bin.list, nodes[root].key);
    bin.tree = Some((nodes, root));
}

/// `TreeNode.putTreeVal` for a key the bin does not hold: it goes under its parent in the tree and right after it in
/// the `next` chain; then `moveRootToFront`.
fn put_tree_val(bin: &mut Bin, c: [i32; 2], v: f32) {
    let (nodes, root) = bin.tree.as_mut().expect("a tree bin");
    let h = spread(hash_code(c));
    let mut p = *root;
    loop {
        let left = dir(&nodes[p], h);
        let next = if left { nodes[p].left } else { nodes[p].right };
        match next {
            Some(q) => p = q,
            None => {
                let x = nodes.len();
                nodes.push(Node { key: c, hash: h, parent: Some(p), left: None, right: None, red: false });
                if left {
                    nodes[p].left = Some(x);
                } else {
                    nodes[p].right = Some(x);
                }
                let after = bin.list.iter().position(|(k, _)| *k == nodes[p].key).expect("the parent is in the list");
                bin.list.insert(after + 1, (c, v));
                *root = balance_insertion(nodes, *root, x);
                move_root_to_front(&mut bin.list, nodes[*root].key);
                return;
            }
        }
    }
}

/// `TreeNode.moveRootToFront`: the root out of its place in the chain, to the front.
fn move_root_to_front(list: &mut Vec<Entry>, key: [i32; 2]) {
    let at = list.iter().position(|(k, _)| *k == key).expect("the root is in the list");
    let root = list.remove(at);
    list.insert(0, root);
}

/// `TreeNode.rotateLeft`.
fn rotate_left(nodes: &mut [Node], mut root: usize, p: usize) -> usize {
    if let Some(r) = nodes[p].right {
        let rl = nodes[r].left;
        nodes[p].right = rl;
        if let Some(rl) = rl {
            nodes[rl].parent = Some(p);
        }
        let pp = nodes[p].parent;
        nodes[r].parent = pp;
        match pp {
            None => {
                root = r;
                nodes[r].red = false;
            }
            Some(pp) if nodes[pp].left == Some(p) => nodes[pp].left = Some(r),
            Some(pp) => nodes[pp].right = Some(r),
        }
        nodes[r].left = Some(p);
        nodes[p].parent = Some(r);
    }
    root
}

/// `TreeNode.rotateRight`.
fn rotate_right(nodes: &mut [Node], mut root: usize, p: usize) -> usize {
    if let Some(l) = nodes[p].left {
        let lr = nodes[l].right;
        nodes[p].left = lr;
        if let Some(lr) = lr {
            nodes[lr].parent = Some(p);
        }
        let pp = nodes[p].parent;
        nodes[l].parent = pp;
        match pp {
            None => {
                root = l;
                nodes[l].red = false;
            }
            Some(pp) if nodes[pp].right == Some(p) => nodes[pp].right = Some(l),
            Some(pp) => nodes[pp].left = Some(l),
        }
        nodes[l].right = Some(p);
        nodes[p].parent = Some(l);
    }
    root
}

/// `TreeNode.balanceInsertion`: the new root.
fn balance_insertion(nodes: &mut [Node], mut root: usize, mut x: usize) -> usize {
    nodes[x].red = true;
    loop {
        let Some(mut xp) = nodes[x].parent else {
            nodes[x].red = false;
            return x;
        };
        let Some(mut xpp) = nodes[xp].parent.filter(|_| nodes[xp].red) else {
            return root;
        };
        let xppl = nodes[xpp].left;
        if Some(xp) == xppl {
            match nodes[xpp].right {
                Some(xppr) if nodes[xppr].red => {
                    nodes[xppr].red = false;
                    nodes[xp].red = false;
                    nodes[xpp].red = true;
                    x = xpp;
                }
                _ => {
                    if nodes[xp].right == Some(x) {
                        x = xp;
                        root = rotate_left(nodes, root, x);
                        xp = nodes[x].parent.expect("rotated under a parent");
                        xpp = match nodes[xp].parent {
                            Some(g) => g,
                            None => {
                                nodes[xp].red = false;
                                continue;
                            }
                        };
                    }
                    nodes[xp].red = false;
                    nodes[xpp].red = true;
                    root = rotate_right(nodes, root, xpp);
                }
            }
        } else {
            match xppl {
                Some(l) if nodes[l].red => {
                    nodes[l].red = false;
                    nodes[xp].red = false;
                    nodes[xpp].red = true;
                    x = xpp;
                }
                _ => {
                    if nodes[xp].left == Some(x) {
                        x = xp;
                        root = rotate_right(nodes, root, x);
                        xp = nodes[x].parent.expect("rotated under a parent");
                        xpp = match nodes[xp].parent {
                            Some(g) => g,
                            None => {
                                nodes[xp].red = false;
                                continue;
                            }
                        };
                    }
                    nodes[xp].red = false;
                    nodes[xpp].red = true;
                    root = rotate_left(nodes, root, xpp);
                }
            }
        }
    }
}

/// `MathHelper.clamp_float(num, min, max)`.
fn clamp_float(num: f32, min: f32, max: f32) -> f32 {
    if num < min {
        min
    } else if num > max {
        max
    } else {
        num
    }
}

/// `ChunkRadiationHandlerSimple.updateSystem`, one world: the spread and the decay, in the map's order.
pub fn update_system(radiation: &mut ChunkMap) {
    update_system_with(radiation, |_, _| {});
}

/// [`update_system`], `wrote(coord, rad)` told after each write of a share, in order: `coord` the chunk of the copy
/// that gives it, `rad` what the receiving chunk holds now (`radiation.get(newCoord)`, `:104`), which the fog reads.
pub fn update_system_with(radiation: &mut ChunkMap, mut wrote: impl FnMut([i32; 2], f32)) {
    let buff = ChunkMap::copy_of(radiation);
    radiation.clear();
    for (coord, value) in buff.iter() {
        if value == 0.0 {
            continue;
        }
        for i in -1..=1i32 {
            for j in -1..=1i32 {
                let kind = i.abs() + j.abs();
                let percent: f32 = if kind == 0 { 0.6 } else if kind == 1 { 0.075 } else { 0.025 };
                let near = [coord[0] + i, coord[1] + j];
                if buff.contains(near) {
                    let rad = radiation.get(near).unwrap_or(0.0);
                    let new = rad + value * percent;
                    // The swapped arguments: clamp_float(0F, newRad * 0.99F - 0.05F, maxRad).
                    radiation.put(near, clamp_float(0.0, new * 0.99 - 0.05, MAX_RAD));
                } else {
                    radiation.put(near, value * percent);
                }
                wrote(coord, radiation.get(near).expect("just written"));
            }
        }
    }
}

/// `RadiationConfig.fogRad` and `fogCh`: a share leaving a chunk over 100 RAD/s, in 1 of 20 (`:105`).
pub const FOG_RAD: f32 = 100.0;
pub const FOG_CHANCE: i32 = 20;

/// `ParticleRadiationFog.renderParticle`'s 25 quads: each one's place from the particle (the `glTranslatef` of the
/// loop add up, there being no push and pop in it, plus that quad's own jitter) and its half size, as
/// `new Random(50)` gives them at every frame. Each step goes on average 2.5 west, 2.5 north and 0.15 down: the
/// fog lies along a diagonal some 60 blocks long.
pub fn fog_quads() -> [([f64; 3], f64); 25] {
    let mut rand = crate::rbmk::jrandom::JavaRandom::new(50);
    let mut moved = [0f32; 3];
    std::array::from_fn(|_| {
        let d = [(rand.next_gaussian() - 1.0) * 2.5, (rand.next_gaussian() - 1.0) * 0.15, (rand.next_gaussian() - 1.0) * 2.5];
        let size = rand.next_double() * 7.5;
        // GL11.glTranslatef((float) dX, (float) dY, (float) dZ), cumulative.
        for k in 0..3 {
            moved[k] += d[k] as f32;
        }
        let jitter = [rand.next_gaussian() * 0.5, rand.next_gaussian() * 0.5, rand.next_gaussian() * 0.5];
        (std::array::from_fn(|k| f64::from(moved[k]) + jitter[k]), size)
    })
}

/// `handleWorldDestruction:218-247`, one of the ten passes over a chunk's columns (`a` then `b`, x then z): a column
/// in three (`rand.nextInt(3) != 0` skips it), at its height less 0 or 1 (`- rand.nextInt(2)`); `heights` are the
/// columns' in that order, None where not loaded. `cell` reads the block there and turns it, with the same random
/// for the leaves' roll.
pub fn destruction_pass(
    chunk: [i32; 2],
    heights: &[Option<i32>],
    rand: &mut crate::rbmk::jrandom::JavaRandom,
    mut cell: impl FnMut([i32; 3], &mut crate::rbmk::jrandom::JavaRandom),
) {
    for a in 0..16 {
        for b in 0..16 {
            if rand.next_int(3) != 0 {
                continue;
            }
            let below = rand.next_int(2);
            let Some(height) = heights[(a * 16 + b) as usize] else { continue };
            cell([chunk[0] * 16 + a, height - below, chunk[1] * 16 + b], rand);
        }
    }
}

/// The columns of a chunk in [`destruction_pass`]'s order.
pub fn chunk_columns(chunk: [i32; 2]) -> Vec<(i32, i32)> {
    (0..16).flat_map(|a| (0..16).map(move |b| (chunk[0] * 16 + a, chunk[1] * 16 + b))).collect()
}

/// `handleWorldDestruction:230-246`, one cell: what it becomes, if anything. `roll7` is `rand.nextInt(7)`, drawn
/// only for leaves. Grass becomes dead earth; tall grass (any meta: shrub, grass, fern) air; leaves
/// (`Material.leaves`, the dead leaves aside) dead leaves 6 times in 7, air the 7th.
pub fn destroyed(block: &str, roll7: impl FnOnce() -> i32) -> Option<&'static str> {
    let id = crate::blockstate::id_of(block);
    match id {
        "minecraft:grass_block" => Some("ntm:waste_earth"),
        "minecraft:short_grass" | "minecraft:fern" => Some("minecraft:air"),
        _ if id.starts_with("minecraft:") && id.ends_with("_leaves") => {
            Some(if roll7() <= 5 { "ntm:waste_leaves[persistent=true]" } else { "minecraft:air" })
        }
        _ => None,
    }
}

/// `getRadiation`: the chunk's value, clamped to 0..100 000 (0 for none).
pub fn get(radiation: &ChunkMap, chunk: [i32; 2]) -> f32 {
    radiation.get(chunk).map_or(0.0, |r| clamp_float(r, 0.0, MAX_RAD))
}

/// `setRadiation` in a loaded chunk.
pub fn set(radiation: &mut ChunkMap, chunk: [i32; 2], rad: f32) {
    radiation.put(chunk, clamp_float(rad, 0.0, MAX_RAD));
}

/// `incrementRad`.
pub fn increment(radiation: &mut ChunkMap, chunk: [i32; 2], rad: f32) {
    set(radiation, chunk, get(radiation, chunk) + rad);
}

/// `decrementRad`.
pub fn decrement(radiation: &mut ChunkMap, chunk: [i32; 2], rad: f32) {
    set(radiation, chunk, (get(radiation, chunk) - rad).max(0.0));
}

/// The field as saved: each world's chunks with what their saves hold (`hfr_simple_radiation`, written by
/// `receiveChunkSave` as the map holds it, over 100 000 too); 0, what `getFloat` gives for a missing key, is left out.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Saved(pub BTreeMap<String, BTreeMap<String, f32>>);

impl Saved {
    fn key(chunk: [i32; 2]) -> String {
        format!("{},{}", chunk[0], chunk[1])
    }

    /// `getFloat(NBT_KEY_CHUNK_RADIATION)` of the chunk's save: 0 when it holds none.
    pub fn get(&self, dim: &str, chunk: [i32; 2]) -> f32 {
        self.0.get(dim).and_then(|m| m.get(&Self::key(chunk))).copied().unwrap_or(0.0)
    }

    /// Whether writing `rad` would change what the chunk's save holds (bit for bit).
    pub fn differs(&self, dim: &str, chunk: [i32; 2], rad: f32) -> bool {
        self.get(dim, chunk).to_bits() != rad.to_bits()
    }

    /// `setFloat(NBT_KEY_CHUNK_RADIATION, rad)`.
    pub fn set(&mut self, dim: &str, chunk: [i32; 2], rad: f32) {
        if rad == 0.0 {
            if let Some(m) = self.0.get_mut(dim) {
                m.remove(&Self::key(chunk));
                if m.is_empty() {
                    self.0.remove(dim);
                }
            }
        } else {
            self.0.entry(dim.to_string()).or_default().insert(Self::key(chunk), rad);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_map_walks_in_the_javas_bucket_order() {
        let mut m = ChunkMap::new();
        for c in [[0, 0], [1, 0], [0, 1], [-1, 0]] {
            m.put(c, 1.0);
        }
        let mut keys: Vec<_> = m.iter().map(|(c, _)| c).collect();
        let mut by_index = keys.clone();
        by_index.sort_by_key(|c| (spread(hash_code(*c)) & 15) as usize);
        assert_eq!(keys, by_index);
        keys.sort();
        assert_eq!(keys, [[-1, 0], [0, 0], [0, 1], [1, 0]]);
    }

    #[test]
    fn one_chunk_spreads_its_shares() {
        let mut m = ChunkMap::new();
        m.put([0, 0], 100.0);
        update_system(&mut m);
        let (v, six, side, corner) = (100.0f32, 0.6f32, 0.075f32, 0.025f32);
        assert_eq!(m.get([0, 0]), Some((v * six) * 0.99 - 0.05));
        assert_eq!(m.get([1, 0]), Some(v * side));
        assert_eq!(m.get([1, 1]), Some(v * corner));
        assert_eq!(m.len(), 9);
    }

    #[test]
    fn the_swapped_clamp_has_no_ceiling_and_floors_at_zero() {
        assert_eq!(clamp_float(0.0, 200_000.0, MAX_RAD), 200_000.0);
        assert_eq!(clamp_float(0.0, -3.0, MAX_RAD), 0.0);
        let mut m = ChunkMap::new();
        m.put([0, 0], 1.0e9);
        update_system(&mut m);
        assert!(m.get([0, 0]).unwrap() > MAX_RAD, "no ceiling on the decayed value");
        assert_eq!(get(&m, [0, 0]), MAX_RAD, "the reading is clamped");
    }

    #[test]
    fn the_writes_are_told_with_their_source_and_the_receivers_value() {
        let mut m = ChunkMap::new();
        m.put([0, 0], 200.0);
        let mut told = Vec::new();
        update_system_with(&mut m, |c, rad| told.push((c, rad)));
        assert_eq!(told.len(), 9);
        assert!(told.iter().all(|(c, _)| *c == [0, 0]), "the source chunk, not the receiver (`coord`)");
        assert!(told.contains(&([0, 0], (200.0f32 * 0.6) * 0.99 - 0.05)));
        assert!(told.contains(&([0, 0], 200.0f32 * 0.075)));
    }

    #[test]
    fn the_fog_lies_along_the_javas_diagonal() {
        let quads = fog_quads();
        assert_eq!(quads, fog_quads(), "the same layout each time (Random(50))");
        assert!(quads.iter().all(|(_, size)| (0.0..7.5).contains(size)));
        let last = quads[24].0;
        assert!(last[0] < -30.0 && last[2] < -30.0 && last[1] < 0.0, "{last:?}");
    }

    /// tools/chunk-radiation-oracle: `new Random(50)`'s draws as `renderParticle` makes them, in the JVM, against
    /// [`fog_quads`]. The sizes and the draws agree bit for bit; a place may be off in its last bit (one of the 75
    /// is): the JVM's `nextGaussian` takes `StrictMath.log` (fdlibm), the port the platform's `ln`.
    #[test]
    fn the_fog_layout_matches_the_java_oracle() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../tools/chunk-radiation-oracle/fixture.json")).unwrap();
        let java: Vec<[u64; 4]> = fixture["fog"]
            .as_array()
            .unwrap()
            .iter()
            .map(|q| std::array::from_fn(|k| q[k].as_u64().unwrap()))
            .collect();
        assert_eq!(java.len(), 25);
        for ((at, size), q) in fog_quads().iter().zip(&java) {
            assert_eq!(size.to_bits(), q[3], "the size, bit for bit");
            for k in 0..3 {
                let j = f64::from_bits(q[k]);
                assert!((at[k] - j).abs() <= 1e-12 * j.abs().max(1.0), "{} against {j}", at[k]);
            }
        }
    }

    /// The content's fog particles (tools/content-gen/fog_particles.py) have the sizes of [`fog_quads`].
    #[test]
    fn the_fog_particles_have_the_quads_sizes() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/assets/ntm/particles");
        if !dir.exists() {
            eprintln!("content missing: skipped");
            return;
        }
        for (n, (_, size)) in fog_quads().iter().enumerate() {
            let doc: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join(format!("rad_fog_{n}.json"))).unwrap()).unwrap();
            let behaviour = &doc["horizon:behaviour"];
            assert_eq!(behaviour["size"][0].as_f64().unwrap().to_bits(), size.to_bits(), "quad {n}");
            assert_eq!((behaviour["lifetime"].as_i64(), behaviour["alpha"][0].as_f64()), (Some(400), Some(0.125)));
        }
    }

    /// A pass visits a column in three, half of them at the block under the height; the leaves' roll comes from the
    /// same random, after the cell's own draws.
    #[test]
    fn a_destruction_pass_visits_a_third_of_the_columns_half_one_below() {
        let heights = vec![Some(65); 256];
        let (mut visited, mut below, mut passes) = (0u32, 0u32, 0u32);
        for seed in 0..400 {
            let mut rand = crate::rbmk::jrandom::JavaRandom::new(seed);
            destruction_pass([1, -1], &heights, &mut rand, |at, _| {
                assert!((16..32).contains(&at[0]) && (-16..0).contains(&at[2]));
                visited += 1;
                below += u32::from(at[1] == 64);
            });
            passes += 1;
        }
        let per_pass = f64::from(visited) / f64::from(passes);
        assert!((80.0..91.0).contains(&per_pass), "{per_pass} columns a pass, 256 / 3 expected");
        let share = f64::from(below) / f64::from(visited);
        assert!((0.47..0.53).contains(&share), "{share} at the block below");
        // Each column at its own height (x first, then z: `a`, then `b`): a cell is at its column's.
        let sloped: Vec<Option<i32>> = (0..16).flat_map(|a| (0..16).map(move |b| Some(100 + a * 16 + b))).collect();
        let mut rand = crate::rbmk::jrandom::JavaRandom::new(3);
        destruction_pass([1, -1], &sloped, &mut rand, |at, _| {
            let column = 100 + (at[0] - 16) * 16 + (at[2] + 16);
            assert!(at[1] == column || at[1] == column - 1, "{at:?}");
        });
        // A column not loaded takes its draws all the same.
        let mut a = crate::rbmk::jrandom::JavaRandom::new(7);
        let mut b = crate::rbmk::jrandom::JavaRandom::new(7);
        destruction_pass([0, 0], &heights, &mut a, |_, _| {});
        destruction_pass([0, 0], &vec![None; 256], &mut b, |_, _| panic!("not loaded"));
        assert_eq!(a.next_int(1000), b.next_int(1000));
    }

    #[test]
    fn world_destruction_turns_grass_tall_grass_and_leaves() {
        assert_eq!(destroyed("minecraft:grass_block[snowy=false]", || panic!("no roll")), Some("ntm:waste_earth"));
        assert_eq!(destroyed("minecraft:short_grass", || panic!()), Some("minecraft:air"));
        assert_eq!(destroyed("minecraft:fern", || panic!()), Some("minecraft:air"));
        assert_eq!(destroyed("minecraft:oak_leaves[distance=1,persistent=false,waterlogged=false]", || 5), Some("ntm:waste_leaves[persistent=true]"));
        assert_eq!(destroyed("minecraft:birch_leaves", || 6), Some("minecraft:air"));
        assert_eq!(destroyed("ntm:waste_leaves[persistent=true]", || panic!()), None);
        assert_eq!(destroyed("minecraft:tall_grass[half=lower]", || panic!()), None, "double_plant, not tallgrass");
        assert_eq!(destroyed("minecraft:dirt", || panic!()), None);
    }

    /// tools/chunk-radiation-oracle: the Java's `get/set/incrementRad/decrementRad/updateSystem`, run verbatim on seeded
    /// increments, decrements, loads and updates, on bins made to collide into trees and on a big patch; at each `dump`
    /// the port's map must walk in the same order, bit for bit.
    #[test]
    fn the_field_matches_the_java_oracle_bit_for_bit() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../tools/chunk-radiation-oracle/fixture.json")).unwrap();
        let mut trees = 0;
        for case in fixture["cases"].as_array().unwrap() {
            let mut m = ChunkMap::new();
            let mut dumps = case["dumps"].as_array().unwrap().iter();
            for op in case["ops"].as_array().unwrap() {
                let int = |k: usize| op[k].as_i64().unwrap() as i32;
                match op[0].as_str().unwrap() {
                    "inc" => increment(&mut m, [int(1), int(2)], f32::from_bits(int(3) as u32)),
                    "dec" => decrement(&mut m, [int(1), int(2)], f32::from_bits(int(3) as u32)),
                    "load" => m.put([int(1), int(2)], f32::from_bits(int(3) as u32)),
                    "update" => update_system(&mut m),
                    "dump" => {
                        let want: Vec<i64> = dumps.next().unwrap().as_array().unwrap().iter().map(|n| n.as_i64().unwrap()).collect();
                        let got: Vec<i64> =
                            m.iter().flat_map(|(c, v)| [c[0] as i64, c[1] as i64, v.to_bits() as i64]).collect();
                        assert_eq!(got.len(), want.len(), "{}", case["seed"]);
                        let at = got.iter().zip(&want).position(|(a, b)| a != b);
                        assert!(at.is_none(), "{}: the first difference at chunk {:?}", case["seed"], at.map(|n| n / 3));
                    }
                    other => panic!("{other}"),
                }
                trees = trees.max(m.trees());
            }
            assert!(dumps.next().is_none());
        }
        assert!(trees > 0, "the fixture makes the trees a full bin becomes");
    }
}
