//! The glamour pass: eye candy over the interior, laid on top of the
//! realistic generation (data/glamour.toml).
//!
//! The realistic model gives a big planet an interesting crust over a few
//! vast, even layers. This pass breaks those layers up at every scale, the
//! way the surface already is: provinces of other rock kilometres across,
//! mottles inside them, winding seams of ore, crystal-lined geodes, great
//! crystal caverns and growth rings in the core. All of it is placed by
//! density (a jittered grid: one chance per square), never by count, so the
//! deep interior of a planet of any size is as varied as a small body's, and
//! each cell costs the same to work out wherever it is.
//!
//! A body's `glamour` (0 to 1) scales it: 0 leaves the locked realistic model
//! untouched; low settings keep to plausible geology; 1 is science fiction.
//! It only works below a skin under the surface, so the crust's own geology
//! stays as generated, and only on solid rock (seas, magma and cores of melt
//! are left alone).
use crate::geology::{pick, range};
use crate::materials::{MatId, MaterialTable, State, VACUUM};
use crate::noise::{cos_sin, fbm, hash, unit};
use serde::Deserialize;

// ---- The rules, as written in data/glamour.toml ----

#[derive(Debug, Clone, Deserialize)]
pub struct PickRow {
    pub material: String,
    #[serde(default = "one")]
    pub weight: f64,
    /// The lowest glamour this appears at.
    #[serde(default)]
    pub glamour: f64,
}

fn one() -> f64 {
    1.0
}

#[derive(Debug, Clone, Deserialize)]
pub struct HostRow {
    pub host: Vec<String>,
    pub rocks: Vec<PickRow>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProvincesFile {
    pub size: f64,
    pub size_m: [f64; 2],
    pub mottle: f64,
    pub change: f64,
    pub blend_m: f64,
    #[serde(default)]
    pub host: Vec<HostRow>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TierRow {
    pub depth: [f64; 2],
    #[serde(alias = "seams")]
    pub crystals: Vec<PickRow>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeodesFile {
    pub spacing_m: f64,
    pub chance: f64,
    pub radius_m: [f64; 2],
    pub grow_m: f64,
    pub shell: String,
    pub lining: Vec<TierRow>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RibbonsFile {
    pub scale_m: f64,
    pub width: f64,
    pub band: Vec<TierRow>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CavernsFile {
    pub from_glamour: f64,
    pub spacing_m: f64,
    pub chance: f64,
    pub radius_m: [f64; 2],
    pub beams: [u32; 2],
    pub beam_width_m: [f64; 2],
    pub crystals: Vec<PickRow>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoreFile {
    pub hosts: Vec<String>,
    pub ring_m: [f64; 2],
    pub ring: f64,
    pub rings: Vec<PickRow>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GlamourFile {
    pub skin: f64,
    pub skin_m: [f64; 2],
    pub provinces: ProvincesFile,
    pub geodes: GeodesFile,
    pub ribbons: RibbonsFile,
    pub caverns: CavernsFile,
    pub core: CoreFile,
}

/// A pick list with material ids and the glamour each needs.
type Picks = Vec<(MatId, f64, f64)>;

fn resolve(rows: &[PickRow], mats: &MaterialTable) -> Result<Picks, String> {
    rows.iter().map(|r| Ok((mats.id(&r.material).map_err(|e| format!("glamour: {e}"))?, r.weight, r.glamour))).collect()
}

/// The rules with material names turned into ids.
#[derive(Debug, Clone)]
pub struct Rules {
    file: GlamourFile,
    /// Province rocks by host material id.
    hosts: Vec<Option<Picks>>,
    lining: Vec<([f64; 2], Picks)>,
    shell: MatId,
    seams: Vec<([f64; 2], Picks)>,
    cavern: Picks,
    core_hosts: Vec<MatId>,
    rings: Picks,
}

impl Rules {
    pub fn resolve(f: &GlamourFile, mats: &MaterialTable) -> Result<Rules, String> {
        let mut hosts = vec![None; mats.list.len()];
        for h in &f.provinces.host {
            let rocks = resolve(&h.rocks, mats)?;
            for name in &h.host {
                // Hosts may name materials a given catalogue lacks.
                if let Ok(id) = mats.id(name) {
                    hosts[id as usize] = Some(rocks.clone());
                }
            }
        }
        Ok(Rules {
            hosts,
            lining: f.geodes.lining.iter().map(|t| Ok((t.depth, resolve(&t.crystals, mats)?))).collect::<Result<_, String>>()?,
            shell: mats.id(&f.geodes.shell)?,
            seams: f.ribbons.band.iter().map(|t| Ok((t.depth, resolve(&t.crystals, mats)?))).collect::<Result<_, String>>()?,
            cavern: resolve(&f.caverns.crystals, mats)?,
            core_hosts: f.core.hosts.iter().filter_map(|n| mats.id(n).ok()).collect(),
            rings: resolve(&f.core.rings, mats)?,
            file: f.clone(),
        })
    }
}

/// Only what this glamour allows, as a weighted list.
fn allowed(p: &Picks, g: f64) -> Vec<(MatId, f64)> {
    p.iter().filter(|r| g + 1e-9 >= r.2).map(|r| (r.0, r.1)).collect()
}

/// One body's glamour pass.
#[derive(Debug, Clone)]
pub struct Glamour {
    g: f64,
    seed: u64,
    radius: f64,
    skin: f64,
    prov: f64,
    mottle: f64,
    change: f64,
    blend: f64,
    hosts: Vec<Vec<(MatId, f64)>>,
    solid: Vec<bool>,
    geode_g: f64,
    geode_chance: f64,
    geode_r: [f64; 2],
    shell: MatId,
    lining: Vec<([f64; 2], Vec<(MatId, f64)>)>,
    rib_scale: f64,
    rib_width: f64,
    seams: Vec<([f64; 2], Vec<(MatId, f64)>)>,
    cav_g: f64,
    cav_chance: f64,
    cav_r: [f64; 2],
    beams: [u32; 2],
    beam_w: [f64; 2],
    cavern: Vec<(MatId, f64)>,
    core_hosts: Vec<MatId>,
    ring_t: f64,
    rings: Vec<(MatId, f64)>,
}

impl Glamour {
    pub fn build(rules: &Rules, mats: &MaterialTable, g: f64, seed: u64, radius: f64) -> Glamour {
        let f = &rules.file;
        let g = g.clamp(0.0, 1.0);
        let prov = (f.provinces.size * radius).clamp(f.provinces.size_m[0], f.provinces.size_m[1]);
        let cav_chance = if g >= f.caverns.from_glamour { f.caverns.chance * (g - f.caverns.from_glamour) / (1.0 - f.caverns.from_glamour).max(1e-9) } else { 0.0 };
        Glamour {
            g,
            seed,
            radius,
            skin: (f.skin * radius).clamp(f.skin_m[0], f.skin_m[1]),
            prov,
            mottle: prov * f.provinces.mottle,
            change: f.provinces.change * g,
            blend: f.provinces.blend_m * prov,
            hosts: rules.hosts.iter().map(|h| h.as_ref().map(|p| allowed(p, g)).unwrap_or_default()).collect(),
            solid: mats.list.iter().map(|m| m.state == State::Solid).collect(),
            geode_g: f.geodes.spacing_m,
            geode_chance: f.geodes.chance * g,
            geode_r: [f.geodes.radius_m[0], f.geodes.radius_m[1] + f.geodes.grow_m * g],
            shell: rules.shell,
            lining: rules.lining.iter().map(|(d, p)| (*d, allowed(p, g))).collect(),
            rib_scale: f.ribbons.scale_m,
            rib_width: f.ribbons.width * g,
            seams: rules.seams.iter().map(|(d, p)| (*d, allowed(p, g))).collect(),
            cav_g: f.caverns.spacing_m,
            cav_chance,
            cav_r: f.caverns.radius_m,
            beams: f.caverns.beams,
            beam_w: f.caverns.beam_width_m,
            cavern: allowed(&rules.cavern, g),
            core_hosts: rules.core_hosts.clone(),
            ring_t: (f.core.ring * radius).clamp(f.core.ring_m[0], f.core.ring_m[1]),
            rings: allowed(&rules.rings, g),
        }
    }

    /// Mark in `can` (by material id) what this pass could add to a body
    /// that already holds the materials marked there.
    pub fn could_add(&self, can: &mut [bool]) {
        let before: Vec<bool> = can.to_vec();
        let mark = |list: &Vec<(MatId, f64)>, can: &mut [bool]| {
            for (m, w) in list {
                if *w > 0.0 {
                    can[*m as usize] = true;
                }
            }
        };
        for (h, list) in self.hosts.iter().enumerate() {
            if before.get(h).copied().unwrap_or(false) {
                mark(list, can);
            }
        }
        if self.geode_chance > 0.0 {
            can[self.shell as usize] = true;
            for (_, l) in &self.lining {
                mark(l, can);
            }
        }
        if self.rib_width > 0.0 {
            for (_, l) in &self.seams {
                mark(l, can);
            }
        }
        if self.cav_chance > 0.0 {
            mark(&self.cavern, can);
        }
        if self.core_hosts.iter().any(|&h| before[h as usize]) {
            mark(&self.rings, can);
        }
    }

    /// The deepest any feature reaches above the skin line (for callers that
    /// want to skip the pass near the surface cheaply).
    pub fn skin(&self) -> f64 {
        self.skin
    }

    fn tier(list: &[([f64; 2], Vec<(MatId, f64)>)], d: f64) -> Option<&Vec<(MatId, f64)>> {
        list.iter().find(|(r, _)| d >= r[0] && d < r[1]).or(list.last()).map(|(_, p)| p)
    }

    /// The nearest jittered-grid point to (x, y) for squares `size` wide:
    /// its square, and the distance to it.
    fn nearest(&self, x: f64, y: f64, size: f64, salt: u64) -> ((i64, i64), f64) {
        let (i, j) = ((x / size).floor() as i64, (y / size).floor() as i64);
        let mut best = ((i, j), f64::INFINITY);
        for dj in -1..=1 {
            for di in -1..=1 {
                let (ci, cj) = (i + di, j + dj);
                let h = hash(self.seed, ci, cj, salt);
                let px = (ci as f64 + 0.1 + 0.8 * unit(h)) * size;
                let py = (cj as f64 + 0.1 + 0.8 * unit(hash(h, 1, 0, salt))) * size;
                let d = (px - x) * (px - x) + (py - y) * (py - y);
                if d < best.1 {
                    best = ((ci, cj), d);
                }
            }
        }
        (best.0, best.1.sqrt())
    }

    /// The material at (fx, fy), radius r, under the ground at `surface`,
    /// where the realistic model put `m` (and the base layer there is `base`).
    pub fn apply(&self, fx: f64, fy: f64, r: f64, surface: f64, base: MatId, m: MatId) -> MatId {
        if self.g <= 0.0 || m == VACUUM || !self.solid[m as usize] {
            return m;
        }
        let depth = surface - r;
        if depth < self.skin * 0.6 {
            return m;
        }
        // A wavy start, never a ruled line.
        if depth < self.skin * 1.4 && depth < self.skin * (0.6 + 0.8 * fbm(self.seed ^ 0x6A01, fx / 400.0, fy / 400.0, 2)) {
            return m;
        }
        let d = (1.0 - r / self.radius).clamp(0.0, 1.0);
        let mut out = m;

        // Provinces and their mottles, in the base layer only.
        let rocks = &self.hosts[base as usize];
        if m == base && !rocks.is_empty() {
            let w = self.blend;
            let wx = fx + w * (2.0 * fbm(self.seed ^ 0x6A02, fx / (self.prov * 0.5), fy / (self.prov * 0.5), 2) - 1.0);
            let wy = fy + w * (2.0 * fbm(self.seed ^ 0x6A03, fx / (self.prov * 0.5), fy / (self.prov * 0.5), 2) - 1.0);
            // On a big body, regions far wider than a province each favour
            // one rock, so it varies at every scale from a whole hemisphere
            // down.
            let mut favoured: Option<usize> = None;
            let region = 0.07 * self.radius;
            if region > 3.0 * self.prov {
                let rw = 0.15 * region;
                let rx = fx + rw * (2.0 * fbm(self.seed ^ 0x6A0B, fx / (region * 0.5), fy / (region * 0.5), 1) - 1.0);
                let ry = fy + rw * (2.0 * fbm(self.seed ^ 0x6A0C, fx / (region * 0.5), fy / (region * 0.5), 1) - 1.0);
                let (rc, _) = self.nearest(rx, ry, region, 0x6A0D);
                let rh = hash(self.seed, rc.0, rc.1, 0x6A0E);
                if unit(rh) < 0.8 {
                    favoured = Some((unit(hash(rh, 1, 0, 0)) * rocks.len() as f64) as usize);
                }
            }
            let (pc, _) = self.nearest(wx, wy, self.prov, 0x6A04);
            let ph = hash(self.seed, pc.0, pc.1, 0x6A05);
            let change = if favoured.is_some() { (self.change * 2.0).min(0.95) } else { self.change };
            if unit(ph) < change {
                out = match favoured {
                    Some(k) if unit(hash(ph, 3, 0, 0)) < 0.65 => rocks[k.min(rocks.len() - 1)].0,
                    _ => pick(rocks, unit(hash(ph, 2, 0, 0))).unwrap_or(m),
                };
            }
            // Mottles: smaller patches of a neighbouring rock, more of them
            // the bolder the setting.
            // (Round, lumpy blobs around jittered points.)
            let (mc, md) = self.nearest(fx, fy, self.mottle, 0x6A06);
            let mh = hash(self.seed, mc.0, mc.1, 0x6A07);
            let reach = self.mottle * (0.18 + 0.22 * unit(hash(mh, 3, 0, 0)));
            if unit(mh) < 0.3 * self.g && md < reach * 1.35 && md < reach * (1.0 + 0.35 * (2.0 * fbm(self.seed ^ 0x6A08, fx / (self.mottle * 0.15), fy / (self.mottle * 0.15), 2) - 1.0)) {
                out = pick(rocks, unit(hash(mh, 2, 0, 0))).unwrap_or(out);
            }
            // Banding: each province is streaked with thin wavy layers of a
            // second rock, at its own angle and spacing (like gneiss).
            let (bc, bs) = {
                let (a, b) = (2.0 * unit(hash(ph, 4, 0, 0)) - 1.0, 0.2 + unit(hash(ph, 4, 1, 0)));
                let l = (a * a + b * b).sqrt();
                (a / l, b / l)
            };
            let period = 25.0 + 40.0 * unit(hash(ph, 5, 0, 0));
            let n = fx * bc + fy * bs + 0.35 * period * (2.0 * fbm(ph, fx / 60.0, fy / 60.0, 2) - 1.0);
            let band = (n / period).floor() as i64;
            let bh = hash(ph, band, 6, 0);
            let share = (0.04 + 0.14 * self.g) * unit(hash(bh, 1, 0, 0));
            if n / period - (band as f64) < share && unit(bh) < 0.6 + 0.3 * self.g {
                out = pick(rocks, unit(hash(bh, 2, 0, 0))).unwrap_or(out);
            }
            // Xenoliths: lumps of other rock, a few metres across.
            let (xc, xd) = self.nearest(fx, fy, 28.0, 0x6A09);
            let xh = hash(self.seed, xc.0, xc.1, 0x6A0A);
            if unit(xh) < 0.3 * self.g {
                let xr = 1.5 + 4.5 * unit(hash(xh, 1, 0, 0));
                if xd < xr * 1.4 && xd < xr * (1.0 + 0.4 * (2.0 * fbm(xh, fx / 2.5, fy / 2.5, 2) - 1.0)) {
                    out = pick(rocks, unit(hash(xh, 2, 0, 0))).unwrap_or(out);
                }
            }
        }

        // Growth rings in a solid core.
        if !self.rings.is_empty() && self.core_hosts.contains(&m) {
            let t = self.ring_t;
            let rr = r + 1.5 * t * (2.0 * fbm(self.seed ^ 0x6A10, fx / (t * 9.0), fy / (t * 9.0), 2) - 1.0);
            let band = (rr / t).floor() as i64;
            let bh = hash(self.seed, band, 0, 0x6A11);
            if unit(bh) < 0.6 * self.g {
                out = pick(&self.rings, unit(hash(bh, 1, 0, 0))).unwrap_or(out);
            }
        }

        // Ribbons: winding seams of ore and gem, two families crossing.
        // (Only in some regions, so seams come in districts.)
        if self.rib_width > 0.0 && fbm(self.seed ^ 0x6A21, fx / (self.rib_scale * 6.0), fy / (self.rib_scale * 6.0), 2) > 0.55 {
            for (k, scale) in [(0u64, self.rib_scale), (1, self.rib_scale * 1.9)] {
                let n = fbm(self.seed ^ (0x6A20 + k), fx / scale, fy / scale, 3);
                if 1.0 - (2.0 * n - 1.0).abs() > 1.0 - self.rib_width {
                    // One ore along each stretch of seam.
                    let s = scale * 1.5;
                    let sh = hash(self.seed, (fx / s).floor() as i64, (fy / s).floor() as i64, 0x6A22 + k);
                    if let Some(p) = Self::tier(&self.seams, d) {
                        out = pick(p, unit(sh)).unwrap_or(out);
                    }
                }
            }
        }

        // Geodes: a shell, crystals pointing in, a hollow heart.
        if self.geode_chance > 0.0 {
            let gsz = self.geode_g;
            let (i, j) = ((fx / gsz).floor() as i64, (fy / gsz).floor() as i64);
            'geodes: for dj in -1..=1 {
                for di in -1..=1 {
                    let (ci, cj) = (i + di, j + dj);
                    let h = hash(self.seed, ci, cj, 0x6A30);
                    if unit(h) >= self.geode_chance {
                        continue;
                    }
                    let cx = (ci as f64 + 0.15 + 0.7 * unit(hash(h, 1, 0, 0))) * gsz;
                    let cy = (cj as f64 + 0.15 + 0.7 * unit(hash(h, 2, 0, 0))) * gsz;
                    let rad = range(self.geode_r, unit(hash(h, 3, 0, 0)));
                    // An egg, not a ball: squashed and turned, with a lumpy rim.
                    let (oc, os) = cos_sin(unit(hash(h, 7, 0, 0)) * std::f64::consts::PI);
                    let squash = 0.6 + 0.4 * unit(hash(h, 8, 0, 0));
                    let (ex, ey) = (fx - cx, fy - cy);
                    let (dx, dy) = (ex * oc + ey * os, (ey * oc - ex * os) / squash);
                    let dist2 = dx * dx + dy * dy;
                    let lump = 1.0 + 0.14 * (2.0 * fbm(h, fx / 3.0, fy / 3.0, 2) - 1.0);
                    let rad = rad * lump;
                    if dist2 >= rad * rad {
                        continue;
                    }
                    let dist = dist2.sqrt();
                    let shell = (0.12 * rad).clamp(0.8, 2.0);
                    if dist > rad - shell {
                        out = self.shell;
                        break 'geodes;
                    }
                    // Crystals: their length varies around the rim, in
                    // pointed teeth. Some geodes are filled solid.
                    let teeth = 10.0 + rad;
                    let a = pseudo_angle(dx, dy) * teeth;
                    let k = a.floor() as i64;
                    let tip = 1.0 - (2.0 * (a - a.floor()) - 1.0).abs();
                    let filled = unit(hash(h, 9, 0, 0)) < 0.4;
                    let len = rad * (0.15 + 0.35 * unit(hash(h, k, 4, 0))) * (0.35 + 0.65 * tip);
                    let lining = Self::tier(&self.lining, d);
                    if dist > rad - shell - len {
                        out = lining.and_then(|p| pick(p, unit(hash(h, 5, 0, 0)))).unwrap_or(self.shell);
                    } else if filled {
                        // A second crystal fills the middle.
                        out = lining.and_then(|p| pick(p, unit(hash(h, 6, 0, 0)))).unwrap_or(self.shell);
                    } else {
                        out = VACUUM;
                    }
                    break 'geodes;
                }
            }
        }

        // Crystal caverns: a great hollow, beams of crystal from its walls.
        if self.cav_chance > 0.0 {
            let csz = self.cav_g;
            let (i, j) = ((fx / csz).floor() as i64, (fy / csz).floor() as i64);
            for dj in -1..=1 {
                for di in -1..=1 {
                    let (ci, cj) = (i + di, j + dj);
                    let h = hash(self.seed, ci, cj, 0x6A40);
                    if unit(h) >= self.cav_chance {
                        continue;
                    }
                    let cx = (ci as f64 + 0.2 + 0.6 * unit(hash(h, 1, 0, 0))) * csz;
                    let cy = (cj as f64 + 0.2 + 0.6 * unit(hash(h, 2, 0, 0))) * csz;
                    let rad = range(self.cav_r, unit(hash(h, 3, 0, 0)));
                    // A flattened hollow (wider than tall, around its own up).
                    let rc = (cx * cx + cy * cy).sqrt().max(1.0);
                    let (ux, uy) = (cx / rc, cy / rc);
                    let (dx, dy) = (fx - cx, fy - cy);
                    let up = dx * ux + dy * uy;
                    let across = dx * uy - dy * ux;
                    let wall = 1.0 + 0.12 * (2.0 * fbm(h, fx / 9.0, fy / 9.0, 2) - 1.0);
                    let e = (across / rad) * (across / rad) + (up / (0.6 * rad)) * (up / (0.6 * rad));
                    if e > wall * wall {
                        continue;
                    }
                    let mut mat = VACUUM;
                    let nb = self.beams[0] + (unit(hash(h, 4, 0, 0)) * (self.beams[1] - self.beams[0] + 1) as f64) as u32;
                    for b in 0..nb as i64 {
                        let bh = hash(h, b, 6, 0);
                        let (c, s) = cos_sin(unit(bh) * std::f64::consts::TAU);
                        // From the wall toward the middle, turned aside a little.
                        let (sx, sy) = (cx + rad * c * 0.98, cy + rad * s * 0.6 * 0.98);
                        let turn = (unit(hash(bh, 1, 0, 0)) - 0.5) * 1.2;
                        let (tc, ts) = cos_sin(turn);
                        let (vx, vy) = (-c * tc + s * ts, -c * ts - s * tc);
                        let vl = (vx * vx + vy * vy).sqrt();
                        let (vx, vy) = (vx / vl, vy / vl);
                        let len = rad * (0.5 + 0.6 * unit(hash(bh, 2, 0, 0)));
                        let half = range(self.beam_w, unit(hash(bh, 3, 0, 0)));
                        let (px, py) = (fx - sx, fy - sy);
                        let t = px * vx + py * vy;
                        if t < -half || t > len {
                            continue;
                        }
                        let off = (px * vy - py * vx).abs();
                        // A pointed tip.
                        let taper = ((len - t) / (half * 1.6)).min(1.0);
                        if off < half * taper {
                            mat = pick(&self.cavern, unit(hash(bh, 4, 0, 0))).unwrap_or(VACUUM);
                            break;
                        }
                    }
                    return mat;
                }
            }
        }
        out
    }
}

/// An angle-like value in [0, 1) around the origin, without trigonometry.
fn pseudo_angle(x: f64, y: f64) -> f64 {
    let p = if x.abs() + y.abs() > 0.0 { x / (x.abs() + y.abs()) } else { 0.0 };
    let a = if y >= 0.0 { 1.0 - p } else { 3.0 + p };
    a / 4.0
}
