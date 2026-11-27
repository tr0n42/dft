//! Sappers: carry a charge (10 kt) to where the going is hardest, hover
//! along to a way down (cracks and caves) or bore one, until deep enough,
//! and go off. The blast (rays, cracks, held
//! chunks, a molten heart, rubble, thrown rock and a plume), rubble
//! settling, chain blasts, and the charges themselves (nuclear, fused
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

/// Rubble cells moved per tick, at most.
pub(super) const SETTLE_PER_TICK: usize = 20_000;

/// The heart of a sapper's blast is heated to this (K): it melts, and cools
/// to glass after about this long (s; it was 25, and Benjamin, 26 Nov: the
/// blast's glow lingered far too long).
pub(super) const MELT_K: f64 = 2000.0;

pub(super) const MELT_COOL_S: f64 = 6.0;

/// Ground this hot next to a sapper (a blast's heat) destroys it, and so sets it off.
pub(super) const SAPPER_COOK_K: f64 = 800.0;

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
    /// off, broken rock pouring out of a plume, and loose rubble and melt
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
    /// until something beside them opens.
    pub(super) fn settle(&mut self) {
        let mut cells: Vec<(i64, i64)> = self.settling.iter().copied().collect();
        // Nearest the centre first (the proving ground goes bottom up).
        cells.sort_unstable_by_key(|c| (c.0 * c.0 + c.1 * c.1, c.0, c.1));
        self.settling = CellSet::default();
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

    /// A sapper goes off at `at`, exactly as the sapper proving ground
    /// (mockups/sapper-proving-ground.html) has it:
    /// - rays out from the charge, each spending its energy on what it passes
    ///   through (little in open passages and rubble, far more in hard rock);
    ///   everything a ray gets through is broken; where one gives out in
    ///   rock, now and then a crack runs on, further in harder rock;
    /// - hard rock away from the heart does not always go to rubble: some of
    ///   it holds together in big pieces, split off along seams (cracked);
    /// - the heart of it melts; of the rest, what the blast still had plenty
    ///   of energy for is thrown clear (softer rock harder), the rest left
    ///   loose where it was, as rubble;
    /// - a plume: if there is an open way from the blast up to the surface
    ///   (cracks, caves, its drill hole, through the rubble), most blasts
    ///   drive a share of the rubble up it and out of the top;
    /// - sappers caught in it go off too.
    /// Rubble, melt and thrown rock are loose: cutters pick them up quickly.
    /// `fissile`: kilograms of uranium in its charge (0: conventional);
    /// what didn't react is partly left in the rubble.
    pub fn sapper_blast(&mut self, at: (f64, f64), yield_kt: f64, fissile: f64) {
        let q0 = clock();
        self.sapper_blast_inner(at, yield_kt, fissile);
        self.prof2[7] += clock() - q0;
    }

    pub(super) fn sapper_blast_inner(&mut self, at: (f64, f64), yield_kt: f64, fissile: f64) {
        let sp = &self.eco.units.sapper;
        let r = sp.radius_m_per_cbrt_kt * yield_kt.max(0.0).cbrt();
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
        let hard = |s: &Self, m: MatId| (s.world.planet.mats.get(m).hardness as f64 / 10.0).clamp(0.05, 0.95);
        // The rays.
        let rays = ((r * 10.0).round() as usize).clamp(360, 5000);
        let reach = (r * 2.4).min(1500.0) as i64;
        let mut broken: CellMap<(f64, f64)> = CellMap::default();
        let mut order: Vec<(i64, i64)> = Vec::new();
        let mut cracked = 0u64;
        // Where each ray stopped breaking rock (the shock cracks it further).
        let mut stop = vec![reach as f64; rays];
        for ray in 0..rays {
            let a = ray as f64 / rays as f64 * std::f64::consts::TAU + (crate::noise::unit(hash(blast, ray as i64, 0, 5)) - 0.5) * 0.01;
            let (ca, sa) = crate::noise::cos_sin(a);
            let mut e = 1.0;
            for step in 0..reach {
                let c = ((at.0 + ca * step as f64).round() as i64, (at.1 + sa * step as f64).round() as i64);
                let m = self.world.material(c.0, c.1);
                if m == VACUUM {
                    e -= 0.12 / r;
                    continue;
                }
                if self.loose.contains(&c) || broken.contains_key(&c) {
                    e -= 0.3 / r;
                    continue;
                }
                let st = self.world.planet.mats.get(m).state;
                if st == State::Liquid || st == State::Gas {
                    e -= 0.12 / r;
                    continue;
                }
                let h = hard(self, m);
                e -= (0.45 + 1.7 * h) / r;
                if e <= 0.0 {
                    stop[ray] = step as f64;
                    // A crack may run on, wandering, further in harder rock.
                    if ray % 4 == 0 {
                        let (mut ax, mut ay, mut aa) = (c.0 as f64, c.1 as f64, a);
                        let len = (r * (0.3 + 0.9 * h)) as i64;
                        for k in 0..len {
                            aa += (crate::noise::unit(hash(blast, ray as i64, k, 9)) - 0.5) * (1.2 - h);
                            let (cc, ss) = crate::noise::cos_sin(aa);
                            ax += cc;
                            ay += ss;
                            let q = (ax.round() as i64, ay.round() as i64);
                            if self.world.material(q.0, q.1) == VACUUM {
                                break;
                            }
                            if self.fractured.insert(q) {
                                cracked += 1;
                            }
                            self.world.set_look(q.0, q.1, Look::CRACKED);
                        }
                    }
                    break;
                }
                broken.insert(c, (e, step as f64));
                order.push(c);
            }
        }
        // What was broken: held in big pieces, melted, thrown clear, or left
        // loose as rubble.
        let piece = |x: i64, y: i64, size: f64| -> u64 {
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
        };
        let mut rubble: Vec<(i64, i64)> = Vec::new();
        let mut thrown = 0u64;
        let mut fling: Vec<((i64, i64), f64, f64)> = Vec::new();
        let power = (0.6 + yield_kt.max(0.0).cbrt() * 0.4).min(3.0);
        for &c in &order {
            let Some(&(e, d)) = broken.get(&c) else { continue };
            let m = self.world.material(c.0, c.1);
            if m == VACUUM {
                continue;
            }
            let h = hard(self, m);
            if h > 0.6 && e < 0.85 && d > r * 0.3 {
                let size = 5.0 + 9.0 * h;
                let id = piece(c.0, c.1, size);
                if crate::noise::unit(hash(id, blast as i64, 0, 61)) < (h - 0.5) * 1.3 {
                    let edge = piece(c.0 + 1, c.1, size) != id || piece(c.0 - 1, c.1, size) != id || piece(c.0, c.1 + 1, size) != id || piece(c.0, c.1 - 1, size) != id;
                    if !edge {
                        // Held together, cracked through, hot.
                        if self.fractured.insert(c) {
                            cracked += 1;
                        }
                        self.world.set_look(c.0, c.1, Look::HELD);
                        continue;
                    }
                }
            }
            // The heart melts.
            if d < r * 0.3 * (1.15 - 0.4 * h) {
                if !seen {
                    // (Out of sight: cooled to glass already.)
                    self.loose.insert(c);
                    self.world.set_look(c.0, c.1, Look::GLASS);
                    continue;
                }
                self.sandbox.heat_cell(&self.world, c, MELT_K);
                self.loose.insert(c);
                self.settling.insert(c);
                self.world.set_look(c.0, c.1, Look::MELT);
                let cool = self.ticks(MELT_COOL_S * (0.7 + 0.6 * crate::noise::unit(hash(blast, c.0, c.1, 21))));
                self.melt.push_back((self.tick + cool, c));
                continue;
            }
            if seen && e > 0.35 + 0.35 * h {
                fling.push((c, e, h));
            } else {
                // Left loose where it was: baked near the heart.
                self.loose.insert(c);
                self.settling.insert(c);
                self.world.set_look(c.0, c.1, if d < r * 0.6 { Look::BAKED } else { Look::RUBBLE });
                rubble.push(c);
            }
        }
        // Thrown clear, out from the blast: at most THROW_MAX cells fly as
        // pieces (a random share, the same every run); the rest of what
        // would have flown stays as rubble, so a blast on screen never
        // stalls the sim flying thousands of pieces.
        if fling.len() > THROW_MAX {
            fling.sort_unstable_by_key(|&(c, _, _)| hash(blast, c.0, c.1, 0x7E40));
            for &(c, e, _) in &fling[THROW_MAX..] {
                let d = broken.get(&c).map(|b| b.1).unwrap_or(r);
                let _ = e;
                self.loose.insert(c);
                self.settling.insert(c);
                self.world.set_look(c.0, c.1, if d < r * 0.6 { Look::BAKED } else { Look::RUBBLE });
                rubble.push(c);
            }
            fling.truncate(THROW_MAX);
        }
        for (c, e, h) in fling {
            {
                let Some(m) = self.world.lift_cell(c.0, c.1) else { continue };
                self.progress.remove(&c);
                self.reserved.remove(&c);
                self.open_cell(c);
                let p = mid(c);
                let dd = ((p.0 - at.0).powi(2) + (p.1 - at.1).powi(2)).sqrt().max(1e-6);
                let (ox, oy) = ((p.0 - at.0) / dd, (p.1 - at.1) / dd);
                let sp = (18.0 + 70.0 * e * (1.0 - 0.6 * h)) * (0.7 + 0.6 * crate::noise::unit(hash(blast, c.0, c.1, 12))) * power;
                let vel = (ox * sp + up.0 * 6.0, oy * sp + up.1 * 6.0);
                thrown += 1;
                if let Some(t) = self.bodies.throw(&mut self.world, p, m, vel) {
                    self.loose.insert(t);
                    self.world.set_look(t.0, t.1, Look::RUBBLE);
                    self.settling.insert(t);
                    self.scrap_landed(t);
                }
            }
        }
        let _ = thrown;
        // The loosened column over it, up to the surface (Benjamin, 25 Nov:
        // the yield sets the depth, deep enough that the ground over it is
        // loosened for the cutters to scoop up).
        let top = if self.eco.units.sapper.depth_m_per_cbrt_kt > 0.0 { self.sapper_column(at, r, blast, &broken, &mut rubble) } else { at };
        // The ground right over it thrown out, leaving a crater (in sight).
        let mut cratered = false;
        if seen && self.eco.units.sapper.depth_m_per_cbrt_kt > 0.0 {
            let height = (len(top) - len(at)).max(1.0);
            let thrown = self.sapper_crater(at, r, yield_kt, height, blast);
            cratered = !thrown.is_empty();
            rubble.retain(|c| !thrown.contains(c));
        }
        // The rock round it cracked by the shock.
        let height = if self.eco.units.sapper.depth_m_per_cbrt_kt > 0.0 { (len(top) - len(at)).max(1.0) } else { 0.0 };
        cracked += self.sapper_fracture(at, r, blast, &stop, height, &broken);
        self.units.cracked += cracked + broken.len() as u64;
        // The plume: most blasts find a way up. (Not where the ground over
        // it was thrown out as a crater: a jet out of one hole beside the
        // crater's even fan looked lopsided; Benjamin, 26 Nov.)
        if seen && !cratered && crate::noise::unit(hash(blast, 0, 0, 77)) < 0.9 && !rubble.is_empty() {
            if let Some(mouth) = self.way_up(c0, r) {
                let share = 0.35 + 0.25 * crate::noise::unit(hash(blast, 0, 0, 7));
                let take = ((rubble.len() as f64 * share).round() as usize).min(rubble.len()).min(PLUME_MAX);
                // A random share of the rubble, the same every run.
                let mut pick: Vec<(u64, (i64, i64))> = rubble.iter().map(|&c| (hash(blast, c.0, c.1, 13), c)).collect();
                pick.sort_unstable();
                let pw = (0.8 + yield_kt.max(0.0).cbrt() * 0.35).min(4.0);
                // How long it pours out (the proving ground's plume).
                let dur = 0.6 + 0.35 * (yield_kt * 10.0 + 1.0).log10();
                if self.plumes.len() < 100 {
                    let m = mid(mouth);
                    self.plumes.push((m.0, m.1, pw as f32));
                }
                let mu = unit_of(mid(mouth));
                let side = (-mu.1, mu.0);
                for (k, &(h, c)) in pick.iter().take(take).enumerate() {
                    let Some(m) = self.world.lift_cell(c.0, c.1) else { continue };
                    self.loose.remove(&c);
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
            }
        }
        self.plume_out.make_contiguous().sort_by_key(|p| p.0);
        // Unreacted uranium: a share is left through the rubble still here.
        let sp = &self.eco.units.sapper;
        let unreacted = (fissile - yield_kt / sp.kt_per_kg_fissioned.max(1e-9)).max(0.0) * sp.unreacted_doped.clamp(0.0, 1.0);
        let here: Vec<(i64, i64)> = rubble.iter().copied().filter(|c| self.loose.contains(c)).collect();
        if unreacted > 0.0 && !here.is_empty() {
            let each = unreacted / here.len() as f64;
            for c in here {
                *self.units.dope.entry(c).or_insert(0.0) += each;
            }
            self.units.doped_kg += unreacted;
        }
        // Sappers caught in it go off too, a moment later.
        self.sappers_caught(at, r * 0.9);
        // Drones caught in it: the director never pulls them back.
        self.blast_harm(at, top, r);
    }

    /// The loosened column of a blast at `at` (radius `r`): every solid cell
    /// from the charge up to the surface, blast-wide at the bottom and
    /// `loosen_top` radii each side at the top, not already broken, is left
    /// loose (rubble) where it is, or for hard rock now and then held in big
    /// cracked pieces (Benjamin: leaving big chunks is good). Returns where
    /// the column breaks the surface; the rubble is added to `rubble`.
    pub(super) fn sapper_column(&mut self, at: (f64, f64), r: f64, blast: u64, broken: &CellMap<(f64, f64)>, rubble: &mut Vec<(i64, i64)>) -> (f64, f64) {
        let lt = self.eco.units.sapper.loosen_top.max(1.0);
        let up = unit_of(at);
        let side = (-up.1, up.0);
        let a = at.1.atan2(at.0);
        // The surface over it, looked at either side of its own shaft (straight
        // down the shaft, the first solid cell is the bottom of the hole).
        let ground = self.ground_beside(at, r);
        let height = (ground - len(at)).max(1.0);
        let piece = |x: i64, y: i64, size: f64| -> u64 {
            let (i, j) = ((x as f64 / size).floor() as i64, (y as f64 / size).floor() as i64);
            hash(0x91ED, i, j, 0)
        };
        let mut cracked = 0u64;
        let mut seen = CellSet::default();
        let mut h = -r * 0.5;
        while h <= height + 1.0 {
            let half = r * (1.0 + (lt - 1.0) * (h / height).clamp(0.0, 1.0));
            let mut s = -half;
            while s <= half {
                let c = cell_of((at.0 + up.0 * h + side.0 * s, at.1 + up.1 * h + side.1 * s));
                s += 0.7;
                if !seen.insert(c) || broken.contains_key(&c) || self.loose.contains(&c) {
                    continue;
                }
                let m = self.world.material(c.0, c.1);
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
                    let id = piece(c.0, c.1, size);
                    if crate::noise::unit(hash(id, blast as i64, 0, 62)) < (hd - 0.5) * 1.1 {
                        if self.fractured.insert(c) {
                            cracked += 1;
                        }
                        self.world.set_look(c.0, c.1, Look::HELD);
                        continue;
                    }
                }
                self.loose.insert(c);
                self.settling.insert(c);
                self.world.set_look(c.0, c.1, Look::RUBBLE);
                rubble.push(c);
            }
            h += 0.7;
        }
        self.units.cracked += cracked;
        (a.cos() * ground, a.sin() * ground)
    }

    /// The ground right over a blast thrown out, leaving a crater (Benjamin,
    /// 26 Nov: the top layer over the blast should be thrown off). A bowl
    /// `CRATER_W` of the loosened column's top half-width each side and
    /// `CRATER_D` of that deep at its middle (and the plants on it) is
    /// lifted and thrown, the top layer first and the deeper ground after:
    /// near the middle steeply up (much of it falls back in), towards the
    /// rim out at about 45 degrees, coming down round the crater. `height`
    /// is how far the ground is over the charge. Returns how many cells.
    pub(super) fn sapper_crater(&mut self, at: (f64, f64), r: f64, yield_kt: f64, height: f64, blast: u64) -> CellSet {
        let lt = self.eco.units.sapper.loosen_top.max(1.0);
        let w0 = (r * lt * CRATER_W).max(2.0);
        let dmax = (w0 * CRATER_D).min(height * 0.8).max(1.0);
        let a0 = at.1.atan2(at.0);
        let g0 = len(at) + height;
        // How hard it throws (as the plume does).
        let pw = (0.8 + 0.35 * yield_kt.max(0.0).cbrt()).min(6.0);
        let flora = |s: &Self, m: MatId| s.units.is_flora.get(m as usize).copied().unwrap_or(false);
        let un = |k: i64, salt: u64| crate::noise::unit(hash(blast, k, 0, salt));
        // A ragged bowl: each side reaches its own way out, and the depth
        // wanders a little (smooth noise, a fifth of the crater across).
        let (wl, wr) = (w0 * (0.85 + 0.3 * un(0, 0xC1)), w0 * (0.85 + 0.3 * un(1, 0xC1)));
        let period = (w0 / 5.0).max(6.0);
        let wander = |s: f64| {
            let t = s / period;
            let i = t.floor();
            let f = t - i;
            let f = f * f * (3.0 - 2.0 * f);
            un(i as i64, 0xC2) * (1.0 - f) + un(i as i64 + 1, 0xC2) * f
        };
        // The ground along it, and its slope: the throw goes out square to
        // the ground there, not to the sky straight up (on a hillside
        // straight up throws the uphill side into the hill).
        // (Down a shaft, a narrow dip, the ground either side of it.)
        let ground = |s: &Self, x: f64| {
            let at = |x: f64| s.ground_at(a0 + x / g0, true);
            let (c, side) = (at(x), at(x - 5.0).min(at(x + 5.0)));
            if c < side - 3.0 { side } else { c }
        };
        let slope = ((ground(self, wr) - ground(self, -wl)) / (wl + wr)).clamp(-1.5, 1.5);
        // (cell, offset across, depth share, depth m)
        let mut take: Vec<((i64, i64), f64, f64, f64)> = Vec::new();
        let mut seen = CellSet::default();
        let mut x = -wl;
        while x <= wr {
            let ang = a0 + x / g0;
            let (ux, uy) = (ang.cos(), ang.sin());
            let gr = ground(self, x);
            let e = if x < 0.0 { x / wl } else { x / wr };
            let d = dmax * (1.0 - e * e) * (0.85 + 0.3 * wander(x));
            // The plants on it.
            let mut z = 0.5;
            while z < 24.0 {
                let c = cell_of((ux * (gr + z), uy * (gr + z)));
                let m = self.world.material(c.0, c.1);
                if m != VACUUM && flora(self, m) && seen.insert(c) {
                    take.push((c, x, 0.0, -z));
                }
                z += 0.7;
            }
            let mut z = 0.0;
            while z <= d {
                let c = cell_of((ux * (gr - 0.5 - z), uy * (gr - 0.5 - z)));
                // (The bottom edge frayed.)
                let frayed = z > d - 1.5 && crate::noise::unit(hash(blast, c.0, c.1, 0xC3)) < 0.5;
                if !frayed && seen.insert(c) && self.world.material(c.0, c.1) != VACUUM {
                    take.push((c, x, z / d.max(1.0), z));
                }
                z += 0.7;
            }
            x += 0.7;
        }
        if take.len() > CRATER_MAX {
            take.sort_by(|a, b| a.2.total_cmp(&b.2));
            take.truncate(CRATER_MAX);
        }
        let out: CellSet = take.iter().map(|t| t.0).collect();
        // Highest first, by height (not depth under the ground just over it,
        // which differs column to column: a piece flew into the cell beside
        // it, as high but due later, and fell back in).
        let top = take.iter().map(|t| len(mid(t.0))).fold(f64::MIN, f64::max);
        let low = take.iter().map(|t| len(mid(t.0))).fold(f64::MAX, f64::min);
        let span = (top - low).max(1.0);
        // The highest of each column (by its place across).
        let mut col_top: HashMap<i64, f64> = HashMap::new();
        for t in &take {
            let e = col_top.entry((t.1 / 0.7).round() as i64).or_insert(f64::MIN);
            *e = e.max(len(mid(t.0)));
        }
        let k = (1.0 + slope * slope).sqrt();
        for (c, x, q, _) in take {
            let u = |k: u64| crate::noise::unit(hash(blast, c.0, c.1, 0xC0 + k));
            let f = (if x < 0.0 { -x / wl } else { x / wr }).min(1.0);
            let sgn = if x.abs() < 0.5 { if u(0) < 0.5 { -1.0 } else { 1.0 } } else { x.signum() };
            // Steep in the middle, about 45 degrees at the rim, and always
            // steeper than the crater's wall beside it (thrown flatter, most
            // of a big crater hit its own wall and fell back in).
            let wall = (2.0 * dmax * f / (if x < 0.0 { wl } else { wr })).atan().to_degrees() * 1.15;
            let th = (82.0 - 40.0 * f).max(wall + 12.0).min(86.0);
            let th = (th + (u(1) - 0.5) * 12.0).clamp(wall + 6.0, 89.0).to_radians();
            // As fast as the blast throws it (not as far as the body's pull
            // would bring it back: on a small body that is a crawl, and the
            // rest of the blast flies out fast): the deep ground slower.
            let v = 35.0 * pw * (1.0 - 0.45 * q) * (0.75 + 0.5 * u(2));
            let p = mid(c);
            let pr = len(p).max(1.0);
            let (ux, uy) = (p.0 / pr, p.1 / pr);
            // Square to the ground: its normal and its line across.
            let nrm = ((ux + uy * slope) / k, (uy - ux * slope) / k);
            let tan = (-nrm.1, nrm.0);
            let (vn, vt) = (v * th.sin(), sgn * v * th.cos());
            let vel = (nrm.0 * vn + tan.0 * vt, nrm.1 * vn + tan.1 * vt);
            // The top layer lifts off first, the deeper ground after it,
            // layer by layer (each on its way before the one under it goes:
            // a jumbled order threw cells into the ground still over them,
            // and the crater filled back in).
            let due = self.tick + self.ticks(0.08 + 0.6 * ((top - len(p)) / span).clamp(0.0, 1.0));
            // It flies from the crater's mouth, over the top of its column (a
            // piece flown from down in the hole could snag on a bit left
            // standing or a pocket something fell into, and drop back in;
            // flown all from one height, pieces caught on each other).
            let up_by = col_top.get(&((x / 0.7).round() as i64)).copied().unwrap_or(top) - len(p) + 1.5;
            self.eject.push_back((due, c, (p.0 + nrm.0 * up_by, p.1 + nrm.1 * up_by), vel));
        }
        self.eject.make_contiguous().sort_by_key(|e| e.0);
        out
    }

    /// The rock round a blast cracked by its shock (Benjamin, 26 Nov: the
    /// shockwave should crack and frack more): past where each ray stopped
    /// breaking rock (`stop`, m, by ray), `CRACK_REACH` blast radii further,
    /// and either side of the loosened column (more widely near the surface,
    /// where the ground lifts), the rock is cracked into blocks: small near,
    /// bigger further out, fewer the further out, more in hard brittle
    /// rock. A cracked cell is cut in half the time (`fracture_dig`); the
    /// blocks' edges show as cracks. `height`: how far the ground is over
    /// the charge (0: no column). Returns how many cells were cracked.
    pub(super) fn sapper_fracture(&mut self, at: (f64, f64), r: f64, blast: u64, stop: &[f64], height: f64, broken: &CellMap<(f64, f64)>) -> u64 {
        let lt = self.eco.units.sapper.loosen_top.max(1.0);
        let up = unit_of(at);
        let side = (-up.1, up.0);
        let rays = stop.len().max(1);
        let reach = r * CRACK_REACH;
        let far = stop.iter().fold(0.0f64, |a, &b| a.max(b)) + reach;
        let wide = far.max(r * lt + r);
        let block = |x: i64, y: i64, size: f64| -> u64 {
            let (i0, j0) = ((x as f64 / size).floor() as i64, (y as f64 / size).floor() as i64);
            let (mut best, mut id) = (f64::INFINITY, 0u64);
            for i in i0 - 1..=i0 + 1 {
                for j in j0 - 1..=j0 + 1 {
                    let px = (i as f64 + crate::noise::unit(hash(47, i, j, size as u64))) * size;
                    let py = (j as f64 + crate::noise::unit(hash(53, j, i, size as u64))) * size;
                    let d = (px - x as f64).powi(2) + (py - y as f64).powi(2);
                    if d < best {
                        best = d;
                        id = hash(0xF2AC, i, j, size as u64);
                    }
                }
            }
            id
        };
        let mut cracked = 0u64;
        let mut seen = CellSet::default();
        let mut h = -far;
        while h <= height + 1.0 {
            let mut s = -wide;
            while s <= wide {
                let c = cell_of((at.0 + up.0 * h + side.0 * s, at.1 + up.1 * h + side.1 * s));
                s += 0.7;
                if !seen.insert(c) || broken.contains_key(&c) || self.loose.contains(&c) {
                    continue;
                }
                let p = mid(c);
                let (dx, dy) = (p.0 - at.0, p.1 - at.1);
                // Round the charge, past where the rays stopped.
                let d = (dx * dx + dy * dy).sqrt();
                let k = ((dy.atan2(dx) / std::f64::consts::TAU * rays as f64).round() as i64).rem_euclid(rays as i64) as usize;
                let mut t = ((d - stop[k]) / reach).max(0.0);
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
                let m = self.world.material(c.0, c.1);
                if m == VACUUM || self.units.is_flora.get(m as usize).copied().unwrap_or(false) {
                    continue;
                }
                let mat = self.world.planet.mats.get(m);
                if mat.state != State::Solid {
                    continue;
                }
                let hd = (mat.hardness as f64 / 10.0).clamp(0.05, 0.95);
                let size = [3.5, 5.0, 7.0, 10.0][((t * 4.0) as usize).min(3)];
                let id = block(c.0, c.1, size);
                if crate::noise::unit(hash(id, blast as i64, 0, 71)) >= ((1.0 - t).powf(0.8) * (0.45 + 0.75 * hd)).min(1.0) {
                    continue;
                }
                // (Newly cracked only: what an earlier blast held or cracked
                // keeps its look.)
                if !self.fractured.insert(c) {
                    continue;
                }
                cracked += 1;
                if (block(c.0 + 1, c.1, size) != id || block(c.0, c.1 + 1, size) != id) && crate::noise::unit(hash(blast, c.0, c.1, 72)) < 0.9 - 0.4 * t {
                    self.world.set_look(c.0, c.1, Look::CRACKED);
                }
            }
            h += 0.7;
        }
        cracked
    }

    /// What a blast does to the swarm (Benjamin, 25 Nov: blasts kill
    /// cutters; haulers are damaged and can die; nothing is pulled back).
    /// Cutters in the loosened column from `at` up to `top` (and
    /// kill_margin_m round it) are wrecked; haulers near `top` lose life,
    /// the nearer the more.
    pub(super) fn blast_harm(&mut self, at: (f64, f64), top: (f64, f64), r: f64) {
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
            if !self.haulers[k].alive {
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
