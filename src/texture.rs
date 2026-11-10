//! Looks for the viewer: textures inside each block, a bevel where blocks
//! meet open space, and the backdrop behind ground the swarm has eaten.
//!
//! Nothing here touches the simulation; it only turns materials into pixels.
use crate::materials::{MatId, MaterialTable, State, VACUUM};
use crate::noise::{hash, unit, value};
use crate::planet::Planet;

/// How a material's blocks are drawn up close.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Ordinary rock: a few stones per block with dark cracks between.
    Rock,
    /// Sediments and foliated rock: fine bands across the block.
    Layered,
    /// Gems and glass: facets that catch the light, the odd sparkle.
    Crystal,
    /// Metals and metal-rich ores: a brushed sheen.
    Metal,
    /// Ices: smooth, pale toward the top, a hairline crack now and then.
    Ice,
    /// Loose powder: speckled grains.
    Grain,
    /// Liquids: soft horizontal swells.
    Liquid,
    /// Gas and vacuum: flat.
    Flat,
}

const CRYSTAL: [&str; 18] = ["crystal", "amethyst", "ruby", "emerald", "diamond", "garnet", "opal", "olivine", "jade", "lapis", "agate", "beryl", "corundum", "glass", "obsidian", "sapphire", "heart", "spodumene"];
const LAYERED: [&str; 14] = ["sandstone", "shale", "limestone", "slate", "schist", "gneiss", "coal", "banded", "marble", "quartzite", "tuff", "gypsum", "aquifer", "majorite"];
const METALS: [&str; 17] = ["Fe", "Ni", "Cu", "Au", "Ag", "Pt", "Pb", "Zn", "Sn", "Ti", "Cr", "Co", "W", "Mo", "Sb", "U", "Hg"];

/// The kind of every material in the table, by id.
/// Whether a material's name says it is an ice, frost or snow (not pumice).
pub fn icy_name(n: &str) -> bool {
    n == "ice" || n.ends_with(" ice") || n.contains("frost") || n.contains("snow")
}

pub fn kinds(mats: &MaterialTable) -> Vec<Kind> {
    mats.list
        .iter()
        .enumerate()
        .map(|(i, m)| {
            if i as MatId == VACUUM {
                return Kind::Flat;
            }
            match m.state {
                State::Liquid => return Kind::Liquid,
                State::Gas => return Kind::Flat,
                State::Powder => return Kind::Grain,
                State::Solid => {}
            }
            let n = m.name.as_str();
            if n.starts_with("flora ") {
                return Kind::Flat;
            }
            if CRYSTAL.iter().any(|k| n.contains(k)) {
                return Kind::Crystal;
            }
            if icy_name(n) {
                return Kind::Ice;
            }
            let metal: f64 = m.composition.iter().filter(|(e, _)| METALS.contains(&e.as_str())).map(|(_, x)| x).sum();
            if metal >= 0.55 {
                return Kind::Metal;
            }
            if LAYERED.iter().any(|k| n.contains(k)) {
                return Kind::Layered;
            }
            Kind::Rock
        })
        .collect()
}

fn scale(c: [u8; 3], f: f64) -> [u8; 3] {
    [(c[0] as f64 * f).clamp(0.0, 255.0) as u8, (c[1] as f64 * f).clamp(0.0, 255.0) as u8, (c[2] as f64 * f).clamp(0.0, 255.0) as u8]
}

/// What surrounds a block: whether each side (up, down, left, right on
/// screen) is open, and whether it is a different material.
#[derive(Clone, Copy, Default)]
pub struct Sides {
    pub open: [bool; 4],
    pub other: [bool; 4],
}

/// One pixel of block (cx, cy) drawn in colour `c` (already shaded), at
/// position (u, v) inside it (0..1, v up), `ppc` pixels per cell across.
pub fn texel(kind: Kind, c: [u8; 3], cx: i64, cy: i64, u: f64, v: f64, ppc: f64, sides: Sides) -> [u8; 3] {
    let h = hash(0x7E7, cx, cy, 1);
    // Every block a touch lighter or darker than its neighbours.
    let mut f = 1.0 + 0.09 * (unit(h) - 0.5) * if kind == Kind::Flat { 0.0 } else { 1.0 };
    if ppc >= 4.0 {
        match kind {
            Kind::Rock => {
                // Stones about half a metre across, continuous from block to
                // block, with dark cracks where two stones meet.
                let (x, y) = ((cx as f64 + u) / 0.7, (cy as f64 + v) / 0.7);
                let (xi, yi) = (x.floor() as i64, y.floor() as i64);
                let (mut d1, mut d2, mut id) = (f64::INFINITY, f64::INFINITY, 0u64);
                for dj in -1..=1 {
                    for di in -1..=1 {
                        let hh = hash(0x7E8, xi + di, yi + dj, 0);
                        let px = (xi + di) as f64 + unit(hh);
                        let py = (yi + dj) as f64 + unit(hash(hh, 1, 0, 0));
                        let d = (px - x) * (px - x) + (py - y) * (py - y);
                        if d < d1 {
                            d2 = d1;
                            d1 = d;
                            id = hh;
                        } else if d < d2 {
                            d2 = d;
                        }
                    }
                }
                f *= 1.0 + 0.16 * (unit(hash(id, 2, 0, 0)) - 0.5);
                if d2.sqrt() - d1.sqrt() < 0.9 / (ppc * 0.7) {
                    f *= 0.78;
                }
            }
            Kind::Layered => {
                // Fine beds running across, continuous from block to block,
                // waving a little.
                let y = cy as f64 + v + 0.25 * value(0x7E9, (cx as f64 + u) / 3.0, 0.5);
                let k = y * 2.6;
                let band = k.floor() as i64;
                f *= 1.0 + 0.16 * (unit(hash(0x7EA, band, 0, 0)) - 0.5);
                if k - k.floor() < 0.8 / (ppc / 2.6) {
                    f *= 0.84;
                }
            }
            Kind::Crystal => {
                // Two facets meeting on a diagonal, one lit, one in shade.
                let flip = unit(hash(h, 1, 0, 0)) < 0.5;
                let lit = if flip { u + v > 1.0 } else { u > v };
                f *= if lit { 1.1 } else { 0.92 };
                // A sparkle.
                let (sx, sy) = (unit(hash(h, 2, 0, 0)), unit(hash(h, 3, 0, 0)));
                if (u - sx).abs() < 0.7 / ppc && (v - sy).abs() < 0.7 / ppc && unit(hash(h, 4, 0, 0)) < 0.12 {
                    return [255, 255, 250];
                }
            }
            Kind::Metal => {
                let t = (u - v + 1.0) / 2.0;
                f *= 0.86 + 0.3 * (1.0 - (2.0 * t - 1.0).abs());
                if ((u + v) * 2.0 + unit(h)).fract() < 1.0 / ppc {
                    f *= 1.15;
                }
            }
            Kind::Ice => {
                f *= 0.94 + 0.12 * v;
                let line = 0.2 + 0.6 * unit(hash(h, 1, 0, 0));
                if unit(hash(h, 2, 0, 0)) < 0.25 && ((u * 0.7 + v * 0.3) - line).abs() < 0.6 / ppc {
                    f *= 1.2;
                }
            }
            Kind::Grain => {
                let n = 5.0;
                let (gi, gj) = (((cx as f64 + u) * n).floor() as i64, ((cy as f64 + v) * n).floor() as i64);
                f *= 1.0 + 0.26 * (unit(hash(0x7EB, gi, gj, 7)) - 0.5);
            }
            Kind::Liquid => {
                let w = v * 2.0 + 0.4 * u + unit(h);
                f *= 1.0 + 0.07 * (1.0 - (2.0 * (w - w.floor()) - 1.0).abs());
            }
            Kind::Flat => {}
        }
        // The bevel: faces open to space catch the light from above and fall
        // into shade below; seams between materials are a hairline darker.
        let e = (1.6 / ppc).max(0.08);
        let near = [1.0 - v < e, v < e, u < e, 1.0 - u < e];
        for s in 0..4 {
            if near[s] {
                if sides.open[s] {
                    f *= [1.3, 0.62, 1.12, 0.78][s];
                } else if sides.other[s] {
                    f *= 0.86;
                }
            }
        }
    }
    scale(c, f)
}

/// The backdrop where there was nothing to begin with.
pub const VOID: [u8; 3] = [10, 10, 16];

/// The size of the backdrop's blocks (it is drawn softer than the ground).
pub const GHOST_M: i64 = 3;

/// The backdrop behind eaten-out ground: a dim, cold ghost of the rock that
/// was in block (gx, gy) (multiples of GHOST_M). Worth caching: it asks the
/// generator.
pub fn ghost(p: &Planet, gx: i64, gy: i64) -> [u8; 3] {
    let m = p.material(gx, gy);
    if m == VACUUM {
        return VOID;
    }
    let c = p.palette.colours[m as usize];
    // Grey it out, darken it, cool it; deeper down, darker.
    let grey = c[0] as f64 * 0.3 + c[1] as f64 * 0.5 + c[2] as f64 * 0.2;
    let mix = |ch: u8| ch as f64 * 0.45 + grey * 0.55;
    let r = ((gx * gx + gy * gy) as f64).sqrt();
    let depth = (p.radius - r).max(0.0) / p.radius;
    let dim = 0.28 - 0.08 * depth;
    [(mix(c[0]) * dim + 6.0) as u8, (mix(c[1]) * dim + 7.0) as u8, (mix(c[2]) * dim + 14.0) as u8]
}

/// The backdrop at point (bx, by) (already moved for parallax), from the
/// ghost colours of the four blocks around it (blended, so the ghost is soft):
/// drill scars running toward the centre, the way the swarm cut in, over a
/// faint mottle.
pub fn backdrop(ghosts: [[u8; 3]; 4], bx: f64, by: f64) -> [u8; 3] {
    // Blend the blocks (corner order: (0,0), (1,0), (0,1), (1,1)).
    let g = GHOST_M as f64;
    let (tx, ty) = ((bx / g - 0.5).rem_euclid(1.0), (by / g - 0.5).rem_euclid(1.0));
    let lerp = |a: u8, b: u8, t: f64| a as f64 + (b as f64 - a as f64) * t;
    let mut c = [0.0; 3];
    for k in 0..3 {
        let top = lerp(ghosts[0][k], ghosts[1][k], tx);
        let bot = lerp(ghosts[2][k], ghosts[3][k], tx);
        c[k] = top + (bot - top) * ty;
    }
    let r = (bx * bx + by * by).sqrt().max(1.0);
    let (ux, uy) = (bx / r, by / r);
    let across = bx * uy - by * ux;
    let groove = value(0x5CA2, across / 2.2, r / 60.0);
    let mut f = if groove > 0.8 { 0.68 } else if groove < 0.1 { 1.2 } else { 1.0 };
    f *= 0.92 + 0.16 * value(0x5CA3, bx / 5.0, by / 5.0);
    [(c[0] * f).clamp(0.0, 255.0) as u8, (c[1] * f).clamp(0.0, 255.0) as u8, (c[2] * f).clamp(0.0, 255.0) as u8]
}
