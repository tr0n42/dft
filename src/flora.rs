//! Life (L1): every body breeds its own flora from the niches its chemistry
//! allows, and grows every plant from a small genome with one growing-tip
//! engine (rules in data/life.toml).
//!
//! Plants are cells of the world like rock: wood, leaf, bloom and the rest,
//! each species with its own copies in its own colours, so cutting, fire,
//! heat and falling work on them unchanged. They are laid over the generator
//! in chunks of 128 x 128 cells, worked out the first time anything looks at
//! a chunk: the anchors in it are found (ground with open space over or
//! under it), each may carry ground cover and a plant, and each plant is
//! grown in one go from its seed. A plant reaches at most REACH cells from
//! its anchor, so a chunk shows the plants rooted in itself and its eight
//! neighbours.
//!
//! Only add, multiply, divide and square roots are used (angles go through
//! `cos_sin`), so a body grows the same flora on every machine.

use crate::materials::{MatId, MaterialTable, State, VACUUM};
use crate::noise::{cos_sin, fbm, hash, unit};
use crate::planet::Planet;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub const CHUNK: i64 = 128;
/// No plant reaches further than this from its anchor, in cells.
pub const REACH: f64 = 120.0;
/// Cells one plant may grow at most.
const BUDGET: usize = 14_000;

// ---- data/life.toml ----

#[derive(Debug, Clone, Deserialize)]
pub struct LifeFile {
    pub depth_m: f64,
    pub sea_light_m: f64,
    pub seep_m: f64,
    pub hot_k: f64,
    pub species: [u32; 2],
    pub soils: Vec<String>,
    pub solvent: Vec<SolventRow>,
    /// The life cycle's timings (src/life.rs).
    #[serde(default)]
    pub cycle: Cycle,
}

/// How fast plants come back and rot away, in seconds of game time.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Cycle {
    /// From a stand being cut, eaten or burned to new growth there, for a
    /// species of growth 1 (faster growers sooner).
    pub regrow_s: f64,
    /// From sprouting to full size, for growth 1.
    pub mature_s: f64,
    /// Living tissue cut off from its plant dies after this.
    pub wilt_s: f64,
    /// Deadwood breaks down into leaf litter after this.
    pub rot_s: f64,
    /// Leaf litter turns to humus after this.
    pub humus_s: f64,
    /// Places waiting to regrow, at most.
    pub max_scars: usize,
    /// Plants growing at once, at most.
    pub max_sprouts: usize,
    /// Dead cells waiting to rot, at most.
    pub max_rotting: usize,
}

impl Default for Cycle {
    fn default() -> Self {
        Cycle { regrow_s: 240.0, mature_s: 900.0, wilt_s: 45.0, rot_s: 600.0, humus_s: 900.0, max_scars: 4000, max_sprouts: 3000, max_rotting: 200_000 }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SolventRow {
    pub name: String,
    pub materials: Vec<String>,
    pub min_k: f64,
    pub max_k: f64,
    #[serde(default)]
    pub glamour: f64,
}

// ---- Niches ----

/// A kind of place a plant can live.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Niche {
    /// Open ground with soil under the sky.
    Soil,
    /// Bare rock and cliffs under the sky.
    Bare,
    /// Ice and frozen ground under the sky.
    Frozen,
    /// A sea floor sunlight still reaches.
    Shallows,
    /// Dark liquid floors: deep sea, buried oceans, cave pools.
    Deeps,
    /// Hot springs, vents and the rims of hot ground.
    Hot,
    /// The floor of a cave.
    CaveFloor,
    /// The roof of a cave: things hang from it.
    CaveRoof,
}

impl Niche {
    pub const ALL: [Niche; 8] = [Niche::Soil, Niche::Bare, Niche::Frozen, Niche::Shallows, Niche::Deeps, Niche::Hot, Niche::CaveFloor, Niche::CaveRoof];

    pub fn name(self) -> &'static str {
        match self {
            Niche::Soil => "soil",
            Niche::Bare => "bare rock",
            Niche::Frozen => "frozen ground",
            Niche::Shallows => "sunlit sea floor",
            Niche::Deeps => "dark liquid floor",
            Niche::Hot => "hot springs and vents",
            Niche::CaveFloor => "cave floor",
            Niche::CaveRoof => "cave roof",
        }
    }

    pub fn sky(self) -> bool {
        matches!(self, Niche::Soil | Niche::Bare | Niche::Frozen)
    }

    pub fn wet(self) -> bool {
        matches!(self, Niche::Shallows | Niche::Deeps)
    }

    pub fn dark(self) -> bool {
        matches!(self, Niche::Deeps | Niche::CaveFloor | Niche::CaveRoof | Niche::Hot)
    }

    /// Plants per anchor cell, and the chance an anchor carries ground cover.
    pub fn density(self) -> (f64, f64) {
        match self {
            Niche::Soil => (0.10, 0.92),
            Niche::Bare => (0.035, 0.45),
            Niche::Frozen => (0.02, 0.3),
            Niche::Shallows => (0.08, 0.55),
            Niche::Deeps => (0.05, 0.45),
            Niche::Hot => (0.12, 0.8),
            Niche::CaveFloor => (0.06, 0.55),
            Niche::CaveRoof => (0.08, 0.5),
        }
    }

    /// Body plans that suit this niche: (plan, weight, lowest glamour).
    fn plans(self) -> &'static [(Plan, f64, f64)] {
        use Plan::*;
        match self {
            Niche::Soil => &[(Tree, 3.0, 0.0), (Shrub, 3.0, 0.0), (Rosette, 2.0, 0.0), (Stalk, 1.5, 0.0), (Vine, 1.0, 0.0), (Cap, 0.5, 0.0), (Lattice, 0.8, 0.7)],
            Niche::Bare => &[(Shrub, 1.5, 0.0), (Rosette, 2.0, 0.0), (Vine, 1.5, 0.0), (Tree, 0.6, 0.0), (Lattice, 1.0, 0.6)],
            Niche::Frozen => &[(Rosette, 1.5, 0.0), (Shrub, 1.0, 0.0), (Cap, 0.5, 0.2), (Lattice, 1.5, 0.5)],
            Niche::Shallows => &[(Tube, 2.0, 0.0), (Vine, 2.5, 0.0), (Rosette, 1.0, 0.0), (Lattice, 1.5, 0.3)],
            Niche::Deeps => &[(Tube, 3.0, 0.0), (Rosette, 1.0, 0.0), (Lattice, 1.5, 0.4), (Cap, 0.7, 0.3)],
            Niche::Hot => &[(Tube, 3.0, 0.0), (Cap, 1.0, 0.0), (Lattice, 1.5, 0.3), (Stalk, 1.0, 0.3)],
            Niche::CaveFloor => &[(Cap, 3.0, 0.0), (Stalk, 1.0, 0.0), (Rosette, 1.0, 0.0), (Tube, 0.8, 0.2), (Lattice, 1.5, 0.5)],
            Niche::CaveRoof => &[(Vine, 3.0, 0.0), (Cap, 1.0, 0.0), (Tube, 1.0, 0.2), (Lattice, 1.0, 0.6)],
        }
    }
}

// ---- The genome ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    Mat,
    Tuft,
    Rosette,
    Vine,
    Shrub,
    Tree,
    Stalk,
    Cap,
    Tube,
    Lattice,
}

impl Plan {
    pub fn name(self) -> &'static str {
        match self {
            Plan::Mat => "mat",
            Plan::Tuft => "tuft",
            Plan::Rosette => "rosette",
            Plan::Vine => "vine",
            Plan::Shrub => "shrub",
            Plan::Tree => "tree",
            Plan::Stalk => "stalk",
            Plan::Cap => "cap",
            Plan::Tube => "tube",
            Plan::Lattice => "lattice",
        }
    }
}

/// The shape a leaf, flower or fruit is stamped in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Organ {
    None,
    /// A round clump.
    Blob,
    /// Half a clump, on the outer side.
    Fan,
    /// A straight line along the way it grows.
    Spike,
    /// A line swept back like a blade of grass.
    Blade,
    /// A small bright knot (often glowing in the dark).
    Lantern,
    /// A hollow ring.
    Bladder,
    /// A spray of short lines.
    Plume,
    /// Two jaws in a V.
    Trap,
    /// A fat seed pod.
    Pod,
}

/// How side shoots and leaves are laid along a stem (the L-system part).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    Random,
    Alternate,
    Opposite,
    Pinnate,
    Whorl,
}

/// The silhouette a big tree's crown fills (space colonisation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Crown {
    Round,
    Cone,
    Umbrella,
    Column,
    Weeping,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Energy {
    Light,
    Chemistry,
    Seepage,
}

/// What hurts a drone. Only things that damage drones count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Defence {
    /// Thorns and spines: wear on any drone that cuts or brushes them.
    Thorns,
    /// Caustic or toxic sap: damage when swallowed, eats hulls.
    Caustic,
    /// A puff of caustic gas when disturbed.
    Gas,
    /// Bursting seed pods: a blast that knocks drones out.
    Pods,
    /// Lashing vines that knock drones out of the air.
    Whip,
    /// Resin that lights when cut near heat.
    Resin,
    /// A snapping trap that catches and digests drones.
    Snap,
    /// A flash that stuns drones in range.
    Flash,
}

impl Defence {
    pub fn name(self) -> &'static str {
        match self {
            Defence::Thorns => "thorns",
            Defence::Caustic => "caustic sap",
            Defence::Gas => "caustic gas",
            Defence::Pods => "bursting pods",
            Defence::Whip => "whipping vines",
            Defence::Resin => "resin",
            Defence::Snap => "snap trap",
            Defence::Flash => "flash",
        }
    }
}

/// The idle motion of a plant (drawn only; the cells never move).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Rigid,
    Sway,
    Ripple,
    Drift,
}

/// One species: its genome and its materials on this body.
#[derive(Clone, Debug)]
pub struct Species {
    pub name: String,
    pub niche: Niche,
    pub plan: Plan,
    /// Ground cover (mats and tufts) rather than a standing plant.
    pub cover: bool,
    pub energy: Energy,
    pub solvent: String,
    /// The temperatures (K) it lives at.
    pub min_k: f64,
    pub max_k: f64,
    // Form.
    pub height: f64,
    pub thick: f64,
    pub taper: f64,
    pub internode: f64,
    pub branch: f64,
    pub angle: f64,
    pub stems: u32,
    pub pattern: Pattern,
    pub crown: Option<Crown>,
    // Steering.
    pub sun: f64,
    pub droop: f64,
    pub hug: f64,
    pub wiggle: f64,
    // Organs.
    pub leaf: Organ,
    pub leaf_r: f64,
    pub tip: Organ,
    pub tip_r: f64,
    pub roots: f64,
    // Life cycle (used by L1e).
    pub growth: f64,
    pub lifespan_s: f64,
    // Defences and motion.
    pub defence: Vec<Defence>,
    pub motion: Motion,
    pub sway: f64,
    // Where and how much.
    pub abundance: f64,
    pub patch_m: f64,
    // Materials (filled in when the body adds them).
    pub stem_m: MatId,
    pub foliage_m: MatId,
    pub organ_m: MatId,
    pub root_m: MatId,
    /// Template material and colour per part: stem, foliage, organ, root.
    pub parts: [(MatId, [u8; 3]); 4],
}

/// What a body's survey found for one niche.
#[derive(Clone, Debug)]
pub struct NicheReport {
    pub niche: Niche,
    pub samples: usize,
    pub low_k: f64,
    pub high_k: f64,
    /// The solvent life uses there, if any fits.
    pub solvent: Option<String>,
    pub energy: Option<Energy>,
    pub alive: bool,
    /// Why not, when it isn't.
    pub why: String,
}

/// A rooted plant waiting to be grown.
#[derive(Clone, Copy, Debug)]
struct Plant {
    species: u16,
    x: i64,
    y: i64,
    /// Away from the ground it grows on (8 directions).
    up: (i8, i8),
    seed: u64,
    age: f64,
    /// It sits in soil (roots grow).
    soil: bool,
}

struct ChunkAnchors {
    plants: Vec<Plant>,
    /// Ground cover cells in this chunk: (x, y, material).
    cover: Vec<(i64, i64, MatId)>,
}

/// One body's flora.
pub struct Flora {
    pub rules: LifeFile,
    pub seed: u64,
    pub glamour: f64,
    pub scale: f64,
    pub species: Vec<Species>,
    pub report: Vec<NicheReport>,
    /// Places found by the survey, by niche: (niche, x, y) of open cells.
    pub spots: Vec<(Niche, i64, i64)>,
    /// Per niche: the cover species and the standing species.
    cover_of: HashMap<Niche, usize>,
    standing: HashMap<Niche, Vec<usize>>,
    soil: Vec<bool>,
    /// Liquids life here can live in (the living solvents' materials).
    living: Vec<bool>,
    icy: Vec<bool>,
    anchors: RwLock<HashMap<(i64, i64), Arc<ChunkAnchors>>>,
    overlay: RwLock<HashMap<(i64, i64), Arc<Vec<MatId>>>>,
    grown: RwLock<HashMap<(i64, i64, i8, i8), Arc<Vec<(i64, i64, MatId, u8)>>>>,
}

impl std::fmt::Debug for Flora {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Flora({} species)", self.species.len())
    }
}

// ---- A small random stream ----

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        unit(hash(self.0, 0, 0, 0x7AB1))
    }
    fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.next()
    }
    fn signed(&mut self) -> f64 {
        2.0 * self.next() - 1.0
    }
    fn chance(&mut self, p: f64) -> bool {
        self.next() < p
    }
    fn pick<T: Copy>(&mut self, list: &[(T, f64)]) -> T {
        let total: f64 = list.iter().map(|e| e.1).sum();
        let mut r = self.next() * total;
        for &(t, w) in list {
            if r < w {
                return t;
            }
            r -= w;
        }
        list[list.len() - 1].0
    }
}

// ---- Colour ----

fn lch(l: f64, c: f64, h: f64) -> [u8; 3] {
    crate::palette::from_lch(l.clamp(0.05, 0.97), c.max(0.0), h.rem_euclid(360.0))
}

/// Steer hue `h` toward `to` by share `w` the short way round.
fn steer(h: f64, to: f64, w: f64) -> f64 {
    let d = (to - h + 540.0).rem_euclid(360.0) - 180.0;
    (h + d * w).rem_euclid(360.0)
}

// ---- Names (dev panel only) ----

fn name(seed: u64) -> String {
    const A: [&str; 24] = ["ka", "lo", "mi", "ve", "tor", "sha", "ru", "nel", "quo", "fen", "dra", "pi", "zel", "or", "um", "thi", "bra", "ys", "gol", "mer", "ash", "cy", "vel", "no"];
    const B: [&str; 12] = ["wort", "fern", "cap", "vine", "reed", "bloom", "moss", "thorn", "frond", "stalk", "bell", "root"];
    let n = 2 + (unit(hash(seed, 1, 0, 0)) * 2.0) as usize;
    let mut s = String::new();
    for i in 0..n {
        s.push_str(A[(unit(hash(seed, 2, i as i64, 0)) * A.len() as f64) as usize % A.len()]);
    }
    let mut c = s.chars();
    let first = c.next().unwrap().to_uppercase().collect::<String>();
    format!("{first}{} {}", c.as_str(), B[(unit(hash(seed, 3, 0, 0)) * B.len() as f64) as usize % B.len()])
}

// ---- Breeding ----

/// Template materials by part.
struct Templates {
    wood: MatId,
    leaf: MatId,
    bloom: MatId,
    fruit: MatId,
    fungus: MatId,
    moss: MatId,
    root: MatId,
    kelp: MatId,
    crystal: MatId,
}

impl Templates {
    fn load(m: &MaterialTable) -> Result<Self, String> {
        Ok(Templates {
            wood: m.id("wood")?,
            leaf: m.id("leaf")?,
            bloom: m.id("bloom")?,
            fruit: m.id("fruit")?,
            fungus: m.id("fungus")?,
            moss: m.id("moss")?,
            root: m.id("root")?,
            kelp: m.id("kelp")?,
            crystal: m.id("living crystal")?,
        })
    }
}

/// Breed one species for a niche. `hue` is the body's palette hue and
/// `accents` its scheme's offsets.
fn breed(seed: u64, niche: Niche, cover: bool, g: f64, t: &Templates, rep: &NicheReport, hue: f64, accents: &[f64]) -> Species {
    let mut r = Rng(seed);
    let plans: Vec<(Plan, f64)> = niche.plans().iter().filter(|p| g >= p.2).map(|p| (p.0, p.1)).collect();
    let plan = if cover {
        if niche == Niche::Soil || niche == Niche::Shallows { Plan::Tuft } else { Plan::Mat }
    } else {
        r.pick(&plans)
    };
    // Strangeness: how far this species strays, up to the body's glamour.
    let odd = g * r.range(0.4, 1.0);
    let giant = 1.0 + odd * r.range(0.0, 0.9);

    let mut s = Species {
        name: name(seed),
        niche,
        plan,
        cover,
        energy: rep.energy.unwrap_or(Energy::Light),
        solvent: rep.solvent.clone().unwrap_or_default(),
        min_k: 0.0,
        max_k: 0.0,
        height: 1.0,
        thick: 1.0,
        taper: 0.6,
        internode: 2.0,
        branch: 0.0,
        angle: 0.5,
        stems: 1,
        pattern: Pattern::Alternate,
        crown: None,
        sun: 0.5,
        droop: 0.0,
        hug: 0.0,
        wiggle: 0.15,
        leaf: Organ::None,
        leaf_r: 1.0,
        tip: Organ::None,
        tip_r: 1.0,
        roots: 0.0,
        growth: 1.0,
        lifespan_s: 3600.0,
        defence: Vec::new(),
        motion: Motion::Sway,
        sway: 0.5,
        abundance: r.range(0.4, 1.6),
        patch_m: r.range(25.0, 220.0),
        stem_m: 0,
        foliage_m: 0,
        organ_m: 0,
        root_m: 0,
        parts: [(t.wood, [0; 3]); 4],
    };
    // Tolerance: a window of at least 60% of what the niche offers, so
    // species sort themselves by warmth but every part of it has some.
    let span = (rep.high_k - rep.low_k).max(10.0);
    let width = span * r.range(0.6, 1.0);
    let start = rep.low_k + (span - width) * r.next();
    s.min_k = start - r.range(5.0, 15.0);
    s.max_k = start + width + r.range(5.0, 15.0);

    let organs_leaf = [(Organ::Blob, 3.0), (Organ::Fan, 1.5), (Organ::Spike, 1.0), (Organ::Blade, 1.5), (Organ::Bladder, 0.4 * g)];
    let organs_tip = [(Organ::None, 2.0), (Organ::Blob, 1.0), (Organ::Lantern, 0.6 + 2.0 * g), (Organ::Plume, 1.0), (Organ::Pod, 0.8), (Organ::Trap, 1.5 * g), (Organ::Spike, 0.6)];
    let patterns = [(Pattern::Random, 1.0), (Pattern::Alternate, 2.0), (Pattern::Opposite, 1.5), (Pattern::Pinnate, 1.0), (Pattern::Whorl, 0.6)];
    match plan {
        Plan::Mat | Plan::Tuft => {
            s.height = if plan == Plan::Tuft { r.range(0.5, 1.5) } else { 0.5 };
            s.motion = if plan == Plan::Tuft { Motion::Ripple } else { Motion::Rigid };
            s.abundance = 1.0;
            s.patch_m = r.range(15.0, 90.0);
            s.growth = r.range(1.0, 4.0);
        }
        Plan::Rosette => {
            s.height = r.range(1.5, 4.0) * giant;
            s.stems = (r.range(4.0, 11.0)) as u32;
            s.thick = 1.0;
            s.droop = r.range(0.0, 0.6);
            s.pattern = r.pick(&[(Pattern::Pinnate, 2.0), (Pattern::Alternate, 1.0), (Pattern::Random, 1.0)]);
            s.leaf = r.pick(&[(Organ::None, 1.0), (Organ::Blade, 1.0), (Organ::Blob, 0.6)]);
            s.leaf_r = 1.0;
            s.tip = r.pick(&organs_tip);
            s.tip_r = r.range(0.8, 2.0);
            s.angle = r.range(0.8, 1.35);
            s.sway = r.range(0.2, 0.6);
        }
        Plan::Vine => {
            s.height = r.range(10.0, 45.0) * giant;
            s.stems = (r.range(1.0, 4.0)) as u32;
            s.hug = r.range(0.6, 1.0);
            s.sun = r.range(0.2, 0.6);
            s.droop = r.range(0.2, 0.7);
            s.wiggle = r.range(0.2, 0.5);
            s.branch = r.range(0.05, 0.2);
            s.angle = r.range(0.4, 1.0);
            s.internode = r.range(1.5, 3.0);
            s.pattern = r.pick(&patterns);
            s.leaf = r.pick(&organs_leaf);
            s.leaf_r = r.range(0.6, 1.6);
            s.tip = r.pick(&organs_tip);
            s.tip_r = r.range(0.8, 2.0);
            s.motion = Motion::Drift;
            s.sway = r.range(0.4, 1.0);
        }
        Plan::Shrub => {
            s.height = r.range(1.5, 6.0) * giant;
            s.stems = (r.range(1.0, 4.0)) as u32;
            s.branch = r.range(0.25, 0.55);
            s.angle = r.range(0.35, 0.8);
            s.internode = r.range(1.0, 2.0);
            s.pattern = r.pick(&patterns);
            s.leaf = r.pick(&organs_leaf);
            s.leaf_r = r.range(0.7, 1.8);
            s.tip = r.pick(&organs_tip);
            s.tip_r = r.range(0.6, 1.5);
            s.roots = r.range(0.2, 0.5);
            s.sway = r.range(0.2, 0.5);
        }
        Plan::Tree => {
            s.height = r.range(10.0, 45.0) * (1.0 + odd * r.range(0.0, 1.7));
            if r.chance(0.08 + 0.15 * g) {
                s.height *= r.range(1.4, 1.9); // a giant
            }
            s.height = s.height.min(110.0);
            s.thick = (s.height / r.range(18.0, 32.0)).clamp(1.0, 7.0);
            s.crown = Some(r.pick(&[(Crown::Round, 3.0), (Crown::Cone, 2.5), (Crown::Umbrella, 1.5), (Crown::Column, 1.0), (Crown::Weeping, 1.0)]));
            s.leaf = r.pick(&[(Organ::Blob, 4.0), (Organ::Fan, 1.0), (Organ::Spike, 1.0), (Organ::Bladder, 0.4 * g)]);
            s.leaf_r = r.range(1.5, 4.0) * (s.height / 30.0).clamp(0.6, 1.5);
            s.tip = r.pick(&[(Organ::None, 3.0), (Organ::Blob, 1.0), (Organ::Lantern, 0.5 + 1.5 * g), (Organ::Pod, 0.6)]);
            s.tip_r = r.range(0.6, 1.4);
            s.roots = r.range(0.25, 0.5);
            s.sway = r.range(0.1, 0.3);
            s.taper = r.range(0.5, 0.8);
            s.lifespan_s = r.range(4.0, 20.0) * 3600.0;
            s.growth = r.range(0.3, 1.0);
        }
        Plan::Stalk => {
            s.height = r.range(5.0, 30.0) * giant;
            s.stems = (r.range(1.0, 7.0)) as u32;
            s.thick = r.range(1.0, 2.0);
            s.angle = r.range(0.05, 0.3);
            s.internode = r.range(2.0, 5.0);
            s.pattern = r.pick(&patterns);
            s.leaf = r.pick(&[(Organ::Blade, 2.0), (Organ::Spike, 1.0), (Organ::None, 1.0)]);
            s.leaf_r = r.range(1.0, 3.0);
            s.tip = r.pick(&organs_tip);
            s.tip_r = r.range(1.0, 2.5);
            s.wiggle = r.range(0.0, 0.1);
            s.sun = 0.9;
            s.growth = r.range(2.0, 6.0); // bamboo surge
            s.roots = r.range(0.1, 0.3);
            s.sway = r.range(0.3, 0.7);
        }
        Plan::Cap => {
            s.height = r.range(1.5, 10.0) * (1.0 + odd * r.range(0.0, 1.5));
            s.thick = (s.height / r.range(5.0, 10.0)).clamp(1.0, 4.0);
            s.stems = if r.chance(0.4) { (r.range(2.0, 6.0)) as u32 } else { 1 };
            s.angle = r.range(0.2, 0.6);
            s.leaf_r = r.range(0.35, 0.6); // cap width as a share of height
            s.tip = r.pick(&[(Organ::None, 2.0), (Organ::Lantern, 1.0 + 2.0 * g), (Organ::Blob, 0.5)]);
            s.wiggle = r.range(0.0, 0.15);
            s.sun = 1.0;
            s.motion = Motion::Rigid;
        }
        Plan::Tube => {
            s.height = r.range(2.0, 14.0) * giant;
            s.stems = (r.range(3.0, 14.0)) as u32;
            s.thick = 1.0;
            s.angle = r.range(0.2, 0.8);
            s.wiggle = r.range(0.1, 0.4);
            s.sun = 0.8;
            s.tip = r.pick(&[(Organ::Plume, 2.0), (Organ::Lantern, 1.0 + g), (Organ::Blob, 1.0), (Organ::Trap, g)]);
            s.tip_r = r.range(0.8, 2.0);
            s.motion = Motion::Drift;
            s.sway = r.range(0.4, 0.9);
        }
        Plan::Lattice => {
            s.height = r.range(4.0, 30.0) * giant;
            s.thick = r.range(1.0, 3.0);
            s.branch = r.range(0.3, 0.7);
            s.angle = [0.5236, 0.7854, 1.0472][(r.next() * 3.0) as usize % 3];
            s.internode = r.range(2.0, 5.0);
            s.wiggle = 0.0;
            s.sun = 1.0;
            s.tip = r.pick(&[(Organ::Spike, 2.0), (Organ::Lantern, 1.0), (Organ::None, 1.0)]);
            s.tip_r = r.range(1.0, 3.0);
            s.motion = Motion::Rigid;
            s.sway = 0.0;
        }
    }
    if niche == Niche::CaveRoof && !matches!(plan, Plan::Vine) {
        s.height = s.height.min(20.0);
    }

    // Defences: most species have none; strange worlds arm more of them.
    if !cover {
        let mut options: Vec<(Defence, f64)> = vec![(Defence::Thorns, 1.0 - 0.6 * g), (Defence::Caustic, if g >= 0.2 { 0.8 } else { 0.0 })];
        if g >= 0.3 {
            options.push((Defence::Gas, 0.6));
            options.push((Defence::Resin, if niche.sky() { 0.6 } else { 0.0 }));
        }
        if g >= 0.4 {
            options.push((Defence::Pods, if matches!(s.tip, Organ::Pod) { 3.0 } else { 0.5 }));
        }
        if matches!(plan, Plan::Vine | Plan::Tube) && g >= 0.3 {
            options.push((Defence::Whip, 2.0));
        }
        if g >= 0.6 {
            options.push((Defence::Snap, if matches!(s.tip, Organ::Trap) { 4.0 } else { 0.4 }));
            options.push((Defence::Flash, if matches!(s.tip, Organ::Lantern) { 1.0 } else { 0.2 }));
        }
        let armed = 0.15 + 0.5 * g;
        if r.chance(armed) {
            let d = r.pick(&options);
            s.defence.push(d);
            if r.chance(0.25 * g) {
                let d2 = r.pick(&options);
                if d2 != d {
                    s.defence.push(d2);
                }
            }
        }
        if s.defence.contains(&Defence::Snap) && s.tip != Organ::Trap {
            s.tip = Organ::Trap;
            s.tip_r = s.tip_r.max(1.5);
        }
        if s.defence.contains(&Defence::Thorns) && s.leaf == Organ::None {
            s.leaf = Organ::Spike;
            s.leaf_r = 1.0;
        }
    }
    if s.plan == Plan::Lattice || s.plan == Plan::Cap {
        s.motion = Motion::Rigid;
    }

    // ---- Colours: natural at glamour 0, steered into the body's scheme and
    // beyond as glamour rises. ----
    let pull = (g / 0.8).clamp(0.0, 1.0);
    let accent = |k: f64| (hue + accents[(k * accents.len() as f64) as usize % accents.len()]).rem_euclid(360.0);
    let dark = niche.dark();
    // Foliage.
    let (fl, fc, fh) = if plan == Plan::Lattice {
        (r.range(0.65, 0.85), r.range(0.05, 0.12), r.range(170.0, 260.0))
    } else if plan == Plan::Cap || (dark && !niche.wet()) {
        (r.range(0.6, 0.88), r.range(0.01, 0.06), r.range(40.0, 90.0)) // pale fungi
    } else if niche.wet() {
        (r.range(0.35, 0.6), r.range(0.05, 0.11), r.range(70.0, 120.0)) // olive kelp
    } else if niche == Niche::Frozen {
        (r.range(0.45, 0.65), r.range(0.04, 0.09), r.range(100.0, 160.0))
    } else {
        (r.range(0.38, 0.62), r.range(0.07, 0.14), r.range(115.0, 150.0)) // greens
    };
    let fh2 = steer(fh, accent(r.next()), pull * r.range(0.5, 1.0));
    let fc2 = fc + (r.range(0.08, 0.18) - fc).max(0.0) * pull;
    let foliage = lch(fl, fc2, fh2);
    // Stems: browns, or tinted with the foliage for soft plants.
    let soft = matches!(plan, Plan::Mat | Plan::Tuft | Plan::Cap | Plan::Tube | Plan::Rosette | Plan::Lattice);
    let stem = if soft {
        lch(fl * r.range(0.75, 0.95), fc2 * 0.8, fh2 + r.range(-15.0, 15.0))
    } else {
        let h = steer(r.range(45.0, 70.0), accent(r.next()), pull * 0.35);
        lch(r.range(0.3, 0.48), r.range(0.03, 0.07) + 0.04 * pull, h)
    };
    // Organs: flowers and fruit in any hue; lanterns bright.
    let oh = if g < 0.2 { r.range(0.0, 360.0) } else { accent(r.next()) + r.range(-20.0, 20.0) };
    let organ = match s.tip {
        Organ::Lantern => lch(r.range(0.82, 0.95), r.range(0.12, 0.2), oh),
        _ => lch(r.range(0.55, 0.8), r.range(0.1, 0.2), oh),
    };
    let root = lch(r.range(0.38, 0.5), 0.04, 60.0);

    let stem_t = match plan {
        Plan::Cap => t.fungus,
        Plan::Tube | Plan::Vine if niche.wet() => t.kelp,
        Plan::Tube => t.fungus,
        Plan::Lattice => t.crystal,
        Plan::Mat | Plan::Tuft => t.moss,
        _ => t.wood,
    };
    let foliage_t = match plan {
        Plan::Cap => t.fungus,
        Plan::Lattice => t.crystal,
        Plan::Mat | Plan::Tuft => t.moss,
        _ if niche.wet() => t.kelp,
        _ if dark => t.fungus,
        _ => t.leaf,
    };
    let organ_t = match s.tip {
        Organ::Pod | Organ::Blob => t.fruit,
        _ if plan == Plan::Lattice => t.crystal,
        _ => t.bloom,
    };
    s.parts = [(stem_t, stem), (foliage_t, foliage), (organ_t, organ), (t.root, root)];
    s
}

// ---- The survey and the body's flora ----

impl Flora {
    /// Survey the body, decide where life can live and breed its species.
    /// The species' materials are not added yet (see `Planet::attach_flora`).
    pub fn survey(p: &Planet, rules: &LifeFile, scale: f64) -> Result<Flora, String> {
        let mats = &p.mats;
        let seed = hash(p.spec.seed, 0, 0, 0xF10A);
        let g = p.spec.glamour.unwrap_or(0.0).clamp(0.0, 1.0);
        let n = mats.len();
        let mut soil = vec![false; n];
        for s in &rules.soils {
            if let Ok(id) = mats.id(s) {
                soil[id as usize] = true;
            }
        }
        let icy: Vec<bool> = mats.list.iter().map(|m| m.state == State::Solid && crate::texture::icy_name(&m.name)).collect();
        let mut seen = vec![false; n];
        let mut samples: HashMap<Niche, Vec<f64>> = HashMap::new();
        let mut spots: Vec<(Niche, i64, i64)> = Vec::new();
        let sea = p.sea();

        // The surface, all the way round.
        let count = ((2.0 * std::f64::consts::PI * p.radius / 16.0) as usize).clamp(720, 8192);
        for i in 0..count {
            let (ux, uy) = cos_sin(2.0 * std::f64::consts::PI * (i as f64 + 0.5) / count as f64);
            let s = p.surface_radius(ux, uy);
            let (x, y) = ((ux * (s - 0.6)).floor() as i64, (uy * (s - 0.6)).floor() as i64);
            let m = p.base_material(x, y);
            if m == VACUUM {
                continue;
            }
            seen[m as usize] = true;
            let mat = mats.get(m);
            let (niche, cx, cy) = if mat.state == State::Liquid {
                match (sea, p.relief.as_ref()) {
                    (Some((sm, _)), Some(rel)) if sm == m => {
                        let d = rel.sea_depth(ux, uy);
                        let fr = s - d - 0.6;
                        let (fx, fy) = ((ux * fr).floor() as i64, (uy * fr).floor() as i64);
                        let fm = p.base_material(fx, fy);
                        seen[fm as usize] = true;
                        (if d <= rules.sea_light_m { Niche::Shallows } else { Niche::Deeps }, fx, fy)
                    }
                    _ => continue,
                }
            } else if soil[m as usize] {
                (Niche::Soil, x, y)
            } else if icy[m as usize] {
                (Niche::Frozen, x, y)
            } else {
                (Niche::Bare, x, y)
            };
            let k = p.ambient_k(cx, cy);
            let niche = if k >= rules.hot_k && !niche.sky() { Niche::Hot } else { niche };
            samples.entry(niche).or_default().push(k);
            if i % 7 == 0 {
                spots.push((niche, cx, cy));
            }
        }

        // Liquids some life could live in (buried oil or magma isn't one).
        let mut solventish = vec![false; n];
        for sv in &rules.solvent {
            for m in &sv.materials {
                if let Ok(id) = mats.id(m) {
                    solventish[id as usize] = mats.get(id).state == State::Liquid;
                }
            }
        }
        // The interior down to the deepest life: caves, buried seas, heat.
        let tries = 12_000;
        for i in 0..tries {
            let h = hash(seed, i, 0, 0x5A3);
            let (ux, uy) = cos_sin(2.0 * std::f64::consts::PI * unit(h));
            let s = p.surface_radius(ux, uy);
            let depth = 3.0 + (rules.depth_m - 3.0) * unit(hash(h, 1, 0, 0));
            let r = s - depth;
            if r <= 1.0 {
                continue;
            }
            let (x, y) = ((ux * r).floor() as i64, (uy * r).floor() as i64);
            let m = p.base_material(x, y);
            seen[m as usize] = true;
            let state = mats.get(m).state;
            let k = p.ambient_k(x, y);
            if m == VACUUM && !p.is_sky(x, y) {
                if k >= rules.hot_k {
                    samples.entry(Niche::Hot).or_default().push(k);
                    spots.push((Niche::Hot, x, y));
                } else {
                    samples.entry(Niche::CaveFloor).or_default().push(k);
                    samples.entry(Niche::CaveRoof).or_default().push(k);
                    spots.push((Niche::CaveFloor, x, y));
                }
            } else if state == State::Liquid && solventish[m as usize] && !p.in_sea(x, y) {
                let niche = if k >= rules.hot_k { Niche::Hot } else { Niche::Deeps };
                samples.entry(niche).or_default().push(k);
                if i % 3 == 0 {
                    spots.push((niche, x, y));
                }
            } else if k >= rules.hot_k && k < 500.0 && i % 4 == 0 {
                // Warm rock: hot springs wherever it opens.
                samples.entry(Niche::Hot).or_default().push(k);
            }
        }

        // ---- Which niches can hold life ----
        let solvents: Vec<(&SolventRow, bool)> = rules
            .solvent
            .iter()
            .map(|sv| (sv, g >= sv.glamour && sv.materials.iter().any(|m| mats.id(m).is_ok_and(|id| seen[id as usize]))))
            .collect();
        let air = p.spec.atmosphere_m > 0.0;
        let mut report = Vec::new();
        for niche in Niche::ALL {
            let Some(ks) = samples.get_mut(&niche) else { continue };
            ks.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let lo = ks[ks.len() / 20];
            let hi = ks[ks.len() - 1 - ks.len() / 20];
            let found = solvents.iter().find(|(sv, present)| *present && hi >= sv.min_k && lo <= sv.max_k);
            let solvent = found.map(|(sv, _)| sv.name.clone());
            // Life only lives where its liquid stays liquid.
            let (lo, hi) = match found {
                Some((sv, _)) => (lo.max(sv.min_k), hi.min(sv.max_k)),
                None => (lo, hi),
            };
            let energy = if niche.sky() || niche == Niche::Shallows {
                Some(Energy::Light)
            } else if niche == Niche::Hot || niche == Niche::Deeps {
                Some(Energy::Chemistry)
            } else {
                None // caves: decided once the surface is known
            };
            let mut why = String::new();
            if solvent.is_none() {
                why = format!("no liquid life can use between {lo:.0} and {hi:.0} K");
            } else if niche.sky() && !air {
                why = "no air: liquids boil off the surface".into();
            }
            report.push(NicheReport { niche, samples: ks.len(), low_k: lo, high_k: hi, alive: why.is_empty(), solvent, energy, why });
        }
        let lit = report.iter().any(|r| r.alive && r.energy == Some(Energy::Light) && r.niche.sky());
        let hot = report.iter().any(|r| r.alive && r.niche == Niche::Hot);
        for r in report.iter_mut().filter(|r| matches!(r.niche, Niche::CaveFloor | Niche::CaveRoof)) {
            if lit {
                r.energy = Some(Energy::Seepage);
            } else if hot {
                r.energy = Some(Energy::Chemistry);
            } else if r.alive {
                r.alive = false;
                r.why = "dark, and nothing living above or hot below to feed on".into();
            }
        }
        if scale <= 0.0 {
            for r in report.iter_mut() {
                r.alive = false;
                r.why = "life is off for this body".into();
            }
        }

        // ---- Breed ----
        let t = Templates::load(mats)?;
        let accents: Vec<f64> = p.palette.scheme.offsets().to_vec();
        let hue = p.palette.base_hue;
        let mut species = Vec::new();
        let mut cover_of = HashMap::new();
        let mut standing: HashMap<Niche, Vec<usize>> = HashMap::new();
        for (ni, rep) in report.iter().enumerate().filter(|(_, r)| r.alive) {
            let ns = hash(seed, ni as i64, 0, 0xB4EE);
            cover_of.insert(rep.niche, species.len());
            species.push(breed(hash(ns, 0, 0, 1), rep.niche, true, g, &t, rep, hue, &accents));
            let [a, b] = rules.species;
            let k = (a as f64 + (b - a + 1) as f64 * unit(hash(ns, 1, 0, 2))).floor() * scale.clamp(0.25, 2.0).sqrt();
            // Open soil is the richest niche: more species, and always trees.
            let k = (k.round() as usize).max(1) + if rep.niche == Niche::Soil { 2 } else { 0 };
            for j in 0..k {
                standing.entry(rep.niche).or_default().push(species.len());
                let mut sp = breed(hash(ns, 2 + j as i64, 0, 3), rep.niche, false, g, &t, rep, hue, &accents);
                if rep.niche == Niche::Soil && j == 0 && sp.plan != Plan::Tree {
                    // Breed again until a tree comes up.
                    for extra in 0..64 {
                        sp = breed(hash(ns, 100 + extra, 0, 3), rep.niche, false, g, &t, rep, hue, &accents);
                        if sp.plan == Plan::Tree {
                            break;
                        }
                    }
                }
                species.push(sp);
            }
        }
        let mut living = vec![false; n];
        for r in report.iter().filter(|r| r.alive) {
            if let Some(sv) = rules.solvent.iter().find(|sv| Some(&sv.name) == r.solvent.as_ref()) {
                for m in &sv.materials {
                    if let Ok(id) = mats.id(m) {
                        if mats.get(id).state == State::Liquid {
                            living[id as usize] = true;
                        }
                    }
                }
            }
        }
        // The sea counts whatever it is called.
        if let Some((sm, _)) = p.sea() {
            if living.iter().any(|&l| l) && report.iter().any(|r| r.alive && r.niche.wet()) {
                living[sm as usize] = true;
            }
        }
        let filter_spots: Vec<(Niche, i64, i64)> = spots.into_iter().filter(|s| cover_of.contains_key(&s.0)).collect();

        Ok(Flora {
            rules: rules.clone(),
            seed,
            glamour: g,
            scale,
            species,
            report,
            spots: filter_spots,
            cover_of,
            standing,
            soil,
            living,
            icy,
            anchors: RwLock::new(HashMap::new()),
            overlay: RwLock::new(HashMap::new()),
            grown: RwLock::new(HashMap::new()),
        })
    }

    /// Whether `m` is a liquid life here can live in.
    pub fn living(&self, m: MatId) -> bool {
        self.living.get(m as usize).copied().unwrap_or(false)
    }

    /// Whether a cell holding `m` is soil roots can grow in.
    pub fn is_soil(&self, m: MatId) -> bool {
        self.soil.get(m as usize).copied().unwrap_or(false)
    }

    /// The material at (x, y) with the flora laid over `base`, the
    /// generator's answer there.
    pub fn over(&self, p: &Planet, x: i64, y: i64, base: MatId) -> MatId {
        let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
        let r = (fx * fx + fy * fy).sqrt().max(1e-9);
        let s = p.surface_radius(fx / r, fy / r);
        if r > s + REACH || r < s - self.rules.depth_m - REACH {
            return base;
        }
        let key = (x.div_euclid(CHUNK), y.div_euclid(CHUNK));
        let ov = self.chunk(p, key);
        if ov.is_empty() {
            return base;
        }
        let i = ((y - key.1 * CHUNK) * CHUNK + (x - key.0 * CHUNK)) as usize;
        match ov[i] {
            0 => base,
            m => m,
        }
    }

    fn chunk(&self, p: &Planet, key: (i64, i64)) -> Arc<Vec<MatId>> {
        if let Some(c) = self.overlay.read().unwrap().get(&key) {
            return c.clone();
        }
        let c = Arc::new(self.build_chunk(p, key));
        self.overlay.write().unwrap().entry(key).or_insert(c).clone()
    }

    /// Whether a chunk may hold life at all.
    fn may_live(&self, p: &Planet, key: (i64, i64)) -> bool {
        let (cx, cy) = ((key.0 * CHUNK + CHUNK / 2) as f64, (key.1 * CHUNK + CHUNK / 2) as f64);
        let r = (cx * cx + cy * cy).sqrt().max(1e-9);
        let s = p.surface_radius(cx / r, cy / r);
        let half = CHUNK as f64 * 0.75;
        !(r - half > s + 4.0 || r + half < s - self.rules.depth_m)
    }

    fn build_chunk(&self, p: &Planet, key: (i64, i64)) -> Vec<MatId> {
        let mut any = false;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if self.may_live(p, (key.0 + dx, key.1 + dy)) {
                    any = true;
                }
            }
        }
        if !any || self.species.is_empty() {
            return Vec::new();
        }
        let n = (CHUNK * CHUNK) as usize;
        let mut out = vec![0 as MatId; n];
        let mut prio = vec![0u8; n];
        let (x0, y0) = (key.0 * CHUNK, key.1 * CHUNK);
        let own = self.anchors(p, key);
        for &(x, y, m) in &own.cover {
            out[((y - y0) * CHUNK + (x - x0)) as usize] = m;
        }
        for dy in -1..=1 {
            for dx in -1..=1 {
                let a = if dx == 0 && dy == 0 { own.clone() } else { self.anchors(p, (key.0 + dx, key.1 + dy)) };
                for pl in &a.plants {
                    // Skip plants that can't reach this chunk.
                    if pl.x < x0 - REACH as i64 || pl.x >= x0 + CHUNK + REACH as i64 || pl.y < y0 - REACH as i64 || pl.y >= y0 + CHUNK + REACH as i64 {
                        continue;
                    }
                    let cells = self.plant_cells(p, pl);
                    for &(x, y, m, pr) in cells.iter() {
                        if x < x0 || x >= x0 + CHUNK || y < y0 || y >= y0 + CHUNK {
                            continue;
                        }
                        let i = ((y - y0) * CHUNK + (x - x0)) as usize;
                        if pr >= prio[i] {
                            out[i] = m;
                            prio[i] = pr;
                        }
                    }
                }
            }
        }
        // Most chunks (deep ground, open sky) hold no life: keep nothing.
        if out.iter().all(|&m| m == 0) {
            return Vec::new();
        }
        out
    }

    fn plant_cells(&self, p: &Planet, pl: &Plant) -> Arc<Vec<(i64, i64, MatId, u8)>> {
        let key = (pl.x, pl.y, pl.up.0, pl.up.1);
        if let Some(c) = self.grown.read().unwrap().get(&key) {
            return c.clone();
        }
        let sp = &self.species[pl.species as usize];
        let wet = sp.niche.wet() || (sp.niche == Niche::Hot && self.living(quick(p, pl.x + pl.up.0 as i64, pl.y + pl.up.1 as i64)));
        let free = |x: i64, y: i64| -> Ground {
            let m = quick(p, x, y);
            let st = p.mats.get(m).state;
            if m == VACUUM {
                if wet { Ground::Solid } else { Ground::Open }
            } else if st == State::Liquid {
                if wet && self.living(m) { Ground::Open } else { Ground::Solid }
            } else if self.is_soil(m) {
                Ground::Soil
            } else {
                Ground::Solid
            }
        };
        // The plant leans with the ground's normal, not the grid.
        let (fx, fy) = (pl.x as f64 + 0.5, pl.y as f64 + 0.5);
        let r = (fx * fx + fy * fy).sqrt().max(1e-9);
        let radial = (fx / r, fy / r);
        let floor = (pl.up.0 as f64) * radial.0 + (pl.up.1 as f64) * radial.1 >= 0.0;
        let up = if floor { radial } else { (-radial.0, -radial.1) };
        let origin = (fx + up.0 * 0.5, fy + up.1 * 0.5);
        let cells = grow(sp, origin, up, (-radial.0, -radial.1), pl.seed, pl.age, pl.soil, &free);
        let c = Arc::new(cells);
        self.grown.write().unwrap().entry(key).or_insert(c).clone()
    }

    fn anchors(&self, p: &Planet, key: (i64, i64)) -> Arc<ChunkAnchors> {
        if let Some(a) = self.anchors.read().unwrap().get(&key) {
            return a.clone();
        }
        let a = Arc::new(self.find_anchors(p, key));
        self.anchors.write().unwrap().entry(key).or_insert(a).clone()
    }

    /// The niche of ground cell (x, y) holding `m`, whose open neighbour in
    /// direction `up` holds `o`; None if nothing lives there.
    fn niche_at(&self, p: &Planet, x: i64, y: i64, m: MatId, up: (i64, i64), o: MatId, floor: bool) -> Option<(Niche, f64)> {
        let (ox, oy) = (x + up.0, y + up.1);
        let ost = p.mats.get(o).state;
        let niche = if ost == State::Liquid {
            if !floor {
                return None;
            }
            match p.sea() {
                Some((sm, level)) if sm == o && p.in_sea(ox, oy) => {
                    let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
                    let d = level - (fx * fx + fy * fy).sqrt();
                    if d <= self.rules.sea_light_m { Niche::Shallows } else { Niche::Deeps }
                }
                _ => Niche::Deeps,
            }
        } else if o == VACUUM {
            if p.is_sky(ox, oy) {
                if !floor {
                    return None;
                }
                if self.is_soil(m) {
                    Niche::Soil
                } else if self.icy[m as usize] {
                    Niche::Frozen
                } else {
                    Niche::Bare
                }
            } else if floor {
                Niche::CaveFloor
            } else {
                Niche::CaveRoof
            }
        } else {
            return None;
        };
        let k = p.ambient_k(x, y);
        let niche = if k >= self.rules.hot_k && !niche.sky() && self.cover_of.contains_key(&Niche::Hot) { Niche::Hot } else { niche };
        if !self.cover_of.contains_key(&niche) {
            return None;
        }
        // Caves fed from above only reach so deep.
        if matches!(niche, Niche::CaveFloor | Niche::CaveRoof) {
            let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
            let r = (fx * fx + fy * fy).sqrt().max(1e-9);
            let depth = p.surface_radius(fx / r, fy / r) - r;
            let feed = self.report.iter().find(|q| q.niche == niche).and_then(|q| q.energy);
            if depth > self.rules.depth_m || (feed == Some(Energy::Seepage) && depth > self.rules.seep_m) {
                return None;
            }
        }
        Some((niche, k))
    }

    fn find_anchors(&self, p: &Planet, key: (i64, i64)) -> ChunkAnchors {
        let mut plants = Vec::new();
        let mut cover = Vec::new();
        if !self.may_live(p, key) {
            return ChunkAnchors { plants, cover };
        }
        let (x0, y0) = (key.0 * CHUNK, key.1 * CHUNK);
        // The generator's cells with a one-cell border, worked out only as
        // needed: anchors are only looked for in a skin under the ground, on
        // sea floors and (if the chunk has any) in caves.
        let mut g = Grid { base: vec![UNKNOWN; ((CHUNK + 2) * (CHUNK + 2)) as usize], x0, y0 };
        let sea_d = |ux: f64, uy: f64| p.relief.as_ref().map_or(0.0, |rel| rel.sea_depth(ux, uy));
        // Caves: a sparse look at the ground below the skin.
        let mut caves = false;
        'look: for j in (0..CHUNK).step_by(6) {
            for i in (0..CHUNK).step_by(6) {
                let (x, y) = (x0 + i, y0 + j);
                let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
                let r = (fx * fx + fy * fy).sqrt().max(1e-9);
                let (ux, uy) = (fx / r, fy / r);
                let s = p.surface_radius(ux, uy);
                let sd = sea_d(ux, uy);
                if r > s - 3.0 || (sd > 0.0 && r > s - sd - 3.0) {
                    continue;
                }
                let m = g.get(p, x, y);
                if m == VACUUM || self.living(m) {
                    caves = true;
                    break 'look;
                }
            }
        }
        // A chunk wholly below the ground (and any sea floor) with no caves
        // has nothing to find.
        if !caves {
            let mut lowest = f64::INFINITY;
            for (i, j) in [(0, 0), (CHUNK, 0), (0, CHUNK), (CHUNK, CHUNK), (CHUNK / 2, CHUNK / 2)] {
                let (fx, fy) = ((x0 + i) as f64, (y0 + j) as f64);
                let r = (fx * fx + fy * fy).sqrt().max(1e-9);
                let (ux, uy) = (fx / r, fy / r);
                lowest = lowest.min(p.surface_radius(ux, uy) - sea_d(ux, uy));
            }
            let (cx, cy) = ((x0 + CHUNK / 2) as f64, (y0 + CHUNK / 2) as f64);
            if (cx * cx + cy * cy).sqrt() + CHUNK as f64 * 0.75 < lowest - 40.0 {
                return ChunkAnchors { plants, cover };
            }
        }
        let open = |m: MatId| m == VACUUM || self.living(m);
        let (dens_scale, g_) = (self.scale, self.glamour);
        for y in y0..y0 + CHUNK {
            for x in x0..x0 + CHUNK {
                {
                    let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
                    let r = (fx * fx + fy * fy).sqrt().max(1e-9);
                    let (ux, uy) = (fx / r, fy / r);
                    let s = p.surface_radius(ux, uy);
                    if r > s {
                        continue; // open sky
                    }
                    let sd = sea_d(ux, uy);
                    let skin = r > s - 3.0 || (sd > 0.0 && (r - (s - sd)).abs() < 3.0);
                    let sea_column = sd > 0.0 && r > s - sd + 3.0;
                    if sea_column || (!skin && !caves) {
                        continue;
                    }
                }
                let m = g.get(p, x, y);
                // Only solid ground (or powder) carries life; any liquid or gas is no footing.
                if m == VACUUM || !matches!(p.mats.get(m).state, State::Solid | State::Powder) {
                    continue;
                }
                let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
                let r = (fx * fx + fy * fy).sqrt().max(1e-9);
                let (ux, uy) = (fx / r, fy / r);
                let up = (step(ux), step(uy));
                let above = g.get(p, x + up.0, y + up.1);
                let below = g.get(p, x - up.0, y - up.1);
                let (found, dir) = if open(above) {
                    (self.niche_at(p, x, y, m, up, above, true), up)
                } else if below == VACUUM {
                    (self.niche_at(p, x, y, m, (-up.0, -up.1), below, false), (-up.0, -up.1))
                } else {
                    // Cave walls: cover only.
                    let side = [(1, 0), (-1, 0), (0, 1), (0, -1)].into_iter().find(|d| g.get(p, x + d.0, y + d.1) == VACUUM && !p.is_sky(x + d.0, y + d.1));
                    match side {
                        Some(d) => {
                            if let Some((niche, _)) = self.niche_at(p, x, y, m, d, VACUUM, true).filter(|n| matches!(n.0, Niche::CaveFloor | Niche::Hot)) {
                                let ci = self.cover_of[&niche];
                                if unit(hash(self.seed, x, y, 0xC0)) < niche.density().1 * 0.6 * self.cover_patch(ci, fx, fy) {
                                    cover.push((x, y, self.species[ci].foliage_m));
                                }
                            }
                        }
                        None => {}
                    }
                    continue;
                };
                let Some((niche, k)) = found else { continue };
                let (dens, cov) = niche.density();
                // Ground cover, in patches.
                let ci = self.cover_of[&niche];
                let cs = &self.species[ci];
                if k >= cs.min_k && k <= cs.max_k && unit(hash(self.seed, x, y, 0xC0)) < cov * self.cover_patch(ci, fx, fy) {
                    cover.push((x, y, cs.foliage_m));
                    // Tufts stand a cell or two above the ground.
                    if cs.plan == Plan::Tuft {
                        let hh = unit(hash(self.seed, x, y, 0xC1));
                        // Straight out from the ground along one axis, so the
                        // blade touches the cell it grows from.
                        let dir = if dir.0 != 0 && dir.1 != 0 {
                            if g.get(p, x + dir.0, y) == VACUUM || self.living(g.get(p, x + dir.0, y)) { (dir.0, 0) } else { (0, dir.1) }
                        } else {
                            dir
                        };
                        let tx = x + dir.0;
                        let ty = y + dir.1;
                        if hh < 0.45 && in_chunk(tx, ty, x0, y0) && open(g.get(p, tx, ty)) {
                            cover.push((tx, ty, cs.foliage_m));
                            if hh < 0.12 * cs.height && in_chunk(tx + dir.0, ty + dir.1, x0, y0) && open(g.get(p, tx + dir.0, ty + dir.1)) {
                                cover.push((tx + dir.0, ty + dir.1, cs.organ_m));
                            }
                        }
                    }
                }
                // A standing plant?
                let Some(list) = self.standing.get(&niche) else { continue };
                // Forests and clearings: a slow wave of density.
                let wave = (fbm(self.seed ^ 0x40E, fx / 600.0, fy / 600.0, 2) * 1.8 - 0.3).clamp(0.05, 1.3);
                let h = hash(self.seed, x, y, 0x9A);
                if unit(h) >= dens * dens_scale * wave {
                    continue;
                }
                // Which species: each in its own patches, within its temperatures.
                let mut best = None;
                let mut best_w = 0.0;
                for &si in list {
                    let s = &self.species[si];
                    if k < s.min_k || k > s.max_k {
                        continue;
                    }
                    let patch = fbm(self.seed ^ (si as u64 * 0x51), fx / s.patch_m, fy / s.patch_m, 2);
                    let w = s.abundance * patch * patch * (0.5 + unit(hash(h, si as i64, 0, 1)));
                    if w > best_w {
                        best_w = w;
                        best = Some(si);
                    }
                }
                let Some(si) = best else { continue };
                let age = 0.35 + 0.65 * unit(hash(h, 3, 0, 0)).sqrt();
                let _ = g_;
                plants.push(Plant { species: si as u16, x, y, up: (dir.0 as i8, dir.1 as i8), seed: hash(h, 4, 0, 0), age, soil: self.is_soil(m) });
            }
        }
        ChunkAnchors { plants, cover }
    }

    fn cover_patch(&self, ci: usize, fx: f64, fy: f64) -> f64 {
        let s = &self.species[ci];
        let v = fbm(self.seed ^ (ci as u64 * 0x77), fx / s.patch_m, fy / s.patch_m, 2);
        ((v - 0.25) * 2.2).clamp(0.0, 1.0)
    }

    /// A random species of this body, as a visitor would come across one:
    /// mostly standing plants, now and then ground cover. None if nothing
    /// lives here.
    pub fn pick(&self, seed: u64) -> Option<usize> {
        let list: Vec<(usize, f64)> = self.species.iter().enumerate().map(|(i, s)| (i, if s.cover { 0.12 } else { s.abundance })).collect();
        if list.is_empty() {
            return None;
        }
        Some(Rng(seed).pick(&list))
    }

    /// Which part of species `si` material `m` is: 0 stem, 1 foliage,
    /// 2 organ, 3 root (and 1 for anything else).
    pub fn part_of(&self, si: usize, m: MatId) -> u8 {
        let s = &self.species[si];
        if m == s.stem_m {
            0
        } else if m == s.organ_m {
            2
        } else if m == s.root_m {
            3
        } else {
            1
        }
    }

    /// Grow species `si` from the ground cell (x, y) out toward `up`, at
    /// `age` (0 to 1), into the world as `get` reports it (not the
    /// generator): it grows around whatever is already there. Returns the
    /// cells it fills, with their priority.
    pub fn grow_at(&self, p: &Planet, si: usize, x: i64, y: i64, up: (i8, i8), seed: u64, age: f64, soil: bool, get: &dyn Fn(i64, i64) -> MatId) -> Vec<(i64, i64, MatId, u8)> {
        let sp = &self.species[si];
        let wet = sp.niche.wet() || (sp.niche == Niche::Hot && self.living(get(x + up.0 as i64, y + up.1 as i64)));
        let free = |x: i64, y: i64| -> Ground {
            let m = get(x, y);
            let st = p.mats.get(m).state;
            if m == VACUUM {
                if wet { Ground::Solid } else { Ground::Open }
            } else if st == State::Liquid {
                if wet && self.living(m) { Ground::Open } else { Ground::Solid }
            } else if self.is_soil(m) {
                Ground::Soil
            } else {
                Ground::Solid
            }
        };
        let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
        let r = (fx * fx + fy * fy).sqrt().max(1e-9);
        let radial = (fx / r, fy / r);
        let floor = (up.0 as f64) * radial.0 + (up.1 as f64) * radial.1 >= 0.0;
        let upv = if floor { radial } else { (-radial.0, -radial.1) };
        let origin = (fx + upv.0 * 0.5, fy + upv.1 * 0.5);
        grow(sp, origin, upv, (-radial.0, -radial.1), seed, age, soil, &free)
    }

    /// The standing species of `niche`, with their weights.
    pub fn standing_of(&self, niche: Niche) -> Vec<(usize, f64)> {
        self.standing.get(&niche).map(|l| l.iter().map(|&i| (i, self.species[i].abundance)).collect()).unwrap_or_default()
    }

    /// The ground-cover species of `niche`, if it is alive here.
    pub fn cover_of(&self, niche: Niche) -> Option<usize> {
        self.cover_of.get(&niche).copied()
    }

    /// Why nothing lives here, a line per place looked at.
    pub fn why_lifeless(&self) -> Vec<String> {
        if self.report.is_empty() {
            return vec!["no ground, sea or cave was found to live in".into()];
        }
        self.report.iter().map(|r| format!("{}: {}", r.niche.name(), if r.alive { "could live, but no species took".to_string() } else { r.why.clone() })).collect()
    }

    /// Grow one species on its own, for galleries: on flat ground (or under a
    /// flat roof for cave-roof species), at `age` (0 to 1). Returns the cells
    /// relative to the anchor, y up.
    pub fn specimen(&self, si: usize, seed: u64, age: f64) -> Vec<(i64, i64, MatId)> {
        let sp = &self.species[si];
        let roof = sp.niche == Niche::CaveRoof;
        let wet = sp.niche.wet();
        let free = |_x: i64, y: i64| -> Ground {
            let ground = if roof { y >= 0 } else { y < 0 };
            if ground {
                if wet || roof { Ground::Solid } else { Ground::Soil }
            } else {
                Ground::Open
            }
        };
        let (up, origin) = if roof { ((0.0, -1.0), (0.5, -0.0)) } else { ((0.0, 1.0), (0.5, 0.0)) };
        if sp.cover {
            // A strip of the cover itself.
            let mut out = Vec::new();
            for x in -6..=6 {
                let gy = if roof { 0 } else { -1 };
                out.push((x, gy, sp.foliage_m));
                if sp.plan == Plan::Tuft && unit(hash(seed, x, 0, 1)) < 0.5 {
                    out.push((x, gy + if roof { -1 } else { 1 }, sp.foliage_m));
                }
            }
            return out;
        }
        grow(sp, origin, up, (0.0, -1.0), seed, age, !roof && !wet, &free).into_iter().map(|(x, y, m, _)| (x, y, m)).collect()
    }
}

const UNKNOWN: MatId = MatId::MAX;

/// A chunk's generator cells (with a one-cell border), worked out lazily.
struct Grid {
    base: Vec<MatId>,
    x0: i64,
    y0: i64,
}

impl Grid {
    fn get(&mut self, p: &Planet, x: i64, y: i64) -> MatId {
        let w = CHUNK + 2;
        let i = ((y - self.y0 + 1) * w + (x - self.x0 + 1)) as usize;
        if self.base[i] == UNKNOWN {
            self.base[i] = quick(p, x, y);
        }
        self.base[i]
    }
}

/// The generator's cell at (x, y), answering open sky without running the
/// whole generator (it gives vacuum above the surface just the same way).
pub fn quick(p: &Planet, x: i64, y: i64) -> MatId {
    let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
    let r = (fx * fx + fy * fy).sqrt();
    if r > 0.0 && r > p.surface_radius(fx / r, fy / r) {
        return VACUUM;
    }
    p.base_material(x, y)
}

fn in_chunk(x: i64, y: i64, x0: i64, y0: i64) -> bool {
    x >= x0 && x < x0 + CHUNK && y >= y0 && y < y0 + CHUNK
}

/// A unit component rounded to a grid step of -1, 0 or 1 (8 directions).
pub(crate) fn step(u: f64) -> i64 {
    if u > 0.38 {
        1
    } else if u < -0.38 {
        -1
    } else {
        0
    }
}

// ---- The growing-tip engine ----

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ground {
    Open,
    Soil,
    Solid,
}

const P_FOLIAGE: u8 = 1;
const P_ORGAN: u8 = 2;
const P_ROOT: u8 = 2;
const P_STEM: u8 = 3;

type V = (f64, f64);

fn add(a: V, b: V) -> V {
    (a.0 + b.0, a.1 + b.1)
}
fn mul(a: V, k: f64) -> V {
    (a.0 * k, a.1 * k)
}
fn norm(a: V) -> V {
    let l = (a.0 * a.0 + a.1 * a.1).sqrt();
    if l < 1e-9 { (0.0, 1.0) } else { (a.0 / l, a.1 / l) }
}
fn rot(a: V, ang: f64) -> V {
    let (c, s) = cos_sin(ang);
    (a.0 * c - a.1 * s, a.0 * s + a.1 * c)
}
/// Left of `a` (90 degrees counterclockwise).
fn perp(a: V) -> V {
    (-a.1, a.0)
}

struct Grower<'a> {
    sp: &'a Species,
    origin: V,
    up: V,
    down: V,
    free: &'a dyn Fn(i64, i64) -> Ground,
    cells: HashMap<(i64, i64), (MatId, u8)>,
    rng: Rng,
    budget: usize,
}

impl<'a> Grower<'a> {
    fn far(&self, p: V) -> bool {
        let d = (p.0 - self.origin.0, p.1 - self.origin.1);
        d.0 * d.0 + d.1 * d.1 > REACH * REACH
    }

    fn put(&mut self, p: V, m: MatId, pr: u8, into_soil: bool) -> bool {
        if self.budget == 0 || self.far(p) {
            return false;
        }
        let c = (p.0.floor() as i64, p.1.floor() as i64);
        if let Some(&(_, old)) = self.cells.get(&c) {
            if old >= pr {
                return true;
            }
        } else {
            let g = (self.free)(c.0, c.1);
            let ok = match g {
                Ground::Open => !into_soil,
                Ground::Soil => into_soil,
                Ground::Solid => false,
            };
            if !ok {
                return false;
            }
        }
        self.budget -= 1;
        self.cells.insert(c, (m, pr));
        true
    }

    /// Cells from `a` to `b` joined edge to edge (never only by a corner, so
    /// the world never sees a plant as loose pieces). False if blocked.
    fn seg(&mut self, a: V, b: V, m: MatId, pr: u8, into_soil: bool) -> bool {
        let (mut cx, mut cy) = (a.0.floor() as i64, a.1.floor() as i64);
        let (tx, ty) = (b.0.floor() as i64, b.1.floor() as i64);
        let mut guard = 0;
        while (cx, cy) != (tx, ty) && guard < 64 {
            guard += 1;
            // Step along whichever axis is further behind the straight line.
            let (dx, dy) = (tx - cx, ty - cy);
            // Step along the axis further behind; if that cell is taken by
            // ground, go round by the other axis.
            let along_x = (cx + dx.signum(), cy);
            let along_y = (cx, cy + dy.signum());
            let (first, second) = if dx.abs() >= dy.abs() && dx != 0 { (along_x, along_y) } else { (along_y, along_x) };
            let alt = second != (cx, cy);
            if self.put((first.0 as f64 + 0.5, first.1 as f64 + 0.5), m, pr, into_soil) {
                (cx, cy) = first;
            } else if alt && self.put((second.0 as f64 + 0.5, second.1 as f64 + 0.5), m, pr, into_soil) {
                (cx, cy) = second;
            } else {
                return false;
            }
        }
        true
    }

    fn open(&self, p: V) -> bool {
        if self.far(p) {
            return false;
        }
        let c = (p.0.floor() as i64, p.1.floor() as i64);
        self.cells.contains_key(&c) || (self.free)(c.0, c.1) == Ground::Open
    }

    fn disk(&mut self, c: V, r: f64, m: MatId, pr: u8) {
        if r < 0.75 {
            self.put(c, m, pr, false);
            return;
        }
        let n = r.ceil() as i64;
        for j in -n..=n {
            for i in -n..=n {
                let (dx, dy) = (i as f64, j as f64);
                if dx * dx + dy * dy <= r * r {
                    self.put((c.0 + dx, c.1 + dy), m, pr, false);
                }
            }
        }
    }

    fn line(&mut self, a: V, d: V, len: f64, m: MatId, pr: u8) {
        let steps = (len * 1.5).ceil() as i64;
        let mut prev = a;
        for i in 1..=steps {
            let p = add(a, mul(d, len * i as f64 / steps.max(1) as f64));
            if !self.seg(prev, p, m, pr, false) {
                break;
            }
            prev = p;
        }
    }

    /// Stamp organ `o` of size `r` at `p`, the stem coming in along `d`.
    fn organ(&mut self, o: Organ, p: V, d: V, r: f64, m: MatId, pr: u8) {
        match o {
            Organ::None => {}
            Organ::Blob | Organ::Pod => {
                let c = add(p, mul(d, r * 0.6));
                self.seg(p, c, m, pr, false);
                self.disk(c, r, m, pr);
            }
            Organ::Fan => {
                let n = r.ceil() as i64;
                for j in -n..=n {
                    for i in -n..=n {
                        let v = (i as f64, j as f64);
                        if v.0 * v.0 + v.1 * v.1 <= r * r && v.0 * d.0 + v.1 * d.1 >= -0.3 {
                            self.put(add(p, v), m, pr, false);
                        }
                    }
                }
            }
            Organ::Spike => self.line(p, d, r * 1.5, m, pr),
            Organ::Blade => {
                let dd = norm(add(d, mul(self.down, 0.5)));
                self.line(p, dd, r * 1.6, m, pr);
            }
            Organ::Lantern => {
                self.seg(p, add(p, d), m, pr, false);
                if r > 1.2 {
                    let c = add(p, d);
                    self.seg(c, add(p, mul(d, 2.0)), m, pr, false);
                    self.seg(c, add(c, perp(d)), m, pr, false);
                    self.seg(c, add(c, mul(perp(d), -1.0)), m, pr, false);
                }
            }
            Organ::Bladder => {
                let c = add(p, mul(d, r));
                let n = 6 + (r * 6.0) as i64;
                let mut prev = p;
                for i in 0..=n {
                    // From the stem round the ring and back.
                    let a = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
                    let (cc, ss) = cos_sin(a);
                    let q = add(c, add(mul(d, -cc * r), mul(perp(d), ss * r)));
                    self.seg(prev, q, m, pr, false);
                    prev = q;
                }
            }
            Organ::Plume => {
                for k in -2..=2 {
                    let dd = rot(d, 0.35 * k as f64);
                    self.line(p, dd, r * (1.6 - 0.2 * (k as f64).abs()), m, pr);
                }
            }
            Organ::Trap => {
                self.line(p, rot(d, 0.6), r * 1.6, m, pr);
                self.line(p, rot(d, -0.6), r * 1.6, m, pr);
                self.put(p, m, pr, false);
            }
        }
    }

    /// Steer direction `d` one step: toward up (sun), toward world down
    /// (droop), a little wander, and around obstacles. None if boxed in.
    fn steer(&mut self, p: V, d: V) -> Option<V> {
        let sp = self.sp;
        let mut d = add(d, mul(self.up, sp.sun * 0.12));
        d = add(d, mul(self.down, sp.droop * 0.12));
        d = add(d, mul(perp(d), self.rng.signed() * sp.wiggle * 0.35));
        d = norm(d);
        if sp.hug > 0.0 {
            // Climbers lean toward a wall they can feel.
            for side in [1.0, -1.0] {
                let probe = add(p, mul(perp(d), 2.0 * side));
                if !self.open(probe) {
                    d = norm(add(d, mul(perp(d), side * 0.15 * sp.hug)));
                    break;
                }
            }
        }
        if self.open(add(p, d)) {
            return Some(d);
        }
        let first = if self.rng.next() < 0.5 { 1.0 } else { -1.0 };
        for a in [0.5, 1.0, 1.57] {
            for s in [first, -first] {
                let dd = rot(d, a * s);
                if self.open(add(p, dd)) {
                    return Some(dd);
                }
            }
        }
        None
    }

    /// Grow a stem from `p` along `d` for `len` cells, `thick` wide at its
    /// base, branching and leafing as the species does; `depth` counts
    /// branchings so far.
    fn stem(&mut self, mut p: V, mut d: V, len: f64, thick: f64, depth: u32) {
        let sp = self.sp;
        let steps = len.max(1.0).ceil() as usize;
        let mut side = 1.0;
        let mut node = 0usize;
        for i in 0..steps {
            if self.budget == 0 {
                return;
            }
            let Some(nd) = self.steer(p, d) else { break };
            d = nd;
            let prev = p;
            p = add(p, d);
            let f = i as f64 / steps as f64;
            let t = thick * (1.0 - sp.taper * f);
            if t >= 1.5 {
                self.seg(prev, p, sp.stem_m, P_STEM, false);
                self.disk(p, t * 0.5, sp.stem_m, P_STEM);
            } else if !self.seg(prev, p, sp.stem_m, P_STEM, false) {
                // Blocked part way: end the stem where it really got to.
                p = prev;
                break;
            }
            let inode = sp.internode.max(1.0) as usize;
            if (i + 1) % inode == 0 && i + 1 < steps {
                node += 1;
                let rest = len * (1.0 - f);
                // Leaves along the stem.
                if sp.leaf != Organ::None && sp.plan != Plan::Tree {
                    let sides: &[f64] = match sp.pattern {
                        Pattern::Opposite | Pattern::Pinnate | Pattern::Whorl => &[1.0, -1.0],
                        Pattern::Alternate => {
                            side = -side;
                            if side > 0.0 { &[1.0] } else { &[-1.0] }
                        }
                        Pattern::Random => {
                            if self.rng.chance(0.5) { &[1.0] } else { &[-1.0] }
                        }
                    };
                    for &s in sides {
                        let ld = norm(add(mul(perp(d), s), mul(d, 0.4)));
                        let r = if sp.pattern == Pattern::Pinnate { sp.leaf_r * (0.4 + 1.2 * (1.0 - f)) } else { sp.leaf_r };
                        let at = add(p, mul(ld, t * 0.5));
                        self.seg(p, at, sp.foliage_m, P_FOLIAGE, false);
                        self.organ(sp.leaf, at, ld, r, sp.foliage_m, P_FOLIAGE);
                    }
                    if sp.pattern == Pattern::Whorl && node % 3 == 0 {
                        self.organ(Organ::Lantern, p, d, 1.0, sp.organ_m, P_ORGAN);
                    }
                }
                // Side branches.
                if depth < 4 && self.rng.chance(sp.branch) {
                    let s = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
                    let bd = rot(d, sp.angle * s * self.rng.range(0.8, 1.2));
                    let bl = rest * self.rng.range(0.45, 0.8);
                    let bt = (t * 0.6).max(1.0);
                    if bl >= 1.5 {
                        self.stem(p, bd, bl, bt, depth + 1);
                    }
                    if sp.plan == Plan::Lattice && self.rng.chance(0.6) {
                        self.stem(p, rot(d, -sp.angle * s), bl, bt, depth + 1);
                    }
                }
            }
        }
        // The end of a stem.
        if sp.plan == Plan::Tree {
            return;
        }
        if matches!(sp.plan, Plan::Shrub | Plan::Vine) && sp.leaf != Organ::None {
            self.organ(Organ::Blob, p, d, sp.leaf_r, sp.foliage_m, P_FOLIAGE);
        }
        if sp.tip != Organ::None && (depth == 0 || self.rng.chance(0.5)) {
            self.organ(sp.tip, p, d, sp.tip_r, sp.organ_m, P_ORGAN);
        }
    }

    fn roots(&mut self, len: f64, n: u32) {
        let sp = self.sp;
        for k in 0..n {
            let mut d = rot(mul(self.up, -1.0), self.rng.range(-1.1, 1.1) * if k == 0 { 0.2 } else { 1.0 });
            let mut p = add(self.origin, mul(self.up, -0.6));
            let steps = (len * self.rng.range(0.6, 1.0)).ceil() as usize;
            for _ in 0..steps {
                d = norm(add(d, mul(perp(d), self.rng.signed() * 0.3)));
                d = norm(add(d, mul(self.up, -0.08)));
                let prev = p;
                p = add(p, d);
                if !self.seg(prev, p, sp.root_m, P_ROOT, true) {
                    break;
                }
            }
        }
    }

    fn cap(&mut self, p: V, d: V, w: f64, h: f64) {
        let sp = self.sp;
        let side = perp(d);
        // Sampled at half cells: the cap is turned to the grid, and a
        // coarser sampling would leave gaps.
        let n = 2 * (w.ceil() as i64 + 1);
        let hn = 2 * (h.ceil() as i64 + 1);
        for j in -hn..=hn {
            for i in -n..=n {
                let (a, b) = (i as f64 * 0.5, j as f64 * 0.5);
                let q = (a / w) * (a / w) + (b / h) * (b / h);
                if q <= 1.0 && b >= -h * 0.35 {
                    let c = add(p, add(mul(side, a), mul(d, b)));
                    let gill = b < -h * 0.1;
                    self.put(c, if gill { sp.organ_m } else { sp.foliage_m }, if gill { P_ORGAN } else { P_FOLIAGE }, false);
                }
            }
        }
        if sp.tip == Organ::Lantern {
            // Glowing spots on the cap.
            for k in 0..3 {
                let a = self.rng.range(-w * 0.7, w * 0.7);
                let c = add(p, add(mul(side, a), mul(d, h * 0.5 * (k as f64 * 0.2 + 0.4))));
                if self.cells.contains_key(&(c.0.floor() as i64, c.1.floor() as i64)) {
                    self.put(c, sp.organ_m, P_STEM, false);
                }
            }
        }
    }

    /// A big tree's crown by space colonisation: attraction points scattered
    /// in the crown's silhouette pull the branches toward them.
    fn crown(&mut self, base: V, shape: Crown, w: f64, h: f64, thick: f64) {
        let sp = self.sp;
        let (up, side) = (self.up, perp(self.up));
        // Attraction points in the silhouette (local a across, b up from base).
        let mut pts: Vec<V> = Vec::new();
        let want = (24.0 + w * h / 12.0).min(140.0) as usize;
        let mut tries = 0;
        while pts.len() < want && tries < want * 20 {
            tries += 1;
            let a = self.rng.range(-1.0, 1.0);
            let b = self.rng.range(0.0, 1.0);
            let inside = match shape {
                Crown::Round => a * a + (2.0 * b - 1.0) * (2.0 * b - 1.0) <= 1.0,
                Crown::Cone => a.abs() <= 1.0 - b,
                Crown::Umbrella => b >= 0.55 && a * a + ((b - 0.8) / 0.25) * ((b - 0.8) / 0.25) <= 1.0,
                Crown::Column => a.abs() <= 0.45 * (1.0 - (2.0 * b - 1.0).abs().powi(4)),
                Crown::Weeping => a * a + (2.0 * b - 1.2) * (2.0 * b - 1.2) <= 1.0 && b > 0.25,
            };
            if inside {
                pts.push(add(base, add(mul(side, a * w * 0.5), mul(up, b * h))));
            }
        }
        // Nodes: (position, parent).
        let mut nodes: Vec<(V, usize)> = vec![(base, usize::MAX)];
        let step = (h / 14.0).clamp(1.2, 3.0);
        let reach = (w.max(h) * 0.6).max(6.0);
        let kill = step * 1.6;
        for _ in 0..80 {
            if pts.is_empty() {
                break;
            }
            let mut pull: HashMap<usize, V> = HashMap::new();
            for q in &pts {
                let mut best = (usize::MAX, reach * reach);
                for (ni, n) in nodes.iter().enumerate() {
                    let dd = (q.0 - n.0 .0) * (q.0 - n.0 .0) + (q.1 - n.0 .1) * (q.1 - n.0 .1);
                    if dd < best.1 {
                        best = (ni, dd);
                    }
                }
                if best.0 != usize::MAX {
                    let n = nodes[best.0].0;
                    let e = pull.entry(best.0).or_insert((0.0, 0.0));
                    *e = add(*e, norm((q.0 - n.0, q.1 - n.1)));
                }
            }
            if pull.is_empty() {
                break;
            }
            let mut keys: Vec<usize> = pull.keys().copied().collect();
            keys.sort();
            for k in keys {
                let d = norm(add(pull[&k], mul(up, 0.15)));
                let np = add(nodes[k].0, mul(d, step));
                nodes.push((np, k));
            }
            pts.retain(|q| !nodes.iter().any(|n| (q.0 - n.0 .0) * (q.0 - n.0 .0) + (q.1 - n.0 .1) * (q.1 - n.0 .1) < kill * kill));
            if nodes.len() > 700 {
                break;
            }
        }
        // Thickness by how much hangs off each node (pipe model).
        let mut load = vec![1.0f64; nodes.len()];
        let mut kids = vec![0u32; nodes.len()];
        for i in (1..nodes.len()).rev() {
            let par = nodes[i].1;
            load[par] += load[i];
            kids[par] += 1;
        }
        let total = load[0];
        for i in 1..nodes.len() {
            let (a, par) = nodes[i];
            let b = nodes[par].0;
            let t = (thick * (load[i] / total).sqrt()).max(1.0);
            let d = norm((a.0 - b.0, a.1 - b.1));
            let len = ((a.0 - b.0) * (a.0 - b.0) + (a.1 - b.1) * (a.1 - b.1)).sqrt();
            let n = (len * 1.5).ceil() as usize;
            let mut prev = b;
            for s in 0..=n {
                let q = add(b, mul(d, len * s as f64 / n.max(1) as f64));
                self.seg(prev, q, sp.stem_m, P_STEM, false);
                if t >= 1.5 {
                    self.disk(q, t * 0.5, sp.stem_m, P_STEM);
                }
                prev = q;
            }
        }
        // Foliage on the twigs.
        for i in 1..nodes.len() {
            if kids[i] == 0 {
                let (a, par) = nodes[i];
                let d = norm((a.0 - nodes[par].0 .0, a.1 - nodes[par].0 .1));
                match sp.leaf {
                    Organ::Spike => {
                        // Needles: drooping sprays.
                        self.line(a, norm(add(perp(d), self.down)), sp.leaf_r * 1.2, sp.foliage_m, P_FOLIAGE);
                        self.line(a, norm(add(mul(perp(d), -1.0), self.down)), sp.leaf_r * 1.2, sp.foliage_m, P_FOLIAGE);
                        self.disk(a, 1.0, sp.foliage_m, P_FOLIAGE);
                    }
                    o => self.organ(o, a, d, sp.leaf_r, sp.foliage_m, P_FOLIAGE),
                }
                if shape == Crown::Weeping {
                    let len = self.rng.range(0.3, 0.7) * h;
                    self.line(a, self.down, len, sp.foliage_m, P_FOLIAGE);
                }
                if sp.tip != Organ::None && self.rng.chance(0.25) {
                    self.organ(sp.tip, a, d, sp.tip_r, sp.organ_m, P_ORGAN);
                }
            }
        }
    }
}

/// Grow one plant of species `sp` from `origin` (the face of the ground it
/// stands on), growing away along `up`; `down` is the world's down there.
fn grow(sp: &Species, origin: V, up: V, down: V, seed: u64, age: f64, soil: bool, free: &dyn Fn(i64, i64) -> Ground) -> Vec<(i64, i64, MatId, u8)> {
    let mut g = Grower { sp, origin, up, down, free, cells: HashMap::new(), rng: Rng(seed), budget: BUDGET };
    let size = sp.height * age;
    let base = origin;
    // Join the plant to the cell it is rooted on, edge to edge.
    if !matches!(sp.plan, Plan::Mat | Plan::Tuft) {
        g.seg(add(origin, mul(up, -0.5)), add(origin, mul(up, 0.6)), sp.stem_m, P_STEM, false);
    }
    match sp.plan {
        Plan::Mat | Plan::Tuft => {}
        Plan::Rosette => {
            let n = sp.stems.max(3);
            for k in 0..n {
                let a = sp.angle * (2.0 * (k as f64 + 0.5) / n as f64 - 1.0) + g.rng.signed() * 0.1;
                let d = rot(up, a);
                let len = size * g.rng.range(0.6, 1.0) * (1.0 - 0.3 * (a / sp.angle.max(0.1)).abs());
                let saved = (g.sp.sun, g.sp.droop);
                let _ = saved;
                g.stem(base, d, len, 1.0, 3);
            }
            if sp.tip != Organ::None {
                // A flowering stalk from the middle.
                let mut t = sp.clone();
                t.leaf = Organ::None;
                t.branch = 0.0;
                let gg = &mut Grower { sp: &t, origin, up, down, free, cells: std::mem::take(&mut g.cells), rng: Rng(seed ^ 0x51), budget: g.budget };
                gg.stem(base, up, size * 1.3, 1.0, 0);
                g.cells = std::mem::take(&mut gg.cells);
                g.budget = gg.budget;
            }
        }
        Plan::Vine | Plan::Tube | Plan::Stalk | Plan::Shrub => {
            let n = sp.stems.max(1);
            for k in 0..n {
                let a = if n == 1 { g.rng.signed() * 0.1 } else { sp.angle.max(0.2) * (2.0 * (k as f64 + 0.5) / n as f64 - 1.0) + g.rng.signed() * 0.1 };
                let d = rot(up, a);
                let len = size * g.rng.range(0.6, 1.0);
                g.stem(base, d, len, sp.thick, if sp.plan == Plan::Shrub { 1 } else { 0 });
            }
        }
        Plan::Cap => {
            let n = sp.stems.max(1);
            for k in 0..n {
                let a = if n == 1 { 0.0 } else { sp.angle * (2.0 * (k as f64 + 0.5) / n as f64 - 1.0) };
                let s = size * if k == 0 { 1.0 } else { g.rng.range(0.4, 0.8) };
                let mut p = base;
                let mut d = rot(up, a);
                let len = (s * 0.6).max(1.0);
                for _ in 0..len.ceil() as usize {
                    match g.steer(p, d) {
                        Some(nd) => d = nd,
                        None => break,
                    }
                    let prev = p;
                    p = add(p, d);
                    g.seg(prev, p, sp.stem_m, P_STEM, false);
                    if sp.thick * (s / size) >= 1.5 {
                        g.disk(p, sp.thick * (s / size) * 0.5, sp.stem_m, P_STEM);
                    }
                }
                g.cap(p, d, (s * sp.leaf_r).max(1.0), (s * 0.22).max(1.0));
            }
        }
        Plan::Tree => {
            let trunk = size * g.rng.range(0.3, 0.5);
            let mut p = base;
            let mut d = rot(up, g.rng.signed() * 0.1);
            let t = sp.thick * age.sqrt();
            let mut ok = true;
            for i in 0..trunk.ceil() as usize {
                match g.steer(p, d) {
                    Some(nd) => d = nd,
                    None => {
                        ok = false;
                        break;
                    }
                }
                let prev = p;
                p = add(p, d);
                let tt = t * (1.0 - 0.3 * i as f64 / trunk.max(1.0));
                g.seg(prev, p, sp.stem_m, P_STEM, false);
                if tt >= 1.5 {
                    g.disk(p, tt * 0.5, sp.stem_m, P_STEM);
                }
                // Flared base.
                if i == 0 && t >= 2.0 {
                    g.disk(add(p, mul(up, -0.5)), t * 0.8, sp.stem_m, P_STEM);
                }
            }
            if ok || trunk > 4.0 {
                let w = size * match sp.crown {
                    Some(Crown::Umbrella) => 0.9,
                    Some(Crown::Column) => 0.35,
                    Some(Crown::Cone) => 0.5,
                    _ => 0.7,
                };
                let h = size - trunk;
                g.crown(p, sp.crown.unwrap_or(Crown::Round), w, h.max(3.0), t * 0.8);
            }
        }
        Plan::Lattice => {
            let n = sp.stems.max(1);
            for k in 0..n {
                let a = if n == 1 { 0.0 } else { sp.angle * (k as f64 - (n as f64 - 1.0) / 2.0) };
                g.stem(base, rot(up, a), size, sp.thick, 0);
            }
        }
    }
    if soil && sp.roots > 0.0 {
        let n = 2 + (sp.height / 15.0) as u32;
        g.roots((size * sp.roots).clamp(2.0, 30.0), n.min(6));
    }
    // Keep only what is joined edge to edge to the cell the plant stands on
    // (a piece cut off by ground in its way would float).
    let anchor = ((origin.0 - up.0 * 0.5).floor() as i64, (origin.1 - up.1 * 0.5).floor() as i64);
    let mut keep: HashMap<(i64, i64), (MatId, u8)> = HashMap::with_capacity(g.cells.len());
    let mut stack = vec![anchor];
    while let Some(c) = stack.pop() {
        for d in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (c.0 + d.0, c.1 + d.1);
            if keep.contains_key(&n) {
                continue;
            }
            if let Some(&v) = g.cells.get(&n) {
                keep.insert(n, v);
                stack.push(n);
            }
        }
    }
    let mut out: Vec<(i64, i64, MatId, u8)> = keep.into_iter().map(|((x, y), (m, p))| (x, y, m, p)).collect();
    out.sort_unstable_by_key(|c| (c.1, c.0));
    out
}
