//! The tunneler as a ground unit: the opener, the swarm's vanguard
//! (Benjamin, 25 Nov; docs/design/ground-units.md).
//!
//! - It sees behind the fog: deployed, it heads for the best ground to open
//!   up, weighing what is truly valuable to the space-bound swarm against
//!   what the swarm needs now (an easier deposit of something needed first,
//!   without forsaking the valuable one), and the dig to reach it. Arrived,
//!   it goes on to the next thing further down.
//! - Special targets: the magma layers round the core, and hard strata the
//!   cutters struggle with. It drills through hard rock (cutters then eat
//!   outward from its shaft, from the staging points it leaves). At magma it
//!   can't get through without being engulfed, it caps the breach (the
//!   magma touching its drill frozen to stone), marks it as a job for the
//!   heat-sink bots, backs up its shaft and seeks another deposit.
//! - Its shaft leans at most `max_lean_deg` off straight down, steered as a
//!   gradient, so tunnels curve.
//! - What it digs goes up its shaft in a plume, cleared by its own crew of
//!   cutters (built with it): mass is kept whole. While the crew is too far
//!   behind it waits rather than be buried.
//! - Its nuclear drill heats the rock round its head (on screen, the
//!   chemistry table melts and changes what it can).
//! - With nothing left to open, it waits for other tunnelers to be clear and
//!   goes off where it is: its drill's uranium, a final gift of matter.
//!
//! Stepped once a game second from `Sim::tunnel_work`.

use super::*;
use crate::production::Unit;

/// What an opener is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenState {
    /// Flying down from the mother swarm to where its shaft starts.
    Fly = 0,
    /// Boring toward its target.
    Bore = 1,
    /// Down an existing shaft (a frozen passage through magma) to bore on
    /// from its bottom.
    Descend = 2,
    /// Backing up its shaft after capping magma.
    Back = 3,
    /// Nothing left to open: waiting for other tunnelers to be clear before
    /// it goes off.
    Done = 4,
    /// Nothing left anywhere: back down its shaft to go off deep.
    Sink = 5,
}

/// What it is boring toward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aim {
    /// The best ground to open up (value for the dig).
    Value = 0,
    /// A magma layer, to break for the heat sinks.
    Magma = 1,
    /// Hard rock the cutters struggle with, to open for them.
    Hard = 2,
}

#[derive(Debug, Clone)]
pub struct Opener {
    pub(super) id: u32,
    pub(super) state: OpenState,
    /// Its drill head, and heading (a unit vector).
    pub(super) head: (f64, f64),
    pub(super) heading: (f64, f64),
    pub(super) prev: (f64, f64),
    pub(super) moved_at: u64,
    /// Its shaft's line, a point every few metres from the mouth down.
    pub(super) path: Vec<(f64, f64)>,
    /// Where it is going, and why.
    pub(super) target: (f64, f64),
    pub(super) aim: Aim,
    /// Drilling time banked toward the next metre.
    pub(super) progress_s: f64,
    /// Metres bored all told, and since the last staging point.
    pub(super) length: f64,
    pub(super) since_hub: f64,
    /// Where it backs up to (an index into `path`).
    pub(super) back_to: usize,
    /// Spoil its crew still has to clear (kg by material).
    pub(super) spoil: Vec<(MatId, f64)>,
    pub(super) spoil_kg: f64,
    /// kg dug in the last second (for the plume).
    pub(super) dug_now: f64,
    /// Magma breaches capped, kg dug all told.
    pub(super) caps: u32,
    pub(super) dug_kg: f64,
    /// Seconds with nothing to do (for the end).
    pub(super) idle_s: f64,
}

/// A breach of magma an opener capped: a job for the heat-sink bots. The
/// shaft down to it (its line, mouth first), the way down through the
/// magma (a unit vector), and how it stands.
#[derive(Debug, Clone)]
pub struct HeatJob {
    pub(super) id: u32,
    pub(super) at: (f64, f64),
    pub(super) down: (f64, f64),
    pub(super) shaft: Vec<(f64, f64)>,
    /// A heat-sink bot is on it.
    pub(super) taken: bool,
    /// The passage is frozen and cleared through the magma, down to here:
    /// tunnelers may bore on from its bottom.
    pub(super) through: Option<(f64, f64)>,
    /// An opener has gone on down it.
    pub(super) used: bool,
    /// Heat sinks wrecked on it (given up at two).
    pub(super) wrecks: u32,
}

/// Metres between the points kept of a shaft's line.
pub(super) const PATH_STEP_M: f64 = 4.0;
/// How many of those points short of its shaft's bottom an opener with
/// nothing left to open goes back down to, to go off (80 m: clear of any
/// cap it left there).
const SINK_BACK: usize = 20;

/// An opener's bore, as its blast at the end sees it: its shaft's line
/// from the mouth down to where it is, and on past it as far as it backed
/// up from the bottom (that part of the line is no longer kept; it is
/// taken to run on straight).
fn bore_line(o: &Opener) -> Vec<(f64, f64)> {
    let mut line = o.path.clone();
    if line.last() != Some(&o.head) {
        line.push(o.head);
    }
    let n = line.len();
    let down = if n >= 2 { unit_of((line[n - 1].0 - line[n - 2].0, line[n - 1].1 - line[n - 2].1)) } else { o.heading };
    let back = SINK_BACK as f64 * PATH_STEP_M;
    line.push((o.head.0 + down.0 * back, o.head.1 + down.1 * back));
    line
}

impl Sim {
    /// What a kilogram of each material is worth to an opener now (Benjamin:
    /// a balance between what is truly valuable to the space-bound swarm and
    /// what the current swarm needs). Worked out now and then.
    pub(super) fn opener_values(&mut self) {
        let mats = &self.world.planet.mats;
        let els = &mats.elements;
        let spec = &self.eco.units.tunneler;
        // What the swarm builds most needs: the elements in a cutter's and a
        // hauler's parts, by kg a unit, against what the stockpile holds.
        let mut demand = vec![0.0f64; els.list.len()];
        let r = &self.eco.recipe;
        for (u, many) in [(Unit::Cutter, 1000.0), (Unit::Hauler, 20.0)] {
            let counts = r.counts(r.of(u));
            for (i, &n) in counts.iter().enumerate() {
                if n == 0 {
                    continue;
                }
                for c in &r.part[i].make {
                    if c.of.is_empty() {
                        continue;
                    }
                    let share = c.kg * n as f64 * many / c.of.len() as f64;
                    for ch in &c.of {
                        if let Some(e) = els.index(&ch.el) {
                            demand[e] += share;
                        }
                    }
                }
            }
        }
        let scarce: Vec<f64> = (0..demand.len()).map(|e| if demand[e] > 0.0 { demand[e] / (demand[e] + self.stock.kg[e]) } else { 0.0 }).collect();
        let mut v = vec![0.0f64; mats.len()];
        for (m, out) in v.iter_mut().enumerate() {
            let mat = mats.get(m as MatId);
            if m as MatId == VACUUM || mat.state != State::Solid {
                continue;
            }
            let space = (mat.multiplier - 1.0).max(0.0);
            let need: f64 = mat.composition.iter().filter_map(|(e, k)| els.index(e).map(|i| k * scarce[i])).sum();
            *out = spec.space_weight * space + spec.need_weight * need;
        }
        self.units.mat_value = v;
    }

    /// Worth of the ground round `p` (a few points 8 m apart), per cell.
    fn opener_worth(&self, p: (f64, f64)) -> f64 {
        let mut sum = 0.0;
        for (dx, dy) in [(0.0, 0.0), (8.0, 0.0), (-8.0, 0.0)] {
            let m = self.world.material((p.0 + dx).floor() as i64, (p.1 + dy).floor() as i64);
            sum += self.units.mat_value.get(m as usize).copied().unwrap_or(0.0);
        }
        sum / 3.0
    }

    /// Whether `p` lies past a capped breach no passage has been opened
    /// through yet (deeper than the cap, near under it).
    fn behind_cap(&self, p: (f64, f64)) -> bool {
        let r = len(p);
        self.units.heat_jobs.iter().any(|j| j.through.is_none() && r < len(j.at) + 10.0 && dist2(p, j.at) < 250.0 * 250.0)
    }

    /// The best target for an opener with its head at `from` going down
    /// (its first one if `first`: anywhere under the swarm's reach and
    /// beyond, its shaft starting at the surface over it). Some((point, why,
    /// score)).
    pub(super) fn opener_target(&self, from: (f64, f64), first: bool, me: u32) -> Option<((f64, f64), Aim, f64)> {
        let spec = &self.eco.units.tunneler;
        let min_r = self.world.planet.radius * spec.min_radius.clamp(0.0, 0.95);
        let taken = |p: (f64, f64)| self.units.openers.iter().any(|o| o.id != me && dist2(o.target, p) < 120.0 * 120.0);
        let hot = |c: (i64, i64)| {
            let m = self.world.material(c.0, c.1);
            m != VACUUM && self.units.freeze_to.get(m as usize).copied().flatten().is_some()
        };
        let mut best: Option<((f64, f64), Aim, f64)> = None;
        let consider = |p: (f64, f64), why: Aim, score: f64, best: &mut Option<((f64, f64), Aim, f64)>| {
            if score > spec.min_score && best.is_none_or(|b| score > b.2) && !taken(p) {
                *best = Some((p, why, score));
            }
        };
        let d0 = spec.depth_scale_m.max(1.0);
        if first {
            // Columns across the mother swarm's reach and a margin past it
            // (the vanguard goes ahead), down to max_depth_m.
            let m = self.mother;
            let r0 = self.world.planet.radius.max(1.0);
            let reach = (m.half + spec.ahead_m / r0).min(std::f64::consts::PI);
            // (At most 96 columns, sampled every 32 m: on a big body each
            // look deep down may build ground never looked at before.)
            let n = ((2.0 * reach * r0 / spec.scan_step_m.max(8.0)).ceil() as usize).clamp(8, 96);
            let stalled: Vec<(f64, f64)> = self.stalled_sites().iter().map(|&(i, _)| self.site_pos(i)).collect();
            for j in 0..n {
                let a = m.centre - reach + 2.0 * reach * (j as f64 + 0.5) / n as f64;
                let (ux, uy) = (a.cos(), a.sin());
                let g = self.ground_at(a, true);
                let hard_here = stalled.iter().any(|s| ang_dist(s.1.atan2(s.0), a) * g < 100.0);
                let mut d = 24.0;
                while d <= spec.max_depth_m && g - d > min_r {
                    let p = (ux * (g - d), uy * (g - d));
                    if self.behind_cap(p) {
                        break;
                    }
                    if hot(cell_of(p)) {
                        // A magma layer here: open it for the heat sinks
                        // (unless it is capped already).
                        let at = (ux * (g - d + 6.0), uy * (g - d + 6.0));
                        if !self.units.heat_jobs.iter().any(|jb| dist2(jb.at, at) < 200.0 * 200.0) {
                            consider(at, Aim::Magma, spec.magma_score / (1.0 + d / d0), &mut best);
                        }
                        break;
                    }
                    let w = self.opener_worth(p);
                    consider(p, Aim::Value, w / (1.0 + d / d0), &mut best);
                    if hard_here && d >= 40.0 && d <= 64.0 {
                        consider(p, Aim::Hard, spec.hard_score / (1.0 + d / d0), &mut best);
                    }
                    d += 32.0;
                }
            }
            // A frozen passage through magma no one has gone on down: on
            // from its bottom.
            for jb in &self.units.heat_jobs {
                if let (Some(b), false) = (jb.through, jb.used) {
                    consider(b, Aim::Value, spec.magma_score * 1.5, &mut best);
                }
            }
            return best;
        }
        // From the head: a cone below it, within the lean allowed.
        let down = unit_of((-from.0, -from.1));
        let lean = spec.max_lean_deg.to_radians();
        for k in -4i32..=4 {
            let th = lean * k as f64 / 4.0;
            let (c, s) = (th.cos(), th.sin());
            let dir = (down.0 * c - down.1 * s, down.0 * s + down.1 * c);
            let mut d = 32.0;
            while d <= spec.reach_m {
                let p = (from.0 + dir.0 * d, from.1 + dir.1 * d);
                if len(p) < min_r || self.behind_cap(p) {
                    break;
                }
                let cell = cell_of(p);
                if hot(cell) {
                    let at = (from.0 + dir.0 * (d - 6.0), from.1 + dir.1 * (d - 6.0));
                    if !self.units.heat_jobs.iter().any(|jb| dist2(jb.at, at) < 200.0 * 200.0) {
                        consider(at, Aim::Magma, spec.magma_score / (1.0 + d / d0), &mut best);
                    }
                    break;
                }
                // Value for the dig: what is there, and what it takes to get
                // there (its depth under the head).
                let w = self.opener_worth(p);
                consider(p, Aim::Value, w / (1.0 + d / d0), &mut best);
                d += 32.0;
            }
        }
        best
    }

    /// Whether any opener would find work now (for the director; looked at
    /// now and then, see `opener_scan`).
    pub(super) fn opener_scan(&mut self) {
        if self.units.mat_value.is_empty() || self.tick % self.ticks(60.0) == 0 {
            self.opener_values();
        }
        let from = self.mother_pos();
        self.units.opener_work = self.opener_target(from, true, u32::MAX).is_some();
    }

    pub(super) fn launch_opener(&mut self) {
        if self.units.mat_value.is_empty() {
            self.opener_values();
        }
        let id = self.units.new_id();
        let from = self.mother_pos();
        let Some((target, aim, _)) = self.opener_target(from, true, id) else { return };
        let a = target.1.atan2(target.0);
        // Down a frozen passage, or a new shaft from the ground over its target.
        let job = self.units.heat_jobs.iter().position(|j| j.through == Some(target) && !j.used);
        let (state, path, head) = match job {
            Some(k) => {
                self.units.heat_jobs[k].used = true;
                let p = self.units.heat_jobs[k].shaft.clone();
                let h = *p.first().unwrap_or(&target);
                (OpenState::Descend, p, h)
            }
            None => {
                let g = self.ground_at(a, true);
                (OpenState::Fly, Vec::new(), (a.cos() * (g + 2.0), a.sin() * (g + 2.0)))
            }
        };
        let start = self.mother_over(a);
        let heading = unit_of((-head.0, -head.1));
        self.units.openers.push(Opener {
            id,
            state,
            head: if state == OpenState::Fly { start } else { head },
            heading,
            prev: start,
            moved_at: self.tick,
            path,
            target,
            aim,
            progress_s: 0.0,
            length: 0.0,
            since_hub: 0.0,
            back_to: 0,
            spoil: Vec::new(),
            spoil_kg: 0.0,
            dug_now: 0.0,
            caps: 0,
            dug_kg: 0.0,
            idle_s: 0.0,
        });
        // (Flying, its head is where it is; `path` is empty until it lands
        // at the mouth, which is `head` as worked out above.)
        if state == OpenState::Fly {
            let k = self.units.openers.len() - 1;
            self.units.openers[k].path = Vec::new();
            self.units.openers[k].target = target;
            self.units.opener_mouths.push((id, head));
        }
        let why = match aim {
            Aim::Value => "the best ground to open",
            Aim::Magma => "a magma layer to break",
            Aim::Hard => "hard rock the cutters struggle with",
        };
        self.director_log_push(format!("a tunneler heads for {why}, {:.0} m down", (self.ground_at(a, true) - len(target)).max(0.0)));
    }

    /// One second of every opener's work.
    pub(super) fn openers_work(&mut self) {
        if self.units.openers.is_empty() {
            return;
        }
        let spec = self.eco.units.tunneler.clone();
        let mut blasts: Vec<((f64, f64), Vec<(f64, f64)>)> = Vec::new();
        for k in 0..self.units.openers.len() {
            let mut o = self.units.openers[k].clone();
            o.prev = o.head;
            o.moved_at = self.tick;
            o.dug_now = 0.0;
            self.opener_crew(&mut o, &spec);
            match o.state {
                OpenState::Fly => {
                    let mouth = self.units.opener_mouths.iter().find(|m| m.0 == o.id).map(|m| m.1).unwrap_or(o.head);
                    if self.fly_over(&mut o.head, mouth, spec.fly_speed, 60.0) {
                        o.state = OpenState::Bore;
                        o.path = vec![mouth];
                        o.heading = unit_of((-mouth.0, -mouth.1));
                        self.units.opener_mouths.retain(|m| m.0 != o.id);
                    }
                }
                OpenState::Descend | OpenState::Sink => {
                    // Down the passage's line, then bore on from its bottom.
                    let mut step = spec.fly_speed * 0.5;
                    while step > 0.0 {
                        let i = o.back_to.min(o.path.len().saturating_sub(1));
                        let Some(&to) = o.path.get(i) else {
                            o.state = OpenState::Done;
                            break;
                        };
                        let d = dist2(o.head, to).sqrt();
                        if d > step {
                            approach(&mut o.head, to, step);
                            break;
                        }
                        o.head = to;
                        step -= d;
                        if i + 1 >= o.path.len() {
                            if o.state == OpenState::Sink {
                                o.state = OpenState::Done;
                                o.idle_s = 0.0;
                                break;
                            }
                            o.state = OpenState::Bore;
                            o.heading = unit_of((-o.head.0, -o.head.1));
                            match self.opener_target(o.head, false, o.id) {
                                Some((t, why, _)) => {
                                    o.target = t;
                                    o.aim = why;
                                }
                                None => {
                                    o.state = OpenState::Back;
                                    o.back_to = o.path.len().saturating_sub(1);
                                    o.idle_s = 0.0;
                                }
                            }
                            break;
                        }
                        o.back_to = i + 1;
                    }
                }
                OpenState::Bore => {
                    self.opener_bore(&mut o, &spec);
                }
                OpenState::Back => {
                    // Up its shaft (its line kept), at its flying pace
                    // halved, to the mouth, where it looks all round for
                    // another deposit (a new shaft); with nothing at all, it
                    // goes back down its shaft and goes off there.
                    let mut step = spec.fly_speed * 0.5;
                    while step > 0.0 {
                        let i = o.back_to.min(o.path.len().saturating_sub(1));
                        let Some(&to) = o.path.get(i) else {
                            o.state = OpenState::Done;
                            break;
                        };
                        let d = dist2(o.head, to).sqrt();
                        if d > step {
                            approach(&mut o.head, to, step);
                            o.idle_s += step;
                            break;
                        }
                        o.head = to;
                        o.idle_s += d;
                        step -= d;
                        if i == 0 {
                            o.idle_s = 0.0;
                            match self.opener_target(o.head, true, o.id) {
                                Some((t, why, _)) => {
                                    // A new shaft, from the ground over it.
                                    let a = t.1.atan2(t.0);
                                    let g = self.ground_at(a, true);
                                    let mouth = (a.cos() * (g + 2.0), a.sin() * (g + 2.0));
                                    o.path.clear();
                                    o.target = t;
                                    o.aim = why;
                                    o.state = OpenState::Fly;
                                    self.units.opener_mouths.retain(|m| m.0 != o.id);
                                    self.units.opener_mouths.push((o.id, mouth));
                                }
                                None => {
                                    // Nothing left: back down its shaft, to
                                    // 80 m short of the bottom (clear of any
                                    // cap it left there), to go off.
                                    let keep = o.path.len().saturating_sub(SINK_BACK).max(1);
                                    o.path.truncate(keep);
                                    o.state = OpenState::Sink;
                                    o.back_to = 0;
                                }
                            }
                            break;
                        }
                        o.back_to = i - 1;
                    }
                }
                OpenState::Done => {
                    // Its crew finishes first; then, with no other tunneler
                    // within reach of its blast, it goes off.
                    o.idle_s += 1.0;
                    let r = self.eco.units.sapper.radius_m_per_cbrt_kt * spec.yield_kt.max(0.0).cbrt();
                    let clear = 2.6 * r;
                    let near = self.units.openers.iter().any(|p| p.id != o.id && p.state != OpenState::Done && dist2(p.head, o.head) < clear * clear);
                    if o.spoil_kg <= 0.0 && !near && o.idle_s >= 3.0 {
                        blasts.push((o.head, bore_line(&o)));
                        o.idle_s = -1.0;
                    }
                }
            }
            self.units.openers[k] = o;
        }
        self.units.openers.retain(|o| o.idle_s >= 0.0);
        for (at, bore) in blasts {
            self.units.tunneler_blasts += 1;
            self.director_log_push("a tunneler has nothing left to open: it goes off where it is".to_string());
            let fissile = spec.fissile_kg;
            self.unit_blast(at, spec.yield_kt, fissile, &bore, spec.bore_m / 2.0);
        }
    }

    /// Its crew clears a second's spoil (credited to the swarm as it comes
    /// up), and on screen the spoil blasts up out of the shaft's mouth.
    fn opener_crew(&mut self, o: &mut Opener, spec: &crate::economy::TunnelerUnitSpec) {
        if o.spoil_kg <= 0.0 {
            return;
        }
        let can = spec.crew as f64 * spec.crew_kg_per_s;
        let share = (can / o.spoil_kg).min(1.0);
        let mut cleared = Vec::new();
        for e in o.spoil.iter_mut() {
            let take = e.1 * share;
            e.1 -= take;
            cleared.push((e.0, take));
        }
        o.spoil.retain(|e| e.1 > 1e-6);
        o.spoil_kg = o.spoil.iter().map(|e| e.1).sum();
        for (m, kg) in cleared {
            let d = self.world.planet.mats.get(m).density.max(1.0);
            self.credit(m, kg / d);
        }
    }

    /// A second of boring for opener `o`.
    fn opener_bore(&mut self, o: &mut Opener, spec: &crate::economy::TunnelerUnitSpec) {
        // Its crew too far behind: it waits rather than be buried.
        if o.spoil_kg > spec.crew_backlog_t * 1000.0 {
            return;
        }
        if self.brownout() {
            return;
        }
        let power = spec.drill_power * self.up("dig speed") * self.perk_dig;
        let cargo = self.eco.harvester.cargo_kg * self.up_cached_cargo;
        let turn = spec.turn_deg_per_m.to_radians();
        let max_lean = spec.max_lean_deg.to_radians();
        let half = spec.bore_m / 2.0;
        o.progress_s += 1.0;
        for _ in 0..4 {
            // Steer toward the target a little each metre (a gradient: the
            // shaft curves), never leaning past max_lean_deg.
            let want = unit_of((o.target.0 - o.head.0, o.target.1 - o.head.1));
            let cross = o.heading.0 * want.1 - o.heading.1 * want.0;
            let ang = cross.clamp(-1.0, 1.0).asin().clamp(-turn, turn);
            let (c, s) = (ang.cos(), ang.sin());
            let mut h = (o.heading.0 * c - o.heading.1 * s, o.heading.0 * s + o.heading.1 * c);
            let down = unit_of((-o.head.0, -o.head.1));
            let lean = (down.0 * h.1 - down.1 * h.0).asin();
            if lean.abs() > max_lean || down.0 * h.0 + down.1 * h.1 < 0.0 {
                let l = lean.clamp(-max_lean, max_lean);
                let (c, s) = (l.cos(), l.sin());
                h = (down.0 * c - down.1 * s, down.0 * s + down.1 * c);
            }
            let next = (o.head.0 + h.0, o.head.1 + h.1);
            let (cells, _) = self.bore_cells(next.0, next.1, half);
            // Magma within reach of its drill: it can't go on without being
            // engulfed. It caps the breach and backs off.
            if self.magma_near(next, half + 2.0) {
                self.opener_cap(o, next, spec);
                return;
            }
            let (mut need_s, mut tonnes) = (0.0, 0.0);
            for c in &cells {
                let mat = self.world.planet.mats.get(self.world.material(c.0, c.1));
                need_s += self.eco.harvester.dig_s_for(mat.hardness) * mat.density / cargo / power.max(1e-9);
                tonnes += mat.density / 1000.0;
            }
            let need_e = tonnes * spec.energy_per_tonne;
            if o.progress_s < need_s || self.energy - need_e < 5.0 * self.upkeep() {
                break;
            }
            self.energy -= need_e;
            o.progress_s -= need_s;
            for c in cells {
                let m = self.world.material(c.0, c.1);
                if m == VACUUM {
                    continue;
                }
                let kg = self.world.planet.mats.get(m).density;
                self.world.mine_cell(c.0, c.1);
                self.undope(c, false);
                self.progress.remove(&c);
                self.reserved.remove(&c);
                self.open_cell(c);
                match o.spoil.iter_mut().find(|e| e.0 == m) {
                    Some(e) => e.1 += kg,
                    None => o.spoil.push((m, kg)),
                }
                o.spoil_kg += kg;
                o.dug_kg += kg;
                o.dug_now += kg;
                self.tunnel_kg += kg;
            }
            // The rock round the shaft is cracked (cutters take it faster).
            let halo = half + self.eco.tunneler.fracture_m;
            let r = halo.ceil() as i64 + 1;
            let (cx, cy) = (next.0.floor() as i64, next.1.floor() as i64);
            for yy in cy - r..=cy + r {
                for xx in cx - r..=cx + r {
                    let (dx, dy) = (xx as f64 + 0.5 - next.0, yy as f64 + 0.5 - next.1);
                    if dx * dx + dy * dy <= halo * halo && self.world.material(xx, yy) != VACUUM {
                        self.fractured.insert((xx, yy));
                    }
                }
            }
            o.head = next;
            o.heading = h;
            o.length += 1.0;
            o.since_hub += 1.0;
            self.tunnel_m += 1;
            if o.path.last().is_none_or(|p| dist2(*p, next) >= PATH_STEP_M * PATH_STEP_M) {
                o.path.push(next);
            }
            // A staging point now and then: cutters eat outward from the
            // shaft (through hard strata too).
            if o.since_hub >= spec.hub_every_m {
                o.since_hub = 0.0;
                self.add_shaft_hub(next);
            }
            if dist2(o.head, o.target) < 8.0 * 8.0 {
                // There: on to the next thing further down.
                if o.aim == Aim::Hard {
                    self.add_shaft_hub(o.head);
                }
                match self.opener_target(o.head, false, o.id) {
                    Some((t, why, _)) => {
                        o.target = t;
                        o.aim = why;
                    }
                    None => {
                        // Nothing further down: back up its shaft to look
                        // elsewhere.
                        o.state = OpenState::Back;
                        o.back_to = o.path.len().saturating_sub(1);
                        o.idle_s = 0.0;
                        return;
                    }
                }
            }
        }
        o.progress_s = o.progress_s.min(60.0);
        // Its nuclear drill heats what is round its head (on screen only:
        // the sandbox steps only what is seen).
        if o.dug_now > 0.0 && self.sandbox.sees(cell_of(o.head)) {
            let ring = half + 1.5;
            for j in 0..8 {
                let a = j as f64 * std::f64::consts::TAU / 8.0 + self.tick as f64 * 0.37;
                let c = cell_of((o.head.0 + a.cos() * ring, o.head.1 + a.sin() * ring));
                if self.world.material(c.0, c.1) != VACUUM {
                    self.sandbox.heat_cell(&self.world, c, spec.drill_heat_k);
                }
            }
        }
        // The plume out of the mouth: as big as what it dug this second.
        if o.dug_now > 0.0 {
            if let Some(&mouth) = o.path.first() {
                if self.sandbox.sees(cell_of(mouth)) && self.plumes.len() < 100 && self.tick % self.ticks(2.0) == 0 {
                    let pw = (o.dug_now / 20_000.0).sqrt().clamp(0.4, 4.0) as f32;
                    self.plumes.push((mouth.0, mouth.1, pw));
                }
            }
        }
    }

    /// Hot liquid within `r` of `p`.
    pub(super) fn magma_near(&self, p: (f64, f64), r: f64) -> bool {
        let ri = r.ceil() as i64;
        let c0 = cell_of(p);
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                if (dx * dx + dy * dy) as f64 > r * r {
                    continue;
                }
                let m = self.world.material(c0.0 + dx, c0.1 + dy);
                if m != VACUUM && self.units.freeze_to.get(m as usize).copied().flatten().is_some() {
                    return true;
                }
            }
        }
        false
    }

    /// Cap a breach of magma ahead of opener `o` at `at`: the magma within
    /// cap_m of its drill frozen to stone (the capping charge: mass kept,
    /// the cell only changes state), a job for the heat sinks, and it backs
    /// up its shaft.
    fn opener_cap(&mut self, o: &mut Opener, at: (f64, f64), spec: &crate::economy::TunnelerUnitSpec) {
        let r = spec.bore_m / 2.0 + spec.cap_m;
        let ri = r.ceil() as i64;
        let c0 = cell_of(at);
        let mut changes = Vec::new();
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                if (dx * dx + dy * dy) as f64 > r * r {
                    continue;
                }
                let c = (c0.0 + dx, c0.1 + dy);
                let m = self.world.material(c.0, c.1);
                let Some(to) = self.units.freeze_to.get(m as usize).copied().flatten() else { continue };
                if self.world.change_cell(c.0, c.1, to).is_some() {
                    changes.push(Change { cell: c, from: m, to });
                    self.world.set_look(c.0, c.1, crate::world::Look::GLASS);
                    // (Chilled and left at rest, as a heat sink leaves what
                    // it freezes.)
                    self.sandbox.temps.insert(c, CHILL_K);
                }
            }
        }
        if !changes.is_empty() {
            self.apply_sandbox(changes);
        }
        o.caps += 1;
        self.units.caps += 1;
        let mut shaft = o.path.clone();
        shaft.push(o.head);
        // Near a breach already known (a passage that was not through after
        // all): that job opens again, from here.
        if let Some(j) = self.units.heat_jobs.iter_mut().find(|j| dist2(j.at, o.head) < 40.0 * 40.0) {
            j.at = o.head;
            j.down = o.heading;
            j.shaft = shaft;
            j.through = None;
            j.used = false;
        } else {
            let id = self.units.new_id();
            self.units.heat_jobs.push(HeatJob { id, at: o.head, down: o.heading, shaft, taken: false, through: None, used: false, wrecks: 0 });
        }
        self.director_log_push(format!("a tunneler reaches magma {:.0} m down and caps it: a job for the heat sinks", self.ground_beside(o.head, 20.0) - len(o.head)));
        o.state = OpenState::Back;
        o.back_to = o.path.len().saturating_sub(1);
        o.idle_s = 0.0;
    }

    /// A staging point for cutters at `p` down a shaft.
    pub(super) fn add_shaft_hub(&mut self, p: (f64, f64)) {
        self.hubs.push(Hub { x: p.0, y: p.1, kind: HubKind::Shaft, ring: None });
        self.hub_load.push(0);
        self.hub_dry.push(false);
        self.hub_gen += 1;
        self.site_wait.push(Vec::new());
        self.flock.push(FlockStats::default());
    }

    /// A unit's charge going off where it is (a tunneler's end): the
    /// sapper's blast at this yield, with no loosened column to the surface
    /// (Benjamin: depth is irrelevant, only the yield).
    /// `bore`: its own bore's line, reaching `half` m either side of it,
    /// whose open space carries the blast's shock as the rock round it would.
    pub(super) fn unit_blast(&mut self, at: (f64, f64), yield_kt: f64, fissile: f64, bore: &[(f64, f64)], half: f64) {
        let keep = self.eco.units.sapper.depth_m_per_cbrt_kt;
        self.eco.units.sapper.depth_m_per_cbrt_kt = 0.0;
        self.blast_in_hole(at, yield_kt, fissile, bore, half);
        self.eco.units.sapper.depth_m_per_cbrt_kt = keep;
    }

    /// The openers as the screen sees them: state, head moving from prev,
    /// heading (a, b), crew backlog share (c), mouth (mark), target (claw),
    /// crew size (lift).
    pub(super) fn opener_views(&self, out: &mut Vec<UnitView>) {
        let t = |tick: u64| tick as f64 * self.eco.tick_s;
        let sec = self.ticks(1.0) as f64 * self.eco.tick_s;
        let spec = &self.eco.units.tunneler;
        for o in &self.units.openers {
            let mouth = o.path.first().copied().or_else(|| self.units.opener_mouths.iter().find(|m| m.0 == o.id).map(|m| m.1)).unwrap_or(o.head);
            out.push(UnitView {
                kind: Unit::Tunneler,
                id: o.id,
                state: o.state as u8,
                from: o.prev,
                to: o.head,
                t0: t(o.moved_at),
                t1: t(o.moved_at) + sec,
                a: o.heading.0 as f32,
                b: o.heading.1 as f32,
                c: (o.spoil_kg / (spec.crew_backlog_t * 1000.0).max(1.0)) as f32,
                mark: mouth,
                lift: spec.crew as f32,
                sweep_from: (o.dug_now, o.length),
                fired_at: -1e9,
                claw: o.target,
                cut_at: -1e9,
            });
        }
    }
}
