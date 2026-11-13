//! Heat-sink bots: hover over a pool of magma, molten metal or sulfur,
//! grow cooling tentacles down through it and freeze it to stone for the
//! cutters; the heat becomes power.
//!
//! Unit file: the heat-sink bot's own behaviour; the platform calls
//! `bots_work` once a game second (and `scan_molten` now and then).

use super::*;

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
    pub(super) id: u32,
    pub(super) state: HeatState,
    pub(super) pos: (f64, f64),
    pub(super) prev: (f64, f64),
    pub(super) moved_at: u64,
    /// The pool's surface cell it works, and where it hovers over it.
    pub(super) spot: (i64, i64),
    pub(super) hover: (f64, f64),
    /// How far its tentacles reach (m), seconds of life left, seconds with
    /// nothing to freeze, kg frozen.
    pub(super) reach: f64,
    pub(super) life: f64,
    pub(super) idle_s: f64,
    pub(super) frozen_kg: f64,
    /// Where its tentacles last froze (up to six), for drawing.
    pub(super) tips: Vec<(i64, i64)>,
}

impl Sim {
    /// Look round the working flocks for open pools of magma: hot liquid
    /// cells with open space over them.
    pub(super) fn scan_molten(&mut self) {
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

    pub(super) fn launch_bot(&mut self) {
        let free = self.units.molten.iter().copied().find(|c| !self.units.bots.iter().any(|b| (b.spot.0 - c.0).pow(2) + (b.spot.1 - c.1).pow(2) < 40 * 40));
        let Some(spot) = free.or_else(|| self.units.molten.first().copied()) else { return };
        let up = unit_of(mid(spot));
        let hover = (mid(spot).0 + up.0 * 7.0, mid(spot).1 + up.1 * 7.0);
        let start = self.mother_over(hover.1.atan2(hover.0));
        let id = self.units.new_id();
        let life = self.eco.units.heatsink.life_s;
        self.units.bots.push(HeatBot { id, state: HeatState::Fly, pos: start, prev: start, moved_at: self.tick, spot, hover, reach: 2.0, life, idle_s: 0.0, frozen_kg: 0.0, tips: Vec::new() });
    }

    pub(super) fn bots_work(&mut self) {
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
}
