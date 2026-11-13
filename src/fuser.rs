//! The fuser: a refinery module that fuses two atoms into one heavier one.
//! The atomic numbers add up (lead 82 + neon 10 = uranium 92) and mass is
//! kept: kilograms in = kilograms out, the two taken in the ratio of their
//! atomic masses. It costs a lot of energy per kilogram.
//!
//! It works from the refined stockpile and keeps each element in `keep`
//! topped up (for now: uranium, for sappers' charges, which nothing else can
//! make). When an input is short it fuses that too, one step down: the
//! heavy partner is always rare, so it is usually made first from common
//! atoms (tellurium from iron + iron and zirconium from calcium + calcium,
//! then uranium from tellurium + zirconium). Of the ways to make an element
//! it takes the one the stockpile can make the most of.

use crate::elements::ElementTable;
use crate::recipes::Stock;
use serde::Deserialize;
use std::collections::BTreeMap;

/// One fusion: `make` from the two elements in `from`.
#[derive(Debug, Clone, Deserialize)]
pub struct Fusion {
    pub make: String,
    pub from: [String; 2],
}

/// `[fuser]` in data/economy.toml. No fusions: no fuser.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct FuserSpec {
    /// Kilograms of what it keeps (uranium) it fuses each second (a step
    /// down, such as the neon for it, comes on top).
    pub kg_per_s: f64,
    /// Energy for every kilogram fused (E).
    pub energy_per_kg: f64,
    /// It only spends energy above this, on top of the swarm's reserve.
    pub spare_energy: f64,
    /// Kilograms of each refined element it keeps in stock.
    pub keep: BTreeMap<String, f64>,
    pub fuse: Vec<Fusion>,
}

/// Steps down it will fuse an input that is short (uranium from tellurium
/// and zirconium, those from iron + iron and calcium + calcium).
pub const DEPTH: u32 = 2;

impl FuserSpec {
    /// Every element named is in the table, and the atomic numbers add up.
    pub fn check(&self, els: &ElementTable) -> Result<(), String> {
        for f in &self.fuse {
            let z = |s: &str| atomic_number(els, s).ok_or(format!("fuser: no element {s}"));
            let (m, a, b) = (z(&f.make)?, z(&f.from[0])?, z(&f.from[1])?);
            if a + b != m {
                return Err(format!("fuser: {} ({a}) + {} ({b}) is not {} ({m})", f.from[0], f.from[1], f.make));
            }
        }
        for s in self.keep.keys() {
            els.index(s).ok_or(format!("fuser: no element {s}"))?;
        }
        Ok(())
    }

    /// The share of fusion `r`'s output each input makes up (by mass; one
    /// entry when both atoms are the same element).
    fn shares(&self, els: &ElementTable, r: usize) -> Vec<(usize, f64)> {
        let f = &self.fuse[r];
        let (a, b) = (els.index(&f.from[0]).unwrap(), els.index(&f.from[1]).unwrap());
        if a == b {
            return vec![(a, 1.0)];
        }
        let (ma, mb) = (els.list[a].mass, els.list[b].mass);
        vec![(a, ma / (ma + mb)), (b, mb / (ma + mb))]
    }

    /// The most of element `el` the refined stock could be fused into, and
    /// the fusion that makes that most (None: no way).
    pub fn makeable(&self, els: &ElementTable, refined: &[f64], el: usize, depth: u32) -> (f64, Option<usize>) {
        let mut best = (0.0, None);
        if depth == 0 {
            return best;
        }
        for (r, f) in self.fuse.iter().enumerate() {
            if els.index(&f.make) != Some(el) {
                continue;
            }
            let kg = self
                .shares(els, r)
                .iter()
                .map(|&(i, share)| (refined[i] + self.makeable(els, refined, i, depth - 1).0) / share)
                .fold(f64::INFINITY, f64::min);
            if kg > best.0 + 1e-12 {
                best = (kg, Some(r));
            }
        }
        best
    }

    /// Fuse up to `kg` of element `el` into the refined stock, fusing short
    /// inputs on the way. Adds what each fusion made to `made`. Returns the
    /// kilograms fused in all (every step).
    pub fn fuse(&self, els: &ElementTable, stock: &mut Stock, made: &mut [f64], el: usize, kg: f64, depth: u32) -> f64 {
        let (can, Some(r)) = self.makeable(els, &stock.refined, el, depth) else { return 0.0 };
        let kg = kg.min(can);
        if kg <= 1e-9 {
            return 0.0;
        }
        let mut steps = 0.0;
        let mut out = 0.0;
        for (i, share) in self.shares(els, r) {
            let need = kg * share;
            let short = need - stock.refined[i];
            if short > 1e-12 {
                steps += self.fuse(els, stock, made, i, short, depth - 1);
            }
            let t = need.min(stock.refined[i]).max(0.0);
            stock.kg[i] = (stock.kg[i] - t).max(0.0);
            stock.refined[i] = (stock.refined[i] - t).max(0.0).min(stock.kg[i]);
            out += t;
        }
        stock.kg[el] += out;
        stock.refined[el] += out;
        made[r] += out;
        steps + out
    }
}

/// An element's atomic number, from its place in the table (hydrogen to
/// barium, hafnium to radium, then thorium and uranium).
pub fn atomic_number(els: &ElementTable, symbol: &str) -> Option<u32> {
    let i = els.index(symbol)? as u32 + 1;
    Some(match symbol {
        "Th" => 90,
        "U" => 92,
        _ if i <= 56 => i,
        _ => i + 15,
    })
}

/// What the fuser has done: kilograms out of each fusion, and energy spent.
#[derive(Debug, Clone, Default)]
pub struct Fuser {
    pub made: Vec<f64>,
    pub energy: f64,
}

impl Fuser {
    pub fn new(spec: &FuserSpec) -> Fuser {
        Fuser { made: vec![0.0; spec.fuse.len()], energy: 0.0 }
    }

    /// Fuse for `dt` seconds: top up each element in `keep`, from energy
    /// above `available` allows. Returns the energy spent.
    pub fn work(&mut self, spec: &FuserSpec, els: &ElementTable, stock: &mut Stock, dt: f64, available: f64) -> f64 {
        if spec.fuse.is_empty() || spec.energy_per_kg <= 0.0 {
            return 0.0;
        }
        if self.made.len() != spec.fuse.len() {
            self.made.resize(spec.fuse.len(), 0.0);
        }
        let mut budget = spec.kg_per_s * dt;
        let mut spent = 0.0;
        for (s, &keep) in &spec.keep {
            let Some(el) = els.index(s) else { continue };
            let left = (available - spent - spec.spare_energy) / spec.energy_per_kg;
            // (A step down can double the kilograms fused: room for it.)
            let want = (keep - stock.refined[el]).min(budget).min(left / 2.0);
            if want <= 1e-6 {
                continue;
            }
            let before = stock.refined[el];
            let steps = spec.fuse(els, stock, &mut self.made, el, want, DEPTH);
            budget -= stock.refined[el] - before;
            spent += steps * spec.energy_per_kg;
        }
        self.energy += spent;
        spent
    }

    /// For the F1 panel: what each fusion has made.
    pub fn lines(&self, spec: &FuserSpec) -> Vec<String> {
        if spec.fuse.is_empty() {
            return Vec::new();
        }
        let parts: Vec<String> = spec
            .fuse
            .iter()
            .zip(&self.made)
            .filter(|&(_, &kg)| kg > 0.0)
            .map(|(f, kg)| format!("{}+{}→{} {:.1} kg", f.from[0], f.from[1], f.make, kg))
            .collect();
        let made = if parts.is_empty() { "nothing yet".to_string() } else { parts.join(", ") };
        vec![format!("  fuser: {made} · {:.0} E spent", self.energy)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> ElementTable {
        ElementTable::load(&format!("{}/data/elements.toml", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn spec() -> FuserSpec {
        let f = |m: &str, a: &str, b: &str| Fusion { make: m.into(), from: [a.into(), b.into()] };
        FuserSpec {
            kg_per_s: 1.0,
            energy_per_kg: 100.0,
            spare_energy: 0.0,
            keep: [("U".to_string(), 10.0)].into_iter().collect(),
            fuse: vec![f("U", "Pb", "Ne"), f("U", "Ta", "K"), f("Ne", "N", "Li")],
        }
    }

    #[test]
    fn the_data_adds_up() {
        let e = crate::economy::Economy::load(&format!("{}/data/economy.toml", env!("CARGO_MANIFEST_DIR"))).unwrap();
        assert!(!e.fuser.fuse.is_empty());
        e.fuser.check(&table()).unwrap();
    }

    #[test]
    fn atomic_numbers() {
        let els = table();
        for (s, z) in [("H", 1), ("Ba", 56), ("Hf", 72), ("Pb", 82), ("Ra", 88), ("Th", 90), ("U", 92)] {
            assert_eq!(atomic_number(&els, s), Some(z), "{s}");
        }
        spec().check(&els).unwrap();
    }

    #[test]
    fn lead_and_neon_made_from_nitrogen_and_lithium() {
        let els = table();
        let sp = spec();
        let (u, pb, n, li) = (els.index("U").unwrap(), els.index("Pb").unwrap(), els.index("N").unwrap(), els.index("Li").unwrap());
        let mut s = Stock::new(&els);
        for (i, kg) in [(pb, 100.0), (n, 100.0), (li, 100.0)] {
            s.kg[i] = kg;
            s.refined[i] = kg;
        }
        let before = s.total();
        let mut f = Fuser::new(&sp);
        // 1 kg of uranium a second, the neon for it fused first (another
        // 0.089 kg fused, and paid for).
        let e = f.work(&sp, &els, &mut s, 1.0, 1e9);
        assert!((s.refined[u] - 1.0).abs() < 1e-9, "{}", s.refined[u]);
        assert!((s.total() - before).abs() < 1e-9, "mass kept");
        assert!((e - 100.0 * (1.0 + 20.18 / 227.38)).abs() < 1e-6, "{e}");
        // Topped up to 10 kg, no further.
        for _ in 0..100 {
            f.work(&sp, &els, &mut s, 1.0, 1e9);
        }
        assert!((s.refined[u] - 10.0).abs() < 1e-6, "{}", s.refined[u]);
        assert!((s.total() - before).abs() < 1e-9, "mass kept");
        // Lead is 207.2 of every 227.4 kg.
        assert!((100.0 - s.kg[pb] - 10.0 * 207.2 / (207.2 + 20.18)).abs() < 0.05, "{}", s.kg[pb]);
        // No energy: nothing.
        s.refined[u] = 0.0;
        assert_eq!(f.work(&sp, &els, &mut s, 1.0, 0.0), 0.0);
    }

    #[test]
    fn uranium_from_iron_and_calcium() {
        let els = table();
        let f = |m: &str, a: &str, b: &str| Fusion { make: m.into(), from: [a.into(), b.into()] };
        let sp = FuserSpec { fuse: vec![f("U", "Te", "Zr"), f("Te", "Fe", "Fe"), f("Zr", "Ca", "Ca")], ..spec() };
        sp.check(&els).unwrap();
        let (u, fe, ca) = (els.index("U").unwrap(), els.index("Fe").unwrap(), els.index("Ca").unwrap());
        let mut s = Stock::new(&els);
        for (i, kg) in [(fe, 100.0), (ca, 100.0)] {
            s.kg[i] = kg;
            s.refined[i] = kg;
        }
        let mut fu = Fuser::new(&sp);
        let e = fu.work(&sp, &els, &mut s, 1.0, 1e9);
        assert!((s.refined[u] - 1.0).abs() < 1e-9, "{}", s.refined[u]);
        assert!((s.total() - 200.0).abs() < 1e-9, "mass kept");
        // Tellurium is 127.6 of every 218.8 kg: that much iron.
        assert!((100.0 - s.kg[fe] - 127.6 / (127.6 + 91.224)).abs() < 1e-3, "{}", s.kg[fe]);
        // Every kilogram fused twice: 200 E.
        assert!((e - 200.0).abs() < 1e-6, "{e}");
    }

    #[test]
    fn takes_the_way_with_the_most() {
        let els = table();
        let sp = spec();
        let (ta, k, pb) = (els.index("Ta").unwrap(), els.index("K").unwrap(), els.index("Pb").unwrap());
        let mut s = Stock::new(&els);
        for (i, kg) in [(ta, 50.0), (k, 50.0), (pb, 1000.0)] {
            s.kg[i] = kg;
            s.refined[i] = kg;
        }
        // No neon or anything to make it from: tantalum and potassium.
        let mut f = Fuser::new(&sp);
        f.work(&sp, &els, &mut s, 1.0, 1e9);
        assert!(f.made[0] == 0.0 && f.made[1] > 0.99, "{:?}", f.made);
    }
}
