//! Placeholder animals: herds on bodies whose spec has a `[fauna]` section
//! (the Dev planet, data/bodies/dev-planet.toml). They wander the land and
//! panic: when a machine comes near (a walker, a sapper, a heat-sink bot, a
//! tunneler, a working flock) or one of their herd is snatched, the whole
//! herd runs from it for a while. No species, art or real behaviour yet:
//! they are there so the abductor has prey to test on (Fauna L1 comes
//! later; see the fauna plan in HANDOFF.md). They never run out: while
//! there are fewer than at the start, a new herd wanders in every
//! `ARRIVE_EVERY_S`.
//!
//! Stepped once a game second (`Sim::fauna_work`), timed in `Sim::fauna_prof`
//! (the unit bench's "animals" column).
//! The abductor (units/abductor.rs) catches them with `Sim::catch_animal`.

use super::*;

/// Walking and running speeds (m/s).
pub(super) const GRAZE_SPEED: f64 = 0.6;
pub(super) const FLEE_SPEED: f64 = 7.0;
/// A herd panics when a machine comes this close (m) to one of it, and runs
/// for this long (s) after the last scare.
pub(super) const PANIC_M: f64 = 80.0;
pub(super) const PANIC_S: f64 = 25.0;
/// How far a herd spreads along the ground (m) before strays turn back.
pub(super) const HERD_SPREAD_M: f64 = 25.0;
/// A fall of more than this (m), the ground eaten from under it, kills one.
pub(super) const FALL_KILLS_M: f64 = 30.0;
/// How often a new herd wanders in while the body has fewer animals than
/// at the start (s).
pub(super) const ARRIVE_EVERY_S: f64 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimalState {
    Graze = 0,
    Flee = 1,
}

#[derive(Debug, Clone)]
pub struct Animal {
    pub(super) id: u32,
    pub(super) herd: u32,
    pub(super) pos: (f64, f64),
    pub(super) prev: (f64, f64),
    pub(super) moved_at: u64,
    pub(super) kg: f64,
    /// Which way it walks along the ground (+1 counter-clockwise).
    pub(super) facing: f64,
    pub(super) state: AnimalState,
    pub(super) panic_s: f64,
}

/// Every animal on the body, and what has become of the rest.
#[derive(Debug, Clone, Default)]
pub struct Fauna {
    pub(super) animals: Vec<Animal>,
    pub(super) next_id: u32,
    /// Animals at the start: new herds wander in while there are fewer.
    pub(super) start: u32,
    /// Herds made so far (each new herd's number).
    pub(super) herds: u32,
    /// Herds alarmed since the last step, and from where (a catch).
    pub(super) alarms: Vec<(u32, (f64, f64))>,
    /// Animals snatched alive by abductors (and their kg), and killed
    /// otherwise (fallen, drowned in lava).
    pub caught: u64,
    pub caught_kg: f64,
    pub lost: u64,
}

/// An animal as the screen needs it: it moves from `from` to `to` between
/// game times t0 and t1.
#[derive(Debug, Clone, Copy)]
pub struct AnimalView {
    pub id: u32,
    pub from: (f64, f64),
    pub to: (f64, f64),
    pub t0: f64,
    pub t1: f64,
    pub kg: f32,
    pub facing: f32,
    pub fleeing: bool,
}

impl AnimalView {
    pub fn at(&self, t: f64) -> (f64, f64) {
        let k = if self.t1 > self.t0 { ((t - self.t0) / (self.t1 - self.t0)).clamp(0.0, 1.0) } else { 1.0 };
        (self.from.0 + (self.to.0 - self.from.0) * k, self.from.1 + (self.to.1 - self.from.1) * k)
    }
}

fn angle(p: (f64, f64)) -> f64 {
    p.1.atan2(p.0)
}

/// `b - a` in radians, in (-pi, pi].
fn ang_diff(a: f64, b: f64) -> f64 {
    let mut d = b - a;
    while d > std::f64::consts::PI {
        d -= std::f64::consts::TAU;
    }
    while d <= -std::f64::consts::PI {
        d += std::f64::consts::TAU;
    }
    d
}

impl Sim {
    /// Dry ground in direction `a` (not sea, not a lava pool): its height.
    pub(super) fn dry_ground(&self, a: f64) -> Option<f64> {
        let g = self.ground_at(a, true);
        let c = ((a.cos() * (g - 0.5)).floor() as i64, (a.sin() * (g - 0.5)).floor() as i64);
        let m = self.world.material(c.0, c.1);
        if m == VACUUM || self.world.planet.mats.get(m).state == State::Liquid {
            None
        } else {
            Some(g)
        }
    }

    /// Scatter the body's herds over its dry land (a new game only).
    pub(super) fn spawn_fauna(&mut self) {
        let Some(f) = self.world.planet.spec.fauna.clone() else { return };
        let herds = (f.herds_per_km * std::f64::consts::TAU * self.world.planet.radius / 1000.0).round().max(1.0) as u32;
        for _ in 0..herds {
            self.spawn_herd(&f);
        }
        self.fauna.start = self.fauna.animals.len() as u32;
    }

    /// One new herd at a spot of dry land.
    fn spawn_herd(&mut self, f: &crate::spec::FaunaSpec) {
        let seed = self.world.planet.spec.seed;
        let h = self.fauna.herds as i64;
        self.fauna.herds += 1;
        let u = |h: u64| crate::noise::unit(h);
        let Some(a0) = (0..20).map(|k| std::f64::consts::TAU * u(hash(seed, h, k, 0xFA0))).find(|&a| self.dry_ground(a).is_some()) else { return };
        let (lo, hi) = (f.herd_size[0].min(f.herd_size[1]), f.herd_size[0].max(f.herd_size[1]));
        let n = lo + ((u(hash(seed, h, 0, 0xFA1)) * (hi - lo + 1) as f64) as u32).min(hi - lo);
        for j in 0..n as i64 {
            let g0 = self.ground_at(a0, true).max(1.0);
            let a = a0 + (u(hash(seed, h, j, 0xFA2)) - 0.5) * 2.0 * HERD_SPREAD_M / g0;
            let Some(g) = self.dry_ground(a) else { continue };
            let kg = f.kg[0] + (f.kg[1] - f.kg[0]) * u(hash(seed, h, j, 0xFA3));
            self.fauna.next_id += 1;
            let pos = (a.cos() * g, a.sin() * g);
            let facing = if u(hash(seed, h, j, 0xFA4)) < 0.5 { -1.0 } else { 1.0 };
            self.fauna.animals.push(Animal { id: self.fauna.next_id, herd: h as u32, pos, prev: pos, moved_at: self.tick, kg, facing, state: AnimalState::Graze, panic_s: 0.0 });
        }
    }

    /// One second of the animals' lives: scares, then a step each.
    pub(super) fn fauna_work(&mut self) {
        if self.fauna.start == 0 {
            return;
        }
        // Never run out: new herds wander in.
        if (self.fauna.animals.len() as u32) < self.fauna.start && self.tick % self.ticks(ARRIVE_EVERY_S) == 0 {
            if let Some(f) = self.world.planet.spec.fauna.clone() {
                self.spawn_herd(&f);
            }
        }
        if self.fauna.animals.is_empty() {
            self.fauna.alarms.clear();
            return;
        }
        // The machines on and near the ground.
        let u = &self.units;
        let mut threats: Vec<(f64, f64)> = Vec::new();
        threats.extend(u.walkers.iter().map(|w| w.pos));
        threats.extend(u.sappers.iter().map(|s| s.pos));
        threats.extend(u.bots.iter().map(|b| b.pos));
        threats.extend(u.bores.iter().map(|b| (b.x, b.y)));
        for i in self.live_sites() {
            threats.push(self.site_pos(i));
        }
        // Herds scared this second, and by what.
        let mut scared: Vec<(u32, (f64, f64))> = std::mem::take(&mut self.fauna.alarms);
        for an in &self.fauna.animals {
            if scared.iter().any(|s| s.0 == an.herd) {
                continue;
            }
            let near = threats.iter().map(|t| (*t, (t.0 - an.pos.0).powi(2) + (t.1 - an.pos.1).powi(2))).filter(|(_, d2)| *d2 < PANIC_M * PANIC_M).min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((t, _)) = near {
                scared.push((an.herd, t));
            }
        }
        // Where each herd is (the middle of its directions).
        let mut middle: std::collections::BTreeMap<u32, (f64, f64)> = std::collections::BTreeMap::new();
        for an in &self.fauna.animals {
            let e = middle.entry(an.herd).or_insert((0.0, 0.0));
            let a = angle(an.pos);
            e.0 += a.cos();
            e.1 += a.sin();
        }
        let tick = self.tick;
        let mut dead = Vec::new();
        for k in 0..self.fauna.animals.len() {
            let mut an = self.fauna.animals[k].clone();
            an.prev = an.pos;
            an.moved_at = tick;
            let a = angle(an.pos);
            let r = len(an.pos).max(1.0);
            if let Some(&(_, t)) = scared.iter().find(|s| s.0 == an.herd) {
                // Away from the scare, along the ground.
                an.state = AnimalState::Flee;
                an.panic_s = PANIC_S;
                an.facing = if ang_diff(angle(t), a) >= 0.0 { 1.0 } else { -1.0 };
            } else if an.state == AnimalState::Flee {
                an.panic_s -= 1.0;
                if an.panic_s <= 0.0 {
                    an.state = AnimalState::Graze;
                }
            } else {
                // Grazing: now and then turn about; strays turn back to the herd.
                let m = middle[&an.herd];
                let d = ang_diff(a, m.1.atan2(m.0));
                if d.abs() * r > HERD_SPREAD_M {
                    an.facing = d.signum();
                } else if crate::noise::unit(hash(tick, an.id as i64, 0, 0xFA5)) < 0.08 {
                    an.facing = -an.facing;
                }
            }
            // Where it stands now (the swarm may have eaten the ground).
            let g = self.ground_at(a, true);
            if r - g > FALL_KILLS_M || self.dry_ground(a).is_none() {
                dead.push(k);
                continue;
            }
            let speed = if an.state == AnimalState::Flee { FLEE_SPEED } else { GRAZE_SPEED };
            let na = a + an.facing * speed / g.max(1.0);
            // Not into water or lava, nor up or down a cliff: turn about.
            match self.dry_ground(na) {
                Some(ng) if (ng - g).abs() <= speed * 1.5 + 1.0 => an.pos = (na.cos() * ng, na.sin() * ng),
                _ => {
                    an.facing = -an.facing;
                    an.pos = (a.cos() * g, a.sin() * g);
                }
            }
            self.fauna.animals[k] = an;
        }
        for k in dead.into_iter().rev() {
            self.fauna.animals.remove(k);
            self.fauna.lost += 1;
        }
    }

    /// Animals near `p` (m, along the ground), nearest first: (index, metres).
    pub(super) fn nearest_animal(&self, p: (f64, f64)) -> Option<(usize, f64)> {
        let a = angle(p);
        let r = len(p).max(1.0);
        self.fauna.animals.iter().enumerate().map(|(i, an)| (i, ang_dist(angle(an.pos), a) * r)).min_by(|x, y| x.1.total_cmp(&y.1).then(x.0.cmp(&y.0)))
    }

    /// Take animal `i` off the body (snatched alive): its kg. Its herd panics.
    pub(super) fn catch_animal(&mut self, i: usize) -> f64 {
        let an = self.fauna.animals.remove(i);
        self.fauna.alarms.push((an.herd, an.pos));
        self.fauna.caught += 1;
        self.fauna.caught_kg += an.kg;
        an.kg
    }

    /// Whether there are any animals left.
    pub fn animals_left(&self) -> usize {
        self.fauna.animals.len()
    }

    /// Animals caught, kg caught, animals lost otherwise.
    pub fn fauna_totals(&self) -> (u64, f64, u64) {
        (self.fauna.caught, self.fauna.caught_kg, self.fauna.lost)
    }

    /// Every animal, for drawing.
    pub fn animal_views(&self) -> Vec<AnimalView> {
        let t = |tick: u64| tick as f64 * self.eco.tick_s;
        let sec = self.ticks(1.0) as f64 * self.eco.tick_s;
        self.fauna
            .animals
            .iter()
            .map(|an| AnimalView { id: an.id, from: an.prev, to: an.pos, t0: t(an.moved_at), t1: t(an.moved_at) + sec, kg: an.kg as f32, facing: an.facing as f32, fleeing: an.state == AnimalState::Flee })
            .collect()
    }

    // ---- Saves and the hash ----

    pub(super) fn write_fauna(&self, o: &mut crate::save::W) {
        let f = &self.fauna;
        o.u32(f.next_id);
        o.u32(f.start);
        o.u32(f.herds);
        o.u64(f.caught);
        o.f64(f.caught_kg);
        o.u64(f.lost);
        o.u32(f.animals.len() as u32);
        for an in &f.animals {
            o.u32(an.id);
            o.u32(an.herd);
            o.u8(an.state as u8);
            for v in [an.pos.0, an.pos.1, an.kg, an.facing, an.panic_s] {
                o.f64(v);
            }
        }
    }

    pub(super) fn read_fauna(&mut self, r: &mut crate::save::R) -> Result<(), String> {
        let tick = self.tick;
        let f = &mut self.fauna;
        f.next_id = r.u32()?;
        f.start = r.u32()?;
        f.herds = r.u32()?;
        f.caught = r.u64()?;
        f.caught_kg = r.f64()?;
        f.lost = r.u64()?;
        for _ in 0..r.u32()? {
            let id = r.u32()?;
            let herd = r.u32()?;
            let state = if r.u8()? == 1 { AnimalState::Flee } else { AnimalState::Graze };
            let mut v = [0.0; 5];
            for x in v.iter_mut() {
                *x = r.f64()?;
            }
            let pos = (v[0], v[1]);
            f.animals.push(Animal { id, herd, pos, prev: pos, moved_at: tick, kg: v[2], facing: v[3], state, panic_s: v[4] });
        }
        Ok(())
    }

    /// The animals, for the state hash (nothing at all on a body without them).
    pub(super) fn fauna_hash(&self, mut mix: impl FnMut(u64)) {
        let f = &self.fauna;
        if f.animals.is_empty() && f.caught == 0 && f.lost == 0 {
            return;
        }
        mix(f.caught);
        mix(f.lost);
        for an in &f.animals {
            mix(an.pos.0.to_bits());
            mix(an.pos.1.to_bits());
        }
    }
}
