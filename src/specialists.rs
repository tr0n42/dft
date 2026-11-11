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
//! - Abductors (where animals live): no body has animals yet, so none is
//!   ever built; their recipe and bay wait for them.
//!
//! Everything here steps once a second of game time (`Sim::specialists_work`).

use super::*;
use crate::production::{Order, Unit};

/// What a body offers the specialists' skills, from what it was made of.
#[derive(Debug, Clone, Copy, Default)]
pub struct Skills {
    /// Magma, molten metal or molten sulfur somewhere in it (heat-sink bots).
    pub magma: bool,
    /// Plants (foresters).
    pub flora: bool,
    /// Animals (abductors): none yet anywhere.
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
            Unit::Abductor => "no animals (none on any body yet)",
        }
    }
}

/// A liquid that freezes back into something solid at or above `min_k`:
/// magma, molten metal, molten sulfur (not water, brine or methane).
fn hot_liquid(mat: &crate::materials::Material, min_k: f64) -> bool {
    mat.state == State::Liquid && mat.freezes_to.is_some() && mat.freezes_at_k.is_some_and(|k| k >= min_k)
}

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
    id: u32,
    state: SapState,
    pos: (f64, f64),
    prev: (f64, f64),
    moved_at: u64,
    /// Where it lands.
    goal: (f64, f64),
    /// Its way down through open cells, and how far along it is (cells).
    path: Vec<(i64, i64)>,
    along: f64,
    /// Metres drilled, seconds armed.
    drilled: f64,
    armed_s: f64,
    /// Depth it wants to go off at (m under the ground at its landing spot),
    /// and that ground's height (distance from the centre).
    want: f64,
    ground_r: f64,
}

/// What a heat-sink bot is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeatState {
    Fly,
    Work,
    /// Burned out or out of work: back up to the mother swarm.
    Leave,
}

#[derive(Debug, Clone)]
pub struct HeatBot {
    id: u32,
    state: HeatState,
    pos: (f64, f64),
    prev: (f64, f64),
    moved_at: u64,
    /// The pool's surface cell it works, and where it hovers over it.
    spot: (i64, i64),
    hover: (f64, f64),
    /// How far its tentacles reach (m), seconds of life left, seconds with
    /// nothing to freeze, kg frozen.
    reach: f64,
    life: f64,
    idle_s: f64,
    frozen_kg: f64,
    /// Where its tentacles last froze (up to six), for drawing.
    tips: Vec<(i64, i64)>,
}

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
}

#[derive(Debug, Clone)]
pub struct Walker {
    id: u32,
    kind: Unit,
    state: WalkState,
    pos: (f64, f64),
    prev: (f64, f64),
    moved_at: u64,
    /// The direction (radians) it is heading for, and which way it faces
    /// along the ground (+1 counter-clockwise).
    goal_a: f64,
    facing: f64,
    life: f64,
    timer: f64,
    /// kg in its bag, and kg taken all told.
    bag: f64,
    taken_kg: f64,
    /// Where its laser last fired, and when (tick).
    laser: (f64, f64),
    laser_at: u64,
    /// The cell its claws are on, and when it last cut (tick).
    claw: (f64, f64),
    cut_at: u64,
}

/// All the specialists, and what the director knows of their work.
#[derive(Debug, Clone, Default)]
pub struct Units {
    pub(super) sappers: Vec<Sapper>,
    pub(super) bots: Vec<HeatBot>,
    pub(super) walkers: Vec<Walker>,
    /// Tunnelers (built as units: wide shafts; see `Sim::tunnel_work`).
    pub(super) bores: Vec<Bore>,
    next_id: u32,
    /// Open pools of magma near the flocks (their surface cells), and
    /// stands of plants (direction, cells of plant there), found lately.
    molten: Vec<(i64, i64)>,
    flora: Vec<(f64, u32)>,
    /// For each material: a hot liquid (and what it freezes to); a plant.
    freeze_to: Vec<Option<MatId>>,
    is_flora: Vec<bool>,
    /// Totals: sappers set off, cells cracked; kg frozen; kg of plants taken,
    /// fires lit.
    pub blasts: u64,
    pub cracked: u64,
    pub frozen_kg: f64,
    pub harvested_kg: f64,
    pub fires: u64,
    /// Sappers destroyed (by a blast, magma, heat or falling rock), each of
    /// which went off.
    pub destroyed: u64,
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
}

impl UnitView {
    pub fn at(&self, t: f64) -> (f64, f64) {
        let k = if self.t1 > self.t0 { ((t - self.t0) / (self.t1 - self.t0)).clamp(0.0, 1.0) } else { 1.0 };
        (self.from.0 + (self.to.0 - self.from.0) * k, self.from.1 + (self.to.1 - self.from.1) * k)
    }
}

/// How often the director looks for magma pools and plants (s).
const SCAN_EVERY_S: f64 = 30.0;
/// Ground this hot next to a sapper (a blast's heat) destroys it, and so sets it off.
const SAPPER_COOK_K: f64 = 800.0;

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

    fn new_id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }
}

fn unit_of(p: (f64, f64)) -> (f64, f64) {
    let l = (p.0 * p.0 + p.1 * p.1).sqrt().max(1e-9);
    (p.0 / l, p.1 / l)
}

fn len(p: (f64, f64)) -> f64 {
    (p.0 * p.0 + p.1 * p.1).sqrt()
}

fn cell_of(p: (f64, f64)) -> (i64, i64) {
    (p.0.floor() as i64, p.1.floor() as i64)
}

fn mid(c: (i64, i64)) -> (f64, f64) {
    (c.0 as f64 + 0.5, c.1 as f64 + 0.5)
}

/// Move `p` toward `to` by at most `step`; true if it got there.
fn approach(p: &mut (f64, f64), to: (f64, f64), step: f64) -> bool {
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
    fn mother_over(&self, a: f64) -> (f64, f64) {
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
    fn ground_at(&self, a: f64, under_plants: bool) -> f64 {
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
    fn live_sites(&self) -> Vec<usize> {
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
                Unit::Abductor => 0,
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
    fn can_make(&self, u: Unit) -> bool {
        self.missing_parts(u).is_empty()
    }

    /// The parts one `u` lacks that the refined stockpile can't make now.
    pub fn missing_parts(&self, u: Unit) -> Vec<String> {
        let r = &self.eco.recipe;
        let need = r.counts(r.of(u));
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
                self.scan_molten();
            }
            if self.prod.bays_of(Unit::Forester) > 0 {
                self.scan_flora();
            }
        }
        self.sappers_work();
        self.bots_work();
        self.walkers_work();
    }

    /// Keep what was dug out by a specialist (a share of it: the rest is
    /// crushed to spoil): mined from the world, credited, into the stockpile.
    fn take_cell(&mut self, c: (i64, i64), keep: f64) {
        let m = self.world.material(c.0, c.1);
        if m == VACUUM {
            return;
        }
        let mat = self.world.planet.mats.get(m);
        let (kg, mult) = (mat.density, mat.multiplier);
        let comp = mat.composition.clone();
        self.world.mine_cell(c.0, c.1);
        self.progress.remove(&c);
        self.reserved.remove(&c);
        self.open_cell(c);
        let credited = kg * mult * keep;
        self.stock.add_composition(&self.world.planet.mats.elements, &comp, kg * keep);
        self.mass += credited;
        self.delivered_credited += credited;
        self.window_mined += kg;
    }

    // ---- Sappers ----

    /// Where a sapper is wanted: the flock where the going is hardest
    /// (slowest per kg lately), away from where other sappers are headed.
    fn sapper_site(&self) -> Option<usize> {
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

    fn launch_sapper(&mut self) {
        let Some(site) = self.sapper_site() else { return };
        let (hx, hy) = self.site_pos(site);
        let a = hy.atan2(hx);
        let g = self.ground_at(a, true);
        let goal = (a.cos() * (g + 1.0), a.sin() * (g + 1.0));
        let start = self.mother_over(a);
        let id = self.units.new_id();
        let sp = &self.eco.units.sapper;
        let r = sp.radius_m_per_cbrt_kt * sp.yield_kt.max(0.0).cbrt();
        self.units.sappers.push(Sapper { id, state: SapState::Fly, pos: start, prev: start, moved_at: self.tick, goal, path: Vec::new(), along: 0.0, drilled: 0.0, armed_s: 0.0, want: r + sp.bury_m, ground_r: g });
    }

    /// From where it landed, its way down: the deepest open cell it can
    /// crawl to (along cracks and through caves, always touching rock)
    /// within crawl_m, stopping at the first deep enough.
    fn sapper_path(&self, from: (f64, f64), ground_r: f64, want: f64, limit_m: f64) -> Vec<(i64, i64)> {
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
            if depth > best_depth {
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

    fn sappers_work(&mut self) {
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
                blasts.push(s.pos);
                s.armed_s = -1.0;
                self.units.sappers[k] = s;
                continue;
            }
            match s.state {
                SapState::Fly => {
                    if approach(&mut s.pos, s.goal, sp.fly_speed) {
                        s.state = SapState::Crawl;
                        s.path = self.sapper_path(s.pos, s.ground_r, s.want, sp.crawl_m);
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
                        for o in [-1.0, 0.0, 1.0] {
                            let q = cell_of((next.0 + side.0 * o, next.1 + side.1 * o));
                            self.take_cell(q, sp.spoil_recovery);
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
                        blasts.push(s.pos);
                        s.armed_s = -1.0;
                    }
                }
            }
            self.units.sappers[k] = s;
        }
        self.units.sappers.retain(|s| s.armed_s >= 0.0);
        for at in blasts {
            self.sapper_blast(at, sp.yield_kt);
        }
    }

    /// Whether whatever is round a sapper at `p` destroys it: its own cell
    /// filled in (buried by falling rock or rubble), or magma or ground hot
    /// from a blast touching it.
    fn sapper_destroyed(&self, p: (f64, f64)) -> bool {
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
        let arm = self.eco.units.sapper.arm_s;
        for s in self.units.sappers.iter_mut() {
            if s.armed_s >= 0.0 && dist2(s.pos, at) < r * r && s.armed_s < arm {
                s.state = SapState::Arm;
                s.armed_s = arm;
                self.units.destroyed += 1;
            }
        }
    }

    /// A sapper goes off at `at`: a flash and a shock wave (for the screen),
    /// a cavity blown out at the heart, and all the rock round about cracked
    /// through (ragged at the edge; hard rock holds best), so cutters take
    /// it in half the time.
    pub fn sapper_blast(&mut self, at: (f64, f64), yield_kt: f64) {
        let sp = &self.eco.units.sapper;
        let r = sp.radius_m_per_cbrt_kt * yield_kt.max(0.0).cbrt();
        self.booms.push(Boom { x: at.0, y: at.1, size_m: (2.0 * r) as f32 });
        self.explosions += 1;
        self.units.blasts += 1;
        self.happen(Happening::Explosion, at);
        let core = (0.3 * r).min(12.0);
        self.crater(at, core);
        let c0 = cell_of(at);
        let ci = core.ceil() as i64;
        for dy in -ci..=ci {
            for dx in -ci..=ci {
                if ((dx * dx + dy * dy) as f64) <= core * core {
                    self.sandbox.heat_cell(&self.world, (c0.0 + dx, c0.1 + dy), BLAST_K);
                }
            }
        }
        let seed = hash(self.tick, c0.0, c0.1, 0x5A99);
        let ri = r.ceil() as i64 + 1;
        let mut cracked = 0u64;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                let c = (c0.0 + dx, c0.1 + dy);
                let m = self.world.material(c.0, c.1);
                if m == VACUUM {
                    continue;
                }
                let mat = self.world.planet.mats.get(m);
                if mat.state != State::Solid && mat.state != State::Powder {
                    continue;
                }
                let d = ((dx * dx + dy * dy) as f64).sqrt();
                let n = crate::noise::unit(hash(seed, dx, dy, 1));
                if d > r * (0.75 + 0.35 * n) * (1.0 - 0.03 * mat.hardness as f64) {
                    continue;
                }
                if self.fractured.insert(c) {
                    cracked += 1;
                }
            }
        }
        self.units.cracked += cracked;
        // Sappers caught in it go off too, a moment later.
        self.sappers_caught(at, r * 0.9);
    }

    // ---- Tunnelers ----

    fn launch_tunneler(&mut self) {
        self.units.bores.push(Bore { x: 0.0, y: 0.0, hx: 0.0, hy: 0.0, aim: (0.0, 0.0), length: 0.0, progress_s: 0.0, active: false, resume_at: self.tick, prev: (0.0, 0.0), moved_at: self.tick });
    }

    /// The old Tunneler's numbers, with the ground unit's own over them.
    pub(super) fn unit_tunneler(&self) -> crate::economy::Tunneler {
        let u = &self.eco.units.tunneler;
        let mut t = self.eco.tunneler.clone();
        t.upkeep = u.upkeep;
        t.bore_m = u.bore_m;
        t.drill_power = u.drill_power;
        t.energy_per_tonne = u.energy_per_tonne;
        t.spoil_recovery = u.spoil_recovery;
        t.max_length_m = u.max_length_m;
        t.hub_every_m = u.hub_every_m;
        t
    }

    /// Where a tunneler starts its next shaft: under a working flock (its
    /// staging points feed that flock's neighbours), else anywhere.
    pub(super) fn unit_bore_start(&mut self) -> Option<(f64, f64)> {
        let sites: Vec<usize> = self.live_sites().into_iter().filter(|&i| self.hubs[i].kind == HubKind::Seed).collect();
        if sites.is_empty() {
            return None;
        }
        let k = (hash(self.tick, self.units.bores.len() as i64, self.tunnels_done as i64, 0x7B0) % sites.len() as u64) as usize;
        let (x, y) = self.site_pos(sites[k]);
        let a = y.atan2(x);
        Some((a.cos(), a.sin()))
    }

    // ---- Heat-sink bots ----

    /// Look round the working flocks for open pools of magma: hot liquid
    /// cells with open space over them.
    fn scan_molten(&mut self) {
        let mut found: Vec<(i64, i64)> = Vec::new();
        let sites = self.live_sites();
        let step = (sites.len() / 64).max(1);
        for &i in sites.iter().step_by(step) {
            let (hx, hy) = self.site_pos(i);
            let a = hy.atan2(hx);
            let g = self.ground_at(a, true);
            let c0 = cell_of((a.cos() * g, a.sin() * g));
            for dy in (-60i64..=60).step_by(2) {
                for dx in (-60i64..=60).step_by(2) {
                    let c = (c0.0 + dx, c0.1 + dy);
                    let m = self.world.material(c.0, c.1);
                    if m == VACUUM || self.units.freeze_to.get(m as usize).copied().flatten().is_none() {
                        continue;
                    }
                    let up = unit_of(mid(c));
                    let above = cell_of((mid(c).0 + up.0 * 1.2, mid(c).1 + up.1 * 1.2));
                    if self.world.material(above.0, above.1) != VACUUM {
                        continue;
                    }
                    if found.iter().any(|f| (f.0 - c.0).pow(2) + (f.1 - c.1).pow(2) < 30 * 30) {
                        continue;
                    }
                    found.push(c);
                    if found.len() >= 64 {
                        break;
                    }
                }
            }
        }
        self.units.molten = found;
    }

    fn launch_bot(&mut self) {
        let free = self.units.molten.iter().copied().find(|c| !self.units.bots.iter().any(|b| (b.spot.0 - c.0).pow(2) + (b.spot.1 - c.1).pow(2) < 40 * 40));
        let Some(spot) = free.or_else(|| self.units.molten.first().copied()) else { return };
        let up = unit_of(mid(spot));
        let hover = (mid(spot).0 + up.0 * 7.0, mid(spot).1 + up.1 * 7.0);
        let start = self.mother_over(hover.1.atan2(hover.0));
        let id = self.units.new_id();
        let life = self.eco.units.heatsink.life_s;
        self.units.bots.push(HeatBot { id, state: HeatState::Fly, pos: start, prev: start, moved_at: self.tick, spot, hover, reach: 2.0, life, idle_s: 0.0, frozen_kg: 0.0, tips: Vec::new() });
    }

    fn bots_work(&mut self) {
        if self.units.bots.is_empty() {
            return;
        }
        let hs = self.eco.units.heatsink.clone();
        for k in 0..self.units.bots.len() {
            let mut b = self.units.bots[k].clone();
            b.prev = b.pos;
            b.moved_at = self.tick;
            match b.state {
                HeatState::Fly => {
                    if approach(&mut b.pos, b.hover, hs.speed) {
                        b.state = HeatState::Work;
                    }
                }
                HeatState::Work => {
                    b.life -= 1.0;
                    b.reach = (b.reach + hs.grow_m_per_s).min(hs.reach_m);
                    // The hot liquid nearest its tentacles (mostly downward:
                    // they grow down through the pool).
                    let ri = b.reach.ceil() as i64;
                    let down = unit_of(mid(b.spot));
                    let mut cands: Vec<(f64, (i64, i64), MatId)> = Vec::new();
                    for dy in -ri..=ri {
                        for dx in -ri..=ri {
                            let d2 = (dx * dx + dy * dy) as f64;
                            if d2 > b.reach * b.reach {
                                continue;
                            }
                            let c = (b.spot.0 + dx, b.spot.1 + dy);
                            let m = self.world.material(c.0, c.1);
                            let Some(to) = self.units.freeze_to.get(m as usize).copied().flatten() else { continue };
                            let _ = to;
                            // Below the spot counts as nearer than beside it.
                            let out = (dx as f64 * down.0 + dy as f64 * down.1).max(0.0);
                            cands.push((d2.sqrt() + 0.5 * out, c, m));
                        }
                    }
                    cands.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
                    if cands.is_empty() {
                        b.idle_s += 1.0;
                    } else {
                        b.idle_s = 0.0;
                    }
                    b.tips.clear();
                    for &(_, c, m) in cands.iter().take(hs.freeze_per_s as usize) {
                        let Some(to) = self.units.freeze_to[m as usize] else { continue };
                        if self.world.change_cell(c.0, c.1, to).is_none() {
                            continue;
                        }
                        self.apply_sandbox(vec![Change { cell: c, from: m, to }]);
                        let mat = self.world.planet.mats.get(m);
                        let (kg, k) = (mat.density, mat.freezes_at_k.unwrap_or(500.0));
                        self.sandbox.temps.insert(c, (k - 150.0).max(100.0));
                        self.sandbox.wake(c);
                        self.energy += kg / 1000.0 * hs.power_per_tonne;
                        b.frozen_kg += kg;
                        self.units.frozen_kg += kg;
                        b.tips.push(c);
                    }
                    if b.life <= 0.0 {
                        b.state = HeatState::Leave;
                    } else if b.idle_s > 20.0 {
                        // Nothing left in reach: another pool, or home.
                        let here = b.spot;
                        let next = self.units.molten.iter().copied().filter(|c| (c.0 - here.0).pow(2) + (c.1 - here.1).pow(2) > 40 * 40).find(|c| !self.units.bots.iter().any(|o| o.id != b.id && (o.spot.0 - c.0).pow(2) + (o.spot.1 - c.1).pow(2) < 40 * 40));
                        match next {
                            Some(spot) => {
                                let up = unit_of(mid(spot));
                                b.spot = spot;
                                b.hover = (mid(spot).0 + up.0 * 7.0, mid(spot).1 + up.1 * 7.0);
                                b.reach = 2.0;
                                b.idle_s = 0.0;
                                b.state = HeatState::Fly;
                            }
                            None => b.state = HeatState::Leave,
                        }
                    }
                }
                HeatState::Leave => {
                    let home = self.mother_over(b.pos.1.atan2(b.pos.0));
                    if approach(&mut b.pos, home, hs.speed) {
                        b.life = f64::NEG_INFINITY;
                    }
                }
            }
            self.units.bots[k] = b;
        }
        self.units.bots.retain(|b| b.life > f64::NEG_INFINITY);
    }

    // ---- Foresters (and abductors) ----

    /// Stands of plants round the body: 256 directions, and round each
    /// working flock, counting plant cells over the ground.
    fn scan_flora(&mut self) {
        let mut out = Vec::new();
        let n = 256;
        let mut dirs: Vec<f64> = (0..n).map(|i| std::f64::consts::TAU * i as f64 / n as f64).collect();
        for i in self.live_sites() {
            let (x, y) = self.site_pos(i);
            dirs.push(y.atan2(x));
        }
        for a in dirs {
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
    fn flora_near(&self, a: f64, w: f64) -> u32 {
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

    fn launch_walker(&mut self, u: Unit) {
        // The thickest stand no other walker is on.
        let busy = |a: f64, ws: &Vec<Walker>| ws.iter().any(|w| ang_dist(w.goal_a, a) * self.world.planet.radius < 150.0);
        let Some(&(a, _)) = self.units.flora.iter().find(|(a, _)| !busy(*a, &self.units.walkers)).or_else(|| self.units.flora.first()) else { return };
        let start = self.mother_over(a);
        let id = self.units.new_id();
        let life = self.eco.units.forester.life_s;
        self.units.walkers.push(Walker { id, kind: u, state: WalkState::Drop, pos: start, prev: start, moved_at: self.tick, goal_a: a, facing: 1.0, life, timer: 0.0, bag: 0.0, taken_kg: 0.0, laser: start, laser_at: 0, claw: start, cut_at: 0 });
    }

    fn walkers_work(&mut self) {
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
                    let na = if da.abs() <= step { w.goal_a } else { a + step * w.facing };
                    let g = self.ground_at(na, true);
                    w.pos = (na.cos() * g, na.sin() * g);
                    if da.abs() <= step {
                        w.state = WalkState::Work;
                        w.timer = 0.0;
                    }
                }
                WalkState::Work => {
                    self.walker_feed(&mut w, &fs);
                }
            }
            self.units.walkers[k] = w;
        }
        self.units.walkers.retain(|w| w.life > 0.0);
    }

    /// One second of a forester's work where it stands: shear the nearest
    /// plants in reach into its bag, creep toward the next, now and then set
    /// the ground cover alight; with nothing left near, stride to the next
    /// stand.
    fn walker_feed(&mut self, w: &mut Walker, fs: &crate::economy::ForesterSpec) {
        let a = w.pos.1.atan2(w.pos.0);
        let r = len(w.pos).max(1.0);
        let up = unit_of(w.pos);
        let side = (-up.1, up.0);
        // Plant cells near it (claws reach a little ahead and above).
        let reach = fs.reach_m;
        let ri = reach.ceil() as i64 + 1;
        let c0 = cell_of(w.pos);
        let mut near: Vec<(f64, (i64, i64))> = Vec::new();
        for dy in -ri - 4..=ri + 4 {
            for dx in -ri - 4..=ri + 4 {
                let c = (c0.0 + dx, c0.1 + dy);
                let m = self.world.material(c.0, c.1);
                if !self.units.is_flora.get(m as usize).copied().unwrap_or(false) {
                    continue;
                }
                let p = mid(c);
                let along = (p.0 - w.pos.0) * side.0 + (p.1 - w.pos.1) * side.1;
                let height = (p.0 - w.pos.0) * up.0 + (p.1 - w.pos.1) * up.1;
                // Its claws reach about its height up and `reach` round.
                if height < -2.0 || height > reach + 4.0 || along.abs() > reach + 4.0 {
                    continue;
                }
                near.push((along.abs() + 0.3 * height.max(0.0), c));
            }
        }
        near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let cuts = (1.0 / fs.cut_s.max(0.05)).round().max(1.0) as usize;
        let mut cut = 0;
        for &(_, c) in near.iter().take(cuts) {
            let m = self.world.material(c.0, c.1);
            let kg = self.world.planet.mats.get(m).density;
            self.take_cell(c, 1.0);
            w.bag += kg;
            w.taken_kg += kg;
            self.units.harvested_kg += kg;
            w.claw = mid(c);
            w.cut_at = self.tick;
            cut += 1;
        }
        if w.bag >= fs.bag_kg {
            // Couriers latch on and carry it up; a new one grows.
            w.bag = 0.0;
        }
        // Its laser: now and then, the ground cover some way off is set alight.
        w.timer += 1.0;
        if w.timer >= fs.burn_every_s {
            w.timer = 0.0;
            let h = hash(self.tick, w.id as i64, 0, 0xF12E);
            let off = (crate::noise::unit(h) * 2.0 - 1.0) * fs.laser_m;
            let aa = a + off / r;
            let g = self.ground_at(aa, true);
            let (ux, uy) = (aa.cos(), aa.sin());
            for hgt in 0..12 {
                let c = cell_of((ux * (g + hgt as f64), uy * (g + hgt as f64)));
                let m = self.world.material(c.0, c.1);
                if self.units.is_flora.get(m as usize).copied().unwrap_or(false) {
                    self.sandbox.heat_cell(&self.world, c, 1100.0);
                    self.units.fires += 1;
                    w.laser = mid(c);
                    w.laser_at = self.tick;
                    break;
                }
            }
        }
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
            out.push(UnitView { kind: Unit::Sapper, id: s.id, state: s.state as u8, from: s.prev, to: s.pos, t0: t(s.moved_at), t1: t(s.moved_at) + sec, a: s.armed_s as f32, b: s.drilled as f32, c: 0.0, mark: s.goal });
        }
        for b in &self.units.bores {
            out.push(UnitView { kind: Unit::Tunneler, id: (b.resume_at as u32) ^ 0x7000_0000, state: b.active as u8, from: b.prev, to: (b.x, b.y), t0: t(b.moved_at), t1: t(b.moved_at) + sec, a: b.hx as f32, b: b.hy as f32, c: b.length as f32, mark: (b.x, b.y) });
        }
        for b in &self.units.bots {
            let tip = b.tips.first().map(|&c| mid(c)).unwrap_or(mid(b.spot));
            out.push(UnitView { kind: Unit::HeatSink, id: b.id, state: b.state as u8, from: b.prev, to: b.pos, t0: t(b.moved_at), t1: t(b.moved_at) + sec, a: b.reach as f32, b: (b.life / self.eco.units.heatsink.life_s.max(1.0)) as f32, c: b.tips.len() as f32, mark: tip });
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
                b: (w.bag / self.eco.units.forester.bag_kg.max(1.0)) as f32,
                c: if firing { 2.0 } else if cutting { 1.0 } else { 0.0 },
                mark: if firing { w.laser } else { w.claw },
            });
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
        v.push(format!("  blasts {} ({} set off by being destroyed; {} cells cracked) · frozen {:.0} t · plants taken {:.0} t, fires {}", u.blasts, u.destroyed, u.cracked, u.frozen_kg / 1000.0, u.harvested_kg / 1000.0, u.fires));
        if self.prod.bays_of(Unit::HeatSink) > 0 {
            v.push(format!("  open magma pools near the flocks: {}", u.molten.len()));
        }
        if self.prod.bays_of(Unit::Forester) > 0 {
            v.push(format!("  stands of plants found: {}", u.flora.len()));
        }
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
            o.u8(w.state as u8);
            for v in [w.pos.0, w.pos.1, w.goal_a, w.facing, w.life, w.timer, w.bag, w.taken_kg] {
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
            u.sappers.push(Sapper { id, state, pos: (f[0], f[1]), prev: (f[0], f[1]), moved_at: tick, goal: (f[2], f[3]), path, along: f[4], drilled: f[5], armed_s: f[6], want: f[7], ground_r: f[8] });
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
            let mut f = [0.0; 8];
            for v in f.iter_mut() {
                *v = r.f64()?;
            }
            let pos = (f[0], f[1]);
            u.walkers.push(Walker { id, kind, state, pos, prev: pos, moved_at: tick, goal_a: f[2], facing: f[3], life: f[4], timer: f[5], bag: f[6], taken_kg: f[7], laser: pos, laser_at: 0, claw: pos, cut_at: 0 });
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
