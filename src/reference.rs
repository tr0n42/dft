//! Reference fingerprints and spec sheets: the realistic models locked in.
//!
//! A body's fingerprint hashes the names of the materials at a fixed set of
//! sample points: scattered through the whole disc and along the surface at
//! several depths. Names rather than ids, so adding materials to the
//! catalogue never moves a fingerprint; only a change to what the generator
//! puts where does.
use crate::planet::Planet;
use std::collections::BTreeMap;

/// The sample points: (x, y) cells. `n_disc` scattered through the disc,
/// `n_dirs` surface directions at each of `depths`.
pub fn sample_points(p: &Planet, n_disc: i64, n_dirs: i64, depths: &[f64]) -> Vec<(i64, i64)> {
    let r = p.max_radius.ceil() as i64 + 1;
    let mut out = Vec::with_capacity((n_disc + n_dirs * depths.len() as i64) as usize);
    for k in 0..n_disc {
        let a = crate::noise::hash(0x5EF, k, 0, 1);
        let b = crate::noise::hash(0x5EF, k, 1, 1);
        out.push(((a % (2 * r as u64)) as i64 - r, (b % (2 * r as u64)) as i64 - r));
    }
    for i in 0..n_dirs {
        let (ux, uy) = crate::geology::random_dir(0x5F0, i);
        let s = p.surface_radius(ux, uy);
        for d in depths {
            let rr = s - d;
            out.push(((ux * rr).floor() as i64, (uy * rr).floor() as i64));
        }
    }
    out
}

fn name_hash(name: &str) -> i64 {
    name.bytes().fold(0xCBF2_9CE4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01B3)) as i64
}

pub const DEPTHS: [f64; 6] = [0.5, 3.0, 20.0, 100.0, 500.0, 2000.0];

/// The body's fingerprint, and the material shares it saw (by name).
pub fn fingerprint(p: &Planet) -> (u64, BTreeMap<String, u32>) {
    let mut h = 0u64;
    let mut seen: BTreeMap<String, u32> = BTreeMap::new();
    for (k, (x, y)) in sample_points(p, 40_000, 3_000, &DEPTHS).into_iter().enumerate() {
        let name = &p.mats.get(p.material(x, y)).name;
        h = crate::noise::hash(h, name_hash(name), k as i64, 0x5F1);
        *seen.entry(name.clone()).or_default() += 1;
    }
    (h, seen)
}

/// Material shares at each depth below the surface (percent, by name).
pub fn depth_profile(p: &Planet, depths: &[f64], n_dirs: i64) -> Vec<(f64, BTreeMap<String, f64>)> {
    depths
        .iter()
        .map(|&d| {
            let mut m: BTreeMap<String, f64> = BTreeMap::new();
            for i in 0..n_dirs {
                let (ux, uy) = crate::geology::random_dir(0x5F2, i);
                let r = p.surface_radius(ux, uy) - d;
                if r <= 0.0 {
                    continue;
                }
                *m.entry(p.mats.get(p.material((ux * r).floor() as i64, (uy * r).floor() as i64)).name.clone()).or_default() += 100.0 / n_dirs as f64;
            }
            (d, m)
        })
        .collect()
}
