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
    /// Living plant tissue, by material (empty: nobody is watching). Cells
    /// of it taken away or set down by a falling body are noted in
    /// `life_notes` for the life cycle (src/life.rs) to act on. Not part of
    /// the world's state or its hash.
    pub life_watch: Vec<bool>,
    pub life_notes: LifeNotes,
    /// How cells look beyond their material: a blast's marks (see `Look`),
    /// by leaf, then by cell. Gone when the cell changes. Not part of the
    /// world's state or its hash (the simulation saves them).
    pub looks: HashMap<(i64, i64), HashMap<(i64, i64), u8>>,
    /// What the generator gave for untouched cells looked up lately (it is
    /// the slow part of a lookup there, and the same cells are asked about
    /// over and over: water resting in a lake, ground round a flock). An
    /// untouched cell always generates the same, so this never goes stale.
    /// Not part of the world's state or its hash.
    pub gen_cache: GenCache,
    /// Ground eaten out of sight (see `FarCut`): everything further out than
    /// a sector's level is gone. Part of the world's state.
    pub far: FarCut,
}

/// Ground eaten out of sight, a whole stretch of the surface at a time (the
/// swarm's far field, src/far.rs): the body is cut into sectors by angle,
/// and in each one every cell further from the centre than its level is
/// gone. Cells generated cell by cell are emptied when the level comes down
/// past them (`World::far_lower`); untouched ones (still as generated) read
/// as empty above it, so nothing has to be generated to eat it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FarCut {
    /// Each sector's level (distance from the centre, m); INFINITY: not cut.
    /// Sector k covers angles [k, k+1) * TAU / n from +x, counter-clockwise.
    pub levels: Vec<f32>,
    /// Whether any sector is cut (the lookups skip the rest when not).
    pub any: bool,
    /// Bumped whenever a level changes (for a renderer's copy). Not part of
    /// the world's state.
    pub version: u64,
    /// Squared distance from the centre under which nothing is cut (the
    /// lowest level less the deepest pit), for a quick answer.
    pub floor2: f64,
    /// Sectors far from the camera now (the swarm there works as a count,
    /// and loose matter there waits until someone looks: see the sandbox).
    /// Not part of the world's state.
    pub asleep: Vec<bool>,
}

/// How far a far sector's eaten face rises toward a higher neighbour (m),
/// and how deep its pits go below its level.
pub const FAR_RAMP_M: f64 = 400.0;
pub const FAR_PIT_M: f64 = 24.0;

/// Smooth noise in 0..1 along a line (value noise, eased).
#[inline]
fn value_noise(s: f64, salt: u64) -> f64 {
    let i = s.floor();
    let f = s - i;
    let h = |k: f64| (crate::noise::hash(0xFA2C_17, k as i64, 0, salt) >> 11) as f64 / (1u64 << 53) as f64;
    let u = f * f * (3.0 - 2.0 * f);
    h(i) * (1.0 - u) + h(i + 1.0) * u
}

impl FarCut {
    /// Sectors round a body this big: about `width_m` of surface each.
    pub fn new(radius_m: f64, width_m: f64) -> Self {
        let n = ((std::f64::consts::TAU * radius_m / width_m.max(1.0)).round() as usize).max(16);
        FarCut { levels: vec![f32::INFINITY; n], any: false, version: 0, floor2: f64::INFINITY, asleep: Vec::new() }
    }

    /// The sector holding direction (x, y) from the centre.
    #[inline]
    pub fn sector_of(&self, x: f64, y: f64) -> usize {
        let n = self.levels.len();
        let t = y.atan2(x) / std::f64::consts::TAU;
        let t = if t < 0.0 { t + 1.0 } else { t };
        ((t * n as f64) as usize).min(n - 1)
    }

    /// The level of the sector holding (x, y) (INFINITY: not cut).
    #[inline]
    pub fn level_at(&self, x: f64, y: f64) -> f64 {
        if !self.any {
            return f64::INFINITY;
        }
        self.levels[self.sector_of(x, y)] as f64
    }

    /// The highest anything stands in direction (x, y) of a cut sector (its
    /// level, or where a ramp up to a neighbour reaches; INFINITY: not cut).
    #[inline]
    pub fn top_at(&self, x: f64, y: f64) -> f64 {
        if !self.any {
            return f64::INFINITY;
        }
        let n = self.levels.len();
        let k = self.sector_of(x, y);
        let own = self.levels[k] as f64;
        if !own.is_finite() {
            return f64::INFINITY;
        }
        let (l, r) = (self.levels[(k + n - 1) % n] as f64, self.levels[(k + 1) % n] as f64);
        own.max(l.max(r).min(own + FAR_RAMP_M))
    }

    /// Whether cell (x, y) lies in a sector far from the camera.
    #[inline]
    pub fn asleep_at(&self, x: i64, y: i64) -> bool {
        if self.asleep.is_empty() {
            return false;
        }
        let k = self.sector_of(x as f64 + 0.5, y as f64 + 0.5);
        self.asleep.get(k).copied().unwrap_or(false)
    }

    /// Whether cell (x, y) has been eaten out of sight: further out than
    /// its sector's level where it stands. Not one flat level: toward a
    /// higher neighbour the ground rises to meet it (a ramp over half the
    /// sector, never a wall), and the eaten face is pitted as flocks leave
    /// it (wide shallow bowls, small bites). The ramp only ever raises a
    /// sector's own level, so a sector is never cut lower by its
    /// neighbours.
    #[inline]
    pub fn cut(&self, x: i64, y: i64) -> bool {
        self.state(x, y) == 2
    }

    /// Cell (x, y) as the far field leaves it: 0 in a sector not cut (or
    /// below anything cut), 1 left standing in a cut sector (its plants
    /// went with the surface eaten above it), 2 eaten.
    #[inline]
    pub fn state(&self, x: i64, y: i64) -> u8 {
        if !self.any {
            return 0;
        }
        let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
        if fx * fx + fy * fy < self.floor2 {
            return 0;
        }
        let n = self.levels.len();
        let t = fy.atan2(fx) / std::f64::consts::TAU;
        let t = (if t < 0.0 { t + 1.0 } else { t }) * n as f64;
        let k = (t as usize).min(n - 1);
        let own = self.levels[k] as f64;
        if !own.is_finite() {
            return 0;
        }
        let r2 = fx * fx + fy * fy;
        // (Quick answers well clear of the face: above where a ramp up to
        // either neighbour could reach.)
        let (l, r) = (self.levels[(k + n - 1) % n] as f64, self.levels[(k + 1) % n] as f64);
        let top = own.max(l.max(r).min(own + FAR_RAMP_M));
        if r2 > top * top {
            return 2;
        }
        let low = own - FAR_PIT_M;
        if r2 < low * low {
            return 1;
        }
        let f = t - k as f64;
        let (j, w) = if f < 0.5 { ((k + n - 1) % n, (0.5 - f) * 2.0) } else { ((k + 1) % n, (f - 0.5) * 2.0) };
        let other = (self.levels[j] as f64).min(own + FAR_RAMP_M);
        let w = w * w * (3.0 - 2.0 * w);
        let lvl = if other > own { own + (other - own) * w } else { own };
        // Pits: along the surface (metres round the body, the same either
        // side of a sector's edge), bowls and bites of several sizes.
        let s = t / n as f64 * std::f64::consts::TAU * own.max(1.0);
        let pit = FAR_PIT_M * (0.45 * value_noise(s / 300.0, 1) + 0.3 * value_noise(s / 80.0, 2) + 0.17 * value_noise(s / 15.0, 3) + 0.08 * value_noise(s / 4.0, 4));
        if r2.sqrt() > lvl - pit { 2 } else { 1 }
    }
}

/// See `World::gen_cache`: split in parts by cell, each with its own lock,
/// so the sandbox's threads rarely wait on one another.
pub struct GenCache {
    parts: Vec<std::sync::Mutex<crate::swarm::CellMap<MatId>>>,
}

impl Default for GenCache {
    fn default() -> Self {
        GenCache { parts: (0..GEN_PARTS).map(|_| Default::default()).collect() }
    }
}

/// Parts of the generator cache, and the most cells each keeps (it is
/// emptied when full: about 40 MB in all).
const GEN_PARTS: usize = 16;
const GEN_PART_MAX: usize = 1 << 17;

impl GenCache {
    fn part(&self, x: i64, y: i64) -> &std::sync::Mutex<crate::swarm::CellMap<MatId>> {
        let h = (x.wrapping_mul(0x9E37_79B9) ^ y.wrapping_mul(0x85EB_CA6B)) as u64;
        &self.parts[((h >> 7) as usize) % GEN_PARTS]
    }

    fn get(&self, x: i64, y: i64, make: impl FnOnce() -> MatId) -> MatId {
        let p = self.part(x, y);
        if let Some(&m) = p.lock().unwrap().get(&(x, y)) {
            return m;
        }
        let m = make();
        let mut g = p.lock().unwrap();
        if g.len() >= GEN_PART_MAX {
            g.clear();
        }
        g.insert((x, y), m);
        m
    }
}

/// How a cell looks beyond its material, as the sapper proving ground draws
/// what a blast leaves: cracked through, a big piece held together, loose
/// rubble, rubble baked by the heat, molten, and melt cooled to glass.
pub struct Look;
impl Look {
    pub const CRACKED: u8 = 1;
    pub const HELD: u8 = 2;
    pub const RUBBLE: u8 = 3;
    pub const BAKED: u8 = 4;
    pub const MELT: u8 = 5;
    pub const GLASS: u8 = 6;
}

/// What happened to living plant cells since the life cycle last looked.
#[derive(Default, Debug)]
pub struct LifeNotes {
    /// Cells of living tissue taken away (cut, burned, eaten, blown up).
    pub removed: Vec<(i64, i64, MatId)>,
    /// Living tissue set down by a falling body: cut off, it will die.
    pub landed: Vec<(i64, i64, MatId)>,
}

/// Notes kept at most (nobody may be reading them).
const LIFE_NOTES_MAX: usize = 200_000;

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
        World { ledger: Ledger::new(initial_kg), planet, tree, nodes, track: None, life_watch: Vec::new(), life_notes: LifeNotes::default(), looks: HashMap::new(), gen_cache: GenCache::default(), far: FarCut::default() }
    }

    /// Note that cell (x, y) changed (if anyone is keeping track).
    #[inline]
    fn mark(&mut self, x: i64, y: i64) {
        if let Some(t) = &mut self.track {
            t.blocks.insert((x >> 6, y >> 6));
        }
    }

    /// Give cell (x, y) a look (0: none).
    pub fn set_look(&mut self, x: i64, y: i64, look: u8) {
        let k = self.leaf_of(x, y);
        if look == 0 {
            if let Some(l) = self.looks.get_mut(&k) {
                if l.remove(&(x, y)).is_some() {
                    if l.is_empty() {
                        self.looks.remove(&k);
                    }
                    self.mark(x, y);
                }
            }
            return;
        }
        self.looks.entry(k).or_default().insert((x, y), look);
        self.mark(x, y);
    }

    /// Cell (x, y)'s look (0: none).
    pub fn look(&self, x: i64, y: i64) -> u8 {
        self.looks.get(&self.leaf_of(x, y)).and_then(|l| l.get(&(x, y)).copied()).unwrap_or(0)
    }

    /// A changed cell loses its look.
    #[inline]
    fn clear_look(&mut self, x: i64, y: i64) {
        if !self.looks.is_empty() {
            let k = self.leaf_of(x, y);
            if let Some(l) = self.looks.get_mut(&k) {
                l.remove(&(x, y));
                if l.is_empty() {
                    self.looks.remove(&k);
                }
            }
        }
    }

    #[inline]
    fn watched(&self, m: MatId) -> bool {
        self.life_watch.get(m as usize).copied().unwrap_or(false)
    }

    #[inline]
    fn note_removed(&mut self, x: i64, y: i64, from: MatId) {
        if self.watched(from) && self.life_notes.removed.len() < LIFE_NOTES_MAX {
            self.life_notes.removed.push((x, y, from));
        }
    }

    /// Life changing cell (x, y) to `to`: a plant growing into open space or
    /// water, or tissue wilting and rotting. The difference in mass is
    /// taken from (or given back to) the air and water life lives on, and
    /// booked as grown. Not noted for the life cycle. Returns what was there.
    pub fn life_cell(&mut self, x: i64, y: i64, to: MatId) -> Option<MatId> {
        if !self.in_bounds(x, y) || self.material(x, y) == to {
            return None;
        }
        let watch = std::mem::take(&mut self.life_watch);
        let put = self.put(x, y, to);
        self.life_watch = watch;
        let (leaf, from) = put?;
        let mats = &self.planet.mats;
        self.ledger.grown[from as usize] -= mats.get(from).density;
        self.ledger.grown[to as usize] += mats.get(to).density;
        if to == VACUUM {
            self.collapse_from(leaf);
        }
        Some(from)
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
                    State::Uniform(m) => return if self.far.cut(x, y) { VACUUM } else { *m },
                    State::Untouched => return self.untouched(x, y, bare),
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
                None | Some(State::Untouched) => return self.untouched(x, y, bare),
                Some(State::Empty) => return VACUUM,
                Some(State::Uniform(m)) => return if self.far.cut(x, y) { VACUUM } else { *m },
                Some(State::Dense(c)) => {
                    let (x0, _, y0, _) = self.tree.bounds(n);
                    return c[((y - y0) * LEAF + (x - x0)) as usize];
                }
                Some(State::Split) => level -= 1,
            }
        }
    }

    /// An untouched cell: as generated (plants and all unless `bare`), less
    /// what the far field ate (and, in a sector it cut, without plants:
    /// they went with the surface).
    #[inline]
    fn untouched(&self, x: i64, y: i64, bare: bool) -> MatId {
        match self.far.state(x, y) {
            2 => VACUUM,
            1 => self.planet.base_material(x, y),
            _ if bare => self.planet.base_material(x, y),
            _ => self.gen_cache.get(x, y, || self.planet.material(x, y)),
        }
    }

    /// What occupies cell (x, y), if the store already knows without asking
    /// the generator: None where the cell is still as generated (untouched).
    /// Lets a renderer keep its own cache of generated cells.
    pub fn material_stored(&self, x: i64, y: i64) -> Option<MatId> {
        self.stored(x, y, true)
    }

    /// `material_stored`, with or without ground eaten out of sight (see
    /// `FarCut`) applied to what is still as generated.
    fn stored(&self, x: i64, y: i64, far: bool) -> Option<MatId> {
        let cut = |x: i64, y: i64| far && self.far.cut(x, y);
        if let Some(n) = self.node_at(0, x, y) {
            if let Some(e) = self.nodes.get(&key(n)) {
                match &e.state {
                    State::Dense(c) => {
                        let (x0, _, y0, _) = self.tree.bounds(n);
                        return Some(c[((y - y0) * LEAF + (x - x0)) as usize]);
                    }
                    State::Empty => return Some(VACUUM),
                    State::Uniform(m) => return Some(if cut(x, y) { VACUUM } else { *m }),
                    State::Untouched => return if cut(x, y) { Some(VACUUM) } else { None },
                    State::Split => {}
                }
            }
        }
        let mut level = self.tree.root_level;
        loop {
            let n = self.node_at(level, x, y)?;
            match self.nodes.get(&key(n)).map(|e| &e.state) {
                None | Some(State::Untouched) => return if cut(x, y) { Some(VACUUM) } else { None },
                Some(State::Empty) => return Some(VACUUM),
                Some(State::Uniform(m)) => return Some(if cut(x, y) { VACUUM } else { *m }),
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
        // (Ground eaten out of sight is left to the renderer: see FarCut.)
        self.stored(x0, y0, false).map(LeafSnap::Uniform)
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
            State::Uniform(m) => {
                let mut c = vec![m; (LEAF * LEAF) as usize];
                if self.far.any {
                    for (i, o) in c.iter_mut().enumerate() {
                        if self.far.cut(x0 + i as i64 % LEAF, y0 + i as i64 / LEAF) {
                            *o = VACUUM;
                        }
                    }
                }
                c.into_boxed_slice()
            }
            State::Untouched => {
                // Row by row side by side (the generator only reads, and
                // is the same on any thread: a leaf is 4,096 of its calls,
                // most of the cost of the first cut in fresh ground).
                use rayon::prelude::*;
                let planet = &*self.planet;
                let far = &self.far;
                let mut c = vec![VACUUM; (LEAF * LEAF) as usize];
                c.par_chunks_mut(LEAF as usize).enumerate().for_each(|(row, out)| {
                    let y = y0 + row as i64;
                    for (i, o) in out.iter_mut().enumerate() {
                        let x = x0 + i as i64;
                        *o = match far.state(x, y) {
                            2 => VACUUM,
                            1 => planet.base_material(x, y),
                            _ => planet.material(x, y),
                        };
                    }
                });
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
        self.note_removed(x, y, m);
        self.clear_look(x, y);
        self.ledger.mine(&self.planet.mats, m, 1.0);
        self.ledger.cell_ops += 1;
        self.collapse_from(leaf);
        Some(m)
    }

    /// Sector k's level comes down to `to`, from `from` (its old level, or
    /// the highest ground in it when first cut): every cell already
    /// generated cell by cell further out than that, in the sector, is
    /// emptied, and the cells emptied are returned (per material) for the
    /// swarm to book as eaten out of sight. Untouched cells need nothing:
    /// they read as empty above the level (see `FarCut`).
    pub fn far_lower(&mut self, k: usize, from: f64, to: f64) -> Vec<f64> {
        let nm = self.nm();
        let mut eaten = vec![0.0; nm];
        let n = self.far.levels.len();
        if k >= n {
            return eaten;
        }
        self.far.levels[k] = to as f32;
        self.far.any = true;
        self.far.version += 1;
        let low = (to - FAR_PIT_M - 1.0).max(0.0);
        self.far.floor2 = self.far.floor2.min(low * low);
        // The band's box: the sector's two edges and middle, at both radii,
        // and half its neighbours (whose ramps lean on its level).
        let tau = std::f64::consts::TAU;
        let (a0, a1) = (tau * (k as f64 - 0.5) / n as f64, tau * (k as f64 + 1.5) / n as f64);
        let to = to - FAR_PIT_M;
        // (Up to where its neighbours' ramps reach, at most.)
        let hi = |j: usize| self.far.levels[j] as f64;
        let from = from.max(hi((k + n - 1) % n).min(from + FAR_RAMP_M)).max(hi((k + 1) % n).min(from + FAR_RAMP_M)) + 1.0;
        let (mut bx0, mut bx1, mut by0, mut by1) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
        for a in [a0, a1, 0.5 * (a0 + a1)] {
            for r in [to - 1.0, from + 1.0] {
                let (x, y) = (a.cos() * r, a.sin() * r);
                bx0 = bx0.min(x);
                bx1 = bx1.max(x);
                by0 = by0.min(y);
                by1 = by1.max(y);
            }
        }
        let (x0, x1, y0, y1) = (bx0.floor() as i64 - 1, bx1.ceil() as i64 + 1, by0.floor() as i64 - 1, by1.ceil() as i64 + 1);
        let (lx0, ly0) = self.leaf_of(x0, y0);
        let (lx1, ly1) = self.leaf_of(x1, y1);
        let to2 = to * to;
        for ly in ly0..=ly1 {
            for lx in lx0..=lx1 {
                let (cx, cy) = self.leaf_corner(lx, ly);
                let Some(leaf) = self.node_at(0, cx, cy) else { continue };
                let far = &self.far;
                let cells: Vec<usize> = match self.nodes.get(&key(leaf)).map(|e| &e.state) {
                    Some(State::Dense(c)) => (0..(LEAF * LEAF) as usize)
                        .filter(|&i| c[i] != VACUUM)
                        .filter(|&i| {
                            let (x, y) = (cx + i as i64 % LEAF, cy + i as i64 / LEAF);
                            let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
                            fx * fx + fy * fy > to2 && far.cut(x, y)
                        })
                        .collect(),
                    _ => continue,
                };
                if cells.is_empty() {
                    continue;
                }
                let e = self.nodes.get_mut(&key(leaf)).unwrap();
                let mut gone = vec![0.0; nm];
                let mut taken = Vec::with_capacity(cells.len());
                if let State::Dense(c) = &mut e.state {
                    for &i in &cells {
                        gone[c[i] as usize] += 1.0;
                        taken.push((cx + i as i64 % LEAF, cy + i as i64 / LEAF, c[i]));
                        c[i] = VACUUM;
                    }
                }
                for m in 1..nm {
                    e.totals[m] -= gone[m];
                }
                for m in 1..nm {
                    if gone[m] != 0.0 {
                        self.adjust_ancestors(leaf, m, -gone[m]);
                        eaten[m] += gone[m];
                    }
                }
                for &(x, y, m) in &taken {
                    self.mark(x, y);
                    self.note_removed(x, y, m);
                    self.clear_look(x, y);
                }
                self.ledger.cell_ops += taken.len() as u64;
                self.collapse_from(leaf);
            }
        }
        // (A renderer learns of untouched ground eaten from the levels
        // themselves: see `version`. Cells emptied above were marked.)
        eaten
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
            self.note_removed(x, y, from);
            self.clear_look(x, y);
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
        // (A look goes with the cell that had it.)
        if !self.looks.is_empty() {
            let (la, lb) = (self.look(a.0, a.1), self.look(b.0, b.1));
            if la != lb {
                self.set_look(a.0, a.1, lb);
                self.set_look(b.0, b.1, la);
            }
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

    /// A flying cell taken out of the air by a drone: booked as mined.
    pub fn mine_aloft(&mut self, m: MatId) {
        self.ledger.aloft[m as usize] -= self.planet.mats.get(m).density;
        self.ledger.mine(&self.planet.mats, m, 1.0);
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
        if self.watched(m) && self.life_notes.landed.len() < LIFE_NOTES_MAX {
            self.life_notes.landed.push((x, y, m));
        }
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
        self.note_removed(x, y, from);
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
        // (Only once something is eaten out of sight, so fingerprints from
        // before the far field are unchanged.)
        if self.far.any {
            for v in &self.far.levels {
                mix(v.to_bits() as u64);
            }
        }
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
