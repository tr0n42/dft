//! Sappers: carry a 5 kt charge to where the going is hardest, crawl or
//! drill down until deep enough, and go off. The blast (rays, cracks, held
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
    /// Crawling down cracks and caves.
    Crawl,
    /// Boring straight down where there was no way deep enough.
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
}

/// At most this many cells of a blast on screen are thrown as flying
/// pieces, and at most this many blown out of its plume; the rest lies as
/// rubble. (Flying thousands of pieces stalled the sim for a blast on view.)
pub(super) const THROW_MAX: usize = 200;

pub(super) const PLUME_MAX: usize = 200;

/// Rubble cells moved per tick, at most.
pub(super) const SETTLE_PER_TICK: usize = 20_000;

/// The heart of a sapper's blast is heated to this (K): it melts, and cools
/// to glass after about this long (s).
pub(super) const MELT_K: f64 = 2000.0;

pub(super) const MELT_COOL_S: f64 = 25.0;

/// Ground this hot next to a sapper (a blast's heat) destroys it, and so sets it off.
pub(super) const SAPPER_COOK_K: f64 = 800.0;

impl Sim {
    /// Where a sapper is wanted: the flock where the going is hardest
    /// (slowest per kg lately), away from where other sappers are headed.
    pub(super) fn sapper_site(&self) -> Option<usize> {
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
        // Its charge, as cast when its bay started it.
        let fissile = self.units.charges.pop_front().unwrap_or_else(|| self.fissile_per_charge());
        let Some(site) = self.sapper_site() else { return };
        let (hx, hy) = self.site_pos(site);
        let a = hy.atan2(hx);
        let g = self.ground_at(a, true);
        let goal = (a.cos() * (g + 1.0), a.sin() * (g + 1.0));
        let start = self.mother_over(a);
        let id = self.units.new_id();
        let sp = &self.eco.units.sapper;
        let r = sp.radius_m_per_cbrt_kt * sp.yield_kt.max(0.0).cbrt();
        self.units.sappers.push(Sapper { id, state: SapState::Fly, pos: start, prev: start, moved_at: self.tick, goal, path: Vec::new(), along: 0.0, drilled: 0.0, armed_s: 0.0, want: r + sp.bury_m, ground_r: g, fissile });
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
            if s.state != SapState::Fly && s.armed_s >= 0.0 && !(s.state == SapState::Arm && s.armed_s >= sp.arm_s) && self.sapper_destroyed(s.pos) {
                self.units.destroyed += 1;
                blasts.push((s.pos, s.fissile));
                s.armed_s = -1.0;
                self.units.sappers[k] = s;
                continue;
            }
            match s.state {
                SapState::Fly => {
                    if approach(&mut s.pos, s.goal, sp.fly_speed) {
                        s.state = SapState::Crawl;
                        // Where the others are going (the end of their way).
                        let taken: Vec<(f64, f64)> = self.units.sappers.iter().filter(|o| o.id != s.id && !o.path.is_empty()).map(|o| mid(*o.path.last().unwrap())).collect();
                        let r = sp.radius_m_per_cbrt_kt * sp.yield_kt.max(0.0).cbrt();
                        s.path = self.sapper_path(s.pos, s.ground_r, s.want, sp.crawl_m, &taken, 0.7 * r);
                        s.along = 0.0;
                    }
                }
                SapState::Crawl => {
                    s.along += sp.crawl_speed;
                    let last = s.path.len().saturating_sub(1);
                    let i = (s.along as usize).min(last);
                    if let Some(&c) = s.path.get(i) {
                        s.pos = mid(c);
                    }
                    if i >= last {
                        s.state = if s.ground_r - len(s.pos) >= s.want { SapState::Arm } else { SapState::Drill };
                    }
                }
                SapState::Drill => {
                    // Straight down, a shaft three wide; what it bores through
                    // spouts up out of the hole (a share is kept).
                    let down = unit_of(s.pos);
                    let mut stop = false;
                    for _ in 0..sp.drill_speed.ceil().max(1.0) as usize {
                        let next = (s.pos.0 - down.0, s.pos.1 - down.1);
                        let c = cell_of(next);
                        let side = (-down.1, down.0);
                        for o in [-1.0, 0.0, 1.0] {
                            let q = cell_of((next.0 + side.0 * o, next.1 + side.1 * o));
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
                        // What it bores through spouts up out of the hole and
                        // comes down round it as loose rubble.
                        for (k, o) in [-1.0, 0.0, 1.0].into_iter().enumerate() {
                            let q = cell_of((next.0 + side.0 * o, next.1 + side.1 * o));
                            let Some(m) = self.world.lift_cell(q.0, q.1) else { continue };
                            self.undope(q, false);
                            self.progress.remove(&q);
                            self.reserved.remove(&q);
                            self.open_cell(q);
                            let h = hash(self.tick, q.0, q.1, 0xD811 + k as u64);
                            let vs = 45.0 + 35.0 * crate::noise::unit(h);
                            let vx = (crate::noise::unit(hash(h, 1, 0, 0)) - 0.5) * 3.0;
                            let from = (s.pos.0 + down.0 * 2.0, s.pos.1 + down.1 * 2.0);
                            if let Some(t) = self.bodies.throw(&mut self.world, from, m, (down.0 * vs + side.0 * vx, down.1 * vs + side.1 * vx)) {
                                self.loose.insert(t);
                                self.world.set_look(t.0, t.1, Look::RUBBLE);
                                self.settling.insert(t);
                                self.scrap_landed(t);
                            }
                        }
                        let _ = c;
                        s.pos = next;
                        s.drilled += 1.0;
                        if s.ground_r - len(s.pos) >= s.want || s.drilled >= sp.drill_m {
                            stop = true;
                            break;
                        }
                    }
                    if stop {
                        s.state = SapState::Arm;
                    }
                }
                SapState::Arm => {
                    s.armed_s += 1.0;
                    if s.armed_s >= sp.arm_s {
                        blasts.push((s.pos, s.fissile));
                        s.armed_s = -1.0;
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

    /// Whether whatever is round a sapper at `p` destroys it: its own cell
    /// filled in (buried by falling rock or rubble), or magma or ground hot
    /// from a blast touching it.
    pub(super) fn sapper_destroyed(&self, p: (f64, f64)) -> bool {
        let c = cell_of(p);
        let m = self.world.material(c.0, c.1);
        if m != VACUUM {
            let mat = self.world.planet.mats.get(m);
            // (Water it can sit in; rock, rubble or magma on top of it, no.)
            if !(mat.state == State::Liquid && self.units.freeze_to.get(m as usize).copied().flatten().is_none()) {
                return true;
            }
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
                    // A crack may run on, wandering, further in harder rock.
                    if ray % 9 == 0 {
                        let (mut ax, mut ay, mut aa) = (c.0 as f64, c.1 as f64, a);
                        let len = (r * (0.2 + 0.7 * h)) as i64;
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
        self.units.cracked += cracked + broken.len() as u64;
        // The plume: most blasts find a way up.
        if seen && crate::noise::unit(hash(blast, 0, 0, 77)) < 0.9 && !rubble.is_empty() {
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
