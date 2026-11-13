//! The swarm: the seed swarm, its drones, the economy loop and launch.
//!
//! Drones run on events: each one is only processed when it arrives, finishes
//! cutting, or delivers. The order of events is (tick, drone id), so the run is
//! identical every time. Logic ticks at 10 per second of game time.
//!
//! M3 simplifications, stated plainly:
//! - Drones fly in straight lines at their speed; tunnels don't constrain them yet.
//! - Each trip cuts one cell (one cubic metre) and carries it home.
//! - A drone that wears out at a delivery point is refined on the spot: its body
//!   returns as Mass, its empty tank as no energy.
//! - Refiners don't wear out.

use crate::economy::Economy;
use crate::materials::{MatId, State, VACUUM};
use crate::bodies::Bodies;
use crate::sandbox::{Change, Sandbox};
use crate::noise::hash;
use crate::flow::FlowField;
use crate::world::World;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

#[path = "specialists.rs"]
mod specialists;
pub use specialists::{Skills, UnitView};

/// A small, fast, deterministic hash for cell coordinates. The standard one is
/// built to resist attacks, which costs time we don't need to spend here.
#[derive(Default)]
pub struct CellHasher(u64);
impl Hasher for CellHasher {
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 ^ *b as u64).wrapping_mul(0x100_0000_01b3);
        }
    }
    fn write_i64(&mut self, v: i64) {
        self.0 = (self.0.rotate_left(26) ^ v as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
    fn write_u64(&mut self, v: u64) {
        self.write_i64(v as i64);
    }
    fn finish(&self) -> u64 {
        self.0 ^ (self.0 >> 29)
    }
}
pub type Fast = BuildHasherDefault<CellHasher>;
pub type CellMap<V> = HashMap<(i64, i64), V, Fast>;
pub type CellSet = HashSet<(i64, i64), Fast>;

/// A set of cells stored as one bit per cell in 64 x 64 tiles: far smaller
/// and quicker than a hash set when the set is large and clustered (all the
/// open space the swarm has made or broken into). Membership only, no order.
#[derive(Default)]
pub struct CellBits {
    tiles: HashMap<(i64, i64), Box<[u64; 64]>, Fast>,
}

impl CellBits {
    fn at(c: (i64, i64)) -> ((i64, i64), usize, u64) {
        ((c.0.div_euclid(64), c.1.div_euclid(64)), c.1.rem_euclid(64) as usize, 1u64 << c.0.rem_euclid(64))
    }
    pub fn contains(&self, c: &(i64, i64)) -> bool {
        let (t, row, bit) = Self::at(*c);
        self.tiles.get(&t).is_some_and(|b| b[row] & bit != 0)
    }
    /// True if it wasn't there before.
    pub fn insert(&mut self, c: (i64, i64)) -> bool {
        let (t, row, bit) = Self::at(c);
        let b = self.tiles.entry(t).or_insert_with(|| Box::new([0u64; 64]));
        let was = b[row] & bit != 0;
        b[row] |= bit;
        !was
    }
    /// True if it was there.
    pub fn remove(&mut self, c: &(i64, i64)) -> bool {
        let (t, row, bit) = Self::at(*c);
        match self.tiles.get_mut(&t) {
            Some(b) => {
                let was = b[row] & bit != 0;
                b[row] &= !bit;
                was
            }
            None => false,
        }
    }
}

/// A set of cells that remembers the order they were added in and empties all
/// at once: a list plus a bit-tile set to keep out repeats. For the sandbox's
/// per-tick sets, which are filled, read and emptied over and over.
#[derive(Default)]
pub struct CellQueue {
    list: Vec<(i64, i64)>,
    bits: CellBits,
}

impl CellQueue {
    pub fn insert(&mut self, c: (i64, i64)) -> bool {
        let new = self.bits.insert(c);
        if new {
            self.list.push(c);
        }
        new
    }
    pub fn contains(&self, c: &(i64, i64)) -> bool {
        self.bits.contains(c)
    }
    pub fn len(&self) -> usize {
        self.list.len()
    }
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
    pub fn iter(&self) -> std::slice::Iter<'_, (i64, i64)> {
        self.list.iter()
    }
    /// Everything in it, emptying it.
    pub fn drain(&mut self) -> Vec<(i64, i64)> {
        self.bits.tiles.clear();
        std::mem::take(&mut self.list)
    }
}

const BUCKET: i64 = 16;
/// How far over the highest ground haulers fly on long trips (m).
const HAULER_CLEAR_M: f64 = 40.0;
/// How many times faster haulers fly round the body, high over the ground,
/// than climbing and coming down.
pub const HAULER_CRUISE_X: f64 = 4.0;
/// Scouts, Refiners and Tunnelers are parked while drone behaviour is rebuilt
/// around flocks and haulers (they come back in the economy pass): the
/// scripted player doesn't unlock them.
const LEGACY_UNITS: bool = false;
/// How hot an explosion leaves the ground it reaches (kelvin).
const BLAST_K: f64 = 1200.0;
/// The longest a new cutter takes to drop from the mother swarm (seconds).
const DROP_S: f64 = 20.0;
/// The steepest the ground is taken to rise between two samples (for
/// finding the highest point without looking at every metre).
const SLOPE: f64 = 0.3;
/// At most this many candidate cells are weighed per target choice.
const CANDIDATES: usize = 48;
/// The director hands out the pool of drones out of work this often (s).
const POOL_EVERY_S: f64 = 5.0;
/// Drones a flock has work for per face cell it can reach (each face takes
/// a drone at a time, and drones are also flying to and from it).
const DRONES_PER_FACE: f64 = 1.5;
/// With at least this many drones left in the pool after handing out, the
/// director opens new flocks on faces no flock works (at most
/// NEW_FLOCKS_PER_ROUND a round, at least NEW_FLOCK_GAP_M from any other).
const POOL_OPEN: usize = 200;
/// Most drones one flock gets from the pool in a round.
const POOL_SEND_MAX: u32 = 60;
const NEW_FLOCKS_PER_ROUND: usize = 4;
const NEW_FLOCK_GAP_M: f64 = 200.0;
/// Most flocks the director will open.
const MAX_FLOCKS: usize = 3000;
/// Each flock's flow field is swept again this often (s), and at once if
/// its hauler has moved more than FIELD_DRIFT_M; a field swept within that
/// of where a search starts is used for it. (Ground dug since a sweep is
/// open space the field doesn't know yet: cells beside it are still
/// reached through the squares next to it, see `FlowField::reach_m`.)
const FIELD_EVERY_S: u64 = 15;
const FIELD_DRIFT_M: f64 = 12.0;
/// How high over the ground a flock's field reaches (m).
const FIELD_SKY_M: f64 = 40.0;
/// How long the swarm director remembers a flock's waits and deliveries (s).
const DIRECTOR_MEMORY_S: f64 = 180.0;
/// How many of the best cells a drone checks it can see before giving up.
const LOS_TRIES: usize = 16;
/// Metres of solid ground a drone's short hop from the cell it just cut to
/// the next may cross (bumps and ribs it slips round). Flights from the
/// hauler follow the flock's flow field instead.
const LOS_SOLID: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubKind {
    Seed,
    Refiner,
    /// A staging point a Tunneler leaves down its shaft, so the flock can work deep.
    Shaft,
}

#[derive(Debug, Clone)]
pub struct Hub {
    pub x: f64,
    pub y: f64,
    pub kind: HubKind,
    /// For the seed swarm's landing points: which point of the orbit ring.
    pub ring: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
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
struct Drone {
    alive: bool,
    home: usize,
    phase: Phase,
    life_left: f64,
    trip_start: u64,
    /// Where it last dug: it looks for its next cell near there.
    last: Option<(i64, i64)>,
    /// Searches in a row that found nothing: it waits longer after each.
    fails: u8,
    /// When the current flight leg began (also what the leg cost in energy).
    leg_start: u64,
    /// kg in the hold, and what it is worth credited.
    hold: f64,
    hold_cr: f64,
    /// Energy carried (E).
    tank: f64,
    /// Its temperature inside (kelvin).
    heat_k: f64,
}

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

/// What a load is made of, roughly: up to six materials and their kg (the
/// least of them dropped as others come), for drawing cargo in its colours.
pub type Mix6 = [(crate::materials::MatId, f32); 6];

/// Add `kg` of material `m` to a mix.
pub fn mix_add(mix: &mut Mix6, m: crate::materials::MatId, kg: f32) {
    if kg <= 0.0 {
        return;
    }
    if let Some(e) = mix.iter_mut().find(|e| e.1 > 0.0 && e.0 == m) {
        e.1 += kg;
        return;
    }
    let (i, least) = mix.iter().enumerate().fold((0, f32::MAX), |b, (i, e)| if e.1 < b.1 { (i, e.1) } else { b });
    if least < kg {
        mix[i] = (m, kg);
    }
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

/// The mother swarm: one long cloud high over the ground, spreading both
/// ways around the body from where it arrived as it grows.
#[derive(Debug, Clone, Copy)]
pub struct Mother {
    /// The direction of its middle (radians, as atan2), and how far it
    /// reaches either side (radians; pi covers the whole body).
    pub centre: f64,
    pub half: f64,
    /// Mass put into growing it (kg).
    pub grown_kg: f64,
    /// Its distance from the body's centre: a circle mother_alt_m above the
    /// highest ground. It comes down as the highest ground is eaten away.
    pub radius: f64,
    /// Mass handed back to the swarm by coming down (kg).
    pub returned_kg: f64,
}

/// What a flock has been doing lately (decaying averages), for the swarm's
/// decisions: where its cutters cut, how fast, and how hard the going is.
#[derive(Debug, Clone, Copy, Default)]
pub struct FlockStats {
    /// Weighted sum of where recent cuts were, and the weight.
    sx: f64,
    sy: f64,
    w: f64,
    /// kg cut lately, and seconds spent cutting it.
    kg: f64,
    dig_s: f64,
    /// Searches lately that found nothing to cut.
    dry: f64,
    /// Seconds since this flock last split, moved help, or was made.
    calm_s: f64,
    /// Seconds its spare hauler has stood empty.
    idle_hauler_s: f64,
    /// Merged into another flock, or left with no cutters: no longer used.
    dead: bool,
    /// Seconds it has had no cutters.
    empty_s: f64,
    /// For the swarm director (see `direct`), over the last few minutes
    /// (each fades by DIRECTOR_MEMORY_S): cutter-seconds spent waiting with a
    /// full hold and nowhere to put it, and kg handed to its haulers; and its
    /// haulers' round trip to the mother swarm and back (seconds, smoothed;
    /// 0 until one has made it).
    wait_cs: f64,
    filled: f64,
    trip_s: f64,
    /// Haulers the director wants it to have.
    want: u32,
}

/// Something blew up: where, and how big (metres across), for the screen.
#[derive(Debug, Clone, Copy)]
pub struct Boom {
    pub x: f64,
    pub y: f64,
    pub size_m: f32,
    /// A sapper's charge going off underground: a flash, a shock ring and
    /// the heat it leaves, rather than a fireball in the open.
    pub buried: bool,
}

/// Something worth a look happened (for the camera director).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Happening {
    /// A full hauler lifting off for the mother swarm.
    HaulerLift,
    /// A flock set down where there was none: dropped by the mother swarm,
    /// or split off another.
    NewFlock,
    Merge,
    Explosion,
    /// Cutters broke into a cave, or into lava or a sea.
    Breach,
    /// Rich ore being cut.
    RichOre,
    /// Drones lost (burned, crushed, run dry).
    Losses,
}

/// Angle between two directions (radians, 0 to pi).
fn ang_dist(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(std::f64::consts::TAU);
    d.min(std::f64::consts::TAU - d)
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

/// How high (distance from the centre) a hauler flies from `from` to `to`:
/// a long way, clear over the highest ground (`top`); a short hop, a
/// little above the higher end.
pub fn route_cruise(from: (f64, f64), to: (f64, f64), top: f64) -> f64 {
    let (r0, r1) = ((from.0 * from.0 + from.1 * from.1).sqrt(), (to.0 * to.0 + to.1 * to.1).sqrt());
    let arc = ang_dist(from.1.atan2(from.0), to.1.atan2(to.0)) * r0.max(r1);
    if arc > 60.0 {
        r0.max(r1).max(top + HAULER_CLEAR_M)
    } else {
        r0.max(r1) + 10.0
    }
}

/// The three parts of a hauler's way: straight up to `cruise`, round the
/// body at that height, straight down. Lengths (m).
fn route_parts(from: (f64, f64), to: (f64, f64), cruise: f64) -> (f64, f64, f64, f64, f64) {
    let (r0, r1) = ((from.0 * from.0 + from.1 * from.1).sqrt(), (to.0 * to.0 + to.1 * to.1).sqrt());
    let (a0, a1) = (from.1.atan2(from.0), to.1.atan2(to.0));
    let mut da = a1 - a0;
    while da > std::f64::consts::PI {
        da -= std::f64::consts::TAU;
    }
    while da < -std::f64::consts::PI {
        da += std::f64::consts::TAU;
    }
    ((cruise - r0).max(0.0), cruise * da.abs(), (cruise - r1).max(0.0), a0, da)
}

/// Length (m) of a hauler's way from `from` to `to` flying at `cruise`, as
/// metres at climbing speed (the way round counts less: it is flown faster).
pub fn route_len(from: (f64, f64), to: (f64, f64), cruise: f64) -> f64 {
    let (up, across, down, _, _) = route_parts(from, to, cruise);
    up + across / HAULER_CRUISE_X + down
}

/// Where a hauler is a share `k` (0 to 1) of the way along.
pub fn route_at(from: (f64, f64), to: (f64, f64), cruise: f64, k: f64) -> (f64, f64) {
    let (up, across, down, a0, da) = route_parts(from, to, cruise);
    let across = across / HAULER_CRUISE_X;
    let total = up + across + down;
    if total < 1e-9 {
        return to;
    }
    let s = k.clamp(0.0, 1.0) * total;
    if s <= up {
        let u = (from.0 * from.0 + from.1 * from.1).sqrt().max(1e-9);
        let r = u + s;
        return (from.0 / u * r, from.1 / u * r);
    }
    if s <= up + across {
        let a = a0 + da * ((s - up) / across.max(1e-9));
        return (a.cos() * cruise, a.sin() * cruise);
    }
    let v = (to.0 * to.0 + to.1 * to.1).sqrt().max(1e-9);
    let r = v + (total - s);
    (to.0 / v * r, to.1 / v * r)
}

#[derive(Debug, Clone, Default)]
pub struct Sample {
    pub t: f64,
    pub harvesters: u32,
    pub packet: u64,
    pub scouts: u32,
    pub refiners: u32,
    pub on_trips: u32,
    pub energy: f64,
    pub mass_available: f64,
    pub credited_total: f64,
    pub consumed: f64,
    pub mass_rate: f64,
    pub avg_haul_m: f64,
    pub hubs: u32,
    pub brownout_share: f64,
}

pub struct Sim {
    pub world: World,
    pub eco: Economy,
    pub tick: u64,
    pub energy: f64,
    /// Mass delivered and credited, minus everything spent.
    pub mass: f64,
    /// Elements delivered to the mother swarm, kept to build cutters and
    /// haulers from (see recipes.rs), and those on their way to it.
    pub stock: crate::recipes::Stock,
    pub transit: crate::recipes::Transit,
    /// Parts made from the stockpile and not yet built into anything (see
    /// recipes.rs).
    pub parts: crate::recipes::Parts,
    /// The fabricator, the factory's bays and the director's orders (see
    /// production.rs).
    pub prod: crate::production::Production,
    /// The same by material (kg, by material id), for showing what was
    /// brought up in its own colours: on its way, and delivered lately
    /// (fading, a few minutes' worth).
    transit_mats: Vec<f64>,
    transit_mats_total: f64,
    /// What each flock has been cutting lately (for its haulers' cargo).
    flock_mix: Vec<Mix6>,
    recent_mats: Vec<f64>,
    /// All delivered, by material, ever (kg): what the swarm has collected.
    pub collected_mats: Vec<f64>,
    /// Drones the flocks had room for at the pool's last round (economy on:
    /// no more are built than there is work for).
    pub last_room: u64,
    /// kg taken out of the stockpile to build with, all told.
    pub stock_spent: f64,
    /// What the refinery's fuser has made.
    pub fuser: crate::fuser::Fuser,
    pub delivered_credited: f64,
    pub spent_mass: f64,
    pub recovered_mass: f64,
    pub levels: Vec<u32>,
    pub scout_unlocked: bool,
    pub refiner_unlocked: bool,
    pub scouts: u32,
    pub hubs: Vec<Hub>,
    ring: Vec<(f64, f64)>,
    /// Direction of each orbit point, and the landing point dropped there (if any).
    ring_dir: Vec<(f64, f64)>,
    ring_hub: Vec<Option<usize>>,
    /// The landing point under the seed swarm right now: new drones start here.
    orbit_hub: usize,
    /// Drones based at each delivery point, so work can be shared out evenly.
    hub_load: Vec<u32>,
    /// Hubs where a search just found nothing free: drones don't search
    /// there again until free work appears within reach (see `work_near`).
    hub_dry: Vec<bool>,
    /// Bumped whenever a hub opens, dries up, ends or appears.
    hub_gen: u64,
    /// Drones waiting for work because every hub was dry (each at its own hub).
    parked: Vec<u32>,
    /// The haulers, and per flock the cutters waiting to unload.
    pub haulers: Vec<Hauler>,
    site_wait: Vec<Vec<u32>>,
    /// Cutters lost when their energy ran out, and haulers broken down.
    pub starved: u64,
    pub haulers_lost: u64,
    /// kg of cargo burned by drones for their own energy.
    pub fuel_kg: f64,
    pub mother: Mother,
    /// The ground's height in evenly spaced directions, as last checked
    /// (it only goes down), for finding the highest point.
    peaks: Vec<f64>,
    /// Around the highest of those, the highest ground found between each
    /// direction and the next (NaN: not looked at yet).
    fine_peaks: Vec<f64>,
    /// Where the sweep over `peaks` has got to, and how many sweeps done.
    peaks_cursor: usize,
    pub peaks_version: u64,
    flock: Vec<FlockStats>,
    /// Flocks split off, merged, and cutters sent to help.
    pub splits: u64,
    pub merges: u64,
    pub helpers_sent: u64,
    pub haulers_retired: u64,
    /// Explosions since the screen last looked.
    booms: Vec<Boom>,
    /// Happenings since the screen last looked (at most a few hundred).
    happenings: Vec<(Happening, f64, f64)>,
    pub explosions: u64,
    drones: Vec<Drone>,
    events: EventQueue,
    /// Diggable cells: what they are made of, and the smell of ore behind them.
    frontier: CellMap<Face>,
    /// The dig face again, by 16 m bucket, with each cell's details, for the
    /// target search (kept exactly in step with `frontier`).
    buckets: CellMap<Vec<((i64, i64), Face)>>,
    /// Dig-face cells that are liquid (lava on the face).
    liquid_faces: usize,
    /// Scratch space for the target search, reused between searches.
    cands: Vec<((i64, i64), f64, f64)>,
    /// Drones currently headed to each cell (up to four, one per face).
    reserved: CellMap<u8>,
    open: CellBits,
    /// Open cells known to be outside the body (not natural voids).
    outside: CellBits,
    /// Landing points over open liquid, where no drones are dropped.
    wet_hubs: std::collections::BTreeSet<usize>,
    next_free_build: u64,
    avg_haul: f64,
    last_far_cell: Option<(i64, i64)>,
    brownout_ticks: u64,
    window_mined: f64,
    window_brownout: u64,
    pub samples: Vec<Sample>,
    pub launched_at: Option<f64>,
    pub seed_mass: f64,
    pub harvesters_built: u64,
    pub deaths: u64,
    /// Why cells in reach were passed over in searches: too far, taken,
    /// unreachable through open space, not worth it; and taken (diagnostic).
    pub passed: [std::sync::atomic::AtomicU64; 5],
    /// Directions (milliradians) where open_flocks has surveyed new ground.
    zones_opened: std::collections::BTreeSet<i64>,
    /// Worn-out cutters taken aboard their hauler as scrap.
    pub recycled: u64,
    /// Cutters built because the flocks had room for them.
    pub cutters_built_for_room: u64,
    pub purchases: Vec<(f64, String)>,
    /// Harvesters beyond the individual cap, working as one packet.
    pub packet: u64,
    packet_budget: f64,
    packet_deaths: f64,
    packet_hub: usize,
    /// Calibration: kg per second delivered by one individual Harvester.
    pub rate_per_drone: f64,
    window_delivered: f64,
    pub packet_leaves: u64,
    packet_last: Option<(i64, i64)>,
    alive_count: u32,
    on_trip_count: u32,
    /// Times a flock moved its hauler away from lava.
    pub site_moves: u64,
    /// Each flock's flow field (see `flow`), by hub, and a count bumped
    /// whenever one is rebuilt (for the screen).
    pub fields: Vec<Option<std::sync::Arc<FlowField>>>,
    pub fields_version: u64,
    /// Work each flock has room for (see `free_faces`), counted with its
    /// field; drones in the pool by the flock they last belonged to; and
    /// when the pool was last handed out.
    capacity: Vec<u32>,
    pooled_at: Vec<u32>,
    pub pool_sent: u64,
    pool_noted: u64,
    pub flocks_opened: u64,
    pub flow_s: f64,
    pub sweeps: u64,
    /// The swarm director's last few calls (see `direct`), haulers it has
    /// built, and when each flock may next note a call it didn't act on.
    pub director_log: std::collections::VecDeque<String>,
    pub haulers_built: u64,
    director_quiet: Vec<u64>,
    /// How hot each liquid was when last blasted out of a vent (0: never).
    eject_k: Vec<f64>,
    /// Times the dead were cleared from the drone list, and how many dead
    /// there must be first.
    pub compactions: u64,
    pub compact_min: usize,
    /// kg already cut from cells that take several trips.
    progress: CellMap<f64>,
    /// Starting advantages bought with Seed Mass.
    pub perks: Vec<String>,
    perk_cargo: f64,
    perk_speed: f64,
    perk_dig: f64,
    perk_energy: f64,
    /// When the launch threshold was reached; the player launches at their next check-in.
    pub ready_at: Option<f64>,
    pub check_ins: u32,
    /// Cargo multiplier from upgrades and perks, kept current for target choice.
    up_cached_cargo: f64,
    /// How well the swarm sees each cell's effort and value right now (0 = blind, 1 = exact).
    pub sense_effort: f64,
    pub sense_value: f64,
    /// Falling sand, heat and reactions.
    pub sandbox: Sandbox,
    /// Loose rigid bodies: rock cut free, and wrecked drones.
    pub bodies: Bodies,
    /// Drones wrecked by lava or heat, and crushed by falling rock.
    pub burned: u64,
    pub crushed: u64,
    /// Which drones are out working each cell (so falling rock can find them).
    working: CellMap<Vec<u32>>,
    /// Loads of lava swallowed by drones.
    pub lava_swallowed: u64,
    pub tunneler_unlocked: bool,
    tunnelers: Vec<Bore>,
    /// Rock cracked by a Tunneler's passing: quicker to cut, and no depth penalty.
    fractured: CellSet,
    /// Loose debris (blast rubble, thrown and fallen rock): cutters pick it
    /// up in `harvester.loose_s` a cell, whatever it is.
    loose: CellSet,
    /// Materials that are always loose (dead plant matter).
    loose_mat: Vec<bool>,
    /// Melt from sapper blasts, cooling: (tick it turns to glass, cell).
    melt: std::collections::VecDeque<(u64, (i64, i64))>,
    /// Loose rubble and melt that may still move: it falls, slides down
    /// slopes, and melt runs sideways into cracks (the proving ground's
    /// settling), a cell or two a tick.
    settling: CellSet,
    /// Loose cells out of sight that would settle: they wait, and settle
    /// once the view comes over them (as sand does).
    settle_wait: CellSet,
    /// Broken rock in the air on its way out of a plume, thrown a little at
    /// a time: (tick, from, material, velocity).
    plume_out: std::collections::VecDeque<(u64, (f64, f64), MatId, (f64, f64))>,
    /// Sappers caught in a blast, going off a moment later: (tick, where).
    chain: Vec<(u64, (f64, f64), f64)>,
    next_tunnel_k: usize,
    /// Metres of shaft bored, shafts finished, and kg the drills cut.
    pub tunnel_m: u64,
    pub tunnels_done: u64,
    pub tunnel_kg: f64,
    /// What this body offers the specialists, and the specialists out (see
    /// specialists.rs).
    pub skills: Skills,
    units: specialists::Units,
    /// Profiling (native builds): seconds spent in each part of a step, named
    /// in PROF_NAMES. The first four add up to the whole step; the rest are
    /// parts of those.
    pub prof: [f64; 8],
    /// (Profiling) finer timings: break_free_big, break_free, bodies step,
    /// rock landed, sandbox step, sandbox changes applied, blast tick, the rest.
    pub prof2: [f64; 12],
    /// How often things happened, named in COUNT_NAMES.
    pub counts: [u64; 6],
    /// Economy off (a test mode for the game): drones cost nothing and the
    /// swarm grows to this many, all simulated one by one; no purchases, no
    /// launch. None (the default) is the real game. Not saved or hashed.
    pub free_drones: Option<u64>,
    /// For a renderer keeping its own copy of every drone's flight leg: the
    /// drones whose leg changed since it last looked. None keeps no record.
    pub leg_track: Option<LegTrack>,
    /// The plants' life cycle (src/life.rs), if anything lives here.
    pub life: Option<crate::life::Life>,
    /// Sapper plumes since the screen last looked: where the broken rock
    /// blasts out of the ground, and how hard (for the dust column).
    pub plumes: Vec<(f64, f64, f32)>,
}

#[cfg(not(target_arch = "wasm32"))]
fn clock() -> f64 {
    use std::sync::OnceLock;
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
}
#[cfg(target_arch = "wasm32")]
fn clock() -> f64 {
    0.0
}

/// cos and sin of 2*pi/n from their power series: plain arithmetic, so the same
/// on every machine (library trig functions are not guaranteed to be).
fn turn(n: u32) -> (f64, f64) {
    let a = 2.0 * std::f64::consts::PI / n as f64;
    let (mut c, mut s, mut term_c, mut term_s) = (0.0, 0.0, 1.0, a);
    for k in 0..12 {
        c += term_c;
        s += term_s;
        let k2 = (2 * k + 1) as f64;
        term_c *= -a * a / (k2 * (k2 + 1.0));
        term_s *= -a * a / ((k2 + 1.0) * (k2 + 2.0));
    }
    (c, s)
}

/// One Tunneler and its shaft.
#[derive(Debug, Clone)]
struct Bore {
    /// Drill head and heading (a unit vector).
    x: f64,
    y: f64,
    hx: f64,
    hy: f64,
    /// The point it is steering toward.
    aim: (f64, f64),
    length: f64,
    /// Drilling time banked toward the next metre.
    progress_s: f64,
    /// Boring now, or pulling out and moving until `resume_at`.
    active: bool,
    resume_at: u64,
    /// Where its head was a second ago, and when it moved on (for the
    /// screen; not saved).
    prev: (f64, f64),
    moved_at: u64,
}

/// cos and sin of an angle in degrees, from their power series (plain
/// arithmetic, the same on every machine).
fn cos_sin_deg(deg: f64) -> (f64, f64) {
    let a = deg * std::f64::consts::PI / 180.0;
    let (mut c, mut s, mut tc, mut ts) = (0.0, 0.0, 1.0, a);
    for k in 0..12 {
        c += tc;
        s += ts;
        let k2 = (2 * k + 1) as f64;
        tc *= -a * a / (k2 * (k2 + 1.0));
        ts *= -a * a / ((k2 + 1.0) * (k2 + 2.0));
    }
    (c, s)
}

fn unit(x: f64, y: f64) -> (f64, f64) {
    let l = (x * x + y * y).sqrt().max(1e-12);
    (x / l, y / l)
}

fn dist2(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0) * (a.0 - b.0) + (a.1 - b.1) * (a.1 - b.1)
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// A cell on the dig face, as the swarm knows it.
#[derive(Debug, Clone, Copy)]
struct Face {
    mat: MatId,
    /// Average extra worth (multiplier above 1) of the rock within a few metres
    /// behind this cell, and further out (only Scouts smell that far).
    near: f32,
    far: f32,
}

/// Directions sampled when smelling for ore: 8 compass points (unit-ish steps).
const NOSE: [(i64, i64); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];

/// Smell the rock around cell c: average extra worth of solid rock at 3 and 6 m
/// (near) and, for Scouts, at 12 and 20 m (far).
fn scent(world: &World, c: (i64, i64), far: bool) -> (f32, f32) {
    let rd = std::cell::RefCell::new(world.reader());
    let worth = |r: i64| -> f64 {
        NOSE.iter()
            .map(|(dx, dy)| {
                let mat = world.planet.mats.get(rd.borrow_mut().get(c.0 + dx * r, c.1 + dy * r));
                if mat.state == State::Solid { (mat.multiplier - 1.0).max(0.0) } else { 0.0 }
            })
            .sum::<f64>()
            / NOSE.len() as f64
    };
    let near = (worth(3) + worth(6)) / 2.0;
    let far = if far { (worth(12) + worth(20)) / 2.0 } else { 0.0 };
    (near as f32, far as f32)
}



impl Sim {
    pub fn new(world: World, eco: Economy) -> Self {
        Self::with_perks(world, eco, &[])
    }

    /// Start a body with starting advantages bought with Seed Mass.
    pub fn with_perks(world: World, eco: Economy, perks: &[String]) -> Self {
        let mut sim = Self::blank(world, eco);
        // The fleet's storage ship brings refined elements to build from.
        if sim.prod.on() {
            for (s, kg) in &sim.eco.production.start_refined {
                if let Some(i) = sim.world.planet.mats.elements.index(s) {
                    sim.stock.kg[i] += kg;
                    sim.stock.refined[i] += kg;
                }
            }
            sim.energy += sim.eco.production.start_energy;
        }
        sim.build_ring();
        sim.mother_work();
        sim.seed_frontier();
        let mut extra = 0;
        for name in perks.iter() {
            let p = sim.eco.perk.iter().find(|p| &p.name == name).expect("unknown perk").clone();
            extra += p.start_extra;
            sim.perks.push(name.clone());
        }
        sim.apply_perks();
        sim.up_cached_cargo = sim.perk_cargo;
        sim.update_sense();
        for _ in 0..sim.eco.seed_swarm.start_harvesters + extra {
            sim.spawn_harvester();
        }
        sim.next_free_build = sim.ticks(sim.eco.seed_swarm.free_build_every_s);
        sim
    }

    /// The perks' multipliers, from their names.
    fn apply_perks(&mut self) {
        (self.perk_cargo, self.perk_speed, self.perk_dig, self.perk_energy) = (1.0, 1.0, 1.0, 1.0);
        for name in self.perks.clone() {
            if let Some(p) = self.eco.perk.iter().find(|p| p.name == name) {
                self.perk_cargo *= p.cargo;
                self.perk_speed *= p.speed;
                self.perk_dig *= p.dig;
                self.perk_energy *= p.energy_cost;
            }
        }
    }

    /// A swarm with nothing yet: no ring, no dig face, no drones.
    fn blank(mut world: World, eco: Economy) -> Self {
        let n_up = eco.upgrade.len();
        // Dead plant matter is always loose debris.
        let loose_mat = ["deadwood", "leaf litter", "humus"].iter().fold(vec![false; world.planet.mats.len()], |mut v, n| {
            if let Ok(m) = world.planet.mats.id(n) {
                v[m as usize] = true;
            }
            v
        });
        let life = crate::life::Life::new(&world);
        if let Some(l) = &life {
            world.life_watch = l.watch();
        }
        let sandbox = Sandbox::new(&world);
        let bodies = Bodies::new(&world);
        let stock = crate::recipes::Stock::new(&world.planet.mats.elements);
        let transit = crate::recipes::Transit::new(&world.planet.mats.elements);
        let parts = crate::recipes::Parts::new(&eco.recipe);
        // The Universal Drone Replicator's bays: only for the specialists
        // this body has a use for.
        let skills = Skills::of(&world, eco.units.heatsink.min_freeze_k);
        let mut units = specialists::Units::default();
        units.prepare(&world, eco.units.heatsink.min_freeze_k);
        let extra: Vec<crate::production::BaySpec> = if eco.units.universal_replicator { eco.units.bay.iter().filter(|b| skills.useful(b.unit)).cloned().collect() } else { Vec::new() };
        let prod = if eco.recipe.part.is_empty() { crate::production::Production::default() } else { crate::production::Production::new(&eco.production, &extra) };
        let sim = Sim {
            stock,
            transit,
            parts,
            prod,
            transit_mats: vec![0.0; world.planet.mats.len()],
            transit_mats_total: 0.0,
            flock_mix: Vec::new(),
            recent_mats: vec![0.0; world.planet.mats.len()],
            collected_mats: vec![0.0; world.planet.mats.len()],
            last_room: 0,
            stock_spent: 0.0,
            fuser: crate::fuser::Fuser::default(),
            world,
            energy: eco.seed_swarm.start_energy,
            eco,
            tick: 0,
            mass: 0.0,
            delivered_credited: 0.0,
            spent_mass: 0.0,
            recovered_mass: 0.0,
            levels: vec![0; n_up],
            scout_unlocked: false,
            refiner_unlocked: false,
            scouts: 0,
            hubs: Vec::new(),
            ring: Vec::new(),
            ring_dir: Vec::new(),
            ring_hub: Vec::new(),
            orbit_hub: 0,
            hub_load: Vec::new(),
            hub_dry: Vec::new(),
            hub_gen: 0,
            parked: Vec::new(),
            haulers: Vec::new(),
            site_wait: Vec::new(),
            starved: 0,
            haulers_lost: 0,
            fuel_kg: 0.0,
            mother: Mother { centre: std::f64::consts::FRAC_PI_2, half: 0.0, grown_kg: 0.0, radius: 0.0, returned_kg: 0.0 },
            peaks: Vec::new(),
            fine_peaks: Vec::new(),
            peaks_cursor: 0,
            peaks_version: 0,
            booms: Vec::new(),
            happenings: Vec::new(),
            explosions: 0,
            flock: Vec::new(),
            splits: 0,
            merges: 0,
            helpers_sent: 0,
            haulers_retired: 0,
            drones: Vec::new(),
            events: EventQueue::default(),
            frontier: CellMap::default(),
            buckets: CellMap::default(),
            liquid_faces: 0,
            cands: Vec::new(),
            reserved: CellMap::default(),
            open: CellBits::default(),
            outside: CellBits::default(),
            wet_hubs: Default::default(),
            next_free_build: 0,
            avg_haul: 0.0,
            last_far_cell: None,
            brownout_ticks: 0,
            window_mined: 0.0,
            window_brownout: 0,
            samples: Vec::new(),
            launched_at: None,
            seed_mass: 0.0,
            harvesters_built: 0,
            deaths: 0,
            recycled: 0,
            cutters_built_for_room: 0,
            passed: Default::default(),
            zones_opened: Default::default(),
            purchases: Vec::new(),
            packet: 0,
            packet_budget: 0.0,
            packet_deaths: 0.0,
            packet_hub: 0,
            rate_per_drone: 0.0,
            window_delivered: 0.0,
            packet_leaves: 0,
            packet_last: None,
            alive_count: 0,
            on_trip_count: 0,
            site_moves: 0,
            fields: Vec::new(),
            fields_version: 0,
            capacity: Vec::new(),
            pooled_at: Vec::new(),
            pool_sent: 0,
            pool_noted: 0,
            flocks_opened: 0,
            flow_s: 0.0,
            sweeps: 0,
            director_log: Default::default(),
            haulers_built: 0,
            director_quiet: Vec::new(),
            eject_k: Vec::new(),
            compactions: 0,
            compact_min: 50_000,
            progress: CellMap::default(),
            perks: Vec::new(),
            perk_cargo: 1.0,
            perk_speed: 1.0,
            perk_dig: 1.0,
            perk_energy: 1.0,
            ready_at: None,
            check_ins: 0,
            up_cached_cargo: 1.0,
            sense_effort: 0.0,
            sense_value: 0.0,
            sandbox,
            bodies,
            burned: 0,
            crushed: 0,
            working: CellMap::default(),
            lava_swallowed: 0,
            tunneler_unlocked: false,
            tunnelers: Vec::new(),
            fractured: CellSet::default(),
            loose: CellSet::default(),
            loose_mat,
            melt: Default::default(),
            settling: CellSet::default(),
            settle_wait: CellSet::default(),
            plume_out: Default::default(),
            chain: Vec::new(),
            next_tunnel_k: 0,
            tunnel_m: 0,
            tunnels_done: 0,
            tunnel_kg: 0.0,
            skills,
            units,
            prof: [0.0; 8],
            prof2: [0.0; 12],
            counts: [0; 6],
            free_drones: None,
            leg_track: None,
            life,
            plumes: Vec::new(),
        };
        sim
    }

    fn ticks(&self, s: f64) -> u64 {
        (s / self.eco.tick_s).ceil().max(1.0) as u64
    }

    pub fn time(&self) -> f64 {
        self.tick as f64 * self.eco.tick_s
    }

    fn up(&self, name: &str) -> f64 {
        match self.eco.upgrade.iter().position(|u| u.name == name) {
            Some(i) => crate::noise::powi(self.eco.upgrade[i].effect, self.levels[i] as i32),
            None => 1.0,
        }
    }

    /// Re-read how well the swarm judges cells: the "swarm intelligence" level,
    /// plus a few levels while Scouts are out.
    fn update_sense(&mut self) {
        let it = &self.eco.intelligence;
        let mut level = self.eco.upgrade.iter().position(|u| u.name == "swarm intelligence").map_or(0, |i| self.levels[i]);
        if self.scouts > 0 {
            level += it.scout_bonus_levels;
        }
        let at = |v: &Vec<f64>| v.get(level as usize).or(v.last()).copied().unwrap_or(0.0);
        self.sense_effort = at(&it.effort);
        self.sense_value = at(&it.value);
    }

    // ---- The seed swarm's landing points ----

    fn build_ring(&mut self) {
        let n = self.eco.seed_swarm.hub_ring;
        let (c, s) = turn(n);
        let (mut ux, mut uy) = (0.0f64, 1.0f64);
        for _ in 0..n {
            let r = self.ground_radius(ux, uy, self.world.planet.max_radius + 1.0) + self.eco.seed_swarm.hover_m;
            self.ring.push((ux * r, uy * r));
            self.ring_dir.push((ux, uy));
            self.ring_hub.push(None);
            let (nx, ny) = (ux * c - uy * s, ux * s + uy * c);
            let l = (nx * nx + ny * ny).sqrt();
            ux = nx / l;
            uy = ny / l;
        }
    }

    /// Radius of the first solid cell met marching inward from `from` along
    /// (ux, uy): the actual ground, craters and digging included.
    fn ground_radius(&self, ux: f64, uy: f64, from: f64) -> f64 {
        let mut r = from;
        while r > 0.0 && self.world.material_bare((ux * r).floor() as i64, (uy * r).floor() as i64) == VACUUM {
            r -= 1.0;
        }
        r.max(0.0)
    }

    /// The mother swarm grows with the mass put into it, spreading both ways
    /// from where it arrived. Each landing point of the ring it comes over
    /// gets a flock (and a hauler), and the ground there opens up. New
    /// drones go to the flock on land with the fewest.
    fn mother_work(&mut self) {
        self.lower_mother();
        let ss = &self.eco.seed_swarm;
        let len = ss.mother_start_m + self.mother.grown_kg / ss.mother_kg_per_m.max(1e-9);
        self.mother.half = (len / (2.0 * self.mother.radius.max(1.0))).min(std::f64::consts::PI);
        for k in 0..self.ring.len() {
            if self.ring_hub[k].is_some() {
                continue;
            }
            let (ux, uy) = self.ring_dir[k];
            if ang_dist(uy.atan2(ux), self.mother.centre) > self.mother.half {
                continue;
            }
            let (x, y) = self.ring[k];
            self.hubs.push(Hub { x, y, kind: HubKind::Seed, ring: Some(k) });
            self.hub_load.push(0);
            self.hub_dry.push(false);
            self.hub_gen += 1;
            self.site_wait.push(Vec::new());
            self.flock.push(FlockStats::default());
            self.ring_hub[k] = Some(self.hubs.len() - 1);
            // The mother swarm drops a hauler for the new flock.
            self.send_hauler(self.hubs.len() - 1);
            self.happen(Happening::NewFlock, (x, y));
            // On a big body, this is where the ground first opens up.
            if !self.surveyed_whole() {
                let solid = self.survey_zone(ux, uy);
                // Over open liquid (a sea) there is nothing to dig yet.
                if solid == 0 {
                    self.wet_hubs.insert(self.hubs.len() - 1);
                    self.hub_gen += 1;
                }
            }
        }
        if let Some(best) = (0..self.hubs.len()).filter(|i| !self.flock[*i].dead && !self.hub_dry[*i] && !self.wet_hubs.contains(i)).min_by_key(|&i| (self.hub_load[i], i)) {
            self.orbit_hub = best;
        } else if !self.hubs.is_empty() {
            self.orbit_hub = self.orbit_hub.min(self.hubs.len() - 1);
        }
    }

    /// The ground's height (distance from the centre) in evenly spaced
    /// directions round the body, the first from +x, counter-clockwise, as
    /// of the last sweep (for the screen's backdrop).
    pub fn ground_heights(&self) -> &[f64] {
        &self.peaks
    }

    /// The mother swarm's distance from the body's centre: a circle
    /// mother_alt_m above the highest ground anywhere on the body.
    pub fn mother_radius(&self) -> f64 {
        self.mother.radius
    }

    /// The ground's height in direction `a` (radians), looking down from `from`.
    /// At most 256 m at a time: deeper, it carries on from there next time
    /// (only the highest ground matters, and that is near where it looks).
    fn ground_down(&self, a: f64, from: f64) -> f64 {
        let (ux, uy) = (a.cos(), a.sin());
        let mut r = from;
        let stop = (from - 256.0).max(0.0);
        while r > stop && self.world.material_bare((ux * r).floor() as i64, (uy * r).floor() as i64) == VACUUM {
            r -= 1.0;
        }
        r
    }

    /// The highest ground between turns t0 and t1 (fractions of a full turn)
    /// whose ends stand at h0 and h1, looking closer wherever it could still
    /// reach `top`: 8 samples a step, down to about 8 m apart.
    fn refine_peak(&self, t0: f64, t1: f64, h0: f64, h1: f64, r: f64, top: f64) -> f64 {
        let tau = std::f64::consts::TAU;
        let width = (t1 - t0) * tau * r;
        let mut best = h0.max(h1);
        if width <= 8.0 {
            return best;
        }
        const PARTS: usize = 8;
        let mut h = [0.0f64; PARTS + 1];
        h[0] = h0;
        h[PARTS] = h1;
        for (j, hj) in h.iter_mut().enumerate().take(PARTS).skip(1) {
            let a = tau * (t0 + (t1 - t0) * j as f64 / PARTS as f64);
            *hj = self.ground_near(a, self.world.planet.surface_radius(a.cos(), a.sin()));
            best = best.max(*hj);
        }
        let top = top.max(best);
        let sub = width / PARTS as f64;
        for j in 0..PARTS {
            if h[j].max(h[j + 1]) >= top - SLOPE * sub {
                let (a, b) = (t0 + (t1 - t0) * j as f64 / PARTS as f64, t0 + (t1 - t0) * (j + 1) as f64 / PARTS as f64);
                best = best.max(self.refine_peak(a, b, h[j], h[j + 1], r, top));
            }
        }
        best
    }

    /// The ground's height in direction `a`, starting from a guess near it:
    /// up while solid (rough ground stands a few metres proud of the
    /// surface), else down to the ground.
    fn ground_near(&self, a: f64, guess: f64) -> f64 {
        let (ux, uy) = (a.cos(), a.sin());
        let solid = |r: f64| self.world.material_bare((ux * r).floor() as i64, (uy * r).floor() as i64) != VACUUM;
        if solid(guess) {
            let mut r = guess;
            while r < guess + 64.0 && solid(r + 1.0) {
                r += 1.0;
            }
            r
        } else {
            self.ground_down(a, guess)
        }
    }

    /// Bring the mother swarm down to mother_alt_m over the highest ground
    /// left (checked in evenly spaced directions, from where the ground was
    /// last seen down to where it is now). Coming down, it gets shorter for
    /// the same reach around the body: the mass it no longer needs goes
    /// back to the swarm, with the energy it makes.
    fn lower_mother(&mut self) {
        let n = self.survey_n().clamp(1024, 16384) as usize;
        let tau = std::f64::consts::TAU;
        let first = self.peaks.len() != n;
        if first {
            // At first, the untouched surface in each direction.
            self.peaks = (0..n)
                .map(|k| {
                    let a = tau * k as f64 / n as f64;
                    self.ground_near(a, self.world.planet.surface_radius(a.cos(), a.sin()))
                })
                .collect();
            self.fine_peaks = vec![f64::NAN; n];
            self.peaks_cursor = 0;
        }
        // A share of the directions each second, the whole way round in a
        // minute (all at once at first), so it never stalls a step.
        let (k0, k1) = if first { (0, n) } else { (self.peaks_cursor, (self.peaks_cursor + n.div_ceil(60)).min(n)) };
        self.peaks_cursor = if k1 >= n { 0 } else { k1 };
        let mut changed = vec![false; k1 - k0 + 1];
        for k in k0..=k1.min(n - 1) {
            let a = tau * k as f64 / n as f64;
            let r = self.ground_down(a, self.peaks[k]);
            changed[k - k0] = r < self.peaks[k];
            self.peaks[k] = r;
        }
        // The highest ground: the samples, and the closer looks between them.
        let mut top = self.peaks.iter().copied().fold(0.0f64, f64::max);
        for &f in &self.fine_peaks {
            if !f.is_nan() {
                top = top.max(f);
            }
        }
        // Between the directions sampled a narrow peak can stand higher:
        // around the highest ones, look closer, and closer again where the
        // ground there could still reach the top (once, and again wherever
        // the ground there has been dug since).
        let r_top = top.max(1.0);
        let spacing = tau * r_top / n as f64;
        if spacing > 12.0 {
            let mut order: Vec<usize> = (k0..k1).collect();
            // Highest first, so the top found rules out more of the rest.
            order.sort_by(|&a, &b| self.peaks[b].max(self.peaks[(b + 1) % n]).total_cmp(&self.peaks[a].max(self.peaks[(a + 1) % n])));
            for k in order {
                let dug = changed[k - k0] || changed.get(k + 1 - k0).copied().unwrap_or(false);
                if self.peaks[k].max(self.peaks[(k + 1) % n]) < top - SLOPE * spacing {
                    // Too low to matter now (and if dug, its old closer look is stale).
                    if dug {
                        self.fine_peaks[k] = f64::NAN;
                    }
                    continue;
                }
                if self.fine_peaks[k].is_nan() || dug {
                    self.fine_peaks[k] = self.refine_peak(k as f64 / n as f64, (k + 1) as f64 / n as f64, self.peaks[k], self.peaks[(k + 1) % n], r_top, top);
                }
                top = top.max(self.fine_peaks[k]);
            }
        }
        // Come down only once each minute's sweep is done (or at first).
        if !first && self.peaks_cursor != 0 {
            return;
        }
        self.peaks_version += 1;
        // The closer looks may be out of date where digging went on between
        // the samples: look again at the highest few until one holds up.
        if spacing > 12.0 {
            for _ in 0..8 {
                let Some(k) = (0..n).filter(|&k| !self.fine_peaks[k].is_nan()).max_by(|&a, &b| self.fine_peaks[a].total_cmp(&self.fine_peaks[b])) else { break };
                let old = self.fine_peaks[k];
                let coarse_top = self.peaks.iter().copied().fold(0.0f64, f64::max);
                if old <= coarse_top {
                    break;
                }
                let now = self.refine_peak(k as f64 / n as f64, (k + 1) as f64 / n as f64, self.peaks[k], self.peaks[(k + 1) % n], r_top, coarse_top);
                self.fine_peaks[k] = now;
                if now >= old {
                    break;
                }
            }
            top = self.peaks.iter().copied().fold(0.0f64, f64::max);
            for &f in &self.fine_peaks {
                if !f.is_nan() {
                    top = top.max(f);
                }
            }
        }
        let want = top + self.eco.hauler.mother_alt_m;
        let old = self.mother.radius;
        if old <= 0.0 {
            self.mother.radius = want;
            return;
        }
        if want >= old - 1.0 {
            return;
        }
        self.mother.radius = want;
        // The same reach, at the new height, needs less of it.
        let ss = &self.eco.seed_swarm;
        let per_m = ss.mother_kg_per_m.max(1e-9);
        let len = (ss.mother_start_m + self.mother.grown_kg / per_m).min(std::f64::consts::TAU * old);
        let len_new = len * want / old;
        let grown_new = ((len_new - ss.mother_start_m) * per_m).max(0.0).min(self.mother.grown_kg);
        let back = self.mother.grown_kg - grown_new;
        if back > 0.0 {
            self.mother.grown_kg = grown_new;
            self.mother.returned_kg += back;
            self.mass += back;
            self.recovered_mass += back;
            self.energy += back / 1000.0 * ss.processing_energy_per_tonne;
        }
    }

    /// Something blows up at `at`, throwing `cells` cells of drone scrap
    /// about (the drone's own body, seeding the ground near it).
    /// A great blast at `at` (something heavy coming down, say): `size_m`
    /// across. It blows out a crater and throws the ground in it about, and
    /// leaves `scrap` cells of wreckage. For the game to set off.
    pub fn big_boom(&mut self, at: (f64, f64), size_m: f32, scrap: u32) {
        self.explode(at, scrap, size_m);
    }

    /// Blow out a crater of radius `r` at `at`: the ground in it (ragged at
    /// the edge, the hardest rock holding best) thrown out and up, landing
    /// round about as loose matter.
    fn crater(&mut self, at: (f64, f64), r: f64) {
        if r < 1.0 {
            return;
        }
        let c0 = (at.0.floor() as i64, at.1.floor() as i64);
        let ri = r.ceil() as i64 + 1;
        let up = {
            let l = (at.0 * at.0 + at.1 * at.1).sqrt().max(1.0);
            (at.0 / l, at.1 / l)
        };
        let seed = hash(self.tick, c0.0, c0.1, 0xC7A7);
        let mut thrown = 0usize;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                let d = ((dx * dx + dy * dy) as f64).sqrt();
                let c = (c0.0 + dx, c0.1 + dy);
                let m = self.world.material(c.0, c.1);
                if m == VACUUM {
                    continue;
                }
                let mat = self.world.planet.mats.get(m);
                // Ragged edge; hard rock resists.
                let n = crate::noise::unit(hash(seed, dx, dy, 1));
                let reach = r * (0.75 + 0.35 * n) * (1.0 - 0.04 * mat.hardness as f64);
                if d > reach {
                    continue;
                }
                let Some(m) = self.world.lift_cell(c.0, c.1) else { continue };
                self.undope(c, false);
                self.progress.remove(&c);
                self.open_cell(c);
                // Out from the blast and up, faster near its heart.
                let (ox, oy) = if d > 1e-6 { (dx as f64 / d, dy as f64 / d) } else { up };
                let k = 1.0 - d / (r + 1.0);
                let sp = (8.0 + 34.0 * k) * (0.6 + 0.8 * crate::noise::unit(hash(seed, dx, dy, 2)));
                let lift = 0.6 + 0.6 * crate::noise::unit(hash(seed, dx, dy, 3));
                let vel = ((ox + up.0 * lift) * sp, (oy + up.1 * lift) * sp);
                let p = (c.0 as f64 + 0.5, c.1 as f64 + 0.5);
                thrown += 1;
                if let Some(t) = self.bodies.throw(&mut self.world, p, m, vel) {
                    self.scrap_landed(t);
                }
            }
        }
        let _ = thrown;
    }

    fn explode(&mut self, at: (f64, f64), cells: u32, size_m: f32) {
        self.booms.push(Boom { x: at.0, y: at.1, size_m, buried: false });
        // Blasts dig: a crater a good part of the blast's size, where it
        // reaches the ground.
        self.crater(at, size_m as f64 * 0.45);
        self.explosions += 1;
        // Sappers caught in it are destroyed, and go off.
        self.sappers_caught(at, size_m as f64 * 0.5);
        // The blast heats the ground it reaches (what burns may catch).
        let r = (size_m as f64 / 2.0).ceil() as i64;
        let c0 = (at.0.floor() as i64, at.1.floor() as i64);
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r {
                    self.sandbox.heat_cell(&self.world, (c0.0 + dx, c0.1 + dy), BLAST_K);
                }
            }
        }
        self.happen(Happening::Explosion, at);
        let scrap = self.world.planet.mats.id("drone-scrap").unwrap();
        for i in 0..cells.max(1) {
            let r = hash(self.tick, i as i64, self.explosions as i64, 0xB00);
            let a = crate::noise::unit(r) * std::f64::consts::TAU;
            let v = 6.0 + 14.0 * crate::noise::unit(r ^ 0x5555);
            let off = 0.5 + 1.5 * crate::noise::unit(r ^ 0xAAAA);
            let p = (at.0 + a.cos() * off, at.1 + a.sin() * off);
            if let Some(c) = self.bodies.fling(&mut self.world, p, scrap, (a.cos() * v, a.sin() * v)) {
                self.scrap_landed(c);
            }
        }
    }

    /// Liquid the sandbox blasted out of vents this tick: thrown (in sight
    /// it flies; out of sight it is set where it comes down), keeping its
    /// heat when it lands.
    fn throw_ejecta(&mut self) {
        if self.sandbox.ejecta.is_empty() {
            return;
        }
        let ejecta = std::mem::take(&mut self.sandbox.ejecta);
        if let Some(e) = ejecta.first() {
            self.happen(Happening::Breach, e.at);
        }
        for e in ejecta {
            if self.eject_k.len() <= e.m as usize {
                self.eject_k.resize(e.m as usize + 1, 0.0);
            }
            self.eject_k[e.m as usize] = e.k;
            if let Some(c) = self.bodies.throw(&mut self.world, e.at, e.m, e.vel) {
                self.liquid_landed(c, e.m);
            }
        }
    }

    /// Thrown liquid came down at cell c: it is ground again (or a puddle),
    /// as hot as it left its vent.
    fn liquid_landed(&mut self, c: (i64, i64), m: MatId) {
        self.sandbox.wake_around_filled(&self.world, c);
        self.apply_sandbox(vec![Change { cell: c, from: VACUUM, to: m }]);
        if let Some(&k) = self.eject_k.get(m as usize).filter(|k| **k > 0.0) {
            self.sandbox.heat_cell(&self.world, c, k);
        }
    }

    /// Explosions since the last call (for the screen).
    pub fn take_booms(&mut self) -> Vec<Boom> {
        std::mem::take(&mut self.booms)
    }

    fn happen(&mut self, what: Happening, at: (f64, f64)) {
        if self.happenings.len() < 400 {
            self.happenings.push((what, at.0, at.1));
        }
    }

    /// What happened since the last call, and where (for the camera director).
    pub fn take_happenings(&mut self) -> Vec<(Happening, f64, f64)> {
        std::mem::take(&mut self.happenings)
    }


    // ---- Tunnelers ----

    /// Average extra worth of the solid rock around (x, y): the centre and
    /// eight points 10 m out.
    fn ore_smell(&self, x: f64, y: f64) -> f64 {
        let (cx, cy) = (x.floor() as i64, y.floor() as i64);
        let mut sum = 0.0;
        for (dx, dy) in std::iter::once((0, 0)).chain(NOSE.iter().copied()) {
            let mat = self.world.planet.mats.get(self.world.material(cx + dx * 10, cy + dy * 10));
            if mat.state == State::Solid {
                sum += (mat.multiplier - 1.0).max(0.0);
            }
        }
        sum / 9.0
    }

    /// Where a shaft starting at (x, y) heading (hx, hy) should aim: the
    /// richest-smelling point aim_depth_m ahead, within the allowed lean. A
    /// swarm that can't judge value just goes straight down.
    fn choose_aim(&self, t: &crate::economy::Tunneler, x: f64, y: f64, hx: f64, hy: f64) -> (f64, f64) {
        let d = t.aim_depth_m;
        let (c, s) = cos_sin_deg(t.max_deviation_deg);
        let half_w = d * s / c;
        let (bx, by) = (x + hx * d, y + hy * d);
        let (px, py) = (-hy, hx);
        let mut best = ((bx, by), 0.0);
        for j in [1i32, -1, 2, -2, 3, -3, 4, -4] {
            let off = half_w * j as f64 / 4.0;
            let p = (bx + px * off, by + py * off);
            let score = self.sense_value * self.ore_smell(p.0, p.1);
            if score > best.1 + 1e-9 {
                best = (p, score);
            }
        }
        best.0
    }

    /// Put Tunneler i on a fresh spot on the surface, spread around the body
    /// (a tunneler built as a unit: under a working flock).
    fn start_bore(&mut self, i: usize, unit: bool, t: &crate::economy::Tunneler) {
        let dir = if unit { self.unit_bore_start() } else { None };
        let (ux, uy) = match dir {
            Some(d) => d,
            None => {
                let n = self.ring_dir.len();
                let mut stride = ((n as f64 * 0.38) as usize).max(1);
                while gcd(stride, n) != 1 {
                    stride += 1;
                }
                let k = self.next_tunnel_k;
                self.next_tunnel_k = (k + stride) % n;
                self.ring_dir[k]
            }
        };
        let r = self.ground_radius(ux, uy, self.world.planet.max_radius + 1.0);
        let (x, y, hx, hy) = (ux * r, uy * r, -ux, -uy);
        let aim = self.choose_aim(t, x, y, hx, hy);
        let tick = self.tick;
        self.bores(unit)[i] = Bore { x, y, hx, hy, aim, length: 0.0, progress_s: 0.0, active: true, resume_at: 0, prev: (x, y), moved_at: tick };
    }

    /// The old Tunnelers, or the tunnelers built as units.
    fn bores(&mut self, unit: bool) -> &mut Vec<Bore> {
        if unit {
            &mut self.units.bores
        } else {
            &mut self.tunnelers
        }
    }

    /// Solid cells within the bore around (x, y), and whether lava is there.
    fn bore_cells(&self, x: f64, y: f64, radius: f64) -> (Vec<(i64, i64)>, bool) {
        let mut out = Vec::new();
        let mut lava = false;
        let r = radius.ceil() as i64 + 1;
        let (cx, cy) = (x.floor() as i64, y.floor() as i64);
        for yy in cy - r..=cy + r {
            for xx in cx - r..=cx + r {
                let (dx, dy) = (xx as f64 + 0.5 - x, yy as f64 + 0.5 - y);
                if dx * dx + dy * dy > radius * radius {
                    continue;
                }
                let m = self.world.material(xx, yy);
                if m == VACUUM {
                    continue;
                }
                if self.world.planet.mats.get(m).state == State::Liquid {
                    lava = true;
                }
                out.push((xx, yy));
            }
        }
        (out, lava)
    }

    /// One second of boring for every Tunneler: the old ones, and the
    /// tunnelers built as units (their own, wider numbers).
    fn tunnel_work(&mut self) {
        if !self.tunnelers.is_empty() {
            let t = self.eco.tunneler.clone();
            self.bore_list(false, &t);
        }
        if !self.units.bores.is_empty() {
            let t = self.unit_tunneler();
            self.bore_list(true, &t);
        }
    }

    fn bore_list(&mut self, as_unit: bool, t: &crate::economy::Tunneler) {
        let (cos_max, _) = cos_sin_deg(t.max_deviation_deg);
        let (_, turn) = cos_sin_deg(t.turn_deg_per_m);
        let cargo = self.eco.harvester.cargo_kg * self.up_cached_cargo;
        let power = t.drill_power * self.up("dig speed") * self.perk_dig;
        for i in 0..self.bores(as_unit).len() {
            let tick = self.tick;
            {
                let b = &mut self.bores(as_unit)[i];
                b.prev = (b.x, b.y);
                b.moved_at = tick;
            }
            if !self.bores(as_unit)[i].active {
                if self.tick >= self.bores(as_unit)[i].resume_at {
                    self.start_bore(i, as_unit, t);
                }
                continue;
            }
            if self.brownout() {
                continue;
            }
            self.bores(as_unit)[i].progress_s += 1.0;
            // Up to four metres a second in soft ground.
            for _ in 0..4 {
                let b = self.bores(as_unit)[i].clone();
                // Steer a little toward the aim, but never lean too far from
                // straight down.
                let (dx, dy) = unit(b.aim.0 - b.x, b.aim.1 - b.y);
                let (mut hx, mut hy) = unit(b.hx + turn * dx, b.hy + turn * dy);
                let (rx, ry) = unit(-b.x, -b.y);
                for _ in 0..8 {
                    if hx * rx + hy * ry >= cos_max {
                        break;
                    }
                    (hx, hy) = unit(hx + 0.25 * rx, hy + 0.25 * ry);
                }
                let (nx, ny) = (b.x + hx, b.y + hy);
                let (cells, lava) = self.bore_cells(nx, ny, t.bore_m / 2.0);
                let too_deep = (nx * nx + ny * ny).sqrt() < t.min_radius * self.world.planet.radius;
                if lava || too_deep || b.length >= t.max_length_m {
                    // Done: pull out and move on (it won't bore into lava).
                    let resume = self.tick + self.ticks(t.relocate_s);
                    let tb = &mut self.bores(as_unit)[i];
                    tb.active = false;
                    tb.resume_at = resume;
                    self.tunnels_done += 1;
                    break;
                }
                let (mut need_s, mut tonnes) = (0.0, 0.0);
                for c in &cells {
                    let mat = self.world.planet.mats.get(self.world.material(c.0, c.1));
                    need_s += self.eco.harvester.dig_s_for(mat.hardness) * mat.density / cargo / power;
                    tonnes += mat.density / 1000.0;
                }
                let need_e = tonnes * t.energy_per_tonne;
                if b.progress_s < need_s || self.energy - need_e < 5.0 * self.upkeep() {
                    break;
                }
                self.energy -= need_e;
                for c in cells {
                    let m = self.world.material(c.0, c.1);
                    let mat = self.world.planet.mats.get(m);
                    let (kg, mult) = (mat.density, mat.multiplier);
                    self.world.mine_cell(c.0, c.1);
                    self.undope(c, false);
                    self.progress.remove(&c);
                    self.open_cell(c);
                    let credited = kg * mult * t.spoil_recovery;
                    self.stock.add_composition(&self.world.planet.mats.elements, &self.world.planet.mats.get(m).composition, kg * t.spoil_recovery);
                    self.mass += credited;
                    self.delivered_credited += credited;
                    self.tunnel_kg += kg;
                    self.window_mined += kg;
                }
                // Crack the rock around the shaft.
                let halo = t.bore_m / 2.0 + t.fracture_m;
                let r = halo.ceil() as i64 + 1;
                let (cx, cy) = (nx.floor() as i64, ny.floor() as i64);
                for yy in cy - r..=cy + r {
                    for xx in cx - r..=cx + r {
                        let (ddx, ddy) = (xx as f64 + 0.5 - nx, yy as f64 + 0.5 - ny);
                        if ddx * ddx + ddy * ddy <= halo * halo && self.world.material(xx, yy) != VACUUM {
                            self.fractured.insert((xx, yy));
                        }
                    }
                }
                let tb = &mut self.bores(as_unit)[i];
                tb.progress_s -= need_s;
                tb.x = nx;
                tb.y = ny;
                tb.hx = hx;
                tb.hy = hy;
                tb.length += 1.0;
                let length = tb.length;
                self.tunnel_m += 1;
                // Every so often, a staging point down the shaft: drones based
                // there dig the cracked walls and the rock beyond.
                if length as u64 % t.hub_every_m as u64 == 0 {
                    self.hubs.push(Hub { x: nx, y: ny, kind: HubKind::Shaft, ring: None });
                    self.hub_load.push(0);
                    self.hub_dry.push(false);
                    self.hub_gen += 1;
                    self.site_wait.push(Vec::new());
                    self.flock.push(FlockStats::default());
                }
            }
            // Unspent drilling time doesn't pile up while waiting on energy.
            let tb = &mut self.bores(as_unit)[i];
            tb.progress_s = tb.progress_s.min(60.0);
        }
    }

    // ---- What can be dug ----

    fn is_exterior(&mut self, x: i64, y: i64) -> bool {
        // Outside stays outside: remember the answer (the check is noise work).
        if self.outside.contains(&(x, y)) {
            return true;
        }
        let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
        let r = (fx * fx + fy * fy).sqrt().max(1e-9);
        let out = r > self.world.planet.surface_radius(fx / r, fy / r) || self.world.planet.in_crater(fx, fy);
        if out {
            self.outside.insert((x, y));
        }
        out
    }

    fn add_frontier(&mut self, c: (i64, i64)) {
        if self.frontier.contains_key(&c) {
            return;
        }
        let m = self.world.material(c.0, c.1);
        // Smelling costs lookups, so it's done only once the swarm can use it.
        let (near, far) = if self.sense_value > 0.0 { scent(&self.world, c, self.scouts > 0) } else { (0.0, 0.0) };
        let face = Face { mat: m, near, far };
        if self.frontier.insert(c, face).is_none() {
            self.buckets.entry((c.0.div_euclid(BUCKET), c.1.div_euclid(BUCKET))).or_default().push((c, face));
            self.work_near(c);
            if self.world.planet.mats.get(m).state == State::Liquid {
                self.liquid_faces += 1;
            }
        }
        // Newly exposed: it may now fall, flow, cool or react.
        self.sandbox.wake(c);
    }

    /// Bring the swarm's picture of the dig face up to date after the sandbox
    /// moved or changed cells.
    fn apply_sandbox(&mut self, changes: Vec<Change>) {
        for ch in changes {
            let c = ch.cell;
            self.bodies.touched(c);
            if ch.to == VACUUM {
                // Emptied (its contents moved away or vented): open space now.
                self.progress.remove(&c);
                self.reserved.remove(&c);
                if self.frontier.contains_key(&c) || self.open.contains(&c) || self.touches_open(c) {
                    self.open_cell_woken(c);
                }
            } else if ch.from == VACUUM {
                // Something flowed or fell into open space.
                let was_open = self.open.remove(&c);
                if was_open || self.touches_open(c) {
                    self.add_frontier(c);
                }
            } else if self.frontier.contains_key(&c) {
                // Changed in place (froze, melted, reacted).
                self.set_face_mat(c, ch.to);
                self.progress.remove(&c);
            }
        }
    }


    fn unwork(&mut self, cell: (i64, i64), id: u32) {
        if let Some(v) = self.working.get_mut(&cell) {
            v.retain(|&d| d != id);
            if v.is_empty() {
                self.working.remove(&cell);
            }
        }
    }

    /// A drone is wrecked: it leaves the swarm and falls as a body of scrap.
    fn wreck_drone(&mut self, id: u32, at: (f64, f64), vel: (f64, f64)) {
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
    fn scrap_landed(&mut self, c: (i64, i64)) {
        self.sandbox.wake_around_filled(&self.world, c);
        self.apply_sandbox(vec![Change { cell: c, from: VACUUM, to: self.world.material(c.0, c.1) }]);
        if self.world.material(c.0, c.1) != VACUUM {
            self.add_frontier(c);
        }
    }

    /// Apply what the loose bodies did this tick: cells that broke free leave
    /// the dig face, cells that landed join it, and fast rock wrecks drones
    /// working where it passes.
    fn body_tick(&mut self) {
        let q0 = clock();
        // (Every step: it works through a steady allowance in small pieces.)
        {
            let lifted = self.bodies.break_free_big(&mut self.world);
            for c in lifted {
                self.progress.remove(&c);
                self.reserved.remove(&c);
                self.open_cell(c);
            }
        }
        let q1 = clock();
        // (Every step, so the work comes in small pieces.)
        {
            let lifted = self.bodies.break_free(&mut self.world);
            for c in lifted {
                self.progress.remove(&c);
                self.reserved.remove(&c);
                self.open_cell(c);
            }
        }
        let q2 = clock();
        let ev = self.bodies.step(&mut self.world);
        let q3 = clock();
        self.prof2[0] += q1 - q0;
        self.prof2[1] += q2 - q1;
        self.prof2[2] += q3 - q2;
        for c in ev.lifted {
            self.progress.remove(&c);
            self.reserved.remove(&c);
            self.open_cell(c);
        }
        for (c, m) in ev.landed {
            if self.eject_k.get(m as usize).is_some_and(|k| *k > 0.0) {
                // Blasted out of a vent: it lands as hot as it left.
                self.liquid_landed(c, m);
                continue;
            }
            // Fallen and thrown rock lies loose, and may roll on.
            self.loose.insert(c);
            self.world.set_look(c.0, c.1, crate::world::Look::RUBBLE);
            self.settling.insert(c);
            self.sandbox.wake_around_filled(&self.world, c);
            self.apply_sandbox(vec![Change { cell: c, from: VACUUM, to: m }]);
        }
        self.prof2[3] += clock() - q3;
        // The drone hovers on the open side of the cell it cuts: crushed if
        // falling rock passes right beside that cell.
        if !ev.crushing.is_empty() {
            let mut hit: Vec<(i64, i64)> = Vec::new();
            for c in &ev.crushing {
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let n = (c.0 + dx, c.1 + dy);
                    if self.working.contains_key(&n) {
                        hit.push(n);
                    }
                }
            }
            hit.sort();
            hit.dedup();
            let mut doomed = Vec::new();
            for cell in hit {
                for &id in &self.working[&cell] {
                    if let Phase::Outbound { from, .. } = self.drones[id as usize].phase {
                        doomed.push((id, cell, from));
                    }
                }
            }
            for (id, cell, from) in doomed {
                self.unwork(cell, id);
                if let Some(n) = self.reserved.get_mut(&cell) {
                    *n -= 1;
                    if *n == 0 {
                        self.reserved.remove(&cell);
                    }
                }
                self.work_near(cell);
                self.on_trip_count -= 1;
                let (cx, cy) = (cell.0 as f64 + 0.5, cell.1 as f64 + 0.5);
                let (ux, uy) = (from.0 - cx, from.1 - cy);
                let l = (ux * ux + uy * uy).sqrt().max(1e-9);
                self.wreck_drone(id, (cx + ux / l * 1.5, cy + uy / l * 1.5), (0.0, 0.0));
                self.crushed += 1;
                self.happen(Happening::Losses, (cx, cy));
            }
        }
    }

    /// Wake the dig-face cells in sunlight under open sky whose material the
    /// Sun can change (it melts or boils below the Sun's heat).
    fn sun_wake(&mut self) {
        let Some(sun) = self.world.planet.spec.sun.clone() else { return };
        let mats = &self.world.planet.mats;
        let sensitive: Vec<bool> = (0..mats.len())
            .map(|m| mats.phases[m].up.iter().flatten().any(|ch| ch.at_k < sun.day_k))
            .collect();
        let t = self.time();
        let mut wake = Vec::new();
        for (&c, f) in &self.frontier {
            if !sensitive[f.mat as usize] {
                continue;
            }
            let (fx, fy) = (c.0 as f64 + 0.5, c.1 as f64 + 0.5);
            let r = (fx * fx + fy * fy).sqrt().max(1e-9);
            if self.world.planet.sky_k(fx / r, fy / r, t) <= self.world.planet.space_k {
                continue;
            }
            wake.push(c);
        }
        for c in wake {
            self.sandbox.wake(c);
        }
    }

    /// Take a cell off the dig face.
    fn remove_face(&mut self, c: (i64, i64)) -> Option<Face> {
        let f = self.frontier.remove(&c)?;
        let b = (c.0.div_euclid(BUCKET), c.1.div_euclid(BUCKET));
        if let Some(list) = self.buckets.get_mut(&b) {
            if let Some(i) = list.iter().position(|(x, _)| *x == c) {
                list.remove(i);
            }
            if list.is_empty() {
                self.buckets.remove(&b);
            }
        }
        if self.world.planet.mats.get(f.mat).state == State::Liquid {
            self.liquid_faces -= 1;
        }
        Some(f)
    }

    /// A dig-face cell changed material in place.
    fn set_face_mat(&mut self, c: (i64, i64), m: MatId) {
        let liquid = |s: &Self, m: MatId| s.world.planet.mats.get(m).state == State::Liquid;
        let Some(old) = self.frontier.get(&c).map(|f| f.mat) else { return };
        if liquid(self, old) {
            self.liquid_faces -= 1;
        }
        if liquid(self, m) {
            self.liquid_faces += 1;
        }
        self.frontier.get_mut(&c).unwrap().mat = m;
        let b = (c.0.div_euclid(BUCKET), c.1.div_euclid(BUCKET));
        if let Some(e) = self.buckets.get_mut(&b).and_then(|l| l.iter_mut().find(|(x, _)| *x == c)) {
            e.1.mat = m;
        }
        self.work_near(c);
    }

    fn touches_open(&self, c: (i64, i64)) -> bool {
        [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dy)| self.open.contains(&(c.0 + dx, c.1 + dy)))
    }

    /// Seconds to cut one load of material `m`, and whether it is lava.
    fn dig_s(&self, m: MatId) -> f64 {
        let mat = self.world.planet.mats.get(m);
        self.eco.harvester.dig_s_for(mat.hardness) / (self.up("dig speed") * self.perk_dig)
    }

    /// Directions marched in from outside to find the surface: about 1.5 per
    /// metre around the highest ground.
    fn survey_n(&self) -> u32 {
        (2.0 * std::f64::consts::PI * self.world.planet.max_radius * 1.5).ceil() as u32
    }

    /// Whether this body is small enough to survey whole at the start.
    fn surveyed_whole(&self) -> bool {
        2.0 * std::f64::consts::PI * self.world.planet.max_radius <= self.eco.seed_swarm.full_survey_m
    }

    /// The first solid cell met marching inward along (ux, uy), added to the
    /// dig face.
    fn survey_dir(&mut self, ux: f64, uy: f64) {
        // Step down from just above the highest possible ground in 0.5 m
        // steps; skipping straight past the steps that are surely above
        // this direction's surface leaves the same steps, so finds the
        // same cell, without thousands of empty lookups on a big body.
        let top = self.world.planet.max_radius + 1.0;
        let skip = ((top - (self.world.planet.surface_radius(ux, uy) + 3.0)) / 0.5).floor().max(0.0);
        let mut r = top - skip * 0.5;
        while r > 0.0 {
            let (x, y) = ((ux * r).floor() as i64, (uy * r).floor() as i64);
            if self.world.material(x, y) != VACUUM {
                self.add_frontier((x, y));
                break;
            }
            r -= 0.5;
        }
    }

    /// As `survey_dir`, but through deep open ground (a big crater) in 4 m
    /// strides, refined to 0.5 m at the end: slivers thinner than a stride
    /// can be passed over, which only matters for the very first look.
    /// Returns whether the cell found is solid ground (not liquid).
    fn survey_dir_fast(&mut self, ux: f64, uy: f64) -> bool {
        let top = self.world.planet.max_radius + 1.0;
        let skip = ((top - (self.world.planet.ground_hint(ux, uy) + 3.0)) / 0.5).floor().max(0.0);
        let mut r = top - skip * 0.5;
        let solid = |w: &World, r: f64| w.material((ux * r).floor() as i64, (uy * r).floor() as i64) != VACUUM;
        while r > 4.0 && !solid(&self.world, r) && !solid(&self.world, r - 4.0) {
            r -= 4.0;
        }
        while r > 0.0 {
            if solid(&self.world, r) {
                let c = ((ux * r).floor() as i64, (uy * r).floor() as i64);
                self.add_frontier(c);
                let m = self.world.material(c.0, c.1);
                // (Liquid counts too: seas are drunk like anything else.)
                let _ = m;
                return true;
            }
            r -= 0.5;
        }
        false
    }

    /// A small body: every solid surface cell, found by marching in from
    /// outside along evenly spaced directions. A big one starts with nothing:
    /// each landing zone is surveyed as its landing point is dropped.
    fn seed_frontier(&mut self) {
        if !self.surveyed_whole() {
            return;
        }
        let n = self.survey_n();
        let (c, s) = turn(n);
        let (mut ux, mut uy) = (0.0f64, 1.0f64);
        for _ in 0..n {
            self.survey_dir(ux, uy);
            let (nx, ny) = (ux * c - uy * s, ux * s + uy * c);
            let l = (nx * nx + ny * ny).sqrt();
            ux = nx / l;
            uy = ny / l;
        }
    }

    /// Survey the ground within `zone_m` either side of direction (ux, uy),
    /// at the same spacing as a whole-body survey.
    /// Returns how many solid (diggable) cells it found.
    fn survey_zone(&mut self, ux: f64, uy: f64) -> usize {
        let n = self.survey_n();
        let (c, s) = turn(n);
        let ground = self.world.planet.surface_radius(ux, uy).max(1.0);
        let half = ((self.eco.seed_swarm.zone_m / ground * n as f64 / (2.0 * std::f64::consts::PI)).ceil() as u32).min(n / 2);
        // Back up half the zone, then sweep across it.
        let (mut vx, mut vy) = (ux, uy);
        for _ in 0..half {
            let (nx, ny) = (vx * c + vy * s, -vx * s + vy * c);
            let l = (nx * nx + ny * ny).sqrt();
            vx = nx / l;
            vy = ny / l;
        }
        let mut solid = 0;
        for _ in 0..=2 * half {
            if self.survey_dir_fast(vx, vy) {
                solid += 1;
            }
            let (nx, ny) = (vx * c - vy * s, vx * s + vy * c);
            let l = (nx * nx + ny * ny).sqrt();
            vx = nx / l;
            vy = ny / l;
        }
        solid
    }

    /// After a cell is dug: it is open space now, its solid neighbours become
    /// diggable, and any natural cave it breaks into opens up too.
    fn open_cell(&mut self, c: (i64, i64)) {
        self.sandbox.wake_around_filled(&self.world, c);
        self.open_cell_woken(c);
        self.unsettle_around(c);
    }

    /// As `open_cell`, for a cell whose neighbourhood the sandbox has already
    /// woken (it emptied the cell itself).
    fn open_cell_woken(&mut self, c: (i64, i64)) {
        self.open.insert(c);
        self.bodies.opened(c);
        self.remove_face(c);
        let mut queue = vec![c];
        let mut budget = 2_000_000usize;
        let liquid_before = self.liquid_faces;
        while let Some((x, y)) = queue.pop() {
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let n = (x + dx, y + dy);
                if self.open.contains(&n) || self.frontier.contains_key(&n) {
                    continue;
                }
                if self.world.material(n.0, n.1) == VACUUM {
                    if self.is_exterior(n.0, n.1) || self.world.in_empty_region(n.0, n.1) {
                        continue;
                    }
                    // A natural void: it joins the open space.
                    if budget > 0 {
                        budget -= 1;
                        self.open.insert(n);
                        queue.push(n);
                    }
                } else {
                    self.add_frontier(n);
                }
            }
        }
        // Broke into a cave (a good few cells of natural void opened at
        // once), or into lava or a sea.
        let opened = 2_000_000 - budget;
        if opened >= 20 || self.liquid_faces > liquid_before {
            self.happen(Happening::Breach, (c.0 as f64 + 0.5, c.1 as f64 + 0.5));
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
    fn weigh(&self, hx: f64, hy: f64, reach_m: f64, packet: bool, peel: bool, picky: bool, field: Option<&FlowField>, cands: &mut Vec<((i64, i64), f64, f64)>) -> f64 {
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
    fn choose(cands: &[((i64, i64), f64, f64)], reach: f64) -> Option<(i64, i64)> {
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
    fn clear_line(&self, from: (f64, f64), c: (i64, i64)) -> bool {
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
    fn pick_target(&mut self, hx: f64, hy: f64, reach_m: f64, packet: bool, peel: bool, picky: bool, home: Option<usize>) -> Option<(i64, i64)> {
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

    fn nearest_hub(&self, x: f64, y: f64) -> usize {
        let mut best = (0, f64::MAX);
        for (i, h) in self.hubs.iter().enumerate() {
            let d = (h.x - x) * (h.x - x) + (h.y - y) * (h.y - y);
            if d < best.1 {
                best = (i, d);
            }
        }
        best.0
    }

    // ---- Drones ----

    fn spawn_harvester(&mut self) {
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
    fn drop_s(&self, dist: f64) -> f64 {
        (dist / self.cutter_speed()).min(DROP_S)
    }

    fn individuals(&self) -> u32 {
        self.alive_count
    }

    pub fn harvesters_alive(&self) -> u64 {
        self.alive_count as u64 + self.packet
    }

    fn cutter_speed(&self) -> f64 {
        self.eco.harvester.speed * self.up("flight speed") * self.perk_speed
    }

    fn hold_cap(&self) -> f64 {
        self.eco.harvester.hold_kg * self.up_cached_cargo
    }

    /// Seconds to cut `kg` of material `m` at cell c.
    fn cut_s(&self, c: (i64, i64), m: MatId, kg: f64) -> f64 {
        if self.is_loose(c, m) {
            // Loose debris: picked up, not cut.
            let d = self.world.planet.mats.get(m).density.max(1.0);
            return self.eco.harvester.loose_s * (kg / d).max(0.05);
        }
        let per_load = self.dig_s(m) * if self.fractured.contains(&c) { self.eco.tunneler.fracture_dig } else { 1.0 };
        per_load * (kg / self.eco.harvester.cargo_kg).max(0.05) / self.cold_factor(c, m)
    }

    /// Whether cell c (holding m) is loose debris.
    fn is_loose(&self, c: (i64, i64), m: MatId) -> bool {
        self.loose_mat.get(m as usize).copied().unwrap_or(false) || (!self.loose.is_empty() && self.loose.contains(&c))
    }

    /// Cold ground: how much faster a cutter works it (and the same, how
    /// much quicker it wears), 1 at cold_ref_k and above.
    fn cold_factor(&self, c: (i64, i64), m: MatId) -> f64 {
        if m == VACUUM {
            return 1.0;
        }
        let h = &self.eco.harvester;
        let t = self.sandbox.temp(&self.world, c, m);
        1.0 + h.cold_gain * ((h.cold_ref_k - t) / h.cold_ref_k.max(1.0)).clamp(0.0, 1.0)
    }

    /// The hottest ground a cutter at cell c touches (kelvin): the cell and
    /// its four neighbours.
    fn contact_k(&self, c: (i64, i64)) -> f64 {
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
    fn heat_drone(&mut self, id: u32, secs: f64, env: f64, contact: bool) -> bool {
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

    /// How dangerous a hauler's spot is: cells of flowing lava (any liquid
    /// hot enough to harm a drone) in the cloud around it (14 m), or lying
    /// open in a pool below it (to 24 m down), where more pouring in would
    /// fill up to it. Hot rock and magma sealed under a crust don't count:
    /// they don't move. 0 is safe. About 60 cells looked at.
    fn site_danger(&self, at: (f64, f64)) -> u32 {
        let hot = self.eco.harvester.damage_k;
        let r = (at.0 * at.0 + at.1 * at.1).sqrt().max(1.0);
        let (ux, uy) = (at.0 / r, at.1 / r);
        let mut n = 0;
        let mut look = |x: f64, y: f64, open: bool| {
            let c = (x.floor() as i64, y.floor() as i64);
            let m = self.world.material(c.0, c.1);
            if m == VACUUM || self.world.planet.mats.get(m).state != State::Liquid || self.sandbox.temp(&self.world, c, m) <= hot {
                return;
            }
            // Below: only a pool open to the sky over it.
            if open && self.world.material((x + ux * 1.5).floor() as i64, (y + uy * 1.5).floor() as i64) != VACUUM {
                return;
            }
            n += 1;
        };
        let mut dx = -14.0;
        while dx <= 14.0 {
            let mut dy = -14.0;
            while dy <= 14.0 {
                if dx * dx + dy * dy <= 14.0 * 14.0 {
                    look(at.0 + dx, at.1 + dy, false);
                }
                dy += 4.0;
            }
            dx += 4.0;
        }
        // Straight down, and a little to each side.
        let (sx, sy) = (-uy, ux);
        let mut d = 16.0;
        while d <= 24.0 {
            for side in [-8.0, 0.0, 8.0] {
                look(at.0 - ux * d + sx * side, at.1 - uy * d + sy * side, true);
            }
            d += 4.0;
        }
        n
    }

    /// Once a second, every flock checks its hauler's spot: if lava has come
    /// close (around it or rising from below), the flock moves to the nearest
    /// safe open spot, preferring higher ground and the side its work is on.
    /// The hauler flies there and the waiting cutters follow.
    fn site_safety(&mut self) {
        for h in 0..self.hubs.len() {
            if self.flock.get(h).is_none_or(|f| f.dead) || self.hubs[h].kind != HubKind::Seed {
                continue;
            }
            let old = self.site_pos(h);
            if self.site_danger(old) == 0 {
                continue;
            }
            let r0 = (old.0 * old.0 + old.1 * old.1).sqrt().max(1.0);
            let f = self.flock[h];
            let work = if f.w > 0.0 { Some((f.sx / f.w, f.sy / f.w)) } else { None };
            let mut best: Option<((f64, f64), f64)> = None;
            for ring in [24.0, 48.0, 96.0, 192.0] {
                for k in 0..16 {
                    let a = k as f64 / 16.0 * std::f64::consts::TAU;
                    let p = (old.0 + a.cos() * ring, old.1 + a.sin() * ring);
                    if self.world.material(p.0.floor() as i64, p.1.floor() as i64) != VACUUM || self.site_danger(p) > 0 {
                        continue;
                    }
                    let up = (p.0 * p.0 + p.1 * p.1).sqrt() - r0;
                    let to_work = work.map_or(0.0, |w| dist2(p, w).sqrt());
                    let score = ring + 0.5 * to_work - 2.0 * up;
                    if best.is_none_or(|(_, b)| score < b) {
                        best = Some((p, score));
                    }
                }
                if best.is_some() {
                    break;
                }
            }
            let Some((p, _)) = best else { continue };
            self.hubs[h].x = p.0;
            self.hubs[h].y = p.1;
            self.site_moves += 1;
            // Its working haulers fly over (arriving, they take the waiting
            // cutters' loads as usual).
            let speed = self.eco.hauler.speed.max(1.0);
            let way = self.hauler_way(old, p);
            for hl in self.haulers.iter_mut() {
                if hl.alive && hl.site == h && hl.state == HaulerState::Working {
                    hl.state = HaulerState::Returning;
                    hl.from = old;
                    hl.to = p;
                    hl.t0 = self.tick;
                    hl.t1 = self.tick + ((way / speed) / self.eco.tick_s).ceil() as u64;
                }
            }
            // The waiting cutters fly along with their loads.
            for id in std::mem::take(&mut self.site_wait[h]) {
                let d = &self.drones[id as usize];
                if !d.alive || d.home != h || d.phase != Phase::Waiting {
                    continue;
                }
                self.drones[id as usize].trip_start = self.tick;
                self.on_trip_count += 1;
                self.head_home(id, old);
                self.leg_changed(id);
            }
        }
    }

    /// One second for the cutters waiting at their flocks: each sits
    /// somewhere in the loose cloud about its hauler (3 to 12 m out, as it is
    /// drawn), and whatever is there heats or cools it. Lava pouring over the
    /// cloud melts the drones caught in it, reckless as ever.
    fn waiting_heat(&mut self) {
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
    fn swallow_heat(&mut self, id: u32, m: MatId, t_cell: f64, kg: f64) -> bool {
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
    fn cut_kg(&self, c: (i64, i64), space: f64) -> f64 {
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
    fn spend(&mut self, id: u32, secs: f64) -> bool {
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
    fn lose_on_trip(&mut self, id: u32, at: (f64, f64)) {
        self.happen(Happening::Losses, at);
        self.on_trip_count -= 1;
        self.wreck_drone(id, at, (0.0, 0.0));
    }

    /// Where a drone on its way somewhere is at this moment (roughly: where
    /// it set off from, for a loss).
    fn site_pos(&self, h: usize) -> (f64, f64) {
        (self.hubs[h].x, self.hubs[h].y)
    }

    /// A working hauler at site h with the most room, if any.
    fn hauler_at(&self, h: usize) -> Option<usize> {
        let mut best: Option<(usize, f64)> = None;
        for (i, hl) in self.haulers.iter().enumerate() {
            if hl.alive && hl.site == h && hl.state == HaulerState::Working && best.is_none_or(|(_, l)| hl.load < l) {
                best = Some((i, hl.load));
            }
        }
        best.map(|b| b.0)
    }

    /// Hand a drone's hold to hauler k.
    fn unload_into(&mut self, id: u32, k: usize) {
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

    /// Fly a drone with a full hold from `from` to its flock's hauler.
    fn head_home(&mut self, id: u32, from: (f64, f64)) {
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
    fn go_cut(&mut self, id: u32, c: (i64, i64), from: (f64, f64), home: Option<usize>) {
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

    fn on_event(&mut self, id: u32) {
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

    /// The nearest other flock within reach with a hauler taking loads.
    fn nearest_working_site(&self, h: usize) -> Option<usize> {
        let p = self.site_pos(h);
        let reach = 2.0 * self.eco.harvester.search_m;
        let mut best: Option<(usize, f64)> = None;
        for hl in self.haulers.iter().filter(|hl| hl.alive && hl.state == HaulerState::Working && hl.site != h) {
            let d = dist2(p, self.site_pos(hl.site));
            if d <= reach * reach && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((hl.site, d));
            }
        }
        best.map(|b| b.0)
    }

    // ---- Flocks ----

    /// A new flock at (x, y), with a hauler on its way down to it.
    fn new_flock(&mut self, x: f64, y: f64) -> usize {
        self.hubs.push(Hub { x, y, kind: HubKind::Seed, ring: None });
        self.hub_load.push(0);
        self.hub_dry.push(false);
        self.hub_gen += 1;
        self.site_wait.push(Vec::new());
        self.flock.push(FlockStats::default());
        let h = self.hubs.len() - 1;
        self.send_hauler(h);
        self.happen(Happening::NewFlock, (x, y));
        h
    }

    /// Move cutters between flocks, all in one pass over the swarm: for
    /// each (from, to, every), every `every`-th cutter of `from` goes to
    /// `to` (all of them with every = 1); cutters waiting with a full hold
    /// go too. Returns how many each move took.
    fn move_cutters(&mut self, moves: &[(usize, usize, usize)]) -> Vec<u64> {
        let mut moved = vec![0u64; moves.len()];
        if moves.is_empty() {
            return moved;
        }
        let mut seen = vec![0usize; moves.len()];
        // Each flock's moves, so a drone finds its own quickly.
        let mut by_from: HashMap<usize, Vec<usize>> = HashMap::new();
        for (i, m) in moves.iter().enumerate() {
            by_from.entry(m.0).or_default().push(i);
        }
        for id in 0..self.drones.len() {
            let d = &self.drones[id];
            if !d.alive {
                continue;
            }
            let Some(list) = by_from.get(&d.home) else { continue };
            let from = d.home;
            let mut to = None;
            for &i in list {
                seen[i] += 1;
                if seen[i] % moves[i].2 == 0 {
                    to = Some((i, moves[i].1));
                    break;
                }
            }
            let Some((i, to)) = to else { continue };
            moved[i] += 1;
            let waiting = self.drones[id].phase == Phase::Waiting;
            self.drones[id].home = to;
            self.hub_load[from] -= 1;
            self.hub_load[to] += 1;
            if waiting {
                // It takes its load to the new flock's hauler.
                self.drones[id].phase = Phase::Idle;
                self.on_trip_count += 1;
                let here = self.site_pos(from);
                self.head_home(id as u32, here);
                self.leg_changed(id as u32);
            }
        }
        for m in moves {
            let from = m.0;
            let drones = &self.drones;
            self.site_wait[from].retain(|&id| drones[id as usize].home == from);
        }
        moved
    }

    /// Blow up a hauler that isn't needed any more.
    fn retire_hauler(&mut self, k: usize) {
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

    /// The flocks talk among themselves every few seconds:
    /// - each flock's hauler drifts to hover over where its cutters work;
    /// - a flock grown too big splits, the new one going to cover fresh
    ///   ground along the surface, away from the others;
    /// - a flock that has run out of work, or has drifted onto another,
    ///   merges into its nearest neighbour;
    /// - a flock stuck on hard ground calls for help, and easier-going
    ///   neighbours send some of their cutters;
    /// - a spare hauler that stands empty is blown up.
    fn flock_work(&mut self) {
        let dt = self.ticks(5.0) as f64 * self.eco.tick_s;
        let spec = self.eco.flock.clone();
        let n = self.hubs.len();
        let decay = (-dt / spec.memory_s).exp();
        // Follow the work.
        for h in 0..n {
            let f = self.flock[h];
            if f.dead || self.hubs[h].kind != HubKind::Seed {
                continue;
            }
            if f.w > 0.0 {
                let (tx, ty) = (f.sx / f.w, f.sy / f.w);
                // Hover a little out from the face, over open ground.
                let l = (tx * tx + ty * ty).sqrt().max(1.0);
                let (tx, ty) = (tx + tx / l * spec.hover_m, ty + ty / l * spec.hover_m);
                let (x, y) = (self.hubs[h].x, self.hubs[h].y);
                let k = spec.follow;
                let (nx, ny) = (x + (tx - x) * k, y + (ty - y) * k);
                if self.world.material(nx.floor() as i64, ny.floor() as i64) == VACUUM && self.site_danger((nx, ny)) == 0 {
                    self.hubs[h].x = nx;
                    self.hubs[h].y = ny;
                }
            }
            let empty = self.active(h) == 0;
            let f = &mut self.flock[h];
            f.empty_s = if empty { f.empty_s + dt } else { 0.0 };
            // No cutters for a while: the flock is gone (its haulers retire).
            if f.empty_s >= spec.calm_s {
                f.dead = true;
                self.hub_gen += 1;
                continue;
            }
            f.calm_s += dt;
            f.sx *= decay;
            f.sy *= decay;
            f.w *= decay;
            f.kg *= decay;
            f.dig_s *= decay;
            f.dry *= decay;
        }
        let live: Vec<usize> = (0..n).filter(|&h| !self.flock[h].dead && self.hubs[h].kind == HubKind::Seed).collect();
        // Cutters to move between flocks, all at the end in one pass.
        let mut moves: Vec<(usize, usize, usize)> = Vec::new();
        let mut helps: Vec<usize> = Vec::new();
        // Merge: out of work, or on top of a neighbour.
        for &a in &live {
            if self.flock[a].dead || self.flock[a].calm_s < spec.calm_s {
                continue;
            }
            let pa = self.site_pos(a);
            let near = live.iter().copied().filter(|&b| b != a && !self.flock[b].dead).min_by(|&x, &y| dist2(pa, self.site_pos(x)).total_cmp(&dist2(pa, self.site_pos(y))));
            let Some(b) = near else { continue };
            let d = dist2(pa, self.site_pos(b)).sqrt();
            let out_of_work = self.flock[a].dry >= spec.dry_merge && self.flock[a].kg < 1.0 && d <= spec.comm_m;
            let on_top = d <= spec.merge_m && self.hub_load[a] <= self.hub_load[b];
            if !(out_of_work || on_top) || self.flock[b].dry >= spec.dry_merge {
                continue;
            }
            moves.push((a, b, 1));
            self.flock[a].dead = true;
            self.hub_gen += 1;
            self.flock[b].calm_s = 0.0;
            self.merges += 1;
            let pb = self.site_pos(b);
            self.happen(Happening::Merge, pb);
            // Its haulers join the other flock, as many as it can use; the
            // rest blow up.
            let mut have = self.haulers.iter().filter(|hl| hl.alive && hl.site == b).count() as u32;
            for k in 0..self.haulers.len() {
                if !self.haulers[k].alive || self.haulers[k].site != a {
                    continue;
                }
                if have < self.eco.hauler.max_per_flock && self.haulers[k].state != HaulerState::Working {
                    self.haulers[k].site = b;
                    have += 1;
                } else if have < self.eco.hauler.max_per_flock && self.haulers[k].load > 0.0 {
                    // A loaded one lifts off to unload, then serves the other flock.
                    self.haulers[k].site = b;
                    have += 1;
                } else {
                    self.retire_hauler(k);
                }
            }
        }
        let live: Vec<usize> = live.into_iter().filter(|&h| !self.flock[h].dead).collect();
        // Split: too many cutters for one face.
        for &h in &live {
            if self.active(h) < spec.split_cutters || self.flock[h].calm_s < spec.calm_s {
                continue;
            }
            let (x, y) = self.site_pos(h);
            let r = (x * x + y * y).sqrt().max(1.0);
            let a = y.atan2(x);
            let step = spec.split_m / r;
            let mut best: Option<((f64, f64), f64)> = None;
            for side in [-1.0, 1.0] {
                let na = a + side * step;
                let (ux, uy) = (na.cos(), na.sin());
                let g = self.ground_radius(ux, uy, r + spec.split_m);
                if g <= 0.0 {
                    continue;
                }
                let p = (ux * (g + spec.hover_m), uy * (g + spec.hover_m));
                let room = live.iter().filter(|&&o| o != h).map(|&o| dist2(p, self.site_pos(o))).fold(f64::INFINITY, f64::min);
                if room > (spec.split_m * 0.5).powi(2) && best.is_none_or(|(_, b)| room > b) && self.site_danger(p) == 0 {
                    best = Some((p, room));
                }
            }
            let Some((p, _)) = best else { continue };
            if self.free_drones.is_none() && !self.pay_hauler() {
                continue;
            }
            let new = self.new_flock(p.0, p.1);
            moves.push((h, new, 2));
            self.flock[h].calm_s = 0.0;
            self.splits += 1;
            // The new ground may need opening up (a big body).
            if !self.surveyed_whole() {
                let l = (p.0 * p.0 + p.1 * p.1).sqrt().max(1.0);
                self.survey_zone(p.0 / l, p.1 / l);
            }
        }
        // Help: a flock on hard going (much slower per tonne than a
        // neighbour, but with work in front of it) draws cutters from it.
        let rate = |f: &FlockStats| if f.kg > 100.0 { f.dig_s / f.kg } else { f64::NAN };
        for &h in &live {
            let fh = self.flock[h];
            let hard = rate(&fh);
            if !(hard > 0.0) || fh.dry > 0.5 || fh.calm_s < spec.calm_s {
                continue;
            }
            let ph = self.site_pos(h);
            for &o in &live {
                let fo = self.flock[o];
                let easy = rate(&fo);
                if o == h || !(easy > 0.0) || fo.calm_s < spec.calm_s || self.hub_load[o] < 20 {
                    continue;
                }
                if hard < spec.hard_factor * easy || dist2(ph, self.site_pos(o)) > spec.comm_m * spec.comm_m {
                    continue;
                }
                let every = (1.0 / spec.help_share).round().max(1.0) as usize;
                helps.push(moves.len());
                moves.push((o, h, every));
                self.flock[o].calm_s = 0.0;
                self.flock[h].calm_s = 0.0;
                break;
            }
        }
        let moved = self.move_cutters(&moves);
        for i in helps {
            self.helpers_sent += moved[i];
        }
        // Spare haulers: one standing empty while another serves the flock.
        for &h in &live {
            let working: Vec<usize> = (0..self.haulers.len()).filter(|&k| self.haulers[k].alive && self.haulers[k].site == h && self.haulers[k].state == HaulerState::Working).collect();
            let total = self.haulers.iter().filter(|hl| hl.alive && hl.site == h).count();
            let empty = working.iter().copied().find(|&k| self.haulers[k].load <= 0.0);
            if total >= 2 && total as u32 > self.flock[h].want && self.site_wait[h].is_empty() && empty.is_some() {
                self.flock[h].idle_hauler_s += dt;
                if self.flock[h].idle_hauler_s >= spec.spare_hauler_s {
                    self.retire_hauler(empty.unwrap());
                    self.flock[h].idle_hauler_s = 0.0;
                }
            } else {
                self.flock[h].idle_hauler_s = 0.0;
            }
        }
        self.haulers.retain(|hl| hl.alive);
    }

    /// Flocks working now.
    pub fn flock_count(&self) -> usize {
        self.flock.iter().zip(&self.hubs).filter(|(f, h)| !f.dead && h.kind == HubKind::Seed).count()
    }

    // ---- Haulers ----

    /// Where the mother swarm is now: high over the ground, circling with the orbit.
    pub fn mother_pos(&self) -> (f64, f64) {
        let a = std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * self.time() / self.eco.seed_swarm.orbit_period_s;
        let r = self.world.planet.max_radius + self.eco.hauler.mother_alt_m;
        (a.cos() * r, a.sin() * r)
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
    fn hauler_way(&self, a: (f64, f64), b: (f64, f64)) -> f64 {
        route_len(a, b, route_cruise(a, b, self.world.planet.max_radius))
    }

    fn mats_add(&mut self, m: crate::materials::MatId, kg: f64) {
        if let Some(v) = self.transit_mats.get_mut(m as usize) {
            *v += kg;
            self.transit_mats_total += kg;
        }
    }

    /// Take `kg` out of what's on its way, its share of every material:
    /// delivered (into what was brought lately) or lost.
    fn mats_take(&mut self, kg: f64, delivered: bool) {
        if self.transit_mats_total <= 1e-9 || kg <= 0.0 {
            return;
        }
        let f = (kg / self.transit_mats_total).min(1.0);
        for ((t, r), c) in self.transit_mats.iter_mut().zip(self.recent_mats.iter_mut()).zip(self.collected_mats.iter_mut()) {
            let d = *t * f;
            *t -= d;
            if delivered {
                *r += d;
                *c += d;
            }
        }
        self.transit_mats_total -= self.transit_mats_total * f;
    }

    /// What has been brought up to the mother swarm lately: (material, kg),
    /// most first, at most `n`.
    pub fn recent_mix(&self, n: usize) -> Vec<(crate::materials::MatId, f64)> {
        let mut v: Vec<(crate::materials::MatId, f64)> = self.recent_mats.iter().enumerate().filter(|(_, k)| **k > 1.0).map(|(m, k)| (m as crate::materials::MatId, *k)).collect();
        v.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        v.truncate(n);
        v
    }

    fn can_expand(&self) -> bool {
        // With a factory: while no hauler is left waiting for a bay.
        if self.factory() {
            return self.prod.queued(crate::production::Unit::Hauler) == 0;
        }
        let els = &self.world.planet.mats.elements;
        let r = &self.eco.recipe;
        if r.hauler.parts.is_empty() {
            return self.mass >= self.eco.hauler.mass_cost + POOL_OPEN as f64 * self.eco.harvester.mass_cost;
        }
        // A hauler, then a first crew of cutters, all from what's on hand.
        let (mut s, mut p) = (self.stock.clone(), self.parts.clone());
        if p.build(&mut s, els, r, &r.hauler).is_none() {
            return false;
        }
        (0..POOL_OPEN / 4).all(|_| p.build(&mut s, els, r, &r.harvester).is_some())
    }

    /// Pay for a hauler (economy on): its energy, and its parts from the
    /// parts stock, any it is short of made at once from the stockpile (or
    /// its mass, with no recipe). Returns whether it could.
    fn pay_hauler(&mut self) -> bool {
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

    /// Pay for a cutter, as `pay_hauler`; `energy` is what it costs now.
    fn pay_cutter(&mut self, energy: f64) -> bool {
        let m = self.eco.harvester.mass_cost;
        if self.energy - energy <= 5.0 * self.upkeep() {
            return false;
        }
        if self.eco.recipe.harvester.parts.is_empty() {
            if self.mass < m {
                return false;
            }
            self.mass -= m;
            self.spent_mass += m;
        } else {
            match self.parts.build(&mut self.stock, &self.world.planet.mats.elements, &self.eco.recipe, &self.eco.recipe.harvester) {
                Some(kg) => self.stock_spent += kg,
                None => return false,
            }
        }
        self.energy -= energy;
        true
    }

    /// A hauler for flock h: ordered from the factory (it comes when built),
    /// or straight away with no factory.
    fn send_hauler(&mut self, h: usize) {
        if self.factory() {
            self.prod.orders.push_back(crate::production::Order::Hauler(h));
        } else {
            self.launch_hauler(h);
        }
    }

    /// Units are built in the factory, over time (the economy on, with a
    /// factory in the economy).
    pub fn factory(&self) -> bool {
        self.prod.on() && self.free_drones.is_none()
    }

    /// A new hauler leaves the mother swarm for flock h.
    fn launch_hauler(&mut self, h: usize) {
        let from = self.rendezvous(h);
        let to = self.site_pos(h);
        let t1 = self.tick + self.ticks(self.hauler_way(from, to) / self.eco.hauler.speed);
        // The seed swarm sets down with its first hauler already in place.
        let state = if self.tick == 0 { HaulerState::Working } else { HaulerState::Returning };
        let t1 = if self.tick == 0 { 0 } else { t1 };
        self.haulers.push(Hauler { site: h, state, load: 0.0, credited: 0.0, from, to, t0: self.tick, t1, life_left: self.eco.hauler.life_s, alive: true, lift_at: 0, mix: Default::default() });
    }

    /// One second of hauler work: lift off when full, unload, come back;
    /// wear out; and send more haulers where cutters are left waiting.
    fn hauler_work(&mut self) {
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
        self.direct();
    }

    /// The flow field for flock h's searches and flights from `at` (its
    /// hauler's spot), if one was swept from near there.
    fn field_for(&self, h: usize, at: (f64, f64)) -> Option<std::sync::Arc<FlowField>> {
        let f = self.fields.get(h)?.as_ref()?;
        (dist2(f.hub, at) <= FIELD_DRIFT_M * FIELD_DRIFT_M).then(|| f.clone())
    }

    /// Once a second: sweep the fields due (every FIELD_EVERY_S per flock,
    /// spread over the seconds, and at once for a flock new or moved), all
    /// side by side on the simulation's threads. Each sweep only reads the
    /// world, so the result is the same on any number of threads.
    fn flow_work(&mut self) {
        let sec = self.tick / self.ticks(1.0).max(1);
        if self.fields.len() < self.hubs.len() {
            self.fields.resize(self.hubs.len(), None);
        }
        let due: Vec<usize> = (0..self.hubs.len())
            .filter(|&h| {
                if self.flock[h].dead || self.hubs[h].kind == HubKind::Refiner {
                    return false;
                }
                match &self.fields[h] {
                    None => true,
                    Some(f) => (sec + h as u64) % FIELD_EVERY_S == 0 || dist2(f.hub, self.site_pos(h)) > FIELD_DRIFT_M * FIELD_DRIFT_M,
                }
            })
            .collect();
        for h in 0..self.hubs.len() {
            if self.flock[h].dead {
                self.fields[h] = None;
            }
        }
        if due.is_empty() {
            return;
        }
        let radius = self.eco.harvester.search_m + 16.0;
        let me: &Sim = self;
        let one = |&h: &usize| {
            let f = me.sweep(me.site_pos(h), radius);
            let cap = me.free_faces(&f, me.site_pos(h));
            (h, std::sync::Arc::new(f), cap)
        };
        let built: Vec<(usize, std::sync::Arc<FlowField>, u32)> = {
            use rayon::prelude::*;
            due.par_iter().map(one).collect()
        };
        if self.capacity.len() < self.hubs.len() {
            self.capacity.resize(self.hubs.len(), 0);
        }
        for (h, f, cap) in built {
            self.sweeps += 1;
            self.capacity[h] = cap;
            self.fields[h] = Some(f);
        }
        self.fields_version += 1;
    }

    /// One flock's field: out from `at` over the 2 m squares a drone can fly
    /// through (at least two of its four cells open: empty, or liquid it
    /// swims through). Untouched ground well under the surface is solid
    /// (unless a cave there has been broken into), and well above it is air,
    /// so only what has been dug and the surface itself are looked at cell by
    /// cell.
    fn sweep(&self, at: (f64, f64), radius: f64) -> FlowField {
        let mut rd = self.world.reader();
        let planet = &self.world.planet;
        let mats = &planet.mats;
        // The ground's height, a metre of arc at a time across the field.
        let r0 = (at.0 * at.0 + at.1 * at.1).sqrt().max(1.0);
        let a0 = at.1.atan2(at.0);
        let span = (radius * 1.5 / r0).min(std::f64::consts::PI);
        let steps = ((2.0 * span * r0).ceil() as usize).clamp(8, 4096);
        let ground: Vec<f64> = (0..=steps)
            .map(|i| {
                let a = a0 - span + 2.0 * span * i as f64 / steps as f64;
                planet.ground_hint(a.cos(), a.sin())
            })
            .collect();
        let ground_at = |x: f64, y: f64| -> f64 {
            let a = y.atan2(x);
            let mut d = a - a0;
            while d > std::f64::consts::PI {
                d -= std::f64::consts::TAU;
            }
            while d < -std::f64::consts::PI {
                d += std::f64::consts::TAU;
            }
            let k = ((d + span) / (2.0 * span) * steps as f64).round();
            if k < 0.0 || k > steps as f64 {
                planet.ground_hint(x / (x * x + y * y).sqrt().max(1e-9), y / (x * x + y * y).sqrt().max(1e-9))
            } else {
                ground[k as usize]
            }
        };
        let mut cell_open = |x: i64, y: i64| -> bool {
            let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
            let r = (fx * fx + fy * fy).sqrt();
            if !rd.touched(x, y) {
                let g = ground_at(fx, fy);
                if r > g + 2.0 {
                    // Drones fly low: the open sky well above the ground isn't
                    // swept (the field would mostly be empty air).
                    return r < g + FIELD_SKY_M;
                }
                if r < g - 6.0 {
                    return self.open.contains(&(x, y));
                }
            }
            let m = rd.get(x, y);
            m == VACUUM || mats.get(m).state == State::Liquid
        };
        FlowField::build(at, radius, |sx, sy| {
            let (x, y) = (sx * crate::flow::CELL, sy * crate::flow::CELL);
            let mut n = 0;
            for (dx, dy) in [(0, 0), (1, 1), (1, 0), (0, 1)] {
                if cell_open(x + dx, y + dy) {
                    n += 1;
                    if n >= 2 {
                        return true;
                    }
                }
            }
            false
        })
    }

    /// A drone with nothing to do joins the pool resting in the mother swarm.
    fn pool_drone(&mut self, id: u32) {
        let d = &mut self.drones[id as usize];
        d.last = None;
        d.phase = Phase::Idle;
        d.fails = d.fails.max(1);
        let home = d.home;
        if self.pooled_at.len() < self.hubs.len() {
            self.pooled_at.resize(self.hubs.len(), 0);
        }
        self.pooled_at[home] += 1;
        self.parked.push(id);
    }

    /// Drones of flock h actually working for it (not resting in the pool).
    fn active(&self, h: usize) -> u32 {
        self.hub_load[h].saturating_sub(self.pooled_at.get(h).copied().unwrap_or(0))
    }

    /// How many more drones the faces flock h can reach (by its field, within
    /// its search reach) have room for: the faces no drone has claimed.
    fn free_faces(&self, f: &FlowField, at: (f64, f64)) -> u32 {
        let reach = self.eco.harvester.search_m;
        let (bx0, bx1) = (((at.0 - reach).floor() as i64).div_euclid(BUCKET), ((at.0 + reach).floor() as i64).div_euclid(BUCKET));
        let (by0, by1) = (((at.1 - reach).floor() as i64).div_euclid(BUCKET), ((at.1 + reach).floor() as i64).div_euclid(BUCKET));
        let mut n = 0u32;
        for by in by0..=by1 {
            for bx in bx0..=bx1 {
                for (c, _) in self.buckets.get(&(bx, by)).into_iter().flatten() {
                    if !self.reserved.contains_key(c) && f.reach_m(*c).is_some_and(|d| d <= reach) {
                        n += 1;
                    }
                }
            }
        }
        (n as f64 * DRONES_PER_FACE) as u32
    }

    /// The director's round with the pool, every POOL_EVERY_S: each flock
    /// gets drones from the pool for the work it has room for (its faces
    /// no drone has claimed, as last counted), most room first; with
    /// many left over and no room anywhere, it opens new flocks on faces no
    /// flock works. Only drones that move cost anything.
    fn allocate(&mut self) {
        // The pool, as it stands (drones may have died in it).
        let drones = &self.drones;
        self.parked.retain(|&id| {
            let d = &drones[id as usize];
            d.alive && d.phase == Phase::Idle
        });
        self.pooled_at = vec![0; self.hubs.len()];
        for &id in &self.parked {
            self.pooled_at[self.drones[id as usize].home] += 1;
        }
        if self.capacity.len() < self.hubs.len() {
            self.capacity.resize(self.hubs.len(), 0);
        }
        let pooled = self.parked.len();
        // Room at each flock.
        let mut room: Vec<(u32, usize)> = (0..self.hubs.len())
            .filter(|&h| !self.flock[h].dead && self.hubs[h].kind == HubKind::Seed && self.fields.get(h).is_some_and(|f| f.is_some()))
            // (Not to a flock whose cutters already stand waiting for a hauler:
            // more would only wait too. And a few at a time, so its haulers
            // and the director can keep up.)
            .filter(|&h| self.site_wait[h].len() < self.eco.hauler.wait_trigger as usize)
            // (Nor to one already needing as many haulers as a flock may
            // have: what it cuts can't be carried away any faster.)
            .filter(|&h| self.flock[h].want < self.eco.hauler.max_per_flock)
            .filter_map(|h| {
                let free = self.capacity[h].min(POOL_SEND_MAX);
                (free > 0).then_some((free, h))
            })
            .collect();
        room.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        let total_room: u64 = room.iter().map(|r| r.0 as u64).sum();
        let mut sent = 0usize;
        let mut flocks = 0usize;
        for (free, h) in room {
            if self.parked.is_empty() {
                break;
            }
            flocks += 1;
            for i in 0..free as usize {
                let Some(id) = self.parked.pop() else { break };
                let old = self.drones[id as usize].home;
                self.pooled_at[old] = self.pooled_at[old].saturating_sub(1);
                if old != h {
                    self.hub_load[old] -= 1;
                    self.hub_load[h] += 1;
                }
                let d = &mut self.drones[id as usize];
                d.home = h;
                d.fails = 0;
                d.last = None;
                // (Down from the mother swarm over the next few seconds.)
                self.events.push(Reverse((self.tick + (i as u64 % 40), id)));
                self.leg_changed(id);
                sent += 1;
                // (Until its field is swept again, that room is taken.)
                self.capacity[h] = self.capacity[h].saturating_sub(1);
            }
        }
        self.pool_sent += sent as u64;
        if sent > 0 && self.tick >= self.pool_noted {
            self.pool_noted = self.tick + self.ticks(30.0);
            self.director_log_push(format!("pool: sent {sent} of {pooled} resting drones to {flocks} flocks"));
        }
        // Room left after this round's sends: what new cutters may fill.
        self.last_room = total_room.saturating_sub(sent as u64);
        // Many left with nowhere to go: open new ground.
        let left = self.parked.len();
        // (With the economy on, few are ever left over, since no more are
        // built than there is room for: the swarm grows onto new ground when
        // the flocks are full and the stockpile can pay for a new flock's
        // hauler and a first crew of cutters.)
        if self.free_drones.is_none() && total_room == 0 && left < POOL_OPEN && self.flock_count() < MAX_FLOCKS && self.can_expand() {
            let opened = self.open_flocks(1);
            if opened > 0 {
                self.flocks_opened += opened as u64;
                self.director_log_push("the flocks are full: opened a new flock on unworked ground".to_string());
            }
        } else if left >= POOL_OPEN && (left as u64) > total_room && self.flock_count() < MAX_FLOCKS {
            let opened = self.open_flocks(NEW_FLOCKS_PER_ROUND);
            if opened > 0 {
                self.flocks_opened += opened as u64;
                self.director_log_push(format!("{left} drones out of work: opened {opened} new flocks on unworked faces"));
            }
        }
    }

    /// Open up to `n` new flocks on faces no flock works: on the face
    /// buckets furthest from nothing, that is, the nearest that are at least
    /// NEW_FLOCK_GAP_M from every working flock (the swarm spreads outward
    /// from where it already is), safe from lava, with open space to hover
    /// in. Returns how many it opened.
    fn open_flocks(&mut self, n: usize) -> usize {
        let live: Vec<(f64, f64)> = (0..self.hubs.len()).filter(|&h| !self.flock[h].dead && self.hubs[h].kind == HubKind::Seed).map(|h| self.site_pos(h)).collect();
        let gap2 = NEW_FLOCK_GAP_M * NEW_FLOCK_GAP_M;
        // On a body too big to survey whole, only ground near where the
        // swarm has landed is known: open up the ground just past the edge
        // of the worked ground first, either side of each flock, where no
        // flock is near (the swarm spreads outward from where it is).
        if !self.surveyed_whole() {
            let mut surveyed = 0;
            for &q in &live {
                if surveyed >= n {
                    break;
                }
                let r = (q.0 * q.0 + q.1 * q.1).sqrt().max(1.0);
                let a = q.1.atan2(q.0);
                let step = 1.5 * NEW_FLOCK_GAP_M / r;
                for side in [1.0, -1.0] {
                    let b = a + side * step;
                    let (ux, uy) = (b.cos(), b.sin());
                    let g = self.world.planet.surface_radius(ux, uy);
                    let p = (ux * g, uy * g);
                    if self.zones_opened.contains(&((b * 1000.0).round() as i64)) || live.iter().any(|q| dist2(p, *q) < gap2) {
                        continue;
                    }
                    self.zones_opened.insert((b * 1000.0).round() as i64);
                    self.survey_zone(ux, uy);
                    surveyed += 1;
                    if surveyed >= n {
                        break;
                    }
                }
            }
        }
        let mut keys: Vec<(i64, i64)> = self.buckets.iter().filter(|(_, v)| v.len() >= 4).map(|(k, _)| *k).collect();
        keys.sort_unstable();
        // (a face of the bucket open to space, distance squared to the
        // nearest flock)
        let mut cands: Vec<(f64, (i64, i64))> = Vec::new();
        for k in keys {
            let list = &self.buckets[&k];
            let open_face = list.iter().map(|x| x.0).find(|c| [(0, 1), (0, -1), (1, 0), (-1, 0)].iter().any(|d| self.world.material(c.0 + d.0, c.1 + d.1) == VACUUM));
            let Some(c) = open_face else { continue };
            let p = (c.0 as f64 + 0.5, c.1 as f64 + 0.5);
            let near = live.iter().map(|q| dist2(p, *q)).fold(f64::INFINITY, f64::min);
            if near >= gap2 {
                cands.push((near, c));
            }
        }
        cands.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut opened: Vec<(f64, f64)> = Vec::new();
        let dbg = std::env::var("DFT_DEBUG_OPEN").is_ok();
        let (mut no_open, mut close, mut danger) = (0, 0, 0);
        let ncands = cands.len();
        for (_, c) in cands {
            if opened.len() >= n {
                break;
            }
            // Hover over the open side of the face, a little way out.
            let Some(open) = [(0, 1), (0, -1), (1, 0), (-1, 0)].into_iter().map(|d| (c.0 + d.0, c.1 + d.1)).find(|o| self.world.material(o.0, o.1) == VACUUM) else {
                no_open += 1;
                continue;
            };
            let (dx, dy) = ((open.0 - c.0) as f64, (open.1 - c.1) as f64);
            let mut p = (open.0 as f64 + 0.5, open.1 as f64 + 0.5);
            for _ in 0..(self.eco.flock.hover_m as i64) {
                let q = (p.0 + dx, p.1 + dy);
                if self.world.material(q.0.floor() as i64, q.1.floor() as i64) != VACUUM {
                    break;
                }
                p = q;
            }
            if opened.iter().any(|q| dist2(p, *q) < gap2) {
                close += 1;
                continue;
            }
            if self.site_danger(p) > 0 {
                danger += 1;
                continue;
            }
            if self.free_drones.is_none() && !self.pay_hauler() {
                break;
            }
            self.new_flock(p.0, p.1);
            opened.push(p);
        }
        if dbg {
            eprintln!("open_flocks t={:.0}: buckets {} cands {} no_open {} close {} danger {} opened {}", self.time(), self.buckets.len(), ncands, no_open, close, danger, opened.len());
        }
        opened.len()
    }

    /// The swarm director, once a second: works out how many haulers each
    /// flock really needs and decides whether another is worth building.
    ///
    /// Need: one hauler at the flock filling while the others make the trip:
    /// one, plus one for each time it would fill during a round trip (a gap
    /// of up to a quarter of its filling time is let be), from the last few
    /// minutes (a moment's crowd isn't chronic demand); one more if cutters
    /// have been left waiting all the same.
    ///
    /// Worth: a hauler frees the cutters waiting for one to cut again, so it
    /// earns what they would cut; spent on cutters instead, the same mass
    /// earns what new cutters cut. The director builds the hauler when it
    /// pays at least as well per kilogram (with the economy off, haulers are
    /// free and it always does). A flock with no hauler at all, or with
    /// cutters waiting and none at it or on its way, gets one at once.
    /// Several can be on their way. The last few calls, and why, are kept
    /// for the F1 panel.
    fn direct(&mut self) {
        let dt = self.ticks(1.0) as f64 * self.eco.tick_s;
        let spec = self.eco.hauler.clone();
        let fade = (-dt / DIRECTOR_MEMORY_S).exp();
        let cap = spec.capacity_kg * self.up_cached_cargo;
        // What a working cutter brings in, and what one costs.
        let per_cutter = self.rate_per_drone.max(1e-6);
        let cutter_mass = self.eco.harvester.mass_cost.max(1.0);
        for site in 0..self.hubs.len() {
            let waiting = self.site_wait[site].len();
            {
                let f = &mut self.flock[site];
                f.wait_cs = f.wait_cs * fade + waiting as f64 * dt;
                f.filled *= fade;
            }
            if self.flock[site].dead || self.hubs[site].kind != HubKind::Seed {
                continue;
            }
            let (mut n, mut here, mut coming) = (0u32, 0u32, 0u32);
            for hl in self.haulers.iter().filter(|h| h.alive && h.site == site) {
                n += 1;
                match hl.state {
                    HaulerState::Working => here += 1,
                    HaulerState::Returning => coming += 1,
                    _ => {}
                }
            }
            // (Those on order from the factory are on their way too.)
            let ordered = self.prod.haulers_for(site);
            n += ordered;
            coming += ordered;
            let f = self.flock[site];
            // Rates over the remembered stretch (a sum fading by `fade` each
            // second settles at rate x DIRECTOR_MEMORY_S).
            let fill_rate = f.filled / DIRECTOR_MEMORY_S;
            let waiting_avg = f.wait_cs / DIRECTOR_MEMORY_S;
            let trip = if f.trip_s > 0.0 {
                f.trip_s
            } else {
                // No trip timed yet: up to the mother swarm and back, plus unloading.
                2.0 * dist2(self.site_pos(site), self.rendezvous(site)).sqrt() / spec.speed.max(1.0) + spec.unload_s as f64
            };
            // How long one takes to fill (it leaves after max_stay_s anyway),
            // and so how many are away at once while one fills: a gap of a
            // quarter of the filling time or less is let be.
            let fill_s = (cap / fill_rate.max(1e-6)).min(spec.max_stay_s as f64).max(1.0);
            let mut want = 1 + (trip / fill_s + 0.75).floor() as u32;
            // (Only while none is already on its way: one sent takes a while
            // to come, and asking for more meanwhile only piles them up.)
            if waiting_avg >= spec.wait_trigger as f64 && coming == 0 {
                want = want.max(n + 1);
            }
            let want = want.clamp(1, spec.max_per_flock);
            self.flock[site].want = want;
            let urgent = (n == 0 && self.active(site) > 0) || (here == 0 && coming == 0 && waiting >= spec.wait_trigger as usize);
            if n >= spec.max_per_flock || (n >= want && !urgent) {
                continue;
            }
            let why = if n == 0 {
                "it has none".to_string()
            } else if urgent {
                format!("{waiting} cutters waiting, none at it or on the way")
            } else {
                format!("needs {want}: {:.1} t/s in, {:.0} s round trip, {:.0} waiting on average", fill_rate / 1000.0, trip, waiting_avg)
            };
            if self.free_drones.is_none() {
                // Worth it? What the waiting cutters would cut, per kg of
                // hauler, against what cutters bought instead would.
                let hauler_pays = waiting_avg.max(waiting as f64) * per_cutter / spec.mass_cost.max(1.0);
                let cutters_pay = per_cutter / cutter_mass;
                if !urgent && hauler_pays < cutters_pay {
                    // (Only worth a note when cutters are actually waiting.)
                    if waiting_avg >= 1.0 {
                        self.director_note(site, format!("flock {site}: no hauler ({why}), cutters pay more"));
                    }
                    continue;
                }
                if !self.pay_hauler() {
                    self.director_note(site, format!("flock {site}: hauler wanted ({why}), can't afford it"));
                    continue;
                }
            }
            self.send_hauler(site);
            if self.factory() {
                self.director_log_push(format!("flock {site}: hauler {} ordered ({why})", n + 1));
            } else {
                self.haulers_built += 1;
                self.director_log_push(format!("flock {site}: hauler {} ({why})", n + 1));
            }
        }
    }

    /// A call the director didn't act on, noted at most once a minute per flock.
    fn director_note(&mut self, site: usize, what: String) {
        let now = self.tick;
        if self.director_quiet.get(site).is_some_and(|&t| now < t) {
            return;
        }
        if self.director_quiet.len() <= site {
            self.director_quiet.resize(site + 1, 0);
        }
        self.director_quiet[site] = now + self.ticks(60.0);
        self.director_log_push(what);
    }

    fn director_log_push(&mut self, what: String) {
        let t = self.time();
        self.director_log.push_back(format!("{:>2}:{:02} {what}", (t / 60.0) as u64, (t % 60.0) as u64));
        while self.director_log.len() > 8 {
            self.director_log.pop_front();
        }
    }

    /// All the matter the swarm holds, kg: (in the stockpile, on its way
    /// up in holds and haulers, built into cutters, built into haulers,
    /// grown into the mother swarm). Their sum is the round's score.
    pub fn matter(&self) -> [f64; 6] {
        let cutters = (self.alive_count as f64 + self.packet as f64) * self.eco.harvester.mass_cost;
        let haulers = self.haulers.iter().filter(|h| h.alive).count() as f64 * self.eco.hauler.mass_cost;
        let r = &self.eco.recipe;
        let in_bays: f64 = self.prod.bays.iter().filter_map(|b| b.job).map(|j| r.kg(if j.order.unit() == crate::production::Unit::Cutter { &r.harvester } else { &r.hauler })).sum();
        [self.stock.total(), self.transit.total, self.parts.kg(r) + in_bays, cutters, haulers, self.mother.grown_kg]
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

    /// Free work may have appeared at cell c (a new face, or a reservation
    /// given up): hubs within reach of it that were dry may search again,
    /// and if any hub can reach it, a couple of waiting drones wake (they go
    /// on to the next hub that isn't dry).
    fn work_near(&mut self, c: (i64, i64)) {
        let (cx, cy) = (c.0 as f64 + 0.5, c.1 as f64 + 0.5);
        let reach = self.eco.harvester.search_m;
        let mut reachable = false;
        for h in 0..self.hubs.len() {
            let (dx, dy) = (self.hubs[h].x - cx, self.hubs[h].y - cy);
            if dx * dx + dy * dy > reach * reach {
                continue;
            }
            reachable = true;
            self.hub_dry[h] = false;
            self.hub_gen += 1;
        }
        let _ = reachable;
    }

    /// Every hub may search again and every waiting drone wakes (after
    /// something that changes what drones will take, and now and then in
    /// case some change was missed).
    fn all_hubs_wet(&mut self) {
        for h in 0..self.hubs.len() {
            self.hub_dry[h] = false;
            self.hub_gen += 1;
        }
    }

    fn on_trip(&self) -> u32 {
        self.on_trip_count
    }

    fn brownout(&self) -> bool {
        self.energy <= 0.0
    }

    // ---- Building and buying ----

    fn try_build(&mut self) {
        if let Some(target) = self.free_drones {
            // Grow toward the target by about 1% of it a second (at least 10),
            // spreading the new drones around the orbit like the real swarm.
            let n = target.saturating_sub(self.harvesters_alive()).min((target / 100).max(10));
            for _ in 0..n {
                self.spawn_harvester();
            }
            // Never short of energy, so nothing browns out.
            self.energy = self.energy.max(1e12);
            return;
        }
        // With a factory: cutters are ordered, as many as there is work
        // for less those on order, and built in its bays.
        let factory = self.factory();
        if factory {
            self.production_work();
        }
        // Parts made ahead from the stockpile, kept in stock (with no
        // factory: the fabricator makes them, over time).
        if !factory && !self.eco.recipe.part.is_empty() {
            self.stock_spent += self.parts.make_ahead(&mut self.stock, &self.world.planet.mats.elements, &self.eco.recipe);
        }
        let reserve = 5.0 * self.upkeep();
        // A Refiner when hauls get long.
        if self.refiner_unlocked
            && self.avg_haul > self.eco.refiner.min_haul_m
            && self.energy - self.eco.refiner.energy_cost > reserve
            && self.mass >= self.eco.refiner.mass_cost
        {
            if let Some(c) = self.last_far_cell.take() {
                let (x, y) = (c.0 as f64 + 0.5, c.1 as f64 + 0.5);
                let h = self.nearest_hub(x, y);
                if ((self.hubs[h].x - x) * (self.hubs[h].x - x) + (self.hubs[h].y - y) * (self.hubs[h].y - y)).sqrt() > self.eco.refiner.spacing_m {
                    self.energy -= self.eco.refiner.energy_cost;
                    self.mass -= self.eco.refiner.mass_cost;
                    self.spent_mass += self.eco.refiner.mass_cost;
                    self.hubs.push(Hub { x, y, kind: HubKind::Refiner, ring: None });
                    self.hub_load.push(0);
                    self.hub_dry.push(false);
                    self.hub_gen += 1;
                    self.site_wait.push(Vec::new());
                    self.flock.push(FlockStats::default());
                    // Drones whose landing point is far from their work move here.
                    let new = self.hubs.len() - 1;
                    self.legs_all_changed();
                    for (i, d) in self.drones.iter_mut().enumerate() {
                        if d.alive && i % 3 == 0 {
                            self.hub_load[d.home] -= 1;
                            self.hub_load[new] += 1;
                            d.home = new;
                        }
                    }
                }
            }
        }
        // Tunnelers: a few big machines, one per so many Harvesters.
        if self.tunneler_unlocked {
            let t = &self.eco.tunneler;
            let want = ((self.harvesters_alive() as f64 / t.per_harvesters) as u32).clamp(1, t.max_count);
            if (self.tunnelers.len() as u32) < want && self.energy - t.energy_cost > reserve && self.mass >= t.mass_cost {
                self.energy -= t.energy_cost;
                self.mass -= t.mass_cost;
                self.spent_mass += t.mass_cost;
                self.tunnelers.push(Bore { x: 0.0, y: 0.0, hx: 0.0, hy: 0.0, aim: (0.0, 0.0), length: 0.0, progress_s: 0.0, active: false, resume_at: self.tick, prev: (0.0, 0.0), moved_at: self.tick });
            }
        }
        // Scouts, to keep the ratio.
        let harvesters = self.harvesters_alive();
        if self.scout_unlocked && (self.scouts as f64) < harvesters as f64 / self.eco.scout.per_harvesters {
            if self.energy - self.eco.scout.energy_cost > reserve && self.mass >= self.eco.scout.mass_cost {
                self.energy -= self.eco.scout.energy_cost;
                self.mass -= self.eco.scout.mass_cost;
                self.spent_mass += self.eco.scout.mass_cost;
                self.scouts += 1;
                if self.scouts == 1 {
                    self.update_sense();
                }
            }
        }
        // Cutters: only as many as the flocks have work for (no more than
        // their room, less those already resting in the pool), as fast as
        // the stockpile and energy allow. The swarm has no cap.
        let mut alive = self.harvesters_alive();
        let resting = self.parked.len() as u64;
        if factory {
            use crate::production::{Order, Unit};
            let on_order = (self.prod.queued(Unit::Cutter) + self.prod.building(Unit::Cutter)) as u64;
            let want = self.last_room.saturating_sub(resting + on_order);
            let room = (self.eco.production.cutter_orders_per_bay as usize * self.prod.bays_of(Unit::Cutter)).saturating_sub(self.prod.queued(Unit::Cutter)) as u64;
            for _ in 0..want.min(room) {
                self.prod.orders.push_back(Order::Cutter);
                self.last_room -= 1;
            }
            // The guaranteed minimum, as below.
            if self.tick >= self.next_free_build && (want > 0 || alive < 10) {
                self.spawn_harvester();
                self.next_free_build = self.tick + self.ticks(self.eco.seed_swarm.free_build_every_s);
            }
            // Specialists, from the Universal Drone Replicator's bays.
            self.order_specialists();
            return;
        }
        let mut want = self.last_room.saturating_sub(resting);
        while want > 0 {
            let cost = self.eco.harvester.energy_cost * self.perk_energy / self.up("fabrication") * crate::noise::powi(self.eco.harvester.energy_cost_growth, alive.min(i32::MAX as u64) as i32);
            if !self.pay_cutter(cost) {
                break;
            }
            self.spawn_harvester();
            alive += 1;
            want -= 1;
            self.last_room -= 1;
            self.cutters_built_for_room += 1;
        }
        let _ = reserve;
        // The guaranteed minimum: a free Harvester every so often, while
        // there is work for one (or almost no swarm left).
        if self.tick >= self.next_free_build && (want > 0 || alive < 10) {
            self.spawn_harvester();
            self.next_free_build = self.tick + self.ticks(self.eco.seed_swarm.free_build_every_s);
        }
    }

    /// One second of production: the fabricator makes parts, the factory's
    /// bays finish units and start on the next orders.
    fn production_work(&mut self) {
        let dt = self.ticks(1.0) as f64 * self.eco.tick_s;
        // The fuser tops up what nothing else can make (uranium), from
        // energy above the reserve.
        let available = self.energy - 5.0 * self.upkeep();
        self.energy -= self.fuser.work(&self.eco.fuser, &self.world.planet.mats.elements, &mut self.stock, dt, available);
        self.fabricate(dt);
        // Orders for flocks gone since are dropped.
        let flock = &self.flock;
        self.prod.orders.retain(|o| match o {
            crate::production::Order::Hauler(s) => !flock.get(*s).is_some_and(|f| f.dead),
            _ => true,
        });
        // Each bay works through the second: a quick unit done, it starts
        // the next, as long as there are orders, parts and energy.
        for b in 0..self.prod.bays.len() {
            let mut budget = dt;
            for _ in 0..100_000 {
                if self.prod.bays[b].job.is_none() && !self.start_unit(b) {
                    break;
                }
                let Some(mut j) = self.prod.bays[b].job else { break };
                if j.left_s > budget {
                    j.left_s -= budget;
                    self.prod.bays[b].job = Some(j);
                    break;
                }
                budget -= j.left_s;
                self.prod.bays[b].job = None;
                self.finish_unit(j.order);
            }
        }
    }

    /// The fabricator: parts from refined elements, one kind at a time.
    /// First the parts the next hauler waiting still lacks (if a hauler bay
    /// is free for it), then the next cutter's (the same), then those for
    /// every order waiting and the stock kept ahead. That decides which kind
    /// it makes; once on a kind it makes a whole run of it (all that every
    /// order waiting and the stock ahead lack) before switching, since
    /// every switch costs time; then it takes the kind furthest short (by
    /// mass) in the most urgent need.
    fn fabricate(&mut self, dt: f64) {
        // Through the whole second: a run finished, it switches and goes on.
        let mut budget = dt;
        for _ in 0..64 {
            if budget <= 1e-9 {
                break;
            }
            let used = self.fab_step(budget);
            if used <= 0.0 || self.prod.fab.idle || self.prod.fab.stalled {
                break;
            }
            budget -= used;
        }
    }

    /// One stretch of the fabricator's work, up to `budget` seconds: a
    /// switch, or making parts until the run is done or the time is up.
    /// Returns the seconds it used.
    fn fab_step(&mut self, budget: f64) -> f64 {
        use crate::production::Unit;
        let r = &self.eco.recipe;
        let n = r.part.len();
        if n == 0 {
            return 0.0;
        }
        let (hc, uc) = (r.counts(&r.harvester), r.counts(&r.hauler));
        let (qc, qh) = (self.prod.queued(Unit::Cutter) as u64, self.prod.queued(Unit::Hauler) as u64);
        let have = &self.parts.n;
        let free = |u: Unit| (self.prod.building(u) < self.prod.bays_of(u)) as u64;
        // The specialists' parts: for one waiting per free bay, and for all waiting.
        let (mut sp_free, mut sp_all) = (vec![0u64; n], vec![0u64; n]);
        for u in Unit::SPECIALISTS {
            let q = self.prod.queued(u) as u64;
            if q == 0 {
                continue;
            }
            let c = r.counts(&self.recipe_now(u));
            for i in 0..n {
                sp_free[i] += q.min(free(u)) * c[i];
                sp_all[i] += q * c[i];
            }
        }
        let tier = |c: u64, h: u64, s: &Vec<u64>| -> Vec<u64> { (0..n).map(|i| (c * hc[i] + h * uc[i] + s[i]).saturating_sub(have[i])).collect() };
        let none = vec![0u64; n];
        let (nh, nc) = (qh.min(free(Unit::Hauler)), qc.min(free(Unit::Cutter)));
        let tiers = [tier(0, nh, &none), tier(nc, nh, &none), tier(nc, nh, &sp_free), tier(r.ahead.harvester as u64 + qc, r.ahead.hauler as u64 + qh, &sp_all)];
        let short = tiers[3].clone();
        let kg: Vec<f64> = r.part.iter().map(|p| p.kg()).collect();
        let fab = &self.prod.fab;
        if fab.part.is_none_or(|p| short[p] == 0) {
            // The most urgent need the stockpile can make (a part it can't,
            // such as a sapper's charge with no uranium left, waits).
            let els = &self.world.planet.mats.elements;
            let makeable = |i: usize| self.stock.plan_refined(els, &r.part[i].make.iter().map(|c| (c, c.kg)).collect::<Vec<_>>()).is_some();
            let mut wanted = false;
            let mut best = None;
            for t in &tiers {
                if !t.iter().any(|&k| k > 0) {
                    continue;
                }
                wanted = true;
                best = (0..n).filter(|&i| t[i] > 0 && makeable(i)).max_by(|&a, &b| (t[a] as f64 * kg[a]).total_cmp(&(t[b] as f64 * kg[b])).then(b.cmp(&a)));
                if best.is_some() {
                    break;
                }
            }
            let spec = &self.eco.production;
            let fab = &mut self.prod.fab;
            match best {
                None => {
                    fab.part = None;
                    fab.progress_kg = 0.0;
                    fab.stalled = wanted;
                    fab.idle = !wanted;
                    return 0.0;
                }
                Some(p) => {
                    if fab.part.is_some() {
                        fab.switch_left_s = spec.fab_switch_s;
                    }
                    fab.part = Some(p);
                    fab.progress_kg = 0.0;
                }
            }
        }
        let spec = &self.eco.production;
        let fab = &mut self.prod.fab;
        fab.idle = false;
        if fab.switch_left_s > 0.0 {
            let used = fab.switch_left_s.min(budget);
            fab.switch_left_s -= used;
            return used;
        }
        let Some(p) = fab.part else { return 0.0 };
        let rate = spec.fab_kg_per_s.max(1e-9);
        let work = fab.progress_kg + rate * budget;
        let mut k = ((work / kg[p]).floor() as u64).min(short[p]);
        if k == 0 {
            // A big part, not done this stretch.
            fab.progress_kg = work;
            return budget;
        }
        loop {
            let comps: Vec<(&crate::recipes::Component, f64)> = r.part[p].make.iter().map(|c| (c, c.kg * k as f64)).collect();
            if let Some(take) = self.stock.plan_refined(&self.world.planet.mats.elements, &comps) {
                self.stock_spent += self.stock.take(&take);
                self.parts.n[p] += k;
                self.parts.made[p] += k;
                fab.stalled = false;
                let left = work - k as f64 * kg[p];
                if k == short[p] {
                    // The run is done: the time left goes on.
                    fab.progress_kg = 0.0;
                    return (budget - left / rate).clamp(1e-6, budget);
                }
                // (Work isn't banked beyond the part in hand.)
                fab.progress_kg = left.min(kg[p]);
                return budget;
            }
            if k == 1 {
                // It can't be made now: next time, another part.
                fab.stalled = true;
                fab.part = None;
                fab.progress_kg = 0.0;
                return budget;
            }
            k /= 2;
        }
    }

    /// Bay b starts on the first order for its kind of unit, if its parts
    /// are in stock and there is the energy to build it. Returns whether it did.
    fn start_unit(&mut self, b: usize) -> bool {
        use crate::production::{Job, Unit};
        let unit = self.prod.bays[b].unit;
        let Some(k) = self.prod.orders.iter().position(|o| o.unit() == unit) else { return false };
        let us = &self.eco.units;
        let energy = match unit {
            Unit::Cutter => self.eco.harvester.energy_cost * self.perk_energy / self.up("fabrication") * crate::noise::powi(self.eco.harvester.energy_cost_growth, self.harvesters_alive().min(i32::MAX as u64) as i32),
            Unit::Hauler => self.eco.hauler.energy_cost,
            Unit::Sapper => us.sapper.energy_cost,
            Unit::Tunneler => us.tunneler.energy_cost,
            Unit::HeatSink => us.heatsink.energy_cost,
            Unit::Forester => us.forester.energy_cost,
            Unit::Abductor => us.abductor.energy_cost,
        };
        // (Specialists wait while a hauler waits on a free bay: a flock
        // without one can't work at all.)
        let swarm_waiting = !matches!(unit, Unit::Cutter | Unit::Hauler) && self.prod.queued(Unit::Hauler) > 0 && self.prod.building(Unit::Hauler) < self.prod.bays_of(Unit::Hauler);
        if swarm_waiting {
            return false;
        }
        // (A hauler waiting on energy goes first: cutters are no use until
        // there is a hauler to fill.)
        let held = if unit == Unit::Cutter && self.prod.queued(Unit::Hauler) > 0 && self.prod.building(Unit::Hauler) < self.prod.bays_of(Unit::Hauler) { self.eco.hauler.energy_cost } else { 0.0 };
        if self.energy - energy - held <= 5.0 * self.upkeep() {
            return false;
        }
        let rec = self.recipe_now(unit);
        if !self.parts.can_claim(&self.eco.recipe, &rec) {
            return false;
        }
        // A sapper's charge: conventional ones are cast now, from the stockpile.
        if unit == Unit::Sapper && !self.cast_charge() {
            return false;
        }
        self.parts.claim(&self.eco.recipe, &rec);
        self.energy -= energy;
        let order = self.prod.orders.remove(k).expect("found above");
        self.prod.bays[b].job = Some(Job { order, left_s: self.prod.bays[b].build_s });
        true
    }

    /// A unit leaves the factory: a cutter drops to the flock that needs
    /// one most, a hauler goes to the flock it was built for (or, if that
    /// flock is gone, the one with the most cutters waiting).
    fn finish_unit(&mut self, order: crate::production::Order) {
        use crate::production::Order;
        match order {
            Order::Cutter => {
                self.spawn_harvester();
                self.cutters_built_for_room += 1;
                self.prod.built[0] += 1;
            }
            Order::Hauler(site) => {
                let live = |i: usize| self.flock.get(i).is_some_and(|f| !f.dead) && self.hubs[i].kind == HubKind::Seed;
                let to = if live(site) { Some(site) } else { (0..self.hubs.len()).filter(|&i| live(i)).max_by_key(|&i| (self.site_wait[i].len(), std::cmp::Reverse(i))) };
                if let Some(s) = to {
                    self.launch_hauler(s);
                    self.director_log_push(format!("flock {s}: a hauler leaves the factory"));
                }
                self.haulers_built += 1;
                self.prod.built[1] += 1;
            }
            Order::Special(u) => {
                self.launch_special(u);
                self.prod.built[u.index()] += 1;
            }
        }
    }

    /// What production is doing, for the F1 panel.
    pub fn production_lines(&self) -> Vec<String> {
        use crate::production::{Order, Unit};
        if !self.factory() {
            return vec!["no factory: units are built at once".into()];
        }
        let mut v = Vec::new();
        let r = &self.eco.recipe;
        let f = &self.prod.fab;
        v.push(match f.part {
            _ if f.idle => "fabricator idle: every part wanted is in stock".into(),
            None if f.stalled => "fabricator waiting: nothing wanted can be made from the stockpile".into(),
            None => "fabricator idle".into(),
            Some(p) if f.switch_left_s > 0.0 => format!("fabricator switching to {} ({:.0} s)", r.part[p].name.to_lowercase(), f.switch_left_s),
            Some(p) if f.stalled => format!("fabricator stalled on {}: the stockpile can't make it", r.part[p].name.to_lowercase()),
            Some(p) => format!("fabricator making {} ({} in stock)", r.part[p].name.to_lowercase(), self.parts.n[p]),
        });
        for u in Unit::ALL {
            if self.prod.bays_of(u) > 0 {
                v.push(format!("{} bays  {} of {} busy · {} waiting", u.name(), self.prod.building(u), self.prod.bays_of(u), self.prod.queued(u)));
            }
        }
        for (i, b) in self.prod.bays.iter().enumerate() {
            if let Some(j) = b.job {
                let what = match j.order {
                    Order::Cutter => "a cutter".to_string(),
                    Order::Hauler(s) => format!("a hauler for flock {s}"),
                    Order::Special(u) => format!("a {}", u.name()),
                };
                v.push(format!("  bay {}: {what}, {:.0} s left", i + 1, j.left_s.max(0.0)));
            }
        }
        v.push(format!("built   {} cutters, {} haulers", self.prod.built[0], self.prod.built[1]));
        v.extend(self.specialist_lines());
        if let Some(l) = &self.life {
            v.extend(l.lines());
        }
        v
    }

    /// One second of packet work. Packets mine whole 64 m blocks at the frontier
    /// at the rate measured from individual drones, so where the camera isn't
    /// looking the swarm still digs at the same pace on average.
    fn packet_work(&mut self) {
        let individuals = self.individuals().max(1) as f64;
        // Calibrate from the last stretch of individual deliveries.
        let measured = self.window_delivered / individuals;
        self.window_delivered = 0.0;
        self.rate_per_drone = if self.rate_per_drone == 0.0 { measured } else { self.rate_per_drone * 0.98 + measured * 0.02 };
        if self.packet == 0 || self.brownout() {
            return;
        }
        self.packet_budget += self.packet as f64 * self.rate_per_drone;
        // Wear: packet drones retire at the same average rate as individuals.
        self.packet_deaths += self.packet as f64 / (self.eco.harvester.life_s * self.up("drone lifespan"));
        while self.packet_deaths >= 1.0 && self.packet > 0 {
            self.packet_deaths -= 1.0;
            self.packet -= 1;
            self.deaths += 1;
            self.mass += self.eco.harvester.mass_cost;
            self.recovered_mass += self.eco.harvester.mass_cost;
        }
        let mut guard = 0;
        while self.packet_budget > 0.0 && guard < 64 {
            guard += 1;
            // Each block is cut from the next landing point on the surface in
            // turn, so the digging spreads around the body. Packets never work
            // from a shaft: a whole block cut deep inside would leave a square
            // cavern.
            for _ in 0..self.hubs.len() {
                self.packet_hub = (self.packet_hub + 1) % self.hubs.len();
                if self.hubs[self.packet_hub].kind != HubKind::Shaft {
                    break;
                }
            }
            let origin = (self.hubs[self.packet_hub].x, self.hubs[self.packet_hub].y);
            let Some(cell) = self.pick_target(origin.0, origin.1, self.eco.harvester.search_m, true, true, true, None) else {
                self.packet_last = None;
                break;
            };
            if let Some(n) = self.reserved.get_mut(&cell) {
                *n -= 1;
                if *n == 0 {
                    self.reserved.remove(&cell);
                }
            }
            self.packet_last = Some(cell);
            let leaf = self.world.node_at(0, cell.0, cell.1).unwrap();
            let paid = self.world.mine_node(leaf);
            let (mut kg, mut credited) = (0.0, 0.0);
            for (m, k) in paid.iter().enumerate() {
                if *k > 0.0 {
                    self.stock.add_composition(&self.world.planet.mats.elements, &self.world.planet.mats.get(m as crate::materials::MatId).composition, *k);
                }
                kg += k;
                credited += k * self.world.planet.mats.get(m as crate::materials::MatId).multiplier;
            }
            self.packet_budget -= kg;
            self.packet_leaves += 1;
            self.window_mined += kg;
            self.mass += credited;
            self.delivered_credited += credited;
            self.energy += kg / 1000.0 * self.eco.seed_swarm.processing_energy_per_tonne * self.up("processing yield");
            // The block's cells are gone from the frontier; its surroundings
            // become diggable.
            let (x0, x1, y0, y1) = self.world.tree.bounds(leaf);
            for y in y0..y1 {
                for x in x0..x1 {
                    if self.remove_face((x, y)).is_some() {
                        self.reserved.remove(&(x, y));
                        // A part-cut cell inside the block: its earlier loads were
                        // already paid, so the block payout slightly overpays them.
                        self.progress.remove(&(x, y));
                    }
                }
            }
            for x in x0 - 1..=x1 {
                for y in [y0 - 1, y1] {
                    if self.world.material(x, y) != VACUUM {
                        self.add_frontier((x, y));
                    }
                }
            }
            for y in y0..y1 {
                for x in [x0 - 1, x1] {
                    if self.world.material(x, y) != VACUUM {
                        self.add_frontier((x, y));
                    }
                }
            }
        }
    }

    fn upkeep(&self) -> f64 {
        // (Cutters out working power themselves from their cargo; the packet,
        // simulated as a whole, is paid for here.)
        self.packet as f64 * self.eco.harvester.upkeep
            + self.scouts as f64 * self.eco.scout.upkeep
            + self.hubs.iter().filter(|h| h.kind == HubKind::Refiner).count() as f64 * self.eco.refiner.upkeep
            + self.tunnelers.iter().filter(|b| b.active).count() as f64 * self.eco.tunneler.upkeep
            + self.units.bores.iter().filter(|b| b.active).count() as f64 * self.eco.units.tunneler.upkeep
    }

    /// The scripted idle player: at each check-in, buy the cheapest affordable
    /// thing, up to a few purchases.
    fn bot_buy(&mut self) {
        for _ in 0..self.eco.player.buys_per_check_in {
            let mut options: Vec<(f64, usize)> = Vec::new();
            for (i, u) in self.eco.upgrade.iter().enumerate() {
                options.push((u.base_cost * crate::noise::powi(u.cost_growth, self.levels[i] as i32), i));
            }
            if LEGACY_UNITS && !self.scout_unlocked {
                options.push((self.eco.scout.unlock_cost, 1000));
            }
            if LEGACY_UNITS && !self.refiner_unlocked {
                options.push((self.eco.refiner.unlock_cost, 1001));
            }
            if LEGACY_UNITS && !self.tunneler_unlocked {
                options.push((self.eco.tunneler.unlock_cost, 1002));
            }
            options.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let (cost, which) = options[0];
            if self.mass < cost {
                return;
            }
            self.mass -= cost;
            self.spent_mass += cost;
            let name = match which {
                1000 => {
                    self.scout_unlocked = true;
                    "unlock Scouts".to_string()
                }
                1001 => {
                    self.refiner_unlocked = true;
                    "unlock Refiners".to_string()
                }
                1002 => {
                    self.tunneler_unlocked = true;
                    "unlock Tunnelers".to_string()
                }
                i => {
                    self.levels[i] += 1;
                    format!("{} level {}", self.eco.upgrade[i].name, self.levels[i])
                }
            };
            self.purchases.push((self.time(), name));
            self.up_cached_cargo = self.up("cargo pods") * self.perk_cargo;
            self.update_sense();
        }
    }

    // ---- The tick ----

    /// Drop dead drones from the list, once they far outnumber the living
    /// (a world that melts drones fast keeps hundreds of thousands): every
    /// living drone gets a new, lower number, in the same order, and every
    /// list that names drones is renumbered. Their pending events are kept
    /// (a dead drone's are dropped). Numbers seed small timing offsets, so
    /// the run differs from one never cleared, but it is still the same on
    /// every machine and thread count.
    fn compact_drones(&mut self) {
        let dead = self.drones.len() - self.alive_count as usize;
        if dead < self.compact_min || dead < self.alive_count as usize {
            return;
        }
        let mut map: Vec<u32> = vec![u32::MAX; self.drones.len()];
        let mut kept = Vec::with_capacity(self.alive_count as usize);
        for (i, d) in std::mem::take(&mut self.drones).into_iter().enumerate() {
            if d.alive {
                map[i] = kept.len() as u32;
                kept.push(d);
            }
        }
        self.drones = kept;
        let re = |id: u32| -> Option<u32> { map.get(id as usize).copied().filter(|&n| n != u32::MAX) };
        let pending = self.events.drain_all();
        for (t, id) in pending {
            if let Some(n) = re(id) {
                self.events.push(Reverse((t, n)));
            }
        }
        self.parked = self.parked.iter().filter_map(|&id| re(id)).collect();
        for w in self.site_wait.iter_mut() {
            *w = w.iter().filter_map(|&id| re(id)).collect();
        }
        for v in self.working.values_mut() {
            *v = v.iter().filter_map(|&id| re(id)).collect();
        }
        self.working.retain(|_, v| !v.is_empty());
        // Everyone's number changed: the screen gets every drone afresh.
        self.legs_all_changed();
        self.compactions += 1;
    }

    pub fn consumed_share(&self) -> f64 {
        let initial: f64 = self.world.ledger.initial.iter().sum();
        self.world.ledger.mined_kg() / initial
    }

    pub fn step(&mut self) {
        let t0 = clock();
        // Drone events due this tick, in (tick, id) order.
        while let Some(id) = self.events.pop_due(self.tick) {
            self.counts[0] += 1;
            self.on_event(id);
            self.leg_changed(id);
        }
        let t1 = clock();
        // Sunlight: every few seconds, wake the bare ground the Sun is on
        // that it could thaw or boil (it then warms in the sandbox).
        self.sandbox.time_s = self.time();
        if self.world.planet.spec.sun.is_some() && self.tick % self.ticks(5.0) == 0 {
            self.sun_wake();
        }
        // The sandbox: falling sand, heat and reactions.
        let q0 = clock();
        let changes = self.sandbox.step(&mut self.world);
        let q1 = clock();
        if !changes.is_empty() {
            self.apply_sandbox(changes);
        }
        let q2 = clock();
        self.throw_ejecta();
        self.blast_tick();
        let q3 = clock();
        self.prof2[4] += q1 - q0;
        self.prof2[5] += q2 - q1;
        self.prof2[6] += q3 - q2;
        let t2 = clock();
        self.body_tick();
        let t3 = clock();
        self.prof[0] += t1 - t0;
        self.prof[1] += t2 - t1;
        self.prof[2] += t3 - t2;
        let dt = self.eco.tick_s;
        self.stock.refine(self.eco.refiner.swarm_kg_per_s * dt);
        self.energy += self.eco.seed_swarm.base_power * dt;
        self.energy -= self.upkeep() * dt;
        if self.energy < 0.0 {
            self.energy = 0.0;
            self.brownout_ticks += 1;
            self.window_brownout += 1;
        }
        if self.tick % self.ticks(self.eco.player.check_in_every_s) == 0 && self.tick > 0 {
            self.check_ins += 1;
            if self.free_drones.is_none() {
                self.all_hubs_wet();
                self.bot_buy();
                // (A purchase can change flight speed.)
                self.legs_all_changed();
            }
            // The player launches at the first check-in after it's allowed.
            if self.ready_at.is_some() && self.launched_at.is_none() && self.free_drones.is_none() {
                self.launched_at = Some(self.time());
                self.seed_mass = self.eco.launch.seed_mass_k * (self.delivered_credited / 1000.0).sqrt();
            }
        }
        if self.tick % self.ticks(1.0) == 0 {
            let t4 = clock();
            self.packet_work();
            self.tunnel_work();
            let qs = clock();
            self.specialists_work();
            self.prof2[11] += clock() - qs;
            if let Some(l) = &mut self.life {
                // Drones still cutting in or round a square (cells reserved
                // for them within a square's width of it).
                let reserved = &self.reserved;
                let busy = |sx: i64, sy: i64| {
                    let s = crate::life::SQUARE;
                    if reserved.is_empty() {
                        return false;
                    }
                    ((sy - 1) * s..(sy + 2) * s).any(|y| ((sx - 1) * s..(sx + 2) * s).any(|x| reserved.contains_key(&(x, y))))
                };
                let q = clock();
                l.work(&mut self.world, &mut self.sandbox, self.tick, self.eco.tick_s, &busy);
                self.prof2[10] += clock() - q;
            }
            self.hauler_work();
            self.site_safety();
            self.waiting_heat();
            let tf = clock();
            self.flow_work();
            self.flow_s += clock() - tf;
            if self.tick % self.ticks(POOL_EVERY_S) == 0 {
                self.allocate();
            }
            let t5 = clock();
            self.try_build();
            let t6 = clock();
            self.mother_work();
            if self.tick % self.ticks(5.0) == 0 {
                self.flock_work();
            }
            if self.tick % self.ticks(60.0) == 0 {
                self.all_hubs_wet();
                self.compact_drones();
            }
            let t7 = clock();
            self.prof[3] += t7 - t4;
            self.prof[5] += t6 - t5;
            self.prof[6] += t5 - t4;
            self.prof[7] += t7 - t6;
            self.counts[5] = (self.drones.len() - self.alive_count as usize) as u64;
        }
        if self.tick % self.ticks(10.0) == 0 {
            let refiners = self.hubs.iter().filter(|h| h.kind == HubKind::Refiner).count() as u32;
            let window = self.ticks(10.0);
            self.samples.push(Sample {
                t: self.time(),
                harvesters: self.alive_count,
                packet: self.packet,
                scouts: self.scouts,
                refiners,
                on_trips: self.on_trip(),
                energy: self.energy,
                mass_available: self.mass,
                credited_total: self.delivered_credited,
                consumed: self.consumed_share(),
                mass_rate: self.window_mined / 10.0,
                avg_haul_m: self.avg_haul,
                hubs: self.hubs.len() as u32,
                brownout_share: self.window_brownout as f64 / window as f64,
            });
            self.window_mined = 0.0;
            self.window_brownout = 0;
        }
        if self.ready_at.is_none() && self.consumed_share() >= self.eco.launch.threshold {
            self.ready_at = Some(self.time());
        }
        self.tick += 1;
    }

    /// Run until launch or the time limit (seconds of game time).
    pub fn run(&mut self, limit_s: f64) {
        let end = self.ticks(limit_s);
        while self.tick < end && self.launched_at.is_none() {
            self.step();
        }
    }

    /// The world's hash plus the swarm's state.
    /// Peel score: how evenly the body has been eaten from the outside in.
    /// A perfect peel leaves everything inside one circle (the smallest that could
    /// hold what's left); this is the share of what's left lying outside it.
    /// 0 = perfectly round peel; near 1 = eaten from one side. Samples every `step` m.
    pub fn peel_score(&self, step: i64) -> f64 {
        let rmax = self.world.planet.max_radius as i64;
        let (mut orig, mut left) = (Vec::new(), Vec::new());
        let mut y = -rmax;
        while y <= rmax {
            let mut x = -rmax;
            while x <= rmax {
                if self.world.planet.material(x, y) != VACUUM {
                    let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
                    let r = (fx * fx + fy * fy).sqrt();
                    orig.push(r);
                    if self.world.material(x, y) != VACUUM {
                        left.push(r);
                    }
                }
                x += step;
            }
            y += step;
        }
        if left.is_empty() {
            return 0.0;
        }
        orig.sort_by(|a, b| a.total_cmp(b));
        let r_star = orig[left.len().min(orig.len() - 1)];
        left.iter().filter(|&&r| r > r_star).count() as f64 / left.len() as f64
    }

    pub fn state_hash(&self) -> u64 {
        let mut h = self.world.state_hash();
        let mut mix = |v: u64| h = hash(h, v as i64, 0, 0x5A);
        mix(self.tick);
        mix(self.energy.to_bits());
        mix(self.mass.to_bits());
        mix(self.harvesters_built);
        mix(self.packet);
        mix(self.packet_budget.to_bits());
        mix(self.deaths);
        mix(self.scouts as u64);
        mix(self.hubs.len() as u64);
        mix(self.tunnel_m);
        for h in &self.haulers {
            mix(h.load.to_bits());
            mix(h.t1);
        }
        mix(self.starved);
        mix(self.bodies.hash());
        mix(self.bodies.stats.cells_landed);
        for b in &self.tunnelers {
            mix(b.x.to_bits());
            mix(b.y.to_bits());
        }
        self.units_hash(&mut mix);
        for l in &self.levels {
            mix(*l as u64);
        }
        for d in &self.drones {
            mix(d.alive as u64);
            mix(d.life_left.to_bits());
        }
        h
    }

    /// Every cell the swarm could dig right now.
    /// How many dig-face cells of each material (for tools).
    pub fn frontier_mats(&self) -> std::collections::BTreeMap<MatId, usize> {
        let mut m = std::collections::BTreeMap::new();
        for f in self.frontier.values() {
            *m.entry(f.mat).or_insert(0) += 1;
        }
        m
    }

    pub fn frontier_len(&self) -> usize {
        self.frontier.len()
    }

    pub fn material(&self, x: i64, y: i64) -> MatId {
        self.world.material(x, y)
    }

    /// Where every individual drone is right now, for drawing: (x, y, state)
    /// with state 0 idle at its landing point, 1 flying out or cutting, 2
    /// flying home loaded. Positions are worked out from each leg's start and
    /// speed; nothing here changes the simulation.
    pub fn drone_points(&self, out: &mut Vec<f32>) {
        out.clear();
        let speed = self.eco.harvester.speed * self.up("flight speed") * self.perk_speed;
        let now = self.tick;
        for d in &self.drones {
            if !d.alive {
                continue;
            }
            let t = (now - d.leg_start.min(now)) as f64 * self.eco.tick_s;
            let lerp = |a: (f64, f64), b: (f64, f64)| {
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let dist = (dx * dx + dy * dy).sqrt().max(1e-9);
                let k = (t * speed / dist).min(1.0);
                (a.0 + dx * k, a.1 + dy * k)
            };
            let (p, s) = match d.phase {
                Phase::Idle | Phase::Waiting => {
                    let h = &self.hubs[d.home.min(self.hubs.len() - 1)];
                    ((h.x, h.y), 0.0)
                }
                Phase::Outbound { cell, from, .. } => (lerp(from, (cell.0 as f64 + 0.5, cell.1 as f64 + 0.5)), 1.0),
                Phase::Inbound { hub, from } => (lerp(from, (self.hubs[hub].x, self.hubs[hub].y)), 2.0),
            };
            out.extend_from_slice(&[p.0 as f32, p.1 as f32, s]);
        }
    }

    /// Every individual drone's current flight leg, for a renderer that moves
    /// them smoothly between simulation steps: a drone is at `from` at time
    /// `t0` (game seconds) and flies toward `to` at `speed` m/s, stopping
    /// there. Nothing here changes the simulation.
    pub fn drone_legs(&self, out: &mut Vec<Leg>) {
        out.clear();
        let speed = self.leg_speed();
        out.extend(self.drones.iter().filter(|d| d.alive).map(|d| self.leg_of(d, speed)));
    }

    fn leg_speed(&self) -> f64 {
        self.eco.harvester.speed * self.up("flight speed") * self.perk_speed
    }

    fn leg_of(&self, d: &Drone, speed: f64) -> Leg {
        if !d.alive {
            return Leg { from: (0.0, 0.0), to: (0.0, 0.0), t0: 0.0, speed: 0.0, state: Leg::GONE };
        }
        let t0 = d.leg_start as f64 * self.eco.tick_s;
        let centre = |c: (i64, i64)| (c.0 as f64 + 0.5, c.1 as f64 + 0.5);
        match d.phase {
            // Out of work (it searched and found none): resting in the
            // mother swarm, not drawn as one of a cloud round its hauler.
            Phase::Idle if d.fails >= 1 => {
                let h = &self.hubs[d.home.min(self.hubs.len() - 1)];
                Leg { from: (h.x, h.y), to: (h.x, h.y), t0, speed, state: Leg::RESTING }
            }
            Phase::Idle | Phase::Waiting => {
                let h = &self.hubs[d.home.min(self.hubs.len() - 1)];
                Leg { from: (h.x, h.y), to: (h.x, h.y), t0, speed, state: 0 }
            }
            Phase::Outbound { cell, from, .. } => Leg { from, to: centre(cell), t0, speed, state: 1 },
            Phase::Inbound { hub, from } => {
                let to = (self.hubs[hub].x, self.hubs[hub].y);
                // Dropping from the mother swarm: as fast as it takes.
                let speed = if d.trip_start == d.leg_start && d.hold <= 0.0 { speed.max(dist2(from, to).sqrt() / DROP_S) } else { speed };
                Leg { from, to, t0, speed, state: 2 }
            }
        }
    }

    fn leg_changed(&mut self, id: u32) {
        if let Some(t) = &mut self.leg_track {
            if !t.all {
                t.changed.push(id);
            }
        }
    }

    fn legs_all_changed(&mut self) {
        if let Some(t) = &mut self.leg_track {
            t.all = true;
            t.changed.clear();
        }
    }

    /// What changed since the last call, for a renderer keeping its own copy
    /// (switch it on with `leg_track = Some(LegTrack::default())`, which
    /// starts with everything to send). Every drone slot ever used has an
    /// index; a slot whose drone is gone holds a leg with state `Leg::GONE`.
    /// Either `Full(all legs)` or `Some(index, leg)` pairs, possibly repeated.
    pub fn take_leg_changes(&mut self) -> LegChanges {
        let Some(mut t) = self.leg_track.take() else { return LegChanges::Some(Vec::new()) };
        let speed = self.leg_speed();
        let out = if t.all {
            LegChanges::Full(self.drones.iter().map(|d| self.leg_of(d, speed)).collect())
        } else {
            t.changed.sort_unstable();
            t.changed.dedup();
            LegChanges::Some(t.changed.iter().map(|&i| (i, self.leg_of(&self.drones[i as usize], speed))).collect())
        };
        t.all = false;
        t.changed.clear();
        self.leg_track = Some(t);
        out
    }

    /// Where loose matter moves and rock is cut free (see sandbox::Attention):
    /// the game sets it to what's on screen. None, everywhere, is the default.
    /// (Profiling) loose cells (rubble, melt, debris) and cells still settling.
    pub fn loose_count(&self) -> (usize, usize) {
        (self.loose.len(), self.settling.len())
    }

    /// (Profiling) the sandbox's running totals and its queues (see
    /// `Sandbox::load`).
    pub fn sand_load(&self) -> (crate::sandbox::Stats, [usize; 5]) {
        (self.sandbox.stats.clone(), self.sandbox.load())
    }

    pub fn set_attention(&mut self, a: crate::sandbox::Attention) {
        self.sandbox.set_attention(a);
        self.bodies.set_attention(a);
        // Rubble that waited out of sight settles once it is in view.
        if !self.settle_wait.is_empty() && a != Some(None) {
            let now: Vec<(i64, i64)> = self.settle_wait.iter().copied().filter(|&c| crate::sandbox::in_attention(a, c)).collect();
            for c in now {
                self.settle_wait.remove(&c);
                self.settling.insert(c);
            }
        }
    }

    /// Cutters waiting at their flock for a hauler.
    /// Living drones by what they are doing: (idle at a hub with no work in
    /// hand, waiting to unload, flying out or cutting, flying home).
    pub fn phase_counts(&self) -> (usize, usize, usize, usize) {
        let mut c = (0, 0, 0, 0);
        for d in self.drones.iter().filter(|d| d.alive) {
            match d.phase {
                Phase::Idle => c.0 += 1,
                Phase::Waiting => c.1 += 1,
                Phase::Outbound { .. } => c.2 += 1,
                Phase::Inbound { .. } => c.3 += 1,
            }
        }
        c
    }

    /// (Debugging) the flocks with most cutters waiting: (flock, waiting,
    /// haulers there working / lifting / unloading / coming, its active load).
    pub fn waiting_report(&self, n: usize) -> Vec<(usize, usize, [u32; 4], u32)> {
        let mut v: Vec<(usize, usize, [u32; 4], u32)> = (0..self.hubs.len())
            .map(|h| {
                let mut st = [0u32; 4];
                for hl in self.haulers.iter().filter(|x| x.alive && x.site == h) {
                    st[match hl.state { HaulerState::Working => 0, HaulerState::Lifting => 1, HaulerState::Unloading => 2, HaulerState::Returning => 3 }] += 1;
                }
                (h, self.site_wait[h].len(), st, self.active(h))
            })
            .collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v.truncate(n);
        v
    }

    /// Drones out of work, resting in the mother swarm.
    pub fn resting_count(&self) -> usize {
        self.drones.iter().filter(|d| d.alive && d.phase == Phase::Idle && d.fails >= 1).count()
    }

    pub fn parked_count(&self) -> usize {
        self.parked.len()
    }

    pub fn waiting_count(&self) -> usize {
        self.site_wait.iter().map(|v| v.len()).sum()
    }

    /// How many drones call each hub home (same order as `hubs`).
    pub fn hub_loads(&self) -> &[u32] {
        &self.hub_load
    }

    /// Individual drones alive (the packet is counted separately).
    pub fn drone_count(&self) -> usize {
        self.alive_count as usize
    }

    /// Landing points and other hubs: (x, y, kind) with kind 0 landing point,
    /// 1 refiner, 2 shaft.
    pub fn hub_points(&self, out: &mut Vec<f32>) {
        out.clear();
        for h in &self.hubs {
            let k = match h.kind {
                HubKind::Refiner => 1.0,
                HubKind::Shaft => 2.0,
                _ => 0.0,
            };
            out.extend_from_slice(&[h.x as f32, h.y as f32, k]);
        }
    }
}

/// One drone's flight leg (see `Sim::drone_legs`). `state`: 0 idle at its
/// landing point, 1 flying out or cutting, 2 flying home loaded.
#[derive(Debug, Clone, Copy)]
pub struct Leg {
    pub from: (f64, f64),
    pub to: (f64, f64),
    pub t0: f64,
    pub speed: f64,
    pub state: u8,
}

/// Drones whose flight leg changed (see `Sim::take_leg_changes`).
#[derive(Debug)]
pub struct LegTrack {
    pub changed: Vec<u32>,
    pub all: bool,
}

impl Default for LegTrack {
    fn default() -> Self {
        LegTrack { changed: Vec::new(), all: true }
    }
}

pub enum LegChanges {
    Full(Vec<Leg>),
    Some(Vec<(u32, Leg)>),
}

impl Leg {
    /// The state of a slot whose drone is gone.
    pub const GONE: u8 = 255;
    /// Out of work, resting in the mother swarm (not drawn).
    pub const RESTING: u8 = 3;

    /// Where the drone is at game time `t`.
    pub fn at(&self, t: f64) -> (f64, f64) {
        let (dx, dy) = (self.to.0 - self.from.0, self.to.1 - self.from.1);
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < 1e-9 {
            return self.to;
        }
        let k = ((t - self.t0).max(0.0) * self.speed / dist).min(1.0);
        (self.from.0 + dx * k, self.from.1 + dy * k)
    }
}

/// What each `Sim::prof` entry measures.
pub const PROF_NAMES: [&str; 8] = ["drone events", "sand and heat", "rock bodies", "per-second work", "events: work search", "per-second: building", "per-second: packet", "per-second: orbit and landing"];
/// What each `Sim::counts` entry counts.
pub const COUNT_NAMES: [&str; 6] = ["drone events", "work searches", "searches finding nothing", "cells weighed", "drones built", "dead drones kept"];

// ---- Saving and loading ----

/// Version of the swarm's part of a save (after the world's part).
const SIM_VERSION: u32 = 13; // 13: the fuser, conventional charges, uranium in rubble; 12: the life cycle and the ledger's grown column; 6: life.toml added materials; 7: the element stockpile; 8: 75 elements, raw and refined; 9: the parts stock; 10: production; 11: specialists

fn put_cell(o: &mut crate::save::W, c: (i64, i64)) {
    o.i64(c.0);
    o.i64(c.1);
}

fn get_cell(r: &mut crate::save::R) -> Result<(i64, i64), String> {
    Ok((r.i64()?, r.i64()?))
}

fn put_opt_cell(o: &mut crate::save::W, c: Option<(i64, i64)>) {
    o.u8(c.is_some() as u8);
    put_cell(o, c.unwrap_or((0, 0)));
}

fn get_opt_cell(r: &mut crate::save::R) -> Result<Option<(i64, i64)>, String> {
    let some = r.u8()? != 0;
    let c = get_cell(r)?;
    Ok(some.then_some(c))
}

/// Cells in a stable order (hash maps iterate in no particular one).
fn sorted<'a>(it: impl Iterator<Item = &'a (i64, i64)>) -> Vec<(i64, i64)> {
    let mut v: Vec<(i64, i64)> = it.copied().collect();
    v.sort_unstable();
    v
}

impl Sim {
    /// Write the whole game state, world included, uncompressed (see
    /// `save::pack`). Not everything is kept exactly, which nobody would
    /// notice in a game about scale:
    /// - drones mid-trip are home again, idle, and what they carried is lost;
    /// - rock in the air is lost (counted as vented);
    /// - the sandbox's cells are all woken on loading, and heat banked
    ///   toward a phase change is kept, as are temperatures;
    /// - histories (samples) and profiling start over.
    pub fn write_save(&self, o: &mut crate::save::W) {
        crate::save::write_world(o, &self.world);
        o.u32(SIM_VERSION);
        o.u64(self.tick);
        o.f64(self.energy);
        o.f64(self.mass);
        o.u32(self.stock.kg.len() as u32);
        for (a, b) in self.stock.kg.iter().zip(&self.transit.kg) {
            o.f64(*a);
            o.f64(*b);
        }
        o.f64(self.transit.total);
        for r in &self.stock.refined {
            o.f64(*r);
        }
        o.u32(self.parts.n.len() as u32);
        for (n, m) in self.parts.n.iter().zip(&self.parts.made) {
            o.u64(*n);
            o.u64(*m);
        }
        self.prod.write(o);
        o.f64(self.delivered_credited);
        o.f64(self.spent_mass);
        o.f64(self.recovered_mass);
        o.u32(self.levels.len() as u32);
        for l in &self.levels {
            o.u32(*l);
        }
        o.u8(self.scout_unlocked as u8);
        o.u8(self.refiner_unlocked as u8);
        o.u8(self.tunneler_unlocked as u8);
        o.u32(self.scouts);
        // Landing points and the orbit ring.
        o.u32(self.hubs.len() as u32);
        for h in &self.hubs {
            o.f64(h.x);
            o.f64(h.y);
            o.u8(match h.kind { HubKind::Seed => 0, HubKind::Refiner => 1, HubKind::Shaft => 2 });
            o.u64(h.ring.map_or(u64::MAX, |k| k as u64));
        }
        for h in 0..self.hubs.len() {
            o.u8(self.flock.get(h).is_some_and(|f| f.dead) as u8);
        }
        o.u32(self.ring.len() as u32);
        for i in 0..self.ring.len() {
            o.f64(self.ring[i].0);
            o.f64(self.ring[i].1);
            o.f64(self.ring_dir[i].0);
            o.f64(self.ring_dir[i].1);
            o.u64(self.ring_hub[i].map_or(u64::MAX, |k| k as u64));
        }
        o.u64(self.orbit_hub as u64);
        o.u32(self.wet_hubs.len() as u32);
        for h in &self.wet_hubs {
            o.u64(*h as u64);
        }
        // Living drones only (they are numbered afresh on loading).
        o.u64(self.alive_count as u64);
        for d in self.drones.iter().filter(|d| d.alive) {
            o.u32(d.home as u32);
            o.f64(d.life_left);
            put_opt_cell(o, d.last);
            o.f64(d.hold);
            o.f64(d.hold_cr);
            o.f64(d.tank);
            o.f64(d.heat_k);
        }
        o.u32(self.haulers.len() as u32);
        for h in &self.haulers {
            o.u64(h.site as u64);
            o.u8(match h.state { HaulerState::Working => 0, HaulerState::Lifting => 1, HaulerState::Unloading => 2, HaulerState::Returning => 3 });
            for v in [h.load, h.credited, h.from.0, h.from.1, h.to.0, h.to.1, h.life_left] {
                o.f64(v);
            }
            o.u64(h.t0);
            o.u64(h.t1);
        }
        o.u64(self.starved);
        o.u64(self.haulers_lost);
        o.f64(self.fuel_kg);
        o.f64(self.mother.centre);
        o.f64(self.mother.half);
        o.f64(self.mother.grown_kg);
        o.f64(self.mother.radius);
        o.f64(self.mother.returned_kg);
        o.u64(self.explosions);
        // The economy's books.
        o.u64(self.next_free_build);
        o.f64(self.avg_haul);
        put_opt_cell(o, self.last_far_cell);
        o.u64(self.brownout_ticks);
        o.f64(self.window_mined);
        o.u64(self.window_brownout);
        o.f64(self.window_delivered);
        o.opt_f64(self.launched_at);
        o.f64(self.seed_mass);
        o.u64(self.harvesters_built);
        o.u64(self.deaths);
        o.u32(self.purchases.len() as u32);
        for (t, what) in &self.purchases {
            o.f64(*t);
            o.text(what);
        }
        o.u64(self.packet);
        o.f64(self.packet_budget);
        o.f64(self.packet_deaths);
        o.u64(self.packet_hub as u64);
        o.f64(self.rate_per_drone);
        o.u64(self.packet_leaves);
        put_opt_cell(o, self.packet_last);
        o.u32(self.perks.len() as u32);
        for p in &self.perks {
            o.text(p);
        }
        o.opt_f64(self.ready_at);
        o.u32(self.check_ins);
        o.u64(self.burned);
        o.u64(self.crushed);
        o.u64(self.lava_swallowed);
        // Tunnelers.
        o.u32(self.tunnelers.len() as u32);
        for b in &self.tunnelers {
            for v in [b.x, b.y, b.hx, b.hy, b.aim.0, b.aim.1, b.length, b.progress_s] {
                o.f64(v);
            }
            o.u8(b.active as u8);
            o.u64(b.resume_at);
        }
        o.u64(self.next_tunnel_k as u64);
        o.u64(self.tunnel_m);
        o.u64(self.tunnels_done);
        o.f64(self.tunnel_kg);
        // Where the ground has been opened, and the dig face.
        let mut tiles: Vec<(&(i64, i64), &Box<[u64; 64]>)> = self.open.tiles.iter().collect();
        tiles.sort_unstable_by_key(|(k, _)| **k);
        o.u64(tiles.len() as u64);
        for (k, rows) in tiles {
            put_cell(o, *k);
            for r in rows.iter() {
                o.u64(*r);
            }
        }
        let face = sorted(self.frontier.keys());
        o.u64(face.len() as u64);
        let mut prev = (0i64, 0i64);
        for c in face {
            // Neighbours in order differ by little: store the step.
            o.i64(c.0 - prev.0);
            o.i64(c.1 - prev.1);
            prev = c;
            let f = &self.frontier[&c];
            o.f32(f.near);
            o.f32(f.far);
        }
        let prog = sorted(self.progress.keys());
        o.u64(prog.len() as u64);
        for c in prog {
            put_cell(o, c);
            o.f64(self.progress[&c]);
        }
        let frac = sorted(self.fractured.iter());
        o.u64(frac.len() as u64);
        for c in frac {
            put_cell(o, c);
        }
        // The sandbox: temperatures, banked heat, and cells waiting to move.
        o.u64(self.sandbox.tick);
        for map in [&self.sandbox.temps, &self.sandbox.latent] {
            let keys = sorted(map.keys());
            o.u64(keys.len() as u64);
            for c in keys {
                put_cell(o, c);
                o.f64(map[&c]);
            }
        }
        let mut pending = self.sandbox.pending();
        pending.sort_unstable();
        pending.dedup();
        o.u64(pending.len() as u64);
        for c in pending {
            put_cell(o, c);
        }
        // Rock in the air: lost on loading (only its materials are kept, for the books).
        let mut air: Vec<MatId> = self.bodies.cells().into_iter().map(|(_, m)| m).collect();
        air.extend(self.plume_out.iter().map(|p| p.2));
        o.u64(air.len() as u64);
        for m in air {
            o.u16(m);
        }
        self.write_units(o);
        self.write_units_12(o);
        o.f64s(&self.world.ledger.grown);
        o.u8(self.life.is_some() as u8);
        if let Some(l) = &self.life {
            l.write(o);
        }
        self.write_charges(o);
    }

    /// Rebuild a game from what `write_save` wrote, on the same body, with
    /// the economy as it is now.
    pub fn read_save(r: &mut crate::save::R, planet: std::sync::Arc<crate::planet::Planet>, eco: Economy) -> Result<Sim, String> {
        let world = crate::save::read_world(r, planet)?;
        // (Version 8 saves have no parts stock, 8 and 9 no production: they
        // start empty.)
        let version = r.u32()?;
        if !(8..=SIM_VERSION).contains(&version) {
            return Err("this save's swarm is from another version".into());
        }
        let mut s = Sim::blank(world, eco);
        s.tick = r.u64()?;
        s.events = EventQueue::starting_at(s.tick);
        s.energy = r.f64()?;
        s.mass = r.f64()?;
        let n = r.u32()? as usize;
        for i in 0..n {
            let (a, b) = (r.f64()?, r.f64()?);
            if i < s.stock.kg.len() {
                s.stock.kg[i] = a;
                s.transit.kg[i] = b;
            }
        }
        s.transit.total = r.f64()?;
        for i in 0..n {
            let v = r.f64()?;
            if i < s.stock.refined.len() {
                s.stock.refined[i] = v.min(s.stock.kg[i]);
            }
        }
        if version >= 9 {
            let n = r.u32()? as usize;
            for i in 0..n {
                let (k, m) = (r.u64()?, r.u64()?);
                if i < s.parts.n.len() {
                    s.parts.n[i] = k;
                    s.parts.made[i] = m;
                }
            }
        }
        if version >= 10 {
            let p = crate::production::Production::read(r, version >= 11)?;
            // (Only if it is the same factory as the economy's.)
            if p.bays.len() == s.prod.bays.len() {
                s.prod = p;
            }
        }
        s.delivered_credited = r.f64()?;
        s.spent_mass = r.f64()?;
        s.recovered_mass = r.f64()?;
        let n = r.u32()? as usize;
        let levels: Vec<u32> = (0..n).map(|_| r.u32()).collect::<Result<_, _>>()?;
        for (i, l) in levels.into_iter().enumerate() {
            if i < s.levels.len() {
                s.levels[i] = l;
            }
        }
        s.scout_unlocked = r.u8()? != 0;
        s.refiner_unlocked = r.u8()? != 0;
        s.tunneler_unlocked = r.u8()? != 0;
        s.scouts = r.u32()?;
        let opt = |v: u64| (v != u64::MAX).then_some(v as usize);
        for _ in 0..r.u32()? {
            let (x, y) = (r.f64()?, r.f64()?);
            let kind = match r.u8()? { 0 => HubKind::Seed, 1 => HubKind::Refiner, _ => HubKind::Shaft };
            let ring = opt(r.u64()?);
            s.hubs.push(Hub { x, y, kind, ring });
        }
        let nh = s.hubs.len();
        let dead: Vec<bool> = (0..nh).map(|_| r.u8().map(|v| v != 0)).collect::<Result<_, _>>()?;
        let bad = |what: &str| format!("corrupt save: {what}");
        for _ in 0..r.u32()? {
            s.ring.push((r.f64()?, r.f64()?));
            s.ring_dir.push((r.f64()?, r.f64()?));
            let h = opt(r.u64()?);
            if h.is_some_and(|h| h >= nh) {
                return Err(bad("orbit ring"));
            }
            s.ring_hub.push(h);
        }
        s.orbit_hub = r.u64()? as usize;
        for _ in 0..r.u32()? {
            s.wet_hubs.insert(r.u64()? as usize);
        }
        if s.orbit_hub >= nh.max(1) || s.wet_hubs.iter().any(|h| *h >= nh) {
            return Err(bad("landing points"));
        }
        s.hub_load = vec![0; nh];
        s.hub_dry = vec![false; nh];
        s.flock = dead.iter().map(|&dead| FlockStats { dead, ..Default::default() }).collect();
        let nd = r.u64()?;
        for _ in 0..nd {
            let home = r.u32()? as usize;
            let life_left = r.f64()?;
            let last = get_opt_cell(r)?;
            let (hold, hold_cr, tank, heat_k) = (r.f64()?, r.f64()?, r.f64()?, r.f64()?);
            if home >= nh {
                return Err(bad("a drone's home"));
            }
            let id = s.drones.len() as u32;
            s.hub_load[home] += 1;
            s.drones.push(Drone { alive: true, home, phase: Phase::Idle, life_left, trip_start: s.tick, leg_start: s.tick, last, fails: 0, hold, hold_cr, tank, heat_k });
            s.alive_count += 1;
            // Back to work over the first second, not all on one tick.
            s.events.push(Reverse((s.tick + (id as u64 % 10), id)));
        }
        s.site_wait = vec![Vec::new(); nh];
        for _ in 0..r.u32()? {
            let site = r.u64()? as usize;
            let state = match r.u8()? { 0 => HaulerState::Working, 1 => HaulerState::Lifting, 2 => HaulerState::Unloading, _ => HaulerState::Returning };
            let v: Vec<f64> = (0..7).map(|_| r.f64()).collect::<Result<_, _>>()?;
            let (t0, t1) = (r.u64()?, r.u64()?);
            if site >= nh {
                return Err(bad("a hauler's flock"));
            }
            s.haulers.push(Hauler { site, state, load: v[0], credited: v[1], from: (v[2], v[3]), to: (v[4], v[5]), t0, t1, life_left: v[6], alive: true, lift_at: 0, mix: Default::default() });
        }
        s.starved = r.u64()?;
        s.haulers_lost = r.u64()?;
        s.fuel_kg = r.f64()?;
        s.mother = Mother { centre: r.f64()?, half: r.f64()?, grown_kg: r.f64()?, radius: r.f64()?, returned_kg: r.f64()? };
        s.explosions = r.u64()?;
        s.next_free_build = r.u64()?;
        s.avg_haul = r.f64()?;
        s.last_far_cell = get_opt_cell(r)?;
        s.brownout_ticks = r.u64()?;
        s.window_mined = r.f64()?;
        s.window_brownout = r.u64()?;
        s.window_delivered = r.f64()?;
        s.launched_at = r.opt_f64()?;
        s.seed_mass = r.f64()?;
        s.harvesters_built = r.u64()?;
        s.deaths = r.u64()?;
        for _ in 0..r.u32()? {
            let t = r.f64()?;
            s.purchases.push((t, r.text()?));
        }
        s.packet = r.u64()?;
        s.packet_budget = r.f64()?;
        s.packet_deaths = r.f64()?;
        s.packet_hub = (r.u64()? as usize).min(nh.saturating_sub(1));
        s.rate_per_drone = r.f64()?;
        s.packet_leaves = r.u64()?;
        s.packet_last = get_opt_cell(r)?;
        for _ in 0..r.u32()? {
            s.perks.push(r.text()?);
        }
        s.apply_perks();
        s.ready_at = r.opt_f64()?;
        s.check_ins = r.u32()?;
        s.burned = r.u64()?;
        s.crushed = r.u64()?;
        s.lava_swallowed = r.u64()?;
        for _ in 0..r.u32()? {
            let v: Vec<f64> = (0..8).map(|_| r.f64()).collect::<Result<_, _>>()?;
            let active = r.u8()? != 0;
            let resume_at = r.u64()?;
            s.tunnelers.push(Bore { x: v[0], y: v[1], hx: v[2], hy: v[3], aim: (v[4], v[5]), length: v[6], progress_s: v[7], active, resume_at, prev: (v[0], v[1]), moved_at: 0 });
        }
        s.next_tunnel_k = r.u64()? as usize;
        s.tunnel_m = r.u64()?;
        s.tunnels_done = r.u64()?;
        s.tunnel_kg = r.f64()?;
        for _ in 0..r.u64()? {
            let k = get_cell(r)?;
            let mut rows = Box::new([0u64; 64]);
            for row in rows.iter_mut() {
                *row = r.u64()?;
            }
            s.open.tiles.insert(k, rows);
        }
        s.up_cached_cargo = s.up("cargo pods") * s.perk_cargo;
        s.update_sense();
        let mut prev = (0i64, 0i64);
        for _ in 0..r.u64()? {
            let c = (prev.0 + r.i64()?, prev.1 + r.i64()?);
            prev = c;
            let (near, far) = (r.f32()?, r.f32()?);
            let m = s.world.material(c.0, c.1);
            if m == VACUUM {
                continue;
            }
            let face = Face { mat: m, near, far };
            if s.frontier.insert(c, face).is_none() {
                s.buckets.entry((c.0.div_euclid(BUCKET), c.1.div_euclid(BUCKET))).or_default().push((c, face));
                if s.world.planet.mats.get(m).state == State::Liquid {
                    s.liquid_faces += 1;
                }
            }
        }
        for _ in 0..r.u64()? {
            let c = get_cell(r)?;
            s.progress.insert(c, r.f64()?);
        }
        for _ in 0..r.u64()? {
            s.fractured.insert(get_cell(r)?);
        }
        s.sandbox.tick = r.u64()?;
        for k in 0..2 {
            for _ in 0..r.u64()? {
                let c = get_cell(r)?;
                let v = r.f64()?;
                if k == 0 { s.sandbox.temps.insert(c, v) } else { s.sandbox.latent.insert(c, v) };
            }
        }
        for _ in 0..r.u64()? {
            s.sandbox.wake(get_cell(r)?);
        }
        for c in sorted(s.sandbox.temps.keys().chain(s.sandbox.latent.keys())) {
            s.sandbox.wake(c);
        }
        for _ in 0..r.u64()? {
            let m = r.u16()?;
            if (m as usize) < s.world.planet.mats.len() {
                s.world.lose_aloft(m);
            }
        }
        if version >= 11 {
            s.read_units(r)?;
        }
        if version >= 12 {
            s.read_units_12(r)?;
            s.resettle();
            s.world.ledger.grown = r.f64s()?;
            let had = r.u8()? != 0;
            if had {
                match &mut s.life {
                    Some(l) => l.read(r)?,
                    None => return Err("this save has a life cycle but the body has no life".into()),
                }
            }
        }
        if version >= 13 {
            s.read_charges(r)?;
        } else {
            let f = s.fissile_per_charge();
            for sp in s.units.sappers.iter_mut() {
                sp.fissile = f;
            }
        }
        s.sandbox.time_s = s.time();
        s.legs_all_changed();
        Ok(s)
    }
}

/// Drones' events by the tick they are due: a ring of one list per tick
/// for the next WHEEL ticks, and further ones kept aside until they come
/// near. Popped in (tick, drone) order, like a heap but without its cost
/// with half a million drones waiting (a heap of that size was a seventh of
/// the simulation's time).
struct EventQueue {
    /// The tick being worked through, and its events still to do (by drone).
    now: u64,
    cur: BinaryHeap<Reverse<u32>>,
    /// Events due in (now, now + WHEEL): slot t % WHEEL.
    wheel: Vec<Vec<u32>>,
    wheel_len: usize,
    /// Events due later than that.
    far: std::collections::BTreeMap<u64, Vec<u32>>,
}

const WHEEL: u64 = 8192;

impl Default for EventQueue {
    fn default() -> Self {
        EventQueue::starting_at(0)
    }
}

impl EventQueue {
    fn starting_at(tick: u64) -> Self {
        EventQueue { now: tick, cur: BinaryHeap::new(), wheel: vec![Vec::new(); WHEEL as usize], wheel_len: 0, far: Default::default() }
    }

    fn push(&mut self, Reverse((t, id)): Reverse<(u64, u32)>) {
        if t <= self.now {
            self.cur.push(Reverse(id));
        } else if t < self.now + WHEEL {
            self.wheel[(t % WHEEL) as usize].push(id);
            self.wheel_len += 1;
        } else {
            self.far.entry(t).or_default().push(id);
        }
    }

    /// The next drone whose event is due by `tick` (lowest number first).
    fn pop_due(&mut self, tick: u64) -> Option<u32> {
        self.advance(tick);
        self.cur.pop().map(|Reverse(id)| id)
    }

    fn advance(&mut self, tick: u64) {
        while self.now < tick && self.cur.is_empty() {
            if self.wheel_len == 0 {
                // Nothing close: skip ahead to the next far event (or to tick).
                let next = self.far.keys().next().copied().unwrap_or(u64::MAX);
                let to = tick.min(next);
                if to > self.now + 1 {
                    self.now = to - 1;
                }
            }
            self.now += 1;
            let slot = std::mem::take(&mut self.wheel[(self.now % WHEEL) as usize]);
            self.wheel_len -= slot.len();
            self.cur.extend(slot.into_iter().map(Reverse));
            while let Some((&k, _)) = self.far.first_key_value() {
                if k >= self.now + WHEEL {
                    break;
                }
                let v = self.far.pop_first().unwrap().1;
                if k <= self.now {
                    self.cur.extend(v.into_iter().map(Reverse));
                } else {
                    self.wheel[(k % WHEEL) as usize].extend_from_slice(&v);
                    self.wheel_len += v.len();
                }
            }
        }
    }

    /// Every pending event (tick, drone), emptying the queue.
    fn drain_all(&mut self) -> Vec<(u64, u32)> {
        let mut out: Vec<(u64, u32)> = self.cur.drain().map(|Reverse(id)| (self.now, id)).collect();
        for d in 1..WHEEL {
            let t = self.now + d;
            for id in std::mem::take(&mut self.wheel[(t % WHEEL) as usize]) {
                out.push((t, id));
            }
        }
        self.wheel_len = 0;
        for (t, v) in std::mem::take(&mut self.far) {
            out.extend(v.into_iter().map(|id| (t, id)));
        }
        out
    }
}

/// The simulation can be read from several threads at once (the drones'
/// searches run side by side).
#[allow(dead_code)]
fn assert_sync() {
    fn is_sync<T: Sync>() {}
    is_sync::<Sim>();
}
