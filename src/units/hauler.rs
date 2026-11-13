//! Haulers: big drones that hover over a flock, take the cutters' loads,
//! lift full to the mother swarm, unload and come back down. Also how many
//! a flock gets, and what the screen draws of them (`HaulerView`).
//!
//! Unit file: the hauler's own behaviour; the platform calls `hauler_work`
//! once a game second.

use super::*;

/// What a hauler is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HaulerState {
    /// Hovering over its flock, taking loads.
    Working,
    /// Flying up to the mother swarm, full.
    Lifting,
    Unloading,
    /// Flying back down to its flock (or down for the first time).
    Returning,
}

/// A hauler: a big drone serving one flock (`site`, a landing point).
#[derive(Debug, Clone)]
pub struct Hauler {
    pub site: usize,
    pub state: HaulerState,
    /// kg aboard, and what it is worth credited.
    pub load: f64,
    pub credited: f64,
    /// The flight under way: from, to, and when it leaves and arrives (ticks).
    pub from: (f64, f64),
    pub to: (f64, f64),
    pub t0: u64,
    pub t1: u64,
    pub life_left: f64,
    pub alive: bool,
    /// When it last lifted off for the mother swarm (tick), to time its trip.
    pub lift_at: u64,
    /// What its load is made of (for drawing).
    pub mix: Mix6,
}

/// A hauler as the screen needs it: it flies from `from` to `to` between
/// game times t0 and t1 (easing in and out), and is `fill` full (0 to 1).
#[derive(Debug, Clone, Copy)]
pub struct HaulerView {
    pub from: (f64, f64),
    pub to: (f64, f64),
    /// Its flying height (distance from the centre): see route_at.
    pub cruise: f64,
    pub t0: f64,
    pub t1: f64,
    pub fill: f32,
    pub state: HaulerState,
    /// What its load is made of, and which hauler it is (for drawing).
    pub mix: Mix6,
    pub id: u32,
}

impl HaulerView {
    pub fn at(&self, t: f64) -> (f64, f64) {
        let k = if self.t1 > self.t0 { ((t - self.t0) / (self.t1 - self.t0)).clamp(0.0, 1.0) } else { 1.0 };
        let k = k * k * (3.0 - 2.0 * k);
        route_at(self.from, self.to, self.cruise, k)
    }
}

impl Sim {
    /// A working hauler at site h with the most room, if any.
    pub(super) fn hauler_at(&self, h: usize) -> Option<usize> {
        let mut best: Option<(usize, f64)> = None;
        let mut look = |i: usize, hl: &Hauler| {
            if hl.alive && hl.site == h && hl.state == HaulerState::Working && best.is_none_or(|(_, l)| hl.load < l) {
                best = Some((i, hl.load));
            }
        };
        if self.haulers_moved {
            for (i, hl) in self.haulers.iter().enumerate() {
                look(i, hl);
            }
        } else if let Some(v) = self.hauler_by_site.get(h) {
            for &i in v {
                look(i, &self.haulers[i]);
            }
        }
        best.map(|b| b.0)
    }

    /// Hand a drone's hold to hauler k.
    pub(super) fn unload_into(&mut self, id: u32, k: usize) {
        let d = &mut self.drones[id as usize];
        let (kg, cr) = (d.hold, d.hold_cr);
        d.hold = 0.0;
        d.hold_cr = 0.0;
        self.haulers[k].load += kg;
        self.haulers[k].credited += cr;
        self.window_delivered += kg;
        // Its cargo, made of what its flock has been cutting.
        let home = self.drones[id as usize].home;
        if let Some(fm) = self.flock_mix.get(home).copied() {
            let total: f32 = fm.iter().map(|e| e.1).sum();
            if total > 0.0 {
                for (m, w) in fm {
                    if w > 0.0 {
                        mix_add(&mut self.haulers[k].mix, m, kg as f32 * w / total);
                    }
                }
            }
        }
        let site = self.haulers[k].site;
        if let Some(f) = self.flock.get_mut(site) {
            f.filled += kg;
        }
    }

    /// Blow up a hauler that isn't needed any more.
    pub(super) fn retire_hauler(&mut self, k: usize) {
        let (top, tick) = (self.world.planet.max_radius, self.tick);
        let lost = std::mem::take(&mut self.haulers[k].load);
        self.transit.take(lost, None);
        self.mats_take(lost, false);
        let hl = &mut self.haulers[k];
        hl.alive = false;
        let at = if hl.state == HaulerState::Working {
            (self.hubs[hl.site].x, self.hubs[hl.site].y)
        } else {
            let t = if hl.t1 > hl.t0 { (tick.saturating_sub(hl.t0) as f64 / (hl.t1 - hl.t0) as f64).clamp(0.0, 1.0) } else { 1.0 };
            route_at(hl.from, hl.to, route_cruise(hl.from, hl.to, top), t * t * (3.0 - 2.0 * t))
        };
        self.haulers_retired += 1;
        let n = (self.eco.hauler.mass_cost / 100.0).round() as u32;
        self.explode(at, n.clamp(1, 40), 12.0);
    }

    /// Where a hauler from flock h unloads: the nearest point of the mother
    /// swarm (it takes loads anywhere along its length).
    pub fn rendezvous(&self, h: usize) -> (f64, f64) {
        let (x, y) = self.site_pos(h);
        // Straight up if the mother swarm is overhead, else its nearer end.
        let a = y.atan2(x);
        let m = self.mother;
        let a = if ang_dist(a, m.centre) <= m.half {
            a
        } else if ang_dist(a, m.centre + m.half) <= ang_dist(a, m.centre - m.half) {
            m.centre + m.half
        } else {
            m.centre - m.half
        };
        let r = self.mother_radius();
        (a.cos() * r, a.sin() * r)
    }

    /// Length (m) of a hauler's way from `a` to `b` (up, round, down).
    pub(super) fn hauler_way(&self, a: (f64, f64), b: (f64, f64)) -> f64 {
        route_len(a, b, route_cruise(a, b, self.world.planet.max_radius))
    }

    /// Pay for a hauler (economy on): its energy, and its parts from the
    /// parts stock, any it is short of made at once from the stockpile (or
    /// its mass, with no recipe). Returns whether it could.
    pub(super) fn pay_hauler(&mut self) -> bool {
        // With a factory, it is paid for when its bay starts on it: here,
        // only whether the queue has room.
        if self.factory() {
            let room = self.eco.production.hauler_orders_per_bay as usize * self.prod.bays_of(crate::production::Unit::Hauler);
            return self.prod.queued(crate::production::Unit::Hauler) < room;
        }
        let (e, m) = (self.eco.hauler.energy_cost, self.eco.hauler.mass_cost);
        if self.energy - e < 5.0 * self.upkeep() {
            return false;
        }
        if self.eco.recipe.hauler.parts.is_empty() {
            if self.mass < m {
                return false;
            }
            self.mass -= m;
            self.spent_mass += m;
        } else {
            match self.parts.build(&mut self.stock, &self.world.planet.mats.elements, &self.eco.recipe, &self.eco.recipe.hauler) {
                Some(kg) => self.stock_spent += kg,
                None => return false,
            }
        }
        self.energy -= e;
        true
    }

    /// A hauler for flock h: ordered from the factory (it comes when built),
    /// or straight away with no factory.
    pub(super) fn send_hauler(&mut self, h: usize) {
        if self.factory() {
            self.prod.orders.push_back(crate::production::Order::Hauler(h));
        } else {
            self.launch_hauler(h);
        }
    }

    /// A new hauler leaves the mother swarm for flock h.
    pub(super) fn launch_hauler(&mut self, h: usize) {
        let from = self.rendezvous(h);
        let to = self.site_pos(h);
        let t1 = self.tick + self.ticks(self.hauler_way(from, to) / self.eco.hauler.speed);
        // The seed swarm sets down with its first hauler already in place.
        let state = if self.tick == 0 { HaulerState::Working } else { HaulerState::Returning };
        let t1 = if self.tick == 0 { 0 } else { t1 };
        self.haulers_moved = true;
        self.haulers.push(Hauler { site: h, state, load: 0.0, credited: 0.0, from, to, t0: self.tick, t1, life_left: self.eco.hauler.life_s, alive: true, lift_at: 0, mix: Default::default() });
    }

    /// One second of hauler work: lift off when full, unload, come back;
    /// wear out; and send more haulers where cutters are left waiting.
    pub(super) fn hauler_work(&mut self) {
        let dt = self.ticks(1.0) as f64 * self.eco.tick_s;
        // (What was brought lately fades: a few minutes' worth.)
        let fade = (-dt / 240.0).exp();
        for r in self.recent_mats.iter_mut() {
            *r *= fade;
        }
        let spec = self.eco.hauler.clone();
        let cap = spec.capacity_kg * self.up_cached_cargo;
        let mut arrived = Vec::new();
        for k in 0..self.haulers.len() {
            if !self.haulers[k].alive {
                continue;
            }
            let site = self.haulers[k].site;
            if self.flock.get(site).is_some_and(|f| f.dead) && matches!(self.haulers[k].state, HaulerState::Working | HaulerState::Returning) {
                // Its flock is gone: nothing left for it to do.
                self.retire_hauler(k);
                continue;
            }
            let here = self.site_pos(site);
            let hot_here = self.contact_k((here.0.floor() as i64, here.1.floor() as i64)) > self.eco.harvester.melt_k;
            let now = self.tick;
            let mother = self.rendezvous(site);
            let top = self.world.planet.max_radius;
            let hl = &mut self.haulers[k];
            hl.life_left -= dt;
            match hl.state {
                HaulerState::Working => {
                    hl.from = here;
                    hl.to = here;
                    // Lava (or anything hotter than it can stand) over its
                    // spot: it melts where it stands, cargo and all.
                    if hot_here {
                        hl.life_left = 0.0;
                    }
                    let stayed = (now - hl.t1) as f64 * self.eco.tick_s;
                    if hl.load >= cap || (hl.load > 0.0 && stayed >= spec.max_stay_s) {
                        if self.happenings.len() < 400 {
                            self.happenings.push((Happening::HaulerLift, here.0, here.1));
                        }
                        hl.state = HaulerState::Lifting;
                        hl.lift_at = now;
                        hl.from = here;
                        hl.to = mother;
                        hl.t0 = now;
                        hl.t1 = now + ((route_len(here, mother, route_cruise(here, mother, top)) / spec.speed) / self.eco.tick_s).ceil() as u64;
                    }
                }
                HaulerState::Lifting if now >= hl.t1 => {
                    let (kg, cr) = (hl.load, hl.credited);
                    hl.mix = Default::default();
                    self.transit.take(kg, Some(&mut self.stock));
                    self.mats_take(kg, true);
                    let hl = &mut self.haulers[k];
                    hl.load = 0.0;
                    hl.credited = 0.0;
                    hl.state = HaulerState::Unloading;
                    hl.from = hl.to;
                    hl.t0 = now;
                    hl.t1 = now + (spec.unload_s / self.eco.tick_s).ceil() as u64;
                    // Delivered to the mother swarm.
                    self.energy += kg / 1000.0 * self.eco.seed_swarm.processing_energy_per_tonne * self.up("processing yield");
                    self.mass += cr;
                    self.delivered_credited += cr;
                    // A share goes into the mother swarm itself, so it spreads.
                    if self.mother.half < std::f64::consts::PI {
                        let g = cr * self.eco.seed_swarm.mother_growth_share;
                        self.mass -= g;
                        self.spent_mass += g;
                        self.mother.grown_kg += g;
                    }
                }
                HaulerState::Unloading if now >= hl.t1 => {
                    hl.state = HaulerState::Returning;
                    hl.to = here;
                    hl.t0 = now;
                    hl.t1 = now + ((route_len(hl.from, here, route_cruise(hl.from, here, top)) / spec.speed) / self.eco.tick_s).ceil() as u64;
                }
                HaulerState::Returning if now >= hl.t1 => {
                    if hl.lift_at > 0 {
                        let trip = (now - hl.lift_at) as f64 * self.eco.tick_s;
                        if let Some(f) = self.flock.get_mut(site) {
                            f.trip_s = if f.trip_s <= 0.0 { trip } else { f.trip_s * 0.7 + trip * 0.3 };
                        }
                        hl.lift_at = 0;
                    }
                    hl.state = HaulerState::Working;
                    hl.from = here;
                    hl.to = here;
                    hl.t1 = now;
                    arrived.push((k, site));
                }
                _ => {}
            }
            if self.haulers[k].life_left <= 0.0 {
                // Broken down: it falls where it is, with whatever it carried.
                let hl = &mut self.haulers[k];
                hl.alive = false;
                let lost = std::mem::take(&mut hl.load);
                self.transit.take(lost, None);
                self.mats_take(lost, false);
                let hl = &mut self.haulers[k];
                let t = if hl.t1 > hl.t0 { ((now - hl.t0) as f64 / (hl.t1 - hl.t0) as f64).clamp(0.0, 1.0) } else { 1.0 };
                let at = route_at(hl.from, hl.to, route_cruise(hl.from, hl.to, top), t * t * (3.0 - 2.0 * t));
                self.haulers_lost += 1;
                // Worn out: it self-destructs where it is, cargo and all.
                let n = (self.eco.hauler.mass_cost / 100.0).round() as u32;
                self.explode(at, n.clamp(1, 40), 12.0);
            }
        }
        // Cutters waiting where a hauler just arrived unload and go back to work.
        for (k, site) in arrived {
            if !self.haulers[k].alive {
                continue;
            }
            for (i, id) in std::mem::take(&mut self.site_wait[site]).into_iter().enumerate() {
                if self.drones[id as usize].alive && self.drones[id as usize].home == site && self.drones[id as usize].phase == Phase::Waiting {
                    self.unload_into(id, k);
                    self.drones[id as usize].phase = Phase::Idle;
                    self.leg_changed(id);
                    // (Back to work over a few seconds, not all in one step.)
                    self.events.push(Reverse((self.tick + (i as u64 % 30), id)));
                }
            }
        }
        self.haulers.retain(|h| h.alive);
        self.haulers_moved = true;
        self.direct();
    }

    /// Every hauler, for drawing.
    pub fn hauler_views(&self) -> Vec<HaulerView> {
        let cap = self.eco.hauler.capacity_kg * self.up_cached_cargo;
        self.haulers
            .iter()
            .enumerate()
            .filter(|(_, h)| h.alive)
            .map(|(k, h)| {
                let (from, to) = if h.state == HaulerState::Working { let p = self.site_pos(h.site); (p, p) } else { (h.from, h.to) };
                HaulerView { from, to, cruise: route_cruise(from, to, self.world.planet.max_radius), t0: h.t0 as f64 * self.eco.tick_s, t1: h.t1 as f64 * self.eco.tick_s, fill: (h.load / cap).min(1.0) as f32, state: h.state, mix: h.mix, id: k as u32 }
            })
            .collect()
    }
}
