//! Totals per quadtree node: how many cells of each material a square region holds.
//!
//! A node is (level, ix, iy). Level 0 is a 64 x 64 leaf; each level up doubles the
//! side. Node coordinates count from the corner of the root square, which is
//! centred on the body.
//!
//! - `exact` counts every cell (multithreaded over leaves). Integer counts, so a
//!   parent is always exactly the sum of its four children.
//! - `estimate` samples a fixed, seeded grid of points. It's fast at any size and
//!   is what coarse mining and the planet-wide readouts use until cells are
//!   actually touched.

use crate::materials::MaterialTable;
use crate::noise::{hash, unit};
use crate::planet::Planet;
use rayon::prelude::*;
use std::collections::BTreeMap;

pub const LEAF: i64 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Node {
    pub level: u32,
    pub ix: i64,
    pub iy: i64,
}

impl Node {
    pub fn side(&self) -> i64 {
        LEAF << self.level
    }
    pub fn children(&self) -> [Node; 4] {
        let l = self.level - 1;
        let (x, y) = (self.ix * 2, self.iy * 2);
        [
            Node { level: l, ix: x, iy: y },
            Node { level: l, ix: x + 1, iy: y },
            Node { level: l, ix: x, iy: y + 1 },
            Node { level: l, ix: x + 1, iy: y + 1 },
        ]
    }
}

/// The quadtree layout for one body.
pub struct Tree {
    /// Level of the root node.
    pub root_level: u32,
    /// World cell coordinate of the root's lower-left corner.
    pub origin: i64,
}

impl Tree {
    pub fn for_planet(p: &Planet) -> Tree {
        let need = (2.0 * p.max_radius).ceil() as i64 + 2;
        let mut level = 0;
        while (LEAF << level) < need {
            level += 1;
        }
        Tree { root_level: level, origin: -(LEAF << level) / 2 }
    }
    pub fn root(&self) -> Node {
        Node { level: self.root_level, ix: 0, iy: 0 }
    }
    /// World-cell bounds [x0, x1) x [y0, y1).
    pub fn bounds(&self, n: Node) -> (i64, i64, i64, i64) {
        let s = n.side();
        let (x0, y0) = (self.origin + n.ix * s, self.origin + n.iy * s);
        (x0, x0 + s, y0, y0 + s)
    }
    /// Total quadtree depth, counting the leaf level.
    pub fn depth(&self) -> u32 {
        self.root_level + 1
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Totals {
    /// Cells of each material, indexed by material id (fractional for estimates).
    pub cells: Vec<f64>,
}

impl Totals {
    pub fn zero(n: usize) -> Self {
        Totals { cells: vec![0.0; n] }
    }
    pub fn add(&mut self, o: &Totals) {
        for (a, b) in self.cells.iter_mut().zip(&o.cells) {
            *a += b;
        }
    }
    /// Physical kilograms per material.
    pub fn kg(&self, mats: &MaterialTable) -> Vec<f64> {
        self.cells.iter().enumerate().map(|(i, c)| c * mats.get(i as crate::materials::MatId).density).collect()
    }
    pub fn total_kg(&self, mats: &MaterialTable) -> f64 {
        self.kg(mats).iter().sum()
    }
    /// Kilograms per element, from each material's composition.
    pub fn elements(&self, mats: &MaterialTable) -> BTreeMap<String, f64> {
        let mut out = BTreeMap::new();
        for (i, kg) in self.kg(mats).iter().enumerate() {
            for (el, frac) in &mats.get(i as crate::materials::MatId).composition {
                *out.entry(el.clone()).or_insert(0.0) += kg * frac;
            }
        }
        out
    }
}

fn box_misses_body(p: &Planet, x0: i64, x1: i64, y0: i64, y1: i64) -> bool {
    // Distance from the centre to the nearest point of the box.
    let nx = 0i64.clamp(x0, x1 - 1) as f64;
    let ny = 0i64.clamp(y0, y1 - 1) as f64;
    nx * nx + ny * ny > (p.max_radius + 2.0) * (p.max_radius + 2.0)
}

/// Count every cell in the node.
pub fn exact(p: &Planet, t: &Tree, n: Node) -> Totals {
    let nm = p.mats.len();
    let (x0, x1, y0, y1) = t.bounds(n);
    if box_misses_body(p, x0, x1, y0, y1) {
        // All vacuum: count it as such, so a parent always equals its children.
        let mut z = Totals::zero(nm);
        z.cells[0] = (n.side() * n.side()) as f64;
        return z;
    }
    let leaves_per_side = n.side() / LEAF;
    let counts: Vec<u64> = (0..leaves_per_side * leaves_per_side)
        .into_par_iter()
        .map(|li| {
            let (lx, ly) = (x0 + (li % leaves_per_side) * LEAF, y0 + (li / leaves_per_side) * LEAF);
            let mut c = vec![0u64; nm];
            if box_misses_body(p, lx, lx + LEAF, ly, ly + LEAF) {
                c[0] = (LEAF * LEAF) as u64;
                return c;
            }
            for y in ly..ly + LEAF {
                for x in lx..lx + LEAF {
                    c[p.material(x, y) as usize] += 1;
                }
            }
            c
        })
        .reduce(
            || vec![0u64; nm],
            |mut a, b| {
                for (x, y) in a.iter_mut().zip(b) {
                    *x += y;
                }
                a
            },
        );
    let mut cells: Vec<f64> = counts.iter().map(|&c| c as f64).collect();
    // Fully empty leaves were counted as vacuum above; keep the vacuum count
    // consistent with the node's area.
    cells[0] = (n.side() * n.side()) as f64 - cells[1..].iter().sum::<f64>();
    Totals { cells }
}

/// Estimate the node's totals from k x k seeded sample points, one per stratum.
/// Rare single-cell inclusions count by their odds, not by whether a sample
/// happens to land on one.
pub fn estimate(p: &Planet, t: &Tree, n: Node, k: i64) -> Totals {
    let nm = p.mats.len();
    let (x0, x1, y0, y1) = t.bounds(n);
    if box_misses_body(p, x0, x1, y0, y1) {
        return Totals::zero(nm);
    }
    let s = n.side() as f64 / k as f64;
    // Weights are added as fixed-point integers (2^-32 units), so the sum is the
    // same however the threads split the work. Floating-point sums are not.
    const ONE: f64 = 4_294_967_296.0;
    let counts: Vec<u128> = (0..k * k)
        .into_par_iter()
        .fold(
            || (vec![0u128; nm], Vec::with_capacity(4)),
            |(mut c, mut odds), i| {
                let (sx, sy) = (i % k, i / k);
                let jx = unit(hash(p.spec.seed ^ 0xE57, sx, sy, n.level as u64));
                let jy = unit(hash(p.spec.seed ^ 0xE58, sx, sy, n.level as u64));
                let x = x0 + ((sx as f64 + jx) * s).floor() as i64;
                let y = y0 + ((sy as f64 + jy) * s).floor() as i64;
                p.material_odds(x.min(x1 - 1), y.min(y1 - 1), &mut odds);
                for &(m, w) in &odds {
                    c[m as usize] += (w * ONE).round() as u128;
                }
                (c, odds)
            },
        )
        .map(|(c, _)| c)
        .reduce(
            || vec![0u128; nm],
            |mut a, b| {
                for (x, y) in a.iter_mut().zip(b) {
                    *x += y;
                }
                a
            },
        );
    let per = (n.side() * n.side()) as f64 / (k * k) as f64;
    Totals { cells: counts.iter().map(|&c| c as f64 / ONE * per).collect() }
}
