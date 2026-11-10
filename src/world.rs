//! The world store: the generated body plus a sparse quadtree of changes.
//!
//! Every node the swarm has touched has an entry. Untouched regions have none
//! below their nearest touched ancestor and are answered by the generator.
//! Each entry carries the cells of each material it still holds, so any
//! region's remaining content is a single read, and a parent always equals the
//! sum of its four children.
//!
//! How totals stay consistent from the first estimate to exact cells:
//! - The root starts with the body's estimate.
//! - When a node is first opened, its children get their own estimates, scaled
//!   so that together they add up exactly to the parent.
//! - When a 64 x 64 leaf is first generated cell by cell, its real counts
//!   replace its share. The difference is booked in the ledger as a
//!   reconciliation and passed up to every ancestor.
//!
//! Entries collapse: a leaf that is fully mined becomes Empty and drops its
//! cells, and four Empty children merge into an Empty parent.

use crate::ledger::Ledger;
use crate::materials::{MatId, VACUUM};
use crate::noise::hash;
use crate::planet::Planet;
use crate::totals::{self, LEAF, Node, Tree};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// A fast hash for node keys (already well-spread Morton codes). The standard
/// hash resists attacks we don't face, at several times the cost.
#[derive(Default)]
pub struct NodeHasher(u64);
impl Hasher for NodeHasher {
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 ^ *b as u64).wrapping_mul(0x100_0000_01b3);
        }
    }
    fn write_u64(&mut self, v: u64) {
        self.0 = (v ^ (v >> 31)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
    fn finish(&self) -> u64 {
        self.0 ^ (self.0 >> 29)
    }
}
pub type NodeMap<V> = HashMap<u64, V, BuildHasherDefault<NodeHasher>>;

#[derive(Debug, Clone, PartialEq)]
pub enum State {
    /// Not yet opened: the generator answers, `totals` is this node's share.
    Untouched,
    /// Opened: the four children have entries.
    Split,
    /// A leaf generated cell by cell: 64 x 64 materials, row by row.
    Dense(Box<[MatId]>),
    /// Fully mined or open space.
    Empty,
    /// Filled with one material (settled rubble, later).
    Uniform(MatId),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub state: State,
    /// Cells of each material still here (index 0, vacuum, stays 0).
    pub totals: Vec<f64>,
}

/// A leaf as stored (see `World::leaf_snapshot`): all one material (empty
/// is all vacuum), or cell by cell, row by row.
#[derive(Debug, Clone)]
pub enum LeafSnap {
    Uniform(MatId),
    Dense(std::sync::Arc<[MatId]>),
}

impl LeafSnap {
    /// The material at offset (i, j) in the leaf.
    pub fn at(&self, i: i64, j: i64) -> MatId {
        match self {
            LeafSnap::Uniform(m) => *m,
            LeafSnap::Dense(c) => c[(j * LEAF + i) as usize],
        }
    }
}

pub struct World {
    /// Shared, so a renderer can read the generator while the world runs.
    pub planet: std::sync::Arc<Planet>,
    pub tree: Tree,
    pub nodes: NodeMap<Entry>,
    pub ledger: Ledger,
    /// Where cells changed since a renderer last looked; None (the default)
    /// keeps no record. Not part of the world's state or its hash.
    pub track: Option<Track>,
}

/// Changed places, for a renderer: 64 x 64 blocks of cells (cell >> 6) and
/// whole rectangles (x0, x1, y0, y1; ends exclusive) emptied at once.
#[derive(Default, Debug)]
pub struct Track {
    pub blocks: std::collections::HashSet<(i64, i64)>,
    pub rects: Vec<[i64; 4]>,
}

/// Samples per side when estimating a child node, by how far below the root
/// it sits. The few nodes covering big shares of the body are opened rarely
/// and their shares are inherited by everything beneath them, so they get many
/// samples (65,000 for the root's children); the many small ones, about 1,000.
/// (Per level of depth, not size: on a big body the 2 km nodes are small
/// shares, and splitting them was most of the work as the swarm spread.)
fn child_samples(level: u32, root_level: u32) -> i64 {
    match root_level.saturating_sub(level) {
        0..=1 => 256,
        2..=3 => 96,
        _ => 32,
    }
}

pub fn key(n: Node) -> u64 {
    // Level in the top 6 bits, then the Morton code of (ix, iy).
    fn spread(mut v: u64) -> u64 {
        v &= 0x1FFF_FFFF;
        v = (v | (v << 16)) & 0x0000_FFFF_0000_FFFF;
        v = (v | (v << 8)) & 0x00FF_00FF_00FF_00FF;
        v = (v | (v << 4)) & 0x0F0F_0F0F_0F0F_0F0F;
        v = (v | (v << 2)) & 0x3333_3333_3333_3333;
        v = (v | (v << 1)) & 0x5555_5555_5555_5555;
        v
    }
    ((n.level as u64) << 58) | spread(n.ix as u64) | (spread(n.iy as u64) << 1)
}

fn nonvacuum(t: &[f64]) -> f64 {
    t[1..].iter().sum()
}

/// Whether a leaf's counts (never negative) hold anything at all: the same
/// answer as `nonvacuum(t) > 0.0`, stopping at the first material found.
fn holds_any(t: &[f64]) -> bool {
    t[1..].iter().any(|&v| v != 0.0)
}

impl World {
    pub fn new(planet: Planet) -> Self {
        Self::shared(std::sync::Arc::new(planet))
    }

    /// A world over a planet someone else also holds (the game's renderer).
    pub fn shared(planet: std::sync::Arc<Planet>) -> Self {
        let tree = Tree::for_planet(&planet);
        let root = tree.root();
        let mut est = totals::estimate(&planet, &tree, root, 1024).cells;
        est[0] = 0.0;
        let initial_kg = est.iter().enumerate().map(|(i, c)| c * planet.mats.get(i as crate::materials::MatId).density).collect();
        let mut nodes = NodeMap::default();
        nodes.insert(key(root), Entry { state: State::Untouched, totals: est });
        World { ledger: Ledger::new(initial_kg), planet, tree, nodes, track: None }
    }

    /// Note that cell (x, y) changed (if anyone is keeping track).
    #[inline]
    fn mark(&mut self, x: i64, y: i64) {
        if let Some(t) = &mut self.track {
            t.blocks.insert((x >> 6, y >> 6));
        }
    }

    fn nm(&self) -> usize {
        self.planet.mats.len()
    }

    /// The node at `level` containing cell (x, y), or None outside the root.
    pub fn node_at(&self, level: u32, x: i64, y: i64) -> Option<Node> {
        let (dx, dy) = (x - self.tree.origin, y - self.tree.origin);
        let side = LEAF << self.tree.root_level;
        if dx < 0 || dy < 0 || dx >= side || dy >= side {
            return None;
        }
        Some(Node { level, ix: dx >> (6 + level), iy: dy >> (6 + level) })
    }

    fn parent(&self, n: Node) -> Option<Node> {
        (n.level < self.tree.root_level).then(|| Node { level: n.level + 1, ix: n.ix >> 1, iy: n.iy >> 1 })
    }

    /// What occupies cell (x, y) now.
    pub fn material(&self, x: i64, y: i64) -> MatId {
        self.material_with(x, y, false)
    }

    /// Like `material`, but where the world is untouched it gives the bare
    /// ground without any plants (cheap: no flora is grown to answer). For
    /// probes that only want the lie of the land.
    pub fn material_bare(&self, x: i64, y: i64) -> MatId {
        self.material_with(x, y, true)
    }

    fn material_with(&self, x: i64, y: i64, bare: bool) -> MatId {
        let generated = |x: i64, y: i64| if bare { self.planet.base_material(x, y) } else { self.planet.material(x, y) };
        // Fast path: most lookups land in a leaf that already has its own
        // entry (near the digging). A leaf entry only exists while every
        // ancestor is split, so it is the answer.
        if let Some(n) = self.node_at(0, x, y) {
            if let Some(e) = self.nodes.get(&key(n)) {
                match &e.state {
                    State::Dense(c) => {
                        let (x0, _, y0, _) = self.tree.bounds(n);
                        return c[((y - y0) * LEAF + (x - x0)) as usize];
                    }
                    State::Empty => return VACUUM,
                    State::Uniform(m) => return *m,
                    State::Untouched => return generated(x, y),
                    State::Split => {}
                }
            }
        }
        let mut level = self.tree.root_level;
        loop {
            let n = match self.node_at(level, x, y) {
                Some(n) => n,
                None => return VACUUM,
            };
            match self.nodes.get(&key(n)).map(|e| &e.state) {
                None | Some(State::Untouched) => return generated(x, y),
                Some(State::Empty) => return VACUUM,
                Some(State::Uniform(m)) => return *m,
                Some(State::Dense(c)) => {
                    let (x0, _, y0, _) = self.tree.bounds(n);
                    return c[((y - y0) * LEAF + (x - x0)) as usize];
                }
                Some(State::Split) => level -= 1,
            }
        }
    }

    /// What occupies cell (x, y), if the store already knows without asking
    /// the generator: None where the cell is still as generated (untouched).
    /// Lets a renderer keep its own cache of generated cells.
    pub fn material_stored(&self, x: i64, y: i64) -> Option<MatId> {
        if let Some(n) = self.node_at(0, x, y) {
            if let Some(e) = self.nodes.get(&key(n)) {
                match &e.state {
                    State::Dense(c) => {
                        let (x0, _, y0, _) = self.tree.bounds(n);
                        return Some(c[((y - y0) * LEAF + (x - x0)) as usize]);
                    }
                    State::Empty => return Some(VACUUM),
                    State::Uniform(m) => return Some(*m),
                    State::Untouched => return None,
                    State::Split => {}
                }
            }
        }
        let mut level = self.tree.root_level;
        loop {
            let n = self.node_at(level, x, y)?;
            match self.nodes.get(&key(n)).map(|e| &e.state) {
                None | Some(State::Untouched) => return None,
                Some(State::Empty) => return Some(VACUUM),
                Some(State::Uniform(m)) => return Some(*m),
                Some(State::Dense(c)) => {
                    let (x0, _, y0, _) = self.tree.bounds(n);
                    return Some(c[((y - y0) * LEAF + (x - x0)) as usize]);
                }
                Some(State::Split) => level -= 1,
            }
        }
    }

    /// The 64 x 64 cells of the leaf with origin (x0, y0), if it has been
    /// generated cell by cell.
    pub fn dense_leaf(&self, x0: i64, y0: i64) -> Option<&[MatId]> {
        let n = self.node_at(0, x0, y0)?;
        match self.nodes.get(&key(n)).map(|e| &e.state) {
            Some(State::Dense(c)) => Some(c),
            _ => None,
        }
    }

    /// The leaf (64 x 64 cells, on the tree's grid) holding cell (x, y).
    pub fn leaf_of(&self, x: i64, y: i64) -> (i64, i64) {
        let o = self.tree.origin;
        ((x - o).div_euclid(LEAF), (y - o).div_euclid(LEAF))
    }

    /// The first cell of leaf (lx, ly).
    pub fn leaf_corner(&self, lx: i64, ly: i64) -> (i64, i64) {
        let o = self.tree.origin;
        (o + lx * LEAF, o + ly * LEAF)
    }

    /// What leaf (lx, ly) holds now, as stored, for a renderer keeping its
    /// own copy: None if it is as generated.
    pub fn leaf_snapshot(&self, lx: i64, ly: i64) -> Option<LeafSnap> {
        let (x0, y0) = self.leaf_corner(lx, ly);
        if let Some(c) = self.dense_leaf(x0, y0) {
            return Some(LeafSnap::Dense(c.into()));
        }
        self.material_stored(x0, y0).map(LeafSnap::Uniform)
    }

    /// Every leaf that isn't as generated (for a renderer starting its copy,
    /// as after loading a save).
    pub fn stored_leaves(&self) -> Vec<(i64, i64)> {
        fn unspread(mut v: u64) -> u64 {
            v &= 0x5555_5555_5555_5555;
            v = (v | (v >> 1)) & 0x3333_3333_3333_3333;
            v = (v | (v >> 2)) & 0x0F0F_0F0F_0F0F_0F0F;
            v = (v | (v >> 4)) & 0x00FF_00FF_00FF_00FF;
            v = (v | (v >> 8)) & 0x0000_FFFF_0000_FFFF;
            v = (v | (v >> 16)) & 0x0000_0000_FFFF_FFFF;
            v
        }
        let mut out = Vec::new();
        for (k, e) in &self.nodes {
            if matches!(e.state, State::Untouched | State::Split) {
                continue;
            }
            let level = (k >> 58) as u32;
            let m = k & ((1u64 << 58) - 1);
            let (ix, iy) = (unspread(m) as i64, unspread(m >> 1) as i64);
            let n = 1i64 << level;
            for dy in 0..n {
                for dx in 0..n {
                    out.push((ix * n + dx, iy * n + dy));
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// A reader for many lookups close together: it remembers the last leaf.
    pub fn reader(&self) -> Reader<'_> {
        Reader { w: self, leaf: None }
    }

    /// True if (x, y) lies in a region recorded as Empty (mined out whole, or
    /// open space), without generating anything.
    pub fn in_empty_region(&self, x: i64, y: i64) -> bool {
        // A leaf with its own entry has every ancestor split: the leaf's own
        // state is the answer.
        if let Some(n) = self.node_at(0, x, y) {
            if let Some(e) = self.nodes.get(&key(n)) {
                return e.state == State::Empty;
            }
        }
        let mut level = self.tree.root_level;
        loop {
            let Some(n) = self.node_at(level, x, y) else { return true };
            match self.nodes.get(&key(n)).map(|e| &e.state) {
                Some(State::Empty) => return true,
                Some(State::Split) if level > 0 => level -= 1,
                _ => return false,
            }
        }
    }

    /// Open an Untouched, Empty or Uniform node so its children have entries.
    fn split(&mut self, n: Node) {
        let e = self.nodes.get(&key(n)).expect("split: no entry").clone();
        let kids = n.children();
        let nm = self.nm();
        let child_entries: Vec<Entry> = match &e.state {
            State::Split => return,
            State::Empty => kids.iter().map(|_| Entry { state: State::Empty, totals: vec![0.0; nm] }).collect(),
            State::Uniform(m) => kids
                .iter()
                .map(|_| {
                    let mut t = vec![0.0; nm];
                    t[*m as usize] = e.totals[*m as usize] / 4.0;
                    Entry { state: State::Uniform(*m), totals: t }
                })
                .collect(),
            State::Untouched => {
                let ests: Vec<Vec<f64>> = kids
                    .iter()
                    .map(|&c| {
                        let mut t = totals::estimate(&self.planet, &self.tree, c, child_samples(c.level, self.tree.root_level).min(c.side())).cells;
                        t[0] = 0.0;
                        t
                    })
                    .collect();
                let holders: Vec<bool> = ests.iter().map(|t| nonvacuum(t) > 0.0).collect();
                let n_holders = holders.iter().filter(|h| **h).count();
                let mut shares = vec![vec![0.0; nm]; 4];
                for m in 1..nm {
                    let want = e.totals[m];
                    if want == 0.0 {
                        continue;
                    }
                    let sum: f64 = ests.iter().map(|t| t[m]).sum();
                    for k in 0..4 {
                        // Scale the children's estimates to add up to the parent.
                        // A material the children's samples all missed is spread
                        // evenly over the children that hold anything at all.
                        shares[k][m] = if sum > 0.0 {
                            want * ests[k][m] / sum
                        } else if n_holders == 0 {
                            want / 4.0
                        } else if holders[k] {
                            want / n_holders as f64
                        } else {
                            0.0
                        };
                    }
                }
                kids.iter()
                    .zip(shares)
                    .map(|(&c, t)| {
                        let (x0, x1, y0, y1) = self.tree.bounds(c);
                        let outside = {
                            let nx = 0i64.clamp(x0, x1 - 1) as f64;
                            let ny = 0i64.clamp(y0, y1 - 1) as f64;
                            nx * nx + ny * ny > (self.planet.max_radius + 2.0).powi(2)
                        };
                        let state = if outside && nonvacuum(&t) == 0.0 { State::Empty } else { State::Untouched };
                        Entry { state, totals: t }
                    })
                    .collect()
            }
            State::Dense(_) => panic!("split: a leaf has no children"),
        };
        for (c, ce) in kids.iter().zip(child_entries) {
            self.nodes.insert(key(*c), ce);
        }
        self.nodes.get_mut(&key(n)).unwrap().state = State::Split;
    }

    /// Add `delta` cells of material `m` to every ancestor of `n` (not `n` itself).
    fn adjust_ancestors(&mut self, n: Node, m: usize, delta: f64) {
        let mut p = self.parent(n);
        while let Some(pn) = p {
            self.nodes.get_mut(&key(pn)).unwrap().totals[m] += delta;
            p = self.parent(pn);
        }
    }

    /// Turn a leaf into Dense cells. Generating it exactly for the first time
    /// reconciles its estimated share with its real content.
    fn materialize(&mut self, leaf: Node) {
        // Already cell by cell: nothing to do (and no copy of its cells).
        if matches!(self.nodes.get(&key(leaf)).unwrap().state, State::Dense(_)) {
            return;
        }
        let e = self.nodes.get(&key(leaf)).unwrap().clone();
        let nm = self.nm();
        let (x0, _, y0, _) = self.tree.bounds(leaf);
        let cells: Box<[MatId]> = match e.state {
            State::Dense(_) => return,
            State::Empty => vec![VACUUM; (LEAF * LEAF) as usize].into_boxed_slice(),
            State::Uniform(m) => vec![m; (LEAF * LEAF) as usize].into_boxed_slice(),
            State::Untouched => {
                let mut c = Vec::with_capacity((LEAF * LEAF) as usize);
                for y in y0..y0 + LEAF {
                    for x in x0..x0 + LEAF {
                        c.push(self.planet.material(x, y));
                    }
                }
                c.into_boxed_slice()
            }
            State::Split => panic!("materialize: leaves never split"),
        };
        let mut counts = vec![0.0; nm];
        for &m in cells.iter() {
            counts[m as usize] += 1.0;
        }
        counts[0] = 0.0;
        self.ledger.generated += counts.iter().enumerate().map(|(i, c)| c * self.planet.mats.get(i as crate::materials::MatId).density).sum::<f64>();
        for m in 1..nm {
            let delta = counts[m] - e.totals[m];
            if delta != 0.0 {
                self.ledger.reconcile(&self.planet.mats, m as crate::materials::MatId, delta);
                self.adjust_ancestors(leaf, m, delta);
            }
        }
        self.nodes.insert(key(leaf), Entry { state: State::Dense(cells), totals: counts });
    }

    /// Walk from the root to the leaf holding (x, y), opening nodes on the way.
    fn leaf_for_edit(&mut self, x: i64, y: i64) -> Option<Node> {
        // Fast path: a leaf already generated cell by cell has every ancestor
        // split, so there is nothing to open on the way.
        let leaf = self.node_at(0, x, y)?;
        if let Some(e) = self.nodes.get(&key(leaf)) {
            if matches!(e.state, State::Dense(_)) {
                return Some(leaf);
            }
        }
        let mut level = self.tree.root_level;
        loop {
            let n = self.node_at(level, x, y)?;
            if level == 0 {
                self.materialize(n);
                return Some(n);
            }
            if self.nodes.get(&key(n)).unwrap().state != State::Split {
                self.split(n);
            }
            level -= 1;
        }
    }

    /// Mine one cell. Returns what was there, or None if it was already empty.
    pub fn mine_cell(&mut self, x: i64, y: i64) -> Option<MatId> {
        let leaf = self.node_at(0, x, y)?;
        // Fast path: nothing to mine in a region already known to be empty.
        if self.material(x, y) == VACUUM {
            return None;
        }
        let leaf2 = self.leaf_for_edit(x, y)?;
        debug_assert_eq!(leaf, leaf2);
        let (x0, _, y0, _) = self.tree.bounds(leaf);
        let i = ((y - y0) * LEAF + (x - x0)) as usize;
        let e = self.nodes.get_mut(&key(leaf)).unwrap();
        let m = match &mut e.state {
            State::Dense(c) => {
                let m = c[i];
                c[i] = VACUUM;
                m
            }
            _ => unreachable!(),
        };
        e.totals[m as usize] -= 1.0;
        self.adjust_ancestors(leaf, m as usize, -1.0);
        self.mark(x, y);
        self.ledger.mine(&self.planet.mats, m, 1.0);
        self.ledger.cell_ops += 1;
        self.collapse_from(leaf);
        Some(m)
    }

    /// Write material `to` into cell (x, y), keeping every node's counts right.
    /// No booking in the ledger: callers move or change matter, never make it.
    fn put(&mut self, x: i64, y: i64, to: MatId) -> Option<(Node, MatId)> {
        let leaf = self.leaf_for_edit(x, y)?;
        self.mark(x, y);
        let (x0, _, y0, _) = self.tree.bounds(leaf);
        let i = ((y - y0) * LEAF + (x - x0)) as usize;
        let e = self.nodes.get_mut(&key(leaf)).unwrap();
        let from = match &mut e.state {
            State::Dense(c) => std::mem::replace(&mut c[i], to),
            _ => unreachable!(),
        };
        if from != to {
            e.totals[from as usize] -= 1.0;
            e.totals[to as usize] += 1.0;
            // One walk up for both materials (each count changes exactly as
            // two separate walks would change it).
            let mut p = self.parent(leaf);
            while let Some(pn) = p {
                let t = &mut self.nodes.get_mut(&key(pn)).unwrap().totals;
                t[from as usize] -= 1.0;
                t[to as usize] += 1.0;
                p = self.parent(pn);
            }
        }
        Some((leaf, from))
    }

    /// True if (x, y) is inside the stored world (cells beyond it are open space).
    pub fn in_bounds(&self, x: i64, y: i64) -> bool {
        self.node_at(0, x, y).is_some()
    }

    /// Swap the contents of two cells: matter moves, none is made or lost.
    pub fn swap_cells(&mut self, a: (i64, i64), b: (i64, i64)) -> bool {
        let (ma, mb) = (self.material(a.0, a.1), self.material(b.0, b.1));
        if ma == mb || !self.in_bounds(a.0, a.1) || !self.in_bounds(b.0, b.1) {
            return false;
        }
        // A swap moves one cell of each material between the two leaves, so
        // only the nodes holding one leaf and not the other change: below
        // their first shared ancestor (none at all within one leaf). Leaving
        // shared nodes alone also keeps them free of round-off from -1 +1.
        // (Only when both leaves are already cell by cell: generating one
        // books its first count against the ancestors, and that has to come
        // in its usual place among the other changes.)
        let dense = |w: &Self, c: (i64, i64)| w.node_at(0, c.0, c.1).and_then(|n| w.nodes.get(&key(n))).is_some_and(|e| matches!(e.state, State::Dense(_)));
        if !dense(self, a) || !dense(self, b) {
            let (la, _) = self.put(a.0, a.1, mb).unwrap();
            let (lb, _) = self.put(b.0, b.1, ma).unwrap();
            self.collapse_from(la);
            if lb != la {
                self.collapse_from(lb);
            }
            return true;
        }
        let la = self.node_at(0, a.0, a.1).unwrap();
        let lb = self.node_at(0, b.0, b.1).unwrap();
        self.mark(a.0, a.1);
        self.mark(b.0, b.1);
        for (leaf, (x, y), to) in [(la, a, mb), (lb, b, ma)] {
            let (x0, _, y0, _) = self.tree.bounds(leaf);
            let i = ((y - y0) * LEAF + (x - x0)) as usize;
            let e = self.nodes.get_mut(&key(leaf)).unwrap();
            let from = match &mut e.state {
                State::Dense(c) => std::mem::replace(&mut c[i], to),
                _ => unreachable!(),
            };
            if la != lb {
                e.totals[from as usize] -= 1.0;
                e.totals[to as usize] += 1.0;
            }
        }
        let (mut pa, mut pb) = (self.parent(la), self.parent(lb));
        while let (Some(na), Some(nb)) = (pa, pb) {
            if na == nb {
                break;
            }
            let t = &mut self.nodes.get_mut(&key(na)).unwrap().totals;
            t[ma as usize] -= 1.0;
            t[mb as usize] += 1.0;
            let t = &mut self.nodes.get_mut(&key(nb)).unwrap().totals;
            t[mb as usize] -= 1.0;
            t[ma as usize] += 1.0;
            pa = self.parent(na);
            pb = self.parent(nb);
        }
        // A leaf can only have emptied if vacuum moved into it.
        if mb == VACUUM {
            self.collapse_from(la);
        }
        if ma == VACUUM && lb != la {
            self.collapse_from(lb);
        }
        true
    }

    /// Change several cells at once (a reaction or phase change). The cells'
    /// total mass before and after must match: matter changes form, never amount.
    pub fn change_cells(&mut self, cells: &[((i64, i64), MatId)]) {
        let (mut before, mut after) = (0.0, 0.0);
        let olds: Vec<MatId> = cells.iter().map(|((x, y), _)| self.material(*x, *y)).collect();
        for (((_, _), to), from) in cells.iter().zip(&olds) {
            before += self.planet.mats.get(*from).density;
            after += self.planet.mats.get(*to).density;
        }
        assert!((before - after).abs() < 1e-9, "a change must keep mass: {before} kg became {after} kg");
        for (((x, y), to), from) in cells.iter().zip(olds) {
            if from == *to {
                continue;
            }
            let (leaf, _) = self.put(*x, *y, *to).unwrap();
            self.ledger.change(&self.planet.mats, from, *to);
            self.collapse_from(leaf);
        }
    }

    /// A gas cell drifting off into space: it leaves the body for good, and the
    /// ledger books it as vented.
    pub fn vent_cell(&mut self, x: i64, y: i64) -> Option<MatId> {
        let m = self.material(x, y);
        if m == VACUUM {
            return None;
        }
        let (leaf, _) = self.put(x, y, VACUUM)?;
        self.ledger.vented[m as usize] += self.planet.mats.get(m).density;
        self.collapse_from(leaf);
        Some(m)
    }

    /// Lift a cell out of the grid into a flying rigid body: the cell becomes
    /// open space and its mass is booked as aloft until it lands.
    pub fn lift_cell(&mut self, x: i64, y: i64) -> Option<MatId> {
        let m = self.material(x, y);
        if m == VACUUM {
            return None;
        }
        let (leaf, _) = self.put(x, y, VACUUM)?;
        self.ledger.aloft[m as usize] += self.planet.mats.get(m).density;
        self.collapse_from(leaf);
        Some(m)
    }

    /// A flying cell drifted off into space for good: booked as vented.
    pub fn lose_aloft(&mut self, m: MatId) {
        let d = self.planet.mats.get(m).density;
        self.ledger.aloft[m as usize] -= d;
        self.ledger.vented[m as usize] += d;
    }

    /// Set a flying cell down into open cell (x, y).
    pub fn land_cell(&mut self, x: i64, y: i64, m: MatId) -> bool {
        if self.material(x, y) != VACUUM || !self.in_bounds(x, y) {
            return false;
        }
        self.put(x, y, m);
        self.ledger.aloft[m as usize] -= self.planet.mats.get(m).density;
        true
    }

    /// Add matter from outside the body (a wrecked drone) into open cell (x, y).
    pub fn deposit_cell(&mut self, x: i64, y: i64, m: MatId) -> bool {
        if self.material(x, y) != VACUUM || !self.in_bounds(x, y) {
            return false;
        }
        self.put(x, y, m);
        self.ledger.deposited[m as usize] += self.planet.mats.get(m).density;
        true
    }

    /// Turn one cell into another material of the same density in place
    /// (lava cooling to rock). Returns what was there.
    pub fn change_cell(&mut self, x: i64, y: i64, to: MatId) -> Option<MatId> {
        let from = self.material(x, y);
        if from == VACUUM || from == to {
            return None;
        }
        assert!(
            (self.planet.mats.get(from).density - self.planet.mats.get(to).density).abs() < 1e-9,
            "a cell can only change into a material of the same density"
        );
        let leaf = self.leaf_for_edit(x, y)?;
        let (x0, _, y0, _) = self.tree.bounds(leaf);
        let i = ((y - y0) * LEAF + (x - x0)) as usize;
        let e = self.nodes.get_mut(&key(leaf)).unwrap();
        if let State::Dense(c) = &mut e.state {
            c[i] = to;
        }
        e.totals[from as usize] -= 1.0;
        e.totals[to as usize] += 1.0;
        self.adjust_ancestors(leaf, from as usize, -1.0);
        self.adjust_ancestors(leaf, to as usize, 1.0);
        self.mark(x, y);
        self.ledger.change(&self.planet.mats, from, to);
        Some(from)
    }

    /// Mine a whole node at once (what off-screen swarm packets do). Pays out
    /// whatever the node still holds and leaves it Empty. Returns kg per material.
    pub fn mine_node(&mut self, n: Node) -> Vec<f64> {
        // Open every ancestor so this node has its own entry.
        let mut path = Vec::new();
        let mut p = self.parent(n);
        while let Some(pn) = p {
            path.push(pn);
            p = self.parent(pn);
        }
        for a in path.iter().rev() {
            if self.nodes.get(&key(*a)).unwrap().state != State::Split {
                self.split(*a);
            }
        }
        let e = self.nodes.get(&key(n)).unwrap().clone();
        let nm = self.nm();
        let mut paid = vec![0.0; nm];
        for m in 1..nm {
            let c = e.totals[m];
            if c != 0.0 {
                self.ledger.mine(&self.planet.mats, m as crate::materials::MatId, c);
                self.adjust_ancestors(n, m, -c);
                paid[m] = c * self.planet.mats.get(m as crate::materials::MatId).density;
            }
        }
        self.ledger.node_ops += 1;
        if self.track.is_some() {
            let (x0, x1, y0, y1) = self.tree.bounds(n);
            self.track.as_mut().unwrap().rects.push([x0, x1, y0, y1]);
        }
        self.remove_descendants(n);
        self.nodes.insert(key(n), Entry { state: State::Empty, totals: vec![0.0; nm] });
        self.collapse_from(n);
        paid
    }

    fn remove_descendants(&mut self, n: Node) {
        if n.level == 0 {
            return;
        }
        if let Some(e) = self.nodes.get(&key(n)) {
            if e.state == State::Split {
                for c in n.children() {
                    self.remove_descendants(c);
                    self.nodes.remove(&key(c));
                }
            }
        }
    }

    /// A leaf with nothing left becomes Empty; four Empty children merge upward.
    fn collapse_from(&mut self, n: Node) {
        if let Some(e) = self.nodes.get_mut(&key(n)) {
            if matches!(e.state, State::Dense(_)) && !holds_any(&e.totals) {
                e.state = State::Empty;
            }
            // Only an Empty node can start a merge upward.
            if e.state != State::Empty {
                return;
            }
        }
        let mut cur = n;
        while let Some(p) = self.parent(cur) {
            let all_empty = p.children().iter().all(|c| matches!(self.nodes.get(&key(*c)).map(|e| &e.state), Some(State::Empty)));
            if !all_empty {
                break;
            }
            for c in p.children() {
                self.nodes.remove(&key(c));
            }
            let pe = self.nodes.get_mut(&key(p)).unwrap();
            pe.state = State::Empty;
            cur = p;
        }
    }

    /// kg per material still in the body, read from the root.
    pub fn remaining_kg(&self) -> Vec<f64> {
        let t = &self.nodes[&key(self.tree.root())].totals;
        t.iter().enumerate().map(|(i, c)| c * self.planet.mats.get(i as crate::materials::MatId).density).collect()
    }

    /// Cells of each material still in node `n` (from its entry, or its
    /// nearest ancestor's share if it has none yet).
    pub fn node_totals(&self, n: Node) -> Option<&Vec<f64>> {
        self.nodes.get(&key(n)).map(|e| &e.totals)
    }

    /// One number that changes if anything about the world or the books does.
    pub fn state_hash(&self) -> u64 {
        let mut keys: Vec<&u64> = self.nodes.keys().collect();
        keys.sort();
        let mut h = hash(self.planet.spec.seed, 0, 0, 0xD7);
        let mut mix = |v: u64| h = hash(h, v as i64, 0, 1);
        for k in keys {
            let e = &self.nodes[k];
            mix(*k);
            match &e.state {
                State::Untouched => mix(1),
                State::Split => mix(2),
                State::Empty => mix(3),
                State::Uniform(m) => mix(4 + ((*m as u64) << 8)),
                State::Dense(c) => {
                    mix(5);
                    for ch in c.chunks(8) {
                        let mut v = 0u64;
                        for (i, b) in ch.iter().enumerate() {
                            v |= (*b as u64) << (8 * i);
                        }
                        mix(v);
                    }
                }
            }
            for t in &e.totals {
                mix(t.to_bits());
            }
        }
        let l = &self.ledger;
        for v in l.initial.iter().chain(&l.mined).chain(&l.reconciled) {
            mix(v.to_bits());
        }
        mix(l.credited.to_bits());
        mix(l.bonus.to_bits());
        mix(l.generated.to_bits());
        mix(l.cell_ops);
        mix(l.node_ops);
        h
    }

    /// Entry counts by state, and bytes held by dense leaves.
    pub fn stats(&self) -> (usize, usize, usize, usize, usize) {
        let (mut untouched, mut split, mut dense, mut empty) = (0, 0, 0, 0);
        for e in self.nodes.values() {
            match e.state {
                State::Untouched => untouched += 1,
                State::Split => split += 1,
                State::Dense(_) => dense += 1,
                State::Empty | State::Uniform(_) => empty += 1,
            }
        }
        (untouched, split, dense, empty, dense * (LEAF * LEAF) as usize)
    }
}

/// Fast repeated lookups near one another (flood fills, collider building):
/// remembers the last generated leaf it read from.
pub struct Reader<'a> {
    w: &'a World,
    leaf: Option<(i64, i64, &'a [MatId])>,
}

impl<'a> Reader<'a> {
    /// True if the leaf holding (x, y) has been generated cell by cell
    /// (something has happened near it).
    pub fn touched(&mut self, x: i64, y: i64) -> bool {
        let o = self.w.tree.origin;
        let (ax, ay) = ((x - o).div_euclid(LEAF) * LEAF + o, (y - o).div_euclid(LEAF) * LEAF + o);
        if let Some((lx, ly, _)) = self.leaf {
            if lx == ax && ly == ay {
                return true;
            }
        }
        match self.w.dense_leaf(ax, ay) {
            Some(c) => {
                self.leaf = Some((ax, ay, c));
                true
            }
            None => false,
        }
    }

    pub fn get(&mut self, x: i64, y: i64) -> MatId {
        // Leaves sit on a grid offset by the tree origin.
        let o = self.w.tree.origin;
        let (ax, ay) = ((x - o).div_euclid(LEAF) * LEAF + o, (y - o).div_euclid(LEAF) * LEAF + o);
        if let Some((lx, ly, c)) = self.leaf {
            if lx == ax && ly == ay {
                return c[((y - ay) * LEAF + (x - ax)) as usize];
            }
        }
        if let Some(c) = self.w.dense_leaf(ax, ay) {
            self.leaf = Some((ax, ay, c));
            return c[((y - ay) * LEAF + (x - ax)) as usize];
        }
        self.w.material(x, y)
    }
}
