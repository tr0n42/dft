//! Metal bodies: the crystal structure of meteoritic iron.
//!
//! Iron-nickel that cooled slowly (a few degrees per million years, deep in a
//! parent body's core) grows as huge parent crystals. Inside each, plates of
//! nickel-poor kamacite form along the crystal's planes, edged by thin
//! nickel-rich taenite, with plessite (a fine mix of the two) filling the
//! corners where plates meet. Cut and etched, that is the Widmanstätten
//! pattern. In a cross-section the planes show as three families of lines.
//!
//! Here: parent crystals are cells of a jittered grid (each with its own
//! orientation); within one, the main family of plates is `spacing_m` apart
//! and the two others wider. Only host materials (the
//! metal core) are restyled.

use crate::materials::{MatId, MaterialTable};
use crate::noise::{fbm, hash, unit};
use crate::spec::Widmanstatten as Spec;

pub struct Widmanstatten {
    host: Vec<MatId>,
    grain: f64,
    spacing: f64,
    edge: f64,
    kamacite: MatId,
    taenite: MatId,
    plessite: MatId,
    r_min: f64,
    r_max: f64,
    seed: u64,
}

/// cos and sin of 60 degrees.
const C60: f64 = 0.5;
const S60: f64 = 0.866_025_403_784_438_6;

impl Widmanstatten {
    pub fn build(s: &Spec, mats: &MaterialTable, seed: u64) -> Result<Self, String> {
        Ok(Widmanstatten {
            host: s.host.iter().map(|h| mats.id(h)).collect::<Result<_, _>>()?,
            grain: s.grain_m.max(4.0),
            spacing: s.spacing_m.max(1.0),
            edge: s.taenite_m.max(0.3),
            kamacite: mats.id(&s.kamacite)?,
            taenite: mats.id(&s.taenite)?,
            plessite: mats.id(&s.plessite)?,
            r_min: s.r_min,
            r_max: s.r_max,
            seed,
        })
    }

    /// The parent crystal at (x, y): its key, from a jittered grid
    /// (the nearest of the nine nearby crystal centres).
    fn crystal(&self, x: f64, y: f64) -> u64 {
        let (gx, gy) = ((x / self.grain).floor() as i64, (y / self.grain).floor() as i64);
        let mut best = (f64::MAX, 0u64);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (cx, cy) = (gx + dx, gy + dy);
                let h = hash(self.seed, cx, cy, 0x3E7A);
                let px = (cx as f64 + 0.15 + 0.7 * unit(h)) * self.grain;
                let py = (cy as f64 + 0.15 + 0.7 * unit(hash(h, 1, 0, 0))) * self.grain;
                let d = (px - x) * (px - x) + (py - y) * (py - y);
                if d < best.0 {
                    best = (d, h);
                }
            }
        }
        best.1
    }

    /// The material at (x, y), at fraction `f` of the radius, over `m`.
    pub fn apply(&self, x: f64, y: f64, f: f64, m: MatId) -> MatId {
        if f < self.r_min || f > self.r_max || !self.host.contains(&m) {
            return m;
        }
        // Parent crystal boundaries wander a little (the plates inside stay
        // straight: they follow the crystal's planes).
        let w = self.grain * 0.12;
        let wx = x + w * (2.0 * fbm(self.seed ^ 0x5A, x / (self.grain * 0.7), y / (self.grain * 0.7), 2) - 1.0);
        let wy = y + w * (2.0 * fbm(self.seed ^ 0x5B, x / (self.grain * 0.7), y / (self.grain * 0.7), 2) - 1.0);
        let h = self.crystal(wx, wy);
        // The crystal's orientation, and its three families of planes.
        let (ox, oy) = crate::geology::random_dir(h, 0);
        let dirs = [(ox, oy), (ox * C60 - oy * S60, ox * S60 + oy * C60), (ox * C60 + oy * S60, -ox * S60 + oy * C60)];
        // The main family is tightest; which one it is varies by crystal.
        let main = (unit(hash(h, 2, 0, 0)) * 3.0) as usize % 3;
        let mut near = 0;
        let mut on_edge = false;
        for (k, (dx, dy)) in dirs.iter().enumerate() {
            let sp = if k == main { self.spacing } else { self.spacing * 3.5 };
            let u = (x * dx + y * dy) / sp + unit(hash(h, 3 + k as i64, 0, 0));
            let fr = u - u.floor();
            // Distance (m) to the nearest plate boundary of this family.
            let d = fr.min(1.0 - fr) * sp;
            if d < self.edge {
                on_edge = true;
                near += 1;
            } else if d < self.edge * 3.0 {
                near += 1;
            }
        }
        if near >= 2 && !on_edge {
            // Where plates of two families crowd together: fine plessite.
            self.plessite
        } else if on_edge {
            self.taenite
        } else {
            self.kamacite
        }
    }
}
