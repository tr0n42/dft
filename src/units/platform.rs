//! The unit platform: what every unit file (units/*.rs) shares. The list
//! of all specialists (`Units`), ids, what a body offers them (`Skills`),
//! the director that orders them, the once-a-second dispatch to each unit
//! (`specialists_work`), shared ground helpers, what the screen sees of
//! them, and their part of saves and hashes.
//!
//! Units: cutter.rs, hauler.rs, sapper.rs, tunneler.rs, heatsink.rs,
//! forester.rs, abductor.rs. The tick itself, flocks, the director of
//! cutters and haulers, and production are in swarm.rs.
//!
//! The specialists: ground units the Universal Drone Replicator builds (for
//! testing; normally they are earned through the meta tree). Each has one
//! skill, and is only built on a body where that skill has something to work
//! on, and only while there is work for it:
//!
//! - Sappers (anywhere): small and sacrificial, each with a small nuclear
//!   charge. One drops to where the going is hardest, crawls down cracks and
//!   caves until it is deep enough (or drills down where there are none),
//!   arms and goes off: a cavity at the heart, and the rock round about
//!   cracked through, so cutters take it in half the time.
//! - Tunnelers (anywhere): the arthropod drill. A wide shaft straight down,
//!   staging points for cutters along it (the old Tunneler's machinery, see
//!   `Sim::tunnel_work`).
//! - Heat-sink bots (where there is magma, molten metal or sulfur): hover
//!   over an open pool, growing cooling tentacles down through it and
//!   freezing it to stone for the cutters; the heat becomes power.
//! - Foresters (where plants grow): crabs on three legs. They stride to the
//!   thickest plants, shear them into their bags, and set the ground cover
//!   alight with their lasers for the cutters to collect as ash.
//! - Abductors (where animals live): stride to the herds and snatch animals
//!   alive. Only the Dev planet has animals yet (placeholder herds that
//!   panic, src/fauna.rs).
//!
//! Everything here steps once a second of game time (`Sim::specialists_work`).

use super::*;
use crate::production::{Order, Unit};
use crate::world::Look;

/// What a body offers the specialists' skills, from what it was made of.
#[derive(Debug, Clone, Copy, Default)]
pub struct Skills {
    /// Magma, molten metal or molten sulfur somewhere in it (heat-sink bots).
    pub magma: bool,
    /// Plants (foresters).
    pub flora: bool,
    /// Animals (abductors): bodies with a [fauna] section (the Dev planet).
    pub fauna: bool,
    /// Uranium in the whole body (kg): sappers' charges, past the fleet's allowance.
    pub uranium_kg: f64,
}

impl Skills {
    /// Read the body's make-up: its starting books, by material; and its
    /// flora (plants are grown over the ground only as it is first looked
    /// at, so they are never in the books: a body has plants if species were
    /// bred for it, each with its own copies of the plant materials).
    pub fn of(world: &World, min_freeze_k: f64) -> Skills {
        let mats = &world.planet.mats;
        let mut s = Skills::default();
        for (m, &kg) in world.ledger.initial.iter().enumerate() {
            if kg <= 0.0 {
                continue;
            }
            let mat = mats.get(m as MatId);
            if hot_liquid(mat, min_freeze_k) {
                s.magma = true;
            }
            s.uranium_kg += kg * mat.composition.get("U").copied().unwrap_or(0.0);
        }
        s.flora = (0..mats.len()).any(|m| mats.get(m as MatId).name.starts_with("flora "));
        s.fauna = world.planet.spec.fauna.as_ref().is_some_and(|f| f.herds_per_km > 0.0);
        s
    }

    /// Whether a unit of this kind has anything to do on this body.
    pub fn useful(&self, u: Unit) -> bool {
        match u {
            Unit::Cutter | Unit::Hauler | Unit::Sapper | Unit::Tunneler => true,
            Unit::HeatSink => self.magma,
            Unit::Forester => self.flora,
            Unit::Abductor => self.fauna,
        }
    }

    /// Why (for the report and the F1 panel).
    pub fn why(&self, u: Unit) -> &'static str {
        match u {
            Unit::Cutter | Unit::Hauler => "every body",
            Unit::Sapper => "solid ground: every body",
            Unit::Tunneler => "solid ground: every body",
            Unit::HeatSink if self.magma => "magma or molten metal",
            Unit::HeatSink => "no magma or molten metal",
            Unit::Forester if self.flora => "plants",
            Unit::Forester => "no plants",
            Unit::Abductor if self.fauna => "animals",
            Unit::Abductor => "no animals (only the Dev planet has any)",
        }
    }
}

/// A liquid that freezes back into something solid at or above `min_k`:
/// magma, molten metal, molten sulfur (not water, brine or methane).
pub(super) fn hot_liquid(mat: &crate::materials::Material, min_k: f64) -> bool {
    mat.state == State::Liquid && mat.freezes_to.is_some() && mat.freezes_at_k.is_some_and(|k| k >= min_k)
}

/// All the specialists, and what the director knows of their work.
#[derive(Debug, Clone, Default)]
pub struct Units {
    pub(super) sappers: Vec<Sapper>,
    pub(super) bots: Vec<HeatBot>,
    pub(super) walkers: Vec<Walker>,
    /// Full bags on the ground, waiting for couriers.
    pub(super) bags: Vec<Bag>,
    /// Tunnelers (built as units: wide shafts; see `Sim::tunnel_work`).
    pub(super) bores: Vec<Bore>,
    pub(super) next_id: u32,
    /// Open pools of magma near the flocks (their surface cells), and
    /// stands of plants (direction, cells of plant there), found lately.
    pub(super) molten: Vec<(i64, i64)>,
    pub(super) flora: Vec<(f64, u32)>,
    /// Plant cells last found in each of the scan's directions round the
    /// body, and the next direction to look at (a share each scan: the
    /// first look at a place builds its ground, which is slow).
    pub(super) flora_dirs: Vec<u32>,
    pub(super) flora_cursor: usize,
    /// For each material: a hot liquid (and what it freezes to); a plant.
    pub(super) freeze_to: Vec<Option<MatId>>,
    pub(super) is_flora: Vec<bool>,
    /// Totals: sappers set off, cells cracked; kg frozen; kg of plants taken,
    /// fires lit.
    pub blasts: u64,
    pub cracked: u64,
    pub frozen_kg: f64,
    pub harvested_kg: f64,
    pub fires: u64,
    /// Leaps walkers made over ground too steep to walk.
    pub leaps: u64,
    /// Sappers destroyed (by a blast, magma, heat or falling rock), each of
    /// which went off.
    pub destroyed: u64,
    /// The uranium in each sapper's charge being built, in bay order (0: a
    /// conventional charge).
    pub(super) charges: std::collections::VecDeque<f64>,
    /// Conventional charges cast, and kilograms of explosive in them.
    pub conventional: u64,
    pub explosive_kg: f64,
    /// Unreacted uranium left in the rubble (kg), by cell, and all told;
    /// and how much of it cutters have brought back.
    pub(super) dope: CellMap<f64>,
    pub doped_kg: f64,
    pub undoped_kg: f64,
}

/// A specialist as the screen needs it: it moves from `from` to `to`
/// between game times t0 and t1 (straight), in state `state` (each kind's
/// own numbering, see `Sim::unit_views`), with up to three numbers more.
#[derive(Debug, Clone, Copy)]
pub struct UnitView {
    pub kind: Unit,
    pub id: u32,
    pub state: u8,
    pub from: (f64, f64),
    pub to: (f64, f64),
    pub t0: f64,
    pub t1: f64,
    pub a: f32,
    pub b: f32,
    pub c: f32,
    /// A point it is working on (laser mark, tentacle tip, drill head...).
    pub mark: (f64, f64),
    /// A walker's body height over the ground (m); 0 for the rest.
    pub lift: f32,
    /// A walker's laser sweep: where it began (it ends at `mark`), and the
    /// game time it fired; its claw's last cut and when.
    pub sweep_from: (f64, f64),
    pub fired_at: f64,
    pub claw: (f64, f64),
    pub cut_at: f64,
}

impl UnitView {
    pub fn at(&self, t: f64) -> (f64, f64) {
        let k = if self.t1 > self.t0 { ((t - self.t0) / (self.t1 - self.t0)).clamp(0.0, 1.0) } else { 1.0 };
        (self.from.0 + (self.to.0 - self.from.0) * k, self.from.1 + (self.to.1 - self.from.1) * k)
    }
}

/// How often the director looks for magma pools and plants (s).
pub(super) const SCAN_EVERY_S: f64 = 30.0;

impl Units {
    /// Fill the per-material lookups for this body.
    pub(super) fn prepare(&mut self, world: &World, min_freeze_k: f64) {
        let mats = &world.planet.mats;
        self.freeze_to = (0..mats.len())
            .map(|m| {
                let mat = mats.get(m as MatId);
                if hot_liquid(mat, min_freeze_k) {
                    mat.freezes_to.as_ref().and_then(|n| mats.id(n).ok())
                } else {
                    None
                }
            })
            .collect();
        self.is_flora = (0..mats.len()).map(|m| mats.get(m as MatId).name.starts_with("flora ")).collect();
    }

    /// Specialists of this kind out now.
    pub fn count(&self, u: Unit) -> usize {
        match u {
            Unit::Sapper => self.sappers.len(),
            Unit::Tunneler => self.bores.len(),
            Unit::HeatSink => self.bots.len(),
            Unit::Forester | Unit::Abductor => self.walkers.iter().filter(|w| w.kind == u).count(),
            _ => 0,
        }
    }

    pub(super) fn new_id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }
}

pub(super) fn unit_of(p: (f64, f64)) -> (f64, f64) {
    let l = (p.0 * p.0 + p.1 * p.1).sqrt().max(1e-9);
    (p.0 / l, p.1 / l)
}

pub(super) fn len(p: (f64, f64)) -> f64 {
    (p.0 * p.0 + p.1 * p.1).sqrt()
}

pub(super) fn cell_of(p: (f64, f64)) -> (i64, i64) {
    (p.0.floor() as i64, p.1.floor() as i64)
}

pub(super) fn mid(c: (i64, i64)) -> (f64, f64) {
    (c.0 as f64 + 0.5, c.1 as f64 + 0.5)
}

/// Move `p` toward `to` by at most `step`; true if it got there.
pub(super) fn approach(p: &mut (f64, f64), to: (f64, f64), step: f64) -> bool {
    let (dx, dy) = (to.0 - p.0, to.1 - p.1);
    let d = (dx * dx + dy * dy).sqrt();
    if d <= step {
        *p = to;
        true
    } else {
        p.0 += dx / d * step;
        p.1 += dy / d * step;
        false
    }
}

impl Sim {
    /// Where the specialists start from: the mother swarm, over direction
    /// `a` if it reaches that far, else its nearer end.
    pub(super) fn mother_over(&self, a: f64) -> (f64, f64) {
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

    /// The ground's height in direction `a`: the first cell met coming down
    /// from above the highest it could be that is solid (and, with
    /// `under_plants`, not a plant).
    pub(super) fn ground_at(&self, a: f64, under_plants: bool) -> f64 {
        let (ux, uy) = (a.cos(), a.sin());
        let mut r = (self.world.planet.surface_radius(ux, uy) + 64.0).min(self.world.planet.max_radius + 1.0);
        for _ in 0..4000 {
            if r <= 0.0 {
                break;
            }
            let m = self.world.material((ux * r).floor() as i64, (uy * r).floor() as i64);
            if m != VACUUM && !(under_plants && self.units.is_flora.get(m as usize).copied().unwrap_or(false)) {
                return r + 0.5;
            }
            r -= 1.0;
        }
        r.max(0.0)
    }

    /// The flocks working now (landing points and shaft points with cutters).
    pub(super) fn live_sites(&self) -> Vec<usize> {
        (0..self.hubs.len()).filter(|&i| self.hubs[i].kind != HubKind::Refiner && !self.flock[i].dead && !self.wet_hubs.contains(&i)).collect()
    }

    // ---- The director ----

    /// Order specialists while there is work for them (with the Universal
    /// Drone Replicator's bays). Called every second with the factory on.
    pub(super) fn order_specialists(&mut self) {
        let spec = self.eco.units.clone();
        if !spec.universal_replicator {
            return;
        }
        for u in Unit::SPECIALISTS {
            if self.prod.bays_of(u) == 0 {
                continue;
            }
            let want = match u {
                Unit::Sapper => {
                    // Only with a charge in stock or the uranium to make one.
                    if self.can_make(u) {
                        spec.sapper.keep as usize
                    } else {
                        0
                    }
                }
                Unit::Tunneler => ((self.harvesters_alive() as f64 / spec.tunneler.per_cutters.max(1.0)) as usize).clamp(1, spec.tunneler.keep.max(1) as usize),
                Unit::HeatSink => (spec.heatsink.keep as usize).min(self.units.molten.len()),
                Unit::Forester => {
                    if self.units.flora.is_empty() {
                        0
                    } else {
                        spec.forester.keep as usize
                    }
                }
                Unit::Abductor => {
                    if self.fauna.animals.is_empty() {
                        0
                    } else {
                        spec.abductor.keep as usize
                    }
                }
                _ => 0,
            };
            let have = self.units.count(u) + self.prod.queued(u) + self.prod.building(u);
            if have < want {
                self.prod.orders.push_back(Order::Special(u));
            }
        }
    }

    /// Whether the parts for one `u` are in stock or could be made from the
    /// refined stockpile now (a sapper's charge needs uranium).
    pub(super) fn can_make(&self, u: Unit) -> bool {
        self.missing_parts(u).is_empty()
    }

    /// The parts one `u` lacks that the refined stockpile can't make now.
    pub fn missing_parts(&self, u: Unit) -> Vec<String> {
        let r = &self.eco.recipe;
        let need = r.counts(&self.recipe_now(u));
        let els = &self.world.planet.mats.elements;
        need.iter()
            .enumerate()
            .filter(|&(i, &k)| {
                let short = k.saturating_sub(self.parts.n[i]);
                short > 0 && {
                    let comps: Vec<(&crate::recipes::Component, f64)> = r.part[i].make.iter().map(|c| (c, c.kg * short as f64)).collect();
                    self.stock.plan_refined(els, &comps).is_none()
                }
            })
            .map(|(i, _)| r.part[i].name.to_lowercase())
            .collect()
    }

    /// A specialist leaves the replicator.
    pub(super) fn launch_special(&mut self, u: Unit) {
        match u {
            Unit::Sapper => self.launch_sapper(),
            Unit::Tunneler => self.launch_tunneler(),
            Unit::HeatSink => self.launch_bot(),
            Unit::Forester | Unit::Abductor => self.launch_walker(u),
            _ => {}
        }
        self.director_log_push(format!("the replicator finishes a {}", u.name()));
    }

    /// Every second: look for work now and then, and step every specialist.
    pub(super) fn specialists_work(&mut self) {
        if !self.eco.units.universal_replicator {
            return;
        }
        if self.tick % self.ticks(SCAN_EVERY_S) == 0 {
            if self.prod.bays_of(Unit::HeatSink) > 0 {
                let q = clock();
                self.scan_molten();
                self.unit_prof[U_HEATSINK] += clock() - q;
            }
            if self.prod.bays_of(Unit::Forester) > 0 {
                let q = clock();
                self.scan_flora();
                self.prof2[8] += clock() - q;
                self.unit_prof[U_FORESTER] += clock() - q;
            }
        }
        let q = clock();
        self.sappers_work();
        let q1 = clock();
        self.unit_prof[U_SAPPER] += q1 - q;
        self.bots_work();
        let q = clock();
        self.unit_prof[U_HEATSINK] += q - q1;
        let ab = self.unit_prof[U_ABDUCTOR];
        self.walkers_work();
        self.prof2[9] += clock() - q;
        // (Walkers are timed together; the abductors' hunts on their own.)
        self.unit_prof[U_FORESTER] += clock() - q - (self.unit_prof[U_ABDUCTOR] - ab);
        // Melt cools to glass.
        while let Some(&(due, c)) = self.melt.front() {
            if due > self.tick {
                break;
            }
            self.melt.pop_front();
            if self.world.look(c.0, c.1) == Look::MELT {
                self.world.set_look(c.0, c.1, Look::GLASS);
            }
        }
    }






    // ---- For the screen ----

    /// Every specialist, for drawing. States: sapper 0 flying, 1 crawling,
    /// 2 drilling, 3 armed (a = seconds armed); tunneler 0 moving, 1 boring
    /// (a, b = heading); heat-sink bot 0 flying, 1 working, 2 leaving (a =
    /// tentacle reach, `mark` = a tentacle tip); walker 0 falling, 1
    /// unfolding, 2 striding, 3 working (a = facing, b = bag share, c = 1
    /// while cutting, `mark` = its laser's mark if fired in the last second,
    /// else its claw).
    pub fn unit_views(&self) -> Vec<UnitView> {
        let t = |tick: u64| tick as f64 * self.eco.tick_s;
        let sec = self.ticks(1.0) as f64 * self.eco.tick_s;
        let mut out = Vec::new();
        for s in &self.units.sappers {
            out.push(UnitView { kind: Unit::Sapper, id: s.id, state: s.state as u8, from: s.prev, to: s.pos, t0: t(s.moved_at), t1: t(s.moved_at) + sec, a: s.armed_s as f32, b: s.drilled as f32, c: 0.0, mark: s.goal , lift: 0.0, sweep_from: (0.0, 0.0), fired_at: -1e9, claw: (0.0, 0.0), cut_at: -1e9 });
        }
        for b in &self.units.bores {
            out.push(UnitView { kind: Unit::Tunneler, id: (b.resume_at as u32) ^ 0x7000_0000, state: b.active as u8, from: b.prev, to: (b.x, b.y), t0: t(b.moved_at), t1: t(b.moved_at) + sec, a: b.hx as f32, b: b.hy as f32, c: b.length as f32, mark: (b.x, b.y) , lift: 0.0, sweep_from: (0.0, 0.0), fired_at: -1e9, claw: (0.0, 0.0), cut_at: -1e9 });
        }
        for b in &self.units.bots {
            let tip = b.tips.first().map(|&c| mid(c)).unwrap_or(mid(b.spot));
            out.push(UnitView { kind: Unit::HeatSink, id: b.id, state: b.state as u8, from: b.prev, to: b.pos, t0: t(b.moved_at), t1: t(b.moved_at) + sec, a: b.reach as f32, b: (b.life / self.eco.units.heatsink.life_s.max(1.0)) as f32, c: b.tips.len() as f32, mark: tip , lift: 0.0, sweep_from: (0.0, 0.0), fired_at: -1e9, claw: (0.0, 0.0), cut_at: -1e9 });
        }
        for w in &self.units.walkers {
            let firing = self.tick.saturating_sub(w.laser_at) < self.ticks(1.0) && w.laser_at > 0;
            let cutting = self.tick.saturating_sub(w.cut_at) < self.ticks(1.0) && w.cut_at > 0;
            out.push(UnitView {
                kind: w.kind,
                id: w.id,
                state: w.state as u8,
                from: w.prev,
                to: w.pos,
                t0: t(w.moved_at),
                t1: t(w.moved_at) + sec,
                a: w.facing as f32,
                b: (w.bag / if w.kind == Unit::Abductor { self.eco.units.abductor.bag_kg } else { self.eco.units.forester.bag_kg }.max(1.0)) as f32,
                c: if firing { 2.0 } else if cutting { 1.0 } else { 0.0 },
                mark: if firing { w.laser } else { w.claw },
                lift: w.lift as f32,
                sweep_from: w.laser_from,
                fired_at: if w.laser_at > 0 { t(w.laser_at) } else { -1e9 },
                claw: w.claw,
                cut_at: if w.cut_at > 0 { t(w.cut_at) } else { -1e9 },
            });
        }
        // Dropped bags (state 9), waiting on the ground: t0 is when the
        // couriers lift it (game seconds).
        for b in &self.units.bags {
            let due = t(b.due);
            out.push(UnitView { kind: b.kind, id: b.id, state: 9, from: b.pos, to: b.pos, t0: due, t1: due, a: 1.0, b: 1.0, c: 0.0, mark: b.pos, lift: 0.0, sweep_from: (0.0, 0.0), fired_at: -1e9, claw: (0.0, 0.0), cut_at: -1e9 });
        }
        out
    }

    /// Specialists of this kind out now.
    pub fn specialists_out(&self, u: Unit) -> usize {
        self.units.count(u)
    }

    /// What the specialists have done: sappers set off, cells they cracked,
    /// kg of magma frozen, kg of plants taken, fires lit.
    pub fn specialist_totals(&self) -> (u64, u64, f64, f64, u64) {
        let u = &self.units;
        (u.blasts, u.cracked, u.frozen_kg, u.harvested_kg, u.fires)
    }

    /// The specialists, for the F1 panel.
    pub fn specialist_lines(&self) -> Vec<String> {
        if !self.eco.units.universal_replicator {
            return Vec::new();
        }
        let mut v = vec!["universal drone replicator (testing):".to_string()];
        for u in Unit::SPECIALISTS {
            let built = self.prod.built[u.index()];
            if self.prod.bays_of(u) == 0 {
                v.push(format!("  {:14} not built here: {}", u.name(), self.skills.why(u)));
            } else {
                let missing = self.missing_parts(u);
                let wait = if missing.is_empty() { String::new() } else { format!(" · can't make: {}", missing.join(", ")) };
                v.push(format!("  {:14} {} out, {} built{wait}", u.name(), self.units.count(u), built));
            }
        }
        let u = &self.units;
        v.push(format!("  blasts {} ({} set off by being destroyed; {} cells cracked) · frozen {:.0} t · plants taken {:.0} t, fires {}, leaps {}", u.blasts, u.destroyed, u.cracked, u.frozen_kg / 1000.0, u.harvested_kg / 1000.0, u.fires, u.leaps));
        if self.prod.bays_of(Unit::HeatSink) > 0 {
            v.push(format!("  open magma pools near the flocks: {}", u.molten.len()));
        }
        if self.prod.bays_of(Unit::Forester) > 0 {
            v.push(format!("  stands of plants found: {}", u.flora.len()));
        }
        if self.skills.fauna {
            let f = &self.fauna;
            v.push(format!("  animals: {} left · {} caught alive ({:.1} t) · {} lost otherwise", f.animals.len(), f.caught, f.caught_kg / 1000.0, f.lost));
        }
        v.extend(self.charge_lines());
        v
    }

    // ---- Saves ----

    pub(super) fn write_units(&self, o: &mut crate::save::W) {
        let u = &self.units;
        o.u32(u.next_id);
        o.u64(u.blasts);
        o.u64(u.cracked);
        o.f64(u.frozen_kg);
        o.f64(u.harvested_kg);
        o.u64(u.fires);
        o.u64(u.destroyed);
        o.u32(u.sappers.len() as u32);
        for s in &u.sappers {
            o.u32(s.id);
            o.u8(s.state as u8);
            for v in [s.pos.0, s.pos.1, s.goal.0, s.goal.1, s.along, s.drilled, s.armed_s, s.want, s.ground_r] {
                o.f64(v);
            }
            o.u32(s.path.len() as u32);
            for &c in &s.path {
                put_cell(o, c);
            }
        }
        o.u32(u.bots.len() as u32);
        for b in &u.bots {
            o.u32(b.id);
            o.u8(b.state as u8);
            put_cell(o, b.spot);
            for v in [b.pos.0, b.pos.1, b.hover.0, b.hover.1, b.reach, b.life, b.idle_s, b.frozen_kg] {
                o.f64(v);
            }
        }
        o.u32(u.walkers.len() as u32);
        for w in &u.walkers {
            o.u32(w.id);
            o.u8(w.kind.index() as u8);
            // A leap is saved as having landed, striding on.
            let (state, pos) = match (w.state, w.jump) {
                (WalkState::Jump, Some(j)) => (WalkState::Stride, (j.a1.cos() * j.g1, j.a1.sin() * j.g1)),
                (s, _) => (s, w.pos),
            };
            o.u8(state as u8);
            for v in [pos.0, pos.1, w.goal_a, w.facing, w.life, w.timer, w.bag, w.taken_kg] {
                o.f64(v);
            }
        }
        o.u32(u.bores.len() as u32);
        for b in &u.bores {
            for v in [b.x, b.y, b.hx, b.hy, b.aim.0, b.aim.1, b.length, b.progress_s] {
                o.f64(v);
            }
            o.u8(b.active as u8);
            o.u64(b.resume_at);
        }
    }

    /// What version 12 added: walkers' heights and bags, dropped bags, and
    /// loose debris.
    pub(super) fn write_units_12(&self, o: &mut crate::save::W) {
        let mut looks: Vec<((i64, i64), u8)> = self.world.looks.values().flat_map(|l| l.iter().map(|(c, k)| (*c, *k))).collect();
        looks.sort_unstable();
        o.u32(looks.len() as u32);
        for (c, k) in looks {
            put_cell(o, c);
            o.u8(k);
        }
        o.u32(self.melt.len() as u32);
        for &(due, c) in &self.melt {
            o.u64(due);
            put_cell(o, c);
        }
        let loose = sorted(self.loose.iter());
        o.u32(loose.len() as u32);
        for c in loose {
            put_cell(o, c);
        }
        let u = &self.units;
        for w in &u.walkers {
            o.f64(w.lift);
            o.u32(w.load.len() as u32);
            for &(m, n) in &w.load {
                o.u16(m);
                o.f64(n);
            }
        }
        o.u32(u.bags.len() as u32);
        for b in &u.bags {
            o.u32(b.id);
            o.u8(b.kind.index() as u8);
            o.f64(b.pos.0);
            o.f64(b.pos.1);
            o.f64(b.kg);
            o.u64(b.due);
            o.u32(b.load.len() as u32);
            for &(m, n) in &b.load {
                o.u16(m);
                o.f64(n);
            }
        }
    }

    /// After loading: loose rubble may move again.
    pub(super) fn resettle(&mut self) {
        self.settling = self.loose.clone();
    }

    pub(super) fn read_units_12(&mut self, r: &mut crate::save::R) -> Result<(), String> {
        for _ in 0..r.u32()? {
            let c = get_cell(r)?;
            let k = r.u8()?;
            self.world.set_look(c.0, c.1, k);
        }
        for _ in 0..r.u32()? {
            let due = r.u64()?;
            let c = get_cell(r)?;
            self.melt.push_back((due, c));
        }
        for _ in 0..r.u32()? {
            self.loose.insert(get_cell(r)?);
        }
        let nm = self.world.planet.mats.len();
        let load = |r: &mut crate::save::R| -> Result<Vec<(MatId, f64)>, String> {
            let mut v = Vec::new();
            for _ in 0..r.u32()? {
                let (m, n) = (r.u16()?, r.f64()?);
                if (m as usize) < nm {
                    v.push((m, n));
                }
            }
            Ok(v)
        };
        for k in 0..self.units.walkers.len() {
            self.units.walkers[k].lift = r.f64()?;
            self.units.walkers[k].load = load(r)?;
        }
        for _ in 0..r.u32()? {
            let id = r.u32()?;
            let kind = Unit::from_index(r.u8()? as usize).ok_or("bad unit")?;
            let pos = (r.f64()?, r.f64()?);
            let kg = r.f64()?;
            let due = r.u64()?;
            let l = load(r)?;
            self.units.bags.push(Bag { id, kind, pos, kg, load: l, due });
        }
        Ok(())
    }

    pub(super) fn read_units(&mut self, r: &mut crate::save::R) -> Result<(), String> {
        let tick = self.tick;
        let u = &mut self.units;
        u.next_id = r.u32()?;
        u.blasts = r.u64()?;
        u.cracked = r.u64()?;
        u.frozen_kg = r.f64()?;
        u.harvested_kg = r.f64()?;
        u.fires = r.u64()?;
        u.destroyed = r.u64()?;
        for _ in 0..r.u32()? {
            let id = r.u32()?;
            let state = [SapState::Fly, SapState::Crawl, SapState::Drill, SapState::Arm][(r.u8()? as usize).min(3)];
            let mut f = [0.0; 9];
            for v in f.iter_mut() {
                *v = r.f64()?;
            }
            let mut path = Vec::new();
            for _ in 0..r.u32()? {
                path.push(get_cell(r)?);
            }
            u.sappers.push(Sapper { id, state, pos: (f[0], f[1]), prev: (f[0], f[1]), moved_at: tick, goal: (f[2], f[3]), path, along: f[4], drilled: f[5], armed_s: f[6], want: f[7], ground_r: f[8], fissile: 0.0 });
        }
        for _ in 0..r.u32()? {
            let id = r.u32()?;
            let state = [HeatState::Fly, HeatState::Work, HeatState::Leave][(r.u8()? as usize).min(2)];
            let spot = get_cell(r)?;
            let mut f = [0.0; 8];
            for v in f.iter_mut() {
                *v = r.f64()?;
            }
            u.bots.push(HeatBot { id, state, pos: (f[0], f[1]), prev: (f[0], f[1]), moved_at: tick, spot, hover: (f[2], f[3]), reach: f[4], life: f[5], idle_s: f[6], frozen_kg: f[7], tips: Vec::new() });
        }
        for _ in 0..r.u32()? {
            let id = r.u32()?;
            let kind = Unit::from_index(r.u8()? as usize).ok_or("bad unit")?;
            let state = [WalkState::Drop, WalkState::Unfold, WalkState::Stride, WalkState::Work][(r.u8()? as usize).min(3)];
            // (A leap is saved as having landed.)
            let mut f = [0.0; 8];
            for v in f.iter_mut() {
                *v = r.f64()?;
            }
            let pos = (f[0], f[1]);
            u.walkers.push(Walker { id, kind, state, pos, prev: pos, moved_at: tick, goal_a: f[2], facing: f[3], life: f[4], timer: f[5], bag: f[6], taken_kg: f[7], laser: pos, laser_at: 0, claw: pos, cut_at: 0, lift: LIFT_MIN_M, load: Vec::new(), jump: None, laser_from: pos });
        }
        for _ in 0..r.u32()? {
            let mut f = [0.0; 8];
            for v in f.iter_mut() {
                *v = r.f64()?;
            }
            let active = r.u8()? != 0;
            let resume_at = r.u64()?;
            u.bores.push(Bore { x: f[0], y: f[1], hx: f[2], hy: f[3], aim: (f[4], f[5]), length: f[6], progress_s: f[7], active, resume_at, prev: (f[0], f[1]), moved_at: tick });
        }
        Ok(())
    }

    /// The specialists, for the state hash.
    pub(super) fn units_hash(&self, mut mix: impl FnMut(u64)) {
        let u = &self.units;
        mix(u.blasts);
        mix(u.cracked);
        mix(u.frozen_kg.to_bits());
        mix(u.harvested_kg.to_bits());
        for s in &u.sappers {
            mix(s.pos.0.to_bits());
            mix(s.pos.1.to_bits());
        }
        for b in &u.bots {
            mix(b.pos.0.to_bits());
            mix(b.reach.to_bits());
        }
        for w in &u.walkers {
            mix(w.pos.0.to_bits());
            mix(w.bag.to_bits());
        }
        for b in &u.bores {
            mix(b.x.to_bits());
            mix(b.y.to_bits());
        }
    }
}

// ---- Sappers' charges: nuclear, or conventional when no uranium can be had ----

impl Sim {

    /// What one `u` takes from the parts stock now: a sapper with a
    /// conventional charge has no nuclear charge.
    pub(super) fn recipe_now(&self, u: Unit) -> crate::recipes::Recipe {
        let mut rec = self.eco.recipe.of(u).clone();
        if u == Unit::Sapper && self.sapper_charge().is_some() {
            rec.parts.retain(|n| n.part != "nuclear-charge");
        }
        rec
    }

}
