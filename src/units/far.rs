//! The far field: the swarm where nobody is looking, a stretch of surface at
//! a time (Benjamin: off screen, or zoomed out far enough, anything goes, as
//! long as what we look at is right; and a planet covered in drones should
//! still run at 15x).
//!
//! The body's surface is cut into sectors about SECTOR_M wide (by angle; see
//! `world::FarCut`). Near the camera the swarm is what it always was: flocks
//! of cutters flown one by one. Further away, once there are more cutters
//! than the game can fly one by one (FAR_BUDGET), whole sectors go far: their
//! flocks fold into a count of cutters, and the sector is eaten as a whole,
//! every FAR_STEP_S, from the top down: its level (everything further from
//! the centre is gone) comes down by what that many cutters would have cut,
//! the highest ground first, as cutters prefer it. What they cut is
//! delivered to the mother swarm at once. When the camera comes back, the
//! sector unfolds into flocks again, over the ground as it now stands.
//!
//! A far sector costs a few dozen lookups every few seconds however many
//! cutters it has, so the swarm can cover a body as big as the Earth.

use super::*;

/// About this much of the surface in each sector (m).
pub const SECTOR_M: f64 = 1024.0;
/// Zoomed out at least this far (m per pixel), a sector on screen may go
/// far too: flocks are drawn as marks there, and a sector's ground is a few
/// pixels deep (see the game's ICON_MPP).
pub const FAR_MPP: f64 = 5.0;
/// Most cutters flown one by one before sectors further from the camera go
/// far, at first: the budget then follows what the simulation can afford
/// (FAR_TARGET_MS), between FAR_BUDGET_MIN and FAR_BUDGET_MAX.
pub const FAR_BUDGET: f64 = 100_000.0;
const FAR_BUDGET_MIN: f64 = 0.0;
const FAR_BUDGET_MAX: f64 = 400_000.0;
/// Real ms the simulation may take per second of game time: 15x with room
/// for drawing and everything else the game does (1000 / 15 = 67; 60 is
/// 16.7x).
pub const FAR_TARGET_MS: f64 = 60.0;
/// Seconds between a far sector's bites (at least: with many far sectors,
/// longer, so there are at most about BITES_PER_S bites a second).
const FAR_STEP_S: f64 = 5.0;
const BITES_PER_S: usize = 400;
/// Columns sampled across a sector to see what its level cuts through.
const SAMPLES: usize = 16;
/// Most a sector's level comes down in one bite (m), in at most this many
/// looks at what it cuts through.
const MAX_BITE_M: f64 = 64.0;
const LOOKS: usize = 12;
/// Cutters that can work a metre of surface at once: what flocks reach on
/// ground with room for them (measured on the living world; see HANDOFF).
pub const COVER_PER_M: f64 = 6.0;
/// A far cutter's pace against a trip's dig time and flights: matched to
/// what cutters flown one by one bring in per cutter at work (waits for
/// haulers included), measured on the living world (about 160 kg a second
/// each with 300k cutters).
const PACE: f64 = 1.0;
/// New cutters a second that drop to flocks on screen past the budget.
const NEAR_NEW_PER_S: u64 = 2_000;
/// Most a far sector's level goes under a neighbour's (m): its ramp spans
/// it (see world::FAR_RAMP_M).
const FAR_STEP_M: f64 = 300.0;
/// Cutters a second at most come down from far sectors to work as flocks.
const UNFOLD_PER_S: u64 = 20_000;
/// New far sectors opened per second at most (the economy's swarm; a fixed
/// benchmark swarm opens 16 times as many).
const OPEN_PER_S: usize = 2;

/// The far field's state (see the module notes).
#[derive(Debug, Clone, Default)]
pub struct Far {
    /// Cutters working each far sector.
    pub cutters: Vec<u64>,
    /// Ground a far sapper loosened in each sector and not yet eaten (kg):
    /// its cutters take it at the pace of loose rubble (Benjamin, 25 Nov:
    /// off screen, a sapper is pure math).
    pub loose_kg: Vec<f64>,
    /// Sappers gone off in far sectors.
    pub blasts: u64,
    /// Wear: cutters due to retire, as a fraction, per sector.
    wear: Vec<f64>,
    /// Sectors the swarm has reached (cutters or flocks there, or once had).
    pub open: Vec<bool>,
    /// Sectors under open liquid (a sea): nothing to eat there.
    pub wet: Vec<bool>,
    /// Sectors far now.
    pub is_far: Vec<bool>,
    /// Sectors the camera shows closely enough that they are always worked
    /// as flocks, whatever the budget (see `far_must_near`).
    pub must: Vec<bool>,
    /// What the camera shows: (cells x0, x1, y0, y1; m per pixel). None: no
    /// camera (tests, benchmarks), so nothing goes far.
    pub view: Option<([i64; 4], f64)>,
    /// Totals, for reports: cutters in far sectors, kg eaten far, flocks
    /// folded and unfolded, cutters unfolded.
    pub total: u64,
    pub eaten_kg: f64,
    pub folded: u64,
    pub unfolded: u64,
    pub cutters_unfolded: u64,
    /// Seconds spent (profiling).
    pub prof: f64,
    /// Cutters flown one by one at most (0: FAR_BUDGET at first). It
    /// follows the simulation's cost unless `fixed`.
    pub budget: f64,
    pub fixed: bool,
    /// Real seconds spent stepping the simulation (all of it), and as of
    /// the last look; real ms per game second lately.
    pub busy_s: f64,
    busy_seen: f64,
    pub cost_ms: f64,
    /// Cutters flown one by one at the last look (working, not pooled).
    pub flown: f64,
    /// Whether any sector is far now.
    pub any_far: bool,
    /// Cutters the sectors the camera shows keep at work (their cover), and
    /// those flown one by one in them now.
    must_cover: u64,
    must_flown: u64,
    /// Pool drones passed over this round (see `far_pop_parked`).
    kept: Vec<u32>,
    /// One past the tick in which no far sector had room.
    no_room_tick: u64,
    /// The sector new cutters went to last, and sectors opened this second.
    last_put: usize,
    opened_now: usize,
}

impl Sim {
    /// The camera's view, from the game, every frame: the rectangle on
    /// screen (cells) and the zoom (m per pixel). None: no camera.
    pub fn set_view(&mut self, v: Option<([i64; 4], f64)>) {
        self.far.view = v;
    }

    /// Over where the mother swarm arrived (for a camera to start at).
    pub fn mother_pos_now(&self) -> (f64, f64) {
        let a = self.mother.centre;
        let r = self.world.planet.ground_hint(a.cos(), a.sin());
        (a.cos() * r, a.sin() * r)
    }

    /// Cutters in far sectors.
    pub fn far_cutters(&self) -> u64 {
        self.far.total
    }

    /// Each far sector with cutters: (middle direction, radians; its level,
    /// m from the centre; cutters), for drawing.
    pub fn far_sectors(&self) -> Vec<(f64, f64, u64)> {
        let n = self.far.cutters.len();
        let tau = std::f64::consts::TAU;
        (0..n).filter(|&k| self.far.cutters[k] > 0).map(|k| (tau * (k as f64 + 0.5) / n as f64, self.world.far.levels.get(k).map_or(0.0, |l| *l as f64), self.far.cutters[k])).collect()
    }

    fn far_n(&self) -> usize {
        self.far.cutters.len()
    }

    /// Set up the sectors (once there is a camera).
    fn far_ready(&mut self) {
        if !self.far.cutters.is_empty() {
            return;
        }
        if self.world.far.levels.is_empty() {
            self.world.far = crate::world::FarCut::new(self.world.planet.radius, SECTOR_M);
        }
        let n = self.world.far.levels.len();
        self.far.cutters = vec![0; n];
        self.far.loose_kg = vec![0.0; n];
        self.far.wear = vec![0.0; n];
        self.far.open = vec![false; n];
        self.far.wet = vec![false; n];
        self.far.is_far = vec![false; n];
        self.far.must = vec![false; n];
    }

    /// Sector k's angles (from, to), radians.
    fn far_span(&self, k: usize) -> (f64, f64) {
        let tau = std::f64::consts::TAU;
        let n = self.far_n() as f64;
        (tau * k as f64 / n, tau * (k + 1) as f64 / n)
    }

    /// Where sector k's ground is (about): the highest cell found in its
    /// sampled columns, and the share of them topped by liquid.
    fn far_top(&self, k: usize) -> (f64, f64) {
        let (a0, a1) = self.far_span(k);
        let (mut top, mut wet) = (0.0f64, 0usize);
        for j in 0..SAMPLES {
            let a = a0 + (a1 - a0) * (j as f64 + 0.5) / SAMPLES as f64;
            let (ux, uy) = (a.cos(), a.sin());
            let lvl = self.world.far.levels[k] as f64;
            let guess = self.world.planet.ground_hint(ux, uy).min(lvl);
            let g = self.ground_near(a, guess);
            let c = ((ux * g).floor() as i64, (uy * g).floor() as i64);
            let m = self.world.material_bare(c.0, c.1);
            if m != VACUUM && self.world.planet.mats.get(m).state == State::Liquid {
                wet += 1;
            }
            top = top.max(g);
        }
        (top + 1.0, wet as f64 / SAMPLES as f64)
    }

    /// The sector holding a point.
    fn far_sector_at(&self, p: (f64, f64)) -> usize {
        self.world.far.sector_of(p.0, p.1)
    }

    /// Whether the camera shows sector k closely enough that it must be
    /// worked cell by cell: on the screen or within a screen of it (three
    /// screens across in all), zoomed in closer than FAR_MPP.
    fn far_must_near(&self, k: usize) -> bool {
        let Some((r, mpp)) = self.far.view else { return true };
        if mpp >= FAR_MPP {
            return false;
        }
        // The view a screen wider each way, as angles round the body and
        // distances from its centre (a view is small beside the body).
        let (w, h) = ((r[1] - r[0]) as f64, (r[3] - r[2]) as f64);
        let (x0, x1, y0, y1) = (r[0] as f64 - w, r[1] as f64 + w, r[2] as f64 - h, r[3] as f64 + h);
        let (cx, cy) = (0.5 * (x0 + x1), 0.5 * (y0 + y1));
        let ca = cy.atan2(cx);
        let (mut amin, mut amax, mut rmin, mut rmax) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, 0.0f64);
        for (x, y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
            let mut d = y.atan2(x) - ca;
            while d > std::f64::consts::PI {
                d -= std::f64::consts::TAU;
            }
            while d < -std::f64::consts::PI {
                d += std::f64::consts::TAU;
            }
            amin = amin.min(d);
            amax = amax.max(d);
            let rr = (x * x + y * y).sqrt();
            rmin = rmin.min(rr);
            rmax = rmax.max(rr);
        }
        // (The body's centre in view: everything is.)
        if x0 <= 0.0 && x1 >= 0.0 && y0 <= 0.0 && y1 >= 0.0 {
            return true;
        }
        let rmin = rmin.min((cx * cx + cy * cy).sqrt());
        // The sector: its angles, and from its floor up to the highest ground.
        let (a0, a1) = self.far_span(k);
        let mut d0 = a0 - ca;
        while d0 > std::f64::consts::PI {
            d0 -= std::f64::consts::TAU;
        }
        while d0 < -std::f64::consts::PI {
            d0 += std::f64::consts::TAU;
        }
        let d1 = d0 + (a1 - a0);
        let lvl = (self.world.far.levels[k] as f64).min(self.world.planet.max_radius);
        let (rin, rout) = ((lvl - 1500.0).max(0.0), self.world.planet.max_radius + 600.0);
        d1 >= amin && d0 <= amax && rout >= rmin && rin <= rmax
    }

    /// Once a second, with a camera: which sectors are far (the nearest to
    /// the camera stay near while the cutters flown one by one fit the
    /// budget), folding and unfolding flocks to match; the far sectors'
    /// bites when due; drones out of work sent to far sectors.
    pub(super) fn far_work(&mut self) {
        if self.far.view.is_none() && self.far.total == 0 {
            return;
        }
        let c0 = clock();
        self.far_ready();
        self.far.opened_now = 0;
        // What the last second cost, and the budget to match: less when
        // over the target, more (slowly) when well under it.
        {
            let spent = (self.far.busy_s - self.far.busy_seen).max(0.0) * 1000.0;
            self.far.busy_seen = self.far.busy_s;
            self.far.cost_ms = if self.far.cost_ms == 0.0 { spent } else { 0.9 * self.far.cost_ms + 0.1 * spent };
            if self.far.budget <= 0.0 {
                self.far.budget = FAR_BUDGET;
            }
            if !self.far.fixed && self.tick % self.ticks(5.0) == 0 {
                if self.far.cost_ms > FAR_TARGET_MS {
                    // (Down to nothing but what the camera shows, if that's
                    // what it takes.)
                    self.far.budget = if self.far.budget < 2_000.0 { FAR_BUDGET_MIN } else { (self.far.budget * 0.85).max(FAR_BUDGET_MIN) };
                } else if self.far.cost_ms < 0.6 * FAR_TARGET_MS && self.far.flown >= 0.8 * self.far.budget {
                    self.far.budget = (self.far.budget * 1.05).max(self.far.budget + 2_000.0).min(FAR_BUDGET_MAX);
                }
            }
        }
        let n = self.far_n();
        // Cutters flown one by one in each sector (working, not pooled).
        let mut load = vec![0.0f64; n];
        let mut has_flock = vec![false; n];
        for h in 0..self.hubs.len() {
            if self.flock[h].dead || self.hubs[h].kind == HubKind::Refiner {
                continue;
            }
            let k = self.far_sector_at(self.site_pos(h));
            load[k] += self.active(h) as f64;
            has_flock[k] = true;
        }
        for k in 0..n {
            if has_flock[k] {
                self.far.open[k] = true;
            }
        }
        // Which go far: those the camera needs stay near; then the rest,
        // nearest the camera first, while they fit the budget (a little
        // more for those near already, so the line doesn't flicker).
        let centre = match self.far.view {
            Some((r, _)) => (0.5 * (r[0] + r[1]) as f64, 0.5 * (r[2] + r[3]) as f64),
            None => (0.0, 1.0),
        };
        let ca = centre.1.atan2(centre.0);
        let mut want_far = vec![false; n];
        let mut used = 0.0;
        let mut rest: Vec<(f64, usize)> = Vec::new();
        for k in 0..n {
            self.far.must[k] = self.far.open[k] && self.far_must_near(k);
            if !self.far.open[k] {
                continue;
            }
            if self.far.must[k] {
                used += load[k] + self.far.cutters[k] as f64;
            } else {
                let (a0, a1) = self.far_span(k);
                rest.push((ang_dist(0.5 * (a0 + a1), ca), k));
            }
        }
        rest.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        self.far.must_cover = (0..n).filter(|&k| self.far.must[k]).map(|k| self.far_cover(k)).sum();
        // (Every cutter homed there, at work or resting in the pool.)
        self.far.must_flown = (0..self.hubs.len()).filter(|&h| !self.flock[h].dead && self.far.must[self.far_sector_at(self.site_pos(h))]).map(|h| self.hub_load[h] as u64).sum();
        let budget = self.far.budget;
        let mut full = false;
        for (_, k) in rest {
            let here = load[k] + self.far.cutters[k] as f64;
            let room = if self.far.is_far[k] { 0.9 * budget } else { 1.1 * budget };
            if !full && used + here <= room {
                used += here;
            } else {
                full = true;
                want_far[k] = true;
            }
        }
        // Fold the flocks of sectors going far; unfold those coming near.
        let fold: Vec<usize> = (0..self.hubs.len()).filter(|&h| !self.flock[h].dead && self.hubs[h].kind != HubKind::Refiner && want_far[self.far_sector_at(self.site_pos(h))]).collect();
        if !fold.is_empty() {
            self.far_fold(&fold);
        }
        for k in 0..n {
            if want_far[k] && !self.far.is_far[k] {
                // Its level starts at the top of its ground.
                if !self.world.far.levels[k].is_finite() {
                    let (top, wet) = self.far_top(k);
                    self.far.wet[k] = wet > 0.5;
                    let _ = self.world.far_lower(k, top + 64.0, top);
                    self.far_forget(k, top + 64.0, top);
                }
            }
            self.far.is_far[k] = want_far[k];
        }
        // Near sectors with cutters still counted: they come down to work as
        // flocks, nearest the camera first, as many a second as the budget
        // has room for (and no more than UNFOLD_PER_S, so the step never
        // stalls); the rest wait in the mother swarm meanwhile.
        // (Every cutter flown one by one counts, at work or resting.)
        let flown = self.alive_count as f64;
        self.far.flown = flown;
        let mut rate = UNFOLD_PER_S;
        let mut allow = ((budget - flown).max(0.0) as u64).min(UNFOLD_PER_S);
        let mut waiting: Vec<(f64, usize)> = (0..n)
            .filter(|&k| !self.far.is_far[k] && self.far.cutters[k] > 0)
            .map(|k| {
                let (a0, a1) = self.far_span(k);
                (ang_dist(0.5 * (a0 + a1), ca), k)
            })
            .collect();
        waiting.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (_, k) in waiting {
            // (What the camera shows gets all its cutters back, budget or
            // not, only no faster than UNFOLD_PER_S.)
            let most = if self.far.must[k] { rate } else { allow.min(rate) };
            if most == 0 {
                continue;
            }
            let got = self.far_unfold(k, most);
            rate -= got.min(rate);
            allow -= got.min(allow);
        }
        self.far.any_far = self.far.is_far.iter().any(|f| *f);
        self.far.total = self.far.cutters.iter().sum();
        self.far_spread();
        // Loose matter in far sectors waits until someone looks; so does a
        // sea the camera doesn't show (flocks beside it would drain it cell
        // by cell, out of sight).
        let asleep: Vec<bool> = (0..n).map(|k| self.far.is_far[k] || (self.far.wet[k] && !self.far.must[k] && self.far.view.is_some())).collect();
        if self.world.far.asleep != asleep {
            self.world.far.asleep = asleep;
        }
        self.far.any_far = self.far.is_far.iter().any(|f| *f);
        // Bites: each far sector every `period` seconds, a share of them
        // each second (at most about BITES_PER_S a second in all).
        let busy = (0..n).filter(|&k| self.far.is_far[k] && self.far.cutters[k] > 0).count();
        let period = (FAR_STEP_S as usize).max(busy.div_ceil(BITES_PER_S));
        let sec = (self.tick / self.ticks(1.0).max(1)) as usize;
        let due: Vec<usize> = (0..n).filter(|&k| (k + sec) % period == 0 && self.far.is_far[k] && self.far.cutters[k] > 0).collect();
        let dt = period as f64;
        for &k in &due {
            self.far_wear(k, dt);
        }
        if !self.brownout() {
            let plans: Vec<(usize, f64, f64, Vec<(MatId, f64)>, f64)> = {
                use rayon::prelude::*;
                let me: &Sim = self;
                due.par_iter().filter(|&&k| me.far.cutters[k] > 0).filter_map(|&k| me.far_plan(k, dt)).collect()
            };
            for (k, from, to, eaten, loose) in plans {
                self.far_apply(k, from, to, &eaten);
                if let Some(l) = self.far.loose_kg.get_mut(k) {
                    *l = (*l - loose).max(0.0);
                }
            }
        }
        self.far.total = self.far.cutters.iter().sum();
        self.far.prof += clock() - c0;
    }

    /// Fold flocks into their sectors: their cutters become counts there
    /// (what they carried delivered), their haulers go up to unload, and
    /// the flocks are gone.
    fn far_fold(&mut self, flocks: &[usize]) {
        let mut sector_of = HashMap::<usize, usize>::new();
        for &h in flocks {
            sector_of.insert(h, self.far_sector_at(self.site_pos(h)));
        }
        let parked: std::collections::HashSet<u32> = self.parked.iter().copied().collect();
        let (mut kg, mut cr) = (0.0, 0.0);
        for id in 0..self.drones.len() {
            let d = &self.drones[id];
            if !d.alive || parked.contains(&(id as u32)) {
                continue;
            }
            let Some(&k) = sector_of.get(&d.home) else { continue };
            let id = id as u32;
            let d = self.drones[id as usize].clone();
            self.unpromise(id);
            match d.phase {
                Phase::Outbound { cell, .. } => {
                    self.unreserve(cell);
                    self.unwork(cell, id);
                    self.on_trip_count -= 1;
                }
                Phase::Inbound { .. } => self.on_trip_count -= 1,
                _ => {}
            }
            self.grabs.remove(&id);
            kg += d.hold;
            cr += d.hold_cr;
            let dr = &mut self.drones[id as usize];
            dr.alive = false;
            dr.hold = 0.0;
            dr.hold_cr = 0.0;
            self.leg_changed(id);
            self.hub_load[d.home] -= 1;
            self.alive_count -= 1;
            self.far.cutters[k] += 1;
        }
        // What they carried, delivered.
        if kg > 0.0 {
            self.transit.take(kg, Some(&mut self.stock));
            self.mats_take(kg, true);
            self.far_credit(kg, cr);
        }
        for &h in flocks {
            self.flock[h].dead = true;
            self.site_wait[h].clear();
            for k in 0..self.haulers.len() {
                if self.haulers[k].alive && self.haulers[k].site == h && matches!(self.haulers[k].state, HaulerState::Working | HaulerState::Returning) {
                    self.release_hauler(k);
                }
            }
            self.far.folded += 1;
        }
        self.hub_gen += 1;
        self.inbound_stale = true;
    }

    /// Sector k comes near: flocks over its ground as it stands, every
    /// NEW_FLOCK_GAP_M along it, its cutters shared among them, resting at
    /// their drop-off points, about to set out.
    fn far_unfold(&mut self, k: usize, most: u64) -> u64 {
        let count = self.far.cutters[k].min(most);
        if count == 0 {
            return 0;
        }
        if std::env::var("DFT_DEBUG_FAR").is_ok() {
            eprintln!("far unfold {k}: {count} of {}", self.far.cutters[k]);
        }
        // Its flocks already there first; else new ones over its ground.
        // (Only those still over the ground as it now stands: a landing
        // point left hanging high over ground eaten since is closed.)
        let mut homes: Vec<usize> = Vec::new();
        for h in 0..self.hubs.len() {
            if self.flock[h].dead || self.hubs[h].kind != HubKind::Seed || self.far_sector_at(self.site_pos(h)) != k {
                continue;
            }
            let p = self.site_pos(h);
            let r = (p.0 * p.0 + p.1 * p.1).sqrt();
            let a = p.1.atan2(p.0);
            if r - self.ground_down(a, r) > 0.5 * self.eco.harvester.search_m {
                self.flock[h].dead = true;
                self.hub_gen += 1;
                continue;
            }
            homes.push(h);
        }
        if homes.is_empty() {
            let (a0, a1) = self.far_span(k);
            let lvl = self.world.far.levels[k] as f64;
            let r = lvl.min(self.world.planet.max_radius);
            // A flock for every split-size crew it brings (no closer than
            // they'd merge), along its ground.
            let width = (a1 - a0) * r;
            let crews = (self.far.cutters[k] as f64 / self.eco.flock.split_cutters.max(1) as f64).ceil();
            let gap = (width / crews.max(1.0)).clamp(self.eco.flock.merge_m * 1.25, NEW_FLOCK_GAP_M);
            let spots = ((width / gap).round() as usize).max(1);
            for j in 0..spots {
                let a = a0 + (a1 - a0) * (j as f64 + 0.5) / spots as f64;
                let (ux, uy) = (a.cos(), a.sin());
                let g = self.ground_deep(a, self.world.planet.ground_hint(ux, uy).min(lvl));
                let c = ((ux * g).floor() as i64, (uy * g).floor() as i64);
                let m = self.world.material_bare(c.0, c.1);
                if m == VACUUM || self.world.planet.mats.get(m).state == State::Liquid {
                    continue;
                }
                let p = (ux * (g + self.eco.flock.hover_m), uy * (g + self.eco.flock.hover_m));
                if self.site_danger(p) > 0 {
                    continue;
                }
                homes.push(self.new_flock(p.0, p.1));
            }
            // The sector's ground, open to its cutters (once, all of it).
            if !homes.is_empty() {
                let a = 0.5 * (a0 + a1);
                self.survey_arc(a.cos(), a.sin(), 0.5 * (a1 - a0) * r + 50.0, true);
            }
            if homes.is_empty() && std::env::var("DFT_DEBUG_FAR").is_ok() {
                let a = 0.5 * (a0 + a1);
                let (ux, uy) = (a.cos(), a.sin());
                let g = self.ground_deep(a, self.world.planet.ground_hint(ux, uy).min(lvl));
                let c = ((ux * g).floor() as i64, (uy * g).floor() as i64);
                eprintln!("far unfold {k}: no spots; level {lvl:.0} surface {:.0} ground found {g:.0} material {} danger {}", self.world.planet.surface_radius(ux, uy), self.world.material_bare(c.0, c.1), self.site_danger((ux * (g + 8.0), uy * (g + 8.0))));
            }
            if homes.is_empty() {
                // Nowhere to land here: they go to work far sectors.
                let all = std::mem::take(&mut self.far.cutters[k]);
                self.far.wet[k] = true;
                let put = self.far_rehome(all);
                if put < all {
                    self.far.cutters[k] = all - put;
                }
                return 0;
            }
            self.far.unfolded += 1;
        }
        self.far.cutters[k] -= count;
        if self.far.cutters[k] == 0 {
            self.far.wear[k] = 0.0;
        }
        for i in 0..count {
            let h = homes[i as usize % homes.len()];
            self.spawn_resting(h, i);
        }
        self.far.cutters_unfolded += count;
        self.place_docks();
        self.hub_gen += 1;
        self.inbound_stale = true;
        count
    }

    /// The ground's height in direction `a` from a guess, however far below
    /// it (a crater or canyon in a sector eaten down to its rim), down to
    /// 5 km under the guess.
    fn ground_deep(&self, a: f64, guess: f64) -> f64 {
        let mut g = self.ground_near(a, guess);
        let (ux, uy) = (a.cos(), a.sin());
        let stop = guess - 5000.0;
        while g > stop.max(1.0) && self.world.material_bare((ux * g).floor() as i64, (uy * g).floor() as i64) == VACUUM {
            g = self.ground_down(a, g - 1.0);
        }
        g
    }

    /// A cutter (from a far sector) resting at flock h's drop-off point,
    /// about to set out.
    fn spawn_resting(&mut self, h: usize, i: u64) {
        let id = self.drones.len() as u32;
        self.hub_load[h] += 1;
        let dock = 0usize;
        self.drones.push(cutter::Drone {
            alive: true,
            home: h,
            phase: Phase::Idle,
            life_left: self.eco.harvester.life_s * self.up("drone lifespan") * (0.2 + 0.8 * ((i * 7919) % 1000) as f64 / 1000.0),
            trip_start: self.tick,
            leg_start: self.tick,
            last: None,
            fails: 0,
            hold: 0.0,
            hold_cr: 0.0,
            tank: self.eco.harvester.tank_e,
            heat_k: 290.0,
            dock: dock as u8,
            due: 0,
            promised: 0.0,
            promised_to: (0, 0),
            off_dock: false,
        });
        self.alive_count += 1;
        self.leg_changed(id);
        // (Setting out once its flock has its flow field, two seconds on,
        // and spread over the next twenty, as a flock's cutters are over
        // their round trips.)
        let spread = self.ticks(20.0);
        self.schedule(id, self.tick + self.ticks(2.0) + (i.wrapping_mul(7919) % spread));
    }

    /// kg delivered to the mother swarm from out of sight (worth `cr`):
    /// as a hauler unloading there.
    fn far_credit(&mut self, kg: f64, cr: f64) {
        self.energy += kg / 1000.0 * self.eco.seed_swarm.processing_energy_per_tonne * self.up("processing yield");
        self.mass += cr;
        self.delivered_credited += cr;
        if self.mother.half < std::f64::consts::PI {
            let g = cr * self.eco.seed_swarm.mother_growth_share;
            self.mass -= g;
            self.spent_mass += g;
            self.mother.grown_kg += g;
        }
    }

    /// Far sector k's cutters wear out over `dt` seconds: they retire at the
    /// rate cutters flown one by one do (worn out at their hauler, they are
    /// recycled, as scrap).
    fn far_wear(&mut self, k: usize, dt: f64) {
        let cutters = self.far.cutters[k] as f64;
        let life = self.eco.harvester.life_s * self.up("drone lifespan");
        self.far.wear[k] += cutters * dt / life.max(1.0);
        let gone = (self.far.wear[k].floor() as u64).min(self.far.cutters[k]);
        if gone > 0 {
            self.far.wear[k] -= gone as f64;
            self.far.cutters[k] -= gone;
            self.deaths += gone;
            self.recycled += gone;
            let scrap = gone as f64 * self.eco.harvester.mass_cost;
            self.far_credit(scrap, scrap);
        }
    }

    /// One bite of far sector k worked out (only reading the world, so the
    /// bites due are worked out side by side): what its cutters would cut in
    /// `dt` seconds, taken off the top of its ground. (Its level from and
    /// to, and kg per material.) None: nothing to cut.
    fn far_plan(&self, k: usize, dt: f64) -> Option<(usize, f64, f64, Vec<(MatId, f64)>, f64)> {
        // (No more work at once than its surface has room for: the rest
        // wait for room, see `far_spread`.)
        let cutters = (self.far.cutters[k] as f64).min(self.far_cover(k) as f64);
        let (a0, a1) = self.far_span(k);
        let mut lvl = self.world.far.levels[k] as f64;
        if !lvl.is_finite() {
            lvl = self.far_top(k).0;
        }
        let from = lvl;
        // Beside a sea, no lower than its surface (a shore is left standing,
        // so no sea pours into what was eaten).
        let n = self.far_n();
        let mut floor = 0.0f64;
        for j in [(k + n - 1) % n, (k + 1) % n] {
            let lj = self.world.far.levels[j] as f64;
            if !lj.is_finite() {
                continue;
            }
            // (A sea is drunk from the top down like any other ground: the
            // land beside it waits for it, so it never pours into what was
            // eaten.)
            if self.far.wet[j] && !self.far.wet[k] {
                floor = floor.max(lj + 1.0);
            }
            // No deeper than FAR_STEP_M under a neighbour: it waits for
            // them, so the ground eaten stays slopes, not canyons.
            floor = floor.max(lj - FAR_STEP_M);
        }
        if lvl <= floor {
            return None;
        }
        let mats = &self.world.planet.mats;
        let mut budget = 0.0;
        let mut loose_used = 0.0;
        let mut eaten: Vec<(MatId, f64)> = Vec::new();
        let width = (a1 - a0) * lvl.max(1.0);
        let mut most = 1.0f64;
        let mut found: Vec<(MatId, f64)> = Vec::with_capacity(SAMPLES);
        for _ in 0..LOOKS {
            // What the level cuts through, column by column.
            let mut per_m = 0.0;
            found.clear();
            for j in 0..SAMPLES {
                let a = a0 + (a1 - a0) * (j as f64 + 0.5) / SAMPLES as f64;
                let r = lvl - 0.5;
                let c = ((a.cos() * r).floor() as i64, (a.sin() * r).floor() as i64);
                let m = self.world.material_bare(c.0, c.1);
                if m != VACUUM {
                    let kg = mats.get(m).density * width / SAMPLES as f64;
                    per_m += kg;
                    found.push((m, kg));
                }
            }
            if budget == 0.0 {
                // What the cutters cut in dt: a hold's worth a trip, at the
                // pace of a trip in what lies at the top here. (Hardness
                // only: a far sector's ground is never looked at cell by
                // cell, so no loose debris, cold or fractures.)
                let hold = self.hold_cap();
                let trip = match found.first().map(|f| f.0) {
                    Some(m) if self.loose_mat.get(m as usize).copied().unwrap_or(false) => self.eco.harvester.loose_s * hold / mats.get(m).density.max(1.0) + 8.0,
                    Some(m) => self.dig_s(m) * hold / self.eco.harvester.cargo_kg + self.eco.harvester.search_m / self.cutter_speed().max(1.0) + 5.0,
                    None => 60.0,
                };
                budget = cutters * hold * PACE * dt / trip.max(1.0);
                // Ground a far sapper loosened goes first, at the pace of
                // loose rubble; the time left over cuts as before.
                let loose = self.far.loose_kg.get(k).copied().unwrap_or(0.0);
                if loose > 0.0 {
                    let dens = found.first().map(|f| mats.get(f.0).density).unwrap_or(2500.0).max(1.0);
                    let loose_trip = self.eco.harvester.loose_s * hold / dens + 8.0;
                    let can = cutters * hold * PACE * dt / loose_trip.max(1.0);
                    loose_used = loose.min(can);
                    budget = loose_used + budget * (1.0 - loose_used / can.max(1e-9));
                }
                // (In steps of a metre, or bigger for a big bite.)
                most = (budget / per_m.max(1.0) / (LOOKS as f64 - 2.0)).max(1.0);
                if budget <= 0.0 {
                    break;
                }
            }
            if per_m < 1.0 {
                // Nothing at this level in any column sampled: down to the
                // highest ground below it (free: nothing is cut).
                let mut best = 0.0f64;
                for j in 0..SAMPLES {
                    let a = a0 + (a1 - a0) * (j as f64 + 0.5) / SAMPLES as f64;
                    best = best.max(self.ground_down(a, lvl));
                }
                if best <= 1.0 || best >= lvl {
                    break;
                }
                lvl = (best + 0.5).max(floor);
                if lvl <= floor {
                    break;
                }
                continue;
            }
            let step = (budget / per_m).min(most).min(lvl - floor);
            if step <= 0.0 {
                break;
            }
            for &(m, kg) in &found {
                match eaten.iter_mut().find(|e| e.0 == m) {
                    Some(e) => e.1 += kg * step,
                    None => eaten.push((m, kg * step)),
                }
            }
            budget -= per_m * step;
            lvl -= step;
            if budget <= 1.0 || from - lvl >= MAX_BITE_M {
                break;
            }
        }
        (lvl < from).then_some((k, from, lvl, eaten, loose_used))
    }

    /// A bite worked out by `far_plan`, taken: the sector's level comes
    /// down, and what was cut is booked as mined and delivered.
    fn far_apply(&mut self, k: usize, from: f64, to: f64, eaten: &[(MatId, f64)]) {
        let _ = self.world.far_lower(k, from, to);
        self.far_forget(k, from, to);
        let (mut kg, mut cr) = (0.0, 0.0);
        for &(m, e) in eaten {
            if e <= 0.0 {
                continue;
            }
            let mat = self.world.planet.mats.get(m);
            self.world.ledger.mine(&self.world.planet.mats, m, e / mat.density);
            self.stock.add_composition(&self.world.planet.mats.elements, &mat.composition, e);
            if let Some(r) = self.recent_mats.get_mut(m as usize) {
                *r += e;
            }
            if let Some(c) = self.collected_mats.get_mut(m as usize) {
                *c += e;
            }
            kg += e;
            cr += e * mat.multiplier;
        }
        self.window_mined += kg;
        self.far.eaten_kg += kg;
        self.far_credit(kg, cr);
    }

    /// Drones resting in the pool with no flock near the camera to go to:
    /// to far sectors with room (the swarm's reach, then the sectors beside
    /// it). Returns how many went.
    pub(super) fn far_take_pool(&mut self, near_full: bool) -> usize {
        if self.far.cutters.is_empty() || self.parked.is_empty() {
            return 0;
        }
        let n = self.far_n();
        let mut sent = 0;
        let mut opened = 0;
        loop {
            let mut any = false;
            for k in 0..n {
                if self.parked.is_empty() {
                    break;
                }
                if !self.far.is_far[k] || !self.far.open[k] {
                    continue;
                }
                let r = self.far_room_of(k);
                for _ in 0..r {
                    let Some(id) = self.far_pop_parked() else { break };
                    let d = &mut self.drones[id as usize];
                    if !d.alive {
                        continue;
                    }
                    d.alive = false;
                    let home = d.home;
                    self.leg_changed(id);
                    self.hub_load[home] -= 1;
                    self.alive_count -= 1;
                    self.far.cutters[k] += 1;
                    sent += 1;
                    any = true;
                }
            }
            // More ground: with many out of work, or (the economy's swarm)
            // with every flock and far sector full and the means to grow.
            let grow = self.parked.len() >= POOL_OPEN || (self.free_drones.is_none() && near_full && self.far.any_far && self.far_room() == 0 && self.can_expand());
            if !grow || self.far_open_next().is_none() || !any && self.far_room() == 0 {
                break;
            }
            let _ = opened;
            opened += 1;
        }
        // Many still resting with no room anywhere: spread over the far
        // sectors (as counts they cost nothing; flown one by one, they
        // would count against the budget doing nothing).
        if self.parked.len() >= POOL_OPEN && self.far.any_far {
            let usable: Vec<usize> = (0..n).filter(|&k| self.far.is_far[k] && self.far.open[k]).collect();
            if !usable.is_empty() {
                let mut i = 0;
                while let Some(id) = self.far_pop_parked() {
                    let d = &mut self.drones[id as usize];
                    if !d.alive {
                        continue;
                    }
                    d.alive = false;
                    let home = d.home;
                    self.leg_changed(id);
                    self.hub_load[home] -= 1;
                    self.alive_count -= 1;
                    self.far.cutters[usable[i % usable.len()]] += 1;
                    i += 1;
                    sent += 1;
                }
            }
        }
        // (Those kept back, back in the pool.)
        let kept = std::mem::take(&mut self.far.kept);
        self.parked.extend(kept);
        self.far.total = self.far.cutters.iter().sum();
        sent
    }

    /// A drone resting in the pool to send far: not one whose flock the
    /// camera looks at (it waits there for room), latest first.
    fn far_pop_parked(&mut self) -> Option<u32> {
        while let Some(&id) = self.parked.last() {
            let h = self.drones[id as usize].home;
            let k = if h < self.hubs.len() { self.far_sector_at(self.site_pos(h)) } else { 0 };
            if self.far.must.get(k).copied().unwrap_or(false) && !self.flock.get(h).is_none_or(|f| f.dead) {
                // (Kept: the rest of the pool is looked at next time.)
                let keep = self.parked.pop().unwrap();
                self.far.kept.push(keep);
                continue;
            }
            return self.parked.pop();
        }
        None
    }

    /// A new cutter, when the flock it would drop to is gone or far: it
    /// joins a far sector with room instead (opening the next one beside
    /// the swarm's reach if none has any). False if there is a flock near
    /// the camera for it.
    pub(super) fn far_absorb_new(&mut self) -> bool {
        if !self.far.any_far {
            return false;
        }
        let h = self.orbit_hub;
        if h < self.hubs.len() && !self.flock[h].dead && self.far_near_at(self.site_pos(h)) {
            let k = self.far_sector_at(self.site_pos(h));
            let short = self.far.must_cover > self.far.must_flown;
            if (self.alive_count as f64) < self.far.budget || (short && self.far.must.get(k).copied().unwrap_or(false)) {
                return false;
            }
        }
        // The far sector with room nearest where it was last put (once a
        // tick at most a search that finds none: then the last one again).
        let n = self.far_n();
        let start = self.far.last_put.min(n - 1);
        let usable = |s: &Self, k: usize| s.far.is_far[k] && s.far.open[k];
        if self.far.no_room_tick == self.tick + 1 {
            let k = if usable(self, start) { start } else {
                match (0..n).find(|&k| usable(self, k)) {
                    Some(k) => k,
                    None => return false,
                }
            };
            self.far.last_put = k;
            self.far.cutters[k] += 1;
            self.far.total += 1;
            return true;
        }
        let mut put = None;
        for i in 0..n {
            for k in [(start + i) % n, (start + n - i) % n] {
                if usable(self, k) && self.far_room_of(k) > 0 {
                    put = Some(k);
                    break;
                }
            }
            if put.is_some() {
                break;
            }
        }
        let k = match put.or_else(|| self.far_open_next()) {
            Some(k) => k,
            None => {
                // No room anywhere this tick.
                self.far.no_room_tick = self.tick + 1;
                if usable(self, start) {
                    start
                } else {
                    match (0..n).find(|&k| usable(self, k)) {
                        Some(k) => k,
                        None => return false,
                    }
                }
            }
        };
        self.far.last_put = k;
        self.far.cutters[k] += 1;
        self.far.total += 1;
        true
    }

    /// `count` new cutters at once, when the flock they would drop to is
    /// gone or far: into far sectors with room, from where the last went on
    /// round the body, opening more beside the swarm's reach as needed, the
    /// rest into the last one. How many went (0 if there is a flock near
    /// the camera for them).
    pub(super) fn far_absorb_many(&mut self, count: u64) -> u64 {
        // (No far sector yet: only once the budget is full, then the first
        // ones open beside the swarm's reach.)
        if count == 0 || self.far.cutters.is_empty() || (!self.far.any_far && (self.alive_count as f64) < self.far.budget) {
            return 0;
        }
        // (Near the camera while the budget has room; beyond it, far.)
        let h = self.orbit_hub;
        let room = (self.far.budget - self.alive_count as f64).max(0.0) as u64;
        if h < self.hubs.len() && !self.flock[h].dead && self.far_near_at(self.site_pos(h)) {
            // (Where the camera looks, a few more a second whatever the
            // budget, so its flocks stay full as cutters wear out.)
            let k = self.far_sector_at(self.site_pos(h));
            let short = self.far.must_cover.saturating_sub(self.far.must_flown);
            let room = if self.far.must.get(k).copied().unwrap_or(false) { room.max(NEAR_NEW_PER_S.min(short)) } else { room };
            if count <= room {
                return 0;
            }
            return self.far_rehome(count - room);
        }
        self.far_rehome(count)
    }

    /// `count` cutters into far sectors with room (see `far_absorb_many`).
    /// How many went.
    fn far_rehome(&mut self, count: u64) -> u64 {
        if count == 0 || self.far.cutters.is_empty() {
            return 0;
        }
        let n = self.far_n();
        let start = self.far.last_put.min(n - 1);
        let mut left = count;
        let mut last = None;
        for i in 0..n {
            if left == 0 {
                break;
            }
            let k = (start + i) % n;
            if !(self.far.is_far[k] && self.far.open[k]) {
                continue;
            }
            last = Some(k);
            let take = self.far_room_of(k).min(left);
            self.far.cutters[k] += take;
            left -= take;
            if take > 0 {
                self.far.last_put = k;
            }
        }
        while left > 0 {
            let Some(k) = self.far_open_next() else { break };
            let take = self.far_room_of(k).min(left);
            self.far.cutters[k] += take;
            left -= take;
            self.far.last_put = k;
            last = Some(k);
        }
        if left > 0 {
            // No room anywhere yet: shared out evenly over the far sectors
            // (they wait for room there; see `far_spread`).
            if last.is_none() {
                return count - left;
            }
            let usable: Vec<usize> = (0..n).filter(|&k| self.far.is_far[k] && self.far.open[k]).collect();
            let each = left / usable.len() as u64;
            let mut extra = left - each * usable.len() as u64;
            for &k in &usable {
                let one = (extra > 0) as u64;
                extra -= one;
                self.far.cutters[k] += each + one;
            }
        }
        self.far.total += count;
        count
    }

    /// Cutters sector k's surface keeps at work (COVER_PER_M).
    fn far_cover(&self, k: usize) -> u64 {
        let (a0, a1) = self.far_span(k);
        let r = (self.world.far.levels[k] as f64).min(self.world.planet.max_radius);
        (COVER_PER_M * (a1 - a0) * r) as u64
    }

    /// Once a second: far sectors with more cutters than room hand the
    /// extra on to sectors with room (opening new ones beside the swarm's
    /// reach), a share each second.
    fn far_spread(&mut self) {
        let n = self.far_n();
        let mut extra = 0u64;
        let mut from = None;
        for k in 0..n {
            if !self.far.is_far[k] {
                continue;
            }
            let cover = self.far_cover(k);
            if self.far.cutters[k] > cover + cover / 2 {
                let e = self.far.cutters[k] - cover;
                self.far.cutters[k] -= e;
                extra += e;
                from = Some(k);
            }
        }
        if extra > 0 {
            self.far.total -= extra;
            let put = self.far_rehome(extra);
            if put < extra {
                // (Nowhere at all to go: they stay.)
                if let Some(k) = from {
                    self.far.cutters[k] += extra - put;
                    self.far.total += extra - put;
                }
            }
        }
    }

    /// Cutters sector k has room for (its whole surface worked, COVER_PER_M).
    fn far_room_of(&self, k: usize) -> u64 {
        let (a0, a1) = self.far_span(k);
        let r = (self.world.far.levels[k] as f64).min(self.world.planet.max_radius);
        ((COVER_PER_M * (a1 - a0) * r) as u64).saturating_sub(self.far.cutters[k])
    }

    /// Open the next far sector beside the swarm's reach (at most so many
    /// a second), on dry ground. Its index, or None.
    fn far_open_next(&mut self) -> Option<usize> {
        let most = if self.free_drones.is_some() { 256 * OPEN_PER_S } else { OPEN_PER_S };
        if self.far.opened_now >= most {
            return None;
        }
        let n = self.far_n();
        // Beside an open sector, nearest the last put first.
        let start = self.far.last_put.min(n - 1);
        for i in 0..n {
            for k in [(start + i) % n, (start + n - i) % n] {
                if self.far.open[k] {
                    continue;
                }
                let (l, r) = ((k + n - 1) % n, (k + 1) % n);
                if !(self.far.open[l] || self.far.open[r]) {
                    continue;
                }
                self.far.open[k] = true;
                self.far.opened_now += 1;
                if !self.world.far.levels[k].is_finite() {
                    let (top, wet) = self.far_top(k);
                    self.far.wet[k] = wet > 0.5;
                    let _ = self.world.far_lower(k, top + 64.0, top);
                    self.far_forget(k, top + 64.0, top);
                }
                if self.far_must_near(k) {
                    continue;
                }
                self.far.is_far[k] = true;
                self.far.any_far = true;
                return Some(k);
            }
        }
        None
    }

    /// Room for more cutters in the far sectors the swarm has reached.
    pub(super) fn far_room(&self) -> u64 {
        let n = self.far_n();
        let mut room = 0;
        for k in 0..n {
            if self.far.is_far[k] && self.far.open[k] {
                let (a0, a1) = self.far_span(k);
                let r = (self.world.far.levels[k] as f64).min(self.world.planet.max_radius);
                room += ((COVER_PER_M * (a1 - a0) * r) as u64).saturating_sub(self.far.cutters[k]);
            }
        }
        room
    }

    /// Whether flock-sized work at p is near (not in a far sector).
    pub(super) fn far_near_at(&self, p: (f64, f64)) -> bool {
        if self.far.is_far.is_empty() {
            return true;
        }
        !self.far.is_far[self.far_sector_at(p)]
    }

    /// Far sectors where the cutters struggle: the ground at the top of the
    /// sector (its sampled columns' commonest hardness) cuts `stall_ratio`
    /// times slower than hardness-3 rock, and no far blast's loosened ground
    /// is left there. Worst first: (sector, how much slower).
    pub(super) fn far_stalled(&self) -> Vec<(usize, f64)> {
        let sp = &self.eco.units.sapper;
        if sp.stall_ratio <= 0.0 || self.far.cutters.is_empty() {
            return Vec::new();
        }
        let soft = self.eco.harvester.dig_s_for(3.0);
        let mut out = Vec::new();
        for k in 0..self.far_n() {
            if !self.far.is_far[k] || self.far.cutters[k] == 0 || self.far.wet[k] || self.far.loose_kg[k] > 0.0 {
                continue;
            }
            let lvl = self.world.far.levels[k] as f64;
            if !lvl.is_finite() {
                continue;
            }
            let (a0, a1) = self.far_span(k);
            let mut hs: Vec<u8> = Vec::new();
            for j in 0..SAMPLES {
                let a = a0 + (a1 - a0) * (j as f64 + 0.5) / SAMPLES as f64;
                let r = lvl - 0.5;
                let m = self.world.material_bare((a.cos() * r).floor() as i64, (a.sin() * r).floor() as i64);
                if m != VACUUM && self.world.planet.mats.get(m).state == State::Solid {
                    hs.push(self.world.planet.mats.get(m).hardness);
                }
            }
            if hs.len() < SAMPLES / 2 {
                continue;
            }
            hs.sort_unstable();
            let ratio = self.eco.harvester.dig_s_for(hs[hs.len() / 2]) / soft;
            if ratio >= sp.stall_ratio {
                out.push((k, ratio));
            }
        }
        out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        out
    }

    /// A sapper sent to far sector k: pure math (Benjamin, 25 Nov). Its
    /// loosened column (as deep as its yield wants, blast-wide at the bottom
    /// and loosen_top radii each side at the top) is booked as loose ground
    /// the sector's cutters eat first, fast; and the share of the sector's
    /// cutters working where the column breaks the surface die.
    pub(super) fn far_sapper_blast(&mut self, k: usize) {
        let sp = self.eco.units.sapper.clone();
        let r = self.sapper_radius();
        let depth = self.sapper_depth();
        let lt = sp.loosen_top.max(1.0);
        let (a0, a1) = self.far_span(k);
        let lvl = self.world.far.levels[k] as f64;
        let a = 0.5 * (a0 + a1);
        let r_at = if lvl.is_finite() { lvl - 0.5 } else { self.world.planet.radius };
        let m = self.world.material_bare((a.cos() * r_at).floor() as i64, (a.sin() * r_at).floor() as i64);
        let dens = if m != VACUUM { self.world.planet.mats.get(m).density } else { 2700.0 };
        // Its area: a trapezium, 2r wide at the charge and 2r*lt at the top.
        let cells = depth * r * (1.0 + lt);
        self.far.loose_kg[k] += cells * dens;
        let width = (a1 - a0) * r_at.max(1.0);
        let share = ((2.0 * r * lt + 2.0 * sp.kill_margin_m) / width.max(1.0)).min(1.0);
        let gone = ((self.far.cutters[k] as f64 * share).round() as u64).min(self.far.cutters[k]);
        if gone > 0 {
            self.far.cutters[k] -= gone;
            self.far.total = self.far.total.saturating_sub(gone);
            self.deaths += gone;
            self.recycled += gone;
            let scrap = gone as f64 * self.eco.harvester.mass_cost;
            self.far_credit(scrap, scrap);
        }
        self.far.blasts += 1;
        self.units.blasts += 1;
        self.explosions += 1;
        self.units.blast_kills += gone;
        self.director_log_push(format!("a sapper goes off in far sector {k} (out of sight): {:.0} kt of ground loosened, {gone} cutters killed", cells * dens / 1e6));
    }

    pub(super) fn write_far(&self, o: &mut crate::save::W) {
        let n = self.far.cutters.len();
        o.u64(n as u64);
        for k in 0..n {
            o.u64(self.far.cutters[k]);
            o.f64(self.far.wear[k]);
            o.u8(self.far.open[k] as u8 | (self.far.wet[k] as u8) << 1 | (self.far.is_far[k] as u8) << 2);
        }
        let l = &self.world.far.levels;
        o.u64(l.len() as u64);
        for v in l {
            o.f32(*v);
        }
        o.f64(self.far.eaten_kg);
    }

    pub(super) fn read_far(&mut self, r: &mut crate::save::R) -> Result<(), String> {
        let n = r.u64()? as usize;
        let mut f = Far { cutters: vec![0; n], loose_kg: vec![0.0; n], wear: vec![0.0; n], open: vec![false; n], wet: vec![false; n], is_far: vec![false; n], must: vec![false; n], ..Default::default() };
        for k in 0..n {
            f.cutters[k] = r.u64()?;
            f.wear[k] = r.f64()?;
            let b = r.u8()?;
            f.open[k] = b & 1 != 0;
            f.wet[k] = b & 2 != 0;
            f.is_far[k] = b & 4 != 0;
        }
        let m = r.u64()? as usize;
        let mut levels = Vec::with_capacity(m);
        for _ in 0..m {
            levels.push(r.f32()?);
        }
        f.eaten_kg = r.f64()?;
        f.total = f.cutters.iter().sum();
        let low = levels.iter().filter(|l| l.is_finite()).fold(f64::INFINITY, |a, &l| a.min(l as f64));
        let floor2 = if low.is_finite() { (low - crate::world::FAR_PIT_M - 1.0).max(0.0).powi(2) } else { f64::INFINITY };
        self.world.far = crate::world::FarCut { any: levels.iter().any(|l| l.is_finite()), levels, version: self.world.far.version + 1, floor2, asleep: Vec::new() };
        self.far = f;
        Ok(())
    }
}
