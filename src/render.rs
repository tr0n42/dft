//! PNG previews of a body: the whole cross-section, and close-ups at 1 m per cell.
//! Underground cells draw darker the deeper they are; hot liquids and the Heart glow.

use crate::materials::{MatId, State, VACUUM};
use crate::noise::{hash, unit};
use crate::planet::Planet;
use rayon::prelude::*;

const SPACE: [u8; 3] = [6, 7, 12];

fn emissive(p: &Planet, m: MatId) -> bool {
    let mat = p.mats.get(m);
    (mat.state == State::Liquid && mat.name != "water") || mat.name == "heart"
}

fn shade(p: &Planet, m: MatId, depth: f64) -> [u8; 3] {
    let c = p.palette.colours[m as usize];
    if emissive(p, m) {
        return c;
    }
    let f = if depth <= 0.0 {
        1.0
    } else if depth < 20.0 {
        1.0 - 0.3 * depth / 20.0
    } else {
        0.7 - 0.25 * (depth / p.radius).min(1.0)
    };
    [(c[0] as f64 * f) as u8, (c[1] as f64 * f) as u8, (c[2] as f64 * f) as u8]
}

/// Anything that can say what occupies a cell: the bare generator, or a mined world.
pub type Cells<'a> = &'a (dyn Fn(i64, i64) -> MatId + Sync);

fn pixel(p: &Planet, cells: Cells, x: i64, y: i64) -> [u8; 3] {
    colour(p, cells(x, y), x, y)
}

/// The colour of cell (x, y) holding material m, shaded by depth, with the
/// atmosphere band and stars drawn in vacuum.
pub fn colour(p: &Planet, m: MatId, x: i64, y: i64) -> [u8; 3] {
    let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
    let r = (fx * fx + fy * fy).sqrt().max(1e-9);
    colour_at(p, m, x, y, p.surface_radius(fx / r, fy / r))
}

/// The same, with the ground's radius in that direction already known (a
/// renderer drawing many cells can look it up from a table).
pub fn colour_at(p: &Planet, m: MatId, x: i64, y: i64, surface: f64) -> [u8; 3] {
    let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
    let r = (fx * fx + fy * fy).sqrt().max(1e-9);
    if m != VACUUM {
        return shade(p, m, surface - r);
    }
    // Atmosphere band, fading with height; open voids underground stay dark.
    if r < surface && !p.in_crater(fx, fy) {
        return [14, 13, 16];
    }
    let h = (r - surface).max(0.0);
    if p.spec.atmosphere_m > 0.0 && h < p.spec.atmosphere_m {
        let a = 0.55 * (1.0 - h / p.spec.atmosphere_m);
        let c = p.palette.atmosphere;
        let mix = |s: u8, t: u8| (s as f64 * (1.0 - a) + t as f64 * a) as u8;
        return [mix(SPACE[0], c[0]), mix(SPACE[1], c[1]), mix(SPACE[2], c[2])];
    }
    if unit(hash(p.spec.seed ^ 0x57A2, x / 7, y / 7, 0)) < 0.0015 {
        return [200, 200, 215];
    }
    SPACE
}

/// Open space at (x, y): black with the odd star. Used for ground the swarm
/// has eaten away, so mined-out space reads as sky, not as a buried cave.
pub fn space(p: &Planet, x: i64, y: i64) -> [u8; 3] {
    if unit(hash(p.spec.seed ^ 0x57A2, x / 7, y / 7, 0)) < 0.0015 {
        return [200, 200, 215];
    }
    SPACE
}

pub fn write_png(path: &str, w: u32, h: u32, rgb: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().map_err(|e| e.to_string())?;
    wr.write_image_data(rgb).map_err(|e| e.to_string())
}

/// The whole body in a square image `size` pixels wide, one sample per pixel.
pub fn whole(p: &Planet, size: u32) -> Vec<u8> {
    whole_with(p, &|x, y| p.material(x, y), size)
}

/// The whole body as `cells` describes it.
pub fn whole_with(p: &Planet, cells: Cells, size: u32) -> Vec<u8> {
    let span = 2.0 * (p.max_radius + p.spec.atmosphere_m) * 1.06;
    let mpp = span / size as f64;
    let mut img = vec![0u8; (size * size * 3) as usize];
    img.par_chunks_mut((size * 3) as usize).enumerate().for_each(|(row, line)| {
        let wy = (span / 2.0 - (row as f64 + 0.5) * mpp).floor() as i64;
        for col in 0..size as usize {
            let wx = (-span / 2.0 + (col as f64 + 0.5) * mpp).floor() as i64;
            line[col * 3..col * 3 + 3].copy_from_slice(&pixel(p, cells, wx, wy));
        }
    });
    // The Heart is a single cell; mark it so it shows at this scale.
    if p.spec.heart {
        let c = (size / 2) as i64;
        for dy in -3i64..=3 {
            for dx in -3i64..=3 {
                if dx * dx + dy * dy <= 9 {
                    let i = (((c + dy) * size as i64 + (c + dx)) * 3) as usize;
                    img[i..i + 3].copy_from_slice(&p.mats.get(p.mats.id("heart").unwrap()).colour);
                }
            }
        }
    }
    img
}

/// A close-up: `w` x `h` cells starting at world cell (x0, y_top) going right and
/// down, drawn at `scale` pixels per cell.
pub fn region(p: &Planet, x0: i64, y_top: i64, w: u32, h: u32, scale: u32) -> Vec<u8> {
    region_with(p, &|x, y| p.material(x, y), x0, y_top, w, h, scale)
}

/// A close-up as `cells` describes it.
pub fn region_with(p: &Planet, cells: Cells, x0: i64, y_top: i64, w: u32, h: u32, scale: u32) -> Vec<u8> {
    let (pw, ph) = (w * scale, h * scale);
    let mut img = vec![0u8; (pw * ph * 3) as usize];
    img.par_chunks_mut((pw * 3) as usize).enumerate().for_each(|(row, line)| {
        let wy = y_top - (row as u32 / scale) as i64;
        for col in 0..pw as usize {
            let wx = x0 + (col as u32 / scale) as i64;
            line[col * 3..col * 3 + 3].copy_from_slice(&pixel(p, cells, wx, wy));
        }
    });
    img
}

/// A band of the body unrolled: columns run through angles `a0`..`a1`
/// (radians from straight up, clockwise), rows from radius `r_top` down to
/// `r_bottom`. With a full turn and a thin band it shows the whole surface;
/// with a narrow wedge from the surface to the centre, everything a drone
/// would dig through on the way down.
pub fn strip(p: &Planet, cells: Cells, a0: f64, a1: f64, r_top: f64, r_bottom: f64, w: u32, h: u32) -> Vec<u8> {
    let mut img = vec![0u8; (w * h * 3) as usize];
    img.par_chunks_mut((w * 3) as usize).enumerate().for_each(|(row, line)| {
        let r = r_top - (row as f64 + 0.5) * (r_top - r_bottom) / h as f64;
        for col in 0..w as usize {
            let a = a0 + (a1 - a0) * (col as f64 + 0.5) / w as f64;
            let (x, y) = ((a.sin() * r).floor() as i64, (a.cos() * r).floor() as i64);
            line[col * 3..col * 3 + 3].copy_from_slice(&pixel(p, cells, x, y));
        }
    });
    img
}
