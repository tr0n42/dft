//! The sandbox: falling-sand cell physics, heat and reactions.
//!
//! Only cells that might do something are simulated. A cell wakes when
//! something next to it changes (a drone digs beside it, a neighbour moves or
//! reacts, or a neighbour's heat is far from its own) and goes back to sleep
//! once a tick passes in which it did nothing.
//!
//! Each tick, in a fixed order (nearest the centre first):
//! 1. heat: tracked only for cells that can change phase or flow; ordinary rock
//!    is a heat bath at the body's own temperature, and open space pulls toward
//!    `physics.space_k`;
//! 2. phase change at the material's melting, freezing, boiling or condensing point;
//! 3. reactions with touching cells, from the reaction table;
//! 4. motion: powders fall toward the centre, liquids fall and spread, gases
//!    rise, and gas that rises off the body is lost to space.
//!
//! Motion only happens where someone could see it. With an attention area
//! set (the game sets it to what's on screen, when cells there are big
//! enough to see), a cell outside it that would fall, flow or rise instead
//! waits, dormant, and moves once the area comes over it. Heat, phase changes
//! and reactions carry on everywhere. With no attention area (tests, tools)
//! everything moves everywhere.
//!
//! Mass never changes: cells move by swapping, and every phase change and
//! reaction keeps the cells' total mass (checked when the tables load).
//! Every random choice comes from a hash of (tick, cell), so a run is the same
//! on every machine and thread count.

use crate::materials::{MatId, State, VACUUM};
use crate::noise::{hash, unit};
use crate::swarm::{CellMap, CellQueue};
use crate::world::World;

pub type Cell = (i64, i64);

/// Share of the gap to its flame temperature a fire closes in a burnable
/// neighbour each tick (so fire creeps through what burns).
const FLAME_LICK: f64 = 0.02;

/// Within this depth of the surface, nothing above weighs enough to hold a
/// melting or boiling point up.
const HOLD_DEPTH_M: f64 = 100.0;

/// One cell's contents changing during a sandbox tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Change {
    pub cell: Cell,
    pub from: MatId,
    pub to: MatId,
}

/// The eight directions, in order around the circle.
const DIRS: [Cell; 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];
const N4: [Cell; 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
/// Cells that are only changing temperature are stepped this often.
const HEAT_EVERY: u64 = 10;
/// Cells cooling or warming only slowly, far from any change of state (a
/// lava lake's crust thickening, rock around a magma body), step this often.
const SLOW_EVERY: u64 = 100;
/// Below this change per heat step (K), and at least SLOW_MARGIN_K from any
/// melting, freezing or boiling point, a cell moves to the slow cadence.
const SLOW_DT_K: f64 = 2.0;
const SLOW_MARGIN_K: f64 = 30.0;
/// Largest share of a temperature gap one exchange may close in a slow step
/// (keeps the long steps stable).
const SLOW_MAX_SHARE: f64 = 0.2;
/// Out of sight (with an attention area set), cells only changing
/// temperature step this often, in long steps (no one sees how a lava flow
/// cools off screen; it only has to come out about right).
const FAR_EVERY: u64 = 100;

/// Below this many cells in a tick, heat is worked out on one thread (not
/// worth handing out).
const PAR_MIN: usize = 2048;

/// A cell's heat for one tick, worked out ahead (see `Sandbox::heat_of`).
#[derive(Clone, Copy, Default)]
struct HeatOut {
    tracked: bool,
    t: f64,
    dt: f64,
    warming: bool,
    wake: [Option<Cell>; 4],
    sun_new: [Option<(Cell, f32)>; 8],
}

/// Running totals, for reports and tests.
#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub moves: u64,
    pub phase_changes: u64,
    pub reactions: u64,
    pub vented: u64,
    /// Cells burned away, and cells burning (counted each step they burn).
    pub burned: u64,
    pub burning: u64,
    pub cells_stepped: u64,
    /// Moves by kind: straight down, down-diagonal, sideways (liquids), and
    /// by powders.
    pub moves_down: u64,
    pub moves_diag: u64,
    pub moves_side: u64,
    pub moves_powder: u64,
    /// Cells of liquid blasted out of vents.
    pub erupted: u64,
}

pub struct Sandbox {
    /// Cells to step next tick.
    next: CellQueue,
    /// Cells only cooling or warming: stepped every HEAT_EVERY ticks, in
    /// bigger steps, since heat moves slowly.
    warm: CellQueue,
    /// Cells only changing temperature slowly: stepped every SLOW_EVERY ticks.
    slow: CellQueue,
    /// Cells only changing temperature, out of sight: every FAR_EVERY ticks.
    far: CellQueue,
    /// Temperatures of cells that are off the body's own temperature (kelvin).
    pub temps: CellMap<f64>,
    /// Heat banked toward a phase change (positive: melting or boiling under
    /// way; negative: freezing or condensing), in kelvin.
    pub latent: CellMap<f64>,
    pub tick: u64,
    /// Game time in seconds (the owner keeps it current; it sets the Sun).
    pub time_s: f64,
    /// The direction to the Sun this tick, and which open cells see it
    /// (refreshed every SUN_STEP_S seconds).
    sun: Option<(f64, f64)>,
    sun_cache: CellMap<f32>,
    /// The body's own heat at cells looked up so far, and whether each is
    /// near the surface (no weight above to hold melting points up).
    ambient: std::sync::Mutex<CellMap<f64>>,
    shallow: std::sync::Mutex<CellMap<bool>>,
    /// Materials the Sun can melt or boil (only these are warmed by it).
    sun_sensitive: Vec<bool>,
    sun_step: i64,
    seed: u64,
    /// For each material: the reactions it takes part in, and whether as input `a`.
    by_mat: Vec<Vec<(usize, bool)>>,
    pub stats: Stats,
    /// Where motion is simulated: cell rectangle (x0, x1, y0, y1), ends
    /// exclusive; Some(None) nowhere; None everywhere.
    attention: Attention,
    /// Cells that would move but lie outside the attention area.
    dormant: Waiting,
    /// Holes into a pressurised layer that blast it out, by 16 m square.
    vents: CellMap<Vent>,
    /// Liquid blasted out this tick, for the owner to throw (see `Eject`).
    pub ejecta: Vec<Eject>,
}

/// One cell of liquid blasted out of a vent: what, from where, how fast,
/// how hot. The cell has already left the ground (booked as in the air).
#[derive(Debug, Clone, Copy)]
pub struct Eject {
    pub m: MatId,
    pub at: (f64, f64),
    pub vel: (f64, f64),
    pub k: f64,
}

/// A hole into a buried layer of liquid whose weight would lift it past the
/// rim. `excess0`: how far past the rim (m) when it opened; the speed it
/// blasts out at comes from that head, falling as it drains: after
/// VENT_CELLS_PER_M cells per metre of it, it is spent.
#[derive(Debug, Clone, Copy)]
struct Vent {
    excess0: f64,
    drained: u32,
    /// How far along the roof the search for liquid to draw has got.
    cursor: u32,
    tick: u64,
    this_tick: u32,
}

/// Side of the squares vents are kept by (m).
const VENT_M: i64 = 16;
/// Cells a vent gives per metre of head past the rim before it is spent
/// (and at least VENT_MIN_CELLS).
const VENT_CELLS_PER_M: f64 = 40.0;
const VENT_MIN_CELLS: f64 = 200.0;
/// Most cells one vent throws in a tick.
const VENT_MAX_PER_TICK: u32 = 12;
/// Gas coming out of the liquid as it rises speeds the blast past what the
/// head alone gives; and how widely the jet spreads (radians).
const VENT_BOOST: f64 = 1.5;
const VENT_SPREAD: f64 = 0.6;
/// Liquids at least this hot (molten rock, metal) are gassy: they are pushed
/// VENT_GAS times the depth of their layer further up than their weight alone
/// would push them.
const VENT_GAS_K: f64 = 900.0;
const VENT_GAS: f64 = 0.5;
/// How far along the roof either side of a vent its liquid is drawn from,
/// starting this far off to the side (not from right under the hole, which
/// would only let the liquid there sink away from it).
const VENT_REACH_M: u32 = 300;
const VENT_SIDE_M: u32 = 3;
/// How far sideways a vent's way up may step at each metre (see `open_to_sky`).
const VENT_WANDER: i64 = 3;

/// Whether open cell n has open space above it all the way out past the
/// surface: the mouth of the hole (just above the rim) if so. The hole may
/// wander a little (drones don't dig straight shafts): where the way straight
/// up is blocked, it may step up to VENT_WANDER cells to either side.
fn open_to_sky(world: &World, n: Cell) -> Option<(f64, f64)> {
    let (fx, fy) = (n.0 as f64 + 0.5, n.1 as f64 + 0.5);
    let nr = (fx * fx + fy * fy).sqrt().max(1e-9);
    let (ux, uy) = (fx / nr, fy / nr);
    let (tx, ty) = (-uy, ux);
    let rim = world.planet.surface_radius(ux, uy);
    let mut side = 0.0f64;
    let mut d = 0.0;
    while nr + d <= rim + 2.0 {
        d += 1.0;
        if d > 4096.0 {
            return None;
        }
        let at = |s: f64| (fx + ux * d + tx * s, fy + uy * d + ty * s);
        let open = |p: (f64, f64)| world.material(p.0.floor() as i64, p.1.floor() as i64) == VACUUM;
        if open(at(side)) {
            continue;
        }
        let mut found = false;
        for k in 1..=VENT_WANDER {
            for s in [side + k as f64, side - k as f64] {
                if open(at(s)) {
                    side = s;
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
        if !found {
            return None;
        }
    }
    Some((fx + ux * d + tx * side, fy + uy * d + ty * side))
}

/// The next cell of liquid `m` to draw for a vent above cell c: just under
/// the layer's roof (radius `top`), nearest the hole first, working outward
/// either side (`cursor` remembers how far it has got). Some(None): none
/// found yet this time (it carries on next tick); None: none left within
/// VENT_REACH_M, the vent is spent.
fn roof_source(world: &World, touched: &CellQueue, c: Cell, top: f64, m: MatId, cursor: &mut u32) -> Option<Option<Cell>> {
    let (cx, cy) = (c.0 as f64 + 0.5, c.1 as f64 + 0.5);
    let rc = (cx * cx + cy * cy).sqrt().max(1e-9);
    let (ux, uy) = (cx / rc, cy / rc);
    let start = *cursor;
    let mut k = start;
    loop {
        let step = k / 3;
        let depth = (k % 3) as f64;
        let dist = step.div_ceil(2) + VENT_SIDE_M;
        if dist > VENT_REACH_M {
            *cursor = k;
            return None;
        }
        let side = if step % 2 == 0 { 1.0 } else { -1.0 };
        let r = (top - 1.5 - depth).max(1.0);
        let (sa, ca) = (side * dist as f64 / r).sin_cos();
        let p = (((ux * ca - uy * sa) * r).floor() as i64, ((ux * sa + uy * ca) * r).floor() as i64);
        if !touched.contains(&p) && world.material(p.0, p.1) == m {
            *cursor = k;
            return Some(Some(p));
        }
        k += 1;
        if k - start > 600 {
            // (Looked far enough for one cell; carry on from here next time.)
            *cursor = k;
            return Some(None);
        }
    }
}

/// Side of the squares waiting cells are kept by.
const WAIT_BUCKET: i64 = 64;

/// Where something is simulated in full: None everywhere, Some(None)
/// nowhere, Some(Some(rect)) inside rect (x0, x1, y0, y1; ends exclusive).
pub type Attention = Option<Option<[i64; 4]>>;

pub fn in_attention(a: Attention, c: Cell) -> bool {
    match a {
        None => true,
        Some(None) => false,
        Some(Some([x0, x1, y0, y1])) => c.0 >= x0 && c.0 < x1 && c.1 >= y0 && c.1 < y1,
    }
}

/// Cells put aside until someone looks, kept by square so the ones a new
/// view covers are found quickly.
#[derive(Default)]
pub struct Waiting {
    map: CellMap<CellQueue>,
    n: usize,
}

impl Waiting {
    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    pub fn add(&mut self, c: Cell) {
        if self.map.entry((c.0.div_euclid(WAIT_BUCKET), c.1.div_euclid(WAIT_BUCKET))).or_default().insert(c) {
            self.n += 1;
        }
    }

    /// Take out every cell the attention area covers.
    pub fn take_seen(&mut self, a: Attention) -> Vec<Cell> {
        let mut out = Vec::new();
        if self.n == 0 {
            return out;
        }
        match a {
            None => {
                for (_, mut v) in self.map.drain() {
                    out.extend(v.drain());
                }
                self.n = 0;
            }
            Some(None) => {}
            Some(Some([x0, x1, y0, y1])) => {
                let (bx0, bx1) = (x0.div_euclid(WAIT_BUCKET), (x1 - 1).div_euclid(WAIT_BUCKET));
                let (by0, by1) = (y0.div_euclid(WAIT_BUCKET), (y1 - 1).div_euclid(WAIT_BUCKET));
                // A huge area: walk the squares in use instead of the area's.
                let keys: Vec<Cell> = if (bx1 - bx0 + 1) * (by1 - by0 + 1) > self.map.len() as i64 {
                    let mut k: Vec<Cell> = self.map.keys().copied().filter(|b| (bx0..=bx1).contains(&b.0) && (by0..=by1).contains(&b.1)).collect();
                    k.sort();
                    k
                } else {
                    let mut k = Vec::new();
                    for by in by0..=by1 {
                        for bx in bx0..=bx1 {
                            if self.map.contains_key(&(bx, by)) {
                                k.push((bx, by));
                            }
                        }
                    }
                    k
                };
                for b in keys {
                    let mut v = self.map.remove(&b).unwrap();
                    let mut keep = CellQueue::default();
                    for c in v.drain() {
                        if in_attention(a, c) {
                            out.push(c);
                            self.n -= 1;
                        } else {
                            keep.insert(c);
                        }
                    }
                    if !keep.is_empty() {
                        self.map.insert(b, keep);
                    }
                }
            }
        }
        out
    }
}

/// Seconds the Sun's position is held between updates (shade, heat).
const SUN_STEP_S: f64 = 2.0;

/// Most cells a falling grain or drop moves in one tick through open space.
const FALL_MAX: usize = 8;

/// Which way is down at a cell: toward the centre, rounded to one of eight directions.
fn down_index(c: Cell) -> usize {
    let (gx, gy) = (-(c.0 as f64 + 0.5), -(c.1 as f64 + 0.5));
    let (ax, ay) = (gx.abs(), gy.abs());
    let d = if ax > 2.414 * ay {
        (gx.signum() as i64, 0)
    } else if ay > 2.414 * ax {
        (0, gy.signum() as i64)
    } else {
        (gx.signum() as i64, gy.signum() as i64)
    };
    DIRS.iter().position(|&x| x == d).unwrap()
}

/// The melting or boiling point where the body's own heat is `ambient`: deep
/// inside, where the body is hotter than the material's point, the weight
/// above holds it solid (or liquid) up to a margin past the local heat.
fn held_point(at_k: f64, ambient: f64, margin: f64) -> f64 {
    if ambient >= at_k { ambient + margin } else { at_k }
}

fn add(c: Cell, d: Cell) -> Cell {
    (c.0 + d.0, c.1 + d.1)
}

impl Sandbox {
    pub fn new(world: &World) -> Self {
        let mats = &world.planet.mats;
        let day_k = world.planet.spec.sun.as_ref().map_or(0.0, |s| s.day_k);
        let sun_sensitive: Vec<bool> = (0..mats.len()).map(|m| mats.phases[m].up.iter().flatten().any(|ch| ch.at_k < day_k)).collect();
        let mut by_mat = vec![Vec::new(); mats.len()];
        for (i, r) in mats.reactions.iter().enumerate() {
            by_mat[r.a as usize].push((i, true));
            by_mat[r.b as usize].push((i, false));
        }
        Sandbox { next: CellQueue::default(), warm: CellQueue::default(), slow: CellQueue::default(), far: CellQueue::default(), latent: CellMap::default(), temps: CellMap::default(), tick: 0, time_s: 0.0, sun: None, sun_cache: CellMap::default(), ambient: Default::default(), shallow: Default::default(), sun_sensitive, sun_step: -1, seed: world.planet.spec.seed ^ 0x5A4D_B0C5, by_mat, stats: Stats::default(), attention: None, dormant: Waiting::default(), vents: CellMap::default(), ejecta: Vec::new() }
    }

    /// Where motion is simulated (see `Attention`; None, everywhere, is the
    /// default). Dormant cells the new area covers wake.
    pub fn set_attention(&mut self, a: Attention) {
        if self.attention == a {
            return;
        }
        self.attention = a;
        for c in self.dormant.take_seen(a) {
            self.next.insert(c);
        }
        // Cells warming out of sight that the new area covers go back to the
        // warming cadence.
        if !self.far.is_empty() {
            for c in self.far.drain() {
                if in_attention(a, c) {
                    self.warm.insert(c);
                } else {
                    self.far.insert(c);
                }
            }
        }
    }

    /// Whether cell `c` is where things are simulated in full (on screen).
    pub fn sees(&self, c: Cell) -> bool {
        in_attention(self.attention, c)
    }

    /// A cell to step for its heat: on the warming cadence, or, out of
    /// sight, the far one.
    fn queue_heat(&mut self, c: Cell) {
        if self.sees(c) {
            self.warm.insert(c);
        } else if !self.far.contains(&c) {
            self.far.insert(c);
        }
    }

    /// Cells waiting, dormant, for someone to look.
    pub fn dormant(&self) -> usize {
        self.dormant.len()
    }

    /// Cells waiting to be stepped.
    pub fn awake(&self) -> usize {
        self.next.len() + self.warm.len() + self.slow.len() + self.far.len()
    }

    /// Awake cells by queue: stepping every tick, warming, slow, and only
    /// warming out of sight (for logs).
    pub fn queue_sizes(&self) -> (usize, usize, usize, usize) {
        (self.next.len(), self.warm.len(), self.slow.len(), self.far.len())
    }

    /// The awake cells (for diagnostics).
    pub fn awake_cells(&self) -> Vec<Cell> {
        self.next.iter().chain(self.warm.iter()).chain(self.slow.iter()).chain(self.far.iter()).copied().collect()
    }

    pub fn wake(&mut self, c: Cell) {
        self.next.insert(c);
    }

    /// Something hot touches cell c (an explosion, a spark): it is at least
    /// `k` kelvin now, and wakes (what burns may catch).
    /// (Profiling) cells waiting in each queue (next, warm, slow, far), and
    /// cells off the body's temperature.
    pub fn load(&self) -> [usize; 5] {
        [self.next.len(), self.warm.len(), self.slow.len(), self.far.len(), self.temps.len()]
    }

    pub fn heat_cell(&mut self, world: &World, c: Cell, k: f64) {
        let m = world.material(c.0, c.1);
        if m == VACUUM {
            return;
        }
        let t = self.temp(world, c, m);
        if k > t {
            self.temps.insert(c, k);
            self.next.insert(c);
        }
    }

    /// Every cell waiting to be stepped, sooner or later (for saving: they
    /// are all woken again on loading).
    pub fn pending(&self) -> Vec<Cell> {
        let mut out: Vec<Cell> = self.next.iter().chain(self.warm.iter()).chain(self.slow.iter()).chain(self.far.iter()).copied().collect();
        for q in self.dormant.map.values() {
            out.extend(q.iter().copied());
        }
        out
    }

    /// Wake a cell and its eight neighbours.
    pub fn wake_around(&mut self, c: Cell) {
        self.next.insert(c);
        for d in DIRS {
            self.next.insert(add(c, d));
        }
    }

    /// Wake a cell and its eight neighbours, skipping the empty ones (an
    /// empty cell has nothing to do; whatever later fills it wakes it then).
    /// An empty cell is let go of its temperature, as stepping it would.
    pub fn wake_around_filled(&mut self, world: &World, c: Cell) {
        let mut rd = world.reader();
        for x in std::iter::once(c).chain(DIRS.iter().map(|d| add(c, *d))) {
            if rd.get(x.0, x.1) == VACUUM {
                self.temps.remove(&x);
            } else {
                self.next.insert(x);
            }
        }
    }

    /// Whether open cell n sees the Sun at time t (for tools and tests).
    pub fn sees_sun(world: &World, n: Cell, t: f64) -> Option<f64> {
        Self::trace_sun(world, n, world.planet.sun_dir(t)?, t)
    }

    fn trace_sun(world: &World, n: Cell, (sx, sy): (f64, f64), time_s: f64) -> Option<f64> {
        let (fx, fy) = (n.0 as f64 + 0.5, n.1 as f64 + 0.5);
        let r = (fx * fx + fy * fy).sqrt().max(1e-9);
        let k = world.planet.sky_k(fx / r, fy / r, time_s);
        if k <= world.planet.space_k {
            return None;
        }
        // March toward the Sun in 2 m strides until out above the ground
        // (crater bowls count as open). Well below the ground line only
        // dug-out regions are looked at cell by cell: untouched ground there
        // is solid. Far hills casting long shadows near dusk are let go.
        let mut rd = world.reader();
        let (mut x, mut y) = (fx, fy);
        for _ in 0..300 {
            x += 2.0 * sx;
            y += 2.0 * sy;
            let r = (x * x + y * y).sqrt().max(1e-9);
            let ground = world.planet.ground_hint(x / r, y / r);
            if r > ground + 2.0 {
                return Some(k);
            }
            // Near the ground line the cell itself decides; deeper, only a
            // dug-out region can be open.
            let (cx, cy) = (x.floor() as i64, y.floor() as i64);
            if (r < ground - 6.0 && !rd.touched(cx, cy)) || rd.get(cx, cy) != VACUUM {
                return None;
            }
        }
        None
    }

    /// Does this material need its temperature tracked (it can change phase, or flows)?
    fn tracks_heat(world: &World, m: MatId) -> bool {
        if m == VACUUM {
            return false;
        }
        let ph = &world.planet.mats.phases[m as usize];
        ph.up.iter().chain(ph.down.iter()).any(|c| c.is_some()) || world.planet.mats.get(m).state == State::Liquid
    }

    /// The temperature of cell c, holding material m.
    pub fn temp(&self, world: &World, c: Cell, m: MatId) -> f64 {
        match self.temps.get(&c) {
            Some(t) => *t,
            None => self.start_k(world, c, m),
        }
    }

    /// The body's own heat at cell c. It never changes (sunlight is applied
    /// separately), so it is worked out once per cell and kept.
    fn ambient_k(&self, world: &World, c: Cell) -> f64 {
        let mut cache = self.ambient.lock().unwrap();
        if let Some(&t) = cache.get(&c) {
            return t;
        }
        if cache.len() > 4_000_000 {
            cache.clear();
        }
        let t = world.planet.ambient_k(c.0, c.1);
        cache.insert(c, t);
        t
    }

    /// The body's heat at cell c as far as pressure goes: deep down, the
    /// weight above holds melting and boiling points up to the local heat
    /// (see `held_point`); near the surface there is no such weight, so
    /// ice on a warm world melts like ice. (0 there: nothing is held.)
    fn held_ambient(&self, world: &World, c: Cell) -> f64 {
        let amb = self.ambient_k(world, c);
        let mut cache = self.shallow.lock().unwrap();
        let shallow = *cache.entry(c).or_insert_with(|| {
            let (x, y) = (c.0 as f64 + 0.5, c.1 as f64 + 0.5);
            let r = (x * x + y * y).sqrt().max(1e-9);
            world.planet.surface_radius(x / r, y / r) - r < HOLD_DEPTH_M
        });
        if cache.len() > 4_000_000 {
            cache.clear();
        }
        if shallow { 0.0 } else { amb }
    }

    /// A cell's temperature as the body made it (as `Planet::start_k`, with
    /// the body's heat from the cache).
    fn start_k(&self, world: &World, c: Cell, m: MatId) -> f64 {
        world.planet.start_k_at(self.ambient_k(world, c), m)
    }

    /// Step 1 of a cell's tick, heat, worked out without changing anything
    /// (so every cell's can be worked out at once): whether its heat is
    /// tracked, its new temperature and change, whether it is still
    /// warming, the neighbours it should wake, and sunlight looked up.
    fn heat_of(&self, world: &World, c: Cell, m: MatId, heat_step: f64, phys: &crate::materials::Physics) -> HeatOut {
        let mut out = HeatOut::default();
        let mut wi = 0usize;
            // 1. Heat.
            // Liquids and gases always carry their own temperature. A solid that
            // could melt only does once heat has reached it (it is already off
            // the body's own temperature, or a neighbour is); otherwise it is
            // part of the heat bath and costs nothing.
            let solid = matches!(world.planet.mats.get(m).state, State::Solid | State::Powder);
            let burn = world.planet.mats.burns[m as usize];
            // Something that burns, with something hot enough to light it
            // touching it, warms like anything else (and may catch).
            let lit_nearby = burn.is_some_and(|b| {
                N4.iter().any(|d| {
                    let n = add(c, *d);
                    let nm = world.material(n.0, n.1);
                    nm != VACUUM && self.temps.get(&n).is_some_and(|&tn| tn >= b.ignite_k)
                })
            });
            let tracked = (Self::tracks_heat(world, m) || burn.is_some())
                && (!solid || self.temps.contains_key(&c) || lit_nearby || (self.sun_sensitive[m as usize] && self.sunlit_peek(world, c, &mut out)) || {
                    // A neighbouring liquid hot enough to melt it (magma against
                    // sulfur). Rock that lava can't melt stays part of the bath.
                    let melt = world.planet.mats.phases[m as usize].up[0].map_or(f64::INFINITY, |ch| held_point(ch.at_k, self.held_ambient(world, c), phys.melt_margin_k));
                    N4.iter().any(|d| {
                        let n = add(c, *d);
                        let nm = world.material(n.0, n.1);
                        nm != VACUUM && world.planet.mats.get(nm).state == State::Liquid && self.temp(world, n, nm) >= melt
                    })
                });
            let mut t = 0.0;
            let mut heat_dt = 0.0;
            // Long slow steps close at most SLOW_MAX_SHARE of any gap at once.
            let share = |x: f64| if heat_step > HEAT_EVERY as f64 { x.min(SLOW_MAX_SHARE) } else { x };
            if tracked {
                t = self.temp(world, c, m);
                let my_conduct = world.planet.mats.get(m).conduct;
                let mut dt = 0.0;
                // Sunlight is applied implicitly (it can be strong next to a
                // long heat step): weight and weighted warmth of sunlit faces.
                let (mut sun_w, mut sun_wk) = (0.0, 0.0);
                for d in N4 {
                    let n = add(c, d);
                    let nm = world.material(n.0, n.1);
                    if nm == VACUUM {
                        // Radiating to open space; where the Sun shines in,
                        // toward what it warms bare ground to.
                        match self.sun_peek(world, n, &mut out) {
                            Some(sky) => {
                                sun_w += heat_step * phys.sunlight;
                                sun_wk += heat_step * phys.sunlight * sky;
                            }
                            None => dt += share(heat_step * phys.radiate * my_conduct * 2.0) * (world.planet.space_k - t),
                        }
                        continue;
                    }
                    let tn = self.temp(world, n, nm);
                    let k = phys.conduct * my_conduct.min(world.planet.mats.get(nm).conduct) * 2.0;
                    dt += share(heat_step * k) * (tn - t);
                    if (tn - t).abs() > phys.wake_gradient_k && Self::tracks_heat(world, nm) {
                        out.wake[wi] = Some(n);
                        wi += 1;
                    }
                }
                if sun_w > 0.0 {
                    // Backward step toward the Sun's warmth: never overshoots.
                    let t1 = t + dt;
                    dt = (t1 + sun_wk) / (1.0 + sun_w) - t;
                }
                t += dt;
                heat_dt = dt;
                if dt.abs() > phys.settle_k * heat_step {
                    out.warming = true;
                }
            }

        out.tracked = tracked;
        out.t = t;
        out.dt = heat_dt;
        let _ = (solid, wi);
        out
    }

    /// `sun_on` for reading only: a new lookup is noted in `out` (kept after).
    fn sun_peek(&self, world: &World, n: Cell, out: &mut HeatOut) -> Option<f64> {
        let (sx, sy) = self.sun?;
        if let Some(&v) = self.sun_cache.get(&n) {
            return (v > 0.0).then_some(v as f64);
        }
        if let Some(&(_, v)) = out.sun_new.iter().flatten().find(|(k, _)| *k == n) {
            return (v > 0.0).then_some(v as f64);
        }
        let v = Self::trace_sun(world, n, (sx, sy), self.sun_step as f64 * SUN_STEP_S);
        if let Some(slot) = out.sun_new.iter_mut().find(|s| s.is_none()) {
            *slot = Some((n, v.unwrap_or(0.0) as f32));
        }
        v
    }

    /// `sunlit` for reading only.
    fn sunlit_peek(&self, world: &World, c: Cell, out: &mut HeatOut) -> bool {
        if self.sun.is_none() {
            return false;
        }
        N4.iter().any(|d| {
            let n = add(c, *d);
            world.material(n.0, n.1) == VACUUM && self.sun_peek(world, n, out).is_some()
        })
    }

    /// Step every awake cell once. Returns every cell whose contents changed.
    pub fn step(&mut self, world: &mut World) -> Vec<Change> {
        self.tick += 1;
        // Where the Sun is: moved on a step, last step's shadows are stale.
        if world.planet.spec.sun.is_some() {
            let k = (self.time_s / SUN_STEP_S).floor() as i64;
            if k != self.sun_step {
                self.sun_step = k;
                self.sun = world.planet.sun_dir(k as f64 * SUN_STEP_S);
                self.sun_cache.clear();
            }
        }
        // (cell, heat step): cells that may move step every tick; cells that
        // are only cooling or warming step every HEAT_EVERY ticks, in one
        // bigger step.
        let heat_tick = self.tick % HEAT_EVERY == 0;
        let mut cells: Vec<(Cell, f64)> = self.next.drain().into_iter().map(|c| (c, 1.0)).collect();
        if heat_tick {
            let drained: Vec<Cell> = self.warm.drain();
            let mut moving = CellQueue::default();
            for (c, _) in &cells {
                moving.insert(*c);
            }
            for c in drained {
                if !moving.contains(&c) {
                    moving.insert(c);
                    cells.push((c, HEAT_EVERY as f64));
                }
            }
            if self.tick % SLOW_EVERY == 0 && !self.slow.is_empty() {
                for c in self.slow.drain() {
                    if !moving.contains(&c) {
                        moving.insert(c);
                        cells.push((c, SLOW_EVERY as f64));
                    }
                }
            }
            if self.tick % FAR_EVERY == 0 && !self.far.is_empty() {
                for c in self.far.drain() {
                    if !moving.contains(&c) {
                        moving.insert(c);
                        cells.push((c, FAR_EVERY as f64));
                    }
                }
            }
        }
        // Keys are unique (they include the cell), so an unstable sort gives
        // the same order, faster.
        cells.sort_unstable_by_key(|&((x, y), _)| (x * x + y * y, x, y));
        let mut changes = Vec::new();
        // Cells already changed this tick: each cell acts at most once.
        let mut touched = CellQueue::default();
        let phys = world.planet.mats.physics.clone();
        // Step 1, heat, for every cell first, side by side on all the
        // simulation's threads: each reads the temperatures as the tick
        // began (so the order cells are done in doesn't matter, and the
        // result is the same on any number of threads). The rest of the
        // tick then goes cell by cell in order, as before.
        let pre: Vec<HeatOut> = {
            let me: &Sandbox = self;
            let w: &World = world;
            let one = |&(c, heat_step): &(Cell, f64)| -> HeatOut {
                if !w.in_bounds(c.0, c.1) {
                    return HeatOut::default();
                }
                let m = w.material(c.0, c.1);
                if m == VACUUM {
                    return HeatOut::default();
                }
                me.heat_of(w, c, m, heat_step, &phys)
            };
            if cells.len() >= PAR_MIN {
                use rayon::prelude::*;
                cells.par_iter().with_min_len(256).map(one).collect()
            } else {
                cells.iter().map(one).collect()
            }
        };
        // Sunlight looked up for the first time this tick, kept.
        for ho in &pre {
            for &(n, v) in ho.sun_new.iter().flatten() {
                self.sun_cache.insert(n, v);
            }
        }
        for (i, &(c, heat_step)) in cells.iter().enumerate() {
            if touched.contains(&c) || !world.in_bounds(c.0, c.1) {
                continue;
            }
            let m = world.material(c.0, c.1);
            if m == VACUUM {
                self.temps.remove(&c);
                continue;
            }
            self.stats.cells_stepped += 1;
            let h = hash(self.seed ^ self.tick.wrapping_mul(0x9E37_79B9), c.0, c.1, 17);
            // Busy moving or reacting (step again next tick), or only changing
            // temperature (step again on the next heat tick).
            let mut busy = false;
            let mut warming = false;

            // 1. Heat (worked out for every cell at once, above).
            let burn = world.planet.mats.burns[m as usize];
            let ho = &pre[i];
            let tracked = ho.tracked;
            let t = ho.t;
            let heat_dt = ho.dt;
            if tracked {
                for n in ho.wake.iter().flatten() {
                    if !self.slow.contains(n) {
                        self.queue_heat(*n);
                    }
                }
                self.temps.insert(c, t);
                if ho.warming {
                    warming = true;
                }
            }
            #[allow(unused_mut)]
            let mut t = t;

            // 1b. Burning: hot enough, with air (open space on a body with
            // oxygen) or an oxidizer touching it. It holds at its flame
            // temperature, its heat flowing on into its neighbours, and burns
            // away bit by bit.
            if let Some(b) = burn.filter(|b| tracked && t >= b.ignite_k) {
                let o2 = world.planet.spec.oxygen / 0.21;
                let mut air = 0.0;
                for d in N4 {
                    let n = add(c, d);
                    let nm = world.material(n.0, n.1);
                    if nm == VACUUM {
                        air += o2;
                    } else if world.planet.mats.oxidizer[nm as usize] {
                        air += 1.0;
                    }
                }
                let air = (air / 4.0).min(1.0);
                if air > 0.0 {
                    t = t.max(b.flame_k);
                    self.temps.insert(c, t);
                    self.stats.burning += 1;
                    busy = true;
                    // Its neighbours feel it at once; flame licks at anything
                    // else that burns, heating it toward catching.
                    for d in N4 {
                        let n = add(c, d);
                        let nm = world.material(n.0, n.1);
                        if nm == VACUUM {
                            continue;
                        }
                        if world.planet.mats.burns[nm as usize].is_some() && !touched.contains(&n) {
                            let tn = self.temp(world, n, nm);
                            if tn < b.flame_k {
                                self.temps.insert(n, tn + (b.flame_k - tn) * FLAME_LICK);
                            }
                            self.next.insert(n);
                        } else {
                            self.queue_heat(n);
                        }
                    }
                    if unit(hash(h, 7, 7, 0xF14E)) < b.rate * air / phys.ticks_per_s {
                        // Burned away: its mass goes off as smoke and gas.
                        if let Some(gone) = world.lift_cell(c.0, c.1) {
                            world.lose_aloft(gone);
                        }
                        self.temps.remove(&c);
                        self.latent.remove(&c);
                        changes.push(Change { cell: c, from: m, to: VACUUM });
                        self.stats.burned += 1;
                        touched.insert(c);
                        self.wake_around(c);
                        continue;
                    }
                }
            }

            // 2. Phase change, with latent heat: past the transition point the
            // temperature holds at it while heat goes into (or comes out of) the
            // change, and only once `latent_k` has gone in does the cell turn.
            if tracked {
                let ph = world.planet.mats.phases[m as usize];
                let mut to = None;
                let mut held = false;
                // Heating: the first transition the cell has reached (melting
                // before boiling).
                // Pressure: deep inside a body the weight above raises melting and
                // boiling points, so the body as made is stable at its own heat.
                // Heating past that (by a margin) is what melts or boils.
                let amb = self.held_ambient(world, c);
                if let Some((ch, at)) = ph.up.iter().flatten().map(|ch| (ch, held_point(ch.at_k, amb, phys.melt_margin_k))).find(|(_, at)| t >= *at) {
                    let bank = self.latent.get(&c).copied().unwrap_or(0.0).max(0.0) + (t - at);
                    t = at;
                    held = true;
                    if bank >= ch.latent_k {
                        to = Some(ch.to);
                        self.latent.remove(&c);
                    } else {
                        self.latent.insert(c, bank);
                    }
                } else if let Some(ch) = ph.down.iter().flatten().find(|ch| t <= ch.at_k) {
                    let bank = self.latent.get(&c).copied().unwrap_or(0.0).min(0.0) - (ch.at_k - t);
                    t = ch.at_k;
                    held = true;
                    if -bank >= ch.latent_k {
                        to = Some(ch.to);
                        self.latent.remove(&c);
                    } else {
                        self.latent.insert(c, bank);
                    }
                } else {
                    self.latent.remove(&c);
                }
                if held {
                    self.temps.insert(c, t);
                    warming = true;
                }
                if let Some(to) = to {
                    world.change_cells(&[(c, to)]);
                    changes.push(Change { cell: c, from: m, to });
                    self.stats.phase_changes += 1;
                    touched.insert(c);
                    self.wake_around(c);
                    continue;
                }
            }

            // 3. Reactions with a touching cell.
            let mut reacted = false;
            let options = self.by_mat[m as usize].clone();
            for (ri, as_a) in options {
                let r = world.planet.mats.reactions[ri];
                let partner = if as_a { r.b } else { r.a };
                for (k, d) in N4.iter().enumerate() {
                    let n = add(c, *d);
                    if touched.contains(&n) || world.material(n.0, n.1) != partner {
                        continue;
                    }
                    busy = true;
                    if unit(hash(h, ri as i64, k as i64, 23)) >= r.chance {
                        continue;
                    }
                    let (mine, theirs) = if as_a { (r.a_to, r.b_to) } else { (r.b_to, r.a_to) };
                    // The products share the inputs' heat, plus what the reaction gives off.
                    let t_mix = (self.temp(world, c, m) + self.temp(world, n, partner)) / 2.0 + r.heat_k;
                    world.change_cells(&[(c, mine), (n, theirs)]);
                    changes.push(Change { cell: c, from: m, to: mine });
                    changes.push(Change { cell: n, from: partner, to: theirs });
                    for x in [c, n] {
                        self.temps.insert(x, t_mix);
                        touched.insert(x);
                        self.wake_around(x);
                    }
                    self.stats.reactions += 1;
                    reacted = true;
                    break;
                }
                if reacted {
                    break;
                }
            }
            if reacted {
                continue;
            }

            // 4. Motion (only where someone could see it; elsewhere the
            // cell waits until they look).
            let mat = world.planet.mats.get(m);
            let state = mat.state;
            if state != State::Solid && !self.sees(c) {
                self.dormant.add(c);
            } else if state != State::Solid {
                let di = down_index(c);
                let flip = h & 1 == 1;
                let (l, r) = if flip { (7, 1) } else { (1, 7) };
                let (sl, sr) = if flip { (6, 2) } else { (2, 6) };
                let order: Vec<usize> = match state {
                    State::Powder => vec![di, (di + l) % 8, (di + r) % 8],
                    State::Liquid => vec![di, (di + l) % 8, (di + r) % 8, (di + sl) % 8, (di + sr) % 8],
                    State::Gas => vec![(di + 4) % 8, (di + 4 + l) % 8, (di + 4 + r) % 8, (di + sl) % 8, (di + sr) % 8],
                    State::Solid => vec![],
                };
                let density = mat.buoyancy();
                let mut target = None;
                // Matter only moves downhill (gas only up): each move must take
                // it strictly nearer the centre (or further, for gas). On a
                // round body the eight directions don't quite follow the
                // ground, and without this a pool's surface or a powder slope
                // can trade cells back and forth forever.
                let r2 = |q: Cell| (2 * q.0 + 1) * (2 * q.0 + 1) + (2 * q.1 + 1) * (2 * q.1 + 1);
                let rc = r2(c);
                for i in order {
                    let n = add(c, DIRS[i]);
                    if touched.contains(&n) {
                        continue;
                    }
                    let rn = r2(n);
                    if (state == State::Gas && rn <= rc) || (state != State::Gas && rn >= rc) {
                        continue;
                    }
                    if !world.in_bounds(n.0, n.1) {
                        if state == State::Gas {
                            target = Some((n, VACUUM, 0));
                            break;
                        }
                        continue;
                    }
                    let nm = world.material(n.0, n.1);
                    let ns = world.planet.mats.get(nm);
                    let ok = nm == VACUUM
                        || match state {
                            State::Powder => matches!(ns.state, State::Liquid | State::Gas),
                            State::Liquid => ns.state == State::Gas || (ns.state == State::Liquid && ns.buoyancy() < density && i == di),
                            // Gas bubbles straight up through loose dust and
                            // liquid (they sink past it the other way).
                            State::Gas => matches!(ns.state, State::Powder | State::Liquid) && i == (di + 4) % 8,
                            _ => false,
                        };
                    if ok {
                        // Falling straight down through open space: keep going,
                        // up to FALL_MAX cells this tick, rather than one cell
                        // per tick (each step of a long drop costs a full move).
                        // (Gas rises through open space the same way.)
                        let mut n = n;
                        let straight = if state == State::Gas { (di + 4) % 8 } else { di };
                        if nm == VACUUM && i == straight {
                            for _ in 1..FALL_MAX {
                                let f = add(n, DIRS[straight]);
                                if touched.contains(&f) || !world.in_bounds(f.0, f.1) || world.material(f.0, f.1) != VACUUM {
                                    break;
                                }
                                n = f;
                            }
                        }
                        target = Some((n, nm, i));
                        break;
                    }
                }
                // A liquid from a buried layer with nowhere downhill to go is
                // pushed up an open hole above it by the weight on its layer.
                // Where that weight only lifts it part way (water under ice),
                // it wells up the hole to where the column balances: the push
                // acts through the whole column at once, liquid appearing at
                // the top of the column above this cell, drawn from the layer
                // a way off under its roof (a whole buried ocean barely
                // notices), so no hole is left here for the liquid around to
                // slosh into.
                // Where the weight would lift it past the rim (magma under
                // denser rock), the hole is a vent and the liquid blasts out
                // of its mouth, thrown up at the speed the extra head gives
                // (see `Vent`), and lands wherever it comes down. What blasts
                // out is drawn from the layer under its roof next to the
                // hole, so the chamber visibly drains there, and the pressure
                // falls as it drains: once the vent has given its charge it
                // goes quiet, and what is left pools in its chamber.
                // Every kilogram comes from the layer: nothing is made.
                let mut pushed = None;
                let mut erupt: Option<(Cell, Cell, f64)> = None;
                if target.is_none() && state == State::Liquid {
                    let rr = ((c.0 as f64 + 0.5) * (c.0 as f64 + 0.5) + (c.1 as f64 + 0.5) * (c.1 as f64 + 0.5)).sqrt();
                    // (From the top of the layer, or from the top of a column
                    // already risen: in between, nothing is open above.)
                    let up_open = world.material(add(c, DIRS[(di + 4) % 8]).0, add(c, DIRS[(di + 4) % 8]).1) == VACUUM;
                    let near_top = world.planet.pool_top(m).is_some_and(|top| (rr - top).abs() < 8.0 || (up_open && rr > top));
                    let su = world.planet.surface_radius(c.0 as f64 / rr.max(1e-9), c.1 as f64 / rr.max(1e-9));
                    let hot = self.temp(world, c, m) >= VENT_GAS_K;
                    // A buried layer; or a pocket the body was made with (a
                    // magma chamber, a sealed lava lake) whose roof has just
                    // been dug through: the rock that was over it presses on
                    // it the same way. (Liquid lying about on the ground, or
                    // run into a hole, is just liquid.)
                    let pocket = if hot && up_open && world.planet.pool_top(m).is_none_or(|t| rr > t + 8.0) {
                        // The pocket itself: down the column of the same liquid
                        // to where it was when the body was made (it may have
                        // risen up the hole since, or drained down a little).
                        let down = DIRS[di];
                        let mut b = c;
                        let mut k = 0;
                        while k < 256 && world.planet.material(b.0, b.1) != m {
                            let nb = add(b, down);
                            if world.material(nb.0, nb.1) != m {
                                break;
                            }
                            b = nb;
                            k += 1;
                        }
                        // Its roof as made: the first thing above that wasn't
                        // part of it.
                        let up = DIRS[(di + 4) % 8];
                        let mut u = add(b, up);
                        let mut k = 0;
                        while k < 64 && world.planet.material(u.0, u.1) == m {
                            u = add(u, up);
                            k += 1;
                        }
                        let roof = world.planet.material(u.0, u.1);
                        (world.planet.material(b.0, b.1) == m && roof != VACUUM && matches!(world.planet.mats.get(roof).state, State::Solid | State::Powder)).then(|| {
                            let ratio = world.planet.mats.get(roof).density / mat.density;
                            let br = (((b.0 as f64 + 0.5).powi(2) + (b.1 as f64 + 0.5).powi(2)).sqrt()).min(rr);
                            (br + (su - br).max(0.0) * ratio, br, false, Some(u))
                        })
                    } else {
                        None
                    };
                    let layer = if pocket.is_none() { world.planet.pressure_head(c.0, c.1, m).filter(|_| near_top).map(|(hd, t)| (hd, t, true, None)) } else { None };
                    if let Some((head, top, is_layer, roof_at)) = pocket.or(layer) {
                        // Molten rock and metal carry gas that comes out of
                        // them as they rise, and drives them out harder than
                        // the weight of the rock alone (VENT_GAS more of the
                        // depth of their roof under the surface).
                        let head = if hot { head + (su - top).max(0.0) * VENT_GAS } else { head };
                        let up = (di + 4) % 8;
                        let mut n = add(c, DIRS[up]);
                        let mut steps = 0;
                        while steps < 4096 && world.material(n.0, n.1) == m {
                            n = add(n, DIRS[up]);
                            steps += 1;
                        }
                        let (fx, fy) = (n.0 as f64 + 0.5, n.1 as f64 + 0.5);
                        let nr = (fx * fx + fy * fy).sqrt();
                        let rim = world.planet.surface_radius(fx / nr.max(1e-9), fy / nr.max(1e-9)) - 1.0;
                        let open_top = !touched.contains(&n) && world.in_bounds(n.0, n.1) && world.material(n.0, n.1) == VACUUM && r2(n) > rc;
                        // A vent only where the hole is open all the way up and
                        // out (not a gap under the layer's own roof).
                        // (Only from the layer itself: liquid it has thrown out
                        // and that lies about on the ground is just liquid.)
                        let sky = if open_top && head > rim + 1.0 && (!is_layer || (rr - top).abs() < 8.0) { open_to_sky(world, n) } else { None };
                        if let Some(mouth) = sky {
                            // (Kept by where the hole broke through the roof of a
                            // pocket, or comes out of the ground over a layer:
                            // neither moves as the liquid in it sinks or the
                            // hole widens.)
                            let at = roof_at.unwrap_or((mouth.0.floor() as i64, mouth.1.floor() as i64));
                            let key = (at.0.div_euclid(VENT_M), at.1.div_euclid(VENT_M));
                            let tick = self.tick;
                            let v = self.vents.entry(key).or_insert(Vent { excess0: head - rim, drained: 0, cursor: 0, tick: 0, this_tick: 0 });
                            if v.tick != tick {
                                v.tick = tick;
                                v.this_tick = 0;
                            }
                            let charge = (v.excess0 * VENT_CELLS_PER_M).max(VENT_MIN_CELLS);
                            let excess = v.excess0 * (1.0 - v.drained as f64 / charge);
                            if excess >= 1.0 && v.this_tick < VENT_MAX_PER_TICK {
                                match roof_source(world, &touched, c, top, m, &mut v.cursor) {
                                    Some(Some(src)) => {
                                        v.drained += 1;
                                        v.this_tick += 1;
                                        erupt = Some((src, n, excess));
                                    }
                                    Some(None) => {}
                                    // Nothing left under the roof near the hole: spent.
                                    None => v.drained = charge.ceil() as u32,
                                }
                            }
                        } else if is_layer && open_top && nr < head.min(rim) {
                            // The source: 300 to 600 m along the roof, either
                            // side, 10 to 40 m under it.
                            let (ux, uy) = ((c.0 as f64 + 0.5) / rr, (c.1 as f64 + 0.5) / rr);
                            let side = if h & 2 == 0 { 1.0 } else { -1.0 };
                            let along = (300.0 + 300.0 * unit(hash(h, 1, 0, 31))) / top * side;
                            let (vx, vy) = (ux - uy * along, uy + ux * along);
                            let l = (vx * vx + vy * vy).sqrt();
                            let dr = top - 10.0 - 30.0 * unit(hash(h, 2, 0, 31));
                            let src = ((vx / l * dr).floor() as i64, (vy / l * dr).floor() as i64);
                            if !touched.contains(&src) && world.material(src.0, src.1) == m {
                                pushed = Some((src, n));
                            }
                        }
                    }
                }
                if let Some((src, n, excess)) = erupt {
                    let t_src = self.temp(world, src, m);
                    world.lift_cell(src.0, src.1);
                    changes.push(Change { cell: src, from: m, to: VACUUM });
                    self.temps.remove(&src);
                    self.latent.remove(&src);
                    touched.insert(src);
                    // (The liquid round the gap may now pool into it.)
                    self.wake_around(src);
                    // Out of the mouth of the hole.
                    let mouth = open_to_sky(world, n).unwrap_or((n.0 as f64 + 0.5, n.1 as f64 + 0.5));
                    let mr = (mouth.0 * mouth.0 + mouth.1 * mouth.1).sqrt().max(1e-9);
                    let (ux, uy) = (mouth.0 / mr, mouth.1 / mr);
                    let g = {
                        let p = crate::bodies::pull(&world.planet, mouth.0, mouth.1);
                        (p.0 * p.0 + p.1 * p.1).sqrt()
                    };
                    let speed = (2.0 * g * excess).sqrt() * VENT_BOOST * (0.8 + 0.4 * unit(hash(h, 4, 0, 37)));
                    let turn = (unit(hash(h, 3, 0, 37)) - 0.5) * VENT_SPREAD;
                    let (sn, cs) = turn.sin_cos();
                    let dir = (ux * cs - uy * sn, ux * sn + uy * cs);
                    let jig = (unit(hash(h, 5, 0, 37)) - 0.5) * 0.8;
                    self.ejecta.push(Eject { m, at: (mouth.0 - uy * jig, mouth.1 + ux * jig), vel: (dir.0 * speed, dir.1 * speed), k: t_src });
                    self.stats.erupted += 1;
                    self.next.insert(c);
                    continue;
                }
                if let Some((src, n)) = pushed {
                    world.swap_cells(src, n);
                    changes.push(Change { cell: src, from: m, to: VACUUM });
                    changes.push(Change { cell: n, from: VACUUM, to: m });
                    if let Some(v) = self.temps.remove(&src) {
                        self.temps.insert(n, v);
                    }
                    self.stats.moves += 1;
                    touched.insert(src);
                    touched.insert(n);
                    self.wake_around_filled(world, n);
                    self.next.insert(c);
                    continue;
                }
                if let Some((n, nm, ti)) = target {
                    busy = true;
                    if unit(hash(h, 0, 0, 29)) < mat.flow {
                        // Gas leaving the stored world, or far above the
                        // surface, is lost to space.
                        let far = {
                            let (fx, fy) = (n.0 as f64, n.1 as f64);
                            (fx * fx + fy * fy).sqrt() > world.planet.max_radius + 64.0
                        };
                        if state == State::Gas && (!world.in_bounds(n.0, n.1) || far) {
                            world.vent_cell(c.0, c.1);
                            changes.push(Change { cell: c, from: m, to: VACUUM });
                            self.temps.remove(&c);
                            self.stats.vented += 1;
                            touched.insert(c);
                            self.wake_around(c);
                            continue;
                        }
                        world.swap_cells(c, n);
                        changes.push(Change { cell: c, from: m, to: nm });
                        changes.push(Change { cell: n, from: nm, to: m });
                        // Heat travels with the matter.
                        let (tc, tn) = (self.temps.remove(&c), self.temps.remove(&n));
                        if let Some(v) = tc {
                            self.temps.insert(n, v);
                        }
                        if let Some(v) = tn {
                            self.temps.insert(c, v);
                        }
                        let (lc, ln) = (self.latent.remove(&c), self.latent.remove(&n));
                        if let Some(v) = lc {
                            self.latent.insert(n, v);
                        }
                        if let Some(v) = ln {
                            self.latent.insert(c, v);
                        }
                        // Liquid leaving the open sea: the sea is so big
                        // (half a world, a kilometre and more deep) that a
                        // pit's worth never shows in its level, so it closes
                        // up behind at once, as if bottomless, and no hole
                        // wanders over its flat surface for ever. (The inflow
                        // is booked as deposited.)
                        if state == State::Liquid && nm == VACUUM && world.planet.sea().is_some_and(|(sm, _)| sm == m) && world.planet.in_sea(c.0, c.1) {
                            world.deposit_cell(c.0, c.1, m);
                            changes.push(Change { cell: c, from: VACUUM, to: m });
                        }
                        self.stats.moves += 1;
                        let rel = (ti + 8 - di) % 8;
                        match rel {
                            0 => self.stats.moves_down += 1,
                            1 | 7 => self.stats.moves_diag += 1,
                            _ => self.stats.moves_side += 1,
                        }
                        if state == State::Powder {
                            self.stats.moves_powder += 1;
                        }
                        touched.insert(c);
                        touched.insert(n);
                        self.wake_around_filled(world, c);
                        self.wake_around_filled(world, n);
                        continue;
                    }
                }
            }

            if busy {
                self.next.insert(c);
            } else if warming {
                // Slow change, far from any change of state: the slow cadence.
                let per_step = heat_dt.abs() * HEAT_EVERY as f64 / heat_step;
                let ph = world.planet.mats.phases[m as usize];
                let near_point = ph.up.iter().chain(ph.down.iter()).flatten().any(|ch| (ch.at_k - t).abs() < SLOW_MARGIN_K);
                if !self.sees(c) {
                    self.far.insert(c);
                } else if per_step < SLOW_DT_K && !near_point && !self.latent.contains_key(&c) {
                    self.slow.insert(c);
                } else {
                    self.warm.insert(c);
                }
            } else if tracked && (t - self.start_k(world, c, m)).abs() < phys.settle_k {
                self.temps.remove(&c);
            }
        }
        changes
    }
}

/// What `open_demo` dug.
pub struct DemoScene {
    pub magma: Cell,
    pub water: Cell,
    /// A point inside the cavity dug under the surface rubble.
    pub cavity: Cell,
    pub opened: usize,
}

/// Set up the sandbox demo on a body with magma and water pockets and a
/// rubble blanket: a 3 m corridor between the closest magma and water cells,
/// and a cavity under the rubble on top. Used by the demo tool and the tests.
pub fn open_demo(w: &mut World, sb: &mut Sandbox) -> DemoScene {
    let mats = w.planet.mats.clone();
    let id = |n: &str| mats.id(n).unwrap();
    let r = w.planet.max_radius as i64 + 2;
    let cells_of = |w: &World, m| {
        let mut v = Vec::new();
        for y in (-r..=r).step_by(2) {
            for x in (-r..=r).step_by(2) {
                if w.material(x, y) == m {
                    v.push((x, y));
                }
            }
        }
        v
    };
    let (ms, ws) = (cells_of(w, id("magma")), cells_of(w, id("water")));
    assert!(!ms.is_empty() && !ws.is_empty(), "this body needs magma and water pockets");
    let mut best = (ms[0], ws[0], i64::MAX);
    for &a in &ms {
        for &b in &ws {
            let d = (a.0 - b.0).pow(2) + (a.1 - b.1).pow(2);
            if d < best.2 {
                best = (a, b, d);
            }
        }
    }
    let (magma, water) = (best.0, best.1);
    let mut opened = 0;
    let mut dig = |w: &mut World, sb: &mut Sandbox, x: i64, y: i64| {
        let m = w.material(x, y);
        if m != VACUUM && mats.get(m).state == State::Solid {
            w.mine_cell(x, y);
            sb.wake_around((x, y));
            opened += 1;
        }
    };
    let steps = (water.0 - magma.0).abs().max((water.1 - magma.1).abs()).max(1);
    for s in 0..=steps {
        let x = magma.0 + (water.0 - magma.0) * s / steps;
        let y = magma.1 + (water.1 - magma.1) * s / steps;
        for dy in -1..=1 {
            for dx in -1..=1 {
                dig(w, sb, x + dx, y + dy);
            }
        }
    }
    let top = (0..=r).rev().find(|&y| w.material(0, y) != VACUUM).unwrap();
    for y in top - 30..top - 8 {
        for x in -14..=14 {
            dig(w, sb, x, y);
        }
    }
    DemoScene { magma, water, cavity: (0, top - 20), opened }
}
