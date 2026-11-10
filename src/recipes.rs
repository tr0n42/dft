//! What drones and haulers are made of: recipes of parts, each part made of
//! one element or another, and the stockpile of elements they are made from.
//!
//! Every element delivered to the mother swarm goes into the stockpile. A
//! part names the elements it can be made of, best first, each with how many
//! kilograms of it stand in for one kilogram of the part (copper wiring, or
//! more of aluminium). Building takes the best there is, then the next, so a
//! world short of one element builds more slowly but never stalls while it
//! has the others. A part with no elements listed takes anything solid in
//! the stockpile, most plentiful first.

use crate::elements::{Class, ElementTable};
use serde::Deserialize;

/// One element a part can be made of, and the kg of it per kg of part
/// ("*": anything solid in the stockpile, most plentiful first).
#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    pub el: String,
    #[serde(default = "one")]
    pub x: f64,
}

fn one() -> f64 {
    1.0
}

#[derive(Debug, Clone, Deserialize)]
pub struct Part {
    pub name: String,
    pub kg: f64,
    /// Best first. Empty: anything solid in the stockpile.
    #[serde(default)]
    pub of: Vec<Choice>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Recipe {
    pub parts: Vec<Part>,
}

impl Recipe {
    /// Kilograms of parts in one.
    pub fn kg(&self) -> f64 {
        self.parts.iter().map(|p| p.kg).sum()
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Recipes {
    pub harvester: Recipe,
    pub hauler: Recipe,
}

/// Elements in the stockpile (kg), by their place in the element table.
/// Everything comes up raw (still bound in the minerals it was dug as);
/// refining frees it as pure element. `refined` is the part of `kg` that is
/// pure (never more than `kg`). Building spends the pure element first.
#[derive(Debug, Clone, Default)]
pub struct Stock {
    pub kg: Vec<f64>,
    pub refined: Vec<f64>,
}

impl Stock {
    pub fn new(elements: &ElementTable) -> Stock {
        Stock { kg: vec![0.0; elements.list.len()], refined: vec![0.0; elements.list.len()] }
    }

    /// Kilograms of element i still raw.
    pub fn raw(&self, i: usize) -> f64 {
        (self.kg[i] - self.refined[i]).max(0.0)
    }

    /// Refine up to `kg` of raw stock, every element in proportion to how
    /// much of it is raw. Returns what was refined.
    pub fn refine(&mut self, kg: f64) -> f64 {
        let raw: f64 = (0..self.kg.len()).map(|i| self.raw(i)).sum();
        if raw <= 1e-9 || kg <= 0.0 {
            return 0.0;
        }
        let f = (kg / raw).min(1.0);
        for i in 0..self.kg.len() {
            let r = self.raw(i);
            self.refined[i] = (self.refined[i] + r * f).min(self.kg[i]);
        }
        raw * f
    }

    /// Take `t` kg of element i out: the pure element first.
    fn spend(&mut self, i: usize, t: f64) {
        self.kg[i] = (self.kg[i] - t).max(0.0);
        self.refined[i] = (self.refined[i] - t).max(0.0).min(self.kg[i]);
    }

    pub fn total(&self) -> f64 {
        self.kg.iter().sum()
    }

    /// Add `kg` of something made of `composition` (element mass fractions).
    pub fn add_composition(&mut self, elements: &ElementTable, composition: &std::collections::BTreeMap<String, f64>, kg: f64) {
        for (s, f) in composition {
            if let Some(i) = elements.index(s) {
                self.kg[i] += f * kg;
            }
        }
    }

    /// What building `r` would take from the stockpile, element by element,
    /// or None if the stockpile can't make it.
    pub fn plan(&self, elements: &ElementTable, r: &Recipe) -> Option<Vec<f64>> {
        let mut take = vec![0.0; self.kg.len()];
        let left = |i: usize, take: &Vec<f64>| (self.kg[i] - take[i]).max(0.0);
        // Anything solid: the most plentiful first (no gases).
        let solid = |take: &Vec<f64>| {
            let mut order: Vec<usize> = (0..self.kg.len())
                .filter(|&i| {
                    let e = &elements.list[i];
                    e.class != Class::Noble && e.symbol != "H" && e.symbol != "N" && e.symbol != "F" && e.symbol != "Cl"
                })
                .collect();
            order.sort_by(|&a, &b| left(b, take).total_cmp(&left(a, take)).then(a.cmp(&b)));
            order
        };
        for p in &r.parts {
            let mut need = p.kg;
            if p.of.is_empty() {
                for i in solid(&take) {
                    if need <= 1e-9 {
                        break;
                    }
                    let u = left(i, &take).min(need);
                    take[i] += u;
                    need -= u;
                }
            } else {
                for c in &p.of {
                    if need <= 1e-9 {
                        break;
                    }
                    let x = c.x.max(1e-9);
                    if c.el == "*" {
                        // Anything solid, at its markup.
                        for i in solid(&take) {
                            if need <= 1e-9 {
                                break;
                            }
                            let u = left(i, &take).min(need * x);
                            take[i] += u;
                            need -= u / x;
                        }
                        continue;
                    }
                    let Some(i) = elements.index(&c.el) else { continue };
                    let u = left(i, &take).min(need * x);
                    take[i] += u;
                    need -= u / x;
                }
            }
            if need > 1e-6 {
                return None;
            }
        }
        Some(take)
    }

    /// Build one of `r` if the stockpile can: take it out. Returns whether it did.
    pub fn build(&mut self, elements: &ElementTable, r: &Recipe) -> bool {
        match self.plan(elements, r) {
            Some(take) => {
                for (i, t) in take.into_iter().enumerate() {
                    if t > 0.0 {
                        self.spend(i, t);
                    }
                }
                true
            }
            None => false,
        }
    }

    /// For each part, the element most of it would be made of now (None if it
    /// can't be made), for the panel.
    pub fn sources(&self, elements: &ElementTable, r: &Recipe) -> Vec<(String, Option<String>)> {
        r.parts
            .iter()
            .map(|p| {
                let one = Recipe { parts: vec![p.clone()] };
                let src = self.plan(elements, &one).map(|take| {
                    let (i, _) = take.iter().enumerate().fold((0, 0.0), |b, (i, &t)| if t > b.1 { (i, t) } else { b });
                    elements.list[i].symbol.clone()
                });
                (p.name.clone(), src)
            })
            .collect()
    }
}

/// Elements carried on their way to the mother swarm (in holds and haulers),
/// as one pool: deliveries draw on it in proportion, losses take from it.
#[derive(Debug, Clone, Default)]
pub struct Transit {
    pub kg: Vec<f64>,
    pub total: f64,
}

impl Transit {
    pub fn new(elements: &ElementTable) -> Transit {
        Transit { kg: vec![0.0; elements.list.len()], total: 0.0 }
    }

    pub fn add_composition(&mut self, elements: &ElementTable, composition: &std::collections::BTreeMap<String, f64>, kg: f64) {
        let mut added = 0.0;
        for (s, f) in composition {
            if let Some(i) = elements.index(s) {
                self.kg[i] += f * kg;
                added += f * kg;
            }
        }
        self.total += added;
    }

    /// Take out `kg` (its share of every element): delivered into `into`, or
    /// lost (None).
    pub fn take(&mut self, kg: f64, into: Option<&mut Stock>) {
        if self.total <= 1e-9 || kg <= 0.0 {
            return;
        }
        let f = (kg / self.total).min(1.0);
        match into {
            Some(s) => {
                for (t, k) in self.kg.iter_mut().zip(s.kg.iter_mut()) {
                    let u = *t * f;
                    *t -= u;
                    *k += u;
                }
            }
            None => {
                for t in self.kg.iter_mut() {
                    *t -= *t * f;
                }
            }
        }
        self.total -= self.total * f;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> ElementTable {
        ElementTable::load(&format!("{}/data/elements.toml", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn substitutes_and_filler() {
        let els = table();
        let r = Recipe {
            parts: vec![
                Part { name: "wiring".into(), kg: 10.0, of: vec![Choice { el: "Cu".into(), x: 1.0 }, Choice { el: "Al".into(), x: 2.0 }] },
                Part { name: "filler".into(), kg: 5.0, of: vec![] },
            ],
        };
        let mut s = Stock::new(&els);
        let (cu, al, o) = (els.index("Cu").unwrap(), els.index("Al").unwrap(), els.index("O").unwrap());
        s.kg[cu] = 4.0;
        s.kg[al] = 12.0;
        s.kg[o] = 100.0;
        // 4 kg of copper, then 6 kg of wiring from 12 kg of aluminium; the
        // filler from oxygen (the most there is).
        let take = s.plan(&els, &r).unwrap();
        assert!((take[cu] - 4.0).abs() < 1e-9 && (take[al] - 12.0).abs() < 1e-9 && (take[o] - 5.0).abs() < 1e-9);
        assert!(s.build(&els, &r));
        assert!(!s.build(&els, &r), "no wiring left");
    }
}
