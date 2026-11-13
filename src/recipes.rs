//! What drones and haulers are made of: parts, each part a mix of
//! components, each component made of one element or another; the parts
//! stock; and the stockpile of elements they are all made from.
//!
//! Every element delivered to the mother swarm goes into the stockpile.
//! Elements are made into parts (Swarm Production Design: neural cores,
//! behaviour chips, servos...), which are kept in the parts stock, and units
//! are built from parts. A component names the elements it can be made of,
//! best first, each with how many kilograms of it stand in for one kilogram
//! of the component (copper wiring, or more of aluminium). Making takes the
//! best there is, then the next, so a world short of one element builds
//! more slowly but never stalls while it has the others. A component with
//! no elements listed takes anything solid in the stockpile, most plentiful
//! first.
//!
//! Until fabricator ships exist, parts are made at once: a stock is kept
//! ahead (`Recipes::ahead`) and anything short when a unit is built is made
//! on the spot.

use crate::elements::{Class, ElementTable};
use serde::Deserialize;

/// One element a component can be made of, and the kg of it per kg of
/// component ("*": anything solid in the stockpile, most plentiful first).
#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    pub el: String,
    #[serde(default = "one")]
    pub x: f64,
}

fn one() -> f64 {
    1.0
}

/// A component of a part: so many kg of something made of these elements.
#[derive(Debug, Clone, Deserialize)]
pub struct Component {
    pub name: String,
    pub kg: f64,
    /// Best first. Empty: anything solid in the stockpile.
    #[serde(default)]
    pub of: Vec<Choice>,
}

/// A kind of part (a neural core, a servo...).
#[derive(Debug, Clone, Deserialize)]
pub struct PartKind {
    pub id: String,
    pub name: String,
    /// What it is in the swarm's lore.
    #[serde(default)]
    pub lore: String,
    pub make: Vec<Component>,
}

impl PartKind {
    /// Kilograms in one.
    pub fn kg(&self) -> f64 {
        self.make.iter().map(|c| c.kg).sum()
    }
}

/// So many of one part.
#[derive(Debug, Clone, Deserialize)]
pub struct Need {
    pub part: String,
    pub n: u32,
}

/// A unit, in parts.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Recipe {
    #[serde(default)]
    pub parts: Vec<Need>,
}

/// Parts kept in stock ahead: enough for this many of each unit.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Ahead {
    #[serde(default)]
    pub harvester: u32,
    #[serde(default)]
    pub hauler: u32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Recipes {
    /// The parts catalogue.
    #[serde(default)]
    pub part: Vec<PartKind>,
    #[serde(default)]
    pub ahead: Ahead,
    #[serde(default)]
    pub harvester: Recipe,
    #[serde(default)]
    pub hauler: Recipe,
    /// The specialists (built by the Universal Drone Replicator).
    #[serde(default)]
    pub sapper: Recipe,
    #[serde(default)]
    pub tunneler: Recipe,
    #[serde(default)]
    pub heatsink: Recipe,
    #[serde(default)]
    pub forester: Recipe,
    #[serde(default)]
    pub abductor: Recipe,
}

impl Recipes {
    pub fn index(&self, id: &str) -> Option<usize> {
        self.part.iter().position(|p| p.id == id)
    }

    /// What one of this kind of unit is made of.
    pub fn of(&self, u: crate::production::Unit) -> &Recipe {
        use crate::production::Unit;
        match u {
            Unit::Cutter => &self.harvester,
            Unit::Hauler => &self.hauler,
            Unit::Sapper => &self.sapper,
            Unit::Tunneler => &self.tunneler,
            Unit::HeatSink => &self.heatsink,
            Unit::Forester => &self.forester,
            Unit::Abductor => &self.abductor,
        }
    }

    /// Every unit's parts are in the catalogue.
    pub fn check(&self) -> Result<(), String> {
        for u in crate::production::Unit::ALL {
            let r = self.of(u);
            for n in &r.parts {
                if self.index(&n.part).is_none() {
                    return Err(format!("recipe.{}: no part \"{}\" in the catalogue", u.recipe_key(), n.part));
                }
            }
        }
        Ok(())
    }

    /// How many of each part (by place in the catalogue) one `r` takes.
    pub fn counts(&self, r: &Recipe) -> Vec<u64> {
        let mut c = vec![0u64; self.part.len()];
        for n in &r.parts {
            if let Some(i) = self.index(&n.part) {
                c[i] += n.n as u64;
            }
        }
        c
    }

    /// Kilograms of parts in one `r`.
    pub fn kg(&self, r: &Recipe) -> f64 {
        self.counts(r).iter().zip(&self.part).map(|(n, p)| *n as f64 * p.kg()).sum()
    }
}

/// The parts stock: how many of each part (by place in the catalogue) are
/// on hand, and how many have been made, all told.
#[derive(Debug, Clone, Default)]
pub struct Parts {
    pub n: Vec<u64>,
    pub made: Vec<u64>,
}

impl Parts {
    pub fn new(r: &Recipes) -> Parts {
        Parts { n: vec![0; r.part.len()], made: vec![0; r.part.len()] }
    }

    /// Kilograms of parts on hand.
    pub fn kg(&self, r: &Recipes) -> f64 {
        self.n.iter().zip(&r.part).map(|(n, p)| *n as f64 * p.kg()).sum()
    }

    /// Build one `unit` from the parts stock, first making any parts it is
    /// short of at once from the stockpile. Returns the kg of elements that
    /// took, or None (and nothing changes) if the stockpile can't make them.
    pub fn build(&mut self, stock: &mut Stock, elements: &ElementTable, r: &Recipes, unit: &Recipe) -> Option<f64> {
        let need = r.counts(unit);
        let short: Vec<u64> = need.iter().zip(&self.n).map(|(k, h)| k.saturating_sub(*h)).collect();
        let comps: Vec<(&Component, f64)> = short.iter().enumerate().filter(|(_, s)| **s > 0).flat_map(|(i, s)| r.part[i].make.iter().map(move |c| (c, c.kg * *s as f64))).collect();
        let kg = if comps.is_empty() { 0.0 } else { stock.take(&stock.plan(elements, &comps)?) };
        for i in 0..need.len() {
            self.made[i] += short[i];
            self.n[i] = self.n[i] + short[i] - need[i];
        }
        Some(kg)
    }

    /// Whether one `unit`'s parts are all in the stock.
    pub fn can_claim(&self, r: &Recipes, unit: &Recipe) -> bool {
        r.counts(unit).iter().zip(&self.n).all(|(k, h)| k <= h)
    }

    /// Take one `unit`'s parts out of the stock, if they are all there.
    pub fn claim(&mut self, r: &Recipes, unit: &Recipe) -> bool {
        let need = r.counts(unit);
        if need.iter().zip(&self.n).any(|(k, h)| k > h) {
            return false;
        }
        for (h, k) in self.n.iter_mut().zip(need) {
            *h -= k;
        }
        true
    }

    /// Make parts toward the stock kept ahead, as far as the stockpile
    /// allows. Returns the kg of elements that took.
    pub fn make_ahead(&mut self, stock: &mut Stock, elements: &ElementTable, r: &Recipes) -> f64 {
        let (h, u) = (r.counts(&r.harvester), r.counts(&r.hauler));
        let mut spent = 0.0;
        for i in 0..r.part.len() {
            let want = r.ahead.harvester as u64 * h[i] + r.ahead.hauler as u64 * u[i];
            let mut k = want.saturating_sub(self.n[i]);
            // As many as can be made: all that's short, or half, or...
            while k > 0 {
                let comps: Vec<(&Component, f64)> = r.part[i].make.iter().map(|c| (c, c.kg * k as f64)).collect();
                if let Some(take) = stock.plan(elements, &comps) {
                    spent += stock.take(&take);
                    self.n[i] += k;
                    self.made[i] += k;
                    break;
                }
                k /= 2;
            }
        }
        spent
    }
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

    /// What making `need` (components, and kg of each) would take from the
    /// stockpile, element by element, or None if the stockpile can't.
    pub fn plan(&self, elements: &ElementTable, need: &[(&Component, f64)]) -> Option<Vec<f64>> {
        self.plan_in(&self.kg, elements, need)
    }

    /// The same from the refined (pure) stockpile only: what the fabricator
    /// works from.
    pub fn plan_refined(&self, elements: &ElementTable, need: &[(&Component, f64)]) -> Option<Vec<f64>> {
        self.plan_in(&self.refined, elements, need)
    }

    fn plan_in(&self, have: &[f64], elements: &ElementTable, need: &[(&Component, f64)]) -> Option<Vec<f64>> {
        let mut take = vec![0.0; have.len()];
        let left = |i: usize, take: &Vec<f64>| (have[i] - take[i]).max(0.0);
        // Anything solid: the most plentiful first (no gases).
        let solid = |take: &Vec<f64>| {
            let mut order: Vec<usize> = (0..have.len())
                .filter(|&i| {
                    let e = &elements.list[i];
                    // (Never uranium: it is kept for sappers' charges, which nothing else can make.)
                    e.class != Class::Noble && e.symbol != "H" && e.symbol != "N" && e.symbol != "F" && e.symbol != "Cl" && e.symbol != "U"
                })
                .collect();
            order.sort_by(|&a, &b| left(b, take).total_cmp(&left(a, take)).then(a.cmp(&b)));
            order
        };
        for &(c, kg) in need {
            let mut need = kg;
            if c.of.is_empty() {
                for i in solid(&take) {
                    if need <= 1e-9 {
                        break;
                    }
                    let u = left(i, &take).min(need);
                    take[i] += u;
                    need -= u;
                }
            } else {
                for ch in &c.of {
                    if need <= 1e-9 {
                        break;
                    }
                    let x = ch.x.max(1e-9);
                    if ch.el == "*" {
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
                    let Some(i) = elements.index(&ch.el) else { continue };
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

    /// Take a plan's elements out. Returns the kg taken.
    pub fn take(&mut self, take: &[f64]) -> f64 {
        let mut kg = 0.0;
        for (i, &t) in take.iter().enumerate() {
            if t > 0.0 {
                self.spend(i, t);
                kg += t;
            }
        }
        kg
    }

    /// For each component of a part, the element most of it would be made
    /// of now from the refined stockpile (None if it can't be), for the panel.
    pub fn sources(&self, elements: &ElementTable, p: &PartKind) -> Vec<(String, Option<String>)> {
        p.make
            .iter()
            .map(|c| {
                let src = self.plan_refined(elements, &[(c, c.kg)]).map(|take| {
                    let (i, _) = take.iter().enumerate().fold((0, 0.0), |b, (i, &t)| if t > b.1 { (i, t) } else { b });
                    elements.list[i].symbol.clone()
                });
                (c.name.clone(), src)
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
        let wiring = Component { name: "wiring".into(), kg: 10.0, of: vec![Choice { el: "Cu".into(), x: 1.0 }, Choice { el: "Al".into(), x: 2.0 }] };
        let filler = Component { name: "filler".into(), kg: 5.0, of: vec![] };
        let need = [(&wiring, 10.0), (&filler, 5.0)];
        let mut s = Stock::new(&els);
        let (cu, al, o) = (els.index("Cu").unwrap(), els.index("Al").unwrap(), els.index("O").unwrap());
        s.kg[cu] = 4.0;
        s.kg[al] = 12.0;
        s.kg[o] = 100.0;
        // 4 kg of copper, then 6 kg of wiring from 12 kg of aluminium; the
        // filler from oxygen (the most there is).
        let take = s.plan(&els, &need).unwrap();
        assert!((take[cu] - 4.0).abs() < 1e-9 && (take[al] - 12.0).abs() < 1e-9 && (take[o] - 5.0).abs() < 1e-9);
        assert!((s.take(&take) - 21.0).abs() < 1e-9);
        assert!(s.plan(&els, &need).is_none(), "no wiring left");
    }

    fn recipes() -> Recipes {
        let wire = |kg: f64| Component { name: "wiring".into(), kg, of: vec![Choice { el: "Cu".into(), x: 1.0 }] };
        Recipes {
            part: vec![PartKind { id: "chip".into(), name: "Chip".into(), lore: String::new(), make: vec![wire(1.0)] }, PartKind { id: "coil".into(), name: "Coil".into(), lore: String::new(), make: vec![wire(2.0)] }],
            ahead: Ahead { harvester: 3, hauler: 0 },
            harvester: Recipe { parts: vec![Need { part: "chip".into(), n: 1 }, Need { part: "coil".into(), n: 2 }] },
            hauler: Recipe::default(),
            ..Recipes::default()
        }
    }

    #[test]
    fn units_from_parts() {
        let els = table();
        let r = recipes();
        r.check().unwrap();
        assert!((r.kg(&r.harvester) - 5.0).abs() < 1e-9);
        let cu = els.index("Cu").unwrap();
        let mut s = Stock::new(&els);
        s.kg[cu] = 12.0;
        let mut p = Parts::new(&r);
        // Ahead: 3 chips (3 kg), then 6 coils would take 12 kg: only 9 kg
        // left, so half of them, 3 (6 kg).
        let spent = p.make_ahead(&mut s, &els, &r);
        assert_eq!(p.n, vec![3, 3]);
        assert!((spent - 9.0).abs() < 1e-9 && (s.kg[cu] - 3.0).abs() < 1e-9);
        // A cutter from stock; the next with its second coil made on the
        // spot (2 kg of the last 3).
        assert!((p.build(&mut s, &els, &r, &r.harvester).unwrap()).abs() < 1e-9);
        assert_eq!(p.n, vec![2, 1]);
        assert!((p.build(&mut s, &els, &r, &r.harvester).unwrap() - 2.0).abs() < 1e-9);
        assert_eq!(p.n, vec![1, 0]);
        assert_eq!(p.made, vec![3, 4]);
        // Two coils short and 1 kg of copper: nothing changes.
        assert!(p.build(&mut s, &els, &r, &r.harvester).is_none());
        assert_eq!(p.n, vec![1, 0]);
    }
}
