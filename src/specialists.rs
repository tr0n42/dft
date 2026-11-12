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
use crate::world::Look;

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
    /// Leaping over ground too steep to walk (a cliff, a mountain).
    Jump,
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
    /// How high its body rides over the ground (m): over the treetops.
    lift: f64,
    /// A leap under way: from (direction, ground radius) to (direction,
    /// ground radius), the arc's top (radius), seconds into it, and its length.
    jump: Option<Jump>,
    /// Where its laser's last sweep began (it ends at `laser`).
    laser_from: (f64, f64),
    /// What is in its bag: cells of each material.
    load: Vec<(MatId, f64)>,
}

/// A walker's leap over ground too steep to walk.
#[derive(Debug, Clone, Copy)]
pub struct Jump {
    a0: f64,
    g0: f64,
    a1: f64,
    g1: f64,
    top: f64,
    t: f64,
    dur: f64,
}

/// A full bag a walker dropped, waiting on the ground for couriers to carry
/// it up to the mother swarm. What it holds is only credited when it gets
/// there.
#[derive(Debug, Clone)]
pub struct Bag {
    id: u32,
    kind: Unit,
    pos: (f64, f64),
    kg: f64,
    load: Vec<(MatId, f64)>,
    /// The tick couriers lift it away.
    due: u64,
}

/// Seconds a bag is seen rising with its couriers after they lift it.
const BAG_RISE_S: f64 = 3.0;

/// Walkers leap ground steeper than this (rise over run, looked at this far
/// ahead), landing where it is gentler than this, at most this far off;
/// they clear the highest ground on the way by this much and cover this
/// many metres a second.
const LEAP_SLOPE: f64 = 1.0;
const LEAP_LOOK_M: f64 = 24.0;
const LEAP_MIN_RISE_M: f64 = 20.0;
const LEAP_LAND_SLOPE: f64 = 0.45;
const LEAP_MAX_M: f64 = 800.0;
const LEAP_CLEAR_M: f64 = 25.0;
const LEAP_SPEED: f64 = 35.0;

/// The most cells a claw shears off a plant at one go.
const SECTION_MAX: usize = 60;

/// Rubble cells moved per tick, at most.
const SETTLE_PER_TICK: usize = 20_000;

/// A walker towers over all plant life: its body rides this far over the
/// tallest plant round it (and never lower than that over bare ground); it
/// rises and settles this fast (m/s).
const LIFT_MIN_M: f64 = 50.0;
const LIFT_OVER_M: f64 = 50.0;
const LIFT_RATE: f64 = 5.0;
/// How far round it (m, each way) it looks for the tallest plant.
const CANOPY_ROUND_M: i64 = 30;
/// The tallest plants a walker looks over (m).
const CANOPY_MAX_M: i64 = 160;

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
    /// Leaps walkers made over ground too steep to walk.
    pub leaps: u64,
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
const SCAN_EVERY_S: f64 = 30.0;
/// The heart of a sapper's blast is heated to this (K): it melts, and cools
/// to glass after about this long (s).
const MELT_K: f64 = 2000.0;
const MELT_COOL_S: f64 = 25.0;
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

    /// The terrain's height in direction `a`: the first solid cell coming
    /// down that isn't a plant, dead plant matter or loose rubble.
    fn terrain_at(&self, a: f64) -> f64 {
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
    /// Spots within `keep_off` of `taken` (where other sappers are headed)
    /// are left to them.
    fn sapper_path(&self, from: (f64, f64), ground_r: f64, want: f64, limit_m: f64, taken: &[(f64, f64)], keep_off: f64) -> Vec<(i64, i64)> {
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
        let mut caught = Vec::new();
        for s in self.units.sappers.iter_mut() {
            if s.armed_s >= 0.0 && dist2(s.pos, at) < r * r {
                // Gone from the field; it goes off a moment later.
                s.armed_s = -1.0;
                self.units.destroyed += 1;
                caught.push((s.id, s.pos));
            }
        }
        self.units.sappers.retain(|s| s.armed_s >= 0.0);
        for (id, pos) in caught {
            let d = 0.12 + 0.1 * crate::noise::unit(hash(self.tick, id as i64, 0, 0xC4A1));
            let due = self.tick + self.ticks(d);
            self.chain.push((due, pos));
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
            for (_, at) in now {
                self.sapper_blast(at, y);
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
            self.settle();
        }
    }

    /// Loose rubble settles, as in the proving ground: down, or down a
    /// slope (both cells beside it open); melt runs sideways too, into the
    /// cracks. Bottom first. Cells that can't move stop being looked at
    /// until something beside them opens.
    fn settle(&mut self) {
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
    pub fn sapper_blast(&mut self, at: (f64, f64), yield_kt: f64) {
        let sp = &self.eco.units.sapper;
        let r = sp.radius_m_per_cbrt_kt * yield_kt.max(0.0).cbrt();
        self.booms.push(Boom { x: at.0, y: at.1, size_m: (2.2 * r) as f32, buried: true });
        self.explosions += 1;
        self.units.blasts += 1;
        self.happen(Happening::Explosion, at);
        let c0 = cell_of(at);
        let blast = hash(self.tick, c0.0, c0.1, 0x5A99);
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
                self.sandbox.heat_cell(&self.world, c, MELT_K);
                self.loose.insert(c);
                self.settling.insert(c);
                self.world.set_look(c.0, c.1, Look::MELT);
                let cool = self.ticks(MELT_COOL_S * (0.7 + 0.6 * crate::noise::unit(hash(blast, c.0, c.1, 21))));
                self.melt.push_back((self.tick + cool, c));
                continue;
            }
            if e > 0.35 + 0.35 * h {
                // Thrown clear, out from the blast.
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
            } else {
                // Left loose where it was: baked near the heart.
                self.loose.insert(c);
                self.settling.insert(c);
                self.world.set_look(c.0, c.1, if d < r * 0.6 { Look::BAKED } else { Look::RUBBLE });
                rubble.push(c);
            }
        }
        let _ = thrown;
        self.units.cracked += cracked + broken.len() as u64;
        // The plume: most blasts find a way up.
        if crate::noise::unit(hash(blast, 0, 0, 77)) < 0.9 && !rubble.is_empty() {
            if let Some(mouth) = self.way_up(c0, r) {
                let share = 0.35 + 0.25 * crate::noise::unit(hash(blast, 0, 0, 7));
                let take = ((rubble.len() as f64 * share).round() as usize).min(rubble.len());
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
        // Sappers caught in it go off too, a moment later.
        self.sappers_caught(at, r * 0.9);
    }

    /// An open way up from cell `from` to open sky over the ground (through
    /// open space and the cells in `loose`), within reach of a blast of
    /// radius `r`: the cell where it comes out, if there is one.
    fn way_up(&self, from: (i64, i64), r: f64) -> Option<(i64, i64)> {
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
        self.units.walkers.push(Walker { id, kind: u, state: WalkState::Drop, pos: start, prev: start, moved_at: self.tick, goal_a: a, facing: 1.0, life, timer: 0.0, bag: 0.0, taken_kg: 0.0, laser: start, laser_at: 0, claw: start, cut_at: 0, lift: LIFT_MIN_M, load: Vec::new(), jump: None, laser_from: start });
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
                    // It works as it goes: claws and laser on what it passes.
                    self.walker_stand(&mut w);
                    self.walker_hands(&mut w, &fs);
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
                    self.walker_feed(&mut w, &fs);
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

    /// Ground too steep to walk just ahead of a walker at direction `a`
    /// going `facing` (+1 counter-clockwise), `left_m` short of its goal: a
    /// leap over it to the first walkable ground beyond, if there is any.
    fn leap_over(&self, a: f64, facing: f64, left_m: f64) -> Option<Jump> {
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
    fn walker_stand(&self, w: &mut Walker) {
        let a = w.pos.1.atan2(w.pos.0);
        let g = self.ground_at(a, true);
        w.pos = (a.cos() * g, a.sin() * g);
        self.walker_rise(w);
    }

    /// Its body rides over the tallest plant round it: up as the forest rises,
    /// down as it is cleared.
    fn walker_rise(&self, w: &mut Walker) {
        let want = (self.canopy_at(w.pos) + LIFT_OVER_M).max(LIFT_MIN_M);
        w.lift += (want - w.lift).clamp(-LIFT_RATE, LIFT_RATE);
    }

    /// Leave a walker's bag on the ground behind it for the couriers, and
    /// start a new one.
    fn drop_bag(&mut self, w: &mut Walker) {
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
    fn bags_work(&mut self) {
        let tick = self.tick;
        // Credited as the couriers lift it; seen rising for a few seconds
        // more, then gone.
        let gone = self.ticks(BAG_RISE_S);
        let mut arrived = Vec::new();
        for b in self.units.bags.iter_mut() {
            if b.due <= tick && !b.load.is_empty() {
                arrived.append(&mut b.load);
            }
        }
        self.units.bags.retain(|b| tick < b.due + gone);
        for (m, cells) in arrived {
            self.credit(m, cells);
        }
    }

    /// Credit `cells` cells of material `m` arriving at the mother swarm.
    fn credit(&mut self, m: MatId, cells: f64) {
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
    fn canopy_at(&self, pos: (f64, f64)) -> f64 {
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
    fn walker_feed(&mut self, w: &mut Walker, fs: &crate::economy::ForesterSpec) {
        let a = w.pos.1.atan2(w.pos.0);
        let r = len(w.pos).max(1.0);
        let cut = self.walker_hands(w, fs);
        self.walker_creep(w, fs, a, r, cut);
    }

    /// A forester's claws and laser, a second's worth, wherever it is: the
    /// cells it sheared.
    fn walker_hands(&mut self, w: &mut Walker, fs: &crate::economy::ForesterSpec) -> usize {
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
    fn walker_creep(&mut self, w: &mut Walker, fs: &crate::economy::ForesterSpec, a: f64, r: f64, cut: usize) {
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
                b: (w.bag / self.eco.units.forester.bag_kg.max(1.0)) as f32,
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
