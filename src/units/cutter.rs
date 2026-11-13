//! Cutters: the swarm's workhorse drones. Each lives on events (see
//! `on_event`): it picks a cell near its flock (`pick_target`), flies out
//! and cuts it, chains to the next while its hold has room, flies home and
//! hands the load to its flock's hauler. Heat, wear and energy are paid as
//! it goes. Also here: building new cutters and losing them.
//!
//! Unit file: the cutter's own behaviour. The platform (swarm.rs: the tick,
//! flocks, the director, production; units/platform.rs: shared helpers)
//! calls in here.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Phase {
    Idle,
    /// Flying out and cutting; on the event, the cell is taken. `from` is
    /// where it set off; `lava` if it chose to swallow lava.
    Outbound { cell: (i64, i64), from: (f64, f64), lava: bool },
    /// Flying from `from` to its flock's hauler with a full hold.
    Inbound { hub: usize, from: (f64, f64) },
    /// At its flock with a full hold and no hauler there to take it.
    Waiting,
}

#[derive(Debug, Clone)]
pub(super) struct Drone {
    pub(super) alive: bool,
    pub(super) home: usize,
    pub(super) phase: Phase,
    pub(super) life_left: f64,
    pub(super) trip_start: u64,
    /// Where it last dug: it looks for its next cell near there.
    pub(super) last: Option<(i64, i64)>,
    /// Searches in a row that found nothing: it waits longer after each.
    pub(super) fails: u8,
    /// When the current flight leg began (also what the leg cost in energy).
    pub(super) leg_start: u64,
    /// kg in the hold, and what it is worth credited.
    pub(super) hold: f64,
    pub(super) hold_cr: f64,
    /// Energy carried (E).
    pub(super) tank: f64,
    /// Its temperature inside (kelvin).
    pub(super) heat_k: f64,
}

impl Sim {
    pub(super) fn spawn_harvester(&mut self) {
        if self.free_drones.is_none() && self.individuals() >= self.eco.individual_cap {
            self.packet += 1;
            self.harvesters_built += 1;
            return;
        }
        let id = self.drones.len() as u32;
        let home = self.orbit_hub;
        self.hub_load[home] += 1;
        // Made in the mother swarm, it drops to its flock.
        let from = self.rendezvous(home);
        let to = self.site_pos(home);
        let fall = self.drop_s(dist2(from, to).sqrt());
        self.on_trip_count += 1;
        self.drones.push(Drone {
            alive: true,
            home,
            phase: Phase::Inbound { hub: home, from },
            life_left: self.eco.harvester.life_s * self.up("drone lifespan"),
            trip_start: self.tick,
            leg_start: self.tick,
            last: None,
            fails: 0,
            hold: 0.0,
            hold_cr: 0.0,
            tank: self.eco.harvester.tank_e,
            heat_k: 290.0,
        });
        self.harvesters_built += 1;
        self.alive_count += 1;
        self.counts[4] += 1;
        self.leg_changed(id);
        self.events.push(Reverse((self.tick + self.ticks(fall) + (id as u64 % 10), id)));
    }

    /// Seconds for a new cutter to drop `dist` metres from the mother swarm:
    /// at flight speed, but never longer than DROP_S (it falls, from high up).
    pub(super) fn drop_s(&self, dist: f64) -> f64 {
        (dist / self.cutter_speed()).min(DROP_S)
    }

    pub(super) fn individuals(&self) -> u32 {
        self.alive_count
    }

    pub fn harvesters_alive(&self) -> u64 {
        self.alive_count as u64 + self.packet
    }

    pub(super) fn cutter_speed(&self) -> f64 {
        self.eco.harvester.speed * self.up("flight speed") * self.perk_speed
    }

    pub(super) fn hold_cap(&self) -> f64 {
        self.eco.harvester.hold_kg * self.up_cached_cargo
    }

    /// Seconds to cut `kg` of material `m` at cell c.
    pub(super) fn cut_s(&self, c: (i64, i64), m: MatId, kg: f64) -> f64 {
        if self.is_loose(c, m) {
            // Loose debris: picked up, not cut.
            let d = self.world.planet.mats.get(m).density.max(1.0);
            return self.eco.harvester.loose_s * (kg / d).max(0.05);
        }
        let per_load = self.dig_s(m) * if self.fractured.contains(&c) { self.eco.tunneler.fracture_dig } else { 1.0 };
        per_load * (kg / self.eco.harvester.cargo_kg).max(0.05) / self.cold_factor(c, m)
    }

    /// Whether cell c (holding m) is loose debris.
    pub(super) fn is_loose(&self, c: (i64, i64), m: MatId) -> bool {
        self.loose_mat.get(m as usize).copied().unwrap_or(false) || (!self.loose.is_empty() && self.loose.contains(&c))
    }

    /// Cold ground: how much faster a cutter works it (and the same, how
    /// much quicker it wears), 1 at cold_ref_k and above.
    pub(super) fn cold_factor(&self, c: (i64, i64), m: MatId) -> f64 {
        if m == VACUUM {
            return 1.0;
        }
        let h = &self.eco.harvester;
        let t = self.sandbox.temp(&self.world, c, m);
        1.0 + h.cold_gain * ((h.cold_ref_k - t) / h.cold_ref_k.max(1.0)).clamp(0.0, 1.0)
    }

    /// The hottest ground a cutter at cell c touches (kelvin): the cell and
    /// its four neighbours.
    pub(super) fn contact_k(&self, c: (i64, i64)) -> f64 {
        let mut t = f64::NEG_INFINITY;
        for d in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (c.0 + d.0, c.1 + d.1);
            let m = self.world.material(n.0, n.1);
            if m != VACUUM {
                t = t.max(self.sandbox.temp(&self.world, n, m));
            }
        }
        if t.is_finite() { t } else { self.world.planet.space_k }
    }

    /// A drone spent `secs` at temperature `env` (in contact with it, or in
    /// open space): its heat moves toward env through its heat sink, and
    /// while hot it wears. True if it melted.
    pub(super) fn heat_drone(&mut self, id: u32, secs: f64, env: f64, contact: bool) -> bool {
        let h = self.eco.harvester.clone();
        let d = &mut self.drones[id as usize];
        let t0 = d.heat_k;
        let g = if contact { h.contact_sink } else { h.heat_sink };
        let k = (-secs.max(0.0) * g / h.heat_capacity_kg.max(1.0)).exp();
        let t1 = env + (t0 - env) * k;
        d.heat_k = t1;
        // Wear for the time spent hot (the average of start and end).
        let over = (0.5 * (t0 + t1) - h.damage_k).max(0.0);
        d.life_left -= h.heat_wear * over / 100.0 * secs;
        t0.max(t1) > h.melt_k
    }

    /// One second for the cutters waiting at their flocks: each sits
    /// somewhere in the loose cloud about its hauler (3 to 12 m out, as it is
    /// drawn), and whatever is there heats or cools it. Lava pouring over the
    /// cloud melts the drones caught in it, reckless as ever.
    pub(super) fn waiting_heat(&mut self) {
        let space_k = self.world.planet.space_k;
        let sec = self.tick / self.ticks(1.0).max(1);
        for site in 0..self.site_wait.len() {
            if self.site_wait[site].is_empty() {
                continue;
            }
            let (hx, hy) = self.site_pos(site);
            let ids = self.site_wait[site].clone();
            let mut lost = false;
            for id in ids {
                let d = &self.drones[id as usize];
                if !d.alive || d.home != site || d.phase != Phase::Waiting {
                    continue;
                }
                // Where in the cloud this second (a fresh spot each second,
                // as the cloud turns).
                let h = hash(id as u64, sec as i64, site as i64, 0x7A17);
                let u = crate::noise::unit(h);
                let a = crate::noise::unit(h ^ 0x9E37) * std::f64::consts::TAU;
                let r = 3.0 + 9.0 * u * u;
                let c = ((hx + a.cos() * r).floor() as i64, (hy + a.sin() * r * 0.8).floor() as i64);
                let m = self.world.material(c.0, c.1);
                let melted = if m == VACUUM {
                    self.heat_drone(id, 1.0, space_k, false)
                } else {
                    let t = self.contact_k(c);
                    self.heat_drone(id, 1.0, t, true)
                };
                if melted {
                    self.burned += 1;
                    self.happen(Happening::Losses, (c.0 as f64 + 0.5, c.1 as f64 + 0.5));
                    self.wreck_drone(id, (c.0 as f64 + 0.5, c.1 as f64 + 0.5), (0.0, 0.0));
                    lost = true;
                }
            }
            if lost {
                let drones = &self.drones;
                self.site_wait[site].retain(|&id| drones[id as usize].alive);
            }
        }
    }

    /// A sip of molten (or any) material `m` at `t_cell`, `kg` of it, freezes
    /// inside the drone: its heat (and the heat of freezing) goes into the
    /// drone. True if that melts it.
    pub(super) fn swallow_heat(&mut self, id: u32, m: MatId, t_cell: f64, kg: f64) -> bool {
        let latent = if self.world.planet.mats.get(m).state == State::Liquid { self.world.planet.mats.phases[m as usize].down.iter().flatten().next().map_or(0.0, |c| c.latent_k) } else { 0.0 };
        let h = &self.eco.harvester;
        let d = &mut self.drones[id as usize];
        let hot = t_cell + latent;
        if hot > d.heat_k {
            d.heat_k += kg * (hot - d.heat_k) / (h.heat_capacity_kg + kg);
        }
        d.heat_k > h.melt_k
    }

    /// kg the next cut at cell c would take, for a drone with `space` left.
    pub(super) fn cut_kg(&self, c: (i64, i64), space: f64) -> f64 {
        let m = self.world.material(c.0, c.1);
        if m == VACUUM {
            return 0.0;
        }
        let mat = self.world.planet.mats.get(m);
        let left = mat.density - self.progress.get(&c).copied().unwrap_or(0.0);
        // Molten stuff is drunk in sips.
        let space = if mat.state == State::Liquid && self.sandbox.temp(&self.world, c, m) > self.eco.harvester.damage_k { space.min(self.eco.harvester.sip_kg) } else { space };
        left.min(space).max(0.0)
    }

    /// A drone spent `secs` working: pay for it from its tank, burning cargo
    /// to top the tank back up. False if it ran dry.
    pub(super) fn spend(&mut self, id: u32, secs: f64) -> bool {
        let per_kg = self.eco.seed_swarm.processing_energy_per_tonne / 1000.0;
        let (upkeep, tank_max) = (self.eco.harvester.upkeep, self.eco.harvester.tank_e);
        let d = &mut self.drones[id as usize];
        d.tank -= upkeep * secs;
        if d.tank < tank_max && d.hold > 0.0 && per_kg > 0.0 {
            let kg = ((tank_max - d.tank) / per_kg).min(d.hold);
            let worth = if d.hold > 0.0 { d.hold_cr / d.hold } else { 1.0 };
            d.hold -= kg;
            d.hold_cr = (d.hold_cr - kg * worth).max(0.0);
            d.tank += kg * per_kg;
            self.fuel_kg += kg;
        }
        d.tank > -1e-9
    }

    /// A drone out working is lost where it is.
    pub(super) fn lose_on_trip(&mut self, id: u32, at: (f64, f64)) {
        self.happen(Happening::Losses, at);
        self.on_trip_count -= 1;
        self.wreck_drone(id, at, (0.0, 0.0));
    }

    /// Fly a drone with a full hold from `from` to its flock's hauler.
    pub(super) fn head_home(&mut self, id: u32, from: (f64, f64)) {
        let home = self.drones[id as usize].home;
        let to = self.site_pos(home);
        let straight = ((to.0 - from.0).powi(2) + (to.1 - from.1).powi(2)).sqrt();
        // The way back round through open space, if the field knows it.
        let dist = self.field_for(home, to).and_then(|f| f.dist_m(from.0, from.1).or_else(|| f.reach_m((from.0.floor() as i64, from.1.floor() as i64)))).map_or(straight, |d| d.max(straight));
        self.avg_haul = self.avg_haul * 0.999 + dist * 0.001;
        let due = self.tick + self.ticks(dist / self.cutter_speed());
        let dr = &mut self.drones[id as usize];
        dr.leg_start = self.tick;
        dr.phase = Phase::Inbound { hub: home, from };
        self.events.push(Reverse((due, id)));
    }

    /// Send a drone from `from` to cut cell c (already reserved).
    pub(super) fn go_cut(&mut self, id: u32, c: (i64, i64), from: (f64, f64), home: Option<usize>) {
        let m = self.world.material(c.0, c.1);
        let space = self.hold_cap() - self.drones[id as usize].hold;
        let kg = self.cut_kg(c, space);
        let (dx, dy) = (c.0 as f64 + 0.5 - from.0, c.1 as f64 + 0.5 - from.1);
        let straight = (dx * dx + dy * dy).sqrt();
        // From the hauler, the way round through open space.
        let way = home.and_then(|h| self.field_for(h, from)).and_then(|f| f.reach_m(c)).map_or(straight, |d| d.max(straight));
        let fly = way / self.cutter_speed();
        let dig = self.cut_s(c, m, kg);
        let lava = self.world.planet.mats.get(m).state == State::Liquid && self.sandbox.temp(&self.world, c, m) > self.eco.harvester.damage_k;
        let dr = &mut self.drones[id as usize];
        dr.phase = Phase::Outbound { cell: c, from, lava };
        dr.leg_start = self.tick;
        self.working.entry(c).or_default().push(id);
        let due = self.tick + self.ticks(fly + dig);
        self.events.push(Reverse((due, id)));
    }

    pub(super) fn on_event(&mut self, id: u32) {
        let d = self.drones[id as usize].clone();
        if !d.alive {
            return;
        }
        let secs = (self.tick - d.leg_start.min(self.tick)) as f64 * self.eco.tick_s;
        match d.phase {
            Phase::Idle | Phase::Waiting => {
                let (hx, hy) = self.site_pos(d.home);
                // Too hot to go out again: it rests by its hauler until its
                // heat sink has brought it down to where heat stops wearing it.
                // (On a world whose own air is hotter than that, as cool as
                // the air will let it get.)
                let h = &self.eco.harvester;
                let env = self.world.planet.space_k;
                let cool_to = h.damage_k.max(env + 50.0);
                if d.heat_k > cool_to + 1.0 && d.phase == Phase::Idle {
                    let tau = h.heat_capacity_kg / h.heat_sink.max(1e-9);
                    let rest = (tau * ((d.heat_k - env) / (cool_to - env).max(1.0)).ln()).clamp(1.0, 600.0);
                    self.heat_drone(id, rest, env, false);
                    self.drones[id as usize].leg_start = self.tick;
                    self.events.push(Reverse((self.tick + self.ticks(rest), id)));
                    return;
                }
                // Back from a load (or loaded with nowhere to put it).
                if d.hold >= 0.9 * self.hold_cap() {
                    if let Some(k) = self.hauler_at(d.home) {
                        self.unload_into(id, k);
                    } else {
                        self.drones[id as usize].phase = Phase::Waiting;
                        self.site_wait[d.home].push(id);
                        return;
                    }
                }
                if self.brownout() {
                    self.events.push(Reverse((self.tick + self.ticks(1.0), id)));
                    return;
                }
                // Landing points sink with the ground, so searching from home
                // stays cheap and keeps each drone working its own patch.
                let from_shaft = self.hubs[d.home].kind == HubKind::Shaft;
                // Picky first: leave rock that pays badly while better is near.
                // If nothing better is near, take what there is rather than idle.
                // (Not at a hub already known to have nothing free.)
                let found = if self.hub_dry[d.home] {
                    None
                } else {
                    let ts = clock();
                    let reach = self.eco.harvester.search_m;
                    let found = match self.pick_target(hx, hy, reach, false, !from_shaft, true, Some(d.home)) {
                        Some(c) => Some(c),
                        None if self.sense_effort > 0.0 => self.pick_target(hx, hy, reach, false, !from_shaft, false, Some(d.home)),
                        None => None,
                    };
                    self.prof[4] += clock() - ts;
                    self.counts[1] += 1;
                    if found.is_none() {
                        self.counts[2] += 1;
                        self.hub_dry[d.home] = true;
                        self.hub_gen += 1;
                        self.flock[d.home].dry += 1.0;
                    }
                    found
                };
                let Some(cell) = found else {
                    // Nothing free for it here: it joins the pool resting in
                    // the mother swarm, and the director sends it wherever
                    // there is work (see `allocate`).
                    self.pool_drone(id);
                    return;
                };
                self.drones[id as usize].fails = 0;
                self.drones[id as usize].trip_start = self.tick;
                self.on_trip_count += 1;
                self.go_cut(id, cell, (hx, hy), Some(d.home));
            }
            Phase::Outbound { cell, from, lava } => {
                self.unwork(cell, id);
                if let Some(n) = self.reserved.get_mut(&cell) {
                    *n -= 1;
                    if *n == 0 {
                        self.reserved.remove(&cell);
                    }
                }
                self.work_near(cell);
                let (cx, cy) = (cell.0 as f64 + 0.5, cell.1 as f64 + 0.5);
                let (ux, uy) = (from.0 - cx, from.1 - cy);
                let l = (ux * ux + uy * uy).sqrt().max(1e-9);
                let beside = (cx + ux / l * 1.5, cy + uy / l * 1.5);
                let _ = lava;
                // Cut what fits in the hold. A cell may take several cuts; it
                // is booked as mined when the last is taken.
                let m = self.world.material(cell.0, cell.1);
                let space = self.hold_cap() - d.hold;
                let kg = self.cut_kg(cell, space);
                // How hot it is here (before the cell is taken), and how cold.
                let space_k = self.world.planet.space_k;
                let t_cell = if m != VACUUM { self.sandbox.temp(&self.world, cell, m) } else { space_k };
                let t_touch = self.contact_k(cell);
                let molten = m != VACUUM && self.world.planet.mats.get(m).state == State::Liquid;
                let cold = self.cold_factor(cell, m);
                if kg > 0.0 {
                    let mult = self.world.planet.mats.get(m).multiplier;
                    let done = self.progress.get(&cell).copied().unwrap_or(0.0);
                    let density = self.world.planet.mats.get(m).density;
                    if done + kg >= density - 1e-9 {
                        self.progress.remove(&cell);
                        self.world.mine_cell(cell.0, cell.1);
                        self.undope(cell, true);
                        self.open_cell(cell);
                    } else {
                        self.progress.insert(cell, done + kg);
                    }
                    if mult >= 2.0 && self.happenings.len() < 200 {
                        self.happenings.push((Happening::RichOre, cx, cy));
                    }
                    self.transit.add_composition(&self.world.planet.mats.elements, &self.world.planet.mats.get(m).composition, kg);
                    self.mats_add(m, kg);
                    {
                        let home = d.home;
                        if self.flock_mix.len() <= home {
                            self.flock_mix.resize(home + 1, Default::default());
                        }
                        // (Remembering the last couple of hundred tonnes.)
                        let fm = &mut self.flock_mix[home];
                        let keep = (1.0 - kg as f32 / 200_000.0).max(0.0);
                        for e in fm.iter_mut() {
                            e.1 *= keep;
                        }
                        mix_add(fm, m, kg as f32);
                    }
                    let dr = &mut self.drones[id as usize];
                    dr.hold += kg;
                    dr.hold_cr += kg * mult;
                    self.window_mined += kg;
                    let dig = self.cut_s(cell, m, kg);
                    let f = &mut self.flock[d.home];
                    f.sx += cx * kg;
                    f.sy += cy * kg;
                    f.w += kg;
                    f.kg += kg;
                    f.dig_s += dig;
                } else if m == VACUUM {
                    self.progress.remove(&cell);
                }
                // Pay for the flight and the cutting it actually did (a cell
                // emptied by another drone meanwhile costs only the flight);
                // out of energy, it's lost.
                let _ = secs;
                let fly = dist2(from, (cx, cy)).sqrt() / self.cutter_speed();
                let paid = fly + if kg > 0.0 { self.cut_s(cell, m, kg) } else { 0.0 };
                if !self.spend(id, paid) {
                    self.starved += 1;
                    self.lose_on_trip(id, beside);
                    return;
                }
                // Heat: cooling in flight, then pressed against the ground it
                // cuts (lava or glowing rock beside it heats it fast), then
                // whatever molten it drank freezing inside it. Too hot: it
                // melts where it works. Cold ground wore it quicker.
                let dig_s = paid - fly;
                let melted = self.heat_drone(id, fly, space_k, false) | self.heat_drone(id, dig_s, t_touch, true) | (kg > 0.0 && molten && self.swallow_heat(id, m, t_cell, kg));
                self.drones[id as usize].life_left -= dig_s * (cold - 1.0);
                if molten && kg > 0.0 && t_cell > self.eco.harvester.damage_k {
                    self.lava_swallowed += 1;
                }
                if melted {
                    self.burned += 1;
                    self.lose_on_trip(id, beside);
                    return;
                }
                self.drones[id as usize].last = Some(cell);
                // Room left: the next cell close by, if there is one (unless
                // it's close to melting: then it heads home, cooling on the
                // way; short of that, reckless, it carries on hot).
                let hot = self.drones[id as usize].heat_k > self.eco.harvester.melt_k - 200.0;
                if self.drones[id as usize].hold < 0.9 * self.hold_cap() && !self.hub_dry[d.home] && !hot {
                    let reach = self.eco.harvester.chain_m;
                    let next = self.pick_target(cx, cy, reach, false, true, true, None).or_else(|| if self.sense_effort > 0.0 { self.pick_target(cx, cy, reach, false, true, false, None) } else { None });
                    if let Some(c) = next {
                        self.go_cut(id, c, (cx, cy), None);
                        return;
                    }
                }
                let at = (cx, cy);
                if dist2(at, self.site_pos(d.home)) > self.eco.refiner.spacing_m * self.eco.refiner.spacing_m {
                    self.last_far_cell = Some(cell);
                }
                self.head_home(id, at);
            }
            Phase::Inbound { hub, from } => {
                let _ = from;
                // A new cutter dropping from the mother swarm comes down on
                // the mother's energy, not its own (it set off when its trip
                // began; any other flight home sets off later).
                let dropped = d.trip_start == d.leg_start && d.hold <= 0.0;
                // (Flying home, its heat sink cools it toward space.)
                if !dropped {
                    let space_k = self.world.planet.space_k;
                    self.heat_drone(id, secs, space_k, false);
                }
                if !dropped && !self.spend(id, secs) {
                    self.starved += 1;
                    let at = self.site_pos(hub);
                    self.lose_on_trip(id, at);
                    return;
                }
                self.on_trip_count -= 1;
                let worked = (self.tick - d.trip_start) as f64 * self.eco.tick_s;
                let dr = &mut self.drones[id as usize];
                dr.life_left -= worked;
                dr.phase = Phase::Idle;
                if self.drones[id as usize].life_left <= 0.0 {
                    // Worn out at the hauler: its load goes in first, and then
                    // the drone itself, as scrap for the mother swarm to make
                    // new drones from (rather than a heap growing under the
                    // hauler, hundreds a minute with a big swarm).
                    let recycled = if let Some(k) = self.hauler_at(hub) {
                        self.unload_into(id, k);
                        let kg = self.eco.harvester.mass_cost;
                        if let Ok(scrap) = self.world.planet.mats.id("drone-scrap") {
                            self.transit.add_composition(&self.world.planet.mats.elements, &self.world.planet.mats.get(scrap).composition, kg);
                            self.mats_add(scrap, kg);
                        }
                        self.haulers[k].load += kg;
                        self.haulers[k].credited += kg;
                        self.recycled += 1;
                        true
                    } else {
                        false
                    };
                    let dr = &mut self.drones[id as usize];
                    dr.alive = false;
                    self.leg_changed(id);
                    self.hub_load[d.home] -= 1;
                    self.alive_count -= 1;
                    self.deaths += 1;
                    // Worn out: it shuts down and falls where it is, as scrap
                    // (no explosion: there are far too many of these).
                    if !recycled {
                        let at = self.site_pos(hub);
                        if let Some(c) = self.bodies.wreck(&mut self.world, at.0, at.1) {
                            self.scrap_landed(c);
                        }
                    }
                    return;
                }
                if self.drones[id as usize].hold <= 0.0 {
                    // Nothing to unload (just dropped from the mother swarm).
                } else if let Some(k) = self.hauler_at(hub) {
                    self.unload_into(id, k);
                } else if let Some(other) = self.nearest_working_site(hub) {
                    // No hauler here: take it to the nearest flock that has one.
                    self.hub_load[d.home] -= 1;
                    self.hub_load[other] += 1;
                    self.drones[id as usize].home = other;
                    self.on_trip_count += 1;
                    let here = self.site_pos(hub);
                    self.head_home(id, here);
                    return;
                } else {
                    self.drones[id as usize].phase = Phase::Waiting;
                    self.site_wait[hub].push(id);
                    return;
                }
                // Share the work: a drone from a crowded landing point moves to
                // the least busy one.
                let old_home = self.drones[id as usize].home;
                let n = self.hub_load.len() as f64;
                let mean = self.alive_count as f64 / n;
                if self.hub_load[old_home] as f64 > 1.25 * mean {
                    let mut best = old_home;
                    for i in 0..self.hub_load.len() {
                        if self.hub_load[i] < self.hub_load[best] && !self.flock[i].dead && !self.wet_hubs.contains(&i) && self.hubs[i].kind == HubKind::Seed {
                            best = i;
                        }
                    }
                    self.hub_load[old_home] -= 1;
                    self.hub_load[best] += 1;
                    self.drones[id as usize].home = best;
                }
                self.events.push(Reverse((self.tick, id)));
            }
        }
    }

    /// A drone is wrecked: it leaves the swarm and falls as a body of scrap.
    pub(super) fn wreck_drone(&mut self, id: u32, at: (f64, f64), vel: (f64, f64)) {
        let dr = &mut self.drones[id as usize];
        if !dr.alive {
            return;
        }
        dr.alive = false;
        let home = dr.home;
        let lost = std::mem::take(&mut dr.hold);
        self.transit.take(lost, None);
        self.mats_take(lost, false);
        self.leg_changed(id);
        self.hub_load[home] -= 1;
        self.alive_count -= 1;
        self.deaths += 1;
        let _ = vel;
        if let Some(c) = self.bodies.wreck(&mut self.world, at.0, at.1) {
            self.scrap_landed(c);
        }
    }

    /// Scrap came to rest at cell c: the sandbox may move it on, and the
    /// cutters can eat it again.
    pub(super) fn scrap_landed(&mut self, c: (i64, i64)) {
        self.sandbox.wake_around_filled(&self.world, c);
        self.apply_sandbox(vec![Change { cell: c, from: VACUUM, to: self.world.material(c.0, c.1) }]);
        if self.world.material(c.0, c.1) != VACUUM {
            self.add_frontier(c);
        }
    }

    /// Pick the best cell to dig near (hx, hy), and reserve it. Searches outward
    /// ring by ring; once something is found it looks a little further (further
    /// with Scouts), weighing about CANDIDATES cells within a round reach.
    /// `packet`: choosing for a packet, which cuts whole 64 m blocks and so stays
    /// out of Tunneler shafts (a block cut there would leave a square cavern).
    /// `peel`: charge for depth so the swarm eats the body from the outside in.
    /// Drones working from a shaft don't: they hollow out round chambers.
    /// `picky`: turn down cells that pay badly (if the swarm can judge effort).
    /// The search behind `pick_target`, changing nothing: fills `cands` with
    /// the cells weighed (cell, score, distance) and returns the round reach.
    pub(super) fn weigh(&self, hx: f64, hy: f64, reach_m: f64, packet: bool, peel: bool, picky: bool, field: Option<&FlowField>, cands: &mut Vec<((i64, i64), f64, f64)>) -> f64 {
        let (bx, by) = ((hx as i64).div_euclid(BUCKET), (hy as i64).div_euclid(BUCKET));
        let scouting = self.scouts > 0;
        // Look no further than reach_m from where the search starts.
        let max_ring = ((reach_m / BUCKET as f64) as i64 + 2).min(((2.0 * self.world.planet.max_radius) / BUCKET as f64) as i64 + 2);
        let extra = if scouting { 1 + ((self.scouts as f64).sqrt() as i64 / 4).min(3) } else { 0 };
        // Every cell weighed: (cell, score, distance).
        cands.clear();
        let any_cracked = !self.fractured.is_empty();
        let mut found_at: Option<i64> = None;
        // The search runs ring by ring (square rings of buckets) and then
        // keeps only cells within a circle, so the swarm's reach is round.
        let mut last_ring: Option<i64> = None;
        let mut reach = f64::INFINITY;
        // What each trip is judged by: credited kg per second of the round trip,
        // as well as the swarm can tell. A blind swarm thinks every cell is
        // average, which comes down to taking the nearest.
        let speed = self.eco.harvester.speed * self.up("flight speed") * self.perk_speed;
        let dig_mult = self.up("dig speed") * self.perk_dig;
        let it = &self.eco.intelligence;
        let (se, sv) = (self.sense_effort, self.sense_value);
        let typ_dig = self.eco.harvester.dig_s_for(it.typical_hardness) / dig_mult;
        let patience = if picky { it.patience * se } else { 0.0 };
        // Scent of buried ore: Scouts smell further.
        let far_nose = if scouting { sv } else { 0.0 };
        let cargo = self.eco.harvester.cargo_kg * self.up_cached_cargo;
        let typ_kg = it.typical_load_kg.min(cargo);
        let hold = self.hold_cap();
        let search_m = reach_m;
        let mut weighed = 0usize;
        for ring in 0..max_ring {
            if last_ring.is_some_and(|l| ring > l) {
                break;
            }
            for dy in -ring..=ring {
                for dx in -ring..=ring {
                    if dx.abs() != ring && dy.abs() != ring {
                        continue;
                    }
                    let key = (bx + dx, by + dy);
                    let Some(list) = self.buckets.get(&key) else { continue };
                    let reserved = &self.reserved;
                    for &(c, f) in list.iter() {
                        // Cheapest test first: a round reach, not the square the
                        // ring search covers.
                        let (ddx, ddy) = (c.0 as f64 + 0.5 - hx, c.1 as f64 + 0.5 - hy);
                        let dist = (ddx * ddx + ddy * ddy).sqrt();
                        if dist > search_m {
                            self.passed[0].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            continue;
                        }

                        let mat = self.world.planet.mats.get(f.mat);
                        // (Liquids are drunk like anything else, seas and lava
                        // included: reckless drones don't steer clear of heat.)
                        // No more drones than the loads the cell has left (at most
                        // four, one per face).
                        let left = mat.density - self.progress.get(&c).copied().unwrap_or(0.0);
                        let loads = ((left / hold).ceil() as u8).clamp(1, 4);
                        if reserved.get(&c).copied().unwrap_or(0) >= loads {
                            self.passed[1].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            continue;
                        }
                        // With the flock's field: the flight is the way
                        // round through open space, and a cell it doesn't
                        // reach can't be flown to at all.
                        let fly = match field {
                            Some(f) => match f.reach_m(c) {
                                Some(d) => d.max(dist),
                                None => {
                                    self.passed[2].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    continue;
                                }
                            },
                            None => dist,
                        };
                        // Effective distance: flight plus a charge for depth, so
                        // the swarm peels the body from the outside in.
                        let (cx, cy) = (c.0 as f64 + 0.5, c.1 as f64 + 0.5);
                        // Rock cracked by a Tunneler: open access (no depth
                        // penalty) and quicker to cut.
                        let loose = self.is_loose(c, f.mat);
                        let cracked = loose || (any_cracked && self.fractured.contains(&c));
                        if cracked && packet {
                            continue;
                        }
                        let depth = if cracked || !peel { 0.0 } else { (self.world.planet.radius - (cx * cx + cy * cy).sqrt()).max(0.0) };
                        let eff = fly + self.eco.seed_swarm.peel * depth;
                        let true_dig = if loose { self.eco.harvester.loose_s } else { self.eco.harvester.dig_s_for(mat.hardness) / dig_mult * if cracked { self.eco.tunneler.fracture_dig } else { 1.0 } };
                        let est_dig = se * true_dig + (1.0 - se) * typ_dig;
                        let est_kg = se * left.min(hold) + (1.0 - se) * typ_kg;
                        // Worth: the cell's own multiplier, plus the smell of richer
                        // rock behind it, which draws tunnels toward buried pockets.
                        let est_mult = sv * mat.multiplier + (1.0 - sv) + it.scent_weight * (sv * f.near as f64 + far_nose * f.far as f64);
                        let travel = 2.0 * eff / speed;
                        let score = est_kg * est_mult / (travel + est_dig);
                        // A drone that can judge effort won't take a cell that pays
                        // much worse than ordinary rock would (hard ore it can't
                        // cut well yet stays standing as terrain).
                        if score < patience * typ_kg / (travel + typ_dig) {
                            self.passed[3].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            continue;
                        }
                        cands.push((c, score, dist));
                        self.passed[4].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        weighed += 1;
                    }
                }
            }
            if !cands.is_empty() && found_at.is_none() {
                found_at = Some(ring);
            }
            // Enough weighed, or a little further looked once something was
            // found (further with Scouts): stop. Keep only cells inside the
            // circle these rings fully cover, so the reach is round.
            if weighed >= CANDIDATES || found_at.is_some_and(|f| ring >= f + extra) {
                last_ring = Some(ring);
                reach = ((ring - 1).max(0) * BUCKET) as f64;
            }
        }
        reach
    }

    /// The best of the cells weighed: within the round reach if any is, else
    /// the best of all.
    pub(super) fn choose(cands: &[((i64, i64), f64, f64)], reach: f64) -> Option<(i64, i64)> {
        let pick = |within: bool| {
            let mut best: Option<((i64, i64), f64)> = None;
            for &(c, score, dist) in cands {
                if within && dist > reach || score == f64::NEG_INFINITY {
                    continue;
                }
                if best.map_or(true, |(bc, bs)| score > bs || (score == bs && c < bc)) {
                    best = Some((c, score));
                }
            }
            best
        };
        pick(true).or_else(|| pick(false)).map(|b| b.0)
    }

    /// Whether a drone at `from` can fly straight to cell c: no more than
    /// LOS_SOLID metres of solid ground in the way (it slips round a bump or
    /// a ridge; liquids it swims through), not a wall of rock.
    pub(super) fn clear_line(&self, from: (f64, f64), c: (i64, i64)) -> bool {
        let to = (c.0 as f64 + 0.5, c.1 as f64 + 0.5);
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1.5 {
            return true;
        }
        let steps = ((len - 1.2) / 0.7).ceil().max(1.0) as usize;
        let mut rd = self.world.reader();
        let mats = &self.world.planet.mats;
        let mut last = (i64::MIN, i64::MIN);
        let mut solid = 0;
        for i in 0..steps {
            let t = i as f64 * 0.7 / len;
            let p = ((from.0 + dx * t).floor() as i64, (from.1 + dy * t).floor() as i64);
            if p == last || p == c {
                continue;
            }
            last = p;
            let m = rd.get(p.0, p.1);
            if m != VACUUM && matches!(mats.get(m).state, State::Solid | State::Powder) {
                solid += 1;
                if solid > LOS_SOLID {
                    return false;
                }
            }
        }
        true
    }

    /// The best cell to cut from (hx, hy), reserved. `home`: the flock it
    /// searches from its hauler for (its field gives the real flights);
    /// None for a short hop from a cell just cut, or the packet.
    pub(super) fn pick_target(&mut self, hx: f64, hy: f64, reach_m: f64, packet: bool, peel: bool, picky: bool, home: Option<usize>) -> Option<(i64, i64)> {
        let mut cands = std::mem::take(&mut self.cands);
        let field = home.and_then(|h| self.field_for(h, (hx, hy)));
        let reach = self.weigh(hx, hy, reach_m, packet, peel, picky, field.as_deref(), &mut cands);
        let mut best = Self::choose(&cands, reach);
        // Without a field (a short hop from the cell just cut), drones fly
        // straight: only to a cell with little rock in the way. (Not for the
        // packet, which isn't flown cell by cell, nor from a shaft, whose
        // drones work the rock a Tunneler has cracked all round it.)
        if !packet && peel && field.is_none() {
            let mut tries = 0;
            while let Some(c) = best {
                if self.clear_line((hx, hy), c) {
                    break;
                }
                tries += 1;
                cands.retain(|x| x.0 != c);
                best = if tries < LOS_TRIES { Self::choose(&cands, reach) } else { None };
            }
        }
        self.counts[3] += cands.len() as u64;
        self.cands = cands;
        let c = best?;
        if self.world.material(c.0, c.1) == VACUUM {
            // Taken by a packet since it was found: drop it and look again.
            self.remove_face(c);
            return self.pick_target(hx, hy, reach_m, packet, peel, picky, home);
        }
        *self.reserved.entry(c).or_insert(0) += 1;
        Some(c)
    }
}
