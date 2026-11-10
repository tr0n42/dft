//! Geology built once when a body loads (G1: strata, folds and faults).
//!
//! Strata are bands of rock that follow the surface, measured as depth below
//! the ground rather than distance from the centre. Where each band starts and
//! ends is worked out at load time for a few thousand directions around the
//! body and stored in a table, so a cell only has to look up two neighbouring
//! directions and blend them. Bands thicken, thin and pinch out along the
//! surface, and folds lift and sink the whole stack, so erosion at the surface
//! cuts across it and exposes deeper bands.
//!
//! Faults are straight breaks running down from the surface. Rock on one side
//! is shifted by the fault's throw, so the bands step across it.
//!
//! Everything here uses only add, multiply, divide and sqrt, so it gives the
//! same answer on every machine.

use crate::materials::{MatId, MaterialTable};
use crate::noise::{fbm, hash, unit};
use crate::spec::{Faults as FaultSpec, Strata as StrataSpec};

/// A direction's position around the body in [0, 4), increasing steadily as
/// the direction turns. Not a true angle, but cheap, exact and invertible.
pub fn pseudo_angle(ux: f64, uy: f64) -> f64 {
    if uy >= 0.0 {
        if ux >= 0.0 { uy / (ux + uy) } else { 1.0 - ux / (uy - ux) }
    } else if ux < 0.0 {
        2.0 - uy / (-ux - uy)
    } else {
        3.0 + ux / (ux - uy)
    }
}

/// The unit direction at pseudo-angle p.
fn direction_at(p: f64) -> (f64, f64) {
    let (x, y) = if p < 1.0 {
        (1.0 - p, p)
    } else if p < 2.0 {
        (1.0 - p, 2.0 - p)
    } else if p < 3.0 {
        (p - 3.0, 2.0 - p)
    } else {
        (p - 3.0, p - 4.0)
    };
    let l = (x * x + y * y).sqrt();
    (x / l, y / l)
}

#[derive(Clone, Copy, PartialEq)]
enum LensAt {
    Base,
    Middle,
    Top,
}

struct Lens {
    /// Centre and half-length, in direction-table units.
    centre: f64,
    half: f64,
    thickness: f64,
    at: LensAt,
    material: MatId,
    nugget: Option<(MatId, f64)>,
    seed: u64,
}

/// Directions per lens bin, so a cell only checks lenses near it.
const LENS_BIN: usize = 64;

pub struct Strata {
    /// Band materials from the top down.
    pub bands: Vec<MatId>,
    nominal: Vec<f64>,
    /// Per band: its lenses, and for each bin of directions, the lenses that
    /// reach into it.
    lenses: Vec<Vec<Lens>>,
    lens_bins: Vec<Vec<Vec<u32>>>,
    n_dir: usize,
    /// For each direction, the depth (m) of the bottom of each band; one row of
    /// `bands.len()` values per direction.
    bottoms: Vec<f32>,
    /// An older era under an unconformity, if any.
    old: Option<Box<Old>>,
}

struct Old {
    stack: Strata,
    /// Per direction: depth of the worn surface, and the gravel above it.
    cover: Vec<f32>,
    gravel: Vec<f32>,
    erode: f64,
    /// The old stack's full thickness: below it lies the basement, a smooth
    /// contact under the folds.
    floor: f64,
    conglomerate: Option<MatId>,
}

impl Strata {
    pub fn build(spec: &StrataSpec, mats: &MaterialTable, seed: u64, radius: f64) -> Result<Self, String> {
        if spec.band.is_empty() {
            return Err("strata need at least one band".into());
        }
        let kinds = spec
            .band
            .iter()
            .map(|b| Ok((mats.id(&b.material)?, b.thickness_m, b.weight.max(0.0))))
            .collect::<Result<Vec<_>, String>>()?;
        let total_w: f64 = kinds.iter().map(|k| k.2).sum();
        if total_w <= 0.0 {
            return Err("strata band weights must add up to more than zero".into());
        }

        // The sequence of bands, top down: picked by weight, never the same
        // rock twice in a row, until the stack is deep enough.
        let mut bands = Vec::new();
        let mut nominal = Vec::new();
        let mut depth = 0.0;
        let mut last = usize::MAX;
        let mut k = 0i64;
        while depth < spec.depth_m && bands.len() < 400 {
            let mut pick = last;
            let mut tries = 0;
            while pick == last && tries < 16 {
                let mut r = unit(hash(seed, k, tries, 0x57A7)) * total_w;
                pick = kinds.len() - 1;
                for (i, kd) in kinds.iter().enumerate() {
                    if r < kd.2 {
                        pick = i;
                        break;
                    }
                    r -= kd.2;
                }
                tries += 1;
                if kinds.len() == 1 {
                    break;
                }
            }
            let (m, [lo, hi], _) = kinds[pick];
            let t = lo + (hi - lo) * unit(hash(seed, k, 0, 0x7A1C));
            bands.push(m);
            nominal.push(t);
            depth += t;
            last = pick;
            k += 1;
        }

        // One row of band bottoms per direction, spaced about 4 m apart at the
        // surface (at least 1,024 directions, at most 65,536).
        let n_dir = ((2.0 * std::f64::consts::PI * radius / 4.0) as usize).clamp(1024, 65_536);
        let nb = bands.len();
        let mut bottoms = vec![0f32; n_dir * nb];
        let pinch_f = 1.0 / spec.pinch_wavelength_m.max(1.0);
        let fold_f = 1.0 / spec.fold_wavelength_m.max(1.0);
        let mut folds = vec![0f32; n_dir];
        for d in 0..n_dir {
            let (ux, uy) = direction_at(4.0 * (d as f64 + 0.5) / n_dir as f64);
            // The surface point in metres, so noise scales are real distances.
            let (sx, sy) = (ux * radius, uy * radius);
            let fold = spec.fold_amplitude_m * (2.0 * fbm(seed ^ 0xF01D, sx * fold_f, sy * fold_f, 2) - 1.0);
            folds[d] = fold as f32;
            let mut z = 0.0;
            for (i, t) in nominal.iter().enumerate() {
                // Thickness swells and thins along the surface; strong pinch
                // lets a band shrink to nothing, leaving a lens.
                let n = fbm(seed ^ (0xB0D0 + i as u64), sx * pinch_f, sy * pinch_f, 2);
                let scale = (1.0 + spec.pinch * (2.0 * n - 1.0) * 1.6).max(0.0);
                z += t * scale;
                // Older, deeper bands have been through more folding than the
                // young ones near the top.
                let age = (i as f64 + 1.0) / nb as f64;
                bottoms[d * nb + i] = (z + fold * (0.35 + 1.3 * age)) as f32;
            }
            // Keep the bottoms in order where strong folding would cross them.
            for i in 1..nb {
                let (lo, hi) = (bottoms[d * nb + i - 1], bottoms[d * nb + i]);
                if hi < lo {
                    bottoms[d * nb + i] = lo;
                }
            }
        }
        // Lenses: deposits inside particular bands (river gravels with gold
        // at a band's base, salt in the middle of a shale, and so on).
        let metres_per_dir = 2.0 * std::f64::consts::PI * radius / n_dir as f64;
        let n_bins = n_dir.div_ceil(LENS_BIN);
        let mut lenses: Vec<Vec<Lens>> = (0..nb).map(|_| Vec::new()).collect();
        let mut lens_bins: Vec<Vec<Vec<u32>>> = (0..nb).map(|_| Vec::new()).collect();
        for (li, l) in spec.lens.iter().enumerate() {
            let host = mats.id(&l.host)?;
            let material = mats.id(&l.material)?;
            let at = match l.position.as_str() {
                "base" => LensAt::Base,
                "middle" => LensAt::Middle,
                "top" => LensAt::Top,
                p => return Err(format!("lens position '{p}' must be base, middle or top")),
            };
            let nugget = match &l.nugget {
                Some(n) => {
                    let id = mats.id(n)?;
                    Some((id, l.nugget_chance * mats.geology.keep(id)))
                }
                None => None,
            };
            let count = (2.0 * std::f64::consts::PI * radius / l.spacing_m.max(1.0)).round() as i64;
            for (bi, &bm) in bands.iter().enumerate() {
                if bm != host {
                    continue;
                }
                for k in 0..count {
                    let ls = hash(seed, (li * 1000 + bi) as i64, k, 0x1E45);
                    let centre = unit(ls) * n_dir as f64;
                    let len = range(l.length_m, unit(hash(ls, 1, 0, 0)));
                    lenses[bi].push(Lens {
                        centre,
                        half: len / 2.0 / metres_per_dir,
                        thickness: l.thickness,
                        at,
                        material,
                        nugget,
                        seed: ls,
                    });
                }
            }
        }
        for bi in 0..nb {
            if lenses[bi].is_empty() {
                continue;
            }
            let mut bins = vec![Vec::new(); n_bins];
            for (i, l) in lenses[bi].iter().enumerate() {
                let lo = (l.centre - l.half).floor() as i64;
                let hi = (l.centre + l.half).ceil() as i64;
                let (b0, b1) = (lo.div_euclid(LENS_BIN as i64), hi.div_euclid(LENS_BIN as i64));
                for b in b0..=b1 {
                    let b = b.rem_euclid(n_bins as i64) as usize;
                    if bins[b].last() != Some(&(i as u32)) {
                        bins[b].push(i as u32);
                    }
                }
            }
            lens_bins[bi] = bins;
        }
        // The older era: its own stack (folded as its spec says), then the
        // depth table of the surface it was worn down to.
        let old = match &spec.old {
            None => None,
            Some(o) => {
                let stack = Strata::build(&o.stack, mats, seed ^ 0x01D5, radius)?;
                let cf = 1.0 / o.cover_wavelength_m.max(1.0);
                let mut cover = vec![0f32; n_dir];
                let mut gravel = vec![0f32; n_dir];
                for d in 0..n_dir {
                    let (ux, uy) = direction_at(4.0 * (d as f64 + 0.5) / n_dir as f64);
                    let (sx, sy) = (ux * radius, uy * radius);
                    // Old valleys and hills on the worn surface, broad and small.
                    let n = fbm(seed ^ 0xC07E, sx * cf, sy * cf, 3);
                    // The worn surface was folded along with the young layers
                    // laid on it, as much as the young band at that depth.
                    let c = range(o.cover_m, n);
                    let age = (c / spec.depth_m.max(1.0)).min(1.0);
                    cover[d] = (c + folds[d] as f64 * (0.35 + 1.3 * age)).max(0.0) as f32;
                    let g = fbm(seed ^ 0x62A7, sx * cf * 3.0, sy * cf * 3.0, 2);
                    gravel[d] = range(o.conglomerate_m, g) as f32;
                }
                let conglomerate = match &o.conglomerate {
                    Some(m) => Some(mats.id(m)?),
                    None => None,
                };
                Some(Box::new(Old { stack, cover, gravel, erode: o.erode_m, floor: o.stack.depth_m, conglomerate }))
            }
        };
        Ok(Strata { bands, nominal, lenses, lens_bins, n_dir, bottoms, old })
    }

    /// The band at `depth` metres below the surface in direction (ux, uy), or
    /// None if that depth lies above the stack (eroded away) or below it.
    pub fn band_at(&self, ux: f64, uy: f64, depth: f64, cx: i64, cy: i64) -> Option<MatId> {
        let p = pseudo_angle(ux, uy) * self.n_dir as f64 / 4.0 - 0.5;
        let p = if p < 0.0 { p + self.n_dir as f64 } else { p };
        let i0 = (p.floor() as usize) % self.n_dir;
        let i1 = (i0 + 1) % self.n_dir;
        let t = (p - p.floor()) as f32;
        let nb = self.bands.len();
        let (r0, r1) = (&self.bottoms[i0 * nb..i0 * nb + nb], &self.bottoms[i1 * nb..i1 * nb + nb]);
        let d = depth as f32;
        if let Some(o) = &self.old {
            let u = o.cover[i0] + (o.cover[i1] - o.cover[i0]) * t;
            if d >= u {
                // Below the worn surface: the old stack, its top cut away,
                // down to a smooth base (its deepest band fills any fold gap).
                let od = depth - u as f64 + o.erode;
                if od >= o.floor {
                    return None;
                }
                return o.stack.band_at(ux, uy, od, cx, cy).or_else(|| o.stack.bands.last().copied());
            }
            if let Some(c) = o.conglomerate {
                if d >= u - (o.gravel[i0] + (o.gravel[i1] - o.gravel[i0]) * t) {
                    return Some(c);
                }
            }
        }
        for i in 0..nb {
            let b = r0[i] + (r1[i] - r0[i]) * t;
            if d < b {
                // Where a fold has sunk the stack, the gap above it is
                // filled by the top band too.
                if self.lenses[i].is_empty() {
                    return Some(self.bands[i]);
                }
                let top = if i == 0 { b - self.nominal[0] as f32 } else { r0[i - 1] + (r1[i - 1] - r0[i - 1]) * t };
                return Some(self.lens_at(i, p, ((d - top) / (b - top).max(0.01)) as f64, cx, cy));
            }
        }
        // The young stack runs out above the worn surface: its last band
        // reaches down to it.
        if self.old.is_some() {
            return self.bands.last().copied();
        }
        None
    }

    /// Band i's material, or a lens in it, at direction-table position p and
    /// fraction tb of the way down through the band.
    fn lens_at(&self, i: usize, p: f64, tb: f64, cx: i64, cy: i64) -> MatId {
        let bin = (p as usize / LENS_BIN) % self.lens_bins[i].len().max(1);
        let n = self.n_dir as f64;
        for &li in &self.lens_bins[i][bin] {
            let l = &self.lenses[i][li as usize];
            let mut dp = (p - l.centre).abs();
            if dp > n / 2.0 {
                dp = n - dp;
            }
            if dp >= l.half {
                continue;
            }
            let q = dp / l.half;
            let prof = l.thickness * (1.0 - q * q);
            let inside = match l.at {
                LensAt::Base => tb > 1.0 - prof,
                LensAt::Top => tb < prof,
                LensAt::Middle => (tb - 0.5).abs() < prof / 2.0,
            };
            if inside {
                if let Some((nm, chance)) = l.nugget {
                    if unit(hash(l.seed, cx, cy, 0x6007)) < chance {
                        return nm;
                    }
                }
                return l.material;
            }
        }
        self.bands[i]
    }

    /// The deepest any band reaches, in metres below the surface.
    pub fn max_depth(&self) -> f64 {
        let young = self.bottoms.iter().fold(0f32, |a, &b| a.max(b)) as f64;
        match &self.old {
            Some(o) => young.max(o.cover.iter().fold(0f32, |a, &b| a.max(b)) as f64 + (o.floor - o.erode).max(0.0)),
            None => young,
        }
    }

    /// Every band material, the older era's included.
    pub fn all_bands(&self) -> Vec<MatId> {
        let mut v = self.bands.clone();
        if let Some(o) = &self.old {
            v.extend(o.stack.all_bands());
            v.extend(o.conglomerate);
        }
        v
    }

    pub fn directions(&self) -> usize {
        self.n_dir
    }
}

pub struct Fault {
    /// Where the fault meets the surface.
    px: f64,
    py: f64,
    /// Unit vector pointing down the fault plane.
    vx: f64,
    vy: f64,
    length: f64,
    throw: f64,
}

pub struct Faults {
    pub list: Vec<Fault>,
    /// For bins of pseudo-angle around the body, the faults that can reach
    /// into that wedge (so a cell checks only the faults near it).
    bins: Vec<Vec<u32>>,
    gouge: Option<MatId>,
    gouge_half: f64,
    /// Share of each fault's throw left standing as a cliff at the surface
    /// (0: worn flat, the old behaviour; 1: the full step).
    pub scarp: f64,
}

impl Faults {
    pub fn build(
        spec: &FaultSpec,
        mats: &MaterialTable,
        seed: u64,
        surface: impl Fn(f64, f64) -> f64,
    ) -> Result<Self, String> {
        let gouge = match &spec.gouge {
            Some(g) => Some(mats.id(g)?),
            None => None,
        };
        let mut list = Vec::new();
        for k in 0..spec.count as i64 {
            let p = 4.0 * unit(hash(seed, k, 1, 0xFA17));
            let (ux, uy) = direction_at(p);
            let rs = surface(ux, uy);
            // Down the radius, leaning to one side by `dip`.
            let side = if unit(hash(seed, k, 2, 0xFA17)) < 0.5 { 1.0 } else { -1.0 };
            let (tx, ty) = (-uy * side, ux * side);
            let (mut vx, mut vy) = (-ux + spec.dip * tx, -uy + spec.dip * ty);
            let l = (vx * vx + vy * vy).sqrt();
            vx /= l;
            vy /= l;
            let [lo, hi] = spec.throw_m;
            let mut throw = lo + (hi - lo) * unit(hash(seed, k, 3, 0xFA17));
            if unit(hash(seed, k, 4, 0xFA17)) < 0.5 {
                throw = -throw;
            }
            let length = spec.length_m * (0.6 + 0.8 * unit(hash(seed, k, 5, 0xFA17)));
            list.push(Fault { px: ux * rs, py: uy * rs, vx, vy, length, throw });
        }
        // A fault moves rock within its length of the break's top, so it can
        // only reach cells within about that distance; as a pseudo-angle
        // (which turns at most 1.3x as fast as a true angle), with margin.
        let n_bins = (list.len() * 8).clamp(16, 4096);
        let mut bins = vec![Vec::new(); n_bins];
        for (i, f) in list.iter().enumerate() {
            let rs = (f.px * f.px + f.py * f.py).sqrt();
            let reach = (1.5 * f.length + 4.0) / (rs - 1.5 * f.length).max(1.0);
            let pad = (1.3 * reach).min(2.0);
            let p = pseudo_angle(f.px / rs, f.py / rs);
            let per = n_bins as f64 / 4.0;
            let (lo, hi) = (((p - pad) * per).floor() as i64, ((p + pad) * per).floor() as i64);
            for b in lo..=hi {
                let b = b.rem_euclid(n_bins as i64) as usize;
                if bins[b].last() != Some(&(i as u32)) {
                    bins[b].push(i as u32);
                }
            }
        }
        Ok(Faults { list, bins, gouge, gouge_half: spec.gouge_width_m / 2.0, scarp: spec.scarp.clamp(0.0, 1.0) })
    }

    /// How far faults have moved the rock at (fx, fy) up or down, in metres of
    /// depth (positive: the cell now holds rock that formed shallower), and
    /// whether the cell sits in a fault's crushed zone.
    /// How deep below the surface any fault reaches, and its largest throw.
    pub fn reach(&self) -> (f64, f64) {
        self.list.iter().fold((0.0, 0.0), |a, f| (f64::max(a.0, f.length + 2.0), f64::max(a.1, f.throw.abs())))
    }

    /// The offset to apply to the layers at (fx, fy). Where part of the throw
    /// still stands as a cliff, the surface moved with the block, so the
    /// layers only shift by the part that has been worn away.
    pub fn shift(&self, fx: f64, fy: f64) -> (f64, Option<MatId>) {
        let (dz, gouge) = self.raw(fx, fy);
        if self.scarp > 0.0 { (dz * (1.0 - self.scarp), gouge) } else { (dz, gouge) }
    }

    /// How far the surface has dropped at (fx, fy), a point on the ground.
    pub fn surface_drop(&self, fx: f64, fy: f64) -> f64 {
        if self.scarp <= 0.0 {
            return 0.0;
        }
        self.raw(fx, fy).0 * self.scarp
    }

    fn raw(&self, fx: f64, fy: f64) -> (f64, Option<MatId>) {
        let mut dz = 0.0;
        let mut gouge = None;
        let r = (fx * fx + fy * fy).sqrt().max(1e-9);
        let n = self.bins.len();
        let b = ((pseudo_angle(fx / r, fy / r) * n as f64 / 4.0) as usize).min(n - 1);
        for &i in &self.bins[b] {
            let f = &self.list[i as usize];
            let (rx, ry) = (fx - f.px, fy - f.py);
            let along = rx * f.vx + ry * f.vy;
            if along < -2.0 || along > f.length {
                continue;
            }
            let across = f.vx * ry - f.vy * rx;
            let reach = f.length;
            if across.abs() > reach {
                continue;
            }
            // The offset fades out toward the fault's deep tip and far from it.
            let a = along.max(0.0) / f.length;
            let tip = 1.0 - a * a;
            if across > 0.0 {
                dz += f.throw * tip * (1.0 - across / reach);
            }
            if self.gouge.is_some() && across.abs() < self.gouge_half * tip.max(0.3) && along >= 0.0 {
                gouge = self.gouge;
            }
        }
        (dz, gouge)
    }
}

// ---------------------------------------------------------------------------
// G2: intrusions and the ore around them.
// ---------------------------------------------------------------------------

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RulesFile {
    #[serde(default)]
    vein_gangue: Option<String>,
    #[serde(default)]
    skarn_hosts: Vec<String>,
    #[serde(default)]
    cumulate_nugget: Option<String>,
    #[serde(default)]
    cumulate_nugget_chance: f64,
    #[serde(default)]
    hydrothermal: Vec<ZoneRow>,
    #[serde(default)]
    contact: Vec<ContactRow>,
    #[serde(default)]
    skarn: Vec<PickRow>,
    #[serde(default)]
    cumulate: Vec<PickRow>,
    #[serde(default)]
    accessory: Vec<AccessoryRow>,
    /// Per material: the share of the places it would form where it does
    /// (1 = everywhere it can; 0.01 = one in a hundred, the rest gangue or
    /// host rock). Tunes how much of each ore a body holds overall.
    #[serde(default)]
    scarcity: std::collections::BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Deserialize)]
struct AccessoryRow {
    material: String,
    host: Vec<String>,
    grid_m: f64,
    chance: f64,
    radius_m: [f64; 2],
    #[serde(default = "any_depth")]
    depth_m: [f64; 2],
}

fn any_depth() -> [f64; 2] {
    [0.0, f64::INFINITY]
}

/// Pods of a mineral scattered through a host rock (`[[geology.accessory]]`).
#[derive(Debug, Clone)]
pub struct Accessory {
    pub material: MatId,
    pub host: Vec<MatId>,
    pub grid: f64,
    pub chance: f64,
    pub radius: [f64; 2],
    pub depth: [f64; 2],
}

impl Accessory {
    /// Whether cell (fx, fy), `depth` metres under the surface, lies in one
    /// of this mineral's pods. `seed` is the body's (and the row's).
    pub fn hit(&self, seed: u64, fx: f64, fy: f64, depth: f64) -> bool {
        if depth < self.depth[0] || depth > self.depth[1] {
            return false;
        }
        let (gx, gy) = ((fx / self.grid).floor() as i64, (fy / self.grid).floor() as i64);
        let h = hash(seed, gx, gy, 0xACC);
        if unit(h) >= self.chance {
            return false;
        }
        // A pod a quarter of the square across at most, kept inside it.
        let rad = range(self.radius, unit(hash(h, 1, 0, 0))).min(self.grid * 0.25);
        let span = self.grid - 2.0 * rad * 1.3;
        let cx = (gx as f64) * self.grid + rad * 1.3 + span * unit(hash(h, 2, 0, 0));
        let cy = (gy as f64) * self.grid + rad * 1.3 + span * unit(hash(h, 3, 0, 0));
        let (dx, dy) = (fx - cx, fy - cy);
        let d2 = dx * dx + dy * dy;
        if d2 > rad * rad * 1.69 {
            return false;
        }
        // Lumpy, not round.
        let r = rad * (0.7 + 0.6 * fbm(h, fx / rad.max(1.0), fy / rad.max(1.0), 2));
        d2 <= r * r
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ZoneRow {
    material: String,
    kind: String,
    zone: [f64; 2],
    #[serde(default = "one")]
    weight: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct ContactRow {
    from: String,
    to: String,
}

#[derive(Debug, Clone, Deserialize)]
struct PickRow {
    material: String,
    #[serde(default = "one")]
    weight: f64,
}

fn one() -> f64 {
    1.0
}

/// Pluton chemistry, which decides the ore it makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Felsic,
    Mafic,
}

fn kind_of(s: &str) -> Result<Kind, String> {
    match s {
        "felsic" => Ok(Kind::Felsic),
        "mafic" => Ok(Kind::Mafic),
        _ => Err(format!("unknown pluton kind '{s}' (felsic or mafic)")),
    }
}

/// The geology rules with materials resolved to ids.
#[derive(Debug, Clone, Default)]
pub struct Rules {
    pub gangue: Option<MatId>,
    pub skarn_hosts: Vec<MatId>,
    /// (material, kind, zone start, zone end, weight)
    pub hydrothermal: Vec<(MatId, Kind, f64, f64, f64)>,
    pub contact: Vec<(MatId, MatId)>,
    pub skarn: Vec<(MatId, f64)>,
    pub cumulate: Vec<(MatId, f64)>,
    pub nugget: Option<MatId>,
    pub nugget_chance: f64,
    pub accessory: Vec<Accessory>,
    /// Per material id: how much of what could form does (see `scarcity`).
    pub scarcity: Vec<f64>,
}

impl Rules {
    pub fn resolve(f: &RulesFile, mats: &MaterialTable) -> Result<Rules, String> {
        let picks = |rows: &Vec<PickRow>| rows.iter().map(|r| Ok((mats.id(&r.material)?, r.weight))).collect::<Result<Vec<_>, String>>();
        Ok(Rules {
            gangue: match &f.vein_gangue {
                Some(g) => Some(mats.id(g)?),
                None => None,
            },
            skarn_hosts: f.skarn_hosts.iter().map(|n| mats.id(n)).collect::<Result<_, _>>()?,
            hydrothermal: f
                .hydrothermal
                .iter()
                .map(|r| Ok((mats.id(&r.material)?, kind_of(&r.kind)?, r.zone[0], r.zone[1], r.weight)))
                .collect::<Result<_, String>>()?,
            contact: f.contact.iter().map(|r| Ok((mats.id(&r.from)?, mats.id(&r.to)?))).collect::<Result<_, String>>()?,
            skarn: picks(&f.skarn)?,
            cumulate: picks(&f.cumulate)?,
            nugget: match &f.cumulate_nugget {
                Some(n) => Some(mats.id(n)?),
                None => None,
            },
            nugget_chance: f.cumulate_nugget_chance,
            scarcity: {
                let mut v = vec![1.0; mats.list.len()];
                for (n, k) in &f.scarcity {
                    v[mats.id(n)? as usize] = k.max(0.0);
                }
                v
            },
            accessory: f
                .accessory
                .iter()
                .map(|r| {
                    Ok(Accessory {
                        material: mats.id(&r.material)?,
                        host: r.host.iter().map(|n| mats.id(n)).collect::<Result<_, _>>()?,
                        grid: r.grid_m.max(4.0),
                        chance: r.chance,
                        radius: r.radius_m,
                        depth: r.depth_m,
                    })
                })
                .collect::<Result<_, String>>()?,
        })
        .map(|mut r: Rules| {
            for a in r.accessory.iter_mut() {
                a.chance = (a.chance * r.scarcity[a.material as usize]).min(0.5);
            }
            let x: Vec<f64> = r.scarcity.iter().map(|k| k.max(1.0)).collect();
            for h in r.hydrothermal.iter_mut() {
                h.4 *= x[h.0 as usize];
            }
            for c in r.cumulate.iter_mut().chain(r.skarn.iter_mut()) {
                c.1 *= x[c.0 as usize];
            }
            for k in r.scarcity.iter_mut() {
                *k = k.min(1.0);
            }
            r
        })
    }

    /// How much of what could form of material m does (1 if not set).
    pub fn keep(&self, m: MatId) -> f64 {
        self.scarcity.get(m as usize).copied().unwrap_or(1.0).min(1.0)
    }

    /// Over 1, scarcity means more than every place it could form: it is
    /// picked that much more often over other ores (and its pods are that
    /// much more common).
    pub fn extra(&self, m: MatId) -> f64 {
        self.scarcity.get(m as usize).copied().unwrap_or(1.0).max(1.0)
    }

    fn baked(&self, m: MatId) -> MatId {
        self.contact.iter().find(|c| c.0 == m).map_or(m, |c| c.1)
    }
}

/// A weighted pick from (material, weight) pairs with a uniform number in [0, 1).
pub fn pick(list: &[(MatId, f64)], u: f64) -> Option<MatId> {
    let total: f64 = list.iter().map(|p| p.1).sum();
    if total <= 0.0 {
        return None;
    }
    let mut r = u * total;
    for p in list {
        if r < p.1 {
            return Some(p.0);
        }
        r -= p.1;
    }
    list.last().map(|p| p.0)
}

struct Pluton {
    cx: f64,
    cy: f64,
    radius: f64,
    /// Unit vector from the body's centre out through the pluton.
    ox: f64,
    oy: f64,
    material: MatId,
    kind: Kind,
    halo: f64,
    aureole: f64,
    skarn: f64,
    seed: u64,
}

/// A straight sheet (dike) or vein from a pluton, with a gentle wander.
struct Segment {
    ax: f64,
    ay: f64,
    dx: f64,
    dy: f64,
    length: f64,
    half_width: f64,
    pluton: u32,
    /// Fixed material for a dike; None for a vein (zoned ore).
    material: Option<MatId>,
    seed: u64,
}

const BUCKET: f64 = 256.0;

pub struct Intrusions {
    plutons: Vec<Pluton>,
    segments: Vec<Segment>,
    /// Bucket size: 256 m, or larger on big bodies so the lookup stays small.
    bucket: f64,
    /// Occupied buckets only, so huge bodies cost nothing extra. Items are
    /// stored in creation order, so overlaps always resolve the same way.
    buckets: HashMap<(i64, i64), (Vec<u32>, Vec<u32>)>,
}

/// A random unit vector, without trig.
pub fn random_dir(seed: u64, i: i64) -> (f64, f64) {
    let mut k = 0;
    loop {
        let x = unit(hash(seed, i, k, 0xD1)) * 2.0 - 1.0;
        let y = unit(hash(seed, i, k, 0xD2)) * 2.0 - 1.0;
        let d = x * x + y * y;
        if d > 1e-4 && d <= 1.0 {
            let l = d.sqrt();
            return (x / l, y / l);
        }
        k += 1;
    }
}

pub fn range(r: [f64; 2], u: f64) -> f64 {
    r[0] + (r[1] - r[0]) * u
}

impl Intrusions {
    pub fn build(
        specs: &[crate::spec::Intrusion],
        mats: &MaterialTable,
        seed: u64,
        radius: f64,
        surface: impl Fn(f64, f64) -> f64,
    ) -> Result<Self, String> {
        let mut plutons = Vec::new();
        let mut segments = Vec::new();
        for (si, s) in specs.iter().enumerate() {
            let material = mats.id(&s.material)?;
            let kind = kind_of(&s.kind)?;
            let dike_mat = match &s.dike_material {
                Some(d) => mats.id(d)?,
                None => material,
            };
            for k in 0..s.count as i64 {
                let ps = hash(seed, si as i64, k, 0x9107);
                let (ox, oy) = random_dir(ps, 0);
                let rad = range(s.radius_m, unit(hash(ps, 1, 0, 0)));
                let top = range(s.top_depth_m, unit(hash(ps, 2, 0, 0)));
                let rs = surface(ox, oy);
                let dist = (rs - top - rad).clamp(rad, radius * 0.95);
                let (cx, cy) = (ox * dist, oy * dist);
                let pi = plutons.len() as u32;
                plutons.push(Pluton {
                    cx,
                    cy,
                    radius: rad,
                    ox,
                    oy,
                    material,
                    kind,
                    halo: s.halo_m,
                    aureole: s.aureole_m,
                    skarn: s.skarn_m,
                    seed: ps,
                });
                // Dikes: from the pluton's roof toward the surface, some all the way.
                let nd = range([s.dikes[0] as f64, s.dikes[1] as f64 + 0.999], unit(hash(ps, 3, 0, 0))) as i64;
                for d in 0..nd {
                    let ds = hash(ps, d, 4, 0xD14E);
                    let lean = 0.5 * (2.0 * unit(hash(ds, 1, 0, 0)) - 1.0);
                    let (tx, ty) = (-oy, ox);
                    let start_off = rad * 0.7 * (2.0 * unit(hash(ds, 2, 0, 0)) - 1.0);
                    let (ax, ay) = (cx + tx * start_off, cy + ty * start_off);
                    let (mut dx, mut dy) = (ox + tx * lean, oy + ty * lean);
                    let l = (dx * dx + dy * dy).sqrt();
                    dx /= l;
                    dy /= l;
                    let reach = (top + rad) * if unit(hash(ds, 3, 0, 0)) < 0.5 { 1.3 } else { 0.4 + 0.5 * unit(hash(ds, 4, 0, 0)) };
                    segments.push(Segment {
                        ax,
                        ay,
                        dx,
                        dy,
                        length: reach,
                        half_width: range(s.dike_width_m, unit(hash(ds, 5, 0, 0))) / 2.0,
                        pluton: pi,
                        material: Some(dike_mat),
                        seed: ds,
                    });
                }
                // Veins: fanning out of the upper side of the pluton.
                let nv = range([s.veins[0] as f64, s.veins[1] as f64 + 0.999], unit(hash(ps, 5, 0, 0))) as i64;
                for v in 0..nv {
                    let vs = hash(ps, v, 6, 0x7E19);
                    let (mut rx, mut ry) = random_dir(vs, 0);
                    if rx * ox + ry * oy < 0.2 {
                        // Reflect into the half facing the surface.
                        let dot = rx * ox + ry * oy;
                        rx -= 2.0 * dot * ox;
                        ry -= 2.0 * dot * oy;
                    }
                    let (ax, ay) = (cx + rx * rad * 0.97, cy + ry * rad * 0.97);
                    let (mut dx, mut dy) = (rx + ox * 0.9, ry + oy * 0.9);
                    let l = (dx * dx + dy * dy).sqrt();
                    dx /= l;
                    dy /= l;
                    segments.push(Segment {
                        ax,
                        ay,
                        dx,
                        dy,
                        length: s.halo_m * (0.5 + 0.5 * unit(hash(vs, 1, 0, 0))),
                        half_width: range(s.vein_width_m, unit(hash(vs, 2, 0, 0))) / 2.0,
                        pluton: pi,
                        material: None,
                        seed: vs,
                    });
                }
            }
        }
        let mut buckets: HashMap<(i64, i64), (Vec<u32>, Vec<u32>)> = HashMap::new();
        let bucket = (radius / 400.0).clamp(BUCKET, 8192.0);
        let cell = |v: f64| (v / bucket).floor() as i64;
        for (i, p) in plutons.iter().enumerate() {
            let reach = p.radius * 1.3 + p.aureole;
            for by in cell(p.cy - reach)..=cell(p.cy + reach) {
                for bx in cell(p.cx - reach)..=cell(p.cx + reach) {
                    buckets.entry((bx, by)).or_default().0.push(i as u32);
                }
            }
        }
        for (i, s) in segments.iter().enumerate() {
            let (bx_, by_) = (s.ax + s.dx * s.length, s.ay + s.dy * s.length);
            let pad = s.half_width + 0.08 * s.length + 4.0;
            for by in cell(s.ay.min(by_) - pad)..=cell(s.ay.max(by_) + pad) {
                for bx in cell(s.ax.min(bx_) - pad)..=cell(s.ax.max(bx_) + pad) {
                    buckets.entry((bx, by)).or_default().1.push(i as u32);
                }
            }
        }
        Ok(Intrusions { plutons, segments, bucket, buckets })
    }

    /// Apply intrusions to the material `m` already chosen for cell (fx, fy).
    pub fn apply(&self, rules: &Rules, fx: f64, fy: f64, mut m: MatId) -> MatId {
        let Some((ps, ss)) = self.buckets.get(&((fx / self.bucket).floor() as i64, (fy / self.bucket).floor() as i64)) else {
            return m;
        };
        let host = m;
        for &i in ps {
            let p = &self.plutons[i as usize];
            let (dx, dy) = (fx - p.cx, fy - p.cy);
            let d = (dx * dx + dy * dy).sqrt();
            if d > p.radius * 1.3 + p.aureole {
                continue;
            }
            // A lumpy outline, flatter on top like a real pluton's roof.
            let wobble = 0.8 + 0.4 * fbm(p.seed, fx / (p.radius * 0.6), fy / (p.radius * 0.6), 3);
            let up = (dx * p.ox + dy * p.oy) / d.max(1e-9);
            let roof = if up > 0.0 { 1.0 - 0.25 * up * up } else { 1.0 };
            let r = p.radius * wobble * roof;
            if d <= r {
                m = p.material;
                // Mafic plutons: dense crystals settle into stripes at the base.
                if p.kind == Kind::Mafic && !rules.cumulate.is_empty() {
                    let below = -(dx * p.ox + dy * p.oy) / p.radius;
                    if below > 0.5 {
                        let stripe = ((below * p.radius) / 3.0).floor() as i64;
                        if unit(hash(p.seed, stripe, 0, 0xC0)) < 0.3 {
                            if let Some(c) = pick(&rules.cumulate, unit(hash(p.seed, stripe, 1, 0xC0))) {
                                if unit(hash(p.seed, stripe, 2, 0xC0)) < rules.keep(c) {
                                    m = c;
                                }
                            }
                            if let Some(n) = rules.nugget {
                                // Platinum-group nuggets in the chromite stripes.
                                if unit(hash(p.seed, fx as i64, fy as i64, 0xC1)) < rules.nugget_chance * rules.keep(n) {
                                    m = n;
                                }
                            }
                        }
                    }
                }
            } else if d - r < p.aureole {
                // Baked rock around the pluton; skarn right against carbonates.
                if d - r < p.skarn && rules.skarn_hosts.contains(&host) {
                    let patch = fbm(p.seed ^ 0x5CA, fx / 6.0, fy / 6.0, 2);
                    match pick(&rules.skarn, patch) {
                        Some(k) if unit(hash(p.seed, (fx / 8.0).floor() as i64, (fy / 8.0).floor() as i64, 0x5CB)) < rules.keep(k) => m = k,
                        _ => m = rules.baked(m),
                    }
                } else {
                    m = rules.baked(m);
                }
            }
        }
        for &i in ss {
            let s = &self.segments[i as usize];
            let (rx, ry) = (fx - s.ax, fy - s.ay);
            let along = rx * s.dx + ry * s.dy;
            if along < 0.0 || along > s.length {
                continue;
            }
            // Dikes run nearly straight; veins wander along their cracks.
            let amp = if s.material.is_some() { 0.012 } else { 0.06 };
            let wander = amp * s.length * (2.0 * fbm(s.seed, along / 45.0, 0.5, 2) - 1.0);
            let across = (s.dx * ry - s.dy * rx) - wander;
            let t = along / s.length;
            let hw = s.half_width * (1.0 - t * t * t * t).max(0.3);
            if across.abs() > hw {
                continue;
            }
            match s.material {
                Some(dm) => m = dm,
                None => {
                    // Mostly quartz, with ore in patches; which ore depends on
                    // how far along the vein (the zone), in stretches of about a
                    // sixth of the halo.
                    let p = &self.plutons[s.pluton as usize];
                    let zone = along / p.halo;
                    let ore = fbm(s.seed ^ 0x0E, fx / 4.0, fy / 4.0, 2) > 0.45;
                    if ore {
                        let stretch = (zone * 6.0).floor() as i64;
                        let cands: Vec<(MatId, f64)> = rules
                            .hydrothermal
                            .iter()
                            .filter(|h| h.1 == p.kind && zone >= h.2 && zone <= h.3)
                            .map(|h| (h.0, h.4))
                            .collect();
                        let keep = |o: MatId| unit(hash(s.seed, stretch, 1, 0x2F)) < rules.keep(o);
                        if let Some(o) = pick(&cands, unit(hash(s.seed, stretch, 0, 0x2E))).filter(|&o| keep(o)) {
                            m = o;
                        } else if let Some(g) = rules.gangue {
                            m = g;
                        }
                    } else if let Some(g) = rules.gangue {
                        m = g;
                    }
                }
            }
        }
        m
    }

    pub fn counts(&self) -> (usize, usize) {
        (self.plutons.len(), self.segments.len())
    }
}

// ---------------------------------------------------------------------------
// G3: rubble piles, caves where they form, ice caps.
// ---------------------------------------------------------------------------

/// A rubble-pile body: packed boulders of different rock, with gaps between
/// them. Boulders are cells of a jittered grid (a Voronoi pattern), their
/// outlines bent by a gentle warp so they look broken rather than tiled.
pub struct Rubble {
    size: f64,
    gap: f64,
    gap_material: MatId,
    voids: f64,
    /// Rocks allowed at each boulder size: big, medium, small.
    rocks: [Vec<(MatId, f64)>; 3],
    r_min: f64,
    r_max: f64,
    seed: u64,
}

impl Rubble {
    pub fn build(s: &crate::spec::Rubble, mats: &MaterialTable, seed: u64) -> Result<Self, String> {
        Ok(Rubble {
            size: s.boulder_m,
            gap: s.gap_m,
            gap_material: mats.id(&s.gap_material)?,
            voids: s.voids,
            rocks: {
                let mut lv: [Vec<(MatId, f64)>; 3] = Default::default();
                for r in &s.rock {
                    let id = mats.id(&r.material)?;
                    let from = match r.largest.as_deref() {
                        None | Some("big") => 0,
                        Some("medium") => 1,
                        Some("small") => 2,
                        Some(o) => return Err(format!("rubble rock size '{o}' must be big, medium or small")),
                    };
                    for (i, l) in lv.iter_mut().enumerate() {
                        if i >= from {
                            l.push((id, r.weight));
                        }
                    }
                }
                lv
            },
            r_min: s.r_min,
            r_max: s.r_max,
            seed,
        })
    }

    /// The nearest site of a jittered grid with cells `s` wide: its cell, and
    /// the half-gap to the next nearest (how far inside its boulder we are).
    fn site(&self, wx: f64, wy: f64, s: f64, salt: u64) -> ((i64, i64), f64) {
        let (gx, gy) = ((wx / s).floor() as i64, (wy / s).floor() as i64);
        let (mut d1, mut d2, mut best) = (f64::MAX, f64::MAX, (0i64, 0i64));
        for j in -1..=1 {
            for i in -1..=1 {
                let (cx, cy) = (gx + i, gy + j);
                let h = hash(self.seed ^ salt, cx, cy, 0x50);
                let sx = (cx as f64 + 0.1 + 0.8 * unit(h)) * s;
                let sy = (cy as f64 + 0.1 + 0.8 * unit(hash(h, 1, 0, 0))) * s;
                let d = (wx - sx) * (wx - sx) + (wy - sy) * (wy - sy);
                if d < d1 {
                    d2 = d1;
                    d1 = d;
                    best = (cx, cy);
                } else if d < d2 {
                    d2 = d;
                }
            }
        }
        (best, (d2.sqrt() - d1.sqrt()) / 2.0)
    }

    /// The boulder material at (fx, fy), with f the fraction of the radius.
    /// Three sizes nest inside each other: a big boulder fills its whole cell;
    /// where there is none, medium boulders fill the space; where those are
    /// missing too, small rubble and dust. Gaps shrink with boulder size.
    pub fn at(&self, fx: f64, fy: f64, f: f64) -> Option<MatId> {
        if f < self.r_min || f > self.r_max {
            return None;
        }
        let s = self.size;
        let wx = fx + 0.25 * s * (2.0 * fbm(self.seed ^ 0x3A, fx / (s * 1.5), fy / (s * 1.5), 2) - 1.0);
        let wy = fy + 0.25 * s * (2.0 * fbm(self.seed ^ 0x3B, fx / (s * 1.5), fy / (s * 1.5), 2) - 1.0);
        for (level, (scale, keep, salt)) in [(2.6, 0.4, 0x100u64), (1.0, 0.55, 0x200), (0.35, 1.0, 0x300)].iter().enumerate() {
            let (cell, inside) = self.site(wx, wy, s * scale, *salt);
            let h = hash(self.seed ^ salt, cell.0, cell.1, 0x51);
            if unit(h) >= *keep {
                continue;
            }
            if inside < self.gap * scale.sqrt() {
                return Some(self.gap_material);
            }
            if level < 2 && unit(hash(h, 3, 0, 0)) < self.voids {
                return Some(crate::materials::VACUUM);
            }
            return pick(&self.rocks[level], unit(hash(h, 2, 0, 0)));
        }
        Some(self.gap_material)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CaveKind {
    /// Dissolved out of carbonate rock: passages that run along the layers.
    Karst,
    /// Drained lava channels inside lava flows: long, low, sinuous tubes.
    Tube,
}

pub struct Cave {
    kind: CaveKind,
    hosts: Vec<MatId>,
    scale: f64,
    threshold: f64,
    max_depth: f64,
    seed: u64,
}

pub struct Caves {
    list: Vec<Cave>,
    /// Metres per unit of pseudo-angle at the surface.
    arc_per_p: f64,
}

impl Caves {
    pub fn build(specs: &[crate::spec::CaveKind], mats: &MaterialTable, seed: u64, radius: f64) -> Result<Self, String> {
        let list = specs
            .iter()
            .enumerate()
            .map(|(i, c)| {
                Ok(Cave {
                    kind: match c.kind.as_str() {
                        "karst" => CaveKind::Karst,
                        "tube" => CaveKind::Tube,
                        k => return Err(format!("unknown cave kind '{k}' (karst or tube)")),
                    },
                    hosts: c.host.iter().map(|n| mats.id(n)).collect::<Result<_, _>>()?,
                    scale: c.scale_m,
                    threshold: c.threshold,
                    max_depth: c.max_depth_m,
                    seed: hash(seed, i as i64, 0, 0xCA4E),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Caves { list, arc_per_p: 2.0 * std::f64::consts::PI * radius / 4.0 })
    }

    /// True if the cell (material m, direction ux/uy, depth below the
    /// surface) is open cave.
    pub fn open(&self, m: MatId, ux: f64, uy: f64, depth: f64) -> bool {
        let arc = pseudo_angle(ux, uy) * self.arc_per_p;
        for c in &self.list {
            if depth > c.max_depth || !c.hosts.contains(&m) {
                continue;
            }
            // Passages follow the layers: noise stretched along the surface.
            let squash = match c.kind {
                CaveKind::Karst => 3.0,
                CaveKind::Tube => 6.0,
            };
            let mask = fbm(c.seed ^ 0x3A5, arc / (c.scale * 6.0), depth * squash / (c.scale * 6.0), 2);
            if mask < 0.42 {
                continue;
            }
            let n = fbm(c.seed, arc / c.scale, depth * squash / c.scale, 3);
            let ridge = 1.0 - (2.0 * n - 1.0).abs();
            let fade = ((mask - 0.42) / 0.12).min(1.0);
            if ridge > c.threshold + (1.0 - c.threshold) * (1.0 - fade) {
                return true;
            }
        }
        false
    }
}

/// Ice caps at the poles (top and bottom of the cross-section), thickening
/// toward each pole.
pub struct IceCaps {
    material: MatId,
    thickness: f64,
    from: f64,
}

impl IceCaps {
    pub fn build(s: &crate::spec::IceCaps, mats: &MaterialTable) -> Result<Self, String> {
        Ok(IceCaps { material: mats.id(&s.material)?, thickness: s.thickness_m, from: s.from_latitude })
    }

    /// The cap's material if (fx, fy) at `depth` below the surface lies in it.
    pub fn at(&self, fx: f64, fy: f64, r: f64, depth: f64) -> Option<MatId> {
        let s = (fy / r).abs();
        if s <= self.from {
            return None;
        }
        let k = (s - self.from) / (1.0 - self.from);
        let wobble = 0.85 + 0.3 * fbm(0x1CE, fx / 60.0, fy / 60.0, 2);
        if depth < self.thickness * k.sqrt() * wobble {
            Some(self.material)
        } else {
            None
        }
    }
}
