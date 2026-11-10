//! The simulation, on a thread of its own. It runs the swarm at the speed the
//! player asked for, in short batches so the renderer can read the world in
//! between, and after each batch publishes what the screen needs: where the
//! drones are flying, which blocks changed, and the numbers for the dev panel.
use dft_sim::swarm::{Leg, LegChanges, Sim};
use parking_lot::{Mutex, MutexGuard};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Game seconds per real second for each speed key; the last one is "as fast
/// as the machine can go".
pub const SPEEDS: [f64; 8] = [1.0, 2.0, 5.0, 10.0, 30.0, 100.0, 300.0, f64::INFINITY];

/// How long the simulation holds the world in one go.
const BATCH: Duration = Duration::from_millis(4);
/// How often it publishes for the screen.
const PUBLISH: Duration = Duration::from_millis(15);

#[derive(Clone, Copy, Debug)]
pub struct Control {
    /// Index into SPEEDS.
    pub speed: usize,
    pub paused: bool,
    /// Run until this game time, then pause (screenshots and tests).
    pub pause_at: Option<f64>,
    /// Economy off: the swarm grows to this many drones for free (see
    /// `Sim::free_drones`). None plays the real economy.
    pub free_drones: Option<u64>,
    /// Where loose matter moves (see `Sandbox::set_attention`): None
    /// everywhere; the game sets it to what's on screen.
    pub attention: Option<Option<[i64; 4]>>,
    /// Ignore the screen and move everything everywhere (dev panel).
    pub full_detail: bool,
    pub quit: bool,
}

/// The numbers the dev panel shows.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub time: f64,
    pub drones: usize,
    pub packet: u64,
    pub consumed: f64,
    pub energy: f64,
    pub mass: f64,
    pub awake: usize,
    pub loose: usize,
    pub deaths: u64,
    pub hubs: usize,
    pub launched_at: Option<f64>,
    /// Game seconds run per real second over the last second.
    pub speed: f64,
    /// Real milliseconds of computing per game second over the last second.
    pub cost_ms: f64,
    /// Where that computing went: drone behaviour, falling sand and heat,
    /// loose rock bodies, and the once-a-second work (shares, summing to 1).
    pub split: [f64; 4],
    /// Running totals from the simulation's own profiling (see
    /// dft_sim::swarm::PROF_NAMES and COUNT_NAMES), for the log.
    pub prof: [f64; 8],
    pub counts: [u64; 6],
    /// Cells the swarm could dig right now.
    pub frontier: usize,
    /// Loose cells waiting, out of sight, to move.
    pub dormant: usize,
    /// Logic steps run so far.
    pub tick: u64,
    /// Haulers, and how many are away (lifting, unloading or coming back).
    pub haulers: usize,
    pub haulers_away: usize,
    /// Cutters lost when their energy ran out; cutters waiting for a hauler.
    pub starved: u64,
    pub waiting: usize,
    /// Flocks working, and what they've done: splits, merges, cutters sent
    /// to help; explosions; how far round the mother swarm reaches (0 to 1).
    pub flocks: usize,
    pub splits: u64,
    pub merges: u64,
    pub helpers: u64,
    pub explosions: u64,
    pub mother_share: f64,
    /// Cells burned away so far, and cutters lost to heat.
    pub burned_cells: u64,
    pub melted: u64,
    /// Times a flock moved its hauler away from lava.
    pub site_moves: u64,
    /// Cells of liquid blasted out of vents, and drops of it in the air.
    pub erupted: u64,
    pub drops: usize,
    /// The swarm director's last few calls, and haulers it has built.
    pub director: Vec<String>,
    /// What production is doing: the fabricator, the factory's bays and
    /// the orders waiting (counted once a second).
    pub production: Vec<String>,
    pub haulers_built: u64,
    /// Drones out of work, resting in the mother swarm (counted once a second).
    pub resting: usize,
    /// The element stockpile (kg, by place in the element table), the parts
    /// stock, cutters built for work waiting, and room for more.
    pub stock: Vec<f64>,
    /// The part of `stock` already refined to pure element.
    pub refined: Vec<f64>,
    /// Raw stockpile refined each second.
    pub refine_rate: f64,
    /// All the swarm's matter (see Sim::matter): the round's score.
    pub matter: [f64; 6],
    /// What has been brought up lately: (colour, kg), most first.
    pub recent_mix: Vec<([u8; 3], f64)>,
    /// Every material collected, ever (kg, by material id).
    pub collected: Vec<f64>,
    /// kg taken out of the stockpile to build with, all told.
    pub stock_spent: f64,
    pub parts: Vec<PartRow>,
    pub built_for_room: u64,
    pub room: u64,
}

/// One kind of part, for the stockpile window's Parts tab.
#[derive(Clone, Debug, Default)]
pub struct PartRow {
    pub name: String,
    pub lore: String,
    /// Kilograms in one.
    pub kg: f64,
    /// On hand, and made all told.
    pub have: u64,
    pub made: u64,
    /// How many a cutter and a hauler take.
    pub per_cutter: u64,
    pub per_hauler: u64,
    /// Each component, and the element most of it is made of now ("-":
    /// it can't be made).
    pub from: Vec<(String, String)>,
}

/// How the simulation thread itself is doing, summed since the log last took it.
#[derive(Clone, Copy, Debug, Default)]
pub struct ThreadPerf {
    /// Batches run, and logic steps in them.
    pub batches: u64,
    pub steps: u64,
    /// Seconds waiting to take the world (the tile builders had it).
    pub wait_s: f64,
    /// Seconds holding it.
    pub hold_s: f64,
    /// Publishing for the screen: how many, seconds in all, the longest.
    pub publishes: u64,
    pub publish_s: f64,
    pub publish_max_s: f64,
    /// Seconds asleep (caught up with the speed asked for).
    pub sleep_s: f64,
}

/// What the simulation has published.
#[derive(Default)]
pub struct Out {
    /// Drone flight legs that changed since the screen last took them: a
    /// whole new set, then changed slots (applied in order).
    pub leg_full: Option<Vec<Leg>>,
    pub leg_changes: Vec<(u32, Leg)>,
    /// Landing points and other hubs: x, y, kind (0 landing, 1 refiner, 2
    /// shaft), and how many drones call it home.
    pub hubs: Vec<(f64, f64, u8, u32)>,
    /// The haulers, each on its flight (or hovering over its flock).
    pub haulers: Vec<dft_sim::swarm::HaulerView>,
    /// The mother swarm: its middle and reach either side (radians), and
    /// its distance from the body's centre (metres).
    pub mother: (f64, f64, f64),
    /// Explosions not yet shown.
    pub booms: Vec<dft_sim::swarm::Boom>,
    /// The ground's height round the body (see Sim::ground_heights), and
    /// which sweep it is from.
    pub heights: Option<(u64, Arc<Vec<f32>>)>,
    /// The longest simulation step in the last second (milliseconds), and
    /// where its time went (see dft_sim::swarm::PROF_NAMES, first four).
    pub step_max: (f64, [f64; 4]),
    /// Happenings the camera director hasn't seen yet.
    pub happenings: Vec<(dft_sim::swarm::Happening, f64, f64)>,
    /// Cells of rock in the air (chunks cut free), and their materials.
    pub chunks: Vec<dft_sim::bodies::AirCell>,
    /// The flocks' flow fields (drones on screen fly along them), with the
    /// simulation's count of rebuilds when they were taken.
    pub fields: Vec<Arc<dft_sim::flow::FlowField>>,
    pub fields_version: u64,
    pub stats: Stats,
    /// When it was published (the screen moves drones on from here).
    pub at: Option<Instant>,
    /// Changed places since the screen last took them.
    pub blocks: HashSet<(i64, i64)>,
    /// The 8 x 8-cell squares whose cells changed in the screen's copy of the
    /// ground since last taken (what zoomed-in tiles redraw for).
    pub fine: HashSet<(i64, i64)>,
    pub rects: Vec<[i64; 4]>,
    pub perf: ThreadPerf,
}

/// The screen's own copy of the ground as dug: every leaf (64 x 64 cells)
/// that isn't as generated, kept up to date by the simulation thread a few
/// hundred leaves a batch, nearest the view first. Tiles are drawn from
/// this and the generator, never from the running world, so drawing never
/// waits on the simulation however busy it is.
pub struct Mirror {
    origin: (i64, i64),
    leaves: std::collections::HashMap<(i64, i64), dft_sim::world::LeafSnap>,
}

impl Mirror {
    /// What cell (x, y) holds, if it isn't as generated.
    pub fn get(&self, x: i64, y: i64) -> Option<dft_sim::materials::MatId> {
        const L: i64 = dft_sim::totals::LEAF;
        let (lx, ly) = ((x - self.origin.0).div_euclid(L), (y - self.origin.1).div_euclid(L));
        let snap = self.leaves.get(&(lx, ly))?;
        Some(snap.at(x - self.origin.0 - lx * L, y - self.origin.1 - ly * L))
    }
}

/// Leaves copied into the mirror per batch, at most.
const MIRROR_PER_BATCH: usize = 384;

pub struct Shared {
    sim: Mutex<Sim>,
    pub mirror: parking_lot::RwLock<Mirror>,
    /// Readers (tile builders, saves) waiting for the world: between batches
    /// the simulation lets them all through before it carries on, so a busy
    /// simulation doesn't make them queue one at a time behind its steps.
    readers: AtomicUsize,
    pub out: Mutex<Out>,
    pub control: Mutex<Control>,
    /// Blasts the screen sets off in the world (a storage drone crashing):
    /// where, how big across (m), and how many cells of wreckage.
    pub blasts: Mutex<Vec<((f64, f64), f32, u32)>>,
}

impl Shared {
    /// The running world if it is free this instant (for things that can
    /// wait for the next frame rather than hold up drawing).
    pub fn try_world(&self) -> Option<MutexGuard<'_, Sim>> {
        self.sim.try_lock()
    }

    /// The running world, for reading. Hold it briefly: the simulation waits.
    pub fn world(&self) -> MutexGuard<'_, Sim> {
        self.readers.fetch_add(1, Ordering::SeqCst);
        let g = self.sim.lock();
        self.readers.fetch_sub(1, Ordering::SeqCst);
        g
    }
}

/// Start the simulation thread.
pub fn start(mut sim: Sim, control: Control) -> Arc<Shared> {
    sim.world.track = Some(Default::default());
    sim.leg_track = Some(Default::default());
    let mirror = Mirror { origin: sim.world.leaf_corner(0, 0), leaves: Default::default() };
    let shared = Arc::new(Shared { sim: Mutex::new(sim), mirror: parking_lot::RwLock::new(mirror), readers: AtomicUsize::new(0), out: Mutex::new(Out::default()), control: Mutex::new(control), blasts: Mutex::new(Vec::new()) });
    let s = shared.clone();
    std::thread::Builder::new().name("simulation".into()).spawn(move || run(&s)).expect("start the simulation thread");
    shared
}

/// Ask the thread to stop (it finishes its batch and ends).
pub fn stop(shared: &Shared) {
    shared.control.lock().quit = true;
}

fn run(shared: &Shared) {
    let mut target = shared.sim.lock().time();
    let mut last = Instant::now();
    let mut last_publish = Instant::now() - PUBLISH;
    // For the speed and cost readings: (wall start, game start, busy seconds).
    let mut window = (Instant::now(), target, 0.0f64);
    let mut prof0 = shared.sim.lock().prof;
    let mut stats = Stats::default();
    let mut perf = ThreadPerf::default();
    let mut heights_seen = 0u64;
    let mut fields_seen = u64::MAX;
    let mut rest_counted = Instant::now() - Duration::from_secs(5);
    // Leaves the screen's copy (see `Mirror`) doesn't have up to date yet:
    // all that aren't as generated to start with (after loading a save,
    // that is all of the dug ground).
    let mut pending: HashSet<(i64, i64)> = shared.sim.lock().world.stored_leaves().into_iter().collect();
    // The longest step this second, and the one shown.
    let mut step_max = (0.0f64, [0.0f64; 4]);
    let mut step_max_shown = (0.0f64, [0.0f64; 4]);
    loop {
        let c = *shared.control.lock();
        if c.quit {
            return;
        }
        let now = Instant::now();
        let dt = (now - last).as_secs_f64();
        last = now;
        let speed = SPEEDS[c.speed.min(SPEEDS.len() - 1)];
        let running = !c.paused;
        let mut caught_up = true;
        {
            let tw = Instant::now();
            let mut s = shared.sim.lock();
            let held = Instant::now();
            perf.wait_s += (held - tw).as_secs_f64();
            if s.free_drones.is_some() && c.free_drones.is_none() {
                // Back to the real economy: drop the free energy.
                s.energy = s.eco.seed_swarm.start_energy;
            }
            s.free_drones = c.free_drones;
            s.set_attention(if c.full_detail { None } else { c.attention });
            for (at, size, scrap) in std::mem::take(&mut *shared.blasts.lock()) {
                s.big_boom(at, size, scrap);
            }
            let t_now = s.time();
            if running {
                // Never let a backlog build up: if the machine can't keep up,
                // the game simply runs slower.
                target = if speed.is_finite() { (target + dt * speed).min(t_now + 0.25 * speed) } else { f64::INFINITY };
            } else {
                target = t_now;
            }
            if let Some(p) = c.pause_at {
                target = target.min(p);
            }
            let start = Instant::now();
            perf.batches += 1;
            while s.time() < target && s.launched_at.is_none() && start.elapsed() < BATCH {
                let t0 = Instant::now();
                let p0 = s.prof;
                s.step();
                let ms = t0.elapsed().as_secs_f64() * 1000.0;
                if ms > step_max.0 {
                    step_max = (ms, [0, 1, 2, 3].map(|i| (s.prof[i] - p0[i]) * 1000.0));
                }
                perf.steps += 1;
            }
            caught_up = caught_up && (s.time() >= target || s.launched_at.is_some());
            window.2 += start.elapsed().as_secs_f64();
            if c.pause_at.is_some_and(|p| s.time() >= p) {
                let mut ctl = shared.control.lock();
                ctl.paused = true;
                ctl.pause_at = None;
            }
            // Readings, once a second.
            let w = window.0.elapsed().as_secs_f64();
            if w >= 1.0 {
                let game = s.time() - window.1;
                stats.speed = game / w;
                if game > 0.0 {
                    stats.cost_ms = window.2 * 1000.0 / game;
                    let d: Vec<f64> = (0..4).map(|i| s.prof[i] - prof0[i]).collect();
                    let total: f64 = d.iter().sum::<f64>().max(1e-12);
                    stats.split = [d[0] / total, d[1] / total, d[2] / total, d[3] / total];
                }
                prof0 = s.prof;
                window = (Instant::now(), s.time(), 0.0);
                step_max_shown = step_max;
                step_max = (0.0, [0.0; 4]);
            }
            if last_publish.elapsed() >= PUBLISH {
                last_publish = Instant::now();
                let tp = Instant::now();
                let changes = s.take_leg_changes();
                use dft_sim::swarm::HubKind;
                let hubs = s.hubs.iter().zip(s.hub_loads()).map(|(h, &n)| (h.x, h.y, match h.kind { HubKind::Seed => 0, HubKind::Refiner => 1, HubKind::Shaft => 2 }, n)).collect();
                stats.time = s.time();
                stats.drones = s.drone_count();
                stats.packet = s.packet;
                stats.consumed = s.consumed_share();
                stats.energy = s.energy;
                stats.mass = s.mass;
                stats.awake = s.sandbox.awake();
                stats.loose = s.bodies.count();
                stats.deaths = s.deaths;
                stats.hubs = s.hubs.len();
                stats.launched_at = s.launched_at;
                stats.prof = s.prof;
                stats.counts = s.counts;
                stats.frontier = s.frontier_len();
                stats.dormant = s.sandbox.dormant();
                stats.tick = s.tick;
                let haulers = s.hauler_views();
                stats.haulers = haulers.len();
                stats.haulers_away = haulers.iter().filter(|h| h.state != dft_sim::swarm::HaulerState::Working).count();
                stats.starved = s.starved;
                stats.waiting = s.waiting_count();
                stats.flocks = s.flock_count();
                stats.splits = s.splits;
                stats.merges = s.merges;
                stats.helpers = s.helpers_sent;
                stats.explosions = s.explosions;
                stats.mother_share = s.mother.half / std::f64::consts::PI;
                stats.burned_cells = s.sandbox.stats.burned;
                stats.melted = s.burned;
                stats.site_moves = s.site_moves;
                stats.erupted = s.sandbox.stats.erupted;
                stats.drops = s.bodies.drops();
                stats.haulers_built = s.haulers_built;
                if rest_counted.elapsed() >= Duration::from_secs(1) {
                    rest_counted = Instant::now();
                    stats.resting = s.resting_count();
                    let els = &s.world.planet.mats.elements;
                    stats.stock = s.stock.kg.clone();
                    stats.refined = s.stock.refined.clone();
                    stats.refine_rate = s.eco.refiner.swarm_kg_per_s;
                    stats.matter = s.matter();
                    stats.stock_spent = s.stock_spent;
                    stats.collected = s.collected_mats.clone();
                    stats.recent_mix = s.recent_mix(12).into_iter().map(|(m, kg)| (s.world.planet.mats.get(m).colour, kg)).collect();
                    let r = &s.eco.recipe;
                    let (per_c, per_h) = (r.counts(&r.harvester), r.counts(&r.hauler));
                    stats.parts = r
                        .part
                        .iter()
                        .enumerate()
                        .map(|(i, p)| PartRow {
                            name: p.name.clone(),
                            lore: p.lore.clone(),
                            kg: p.kg(),
                            have: s.parts.n.get(i).copied().unwrap_or(0),
                            made: s.parts.made.get(i).copied().unwrap_or(0),
                            per_cutter: per_c[i],
                            per_hauler: per_h[i],
                            from: s.stock.sources(els, p).into_iter().map(|(c, e)| (c, e.unwrap_or_else(|| "-".into()))).collect(),
                        })
                        .collect();
                    stats.built_for_room = s.cutters_built_for_room;
                    stats.production = s.production_lines();
                    stats.room = s.last_room;
                }
                if stats.director.len() != s.director_log.len() || stats.director.last() != s.director_log.back() {
                    stats.director = s.director_log.iter().cloned().collect();
                }
                let chunks = if s.bodies.count() > 0 { s.bodies.drawn() } else { Vec::new() };
                let mother = (s.mother.centre, s.mother.half, s.mother.radius);
                let fields = if s.fields_version != fields_seen {
                    fields_seen = s.fields_version;
                    Some(s.fields.iter().flatten().cloned().collect::<Vec<_>>())
                } else {
                    None
                };
                let heights = if s.peaks_version != heights_seen {
                    heights_seen = s.peaks_version;
                    Some((s.peaks_version, Arc::new(s.ground_heights().iter().map(|&h| h as f32).collect::<Vec<f32>>())))
                } else {
                    None
                };
                let mut booms = s.take_booms();
                let mut happenings = s.take_happenings();
                // The screen's copy of the ground: the leaves changed since,
                // then a few hundred of those waiting, nearest the view first.
                let mut fine: Vec<(i64, i64)> = Vec::new();
                let mirrored = {
                    const L: i64 = dft_sim::totals::LEAF;
                    let o = s.world.leaf_corner(0, 0);
                    let leaf = |x: i64, y: i64| ((x - o.0).div_euclid(L), (y - o.1).div_euclid(L));
                    let tr = s.world.track.as_ref().unwrap();
                    for &(bx, by) in &tr.blocks {
                        let (x0, y0) = (bx << 6, by << 6);
                        for (dx, dy) in [(0, 0), (63, 0), (0, 63), (63, 63)] {
                            pending.insert(leaf(x0 + dx, y0 + dy));
                        }
                    }
                    for r in &tr.rects {
                        let (a, b) = (leaf(r[0], r[2]), leaf(r[1] - 1, r[3] - 1));
                        for ly in a.1..=b.1 {
                            for lx in a.0..=b.0 {
                                pending.insert((lx, ly));
                            }
                        }
                    }
                    let mut chosen: Vec<(i64, i64)> = pending.iter().copied().collect();
                    if chosen.len() > MIRROR_PER_BATCH {
                        let centre = match c.attention {
                            Some(Some([x0, x1, y0, y1])) => leaf((x0 + x1) / 2, (y0 + y1) / 2),
                            _ => (0, 0),
                        };
                        let d = |k: &(i64, i64)| ((k.0 - centre.0).pow(2) + (k.1 - centre.1).pow(2), *k);
                        chosen.select_nth_unstable_by_key(MIRROR_PER_BATCH, d);
                        chosen.truncate(MIRROR_PER_BATCH);
                    }
                    let snaps: Vec<((i64, i64), Option<dft_sim::world::LeafSnap>)> = chosen.iter().map(|&(lx, ly)| ((lx, ly), s.world.leaf_snapshot(lx, ly))).collect();
                    let mut blocks = Vec::with_capacity(snaps.len() * 4);
                    {
                        let mut m = shared.mirror.write();
                        for (k, snap) in snaps {
                            pending.remove(&k);
                            // Which 8 x 8 squares of it changed (all of it,
                            // where it was as generated before or is now).
                            let (x0, y0) = (o.0 + k.0 * L, o.1 + k.1 * L);
                            const SQ: i64 = 8;
                            match (m.leaves.get(&k), &snap) {
                                (Some(a), Some(b)) => {
                                    for sy in 0..L / SQ {
                                        for sx in 0..L / SQ {
                                            let differs = match (a, b) {
                                                (dft_sim::world::LeafSnap::Uniform(p), dft_sim::world::LeafSnap::Uniform(q)) => p != q,
                                                _ => (0..SQ).any(|j| (0..SQ).any(|i| a.at(sx * SQ + i, sy * SQ + j) != b.at(sx * SQ + i, sy * SQ + j))),
                                            };
                                            if differs {
                                                fine.push(((x0 >> 3) + sx, (y0 >> 3) + sy));
                                            }
                                        }
                                    }
                                }
                                (None, None) => {}
                                _ => {
                                    for sy in 0..L / SQ {
                                        for sx in 0..L / SQ {
                                            fine.push(((x0 >> 3) + sx, (y0 >> 3) + sy));
                                        }
                                    }
                                }
                            }
                            match snap {
                                Some(v) => {
                                    m.leaves.insert(k, v);
                                }
                                None => {
                                    m.leaves.remove(&k);
                                }
                            }
                            let (x0, y0) = (o.0 + k.0 * L, o.1 + k.1 * L);
                            for (dx, dy) in [(0, 0), (L - 1, 0), (0, L - 1), (L - 1, L - 1)] {
                                blocks.push(((x0 + dx) >> 6, (y0 + dy) >> 6));
                            }
                        }
                    }
                    blocks
                };
                let track = s.world.track.as_mut().unwrap();
                let mut out = shared.out.lock();
                match changes {
                    LegChanges::Full(v) => {
                        out.leg_full = Some(v);
                        out.leg_changes.clear();
                    }
                    LegChanges::Some(mut v) => out.leg_changes.append(&mut v),
                }
                out.hubs = hubs;
                out.haulers = haulers;
                out.mother = mother;
                if heights.is_some() {
                    out.heights = heights;
                }
                out.step_max = step_max_shown;
                if out.happenings.len() < 2000 {
                    out.happenings.append(&mut happenings);
                }
                if out.booms.len() < 2000 {
                    out.booms.append(&mut booms);
                }
                out.chunks = chunks;
                if let Some(f) = fields {
                    out.fields = f;
                    out.fields_version = fields_seen;
                }
                out.stats = stats.clone();
                out.at = Some(Instant::now());
                out.blocks.extend(track.blocks.drain());
                // (Tiles over leaves just copied are redrawn from the copy.)
                out.blocks.extend(mirrored);
                out.fine.extend(fine);
                out.rects.append(&mut track.rects);
                let pt = tp.elapsed().as_secs_f64();
                perf.publishes += 1;
                perf.publish_s += pt;
                perf.publish_max_s = perf.publish_max_s.max(pt);
                // Hand the thread's own numbers over, to be summed by the log.
                let o = &mut out.perf;
                o.batches += perf.batches;
                o.steps += perf.steps;
                o.wait_s += perf.wait_s;
                o.hold_s += perf.hold_s + held.elapsed().as_secs_f64();
                o.publishes += perf.publishes;
                o.publish_s += perf.publish_s;
                o.publish_max_s = o.publish_max_s.max(perf.publish_max_s);
                o.sleep_s += perf.sleep_s;
                perf = ThreadPerf::default();
            } else {
                perf.hold_s += held.elapsed().as_secs_f64();
            }
            // Hand the world straight to anyone waiting to read it.
            MutexGuard::unlock_fair(s);
            // Let everyone else waiting read before the next batch (at most
            // a few milliseconds).
            let w0 = Instant::now();
            while shared.readers.load(Ordering::SeqCst) > 0 && w0.elapsed() < Duration::from_millis(6) {
                std::thread::yield_now();
            }
        }
        if caught_up {
            let z = Instant::now();
            std::thread::sleep(Duration::from_millis(2));
            perf.sleep_s += z.elapsed().as_secs_f64();
        }
    }
}
