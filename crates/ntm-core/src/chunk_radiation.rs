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
            }
        }
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
