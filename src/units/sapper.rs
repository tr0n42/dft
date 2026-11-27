//! Sappers: carry a charge (10 kt) to where the going is hardest, hover
//! along to a way down (cracks and caves) or bore one, until deep enough,
//! and go off. The blast (a shock front running out from the charge over
//! the ticks: rays, cracks, held chunks, a molten heart, rubble, thrown
//! rock, the loosened column, the crater and a plume), rubble settling,
//! chain blasts, and the charges themselves (nuclear, fused uranium, or
//! conventional; uranium left in rubble).
//!
//! Unit file: the sapper's own behaviour; the platform calls `sappers_work`
//! once a game second and `blast_tick` every tick.

use super::*;
use crate::production::Unit;
use crate::world::Look;

/// What a sapper is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SapState {
    /// Dropping from the mother swarm to the ground.
    Fly,
    /// Along the ground and down cracks and caves to its spot, hovering
    /// HOVER_M up while it moves (Benjamin, 26 Nov).
    Crawl,
    /// Boring straight down where there was no way deep enough, a shaft
    /// as wide as its hull.
    Drill,
    /// Armed: its light blinking faster.
    Arm,
}

#[derive(Debug, Clone)]
pub struct Sapper {
    pub(super) id: u32,
    pub(super) state: SapState,
    pub(super) pos: (f64, f64),
    pub(super) prev: (f64, f64),
    pub(super) moved_at: u64,
    /// Where it lands.
    pub(super) goal: (f64, f64),
    /// Its way down through open cells, and how far along it is (cells).
    pub(super) path: Vec<(i64, i64)>,
    pub(super) along: f64,
    /// Metres drilled, seconds armed.
    pub(super) drilled: f64,
    pub(super) armed_s: f64,
    /// Depth it wants to go off at (m under the ground at its landing spot),
    /// and that ground's height (distance from the centre).
    pub(super) want: f64,
    pub(super) ground_r: f64,
    /// Kilograms of uranium in its charge (0: a conventional charge).
    pub(super) fissile: f64,
    /// Drilling sideways (a unit vector) to keep its blast clear of a
    /// tunneler; (0, 0) when drilling down.
    pub(super) side: (f64, f64),
    /// Metres drilled sideways so far.
    pub(super) sideways: f64,
}

/// Farthest a sapper drills sideways to keep clear of tunnelers (m).
pub(super) const SIDEWAYS_MAX_M: f64 = 200.0;

/// At most this many cells of a blast on screen are thrown as flying
/// pieces, and at most this many blown out of its plume; the rest lies as
/// rubble. (Flying thousands of pieces stalled the sim for a blast on view;
/// 200 each until 26 Nov, raised with the 10 kt yield so more goes flying.)
pub(super) const THROW_MAX: usize = 500;

pub(super) const PLUME_MAX: usize = 500;
/// The crater thrown out over a blast: this share of the loosened column's
/// top half-width each side...
pub(super) const CRATER_W: f64 = 0.85;
/// ...and this share of that deep at its middle.
pub(super) const CRATER_D: f64 = 0.4;
/// Most cells thrown out of a crater (the top layers first; the rest stays
/// as rubble). Raised from 3000 on 26 Nov (Benjamin: the throw looked
/// uneven) to what the screen can fly at once.
pub(super) const CRATER_MAX: usize = 20_000;
/// A sapper hovers this high over the ground while it moves along it
/// looking for its way down (less under a low roof) (Benjamin, 26 Nov).
pub(super) const HOVER_M: f64 = 5.0;
/// Half the width of the shaft a sapper bores (m either side of its middle):
/// as wide as its hull (it was 3 m wide; Benjamin, 26 Nov: wider).
pub(super) const SHAFT_HALF: i64 = 3;
/// How far past the broken rock the shock cracks it, in blast radii.
pub(super) const CRACK_REACH: f64 = 1.2;

/// Rubble cells moved per tick, at most...
pub(super) const SETTLE_PER_TICK: usize = 20_000;
/// ...and looked at, at most (the nearest the centre first). Every cell
/// waiting was once gone through each tick, so a big blast's few hundred
/// thousand cells of rubble cost tens of milliseconds a tick for a minute.
const SETTLE_LOOK: usize = 3 * SETTLE_PER_TICK;

/// Loose cells that may settle, kept nearest the centre first (as `settle`
/// goes through them), so a tick takes the nearest few without sorting or
/// going through all the rest.
#[derive(Default)]
pub(super) struct Settling(std::collections::BTreeSet<(i64, i64, i64)>);

impl Settling {
    pub(super) fn of(cells: &CellSet) -> Settling {
        Settling(cells.iter().map(|c| (c.0 * c.0 + c.1 * c.1, c.0, c.1)).collect())
    }

    pub(super) fn insert(&mut self, c: (i64, i64)) {
        self.0.insert((c.0 * c.0 + c.1 * c.1, c.0, c.1));
    }

    pub(super) fn len(&self) -> usize {
        self.0.len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The nearest `n` (at most) taken out, nearest first.
    fn take_nearest(&mut self, n: usize) -> Vec<(i64, i64)> {
        let mut out = Vec::with_capacity(n.min(self.0.len()));
        while out.len() < n {
            let Some((_, x, y)) = self.0.pop_first() else { break };
            out.push((x, y));
        }
        out
    }
}

/// The heart of a sapper's blast is heated to this (K): it melts, and cools
/// to glass after about this long (s; it was 25, and Benjamin, 26 Nov: the
/// blast's glow lingered far too long).
pub(super) const MELT_K: f64 = 2000.0;

pub(super) const MELT_COOL_S: f64 = 6.0;

/// Ground this hot next to a sapper (a blast's heat) destroys it, and so sets it off.
pub(super) const SAPPER_COOK_K: f64 = 800.0;

/// The blocks a blast's shock cracks rock into, of one size: a Voronoi
/// pattern, each block round a site jittered in its square of the grid.
/// Each site (and its block's roll of the dice) is worked out once, not
/// nine times for every cell looked at, and kept in tiles of 32 x 32 sites
/// over the box `x0..x1`, `y0..y1`, each made when first needed (made all
/// at once, a big blast's box took milliseconds to fill).
struct Blocks {
    size: f64,
    blast: u64,
    /// The first tile's first site, and how many tiles across and down.
    i0: i64,
    j0: i64,
    ni: i64,
    nj: i64,
    /// Each site's place and its block's roll.
    tiles: Vec<Option<Box<[[f64; 3]]>>>,
}

/// Sites along a side of a tile of blocks.
const TILE: i64 = 32;

impl Blocks {
    fn new(size: f64, blast: u64, x0: i64, y0: i64, x1: i64, y1: i64) -> Blocks {
        let (i0, j0) = ((x0 as f64 / size).floor() as i64 - 2, (y0 as f64 / size).floor() as i64 - 2);
        let (ni, nj) = (((x1 as f64 / size).floor() as i64 + 3 - i0 + TILE - 1) / TILE, ((y1 as f64 / size).floor() as i64 + 3 - j0 + TILE - 1) / TILE);
        Blocks { size, blast, i0, j0, ni, nj, tiles: (0..ni * nj).map(|_| None).collect() }
    }

    fn work_out(&self, i: i64, j: i64) -> [f64; 3] {
        let (size, k) = (self.size, self.size as u64);
        let px = (i as f64 + crate::noise::unit(hash(47, i, j, k))) * size;
        let py = (j as f64 + crate::noise::unit(hash(53, j, i, k))) * size;
        let id = hash(0xF2AC, i, j, k);
        [px, py, crate::noise::unit(hash(id, self.blast as i64, 0, 71))]
    }

    fn site(&mut self, i: i64, j: i64) -> [f64; 3] {
        let (a, b) = (i - self.i0, j - self.j0);
        let (ta, tb) = (a.div_euclid(TILE), b.div_euclid(TILE));
        if ta < 0 || tb < 0 || ta >= self.ni || tb >= self.nj {
            return self.work_out(i, j);
        }
        let k = (tb * self.ni + ta) as usize;
        if self.tiles[k].is_none() {
            let (fi, fj) = (self.i0 + ta * TILE, self.j0 + tb * TILE);
            self.tiles[k] = Some((0..TILE * TILE).map(|n| self.work_out(fi + n % TILE, fj + n / TILE)).collect());
        }
        let tile = self.tiles[k].as_ref().unwrap();
        tile[((b - tb * TILE) * TILE + (a - ta * TILE)) as usize]
    }

    /// The block cell (x, y) is in (its site's square), and its roll.
    fn of(&mut self, x: i64, y: i64) -> ((i64, i64), f64) {
        let size = self.size;
        let (i0, j0) = ((x as f64 / size).floor() as i64, (y as f64 / size).floor() as i64);
        let (mut best, mut id, mut roll) = (f64::INFINITY, (0, 0), 0.0);
        for i in i0 - 1..=i0 + 1 {
            for j in j0 - 1..=j0 + 1 {
                let [px, py, u] = self.site(i, j);
                let d = (px - x as f64).powi(2) + (py - y as f64).powi(2);
                if d < best {
                    best = d;
                    id = (i, j);
                    roll = u;
                }
            }
        }
        (id, roll)
    }
}

/// How fast a blast's shock front runs out through the rock (m a game
/// second). Real shocks in rock run at kilometres a second; this is slow
/// enough to see the broken ground and the cracks race out from the charge
/// (the 110 m of a 10 kt blast in about half a second, where BLAST_BUDGET
/// keeps up), about as fast as the shock ring the game draws.
pub(super) const FRONT_MPS: f64 = 200.0;

/// The shock front is worked out in shells this thick (m).
const SHELL_M: f64 = 0.5;

/// The most blast work done in a tick, shared by all the blasts going off
/// (see `blasts_advance`): about 8 to 12 ms in a release build, of the 100 ms
/// a tick has at normal speed (at a quarter of that, 1000 kt took 23 game
/// seconds to work through; 27 Nov). Counted in work
/// (a ray's step, a cell of the column or the cracked zone looked at, a
/// cell of rubble gone through at the end), not clock time, so a run comes
/// out the same every time. When it runs out, a front lags behind FRONT_MPS
/// and catches up later: a big blast's front crawls (about 2 s of work at
/// 10,000 kt takes some 20 game seconds), but no tick stalls.
pub(super) const BLAST_BUDGET: u64 = 120_000;

/// The work of generating a leaf of the world cell by cell (4,096 cells:
/// a fraction of a millisecond deep down, a few near the surface, more for
/// the first in a part of the world nothing has touched), which the first
/// change in it does: a blast throwing rock out of fresh ground pays it.
const FRESH_LEAF: u64 = 20_000;

/// The work of asking the world what is in a cell (in ground nothing has
/// changed yet, the generator works it out: dearer near the surface), and
/// of changing a cell (loosening or cracking it), against a cell looked at;
/// and how many cells of a shell can be passed over (outside the column, or
/// over the ground) for the work of looking at one.
const WORK_LOOK: u64 = 4;
const WORK_CHANGE: u64 = 3;
const WORK_PASS: u64 = 8;

/// The cells whose middles lie from `lo` to `hi` m from `at`, into `out`:
/// each cell round a blast is in exactly one shell, so the column and the
/// cracks deal with it once. With `wedge` (two edges' inward normals),
/// only the cells in the wedge between them (and a metre either side) are
/// wanted: the rest of each row is not walked at all.
fn shell_cells(out: &mut Vec<(i64, i64)>, at: (f64, f64), lo: f64, hi: f64, wedge: Option<[(f64, f64); 2]>) {
    out.clear();
    if hi <= 0.0 {
        return;
    }
    let (lo2, hi2) = (lo.max(0.0).powi(2), hi * hi);
    let mut push = |x: i64, y: i64, dy2: f64| {
        let dx = x as f64 + 0.5 - at.0;
        let d2 = dx * dx + dy2;
        if d2 >= lo2 && d2 < hi2 {
            out.push((x, y));
        }
    };
    let cx = at.0 - 0.5;
    for y in (at.1 - 0.5 - hi).floor() as i64..=(at.1 - 0.5 + hi).ceil() as i64 {
        let dy = y as f64 + 0.5 - at.1;
        let dy2 = dy * dy;
        if dy2 >= hi2 {
            continue;
        }
        let xmax = (hi2 - dy2).sqrt();
        let xmin = if lo2 > dy2 { (lo2 - dy2).sqrt() } else { 0.0 };
        // The part of the row in the wedge: n.0 * dx + n.1 * dy >= -1 for
        // both edges bounds dx from one side each.
        let (mut wlo, mut whi) = (i64::MIN, i64::MAX);
        if let Some(ns) = wedge {
            let mut none = false;
            for n in ns {
                let c = -1.0 - n.1 * dy;
                if n.0.abs() < 1e-9 {
                    none |= c > 0.0;
                } else if n.0 > 0.0 {
                    wlo = wlo.max((c / n.0 + cx).floor() as i64);
                } else {
                    whi = whi.min((c / n.0 + cx).ceil() as i64);
                }
            }
            if none || wlo > whi {
                continue;
            }
        }
        // Left of the middle and right of it (one run where they meet).
        let (l0, l1) = ((cx - xmax).floor() as i64, (cx - xmin).ceil() as i64);
        let (r0, r1) = ((cx + xmin).floor() as i64, (cx + xmax).ceil() as i64);
        if l1 >= r0 {
            for x in l0.max(wlo)..=r1.min(whi) {
                push(x, y, dy2);
            }
        } else {
            for x in l0.max(wlo)..=l1.min(whi) {
                push(x, y, dy2);
            }
            for x in r0.max(wlo)..=r1.min(whi) {
                push(x, y, dy2);
            }
        }
    }
}

/// How hard a material is for a blast's rays (0.05 to 0.95).
fn blast_hardness(s: &Sim, m: MatId) -> f64 {
    (s.world.planet.mats.get(m).hardness as f64 / 10.0).clamp(0.05, 0.95)
}

/// The big piece cell (x, y) is in, for hard rock held together by a blast:
/// a Voronoi pattern of pieces about `size` across.
fn blast_piece(x: i64, y: i64, size: f64) -> u64 {
    let (i0, j0) = ((x as f64 / size).floor() as i64, (y as f64 / size).floor() as i64);
    let (mut best, mut id) = (f64::INFINITY, 0u64);
    for i in i0 - 1..=i0 + 1 {
        for j in j0 - 1..=j0 + 1 {
            let px = (i as f64 + crate::noise::unit(hash(41, i, j, 0))) * size;
            let py = (j as f64 + crate::noise::unit(hash(43, j, i, 0))) * size;
            let d = (px - x as f64).powi(2) + (py - y as f64).powi(2);
            if d < best {
                best = d;
                id = hash(0x91EC, i, j, 0);
            }
        }
    }
    id
}

/// One ray out from a charge: its direction, the energy it has left, the
/// next step (m) it takes, and the crack running on past where it gave out.
#[derive(Clone, Copy)]
struct Ray {
    a: f64,
    ca: f64,
    sa: f64,
    e: f64,
    step: i64,
    stopped: bool,
    crack: Option<Crack>,
}

/// A crack wandering on from where a ray gave out: where it has got to, its
/// heading, steps taken and to take, and the hardness of the rock it began in.
#[derive(Clone, Copy)]
struct Crack {
    x: f64,
    y: f64,
    a: f64,
    k: i64,
    len: i64,
    h: f64,
}

/// Where a crater being worked out has got to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum CraterStage {
    /// Looking up the ground along it.
    Ground,
    /// Finding the cells to throw.
    Scan,
    /// Working out where and how fast each flies.
    Throw,
    /// Waiting for the front to reach the ground.
    Ready,
}

/// A blast's crater (see `crater_plan`), worked out a budget's worth at a
/// time ahead of the shock front (it looks only at the ground over the
/// blast, which the front leaves as it was until it gets there), and
/// thrown when the front reaches the ground.
pub(super) struct Crater {
    blast: u64,
    /// Its middle's angle round the body and its ground's distance from the
    /// centre; how far it reaches either side and how deep it is.
    a0: f64,
    g0: f64,
    wl: f64,
    wr: f64,
    dmax: f64,
    /// How far across its depth wanders, how hard it throws, and the
    /// ground's slope.
    period: f64,
    pw: f64,
    slope: f64,
    /// The ground looked up so far (where the next look starts, and the
    /// next place across), and along it: (across, ground, depth).
    near: f64,
    x: f64,
    cols: Vec<(f64, f64, f64)>,
    /// How many places to look through, and the share of its depth that
    /// gives about that many.
    want: f64,
    share: f64,
    /// The next column (or cell) to go through.
    i: usize,
    /// (cell, offset across, depth share, depth m)
    take: Vec<((i64, i64), f64, f64, f64)>,
    seen: CellSet,
    /// The highest of the cells, how far they reach down, and the highest
    /// of each column.
    top: f64,
    span: f64,
    col_top: HashMap<i64, f64>,
    /// What is thrown: (ticks after it goes, cell, where it flies from,
    /// velocity), and the cells.
    out: Vec<(u64, (i64, i64), (f64, f64), (f64, f64))>,
    cells: CellSet,
    stage: CraterStage,
}

impl Crater {
    /// The depth wandering across it (smooth noise).
    fn wander(&self, s: f64) -> f64 {
        let un = |k: i64| crate::noise::unit(hash(self.blast, k, 0, 0xC2));
        let t = s / self.period;
        let i = t.floor();
        let f = t - i;
        let f = f * f * (3.0 - 2.0 * f);
        un(i as i64) * (1.0 - f) + un(i as i64 + 1) * f
    }

    /// Only the top layers of a big bowl are thrown (CRATER_MAX cells), so
    /// only the share of its depth with about `want` places in it is looked
    /// through (each cell is met about twice): a big crater's whole bowl was
    /// ten times as many cells, and looking at each was most of its time.
    fn share_for(&self) -> f64 {
        let places = |q: f64| self.cols.iter().map(|c| ((q * c.2.max(1.0)).min(c.2) / 0.7).floor() + 1.0).sum::<f64>();
        if places(1.0) <= self.want {
            return 1.0;
        }
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..30 {
            let q = (lo + hi) / 2.0;
            if places(q) < self.want {
                lo = q;
            } else {
                hi = q;
            }
        }
        hi
    }
}

/// What a blast still has to do once all its rubble is made (its rays
/// have all given out, the column is loosened and the crater thrown), while
/// the shock's cracks may still be running out.
#[derive(Clone, Copy)]
enum Fin {
    /// Rubble is still being made.
    Rubble,
    /// The plume is next.
    Plume,
    /// Finding the rubble still here (cells gone through).
    Count(usize),
    /// Doping it with the unreacted uranium (cells gone through, kg each).
    Dope(usize, f64),
    Done,
}

/// A sapper's blast going off (Benjamin's idea, 26 Nov: the shock as a
/// moving front). Its front runs out from the charge FRONT_MPS, and each
/// tick the rock between last tick's front and this tick's is dealt with:
/// the rays step on to it, and each cell they break is held, melted, thrown
/// or left as rubble right then; the column is loosened and the rock round
/// it cracked, shell by shell by distance from the charge; the crater is
/// thrown out once the front reaches the ground over it. Once all the
/// rubble is made, the plume and the uranium left in the rubble follow,
/// while the shock's cracks may still be running out. The work is capped
/// per tick (BLAST_BUDGET), so a blast of any yield costs little a tick.
/// Not saved: `finish_blasts` before a save.
pub(super) struct BlastJob {
    at: (f64, f64),
    c0: (i64, i64),
    /// Its radius (m), and the column's half-width at the top in radii.
    r: f64,
    lt: f64,
    yield_kt: f64,
    /// Kg of unreacted uranium still to leave through the rubble.
    unreacted: f64,
    /// Its roll of the dice.
    blast: u64,
    /// Whether it is on screen when it goes off (out of sight it is
    /// approximated: nothing thrown, the heart straight to glass).
    seen: bool,
    up: (f64, f64),
    side: (f64, f64),
    /// How far the ground is over the charge (0: no column), and the
    /// ground there.
    height: f64,
    top: (f64, f64),
    /// Whether the cutters in the column have been caught a second time,
    /// once the front reached the ground.
    harmed: bool,
    /// How hard it throws; the share of what could be thrown that is; how
    /// many cells are likely to be able to be thrown, and have been so far;
    /// and how many have been.
    power: f64,
    throw_share: f64,
    likely: f64,
    cand: u64,
    thrown: usize,
    rays: Vec<Ray>,
    /// The rays still going (or whose cracks are), how many have not given
    /// out yet, and how far they can go.
    live: Vec<u32>,
    going: usize,
    reach: i64,
    /// Where each ray stopped breaking rock (m; `reach` if it never did).
    stop: Vec<f64>,
    /// Once all the rays are done: how far out their cracks reach (m).
    ray_end: Option<f64>,
    /// What it broke, with the ray's energy left there and the distance.
    broken: CellMap<(f64, f64)>,
    /// What it left as rubble where it was, and of that what is still
    /// there once the rubble is all made (to dope).
    rubble: Vec<(i64, i64)>,
    here: Vec<(i64, i64)>,
    /// How far the front has run (m), the shells done, and whether it has
    /// run its course (past the column and every crack).
    front: f64,
    shell: u64,
    front_done: bool,
    /// The farthest the column reaches from the charge (m).
    column_end: f64,
    blocks: [Blocks; 4],
    /// Its crater, worked out ahead of the front (on screen, with a
    /// column), and the cells thrown out once the front reaches the ground.
    plan: Option<Crater>,
    crater: CellSet,
    crater_done: bool,
    cratered: bool,
    /// (The cells of a shell, kept to save allocating them every shell.)
    cells: Vec<(i64, i64)>,
    fin: Fin,
}

impl BlastJob {
    /// Whether it has run its course: its front, and all it does after.
    fn done(&self) -> bool {
        self.front_done && matches!(self.fin, Fin::Done)
    }
}

impl Sim {
    /// A sapper's blast radius (m) for its yield.
    pub(super) fn sapper_radius(&self) -> f64 {
        let sp = &self.eco.units.sapper;
        sp.radius_m_per_cbrt_kt * sp.yield_kt.max(0.0).cbrt()
    }

    /// How deep a sapper goes off (m under the ground where it landed): from
    /// its yield (Benjamin, 25 Nov), or the old rule (blast radius + bury_m).
    pub(super) fn sapper_depth(&self) -> f64 {
        let sp = &self.eco.units.sapper;
        if sp.depth_m_per_cbrt_kt > 0.0 {
            sp.depth_m_per_cbrt_kt * sp.yield_kt.max(0.0).cbrt()
        } else {
            self.sapper_radius() + sp.bury_m
        }
    }

    /// How much slower than in hardness-3 rock flock `i` has been cutting
    /// lately, per kg (1: as fast; 0 if it has cut too little to tell).
    pub(super) fn flock_stall(&self, i: usize) -> f64 {
        let f = &self.flock[i];
        // At least a few loads' worth lately, or it can't be told.
        if f.kg < 3.0 * self.eco.harvester.cargo_kg {
            return 0.0;
        }
        let soft = self.eco.harvester.dig_s_for(3.0) / self.eco.harvester.cargo_kg.max(1.0) / (self.up("dig speed") * self.perk_dig).max(1e-9);
        f.dig_s / f.kg / soft.max(1e-12)
    }

    /// Flocks where the cutters are struggling (cutting `stall_ratio` times
    /// slower than in soft rock), worst first, leaving out those a sapper is
    /// already headed for: (flock, how much slower).
    pub(super) fn stalled_sites(&self) -> Vec<(usize, f64)> {
        let sp = &self.eco.units.sapper;
        let keep_off = self.sapper_radius() * sp.loosen_top.max(1.0) * 2.0;
        let mut out: Vec<(usize, f64)> = self
            .live_sites()
            .into_iter()
            .filter_map(|i| {
                let k = self.flock_stall(i);
                if k < sp.stall_ratio {
                    return None;
                }
                let (x, y) = self.site_pos(i);
                let taken = self.units.sappers.iter().any(|s| dist2(s.goal, (x, y)) < keep_off * keep_off);
                (!taken).then_some((i, k))
            })
            .collect();
        out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        out
    }

    /// Where a sapper is wanted: the flock struggling most (with need-driven
    /// ordering), else the flock where the going is hardest (slowest per kg
    /// lately), away from where other sappers are headed.
    pub(super) fn sapper_site(&self) -> Option<usize> {
        if self.eco.units.sapper.stall_ratio > 0.0 {
            if let Some(&(i, _)) = self.stalled_sites().first() {
                return Some(i);
            }
        }
        let sites = self.live_sites();
        let mut best: Option<(usize, f64)> = None;
        for &i in &sites {
            let f = &self.flock[i];
            let hard = if f.kg > 1.0 { f.dig_s / f.kg * 1000.0 } else { 1.0 };
            let (x, y) = self.site_pos(i);
            let taken = self.units.sappers.iter().filter(|s| dist2(s.goal, (x, y)) < 80.0 * 80.0).count();
            let jitter = crate::noise::unit(hash(self.tick, i as i64, self.units.next_id as i64, 0x5A9)) * 0.5;
            let score = hard * (1.0 + jitter) / (1.0 + 4.0 * taken as f64);
            if best.is_none_or(|b| score > b.1) {
                best = Some((i, score));
            }
        }
        best.map(|b| b.0)
    }

    pub(super) fn launch_sapper(&mut self) {
        // Its charge, as cast when its bay started it (one made free to
        // watch, a new one: it takes nothing the swarm made).
        let fissile = if self.units.summoning { self.fissile_per_charge() } else { self.units.charges.pop_front().unwrap_or_else(|| self.fissile_per_charge()) };
        // No flock on view struggling, but a far sector is: it goes off
        // there, as math.
        let near = self.eco.units.sapper.stall_ratio <= 0.0 || !self.stalled_sites().is_empty();
        if !near {
            if let Some(&(k, _)) = self.far_stalled().first() {
                self.far_sapper_blast(k);
                return;
            }
        }
        let Some(site) = self.sapper_site() else { return };
        let (hx, hy) = self.site_pos(site);
        let a = hy.atan2(hx);
        let g = self.ground_at(a, true);
        let goal = (a.cos() * (g + 1.0), a.sin() * (g + 1.0));
        let start = self.mother_over(a);
        let id = self.units.new_id();
        let want = self.sapper_depth();
        self.units.sappers.push(Sapper { id, state: SapState::Fly, pos: start, prev: start, moved_at: self.tick, goal, path: Vec::new(), along: 0.0, drilled: 0.0, armed_s: 0.0, want, ground_r: g, fissile, side: (0.0, 0.0), sideways: 0.0 });
        self.director_log_push(format!("a sapper is sent where the cutters struggle ({:.0} m deep, {} kt)", want, self.eco.units.sapper.yield_kt));
    }

    /// Where a blast at `at` would catch a tunneler (its head or its body
    /// within the loosened column and a margin): the nearest such point.
    pub(super) fn tunneler_in_blast(&self, at: (f64, f64)) -> Option<(f64, f64)> {
        let sp = &self.eco.units.sapper;
        let r = self.sapper_radius();
        let clear = r * sp.loosen_top.max(1.0) + 2.0 * sp.kill_margin_m + 12.0;
        self.tunneler_points()
            .into_iter()
            .filter(|&p| dist2(p, at) < clear * clear)
            .min_by(|a, b| dist2(*a, at).total_cmp(&dist2(*b, at)))
    }

    /// From where it landed, its way down: the deepest open cell it can
    /// crawl to (along cracks and through caves, always touching rock)
    /// within crawl_m, stopping at the first deep enough.
    /// Spots within `keep_off` of `taken` (where other sappers are headed)
    /// are left to them.
    pub(super) fn sapper_path(&self, from: (f64, f64), ground_r: f64, want: f64, limit_m: f64, taken: &[(f64, f64)], keep_off: f64) -> Vec<(i64, i64)> {
        let start = cell_of(from);
        let solid = |c: (i64, i64)| self.world.material(c.0, c.1) != VACUUM;
        let crawlable = |c: (i64, i64)| {
            !solid(c) && len(mid(c)) < ground_r + 3.0 && [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, 1), (1, -1), (-1, -1)].iter().any(|d| solid((c.0 + d.0, c.1 + d.1)))
        };
        let mut parent: std::collections::HashMap<(i64, i64), ((i64, i64), u32)> = std::collections::HashMap::new();
        let mut queue = std::collections::VecDeque::new();
        parent.insert(start, (start, 0));
        queue.push_back(start);
        let (mut best, mut best_depth) = (start, ground_r - len(mid(start)));
        while let Some(c) = queue.pop_front() {
            let (_, d) = parent[&c];
            let depth = ground_r - len(mid(c));
            let claimed = taken.iter().any(|&t| dist2(t, mid(c)) < keep_off * keep_off);
            if depth > best_depth && !claimed {
                best = c;
                best_depth = depth;
                if depth >= want {
                    break;
                }
            }
            if d as f64 >= limit_m || parent.len() > 40_000 {
                continue;
            }
            for (dx, dy) in [(0, -1), (1, 0), (-1, 0), (0, 1), (1, -1), (-1, -1), (1, 1), (-1, 1)] {
                let n = (c.0 + dx, c.1 + dy);
                if parent.contains_key(&n) || !crawlable(n) {
                    continue;
                }
                parent.insert(n, (c, d + 1));
                queue.push_back(n);
            }
        }
        let mut path = vec![best];
        let mut c = best;
        while c != start {
            c = parent[&c].0;
            path.push(c);
        }
        path.reverse();
        path
    }

    pub(super) fn sappers_work(&mut self) {
        if self.units.sappers.is_empty() {
            return;
        }
        let sp = self.eco.units.sapper.clone();
        let mut blasts = Vec::new();
        for k in 0..self.units.sappers.len() {
            let mut s = self.units.sappers[k].clone();
            s.prev = s.pos;
            s.moved_at = self.tick;
            // Anything that destroys a sapper sets it off: buried by falling
            // rock, or touched by magma or a blast's heat.
            // (Not while drilling: its own spoil falling back into the hole
            // is pushed up past it by the drill. Drilling into magma it stops
            // and arms, and the magma then destroys it.)
            // Buried while crawling (rubble came down on it): it drills on.
            if s.state == SapState::Crawl && {
                let c = cell_of(s.pos);
                let m = self.world.material(c.0, c.1);
                m != VACUUM && self.world.planet.mats.get(m).state == State::Solid
            } {
                s.state = SapState::Drill;
            }
            if !matches!(s.state, SapState::Fly | SapState::Drill) && s.armed_s >= 0.0 && !(s.state == SapState::Arm && s.armed_s >= sp.arm_s) && self.sapper_destroyed(s.pos) {
                self.units.destroyed += 1;
                blasts.push((s.pos, s.fissile));
                s.armed_s = -1.0;
                self.units.sappers[k] = s;
                continue;
            }
            match s.state {
                SapState::Fly => {
                    let over = self.hover_over(s.goal);
                    if approach(&mut s.pos, over, sp.fly_speed) {
                        s.state = SapState::Crawl;
                        // Where the others are going (the end of their way).
                        let taken: Vec<(f64, f64)> = self.units.sappers.iter().filter(|o| o.id != s.id && !o.path.is_empty()).map(|o| mid(*o.path.last().unwrap())).collect();
                        let r = sp.radius_m_per_cbrt_kt * sp.yield_kt.max(0.0).cbrt();
                        s.path = self.sapper_path(s.goal, s.ground_r, s.want, sp.crawl_m, &taken, 0.7 * r);
                        s.along = 0.0;
                    }
                }
                SapState::Crawl => {
                    s.along += sp.crawl_speed;
                    let last = s.path.len().saturating_sub(1);
                    let i = (s.along as usize).min(last);
                    // Along its way hovering over the ground (Benjamin, 26
                    // Nov), then down to its bottom to bore or go off.
                    if let Some(&c) = s.path.get(i) {
                        s.pos = if i >= last { mid(c) } else { self.hover_over(mid(c)) };
                    }
                    if i >= last {
                        s.state = if s.ground_r - len(s.pos) >= s.want { SapState::Arm } else { SapState::Drill };
                    }
                }
                SapState::Drill => {
                    // Straight down (or sideways, keeping clear of a
                    // tunneler), a shaft as wide as its hull; what it bores
                    // through spouts up out of the hole and lands round it.
                    let down = unit_of(s.pos);
                    let sideways = s.side != (0.0, 0.0);
                    let dir = if sideways { s.side } else { (-down.0, -down.1) };
                    let mut stop = false;
                    for _ in 0..sp.drill_speed.ceil().max(1.0) as usize {
                        let next = (s.pos.0 + dir.0, s.pos.1 + dir.1);
                        let across = (-dir.1, dir.0);
                        for o in -SHAFT_HALF..=SHAFT_HALF {
                            let o = o as f64;
                            let q = cell_of((next.0 + across.0 * o, next.1 + across.1 * o));
                            let m = self.world.material(q.0, q.1);
                            if m != VACUUM && self.world.planet.mats.get(m).state == State::Liquid {
                                stop = true;
                            }
                        }
                        if stop {
                            // Drilled into liquid: magma destroys it (it goes
                            // off at once, next second); water or brine it
                            // just stops at and arms.
                            break;
                        }
                        for (k, o) in (-SHAFT_HALF..=SHAFT_HALF).enumerate() {
                            let o = o as f64;
                            let q = cell_of((next.0 + across.0 * o, next.1 + across.1 * o));
                            let Some(m) = self.world.lift_cell(q.0, q.1) else { continue };
                            self.undope(q, false);
                            self.progress.remove(&q);
                            self.reserved.remove(&q);
                            self.open_cell(q);
                            let h = hash(self.tick, q.0, q.1, 0xD811 + k as u64);
                            let vs = 45.0 + 35.0 * crate::noise::unit(h);
                            let vx = (crate::noise::unit(hash(h, 1, 0, 0)) - 0.5) * 3.0;
                            let side = (-down.1, down.0);
                            let from = (s.pos.0 + down.0 * 2.0, s.pos.1 + down.1 * 2.0);
                            if let Some(t) = self.bodies.throw(&mut self.world, from, m, (down.0 * vs + side.0 * vx, down.1 * vs + side.1 * vx)) {
                                self.loose.insert(t);
                                self.world.set_look(t.0, t.1, Look::RUBBLE);
                                self.settling.insert(t);
                                self.scrap_landed(t);
                            }
                        }
                        s.pos = next;
                        if sideways {
                            s.sideways += 1.0;
                            if s.sideways >= SIDEWAYS_MAX_M || self.tunneler_in_blast(s.pos).is_none() {
                                stop = true;
                                break;
                            }
                        } else {
                            s.drilled += 1.0;
                            if s.ground_r - len(s.pos) >= s.want || s.drilled >= sp.drill_m {
                                stop = true;
                                break;
                            }
                        }
                    }
                    if stop {
                        s.state = SapState::Arm;
                        s.side = (0.0, 0.0);
                    }
                }
                SapState::Arm => {
                    // A tunneler within reach of its blast: it drills
                    // sideways away from it first (Benjamin: sappers never
                    // catch a tunneler; they can dig sideways to steer clear).
                    let near = self.tunneler_in_blast(s.pos);
                    if near.is_some() && s.sideways >= SIDEWAYS_MAX_M {
                        // As far sideways as it goes: it waits, armed, for
                        // the tunneler to move on.
                        s.armed_s = 0.0;
                    } else if let Some(t) = near {
                        let down = unit_of(s.pos);
                        let side = (-down.1, down.0);
                        let away = if (s.pos.0 - t.0) * side.0 + (s.pos.1 - t.1) * side.1 >= 0.0 { side } else { (-side.0, -side.1) };
                        s.side = away;
                        s.state = SapState::Drill;
                        s.armed_s = 0.0;
                    } else {
                        s.armed_s += 1.0;
                        if s.armed_s >= sp.arm_s {
                            blasts.push((s.pos, s.fissile));
                            s.armed_s = -1.0;
                        }
                    }
                }
            }
            self.units.sappers[k] = s;
        }
        self.units.sappers.retain(|s| s.armed_s >= 0.0);
        for (at, fissile) in blasts {
            self.sapper_blast(at, sp.yield_kt, fissile);
        }
    }

    /// Where a sapper hovers over the ground at `p`: HOVER_M up, or less
    /// under a low roof.
    pub(super) fn hover_over(&self, p: (f64, f64)) -> (f64, f64) {
        let up = unit_of(p);
        let mut h = 0.0;
        while h < HOVER_M + 1.5 {
            let c = cell_of((p.0 + up.0 * (h + 1.0), p.1 + up.1 * (h + 1.0)));
            if self.world.material(c.0, c.1) != VACUUM {
                break;
            }
            h += 1.0;
        }
        let h = (h - 1.5).clamp(0.0, HOVER_M);
        (p.0 + up.0 * h, p.1 + up.1 * h)
    }

    /// A test: a charge of `kt` going off under the ground at `over`, as
    /// deep as a sapper would bury it (the game's --test-blast, for seeing
    /// how big a blast the screen can show).
    pub fn test_blast(&mut self, over: (f64, f64), kt: f64) {
        let sp = &self.eco.units.sapper;
        let depth = if sp.depth_m_per_cbrt_kt > 0.0 { sp.depth_m_per_cbrt_kt * kt.max(0.0).cbrt() } else { sp.radius_m_per_cbrt_kt * kt.max(0.0).cbrt() + sp.bury_m };
        let a = over.1.atan2(over.0);
        let g = self.ground_at(a, true);
        let at = (a.cos() * (g - depth), a.sin() * (g - depth));
        let fissile = self.fissile_per_charge();
        self.sapper_blast(at, kt, fissile);
    }

    /// Whether whatever is round a sapper at `p` destroys it: magma, or
    /// ground hot from a blast, touching it.
    pub(super) fn sapper_destroyed(&self, p: (f64, f64)) -> bool {
        let c = cell_of(p);
        let m = self.world.material(c.0, c.1);
        // (Rock or rubble coming down on it doesn't: it is sealed in, which
        // a charge doesn't mind, and one buried while crawling drills on.
        // Magma flowing over it does.)
        if m != VACUUM && self.units.freeze_to.get(m as usize).copied().flatten().is_some() {
            return true;
        }
        for d in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (c.0 + d.0, c.1 + d.1);
            let m = self.world.material(n.0, n.1);
            if m == VACUUM {
                continue;
            }
            if self.units.freeze_to.get(m as usize).copied().flatten().is_some() {
                return true;
            }
            if self.sandbox.temps.get(&n).is_some_and(|&k| k >= SAPPER_COOK_K) {
                return true;
            }
        }
        false
    }

    /// Sappers within `r` of a blast at `at` are destroyed by it, and go
    /// off a moment later (the next second): one blast can set off a chain.
    pub(super) fn sappers_caught(&mut self, at: (f64, f64), r: f64) {
        let mut caught = Vec::new();
        for s in self.units.sappers.iter_mut() {
            if s.armed_s >= 0.0 && dist2(s.pos, at) < r * r {
                // Gone from the field; it goes off a moment later.
                s.armed_s = -1.0;
                self.units.destroyed += 1;
                caught.push((s.id, s.pos, s.fissile));
            }
        }
        self.units.sappers.retain(|s| s.armed_s >= 0.0);
        for (id, pos, fissile) in caught {
            let d = 0.12 + 0.1 * crate::noise::unit(hash(self.tick, id as i64, 0, 0xC4A1));
            let due = self.tick + self.ticks(d);
            self.chain.push((due, pos, fissile));
        }
    }

    /// The blasts' moving parts, every tick: sappers caught in a blast going
    /// off, the shock fronts of blasts going off, the crater's ground thrown
    /// out, broken rock pouring out of a plume, and loose rubble and melt
    /// settling.
    pub(super) fn blast_tick(&mut self) {
        if !self.chain.is_empty() {
            let tick = self.tick;
            let (now, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.chain).into_iter().partition(|c| c.0 <= tick);
            self.chain = later;
            let y = self.eco.units.sapper.yield_kt;
            for (_, at, fissile) in now {
                self.sapper_blast(at, y, fissile);
            }
        }
        self.blasts_advance();
        // The crater's ground due now: all of it lifted first, then thrown
        // (thrown one by one, a piece could fly into a neighbour due the
        // same tick but not yet lifted, and fall back in).
        let mut flying = Vec::new();
        while let Some(&(due, c, from, vel)) = self.eject.front() {
            if due > self.tick {
                break;
            }
            self.eject.pop_front();
            let Some(m) = self.world.lift_cell(c.0, c.1) else { continue };
            self.loose.remove(&c);
            self.progress.remove(&c);
            self.reserved.remove(&c);
            self.open_cell(c);
            flying.push((from, m, vel));
        }
        for (from, m, vel) in flying {
            if let Some(t) = self.bodies.throw(&mut self.world, from, m, vel) {
                self.loose.insert(t);
                self.world.set_look(t.0, t.1, Look::RUBBLE);
                self.settling.insert(t);
                self.scrap_landed(t);
            }
        }
        while let Some(&(due, from, m, vel)) = self.plume_out.front() {
            if due > self.tick {
                break;
            }
            self.plume_out.pop_front();
            if let Some(t) = self.bodies.throw(&mut self.world, from, m, vel) {
                self.loose.insert(t);
                self.world.set_look(t.0, t.1, Look::RUBBLE);
                self.settling.insert(t);
                self.scrap_landed(t);
            }
        }
        if !self.settling.is_empty() {
            let q0 = clock();
            self.settle();
            self.prof2[1] += 0.0 * q0;
        }
    }

    /// Loose rubble settles, as in the proving ground: down, or down a
    /// slope (both cells beside it open); melt runs sideways too, into the
    /// cracks. Bottom first. Cells that can't move stop being looked at
    /// until something beside them opens. At most SETTLE_LOOK cells are
    /// looked at a tick and SETTLE_PER_TICK moved; the rest wait their turn.
    pub(super) fn settle(&mut self) {
        // Nearest the centre first (the proving ground goes bottom up).
        let cells = self.settling.take_nearest(SETTLE_LOOK);
        let mut moves = 0usize;
        for c in cells {
            if moves >= SETTLE_PER_TICK {
                self.settling.insert(c);
                continue;
            }
            let m = self.world.material(c.0, c.1);
            if m == VACUUM || !self.loose.contains(&c) {
                continue;
            }
            let look = self.world.look(c.0, c.1);
            if look == Look::GLASS {
                continue;
            }
            // Out of sight it waits, and settles once the view comes over it.
            if !self.sandbox.sees(c) {
                self.settle_wait.insert(c);
                continue;
            }
            let melt = look == Look::MELT;
            let p = mid(c);
            let r = len(p).max(1.0);
            let step = |u: f64| if u > 0.38 { 1 } else if u < -0.38 { -1 } else { 0 };
            let down = (-step(p.0 / r), -step(p.1 / r));
            let side = (-down.1, down.0);
            let open = |q: (i64, i64)| self.world.material(q.0, q.1) == VACUUM;
            let h = hash(self.tick, c.0, c.1, 0x5E77);
            let s = if h & 1 == 0 { 1 } else { -1 };
            let below = (c.0 + down.0, c.1 + down.1);
            let mut to = None;
            if open(below) {
                to = Some(below);
            } else {
                for k in [s, -s] {
                    let sd = (c.0 + side.0 * k, c.1 + side.1 * k);
                    let dg = (below.0 + side.0 * k, below.1 + side.1 * k);
                    if open(sd) && open(dg) {
                        to = Some(dg);
                        break;
                    }
                }
                if to.is_none() && melt {
                    let sd = (c.0 + side.0 * s, c.1 + side.1 * s);
                    if open(sd) && crate::noise::unit(hash(h, 1, 0, 0)) < 0.6 {
                        to = Some(sd);
                    }
                }
            }
            let Some(n) = to else { continue };
            if !self.world.swap_cells(c, n) {
                continue;
            }
            moves += 1;
            self.loose.remove(&c);
            self.loose.insert(n);
            if let Some(u) = self.units.dope.remove(&c) {
                self.units.dope.insert(n, u);
            }
            self.progress.remove(&c);
            self.reserved.remove(&c);
            self.sandbox.wake_around(c);
            self.sandbox.wake_around(n);
            self.apply_sandbox(vec![crate::sandbox::Change { cell: c, from: m, to: VACUUM }, crate::sandbox::Change { cell: n, from: VACUUM, to: m }]);
            self.settling.insert(n);
            // Melt that flowed takes its cooling with it (the queue's entry
            // for where it was finds no melt there; before this, melt that
            // had run never cooled and glowed on: Benjamin, 26 Nov). It cools
            // a couple of seconds on, or with the last melt queued if later,
            // keeping the queue in order.
            if melt {
                let due = (self.tick + self.ticks(2.0)).max(self.melt.back().map_or(0, |b| b.0));
                self.melt.push_back((due, n));
            }
            // What rested on it may follow.
            self.unsettle_around(c);
        }
    }

    /// Loose cells round an opened cell `c` may be free to move again.
    pub(super) fn unsettle_around(&mut self, c: (i64, i64)) {
        if self.loose.is_empty() {
            return;
        }
        for d in [(0, 1), (0, -1), (1, 0), (-1, 0), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let q = (c.0 + d.0, c.1 + d.1);
            if self.loose.contains(&q) {
                self.settling.insert(q);
            }
        }
    }

    /// A sapper goes off at `at`, as the sapper proving ground
    /// (mockups/sapper-proving-ground.html) has it, as a shock front running
    /// out from the charge over the next ticks (see `BlastJob`):
    /// - rays out from the charge, each spending its energy on what it passes
    ///   through (little in open passages and rubble, far more in hard rock);
    ///   everything a ray gets through is broken; where one gives out in
    ///   rock, now and then a crack runs on, further in harder rock;
    /// - hard rock away from the heart does not always go to rubble: some of
    ///   it holds together in big pieces, split off along seams (cracked);
    /// - the heart of it melts; of the rest, what the blast still had plenty
    ///   of energy for is thrown clear (softer rock harder), the rest left
    ///   loose where it was, as rubble;
    /// - the loosened column up to the surface, the crater thrown out over
    ///   it once the front reaches the ground, and the shock's cracks round it;
    /// - a plume: if there is an open way from the blast up to the surface
    ///   (cracks, caves, its drill hole, through the rubble), most blasts
    ///   drive a share of the rubble up it and out of the top;
    /// - sappers caught in it go off too.
    /// Rubble, melt and thrown rock are loose: cutters pick them up quickly.
    /// `fissile`: kilograms of uranium in its charge (0: conventional);
    /// what didn't react is partly left in the rubble.
    pub fn sapper_blast(&mut self, at: (f64, f64), yield_kt: f64, fissile: f64) {
        let q0 = clock();
        self.blast_start(at, yield_kt, fissile);
        self.prof2[7] += clock() - q0;
    }

    /// What happens the moment a charge goes off: the flash, the counts, the
    /// drones and sappers caught in it; then its shock front is set going.
    pub(super) fn blast_start(&mut self, at: (f64, f64), yield_kt: f64, fissile: f64) {
        let sp = &self.eco.units.sapper;
        let r = sp.radius_m_per_cbrt_kt * yield_kt.max(0.0).cbrt();
        let column = sp.depth_m_per_cbrt_kt > 0.0;
        let lt = sp.loosen_top.max(1.0);
        let unreacted = (fissile - yield_kt / sp.kt_per_kg_fissioned.max(1e-9)).max(0.0) * sp.unreacted_doped.clamp(0.0, 1.0);
        self.booms.push(Boom { x: at.0, y: at.1, size_m: (2.2 * r) as f32, buried: true });
        self.explosions += 1;
        self.units.blasts += 1;
        self.happen(Happening::Explosion, at);
        let c0 = cell_of(at);
        let blast = hash(self.tick, c0.0, c0.1, 0x5A99);
        // Out of sight (no part of it on screen) it is approximated: the
        // heart goes straight to glass, and nothing is thrown or blown out of
        // a plume (it all lies as rubble where it broke).
        let seen = {
            let k = (r * 2.4).ceil() as i64;
            [(0, 0), (k, k), (k, -k), (-k, k), (-k, -k), (k, 0), (-k, 0), (0, k), (0, -k)].iter().any(|d| self.sandbox.sees((c0.0 + d.0, c0.1 + d.1)))
        };
        let up = unit_of(at);
        let side = (-up.1, up.0);
        // The surface over it, looked at either side of its own shaft
        // (straight down the shaft, the first solid cell is the bottom of the
        // hole): the loosened column runs up to it (Benjamin, 25 Nov: the
        // yield sets the depth, deep enough that the ground over it is
        // loosened for the cutters to scoop up).
        let (top, height) = if column {
            let ground = self.ground_beside(at, r);
            let a = at.1.atan2(at.0);
            ((a.cos() * ground, a.sin() * ground), (ground - len(at)).max(1.0))
        } else {
            (at, 0.0)
        };
        let rays = ((r * 10.0).round() as usize).clamp(360, 5000);
        let reach = (r * 2.4).min(1500.0) as i64;
        let ray_list: Vec<Ray> = (0..rays)
            .map(|k| {
                let a = k as f64 / rays as f64 * std::f64::consts::TAU + (crate::noise::unit(hash(blast, k as i64, 0, 5)) - 0.5) * 0.01;
                let (ca, sa) = crate::noise::cos_sin(a);
                Ray { a, ca, sa, e: 1.0, step: 0, stopped: false, crack: None }
            })
            .collect();
        // How much of what could be thrown flies: THROW_MAX over how many
        // cells are likely to be thrown, worked out up front so each cell
        // can be decided the moment it breaks. In rock as hard as round the
        // charge, one ray alone has energy enough to throw a cell between
        // where the heart's melt ends and where its energy falls under the
        // throw's bar; but near the charge the rays cross each other's
        // broken rock, which costs them little, and carry on further, so
        // measured (10 to 10,000 kt) about 4.5 times that ring's area could
        // fly. Aimed a little under THROW_MAX, and put right as the front
        // goes should more turn up than that (see `blast_work`), so the cap
        // seldom cuts off the outer part of the throw.
        let (mut sum, mut n) = (0.0, 0);
        for k in 0..16 {
            let (ca, sa) = crate::noise::cos_sin(k as f64 / 16.0 * std::f64::consts::TAU);
            let c = cell_of((at.0 + ca * r * 0.33, at.1 + sa * r * 0.33));
            let m = self.world.material(c.0, c.1);
            if m != VACUUM && self.world.planet.mats.get(m).state == State::Solid {
                sum += blast_hardness(self, m);
                n += 1;
            }
        }
        let h = if n > 0 { sum / n as f64 } else { 0.5 };
        let outer = r * (0.65 - 0.35 * h) / (0.45 + 1.7 * h);
        let inner = r * 0.3 * (1.15 - 0.4 * h);
        let likely = 4.5 * std::f64::consts::PI * (outer * outer - inner * inner).max(0.0);
        let throw_share = (THROW_MAX as f64 / likely.max(1.0)).min(1.0);
        // The box the cracks' blocks are kept over: as far as the cracks can
        // reach (CRACK_REACH past the farthest a ray can go, or beside the
        // column).
        let far = reach as f64 + r * CRACK_REACH;
        let reach_all = far.max((height + 2.0).hypot(r * lt + r)) + 2.0;
        let (x0, y0, x1, y1) = ((at.0 - reach_all) as i64, (at.1 - reach_all) as i64, (at.0 + reach_all) as i64, (at.1 + reach_all) as i64);
        let blocks = [3.5, 5.0, 7.0, 10.0].map(|size| Blocks::new(size, blast, x0, y0, x1, y1));
        let plan = (seen && column).then(|| self.crater_plan(at, r, yield_kt, height, blast));
        let column_end = if column { ((height + 1.0).powi(2) + (r * lt).powi(2)).sqrt() + 1.0 } else { 0.0 };
        self.blast_jobs.push(BlastJob {
            at,
            c0,
            r,
            lt,
            yield_kt,
            unreacted,
            blast,
            seen,
            up,
            side,
            height,
            top,
            harmed: false,
            power: (0.6 + yield_kt.max(0.0).cbrt() * 0.4).min(3.0),
            throw_share,
            likely,
            cand: 0,
            thrown: 0,
            rays: ray_list,
            live: (0..rays as u32).collect(),
            going: rays,
            reach,
            stop: vec![reach as f64; rays],
            ray_end: None,
            broken: CellMap::default(),
            rubble: Vec::new(),
            here: Vec::new(),
            front: 0.0,
            shell: 0,
            front_done: false,
            column_end,
            blocks,
            crater_done: plan.is_none(),
            plan,
            crater: CellSet::default(),
            cratered: false,
            cells: Vec::new(),
            fin: Fin::Rubble,
        });
        // Sappers caught in it go off too, a moment later.
        self.sappers_caught(at, r * 0.9);
        // Drones caught in it: the director never pulls them back.
        self.blast_harm(at, top, r, true);
    }

    /// Blasts going off, every tick: each one's shock front moves on
    /// FRONT_MPS, and the rock it has passed is broken, loosened and cracked
    /// up to it, within BLAST_BUDGET between them (past that, a front lags
    /// and catches up later). Each blast gets an even share first, so one
    /// going off later (a chain blast, a second sapper) breaks its ground
    /// at once and doesn't wait for a big one before it; what a blast leaves
    /// of its share goes to the others, those on screen first, then the
    /// oldest first.
    pub(super) fn blasts_advance(&mut self) {
        if self.blast_jobs.is_empty() {
            return;
        }
        let q0 = clock();
        let mut jobs = std::mem::take(&mut self.blast_jobs);
        // (A stable sort: otherwise the oldest first, the same every run.)
        jobs.sort_by_key(|j| !j.seen);
        let run = FRONT_MPS * self.eco.tick_s;
        for j in jobs.iter_mut() {
            j.front += run;
        }
        let share = BLAST_BUDGET / jobs.len() as u64;
        let mut left = BLAST_BUDGET;
        for j in jobs.iter_mut() {
            let mut b = share;
            self.blast_work(j, &mut b);
            left -= share - b;
        }
        for j in jobs.iter_mut() {
            if left == 0 {
                break;
            }
            if !j.done() {
                self.blast_work(j, &mut left);
            }
        }
        jobs.retain(|j| !j.done());
        jobs.append(&mut self.blast_jobs);
        self.blast_jobs = jobs;
        self.prof2[7] += clock() - q0;
    }

    /// Every blast going off is finished at once, and the sappers caught in
    /// one go off now rather than a moment later (a save keeps no blast half
    /// done, and doesn't keep those: call this before `write_save`).
    pub fn finish_blasts(&mut self) {
        while !self.chain.is_empty() {
            let y = self.eco.units.sapper.yield_kt;
            for (_, at, fissile) in std::mem::take(&mut self.chain) {
                self.sapper_blast(at, y, fissile);
            }
        }
        let mut jobs = std::mem::take(&mut self.blast_jobs);
        for j in jobs.iter_mut() {
            j.front = f64::INFINITY;
            let mut budget = u64::MAX;
            self.blast_work(j, &mut budget);
        }
        jobs.retain(|j| !j.done());
        jobs.append(&mut self.blast_jobs);
        self.blast_jobs = jobs;
    }

    /// How many blasts are still going off (their fronts still running, or
    /// their rubble still being gone through).
    pub fn blasts_pending(&self) -> usize {
        self.blast_jobs.len()
    }

    /// One blast's work, shell by shell out to its front while the budget
    /// lasts, and what is done once its rubble is all made.
    fn blast_work(&mut self, j: &mut BlastJob, budget: &mut u64) {
        self.blast_fin(j, budget);
        // The crater is worked out first, with up to half the budget, so
        // that it is ready when the front reaches the ground.
        if let Some(cr) = j.plan.as_mut().filter(|cr| cr.stage != CraterStage::Ready) {
            let mut half = *budget / 2;
            let had = half;
            self.crater_work(cr, &mut half);
            *budget -= had - half;
        }
        // The ground right over it thrown out, leaving a crater (in sight),
        // once the shock reaches it: by the clock, not by how far the
        // budget has got through the rock (a big blast's cracks take a
        // while to work out, and its crater sat still for some 15 s
        // waiting for them; 27 Nov). What it throws is planned on the
        // ground as it was; the rays and column then meet open air there.
        // (Should it not be worked out yet, it goes a little late.)
        if !j.crater_done && j.height > 0.0 && j.front >= j.height && j.plan.as_ref().is_some_and(|cr| cr.stage == CraterStage::Ready) {
            if let Some(cr) = j.plan.take() {
                j.crater = self.crater_throw(cr);
            }
            j.cratered = !j.crater.is_empty();
            j.crater_done = true;
            *budget = budget.saturating_sub(j.crater.len() as u64);
        }
        while !j.front_done && *budget > 0 && j.shell as f64 * SHELL_M < j.front {
            let lo = j.shell as f64 * SHELL_M;
            let hi = lo + SHELL_M;
            // The rays run a little ahead (3 m) of the column and the
            // cracks, so that no cell is loosened or cracked that a ray would
            // yet break (a ray's step can fall in a cell a metre or so nearer
            // or farther); in each shell the column goes first, and the
            // cracks leave what it loosened.
            let (mut work, ready) = self.blast_rays(j, hi + 3.0, *budget);
            if !ready {
                // (The budget ran out with rays still short of the shell:
                // they go on from there next tick.)
                *budget = budget.saturating_sub(work.max(1));
                continue;
            }
            if j.height > 0.0 {
                // (Past its top corners the column has nothing left to loosen.)
                if lo <= j.column_end {
                    work += self.blast_column(j, lo, hi);
                }
                // The column is loosened up to the ground now: cutters sent
                // into it since the charge went off are caught too (haulers
                // were hurt once already, when it went off).
                if !j.harmed && lo >= j.height {
                    j.harmed = true;
                    self.blast_harm(j.at, j.top, j.r, false);
                }
            }
            work += self.blast_cracks(j, lo, hi);
            j.shell += 1;
            *budget = budget.saturating_sub(work.max(1));
            // More could be thrown than was reckoned: the share is cut so
            // that what is left of THROW_MAX is spread over the rest (taking
            // that the count so far is 80% of all), and not used up before
            // the front gets to the outer part.
            if j.seen && j.cand as f64 > 0.8 * j.likely {
                j.likely = j.cand as f64 / 0.8;
                j.throw_share = (THROW_MAX.saturating_sub(j.thrown) as f64 / (j.likely - j.cand as f64).max(1.0)).clamp(0.0, 1.0);
            }
            // All the rubble made: the plume and the uranium can follow,
            // while the shock's cracks still run out.
            if matches!(j.fin, Fin::Rubble) && j.going == 0 && j.crater_done && (j.height <= 0.0 || lo > j.column_end) {
                j.fin = Fin::Plume;
            }
            let crack_end = j.ray_end.unwrap_or(f64::INFINITY).max(if j.height > 0.0 { ((j.height + 2.0).powi(2) + (j.r * j.lt + j.r).powi(2)).sqrt() } else { 0.0 });
            if j.live.is_empty() && j.crater_done && lo > j.column_end && lo > crack_end {
                j.front_done = true;
            }
        }
        self.blast_fin(j, budget);
    }

    /// What a blast does once all its rubble is made, while the budget
    /// lasts: the plume, then the unreacted uranium left through the rubble.
    fn blast_fin(&mut self, j: &mut BlastJob, budget: &mut u64) {
        loop {
            match j.fin {
                Fin::Rubble | Fin::Done => return,
                Fin::Plume => {
                    let work = self.blast_plume(j);
                    *budget = budget.saturating_sub(work);
                    j.fin = if j.unreacted > 0.0 && !j.rubble.is_empty() { Fin::Count(0) } else { Fin::Done };
                }
                // Unreacted uranium: a share is left through the rubble still
                // here (not thrown out of the crater or the plume), gone
                // through a budget's worth at a time: found, then doped.
                Fin::Count(i) => {
                    if *budget == 0 {
                        return;
                    }
                    let end = (i as u64).saturating_add(*budget).min(j.rubble.len() as u64) as usize;
                    for &c in &j.rubble[i..end] {
                        if self.loose.contains(&c) && !j.crater.contains(&c) {
                            j.here.push(c);
                        }
                    }
                    *budget -= (end - i) as u64;
                    j.fin = if end < j.rubble.len() {
                        Fin::Count(end)
                    } else if !j.here.is_empty() {
                        Fin::Dope(0, j.unreacted / j.here.len() as f64)
                    } else {
                        Fin::Done
                    };
                }
                // (What has gone since it was found gets none, and the kg
                // left never runs over what didn't react: rubble that
                // settled into a place found empty would otherwise make
                // uranium out of nothing.)
                Fin::Dope(i, each) => {
                    if *budget == 0 {
                        return;
                    }
                    let end = (i as u64).saturating_add(*budget).min(j.here.len() as u64) as usize;
                    for &c in &j.here[i..end] {
                        let kg = each.min(j.unreacted);
                        if kg > 0.0 && self.loose.contains(&c) {
                            *self.units.dope.entry(c).or_insert(0.0) += kg;
                            self.units.doped_kg += kg;
                            j.unreacted -= kg;
                        }
                    }
                    *budget -= (end - i) as u64;
                    j.fin = if end < j.here.len() { Fin::Dope(end, each) } else { Fin::Done };
                }
            }
        }
    }

    /// The rays of a blast, each stepped on to `limit` (m from the charge)
    /// or until it gives out; the cracks running on past where they gave
    /// out follow, up to the same distance. Once `budget` is spent the rest
    /// of the rays wait for the next call. Returns the work done, and
    /// whether every ray got there.
    fn blast_rays(&mut self, j: &mut BlastJob, limit: f64, budget: u64) -> (u64, bool) {
        let lim = (limit.ceil() as i64).min(j.reach);
        let r = j.r;
        let mut work = 0u64;
        let live = std::mem::take(&mut j.live);
        let mut keep = Vec::with_capacity(live.len());
        let mut ready = true;
        for (n, &i) in live.iter().enumerate() {
            if work >= budget {
                keep.extend_from_slice(&live[n..]);
                ready = false;
                break;
            }
            let k = i as usize;
            let mut ray = j.rays[k];
            let was = ray.stopped;
            while !ray.stopped && ray.step < lim {
                let step = ray.step;
                ray.step += 1;
                work += 2;
                let c = ((j.at.0 + ray.ca * step as f64).round() as i64, (j.at.1 + ray.sa * step as f64).round() as i64);
                // (Broken by another ray already: rubble to pass through,
                // even where it has since been thrown out.)
                if j.broken.contains_key(&c) {
                    ray.e -= 0.3 / r;
                    continue;
                }
                let m = self.world.material(c.0, c.1);
                if m == VACUUM {
                    ray.e -= 0.12 / r;
                    continue;
                }
                if self.loose.contains(&c) {
                    ray.e -= 0.3 / r;
                    continue;
                }
                let st = self.world.planet.mats.get(m).state;
                if st == State::Liquid || st == State::Gas {
                    ray.e -= 0.12 / r;
                    continue;
                }
                let h = blast_hardness(self, m);
                ray.e -= (0.45 + 1.7 * h) / r;
                if ray.e <= 0.0 {
                    ray.stopped = true;
                    j.stop[k] = step as f64;
                    // A crack may run on, wandering, further in harder rock.
                    if k % 4 == 0 {
                        ray.crack = Some(Crack { x: c.0 as f64, y: c.1 as f64, a: ray.a, k: 0, len: (r * (0.3 + 0.9 * h)) as i64, h });
                    }
                    break;
                }
                j.broken.insert(c, (ray.e, step as f64));
                self.units.cracked += 1;
                work += self.blast_broken(j, c, h, ray.e, step as f64);
            }
            if ray.step >= j.reach {
                ray.stopped = true;
            }
            if ray.stopped && !was {
                j.going -= 1;
            }
            if let Some(mut cr) = ray.crack {
                let mut ended = false;
                while cr.k < cr.len {
                    let aa = cr.a + (crate::noise::unit(hash(j.blast, k as i64, cr.k, 9)) - 0.5) * (1.2 - cr.h);
                    let (cc, ss) = crate::noise::cos_sin(aa);
                    let (x, y) = (cr.x + cc, cr.y + ss);
                    // (It runs no faster than the front.)
                    if (x - j.at.0).hypot(y - j.at.1) >= limit {
                        break;
                    }
                    cr = Crack { x, y, a: aa, k: cr.k + 1, ..cr };
                    work += 1;
                    let q = (x.round() as i64, y.round() as i64);
                    if self.world.material(q.0, q.1) == VACUUM {
                        ended = true;
                        break;
                    }
                    // (Rock this blast broke, and rubble, keep their look.)
                    if j.broken.contains_key(&q) || self.loose.contains(&q) {
                        continue;
                    }
                    if self.fractured.insert(q) {
                        self.units.cracked += 1;
                    }
                    self.world.set_look(q.0, q.1, Look::CRACKED);
                }
                ray.crack = if ended || cr.k >= cr.len { None } else { Some(cr) };
            }
            j.rays[k] = ray;
            if !ray.stopped || ray.crack.is_some() {
                keep.push(i);
            }
        }
        j.live = keep;
        // All the rays done: how far out the shock can crack rock.
        if j.live.is_empty() && j.ray_end.is_none() {
            j.ray_end = Some(j.stop.iter().fold(0.0f64, |a, &b| a.max(b)) + j.r * CRACK_REACH + 1.0);
        }
        (work, ready)
    }

    /// A cell a blast's ray has just broken (`e`: the ray's energy left,
    /// `d`: m from the charge): held in a big piece, melted, thrown clear, or
    /// left loose as rubble. Returns the work done (deciding and changing a
    /// cell costs about as much as five of a ray's steps, throwing one
    /// twenty, and more where that is the first change in its leaf).
    fn blast_broken(&mut self, j: &mut BlastJob, c: (i64, i64), h: f64, e: f64, d: f64) -> u64 {
        let (r, blast) = (j.r, j.blast);
        if h > 0.6 && e < 0.85 && d > r * 0.3 {
            let size = 5.0 + 9.0 * h;
            let id = blast_piece(c.0, c.1, size);
            if crate::noise::unit(hash(id, blast as i64, 0, 61)) < (h - 0.5) * 1.3 {
                let edge = blast_piece(c.0 + 1, c.1, size) != id || blast_piece(c.0 - 1, c.1, size) != id || blast_piece(c.0, c.1 + 1, size) != id || blast_piece(c.0, c.1 - 1, size) != id;
                if !edge {
                    // Held together, cracked through, hot.
                    if self.fractured.insert(c) {
                        self.units.cracked += 1;
                    }
                    self.world.set_look(c.0, c.1, Look::HELD);
                    return 10;
                }
            }
        }
        // The heart melts.
        if d < r * 0.3 * (1.15 - 0.4 * h) {
            if !j.seen {
                // (Out of sight: cooled to glass already.)
                self.loose.insert(c);
                self.world.set_look(c.0, c.1, Look::GLASS);
                return 10;
            }
            self.sandbox.heat_cell(&self.world, c, MELT_K);
            self.loose.insert(c);
            self.settling.insert(c);
            self.world.set_look(c.0, c.1, Look::MELT);
            let cool = self.ticks(MELT_COOL_S * (0.7 + 0.6 * crate::noise::unit(hash(blast, c.0, c.1, 21))));
            self.melt.push_back((self.tick + cool, c));
            return 10;
        }
        // Thrown clear, out from the blast: a random share of what could be
        // (the same every run, and at most THROW_MAX), so a blast on screen
        // never stalls the sim flying thousands of pieces.
        let can = j.seen && e > 0.35 + 0.35 * h;
        j.cand += can as u64;
        if can && j.thrown < THROW_MAX && crate::noise::unit(hash(blast, c.0, c.1, 0x7E40)) < j.throw_share {
            let fresh = if self.world.dense_leaf(c.0, c.1).is_none() { FRESH_LEAF } else { 0 };
            if let Some(m) = self.world.lift_cell(c.0, c.1) {
                self.progress.remove(&c);
                self.reserved.remove(&c);
                self.open_cell(c);
                let p = mid(c);
                let dd = ((p.0 - j.at.0).powi(2) + (p.1 - j.at.1).powi(2)).sqrt().max(1e-6);
                let (ox, oy) = ((p.0 - j.at.0) / dd, (p.1 - j.at.1) / dd);
                let sp = (18.0 + 70.0 * e * (1.0 - 0.6 * h)) * (0.7 + 0.6 * crate::noise::unit(hash(blast, c.0, c.1, 12))) * j.power;
                let vel = (ox * sp + j.up.0 * 6.0, oy * sp + j.up.1 * 6.0);
                j.thrown += 1;
                if let Some(t) = self.bodies.throw(&mut self.world, p, m, vel) {
                    self.loose.insert(t);
                    self.world.set_look(t.0, t.1, Look::RUBBLE);
                    self.settling.insert(t);
                    self.scrap_landed(t);
                }
                return 40 + fresh;
            }
        }
        // Left loose where it was: baked near the heart.
        self.loose.insert(c);
        self.settling.insert(c);
        self.world.set_look(c.0, c.1, if d < r * 0.6 { Look::BAKED } else { Look::RUBBLE });
        j.rubble.push(c);
        10
    }

    /// The shell from `lo` to `hi` m from the charge of a blast's loosened
    /// column: every solid cell from the charge up to the surface,
    /// blast-wide at the bottom and `loosen_top` radii each side at the top,
    /// not already broken, is left loose (rubble) where it is, or for hard
    /// rock now and then held in big cracked pieces (Benjamin: leaving big
    /// chunks is good). Returns the work done.
    fn blast_column(&mut self, j: &mut BlastJob, lo: f64, hi: f64) -> u64 {
        let (r, lt, height) = (j.r, j.lt, j.height);
        let mut cells = std::mem::take(&mut j.cells);
        shell_cells(&mut cells, j.at, lo, hi, None);
        let mut work = cells.len() as u64 / WORK_PASS;
        for &c in &cells {
            let p = mid(c);
            let (dx, dy) = (p.0 - j.at.0, p.1 - j.at.1);
            let h = dx * j.up.0 + dy * j.up.1;
            if h < -0.5 * r || h > height + 1.0 || (dx * j.side.0 + dy * j.side.1).abs() > r * (1.0 + (lt - 1.0) * (h / height).clamp(0.0, 1.0)) {
                continue;
            }
            work += 1;
            if j.broken.contains_key(&c) || self.loose.contains(&c) {
                continue;
            }
            let m = self.world.material(c.0, c.1);
            work += WORK_LOOK;
            if m == VACUUM || self.units.is_flora.get(m as usize).copied().unwrap_or(false) {
                continue;
            }
            let mat = self.world.planet.mats.get(m);
            if mat.state != State::Solid {
                continue;
            }
            let hd = (mat.hardness as f64 / 10.0).clamp(0.05, 0.95);
            // Hard rock: some of it holds in big pieces, cracked through.
            if hd > 0.6 {
                let size = 6.0 + 10.0 * hd;
                let id = hash(0x91ED, (c.0 as f64 / size).floor() as i64, (c.1 as f64 / size).floor() as i64, 0);
                if crate::noise::unit(hash(id, j.blast as i64, 0, 62)) < (hd - 0.5) * 1.1 {
                    if self.fractured.insert(c) {
                        self.units.cracked += 1;
                    }
                    self.world.set_look(c.0, c.1, Look::HELD);
                    work += WORK_CHANGE;
                    continue;
                }
            }
            self.loose.insert(c);
            self.settling.insert(c);
            self.world.set_look(c.0, c.1, Look::RUBBLE);
            j.rubble.push(c);
            work += WORK_CHANGE;
        }
        j.cells = cells;
        work
    }

    /// The ground at angle `a` for a crater, where a dip narrower than the
    /// two looks `side` either side of it (a shaft or crack) counts as not
    /// there. The three looks start from `near`, the highest ground found a
    /// step before, and set it to theirs, so each walks down only a few
    /// cells of air. It is the highest, not each look's own, because a look
    /// that went down a hole would carry the hole's floor on and then take a
    /// cave or bore beside the hole for the ground; a hole narrower than the
    /// looks always has one of them outside it, which keeps the start up at
    /// the surface.
    pub(super) fn crater_ground(&self, a: f64, side: f64, near: &mut f64) -> f64 {
        let at = |a: f64| self.ground_from(a, true, *near);
        let (c, l, r) = (at(a), at(a - side), at(a + side));
        *near = c.max(l).max(r);
        let side = l.min(r);
        if c < side - 3.0 { side } else { c }
    }

    /// The ground right over a blast to be thrown out, leaving a crater
    /// (Benjamin, 26 Nov: the top layer over the blast should be thrown
    /// off). A bowl `CRATER_W` of the loosened column's top half-width each
    /// side and `CRATER_D` of that deep at its middle (and the plants on it)
    /// is lifted and thrown, the top layer first and the deeper ground
    /// after: near the middle steeply up (much of it falls back in), towards
    /// the rim out at about 45 degrees, coming down round the crater.
    /// `height` is how far the ground is over the charge. This only sets it
    /// up: `crater_work` works it out, `crater_throw` throws it.
    pub(super) fn crater_plan(&self, at: (f64, f64), r: f64, yield_kt: f64, height: f64, blast: u64) -> Crater {
        let lt = self.eco.units.sapper.loosen_top.max(1.0);
        let w0 = (r * lt * CRATER_W).max(2.0);
        let dmax = (w0 * CRATER_D).min(height * 0.8).max(1.0);
        let a0 = at.1.atan2(at.0);
        let g0 = len(at) + height;
        let un = |k: i64, salt: u64| crate::noise::unit(hash(blast, k, 0, salt));
        // A ragged bowl: each side reaches its own way out, and the depth
        // wanders a little (smooth noise, a fifth of the crater across).
        let (wl, wr) = (w0 * (0.85 + 0.3 * un(0, 0xC1)), w0 * (0.85 + 0.3 * un(1, 0xC1)));
        // The ground's slope across it: the throw goes out square to the
        // ground there, not to the sky straight up (on a hillside straight
        // up throws the uphill side into the hill).
        let (mut nl, mut nr) = (g0, g0);
        let right = self.crater_ground(a0 + wr / g0, 5.0 / g0, &mut nr);
        let left = self.crater_ground(a0 - wl / g0, 5.0 / g0, &mut nl);
        let slope = ((right - left) / (wl + wr)).clamp(-1.5, 1.5);
        Crater {
            blast,
            a0,
            g0,
            wl,
            wr,
            dmax,
            period: (w0 / 5.0).max(6.0),
            // How hard it throws (as the plume does).
            pw: (0.8 + 0.35 * yield_kt.max(0.0).cbrt()).min(6.0),
            slope,
            near: g0,
            x: -wl,
            cols: Vec::new(),
            want: CRATER_MAX as f64 * 2.5,
            share: 1.0,
            i: 0,
            take: Vec::new(),
            seen: CellSet::default(),
            top: 0.0,
            span: 1.0,
            col_top: HashMap::new(),
            out: Vec::new(),
            cells: CellSet::default(),
            stage: CraterStage::Ground,
        }
    }

    /// A crater worked out on, while `budget` lasts: the ground along it,
    /// then the cells to throw, then where and how fast each flies.
    pub(super) fn crater_work(&mut self, cr: &mut Crater, budget: &mut u64) {
        let flora = |s: &Self, m: MatId| s.units.is_flora.get(m as usize).copied().unwrap_or(false);
        while *budget > 0 {
            match cr.stage {
                CraterStage::Ground => {
                    if cr.x > cr.wr {
                        cr.share = cr.share_for();
                        cr.stage = CraterStage::Scan;
                        continue;
                    }
                    // (Down a shaft, a narrow dip, the ground either side of it.)
                    let x = cr.x;
                    let gr = self.crater_ground(cr.a0 + x / cr.g0, 5.0 / cr.g0, &mut cr.near);
                    let e = if x < 0.0 { x / cr.wl } else { x / cr.wr };
                    cr.cols.push((x, gr, cr.dmax * (1.0 - e * e) * (0.85 + 0.3 * cr.wander(x))));
                    cr.x += 0.7;
                    *budget = budget.saturating_sub(40);
                }
                CraterStage::Scan => {
                    if cr.i >= cr.cols.len() {
                        // Too few, in caves: looked through deeper.
                        if cr.take.len() < CRATER_MAX && cr.share < 1.0 {
                            cr.want *= 2.0;
                            cr.share = cr.share_for();
                            cr.i = 0;
                            cr.take.clear();
                            cr.seen.clear();
                            continue;
                        }
                        if cr.take.len() > CRATER_MAX {
                            cr.take.sort_by(|a, b| a.2.total_cmp(&b.2));
                            cr.take.truncate(CRATER_MAX);
                        }
                        // Highest first, by height (not depth under the ground
                        // just over it, which differs column to column: a piece
                        // flew into the cell beside it, as high but due later,
                        // and fell back in).
                        cr.top = cr.take.iter().map(|t| len(mid(t.0))).fold(f64::MIN, f64::max);
                        let low = cr.take.iter().map(|t| len(mid(t.0))).fold(f64::MAX, f64::min);
                        cr.span = (cr.top - low).max(1.0);
                        // The highest of each column (by its place across).
                        for t in &cr.take {
                            let e = cr.col_top.entry((t.1 / 0.7).round() as i64).or_insert(f64::MIN);
                            *e = e.max(len(mid(t.0)));
                        }
                        cr.seen = CellSet::default();
                        cr.i = 0;
                        cr.stage = CraterStage::Throw;
                        *budget = budget.saturating_sub(cr.take.len() as u64);
                        continue;
                    }
                    let (x, gr, d) = cr.cols[cr.i];
                    cr.i += 1;
                    let ang = cr.a0 + x / cr.g0;
                    let (ux, uy) = (ang.cos(), ang.sin());
                    let mut work = 0;
                    // The plants on it.
                    let mut z = 0.5;
                    while z < 24.0 {
                        let c = cell_of((ux * (gr + z), uy * (gr + z)));
                        let m = self.world.material(c.0, c.1);
                        if m != VACUUM && flora(self, m) && cr.seen.insert(c) {
                            cr.take.push((c, x, 0.0, -z));
                        }
                        z += 0.7;
                        work += 1 + WORK_LOOK;
                    }
                    let deep = d.min(cr.share * d.max(1.0));
                    let mut z = 0.0;
                    while z <= deep {
                        let c = cell_of((ux * (gr - 0.5 - z), uy * (gr - 0.5 - z)));
                        // (The bottom edge frayed.)
                        let frayed = z > d - 1.5 && crate::noise::unit(hash(cr.blast, c.0, c.1, 0xC3)) < 0.5;
                        if !frayed && cr.seen.insert(c) {
                            work += 2 * WORK_LOOK;
                            if self.world.material(c.0, c.1) != VACUUM {
                                cr.take.push((c, x, z / d.max(1.0), z));
                            }
                        }
                        z += 0.7;
                        work += 1;
                    }
                    *budget = budget.saturating_sub(work);
                }
                CraterStage::Throw => {
                    if cr.i >= cr.take.len() {
                        // In the order they go (a stable sort: as they were
                        // found, among those due together).
                        cr.out.sort_by_key(|o| o.0);
                        cr.take = Vec::new();
                        cr.col_top = HashMap::new();
                        cr.stage = CraterStage::Ready;
                        continue;
                    }
                    let (c, x, q, _) = cr.take[cr.i];
                    cr.i += 1;
                    let (wl, wr, slope) = (cr.wl, cr.wr, cr.slope);
                    let k = (1.0 + slope * slope).sqrt();
                    let u = |k: u64| crate::noise::unit(hash(cr.blast, c.0, c.1, 0xC0 + k));
                    let f = (if x < 0.0 { -x / wl } else { x / wr }).min(1.0);
                    let sgn = if x.abs() < 0.5 { if u(0) < 0.5 { -1.0 } else { 1.0 } } else { x.signum() };
                    // Steep in the middle, about 45 degrees at the rim, and
                    // always steeper than the crater's wall beside it (thrown
                    // flatter, most of a big crater hit its own wall and fell
                    // back in).
                    let wall = (2.0 * cr.dmax * f / (if x < 0.0 { wl } else { wr })).atan().to_degrees() * 1.15;
                    let th = (82.0 - 40.0 * f).max(wall + 12.0).min(86.0);
                    let th = (th + (u(1) - 0.5) * 12.0).clamp(wall + 6.0, 89.0).to_radians();
                    // As fast as the blast throws it (not as far as the body's
                    // pull would bring it back: on a small body that is a
                    // crawl, and the rest of the blast flies out fast): the
                    // deep ground slower.
                    let v = 35.0 * cr.pw * (1.0 - 0.45 * q) * (0.75 + 0.5 * u(2));
                    let p = mid(c);
                    let pr = len(p).max(1.0);
                    let (ux, uy) = (p.0 / pr, p.1 / pr);
                    // Square to the ground: its normal and its line across.
                    let nrm = ((ux + uy * slope) / k, (uy - ux * slope) / k);
                    let tan = (-nrm.1, nrm.0);
                    let (vn, vt) = (v * th.sin(), sgn * v * th.cos());
                    let vel = (nrm.0 * vn + tan.0 * vt, nrm.1 * vn + tan.1 * vt);
                    // The top layer lifts off first, the deeper ground after
                    // it, layer by layer (each on its way before the one under
                    // it goes: a jumbled order threw cells into the ground
                    // still over them, and the crater filled back in).
                    let after = self.ticks(0.08 + 0.6 * ((cr.top - len(p)) / cr.span).clamp(0.0, 1.0));
                    // It flies from the crater's mouth, over the top of its
                    // column (a piece flown from down in the hole could snag on
                    // a bit left standing or a pocket something fell into, and
                    // drop back in; flown all from one height, pieces caught
                    // on each other).
                    let up_by = cr.col_top.get(&((x / 0.7).round() as i64)).copied().unwrap_or(cr.top) - len(p) + 1.5;
                    cr.out.push((after, c, (p.0 + nrm.0 * up_by, p.1 + nrm.1 * up_by), vel));
                    cr.cells.insert(c);
                    // Its leaf of the world generated now, a budget's worth
                    // at a time, not by the first of it lifted (dozens of
                    // leaves at once for a big crater).
                    let fresh = if self.world.generate_leaf(c.0, c.1) { FRESH_LEAF } else { 0 };
                    *budget = budget.saturating_sub(3 + fresh);
                }
                CraterStage::Ready => return,
            }
        }
    }

    /// A worked-out crater is thrown: its cells queued to be lifted and
    /// thrown from now on, a layer at a time (see `blast_tick`). Returns the
    /// cells.
    pub(super) fn crater_throw(&mut self, cr: Crater) -> CellSet {
        let now = self.tick;
        let later = self.eject.back().is_some_and(|e| e.0 > now + cr.out.first().map_or(0, |o| o.0));
        self.eject.extend(cr.out.into_iter().map(|(after, c, from, vel)| (now + after, c, from, vel)));
        // (Only when another crater's ground is still due later than some of
        // this one's does the queue need putting back in order.)
        if later {
            self.eject.make_contiguous().sort_by_key(|e| e.0);
        }
        cr.cells
    }

    /// The shell from `lo` to `hi` m from the charge of the rock round a
    /// blast cracked by its shock (Benjamin, 26 Nov: the shockwave should
    /// crack and frack more): past where each ray stopped breaking rock,
    /// `CRACK_REACH` blast radii further, and either side of the loosened
    /// column (more widely near the surface, where the ground lifts), the
    /// rock is cracked into blocks: small near, bigger further out, fewer the
    /// further out, more in hard brittle rock. A cracked cell is cut in half
    /// the time (`fracture_dig`); the blocks' edges show as cracks. Returns
    /// the work done.
    fn blast_cracks(&mut self, j: &mut BlastJob, lo: f64, hi: f64) -> u64 {
        if hi <= 0.0 {
            return 0;
        }
        let (at, r, lt, height, up, side) = (j.at, j.r, j.lt, j.height, j.up, j.side);
        let rays = j.stop.len().max(1);
        let reach = r * CRACK_REACH;
        // Past where any ray's cracks reach, only either side of the column.
        let past_rays = j.ray_end.is_some_and(|e| lo >= e);
        let band = |h: f64| {
            if !past_rays {
                return f64::INFINITY;
            }
            if height <= 0.0 || h < -0.5 * r || h > height + 2.0 {
                return -1.0;
            }
            let f = (h / height).clamp(0.0, 1.0);
            r * (1.0 + (lt - 1.0) * f) + r * (0.35 + 0.65 * f)
        };
        if past_rays && height <= 0.0 {
            return 0;
        }
        // Past the rays' cracks, and far enough out that nothing below the
        // charge is beside the column, the band either side of it lies
        // within asin(widest / lo) of straight up: only that wedge of the
        // shell is walked (late in a big blast, a quarter of it or less).
        let wedge = (past_rays && lo >= 1.5 * r).then(|| {
            let s = ((r * lt + r) / lo).min(1.0);
            let c = (1.0 - s * s).sqrt();
            let e1 = (up.0 * c - up.1 * s, up.0 * s + up.1 * c);
            let e2 = (up.0 * c + up.1 * s, -up.0 * s + up.1 * c);
            [(e1.1, -e1.0), (-e2.1, e2.0)]
        });
        let mut cells = std::mem::take(&mut j.cells);
        shell_cells(&mut cells, at, lo, hi, wedge);
        let mut work = cells.len() as u64 / WORK_PASS;
        for &c in &cells {
            let p = mid(c);
            let (dx, dy) = (p.0 - at.0, p.1 - at.1);
            // (Not over the ground over it.)
            let h = dx * up.0 + dy * up.1;
            if h > height + 1.0 || (dx * side.0 + dy * side.1).abs() > band(h) {
                continue;
            }
            work += 1;
            // Round the charge, past where the rays stopped.
            let d = (dx * dx + dy * dy).sqrt();
            let k = ((dy.atan2(dx) / std::f64::consts::TAU * rays as f64).round() as i64).rem_euclid(rays as i64) as usize;
            let mut t = ((d - j.stop[k]) / reach).max(0.0);
            // Either side of the column, more widely near the surface.
            if height > 0.0 {
                let along = dx * up.0 + dy * up.1;
                if along >= -0.5 * r && along <= height + 2.0 {
                    let f = (along / height).clamp(0.0, 1.0);
                    let half = r * (1.0 + (lt - 1.0) * f);
                    let out = (dx * side.0 + dy * side.1).abs() - half;
                    t = t.min((out / (r * (0.35 + 0.65 * f))).max(0.0));
                }
            }
            if t >= 1.0 {
                continue;
            }
            let blocks = &mut j.blocks[((t * 4.0) as usize).min(3)];
            let (id, roll) = blocks.of(c.0, c.1);
            let near = (1.0 - t).powf(0.8);
            // Not cracked even if it were the hardest rock: no need to
            // look at what it is (asking the world is the dear part).
            if roll >= (near * (0.45 + 0.75 * 0.95)).min(1.0) || j.broken.contains_key(&c) || self.loose.contains(&c) {
                continue;
            }
            let m = self.world.material(c.0, c.1);
            work += WORK_LOOK;
            if m == VACUUM || self.units.is_flora.get(m as usize).copied().unwrap_or(false) {
                continue;
            }
            let mat = self.world.planet.mats.get(m);
            if mat.state != State::Solid {
                continue;
            }
            let hd = (mat.hardness as f64 / 10.0).clamp(0.05, 0.95);
            if roll >= (near * (0.45 + 0.75 * hd)).min(1.0) {
                continue;
            }
            // (Newly cracked only: what an earlier blast held or cracked
            // keeps its look.)
            if !self.fractured.insert(c) {
                continue;
            }
            self.units.cracked += 1;
            work += WORK_CHANGE;
            if (blocks.of(c.0 + 1, c.1).0 != id || blocks.of(c.0, c.1 + 1).0 != id) && crate::noise::unit(hash(j.blast, c.0, c.1, 72)) < 0.9 - 0.4 * t {
                self.world.set_look(c.0, c.1, Look::CRACKED);
            }
        }
        j.cells = cells;
        work
    }

    /// A blast's plume, once its rubble is all made: most blasts find a way
    /// up. (Not where the ground over it was thrown out as a crater: a jet
    /// out of one hole beside the crater's even fan looked lopsided;
    /// Benjamin, 26 Nov.) Returns the work done.
    fn blast_plume(&mut self, j: &BlastJob) -> u64 {
        let blast = j.blast;
        if !j.seen || j.cratered || crate::noise::unit(hash(blast, 0, 0, 77)) >= 0.9 || j.rubble.is_empty() {
            return 0;
        }
        let Some(mouth) = self.way_up(j.c0, j.r) else { return 0 };
        // A random share of the rubble still lying where it broke, the same
        // every run. (By now some of it may have settled away or been taken,
        // and melt or other rubble come into its place: only loose rubble
        // is blown out, never liquid that flowed in.)
        let mut pick: Vec<(u64, (i64, i64))> = j.rubble.iter().filter(|c| self.loose.contains(c) && self.world.look(c.0, c.1) != Look::MELT).map(|&c| (hash(blast, c.0, c.1, 13), c)).collect();
        let work = j.rubble.len() as u64 + pick.len() as u64 * WORK_LOOK;
        let share = 0.35 + 0.25 * crate::noise::unit(hash(blast, 0, 0, 7));
        let take = ((pick.len() as f64 * share).round() as usize).min(pick.len()).min(PLUME_MAX);
        if take == 0 {
            return work;
        }
        // (The lowest hashes found without sorting them all.)
        if take < pick.len() {
            pick.select_nth_unstable(take);
            pick.truncate(take);
        }
        pick.sort_unstable();
        let pw = (0.8 + j.yield_kt.max(0.0).cbrt() * 0.35).min(4.0);
        // How long it pours out (the proving ground's plume).
        let dur = 0.6 + 0.35 * (j.yield_kt * 10.0 + 1.0).log10();
        if self.plumes.len() < 100 {
            let m = mid(mouth);
            self.plumes.push((m.0, m.1, pw as f32));
        }
        let mu = unit_of(mid(mouth));
        let side = (-mu.1, mu.0);
        for (k, &(h, c)) in pick.iter().enumerate() {
            let Some(m) = self.world.lift_cell(c.0, c.1) else { continue };
            self.loose.remove(&c);
            self.undope(c, false);
            self.progress.remove(&c);
            self.reserved.remove(&c);
            self.open_cell(c);
            let vx = (crate::noise::unit(hash(h, k as i64, 1, 0)) - 0.5) * 16.0 * pw;
            let vy = (50.0 + 60.0 * crate::noise::unit(hash(h, k as i64, 2, 0))) * pw;
            let from = mid(mouth);
            let from = (from.0 + side.0 * (crate::noise::unit(hash(h, k as i64, 3, 0)) - 0.5) * 2.0, from.1 + side.1 * (crate::noise::unit(hash(h, k as i64, 3, 0)) - 0.5) * 2.0);
            // Out over the plume's time, a steady stream.
            let at = self.tick + self.ticks(dur * k as f64 / take.max(1) as f64);
            self.plume_out.push_back((at, from, m, (side.0 * vx + mu.0 * vy, side.1 * vx + mu.1 * vy)));
        }
        self.plume_out.make_contiguous().sort_by_key(|p| p.0);
        work
    }

    /// What a blast does to the swarm (Benjamin, 25 Nov: blasts kill
    /// cutters; haulers are damaged and can die; nothing is pulled back).
    /// Cutters in the loosened column from `at` up to `top` (and
    /// kill_margin_m round it) are wrecked; haulers near `top` lose life,
    /// the nearer the more (only when `haulers`: the second time a blast
    /// catches cutters, once its front reaches the ground, they aren't
    /// hurt again).
    pub(super) fn blast_harm(&mut self, at: (f64, f64), top: (f64, f64), r: f64, haulers: bool) {
        let sp = &self.eco.units.sapper;
        let (lt, margin, harm_r) = (sp.loosen_top.max(1.0), sp.kill_margin_m, sp.hauler_harm_r * r);
        let up = unit_of(at);
        let side = (-up.1, up.0);
        let height = ((top.0 - at.0) * up.0 + (top.1 - at.1) * up.1).max(1.0);
        let now = self.time();
        let speed = self.leg_speed();
        let mut dead: Vec<(u32, (f64, f64))> = Vec::new();
        for (id, d) in self.drones.iter().enumerate() {
            if !d.alive {
                continue;
            }
            let leg = self.leg_of(d, speed);
            if leg.state == Leg::RESTING {
                continue;
            }
            // Where it is, where its trip goes or came from, and where it
            // last cut: a cutter working in the column is caught (off
            // screen, cutters do their trips at once and wait at their
            // hauler, so where they work tells, not where they are).
            let inside = |p: (f64, f64)| {
                let (dx, dy) = (p.0 - at.0, p.1 - at.1);
                let along = dx * up.0 + dy * up.1;
                if along < -r - margin || along > height + margin {
                    return false;
                }
                let half = r * (1.0 + (lt - 1.0) * (along / height).clamp(0.0, 1.0)) + margin;
                (dx * side.0 + dy * side.1).abs() <= half
            };
            let p = leg.at(now);
            if inside(p) || (leg.state != 0 && (inside(leg.from) || inside(leg.to))) || d.last.is_some_and(|c| inside(mid(c))) {
                dead.push((id as u32, p));
            }
        }
        let killed = dead.len() as u64;
        for (id, p) in dead {
            self.wreck_drone(id, p, (0.0, 0.0));
        }
        self.units.blast_kills += killed;
        let life = self.eco.hauler.life_s;
        let mut hurt = 0u64;
        for k in 0..self.haulers.len() {
            if !haulers || !self.haulers[k].alive {
                continue;
            }
            let p = self.hauler_pos(k);
            let d = dist2(p, top).sqrt().min(dist2(p, at).sqrt());
            let harm = (1.0 - d / harm_r.max(1.0)).clamp(0.0, 1.0);
            if harm > 0.0 {
                // Within the first third of the reach: wrecked.
                self.haulers[k].life_left -= life * harm * 1.5;
                hurt += 1;
            }
        }
        self.units.blast_hurt_haulers += hurt;
        if killed > 0 || hurt > 0 {
            self.director_log_push(format!("a blast kills {killed} cutters and damages {hurt} haulers"));
        }
    }

    /// An open way up from cell `from` to open sky over the ground (through
    /// open space and the cells in `loose`), within reach of a blast of
    /// radius `r`: the cell where it comes out, if there is one.
    pub(super) fn way_up(&self, from: (i64, i64), r: f64) -> Option<(i64, i64)> {
        let limit = r * 3.0 + 90.0;
        let mut seen = CellSet::default();
        let mut queue = std::collections::VecDeque::new();
        seen.insert(from);
        queue.push_back(from);
        // The ground's height by direction, worked out as needed.
        let mut tops: HashMap<i64, f64> = HashMap::new();
        let r0 = len(mid(from)).max(1.0);
        while let Some(c) = queue.pop_front() {
            if seen.len() > 40_000 {
                break;
            }
            let p = mid(c);
            let a = p.1.atan2(p.0);
            let key = (a * r0).round() as i64;
            let top = *tops.entry(key).or_insert_with(|| self.ground_at(a, true));
            if len(p) >= top {
                return Some(c);
            }
            // Up, across, down, and up a diagonal (never down one), as the
            // proving ground looks, in the frame of the ground here.
            let pu = unit_of(mid(c));
            let st = |u: f64| if u > 0.38 { 1 } else if u < -0.38 { -1 } else { 0 };
            let upv = (st(pu.0), st(pu.1));
            let side = (-upv.1, upv.0);
            let dirs = [upv, side, (-side.0, -side.1), (-upv.0, -upv.1), (upv.0 + side.0, upv.1 + side.1), (upv.0 - side.0, upv.1 - side.1)];
            for d in dirs {
                let n = (c.0 + d.0, c.1 + d.1);
                if seen.contains(&n) {
                    continue;
                }
                let dx = (n.0 - from.0) as f64;
                let dy = (n.1 - from.1) as f64;
                if dx * dx + dy * dy > limit * limit {
                    continue;
                }
                let m = self.world.material(n.0, n.1);
                if m != VACUUM && !self.loose.contains(&n) && !self.units.is_flora.get(m as usize).copied().unwrap_or(false) {
                    continue;
                }
                seen.insert(n);
                queue.push_back(n);
            }
        }
        None
    }

    /// Kilograms of uranium in one nuclear charge (its strict "fissile"
    /// components).
    pub(super) fn fissile_per_charge(&self) -> f64 {
        let r = &self.eco.recipe;
        let Some(p) = r.index("nuclear-charge") else { return 0.0 };
        r.part[p].make.iter().filter(|c| !c.of.is_empty() && c.of.iter().all(|ch| ch.el == "U")).map(|c| c.kg).sum()
    }

    /// Whether a sapper can have a nuclear charge: one in stock, the
    /// stockpile can make one, or the fuser can make the uranium for one.
    pub(super) fn nuclear_ok(&self) -> bool {
        let r = &self.eco.recipe;
        let Some(p) = r.index("nuclear-charge") else { return true };
        if self.parts.n[p] > 0 {
            return true;
        }
        let els = &self.world.planet.mats.elements;
        let comps: Vec<(&crate::recipes::Component, f64)> = r.part[p].make.iter().map(|c| (c, c.kg)).collect();
        if self.stock.plan_refined(els, &comps).is_some() {
            return true;
        }
        let f = &self.eco.fuser;
        let Some(u) = els.index("U") else { return false };
        if f.fuse.is_empty() {
            return false;
        }
        let need = self.fissile_per_charge();
        let can = self.stock.refined[u] + f.makeable(els, &self.stock.refined, u, crate::fuser::DEPTH).0;
        // (Three steps of fusing at most, for every kilogram.)
        let energy = self.energy - 5.0 * self.upkeep() - f.spare_energy;
        can >= need && energy >= 3.0 * need * f.energy_per_kg
    }

    /// The conventional charge a sapper would be built with now: none while
    /// a nuclear one can be had, else the first the stockpile can cast (with
    /// `charge_keep_back` as much again left over). None too when none can be.
    pub(super) fn sapper_charge(&self) -> Option<usize> {
        if self.nuclear_ok() {
            return None;
        }
        let sp = &self.eco.units.sapper;
        let els = &self.world.planet.mats.elements;
        sp.charge.iter().position(|ch| {
            let kg = ch.kg_for(sp.yield_kt) * (1.0 + sp.charge_keep_back.max(0.0));
            let comps: Vec<(&crate::recipes::Component, f64)> = ch.make.iter().map(|c| (c, c.kg * kg)).collect();
            self.stock.plan_refined(els, &comps).is_some()
        })
    }

    /// A sapper's bay starts it: cast its conventional charge straight from
    /// the stockpile, or note its nuclear one. False (nothing taken) if the
    /// charge can't be cast after all.
    pub(super) fn cast_charge(&mut self) -> bool {
        match self.sapper_charge() {
            None => {
                let f = self.fissile_per_charge();
                self.units.charges.push_back(f);
                true
            }
            Some(i) => {
                let sp = &self.eco.units.sapper;
                let ch = &sp.charge[i];
                let kg = ch.kg_for(sp.yield_kt);
                let comps: Vec<(&crate::recipes::Component, f64)> = ch.make.iter().map(|c| (c, c.kg * kg)).collect();
                let Some(take) = self.stock.plan_refined(&self.world.planet.mats.elements, &comps) else { return false };
                let name = ch.name.clone();
                let spent = self.stock.take(&take);
                self.stock_spent += spent;
                self.units.conventional += 1;
                self.units.explosive_kg += spent;
                self.units.charges.push_back(0.0);
                self.director_log_push(format!("no uranium: a sapper gets a {:.0} t {name} charge", spent / 1000.0));
                true
            }
        }
    }

    /// A cell is taken whole: unreacted uranium left in it goes with it
    /// (`carried`: into the cutter's load, else lost).
    pub(super) fn undope(&mut self, c: (i64, i64), carried: bool) {
        if self.units.dope.is_empty() {
            return;
        }
        let Some(u) = self.units.dope.remove(&c) else { return };
        if !carried {
            return;
        }
        if let Some(i) = self.world.planet.mats.elements.index("U") {
            self.transit.kg[i] += u;
            self.transit.total += u;
            self.units.undoped_kg += u;
        }
    }

    /// Sappers' charges so far: conventional charges cast, kg of explosive
    /// in them, kg of unreacted uranium left in rubble, kg of it brought back.
    pub fn charge_totals(&self) -> (u64, f64, f64, f64) {
        let u = &self.units;
        (u.conventional, u.explosive_kg, u.doped_kg, u.undoped_kg)
    }

    /// Charges and the fuser, for the F1 panel.
    pub fn charge_lines(&self) -> Vec<String> {
        let u = &self.units;
        let mut v = Vec::new();
        if self.prod.bays_of(Unit::Sapper) > 0 {
            let now = match self.sapper_charge() {
                Some(i) => {
                    let sp = &self.eco.units.sapper;
                    let ch = &sp.charge[i];
                    format!("conventional ({}, {:.0} t)", ch.name, ch.kg_for(sp.yield_kt) / 1000.0)
                }
                None if self.nuclear_ok() => "nuclear".to_string(),
                None => "none yet (no uranium, not enough for a conventional one)".to_string(),
            };
            v.push(format!(
                "  sapper charges now: {now} · conventional cast {} ({:.0} t of explosive) · uranium left in rubble {:.1} kg, brought back {:.1} kg",
                u.conventional,
                u.explosive_kg / 1000.0,
                u.doped_kg,
                u.undoped_kg
            ));
        }
        v.extend(self.fuser.lines(&self.eco.fuser));
        v
    }

    /// Saves (version 13): the fuser, sappers' charges, uranium in rubble.
    pub(super) fn write_charges(&self, o: &mut crate::save::W) {
        o.f64s(&self.fuser.made);
        o.f64(self.fuser.energy);
        let u = &self.units;
        o.f64s(&u.sappers.iter().map(|s| s.fissile).collect::<Vec<_>>());
        o.f64s(&u.charges.iter().copied().collect::<Vec<_>>());
        o.u64(u.conventional);
        o.f64(u.explosive_kg);
        o.f64(u.doped_kg);
        o.f64(u.undoped_kg);
        let mut dope: Vec<((i64, i64), f64)> = u.dope.iter().map(|(c, k)| (*c, *k)).collect();
        dope.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        o.u32(dope.len() as u32);
        for (c, k) in dope {
            put_cell(o, c);
            o.f64(k);
        }
    }

    pub(super) fn read_charges(&mut self, r: &mut crate::save::R) -> Result<(), String> {
        self.fuser.made = r.f64s()?;
        self.fuser.energy = r.f64()?;
        let fissile = r.f64s()?;
        for (s, f) in self.units.sappers.iter_mut().zip(fissile) {
            s.fissile = f;
        }
        self.units.charges = r.f64s()?.into_iter().collect();
        self.units.conventional = r.u64()?;
        self.units.explosive_kg = r.f64()?;
        self.units.doped_kg = r.f64()?;
        self.units.undoped_kg = r.f64()?;
        for _ in 0..r.u32()? {
            let c = get_cell(r)?;
            let k = r.f64()?;
            self.units.dope.insert(c, k);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn sim() -> Sim {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let base = crate::load_body(&data, "barren-world").unwrap();
        let mut spec = base.spec.clone();
        spec.radius_m = 3000.0;
        let p = crate::planet::Planet::new(spec, base.mats.clone()).unwrap();
        let eco = crate::economy::Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
        Sim::new(crate::world::World::new(p), eco)
    }

    /// The crater's ground, walked across from one look to the next, is
    /// the ground found walking down from the sky every time, also beside
    /// a sapper's shaft with a bore along its bottom, and a shaft that
    /// opens into a chamber (where a look that kept the hole's floor would
    /// take the bore's or chamber's floor for the ground).
    #[test]
    fn a_crater_finds_the_ground_beside_shafts_bores_and_chambers() {
        for case in 0..2 {
            let mut s = sim();
            let top = |s: &Sim, x: i64| {
                let mut y = -(s.world.planet.max_radius as i64 + 5);
                while s.world.material(x, y) == VACUUM {
                    y += 1;
                }
                y
            };
            let dig = |s: &mut Sim, xs: std::ops::RangeInclusive<i64>, from: Option<i64>, to: i64| {
                for x in xs {
                    for y in from.unwrap_or_else(|| top(s, x))..=to {
                        s.world.lift_cell(x, y);
                    }
                }
            };
            let y0 = top(&s, 0);
            if case == 0 {
                // A 7 m shaft 20 m off, 80 m down, and a bore along to the charge.
                dig(&mut s, -23..=-17, None, y0 + 83);
                dig(&mut s, -23..=-1, Some(y0 + 77), y0 + 83);
            } else {
                // A 7 m shaft 30 m down into a 25 m by 8 m chamber beyond it.
                dig(&mut s, -8..=-2, None, y0 + 30);
                dig(&mut s, -8..=17, Some(y0 + 30), y0 + 38);
            }
            let g0 = -(y0 as f64) + 1.0;
            let a0 = (-1.0f64).atan2(0.0);
            let mut near = g0;
            let mut x = -60.0;
            while x <= 60.0 {
                let (a, side) = (a0 + x / g0, 5.0 / g0);
                let got = s.crater_ground(a, side, &mut near);
                // (With no hint each look walks down from the sky.)
                let mut sky = f64::INFINITY;
                let want = s.crater_ground(a, side, &mut sky);
                assert_eq!(got, want, "case {case}: the ground {x} m along");
                x += 0.7;
            }
        }
    }
}
