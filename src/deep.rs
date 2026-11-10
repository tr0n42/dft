//! Structure in the deep interior: the mantle and the core.
//!
//! Real mantles and cores look plain in textbook diagrams but are full of
//! structure seismology can see. Here, all of it is built once at load time as
//! bounded shapes (plumes, slabs, piles) or as cheap functions of position
//! (convection streaks, core crystals and flow), so the per-cell cost stays
//! small and huge bodies still generate on demand.
//!
//! Only add, multiply, divide and sqrt are used, so results match everywhere.

use crate::geology::{pick, pseudo_angle, random_dir, range};
use crate::materials::{MatId, MaterialTable};
use crate::noise::{fbm, hash, unit};
use crate::spec::{Deep as DeepSpec, Pick};

fn picks(list: &[Pick], mats: &MaterialTable) -> Result<Vec<(MatId, f64)>, String> {
    list.iter().map(|p| Ok((mats.id(&p.material)?, p.weight))).collect()
}

/// 0 at t = 0 and t = 1, 1 at t = 0.5: a rounded hump.
fn bump(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    4.0 * t * (1.0 - t)
}

struct Streaks {
    hosts: Vec<MatId>,
    r0: f64,
    r1: f64,
    cells: f64,
    bands: f64,
    width: f64,
    fraction: f64,
    warp: f64,
    rocks: Vec<(MatId, f64)>,
    seed: u64,
}

impl Streaks {
    /// The streak material at (fx, fy), if the cell lies in a streak.
    fn at(&self, fx: f64, fy: f64, r: f64, m: MatId) -> Option<MatId> {
        if r < self.r0 || r > self.r1 || !self.hosts.contains(&m) {
            return None;
        }
        // Position in the convection cell: u around the body, v up through
        // the shell, both bent by a slow warp so loops aren't perfect ovals.
        let a = pseudo_angle(fx / r, fy / r) / 4.0;
        let span = self.r1 - self.r0;
        let ws = span * 0.6;
        let wu = self.warp * (2.0 * fbm(self.seed ^ 0xA1, fx / ws, fy / ws, 3) - 1.0);
        let wv = self.warp * (2.0 * fbm(self.seed ^ 0xA2, fx / ws, fy / ws, 3) - 1.0);
        let cu = a * self.cells + wu * 0.5;
        let cell = cu.floor();
        let u = cu - cell;
        let v = (r - self.r0) / span + wv * 0.5;
        // Streamlines of a convection cell are closed loops around its middle:
        // the level sets of bump(u) * bump(v).
        let psi = bump(u) * bump(v);
        let q = psi * self.bands + 0.15 * (2.0 * fbm(self.seed ^ 0xA3, fx / (ws * 0.2), fy / (ws * 0.2), 2) - 1.0);
        let band = q.floor() as i64;
        let f = q - q.floor();
        let ci = (cell as i64).rem_euclid(self.cells as i64);
        let h = hash(self.seed, ci, band, 0x57E);
        if unit(h) >= self.fraction || f >= self.width {
            return None;
        }
        pick(&self.rocks, unit(hash(h, 1, 0, 0)))
    }
}

struct Plume {
    ux: f64,
    uy: f64,
    tx: f64,
    ty: f64,
    base: f64,
    head_r: f64,
    stem_half: f64,
    head: f64,
    seed: u64,
}

struct Plumes {
    list: Vec<Plume>,
    material: MatId,
    melt: Option<MatId>,
    melt_share: f64,
    heat: f64,
}

impl Plumes {
    /// The plume's lateral wander at distance `along` from the centre.
    fn wander(p: &Plume, along: f64) -> f64 {
        p.stem_half * 4.0 * (2.0 * fbm(p.seed, along / (p.stem_half * 30.0), 0.5, 2) - 1.0)
    }

    /// How deep inside a plume (fx, fy) lies: >0 inside (1 at the middle),
    /// with the part of it (stem or head) and the plume.
    fn inside(&self, fx: f64, fy: f64) -> Option<(f64, bool, usize)> {
        for (i, p) in self.list.iter().enumerate() {
            let along = fx * p.ux + fy * p.uy;
            if along < p.base || along > p.head_r + p.head {
                continue;
            }
            let raw = fx * p.tx + fy * p.ty;
            // The widest a plume can reach sideways: wander plus head or stem.
            if raw.abs() > p.stem_half * 4.0 + (p.head * 1.4 * 1.15).max(p.stem_half * 1.2) {
                continue;
            }
            // Above the stem's top and too far out for the head (its edge
            // wobbles by at most 15%): nothing here, and no noise needed.
            if along >= p.head_r && (along - p.head_r) / (p.head * 0.55) >= 1.15 {
                continue;
            }
            let lat = raw - Self::wander(p, along);
            // The head: a flattened mushroom cap, wider than tall.
            let (dr, dl) = ((along - p.head_r) / (p.head * 0.55), lat / (p.head * 1.4));
            let wob = 0.85 + 0.3 * fbm(p.seed ^ 0x4EAD, fx / (p.head * 0.5), fy / (p.head * 0.5), 2);
            let hd = (dr * dr + dl * dl).sqrt() / wob;
            if hd < 1.0 {
                return Some((1.0 - hd, true, i));
            }
            // The stem: narrowing a little as it rises.
            let w = p.stem_half * (1.2 - 0.4 * (along - p.base) / (p.head_r - p.base).max(1.0));
            if along < p.head_r && lat.abs() < w {
                return Some((1.0 - lat.abs() / w, false, i));
            }
        }
        None
    }

    fn at(&self, fx: f64, fy: f64) -> Option<MatId> {
        let (_, head, i) = self.inside(fx, fy)?;
        if head {
            if let Some(melt) = self.melt {
                let p = &self.list[i];
                if fbm(p.seed ^ 0x3E17, fx / (p.head * 0.22), fy / (p.head * 0.22), 3) > 1.0 - self.melt_share {
                    return Some(melt);
                }
            }
        }
        Some(self.material)
    }

    fn heat(&self, fx: f64, fy: f64) -> f64 {
        self.inside(fx, fy).map_or(0.0, |(d, _, _)| self.heat * (0.5 + 0.5 * d))
    }
}

/// A slab as a polyline with a thickness, plus the bucket grid of its segments.
struct Slab {
    pts: Vec<(f64, f64)>,
    /// Each segment's bounding box, padded by the slab's widest half-thickness.
    boxes: Vec<(f64, f64, f64, f64)>,
    half: f64,
    seed: u64,
    min: (f64, f64),
    max: (f64, f64),
}

struct Slabs {
    list: Vec<Slab>,
    material: MatId,
    chill: f64,
    diamond: Option<(MatId, f64)>,
    diamond_below: f64,
}

impl Slabs {
    /// Depth into the nearest slab: >0 inside (1 at its middle line).
    fn inside(&self, fx: f64, fy: f64) -> Option<f64> {
        let mut best: Option<f64> = None;
        for s in &self.list {
            let pad = s.half * 1.5;
            if fx < s.min.0 - pad || fx > s.max.0 + pad || fy < s.min.1 - pad || fy > s.max.1 + pad {
                continue;
            }
            let n = s.pts.len();
            for k in 0..n - 1 {
                let bx = s.boxes[k];
                if fx < bx.0 || fx > bx.1 || fy < bx.2 || fy > bx.3 {
                    continue;
                }
                let (a, b) = (s.pts[k], s.pts[k + 1]);
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let l2 = dx * dx + dy * dy;
                let t = (((fx - a.0) * dx + (fy - a.1) * dy) / l2).clamp(0.0, 1.0);
                let (px, py) = (a.0 + dx * t - fx, a.1 + dy * t - fy);
                let d = (px * px + py * py).sqrt();
                if d >= s.half * 1.2 {
                    continue;
                }
                // Thinner toward the slab's leading edge, with a lumpy outline.
                let along = (k as f64 + t) / (n - 1) as f64;
                let wob = 0.8 + 0.4 * fbm(s.seed, fx / (s.half * 6.0), fy / (s.half * 6.0), 2);
                let half = s.half * (1.0 - 0.6 * along * along) * wob;
                if d < half {
                    let v = 1.0 - d / half;
                    best = Some(best.map_or(v, |b: f64| b.max(v)));
                }
            }
        }
        best
    }

    fn at(&self, fx: f64, fy: f64, r: f64) -> Option<MatId> {
        self.inside(fx, fy)?;
        if let Some((dm, chance)) = self.diamond {
            if r < self.diamond_below && unit(hash(0xD1A, fx as i64, fy as i64, 0x5D)) < chance {
                return Some(dm);
            }
        }
        Some(self.material)
    }

    fn heat(&self, fx: f64, fy: f64) -> f64 {
        self.inside(fx, fy).map_or(0.0, |d| -self.chill * (0.5 + 0.5 * d))
    }
}

struct Pile {
    ux: f64,
    uy: f64,
    /// Half-width as a chord between unit vectors (about the angle, in radians).
    chord: f64,
    height: f64,
    seed: u64,
}

struct Piles {
    list: Vec<Pile>,
    at: f64,
    material: MatId,
    melt: Option<MatId>,
    melt_m: f64,
}

impl Piles {
    fn at(&self, fx: f64, fy: f64, r: f64) -> Option<MatId> {
        let up = r - self.at;
        if up < 0.0 {
            return None;
        }
        let (ux, uy) = (fx / r, fy / r);
        for p in &self.list {
            if up > p.height * 1.3 {
                continue;
            }
            let (dx, dy) = (ux - p.ux, uy - p.uy);
            let c = (dx * dx + dy * dy).sqrt() / p.chord;
            if c > 1.25 {
                continue;
            }
            // A low dome with steep, lumpy sides.
            let wob = 0.8 + 0.4 * fbm(p.seed, fx / (p.height * 2.0), fy / (p.height * 2.0), 3);
            let h = p.height * (1.0 - c * c * c * c).max(0.0) * wob;
            if up < h {
                return Some(self.material);
            }
            // Thin pockets of melt along the floor at the pile's edges.
            if let Some(m) = self.melt {
                if up < self.melt_m * wob && c > 0.85 && c < 1.2 {
                    return Some(m);
                }
            }
        }
        None
    }
}

struct InnerCore {
    host: MatId,
    radius: f64,
    grain: f64,
    alt: MatId,
    precious: MatId,
    centre: f64,
    edge: f64,
    boundary: f64,
    seed: u64,
}

impl InnerCore {
    /// Crystals radiating from the centre: grains on a grid of (radius,
    /// distance around), long in the radial direction. Precious alloy gathers
    /// on the grain boundaries, most of all near the centre.
    fn at(&self, fx: f64, fy: f64, r: f64, m: MatId) -> Option<MatId> {
        if m != self.host || r < 1.0 {
            return None;
        }
        let a = pseudo_angle(fx / r, fy / r) / 4.0;
        // Crystals grow outward from the centre, so they narrow toward it. To
        // stop them pinching to a point, the count around halves each time
        // the radius halves (growth shells), and the innermost core has small
        // even grains of its own.
        if r < self.radius * 0.18 {
            let g = self.grain * 0.6;
            let (gx, gy) = ((fx / g).floor() as i64, (fy / g).floor() as i64);
            let (mut d1, mut d2, mut best) = (f64::MAX, f64::MAX, (0i64, 0i64));
            for j in -1..=1 {
                for i in -1..=1 {
                    let (cx, cy) = (gx + i, gy + j);
                    let h = hash(self.seed ^ 0x11C, cx, cy, 0x1C);
                    let (sx, sy) = ((cx as f64 + unit(h)) * g, (cy as f64 + unit(hash(h, 1, 0, 0))) * g);
                    let d = (fx - sx) * (fx - sx) + (fy - sy) * (fy - sy);
                    if d < d1 {
                        d2 = d1;
                        d1 = d;
                        best = (cx, cy);
                    } else if d < d2 {
                        d2 = d;
                    }
                }
            }
            if (d2.sqrt() - d1.sqrt()) / 2.0 < self.boundary && unit(hash(self.seed, fx as i64, fy as i64, 0x9E)) < self.centre {
                return Some(self.precious);
            }
            return if unit(hash(self.seed ^ 0x11C, best.0, best.1, 0xA17)) < 0.5 { Some(self.alt) } else { None };
        }
        let mut shell_r = self.radius;
        while r < shell_r * 0.5 {
            shell_r *= 0.5;
        }
        let around = (2.0 * std::f64::consts::PI * shell_r / self.grain).round().max(6.0);
        let qa = a * around;
        let qr = r / (self.grain * 3.5);
        let metres_per_qa = 2.0 * std::f64::consts::PI * r / around;
        let (gr, ga) = (qr.floor() as i64, qa.floor() as i64);
        let (mut d1, mut d2, mut best) = (f64::MAX, f64::MAX, (0i64, 0i64));
        for j in -1..=1 {
            for i in -1..=1 {
                let (cr, ca) = (gr + j, ga + i);
                let h = hash(self.seed, cr, ca.rem_euclid(around as i64), 0x1C);
                let sr = cr as f64 + 0.15 + 0.7 * unit(h);
                let sa = ca as f64 + 0.15 + 0.7 * unit(hash(h, 1, 0, 0));
                // Distances in metres, so grains come out elongated radially.
                let (dr, da) = ((qr - sr) * 3.5, (qa - sa) * metres_per_qa / self.grain);
                let d = dr * dr + da * da;
                if d < d1 {
                    d2 = d1;
                    d1 = d;
                    best = (cr, ca);
                } else if d < d2 {
                    d2 = d;
                }
            }
        }
        let edge = (d2.sqrt() - d1.sqrt()) / 2.0 * self.grain;
        if edge < self.boundary {
            let f = (r / self.radius).min(1.0);
            let chance = self.centre + (self.edge - self.centre) * f;
            if unit(hash(self.seed, fx as i64, fy as i64, 0x9E)) < chance {
                return Some(self.precious);
            }
        }
        if unit(hash(self.seed, best.0, best.1.rem_euclid(around as i64), 0xA17)) < 0.5 {
            return Some(self.alt);
        }
        None
    }
}

struct OuterCore {
    host: MatId,
    outer: f64,
    material: MatId,
    column: f64,
    share: f64,
    top: Option<MatId>,
    top_m: f64,
    seed: u64,
}

impl OuterCore {
    fn at(&self, fx: f64, fy: f64, r: f64, m: MatId) -> Option<MatId> {
        if m != self.host {
            return None;
        }
        if let Some(t) = self.top {
            let wob = 0.6 + 0.8 * fbm(self.seed ^ 0x70, fx / (self.top_m * 8.0), fy / (self.top_m * 8.0), 2);
            if r > self.outer - self.top_m * wob {
                return Some(t);
            }
        }
        // Columns parallel to the spin axis (vertical here): noise long in y.
        let n = fbm(self.seed, fx / self.column, fy / (self.column * 9.0), 3);
        if n > 1.0 - self.share {
            return Some(self.material);
        }
        None
    }
}

pub struct Deep {
    streaks: Vec<Streaks>,
    plumes: Vec<Plumes>,
    slabs: Vec<Slabs>,
    piles: Option<Piles>,
    inner: Option<InnerCore>,
    outer: Option<OuterCore>,
}

impl Deep {
    pub fn build(
        spec: &DeepSpec,
        layers: &[crate::spec::Layer],
        mats: &MaterialTable,
        seed: u64,
        radius: f64,
        surface: impl Fn(f64, f64) -> f64,
    ) -> Result<Self, String> {
        let streaks = spec
            .streaks
            .iter()
            .enumerate()
            .map(|(i, s)| {
                Ok(Streaks {
                    hosts: s.host.iter().map(|n| mats.id(n)).collect::<Result<_, _>>()?,
                    r0: s.r_min * radius,
                    r1: s.r_max * radius,
                    cells: s.cells.max(1) as f64,
                    bands: s.bands,
                    width: s.width,
                    fraction: s.fraction,
                    warp: s.warp,
                    rocks: picks(&s.rock, mats)?,
                    seed: hash(seed, i as i64, 0, 0x57A),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;

        let mut plumes = Vec::new();
        for (pi, p) in spec.plume.iter().enumerate() {
            let mut list = Vec::new();
            for k in 0..p.count as i64 {
                let s = hash(seed, pi as i64, k, 0x9_1E);
                let (ux, uy) = random_dir(s, 0);
                list.push(Plume {
                    ux,
                    uy,
                    tx: -uy,
                    ty: ux,
                    base: p.base * radius,
                    head_r: p.top * radius,
                    stem_half: range(p.stem_m, unit(hash(s, 1, 0, 0))) / 2.0,
                    head: range(p.head_m, unit(hash(s, 2, 0, 0))),
                    seed: s,
                });
            }
            plumes.push(Plumes {
                list,
                material: mats.id(&p.material)?,
                melt: match &p.melt {
                    Some(m) => Some(mats.id(m)?),
                    None => None,
                },
                melt_share: p.melt_share,
                heat: p.heat_k,
            });
        }

        let mut slabs = Vec::new();
        for (si, sp) in spec.slab.iter().enumerate() {
            let mut list = Vec::new();
            for k in 0..sp.count as i64 {
                let s = hash(seed, si as i64, k, 0x5_1AB);
                let (ux, uy) = random_dir(s, 0);
                let side = if unit(hash(s, 1, 0, 0)) < 0.5 { 1.0 } else { -1.0 };
                let (tx, ty) = (-uy * side, ux * side);
                let rs = surface(ux, uy);
                // A curve that starts shallow at the trench and steepens:
                // a quadratic Bezier from the surface to its deepest point.
                let bottom = sp.bottom * radius;
                let lateral = (rs - bottom) * (0.35 + 0.3 * unit(hash(s, 2, 0, 0)));
                let p0 = (ux * rs, uy * rs);
                let p1 = (ux * (rs - (rs - bottom) * 0.25) + tx * lateral * 0.9, uy * (rs - (rs - bottom) * 0.25) + ty * lateral * 0.9);
                let p2 = (ux * bottom + tx * lateral, uy * bottom + ty * lateral);
                let n = 40;
                let mut pts: Vec<(f64, f64)> = (0..=n)
                    .map(|i| {
                        let t = i as f64 / n as f64;
                        let (a, b, c) = ((1.0 - t) * (1.0 - t), 2.0 * t * (1.0 - t), t * t);
                        (a * p0.0 + b * p1.0 + c * p2.0, a * p0.1 + b * p1.1 + c * p2.1)
                    })
                    .collect();
                // Some slabs stall at the transition zone and lie flat along it.
                if sp.stall_at > 0.0 && unit(hash(s, 3, 0, 0)) < sp.stall_chance {
                    let rr = sp.stall_at * radius;
                    pts.retain(|p| (p.0 * p.0 + p.1 * p.1).sqrt() >= rr);
                    let last = *pts.last().unwrap();
                    let (lx, ly) = (last.0 / rr.max(1.0), last.1 / rr.max(1.0));
                    let l = (lx * lx + ly * ly).sqrt();
                    let (mut cx, mut cy) = (lx / l, ly / l);
                    let reach = rr * (0.25 + 0.25 * unit(hash(s, 4, 0, 0)));
                    for _ in 0..16 {
                        cx += tx * reach / 16.0 / rr;
                        cy += ty * reach / 16.0 / rr;
                        let l = (cx * cx + cy * cy).sqrt();
                        cx /= l;
                        cy /= l;
                        pts.push((cx * rr, cy * rr));
                    }
                }
                let min = pts.iter().fold((f64::MAX, f64::MAX), |m, p| (m.0.min(p.0), m.1.min(p.1)));
                let max = pts.iter().fold((f64::MIN, f64::MIN), |m, p| (m.0.max(p.0), m.1.max(p.1)));
                let pad = sp.thickness_m / 2.0 * 1.2;
                let boxes = pts
                    .windows(2)
                    .map(|w| (w[0].0.min(w[1].0) - pad, w[0].0.max(w[1].0) + pad, w[0].1.min(w[1].1) - pad, w[0].1.max(w[1].1) + pad))
                    .collect();
                list.push(Slab { pts, boxes, half: sp.thickness_m / 2.0, seed: s, min, max });
            }
            slabs.push(Slabs {
                list,
                material: mats.id(&sp.material)?,
                chill: sp.chill_k,
                diamond: match &sp.diamond {
                    Some(d) => Some((mats.id(d)?, sp.diamond_chance)),
                    None => None,
                },
                diamond_below: if sp.stall_at > 0.0 { sp.stall_at * radius * 1.05 } else { radius * 0.8 },
            });
        }

        let piles = match &spec.piles {
            Some(p) => {
                let list = (0..p.count as i64)
                    .map(|k| {
                        let s = hash(seed, k, 0, 0x9_11E);
                        let (ux, uy) = random_dir(s, 0);
                        Pile {
                            ux,
                            uy,
                            chord: range(p.width, unit(hash(s, 1, 0, 0))) * std::f64::consts::PI,
                            height: range(p.height_m, unit(hash(s, 2, 0, 0))),
                            seed: s,
                        }
                    })
                    .collect();
                Some(Piles {
                    list,
                    at: p.at * radius,
                    material: mats.id(&p.material)?,
                    melt: match &p.melt {
                        Some(m) => Some(mats.id(m)?),
                        None => None,
                    },
                    melt_m: p.melt_m,
                })
            }
            None => None,
        };

        let layer_outer = |name: &str| layers.iter().find(|l| l.material == name).map(|l| l.outer * radius);
        let inner = match &spec.inner_core {
            Some(c) => Some(InnerCore {
                host: mats.id(&c.host)?,
                radius: layer_outer(&c.host).unwrap_or(radius),
                grain: c.grain_m,
                alt: mats.id(&c.alt)?,
                precious: mats.id(&c.precious)?,
                centre: c.precious_centre,
                edge: c.precious_edge,
                boundary: c.boundary_m,
                seed: seed ^ 0x1C0E,
            }),
            None => None,
        };
        let outer = match &spec.outer_core {
            Some(c) => Some(OuterCore {
                host: mats.id(&c.host)?,
                outer: layer_outer(&c.host).unwrap_or(radius),
                material: mats.id(&c.material)?,
                column: c.column_m,
                share: c.share,
                top: match &c.top_material {
                    Some(t) => Some(mats.id(t)?),
                    None => None,
                },
                top_m: c.top_m,
                seed: seed ^ 0x0C0E,
            }),
            None => None,
        };
        Ok(Deep { streaks, plumes, slabs, piles, inner, outer })
    }

    /// The deep-interior material at (fx, fy), given the base layer's `m`.
    pub fn apply(&self, fx: f64, fy: f64, r: f64, mut m: MatId) -> MatId {
        if let Some(c) = &self.inner {
            if let Some(x) = c.at(fx, fy, r, m) {
                return x;
            }
        }
        if let Some(c) = &self.outer {
            if let Some(x) = c.at(fx, fy, r, m) {
                return x;
            }
        }
        for s in &self.streaks {
            if let Some(x) = s.at(fx, fy, r, m) {
                m = x;
                break;
            }
        }
        if let Some(p) = &self.piles {
            if let Some(x) = p.at(fx, fy, r) {
                m = x;
            }
        }
        for p in &self.plumes {
            if let Some(x) = p.at(fx, fy) {
                m = x;
            }
        }
        for s in &self.slabs {
            if let Some(x) = s.at(fx, fy, r) {
                m = x;
            }
        }
        m
    }

    /// How much plumes warm and slabs chill the rock at (fx, fy), in kelvin.
    pub fn heat(&self, fx: f64, fy: f64) -> f64 {
        let mut dt = 0.0;
        for p in &self.plumes {
            dt += p.heat(fx, fy);
        }
        for s in &self.slabs {
            dt += s.heat(fx, fy);
        }
        dt
    }
}

/// Where the deep features meet the surface, for the surface features built
/// over them: each plume's direction and the radius of its head's top, and
/// each slab's trench direction with the way it dips. Uses the same seeded
/// draws as `Deep::build` (called with the same seed).
pub struct Anchors {
    pub plumes: Vec<(f64, f64, f64)>,
    pub slabs: Vec<(f64, f64, f64, f64)>,
}

pub fn anchors(spec: &DeepSpec, seed: u64, radius: f64) -> Anchors {
    let mut plumes = Vec::new();
    for (pi, p) in spec.plume.iter().enumerate() {
        for k in 0..p.count as i64 {
            let s = hash(seed, pi as i64, k, 0x9_1E);
            let (ux, uy) = random_dir(s, 0);
            let head = range(p.head_m, unit(hash(s, 2, 0, 0)));
            plumes.push((ux, uy, p.top * radius + head * 0.55));
        }
    }
    let mut slabs = Vec::new();
    for (si, sp) in spec.slab.iter().enumerate() {
        for k in 0..sp.count as i64 {
            let s = hash(seed, si as i64, k, 0x5_1AB);
            let (ux, uy) = random_dir(s, 0);
            let side = if unit(hash(s, 1, 0, 0)) < 0.5 { 1.0 } else { -1.0 };
            slabs.push((ux, uy, -uy * side, ux * side));
        }
    }
    Anchors { plumes, slabs }
}
