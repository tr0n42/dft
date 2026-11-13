//! Loose rigid bodies: chunks of rock cut free by digging, and wrecked drones.
//!
//! Solid rock that digging has cut off from everything else (an island of at
//! most `physics.island_max_cells` cells) is lifted out of the grid into a
//! rigid body made of 1 m voxels. It falls toward the centre, tumbles and
//! stacks against the ground (the Rapier physics library, in its
//! cross-platform deterministic mode), and once it has been still for
//! `physics.settle_s` it sets down into the grid again, cell by cell, as close
//! to where it came to rest as there is room.
//!
//! While a body is in the air its cells are booked as aloft in the ledger, so
//! the books balance at every moment. Wrecked drones become one-cell bodies of
//! drone scrap, booked as deposited (their matter came from the swarm).
//!
//! The ground a body collides with is built from the grid as voxel colliders,
//! one per 64 m leaf near a moving body, rebuilt every few ticks.
//!
//! The physics works in 32-bit floats, which are only fine-grained near their
//! origin (at Earth's radius one step is half a metre). So the body is split
//! into square frames `FRAME` metres wide, each with its own physics world
//! centred on it: a body is simulated in the frame it broke free in, never
//! more than a few kilometres from that frame's centre. Bodies smaller than
//! half a frame wide fit in the single frame at the centre (origin 0).

use crate::materials::{MatId, State, VACUUM};
use crate::swarm::{CellMap, CellSet};
use crate::totals::LEAF;
use crate::world::World;
use rapier2d::prelude::*;

pub type Cell = (i64, i64);

/// A big chunk cut free (more than `island_max_cells`): it moves as one
/// rigid piece, without tumbling, under the pull it feels where it is, until
/// it meets the ground (and sets down there) or drifts off for good.
/// A cell of rock (or scrap) in the air, for drawing: its middle in world
/// metres, its body's turn (radians), the cell it was cut from, its material.
#[derive(Clone, Copy, Debug)]
pub struct AirCell {
    pub x: f64,
    pub y: f64,
    pub angle: f32,
    pub home: Cell,
    pub m: MatId,
}

struct Slab {
    /// Each cell: offset from `origin`, and material.
    cells: Vec<((i32, i32), MatId)>,
    /// The cells on its outline (only these can meet anything).
    edge: Vec<(i32, i32)>,
    /// Where it was cut from, and its middle (offset from there).
    origin: Cell,
    middle: (f64, f64),
    /// How far it has moved since (metres), its velocity, and the whole
    /// cells it is displaced by now.
    pos: (f64, f64),
    vel: (f64, f64),
    at: (i64, i64),
    /// Its mean hardness, and whether that's soft enough to crumble.
    hardness: f64,
    soft: bool,
    /// Pieces owed to crumbling, building up as it moves.
    shed_acc: f64,
    /// Ground cells it may still knock loose in its life.
    gouge_left: usize,
}

impl Slab {
    fn rebuild_edge(&mut self) {
        let set: std::collections::HashSet<(i32, i32)> = self.cells.iter().map(|c| c.0).collect();
        self.edge = self.cells.iter().map(|c| c.0).filter(|&(i, j)| [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|d| !set.contains(&(i + d.0, j + d.1)))).collect();
        let n = self.cells.len().max(1) as f64;
        let (mx, my) = self.cells.iter().fold((0.0, 0.0), |a, c| (a.0 + c.0 .0 as f64 + 0.5, a.1 + c.0 .1 as f64 + 0.5));
        self.middle = (mx / n, my / n);
    }
}

/// Rock this soft or softer (mean hardness) crumbles as it moves and
/// shatters on hard knocks.
const SOFT: f64 = 4.0;
/// A knock at this speed or more (m/s) counts as a hard one.
const SMASH_M_S: f64 = 1.2;
/// How many cells one point of hardness holds up by a strip one cell wide
/// (beyond it, the strip is all but sure to snap).
const NECK_CELLS_PER_HARDNESS: f64 = 60.0;
/// A knock this hard or harder (m/s) can knock softer ground loose.
const GOUGE_M_S: f64 = 3.0;
/// Most small bodies in the air at once (pieces beyond it are set down).
const MAX_LOOSE: usize = 400;
/// Most cells waiting for the big-chunk search looked at in one step.
const BIG_SCAN: usize = 4096;

/// Moving rock crushes drones in its way when heavy enough for its speed:
/// a single cell must go fast, a big slab hardly at all.
fn crushes(speed: f64, cells: usize, crush_speed: f64) -> bool {
    speed > 0.25 && speed * (cells as f64).sqrt() > 2.0 * crush_speed
}

/// Mean hardness of some cells.
fn mean_hardness(w: &World, mats: impl Iterator<Item = MatId>) -> f64 {
    let (mut s, mut n) = (0.0, 0.0);
    for m in mats {
        s += w.planet.mats.get(m).hardness as f64;
        n += 1.0;
    }
    if n > 0.0 { s / n } else { 0.0 }
}

/// The pull of the body at (x, y), m/s², toward its centre: the real
/// gravity there (inside the body, from its make-up; outside, falling off
/// with the square of the distance). Big planets pull hard; small asteroids
/// hardly at all, so chunks there drift.
pub fn pull(p: &crate::planet::Planet, x: f64, y: f64) -> (f64, f64) {
    let r = (x * x + y * y).sqrt().max(1.0);
    let g = if r <= p.radius { p.real_gravity_at(x.floor() as i64, y.floor() as i64) } else { p.real_surface_gravity() * (p.radius / r).powi(2) };
    (-x / r * g, -y / r * g)
}

/// A drop of liquid in flight: what, where, how fast, and where it was
/// thrown from (it keeps that place's colour).
#[derive(Clone, Copy)]
struct Drop {
    m: MatId,
    p: (f64, f64),
    v: (f64, f64),
    from: Cell,
}

/// Most drops in flight at once (more are set straight down where they land).
const MAX_DROPS: usize = 20_000;

/// Where a cell thrown from `at` at `vel` comes down: up and down again at
/// the same height, carried along the ground by its sideways speed meanwhile
/// (a simple throw over level ground). Set down there; None if it couldn't be.
fn land_thrown(w: &mut World, at: (f64, f64), m: MatId, vel: (f64, f64)) -> Option<Cell> {
    let r0 = (at.0 * at.0 + at.1 * at.1).sqrt().max(1.0);
    let (ux, uy) = (at.0 / r0, at.1 / r0);
    let (tx, ty) = (-uy, ux);
    let g = {
        let p = pull(&w.planet, at.0, at.1);
        (p.0 * p.0 + p.1 * p.1).sqrt().max(1e-6)
    };
    let vr = (vel.0 * ux + vel.1 * uy).max(0.0);
    let vt = vel.0 * tx + vel.1 * ty;
    let flight = (2.0 * vr / g).min(3600.0);
    let along = (vt * flight).clamp(-0.5 * r0, 0.5 * r0);
    let a = along / r0;
    let (ca, sa) = (a.cos(), a.sin());
    let land = ((ux * ca - uy * sa) * r0, (ux * sa + uy * ca) * r0);
    let c = ground_below(w, land.0, land.1);
    match settle_near(w, c, m) {
        Some(t) => Some(t),
        None => {
            w.lose_aloft(m);
            None
        }
    }
}

/// Set a cell that is in the air down into the open cell nearest c.
fn settle_near(w: &mut World, c: Cell, m: MatId) -> Option<Cell> {
    for ring in 0..16i64 {
        for dy in -ring..=ring {
            for dx in -ring..=ring {
                if (dx.abs() == ring || dy.abs() == ring) && w.land_cell(c.0 + dx, c.1 + dy, m) {
                    return Some((c.0 + dx, c.1 + dy));
                }
            }
        }
    }
    None
}

/// A kick of up to `speed` m/s in a direction mostly away from the centre.
fn kick(c: Cell, speed: f64) -> (f64, f64) {
    let h = crate::noise::hash(0x5A1B, c.0, c.1, 1);
    let r = ((c.0 * c.0 + c.1 * c.1) as f64).sqrt().max(1.0);
    let up = (c.0 as f64 / r, c.1 as f64 / r);
    let turn = (crate::noise::unit(h) - 0.5) * 2.0;
    let (s, co) = turn.sin_cos();
    let v = speed * (0.5 + 0.5 * crate::noise::unit(crate::noise::hash(h, 2, 0, 0)));
    ((up.0 * co - up.1 * s) * v, (up.0 * s + up.1 * co) * v)
}

/// The shove a chunk gets the moment it's cut free: half are thrown out
/// toward space, half pushed sideways and set spinning so they roll. Returns
/// velocity (m/s) and spin (radians a second).
fn orphan_push(c: Cell, speed: f64, cells: usize) -> ((f64, f64), f64) {
    let h = crate::noise::hash(0x0FA5, c.0, c.1, 2);
    let r = ((c.0 * c.0 + c.1 * c.1) as f64).sqrt().max(1.0);
    let up = (c.0 as f64 / r, c.1 as f64 / r);
    let size = (cells as f64).sqrt().max(1.0);
    if crate::noise::unit(h) < 0.5 {
        // Thrown out: up to 2.5 times the usual kick, nearly straight up.
        let v = speed * (1.0 + 1.5 * crate::noise::unit(crate::noise::hash(h, 1, 0, 0)));
        let turn = (crate::noise::unit(crate::noise::hash(h, 2, 0, 0)) - 0.5) * 0.8;
        let (s, co) = turn.sin_cos();
        (((up.0 * co - up.1 * s) * v, (up.0 * s + up.1 * co) * v), 0.0)
    } else {
        // Rolling: along the ground either way, a little lifted.
        let side = if crate::noise::unit(crate::noise::hash(h, 3, 0, 0)) < 0.5 { 1.0 } else { -1.0 };
        let v = speed * (0.8 + 0.8 * crate::noise::unit(crate::noise::hash(h, 4, 0, 0)));
        let (tx, ty) = (-up.1 * side, up.0 * side);
        ((tx * v + up.0 * v * 0.2, ty * v + up.1 * v * 0.2), -side * v / (0.5 * size))
    }
}

/// A loose body and the cells it is made of.
struct Loose {
    /// The frame it moves in.
    frame: Cell,
    /// The cell its local (0, 0) was cut from (each cell keeps the texture
    /// of the place it came from while it flies).
    origin: Cell,
    handle: RigidBodyHandle,
    /// Local voxel offset and material of each cell.
    cells: Vec<((i32, i32), MatId)>,
    /// Seconds it has been nearly still.
    still_s: f64,
    /// Wrecked drone (not rock).
    wreck: bool,
    /// Soft enough to shatter on a hard knock.
    soft: bool,
    /// Its speed last tick (a sudden drop is a knock).
    last_speed: f64,
    /// Ground cells it may still knock loose in its life.
    gouge_left: usize,
    /// Ticks since it was launched (its first moments aren't knocks).
    age: u32,
}

/// What happened to bodies during a tick.
#[derive(Debug, Default)]
pub struct Events {
    /// Cells a body was set down into (the grid gained matter there).
    pub landed: Vec<(Cell, MatId)>,
    /// Cells occupied by fast-moving rock this tick: a drone working there is crushed.
    pub crushing: CellSet,
    /// Ground cells knocked loose by rock slamming into them (they left the grid).
    pub lifted: Vec<Cell>,
}

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub broken_free: u64,
    pub cells_lifted: u64,
    pub settled: u64,
    pub cells_landed: u64,
    pub wrecks: u64,
    /// How far settling cells had to look for room (sum and largest ring).
    pub settle_rings: u64,
    pub settle_ring_max: u64,
    /// Soft chunks smashed into pieces.
    pub shattered: u64,
}

/// Width of a physics frame, metres.
const FRAME: i64 = 16_384;

/// One physics world, centred on its frame.
struct Frame {
    phys: PhysicsWorld,
    origin: (f64, f64),
    /// Ground colliders by leaf origin, and the tick each was built.
    ground: CellMap<(ColliderHandle, RigidBodyHandle, u64)>,
    /// When each ground leaf was last near a body (unused ground is dropped later).
    ground_used: CellMap<u64>,
}

/// The frame a cell belongs to (frame (0, 0) is centred on the body's centre).
fn frame_of(c: Cell) -> Cell {
    ((c.0 + FRAME / 2).div_euclid(FRAME), (c.1 + FRAME / 2).div_euclid(FRAME))
}

pub struct Bodies {
    frames: std::collections::BTreeMap<Cell, Frame>,
    dt: Real,
    loose: Vec<Loose>,
    /// Drops of liquid in flight (blasted out of vents): plain throws, not
    /// physics bodies, so there can be thousands.
    drops: Vec<Drop>,
    /// Leaves whose ground changed a little (digging nearby): rebuilt within
    /// a second. Leaves a body just left or landed in are rebuilt at once.
    soft_stale: CellSet,
    /// Solid cells next to fresh open space, to check for having been cut free.
    dirty: std::collections::BTreeSet<Cell>,
    /// Dirty cells that were in sight when marked: checked first.
    dirty_seen: std::collections::BTreeSet<Cell>,
    /// Where the last step's look through `big_dirty` stopped.
    big_cursor: Option<Cell>,
    /// Ground colliders' cells for leaves still as generated (they never change).
    untouched_vox: CellMap<Vec<IVector>>,
    slabs: Vec<Slab>,
    /// Cells to check, now and then, for big chunks cut free.
    big_dirty: std::collections::BTreeSet<Cell>,
    /// Cells the big search may look at now (see `break_free_big`), in
    /// sight and out of it.
    big_tokens: f64,
    far_tokens: f64,
    pub tick: u64,
    pub stats: Stats,
    /// Rock is only cut free where someone could see it fall (see
    /// sandbox::Attention); elsewhere the cells to check wait.
    attention: crate::sandbox::Attention,
    waiting: crate::sandbox::Waiting,
}

/// The open cell resting on the ground straight below (x, y), toward the
/// body's centre: where something falling from there comes to rest.
pub fn ground_below(w: &World, x: f64, y: f64) -> Cell {
    let l = (x * x + y * y).sqrt().max(1.0);
    let (ux, uy) = (x / l, y / l);
    // High above the body: skip straight down to just over its surface
    // (nothing stands higher than a few metres above it).
    let top = w.planet.surface_radius(ux, uy) + 32.0;
    let (x, y, l) = if l > top { (ux * top, uy * top, top) } else { (x, y, l) };
    let mut c = (x.floor() as i64, y.floor() as i64);
    let mut r = l;
    while r > 1.0 {
        let nr = r - 0.5;
        let n = ((ux * nr).floor() as i64, (uy * nr).floor() as i64);
        if n != c {
            if w.material(n.0, n.1) != VACUUM {
                break;
            }
            c = n;
        }
        r = nr;
    }
    c
}

/// Solid enough to stand on and to be cut free: solid rock (powders, liquids
/// and gases are the sandbox's business).
fn is_rock(w: &World, m: MatId) -> bool {
    m != VACUUM && w.planet.mats.get(m).state == State::Solid
}

impl Bodies {
    pub fn new(world: &World) -> Self {
        let dt = 1.0 / world.planet.mats.physics.ticks_per_s as Real;
        Bodies { frames: Default::default(), dt, loose: Vec::new(), drops: Vec::new(), soft_stale: CellSet::default(), dirty: Default::default(), dirty_seen: Default::default(), big_cursor: None, untouched_vox: CellMap::default(), slabs: Vec::new(), big_dirty: Default::default(), big_tokens: 0.0, far_tokens: 0.0, tick: 0, stats: Stats::default(), attention: None, waiting: Default::default() }
    }

    /// The frame with key k, made on first use.
    fn frame(&mut self, k: Cell) -> &mut Frame {
        let dt = self.dt;
        self.frames.entry(k).or_insert_with(|| {
            let mut phys = PhysicsWorld::default();
            phys.gravity = Vector::ZERO;
            phys.integration_parameters.dt = dt;
            Frame { phys, origin: ((k.0 * FRAME) as f64, (k.1 * FRAME) as f64), ground: CellMap::default(), ground_used: CellMap::default() }
        })
    }

    /// Where the point `local` of a body lies, in world metres.
    fn world_point(&self, l: &Loose, local: Vector) -> (f64, f64) {
        let f = &self.frames[&l.frame];
        let p = f.phys.bodies[l.handle].position() * local;
        (p.x as f64 + f.origin.0, p.y as f64 + f.origin.1)
    }

    /// Where rock is cut free (None, everywhere, is the default). Checks
    /// waiting where the new area covers are done next.
    pub fn set_attention(&mut self, a: crate::sandbox::Attention) {
        if self.attention == a {
            return;
        }
        self.attention = a;
        for c in self.waiting.take_seen(a) {
            self.dirty_seen.insert(c);
        }
    }

    /// Bodies in the air right now.
    pub fn count(&self) -> usize {
        self.loose.len() + self.slabs.len()
    }

    /// Drops of liquid in flight right now.
    pub fn drops(&self) -> usize {
        self.drops.len()
    }

    /// A cell next to `c` just opened: check its solid neighbours for being cut free.
    pub fn opened(&mut self, c: Cell) {
        self.soft_stale.insert((c.0.div_euclid(LEAF) * LEAF, c.1.div_euclid(LEAF) * LEAF));
        let seen = crate::sandbox::in_attention(self.attention, c);
        for d in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (c.0 + d.0, c.1 + d.1);
            if seen {
                self.dirty_seen.insert(n);
            } else {
                self.dirty.insert(n);
            }
        }
    }

    /// The cells of every body in the air, with their materials (for drawing).
    /// Every cell in the air as it should be drawn: smoothly placed and
    /// turned with its body, with the cell it was cut from.
    pub fn drawn(&self) -> Vec<AirCell> {
        let mut out = Vec::new();
        for l in &self.loose {
            let o = self.world_point(l, Vector::new(0.0, 0.0));
            let e = self.world_point(l, Vector::new(1.0, 0.0));
            let angle = (e.1 - o.1).atan2(e.0 - o.0) as f32;
            for ((i, j), m) in &l.cells {
                let p = self.world_point(l, Vector::new(*i as Real + 0.5, *j as Real + 0.5));
                out.push(AirCell { x: p.0, y: p.1, angle, home: (l.origin.0 + *i as i64, l.origin.1 + *j as i64), m: *m });
            }
        }
        for d in &self.drops {
            out.push(AirCell { x: d.p.0, y: d.p.1, angle: 0.0, home: d.from, m: d.m });
        }
        for sl in &self.slabs {
            for ((i, j), m) in &sl.cells {
                let home = (sl.origin.0 + *i as i64, sl.origin.1 + *j as i64);
                out.push(AirCell { x: home.0 as f64 + 0.5 + sl.pos.0, y: home.1 as f64 + 0.5 + sl.pos.1, angle: 0.0, home, m: *m });
            }
        }
        out
    }

    pub fn cells(&self) -> Vec<(Cell, MatId)> {
        let mut out = Vec::new();
        for l in &self.loose {
            for ((i, j), m) in &l.cells {
                let p = self.world_point(l, Vector::new(*i as Real + 0.5, *j as Real + 0.5));
                out.push(((p.0.floor() as i64, p.1.floor() as i64), *m));
            }
        }
        for sl in &self.slabs {
            for ((i, j), m) in &sl.cells {
                out.push(((sl.origin.0 + *i as i64 + sl.at.0, sl.origin.1 + *j as i64 + sl.at.1), *m));
            }
        }
        out
    }

    /// Find the island of solid rock containing `start`, if it is small enough
    /// to have been cut free. `attached` collects cells known to hold fast.
    fn island(w: &World, start: Cell, limit: usize, attached: &mut CellSet) -> Option<Vec<Cell>> {
        let mut rd = w.reader();
        if attached.contains(&start) || !is_rock(w, rd.get(start.0, start.1)) {
            return None;
        }
        let mut seen = CellSet::default();
        let mut stack = vec![start];
        seen.insert(start);
        while let Some(c) = stack.pop() {
            for d in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let n = (c.0 + d.0, c.1 + d.1);
                if seen.contains(&n) {
                    continue;
                }
                // Reaching ground nothing has touched yet (still as the
                // generator made it) means reaching the bulk of the body.
                if attached.contains(&n) || !w.in_bounds(n.0, n.1) || !rd.touched(n.0, n.1) {
                    attached.extend(seen);
                    return None;
                }
                if !is_rock(w, rd.get(n.0, n.1)) {
                    continue;
                }
                seen.insert(n);
                if seen.len() > limit {
                    attached.extend(seen);
                    return None;
                }
                stack.push(n);
            }
        }
        let mut cells: Vec<Cell> = seen.into_iter().collect();
        cells.sort();
        Some(cells)
    }

    /// Lift these grid cells into a new body. Returns the cells lifted.
    fn launch_spinning(&mut self, w: &mut World, cells: &[Cell], wreck: Option<MatId>, vel: Vector, spin: Real) -> Vec<Cell> {
        let mut out = Vec::new();
        for &c in cells {
            let m = match wreck {
                Some(m) => m,
                None => match w.lift_cell(c.0, c.1) {
                    Some(m) => m,
                    None => continue,
                },
            };
            self.stale(c);
            out.push((c, m));
        }
        self.launch_lifted(w, &out, wreck.is_some(), vel, spin);
        cells.to_vec()
    }

    /// Make a tumbling body of cells already out of the grid.
    fn launch_lifted(&mut self, w: &World, cells: &[(Cell, MatId)], wreck: bool, vel: Vector, spin: Real) {
        if cells.is_empty() {
            return;
        }
        let (mut sx, mut sy) = (0i64, 0i64);
        for (c, _) in cells {
            sx += c.0;
            sy += c.1;
        }
        let n = cells.len() as i64;
        let origin = (sx.div_euclid(n), sy.div_euclid(n));
        let mut local = Vec::new();
        let mut voxels = Vec::new();
        let mut mass = 0.0;
        for &(c, m) in cells {
            let off = ((c.0 - origin.0) as i32, (c.1 - origin.1) as i32);
            local.push((off, m));
            voxels.push(IVector::new(off.0, off.1));
            mass += w.planet.mats.get(m).density;
        }
        let density = (mass / local.len() as f64) as Real;
        let soft = !wreck && mean_hardness(w, local.iter().map(|c| c.1)) <= SOFT;
        let fk = frame_of(origin);
        let f = self.frame(fk);
        let at = ((origin.0 as f64 - f.origin.0) as Real, (origin.1 as f64 - f.origin.1) as Real);
        let (handle, _) = f.phys.insert(
            RigidBodyBuilder::dynamic().translation(Vector::new(at.0, at.1)).linvel(vel).angvel(spin).ccd_enabled(true),
            ColliderBuilder::voxels(Vector::new(1.0, 1.0), &voxels).density(density).friction(0.7).restitution(0.05),
        );
        let speed = (vel.x * vel.x + vel.y * vel.y).sqrt() as f64;
        let gouge_left = local.len() / 3;
        self.loose.push(Loose { frame: fk, origin, handle, cells: local, still_s: 0.0, wreck, soft, last_speed: speed, gouge_left, age: 0 });
    }

    /// The ground covering cell c changed a little: rebuild it within a second.
    pub fn touched(&mut self, c: Cell) {
        self.soft_stale.insert((c.0.div_euclid(LEAF) * LEAF, c.1.div_euclid(LEAF) * LEAF));
    }

    /// The ground collider covering cell c no longer matches the grid: rebuild it now.
    fn stale(&mut self, c: Cell) {
        let k = (c.0.div_euclid(LEAF) * LEAF, c.1.div_euclid(LEAF) * LEAF);
        for f in self.frames.values_mut() {
            if let Some(g) = f.ground.get_mut(&k) {
                g.2 = 0;
            }
        }
    }

    /// A wrecked drone at (x, y) becomes a cell of drone scrap in the nearest
    /// open cell (matter from the swarm, booked as deposited). Scrap is loose,
    /// so the sandbox lets it fall and pile up. Returns where it went.
    pub fn wreck(&mut self, w: &mut World, x: f64, y: f64) -> Option<Cell> {
        let scrap = w.planet.mats.id("drone-scrap").unwrap();
        // It falls: set down on the ground below, not left hanging in the air.
        let c = ground_below(w, x, y);
        self.stats.wrecks += 1;
        for ring in 0..16i64 {
            for dy in -ring..=ring {
                for dx in -ring..=ring {
                    if (dx.abs() == ring || dy.abs() == ring) && w.deposit_cell(c.0 + dx, c.1 + dy, scrap) {
                        return Some((c.0 + dx, c.1 + dy));
                    }
                }
            }
        }
        None
    }

    /// Debris thrown out by an explosion: one cell of `m`, not yet in the
    /// grid, at `at` flying at `vel`. In sight (and with room) it tumbles as
    /// a body and lands where it falls; otherwise it is set straight into the
    /// nearest open cell (and falls from there if it is powder). Returns the
    /// cell it was set into, if it was.
    pub fn fling(&mut self, w: &mut World, at: (f64, f64), m: MatId, vel: (f64, f64)) -> Option<Cell> {
        let c = (at.0.floor() as i64, at.1.floor() as i64);
        if self.loose.len() < MAX_LOOSE && crate::sandbox::in_attention(self.attention, c) && w.material(c.0, c.1) == VACUUM && w.in_bounds(c.0, c.1) {
            let spin = ((c.0 * 31 + c.1 * 17).rem_euclid(13) as Real - 6.0) * 0.8;
            self.launch_lifted(w, &[(c, m)], true, Vector::new(vel.0 as Real, vel.1 as Real), spin);
            return None;
        }
        // Out of sight (or too much in the air): it lands on the ground below.
        let c = ground_below(w, at.0, at.1);
        for ring in 0..16i64 {
            for dy in -ring..=ring {
                for dx in -ring..=ring {
                    if (dx.abs() == ring || dy.abs() == ring) && w.deposit_cell(c.0 + dx, c.1 + dy, m) {
                        return Some((c.0 + dx, c.1 + dy));
                    }
                }
            }
        }
        None
    }

    /// A cell already lifted out of the ground (booked as in the air) thrown
    /// from `at` at `vel`: liquid blasted out of a pressurised hole. In sight
    /// it flies as a drop and lands where it falls; otherwise (or with too
    /// many in the air) it is set straight down where it would come down.
    /// Returns the cell it was set into, if it was.
    pub fn throw(&mut self, w: &mut World, at: (f64, f64), m: MatId, vel: (f64, f64)) -> Option<Cell> {
        let c = (at.0.floor() as i64, at.1.floor() as i64);
        if self.drops.len() < MAX_DROPS && crate::sandbox::in_attention(self.attention, c) && w.material(c.0, c.1) == VACUUM {
            self.drops.push(Drop { m, p: at, v: vel, from: c });
            return None;
        }
        land_thrown(w, at, m, vel)
    }

    /// Move the drops in flight one tick; set down those that hit something.
    fn step_drops(&mut self, w: &mut World, ev: &mut Events, dt: f64) {
        let far = w.planet.max_radius * 3.0;
        let mut i = 0;
        while i < self.drops.len() {
            let d = self.drops[i];
            let c0 = (d.p.0.floor() as i64, d.p.1.floor() as i64);
            // Out of sight (the view moved away, or zoomed out): down at once where it would land.
            if !crate::sandbox::in_attention(self.attention, c0) {
                self.drops.swap_remove(i);
                if let Some(t) = land_thrown(w, d.p, d.m, d.v) {
                    ev.landed.push((t, d.m));
                }
                continue;
            }
            let a = pull(&w.planet, d.p.0, d.p.1);
            let v = (d.v.0 + a.0 * dt, d.v.1 + a.1 * dt);
            let to = (d.p.0 + v.0 * dt, d.p.1 + v.1 * dt);
            if (to.0 * to.0 + to.1 * to.1).sqrt() > far {
                self.drops.swap_remove(i);
                w.lose_aloft(d.m);
                continue;
            }
            // Along the way a metre at a time: the last open cell before
            // anything in the way is where it lands.
            let len = ((to.0 - d.p.0).powi(2) + (to.1 - d.p.1).powi(2)).sqrt();
            let n = (len.ceil() as usize).max(1);
            let mut last = c0;
            let mut hit = false;
            for k in 1..=n {
                let t = k as f64 / n as f64;
                let p = ((d.p.0 + (to.0 - d.p.0) * t).floor() as i64, (d.p.1 + (to.1 - d.p.1) * t).floor() as i64);
                if p == last {
                    continue;
                }
                if !w.in_bounds(p.0, p.1) || w.material(p.0, p.1) != VACUUM {
                    hit = true;
                    break;
                }
                last = p;
            }
            if hit {
                self.drops.swap_remove(i);
                match settle_near(w, last, d.m) {
                    Some(t) => ev.landed.push((t, d.m)),
                    None => w.lose_aloft(d.m),
                }
                continue;
            }
            self.drops[i].p = to;
            self.drops[i].v = v;
            i += 1;
        }
    }

    /// Check the dirty cells for islands cut free and lift them. Returns the
    /// cells that left the grid.
    pub fn break_free(&mut self, w: &mut World) -> Vec<Cell> {
        if self.dirty.is_empty() && self.dirty_seen.is_empty() {
            return Vec::new();
        }
        let limit = w.planet.mats.physics.island_max_cells;
        // A few hundred a step at most, lowest first (after the view moves,
        // or with a huge swarm cutting, many wait): the rest wait for the
        // next steps. (Kept in order, so taking them costs nothing extra
        // however many wait.)
        const MAX_STARTS: usize = 500;
        // Those in sight first: with a huge swarm digging everywhere, cells
        // out of sight could otherwise hold up what's on screen for a minute.
        // (Out of sight they only go to wait, which is cheap: many a step.)
        let mut starts: Vec<Cell> = Vec::with_capacity(MAX_STARTS.min(self.dirty.len()));
        while starts.len() < MAX_STARTS {
            let Some(c) = self.dirty_seen.pop_first() else { break };
            self.dirty.remove(&c);
            starts.push(c);
        }
        let mut aside = 0;
        while starts.len() < MAX_STARTS && aside < 20 * MAX_STARTS {
            let Some(c) = self.dirty.pop_first() else { break };
            if !crate::sandbox::in_attention(self.attention, c) {
                self.waiting.add(c);
                self.big_dirty.insert(c);
                aside += 1;
                continue;
            }
            starts.push(c);
        }
        starts.sort();
        let mut attached = CellSet::default();
        let mut lifted = Vec::new();
        for s in starts {
            if !crate::sandbox::in_attention(self.attention, s) {
                self.waiting.add(s);
                self.big_dirty.insert(s);
                continue;
            }
            if !attached.contains(&s) {
                // (It may be part of a chunk too big for this quick search.)
                self.big_dirty.insert(s);
            }
            if let Some(cells) = Self::island(w, s, limit, &mut attached) {
                // Specks too small to matter cling where they are.
                if cells.len() < w.planet.mats.physics.island_min_cells {
                    attached.extend(cells);
                    continue;
                }
                let (v, spin) = orphan_push(cells[0], w.planet.mats.physics.chunk_kick_m_s, cells.len());
                let out = self.launch_spinning(w, &cells, None, Vector::new(v.0 as Real, v.1 as Real), spin as Real);
                self.stats.broken_free += 1;
                self.stats.cells_lifted += out.len() as u64;
                lifted.extend(out);
            }
        }
        lifted
    }

    /// The slower search for big chunks cut free (up to `slab_max_cells`),
    /// over the cells set aside by `break_free`, within a budget of cells
    /// looked at. Returns the cells that left the grid.
    pub fn break_free_big(&mut self, w: &mut World) -> Vec<Cell> {
        if self.big_dirty.is_empty() {
            return Vec::new();
        }
        let phys = w.planet.mats.physics.clone();
        // A steady allowance of cells to look at (150,000 every slab_check_s
        // in sight; out of sight a tenth, and no search for rock hanging by
        // a thread, the costly part: what comes loose there still falls,
        // only less of it, and later). It is spent as soon as there is
        // enough for one whole search, so the work comes in small pieces
        // every few steps rather than one long pause.
        let per_tick = 1.0 / (phys.slab_check_s * phys.ticks_per_s).max(1.0);
        self.big_tokens = (self.big_tokens + 150_000.0 * per_tick).min(60_000.0);
        self.far_tokens = (self.far_tokens + 15_000.0 * per_tick).min(30_000.0);
        let need = phys.slab_max_cells as f64;
        if self.big_tokens < need && self.far_tokens < need {
            return Vec::new();
        }
        let mut attached = CellSet::default();
        let mut lifted = Vec::new();
        let mut budget: usize = if self.big_tokens >= need { self.big_tokens as usize } else { 0 };
        let mut far_budget: usize = if self.far_tokens >= need { self.far_tokens as usize } else { 0 };
        let (b0, f0) = (budget, far_budget);
        // Through the waiting cells in order, a few at a time (copying the
        // whole set every step was costly with a big swarm digging).
        // (It carries on each step from where the last stopped, and looks at
        // no more than BIG_SCAN waiting cells a step: with a huge swarm
        // digging there can be twenty thousand.)
        let mut cursor: Option<Cell> = self.big_cursor;
        let mut scanned = 0usize;
        let mut wrapped = false;
        'all: loop {
            if scanned >= BIG_SCAN {
                break;
            }
            let chunk: Vec<Cell> = match cursor {
                None => self.big_dirty.iter().take(64).copied().collect(),
                Some(c) => self.big_dirty.range((std::ops::Bound::Excluded(c), std::ops::Bound::Unbounded)).take(64).copied().collect(),
            };
            let Some(&last) = chunk.last() else {
                if wrapped || cursor.is_none() {
                    cursor = None;
                    break;
                }
                wrapped = true;
                cursor = None;
                continue;
            };
            scanned += chunk.len();
            cursor = Some(last);
            for s in chunk {
                let seen = crate::sandbox::in_attention(self.attention, s);
                // Only start a search there is enough left to finish.
                let enough = need as usize;
                if (seen && budget < enough) || (!seen && far_budget < enough) {
                    if budget < enough && far_budget < enough {
                        break 'all;
                    }
                    continue;
                }
                self.big_dirty.remove(&s);
                let b = if seen { &mut budget } else { &mut far_budget };
                // Hanging by a thread: rock held on only through strips one cell
                // wide may snap, the likelier the heavier it is for its hardness.
                let found = match Self::island_whole(w, s, phys.slab_max_cells, &mut attached, b) {
                    Some(c) => Some(c),
                    None if !seen => None,
                    None => Self::island_necked(w, s, phys.slab_max_cells, b).filter(|c| {
                        let h = mean_hardness(w, c.iter().map(|x| w.material(x.0, x.1))).max(1.0);
                        let p = (c.len() as f64 / (NECK_CELLS_PER_HARDNESS * h)).clamp(0.05, 1.0);
                        crate::noise::unit(crate::noise::hash(0x9EC4, s.0, s.1, self.tick)) < p
                    }),
                };
                if let Some(cells) = found {
                    if cells.len() < phys.island_min_cells {
                        attached.extend(cells);
                        continue;
                    }
                    self.stats.broken_free += 1;
                    self.stats.cells_lifted += cells.len() as u64;
                    let (v, spin) = orphan_push(cells[0], phys.chunk_kick_m_s, cells.len());
                    let out = if cells.len() <= phys.island_max_cells { self.launch_spinning(w, &cells, None, Vector::new(v.0 as Real, v.1 as Real), spin as Real) } else { self.launch_slab(w, &cells, v) };
                    lifted.extend(out);
                }
            }
        }
        self.big_cursor = cursor;
        self.big_tokens -= (b0 - budget) as f64;
        self.far_tokens -= (f0 - far_budget) as f64;
        // Too many waiting: start over from the newest (the rest are mostly
        // bulk ground anyway).
        if self.big_dirty.len() > 20_000 {
            self.big_dirty.clear();
        }
        lifted
    }

    /// Like `island`, but through ground nothing has touched too (big chunks
    /// can hold plenty of it), up to `limit` cells; `budget` counts down the
    /// cells looked at.
    fn island_whole(w: &World, start: Cell, limit: usize, attached: &mut CellSet, budget: &mut usize) -> Option<Vec<Cell>> {
        let mut rd = w.reader();
        if attached.contains(&start) || !is_rock(w, rd.get(start.0, start.1)) {
            return None;
        }
        let mut seen = CellSet::default();
        let mut stack = vec![start];
        seen.insert(start);
        // Ground whose 64 m leaf and all eight around it are untouched is
        // the bulk of the body: a chunk under the size limit can't hold a
        // 192 m square of it. (Answered once per leaf.)
        let mut bulk: CellMap<bool> = CellMap::default();
        while let Some(c) = stack.pop() {
            for d in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let n = (c.0 + d.0, c.1 + d.1);
                if seen.contains(&n) {
                    continue;
                }
                if attached.contains(&n) || !w.in_bounds(n.0, n.1) {
                    attached.extend(seen);
                    return None;
                }
                let leaf = (n.0.div_euclid(LEAF), n.1.div_euclid(LEAF));
                let deep = *bulk.entry(leaf).or_insert_with(|| {
                    (-1..=1).all(|j| (-1..=1).all(|i| !rd.touched((leaf.0 + i) * LEAF, (leaf.1 + j) * LEAF)))
                });
                if deep {
                    attached.extend(seen);
                    return None;
                }
                *budget = budget.saturating_sub(1);
                if !is_rock(w, rd.get(n.0, n.1)) {
                    continue;
                }
                seen.insert(n);
                if seen.len() > limit || *budget == 0 {
                    attached.extend(seen);
                    return None;
                }
                stack.push(n);
            }
        }
        let mut cells: Vec<Cell> = seen.into_iter().collect();
        cells.sort();
        Some(cells)
    }

    /// The rock around `start` that holds on to the rest only through thin
    /// strips: like `island_whole`, but only through cells with at least
    /// five solid cells in their 3 x 3 (so a strip one cell wide, or a
    /// corner-to-corner touch, doesn't count as holding).
    fn island_necked(w: &World, start: Cell, limit: usize, budget: &mut usize) -> Option<Vec<Cell>> {
        let mut rd = w.reader();
        let mut thick: CellMap<bool> = CellMap::default();
        let mut is_thick = |c: Cell, rd: &mut crate::world::Reader, budget: &mut usize| -> bool {
            *thick.entry(c).or_insert_with(|| {
                *budget = budget.saturating_sub(9);
                let mut n = 0;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if is_rock(w, rd.get(c.0 + dx, c.1 + dy)) {
                            n += 1;
                        }
                    }
                }
                n >= 5
            })
        };
        if !is_rock(w, rd.get(start.0, start.1)) || !is_thick(start, &mut rd, budget) {
            return None;
        }
        let mut seen = CellSet::default();
        let mut stack = vec![start];
        seen.insert(start);
        let mut bulk: CellMap<bool> = CellMap::default();
        while let Some(c) = stack.pop() {
            for d in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let n = (c.0 + d.0, c.1 + d.1);
                if seen.contains(&n) {
                    continue;
                }
                if !w.in_bounds(n.0, n.1) || *budget == 0 {
                    return None;
                }
                let leaf = (n.0.div_euclid(LEAF), n.1.div_euclid(LEAF));
                if *bulk.entry(leaf).or_insert_with(|| (-1..=1).all(|j| (-1..=1).all(|i| !rd.touched((leaf.0 + i) * LEAF, (leaf.1 + j) * LEAF)))) {
                    return None;
                }
                if !is_rock(w, rd.get(n.0, n.1)) || !is_thick(n, &mut rd, budget) {
                    continue;
                }
                seen.insert(n);
                if seen.len() > limit {
                    return None;
                }
                stack.push(n);
            }
        }
        let mut cells: Vec<Cell> = seen.into_iter().collect();
        cells.sort();
        Some(cells)
    }

    /// Lift these grid cells into a slab. Returns the cells lifted.
    fn launch_slab(&mut self, w: &mut World, cells: &[Cell], vel: (f64, f64)) -> Vec<Cell> {
        let origin = cells[0];
        let mut local = Vec::with_capacity(cells.len());
        let set: CellSet = cells.iter().copied().collect();
        let mut edge = Vec::new();
        let (mut mx, mut my) = (0.0, 0.0);
        for &c in cells {
            let Some(m) = w.lift_cell(c.0, c.1) else { continue };
            self.stale(c);
            let off = ((c.0 - origin.0) as i32, (c.1 - origin.1) as i32);
            local.push((off, m));
            mx += off.0 as f64 + 0.5;
            my += off.1 as f64 + 0.5;
            if [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|d| !set.contains(&(c.0 + d.0, c.1 + d.1))) {
                edge.push(off);
            }
        }
        if local.is_empty() {
            return Vec::new();
        }
        let n = local.len() as f64;
        let hardness = mean_hardness(w, local.iter().map(|c| c.1));
        self.slabs.push(Slab { cells: local, edge, origin, middle: (mx / n, my / n), pos: (0.0, 0.0), vel, at: (0, 0), hardness, soft: hardness <= SOFT, shed_acc: 0.0, gouge_left: n as usize / 3 });
        cells.to_vec()
    }

    /// Move the slabs one tick: pull, move cell by cell (sliding along the
    /// ground when blocked, so a slab on a slope slides and grinds down it),
    /// shed pieces as it goes, smash on hard impacts, and set down once it
    /// has come to rest against the ground (or lose it if it drifts far off).
    fn step_slabs(&mut self, w: &mut World, ev: &mut Events, dt: f64) {
        let phys = w.planet.mats.physics.clone();
        let far = w.planet.max_radius * 3.0;
        let mut i = 0;
        while i < self.slabs.len() {
            let (cx, cy) = {
                let sl = &self.slabs[i];
                (sl.origin.0 as f64 + sl.middle.0 + sl.pos.0, sl.origin.1 as f64 + sl.middle.1 + sl.pos.1)
            };
            if (cx * cx + cy * cy).sqrt() > far {
                // Gone for good.
                let sl = self.slabs.swap_remove(i);
                for (_, m) in sl.cells {
                    w.lose_aloft(m);
                }
                continue;
            }
            let a = pull(&w.planet, cx, cy);
            let sl = &mut self.slabs[i];
            sl.vel.0 += a.0 * dt;
            sl.vel.1 += a.1 * dt;
            sl.pos.0 += sl.vel.0 * dt;
            sl.pos.1 += sl.vel.1 * dt;
            // Step toward where it would be, a cell at a time, checking its
            // outline against the grid. Blocked along one axis, it stops dead
            // that way and slides on along the other, losing some speed.
            let mut contact = false;
            let mut impact = 0.0f64;
            let mut struck: Vec<Cell> = Vec::new();
            for _ in 0..64 {
                let target = (sl.pos.0.round() as i64, sl.pos.1.round() as i64);
                if sl.at == target {
                    break;
                }
                let (dx, dy) = (target.0 - sl.at.0, target.1 - sl.at.1);
                let along_x = dx.abs() >= dy.abs();
                let step = if along_x { (dx.signum(), 0) } else { (0, dy.signum()) };
                let next = (sl.at.0 + step.0, sl.at.1 + step.1);
                let mut blocked = false;
                for &(ci, cj) in &sl.edge {
                    let c = (sl.origin.0 + ci as i64 + next.0, sl.origin.1 + cj as i64 + next.1);
                    if w.in_bounds(c.0, c.1) && w.material(c.0, c.1) != VACUUM {
                        blocked = true;
                        if struck.len() < 64 {
                            struck.push(c);
                        }
                    }
                }
                if blocked {
                    contact = true;
                    if along_x {
                        impact = impact.max(sl.vel.0.abs());
                        sl.vel.0 = 0.0;
                        sl.pos.0 = sl.at.0 as f64;
                        sl.vel.1 *= 0.85;
                    } else {
                        impact = impact.max(sl.vel.1.abs());
                        sl.vel.1 = 0.0;
                        sl.pos.1 = sl.at.1 as f64;
                        sl.vel.0 *= 0.85;
                    }
                    continue;
                }
                sl.at = next;
            }
            let speed = (sl.vel.0 * sl.vel.0 + sl.vel.1 * sl.vel.1).sqrt();
            let n = sl.cells.len();
            // Heavy and moving: it crushes drones working in its way.
            if crushes(speed, n, phys.crush_speed_m_s) {
                for &(ci, cj) in sl.edge.iter().take(4096) {
                    ev.crushing.insert((sl.origin.0 + ci as i64 + sl.at.0, sl.origin.1 + cj as i64 + sl.at.1));
                }
            }
            let soft = sl.soft;
            let hardness = sl.hardness;
            // A hard knock: soft rock shatters into pieces; hard rock knocks
            // loose softer ground it hits, and chips.
            if impact > SMASH_M_S {
                if soft {
                    let sl = self.slabs.swap_remove(i);
                    self.shatter(w, ev, sl);
                    continue;
                }
                let left = self.slabs[i].gouge_left;
                let n = self.gouge(w, ev, &struck, hardness, impact, left);
                self.slabs[i].gouge_left -= n;
                let chips = 1 + (impact / SMASH_M_S) as usize;
                self.shed(w, ev, i, chips);
            }
            // Pieces break off as it moves: soft rock crumbles, hard rock
            // now and then loses a piece.
            let sl = &mut self.slabs[i];
            let rate = if soft { 0.02 } else { 0.002 };
            sl.shed_acc += sl.edge.len() as f64 * rate * speed * dt;
            let k = sl.shed_acc.floor() as usize;
            if k > 0 {
                self.slabs[i].shed_acc -= k as f64;
                self.shed(w, ev, i, k.min(32));
            }
            let sl = &self.slabs[i];
            if sl.cells.len() < 2 || (contact && speed < 0.15) {
                // At rest against the ground: set every cell into the grid
                // where it lies (or as near as there is room).
                let sl = self.slabs.swap_remove(i);
                for ((ci, cj), m) in sl.cells {
                    let c = (sl.origin.0 + ci as i64 + sl.at.0, sl.origin.1 + cj as i64 + sl.at.1);
                    self.set_down(w, ev, c, m);
                }
                self.stats.settled += 1;
                continue;
            }
            i += 1;
        }
    }

    /// Set an airborne cell into the grid at c, or the nearest open cell.
    fn set_down(&mut self, w: &mut World, ev: &mut Events, c: Cell, m: MatId) {
        for ring in 0..64i64 {
            for dy in -ring..=ring {
                for dx in -ring..=ring {
                    if (dx.abs() == ring || dy.abs() == ring) && w.land_cell(c.0 + dx, c.1 + dy, m) {
                        let t = (c.0 + dx, c.1 + dy);
                        self.stale(t);
                        ev.landed.push((t, m));
                        self.stats.cells_landed += 1;
                        return;
                    }
                }
            }
        }
        w.lose_aloft(m);
    }

    /// Break `k` cells off slab i's outline as pieces flying off with it
    /// (or, with too much in the air already, dropped where they are).
    fn shed(&mut self, w: &mut World, ev: &mut Events, i: usize, k: usize) {
        let sl = &mut self.slabs[i];
        if sl.cells.len() <= k + 1 {
            return;
        }
        let mut gone: Vec<((i32, i32), MatId)> = Vec::new();
        for q in 0..k {
            if sl.edge.is_empty() {
                break;
            }
            let pick = (crate::noise::hash(0x5E7D, sl.at.0 + q as i64, sl.at.1, self.tick) % sl.edge.len() as u64) as usize;
            let off = sl.edge.swap_remove(pick);
            if let Some(p) = sl.cells.iter().position(|(o, _)| *o == off) {
                gone.push(sl.cells.swap_remove(p));
            }
        }
        sl.rebuild_edge();
        let (origin, at, vel) = (sl.origin, sl.at, sl.vel);
        for (q, ((ci, cj), m)) in gone.into_iter().enumerate() {
            let c = (origin.0 + ci as i64 + at.0, origin.1 + cj as i64 + at.1);
            let j = kick(c, 0.6 + 0.2 * q as f64);
            self.piece(w, ev, &[(c, m)], (vel.0 + j.0, vel.1 + j.1), (j.0 - j.1) * 2.0);
        }
    }

    /// A soft slab smashes: into pieces of about 3 x 3 cells, scattering.
    fn shatter(&mut self, w: &mut World, ev: &mut Events, sl: Slab) {
        let mut groups: std::collections::BTreeMap<(i32, i32), Vec<(Cell, MatId)>> = Default::default();
        for ((ci, cj), m) in &sl.cells {
            let c = (sl.origin.0 + *ci as i64 + sl.at.0, sl.origin.1 + *cj as i64 + sl.at.1);
            groups.entry((ci.div_euclid(3), cj.div_euclid(3))).or_default().push((c, *m));
        }
        for (k, (_, cells)) in groups.into_iter().enumerate() {
            let j = kick(cells[0].0, 1.0 + 0.1 * (k % 7) as f64);
            self.piece(w, ev, &cells, (sl.vel.0 * 0.4 + j.0, sl.vel.1 * 0.4 + j.1), j.0 * 3.0);
        }
        self.stats.shattered += 1;
    }

    /// Hard rock slamming into softer ground knocks some of it loose.
    /// Returns how many it knocked loose.
    fn gouge(&mut self, w: &mut World, ev: &mut Events, struck: &[Cell], hardness: f64, impact: f64, limit: usize) -> usize {
        if impact < GOUGE_M_S {
            return 0;
        }
        let k = ((impact / GOUGE_M_S) as usize * 2).clamp(1, 12).min(limit);
        let mut n = 0;
        for &c in struck {
            if n >= k {
                break;
            }
            let m = w.material(c.0, c.1);
            // Only ground clearly softer than the rock hitting it gives way.
            if !is_rock(w, m) || (w.planet.mats.get(m).hardness as f64) > hardness - 2.0 {
                continue;
            }
            if let Some(m) = w.lift_cell(c.0, c.1) {
                self.stale(c);
                ev.lifted.push(c);
                let j = kick(c, (impact * 0.25).min(3.0));
                self.piece(w, ev, &[(c, m)], j, j.0 * 4.0);
                n += 1;
            }
        }
        n
    }

    /// Launch airborne cells as a small tumbling body, unless too many are
    /// in the air already: then they're simply set down where they are.
    fn piece(&mut self, w: &mut World, ev: &mut Events, cells: &[(Cell, MatId)], vel: (f64, f64), spin: f64) {
        if self.loose.len() >= MAX_LOOSE {
            for &(c, m) in cells {
                self.set_down(w, ev, c, m);
            }
            return;
        }
        self.launch_lifted(w, cells, false, Vector::new(vel.0 as Real, vel.1 as Real), spin as Real);
    }

    /// Make sure the ground near every body in frame `fk` has a fresh collider.
    fn refresh_ground(&mut self, w: &World, fk: Cell) {
        let mut want = CellSet::default();
        for l in self.loose.iter().filter(|l| l.frame == fk) {
            let f = &self.frames[&fk];
            let t = f.phys.bodies[l.handle].translation();
            let p = (t.x as f64 + f.origin.0, t.y as f64 + f.origin.1);
            let reach = (l.cells.len() as f64).sqrt() + 8.0;
            let (x0, x1) = ((p.0 - reach).floor() as i64, (p.0 + reach).ceil() as i64);
            let (y0, y1) = ((p.1 - reach).floor() as i64, (p.1 + reach).ceil() as i64);
            let mut lx = x0.div_euclid(LEAF) * LEAF;
            while lx <= x1 {
                let mut ly = y0.div_euclid(LEAF) * LEAF;
                while ly <= y1 {
                    want.insert((lx, ly));
                    ly += LEAF;
                }
                lx += LEAF;
            }
        }
        let tick = self.tick;
        let soft_stale = &mut self.soft_stale;
        let f = self.frames.get_mut(&fk).unwrap();
        // Drop ground no body is near; rebuild stale ground every few ticks.
        let mut keys: Vec<Cell> = f.ground.keys().copied().collect();
        keys.sort();
        for k in keys {
            let (ch, bh, built) = f.ground[&k];
            if want.contains(&k) {
                f.ground_used.insert(k, tick);
            }
            let unused = tick > f.ground_used.get(&k).copied().unwrap_or(0) + 300;
            let soft = want.contains(&k) && soft_stale.contains(&k) && tick >= built + 50;
            if unused || built == 0 || soft {
                soft_stale.remove(&k);
                let _ = ch;
                f.phys.remove_body(bh);
                f.ground.remove(&k);
                if unused {
                    f.ground_used.remove(&k);
                }
            }
        }
        let mut want: Vec<Cell> = want.into_iter().collect();
        want.sort();
        for (lx, ly) in want {
            if f.ground.contains_key(&(lx, ly)) {
                continue;
            }
            // Rock rests on liquid too (magma, water): it would sink only
            // slowly, and settling inside a deep pool would leave it nowhere
            // to go but far up and out.
            let solid = |m: MatId| m != VACUUM && matches!(w.planet.mats.get(m).state, State::Solid | State::Powder | State::Liquid);
            // (Ground leaves line up with the world's leaves whenever the
            // world's origin is on the 64 m grid, as it is for any body
            // bigger than a few dozen metres.)
            let aligned = (lx - w.tree.origin).rem_euclid(LEAF) == 0;
            let vox: Vec<IVector> = match w.dense_leaf(lx, ly).filter(|_| aligned) {
                // Generated cell by cell: read the cells directly.
                Some(cells) => {
                    let mut v = Vec::new();
                    for y in 0..LEAF {
                        for x in 0..LEAF {
                            if solid(cells[(y * LEAF + x) as usize]) {
                                v.push(IVector::new(x as i32, y as i32));
                            }
                        }
                    }
                    v
                }
                // Untouched: as the generator made it, which never changes,
                // so it is worked out once and kept.
                None if !aligned => {
                    let mut v = Vec::new();
                    let mut rd = w.reader();
                    for y in 0..LEAF {
                        for x in 0..LEAF {
                            if solid(rd.get(lx + x, ly + y)) {
                                v.push(IVector::new(x as i32, y as i32));
                            }
                        }
                    }
                    v
                }
                // Mined out whole since: nothing to stand on.
                None if w.in_empty_region(lx, ly) => Vec::new(),
                None => {
                    if self.untouched_vox.len() > 512 {
                        self.untouched_vox.clear();
                    }
                    self.untouched_vox
                        .entry((lx, ly))
                        .or_insert_with(|| {
                            let mut v = Vec::new();
                            let mut rd = w.reader();
                            for y in 0..LEAF {
                                for x in 0..LEAF {
                                    if solid(rd.get(lx + x, ly + y)) {
                                        v.push(IVector::new(x as i32, y as i32));
                                    }
                                }
                            }
                            v
                        })
                        .clone()
                }
            };
            if vox.is_empty() {
                continue;
            }
            let at = Vector::new((lx as f64 - f.origin.0) as Real, (ly as f64 - f.origin.1) as Real);
            let (bh, ch) = f.phys.insert(RigidBodyBuilder::fixed().translation(at), ColliderBuilder::voxels(Vector::new(1.0, 1.0), &vox).friction(0.7));
            f.ground.insert((lx, ly), (ch, bh, tick));
        }
    }

    /// Set a body's cells down into the grid where it lies, each in the
    /// nearest open cell (searching outward, then upward if crowded).
    fn settle(&mut self, w: &mut World, i: usize, ev: &mut Events) {
        self.settle_at(w, i, ev, false);
    }

    /// Set body i down: where it is, or (`drop`) on the ground straight
    /// below it, as if it had fallen there.
    fn settle_at(&mut self, w: &mut World, i: usize, ev: &mut Events, drop: bool) {
        let l = self.loose.remove(i);
        let f = self.frames.get_mut(&l.frame).unwrap();
        let pose = *f.phys.bodies[l.handle].position();
        let origin = f.origin;
        f.phys.remove_body(l.handle);
        for ((ci, cj), m) in l.cells {
            let p = pose * Vector::new(ci as Real + 0.5, cj as Real + 0.5);
            let c = ((p.x as f64 + origin.0).floor() as i64, (p.y as f64 + origin.1).floor() as i64);
            let c = if drop { ground_below(w, c.0 as f64 + 0.5, c.1 as f64 + 0.5) } else { c };
            // Outward from the centre is "up".
            let r = ((c.0 * c.0 + c.1 * c.1) as f64).sqrt().max(1.0);
            let up = ((c.0 as f64 / r).round() as i64, (c.1 as f64 / r).round() as i64);
            // Nearest open cell, ring by ring; among equals, the one furthest up.
            let mut placed = false;
            'search: for ring in 0..12i64 {
                let mut ring_cells = Vec::new();
                for dy in -ring..=ring {
                    for dx in -ring..=ring {
                        if dx.abs() == ring || dy.abs() == ring {
                            ring_cells.push((c.0 + dx, c.1 + dy));
                        }
                    }
                }
                ring_cells.sort_by_key(|t| (-((t.0 - c.0) * up.0 + (t.1 - c.1) * up.1), t.0, t.1));
                for t in ring_cells {
                    if !w.in_bounds(t.0, t.1) || w.material(t.0, t.1) != VACUUM {
                        continue;
                    }
                    let ok = if l.wreck { w.deposit_cell(t.0, t.1, m) } else { w.land_cell(t.0, t.1, m) };
                    if ok {
                        self.stats.settle_rings += ring as u64;
                        self.stats.settle_ring_max = self.stats.settle_ring_max.max(ring as u64);
                        ev.landed.push((t, m));
                        self.stale(t);
                        placed = true;
                        break 'search;
                    }
                }
            }
            if !placed {
                // Buried in rock: set it down straight above, where the
                // ground opens.
                let (ux, uy) = (c.0 as f64 / r, c.1 as f64 / r);
                for k in 1..100_000i64 {
                    let t = ((c.0 as f64 + 0.5 + ux * k as f64).floor() as i64, (c.1 as f64 + 0.5 + uy * k as f64).floor() as i64);
                    if w.in_bounds(t.0, t.1) && w.material(t.0, t.1) == VACUUM {
                        let ok = if l.wreck { w.deposit_cell(t.0, t.1, m) } else { w.land_cell(t.0, t.1, m) };
                        if ok {
                            ev.landed.push((t, m));
                            self.stale(t);
                            placed = true;
                            break;
                        }
                    }
                }
            }
            if !placed {
                if l.wreck {
                    let d = w.planet.mats.get(m).density;
                    w.ledger.aloft[m as usize] += d;
                }
                w.lose_aloft(m);
            }
            self.stats.cells_landed += 1;
        }
        self.stats.settled += 1;
    }

    /// One physics tick: pull, collide, settle what has come to rest.
    pub fn step(&mut self, w: &mut World) -> Events {
        self.tick += 1;
        let mut ev = Events::default();
        let phys = w.planet.mats.physics.clone();
        if !self.slabs.is_empty() {
            self.step_slabs(w, &mut ev, 1.0 / phys.ticks_per_s);
        }
        if !self.drops.is_empty() {
            self.step_drops(w, &mut ev, 1.0 / phys.ticks_per_s);
        }
        if self.loose.is_empty() {
            return ev;
        }
        // Out of sight, a body isn't tumbled through the air step by step:
        // it is set down at once on the ground below it. (Zoomed out too,
        // with nowhere looked at closely: too small to see tumble.)
        let mut i = 0;
        while i < self.loose.len() {
            let l = &self.loose[i];
            let f = &self.frames[&l.frame];
            let t = f.phys.bodies[l.handle].translation();
            let c = ((t.x as f64 + f.origin.0).floor() as i64, (t.y as f64 + f.origin.1).floor() as i64);
            if crate::sandbox::in_attention(self.attention, c) {
                i += 1;
            } else {
                self.settle_at(w, i, &mut ev, true);
                self.stats.settled += 1;
            }
        }
        if self.loose.is_empty() {
            return ev;
        }
        // Only frames with bodies in the air do any work.
        let mut active: Vec<Cell> = self.loose.iter().map(|l| l.frame).collect();
        active.sort();
        active.dedup();
        for &fk in &active {
            self.refresh_ground(w, fk);
        }
        for l in &self.loose {
            let f = self.frames.get_mut(&l.frame).unwrap();
            let b = &mut f.phys.bodies[l.handle];
            // Pull toward the body's centre. In the central frame the frame's
            // own coordinates are world metres; elsewhere they're worked out
            // in 64-bit.
            // The pull it feels where it is (worked out in 64-bit).
            let c = b.center_of_mass();
            let (cx, cy) = (c.x as f64 + f.origin.0, c.y as f64 + f.origin.1);
            let (ax, ay) = pull(&w.planet, cx, cy);
            let m = b.mass() as f64;
            let force = Vector::new((ax * m) as Real, (ay * m) as Real);
            b.reset_forces(false);
            b.add_force(force, true);
        }
        for &fk in &active {
            self.frames.get_mut(&fk).unwrap().phys.step();
        }
        let dt = 1.0 / phys.ticks_per_s;
        let mut i = 0;
        while i < self.loose.len() {
            let l = &self.loose[i];
            let f = &self.frames[&l.frame];
            let b = &f.phys.bodies[l.handle];
            let speed = b.linvel().length() as f64;
            if !l.wreck && crushes(speed, l.cells.len(), phys.crush_speed_m_s) {
                for ((ci, cj), _) in &l.cells {
                    let p = self.world_point(l, Vector::new(*ci as Real + 0.5, *cj as Real + 0.5));
                    ev.crushing.insert((p.0.floor() as i64, p.1.floor() as i64));
                }
            }
            // A hard knock (a sudden loss of speed): soft rock shatters into
            // single pieces; hard rock knocks loose softer ground it hits.
            let knock = l.last_speed - speed;
            if !l.wreck && l.age > 3 && l.cells.len() > 1 && knock > SMASH_M_S.min(0.5 * l.last_speed).max(0.3) && l.last_speed > 0.6 {
                if l.soft {
                    let l = self.loose.remove(i);
                    self.burst(w, &mut ev, l, knock);
                    continue;
                }
                let cells: Vec<Cell> = l
                    .cells
                    .iter()
                    .map(|((ci, cj), _)| {
                        let p = self.world_point(l, Vector::new(*ci as Real + 0.5, *cj as Real + 0.5));
                        (p.0.floor() as i64, p.1.floor() as i64)
                    })
                    .collect();
                let h = mean_hardness(w, l.cells.iter().map(|c| c.1));
                let mut struck = Vec::new();
                for c in &cells {
                    for d in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        let n = (c.0 + d.0, c.1 + d.1);
                        if !cells.contains(&n) && w.material(n.0, n.1) != VACUUM {
                            struck.push(n);
                        }
                    }
                }
                struck.sort();
                struck.dedup();
                let left = self.loose[i].gouge_left;
                let n = self.gouge(w, &mut ev, &struck, h, knock * 2.0, left);
                self.loose[i].gouge_left -= n;
            }
            self.loose[i].last_speed = speed;
            self.loose[i].age = self.loose[i].age.saturating_add(1);
            let (l, f) = (&self.loose[i], &self.frames[&self.loose[i].frame]);
            let b = &f.phys.bodies[l.handle];
            let still = speed < 0.1 && (b.angvel() as f64).abs() < 0.1;
            let sleeping = b.is_sleeping();
            let t = b.translation();
            let (wx, wy) = (t.x as f64 + f.origin.0, t.y as f64 + f.origin.1);
            self.loose[i].still_s = if still { self.loose[i].still_s + dt } else { 0.0 };
            let far = (wx * wx + wy * wy).sqrt() > w.planet.max_radius * 3.0;
            if far {
                // Drifted off for good.
                let l = self.loose.remove(i);
                self.frames.get_mut(&l.frame).unwrap().phys.remove_body(l.handle);
                for (_, m) in l.cells {
                    if l.wreck {
                        // (Scrap was booked as deposited; it leaves again.)
                        let d = w.planet.mats.get(m).density;
                        w.ledger.aloft[m as usize] += d;
                    }
                    w.lose_aloft(m);
                }
                continue;
            }
            if self.loose[i].still_s >= phys.settle_s || sleeping {
                self.settle(w, i, &mut ev);
            } else {
                i += 1;
            }
        }
        ev
    }

    /// A soft body bursts into single pieces, scattering.
    fn burst(&mut self, w: &mut World, ev: &mut Events, l: Loose, knock: f64) {
        let f = self.frames.get_mut(&l.frame).unwrap();
        let pose = *f.phys.bodies[l.handle].position();
        let v = f.phys.bodies[l.handle].linvel();
        let origin = f.origin;
        f.phys.remove_body(l.handle);
        for (k, ((ci, cj), m)) in l.cells.into_iter().enumerate() {
            let p = pose * Vector::new(ci as Real + 0.5, cj as Real + 0.5);
            let c = ((p.x as f64 + origin.0).floor() as i64, (p.y as f64 + origin.1).floor() as i64);
            let j = kick(c, (knock * (0.15 + 0.05 * (k % 5) as f64)).min(3.0));
            self.piece(w, ev, &[(c, m)], (v.x as f64 * 0.3 + j.0, v.y as f64 * 0.3 + j.1), j.0 * 3.0);
        }
        self.stats.shattered += 1;
    }

    /// Set every body down now (before saving, or at the end of a run).
    pub fn settle_all(&mut self, w: &mut World) -> Events {
        let mut ev = Events::default();
        while !self.loose.is_empty() {
            self.settle(w, 0, &mut ev);
        }
        // Slabs drop straight down where they are (a long fall in one go).
        while !self.slabs.is_empty() {
            let n = self.slabs.len();
            for sl in &mut self.slabs {
                let (cx, cy) = (sl.origin.0 as f64 + sl.middle.0, sl.origin.1 as f64 + sl.middle.1);
                let r = (cx * cx + cy * cy).sqrt().max(1.0);
                sl.vel = (-cx / r * 50.0, -cy / r * 50.0);
            }
            for _ in 0..100_000 {
                if self.slabs.len() < n {
                    break;
                }
                self.step_slabs(w, &mut ev, 0.1);
            }
            if self.slabs.len() >= n {
                // Stuck somehow: lose it rather than loop.
                let sl = self.slabs.pop().unwrap();
                for (_, m) in sl.cells {
                    w.lose_aloft(m);
                }
            }
        }
        ev
    }

    /// A number that changes if any body moves.
    pub fn hash(&self) -> u64 {
        let mut h = 0u64;
        for sl in &self.slabs {
            h = crate::noise::hash(h, sl.at.0, sl.at.1, 0x51AB);
        }
        for l in &self.loose {
            let b = &self.frames[&l.frame].phys.bodies[l.handle];
            h = crate::noise::hash(h, b.translation().x.to_bits() as i64, b.translation().y.to_bits() as i64, 0xB0D1);
        }
        h
    }
}
