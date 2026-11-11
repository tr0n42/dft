//! Production in the swarm (Swarm Production Design, phases 2 and 3): the
//! fabricator makes parts from refined elements, one kind at a time, and
//! the factory builds ground units from parts in its bays, each taking its
//! time. The director's orders queue in front of the factory.
//!
//! The swarm arrives as a fleet: a storage ship with an allowance of
//! refined elements, a refiner, a fabricator, and a factory carrying the
//! first cutters. The first haulers are built from the allowance.
//!
//! This is the skeleton: one fabricator and one factory, material handed
//! between them at once (couriers, placement along the mother swarm and
//! swarm-built machines come next). Only the numbers live here; the work
//! itself is done in swarm.rs, which owns the drones and flocks.

use serde::Deserialize;
use std::collections::{BTreeMap, VecDeque};

/// The kinds of ground unit the factory builds. Cutters and haulers are the
/// swarm's own; the rest are specialists, built by the Universal Drone
/// Replicator (for testing: normally they are earned through the meta tree).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    Cutter,
    Hauler,
    Sapper,
    Tunneler,
    #[serde(rename = "heatsink")]
    HeatSink,
    Forester,
    Abductor,
}

impl Unit {
    pub const ALL: [Unit; 7] = [Unit::Cutter, Unit::Hauler, Unit::Sapper, Unit::Tunneler, Unit::HeatSink, Unit::Forester, Unit::Abductor];
    /// The specialists.
    pub const SPECIALISTS: [Unit; 5] = [Unit::Sapper, Unit::Tunneler, Unit::HeatSink, Unit::Forester, Unit::Abductor];

    pub fn name(self) -> &'static str {
        match self {
            Unit::Cutter => "cutter",
            Unit::Hauler => "hauler",
            Unit::Sapper => "sapper",
            Unit::Tunneler => "tunneler",
            Unit::HeatSink => "heat-sink bot",
            Unit::Forester => "forester",
            Unit::Abductor => "abductor",
        }
    }

    /// Its table name in data/economy.toml ([recipe.<key>]).
    pub fn recipe_key(self) -> &'static str {
        match self {
            Unit::Cutter => "harvester",
            Unit::Hauler => "hauler",
            Unit::Sapper => "sapper",
            Unit::Tunneler => "tunneler",
            Unit::HeatSink => "heatsink",
            Unit::Forester => "forester",
            Unit::Abductor => "abductor",
        }
    }

    /// Its place in Unit::ALL.
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(i: usize) -> Option<Unit> {
        Unit::ALL.get(i).copied()
    }
}

/// Bays of one kind in the starting factory.
#[derive(Debug, Clone, Deserialize)]
pub struct BaySpec {
    pub unit: Unit,
    /// How many.
    pub n: u32,
    /// Seconds to build one unit in one bay.
    pub build_s: f64,
}

/// `[production]` in data/economy.toml.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ProductionSpec {
    /// The fleet's allowance of refined elements (kg, by symbol).
    #[serde(default)]
    pub start_refined: BTreeMap<String, f64>,
    /// Energy the fleet brings, on top of the seed swarm's (E): enough to
    /// build the first haulers before any delivery.
    #[serde(default)]
    pub start_energy: f64,
    /// Parts the fabricator makes each second (kg).
    #[serde(default)]
    pub fab_kg_per_s: f64,
    /// Seconds the fabricator takes to switch to another part kind.
    #[serde(default)]
    pub fab_switch_s: f64,
    /// The starting factory's bays. None: no factory (units are built at
    /// once, as before production).
    #[serde(default)]
    pub bay: Vec<BaySpec>,
    /// Haulers that may wait in the queue per hauler bay (more flocks than
    /// that wait to open).
    #[serde(default = "two")]
    pub hauler_orders_per_bay: u32,
    /// Cutters that may wait in the queue per cutter bay.
    #[serde(default = "two")]
    pub cutter_orders_per_bay: u32,
}

fn two() -> u32 {
    2
}

/// Something the director wants built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    Cutter,
    /// A hauler for this flock.
    Hauler(usize),
    /// A specialist (sapper, tunneler...: never a cutter or hauler).
    Special(Unit),
}

impl Order {
    pub fn unit(self) -> Unit {
        match self {
            Order::Cutter => Unit::Cutter,
            Order::Hauler(_) => Unit::Hauler,
            Order::Special(u) => u,
        }
    }
}

/// A unit being built in a bay.
#[derive(Debug, Clone, Copy)]
pub struct Job {
    pub order: Order,
    pub left_s: f64,
}

/// One bay of the factory: it builds one kind of unit, one at a time.
#[derive(Debug, Clone)]
pub struct Bay {
    pub unit: Unit,
    pub build_s: f64,
    pub job: Option<Job>,
}

/// The fabricator: one part kind at a time.
#[derive(Debug, Clone, Default)]
pub struct Fabricator {
    /// The part kind it is set up for (place in the catalogue).
    pub part: Option<usize>,
    /// Seconds left switching to `part`.
    pub switch_left_s: f64,
    /// kg of work done toward the next parts.
    pub progress_kg: f64,
    /// It had work but the stockpile couldn't make the part.
    pub stalled: bool,
    /// No part is short: nothing to do.
    pub idle: bool,
}

/// The production state.
#[derive(Debug, Clone, Default)]
pub struct Production {
    pub orders: VecDeque<Order>,
    pub bays: Vec<Bay>,
    pub fab: Fabricator,
    /// Units the factory has finished, by kind (in Unit::ALL's order).
    pub built: [u64; 7],
}

impl Production {
    /// The starting factory, plus `extra` bays (the Universal Drone
    /// Replicator's, for the specialists this body has a use for).
    pub fn new(spec: &ProductionSpec, extra: &[BaySpec]) -> Production {
        let mut bays = Vec::new();
        for b in spec.bay.iter().chain(extra) {
            for _ in 0..b.n {
                bays.push(Bay { unit: b.unit, build_s: b.build_s, job: None });
            }
        }
        Production { orders: VecDeque::new(), bays, fab: Fabricator::default(), built: [0; 7] }
    }

    /// There is a factory: units are built in it, over time.
    pub fn on(&self) -> bool {
        !self.bays.is_empty()
    }

    pub fn bays_of(&self, u: Unit) -> usize {
        self.bays.iter().filter(|b| b.unit == u).count()
    }

    /// Orders waiting (not yet started) for this kind of unit.
    pub fn queued(&self, u: Unit) -> usize {
        self.orders.iter().filter(|o| o.unit() == u).count()
    }

    /// Units of this kind being built now.
    pub fn building(&self, u: Unit) -> usize {
        self.bays.iter().filter(|b| b.unit == u && b.job.is_some()).count()
    }

    /// Haulers ordered for this flock, waiting or being built.
    pub fn haulers_for(&self, site: usize) -> u32 {
        let o = Order::Hauler(site);
        (self.orders.iter().filter(|x| **x == o).count() + self.bays.iter().filter(|b| b.job.is_some_and(|j| j.order == o)).count()) as u32
    }

    // ---- Saves ----

    pub fn write(&self, o: &mut crate::save::W) {
        // (A specialist: u64::MAX less 1 + its place in Unit::ALL.)
        let order = |o: &mut crate::save::W, x: Order| match x {
            Order::Cutter => o.u64(u64::MAX),
            Order::Hauler(s) => o.u64(s as u64),
            Order::Special(u) => o.u64(u64::MAX - 1 - u.index() as u64),
        };
        o.u32(self.orders.len() as u32);
        for x in &self.orders {
            order(o, *x);
        }
        o.u32(self.bays.len() as u32);
        for b in &self.bays {
            o.u8(b.unit as u8);
            o.f64(b.build_s);
            match b.job {
                Some(j) => {
                    o.u8(1);
                    order(o, j.order);
                    o.f64(j.left_s);
                }
                None => o.u8(0),
            }
        }
        o.u64(self.fab.part.map_or(u64::MAX, |p| p as u64));
        o.f64(self.fab.switch_left_s);
        o.f64(self.fab.progress_kg);
        for b in self.built {
            o.u64(b);
        }
    }

    /// `all_kinds`: the save counts every kind built (version 11 on), not
    /// just cutters and haulers.
    pub fn read(r: &mut crate::save::R, all_kinds: bool) -> Result<Production, String> {
        let order = |r: &mut crate::save::R| -> Result<Order, String> {
            let v = r.u64()?;
            Ok(if v == u64::MAX {
                Order::Cutter
            } else if v >= u64::MAX - 1 - Unit::ALL.len() as u64 {
                Order::Special(Unit::from_index((u64::MAX - 1 - v) as usize).ok_or("bad unit")?)
            } else {
                Order::Hauler(v as usize)
            })
        };
        let mut p = Production::default();
        for _ in 0..r.u32()? {
            p.orders.push_back(order(r)?);
        }
        for _ in 0..r.u32()? {
            let unit = Unit::from_index(r.u8()? as usize).ok_or("bad unit")?;
            let build_s = r.f64()?;
            let job = if r.u8()? != 0 { Some(Job { order: order(r)?, left_s: r.f64()? }) } else { None };
            p.bays.push(Bay { unit, build_s, job });
        }
        let part = r.u64()?;
        p.fab.part = (part != u64::MAX).then_some(part as usize);
        p.fab.switch_left_s = r.f64()?;
        p.fab.progress_kg = r.f64()?;
        for b in p.built.iter_mut().take(if all_kinds { 7 } else { 2 }) {
            *b = r.u64()?;
        }
        Ok(p)
    }
}
