//! Foresters (walkers): tall sideways crabs that stride to the thickest
//! stands, shear plants into their bags with their claws, burn the ground
//! cover with their lasers, leap steep terrain, and drop full bags for the
//! couriers. Abductors walk on the same machinery (see abductor.rs).
//!
//! Unit file: the forester's own behaviour; the platform calls
//! `walkers_work` once a game second (and `scan_flora` now and then).

use super::*;
use crate::production::Unit;

/// What a walker (forester or abductor) is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkState {
    /// Falling from the replicator, folded.
    Drop,
    /// On the ground, unfolding and standing.
    Unfold,
    /// Striding to the next stand of life.
    Stride,
    /// Working: creeping from one catch to the next.
    Work,
    /// Leaping over ground too steep to walk (a cliff, a mountain).
    Jump,
}

#[derive(Debug, Clone)]
pub struct Walker {
    pub(super) id: u32,
    pub(super) kind: Unit,
    pub(super) state: WalkState,
    pub(super) pos: (f64, f64),
    pub(super) prev: (f64, f64),
    pub(super) moved_at: u64,
    /// The direction (radians) it is heading for, and which way it faces
    /// along the ground (+1 counter-clockwise).
    pub(super) goal_a: f64,
    pub(super) facing: f64,
    pub(super) life: f64,
    pub(super) timer: f64,
    /// kg in its bag, and kg taken all told.
    pub(super) bag: f64,
    pub(super) taken_kg: f64,
    /// Where its laser last fired, and when (tick).
    pub(super) laser: (f64, f64),
    pub(super) laser_at: u64,
    /// The cell its claws are on, and when it last cut (tick).
    pub(super) claw: (f64, f64),
    pub(super) cut_at: u64,
    /// How high its body rides over the ground (m): over the treetops.
    pub(super) lift: f64,
    /// A leap under way: from (direction, ground radius) to (direction,
    /// ground radius), the arc's top (radius), seconds into it, and its length.
    pub(super) jump: Option<Jump>,
    /// Where its laser's last sweep began (it ends at `laser`).
    pub(super) laser_from: (f64, f64),
    /// What is in its bag: cells of each material.
    pub(super) load: Vec<(MatId, f64)>,
}

/// A walker's leap over ground too steep to walk.
#[derive(Debug, Clone, Copy)]
pub struct Jump {
    pub(super) a0: f64,
    pub(super) g0: f64,
    pub(super) a1: f64,
    pub(super) g1: f64,
    pub(super) top: f64,
    pub(super) t: f64,
    pub(super) dur: f64,
}

/// A full bag a walker dropped, waiting on the ground for couriers to carry
/// it up to the mother swarm. What it holds is only credited when it gets
/// there.
#[derive(Debug, Clone)]
pub struct Bag {
    pub(super) id: u32,
    pub(super) kind: Unit,
    pub(super) pos: (f64, f64),
    pub(super) kg: f64,
    pub(super) load: Vec<(MatId, f64)>,
    /// The tick couriers lift it away.
    pub(super) due: u64,
}

/// Seconds a bag is seen rising with its couriers after they lift it.
pub(super) const BAG_RISE_S: f64 = 3.0;

/// Walkers leap ground steeper than this (rise over run, looked at this far
/// ahead), landing where it is gentler than this, at most this far off;
/// they clear the highest ground on the way by this much and cover this
/// many metres a second.
pub(super) const LEAP_SLOPE: f64 = 1.0;

pub(super) const LEAP_LOOK_M: f64 = 24.0;

pub(super) const LEAP_MIN_RISE_M: f64 = 20.0;

pub(super) const LEAP_LAND_SLOPE: f64 = 0.45;

pub(super) const LEAP_MAX_M: f64 = 800.0;

pub(super) const LEAP_CLEAR_M: f64 = 25.0;

pub(super) const LEAP_SPEED: f64 = 35.0;

/// The most cells a claw shears off a plant at one go.
pub(super) const SECTION_MAX: usize = 60;

/// A walker towers over all plant life: its body rides this far over the
/// tallest plant round it (and never lower than that over bare ground); it
/// rises and settles this fast (m/s).
pub(super) const LIFT_MIN_M: f64 = 50.0;

pub(super) const LIFT_OVER_M: f64 = 50.0;

pub(super) const LIFT_RATE: f64 = 5.0;

/// How far round it (m, each way) it looks for the tallest plant.
pub(super) const CANOPY_ROUND_M: i64 = 30;

/// The tallest plants a walker looks over (m).
pub(super) const CANOPY_MAX_M: i64 = 160;

impl Sim {
    /// Stands of plants round the body: 256 directions, and round each
    /// working flock, counting plant cells over the ground.
    pub(super) fn scan_flora(&mut self) {
        const DIRS: usize = 256;
        const PER_SCAN: usize = 8;
        let dir = |i: usize| std::f64::consts::TAU * i as f64 / DIRS as f64;
        if self.units.flora_dirs.len() != DIRS {
            self.units.flora_dirs = vec![0; DIRS];
            self.units.flora_cursor = 0;
        }
        // A share of the directions round the body each scan, in turn.
        for _ in 0..PER_SCAN {
            let i = self.units.flora_cursor;
            self.units.flora_cursor = (i + 1) % DIRS;
            self.units.flora_dirs[i] = self.flora_near(dir(i), 0.0);
        }
        let mut out: Vec<(f64, u32)> = self.units.flora_dirs.iter().enumerate().filter(|&(_, &k)| k > 0).map(|(i, &k)| (dir(i), k)).collect();
        // And over every flock, every scan.
        for i in self.live_sites() {
            let (x, y) = self.site_pos(i);
            let a = y.atan2(x);
            let k = self.flora_near(a, 0.0);
            if k > 0 {
                out.push((a, k));
            }
        }
        out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.total_cmp(&b.0)));
        out.truncate(64);
        self.units.flora = out;
    }

    /// Plant cells in the column in direction `a` (and `w` m either side,
    /// every 2 m), from just under the ground to 40 m over it.
    pub(super) fn flora_near(&self, a: f64, w: f64) -> u32 {
        let g = self.ground_at(a, true);
        let mut k = 0;
        let steps = (w / 2.0).ceil() as i64;
        for j in -steps..=steps {
            let aa = a + j as f64 * 2.0 / g.max(1.0);
            let (ux, uy) = (aa.cos(), aa.sin());
            for h in -2..40 {
                let r = g + h as f64;
                let m = self.world.material((ux * r).floor() as i64, (uy * r).floor() as i64);
                if self.units.is_flora.get(m as usize).copied().unwrap_or(false) {
                    k += 1;
                }
            }
        }
        k
    }

    pub(super) fn launch_walker(&mut self, u: Unit) {
        if u == Unit::Abductor {
            // To the nearest herd no other abductor is after.
            let Some(a) = self.abductor_goal(self.mother.centre, 0) else { return };
            let start = self.mother_over(a);
            let id = self.units.new_id();
            let life = self.eco.units.forester.life_s;
            self.units.walkers.push(Walker { id, kind: u, state: WalkState::Drop, pos: start, prev: start, moved_at: self.tick, goal_a: a, facing: 1.0, life, timer: 0.0, bag: 0.0, taken_kg: 0.0, laser: start, laser_at: 0, claw: start, cut_at: 0, lift: LIFT_MIN_M, load: Vec::new(), jump: None, laser_from: start });
            return;
        }
        // The thickest stand no other walker is on.
        let busy = |a: f64, ws: &Vec<Walker>| ws.iter().any(|w| ang_dist(w.goal_a, a) * self.world.planet.radius < 150.0);
        let Some(&(a, _)) = self.units.flora.iter().find(|(a, _)| !busy(*a, &self.units.walkers)).or_else(|| self.units.flora.first()) else { return };
        let start = self.mother_over(a);
        let id = self.units.new_id();
        let life = self.eco.units.forester.life_s;
        self.units.walkers.push(Walker { id, kind: u, state: WalkState::Drop, pos: start, prev: start, moved_at: self.tick, goal_a: a, facing: 1.0, life, timer: 0.0, bag: 0.0, taken_kg: 0.0, laser: start, laser_at: 0, claw: start, cut_at: 0, lift: LIFT_MIN_M, load: Vec::new(), jump: None, laser_from: start });
    }

    pub(super) fn walkers_work(&mut self) {
        if self.units.walkers.is_empty() {
            return;
        }
        let fs = self.eco.units.forester.clone();
        for k in 0..self.units.walkers.len() {
            let mut w = self.units.walkers[k].clone();
            w.prev = w.pos;
            w.moved_at = self.tick;
            w.life -= 1.0;
            let a = w.pos.1.atan2(w.pos.0);
            match w.state {
                WalkState::Drop => {
                    // Falls under gravity from the replicator's gantry.
                    let g = self.ground_at(a, true);
                    w.timer += 1.0;
                    let fall = (4.0 * w.timer * w.timer).min(len(w.pos) - g);
                    let u = unit_of(w.pos);
                    w.pos = (w.pos.0 - u.0 * fall, w.pos.1 - u.1 * fall);
                    if len(w.pos) <= g + 0.01 {
                        w.state = WalkState::Unfold;
                        w.timer = 0.0;
                    }
                }
                WalkState::Unfold => {
                    self.walker_stand(&mut w);
                    w.timer += 1.0;
                    if w.timer >= 3.0 {
                        w.state = WalkState::Stride;
                    }
                }
                WalkState::Stride => {
                    let r = self.ground_at(a, true).max(1.0);
                    let da = {
                        let mut d = w.goal_a - a;
                        while d > std::f64::consts::PI {
                            d -= std::f64::consts::TAU;
                        }
                        while d < -std::f64::consts::PI {
                            d += std::f64::consts::TAU;
                        }
                        d
                    };
                    let step = fs.stride_speed / r;
                    w.facing = if da >= 0.0 { 1.0 } else { -1.0 };
                    // A forester works as it goes: claws and laser on what
                    // it passes.
                    self.walker_stand(&mut w);
                    if w.kind == Unit::Forester {
                        self.walker_hands(&mut w, &fs);
                    }
                    if let Some(j) = self.leap_over(a, w.facing, da.abs() * r) {
                        w.jump = Some(j);
                        w.state = WalkState::Jump;
                        self.units.leaps += 1;
                    } else {
                        let na = if da.abs() <= step { w.goal_a } else { a + step * w.facing };
                        let g = self.ground_at(na, true);
                        w.pos = (na.cos() * g, na.sin() * g);
                        self.walker_rise(&mut w);
                        if da.abs() <= step {
                            w.state = WalkState::Work;
                            w.timer = 0.0;
                        }
                    }
                }
                WalkState::Jump => {
                    // Along its arc, a second at a time; down on the far side.
                    match w.jump.as_mut() {
                        Some(j) => {
                            j.t = (j.t + 1.0).min(j.dur);
                            let k = j.t / j.dur;
                            let aa = j.a0 + (j.a1 - j.a0) * k;
                            let base = j.g0 + (j.g1 - j.g0) * k;
                            let mid_line = (j.g0 + j.g1) / 2.0;
                            let rr = base + 4.0 * (j.top - mid_line) * k * (1.0 - k);
                            w.pos = (aa.cos() * rr, aa.sin() * rr);
                            if j.t >= j.dur {
                                w.jump = None;
                                w.state = WalkState::Stride;
                                self.walker_stand(&mut w);
                            }
                        }
                        None => w.state = WalkState::Stride,
                    }
                }
                WalkState::Work => {
                    self.walker_stand(&mut w);
                    if w.kind == Unit::Abductor {
                        let q = clock();
                        self.abductor_hunt(&mut w);
                        self.unit_prof[U_ABDUCTOR] += clock() - q;
                    } else {
                        self.walker_feed(&mut w, &fs);
                    }
                }
            }
            self.units.walkers[k] = w;
        }
        // A walker at the end of its life leaves its bag for the couriers.
        for k in 0..self.units.walkers.len() {
            if self.units.walkers[k].life <= 0.0 && self.units.walkers[k].bag > 0.0 {
                let mut w = self.units.walkers[k].clone();
                self.drop_bag(&mut w);
                self.units.walkers[k] = w;
            }
        }
        self.units.walkers.retain(|w| w.life > 0.0);
        self.bags_work();
    }

    /// The terrain's height in direction `a`: the first solid cell coming
    /// down that isn't a plant, dead plant matter or loose rubble.
    pub(super) fn terrain_at(&self, a: f64) -> f64 {
        let (ux, uy) = (a.cos(), a.sin());
        let mut r = (self.world.planet.surface_radius(ux, uy) + 64.0).min(self.world.planet.max_radius + 1.0);
        for _ in 0..4000 {
            if r <= 0.0 {
                break;
            }
            let c = ((ux * r).floor() as i64, (uy * r).floor() as i64);
            let m = self.world.material(c.0, c.1);
            let soft = self.units.is_flora.get(m as usize).copied().unwrap_or(false) || self.loose_mat.get(m as usize).copied().unwrap_or(false) || self.loose.contains(&c);
            if m != VACUUM && !soft {
                return r + 0.5;
            }
            r -= 1.0;
        }
        r.max(0.0)
    }

    /// Ground too steep to walk just ahead of a walker at direction `a`
    /// going `facing` (+1 counter-clockwise), `left_m` short of its goal: a
    /// leap over it to the first walkable ground beyond, if there is any.
    pub(super) fn leap_over(&self, a: f64, facing: f64, left_m: f64) -> Option<Jump> {
        let r = self.terrain_at(a).max(1.0);
        let at = |m: f64| self.terrain_at(a + facing * m / r);
        let g0 = at(0.0);
        // Steep just ahead, and for a good height? (Not a bump, a tree, a
        // fallen trunk or a heap of rubble.)
        let ahead = LEAP_LOOK_M.min(left_m.max(1.0));
        let rise = (at(ahead) - g0).abs();
        if rise / ahead < LEAP_SLOPE || rise < LEAP_MIN_RISE_M {
            return None;
        }
        // The first spot past it gentle enough to stand on, and the highest
        // ground on the way.
        let mut top = g0;
        let mut m = ahead;
        while m <= LEAP_MAX_M {
            let g = at(m);
            top = top.max(g);
            let slope = (at(m + 4.0) - at(m - 4.0)).abs() / 8.0;
            if m > ahead + 8.0 && slope < LEAP_LAND_SLOPE {
                let dur = (m / LEAP_SPEED).max(2.0).ceil();
                return Some(Jump { a0: a, g0, a1: a + facing * m / r, g1: g, top: top + LEAP_CLEAR_M, t: 0.0, dur });
            }
            m += 4.0;
        }
        None
    }

    /// Keep a standing walker's feet on the ground (the swarm may have eaten
    /// it away under it) and its body over the treetops.
    pub(super) fn walker_stand(&self, w: &mut Walker) {
        let a = w.pos.1.atan2(w.pos.0);
        let g = self.ground_at(a, true);
        w.pos = (a.cos() * g, a.sin() * g);
        self.walker_rise(w);
    }

    /// Its body rides over the tallest plant round it: up as the forest rises,
    /// down as it is cleared.
    pub(super) fn walker_rise(&self, w: &mut Walker) {
        let want = (self.canopy_at(w.pos) + LIFT_OVER_M).max(LIFT_MIN_M);
        w.lift += (want - w.lift).clamp(-LIFT_RATE, LIFT_RATE);
    }

    /// Leave a walker's bag on the ground behind it for the couriers, and
    /// start a new one.
    pub(super) fn drop_bag(&mut self, w: &mut Walker) {
        if w.bag <= 0.0 {
            return;
        }
        let up = unit_of(w.pos);
        let side = (-up.1, up.0);
        let behind = (w.pos.0 - side.0 * w.facing * 6.0, w.pos.1 - side.1 * w.facing * 6.0);
        let a = behind.1.atan2(behind.0);
        let g = self.ground_at(a, true);
        let pos = (a.cos() * g, a.sin() * g);
        let (pickup_s, speed) = (self.eco.units.forester.pickup_s, self.eco.units.forester.courier_speed.max(1.0));
        let m = self.mother_pos();
        let d = ((m.0 - pos.0).powi(2) + (m.1 - pos.1).powi(2)).sqrt();
        let id = self.units.new_id();
        let due = self.tick + self.ticks(pickup_s + d / speed);
        self.units.bags.push(Bag { id, kind: w.kind, pos, kg: w.bag, load: std::mem::take(&mut w.load), due });
        w.bag = 0.0;
    }

    /// Bags the couriers have reached go up to the mother swarm: what they
    /// hold is credited now.
    pub(super) fn bags_work(&mut self) {
        let tick = self.tick;
        // Credited as the couriers lift it; seen rising for a few seconds
        // more, then gone.
        let gone = self.ticks(BAG_RISE_S);
        let mut arrived = Vec::new();
        let mut live = 0.0;
        for b in self.units.bags.iter_mut() {
            if b.due <= tick && !b.load.is_empty() {
                arrived.append(&mut b.load);
            }
            // An abductor's sacs: live animals, credited once.
            if b.due <= tick && b.kind == Unit::Abductor && b.kg > 0.0 {
                live += b.kg;
                b.kg = 0.0;
            }
        }
        self.units.bags.retain(|b| tick < b.due + gone);
        for (m, cells) in arrived {
            self.credit(m, cells);
        }
        if live > 0.0 {
            self.credit_live(live);
        }
    }

    /// Credit `cells` cells of material `m` arriving at the mother swarm.
    pub(super) fn credit(&mut self, m: MatId, cells: f64) {
        let mat = self.world.planet.mats.get(m);
        let kg = mat.density * cells;
        let credited = kg * mat.multiplier;
        let comp = mat.composition.clone();
        self.stock.add_composition(&self.world.planet.mats.elements, &comp, kg);
        self.mass += credited;
        self.delivered_credited += credited;
        self.window_mined += kg;
    }

    /// The top of the plants round `pos` (within a few metres), as height
    /// over the ground there (m); 0 if bare.
    pub(super) fn canopy_at(&self, pos: (f64, f64)) -> f64 {
        let up = unit_of(pos);
        let side = (-up.1, up.0);
        let mut top = 0.0f64;
        for k in (-CANOPY_ROUND_M..=CANOPY_ROUND_M).step_by(3) {
            let k = k as f64;
            let base = (pos.0 + side.0 * k, pos.1 + side.1 * k);
            for h in (1..=CANOPY_MAX_M).rev() {
                if (h as f64) <= top {
                    break;
                }
                let c = cell_of((base.0 + up.0 * h as f64, base.1 + up.1 * h as f64));
                let m = self.world.material(c.0, c.1);
                if self.units.is_flora.get(m as usize).copied().unwrap_or(false) {
                    top = h as f64;
                    break;
                }
            }
        }
        top
    }

    /// One second of a forester's work where it stands, as the mockup works
    /// it: a claw goes for the nearest plant within reach ahead (behind if
    /// none) and shears a section off its top (about 4 m of it) into the
    /// bag; its laser sweeps the ground some way off, setting the cover and
    /// the small plants alight; with nothing in reach it creeps toward the
    /// nearest plant, and with nothing near, strides to the next stand.
    pub(super) fn walker_feed(&mut self, w: &mut Walker, fs: &crate::economy::ForesterSpec) {
        let a = w.pos.1.atan2(w.pos.0);
        let r = len(w.pos).max(1.0);
        let cut = self.walker_hands(w, fs);
        self.walker_creep(w, fs, a, r, cut);
    }

    /// A forester's claws and laser, a second's worth, wherever it is: the
    /// cells it sheared.
    pub(super) fn walker_hands(&mut self, w: &mut Walker, fs: &crate::economy::ForesterSpec) -> usize {
        let a = w.pos.1.atan2(w.pos.0);
        let r = len(w.pos).max(1.0);
        let up = unit_of(w.pos);
        let side = (-up.1, up.0);
        let flora = |s: &Self, c: (i64, i64)| s.units.is_flora.get(s.world.material(c.0, c.1) as usize).copied().unwrap_or(false);
        // The plant to go for: the nearest column of plant within reach
        // (ahead first), and its top.
        let reach = fs.reach_m;
        let mut best: Option<(f64, f64, (i64, i64))> = None;
        let mut along_m = -reach;
        while along_m <= reach {
            let base = (w.pos.0 + side.0 * along_m * w.facing, w.pos.1 + side.1 * along_m * w.facing);
            for h in (0..=(w.lift.ceil() as i64 + 2)).rev() {
                let c = cell_of((base.0 + up.0 * h as f64, base.1 + up.1 * h as f64));
                if flora(self, c) {
                    // Either side, nearest first (its claws are one a side).
                    let d = along_m.abs();
                    if best.is_none_or(|b| d < b.0) {
                        best = Some((d, h as f64, c));
                    }
                    break;
                }
            }
            along_m += 1.0;
        }
        let mut cut = 0;
        if let Some((_, top_h, c)) = best {
            // A section off its top: the plant's cells in the top 4 m round
            // that column.
            let tc = mid(c);
            let mut section: Vec<(i64, i64)> = Vec::new();
            for dy in -6..=6 {
                for dx in -6..=6 {
                    let q = (c.0 + dx, c.1 + dy);
                    if !flora(self, q) {
                        continue;
                    }
                    let p = mid(q);
                    let along = (p.0 - tc.0) * side.0 + (p.1 - tc.1) * side.1;
                    let height = (p.0 - w.pos.0) * up.0 + (p.1 - w.pos.1) * up.1;
                    if along.abs() <= 4.5 && height >= top_h - 4.0 {
                        section.push(q);
                    }
                }
            }
            section.sort_unstable();
            for q in section.into_iter().take(SECTION_MAX) {
                let m = self.world.material(q.0, q.1);
                let kg = self.world.planet.mats.get(m).density;
                // Into its bag: credited once couriers carry the bag up.
                self.world.mine_cell(q.0, q.1);
                self.undope(q, false);
                self.progress.remove(&q);
                self.reserved.remove(&q);
                self.open_cell(q);
                match w.load.iter_mut().find(|e| e.0 == m) {
                    Some(e) => e.1 += 1.0,
                    None => w.load.push((m, 1.0)),
                }
                w.bag += kg;
                w.taken_kg += kg;
                self.units.harvested_kg += kg;
                cut += 1;
            }
            w.claw = tc;
            w.cut_at = self.tick;
        }
        if w.bag >= fs.bag_kg {
            // Full, and nothing to hand it to: it drops the bag for the
            // couriers to pick up, grows a new one and carries on.
            self.drop_bag(w);
        }
        // Its laser: every couple of seconds, a sweep along the ground off to
        // one side, setting alight what grows there.
        w.timer += 1.0;
        let h = hash(self.tick, w.id as i64, 0, 0xF12E);
        if w.timer >= fs.burn_every_s + crate::noise::unit(h) {
            w.timer = 0.0;
            let d = if crate::noise::unit(hash(h, 1, 0, 0)) < 0.5 { -1.0 } else { 1.0 };
            let from_m = d * (10.0 + 20.0 * crate::noise::unit(hash(h, 2, 0, 0)));
            let to_m = from_m + d * (15.0 + 25.0 * crate::noise::unit(hash(h, 3, 0, 0)));
            let mut lit = 0;
            let (mut first, mut last) = (None, None);
            let n = (to_m - from_m).abs().ceil() as i64;
            for k in 0..=n {
                let m_along = from_m + d * k as f64;
                let aa = a + m_along / r;
                let g = self.ground_at(aa, false);
                let (ux, uy) = (aa.cos(), aa.sin());
                let gp = (ux * g, uy * g);
                if first.is_none() {
                    first = Some(gp);
                }
                last = Some(gp);
                // The ground cover and small plants there (down to the ground).
                for hgt in -2..6 {
                    let c = cell_of((ux * (g + hgt as f64), uy * (g + hgt as f64)));
                    if flora(self, c) {
                        self.sandbox.heat_cell(&self.world, c, 1100.0);
                        lit += 1;
                        break;
                    }
                }
            }
            self.units.fires += lit;
            if let (Some(f), Some(l)) = (first, last) {
                w.laser_from = f;
                w.laser = l;
                w.laser_at = self.tick;
            }
        }
        cut
    }

    /// With nothing in its claws' reach: creep toward the nearest plant, or
    /// stride off to the next stand.
    pub(super) fn walker_creep(&mut self, w: &mut Walker, fs: &crate::economy::ForesterSpec, a: f64, r: f64, cut: usize) {
        if cut == 0 {
            // Nothing in its claws' reach: creep toward the nearest plant
            // near by, else stride off to the next stand.
            let ahead = [1.0, -1.0].into_iter().find_map(|s: f64| {
                (1..=40).find(|j| self.flora_near(a + s * *j as f64 * 3.0 / r, 0.0) > 0).map(|j| (s, j))
            });
            match ahead {
                Some((s, j)) => {
                    w.facing = s;
                    let step = (fs.work_speed).min(j as f64 * 3.0);
                    let na = a + s * step / r;
                    let g = self.ground_at(na, true);
                    w.pos = (na.cos() * g, na.sin() * g);
                }
                None => {
                    let busy = |x: f64, ws: &Vec<Walker>| ws.iter().any(|o| o.id != w.id && ang_dist(o.goal_a, x) * self.world.planet.radius < 150.0);
                    let next = self.units.flora.iter().filter(|(x, _)| ang_dist(*x, a) * r > 60.0).filter(|(x, _)| !busy(*x, &self.units.walkers)).min_by(|p, q| ang_dist(p.0, a).total_cmp(&ang_dist(q.0, a))).map(|p| p.0);
                    match next {
                        Some(x) => {
                            w.goal_a = x;
                            w.state = WalkState::Stride;
                        }
                        None => w.life = w.life.min(30.0),
                    }
                }
            }
        }
    }
}
