//! The fleet commander: air traffic control for the haulers (Benjamin, 14
//! Nov). Haulers aren't tied to flocks but to the work:
//! - the director's figure for each flock's coming volume (what its
//!   cutters have been cutting lately) sets how many drop-off points it
//!   gets: one, or more while a single hauler would fill too fast; they hang
//!   in clear air over the flock's work, spread along its face, at least a
//!   hauler's length apart from any other (stacked higher where there's no
//!   room), so cutters on the ground fly beneath them;
//! - each point calls for a hauler ahead of time: when it has none, or the
//!   one there will be full by the time another could arrive. Calls go on
//!   one stack for the whole swarm, and the newest is answered first
//!   (LIFO), so the busiest flocks, which call most often, get most of
//!   the fleet;
//! - free haulers wait at the mother swarm; the nearest answers each call.
//!   A relief arriving sends the one it relieves up to unload. New haulers
//!   are built only for calls no hauler will be free for soon; one idle
//!   too long is recycled.
//!
//! Unit file: the platform calls `fleet_work` once a game second (from
//! `hauler_work`).

use super::*;

/// A drop-off point asking for a hauler: flock `site`, its point `dock`,
/// since tick `at`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Call {
    pub site: usize,
    pub dock: usize,
    pub at: u64,
}

/// The fleet commander's books, for the F1 panel.
#[derive(Debug, Clone, Copy, Default)]
pub struct FleetBooks {
    pub calls: u64,
    pub answered: u64,
    pub built: u64,
    pub recycled: u64,
    pub reliefs: u64,
}

/// A drop-off point stays where it is while it is within this far (m, along
/// the ground) of where it would be placed now, and still clear.
const DOCK_KEEP_M: f64 = 40.0;

impl Sim {
    /// Where flock h's drop-off point d hangs (the flock's landing point if
    /// it has none yet).
    pub(super) fn dock_pos(&self, h: usize, d: usize) -> (f64, f64) {
        self.docks.get(h).and_then(|v| v.get(d)).copied().unwrap_or_else(|| self.site_pos(h))
    }

    /// The drop-off point of flock h a cutter at `from` should take its
    /// load to: the nearest with a hauler there (or on its way), else the
    /// nearest.
    /// Visit every hauler of flock h (by the index, unless the list has
    /// changed since it was made: then all of them, filtered).
    pub(super) fn each_hauler_of(&self, h: usize, mut f: impl FnMut(usize, &Hauler)) {
        if self.haulers_moved {
            for (i, hl) in self.haulers.iter().enumerate() {
                if hl.site == h {
                    f(i, hl);
                }
            }
        } else if let Some(v) = self.hauler_by_site.get(h) {
            for &i in v {
                f(i, &self.haulers[i]);
            }
        }
    }

    pub(super) fn pick_dock(&self, h: usize, from: (f64, f64)) -> usize {
        let Some(v) = self.docks.get(h) else { return 0 };
        let cap = self.eco.hauler.capacity_kg * self.up_cached_cargo;
        // (One that the loads already on their way would fill doesn't
        // count: it is as good as full.)
        let coming = |d: usize| self.incoming.get(&(h, d)).copied().unwrap_or(0.0);
        let served = |d: usize| {
            let mut yes = false;
            self.each_hauler_of(h, |_, hl| yes |= hl.alive && hl.dock == d && matches!(hl.state, HaulerState::Returning | HaulerState::Working) && hl.load + coming(d) < cap);
            yes
        };
        let near = |list: &mut dyn Iterator<Item = usize>| list.min_by(|&a, &b| dist2(from, v[a]).total_cmp(&dist2(from, v[b])));
        near(&mut (0..v.len()).filter(|&d| served(d))).or_else(|| near(&mut (0..v.len()))).unwrap_or(0)
    }

    /// A working hauler at flock h's drop-off point d with room, the emptiest.
    pub(super) fn hauler_at_dock(&self, h: usize, d: usize) -> Option<usize> {
        let cap = self.eco.hauler.capacity_kg * self.up_cached_cargo;
        let mut best: Option<(usize, f64)> = None;
        self.each_hauler_of(h, |k, hl| {
            if hl.alive && hl.dock == d && hl.state == HaulerState::Working && hl.load < cap && best.is_none_or(|(_, l)| hl.load < l) {
                best = Some((k, hl.load));
            }
        });
        best.map(|b| b.0)
    }

    /// The flock's coming volume as the director sees it (kg a second):
    /// what its cutters have cut lately.
    pub(super) fn flock_rate(&self, h: usize) -> f64 {
        self.flock[h].kg / self.eco.flock.memory_s.max(1.0)
    }

    /// Place every live flock's drop-off points: how many from its volume,
    /// spread along its face over its work, high in clear air, and no two
    /// (of any flocks) closer than dock_gap_m (a later one is stacked
    /// higher). Haulers at a point that's gone go up to unload.
    pub(super) fn place_docks(&mut self) {
        let fs = self.eco.fleet.clone();
        let cap = self.eco.hauler.capacity_kg * self.up_cached_cargo;
        let n = self.hubs.len();
        if self.docks.len() < n {
            self.docks.resize(n, Vec::new());
        }
        let was = self.docks.clone();
        // Points placed so far, by square dock_gap_m wide (only the 3x3
        // round a point can be within dock_gap_m of it).
        let gap = fs.dock_gap_m.max(1.0);
        let key = |p: (f64, f64)| ((p.0 / gap).floor() as i64, (p.1 / gap).floor() as i64);
        let mut grid: HashMap<(i64, i64), Vec<(f64, f64)>> = HashMap::new();
        let near = |grid: &HashMap<(i64, i64), Vec<(f64, f64)>>, p: (f64, f64)| {
            let (kx, ky) = key(p);
            (-1..=1).any(|dx| (-1..=1).any(|dy| grid.get(&(kx + dx, ky + dy)).is_some_and(|v| v.iter().any(|&q| dist2(q, p) < gap * gap))))
        };
        for h in 0..n {
            if self.flock[h].dead || self.hubs[h].kind != HubKind::Seed {
                self.docks[h].clear();
                continue;
            }
            let rate = self.flock_rate(h);
            let want = ((rate * fs.fill_min_s / cap.max(1.0)).ceil() as usize).clamp(1, fs.max_docks.max(1));
            self.flock[h].want = want as u32;
            let f = self.flock[h];
            let centre = if f.w > 0.0 { (f.sx / f.w, f.sy / f.w) } else { self.site_pos(h) };
            let rc = len(centre).max(1.0);
            let a0 = centre.1.atan2(centre.0);
            let mut list = Vec::with_capacity(want);
            for i in 0..want {
                let a = a0 + (i as f64 - (want - 1) as f64 / 2.0) * fs.dock_spread_m / rc;
                // Where it was is still good (within DOCK_KEEP_M of where it
                // would go now, along the ground, with clear air half its
                // hover under it, and room): it stays. Points that shuffle
                // with every shift in where the flock cuts send their
                // waiting cutters back and forth for nothing.
                if let Some(&o) = was.get(h).and_then(|v| v.get(i)) {
                    let ao = o.1.atan2(o.0);
                    let da = (ao - a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
                    if da.abs() * rc <= DOCK_KEEP_M && !near(&grid, o) && self.clear_under(o, 0.5 * fs.dock_hover_m) {
                        grid.entry(key(o)).or_default().push(o);
                        list.push(o);
                        continue;
                    }
                }
                // Clear of the ground under it and a hauler's length either side.
                let half = 0.5 * fs.dock_gap_m / rc;
                let g = [a - half, a, a + half].iter().map(|&b| self.ground_at(b, false)).fold(0.0, f64::max);
                let (ux, uy) = (a.cos(), a.sin());
                let mut r = g + fs.dock_hover_m;
                // A hauler's length clear of every point placed already.
                let top = g + fs.dock_stack_max_m.max(fs.dock_hover_m);
                while r < top && near(&grid, (ux * r, uy * r)) {
                    r += fs.dock_gap_m;
                }
                let mut p = (ux * r, uy * r);
                // A small shift isn't worth the traffic: it stays put.
                if let Some(&o) = was.get(h).and_then(|v| v.get(i)) {
                    let keep = 0.5 * fs.dock_gap_m;
                    if dist2(o, p) < keep * keep && !near(&grid, o) {
                        p = o;
                    }
                }
                grid.entry(key(p)).or_default().push(p);
                list.push(p);
            }
            self.docks[h] = list;
        }
        // Cutters going to (or waiting at) a point that moved go after it.
        let mut moved = std::collections::HashSet::new();
        for h in 0..was.len() {
            for (d, &o) in was[h].iter().enumerate() {
                if self.docks.get(h).and_then(|v| v.get(d)).is_none_or(|&p| dist2(o, p) > 0.25) {
                    moved.insert((h, d));
                }
            }
        }
        if !moved.is_empty() {
            let t = crate::swarm::clock();
            self.docks_moved(&moved, was);
            self.prof4[1] += crate::swarm::clock() - t;
            self.moved_docks += moved.len() as u64;
        }
        // Haulers at a point that's gone: up to unload (and then free).
        for k in 0..self.haulers.len() {
            let hl = &self.haulers[k];
            if !hl.alive || !matches!(hl.state, HaulerState::Working | HaulerState::Returning) {
                continue;
            }
            if hl.dock >= self.docks.get(hl.site).map_or(0, |v| v.len()) {
                self.release_hauler(k);
            }
        }
    }

    /// Open space (nothing but vacuum) for `depth` m straight down under p.
    fn clear_under(&self, p: (f64, f64), depth: f64) -> bool {
        let r = len(p).max(1.0);
        let (ux, uy) = (p.0 / r, p.1 / r);
        (0..=depth.ceil() as i64).all(|k| {
            let q = r - k as f64;
            self.world.material((ux * q).floor() as i64, (uy * q).floor() as i64) == VACUUM
        })
    }

    /// Hauler k leaves its drop-off point: up to the mother swarm to unload
    /// what it has (then it's free for the next call).
    pub(super) fn release_hauler(&mut self, k: usize) {
        let here = self.hauler_pos(k);
        let mother = self.rendezvous(self.haulers[k].site.min(self.hubs.len() - 1));
        let top = self.world.planet.max_radius;
        let speed = self.eco.hauler.speed.max(1.0);
        let now = self.tick;
        let tick_s = self.eco.tick_s;
        let hl = &mut self.haulers[k];
        hl.state = HaulerState::Lifting;
        hl.lift_at = now;
        hl.from = here;
        hl.to = mother;
        hl.t0 = now;
        hl.t1 = now + ((route_len(here, mother, route_cruise(here, mother, top)) / speed) / tick_s).ceil() as u64;
        self.haulers_moved = true;
    }

    /// Where hauler k is now.
    pub(super) fn hauler_pos(&self, k: usize) -> (f64, f64) {
        let hl = &self.haulers[k];
        let t = if hl.t1 > hl.t0 { (self.tick.saturating_sub(hl.t0) as f64 / (hl.t1 - hl.t0) as f64).clamp(0.0, 1.0) } else { 1.0 };
        let k = t * t * (3.0 - 2.0 * t);
        match hl.state {
            // (Drifting after its point: straight, eased.)
            HaulerState::Working => (hl.from.0 + (hl.to.0 - hl.from.0) * k, hl.from.1 + (hl.to.1 - hl.from.1) * k),
            HaulerState::Idle | HaulerState::Unloading => hl.to,
            _ => route_at(hl.from, hl.to, route_cruise(hl.from, hl.to, self.world.planet.max_radius), k),
        }
    }

    /// One second of air traffic control: place the drop-off points (every
    /// few seconds), take calls, answer them newest first, build haulers
    /// for calls no one will be free for soon, recycle idle ones.
    pub(super) fn fleet_work(&mut self) {
        // (The haulers by flock, fresh: the list was just tidied.)
        let c0 = crate::swarm::clock();
        self.reindex();
        self.catch_up();
        let c1 = crate::swarm::clock();
        let fs = self.eco.fleet.clone();
        let spec = self.eco.hauler.clone();
        let cap = spec.capacity_kg * self.up_cached_cargo;
        let dt = self.ticks(1.0) as f64 * self.eco.tick_s;
        // The director's books on each flock (for the panel and its notes).
        let fade = (-dt / DIRECTOR_MEMORY_S).exp();
        for site in 0..self.hubs.len() {
            let waiting = self.site_wait[site].len();
            let f = &mut self.flock[site];
            f.wait_cs = f.wait_cs * fade + waiting as f64 * dt;
            f.filled *= fade;
        }
        let c2 = crate::swarm::clock();
        self.sort_adrift();
        let c3 = crate::swarm::clock();
        let new_flock = (0..self.hubs.len()).any(|h| !self.flock[h].dead && self.hubs[h].kind == HubKind::Seed && self.docks.get(h).is_none_or(|v| v.is_empty()));
        if new_flock || self.tick % self.ticks(5.0).max(1) == 0 {
            self.place_docks();
        }
        let c4 = crate::swarm::clock();
        // Calls: a point with no hauler, or whose hauler will be full by
        // the time a relief from the mother swarm could get there.
        self.calls.retain(|c| self.docks.get(c.site).is_some_and(|v| c.dock < v.len()));
        let now = self.tick;
        let tick_s = self.eco.tick_s;
        let mut serving: HashMap<(usize, usize), (u32, f64)> = HashMap::new();
        for hl in self.haulers.iter().filter(|hl| hl.alive) {
            match hl.state {
                HaulerState::Returning => serving.entry((hl.site, hl.dock)).or_insert((0, f64::INFINITY)).0 += 1,
                HaulerState::Working => {
                    let e = serving.entry((hl.site, hl.dock)).or_insert((0, f64::INFINITY));
                    // Seconds until it lifts: full, or its stay is up.
                    let rate = self.flock_rate(hl.site) / self.docks.get(hl.site).map_or(1, |v| v.len().max(1)) as f64;
                    let stayed = now.saturating_sub(hl.since) as f64 * tick_s;
                    let coming = self.incoming.get(&(hl.site, hl.dock)).copied().unwrap_or(0.0);
                    let left = ((cap - hl.load - coming).max(0.0) / rate.max(1e-6)).min((spec.max_stay_s - stayed).max(0.0));
                    e.1 = e.1.min(left);
                }
                _ => {}
            }
        }
        let called: std::collections::HashSet<(usize, usize)> = self.calls.iter().map(|c| (c.site, c.dock)).collect();
        for h in 0..self.docks.len() {
            for d in 0..self.docks[h].len() {
                if called.contains(&(h, d)) {
                    continue;
                }
                let need = match serving.get(&(h, d)) {
                    None => true,
                    // Only ones working there: the soonest to lift is relieved in time.
                    Some(&(coming, lifts_in)) => coming == 0 && {
                        let p = self.docks[h][d];
                        let eta = self.hauler_way(self.rendezvous(h), p) / spec.speed.max(1.0);
                        lifts_in <= eta + fs.call_margin_s
                    },
                };
                if need {
                    self.calls.push(Call { site: h, dock: d, at: now });
                    self.fleet_books.calls += 1;
                }
            }
        }
        let c5 = crate::swarm::clock();
        // Answer the newest call first, each with the nearest free hauler.
        let mut free: Vec<usize> = (0..self.haulers.len()).filter(|&k| self.haulers[k].alive && self.haulers[k].state == HaulerState::Idle).collect();
        while !free.is_empty() {
            let Some(c) = self.calls.pop() else { break };
            let p = self.dock_pos(c.site, c.dock);
            let (i, _) = free.iter().enumerate().min_by(|a, b| dist2(self.haulers[*a.1].to, p).total_cmp(&dist2(self.haulers[*b.1].to, p))).expect("not empty");
            let k = free.swap_remove(i);
            self.send_to_dock(k, c.site, c.dock);
            self.fleet_books.answered += 1;
        }
        // Calls left: build for those no hauler on its way up will be free for soon.
        let soon = self.haulers.iter().filter(|hl| hl.alive && matches!(hl.state, HaulerState::Lifting | HaulerState::Unloading)).count() + self.prod.queued(crate::production::Unit::Hauler) + self.prod.building(crate::production::Unit::Hauler);
        let short = self.calls.len().saturating_sub(soon);
        // (Benchmarks with a fixed swarm build as many as it takes.)
        let most = if self.free_drones.is_some() { 64 } else { 4 };
        for i in 0..short.min(most) {
            let c = self.calls[self.calls.len() - 1 - i];
            if self.free_drones.is_none() && !self.pay_hauler() {
                self.director_note(c.site, format!("flock {}: a hauler called for, can't afford one", c.site));
                break;
            }
            self.send_hauler(c.site);
            self.fleet_books.built += 1;
            if !self.factory() {
                self.haulers_built += 1;
            }
            self.director_log_push(format!("flock {}: a hauler built for its call ({} calls waiting)", c.site, self.calls.len()));
        }
        let c6 = crate::swarm::clock();
        self.prof4[0] += c1 - c0;
        self.prof4[1] += c2 - c1;
        self.prof4[2] += c3 - c2;
        self.prof4[3] += c4 - c3;
        self.prof4[4] += c5 - c4;
        self.prof4[5] += c6 - c5;
        // Idle too long at the mother swarm: recycled.
        for k in 0..self.haulers.len() {
            let hl = &self.haulers[k];
            if hl.alive && hl.state == HaulerState::Idle && now.saturating_sub(hl.t1) as f64 * tick_s >= fs.idle_recycle_s {
                self.haulers[k].alive = false;
                self.haulers_retired += 1;
                self.fleet_books.recycled += 1;
                let m = self.eco.hauler.mass_cost;
                self.mass += m;
                self.recovered_mass += m;
                self.haulers_moved = true;
            }
        }
    }

    /// Send hauler k to flock h's drop-off point d.
    pub(super) fn send_to_dock(&mut self, k: usize, h: usize, d: usize) {
        let from = self.hauler_pos(k);
        let to = self.dock_pos(h, d);
        let t1 = self.tick + self.ticks(self.hauler_way(from, to) / self.eco.hauler.speed.max(1.0));
        let hl = &mut self.haulers[k];
        hl.site = h;
        hl.dock = d;
        hl.state = HaulerState::Returning;
        hl.from = from;
        hl.to = to;
        hl.t0 = self.tick;
        hl.t1 = t1;
        self.haulers_moved = true;
    }

    /// For tests: kg counted on its way to drop-off points, and kg actually
    /// in the holds of live cutters flying to one.
    pub fn incoming_check(&self) -> (f64, f64) {
        let counted = self.incoming.values().sum();
        let flying = self.drones.iter().filter(|d| d.alive).map(|d| d.promised).sum();
        (counted, flying)
    }

    /// The fleet commander, for the F1 panel.
    pub fn fleet_lines(&self) -> Vec<String> {
        let b = self.fleet_books;
        let mut st = [0u32; 5];
        for hl in self.haulers.iter().filter(|h| h.alive) {
            st[match hl.state {
                HaulerState::Working => 0,
                HaulerState::Returning => 1,
                HaulerState::Lifting => 2,
                HaulerState::Unloading => 3,
                HaulerState::Idle => 4,
            }] += 1;
        }
        let docks: usize = self.docks.iter().map(|v| v.len()).sum();
        vec![
            format!("fleet commander: {docks} drop-off points · {} calls waiting", self.calls.len()),
            format!("  haulers: {} at a point, {} on the way, {} lifting, {} unloading, {} free", st[0], st[1], st[2], st[3], st[4]),
            format!("  calls {} · answered {} · built for {} · reliefs {} · recycled {}", b.calls, b.answered, b.built, b.reliefs, b.recycled),
            format!("cutters: {} loose bodies grabbed out of the air · {} on the way to one", self.grabbed, self.grabs.len()),
        ]
    }
}
