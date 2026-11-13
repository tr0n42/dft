//! Tunnelers: bore shafts down from a flock, leaving staging points
//! (shaft hubs) along them so cutters can work deep.
//!
//! Unit file: the tunneler's own behaviour; the platform calls
//! `tunnel_work` once a game second.

use super::*;

/// One Tunneler and its shaft.
#[derive(Debug, Clone)]
pub(super) struct Bore {
    /// Drill head and heading (a unit vector).
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) hx: f64,
    pub(super) hy: f64,
    /// The point it is steering toward.
    pub(super) aim: (f64, f64),
    pub(super) length: f64,
    /// Drilling time banked toward the next metre.
    pub(super) progress_s: f64,
    /// Boring now, or pulling out and moving until `resume_at`.
    pub(super) active: bool,
    pub(super) resume_at: u64,
    /// Where its head was a second ago, and when it moved on (for the
    /// screen; not saved).
    pub(super) prev: (f64, f64),
    pub(super) moved_at: u64,
}

impl Sim {
    /// Average extra worth of the solid rock around (x, y): the centre and
    /// eight points 10 m out.
    pub(super) fn ore_smell(&self, x: f64, y: f64) -> f64 {
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
    pub(super) fn choose_aim(&self, t: &crate::economy::Tunneler, x: f64, y: f64, hx: f64, hy: f64) -> (f64, f64) {
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
    pub(super) fn start_bore(&mut self, i: usize, unit: bool, t: &crate::economy::Tunneler) {
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
    pub(super) fn bores(&mut self, unit: bool) -> &mut Vec<Bore> {
        if unit {
            &mut self.units.bores
        } else {
            &mut self.tunnelers
        }
    }

    /// Solid cells within the bore around (x, y), and whether lava is there.
    pub(super) fn bore_cells(&self, x: f64, y: f64, radius: f64) -> (Vec<(i64, i64)>, bool) {
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
    pub(super) fn tunnel_work(&mut self) {
        if !self.tunnelers.is_empty() {
            let t = self.eco.tunneler.clone();
            self.bore_list(false, &t);
        }
        if !self.units.bores.is_empty() {
            let t = self.unit_tunneler();
            self.bore_list(true, &t);
        }
    }

    pub(super) fn bore_list(&mut self, as_unit: bool, t: &crate::economy::Tunneler) {
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

    pub(super) fn launch_tunneler(&mut self) {
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
}
