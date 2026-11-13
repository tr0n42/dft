//! Abductors: tall three-legged walkers that catch animals alive in stasis
//! sacs (worth more than laser-cooked ones; each catch is counted for the
//! future menagerie). They walk on the forester's machinery (forester.rs,
//! `Walker` with kind `Unit::Abductor`: dropping, unfolding, striding,
//! leaping); what is theirs is here: where to go, the hunt, and what a sac
//! of live animals is worth when the couriers carry it up.
//!
//! Placeholder behaviour for testing on the Dev planet's placeholder animals
//! (src/fauna.rs): stride to a herd, chase the nearest animal, snatch it
//! when in reach (its herd panics and runs), and drop full sacs for the
//! couriers. Only ordered while there are animals.
//!
//! Unit file: the abductor's own behaviour goes here as it is built.

use super::*;
use crate::production::Unit;

/// Flesh, for the books: roughly a mammal's make-up by mass.
const FLESH: [(&str, f64); 8] = [("O", 0.65), ("C", 0.18), ("H", 0.10), ("N", 0.03), ("Ca", 0.015), ("P", 0.01), ("K", 0.004), ("S", 0.011)];

impl Sim {
    /// Where to send an abductor at direction `from`: the nearest animal no
    /// other abductor is after (or the nearest of all).
    pub(super) fn abductor_goal(&self, from: f64, me: u32) -> Option<f64> {
        let r = self.world.planet.radius;
        let busy = |a: f64| self.units.walkers.iter().any(|w| w.kind == Unit::Abductor && w.id != me && ang_dist(w.goal_a, a) * r < 150.0);
        let near = |free: bool| {
            self.fauna.animals.iter().map(|an| an.pos.1.atan2(an.pos.0)).filter(|&a| !free || !busy(a)).min_by(|x, y| ang_dist(*x, from).total_cmp(&ang_dist(*y, from)))
        };
        near(true).or_else(|| near(false))
    }

    /// A second of an abductor's hunt where it stands: snatch the nearest
    /// animal if in reach, else run it down, else stride to another herd.
    pub(super) fn abductor_hunt(&mut self, w: &mut Walker) {
        let spec = self.eco.units.abductor.clone();
        let a = w.pos.1.atan2(w.pos.0);
        let r = len(w.pos).max(1.0);
        match self.nearest_animal(w.pos) {
            None => w.life = w.life.min(30.0),
            Some((i, d)) if d <= spec.reach_m => {
                // Into a stasis sac, alive.
                let p = self.fauna.animals[i].pos;
                let kg = self.catch_animal(i);
                w.bag += kg;
                w.taken_kg += kg;
                w.claw = p;
                w.cut_at = self.tick;
                if w.bag >= spec.bag_kg {
                    self.drop_bag(w);
                }
            }
            Some((i, d)) if d > spec.chase_m => {
                // Too far to run down: stride (and leap) toward it.
                let p = self.fauna.animals[i].pos;
                w.goal_a = self.abductor_goal(a, w.id).unwrap_or(p.1.atan2(p.0));
                w.state = WalkState::Stride;
            }
            Some((i, d)) => {
                let p = self.fauna.animals[i].pos;
                let target = p.1.atan2(p.0);
                let mut da = target - a;
                while da > std::f64::consts::PI {
                    da -= std::f64::consts::TAU;
                }
                while da < -std::f64::consts::PI {
                    da += std::f64::consts::TAU;
                }
                w.facing = if da >= 0.0 { 1.0 } else { -1.0 };
                let step = spec.chase_speed.min(d - spec.reach_m * 0.5).max(0.0);
                let na = a + w.facing * step / r;
                let g = self.ground_at(na, true);
                w.pos = (na.cos() * g, na.sin() * g);
                w.goal_a = target;
            }
        }
    }

    /// A sac of live animals reaches the mother swarm: credit it, worth
    /// `live_multiplier` times its mass.
    pub(super) fn credit_live(&mut self, kg: f64) {
        let comp: std::collections::BTreeMap<String, f64> = FLESH.iter().map(|(e, k)| (e.to_string(), *k)).collect();
        self.stock.add_composition(&self.world.planet.mats.elements, &comp, kg);
        let credited = kg * self.eco.units.abductor.live_multiplier.max(1.0);
        self.mass += credited;
        self.delivered_credited += credited;
        self.window_mined += kg;
    }
}
