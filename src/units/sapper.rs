//! Sappers: carry a charge (10 kt) to where the going is hardest, hover
//! along to a way down (cracks and caves) or bore one, until deep enough,
//! and go off. The blast (a shock front running out from the charge over
//! the ticks: rays, cracks, held chunks (a few thrown out whole), a molten
//! heart, rubble, thrown rock, the loosened column, the crater and a plume),
//! rubble settling, chain blasts, and the charges themselves (nuclear, fused
//! uranium, or conventional; uranium left in rubble).
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
/// At most this many of a blast's held pieces (hard rock held together in
/// big chunks) are thrown out whole as tumbling rock bodies, on screen only:
/// those in its crater's ground first, as the crater goes, then those left
/// lying open to the crater, the cavity or the sky, highest first. The
/// crater's others fly with its ground, as before, and the rest stay where
/// they are, cracked. (Each is a physics body; the game keeps at most 400
/// small bodies in the air at once.)
const CHUNKS_MAX: usize = 24;
/// A thrown piece flies at this share of the speed the blast throws its
/// crater's ground at the middle (35 m/s x its power): heavy, so a good deal
/// slower than even the crater's rim (EJECTA_RIM_SHARE of it), and slower
/// still the bigger it is.
const CHUNK_SHARE: f64 = 0.2;
/// A piece left in the ground is thrown only where it can move this far (m)
/// its way without running into anything (a cell thrown out beside it
/// leaves a hole too small to throw it into)...
const CHUNK_CLEAR: i64 = 4;
/// ...and as far as it flies in this long (s), and at least its own size.
const CHUNK_LOOK_S: f64 = 0.3;
/// The crater thrown out over a blast: this share of the loosened column's
/// top half-width each side...
pub(super) const CRATER_W: f64 = 0.85;
/// ...and this share of that deep at its middle.
pub(super) const CRATER_D: f64 = 0.4;
/// Most cells thrown out of a crater (the top layers first; the rest stays
/// as rubble). Raised from 3000 on 26 Nov (Benjamin: the throw looked
/// uneven) to what the screen can fly at once (20,000), and on 27 Nov, with
/// the ground thrown in clumps, well past the whole bowl of a 10,000 kt
/// blast (about 50,000 cells; at 20,000, craters past about 1000 kt came
/// out shallower than their size).
pub(super) const CRATER_MAX: usize = 250_000;
/// A crater's ground flies as single cells while it is at most this many;
/// past that, in clumps of neighbouring cells (2 x 2 up to 4 x 4, see
/// `bodies::CLUMP_MAX`), so a big crater throws all its ground with at most
/// about CRATER_MAX / 16 pieces in the air (flying 20,000 single cells was
/// most of the dearest tick of a big blast).
pub(super) const CRATER_PIECES: usize = 4_000;
/// The side of the biggest clump (cells).
const CLUMP_SIDE: usize = 4;
/// At most about this many of a crater's cells are lifted out a game second
/// (each one lifted opens the ground under it, at a few microseconds): a big
/// crater is thrown out over a few seconds, not all in the 0.6 s a small
/// one takes.
const CRATER_LIFT_PER_S: f64 = 40_000.0;
/// How fast a crater throws its ground: fastest in the middle (the blast's
/// own speed), falling off towards the rim to EJECTA_RIM_SHARE of that
/// (as Housen and Holsapple's scaling has it, if more gently), and never
/// slower than gets it past the rim over the ground as it lies.
/// (Tried 27 Nov: the rim as slow as the body's pull brings it down just
/// past the rim, the physics of a real asteroid. On these small bodies the
/// pull is so weak that the ground then hung over the crater in two thin
/// streams for minutes, like smoke; the fast fan Benjamin had approved was
/// put back, gentler at the rim, until he picks.)
const EJECTA_RIM_SHARE: f64 = 0.45;
/// A sapper hovers this high over the ground while it moves along it
/// looking for its way down (less under a low roof) (Benjamin, 26 Nov).
pub(super) const HOVER_M: f64 = 5.0;
/// Half the width of the shaft a sapper bores (m either side of its middle):
/// as wide as its hull (it was 3 m wide; Benjamin, 26 Nov: wider).
pub(super) const SHAFT_HALF: i64 = 3;
/// How far past the broken rock the shock cracks it all through into
/// blocks, in blast radii.
pub(super) const CRACK_REACH: f64 = 1.2;
/// How much further again (blast radii) it cracks the rock thinly: only
/// some of the bigger blocks out there crack, fewer the further out, and
/// only some of their edges, the cracks themselves, are cracked and drawn
/// (the rock between them holds). Underground tests crack rock out to 80 to
/// 120 m/kt^1/3 (docs/explosions-research.md); with this the cracks reach
/// 70 to 90 m/kt^1/3 at the farthest, where the full blocks alone reached 45
/// to 50.
pub(super) const CRACK_FAR: f64 = 3.2;
/// The far zone starts this far into the near one (as a share of
/// CRACK_REACH), and comes in gradually up to the near one's end, so that
/// the two together thin out steadily. (Starting where the near one ended,
/// it was thicker than the near one's faded end: a thin ring with a denser
/// ring of cracks outside it.)
const FAR_FROM: f64 = 0.4;
/// How wide the far zone is in all (blast radii), and how far into it (as
/// a share of that) it has come in fully.
const FAR_W: f64 = CRACK_REACH * (1.0 - FAR_FROM) + CRACK_FAR;
const FAR_IN: f64 = CRACK_REACH * (1.0 - FAR_FROM) / FAR_W;
/// The share of the far zone's blocks that crack once it has come in, in
/// the hardest rock at 10 kt (more for a bigger blast, as its blocks are
/// bigger, so that its cracks lie about as thickly; fewer in softer rock,
/// and none at its outer edge)...
const FAR_SHARE: f64 = 0.2;
/// ...and the share of a cracked block's edges that are cracks there (less
/// at its outer edge).
const FAR_EDGE: f64 = 0.7;
/// The far zone's blocks (m across) in its inner, middle and outer thirds,
/// at 10 kt; bigger for bigger blasts, as the square root of the radius, so
/// that a big blast's far cracks grow with the length of their edges rather
/// than with the area they cover.
const FAR_SIZES: [f64; 3] = [10.0, 14.0, 20.0];

/// After a buried blast the roof of its cavity falls in, leaving a chimney
/// of rubble over it (docs/explosions-research.md). Where a blast has no
/// loosened column (a tunneler's end), the rock over it up to this many melt
/// radii (the molten heart's radius) over the charge is left loose. Real
/// chimneys rise 4 to 6 cavity radii; ours is the top of that, as our melt
/// radius (about 4 m/kt^1/3) is at the small end of real cavities (4 to 12).
const CHIMNEY_TALL: f64 = 6.0;
/// In soft rock the rays break the ground out to nearly 2 blast radii, past
/// 6 melt radii, so the chimney always rises at least this many melt radii
/// over where the rays going straight up stopped (real chimneys reach
/// about as far as the crushed rock round the cavity, or a little past it).
const CHIMNEY_OVER: f64 = 2.0;
/// The rays this close to straight up (radians) tell where the broken
/// ground ends over the charge.
const CHIMNEY_RAYS: f64 = 0.26;
/// The chimney's half-width at the charge and at its top, in melt radii: a
/// little wider than the cavity at the bottom, narrowing upwards, and
/// rounded off over its last fifth.
const CHIMNEY_BASE: f64 = 2.5;
const CHIMNEY_TOP: f64 = 1.5;
/// The roof starts to fall in this long after the blast (game s), so that
/// it reads as a collapse after the shock and not as part of it...
const CHIMNEY_DELAY_S: f64 = 1.0;
/// ...and the collapse climbs from the top of the broken ground to the
/// chimney's top over this long (s), as a real one works its way up.
const CHIMNEY_CLIMB_S: f64 = 1.0;

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
    pub(super) fn of(cells: &CellBits) -> Settling {
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
/// Looking up the ground's height in one direction, walking down from over
/// the highest it could be, and from just over the ground beside it.
const WORK_GROUND: u64 = 5_000;
const WORK_GROUND_NEAR: u64 = 1_500;
/// Lifting a cell of rubble out into a blast's plume.
const WORK_PLUME: u64 = 500;
/// Going on from a node of the shock's field to its 16 neighbours (see
/// `ShockField`).
const WORK_FIELD: u64 = 3;
/// Looking at one place in a node's square: the cell alone, with none of
/// what a ray does there (measured, 27 Nov: 130 to 270 ns each, against
/// about 135 ns a unit of work over a blast's dearest tick).
const WORK_FIELD_LOOK: u64 = 3;

/// The shock's field (see `ShockField`) is worked out on a grid of nodes
/// this many to a blast radius: 3.4 m apart at 10 kt and 33 m at 10,000 kt,
/// about 130 across at any yield.
const FIELD_PER_R: f64 = 9.0;
/// How many places across (and down) each node's square are looked at, to
/// find how much of it is open space, where its side spans more than one
/// cell more than this (from about 20 kt up); a smaller square has every
/// cell it overlaps looked at. (One place alone made a 1 m hole a node
/// lands on a wall a node wide, and missed one a little way off: the
/// shadows came and went with where the grid fell.)
const FIELD_LOOKS: usize = 4;
/// How thick open space must lie across the straight line from the charge
/// (in nodes: a ninth of a blast radius) to cast a shadow at all, and to
/// cast a whole one. Thinner than that, a fissure or a cutter's pit, the
/// shock carries on past it (it isn't open all the way across, or its
/// sides are pressed shut).
const SHADOW_FROM: f64 = 1.0;
const SHADOW_FULL: f64 = 1.4;
/// What the shock loses (energy; a ray starts with 1) where open space lies
/// across the straight line from the charge: it has to turn a corner to get
/// there, and a shock that does loses most of its strength (most of it
/// reflects off the cave's wall, and what bends round spreads thin).
const SHADOW_TURN: f64 = 1.5;
/// How many times over the extra cost of the way round open space counts
/// against the shock as well: behind a big cave, the further round the
/// shock has to go, the less of it gets there.
const SHADOW_COST: f64 = 10.0;
/// The most a shadow takes off the shock's energy (a ray starts with 1).
const SHADOW_MAX: f64 = 3.0;
/// A ray that gives out in a shadow deeper than this (energy) leaves no
/// crack running on into it.
const SHADOW_CRACK: f64 = 0.2;
/// Along a soft layer the rock cracked all through runs on further past the
/// broken rock than CRACK_REACH, by what the shock spends getting there, but
/// at most this many times as far.
const CRACK_SOFT: f64 = 1.25;
/// What the cheapest way to a place must save over the straight line (a
/// share of the straight line's cost) before a ray there gains by it: in
/// rock of mixed layers some way is always a little cheaper, and without
/// this every blast broke 5 to 8% more rock than in even rock.
const GAIN_SLACK: f64 = 0.05;

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

/// The cells whose middles lie from `h0` to `h1` m up the line from `at`
/// along `up` (a unit vector), and less than `w` m either side of it, into
/// `out`: each cell over a blast is in exactly one such band, so the
/// chimney deals with it once, bottom up. Only the part of each row inside
/// the band is walked.
fn band_cells(out: &mut Vec<(i64, i64)>, at: (f64, f64), up: (f64, f64), h0: f64, h1: f64, w: f64) {
    out.clear();
    if h1 <= h0 || w <= 0.0 {
        return;
    }
    let side = (-up.1, up.0);
    let corners = [(h0, -w), (h0, w), (h1, -w), (h1, w)].map(|(a, b)| (at.0 + up.0 * a + side.0 * b, at.1 + up.1 * a + side.1 * b));
    let span = |v: [f64; 4]| ((v.iter().fold(f64::INFINITY, |a, &b| a.min(b)) - 0.5).floor() as i64, (v.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)) - 0.5).ceil() as i64);
    let ((x0, x1), (y0, y1)) = (span(corners.map(|p| p.0)), span(corners.map(|p| p.1)));
    for y in y0..=y1 {
        let dy = y as f64 + 0.5 - at.1;
        // Each of the band's two pairs of sides bounds the row from both
        // ends, where it isn't parallel to the row (a cell either way to
        // spare: each cell is then tested exactly).
        let (mut a, mut b) = (x0, x1);
        for (n, p, q) in [(up, h0, h1), (side, -w, w)] {
            if n.0.abs() > 1e-9 {
                let (s, t) = ((p - n.1 * dy) / n.0, (q - n.1 * dy) / n.0);
                a = a.max((s.min(t) + at.0 - 0.5).floor() as i64 - 1);
                b = b.min((s.max(t) + at.0 - 0.5).ceil() as i64 + 1);
            }
        }
        for x in a..=b {
            let dx = x as f64 + 0.5 - at.0;
            let along = dx * up.0 + dy * up.1;
            if along >= h0 && along < h1 && (dx * side.0 + dy * side.1).abs() < w {
                out.push((x, y));
            }
        }
    }
}

/// How far into the far cracked zone a point lies (see `BlastJob::far_u`),
/// from how far into the near zone it lies by the rays alone.
fn far_u_of(tr: f64) -> f64 {
    (tr - FAR_FROM) * CRACK_REACH / FAR_W
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

/// The big piece a blast's loosened column holds cell `c` in, if it holds
/// it (`hd`: its rock's hardness, 0.05 to 0.95; `blast`: the blast's roll of
/// the dice): only hard rock, in squares bigger in harder rock, now and
/// then. It is a key for the piece (from the column, its square's size, and
/// which square). The same whenever it is asked: the crater, planned before
/// the column gets to its ground, asks it too (see `crater_work`).
fn column_piece(c: (i64, i64), hd: f64, blast: u64) -> Option<(bool, u64, u64)> {
    if hd <= 0.6 {
        return None;
    }
    let size = 6.0 + 10.0 * hd;
    let id = hash(0x91ED, (c.0 as f64 / size).floor() as i64, (c.1 as f64 / size).floor() as i64, 0);
    (crate::noise::unit(hash(id, blast as i64, 0, 62)) < (hd - 0.5) * 1.1).then_some((true, size.to_bits(), id))
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

/// A block of the far cracked zone (see CRACK_FAR) waiting for the front:
/// how far out the front must be before it is decided (m from the charge:
/// past all of it and all its neighbours' sites, so that the rays round it
/// have all given out or gone by and every neighbour is judged the same
/// whenever it is asked about), how far out its site is, which of the far
/// patterns it is in, and its square in that pattern.
#[derive(Clone, Copy)]
struct FarBlock {
    due: f64,
    d: f64,
    b: u8,
    i: i64,
    k: i64,
}

/// A block's outline: its corners, each with the neighbouring block whose
/// edge runs from it to the next corner (none for the sides of the square
/// it was cut from).
type Outline = Vec<((f64, f64), Option<(i64, i64)>)>;

/// A convex outline cut down to the side of the line through `m` that the
/// normal `n` points away from; the new side is block `by`'s edge.
fn clip_outline(poly: &mut Outline, m: (f64, f64), n: (f64, f64), by: (i64, i64)) {
    let side = |p: (f64, f64)| (p.0 - m.0) * n.0 + (p.1 - m.1) * n.1;
    let mut out = Vec::with_capacity(poly.len() + 1);
    for e in 0..poly.len() {
        let (a, ta) = poly[e];
        let b = poly[(e + 1) % poly.len()].0;
        let (sa, sb) = (side(a), side(b));
        let cut = || {
            let f = sa / (sa - sb);
            (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f)
        };
        if sa <= 0.0 {
            out.push((a, ta));
            if sb > 0.0 {
                out.push((cut(), Some(by)));
            }
        } else if sb <= 0.0 {
            out.push((cut(), ta));
        }
    }
    *poly = out;
}

/// Where a crater being worked out has got to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum CraterStage {
    /// Looking up the ground along it.
    Ground,
    /// Finding the cells to throw.
    Scan,
    /// Finding the highest of them, and of each column.
    Tops,
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
    /// How far across its depth wanders, how hard it throws, the ground's
    /// slope, and the body's pull there (m/s2).
    period: f64,
    pw: f64,
    slope: f64,
    pull: f64,
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
    /// of each column; how long (s) it takes to throw it all out, and the
    /// side of its clumps (m; 1, single cells, for a small crater).
    top: f64,
    span: f64,
    col_top: HashMap<i64, f64>,
    spread: f64,
    side: f64,
    /// What is thrown: the pieces that fly (while being worked out, a
    /// clump's `from` and `vel` are the sums over its cells), the clumps
    /// still taking cells, by when they go and their block of the crater (a
    /// square `side` m across, in its own frame: across it, and down from
    /// its top), and the cells.
    pieces: Vec<Ejecta>,
    open: std::collections::BTreeMap<(u64, i64, i64), usize>,
    cells: CellSet,
    stage: CraterStage,
    /// Its cells that the loosened column holds in big pieces (see
    /// `column_piece`), by piece: the clump each is in (in `pieces`), and
    /// how it would fly on its own. Thrown out whole as rock bodies, where
    /// there are enough of them joined up.
    held: std::collections::BTreeMap<(bool, u64, u64), Vec<(usize, Ejecta)>>,
}

/// A piece of a crater's ground to be thrown: when (the tick; while being
/// worked out, ticks after the crater goes), its cells (one, or a clump of
/// neighbours lifted and flying together), where it flies from and how fast,
/// and how far along its way it starts (s: see `blast_tick`); or (`whole`)
/// a held piece of hard rock, thrown out whole as a tumbling rock body from
/// where it lies (`lead` is then how fast it turns, radians a second), with
/// each of its cells as it would fly on its own (`each`), should it not be
/// able to go whole.
pub(super) struct Ejecta {
    pub(super) due: u64,
    pub(super) cells: Vec<(i64, i64)>,
    pub(super) from: (f64, f64),
    pub(super) vel: (f64, f64),
    pub(super) lead: f64,
    pub(super) whole: bool,
    pub(super) each: Vec<Ejecta>,
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

/// A blast's plume being worked out (see `way_up` and `blast_plume`), kept
/// from one tick to the next: the search for its way up (where it started,
/// how far it may go, and the cells seen, still to look from, and the
/// ground's height by direction); then the way up found, and its rubble
/// gone through so far for what it could blow out (how far, and what).
struct WayUp {
    from: (i64, i64),
    limit: f64,
    r0: f64,
    seen: CellSet,
    queue: std::collections::VecDeque<(i64, i64)>,
    tops: HashMap<i64, f64>,
    mouth: Option<(i64, i64)>,
    next: usize,
    pick: Vec<(u64, (i64, i64))>,
    /// Then how many of what it picked it blows out, the tick it began, and
    /// how many it has lifted out so far.
    take: Option<usize>,
    start: u64,
    launched: usize,
}

impl WayUp {
    /// A search starting from cell `from`, within reach of a blast of
    /// radius `r`.
    fn new(from: (i64, i64), r: f64) -> WayUp {
        let mut seen = CellSet::default();
        seen.insert(from);
        WayUp { from, limit: r * 3.0 + 90.0, r0: len(mid(from)).max(1.0), seen, queue: [from].into(), tops: HashMap::new(), mouth: None, next: 0, pick: Vec::new(), take: None, start: 0, launched: 0 }
    }
}

/// How far a blast has got with throwing out its held pieces (see
/// `blast_chunks`).
enum Chunks {
    /// Waiting for all its rock to be broken and loosened, and its crater
    /// to be all lifted out.
    Wait,
    /// Going through its pieces, highest first: their order, and the next.
    Next(Vec<usize>, usize),
    Done,
}

/// The steps from a node of the shock's grid to its 16 nearest: across,
/// down, and how long (in nodes). Every step is as long as the straight line,
/// so in even rock the cheapest way over the grid is at most 2.7% longer than
/// the straight line (between the steps' directions; see `grid_len`).
const SQ5: f64 = 2.236_067_977_499_79;
const FIELD_STEPS: [(i64, i64, f64); 16] = [
    (1, 0, 1.0),
    (-1, 0, 1.0),
    (0, 1, 1.0),
    (0, -1, 1.0),
    (1, 1, std::f64::consts::SQRT_2),
    (1, -1, std::f64::consts::SQRT_2),
    (-1, 1, std::f64::consts::SQRT_2),
    (-1, -1, std::f64::consts::SQRT_2),
    (2, 1, SQ5),
    (2, -1, SQ5),
    (-2, 1, SQ5),
    (-2, -1, SQ5),
    (1, 2, SQ5),
    (1, -2, SQ5),
    (-1, 2, SQ5),
    (-1, -2, SQ5),
];

/// How long the shortest way is over the grid's steps (in nodes) to a node
/// `a` across and `b` down from where it starts: the two steps whose
/// directions lie either side of the straight line's.
fn grid_len(a: i64, b: i64) -> f64 {
    let (a, b) = (a.abs().max(b.abs()) as f64, a.abs().min(b.abs()) as f64);
    if 2.0 * b <= a {
        b * SQ5 + (a - 2.0 * b)
    } else {
        (a - b) * SQ5 + (2.0 * b - a) * std::f64::consts::SQRT_2
    }
}

/// The places looked at along one side of a node's square, from `lo` to
/// `lo + g` (m), into `out` as (where, its share of the side): every cell
/// the side overlaps, by how much it overlaps, while that is no more than
/// FIELD_LOOKS + 1 cells, and otherwise FIELD_LOOKS places evenly spread.
/// (Weighed by how much of each cell is in the square, a thin layer of open
/// space counts the same wherever the grid falls on it.)
fn field_looks(out: &mut Vec<(f64, f64)>, lo: f64, g: f64) {
    out.clear();
    let (a, b) = (lo.floor() as i64, (lo + g).ceil() as i64);
    if b - a <= FIELD_LOOKS as i64 + 1 {
        for c in a..b {
            let w = ((lo + g).min(c as f64 + 1.0) - lo.max(c as f64)) / g;
            if w > 0.0 {
                out.push((c as f64 + 0.5, w));
            }
        }
    } else {
        let each = 1.0 / FIELD_LOOKS as f64;
        for k in 0..FIELD_LOOKS {
            out.push((lo + (k as f64 + 0.5) * each * g, each));
        }
    }
}

/// Where the working out of a shock's field has got to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FieldStage {
    /// Looking at what is at each node.
    Look,
    /// Giving open space the cost of the rock round it.
    Fill,
    /// The cheapest ways out, open space carried as rock...
    Free,
    /// ...and open space stopping the shock.
    Shut,
    /// The straight lines out, and what the rays read.
    Line,
    Done,
}

/// What it costs a blast's shock to reach each place round it, worked out
/// before its rays set off (docs/explosions-research.md: "arrival time with
/// fast marching"), so that a cave shields the rock behind it and a soft
/// layer carries the shock along it. On a grid of nodes round the charge
/// (FIELD_PER_R to a blast radius, out as far as the cracks can reach), each
/// node's ground costs the shock what it costs a ray, (0.45 + 1.7 x
/// hardness) / r a metre in rock, rubble 0.3 / r, water or magma 0.12 / r
/// (a liquid carries a shock well), and how much of each node's square is
/// open space (vacuum or gas) is looked at in FIELD_LOOKS^2 places. The
/// cheapest way out to each node over the grid is found (Dijkstra's way)
/// twice: once with open space carried as if it were the rock round it,
/// and once with nodes mostly open stopping the shock: it reaches a cave's
/// wall but doesn't cross the cave (a free surface reflects a shock). The
/// cost along the straight line to each node is worked out too, nearest the
/// charge first, and with it how thick the open space is that lies across
/// that line. A ray then has, at each cell, its own energy left (as it
/// always had), plus what the cheapest way there saves over the straight
/// one past GAIN_SLACK (so along a soft layer, and in the rock beside one, a
/// blast breaks further), less the shadow: SHADOW_TURN where open space
/// lies across the straight line, and the extra cost of the way round it,
/// counted SHADOW_COST times over, both in full where the open space is
/// SHADOW_FULL thick, and not at all under SHADOW_FROM. (The way round alone
/// is not enough: behind a cave a few nodes across it is only a few per
/// cent longer, and the shadow came out a faint line.) The cracks read the
/// same field.
///
/// Open space in the charge's own chamber (within the melt radius: the
/// fireball fills it), up the sapper's own shaft and along a tunneler's own
/// bore carries the shock as the rock round it would, so such a blast comes
/// out as it did.
struct ShockField {
    at: (f64, f64),
    /// The nodes' spacing (m), how many across, and the middle one's
    /// place across (and down) the grid, which is at the charge.
    g: f64,
    n: usize,
    h: i64,
    /// What it is worked out for: the blast's radius, what the rock round
    /// the charge costs a metre, and the melt radius.
    r: f64,
    k_ref: f64,
    melt: f64,
    /// The unit's own hole (the sapper's shaft, a tunneler's bore): the
    /// nodes it runs through, whose open space carries the shock.
    hole: Vec<bool>,
    /// Each node's cost a metre (for open space, the rock round it's),
    /// whether any of it carries the shock, how much of it is open space
    /// (0 to 1), and whether it stops the shock (mostly open).
    k: Vec<f64>,
    solid: Vec<bool>,
    open: Vec<f64>,
    stops: Vec<bool>,
    /// The cheapest cost out to each node with open space carried, and
    /// stopped; the cost along the straight line out, and that line's length
    /// (in nodes) as worked out the same way, to put its errors right.
    free: Vec<f64>,
    shut: Vec<f64>,
    line: Vec<f64>,
    unit: Vec<f64>,
    /// How thick the open space is across the straight line out to each
    /// node, the node itself included (in nodes: the shadow's edge is soft
    /// over a node or so).
    thick: Vec<f64>,
    /// What a ray gains at each node over its own energy (less than nothing
    /// in a shadow), what the shock spends getting there, and its shadow.
    gain: Vec<f32>,
    cost: Vec<f32>,
    shade: Vec<f32>,
    /// The nodes nearest the charge first, and the ways still to go on from
    /// (by cost, then node).
    order: Vec<u32>,
    heap: std::collections::BinaryHeap<std::cmp::Reverse<(u64, u32)>>,
    stage: FieldStage,
    next: usize,
    /// The work it took, and how long (s; for the timings).
    work: u64,
    secs: f64,
}

impl ShockField {
    /// A field round a charge at `at` of blast radius `r`, out to `reach` m;
    /// `hole` is the unit's own hole (a line of points, and how far either
    /// side of it the hole reaches).
    fn new(at: (f64, f64), r: f64, k_ref: f64, melt: f64, hole: (&[(f64, f64)], f64), reach: f64) -> ShockField {
        let g = (r / FIELD_PER_R).max(1.0);
        let h = (reach / g).ceil() as i64 + 1;
        let n = (2 * h + 1) as usize;
        let nn = n * n;
        // The nodes within a node of the hole, found along each of its
        // pieces half a node at a time.
        let mut mark = vec![false; nn];
        let wide = hole.1 + g;
        let span = (wide / g).ceil() as i64;
        for w in hole.0.windows(2) {
            let (a, b) = (w[0], w[1]);
            let steps = ((b.0 - a.0).hypot(b.1 - a.1) / (0.5 * g)).ceil().max(1.0) as usize;
            for s in 0..=steps {
                let t = s as f64 / steps as f64;
                let p = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                let (u, v) = (((p.0 - at.0) / g).round() as i64, ((p.1 - at.1) / g).round() as i64);
                if u.abs() > h + span || v.abs() > h + span {
                    continue;
                }
                for y in (v - span).max(-h)..=(v + span).min(h) {
                    for x in (u - span).max(-h)..=(u + span).min(h) {
                        let q = (at.0 + x as f64 * g, at.1 + y as f64 * g);
                        if (q.0 - p.0).hypot(q.1 - p.1) <= wide {
                            mark[((y + h) * n as i64 + x + h) as usize] = true;
                        }
                    }
                }
            }
        }
        ShockField {
            at,
            g,
            n,
            h,
            r,
            k_ref,
            melt,
            hole: mark,
            k: vec![0.0; nn],
            solid: vec![false; nn],
            open: vec![0.0; nn],
            stops: vec![false; nn],
            free: vec![f64::INFINITY; nn],
            shut: vec![f64::INFINITY; nn],
            line: vec![0.0; nn],
            unit: vec![0.0; nn],
            thick: vec![0.0; nn],
            gain: vec![0.0; nn],
            cost: vec![0.0; nn],
            shade: vec![0.0; nn],
            order: Vec::new(),
            heap: Default::default(),
            stage: FieldStage::Look,
            next: 0,
            work: 0,
            secs: 0.0,
        }
    }

    fn ready(&self) -> bool {
        self.stage == FieldStage::Done
    }

    /// Where node `i` is.
    fn pos(&self, i: usize) -> (f64, f64) {
        let (x, y) = ((i % self.n) as i64 - self.h, (i / self.n) as i64 - self.h);
        (self.at.0 + x as f64 * self.g, self.at.1 + y as f64 * self.g)
    }

    /// At `p`, from the nodes round it: what a ray gains over its own
    /// energy, what the shock spends getting there, and its shadow (None
    /// off the grid).
    fn at(&self, p: (f64, f64)) -> Option<(f64, f64, f64)> {
        let u = (p.0 - self.at.0) / self.g + self.h as f64;
        let v = (p.1 - self.at.1) / self.g + self.h as f64;
        let top = (self.n - 1) as f64;
        if !(u >= 0.0 && v >= 0.0 && u < top && v < top) {
            return None;
        }
        let (i, j) = (u as usize, v as usize);
        let (fu, fv) = (u - i as f64, v - j as f64);
        let ix = j * self.n + i;
        let ids = [ix, ix + 1, ix + self.n, ix + self.n + 1];
        let w = [(1.0 - fu) * (1.0 - fv), fu * (1.0 - fv), (1.0 - fu) * fv, fu * fv];
        let mix = |v: &[f32]| ids.iter().zip(w).map(|(&i, w)| v[i] as f64 * w).sum::<f64>();
        Some((mix(&self.gain), mix(&self.cost), mix(&self.shade)))
    }

    /// `v` between the nodes round the point (`x`, `y`) across and down the
    /// grid.
    fn mix(&self, v: &[f64], x: f64, y: f64) -> f64 {
        let (i, j) = (x as usize, y as usize);
        let (fu, fv) = (x - i as f64, y - j as f64);
        let ix = j * self.n + i;
        v[ix] * (1.0 - fu) * (1.0 - fv) + v[ix + 1] * fu * (1.0 - fv) + v[ix + self.n] * (1.0 - fu) * fv + v[ix + self.n + 1] * fu * fv
    }

    /// Open space at node `i` that carries the shock as the rock round it
    /// would: the charge's own chamber, and the unit's own hole.
    fn carries(&self, i: usize) -> bool {
        let p = self.pos(i);
        let (dx, dy) = (p.0 - self.at.0, p.1 - self.at.1);
        (dx * dx + dy * dy).sqrt() <= self.melt.max(1.5 * self.g) || self.hole[i]
    }

    /// Open space carried as the rock round it: its cost is that rock's
    /// (the nodes of rock beside it, or the rock round the charge), and the
    /// nodes that stop the shock (mostly open) are marked. Then the nodes
    /// are put in order, nearest the charge first, and the first search set
    /// going. Returns the work done.
    fn fill(&mut self) -> u64 {
        let (n, h) = (self.n as i64, self.h);
        for i in 0..self.n * self.n {
            if self.open[i] > 0.0 && self.carries(i) {
                self.open[i] = 0.0;
            }
            self.stops[i] = self.open[i] >= 0.5;
            if self.solid[i] {
                continue;
            }
            let (x, y) = ((i as i64) % n, (i as i64) / n);
            let (mut sum, mut cnt) = (0.0, 0);
            for (dx, dy) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
                let (a, b) = (x + dx, y + dy);
                if a >= 0 && b >= 0 && a < n && b < n && self.solid[(b * n + a) as usize] {
                    sum += self.k[(b * n + a) as usize];
                    cnt += 1;
                }
            }
            self.k[i] = if cnt > 0 { sum / cnt as f64 } else { self.k_ref };
        }
        // (By distance squared, in whole nodes: counted, not sorted.)
        let mut counts = vec![0u32; (2 * h * h + 2) as usize];
        for i in 0..self.n * self.n {
            let (x, y) = ((i as i64) % n - h, (i as i64) / n - h);
            counts[(x * x + y * y) as usize + 1] += 1;
        }
        for d in 1..counts.len() {
            counts[d] += counts[d - 1];
        }
        self.order = vec![0; self.n * self.n];
        for i in 0..self.n * self.n {
            let (x, y) = ((i as i64) % n - h, (i as i64) / n - h);
            let d = (x * x + y * y) as usize;
            self.order[counts[d] as usize] = i as u32;
            counts[d] += 1;
        }
        let c = (h * n + h) as usize;
        self.free[c] = 0.0;
        self.heap.push(std::cmp::Reverse((0, c as u32)));
        (self.n * self.n) as u64 / 2
    }

    /// The cheapest ways out from the charge, open space carried (`shut`
    /// false) or stopping the shock (true): it can be reached, but the
    /// shock goes no further from it, and doesn't slip between two such
    /// nodes. While `budget` lasts; true once all done.
    fn march(&mut self, shut: bool, budget: &mut u64) -> bool {
        let (n, g) = (self.n as i64, self.g);
        let (k, stops) = (&self.k, &self.stops);
        let (dist, heap) = (if shut { &mut self.shut } else { &mut self.free }, &mut self.heap);
        let stop = |x: i64, y: i64| shut && stops[(y * n + x) as usize];
        while *budget > 0 {
            let Some(std::cmp::Reverse((bits, i))) = heap.pop() else { return true };
            *budget = budget.saturating_sub(WORK_FIELD);
            self.work += WORK_FIELD;
            let (i, c) = (i as usize, f64::from_bits(bits));
            let (x, y) = ((i as i64) % n, (i as i64) / n);
            if c > dist[i] || stop(x, y) {
                continue;
            }
            for &(dx, dy, l) in &FIELD_STEPS {
                let (a, b) = (x + dx, y + dy);
                if a < 0 || b < 0 || a >= n || b >= n {
                    continue;
                }
                let j = (b * n + a) as usize;
                // A long step costs what the two nodes it passes between
                // do as well; it and a slanting one are closed if both of
                // the nodes they pass between stop the shock. (Closing a long
                // step if either did made the shock's way along a cave's
                // wall or the ground a few per cent dearer than its way
                // through the rock beside them, a shadow where there is none.)
                let mut sum = (k[i] + k[j]) * 0.5;
                if dx.abs() == 2 || dy.abs() == 2 {
                    let (m1, m2) = if dx.abs() == 2 { ((x + dx / 2, y), (x + dx / 2, b)) } else { ((x, y + dy / 2), (a, y + dy / 2)) };
                    if stop(m1.0, m1.1) && stop(m2.0, m2.1) {
                        continue;
                    }
                    sum = (sum + (k[(m1.1 * n + m1.0) as usize] + k[(m2.1 * n + m2.0) as usize]) * 0.5) * 0.5;
                } else if dx != 0 && dy != 0 && stop(a, y) && stop(x, b) {
                    continue;
                }
                let to = c + l * g * sum;
                if to < dist[j] {
                    dist[j] = to;
                    heap.push(std::cmp::Reverse((to.to_bits(), j as u32)));
                }
            }
        }
        heap.is_empty()
    }

    /// The straight line out to node `i` (all nodes nearer the charge done
    /// already), and what the rays and cracks read there.
    fn line_to(&mut self, i: usize) {
        let (x, y) = ((i % self.n) as i64 - self.h, (i / self.n) as i64 - self.h);
        let d2 = x * x + y * y;
        if d2 == 0 {
            return;
        }
        let d = (d2 as f64).sqrt();
        let c = (self.h * self.n as i64 + self.h) as usize;
        // (How thick the open space across the line is short of this node:
        // open space itself is not in its own shadow, so the rock on the
        // charge's side of a cave, between this node and the next, is not.)
        let before = if d2 <= 2 {
            self.line[i] = self.g * d * (self.k[c] + self.k[i]) * 0.5;
            self.unit[i] = d;
            0.0
        } else {
            // From the line's point two nodes back towards the charge
            // (the nodes round it are all nearer the charge than this one),
            // and the open space there and one node back. (Summed a node at
            // a time along the line, between the nodes round each point, a
            // thin layer of open space counts as thick as it is wherever
            // the grid falls.)
            let s = 1.0 - 2.0 / d;
            let (qx, qy) = (self.h as f64 + x as f64 * s, self.h as f64 + y as f64 * s);
            self.line[i] = self.mix(&self.line, qx, qy) + self.g * (self.k[i] + self.mix(&self.k, qx, qy));
            self.unit[i] = self.mix(&self.unit, qx, qy) + 2.0;
            let s1 = 1.0 - 1.0 / d;
            let (mx, my) = (self.h as f64 + x as f64 * s1, self.h as f64 + y as f64 * s1);
            self.mix(&self.thick, qx, qy) + self.mix(&self.open, mx, my)
        };
        self.thick[i] = before + self.open[i];
        let f = ((before - SHADOW_FROM) / (SHADOW_FULL - SHADOW_FROM)).clamp(0.0, 1.0);
        // The grid's errors put right: in even rock, the cheapest ways and
        // the straight line then cost what the straight line truly does.
        let fix = d / grid_len(x, y);
        let free = self.free[i] * fix;
        // The way round open space counts only where it blocks the straight
        // line (elsewhere the grid's small turns round a node that stops
        // the shock are no shadow), and only so far.
        let extra = (self.shut[i] * fix - free).clamp(0.0, SHADOW_MAX / SHADOW_COST);
        let shade = (f * (SHADOW_COST * extra + SHADOW_TURN)).min(SHADOW_MAX);
        let shut = free + f * extra;
        let line = self.line[i] * d / self.unit[i];
        self.gain[i] = ((line * (1.0 - GAIN_SLACK) - shut).max(0.0) - shade) as f32;
        self.cost[i] = (shut + shade) as f32;
        self.shade[i] = shade as f32;
    }
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
    /// How far the ground is over the charge (0: no column), the same
    /// whether or not there is a column (what the cracks and the chimney
    /// stop under), and the ground there.
    height: f64,
    ground_h: f64,
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
    /// Where each ray stopped breaking rock (m; `reach` if it never did),
    /// and what the shock had spent getting there by the field, outside any
    /// shadow, less the energy the ray was short by (not a number if it
    /// never stopped), which its cracks run on from.
    stop: Vec<f64>,
    stop_cost: Vec<f64>,
    /// The shock's field (see `ShockField`), worked out before the rays set
    /// off; and what the shock spends in the rock round the charge going
    /// CRACK_REACH radii (how its spending past a ray's end is turned into
    /// how far into the cracked zone a place is).
    field: ShockField,
    crack_cost: f64,
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
    /// The far cracked zone (CRACK_FAR): its three patterns of blocks,
    /// bigger further out; the distances from the charge each pattern's
    /// sites are looked for in; its blocks waiting for the front, the
    /// soonest last (listed on the first shell); and whether those too far
    /// out to crack have been dropped (once all the rays are done).
    far_blocks: [Blocks; 3],
    far_range: [(f64, f64); 3],
    far: Option<Vec<FarBlock>>,
    far_pruned: bool,
    /// Which of the far blocks asked about so far crack (see `far_cracked`).
    far_seen: HashMap<(u8, i64, i64), bool>,
    /// Its crater, worked out ahead of the front (on screen, with a
    /// column), and the cells thrown out once the front reaches the ground.
    plan: Option<Crater>,
    crater: CellSet,
    crater_done: bool,
    cratered: bool,
    /// (The cells of a shell, kept to save allocating them every shell.)
    cells: Vec<(i64, i64)>,
    fin: Fin,
    /// Its plume's search for a way up, while that goes on.
    way: Option<WayUp>,
    /// The molten heart's radius (m); how tall its rubble chimney is over
    /// the charge (m; 0 where a loosened column already took that ground),
    /// and whether that is settled yet (once the rays are all done, from
    /// where those going up stopped); where its collapse starts climbing
    /// from (m over the charge), and how far up it has fallen in so far.
    melt_r: f64,
    chimney: f64,
    chimney_set: bool,
    chimney_from: f64,
    chimney_up: f64,
    /// Its held pieces (on screen only): the cells of each, in the order
    /// found, and which is which (from the rays or the column, the size of
    /// its pattern, and its piece of that); the tick by which its crater's
    /// ground has all been lifted out; how far it has got with throwing
    /// pieces out, and how many it threw and how many cells they held.
    held: Vec<Vec<(i64, i64)>>,
    held_ix: HashMap<(bool, u64, u64), usize>,
    crater_open: u64,
    chunks: Chunks,
    flung: usize,
    flung_cells: usize,
}

impl BlastJob {
    /// Whether it has run its course: its front, all it does after, its
    /// chimney, and its held pieces thrown out.
    fn done(&self) -> bool {
        self.front_done && matches!(self.fin, Fin::Done) && self.chimney_up >= self.chimney && matches!(self.chunks, Chunks::Done)
    }

    /// Cell `c` is held in piece `key` (kept on screen only, where some of
    /// the pieces may be thrown out whole).
    fn hold(&mut self, key: (bool, u64, u64), c: (i64, i64)) {
        if !self.seen {
            return;
        }
        let n = self.held.len();
        let i = *self.held_ix.entry(key).or_insert(n);
        if i == n {
            self.held.push(Vec::new());
        }
        self.held[i].push(c);
    }

    /// How far either side of the chimney's middle it reaches at `h` m over
    /// the charge: narrowing upwards, and rounded off over its top fifth.
    fn chimney_w(&self, h: f64) -> f64 {
        if h < 0.0 || h > self.chimney {
            return 0.0;
        }
        let f = h / self.chimney;
        let w = self.melt_r * (CHIMNEY_BASE - (CHIMNEY_BASE - CHIMNEY_TOP) * f);
        if f <= 0.8 {
            return w;
        }
        let g = (f - 0.8) / 0.2;
        w * (1.0 - g * g).max(0.0).sqrt()
    }

    /// How far into the cracked zone the point `p` lies, from 0 where the
    /// rays stopped breaking rock (or beside the loosened column) to 1
    /// CRACK_REACH radii further; and the same from the rays alone, for the
    /// far zone (which never reaches further out than it would in even rock).
    fn crack_t(&self, p: (f64, f64)) -> (f64, f64) {
        let (dx, dy) = (p.0 - self.at.0, p.1 - self.at.1);
        let rays = self.stop.len().max(1);
        // Round the charge, past where the rays stopped.
        let d = (dx * dx + dy * dy).sqrt();
        let k = ((dy.atan2(dx) / std::f64::consts::TAU * rays as f64).round() as i64).rem_euclid(rays as i64) as usize;
        let td = ((d - self.stop[k]) / (self.r * CRACK_REACH)).max(0.0);
        // By what the shock spends getting there past the ray's end, as
        // the field has it: further along a soft layer (up to CRACK_SOFT
        // times), never less far than in even rock (measured so, a place
        // where the rock was harder than round the charge had its cracks
        // cut short, by up to half at 2 radii), and less far into the
        // shadow of open space. The far zone takes the shadow alone.
        let (tr, mut t, shade) = match self.field.at(p) {
            Some((_, cost, raw)) => {
                let shade = raw / self.crack_cost;
                if self.stop_cost[k].is_finite() {
                    let tf = ((cost - raw - self.stop_cost[k]) / self.crack_cost).max(0.0);
                    (td + shade, td.min(tf.max(td / CRACK_SOFT)) + shade, shade)
                } else {
                    (td + shade, td + shade, shade)
                }
            }
            None => (td, td, 0.0),
        };
        // Either side of the column, more widely near the surface.
        if self.height > 0.0 {
            let along = dx * self.up.0 + dy * self.up.1;
            if along >= -0.5 * self.r && along <= self.height + 2.0 {
                let f = (along / self.height).clamp(0.0, 1.0);
                let half = self.r * (1.0 + (self.lt - 1.0) * f);
                let out = (dx * self.side.0 + dy * self.side.1).abs() - half;
                t = t.min((out / (self.r * (0.35 + 0.65 * f))).max(0.0) + shade);
            }
        }
        (t, tr)
    }

    /// How far into the far cracked zone the point `p` lies, from 0 at its
    /// inner edge (FAR_FROM into the near zone past where the rays stopped)
    /// to 1 at its outer edge (CRACK_FAR past the near zone).
    fn far_u(&self, p: (f64, f64)) -> f64 {
        far_u_of(self.crack_t(p).1)
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
                            self.cell_mined(q);
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
        // A clump's cells are lifted together and fly as one piece.
        let mut flying = Vec::new();
        let mut whole = Vec::new();
        while self.eject.front().is_some_and(|e| e.due <= self.tick) {
            let Some(e) = self.eject.pop_front() else { break };
            if e.whole {
                whole.push(e);
                continue;
            }
            let mut mats = Vec::with_capacity(e.cells.len());
            for &c in &e.cells {
                let Some(m) = self.world.lift_cell(c.0, c.1) else { continue };
                // (Off the cracked cells too: left there, the hole was taken
                // for cracked rock once something filled it.)
                self.cell_mined(c);
                self.progress.remove(&c);
                self.reserved.remove(&c);
                self.open_cell(c);
                mats.push(m);
            }
            if mats.is_empty() {
                continue;
            }
            let (from, vel) = self.eject_start(e.from, e.vel, e.lead);
            flying.push((from, mats, vel));
        }
        // Held pieces of hard rock, thrown out whole as rock bodies once the
        // rest of the ground due with them is out of their way. (Out of
        // sight, or with too many bodies in the air already, its cells fly
        // each as it would have with the rest of the crater, one by one; so
        // does any of its cells that is no longer solid rock.)
        for e in whole {
            let mut gone = self.bodies.throw_chunk(&mut self.world, &e.cells, e.vel, e.lead);
            let taken: CellSet = gone.iter().copied().collect();
            for q in &e.each {
                let c = q.cells[0];
                if taken.contains(&c) {
                    continue;
                }
                if let Some(m) = self.world.lift_cell(c.0, c.1) {
                    let (from, vel) = self.eject_start(q.from, q.vel, q.lead);
                    flying.push((from, vec![m], vel));
                    gone.push(c);
                }
            }
            for c in gone {
                self.loose.remove(&c);
                self.fractured.remove(&c);
                self.progress.remove(&c);
                self.reserved.remove(&c);
                self.open_cell(c);
            }
        }
        let mut landed = Vec::new();
        for (from, mats, vel) in flying {
            self.bodies.throw_clump(&mut self.world, from, &mats, vel, &mut landed);
        }
        for (t, _) in landed {
            self.loose.insert(t);
            self.world.set_look(t.0, t.1, Look::RUBBLE);
            self.settling.insert(t);
            self.scrap_landed(t);
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

    /// Where and how fast a piece of a crater's ground thrown from `from` at
    /// `vel` starts, `lead` s along its way. It starts as far along as it
    /// would have got had it left with the top of its column (the deeper,
    /// the further), so a column goes up as a stream, not as a sheet over
    /// the hole: slow ground all thrown from the column's mouth a tick or so
    /// apart hung there in one layer (27 Nov). Where that place isn't open,
    /// it goes from the mouth.
    fn eject_start(&self, from: (f64, f64), vel: (f64, f64), lead: f64) -> ((f64, f64), (f64, f64)) {
        if lead <= 0.0 {
            return (from, vel);
        }
        let a = crate::bodies::pull(&self.world.planet, from.0, from.1);
        let t = lead;
        let p = (from.0 + vel.0 * t + 0.5 * a.0 * t * t, from.1 + vel.1 * t + 0.5 * a.1 * t * t);
        let c = cell_of(p);
        if self.world.in_bounds(c.0, c.1) && self.world.material(c.0, c.1) == VACUUM {
            (p, (vel.0 + a.0 * t, vel.1 + a.1 * t))
        } else {
            (from, vel)
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
            // The rubble moves on and leaves an empty cell, not a cracked
            // one. (Where it lands it is cut as loose, so it needs no
            // cracked mark there either.)
            self.loose.remove(&c);
            self.fractured.remove(&c);
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
        self.blast_in_hole(at, yield_kt, fissile, &[], 0.0);
    }

    /// A blast going off in a unit's own hole (a tunneler's bore: a line of
    /// points, and how far either side of it the bore reaches), whose open
    /// space carries the shock as the rock round it would (see
    /// `ShockField`).
    pub(super) fn blast_in_hole(&mut self, at: (f64, f64), yield_kt: f64, fissile: f64, bore: &[(f64, f64)], half: f64) {
        let q0 = clock();
        self.blast_start(at, yield_kt, fissile, bore, half);
        self.prof2[7] += clock() - q0;
    }

    /// A test: a charge of `kt` going off at `at` with no loosened column,
    /// in a bore along `bore` reaching `half` m either side of it, as a
    /// tunneler's end.
    pub fn test_bore_blast(&mut self, at: (f64, f64), kt: f64, bore: &[(f64, f64)], half: f64) {
        let fissile = self.fissile_per_charge();
        self.unit_blast(at, kt, fissile, bore, half);
    }

    /// What happens the moment a charge goes off: the flash, the counts, the
    /// drones and sappers caught in it; then its shock front is set going.
    pub(super) fn blast_start(&mut self, at: (f64, f64), yield_kt: f64, fissile: f64, bore: &[(f64, f64)], half: f64) {
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
        // (Looked up with no column too: the cracks stop at the ground, and
        // the chimney under it.)
        let ground = self.ground_beside(at, r);
        let ground_h = (ground - len(at)).max(1.0);
        let (top, height) = if column {
            let a = at.1.atan2(at.0);
            ((a.cos() * ground, a.sin() * ground), ground_h)
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
        // (`inner` is also how far the heart melts in rock this hard: the
        // cavity whose roof falls in as the chimney, where there is no
        // loosened column. This is the least the chimney rises; it may rise
        // more once the rays show how far they broke the ground.)
        let chimney = if column { 0.0 } else { CHIMNEY_TALL * inner };
        let throw_share = (THROW_MAX as f64 / likely.max(1.0)).min(1.0);
        // The box the cracks' blocks are kept over: as far as the cracks can
        // reach (CRACK_REACH past the farthest a ray can go, or beside the
        // column).
        let far = reach as f64 + r * CRACK_REACH;
        let reach_all = far.max((height + 2.0).hypot(r * lt + r)) + 2.0;
        let (x0, y0, x1, y1) = ((at.0 - reach_all) as i64, (at.1 - reach_all) as i64, (at.0 + reach_all) as i64, (at.1 + reach_all) as i64);
        let blocks = [3.5, 5.0, 7.0, 10.0].map(|size| Blocks::new(size, blast, x0, y0, x1, y1));
        // The far zone's blocks, and how far out each pattern's sites may
        // lie: its third of the far zone, from as near as a ray can stop
        // (each step costs a ray at most 2.07 / r of its energy, so none
        // stops short of about half a radius) to as far as
        // one can reach, widened by how far a block reaches from its site
        // (no further than 1.42 sides: every point has a site in its own
        // square of the grid).
        let far_sizes = FAR_SIZES.map(|s| s * (r / 30.0).sqrt().max(1.0));
        let far_range: [(f64, f64); 3] = std::array::from_fn(|b| {
            let (third, spill, inner) = (r * FAR_W / 3.0, 1.42 * far_sizes[b], r * CRACK_REACH * FAR_FROM);
            (0.4 * r + inner + third * b as f64 - spill, reach as f64 + inner + third * (b + 1) as f64 + spill)
        });
        let out = far_range[2].1;
        let far_blocks = far_sizes.map(|size| Blocks::new(size, blast, (at.0 - out) as i64, (at.1 - out) as i64, (at.0 + out) as i64, (at.1 + out) as i64));
        let plan = (seen && column).then(|| self.crater_plan(at, r, yield_kt, height, blast));
        let column_end = if column { ((height + 1.0).powi(2) + (r * lt).powi(2)).sqrt() + 1.0 } else { 0.0 };
        // The shock's field, out as far as any crack can reach (the far zone
        // past the farthest a ray can go, or beside the column).
        let field_reach = (reach as f64 + r * (CRACK_REACH + CRACK_FAR)).max((height + 2.0).hypot(r * lt + r)) + 2.0;
        // (Its own hole: the sapper's shaft up to the ground, or the bore
        // it was given.)
        let shaft = [at, top];
        let hole: (&[(f64, f64)], f64) = if column { (&shaft, SHAFT_HALF as f64) } else { (bore, half) };
        let field = ShockField::new(at, r, (0.45 + 1.7 * h) / r, inner, hole, field_reach);
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
            ground_h,
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
            stop_cost: vec![f64::NAN; rays],
            field,
            crack_cost: CRACK_REACH * (0.45 + 1.7 * h),
            ray_end: None,
            broken: CellMap::default(),
            rubble: Vec::new(),
            here: Vec::new(),
            front: 0.0,
            shell: 0,
            front_done: false,
            column_end,
            blocks,
            far_blocks,
            far_range,
            far: None,
            far_pruned: false,
            far_seen: HashMap::new(),
            crater_done: plan.is_none(),
            plan,
            crater: CellSet::default(),
            cratered: false,
            cells: Vec::new(),
            fin: Fin::Rubble,
            way: None,
            melt_r: inner,
            chimney,
            chimney_set: false,
            chimney_from: 0.0,
            chimney_up: 0.0,
            held: Vec::new(),
            held_ix: HashMap::new(),
            crater_open: 0,
            chunks: if seen { Chunks::Wait } else { Chunks::Done },
            flung: 0,
            flung_cells: 0,
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

    /// The cells cracked so far (by blasts' shocks and held pieces, and by
    /// tunnelers and openers), which are cut in half the time.
    pub fn cracked_cells(&self) -> impl Iterator<Item = (i64, i64)> + '_ {
        self.fractured.iter()
    }

    /// The loose cells (rubble, melt and thrown rock, which cutters take
    /// quickly, and which settle).
    pub fn loose_cells(&self) -> impl Iterator<Item = (i64, i64)> + '_ {
        self.loose.iter()
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
                // (Its pieces are in the order they go.)
                j.crater_open = self.tick + cr.pieces.last().map_or(0, |p| p.due);
                // Its held pieces thrown out whole count towards the
                // blast's CHUNKS_MAX.
                for p in cr.pieces.iter().filter(|p| p.whole) {
                    j.flung += 1;
                    j.flung_cells += p.cells.len();
                }
                j.crater = self.crater_throw(cr);
            }
            j.cratered = !j.crater.is_empty();
            j.crater_done = true;
            *budget = budget.saturating_sub(j.crater.len() as u64);
        }
        // The shock's field is worked out before the rays set off (they
        // and the cracks read it): its grid is as many nodes across at any
        // yield, 670,000 to 830,000 work (35 to 70 ms, most of it looking at
        // the ground all over each node), so it takes a blast's first six
        // or seven ticks, and the front catches up after.
        if !j.field.ready() && *budget > 0 {
            let q0 = clock();
            self.field_work(&mut j.field, budget);
            j.field.secs += clock() - q0;
            if j.field.ready() {
                self.last_field = (j.field.n * j.field.n, j.field.work, j.field.secs);
            }
        }
        while j.field.ready() && !j.front_done && *budget > 0 && j.shell as f64 * SHELL_M < j.front {
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
            work += self.blast_far_cracks(j, hi);
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
            if j.live.is_empty() && j.crater_done && lo > j.column_end && lo > crack_end && j.far.as_ref().is_some_and(|q| q.is_empty()) {
                j.front_done = true;
            }
        }
        // What the front left of the budget goes to the crater too (a big
        // one takes many ticks to work out, and once the rock is all dealt
        // with, the blast only waits for it).
        if let Some(cr) = j.plan.as_mut().filter(|cr| cr.stage != CraterStage::Ready) {
            self.crater_work(cr, budget);
        }
        self.blast_chunks(j, budget);
        // (The plume before the chimney: see `blast_chimney`.)
        self.blast_fin(j, budget);
        self.blast_chimney(j, budget);
    }

    /// What a blast does once all its rubble is made, while the budget
    /// lasts: the plume, then the unreacted uranium left through the rubble.
    fn blast_fin(&mut self, j: &mut BlastJob, budget: &mut u64) {
        loop {
            match j.fin {
                Fin::Rubble | Fin::Done => return,
                Fin::Plume => {
                    if Self::plume_wanted(j) {
                        if *budget == 0 {
                            return;
                        }
                        // Its way up searched for, its rubble gone through,
                        // and what it blows out lifted, a budget's worth at
                        // a time.
                        let mut way = j.way.take().unwrap_or_else(|| WayUp::new(j.c0, j.r));
                        if way.mouth.is_none() {
                            match self.way_up(&mut way, budget) {
                                None => {
                                    j.way = Some(way);
                                    return;
                                }
                                Some(m) => way.mouth = m,
                            }
                        }
                        if way.mouth.is_some() && !(self.plume_rubble(j, &mut way, budget) && self.blast_plume(j, &mut way, budget)) {
                            j.way = Some(way);
                            return;
                        }
                    }
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

    /// A blast's shock field (see `ShockField`) worked out while `budget`
    /// lasts; the rest next tick. What is at each node is looked at in the
    /// world as it is when the blast goes off, all over the node's square
    /// (see `field_looks`).
    fn field_work(&self, f: &mut ShockField, budget: &mut u64) {
        let nn = f.n * f.n;
        let space = self.world.planet.max_radius + 1.0;
        let (mut xs, mut ys) = (Vec::new(), Vec::new());
        while *budget > 0 && !f.ready() {
            let before = f.work;
            match f.stage {
                FieldStage::Look => {
                    let end = (f.next + 64).min(nn);
                    for i in f.next..end {
                        let (x, y) = ((i % f.n) as i64 - f.h, (i / f.n) as i64 - f.h);
                        // (The grid's corners, past where any crack can reach,
                        // are taken to be rock like that round the charge.)
                        if x * x + y * y > f.h * f.h {
                            f.solid[i] = true;
                            f.k[i] = f.k_ref;
                            f.work += 1;
                            continue;
                        }
                        let p = f.pos(i);
                        field_looks(&mut xs, p.0 - 0.5 * f.g, f.g);
                        field_looks(&mut ys, p.1 - 0.5 * f.g, f.g);
                        let (mut open, mut sum, mut held) = (0.0, 0.0, 0.0);
                        for &(qx, wx) in &xs {
                            for &(qy, wy) in &ys {
                                let (q, w) = ((qx, qy), wx * wy);
                                // (Out in space there is nothing to ask the
                                // world.)
                                if len(q) > space {
                                    open += w;
                                    f.work += 1;
                                    continue;
                                }
                                f.work += WORK_FIELD_LOOK;
                                let c = cell_of(q);
                                // (As the rays have it: powder is rock, and a
                                // liquid carries the shock cheaply.)
                                let m = self.world.material(c.0, c.1);
                                if m == VACUUM {
                                    open += w;
                                    continue;
                                }
                                let k = if self.loose.contains(&c) {
                                    0.3
                                } else {
                                    match self.world.planet.mats.get(m).state {
                                        State::Gas => {
                                            open += w;
                                            continue;
                                        }
                                        State::Liquid => 0.12,
                                        _ => 0.45 + 1.7 * blast_hardness(self, m),
                                    }
                                };
                                sum += k * w;
                                held += w;
                            }
                        }
                        f.open[i] = open;
                        if held > 0.0 {
                            f.solid[i] = true;
                            f.k[i] = sum / held / f.r;
                        }
                    }
                    f.next = end;
                    if end == nn {
                        f.stage = FieldStage::Fill;
                    }
                }
                FieldStage::Fill => {
                    f.work += f.fill();
                    f.stage = FieldStage::Free;
                }
                FieldStage::Free => {
                    if f.march(false, budget) {
                        let c = (f.h * f.n as i64 + f.h) as usize;
                        f.shut[c] = 0.0;
                        f.heap.push(std::cmp::Reverse((0, c as u32)));
                        f.stage = FieldStage::Shut;
                    }
                    continue;
                }
                FieldStage::Shut => {
                    if f.march(true, budget) {
                        f.stage = FieldStage::Line;
                        f.next = 0;
                    }
                    continue;
                }
                FieldStage::Line => {
                    let end = (f.next + 1024).min(nn);
                    for k in f.next..end {
                        f.line_to(f.order[k] as usize);
                    }
                    f.work += (end - f.next) as u64;
                    f.next = end;
                    if end == nn {
                        // (Only what the rays and cracks read is kept.)
                        f.stage = FieldStage::Done;
                        f.order = Vec::new();
                        f.heap = Default::default();
                        f.hole = Vec::new();
                        f.k = Vec::new();
                        f.solid = Vec::new();
                        f.open = Vec::new();
                        f.stops = Vec::new();
                        f.free = Vec::new();
                        f.shut = Vec::new();
                        f.line = Vec::new();
                        f.unit = Vec::new();
                        f.thick = Vec::new();
                    }
                }
                FieldStage::Done => {}
            }
            *budget = budget.saturating_sub(f.work - before);
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
                // What the shock has here by the field: more where it gets
                // here more cheaply another way than straight along the ray
                // (along a soft layer), less in the shadow of open space.
                let (gain, cost, shade) = j.field.at(mid(c)).unwrap_or((0.0, f64::NAN, 0.0));
                let e = (ray.e + gain).min(1.0);
                if e <= 0.0 {
                    ray.stopped = true;
                    j.stop[k] = step as f64;
                    // (Its cracks run on from what the shock spent here
                    // outside any shadow: in one, they get the shadow's
                    // whole cost on top.)
                    j.stop_cost[k] = cost - shade + (e + shade).min(0.0);
                    // A crack may run on, wandering, further in harder rock
                    // (not into a shadow).
                    if k % 4 == 0 && shade < SHADOW_CRACK {
                        ray.crack = Some(Crack { x: c.0 as f64, y: c.1 as f64, a: ray.a, k: 0, len: (r * (0.3 + 0.9 * h)) as i64, h });
                    }
                    break;
                }
                j.broken.insert(c, (e, step as f64));
                self.units.cracked += 1;
                work += self.blast_broken(j, c, h, e, step as f64);
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
        // All the rays done: how far out the shock can crack rock all
        // through (further along a soft layer: see `crack_t`).
        if j.live.is_empty() && j.ray_end.is_none() {
            j.ray_end = Some(j.stop.iter().fold(0.0f64, |a, &b| a.max(b)) + j.r * CRACK_REACH * CRACK_SOFT + 1.0);
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
                    j.hold((false, size.to_bits(), id), c);
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
                // (A cell a ray cracked on its way may be thrown: it leaves
                // no cracked mark on the hole.)
                self.cell_mined(c);
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
            if let Some(key) = column_piece(c, hd, j.blast) {
                if self.fractured.insert(c) {
                    self.units.cracked += 1;
                }
                self.world.set_look(c.0, c.1, Look::HELD);
                j.hold(key, c);
                work += WORK_CHANGE;
                continue;
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

    /// The roof of a blast's cavity falling in (where it has no loosened
    /// column: a tunneler's end): CHIMNEY_DELAY_S after the blast, the rock
    /// over the broken ground, in a chimney narrowing up to CHIMNEY_TALL melt
    /// radii over the charge (or CHIMNEY_OVER past the broken ground, if
    /// that is higher), is left loose (rubble) where it is, a band at a time
    /// from the top of the broken ground up over CHIMNEY_CLIMB_S, and
    /// settles into what opened under it. Only once the front has passed all
    /// of it, so the rays and the cracks have dealt with that rock first and
    /// come out as they would without it. While `budget` lasts; the rest
    /// goes on next tick.
    ///
    /// Its rubble is not in the blast's own (`rubble`): that is what the
    /// fireball broke, through which the unreacted uranium is left and out
    /// of which the plume blows. The chimney is rock from over it that falls
    /// in afterwards, which saw neither.
    fn blast_chimney(&mut self, j: &mut BlastJob, budget: &mut u64) {
        if j.chimney_up >= j.chimney || *budget == 0 || j.ray_end.is_none() {
            return;
        }
        // Once the rays are all done: where those going straight up stopped
        // breaking rock (the middle of them, as one may run far up a crack
        // or a cave), and the lowest of them, under which all is broken.
        if !j.chimney_set {
            j.chimney_set = true;
            use std::f64::consts::{PI, TAU};
            let a_up = j.up.1.atan2(j.up.0);
            let off_up = |a: f64| ((a - a_up + PI).rem_euclid(TAU) - PI).abs();
            let mut ups: Vec<f64> = j.rays.iter().zip(&j.stop).filter(|(ray, _)| off_up(ray.a) < CHIMNEY_RAYS).map(|(_, &d)| d).collect();
            *budget = budget.saturating_sub(j.rays.len() as u64);
            if !ups.is_empty() {
                ups.sort_by(|a, b| a.total_cmp(b));
                j.chimney = j.chimney.max(ups[ups.len() / 2] + CHIMNEY_OVER * j.melt_r);
            }
            // It stops a melt radius under the ground: there is almost no
            // empty space under it for the surface to sink into, so reaching
            // it only turned a strip of the surface to rubble where it lay,
            // which then ran down any slope beside it.
            j.chimney = j.chimney.min((j.ground_h - j.melt_r).max(0.0));
            if !ups.is_empty() {
                j.chimney_from = (ups[0] - 1.0).clamp(0.0, j.chimney);
            }
            j.chimney_up = j.chimney_from;
        }
        // (Not before the plume has found its way up, or not: it may go
        // through the broken rock but not the roof that falls in later, and
        // a save, which finishes the blast at once, must find the same way.)
        if matches!(j.fin, Fin::Rubble | Fin::Plume) {
            return;
        }
        let tall = j.chimney;
        let reach = tall.hypot(CHIMNEY_TOP * j.melt_r).max(CHIMNEY_BASE * j.melt_r) + 1.0;
        if !j.front_done && j.shell as f64 * SHELL_M <= reach {
            return;
        }
        // (The front has run FRONT_MPS since the blast, so it tells the
        // time; `finish_blasts` sets it to infinity, and all of it falls in.)
        let since = j.front / FRONT_MPS - CHIMNEY_DELAY_S;
        let want = j.chimney_from + (tall - j.chimney_from) * (since / CHIMNEY_CLIMB_S).clamp(0.0, 1.0);
        let mut cells = std::mem::take(&mut j.cells);
        while j.chimney_up < want && *budget > 0 {
            let (h0, h1) = (j.chimney_up, j.chimney_up + SHELL_M);
            // (Narrowing upwards: the band is as wide as its bottom.)
            band_cells(&mut cells, j.at, j.up, h0, h1, j.chimney_w(h0).max(j.chimney_w(h1)) + 1.0);
            let mut work = 1 + cells.len() as u64 / WORK_PASS;
            for &c in &cells {
                let p = mid(c);
                let (dx, dy) = (p.0 - j.at.0, p.1 - j.at.1);
                let across = (dx * j.side.0 + dy * j.side.1).abs();
                let w = j.chimney_w(dx * j.up.0 + dy * j.up.1);
                // (Its sides and roof frayed, not cut clean.)
                if across >= w || (across > w - 1.5 && crate::noise::unit(hash(j.blast, c.0, c.1, 0xC41)) < 0.5) {
                    continue;
                }
                work += 1;
                if j.broken.contains_key(&c) || self.loose.contains(&c) {
                    continue;
                }
                let m = self.world.material(c.0, c.1);
                work += WORK_LOOK;
                if m == VACUUM || self.units.is_flora.get(m as usize).copied().unwrap_or(false) || self.world.planet.mats.get(m).state != State::Solid {
                    continue;
                }
                self.loose.insert(c);
                self.settling.insert(c);
                self.world.set_look(c.0, c.1, Look::RUBBLE);
                work += WORK_CHANGE;
            }
            j.chimney_up = h1;
            *budget = budget.saturating_sub(work);
        }
        j.cells = cells;
    }

    /// A blast's held pieces (hard rock held together in big chunks) left in
    /// the ground once its crater is out (those in the crater's ground went
    /// with it, see `crater_chunks`), thrown out whole as tumbling rock
    /// bodies, on screen: once all its rock is broken and loosened and its
    /// crater all lifted out, so that the open space they can fly into is
    /// there (the crater, the cavity, the sky). Highest first (nearest the
    /// crater and the sky), each that can go (see `blast_chunk`), up to
    /// CHUNKS_MAX with the crater's; the rest stay held where they are.
    /// While `budget` lasts; the rest goes on next tick. A save
    /// (`finish_blasts`) throws none: it loses rock bodies in the air, so
    /// they stay held.
    fn blast_chunks(&mut self, j: &mut BlastJob, budget: &mut u64) {
        loop {
            match std::mem::replace(&mut j.chunks, Chunks::Done) {
                Chunks::Done => return,
                Chunks::Wait => {
                    if j.front.is_infinite() {
                        break;
                    }
                    let loosened = j.height <= 0.0 || j.shell as f64 * SHELL_M > j.column_end;
                    // (The crater's last layer goes on its tick only after
                    // this, in `blast_tick`: so not before the tick after.)
                    if !j.live.is_empty() || !j.crater_done || !loosened || self.tick <= j.crater_open {
                        j.chunks = Chunks::Wait;
                        return;
                    }
                    if j.held.is_empty() {
                        break;
                    }
                    let (at, up) = (j.at, j.up);
                    let over = |c: &(i64, i64)| {
                        let p = mid(*c);
                        (p.0 - at.0) * up.0 + (p.1 - at.1) * up.1
                    };
                    let mut order: Vec<(f64, usize)> = j.held.iter().enumerate().map(|(i, cells)| (cells.iter().map(over).fold(f64::MIN, f64::max), i)).collect();
                    order.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
                    *budget = budget.saturating_sub(j.held.iter().map(|c| c.len() as u64).sum::<u64>() + 2 * order.len() as u64);
                    j.chunks = Chunks::Next(order.into_iter().map(|o| o.1).collect(), 0);
                }
                Chunks::Next(order, k) => {
                    if j.front.is_infinite() || k >= order.len() || j.flung >= CHUNKS_MAX {
                        break;
                    }
                    if *budget == 0 {
                        j.chunks = Chunks::Next(order, k);
                        return;
                    }
                    let cells = std::mem::take(&mut j.held[order[k]]);
                    let work = self.blast_chunk(j, &cells);
                    *budget = budget.saturating_sub(work);
                    j.chunks = Chunks::Next(order, k + 1);
                }
            }
        }
        self.last_chunks = (j.held.len(), j.flung, j.flung_cells);
        j.held = Vec::new();
        j.held_ix = HashMap::new();
    }

    /// One held piece of a blast (`cells`, those it held in it) thrown out
    /// whole as a tumbling rock body, if it can go: of its cells still held
    /// (the crater may have taken some), the biggest joined-up part (far out,
    /// the gaps between the rays can split a piece), if that is big enough to
    /// be a body at all (`island_min_cells`) and small enough to tumble
    /// (`island_max_cells`), lies open to empty space and can move into it
    /// as far as it flies in its first moments (see CHUNK_CLEAR). It flies
    /// out from the charge, turned
    /// towards its open side (see `ways` below), at CHUNK_SHARE of the speed
    /// the blast throws its crater at, slower the bigger it is. It is lifted
    /// out of the ground as any chunk that breaks free is, so it is booked
    /// as in the air until it lands or leaves the body. Returns the work
    /// done.
    fn blast_chunk(&mut self, j: &mut BlastJob, cells: &[(i64, i64)]) -> u64 {
        let mut work = cells.len() as u64;
        let mut held = CellSet::default();
        for &c in cells {
            if self.world.look(c.0, c.1) != Look::HELD || self.loose.contains(&c) {
                continue;
            }
            let m = self.world.material(c.0, c.1);
            work += WORK_LOOK;
            if m != VACUUM && self.world.planet.mats.get(m).state == State::Solid {
                held.insert(c);
            }
        }
        let least = self.world.planet.mats.physics.island_min_cells.max(1);
        if held.len() < least {
            return work;
        }
        let mut part: Vec<(i64, i64)> = Vec::new();
        let mut seen = CellSet::default();
        for &c in cells {
            if !held.contains(&c) || !seen.insert(c) {
                continue;
            }
            let mut here = vec![c];
            let mut k = 0;
            while k < here.len() {
                let q = here[k];
                k += 1;
                for d in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let n = (q.0 + d.0, q.1 + d.1);
                    if held.contains(&n) && seen.insert(n) {
                        here.push(n);
                    }
                }
            }
            work += 4 * here.len() as u64;
            if here.len() > part.len() {
                part = here;
            }
        }
        // (Nor bigger than a tumbling body may be: see `Bodies::throw_chunk`.)
        if part.len() < least || part.len() > self.world.planet.mats.physics.island_max_cells.max(least) {
            return work;
        }
        part.sort_unstable();
        let mine: CellSet = part.iter().copied().collect();
        // The sides of it facing empty space, and its middle.
        let (mut open, mut sum) = ((0.0, 0.0), (0.0, 0.0));
        for &c in &part {
            let p = mid(c);
            sum = (sum.0 + p.0, sum.1 + p.1);
            for d in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let n = (c.0 + d.0, c.1 + d.1);
                work += 1;
                if mine.contains(&n) {
                    continue;
                }
                work += WORK_LOOK;
                if self.world.material(n.0, n.1) == VACUUM {
                    open = (open.0 + d.0 as f64, open.1 + d.1 as f64);
                }
            }
        }
        if open == (0.0, 0.0) {
            return work;
        }
        let n = part.len() as f64;
        let unit = |v: (f64, f64)| {
            let l = v.0.hypot(v.1);
            (l > 1e-9).then(|| (v.0 / l, v.1 / l))
        };
        let out = unit((sum.0 / n - j.at.0, sum.1 / n - j.at.1)).unwrap_or(j.up);
        let open = unit(open).unwrap_or(out);
        // Out from the charge turned towards the open side; or else to that
        // side, turned up to 60 degrees either way; or else straight up:
        // whichever it can move off along first, but never back towards the
        // charge. (A piece standing out of the crater's sloping wall is open
        // on its side, but slid straight that way it runs into the slope
        // under it.)
        let turn = |a: f64| (open.0 * a.cos() - open.1 * a.sin(), open.0 * a.sin() + open.1 * a.cos());
        let ways = [unit((out.0 + 2.0 * open.0, out.1 + 2.0 * open.1)).unwrap_or(open), open, turn(0.5), turn(-0.5), turn(1.0), turn(-1.0), j.up];
        // How fast it flies (as fast as the crater's middle flies, times the
        // share), and so how far it must be able to go clear: as far as it
        // flies in its first moments (CHUNK_LOOK_S), and at least its own
        // size. (Only CHUNK_CLEAR, a piece beside a narrow shaft or tunnel
        // went across it into the far wall.)
        let u = |k: u64| crate::noise::unit(hash(j.blast, part[0].0, part[0].1, 0xC4A0 + k));
        let fast = 35.0 * (0.8 + 0.35 * j.yield_kt.max(0.0).cbrt()).min(6.0);
        let v = CHUNK_SHARE * fast * (0.7 + 0.6 * u(0)) * (60.0 / n).sqrt().clamp(0.6, 1.2);
        let size = {
            let (x0, x1) = part.iter().fold((i64::MAX, i64::MIN), |a, c| (a.0.min(c.0), a.1.max(c.0)));
            let (y0, y1) = part.iter().fold((i64::MAX, i64::MIN), |a, c| (a.0.min(c.1), a.1.max(c.1)));
            (x1 - x0).max(y1 - y0) + 1
        };
        let clear = CHUNK_CLEAR.max(size).max((v * CHUNK_LOOK_S).ceil() as i64);
        let mut way = None;
        'ways: for w in ways {
            if w.0 * out.0 + w.1 * out.1 < 0.0 {
                continue;
            }
            for k in 1..=clear {
                let s = ((w.0 * k as f64).round() as i64, (w.1 * k as f64).round() as i64);
                for &c in &part {
                    let q = (c.0 + s.0, c.1 + s.1);
                    work += 1;
                    if mine.contains(&q) {
                        continue;
                    }
                    work += WORK_LOOK;
                    if self.world.material(q.0, q.1) != VACUUM {
                        continue 'ways;
                    }
                }
            }
            way = Some(w);
            break;
        }
        let Some(way) = way else { return work };
        let spin = (u(1) - 0.5) * 1.2;
        // (Lifting the first cell of a leaf nothing has changed yet makes it.)
        let mut fresh: Vec<(i64, i64)> = part.iter().filter(|c| self.world.dense_leaf(c.0, c.1).is_none()).map(|c| (c.0.div_euclid(crate::totals::LEAF), c.1.div_euclid(crate::totals::LEAF))).collect();
        fresh.sort_unstable();
        fresh.dedup();
        let lifted = self.bodies.throw_chunk(&mut self.world, &part, (way.0 * v, way.1 * v), spin);
        if lifted.is_empty() {
            return work;
        }
        for &c in &lifted {
            self.fractured.remove(&c);
            self.progress.remove(&c);
            self.reserved.remove(&c);
            self.open_cell(c);
        }
        j.flung += 1;
        j.flung_cells += lifted.len();
        work + 40 * lifted.len() as u64 + FRESH_LEAF * fresh.len() as u64
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
        let pull = {
            let p = crate::bodies::pull(&self.world.planet, a0.cos() * g0, a0.sin() * g0);
            (p.0 * p.0 + p.1 * p.1).sqrt()
        };
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
            pull,
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
            spread: 0.6,
            side: 1.0,
            pieces: Vec::new(),
            open: Default::default(),
            cells: CellSet::default(),
            stage: CraterStage::Ground,
            held: Default::default(),
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
                        // (The top layers kept: picked out in one pass, not
                        // sorted, as a big crater has a few hundred thousand.)
                        if cr.take.len() > CRATER_MAX {
                            cr.take.select_nth_unstable_by(CRATER_MAX, |a, b| a.2.total_cmp(&b.2));
                            cr.take.truncate(CRATER_MAX);
                        }
                        // The bigger the crater, the longer it takes to throw
                        // out, and the bigger the clumps it throws.
                        let n = cr.take.len();
                        cr.spread = (n as f64 / CRATER_LIFT_PER_S).max(0.6);
                        let mut side = 1;
                        while side < CLUMP_SIDE && n > CRATER_PIECES * side * side {
                            side += 1;
                        }
                        cr.side = side as f64;
                        cr.top = f64::MIN;
                        cr.span = f64::MAX;
                        cr.seen = CellSet::default();
                        cr.i = 0;
                        cr.stage = CraterStage::Tops;
                        *budget = budget.saturating_sub(n as u64);
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
                CraterStage::Tops => {
                    if cr.i >= cr.take.len() {
                        cr.span = (cr.top - cr.span).max(1.0);
                        cr.i = 0;
                        cr.stage = CraterStage::Throw;
                        continue;
                    }
                    // Highest first, by height (not depth under the ground
                    // just over it, which differs column to column: a piece
                    // flew into the cell beside it, as high but due later,
                    // and fell back in). (`span` holds the lowest until all
                    // are gone through.)
                    let end = cr.i.saturating_add((*budget / 2).max(1) as usize).min(cr.take.len());
                    for t in &cr.take[cr.i..end] {
                        let h = len(mid(t.0));
                        cr.top = cr.top.max(h);
                        cr.span = cr.span.min(h);
                        // The highest of each column (by its place across).
                        let e = cr.col_top.entry((t.1 / 0.7).round() as i64).or_insert(f64::MIN);
                        *e = e.max(h);
                    }
                    *budget = budget.saturating_sub(2 * (end - cr.i) as u64);
                    cr.i = end;
                }
                CraterStage::Throw => {
                    if cr.i >= cr.take.len() {
                        // The held pieces to throw out whole taken out of
                        // their clumps first.
                        let whole = self.crater_chunks(cr, budget);
                        // In the order they go (a stable sort: as they were
                        // found, among those due together); a clump flies
                        // from the middle of its cells, at their mean
                        // velocity, as far along as they are on average.
                        for p in cr.pieces.iter_mut() {
                            let n = p.cells.len() as f64;
                            p.from = (p.from.0 / n, p.from.1 / n);
                            p.vel = (p.vel.0 / n, p.vel.1 / n);
                            p.lead /= n;
                        }
                        cr.pieces.extend(whole);
                        cr.pieces.sort_by_key(|p| p.due);
                        cr.take = Vec::new();
                        cr.col_top = HashMap::new();
                        cr.open = Default::default();
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
                    let th = (th + (u(1) - 0.5) * 12.0).clamp(wall + 6.0, 89.0);
                    // That is measured from the slope; over the true level,
                    // the ground it flies over rises at `rise` (uphill, the
                    // slope's own angle; downhill, less than level). Kept
                    // under 80 degrees over the level (steeper on the uphill
                    // side of a hillside, it went up nearly straight and came
                    // down in the crater), and under 88 even where its wall
                    // is steeper than that.
                    let rise = slope.atan().to_degrees() * sgn;
                    let th = th.min((80.0 - rise).max(wall + 6.0)).min(88.0 - rise).to_radians();
                    let (rise, lift) = (rise.to_radians(), th + rise.to_radians());
                    // Fast in the middle, slower towards the rim
                    // (EJECTA_RIM_SHARE), the deep ground slower again.
                    let w = if x < 0.0 { wl } else { wr };
                    let fast = 35.0 * cr.pw;
                    let fall = fast * (EJECTA_RIM_SHARE + (1.0 - EJECTA_RIM_SHARE) * (1.0 - f).powf(1.5)) * (1.0 - 0.45 * q) * (0.85 + 0.3 * u(2));
                    // But never too slow to get past the rim over the ground
                    // as it really lies (the rule above is for level ground:
                    // on the uphill side of a hillside the slow ground came
                    // down short, back in the crater). Over ground rising at
                    // `rise`, thrown at `lift` over the level, it comes down
                    // 2 v^2 cos(lift) sin(lift - rise) / (g cos(rise))
                    // across (measured level, as the crater's own across
                    // is); it should get 10% past the rim, and a metre.
                    let reach = (w - x.abs()).max(0.0) * 1.1 + 1.0;
                    let need = (cr.pull * reach * rise.cos() / (2.0 * lift.cos().max(0.02) * th.sin().max(0.02))).sqrt();
                    let v = fall.max(1.1 * need).min(fast);
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
                    let after = self.ticks(0.08 + cr.spread * ((cr.top - len(p)) / cr.span).clamp(0.0, 1.0));
                    // It flies from the crater's mouth, over the top of its
                    // column (a piece flown from down in the hole could snag on
                    // a bit left standing or a pocket something fell into, and
                    // drop back in; flown all from one height, pieces caught
                    // on each other).
                    let up_by = cr.col_top.get(&((x / 0.7).round() as i64)).copied().unwrap_or(cr.top) - len(p) + 1.5;
                    let from = (p.0 + nrm.0 * up_by, p.1 + nrm.1 * up_by);
                    // And it starts as far along its way as it is deep under
                    // its column's top (the time that takes), so a column
                    // flies off as a stream as long as it was deep; at most
                    // half way to the top of its path.
                    let rise_v = (vel.0 * ux + vel.1 * uy).max(0.0);
                    let lead = ((up_by - 1.5).max(0.0) / v.max(1e-3)).min(0.5 * rise_v / cr.pull.max(1e-6));
                    cr.cells.insert(c);
                    // Its leaf of the world generated now, a budget's worth
                    // at a time, not by the first of it lifted (dozens of
                    // leaves at once for a big crater).
                    let fresh = if self.world.generate_leaf(c.0, c.1) { FRESH_LEAF } else { 0 };
                    *budget = budget.saturating_sub(3 + WORK_LOOK + fresh);
                    // A small crater throws its ground cell by cell; a big
                    // one in clumps, a square of it at a time (those of its
                    // cells that go in the same tick, up to CLUMP_MAX).
                    let open = if cr.side > 1.0 {
                        let key = (after, (x / cr.side).floor() as i64, ((cr.top - len(p)) / cr.side).floor() as i64);
                        match cr.open.get(&key) {
                            Some(&i) if cr.pieces[i].cells.len() < crate::bodies::CLUMP_MAX => Some(i),
                            _ => {
                                cr.open.insert(key, cr.pieces.len());
                                None
                            }
                        }
                    } else {
                        None
                    };
                    let i = match open {
                        Some(i) => {
                            let pc = &mut cr.pieces[i];
                            pc.cells.push(c);
                            pc.from = (pc.from.0 + from.0, pc.from.1 + from.1);
                            pc.vel = (pc.vel.0 + vel.0, pc.vel.1 + vel.1);
                            pc.lead += lead;
                            i
                        }
                        None => {
                            cr.pieces.push(Ejecta { due: after, cells: vec![c], from, vel, lead, whole: false, each: Vec::new() });
                            cr.pieces.len() - 1
                        }
                    };
                    // Hard rock the loosened column holds in big pieces
                    // (the rays never reach this far up, so only the
                    // column's rule can hold it): also kept by piece, with
                    // the clump it is in, as some pieces are taken out of
                    // their clumps to be thrown out whole (`crater_chunks`).
                    let m = self.world.material(c.0, c.1);
                    let rock = m != VACUUM && !flora(self, m) && !self.loose.contains(&c) && self.world.planet.mats.get(m).state == State::Solid;
                    if let Some(key) = rock.then(|| column_piece(c, blast_hardness(self, m), cr.blast)).flatten() {
                        cr.held.entry(key).or_default().push((i, Ejecta { due: after, cells: vec![c], from, vel, lead, whole: false, each: Vec::new() }));
                    }
                }
                CraterStage::Ready => return,
            }
        }
    }

    /// A worked-out crater's held pieces (see `column_piece`): each with
    /// enough cells joined up in the crater to make a rock body
    /// (`island_min_cells`) and no more than a tumbling one may have
    /// (`island_max_cells`), the first CHUNKS_MAX to go, is taken out of the
    /// clumps its cells were in, to be thrown out whole when its lowest
    /// layer goes. By then the ground beside it and over its foot has gone,
    /// so it moves off only away from what is left. (Thrown when its top
    /// layer went, it flew out sideways into the ground still standing
    /// beside its lower part.) It flies from where it lies, the way the
    /// ground there flies but at CHUNK_SHARE of the speed the crater's
    /// middle flies at, slower the bigger it is. A piece cut in two (by the
    /// sapper's shaft up the crater's middle, or the crater's frayed bottom
    /// edge) goes as two, each if it is big enough; the rest of it flies in
    /// its clumps with the crater's other ground. Called before the clumps'
    /// sums are made means; returns the pieces thrown whole.
    fn crater_chunks(&self, cr: &mut Crater, budget: &mut u64) -> Vec<Ejecta> {
        let least = self.world.planet.mats.physics.island_min_cells.max(1);
        // (No bigger than a tumbling body may be: see `Bodies::throw_chunk`.)
        let most = self.world.planet.mats.physics.island_max_cells.max(least);
        let mut work = 0u64;
        // Each piece's joined-up parts big enough to go: (when its top goes,
        // its cells with the clump each is in).
        let mut parts: Vec<(u64, Vec<(usize, Ejecta)>)> = Vec::new();
        for list in std::mem::take(&mut cr.held).into_values() {
            work += 8 * list.len() as u64;
            let at: HashMap<(i64, i64), usize> = list.iter().enumerate().map(|(k, e)| (e.1.cells[0], k)).collect();
            let mut list: Vec<Option<(usize, Ejecta)>> = list.into_iter().map(Some).collect();
            let mut done = vec![false; list.len()];
            for s in 0..list.len() {
                if done[s] {
                    continue;
                }
                done[s] = true;
                let mut here = vec![s];
                let mut k = 0;
                while k < here.len() {
                    let c = list[here[k]].as_ref().map_or((0, 0), |e| e.1.cells[0]);
                    k += 1;
                    for d in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        if let Some(&n) = at.get(&(c.0 + d.0, c.1 + d.1)) {
                            if !done[n] {
                                done[n] = true;
                                here.push(n);
                            }
                        }
                    }
                }
                if here.len() >= least && here.len() <= most {
                    let part: Vec<(usize, Ejecta)> = here.iter().filter_map(|&k| list[k].take()).collect();
                    parts.push((part.iter().map(|e| e.1.due).min().unwrap_or(0), part));
                }
            }
        }
        // (Stable: in the order of their keys, among those due together.)
        parts.sort_by_key(|p| p.0);
        let mut whole = Vec::new();
        for (_, part) in parts.into_iter().take(CHUNKS_MAX) {
            let n = part.len() as f64;
            let (mut at, mut sum, mut due) = ((0.0, 0.0), (0.0, 0.0), 0);
            for (i, e) in &part {
                let c = e.cells[0];
                // Out of its clump (left empty, the clump is dropped below).
                let pc = &mut cr.pieces[*i];
                pc.cells.retain(|&q| q != c);
                pc.from = (pc.from.0 - e.from.0, pc.from.1 - e.from.1);
                pc.vel = (pc.vel.0 - e.vel.0, pc.vel.1 - e.vel.1);
                pc.lead -= e.lead;
                let p = mid(c);
                at = (at.0 + p.0 / n, at.1 + p.1 / n);
                sum = (sum.0 + e.vel.0, sum.1 + e.vel.1);
                due = due.max(e.due);
            }
            work += 40 * part.len() as u64;
            let mut cells: Vec<(i64, i64)> = part.iter().map(|e| e.1.cells[0]).collect();
            cells.sort_unstable();
            let c = cells[0];
            let u = |k: u64| crate::noise::unit(hash(cr.blast, c.0, c.1, 0xC4A0 + k));
            let v = CHUNK_SHARE * 35.0 * cr.pw * (0.7 + 0.6 * u(0)) * (60.0 / n).sqrt().clamp(0.6, 1.2);
            let l = sum.0.hypot(sum.1).max(1e-9);
            // (Each cell as it would have flown, kept should the piece not
            // be able to go whole when its time comes: see `blast_tick`.)
            let each = part.into_iter().map(|e| e.1).collect();
            whole.push(Ejecta { due, cells, from: at, vel: (sum.0 / l * v, sum.1 / l * v), lead: (u(1) - 0.5) * 1.2, whole: true, each });
        }
        cr.pieces.retain(|p| !p.cells.is_empty());
        *budget = budget.saturating_sub(work + cr.pieces.len() as u64 / WORK_PASS);
        whole
    }

    /// A worked-out crater is thrown: its cells queued to be lifted and
    /// thrown from now on, a layer at a time (see `blast_tick`). Returns the
    /// cells.
    pub(super) fn crater_throw(&mut self, cr: Crater) -> CellSet {
        let now = self.tick;
        self.last_crater = (cr.cells.len(), cr.dmax);
        let later = self.eject.back().is_some_and(|e| e.due > now + cr.pieces.first().map_or(0, |p| p.due));
        self.eject.extend(cr.pieces.into_iter().map(|p| Ejecta { due: now + p.due, ..p }));
        // (Only when another crater's ground is still due later than some of
        // this one's does the queue need putting back in order.)
        if later {
            self.eject.make_contiguous().sort_by_key(|e| e.due);
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
    /// the time (`fracture_dig`); the blocks' edges show as cracks. (Further
    /// out still, the thin far zone: `blast_far_cracks`.) Returns the work
    /// done.
    fn blast_cracks(&mut self, j: &mut BlastJob, lo: f64, hi: f64) -> u64 {
        if hi <= 0.0 {
            return 0;
        }
        let (at, r, lt, height, up, side, ground_h) = (j.at, j.r, j.lt, j.height, j.up, j.side, j.ground_h);
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
        // (Nor past the band's far corners: the front runs on that far for
        // the far zone's cracks, and walking the rows of its empty shells
        // here cost a big blast's last tick some 8 ms.)
        if past_rays && (height <= 0.0 || lo > (height + 2.0).hypot(r * (lt + 1.0))) {
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
            // (Not over the ground over it: with no column too, or a blast
            // with none cracked only the rock under its charge's level.)
            let h = dx * up.0 + dy * up.1;
            if h > ground_h + 1.0 || (dx * side.0 + dy * side.1).abs() > band(h) {
                continue;
            }
            work += 1;
            let (t, _) = j.crack_t(p);
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

    /// The far cracked zone (CRACK_FAR), out to `hi` m from the charge:
    /// past the rock cracked all through into blocks, the shock cracks it
    /// more and more thinly. Decided a block at a time, not a cell at a
    /// time (out there a cell-by-cell walk would look at ten times as many
    /// cells as it cracks): each block whose turn has come cracks or not,
    /// and only along the edges of those that do are cells looked at.
    /// Returns the work done.
    fn blast_far_cracks(&mut self, j: &mut BlastJob, hi: f64) -> u64 {
        let mut work = 0;
        // On the first shell, every block that might crack is listed, in
        // the order the front reaches it.
        let mut q = match j.far.take() {
            Some(q) => q,
            None => {
                let mut q = Vec::new();
                for b in 0..3 {
                    let size = j.far_blocks[b].size;
                    let (lo, top) = j.far_range[b];
                    let (i0, i1) = (((j.at.0 - top) / size).floor() as i64, ((j.at.0 + top) / size).floor() as i64);
                    let (k0, k1) = (((j.at.1 - top) / size).floor() as i64, ((j.at.1 + top) / size).floor() as i64);
                    for i in i0..=i1 {
                        for k in k0..=k1 {
                            let [px, py, _] = j.far_blocks[b].site(i, k);
                            let d = (px - j.at.0).hypot(py - j.at.1);
                            if d >= lo && d <= top {
                                q.push(FarBlock { due: d + 2.9 * size + 3.0, d, b: b as u8, i, k });
                            }
                        }
                    }
                    work += ((i1 - i0 + 1) * (k1 - k0 + 1)) as u64;
                }
                q.sort_by(|x, y| y.due.total_cmp(&x.due).then((y.b, y.i, y.k).cmp(&(x.b, x.i, x.k))));
                work += q.len() as u64;
                q
            }
        };
        // Once all the rays are done, the blocks too far out to crack are
        // dropped, so the front need not run on to where the farthest a ray
        // could have reached would have put them.
        if !j.far_pruned {
            if j.ray_end.is_some() {
                // (The far zone reaches no further out than in even rock.)
                let out = j.stop.iter().fold(0.0f64, |a, &b| a.max(b)) + j.r * (CRACK_REACH + CRACK_FAR);
                let sizes = [0, 1, 2].map(|b| j.far_blocks[b].size);
                work += q.len() as u64;
                q.retain(|f| f.d <= out + 1.42 * sizes[f.b as usize]);
                j.far_pruned = true;
            }
        }
        while q.last().is_some_and(|f| f.due <= hi) {
            let f = q.pop().unwrap();
            work += self.far_block(j, f);
        }
        j.far = Some(q);
        work
    }

    /// Whether block (i, k) of far pattern `b` cracks (see `far_judged`),
    /// kept the first time it is asked, so it is the same whenever it is
    /// asked again (what is at its site may change in between: the crater
    /// lifted out, or a cutter digging). Returns that and the work done.
    fn far_cracked(&self, j: &mut BlastJob, b: usize, i: i64, k: i64) -> (bool, u64) {
        if let Some(&yes) = j.far_seen.get(&(b as u8, i, k)) {
            return (yes, 1);
        }
        let (yes, work) = self.far_judged(j, b, i, k);
        j.far_seen.insert((b as u8, i, k), yes);
        (yes, work + 1)
    }

    /// Whether block (i, k) of far pattern `b` cracks: more often nearer the
    /// inner edge of the far zone and in harder rock (judged by what is at
    /// its site; in the air or in water, as middling rock), never outside
    /// its pattern's third of the zone (and the reach of its blocks past
    /// it). Returns that and the work done.
    fn far_judged(&self, j: &mut BlastJob, b: usize, i: i64, k: i64) -> (bool, u64) {
        let [px, py, roll] = j.far_blocks[b].site(i, k);
        let (lo, top) = j.far_range[b];
        let d = (px - j.at.0).hypot(py - j.at.1);
        if d < lo || d > top {
            return (false, 1);
        }
        let u = j.far_u((px, py));
        let spill = 1.42 * j.far_blocks[b].size / (j.r * FAR_W);
        if u < b as f64 / 3.0 - spill || u > (b + 1) as f64 / 3.0 + spill {
            return (false, 1);
        }
        // Coming in over the near zone's end (FAR_IN), then fewer further
        // out, slowly at first (as their edges crack less often too, and
        // their blocks grow); more of them for a bigger blast's bigger
        // blocks (as many times as they are bigger than at 10 kt), up to
        // three in four.
        let grow = j.far_blocks[0].size / FAR_SIZES[0];
        let u = u.clamp(0.0, 1.0);
        let p = (FAR_SHARE * grow).min(0.75) * (1.0 - u).sqrt() * (u / FAR_IN).min(1.0);
        // (Not cracked even in the hardest rock: no need to ask the world.)
        if roll >= p * (0.45 + 0.75 * 0.95) {
            return (false, 1);
        }
        let c = cell_of((px, py));
        let m = self.world.material(c.0, c.1);
        let hd = match (m != VACUUM).then(|| self.world.planet.mats.get(m)) {
            Some(mat) if mat.state == State::Solid => (mat.hardness as f64 / 10.0).clamp(0.05, 0.95),
            _ => 0.5,
        };
        (roll < p * (0.45 + 0.75 * hd), 1 + WORK_LOOK)
    }

    /// A block of the far cracked zone whose turn has come: if it cracks,
    /// its outline is found (a square round its site cut down by the line
    /// halfway to each neighbour's site), and some of its edges crack. An
    /// edge between two blocks that both crack is dealt with by the first of
    /// them in grid order only. Returns the work done.
    fn far_block(&mut self, j: &mut BlastJob, f: FarBlock) -> u64 {
        let (b, me) = (f.b as usize, (f.i, f.k));
        let (cracks, mut work) = self.far_cracked(j, b, f.i, f.k);
        if !cracks {
            return work;
        }
        let [px, py, _] = j.far_blocks[b].site(f.i, f.k);
        let w = 3.0 * j.far_blocks[b].size;
        let mut outline: Outline = vec![((px - w, py - w), None), ((px + w, py - w), None), ((px + w, py + w), None), ((px - w, py + w), None)];
        // (Its neighbours all lie within two squares of its own: there is a
        // site in every square, and a block reaches no further than 1.42
        // sides from its own site.)
        for di in -2..=2 {
            for dk in -2..=2 {
                if (di, dk) == (0, 0) {
                    continue;
                }
                let n = (f.i + di, f.k + dk);
                let [qx, qy, _] = j.far_blocks[b].site(n.0, n.1);
                clip_outline(&mut outline, ((px + qx) * 0.5, (py + qy) * 0.5), (qx - px, qy - py), n);
                work += 2;
            }
        }
        for e in 0..outline.len() {
            let (a, Some(n)) = outline[e] else { continue };
            let z = outline[(e + 1) % outline.len()].0;
            if (z.0 - a.0).hypot(z.1 - a.1) < 0.5 {
                continue;
            }
            if n < me {
                let (theirs, w) = self.far_cracked(j, b, n.0, n.1);
                work += w;
                if theirs {
                    continue;
                }
            }
            // Fewer of the edges further out.
            let (s0, s1) = if me < n { (me, n) } else { (n, me) };
            let ([ax, ay, _], [bx, by, _]) = (j.far_blocks[b].site(s0.0, s0.1), j.far_blocks[b].site(s1.0, s1.1));
            let u = j.far_u(((ax + bx) * 0.5, (ay + by) * 0.5)).clamp(0.0, 1.0);
            let key = hash(hash(j.blast, s0.0, s0.1, 0xFA0 + b as u64), s1.0, s1.1, 74);
            if crate::noise::unit(key) >= FAR_EDGE - 0.5 * u {
                continue;
            }
            // Bent a little at its middle, across it, so the cracks don't
            // run dead straight from corner to corner.
            let l = (bx - ax).hypot(by - ay).max(1e-9);
            let bend = (crate::noise::unit(hash(key, 0, 0, 75)) - 0.5) * 0.3 * (z.0 - a.0).hypot(z.1 - a.1);
            let m = ((a.0 + z.0) * 0.5 + (bx - ax) / l * bend, (a.1 + z.1) * 0.5 + (by - ay) / l * bend);
            work += self.far_crack_line(j, b, a, m) + self.far_crack_line(j, b, m, z);
        }
        work
    }

    /// The cells along a crack of the far zone from `a` to `z`, cracked
    /// where they are in pattern `b`'s third of the zone (outside the rock
    /// cracked all through, or in its faded end, FAR_FROM on), under the
    /// ground, and solid rock not already broken, loosened or cracked.
    /// Returns the work done.
    fn far_crack_line(&mut self, j: &BlastJob, b: usize, a: (f64, f64), z: (f64, f64)) -> u64 {
        let (lo, hi) = (b as f64 / 3.0, (b + 1) as f64 / 3.0);
        // (Steps of 0.7 m: no cell of the line is skipped over.)
        let n = ((z.0 - a.0).hypot(z.1 - a.1) / 0.7).ceil().max(1.0) as i64;
        let (mut work, mut last) = (0, None);
        for s in 0..=n {
            let f = s as f64 / n as f64;
            let c = cell_of((a.0 + (z.0 - a.0) * f, a.1 + (z.1 - a.1) * f));
            if last == Some(c) {
                continue;
            }
            last = Some(c);
            work += 1;
            let p = mid(c);
            if (p.0 - j.at.0) * j.up.0 + (p.1 - j.at.1) * j.up.1 > j.ground_h + 1.0 {
                continue;
            }
            let (t, tr) = j.crack_t(p);
            let u = far_u_of(tr);
            if t < FAR_FROM || u < lo || u >= hi || j.broken.contains_key(&c) || self.loose.contains(&c) {
                continue;
            }
            let m = self.world.material(c.0, c.1);
            work += WORK_LOOK;
            if m == VACUUM || self.units.is_flora.get(m as usize).copied().unwrap_or(false) || self.world.planet.mats.get(m).state != State::Solid {
                continue;
            }
            // (Newly cracked only: what an earlier blast held or cracked
            // keeps its look.)
            if !self.fractured.insert(c) {
                continue;
            }
            self.units.cracked += 1;
            work += WORK_CHANGE;
            self.world.set_look(c.0, c.1, Look::CRACKED);
        }
        work
    }

    /// Whether a blast blows a plume, once its rubble is all made: most
    /// blasts on screen find a way up. (Not where the ground over it was
    /// thrown out as a crater: a jet out of one hole beside the crater's even
    /// fan looked lopsided; Benjamin, 26 Nov.)
    fn plume_wanted(j: &BlastJob) -> bool {
        j.seen && !j.cratered && crate::noise::unit(hash(j.blast, 0, 0, 77)) < 0.9 && !j.rubble.is_empty()
    }

    /// A blast's rubble gone through for what its plume could blow out, from
    /// where `way` has got to while `budget` lasts: a random share of the
    /// rubble still lying where it broke, the same every run. (By now some
    /// of it may have settled away or been taken, and melt or other rubble
    /// come into its place: only loose rubble is blown out, never liquid
    /// that flowed in.) Returns whether it is all gone through. (Done all at
    /// once, a big blast's few hundred thousand cells of rubble took 15 to
    /// 45 ms.)
    fn plume_rubble(&self, j: &BlastJob, way: &mut WayUp, budget: &mut u64) -> bool {
        while way.next < j.rubble.len() {
            if *budget == 0 {
                return false;
            }
            let end = (way.next as u64).saturating_add((*budget / (1 + WORK_LOOK)).max(1)).min(j.rubble.len() as u64) as usize;
            for &c in &j.rubble[way.next..end] {
                if self.loose.contains(&c) && self.world.look(c.0, c.1) != Look::MELT {
                    way.pick.push((hash(j.blast, c.0, c.1, 13), c));
                }
            }
            *budget = budget.saturating_sub((1 + WORK_LOOK) * (end - way.next) as u64);
            way.next = end;
        }
        true
    }

    /// The plume blown out of the way up `way` found, of the rubble it
    /// picked (see `plume_wanted`, `plume_rubble`): the share of it that
    /// goes chosen first, then lifted out while `budget` lasts (each cell
    /// lifted opens the ground round it, which took up to 20 ms for a big
    /// blast's 500 all at once), each queued to fly when its turn in the
    /// stream comes, counted from when the plume began. Returns whether it
    /// is all blown out.
    fn blast_plume(&mut self, j: &BlastJob, way: &mut WayUp, budget: &mut u64) -> bool {
        let blast = j.blast;
        let Some(mouth) = way.mouth else { return true };
        if way.take.is_none() {
            let pick = &mut way.pick;
            *budget = budget.saturating_sub(pick.len() as u64);
            let share = 0.35 + 0.25 * crate::noise::unit(hash(blast, 0, 0, 7));
            let take = ((pick.len() as f64 * share).round() as usize).min(pick.len()).min(PLUME_MAX);
            // (The lowest hashes found without sorting them all.)
            if take < pick.len() {
                pick.select_nth_unstable(take);
                pick.truncate(take);
            }
            pick.sort_unstable();
            way.take = Some(take);
            way.start = self.tick;
        }
        let (pick, take) = (&way.pick, way.take.unwrap_or(0));
        if take == 0 {
            return true;
        }
        let pw = (0.8 + j.yield_kt.max(0.0).cbrt() * 0.35).min(4.0);
        // How long it pours out (the proving ground's plume).
        let dur = 0.6 + 0.35 * (j.yield_kt * 10.0 + 1.0).log10();
        if way.launched == 0 && self.plumes.len() < 100 {
            let m = mid(mouth);
            self.plumes.push((m.0, m.1, pw as f32));
        }
        let mu = unit_of(mid(mouth));
        let side = (-mu.1, mu.0);
        while way.launched < pick.len() && *budget > 0 {
            let k = way.launched;
            let (h, c) = pick[k];
            way.launched += 1;
            *budget = budget.saturating_sub(WORK_PLUME);
            // (Picked over a few ticks: looked at again.)
            if !self.loose.contains(&c) || self.world.look(c.0, c.1) == Look::MELT {
                continue;
            }
            let Some(m) = self.world.lift_cell(c.0, c.1) else { continue };
            self.cell_mined(c);
            self.undope(c, false);
            self.progress.remove(&c);
            self.reserved.remove(&c);
            self.open_cell(c);
            let vx = (crate::noise::unit(hash(h, k as i64, 1, 0)) - 0.5) * 16.0 * pw;
            let vy = (50.0 + 60.0 * crate::noise::unit(hash(h, k as i64, 2, 0))) * pw;
            let from = mid(mouth);
            let from = (from.0 + side.0 * (crate::noise::unit(hash(h, k as i64, 3, 0)) - 0.5) * 2.0, from.1 + side.1 * (crate::noise::unit(hash(h, k as i64, 3, 0)) - 0.5) * 2.0);
            // Out over the plume's time, a steady stream.
            let at = way.start + self.ticks(dur * k as f64 / take.max(1) as f64);
            self.plume_out.push_back((at, from, m, (side.0 * vx + mu.0 * vy, side.1 * vx + mu.1 * vy)));
        }
        self.plume_out.make_contiguous().sort_by_key(|p| p.0);
        way.launched >= pick.len()
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

    /// Searching on for an open way up from a blast's charge to open sky
    /// over the ground (through open space and the cells in `loose`), within
    /// reach of the blast, while `budget` lasts (it was done all in one
    /// tick, up to 40,000 cells and their ground: 30 to 140 ms). Returns the
    /// cell where it comes out, or None for no way, once the search is
    /// over; or None, the search kept to go on next tick.
    fn way_up(&self, w: &mut WayUp, budget: &mut u64) -> Option<Option<(i64, i64)>> {
        let (from, limit, r0) = (w.from, w.limit, w.r0);
        while *budget > 0 {
            let Some(c) = w.queue.pop_front() else { return Some(None) };
            if w.seen.len() > 40_000 {
                return Some(None);
            }
            let mut work = 1;
            let p = mid(c);
            let a = p.1.atan2(p.0);
            let key = (a * r0).round() as i64;
            // The ground's height by direction, worked out as needed: a walk
            // down through the air, each cell of it worked out from the
            // generator (about half a millisecond from over the highest the
            // ground could be), so started just over the ground found a
            // step or so to the side, where there is one.
            let top = match w.tops.get(&key) {
                Some(&t) => t,
                None => {
                    let near = (1..=3).find_map(|k| w.tops.get(&(key - k)).or_else(|| w.tops.get(&(key + k))).copied());
                    let t = match near {
                        Some(near) => {
                            work += WORK_GROUND_NEAR;
                            self.ground_from(a, true, near)
                        }
                        None => {
                            work += WORK_GROUND;
                            self.ground_at(a, true)
                        }
                    };
                    w.tops.insert(key, t);
                    t
                }
            };
            if len(p) >= top {
                return Some(Some(c));
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
                if w.seen.contains(&n) {
                    continue;
                }
                let dx = (n.0 - from.0) as f64;
                let dy = (n.1 - from.1) as f64;
                if dx * dx + dy * dy > limit * limit {
                    continue;
                }
                work += WORK_LOOK;
                let m = self.world.material(n.0, n.1);
                if m != VACUUM && !self.loose.contains(&n) && !self.units.is_flora.get(m as usize).copied().unwrap_or(false) {
                    continue;
                }
                w.seen.insert(n);
                w.queue.push_back(n);
            }
            *budget = budget.saturating_sub(work);
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
