//! Life (L1e): the plants' life cycle, run by the swarm's simulation once a
//! second.
//!
//! The flora (src/flora.rs) lays each body's plants over the generator. From
//! then on they are ordinary cells, and this is what happens to them:
//! - **Dying:** living tissue a falling body sets down (a tree whose trunk
//!   was cut, a bush whose ground fell away) is cut off from its roots. After
//!   `wilt_s` it dies: wood and roots to deadwood, leaves, flowers and fruit
//!   to leaf litter. Living crystal doesn't die.
//! - **Rotting:** deadwood breaks down into leaf litter, and leaf litter into
//!   humus, which is soil: a fallen forest becomes ground for the next.
//! - **Spreading back:** where living tissue is taken away (cut, eaten,
//!   burned, blown up), the place is remembered in 8 m squares. Every
//!   `regrow_s` or so (sooner for fast growers) a square is looked at: if
//!   anything still lives within a square of it, that life spreads into the
//!   open ground there (ground cover, and now and then a new plant of a
//!   species growing next to it). A square that grew anything wakes the
//!   squares round it, so green creeps out over eaten ground; a square is
//!   let go once it is full, or once nothing near it is alive (dead wood and
//!   litter seed nothing).
//! - **Growing:** a sprout grows to its full size over `mature_s` (divided by
//!   its species' growth), regrown from its seed at its new age every few
//!   seconds and written into whatever open space is still there.
//!
//! Growing takes matter from the air and water plants live on, and wilting
//! and rotting give some back: the difference is booked as `grown` in the
//! ledger. Every random choice is a hash of the place and the tick, so a run
//! is the same every time.

use crate::flora::{Flora, Niche};
use crate::materials::{MatId, State, VACUUM};
use crate::noise::{hash, unit};
use crate::sandbox::Sandbox;
use crate::save::{R, W};
use crate::swarm::CellSet;
use crate::world::World;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Side of the squares places are remembered in, in cells.
pub const SQUARE: i64 = 8;
/// Sprouts grown again per second, at most.
const SPROUTS_PER_S: usize = 80;
/// Squares regrown per second, at most.
const SCARS_PER_S: usize = 60;
/// Dead cells rotted per second, at most.
const ROTS_PER_S: usize = 5000;
/// A sprout is grown again once it has aged this much (0 to 1).
const AGE_STEP: f64 = 0.04;
/// A place drones are still at work on is looked at again after this (s).
const BUSY_WAIT_S: f64 = 60.0;
/// A square with room left but nothing new grown is let go after this many
/// looks.
const MAX_TRIES: u16 = 12;
/// A square holding this many cells of standing plants is wooded enough: no
/// new plants sprout in it (about one grown tree).
const WOODED_CELLS: u32 = 60;
/// New plants per square, at most.
const PLANTS_PER_SQUARE: usize = 2;
const NONE: u16 = u16::MAX;

/// What looking at a square came to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Spread {
    /// No ground in it to grow on (open air, or solid rock).
    NoGround,
    /// Nothing alive within a square of it.
    NoLife,
    /// Something grew.
    Grew,
    /// Open ground life near by could take, but none did this time.
    Room,
    /// No open ground left that life near by could take.
    Full,
}

/// A plant growing back.
#[derive(Clone, Debug)]
struct Sprout {
    species: u16,
    x: i64,
    y: i64,
    up: (i8, i8),
    seed: u64,
    /// The tick it sprouted.
    born: u64,
    /// The size it will stop at (0 to 1), and the age it was last grown to.
    target: f64,
    grown: f64,
    /// The ground it stands on (it dies if that goes), and what it grows
    /// into (open space, or the water it lives in).
    ground: MatId,
    medium: MatId,
    soil: bool,
}

/// What the life cycle has done, for the developer panel and tests.
#[derive(Clone, Debug, Default)]
pub struct LifeStats {
    /// Places that grew back, plants sprouted and plants grown to full size.
    pub regrown: u64,
    pub sprouted: u64,
    pub matured: u64,
    /// Sprouts that lost their ground before they were grown.
    pub lost: u64,
    /// Cells written by growing (cover and plants), cells that died and
    /// cells that rotted a stage further.
    pub cells_grown: u64,
    pub cells_died: u64,
    pub cells_rotted: u64,
}

pub struct Life {
    /// Per material: the species whose living tissue it is (NONE if none),
    /// and what it turns into when it dies or rots (VACUUM: nothing).
    species_of: Vec<u16>,
    next_of: Vec<MatId>,
    /// Seconds after it forms before each material dies or rots further.
    delay_of: Vec<f64>,
    is_tissue: Vec<bool>,
    /// Places waiting to grow back: (due tick, square x, square y, looks
    /// with nothing grown).
    scars: BinaryHeap<Reverse<(u64, i64, i64, u16)>>,
    waiting: CellSet,
    sprouts: Vec<Sprout>,
    cursor: usize,
    /// Dead cells waiting to rot: (due tick, x, y, material).
    rotting: BinaryHeap<Reverse<(u64, i64, i64, MatId)>>,
    pub stats: LifeStats,
}

impl Life {
    /// The life cycle for a world's flora; None if nothing lives there.
    pub fn new(world: &World) -> Option<Life> {
        let p = &world.planet;
        let f = p.flora.as_ref()?;
        if f.species.is_empty() {
            return None;
        }
        let mats = &p.mats;
        let n = mats.len();
        let cycle = &f.rules.cycle;
        let (deadwood, litter, humus) = (mats.id("deadwood").ok()?, mats.id("leaf litter").ok()?, mats.id("humus").ok()?);
        let crystal = mats.id("living crystal").ok();
        let mut species_of = vec![NONE; n];
        let mut next_of = vec![VACUUM; n];
        let mut delay_of = vec![0.0; n];
        for (i, sp) in f.species.iter().enumerate() {
            for (k, m) in [sp.stem_m, sp.foliage_m, sp.organ_m, sp.root_m].into_iter().enumerate() {
                if m == VACUUM {
                    continue;
                }
                species_of[m as usize] = i as u16;
                // Living crystal never dies.
                if Some(sp.parts[k].0) == crystal {
                    continue;
                }
                next_of[m as usize] = if k == 0 || k == 3 { deadwood } else { litter };
                delay_of[m as usize] = cycle.wilt_s;
            }
        }
        next_of[deadwood as usize] = litter;
        delay_of[deadwood as usize] = cycle.rot_s;
        next_of[litter as usize] = humus;
        delay_of[litter as usize] = cycle.humus_s;
        let is_tissue = species_of.iter().map(|&s| s != NONE).collect();
        Some(Life {
            species_of,
            next_of,
            delay_of,
            is_tissue,
            scars: BinaryHeap::new(),
            waiting: CellSet::default(),
            sprouts: Vec::new(),
            cursor: 0,
            rotting: BinaryHeap::new(),
            stats: LifeStats::default(),
        })
    }

    /// Which materials the world should watch (living tissue).
    pub fn watch(&self) -> Vec<bool> {
        self.is_tissue.clone()
    }

    /// Places waiting to regrow, plants growing and dead cells rotting.
    pub fn counts(&self) -> (usize, usize, usize) {
        (self.scars.len(), self.sprouts.len(), self.rotting.len())
    }

    /// A second's work at tick `tick` (`tick_s` seconds a tick). `busy`
    /// says whether drones are still at work in or round a square: nothing
    /// grows back there until they have moved on.
    pub fn work(&mut self, world: &mut World, sandbox: &mut Sandbox, tick: u64, tick_s: f64, busy: &dyn Fn(i64, i64) -> bool) {
        let planet = world.planet.clone();
        let Some(f) = planet.flora.as_ref() else { return };
        let cycle = f.rules.cycle.clone();
        let ticks = |s: f64| (s / tick_s).ceil().max(1.0) as u64;

        // What happened to living cells since the last look.
        let notes = std::mem::take(&mut world.life_notes);
        for (x, y, m) in notes.removed {
            let sq = (x.div_euclid(SQUARE), y.div_euclid(SQUARE));
            let si = self.species_of.get(m as usize).copied().unwrap_or(NONE);
            if si == NONE || self.waiting.contains(&sq) || self.scars.len() >= cycle.max_scars {
                continue;
            }
            let growth = f.species[si as usize].growth.max(0.1);
            let jitter = 0.6 + 0.8 * unit(hash(f.seed, sq.0, sq.1, tick ^ 0x5CA2));
            self.waiting.insert(sq);
            self.scars.push(Reverse((tick + ticks(cycle.regrow_s / growth * jitter), sq.0, sq.1, 0)));
        }
        for (x, y, m) in notes.landed {
            self.rot_later(x, y, m, tick, &ticks, cycle.max_rotting);
        }

        // Dying and rotting.
        let mut done = 0;
        while let Some(&Reverse((due, x, y, m))) = self.rotting.peek() {
            if due > tick || done >= ROTS_PER_S {
                break;
            }
            self.rotting.pop();
            done += 1;
            let Some(c) = find_near(world, x, y, m) else { continue };
            let to = self.next_of[m as usize];
            if to == VACUUM {
                continue;
            }
            world.life_cell(c.0, c.1, to);
            sandbox.wake_around(c);
            if self.is_tissue[m as usize] {
                self.stats.cells_died += 1;
            } else {
                self.stats.cells_rotted += 1;
            }
            self.rot_later(c.0, c.1, to, tick, &ticks, cycle.max_rotting);
        }

        // Places growing back.
        let mut done = 0;
        let mut later = Vec::new();
        let mut woken: Vec<(i64, i64)> = Vec::new();
        let again = |sq: (i64, i64), tries: u16| {
            let jitter = 0.6 + 0.8 * unit(hash(f.seed ^ tick, sq.0, sq.1, 0xA6A1));
            Reverse((tick + ticks(cycle.regrow_s * 0.5 * jitter), sq.0, sq.1, tries))
        };
        while let Some(&Reverse((due, sx, sy, tries))) = self.scars.peek() {
            if due > tick || done >= SCARS_PER_S {
                break;
            }
            self.scars.pop();
            done += 1;
            if busy(sx, sy) {
                // The swarm is still eating here: look again in a while.
                later.push(Reverse((tick + ticks(BUSY_WAIT_S), sx, sy, tries)));
                continue;
            }
            // Taken from a crown or a trunk, high over the ground: it grows
            // back on the ground below (the first square down with any).
            let (cx, cy) = ((sx * SQUARE + SQUARE / 2) as f64, (sy * SQUARE + SQUARE / 2) as f64);
            let r = (cx * cx + cy * cy).sqrt().max(1e-9);
            let mut outcome = Spread::NoGround;
            let mut at = (sx, sy);
            for k in 0..12 {
                let q = (((cx - cx / r * (k * SQUARE) as f64) / SQUARE as f64).floor() as i64, ((cy - cy / r * (k * SQUARE) as f64) / SQUARE as f64).floor() as i64);
                outcome = self.regrow(world, f, q.0, q.1, tick, cycle.max_sprouts);
                if outcome != Spread::NoGround {
                    at = q;
                    break;
                }
            }
            self.waiting.remove(&(sx, sy));
            match outcome {
                Spread::Grew => {
                    // Still room? Look again; and wake the squares round it.
                    if !self.waiting.contains(&at) {
                        self.waiting.insert(at);
                        later.push(again(at, 0));
                    }
                    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
                        woken.push((at.0 + dx, at.1 + dy));
                    }
                }
                Spread::Room if tries < MAX_TRIES => {
                    if !self.waiting.contains(&at) {
                        self.waiting.insert(at);
                        later.push(again(at, tries + 1));
                    }
                }
                _ => {}
            }
        }
        for sq in woken {
            if self.scars.len() + later.len() >= cycle.max_scars {
                break;
            }
            if self.waiting.insert(sq) {
                later.push(again(sq, 0));
            }
        }
        self.scars.extend(later);

        // Sprouts growing.
        let n = self.sprouts.len();
        let mut i = self.cursor.min(n);
        let mut looked = 0;
        while looked < SPROUTS_PER_S.min(n) && !self.sprouts.is_empty() {
            if i >= self.sprouts.len() {
                i = 0;
            }
            looked += 1;
            let s = self.sprouts[i].clone();
            // Its ground is gone (eaten, fallen, melted)? Its own roots
            // growing into it don't count.
            let g = world.material(s.x, s.y);
            if g == VACUUM || !matches!(planet.mats.get(g).state, State::Solid | State::Powder) || (g != s.ground && self.species_of[g as usize] != s.species) {
                self.stats.lost += 1;
                self.sprouts.swap_remove(i);
                continue;
            }
            let sp = &f.species[s.species as usize];
            let mature = ticks(cycle.mature_s / sp.growth.max(0.1)) as f64;
            let age = (0.06 + (tick.saturating_sub(s.born)) as f64 / mature).min(s.target);
            let last = age >= s.target;
            if age - s.grown >= AGE_STEP || last {
                let cells = {
                    // It grows as if from nothing again: its own cells are
                    // open space (or the ground its roots are in).
                    let w: &World = world;
                    let own = &self.species_of;
                    let root = sp.root_m;
                    f.grow_at(&planet, s.species as usize, s.x, s.y, s.up, s.seed, age, s.soil, &|x, y| {
                        let m = w.material(x, y);
                        if own[m as usize] == s.species {
                            if m == root { s.ground } else { s.medium }
                        } else {
                            m
                        }
                    })
                };
                let wet = sp.niche.wet() || sp.niche == Niche::Hot;
                for (x, y, m, _) in cells {
                    let cur = world.material(x, y);
                    let room = cur == VACUUM || (wet && f.living(cur)) || (m == sp.root_m && f.is_soil(cur) && !self.tissue(cur));
                    if room && world.life_cell(x, y, m).is_some() {
                        self.stats.cells_grown += 1;
                    }
                }
                self.sprouts[i].grown = age;
            }
            if last {
                self.stats.matured += 1;
                self.sprouts.swap_remove(i);
            } else {
                i += 1;
            }
        }
        self.cursor = i;
    }

    fn tissue(&self, m: MatId) -> bool {
        self.is_tissue.get(m as usize).copied().unwrap_or(false)
    }

    fn rot_later(&mut self, x: i64, y: i64, m: MatId, tick: u64, ticks: &dyn Fn(f64) -> u64, max: usize) {
        let mi = m as usize;
        if mi >= self.next_of.len() || self.next_of[mi] == VACUUM || self.rotting.len() >= max {
            return;
        }
        let jitter = 0.7 + 0.6 * unit(hash(tick, x, y, 0x207));
        self.rotting.push(Reverse((tick + ticks(self.delay_of[mi] * jitter), x, y, m)));
    }

    /// Let the life round square (sx, sy) spread into it: ground cover on
    /// open ground, and now and then a new plant, of species living within
    /// a square of it.
    fn regrow(&mut self, world: &mut World, f: &Flora, sx: i64, sy: i64, tick: u64, max_sprouts: usize) -> Spread {
        let planet = world.planet.clone();
        let p = &*planet;
        // The ground of the square, in an order of its own: (hash, cell,
        // ground, facing up?).
        let h0 = hash(f.seed ^ tick, sx, sy, 0x6E0);
        let mut cells: Vec<(u64, i64, i64)> = (0..SQUARE * SQUARE).map(|k| (hash(h0, k, 0, 0), sx * SQUARE + k % SQUARE, sy * SQUARE + k / SQUARE)).collect();
        cells.sort_unstable();
        let solid = |m: MatId| m != VACUUM && matches!(p.mats.get(m).state, State::Solid | State::Powder);
        let mut ground = Vec::new();
        for &(h, x, y) in &cells {
            let m = world.material(x, y);
            // Ground: bare, or under ground cover (cover is the ground's top
            // layer, as the flora lays it; plants root through it).
            if !solid(m) || (self.tissue(m) && !self.is_cover(f, m)) {
                continue;
            }
            let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
            let r = (fx * fx + fy * fy).sqrt().max(1e-9);
            let up = (step(fx / r), step(fy / r));
            // Facing up (a floor) or down (a roof), into what?
            for (dir, roof) in [(up, false), ((-up.0, -up.1), true)] {
                let o = world.material(x + dir.0, y + dir.1);
                if o == VACUUM || f.living(o) {
                    ground.push((h, x, y, m, dir, roof, o));
                    break;
                }
            }
        }
        if ground.is_empty() {
            return Spread::NoGround;
        }
        // What lives within a square of it, and how much standing plant is
        // in the square itself already.
        let mut near: Vec<(usize, u32)> = Vec::new();
        let mut standing_here = 0;
        for y in (sy - 1) * SQUARE..(sy + 2) * SQUARE {
            for x in (sx - 1) * SQUARE..(sx + 2) * SQUARE {
                let si = self.species_of.get(world.material(x, y) as usize).copied().unwrap_or(NONE);
                if si == NONE {
                    continue;
                }
                if !f.species[si as usize].cover && x.div_euclid(SQUARE) == sx && y.div_euclid(SQUARE) == sy {
                    standing_here += 1;
                }
                match near.iter_mut().find(|e| e.0 == si as usize) {
                    Some(e) => e.1 += 1,
                    None => near.push((si as usize, 1)),
                }
            }
        }
        if near.is_empty() {
            return Spread::NoLife;
        }
        near.sort_unstable();
        // Wooded enough already: no more new plants here.
        let wooded = standing_here >= WOODED_CELLS;
        let mut planted = 0;
        let mut grew = false;
        let mut room = false;
        for (h, x, y, m, dir, roof, o) in ground {
            let k = p.ambient_k(x, y);
            let covered = self.is_cover(f, m);
            // Soil, or ground cover of soil life over it.
            let soil = f.is_soil(m) || (covered && f.species[self.species_of[m as usize] as usize].niche == Niche::Soil);
            // The species near by that could live on this bit of ground.
            let fits = |i: usize| {
                let s = &f.species[i];
                let n = s.niche;
                (n == Niche::CaveRoof) == roof && (n.wet() == f.living(o) || n == Niche::Hot) && (n != Niche::Soil || soil) && k >= s.min_k && k <= s.max_k
            };
            let mut standing: Vec<(usize, f64)> = near.iter().filter(|e| !f.species[e.0].cover && fits(e.0)).map(|e| (e.0, e.1 as f64)).collect();
            let mut cover: Vec<(usize, f64)> = near.iter().filter(|e| f.species[e.0].cover && fits(e.0)).map(|e| (e.0, e.1 as f64)).collect();
            if standing.is_empty() && cover.is_empty() {
                // Nothing near by suits this ground (forest soil life next to
                // rock the soil was eaten off): the life this body has for
                // such ground takes it instead.
                let niche = if roof {
                    Niche::CaveRoof
                } else if f.living(o) {
                    if f.cover_of(Niche::Shallows).is_some() { Niche::Shallows } else { Niche::Deeps }
                } else if soil {
                    Niche::Soil
                } else {
                    Niche::Bare
                };
                standing = f.standing_of(niche).into_iter().filter(|&(i, _)| fits(i)).collect();
                cover = f.cover_of(niche).into_iter().filter(|&i| fits(i)).map(|i| (i, 1.0)).collect();
                if standing.is_empty() && cover.is_empty() {
                    continue;
                }
            }
            let niche = f.species[standing.first().or(cover.first()).unwrap().0].niche;
            let (dens, cov) = niche.density();
            let may_plant = !wooded && !standing.is_empty();
            if !covered && !cover.is_empty() || may_plant {
                room = true;
            }
            // A new plant here?
            if may_plant && planted < PLANTS_PER_SQUARE && self.sprouts.len() < max_sprouts && unit(hash(h, 1, 0, 0)) < (dens * f.scale * 1.5).min(0.5) {
                if let Some(pick) = pick(&standing, unit(hash(h, 2, 0, 0))) {
                    planted += 1;
                    self.stats.sprouted += 1;
                    self.sprouts.push(Sprout {
                        species: pick as u16,
                        x,
                        y,
                        up: (dir.0 as i8, dir.1 as i8),
                        seed: hash(h, 3, 0, 0),
                        born: tick,
                        target: 0.35 + 0.65 * unit(hash(h, 4, 0, 0)).sqrt(),
                        grown: 0.0,
                        ground: m,
                        medium: o,
                        soil,
                    });
                    grew = true;
                    continue;
                }
            }
            // Ground cover: the ground's top layer turns to it.
            if !covered {
                if let Some(ci) = pick(&cover, unit(hash(h, 6, 0, 0))) {
                    if unit(hash(h, 5, 0, 0)) < cov && world.life_cell(x, y, f.species[ci].foliage_m).is_some() {
                        self.stats.cells_grown += 1;
                        grew = true;
                    }
                }
            }
        }
        if grew {
            self.stats.regrown += 1;
            Spread::Grew
        } else if room {
            Spread::Room
        } else {
            Spread::Full
        }
    }

    /// Whether `m` is ground cover (the living top layer of the ground).
    fn is_cover(&self, f: &Flora, m: MatId) -> bool {
        match self.species_of.get(m as usize).copied() {
            Some(si) if si != NONE => f.species[si as usize].cover,
            _ => false,
        }
    }

    pub fn write(&self, o: &mut W) {
        let mut scars: Vec<(u64, i64, i64, u16)> = self.scars.iter().map(|r| r.0).collect();
        scars.sort_unstable();
        o.u32(scars.len() as u32);
        for (t, x, y, s) in scars {
            o.u64(t);
            o.i64(x);
            o.i64(y);
            o.u16(s);
        }
        o.u32(self.sprouts.len() as u32);
        for s in &self.sprouts {
            o.u16(s.species);
            o.i64(s.x);
            o.i64(s.y);
            o.u8(s.up.0 as u8);
            o.u8(s.up.1 as u8);
            o.u64(s.seed);
            o.u64(s.born);
            o.f64(s.target);
            o.f64(s.grown);
            o.u16(s.ground);
            o.u16(s.medium);
            o.u8(s.soil as u8);
        }
        let mut rot: Vec<(u64, i64, i64, MatId)> = self.rotting.iter().map(|r| r.0).collect();
        rot.sort_unstable();
        o.u32(rot.len() as u32);
        for (t, x, y, m) in rot {
            o.u64(t);
            o.i64(x);
            o.i64(y);
            o.u16(m);
        }
        let st = &self.stats;
        for v in [st.regrown, st.sprouted, st.matured, st.lost, st.cells_grown, st.cells_died, st.cells_rotted] {
            o.u64(v);
        }
    }

    pub fn read(&mut self, r: &mut R) -> Result<(), String> {
        let nm = self.species_of.len();
        for _ in 0..r.u32()? {
            let (t, x, y, s) = (r.u64()?, r.i64()?, r.i64()?, r.u16()?);
            self.waiting.insert((x, y));
            self.scars.push(Reverse((t, x, y, s)));
        }
        for _ in 0..r.u32()? {
            let s = Sprout {
                species: r.u16()?,
                x: r.i64()?,
                y: r.i64()?,
                up: (r.u8()? as i8, r.u8()? as i8),
                seed: r.u64()?,
                born: r.u64()?,
                target: r.f64()?,
                grown: r.f64()?,
                ground: r.u16()?,
                medium: r.u16()?,
                soil: r.u8()? != 0,
            };
            self.sprouts.push(s);
        }
        for _ in 0..r.u32()? {
            let (t, x, y, m) = (r.u64()?, r.i64()?, r.i64()?, r.u16()?);
            if (m as usize) < nm {
                self.rotting.push(Reverse((t, x, y, m)));
            }
        }
        let st = &mut self.stats;
        for v in [&mut st.regrown, &mut st.sprouted, &mut st.matured, &mut st.lost, &mut st.cells_grown, &mut st.cells_died, &mut st.cells_rotted] {
            *v = r.u64()?;
        }
        Ok(())
    }

    /// Somewhere to see regrowth: the newest plant growing back that is big
    /// enough to see (else the newest of all). None if nothing is growing.
    pub fn showcase(&self) -> Option<(f64, f64)> {
        let seen = self.sprouts.iter().filter(|s| s.grown >= 0.15).max_by_key(|s| (s.born, s.x, s.y));
        seen.or_else(|| self.sprouts.iter().max_by_key(|s| (s.born, s.x, s.y))).map(|s| (s.x as f64 + 0.5, s.y as f64 + 0.5))
    }

    /// Lines for the developer panel.
    pub fn lines(&self) -> Vec<String> {
        let st = &self.stats;
        vec![
            format!("Life: {} places waiting to grow back, {} plants growing, {} dead cells rotting", self.scars.len(), self.sprouts.len(), self.rotting.len()),
            format!("  grown back {} places · sprouted {} · full size {} · lost {}", st.regrown, st.sprouted, st.matured, st.lost),
            format!("  cells grown {} · died {} · rotted {}", st.cells_grown, st.cells_died, st.cells_rotted),
        ]
    }
}

/// The cell holding `m` at (x, y), or a few cells below it (dead leaves and
/// litter are powder: they may have fallen).
fn find_near(world: &World, x: i64, y: i64, m: MatId) -> Option<(i64, i64)> {
    if world.material(x, y) == m {
        return Some((x, y));
    }
    let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
    let r = (fx * fx + fy * fy).sqrt().max(1e-9);
    let (dx, dy) = (-step(fx / r), -step(fy / r));
    (1..=6).map(|k| (x + dx * k, y + dy * k)).find(|c| world.material(c.0, c.1) == m)
}

/// A unit component rounded to -1, 0 or 1 (as the flora does).
fn step(u: f64) -> i64 {
    if u > 0.38 {
        1
    } else if u < -0.38 {
        -1
    } else {
        0
    }
}

/// A weighted pick, `u` from 0 to 1.
fn pick(list: &[(usize, f64)], u: f64) -> Option<usize> {
    let total: f64 = list.iter().map(|e| e.1).sum();
    if list.is_empty() || total <= 0.0 {
        return None;
    }
    let mut r = u * total;
    for &(i, w) in list {
        if r < w {
            return Some(i);
        }
        r -= w;
    }
    Some(list[list.len() - 1].0)
}
