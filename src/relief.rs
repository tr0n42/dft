//! G5: the surface's shape, built from the body's geology.
//!
//! At load time the surface becomes a table around the body: for about one
//! direction every 4 m, how far the ground sits above or below the plain
//! noise surface. Volcanoes, rifts, impact basins, mountain belts, plateaus,
//! fault cliffs, crater rims and a lowland hemisphere each add to it.
//!
//! Each feature records two things per direction:
//! - the **structural** offset: ground that moved with the rock beneath it
//!   (an uplifted range, a dropped hemisphere, a fault block). Layers below
//!   follow it.
//! - a **pile**: rock laid on top (a volcano's lava flows, an ejecta blanket,
//!   lava filling a basin, sediment in a rift). Its material comes from the
//!   feature, in bands by height for volcanoes.
//!
//! Excavations (basins, rifts) lower the surface without moving the layers,
//! so they cut down into them.
//!
//! Each body type lists its features with a chance, a count range and size
//! ranges, so every seed draws its own set and its own character.

use crate::geology::{pick, pseudo_angle, random_dir, range};
use crate::materials::{MatId, MaterialTable};
use crate::noise::{fbm, hash, unit};
use crate::spec::Relief as ReliefSpec;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Volcano,
    Rift,
    Basin,
    Range,
    Plateau,
    Lowland,
    Ejecta,
    Trench,
    Boulder,
}

/// One feature drawn for this seed.
#[derive(Clone, Debug)]
pub struct Feature {
    pub kind: Kind,
    pub ux: f64,
    pub uy: f64,
    pub width: f64,
    pub height: f64,
    /// Volcano: share of the width collapsed into a caldera; basin: rim height.
    pub extra: f64,
    pub steep: bool,
    /// Pile materials (volcano flows in bands; one material for fills).
    pub materials: Vec<(MatId, f64)>,
    pub band_m: [f64; 2],
    pub fill_m: f64,
    pub seed: u64,
    /// Volcanoes: still molten underneath, what the chamber froze to if not,
    /// and (hotspots) the radius of the plume head feeding it, 0 if none.
    pub active: bool,
    pub chamber: MatId,
    pub feed_r: f64,
    /// What rises through it, how hot that is kept, and what dikes freeze to
    /// (None: the body's magma, its usual heat, dolerite).
    pub melt: Option<(MatId, f64, MatId)>,
    /// Liquid fills: the lid's material and thickness.
    pub crust: Option<(MatId, f64)>,
    /// Basins and lowlands: the rock under the fill and how deep it goes.
    pub floor: Option<(MatId, f64)>,
    /// Lowlands: depth of a level sea below the mean radius (0: a plain fill).
    pub sea: f64,
}

/// One straight piece of a volcano's plumbing (conduit, dike, sill, feed).
struct Part {
    a: (f64, f64),
    b: (f64, f64),
    half: f64,
    mat: MatId,
    bbox: [f64; 4],
}

/// What lies under one volcano: a chamber and the pipes and sheets off it.
struct Plumb {
    bbox: [f64; 4],
    /// Chamber centre, its radial and tangential directions and half-sizes.
    c: (f64, f64),
    u: (f64, f64),
    radial: f64,
    tangential: f64,
    chamber: MatId,
    parts: Vec<Part>,
    active: bool,
    magma: MatId,
    /// How hot live melt is kept.
    hold_k: f64,
    seed: u64,
}

fn grow(b: &mut [f64; 4], x: f64, y: f64, m: f64) {
    b[0] = b[0].min(x - m);
    b[1] = b[1].min(y - m);
    b[2] = b[2].max(x + m);
    b[3] = b[3].max(y + m);
}

fn inside(b: &[f64; 4], x: f64, y: f64) -> bool {
    x >= b[0] && x <= b[2] && y >= b[1] && y <= b[3]
}

/// Distance from (x, y) to the segment a-b.
fn seg_dist(a: (f64, f64), b: (f64, f64), x: f64, y: f64) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((x - a.0) * dx + (y - a.1) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    let (px, py) = (a.0 + dx * t - x, a.1 + dy * t - y);
    (px * px + py * py).sqrt()
}

fn unit_vec(x: f64, y: f64) -> (f64, f64) {
    let l = (x * x + y * y).sqrt();
    (x / l, y / l)
}

pub struct Relief {
    n_dir: usize,
    /// Per direction: total offset of the ground (m), the part of it the
    /// layers follow, the pile thickness, and which feature owns the pile.
    total: Vec<f32>,
    structural: Vec<f32>,
    pile: Vec<f32>,
    owner: Vec<u16>,
    /// Where a volcano was built over an earlier pile: the volcano's own
    /// thickness, and the feature whose pile lies under it (0 if none).
    top: Vec<f32>,
    under: Vec<u16>,
    /// Thickness of a solid lid at the top of a liquid fill (0: none).
    lid: Vec<f32>,
    /// Depth below the ground where a fill's floor rock begins (0: none).
    bed: Vec<f32>,
    /// Depth of the sea in each direction (empty: no sea).
    sea_d: Vec<f32>,
    /// The sea's liquid and the radius of its surface.
    pub sea: Option<(MatId, f64)>,
    pub features: Vec<Feature>,
    max_up: f64,
    plumbing: Vec<Plumb>,
    /// Bounds of all plumbing, heat halos included.
    plumb_box: [f64; 4],
    /// A coarse grid over `plumb_box`: per square, the (volcano, part) pairs
    /// that reach it (part u16::MAX is the chamber), heat halos included.
    grid: Vec<Vec<(u16, u16)>>,
    grid_n: usize,
    grid_cell: f64,
}

/// 0 below a, 1 above b, smooth in between.
fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The unit direction at pseudo-angle p (the inverse of `pseudo_angle`).
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

impl Relief {
    /// Draw this seed's features and build the table. `base` is the plain
    /// noise surface radius in a direction.
    pub fn build(specs: &[ReliefSpec], mats: &MaterialTable, seed: u64, radius: f64, anchors: Option<&crate::deep::Anchors>, base: impl Fn(f64, f64) -> f64) -> Result<Self, String> {
        let mut features = Vec::new();
        let none = crate::deep::Anchors { plumes: vec![], slabs: vec![] };
        let anchors = anchors.unwrap_or(&none);
        for (i, s) in specs.iter().enumerate() {
            let es = hash(seed, i as i64, 0, 0x2E11);
            let kind = s.kind.as_str();
            // Families tied to the deep interior roll per plume or slab.
            if !matches!(kind, "hotspot" | "arc" | "trench") && unit(es) >= s.chance {
                continue;
            }
            let materials = s.material.iter().map(|p| Ok((mats.id(&p.material)?, p.weight))).collect::<Result<Vec<_>, String>>()?;
            let chamber = mats.id(s.chamber.as_deref().unwrap_or(if s.steep { "granite" } else { "gabbro" }))?;
            let crust = if s.crust_m > 0.0 {
                let name = match &s.crust {
                    Some(c) => c.clone(),
                    None => materials.first().and_then(|m| mats.get(m.0).freezes_to.clone()).ok_or("a crust needs a material to freeze to")?,
                };
                Some((mats.id(&name)?, s.crust_m))
            } else {
                None
            };
            let floor = match &s.floor {
                Some(name) if s.floor_m > 0.0 => Some((mats.id(name)?, s.floor_m)),
                _ => None,
            };
            let melt = match &s.melt {
                None => None,
                Some(name) => {
                    let id = mats.id(name)?;
                    // Kept a little above its freezing point.
                    let hold = mats.get(id).freezes_at_k.map_or(1450.0, |f| f + 5.0);
                    let dike = match &s.dike {
                        Some(d) => mats.id(d)?,
                        None => materials.first().map_or(chamber, |m| m.0),
                    };
                    Some((id, hold, dike))
                }
            };
            let feature = |kind: Kind, fs: u64, ux: f64, uy: f64, feed_r: f64| Feature {
                kind,
                ux,
                uy,
                width: range(s.width_m, unit(hash(fs, 1, 0, 0))),
                height: range(s.height_m, unit(hash(fs, 2, 0, 0))),
                extra: s.extra,
                steep: s.steep,
                materials: materials.clone(),
                band_m: s.band_m,
                fill_m: s.fill_m,
                seed: fs,
                active: unit(hash(fs, 3, 0, 0)) < s.active_chance,
                chamber,
                feed_r,
                melt,
                crust,
                floor,
                sea: if kind == Kind::Lowland { s.sea_level_m } else { 0.0 },
            };
            match kind {
                "hotspot" => {
                    // A shield over each plume that reaches up (by chance).
                    for (k, &(ux, uy, top)) in anchors.plumes.iter().enumerate() {
                        let fs = hash(es, k as i64, 3, 0x2E13);
                        if unit(fs) < s.chance {
                            features.push(feature(Kind::Volcano, fs, ux, uy, top));
                        }
                    }
                }
                "arc" | "trench" => {
                    for (k, &(ux, uy, tx, ty)) in anchors.slabs.iter().enumerate() {
                        let ss = hash(es, k as i64, 4, 0x2E14);
                        if unit(ss) >= s.chance {
                            continue;
                        }
                        if kind == "trench" {
                            features.push(feature(Kind::Trench, ss, ux, uy, 0.0));
                            continue;
                        }
                        // A chain of cones set back from the trench, over where
                        // the slab has sunk deep enough to melt.
                        let n = range([s.count[0] as f64, s.count[1] as f64 + 0.999], unit(hash(ss, 1, 0, 0))) as i64;
                        for c in 0..n {
                            let fs = hash(ss, c, 5, 0x2E15);
                            let off = range(s.offset_m, unit(hash(fs, 4, 0, 0))) / radius;
                            let (vx, vy) = unit_vec(ux + tx * off, uy + ty * off);
                            features.push(feature(Kind::Volcano, fs, vx, vy, 0.0));
                        }
                    }
                }
                _ => {
                    let kind = match kind {
                        "volcano" => Kind::Volcano,
                        "rift" => Kind::Rift,
                        "basin" => Kind::Basin,
                        "range" => Kind::Range,
                        "plateau" => Kind::Plateau,
                        "lowland" => Kind::Lowland,
                        "boulders" => Kind::Boulder,
                        k => return Err(format!("unknown relief kind '{k}' (volcano, hotspot, arc, trench, rift, basin, range, plateau, lowland, boulders)")),
                    };
                    let n = range([s.count[0] as f64, s.count[1] as f64 + 0.999], unit(hash(es, 1, 0, 0))) as i64;
                    for k in 0..n {
                        let fs = hash(es, k, 2, 0x2E12);
                        let (ux, uy) = random_dir(fs, 0);
                        let mut f = feature(kind, fs, ux, uy, 0.0);
                        if kind == Kind::Boulder && !f.materials.is_empty() {
                            // Each boulder is one rock, picked by weight.
                            f.materials = pick(&f.materials, unit(hash(fs, 5, 0, 0))).map(|m| vec![(m, 1.0)]).unwrap_or_default();
                        }
                        features.push(f);
                    }
                }
            }
        }

        let n_dir = ((2.0 * std::f64::consts::PI * radius / 4.0) as usize).clamp(1024, 65_536);
        let mut r = Relief {
            n_dir,
            total: vec![0.0; n_dir],
            structural: vec![0.0; n_dir],
            pile: vec![0.0; n_dir],
            owner: vec![0; n_dir],
            top: vec![0.0; n_dir],
            under: vec![0; n_dir],
            lid: vec![0.0; n_dir],
            bed: vec![0.0; n_dir],
            sea_d: Vec::new(),
            sea: None,
            features,
            max_up: 0.0,
            plumbing: Vec::new(),
            plumb_box: [0.0; 4],
            grid: Vec::new(),
            grid_n: 0,
            grid_cell: 1.0,
        };
        // Lowlands first, then the rest in order, volcanoes last: later
        // features build on top, and lava and ash bury whatever was there.
        let mut order: Vec<usize> = (0..r.features.len()).collect();
        order.sort_by_key(|&i| match r.features[i].kind {
            Kind::Lowland => 0,
            Kind::Volcano => 2,
            Kind::Boulder => 3,
            _ => 1,
        });
        for d in 0..n_dir {
            let (ux, uy) = direction_at(4.0 * (d as f64 + 0.5) / n_dir as f64);
            let rs = base(ux, uy);
            for &fi in &order {
                r.apply(fi, d, ux, uy, rs);
            }
        }
        r.update_max();
        Ok(r)
    }

    fn update_max(&mut self) {
        self.max_up = self.total.iter().fold(0f32, |a, &b| a.max(b)) as f64;
    }

    /// Add feature `fi`'s shape to direction d.
    fn apply(&mut self, fi: usize, d: usize, ux: f64, uy: f64, rs: f64) {
        let f = &self.features[fi];
        let (dx, dy) = (ux - f.ux, uy - f.uy);
        // Distance along the surface from the feature's centre (the chord,
        // close to the arc for anything smaller than the body).
        let x = (dx * dx + dy * dy).sqrt() * rs;
        let (w, h) = (f.width, f.height);
        let (mut total, mut structural, mut pile, mut lid) = (0.0, 0.0, 0.0, 0.0f64);
        // Where the fill ends and its floor rock begins (0: no floor).
        let mut bed = 0.0f64;
        match f.kind {
            Kind::Volcano => {
                if x >= w {
                    return;
                }
                let t = x / w;
                // Shields are broad and gentle; strato cones steep and concave.
                let mut z = if f.steep { h * (1.0 - t) * (1.0 - t) } else { h * (1.0 - t * t) * (1.0 - t * t).sqrt() };
                // A caldera: a flat-floored collapse at the summit.
                let cw = f.extra * w;
                if cw > 0.0 && x < cw {
                    let rim = if f.steep { h * (1.0 - f.extra) * (1.0 - f.extra) } else { h * (1.0 - f.extra * f.extra) * (1.0 - f.extra * f.extra).sqrt() };
                    let floor = rim - (h - rim).max(h * 0.12) * 0.9;
                    z = floor + (rim - floor) * smooth(0.75, 1.0, x / cw);
                }
                // Lumpy flanks from flows and side vents.
                z *= 0.9 + 0.2 * fbm(f.seed, ux * rs / (w * 0.25), uy * rs / (w * 0.25), 2);
                total = z;
                pile = z;
            }
            Kind::Rift => {
                if x >= w * 1.8 {
                    return;
                }
                // A flat floor dropped between steep walls, with raised shoulders.
                let drop = h * (1.0 - smooth(0.75, 1.0, x / w));
                let shoulder = 0.15 * h * smooth(0.9, 1.1, x / w) * (1.0 - smooth(1.2, 1.8, x / w));
                total = shoulder - drop;
                structural = shoulder;
                // Sediment fills the floor (it lies within the dropped part).
                pile = f.fill_m.min(drop) * (1.0 - smooth(0.6, 0.9, x / w));
            }
            Kind::Basin => {
                if x >= w * 1.6 {
                    return;
                }
                let t = x / w;
                let bowl = if t < 1.0 { h * (1.0 - t * t) } else { 0.0 };
                let rim = f.extra * (1.0 - ((t - 1.0) / 0.35) * ((t - 1.0) / 0.35)).max(0.0);
                total = rim - bowl;
                // Lava floods the floor, filling it partway.
                let fill = (f.fill_m).min(bowl);
                pile = rim + fill * (1.0 - smooth(0.7, 0.95, t));
                total += fill * (1.0 - smooth(0.7, 0.95, t));
                if let Some((_, th)) = f.floor {
                    let under = th * (1.0 - smooth(0.85, 1.05, t));
                    if under > 0.5 {
                        bed = pile;
                        pile += under;
                    }
                }
                // A liquid fill lies under a lid, and its rim is lid too.
                if let Some((_, th)) = f.crust {
                    lid = (rim + th).min(pile);
                }
            }
            Kind::Range => {
                if x >= w {
                    return;
                }
                let t = x / w;
                // A broad uplift with ridged peaks along it.
                let n = fbm(f.seed, ux * rs / (w * 0.35), uy * rs / (w * 0.35), 4);
                let ridge = 1.0 - (2.0 * n - 1.0).abs();
                // The layers rise in one broad fold; peaks and valleys are then
                // carved out of it, so valleys cut down through the layers.
                structural = h * (1.0 - t * t);
                total = structural * (0.45 + 0.55 * ridge);
            }
            Kind::Plateau => {
                if x >= w * 4.0 {
                    return;
                }
                // A mesa is what's left when the land around it wore away: the
                // whole region was raised (the layers stay flat across it), and
                // outside the mesa the ground was cut back down, so its cliffs
                // expose the layers. Far out, the raised region eases back down.
                let edge = 1.0 - smooth(0.8, 1.2, x / w + 0.08 * (2.0 * fbm(f.seed, ux * rs / (w * 0.1), uy * rs / (w * 0.1), 2) - 1.0));
                let region = 1.0 - smooth(1.6, 4.0, x / w);
                total = h * edge * region.max(edge);
                structural = h * region.max(edge);
            }
            Kind::Lowland => {
                // One half of the body sits lower and is flooded with lava.
                let c = ux * f.ux + uy * f.uy;
                let s = smooth(-1.0, 1.0, c * rs / w.max(1.0));
                total = -h * s;
                structural = total;
                // A sea is poured in at the end (`flood`).
                pile = if f.sea > 0.0 { 0.0 } else { f.fill_m * s };
                if let Some((_, th)) = f.crust {
                    lid = th.min(pile);
                }
                if let Some((_, th)) = f.floor.filter(|_| f.sea <= 0.0) {
                    if th * s > 0.5 {
                        bed = pile;
                        pile += th * s;
                    }
                }
            }
            Kind::Boulder => {
                // A rounded block resting on the ground, partly sunk into it
                // (`extra` is the share of its radius below the ground).
                if x >= w {
                    return;
                }
                let z = (w * w - x * x).sqrt() - f.extra * w;
                if z <= 0.0 {
                    return;
                }
                total = z;
                pile = z;
            }
            Kind::Trench => {
                if x >= w {
                    return;
                }
                // The plate bends down into the deep: the layers go with it.
                let t = x / w;
                let q = 1.0 - t * t;
                total = -h * q * q;
                structural = total;
            }
            Kind::Ejecta => {}
        }
        self.total[d] += total as f32;
        self.structural[d] += structural as f32;
        if pile > 0.05 {
            if matches!(f.kind, Kind::Volcano | Kind::Boulder) && self.owner[d] != 0 && self.pile[d] > 0.05 {
                // Lava piles up over what was there: keep the old pile below.
                self.under[d] = self.owner[d];
                self.top[d] = pile as f32;
                self.pile[d] += pile as f32;
                if self.bed[d] > 0.0 {
                    self.bed[d] += pile as f32;
                }
            } else {
                self.pile[d] = pile as f32;
                self.top[d] = 0.0;
                self.under[d] = 0;
                self.bed[d] = bed as f32;
            }
            self.lid[d] = lid as f32;
            self.owner[d] = fi as u16 + 1;
        }
    }

    /// Fill every hollow below a lowland's sea level, flat on top: the sea
    /// lies over whatever was built there (islands stand out of it), with its
    /// floor rock under it where the ground had nothing piled on it.
    pub fn flood(&mut self, radius: f64, base: impl Fn(f64, f64) -> f64) {
        let Some(fi) = self.features.iter().position(|f| f.sea > 0.0 && !f.materials.is_empty()) else {
            return;
        };
        let level = radius - self.features[fi].sea;
        let floor = self.features[fi].floor;
        self.sea = Some((self.features[fi].materials[0].0, level));
        self.sea_d = vec![0.0; self.n_dir];
        for d in 0..self.n_dir {
            let (ux, uy) = direction_at(4.0 * (d as f64 + 0.5) / self.n_dir as f64);
            let ground = base(ux, uy) + self.total[d] as f64;
            let depth = (level - ground) as f32;
            if depth <= 0.05 {
                continue;
            }
            if self.owner[d] != 0 && self.pile[d] > 0.05 {
                self.under[d] = self.owner[d];
                self.top[d] = depth;
                self.pile[d] += depth;
                self.bed[d] = 0.0;
            } else {
                self.pile[d] = depth;
                self.top[d] = 0.0;
                self.under[d] = 0;
                self.bed[d] = 0.0;
                if let Some((_, th)) = floor {
                    self.bed[d] = depth;
                    self.pile[d] = depth + th as f32;
                }
            }
            self.lid[d] = 0.0;
            self.sea_d[d] = depth;
            self.total[d] += depth;
            self.owner[d] = fi as u16 + 1;
        }
        self.update_max();
    }

    /// Fault blocks: where faults leave cliffs, the ground steps with them.
    pub fn add_scarps(&mut self, faults: &crate::geology::Faults, base: impl Fn(f64, f64) -> f64) {
        if faults.scarp <= 0.0 {
            return;
        }
        for d in 0..self.n_dir {
            let (ux, uy) = direction_at(4.0 * (d as f64 + 0.5) / self.n_dir as f64);
            let rs = base(ux, uy) + self.total[d] as f64;
            let drop = faults.surface_drop(ux * rs, uy * rs) as f32;
            self.total[d] -= drop;
            self.structural[d] -= drop;
        }
        self.update_max();
    }

    /// Crater rims and the blanket of shattered rock thrown out around them.
    pub fn add_rims(&mut self, craters: &[(f64, f64, f64)], rim: f64, ejecta: MatId, base: impl Fn(f64, f64) -> f64) {
        if rim <= 0.0 {
            return;
        }
        let fi = self.features.len();
        self.features.push(Feature {
            kind: Kind::Ejecta,
            ux: 0.0,
            uy: 0.0,
            width: 0.0,
            height: 0.0,
            extra: 0.0,
            steep: false,
            materials: vec![(ejecta, 1.0)],
            band_m: [0.0, 0.0],
            fill_m: 0.0,
            seed: 0,
            active: false,
            chamber: ejecta,
            feed_r: 0.0,
            melt: None,
            crust: None,
            floor: None,
            sea: 0.0,
        });
        for d in 0..self.n_dir {
            let (ux, uy) = direction_at(4.0 * (d as f64 + 0.5) / self.n_dir as f64);
            let rs = base(ux, uy) + self.total[d] as f64;
            let mut up = 0.0;
            for &(cx, cy, cr) in craters {
                let (dx, dy) = (ux * rs - cx, uy * rs - cy);
                let dist = (dx * dx + dy * dy).sqrt();
                // The bowl's centre sits above the ground, so measure from where
                // the bowl meets the surface (about 0.8 radii from its middle).
                let t = dist / cr;
                if !(0.75..2.6).contains(&t) {
                    continue;
                }
                up += rim * cr * if t < 1.0 { smooth(0.75, 1.0, t) } else { { let q = (1.0 - (t - 1.0) / 1.6).max(0.0); q * q * q } };
            }
            if up > 0.05 {
                self.total[d] += up as f32;
                let on_boulder = self.owner[d] != 0 && self.features[self.owner[d] as usize - 1].kind == Kind::Boulder;
                if on_boulder {
                    // Boulders lie on the blanket: it goes under them.
                    if self.under[d] == 0 {
                        self.top[d] = self.pile[d];
                        self.under[d] = fi as u16 + 1;
                    }
                    self.pile[d] += up as f32;
                } else if self.owner[d] == 0 || up as f32 > self.pile[d] {
                    self.pile[d] = up as f32;
                    self.owner[d] = fi as u16 + 1;
                    self.top[d] = 0.0;
                    self.under[d] = 0;
                }
            }
        }
        self.update_max();
    }

    fn at(&self, ux: f64, uy: f64) -> (usize, usize, f32) {
        let p = pseudo_angle(ux, uy) * self.n_dir as f64 / 4.0 - 0.5;
        let p = if p < 0.0 { p + self.n_dir as f64 } else { p };
        let i0 = (p.floor() as usize) % self.n_dir;
        (i0, (i0 + 1) % self.n_dir, (p - p.floor()) as f32)
    }

    /// The ground's offset from the plain noise surface in direction (ux, uy).
    pub fn offset(&self, ux: f64, uy: f64) -> f64 {
        let (a, b, t) = self.at(ux, uy);
        (self.total[a] + (self.total[b] - self.total[a]) * t) as f64
    }

    /// How far the layers' reference surface sits below the actual ground
    /// (piles on top and excavations both change the ground but not the layers).
    pub fn unstructural(&self, ux: f64, uy: f64) -> f64 {
        let (a, b, t) = self.at(ux, uy);
        let ta = self.total[a] - self.structural[a];
        let tb = self.total[b] - self.structural[b];
        (ta + (tb - ta) * t) as f64
    }

    /// The pile material at `depth` below the ground, if the cell is in one.
    pub fn pile_at(&self, ux: f64, uy: f64, depth: f64, cx: i64, cy: i64) -> Option<MatId> {
        let (a, b, t) = self.at(ux, uy);
        let pile = (self.pile[a] + (self.pile[b] - self.pile[a]) * t) as f64;
        if depth >= pile || depth < -1.0 {
            return None;
        }
        let n = if t < 0.5 { a } else { b };
        let mut owner = self.owner[n];
        if self.under[n] != 0 {
            let top = (self.top[a] + (self.top[b] - self.top[a]) * t) as f64;
            if depth >= top {
                owner = self.under[n];
            }
        }
        let f = self.features.get((owner as usize).checked_sub(1)?)?;
        if self.bed[n] > 0.0 {
            let fl = [self.owner[n], self.under[n]].iter().filter(|&&o| o != 0).find_map(|&o| self.features[o as usize - 1].floor);
            if let Some((floor, _)) = fl {
                let bed = (self.bed[a] + (self.bed[b] - self.bed[a]) * t) as f64;
                if depth >= bed {
                    return Some(floor);
                }
            }
        }
        if let Some((lid_mat, _)) = f.crust {
            if owner == self.owner[n] && depth < self.lid[n] as f64 {
                return Some(lid_mat);
            }
        }
        if f.materials.len() == 1 || f.kind != Kind::Volcano {
            // Fills and blankets are one material.
            return f.materials.first().map(|m| m.0);
        }
        // Volcano flows drape the cone: bands run parallel to its slopes, by
        // depth below the surface, each band one flow; edges wander a little.
        let height = depth;
        let avg = (f.band_m[0] + f.band_m[1]) / 2.0;
        let wob = 0.35 * avg * (2.0 * fbm(f.seed ^ 0xF10, cx as f64 / (avg * 6.0), cy as f64 / (avg * 6.0), 2) - 1.0);
        let band = ((height + wob) / avg.max(0.5)).floor() as i64;
        pick(&f.materials, unit(hash(f.seed, band, 0, 0xF11)))
    }

    /// Build what lies under each volcano, once the surface is final.
    /// `base` is the plain noise surface radius in a direction; `magma` the melt.
    pub fn build_plumbing(&mut self, radius: f64, body_magma: MatId, body_dike: MatId, hold_k: f64, base: impl Fn(f64, f64) -> f64) {
        let surface = |ux: f64, uy: f64| base(ux, uy) + self.offset(ux, uy);
        let mut out = Vec::new();
        for f in self.features.iter().filter(|f| f.kind == Kind::Volcano) {
            let (ux, uy) = (f.ux, f.uy);
            let (tx, ty) = (-uy, ux);
            let (magma, hold, dike) = f.melt.unwrap_or((body_magma, hold_k, body_dike));
            let w = f.width;
            let s = f.seed ^ 0x9_1B;
            let top = surface(ux, uy);
            let base = top - self.offset(ux, uy);
            // A flattened chamber some way below the old ground.
            let depth = (0.7 * w).min(0.2 * radius);
            let (radial, tangential) = (0.15 * w, 0.28 * w);
            // Measured from the lower of the old ground and the ground the
            // volcano was built on (it may stand in a trench or a basin).
            let cr = base.min(top - f.height) - depth;
            let c = (ux * cr, uy * cr);
            let rock = f.materials.first().map_or(f.chamber, |m| m.0);
            let hot = if f.active { magma } else { rock };
            let mut parts = Vec::new();
            let mut seg = |a: (f64, f64), b: (f64, f64), half: f64, mat: MatId| {
                let bbox = [a.0.min(b.0) - half, a.1.min(b.1) - half, a.0.max(b.0) + half, a.1.max(b.1) + half];
                parts.push(Part { a, b, half, mat, bbox });
            };
            // A wandering path from radius r0 to r1 along direction (vx, vy),
            // drifting sideways a little.
            let mut path = |r0: f64, r1: f64, v: (f64, f64), n: i64, jit: f64, half: f64, mat: MatId, salt: i64| {
                let t = (-v.1, v.0);
                let mut prev = (v.0 * r0, v.1 * r0);
                for k in 1..=n {
                    let q = k as f64 / n as f64;
                    let r = r0 + (r1 - r0) * q;
                    let j = if k == n { 0.0 } else { jit * (2.0 * unit(hash(s, k, salt, 0x9_1C)) - 1.0) };
                    let p = (v.0 * r + t.0 * j, v.1 * r + t.1 * j);
                    seg(prev, p, half, mat);
                    prev = p;
                }
            };
            // The conduit from the chamber to the vent: a lava lake in the
            // crater when live, a plug of hard rock (a volcanic neck) when dead.
            let cw = (0.012 * w).clamp(1.0, 40.0);
            path(cr + radial * 0.5, top + 1.0, (ux, uy), 4, 0.03 * w, cw, hot, 1);
            // Hotspots: the feed from the plume head below.
            if f.feed_r > 0.0 && f.feed_r < cr {
                path(f.feed_r, cr, (ux, uy), 8, 0.05 * (cr - f.feed_r).min(20.0 * w), (0.03 * w).clamp(2.0, 80.0), if f.active { magma } else { f.chamber }, 2);
            }
            // Radial dikes fanning up and out to the flanks; some reach the surface.
            let nd = 2 + (unit(hash(s, 0, 3, 0)) * 2.999) as i64;
            for k in 0..nd {
                let h = hash(s, k, 4, 0);
                let side = if unit(h) < 0.5 { -1.0 } else { 1.0 };
                let off = side * w * range([0.25, 0.85], unit(hash(h, 1, 0, 0)));
                let (vx, vy) = unit_vec(ux + tx * off / top, uy + ty * off / top);
                let end = surface(vx, vy) - f.height * 0.3 * unit(hash(h, 2, 0, 0)).max(0.0);
                let start = (c.0 + ux * radial * 0.6 + tx * off * 0.15, c.1 + uy * radial * 0.6 + ty * off * 0.15);
                // Live volcanoes have a few dikes still molten (feeding fissures).
                let mat = if f.active && unit(hash(h, 3, 0, 0)) < 0.4 { magma } else { dike };
                seg(start, (vx * end, vy * end), (0.01 * w).clamp(1.2, 20.0), mat);
            }
            // Sills: sheets that spread sideways between the layers.
            let ns = 1 + (unit(hash(s, 0, 5, 0)) * 1.999) as i64;
            for k in 0..ns {
                let h = hash(s, k, 6, 0);
                let r = cr + radial + (base - cr - radial) * range([0.25, 0.8], unit(hash(h, 1, 0, 0)));
                let len = w * range([0.2, 0.5], unit(hash(h, 2, 0, 0)));
                let side = if unit(h) < 0.5 { -1.0 } else { 1.0 };
                let (a, b) = (-0.15 * len * side, len * side);
                let p0 = unit_vec(ux + tx * a / r, uy + ty * a / r);
                let p1 = unit_vec(ux + tx * b / r, uy + ty * b / r);
                seg((p0.0 * r, p0.1 * r), (p1.0 * r, p1.1 * r), (0.015 * w).clamp(1.5, 30.0), dike);
            }
            let mut bbox = [c.0, c.1, c.0, c.1];
            grow(&mut bbox, c.0, c.1, tangential * 1.4 + 0.6 * w);
            for p in &parts {
                let (lo, hi) = ((p.bbox[0], p.bbox[1]), (p.bbox[2], p.bbox[3]));
                grow(&mut bbox, lo.0, lo.1, 4.0 * p.half + 6.0);
                grow(&mut bbox, hi.0, hi.1, 4.0 * p.half + 6.0);
            }
            out.push(Plumb {
                bbox,
                c,
                u: (ux, uy),
                radial,
                tangential,
                chamber: if f.active { magma } else { f.chamber },
                parts,
                active: f.active,
                magma,
                hold_k: hold,
                seed: s,
            });
        }
        let mut all = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for p in &out {
            let b = p.bbox;
            grow(&mut all, b[0], b[1], 0.0);
            grow(&mut all, b[2], b[3], 0.0);
        }
        self.plumb_box = all;
        self.plumbing = out;
        self.build_grid();
    }

    fn build_grid(&mut self) {
        if self.plumbing.is_empty() {
            return;
        }
        let b = self.plumb_box;
        let n = 256usize;
        let cell = ((b[2] - b[0]).max(b[3] - b[1]) / n as f64).max(1.0);
        let mut grid = vec![Vec::new(); n * n];
        let half_diag = cell * 0.7072;
        // Every grid square within `reach` of the shape `dist` measures.
        let mut mark = |lo: (f64, f64), hi: (f64, f64), reach: f64, id: (u16, u16), dist: &dyn Fn(f64, f64) -> f64| {
            let gx0 = (((lo.0 - reach - b[0]) / cell).floor().max(0.0) as usize).min(n - 1);
            let gy0 = (((lo.1 - reach - b[1]) / cell).floor().max(0.0) as usize).min(n - 1);
            let gx1 = (((hi.0 + reach - b[0]) / cell).floor().max(0.0) as usize).min(n - 1);
            let gy1 = (((hi.1 + reach - b[1]) / cell).floor().max(0.0) as usize).min(n - 1);
            for gy in gy0..=gy1 {
                for gx in gx0..=gx1 {
                    let (x, y) = (b[0] + (gx as f64 + 0.5) * cell, b[1] + (gy as f64 + 0.5) * cell);
                    if dist(x, y) <= reach + half_diag {
                        grid[gy * n + gx].push(id);
                    }
                }
            }
        };
        for (pi, p) in self.plumbing.iter().enumerate() {
            for (qi, q) in p.parts.iter().enumerate() {
                let reach = q.half + 4.0 * q.half + 6.0;
                mark((q.bbox[0], q.bbox[1]), (q.bbox[2], q.bbox[3]), reach, (pi as u16, qi as u16), &|x, y| seg_dist(q.a, q.b, x, y));
            }
            let reach = 2.2 * p.tangential;
            mark(p.c, p.c, reach, (pi as u16, u16::MAX), &|x, y| ((x - p.c.0) * (x - p.c.0) + (y - p.c.1) * (y - p.c.1)).sqrt());
        }
        self.grid = grid;
        self.grid_n = n;
        self.grid_cell = cell;
    }

    /// The grid square's list at (x, y).
    fn grid_at(&self, x: f64, y: f64) -> &[(u16, u16)] {
        if self.grid.is_empty() || !inside(&self.plumb_box, x, y) {
            return &[];
        }
        let n = self.grid_n;
        let gx = (((x - self.plumb_box[0]) / self.grid_cell) as usize).min(n - 1);
        let gy = (((y - self.plumb_box[1]) / self.grid_cell) as usize).min(n - 1);
        &self.grid[gy * n + gx]
    }

    /// The chamber's size measure at (x, y): below 1 is inside its lumpy outline.
    fn chamber_norm(p: &Plumb, x: f64, y: f64) -> f64 {
        let (dx, dy) = (x - p.c.0, y - p.c.1);
        let a = (dx * p.u.0 + dy * p.u.1) / p.radial;
        let b = (dy * p.u.0 - dx * p.u.1) / p.tangential;
        let n = (a * a + b * b).sqrt();
        if n > 1.4 {
            return n;
        }
        n / (0.8 + 0.4 * fbm(p.seed, x / (p.tangential * 0.5), y / (p.tangential * 0.5), 2))
    }

    /// Volcano plumbing at (x, y), if any. Earlier volcanoes win, and within
    /// one, its pipes and sheets win over its chamber.
    pub fn plumbing_at(&self, x: f64, y: f64) -> Option<MatId> {
        let list = self.grid_at(x, y);
        let mut i = 0;
        while i < list.len() {
            // Entries come grouped by volcano: parts first, then the chamber.
            let pi = list[i].0;
            let p = &self.plumbing[pi as usize];
            let mut chamber = false;
            while i < list.len() && list[i].0 == pi {
                let qi = list[i].1;
                if qi == u16::MAX {
                    chamber = true;
                } else {
                    let q = &p.parts[qi as usize];
                    if inside(&q.bbox, x, y) && seg_dist(q.a, q.b, x, y) < q.half {
                        return Some(q.mat);
                    }
                }
                i += 1;
            }
            if chamber && Self::chamber_norm(p, x, y) < 1.0 {
                return Some(p.chamber);
            }
        }
        None
    }

    /// How strongly live melt heats (x, y), and how hot it keeps things: 1 in
    /// the melt, fading out around it (the strongest nearby volcano wins).
    pub fn magma_glow(&self, x: f64, y: f64) -> (f64, f64) {
        let fade = |e: f64| if e <= 0.0 { 1.0 } else { let q = (1.0 - e).max(0.0); q * q };
        let mut g: f64 = 0.0;
        let mut hold = 0.0;
        for &(pi, qi) in self.grid_at(x, y) {
            let p = &self.plumbing[pi as usize];
            if !p.active {
                continue;
            }
            let v = if qi == u16::MAX {
                fade((Self::chamber_norm(p, x, y) - 1.0) / 1.2)
            } else {
                let q = &p.parts[qi as usize];
                if q.mat != p.magma {
                    continue;
                }
                let m = 4.0 * q.half + 6.0;
                fade((seg_dist(q.a, q.b, x, y) - q.half) / m)
            };
            if v > g {
                g = v;
                hold = p.hold_k;
            }
        }
        (g, hold)
    }

    /// Whether the cell at `depth` below the ground is part of a boulder
    /// resting on the surface (bare rock: no regolith on it).
    pub fn in_boulder(&self, ux: f64, uy: f64, depth: f64) -> bool {
        let (a, b, t) = self.at(ux, uy);
        let n = if t < 0.5 { a } else { b };
        let o = self.owner[n];
        if o == 0 || self.features[o as usize - 1].kind != Kind::Boulder {
            return false;
        }
        let lim = if self.under[n] != 0 { self.top[a] + (self.top[b] - self.top[a]) * t } else { self.pile[a] + (self.pile[b] - self.pile[a]) * t };
        depth < lim as f64
    }

    /// Each volcano's chamber centre, in the order of the volcano features.
    pub fn chamber_centres(&self) -> Vec<(f64, f64)> {
        self.plumbing.iter().map(|p| p.c).collect()
    }

    /// Table values in a direction (for debugging tools).
    pub fn table_at(&self, ux: f64, uy: f64) -> (f32, f32, f32, u16) {
        let (a, _, _) = self.at(ux, uy);
        (self.total[a], self.structural[a], self.pile[a], self.owner[a])
    }

    /// How deep the sea is in direction (ux, uy) (the shallower of the two
    /// table entries either side), 0 where there is none.
    pub fn sea_depth(&self, ux: f64, uy: f64) -> f64 {
        if self.sea_d.is_empty() {
            return 0.0;
        }
        let (a, b, _) = self.at(ux, uy);
        self.sea_d[a].min(self.sea_d[b]) as f64
    }

    pub fn debug_at(&self, ux: f64, uy: f64) -> (f32, f32, u16, f32, f32) {
        let (a, _, _) = self.at(ux, uy);
        (self.pile[a], self.top[a], self.under[a], self.bed[a], self.lid[a])
    }

    pub fn max_up(&self) -> f64 {
        self.max_up
    }
}
