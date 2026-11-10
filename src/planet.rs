//! The generator: `material(x, y)` for any 1 m cell of a body, computed from its
//! spec and seed alone. Nothing is stored, and the same inputs always give the
//! same answer on any machine.
//!
//! Coordinates are integer cells with the body's centre at (0, 0) and y pointing up.

use crate::materials::{MatId, MaterialTable, VACUUM};
use crate::noise::{fbm, hash, ridge, unit};
use crate::geology::{Caves as CaveKinds, Faults, IceCaps, Intrusions, Rubble, Strata};
use crate::spec::BodySpec;

struct Layer {
    outer: f64,
    material: MatId,
    boundary_noise: f64,
}

#[derive(Clone)]
struct Blob {
    cx: f64,
    cy: f64,
    radius: f64,
    material: MatId,
    seed: u64,
    /// Pockets of melt (magma, molten sulfur): how hot the melt and the rock
    /// around it are kept (0 for solid blobs).
    hold_k: f64,
}

struct Vein {
    material: MatId,
    host: Vec<MatId>,
    freq: f64,
    width: f64,
    r_min: f64,
    r_max: f64,
    seed: u64,
}

struct Sprinkle {
    material: MatId,
    host: Vec<MatId>,
    chance: f64,
    r_min: f64,
    r_max: f64,
    seed: u64,
}

struct Pipe {
    ux: f64,
    uy: f64,
    half_width: f64,
    r_start: f64,
}

struct Crater {
    cx: f64,
    cy: f64,
    radius: f64,
}

struct Caves {
    material: MatId,
    freq: f64,
    threshold: f64,
    r_min: f64,
    r_max: f64,
}

const BUCKET: f64 = 512.0;

/// A cell's context after the first generation stage.
struct Cell {
    fx: f64,
    fy: f64,
    r: f64,
    f: f64,
    surface: f64,
    m: MatId,
}

/// Below this share (0.1%) an element is only a trace in common rock.
pub const TRACE_GRADE: f64 = 0.001;

pub struct Planet {
    pub spec: BodySpec,
    /// What this body is known for (data/specialties.toml), rolled from its seed.
    pub specialties: Vec<crate::specialty::Rolled>,
    /// Jackpot lodes: a few big pods of one rich material.
    pub pods: Vec<crate::specialty::Pod>,
    pub mats: MaterialTable,
    pub radius: f64,
    /// Largest possible surface radius, so anything beyond it is vacuum.
    pub max_radius: f64,
    /// Buried liquid layers (an ocean under ice, a magma ocean under rock):
    /// the liquid, its top as a share of the radius, and how much denser the
    /// layer over it is (the weight on it pushes it up any hole).
    pools: Vec<(MatId, f64, f64)>,
    seed: u64,
    layers: Vec<Layer>,
    caves: Option<Caves>,
    blobs: Vec<Blob>,
    /// Occupied buckets only (a full grid would be billions of squares on a
    /// planet-size body).
    buckets: std::collections::HashMap<(i64, i64), Vec<u32>>,
    veins: Vec<Vein>,
    sprinkles: Vec<Sprinkle>,
    pipes: Vec<Pipe>,
    pipe_material: MatId,
    craters: Vec<Crater>,
    regolith: MatId,
    heart: MatId,
    /// Gameplay gravity at the surface, m/s².
    pub gravity: f64,
    /// Sky temperature for radiating surfaces, K.
    pub space_k: f64,
    /// Radial profile from the centre to the nominal surface, PROFILE_N + 1
    /// samples: real gravity (m/s²) and pressure (Pa) at each.
    profile_g: Vec<f64>,
    profile_p: Vec<f64>,
    pub strata: Option<Strata>,
    /// Below this depth no strata band, fault offset or fault gouge can occur.
    strata_reach: f64,
    /// Below this depth no crater bowl, melt or breccia can occur.
    crater_reach: f64,
    pub faults: Option<Faults>,
    pub intrusions: Option<Intrusions>,
    pub relief: Option<crate::relief::Relief>,
    /// The blob buckets' origin (max_radius before relief raised it).
    bucket_origin: f64,
    rubble: Option<Rubble>,
    widmanstatten: Option<crate::metal::Widmanstatten>,
    /// The glamour pass over the interior (None at glamour 0).
    glamour: Option<crate::glamour::Glamour>,
    /// How each material is drawn on this body (its colour scheme).
    pub palette: crate::palette::Palette,
    deep: Option<crate::deep::Deep>,
    cave_kinds: Option<CaveKinds>,
    ice_caps: Option<IceCaps>,
    /// Shattered rock and melt under craters: (fraction of radius, material).
    breccia: Option<(f64, MatId)>,
    melt: Option<(f64, MatId)>,
    /// The body's flora (life), laid over the generator.
    pub flora: Option<crate::flora::Flora>,
}

const PROFILE_N: usize = 512;
const G_NEWTON: f64 = 6.674e-11;

/// A random direction from a seed, without trig: a point picked inside the
/// unit circle and scaled to length 1.
fn direction(seed: u64, i: u64) -> (f64, f64) {
    let mut k = 0;
    loop {
        let x = unit(hash(seed, i as i64, k, 1)) * 2.0 - 1.0;
        let y = unit(hash(seed, i as i64, k, 2)) * 2.0 - 1.0;
        let d = x * x + y * y;
        if d > 1e-4 && d <= 1.0 {
            let l = d.sqrt();
            return (x / l, y / l);
        }
        k += 1;
    }
}

/// Live magma is held at least this hot.
const MAGMA_K: f64 = 1450.0;

impl Planet {
    pub fn new(spec: BodySpec, mats: MaterialTable) -> Result<Self, String> {
        // (A table another body already changed: start from it as loaded.)
        let mats = match mats.pristine.clone() {
            Some(p) => (*p).clone(),
            None => mats,
        };
        let seed = spec.seed;
        let radius = spec.radius_m;
        let max_radius = radius * (1.0 + spec.shape.roughness) + 2.0;
        let layers = spec
            .layer
            .iter()
            .map(|l| Ok(Layer { outer: l.outer, material: mats.id(&l.material)?, boundary_noise: l.boundary_noise }))
            .collect::<Result<Vec<_>, String>>()?;
        let caves = match &spec.caves {
            Some(c) => Some(Caves {
                material: mats.id(&c.material)?,
                freq: 1.0 / c.scale_m,
                threshold: c.threshold,
                r_min: c.r_min,
                r_max: c.r_max,
            }),
            None => None,
        };

        let mut blobs = Vec::new();
        for (bi, b) in spec.blob.iter().enumerate() {
            let m = mats.id(&b.material)?;
            for k in 0..b.count as u64 {
                let s = hash(seed, bi as i64, k as i64, 0xB10B);
                let (ux, uy) = direction(s, 0);
                let r = radius * (b.r_min + (b.r_max - b.r_min) * unit(hash(s, 1, 0, 0)));
                let rad = b.radius_min_m + (b.radius_max_m - b.radius_min_m) * unit(hash(s, 2, 0, 0));
                // A pocket of melt sits in rock it has heated: the melt a
                // margin above freezing, the rock around warm, so a breach
                // finds it molten and it cools from where it's opened.
                let hold_k = match (mats.get(m).state, mats.get(m).freezes_at_k) {
                    (crate::materials::State::Liquid, Some(f)) => f + 60.0,
                    _ => 0.0,
                };
                blobs.push(Blob { cx: ux * r, cy: uy * r, radius: rad, material: m, seed: s, hold_k });
            }
        }
        // Spatial buckets so each cell only checks the blobs near it.
        let mut buckets: std::collections::HashMap<(i64, i64), Vec<u32>> = std::collections::HashMap::new();
        for (i, b) in blobs.iter().enumerate() {
            let reach = b.radius * 1.3;
            let to_b = |v: f64| ((v + max_radius) / BUCKET).floor() as i64;
            for by in to_b(b.cy - reach)..=to_b(b.cy + reach) {
                for bx in to_b(b.cx - reach)..=to_b(b.cx + reach) {
                    buckets.entry((bx, by)).or_default().push(i as u32);
                }
            }
        }

        let ids = |names: &Vec<String>| names.iter().map(|n| mats.id(n)).collect::<Result<Vec<_>, String>>();
        let veins = spec
            .vein
            .iter()
            .enumerate()
            .map(|(i, v)| {
                Ok(Vein {
                    material: mats.id(&v.material)?,
                    host: ids(&v.host)?,
                    freq: 1.0 / v.scale_m,
                    width: v.width,
                    r_min: v.r_min,
                    r_max: v.r_max,
                    seed: hash(seed, i as i64, 0, 0x7E1A),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let sprinkles = spec
            .sprinkle
            .iter()
            .enumerate()
            .map(|(i, s)| {
                Ok(Sprinkle {
                    material: mats.id(&s.material)?,
                    host: ids(&s.host)?,
                    chance: s.chance,
                    r_min: s.r_min,
                    r_max: s.r_max,
                    seed: hash(seed, i as i64, 0, 0x5B41),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;

        let (pipes, pipe_material) = match &spec.pipes {
            Some(p) => (
                (0..p.count as u64)
                    .map(|k| {
                        let (ux, uy) = direction(hash(seed, k as i64, 0, 0x919E), 0);
                        Pipe { ux, uy, half_width: p.width_m / 2.0, r_start: p.r_min * radius }
                    })
                    .collect(),
                mats.id(&p.material)?,
            ),
            None => (Vec::new(), VACUUM),
        };

        let mut planet = Planet {
            spec: spec.clone(),
            radius,
            max_radius,
            seed,
            layers,
            caves,
            blobs,
            buckets,
            veins,
            sprinkles,
            pipes,
            pipe_material,
            craters: Vec::new(),
            regolith: mats.id(spec.regolith_material.as_deref().unwrap_or("regolith"))?,
            heart: mats.id("heart")?,
            gravity: spec.gravity_m_s2.unwrap_or(mats.physics.gravity_m_s2),
            space_k: spec.space_k.unwrap_or(mats.physics.space_k),
            profile_g: Vec::new(),
            profile_p: Vec::new(),
            strata: None,
            strata_reach: 0.0,
            crater_reach: 0.0,
            faults: None,
            intrusions: None,
            relief: None,
            bucket_origin: max_radius,
            rubble: None,
            pools: Vec::new(),
            widmanstatten: None,
            glamour: None,
            flora: None,
            specialties: Vec::new(),
            pods: Vec::new(),
            palette: crate::palette::Palette::natural(&mats, spec.atmosphere_colour),
            deep: None,
            cave_kinds: None,
            ice_caps: None,
            breccia: None,
            melt: None,
            mats,
        };
        planet.build_profile();
        for w in planet.spec.layer.windows(2) {
            let (liq, over) = (planet.mats.id(&w[0].material)?, planet.mats.id(&w[1].material)?);
            if planet.mats.get(liq).state == crate::materials::State::Liquid {
                let ratio = planet.mats.get(over).density / planet.mats.get(liq).density;
                planet.pools.push((liq, w[0].outer, ratio));
            }
        }
        if let Some(s) = &spec.strata {
            planet.strata = Some(Strata::build(s, &planet.mats, seed ^ 0x57A7A, radius)?);
        }
        if let Some(f) = &spec.faults {
            let p = &planet;
            let faults = Faults::build(f, &p.mats, seed ^ 0xFA17, |ux, uy| p.surface_radius(ux, uy))?;
            planet.faults = Some(faults);
        }
        // G5: the surface's shape. Faults were placed on the plain noise
        // surface; their cliffs and every other feature go into one table.
        if !spec.relief.is_empty() || planet.faults.as_ref().is_some_and(|f| f.scarp > 0.0) || spec.craters.as_ref().is_some_and(|c| c.rim > 0.0) {
            let p = &planet;
            let anchors = spec.deep.as_ref().map(|d| crate::deep::anchors(d, seed ^ 0xDEE9, radius));
            let mut rel = crate::relief::Relief::build(&spec.relief, &p.mats, seed ^ 0x2E1, radius, anchors.as_ref(), |ux, uy| p.base_surface(ux, uy))?;
            if let Some(f) = &p.faults {
                rel.add_scarps(f, |ux, uy| p.base_surface(ux, uy));
            }
            planet.relief = Some(rel);
        }
        if let Some(d) = &spec.deep {
            let p = &planet;
            let deep = crate::deep::Deep::build(d, &spec.layer, &p.mats, seed ^ 0xDEE9, radius, |ux, uy| p.surface_radius(ux, uy))?;
            planet.deep = Some(deep);
        }
        if let Some(r) = &spec.rubble {
            planet.rubble = Some(Rubble::build(r, &planet.mats, seed ^ 0x2AB)?);
        }
        if let Some(g) = spec.glamour.filter(|g| *g > 0.0) {
            let pal = spec.palette.clone().unwrap_or_default();
            let scheme = match &pal.scheme {
                Some(n) => Some(crate::palette::Scheme::parse(n).ok_or(format!("unknown colour scheme '{n}' (analogous, complementary, split-complementary, triadic, square)"))?),
                None => None,
            };
            planet.palette = crate::palette::Palette::build(&planet.mats, spec.atmosphere_colour, seed ^ 0xC01, g, scheme, pal.hue);
        }
        if let (Some(g), Some(rules)) = (spec.glamour.filter(|g| *g > 0.0), planet.mats.glamour.as_ref()) {
            planet.glamour = Some(crate::glamour::Glamour::build(rules, &planet.mats, g, seed ^ 0x6A, radius));
        }
        if let Some(w) = &spec.widmanstatten {
            planet.widmanstatten = Some(crate::metal::Widmanstatten::build(w, &planet.mats, seed ^ 0x3E7)?);
        }
        if !spec.cave.is_empty() {
            planet.cave_kinds = Some(CaveKinds::build(&spec.cave, &planet.mats, seed ^ 0xCA5, radius)?);
        }
        if let Some(ic) = &spec.ice_caps {
            planet.ice_caps = Some(IceCaps::build(ic, &planet.mats)?);
        }
        if let Some(c) = &spec.craters {
            if c.breccia > 0.0 {
                planet.breccia = Some((c.breccia, planet.mats.id(c.breccia_material.as_deref().unwrap_or("impact breccia"))?));
            }
            if c.melt > 0.0 {
                planet.melt = Some((c.melt, planet.mats.id(c.melt_material.as_deref().unwrap_or("obsidian"))?));
            }
        }
        if let Some(st) = &planet.strata {
            let (fr, ft) = planet.faults.as_ref().map_or((0.0, 0.0), |f| f.reach());
            planet.strata_reach = (st.max_depth() + 1.0 + ft).max(fr);
        }
        if !spec.intrusion.is_empty() {
            let p = &planet;
            let ints = Intrusions::build(&spec.intrusion, &p.mats, seed ^ 0x1A7, radius, |ux, uy| p.surface_radius(ux, uy))?;
            planet.intrusions = Some(ints);
        }
        if let Some(c) = &spec.craters {
            for k in 0..c.count as u64 {
                let s = hash(seed, k as i64, 0, 0xC4A7);
                let (ux, uy) = direction(s, 0);
                let rad = c.radius_min_m + (c.radius_max_m - c.radius_min_m) * unit(hash(s, 3, 0, 0)).powi(2);
                let rs = planet.surface_radius(ux, uy);
                // Bowl: a circle whose centre sits above the surface, so it cuts in
                // to about 40% of its radius.
                let lift = rs + rad * 0.6;
                planet.craters.push(Crater { cx: ux * lift, cy: uy * lift, radius: rad });
                // Bowls reach 0.4 radii down and breccia below that; 2.5 radii
                // is a safe bound even where the surface rises or falls nearby.
                planet.crater_reach = planet.crater_reach.max(2.5 * rad + 4.0);
            }
            // Raised rims with a blanket of shattered rock around each crater.
            if c.rim > 0.0 {
                let ejecta = planet.mats.id(c.breccia_material.as_deref().unwrap_or("impact breccia"))?;
                let list: Vec<(f64, f64, f64)> = planet.craters.iter().map(|k| (k.cx, k.cy, k.radius)).collect();
                let mut rel = planet.relief.take().unwrap();
                {
                    let p = &planet;
                    rel.add_rims(&list, c.rim, ejecta, |ux, uy| p.base_surface(ux, uy));
                }
                planet.relief = Some(rel);
            }
        }
        // What lies under each volcano, now the surface is final.
        if planet.relief.as_ref().is_some_and(|r| r.features.iter().any(|f| f.kind == crate::relief::Kind::Volcano)) {
            let magma = planet.mats.id("magma")?;
            let dike = planet.mats.id("dolerite")?;
            let mut rel = planet.relief.take().unwrap();
            {
                let p = &planet;
                rel.build_plumbing(radius, magma, dike, MAGMA_K, |ux, uy| p.base_surface(ux, uy));
            }
            planet.relief = Some(rel);
        }
        // Seas fill the hollows last, so they lie flat over everything.
        if planet.relief.as_ref().is_some_and(|r| r.features.iter().any(|f| f.sea > 0.0)) {
            let mut rel = planet.relief.take().unwrap();
            {
                let p = &planet;
                rel.flood(radius, |ux, uy| p.base_surface(ux, uy));
            }
            planet.relief = Some(rel);
        }
        if let Some(rel) = &planet.relief {
            planet.max_radius += rel.max_up().max(0.0);
        }
        // What this body is known for: rolled from its seed, applied to its own
        // copy of the materials and geology rules.
        if let Some(file) = planet.mats.specialties.clone() {
            let can = planet.possible_materials();
            let picked = crate::specialty::roll(&file, &planet.mats, &can, seed ^ 0x5BEC_1A11, &planet.spec.archetype, &planet.spec.bias);
            let mut mats = planet.mats.clone();
            mats.pristine = Some(std::sync::Arc::new(planet.mats.clone()));
            let (rolled, pods) = {
                let p = &planet;
                crate::specialty::apply(&file, &picked, &mut mats, &can, seed ^ 0x5BEC_1A12, |l, ps| {
                    let host: Vec<MatId> = l.host.iter().filter_map(|n| p.mats.id(n).ok()).collect();
                    for k in 0..40 {
                        let (ux, uy) = crate::geology::random_dir(ps, k);
                        let f = l.r[0] + (l.r[1] - l.r[0]) * unit(hash(ps, k, 1, 7));
                        let r = p.surface_radius(ux, uy) * f;
                        let (x, y) = (ux * r, uy * r);
                        let m = p.base_material(x.floor() as i64, y.floor() as i64);
                        let ok = if host.is_empty() { matches!(p.mats.get(m).state, crate::materials::State::Solid | crate::materials::State::Powder) } else { host.contains(&m) };
                        if ok {
                            return Some((x, y));
                        }
                    }
                    None
                })
            };
            planet.mats = mats;
            planet.specialties = rolled;
            planet.pods = pods;
        }
        // Life last, over the finished ground.
        if let (Some(scale), Some(rules)) = (planet.spec.life, planet.mats.life.clone()) {
            if scale > 0.0 {
                let mut f = crate::flora::Flora::survey(&planet, &rules, scale)?;
                for (i, sp) in f.species.iter_mut().enumerate() {
                    let mut ids = [0; 4];
                    for (k, part) in ["stem", "foliage", "organ", "root"].iter().enumerate() {
                        let (t, c) = sp.parts[k];
                        ids[k] = planet.mats.add_copy(t, format!("flora {i} {part}"), c);
                        planet.palette.colours.push(c);
                    }
                    sp.stem_m = ids[0];
                    sp.foliage_m = ids[1];
                    sp.organ_m = ids[2];
                    sp.root_m = ids[3];
                }
                planet.flora = Some(f);
                planet.max_radius += crate::flora::REACH;
            }
        }
        Ok(planet)
    }

    /// Real gravity and pressure through the body. The cross-section stands for
    /// a sphere built from the same layers, so mass grows with r³: each shell
    /// adds 4πr²ρ dr, gravity at r is GM(r)/r², and pressure is the weight of
    /// everything above, the integral of ρg from the surface down. Only the
    /// base layers count; blobs and veins are too small to matter.
    fn build_profile(&mut self) {
        let n = PROFILE_N;
        let dr = self.radius / n as f64;
        let density = |f: f64| {
            let l = self.layers.iter().find(|l| f <= l.outer).or(self.layers.last()).unwrap();
            self.mats.get(l.material).density
        };
        let mut g = vec![0.0; n + 1];
        let mut m = 0.0;
        for i in 1..=n {
            let r = i as f64 * dr;
            let rho = density((i as f64 - 0.5) / n as f64);
            // Shell from r - dr to r, by its exact volume.
            let r0 = r - dr;
            m += 4.0 / 3.0 * std::f64::consts::PI * (r * r * r - r0 * r0 * r0) * rho;
            g[i] = G_NEWTON * m / (r * r);
        }
        let mut p = vec![0.0; n + 1];
        for i in (0..n).rev() {
            let rho = density((i as f64 + 0.5) / n as f64);
            p[i] = p[i + 1] + rho * 0.5 * (g[i] + g[i + 1]) * dr;
        }
        self.profile_g = g;
        self.profile_p = p;
    }

    fn profile_at(&self, table: &[f64], x: i64, y: i64) -> f64 {
        let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
        let f = ((fx * fx + fy * fy).sqrt() / self.radius * PROFILE_N as f64).clamp(0.0, PROFILE_N as f64);
        let i = (f.floor() as usize).min(PROFILE_N - 1);
        let t = f - i as f64;
        table[i] * (1.0 - t) + table[i + 1] * t
    }

    /// Pressure at cell (x, y) from the weight of the body above it, in pascals.
    /// Zero at the surface and above.
    pub fn pressure_pa(&self, x: i64, y: i64) -> f64 {
        self.profile_at(&self.profile_p, x, y)
    }

    /// The real pull of gravity at cell (x, y), in m/s² (not the gameplay value).
    pub fn real_gravity_at(&self, x: i64, y: i64) -> f64 {
        self.profile_at(&self.profile_g, x, y)
    }

    /// The real surface gravity of a body this size and makeup, in m/s².
    pub fn real_surface_gravity(&self) -> f64 {
        self.profile_g[PROFILE_N]
    }

    /// Surface radius in the direction (ux, uy), which must have length 1.
    pub fn surface_radius(&self, ux: f64, uy: f64) -> f64 {
        match &self.relief {
            // Open sea lies dead level (the table between its samples
            // would leave it rippled by a fraction of a metre).
            Some(r) if r.sea.is_some() && r.sea_depth(ux, uy) > 0.0 => r.sea.unwrap().1,
            Some(r) => self.base_surface(ux, uy) + r.offset(ux, uy),
            None => self.base_surface(ux, uy),
        }
    }

    /// The plain noise surface, before G5 features shape it.
    pub fn base_surface(&self, ux: f64, uy: f64) -> f64 {
        let k = self.radius / self.spec.shape.wavelength_m;
        let n = fbm(self.seed ^ 0x5u64, ux * k + 100.0, uy * k + 100.0, 4);
        self.radius * (1.0 + self.spec.shape.roughness * (2.0 * n - 1.0))
    }

    /// How strongly a pocket of melt heats (x, y) (1 well inside it, fading
    /// to 0 at 1.3 radii) and to what.
    fn blob_glow(&self, x: f64, y: f64) -> (f64, f64) {
        let bx = ((x + self.bucket_origin) / BUCKET).floor() as i64;
        let by = ((y + self.bucket_origin) / BUCKET).floor() as i64;
        let Some(list) = self.buckets.get(&(bx, by)) else {
            return (0.0, 0.0);
        };
        let (mut g, mut hold) = (0.0f64, 0.0);
        for &i in list {
            let b = &self.blobs[i as usize];
            if b.hold_k == 0.0 {
                continue;
            }
            let (dx, dy) = (x - b.cx, y - b.cy);
            let q = (dx * dx + dy * dy).sqrt() / b.radius;
            let v = if q < 0.7 { 1.0 } else { let e = ((1.3 - q) / 0.6).clamp(0.0, 1.0); e * e };
            if v > g {
                g = v;
                hold = b.hold_k;
            }
        }
        (g, hold)
    }

    fn blob_at(&self, x: f64, y: f64) -> Option<MatId> {
        let bx = ((x + self.bucket_origin) / BUCKET).floor() as i64;
        let by = ((y + self.bucket_origin) / BUCKET).floor() as i64;
        let Some(list) = self.buckets.get(&(bx, by)) else {
            return None;
        };
        // Later blobs in the spec win where blobs overlap, so the order is fixed.
        let mut found = None;
        for &i in list {
            let b = &self.blobs[i as usize];
            let (dx, dy) = (x - b.cx, y - b.cy);
            let d2 = dx * dx + dy * dy;
            let reach = b.radius * 1.3;
            if d2 > reach * reach {
                continue;
            }
            let wobble = 0.7 + 0.6 * fbm(b.seed, x / (b.radius * 0.7), y / (b.radius * 0.7), 2);
            let r = b.radius * wobble;
            if d2 <= r * r {
                found = Some(b.material);
            }
        }
        found
    }

    /// True inside a crater bowl, which is open to space.
    /// The direction to the Sun at game time `t`, if the body has one.
    pub fn sun_dir(&self, t: f64) -> Option<(f64, f64)> {
        let sun = self.spec.sun.as_ref()?;
        let a = (sun.start_deg / 360.0 + t / sun.day_s.max(1.0)) * 2.0 * std::f64::consts::PI;
        let (c, s) = crate::noise::cos_sin(a);
        // Angle measured from +y, counterclockwise.
        Some((-s, c))
    }

    /// What open sky above a surface facing (ux, uy) warms or cools exposed
    /// ground toward at game time `t`: the Sun's equilibrium temperature on
    /// the day side (falling as the fourth root of the Sun's height, as for
    /// any surface that only radiates), cold space on the night side.
    pub fn sky_k(&self, ux: f64, uy: f64, t: f64) -> f64 {
        match (self.sun_dir(t), &self.spec.sun) {
            (Some((sx, sy)), Some(sun)) => {
                let h = ux * sx + uy * sy;
                if h > 0.0 {
                    (sun.day_k * h.sqrt().sqrt()).max(self.space_k)
                } else {
                    self.space_k
                }
            }
            _ => self.space_k,
        }
    }

    /// Whether open cell (x, y) is outside the body (sky), not a void inside it.
    pub fn is_sky(&self, x: i64, y: i64) -> bool {
        let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
        let r = (fx * fx + fy * fy).sqrt().max(1e-9);
        r > self.surface_radius(fx / r, fy / r) || self.in_crater(fx, fy)
    }

    /// How steep the ground is in direction (ux, uy): rise over run across
    /// 4 m of surface.
    pub fn slope_at(&self, ux: f64, uy: f64) -> f64 {
        let s = self.surface_radius(ux, uy);
        let d = 2.0 / s;
        let (ax, ay) = (ux - uy * d, uy + ux * d);
        let (bx, by) = (ux + uy * d, uy - ux * d);
        let (la, lb) = ((ax * ax + ay * ay).sqrt(), (bx * bx + by * by).sqrt());
        (self.surface_radius(ax / la, ay / la) - self.surface_radius(bx / lb, by / lb)).abs() / 4.0
    }

    /// The sea's liquid and the radius of its surface, if the body has one.
    pub fn sea(&self) -> Option<(MatId, f64)> {
        self.relief.as_ref().and_then(|r| r.sea)
    }

    /// Whether cell (x, y) lies where the sea was made: below its surface and
    /// above its floor.
    pub fn in_sea(&self, x: i64, y: i64) -> bool {
        let Some((_, level)) = self.sea() else {
            return false;
        };
        let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
        let r = (fx * fx + fy * fy).sqrt().max(1e-9);
        r < level && level - r < self.relief.as_ref().map_or(0.0, |rel| rel.sea_depth(fx / r, fy / r))
    }

    /// How high liquid `m` from a buried layer is pushed up a hole at (x, y):
    /// the weight of what lies over the layer lifts it to where the column
    /// balances (about nine tenths of the way up through ice for water;
    /// past the surface for magma under denser rock: see the sandbox's
    /// vents). None if `m` isn't a buried layer's liquid. Also gives the
    /// layer's top radius.
    /// The top radius of the buried layer of liquid `m`, if there is one.
    pub fn pool_top(&self, m: MatId) -> Option<f64> {
        self.pools.iter().find(|p| p.0 == m).map(|p| p.1 * self.radius)
    }

    pub fn pressure_head(&self, x: i64, y: i64, m: MatId) -> Option<(f64, f64)> {
        let &(_, top, ratio) = self.pools.iter().find(|p| p.0 == m)?;
        let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
        let r = (fx * fx + fy * fy).sqrt().max(1e-9);
        let surface = self.surface_radius(fx / r, fy / r);
        let top_r = top * self.radius;
        Some((top_r + (surface - top_r) * ratio, top_r))
    }

    /// A radius at or above the ground along (ux, uy): the surface, or lower
    /// where a crater bowl has cut it away (everything between is open).
    pub fn ground_hint(&self, ux: f64, uy: f64) -> f64 {
        let mut r = self.surface_radius(ux, uy);
        // Bowls the ray passes through, deepest first, chained: each one's
        // exit must lie inside open ground already found.
        loop {
            let mut moved = false;
            for c in &self.craters {
                let b = ux * c.cx + uy * c.cy;
                let disc = b * b - (c.cx * c.cx + c.cy * c.cy - c.radius * c.radius);
                if disc <= 0.0 {
                    continue;
                }
                let q = disc.sqrt();
                let (lo, hi) = (b - q, b + q);
                // The ray is inside this bowl from hi down to lo; if r lies in
                // that span, everything down to lo is open.
                // (Only ever downward: with r just set to lo + 1, rounding
                // could make lo < r - 1 true again for ever.)
                if hi >= r - 1.0 && lo + 1.0 < r {
                    r = lo + 1.0;
                    moved = true;
                }
            }
            if !moved {
                return r;
            }
        }
    }

    /// The smallest distance from the centre any crater bowl reaches (no
    /// point closer is in a crater). Infinite with no craters.
    pub fn crater_floor(&self) -> f64 {
        self.craters.iter().map(|c| (c.cx * c.cx + c.cy * c.cy).sqrt() - c.radius).fold(f64::INFINITY, f64::min)
    }

    /// The crater bowls touching a rectangle (x0, x1, y0, y1), as (centre x,
    /// centre y, radius): for drawing many points in one area.
    pub fn craters_in(&self, x0: f64, x1: f64, y0: f64, y1: f64) -> Vec<(f64, f64, f64)> {
        self.craters.iter().filter(|c| c.cx + c.radius > x0 && c.cx - c.radius < x1 && c.cy + c.radius > y0 && c.cy - c.radius < y1).map(|c| (c.cx, c.cy, c.radius)).collect()
    }

    pub fn in_crater(&self, fx: f64, fy: f64) -> bool {
        self.craters.iter().any(|c| {
            let (dx, dy) = (fx - c.cx, fy - c.cy);
            dx * dx + dy * dy < c.radius * c.radius
        })
    }

    /// Everything up to the single-cell inclusions. Returns either a final answer
    /// (vacuum, the Heart) or the cell's context and its material so far.
    fn stage1(&self, x: i64, y: i64) -> Result<Cell, MatId> {
        let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
        let r2 = fx * fx + fy * fy;
        if r2 > self.max_radius * self.max_radius {
            return Err(VACUUM);
        }
        if self.spec.heart && x == 0 && y == 0 {
            return Err(self.heart);
        }
        let r = r2.sqrt();
        let (ux, uy) = (fx / r, fy / r);
        let surface = self.surface_radius(ux, uy);
        if r > surface {
            return Err(VACUUM);
        }
        let f = r / self.radius;

        // Base layer, with wavy boundaries.
        let mut m = self.layers.last().unwrap().material;
        for (i, l) in self.layers.iter().enumerate() {
            // The wobble can't move a boundary further than this, so far from
            // it the answer is known without computing any noise.
            let reach = l.boundary_noise + 25.0 / self.radius;
            if f < l.outer - reach {
                m = l.material;
                break;
            }
            if f > l.outer + reach {
                continue;
            }
            let wobble = if l.boundary_noise > 0.0 {
                // Kilometre-scale waves plus metre-scale roughness, so a boundary
                // never looks ruled at any zoom.
                l.boundary_noise * (2.0 * fbm(self.seed ^ (i as u64 + 11), fx / 2500.0, fy / 2500.0, 3) - 1.0)
                    + (25.0 / self.radius) * (2.0 * fbm(self.seed ^ (i as u64 + 71), fx / 45.0, fy / 45.0, 3) - 1.0)
            } else {
                0.0
            };
            if f <= l.outer + wobble {
                m = l.material;
                break;
            }
        }

        let base = m;
        // The deep interior: mantle streaks, plumes, slabs, piles, core structure.
        if let Some(d) = &self.deep {
            m = d.apply(fx, fy, r, m);
        }

        // The glamour pass: eye candy over the interior.
        if let Some(gl) = &self.glamour {
            m = gl.apply(fx, fy, r, surface, base, m);
        }

        // Rubble piles: packed boulders in place of the base layer.
        if let Some(rb) = &self.rubble {
            if let Some(b) = rb.at(fx, fy, f) {
                m = b;
            }
        }

        // Metal: the crystal structure of slowly cooled iron.
        if let Some(wm) = &self.widmanstatten {
            m = wm.apply(fx, fy, f, m);
        }

        // Strata over the base layers, shifted by any faults.
        // (Skipped below the deepest strata and faults: nothing there to find.)
        // Layers follow the structural surface: piles laid on top (volcanoes,
        // blankets, fills) and excavations (basins, rifts) don't move them.
        let reference = match &self.relief {
            Some(rel) => surface - rel.unstructural(ux, uy),
            None => surface,
        };
        if let Some(st) = self.strata.as_ref().filter(|_| reference - r <= self.strata_reach) {
            let (dz, gouge) = match &self.faults {
                Some(fs) => fs.shift(fx, fy),
                None => (0.0, None),
            };
            let rough = 0.8 * (2.0 * fbm(self.seed ^ 0x2B0F, fx / 7.0, fy / 7.0, 2) - 1.0);
            if let Some(b) = st.band_at(ux, uy, reference - r - dz + rough, x, y) {
                m = b;
            }
            if let Some(g) = gouge {
                m = g;
            }
        }

        // Rock piled on top: volcano flows, crater blankets, lava and sediment fills.
        if let Some(rel) = &self.relief {
            if let Some(pm) = rel.pile_at(ux, uy, surface - r, x, y) {
                m = pm;
            }
        }

        // Volcano plumbing: chambers, conduits, dikes and sills.
        if let Some(pm) = self.relief.as_ref().and_then(|rel| rel.plumbing_at(fx, fy)) {
            m = pm;
        }

        // Plutons, dikes and ore veins cut through everything before them.
        if let Some(ints) = &self.intrusions {
            m = ints.apply(&self.mats.geology, fx, fy, m);
        }

        // Ore veins in their host rock.
        for v in &self.veins {
            if f >= v.r_min && f <= v.r_max && v.host.contains(&m) && ridge(v.seed, fx * v.freq, fy * v.freq) > 1.0 - v.width {
                m = v.material;
            }
        }

        // Kimberlite pipes rising from deep down to the surface.
        for p in &self.pipes {
            let along = fx * p.ux + fy * p.uy;
            if along > p.r_start {
                let across = (fx * p.uy - fy * p.ux).abs();
                let w = p.half_width * (0.8 + 0.4 * fbm(self.seed ^ 0x99, along / 200.0, 0.5, 2));
                if across < w {
                    m = self.pipe_material;
                }
            }
        }

        if let Some(b) = self.blob_at(fx, fy) {
            m = b;
        }
        Ok(Cell { fx, fy, r, f, surface, m })
    }

    /// Everything after the inclusions: caves, craters and the regolith blanket.
    fn finish(&self, c: &Cell, mut m: MatId) -> MatId {
        if let Some(cv) = &self.caves {
            // Tunnels follow the ridges of a noise field. They thin out and stop
            // near the edges of their depth range (with a wavy edge, never a
            // straight line), and a second, larger noise field leaves some regions
            // solid.
            let jitter = 0.015 * (2.0 * fbm(self.seed ^ 0xED6E, c.fx / 700.0, c.fy / 700.0, 2) - 1.0);
            let fj = c.f + jitter;
            if fj >= cv.r_min && fj <= cv.r_max {
                let edge = ((fj - cv.r_min).min(cv.r_max - fj) / 0.03).min(1.0);
                // Fade tunnels out across the mask's edge, so they pinch shut
                // instead of being cut off along a smooth contour.
                let mask = fbm(self.seed ^ 0x3A5, c.fx * cv.freq * 0.2, c.fy * cv.freq * 0.2, 2);
                let fade = ((mask - 0.34) / 0.16).clamp(0.0, 1.0);
                let open = edge * fade;
                if open > 0.0 {
                    let n = fbm(self.seed ^ 0xCA7E, c.fx * cv.freq, c.fy * cv.freq, 3);
                    let ridge_v = 1.0 - (2.0 * n - 1.0).abs();
                    if ridge_v > cv.threshold + (1.0 - cv.threshold) * (1.0 - open) {
                        m = cv.material;
                    }
                }
            }
        }

        // Caves only where they form: karst in carbonates, tubes in lava.
        if let Some(ck) = &self.cave_kinds {
            if m != VACUUM && ck.open(m, c.fx / c.r, c.fy / c.r, c.surface - c.r) {
                m = VACUUM;
            }
        }

        let near_craters = c.surface - c.r <= self.crater_reach;
        if near_craters && self.in_crater(c.fx, c.fy) {
            return VACUUM;
        }
        // Under each bowl: a lining of impact melt, then shattered rock.
        if near_craters && m != VACUUM && (self.breccia.is_some() || self.melt.is_some()) {
            for cr in &self.craters {
                let (dx, dy) = (c.fx - cr.cx, c.fy - cr.cy);
                let d = (dx * dx + dy * dy).sqrt() - cr.radius;
                if let Some((frac, mm)) = self.melt {
                    // Melt pools on the floor: only the lower part of the bowl.
                    let below = -(dx * cr.cx + dy * cr.cy) / ((cr.cx * cr.cx + cr.cy * cr.cy).sqrt() * (d + cr.radius)).max(1e-9);
                    if d < frac * cr.radius && below > 0.6 {
                        m = mm;
                        break;
                    }
                }
                if let Some((frac, bm)) = self.breccia {
                    if d < frac * cr.radius * 1.2 && d < frac * cr.radius * (0.8 + 0.4 * fbm(self.seed ^ 0xB4EC, c.fx / 30.0, c.fy / 30.0, 2)) {
                        m = bm;
                        break;
                    }
                }
            }
        }
        if let Some(ic) = &self.ice_caps {
            if m != VACUUM {
                if let Some(ice) = ic.at(c.fx, c.fy, c.r, c.surface - c.r) {
                    m = ice;
                }
            }
        }

        // Loose regolith blankets solid ground at the surface.
        // (Boulders lying on it stay bare rock.)
        if m != VACUUM
            && c.surface - c.r < self.spec.regolith_depth_m
            && self.mats.get(m).state == crate::materials::State::Solid
            && !self.relief.as_ref().is_some_and(|rel| rel.in_boulder(c.fx / c.r, c.fy / c.r, c.surface - c.r))
            && self.spec.regolith_max_slope.map_or(true, |max| self.slope_at(c.fx / c.r, c.fy / c.r) < max)
        {
            m = self.regolith;
        }
        m
    }

    /// What occupies cell (x, y).
    /// The body's own temperature at (x, y) before anything disturbs it: a
    /// smooth profile through the layers' temperatures, hottest at the centre.
    pub fn ambient_k(&self, x: i64, y: i64) -> f64 {
        let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
        // An open sea is stirred to one temperature top to bottom: the
        // surface's (colder toward the poles), whatever the rock below holds.
        if self.in_sea(x, y) {
            let mut t = self.spec.layer.last().map_or(3.0, |l| l.temperature_k);
            if self.spec.polar_cooling_k != 0.0 {
                let s = fy / (fx * fx + fy * fy).sqrt().max(1e-9);
                t = (t - self.spec.polar_cooling_k * s * s).max(self.space_k);
            }
            return t;
        }
        // Under a sea, the rock warms with depth below the sea floor, not
        // below the sea's surface; and the sea keeps the top few hundred
        // metres of its floor cool (cliffs and steps on the floor included).
        let rr = (fx * fx + fy * fy).sqrt();
        let lift = match self.relief.as_ref().filter(|r| r.sea.is_some()) {
            Some(rel) if rr > 0.0 => match rel.sea_depth(fx / rr, fy / rr) {
                d if d > 0.0 => d + 400.0,
                _ => 0.0,
            },
            _ => 0.0,
        };
        let f = (rr + lift) / self.radius;
        let layers = &self.spec.layer;
        let mut t = layers.last().map_or(3.0, |l| l.temperature_k);
        let mut prev = (0.0, layers.first().map_or(3.0, |l| l.temperature_k));
        for l in layers {
            if f <= l.outer {
                let span = (l.outer - prev.0).max(1e-9);
                t = prev.1 + (l.temperature_k - prev.1) * ((f - prev.0) / span).clamp(0.0, 1.0);
                break;
            }
            prev = (l.outer, l.temperature_k);
        }
        // Plumes run hot and sinking slabs cold.
        if let Some(d) = &self.deep {
            t = (t + d.heat(fx, fy)).max(self.space_k);
        }
        // Pockets of melt keep themselves and the rock around them hot.
        {
            let (g, hold) = self.blob_glow(fx, fy);
            if g > 0.0 {
                t += (t.max(hold) - t) * g;
            }
        }
        // Live magma under active volcanoes stays molten.
        if let Some(rel) = &self.relief {
            let (g, hold) = rel.magma_glow(fx, fy);
            if g > 0.0 {
                t += (t.max(hold) - t) * g;
            }
        }
        // Poles at the top and bottom are colder near the surface.
        if self.spec.polar_cooling_k != 0.0 {
            let r = f * self.radius;
            let depth = self.radius - r;
            if depth < self.spec.polar_depth_m && r > 0.0 {
                let s = fy / r;
                let reach = 1.0 - depth.max(0.0) / self.spec.polar_depth_m;
                t = (t - self.spec.polar_cooling_k * s * s * reach).max(self.space_k);
            }
        }
        t
    }

    /// The starting temperature of a cell holding material m at (x, y): the
    /// body's own temperature, but a liquid at least a little above freezing.
    pub fn start_k(&self, x: i64, y: i64, m: MatId) -> f64 {
        self.start_k_at(self.ambient_k(x, y), m)
    }

    /// `start_k` given the body's heat `t` at the cell.
    pub fn start_k_at(&self, t: f64, m: MatId) -> f64 {
        let ph = &self.mats.phases[m as usize];
        match ph.down[0] {
            Some(freeze) if self.mats.get(m).state == crate::materials::State::Liquid && t <= freeze.at_k => {
                // A pocket of liquid in rock colder than its freezing point
                // (magma in a cool crust) starts a margin above freezing, but
                // never as far as boiling.
                let mut start = freeze.at_k + self.mats.physics.melt_margin_k;
                if let Some(boil) = ph.up[1] {
                    start = start.min((freeze.at_k + boil.at_k) / 2.0);
                }
                start
            }
            _ => t,
        }
    }

    /// Every material the generator could put somewhere on this body, by id:
    /// what its spec names, and what the geology rules add to those (ore in
    /// the veins of its kinds of pluton, skarn, baked rock, accessory pods),
    /// with glamour and life when they are on. Generous rather than exact: a
    /// material here may turn out to be absent, but one missing from it is
    /// never generated.
    pub fn possible_materials(&self) -> Vec<bool> {
        let mats = &self.mats;
        let n = mats.len();
        let mut text = format!("{:?}", self.spec);
        if self.spec.life.is_some() {
            text += &format!("{:?}", mats.life);
        }
        // What the generator falls back on when a spec leaves it unsaid.
        // (Only where the feature that falls back on them is there.)
        text += r#""vacuum""#;
        if self.spec.regolith_depth_m > 0.0 {
            text += r#""regolith""#;
        }
        if self.spec.craters.is_some() {
            text += r#""impact breccia" "obsidian""#;
        }
        if self.relief.as_ref().is_some_and(|r| r.features.iter().any(|f| f.kind == crate::relief::Kind::Volcano)) {
            text += r#""dolerite" "magma" "granite" "gabbro""#;
        }
        if text.contains("heart: true") {
            text += r#""heart""#;
        }
        let mut can: Vec<bool> = (0..n).map(|m| text.contains(&format!("\"{}\"", mats.get(m as MatId).name))).collect();
        let g = &mats.geology;
        let kinds: Vec<String> = self.spec.intrusion.iter().map(|i| i.kind.clone()).collect();
        let felsic = kinds.iter().any(|k| k == "felsic");
        let mafic = kinds.iter().any(|k| k == "mafic");
        if felsic || mafic {
            if let Some(gg) = g.gangue {
                can[gg as usize] = true;
            }
            for h in &g.hydrothermal {
                if (h.1 == crate::geology::Kind::Felsic && felsic) || (h.1 == crate::geology::Kind::Mafic && mafic) {
                    can[h.0 as usize] = true;
                }
            }
            if g.skarn_hosts.iter().any(|&m| can[m as usize]) {
                for s in &g.skarn {
                    can[s.0 as usize] = true;
                }
            }
            for &(from, to) in &g.contact {
                if can[from as usize] {
                    can[to as usize] = true;
                }
            }
        }
        if mafic {
            for c in &g.cumulate {
                can[c.0 as usize] = true;
            }
            if let Some(nu) = g.nugget {
                can[nu as usize] = true;
            }
        }
        if let Some(gl) = &self.glamour {
            gl.could_add(&mut can);
        }
        for p in &self.pods {
            can[p.material as usize] = true;
        }
        for a in &g.accessory {
            if a.host.iter().any(|&h| can[h as usize]) {
                can[a.material as usize] = true;
            }
        }
        can
    }

    /// Which elements (by their place in the table) this body holds anywhere,
    /// as `possible_materials` judges it.
    pub fn possible_elements(&self) -> Vec<bool> {
        self.element_grades().into_iter().map(|g| g > 0.0).collect()
    }

    /// For each element (by its place in the table), the richest share of it
    /// in any material this body could hold: 0 means none at all, under
    /// `TRACE_GRADE` only traces in common rock, above it an ore of some kind.
    pub fn element_grades(&self) -> Vec<f64> {
        let els = &self.mats.elements;
        let mut out = vec![0.0f64; els.list.len()];
        for (m, ok) in self.possible_materials().into_iter().enumerate() {
            if ok {
                for (s, f) in &self.mats.get(m as MatId).composition {
                    if let Some(i) = els.index(s) {
                        out[i] = out[i].max(*f);
                    }
                }
            }
        }
        out
    }

    /// The jackpot lodes' pods.
    fn lodes(&self, c: &Cell, mut m: MatId) -> MatId {
        for p in &self.pods {
            if p.hit(&self.mats, c.fx, c.fy, m) {
                m = p.material;
            }
        }
        m
    }

    /// Pods of accessory minerals (`[[geology.accessory]]`) in their host rock.
    fn accessory(&self, c: &Cell, mut m: MatId) -> MatId {
        for (k, a) in self.mats.geology.accessory.iter().enumerate() {
            if a.host.contains(&m) && a.hit(hash(self.seed, k as i64, 0, 0xACC5), c.fx, c.fy, c.surface - c.r) {
                m = a.material;
            }
        }
        m
    }

    /// What occupies cell (x, y), flora included.
    pub fn material(&self, x: i64, y: i64) -> MatId {
        let m = self.base_material(x, y);
        match &self.flora {
            Some(f) => f.over(self, x, y, m),
            None => m,
        }
    }

    /// What the ground generator puts at (x, y), before any life.
    pub fn base_material(&self, x: i64, y: i64) -> MatId {
        let c = match self.stage1(x, y) {
            Ok(c) => c,
            Err(m) => return m,
        };
        let mut m = c.m;
        // Single-cell inclusions, such as nuggets.
        for s in &self.sprinkles {
            if c.f >= s.r_min && c.f <= s.r_max && s.host.contains(&m) && unit(hash(s.seed, x, y, 3)) < s.chance * self.mats.geology.keep(s.material) {
                m = s.material;
            }
        }
        m = self.accessory(&c, m);
        m = self.lodes(&c, m);
        self.finish(&c, m)
    }

    /// The odds of each material in cell (x, y): the same as `material`, except
    /// that single-cell inclusions count by their probability instead of a dice
    /// roll. Estimates built from this get rare deposits right on average.
    pub fn material_odds(&self, x: i64, y: i64, out: &mut Vec<(MatId, f64)>) {
        out.clear();
        let c = match self.stage1(x, y) {
            Ok(c) => c,
            Err(m) => {
                out.push((m, 1.0));
                return;
            }
        };
        out.push((c.m, 1.0));
        for s in &self.sprinkles {
            if c.f < s.r_min || c.f > s.r_max {
                continue;
            }
            let n = out.len();
            for i in 0..n {
                let (m, p) = out[i];
                let chance = s.chance * self.mats.geology.keep(s.material);
                if s.host.contains(&m) && p > 0.0 {
                    out[i].1 = p * (1.0 - chance);
                    out.push((s.material, p * chance));
                }
            }
        }
        for e in out.iter_mut() {
            e.0 = self.finish(&c, self.lodes(&c, self.accessory(&c, e.0)));
        }
    }
}
