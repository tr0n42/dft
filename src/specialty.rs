//! Planet specialties (data/specialties.toml): what makes one body worth more
//! than another of its kind. Each body rolls a few from its seed: a rare vein
//! far richer than usual, a trace element spread through all its rock, a
//! district rich in some metals and poor in others, and now and then a
//! jackpot lode. Rolled once when the body is built and applied to its own
//! copy of the material table and geology rules.

use crate::materials::{MatId, MaterialTable, State};
use crate::noise::{hash, unit};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize)]
pub struct File {
    pub roll: Roll,
    #[serde(default)]
    pub specialty: Vec<Specialty>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Roll {
    /// Chance of 0, 1, 2, 3 ... specialties.
    pub count_odds: Vec<f64>,
    pub jackpot_chance: f64,
    /// Other odds for some kinds of body (asteroids roll more, and more
    /// jackpots: they are pit stops).
    #[serde(default)]
    pub by_archetype: Vec<ArchRoll>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArchRoll {
    pub archetype: Vec<String>,
    #[serde(default)]
    pub count_odds: Option<Vec<f64>>,
    #[serde(default)]
    pub jackpot_chance: Option<f64>,
}

/// How the meta tree steers a body's roll (written into the body's spec when
/// the player picks it; neutral by default). Element and specialty factors
/// multiply the chance of specialties that bring those elements, or of those
/// specialties by name; `jackpot` multiplies the jackpot chance; `extra` adds
/// rolls; `off` rolls none (for measuring the plain generator).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Bias {
    #[serde(default)]
    pub off: bool,
    #[serde(default)]
    pub element: BTreeMap<String, f64>,
    #[serde(default)]
    pub specialty: BTreeMap<String, f64>,
    #[serde(default)]
    pub jackpot: Option<f64>,
    #[serde(default)]
    pub extra: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Specialty {
    pub name: String,
    pub riff: String,
    #[serde(default = "one")]
    pub weight: f64,
    #[serde(default)]
    pub jackpot: bool,
    #[serde(default)]
    pub needs: Vec<String>,
    /// Only on these kinds of body (archetypes); empty: any.
    #[serde(default)]
    pub only: Vec<String>,
    #[serde(default)]
    pub boost: BTreeMap<String, f64>,
    #[serde(default)]
    pub enrich: Vec<Enrich>,
    #[serde(default)]
    pub lode: Option<LodeSpec>,
}

fn one() -> f64 {
    1.0
}

#[derive(Debug, Clone, Deserialize)]
pub struct Enrich {
    pub materials: Vec<String>,
    pub element: String,
    pub share: [f64; 2],
}

#[derive(Debug, Clone, Deserialize)]
pub struct LodeSpec {
    pub material: String,
    #[serde(default)]
    pub host: Vec<String>,
    #[serde(default)]
    pub any_solid: bool,
    pub count: [u32; 2],
    pub radius_m: [f64; 2],
    /// Where, as a share of the body's radius.
    pub r: [f64; 2],
}

/// A specialty as it came out on one body, with its numbers.
#[derive(Debug, Clone)]
pub struct Rolled {
    pub name: String,
    pub riff: String,
    pub jackpot: bool,
    /// One line per effect, for the player ("Ge 2.1% in sphalerite").
    pub effects: Vec<String>,
}

/// One big pod of a lode.
#[derive(Debug, Clone)]
pub struct Pod {
    pub cx: f64,
    pub cy: f64,
    pub radius: f64,
    pub material: MatId,
    /// Replaces only these (empty: any solid ground).
    pub host: Vec<MatId>,
    pub seed: u64,
}

fn range(r: [f64; 2], u: f64) -> f64 {
    r[0] + (r[1] - r[0]) * u
}

impl File {
    pub fn load(path: &std::path::Path) -> Result<Option<File>, String> {
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        toml::from_str(&text).map(Some).map_err(|e| format!("specialties: {e}"))
    }

    /// Check every name in the catalogue against the material and element tables.
    pub fn check(&self, mats: &MaterialTable) -> Result<(), String> {
        for s in &self.specialty {
            let err = |e: String| format!("specialty '{}': {e}", s.name);
            for n in s.boost.keys().chain(s.needs.iter()) {
                mats.id(n).map_err(err)?;
            }
            for e in &s.enrich {
                mats.elements.get(&e.element).ok_or_else(|| err(format!("unknown element {}", e.element)))?;
                for n in &e.materials {
                    mats.id(n).map_err(err)?;
                }
            }
            if let Some(l) = &s.lode {
                mats.id(&l.material).map_err(err)?;
                for n in &l.host {
                    mats.id(n).map_err(err)?;
                }
            }
        }
        Ok(())
    }
}

impl Specialty {
    /// Whether a body that could hold the materials marked in `can` could
    /// show this specialty.
    pub fn fits(&self, mats: &MaterialTable, can: &[bool], archetype: &str) -> bool {
        if !self.only.is_empty() && !self.only.iter().any(|a| a == archetype) {
            return false;
        }
        let has = |n: &String| mats.id(n).is_ok_and(|m| can[m as usize]);
        if !self.needs.is_empty() {
            return self.needs.iter().any(has);
        }
        let mut wanted: Vec<&String> = self.boost.iter().filter(|(_, v)| **v > 1.0).map(|(k, _)| k).collect();
        for e in &self.enrich {
            wanted.extend(e.materials.iter());
        }
        if let Some(l) = &self.lode {
            if l.any_solid {
                return true;
            }
            wanted.extend(l.host.iter());
        }
        wanted.iter().any(|n| has(n))
    }
}

/// Weighted pick of an index from `list` with u in [0, 1).
fn pick(list: &[(usize, f64)], u: f64) -> Option<usize> {
    let total: f64 = list.iter().map(|p| p.1).sum();
    if total <= 0.0 {
        return None;
    }
    let mut r = u * total;
    for p in list {
        if r < p.1 {
            return Some(p.0);
        }
        r -= p.1;
    }
    list.last().map(|p| p.0)
}

/// The elements a specialty brings (what the meta tree's element bias
/// matches): enriched elements, and the rarer elements of the materials it
/// boosts or lodes.
pub fn elements_of(s: &Specialty, mats: &MaterialTable) -> Vec<String> {
    const COMMON: [&str; 14] = ["O", "Si", "S", "H", "C", "N", "Fe", "Al", "Mg", "Ca", "Na", "K", "Cl", "F"];
    let mut out: Vec<String> = s.enrich.iter().map(|e| e.element.clone()).collect();
    let mut names: Vec<&String> = s.boost.iter().filter(|(_, v)| **v > 1.0).map(|(k, _)| k).collect();
    if let Some(l) = &s.lode {
        names.push(&l.material);
    }
    for n in names {
        if let Ok(m) = mats.id(n) {
            for (e, f) in &mats.get(m).composition {
                if *f >= 0.001 && !COMMON.contains(&e.as_str()) {
                    out.push(e.clone());
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Which specialties a body with this seed gets, by index into the catalogue:
/// the ordinary ones first, then a jackpot if it rolled one.
pub fn roll(file: &File, mats: &MaterialTable, can: &[bool], seed: u64, archetype: &str, bias: &Bias) -> Vec<usize> {
    if bias.off {
        return Vec::new();
    }
    // (Mixed once first: seeds a fixed step apart would otherwise roll alike.)
    let seed = hash(hash(seed, 0x51, 0x52, 0x53), 0x54, 0x55, 0x56);
    let h = |a: i64, b: i64| unit(hash(seed, a, b, 0x5BEC));
    let arch = file.roll.by_archetype.iter().find(|a| a.archetype.iter().any(|x| x == archetype));
    let odds = arch.and_then(|a| a.count_odds.clone()).unwrap_or_else(|| file.roll.count_odds.clone());
    let jackpot = arch.and_then(|a| a.jackpot_chance).unwrap_or(file.roll.jackpot_chance) * bias.jackpot.unwrap_or(1.0);
    // How many.
    let mut n = 0;
    let mut u = h(0, 0);
    for (k, p) in odds.iter().enumerate() {
        if u < *p {
            n = k;
            break;
        }
        u -= p;
        n = k;
    }
    n += bias.extra as usize;
    // Each one's weight, steered by the bias.
    let weight = |s: &Specialty| {
        let by_el = elements_of(s, mats).iter().filter_map(|e| bias.element.get(e)).fold(1.0f64, |a, &b| a.max(b));
        s.weight * by_el * bias.specialty.get(&s.name).copied().unwrap_or(1.0)
    };
    let mut out = Vec::new();
    let mut pool: Vec<(usize, f64)> = file.specialty.iter().enumerate().filter(|(_, s)| !s.jackpot && s.fits(mats, can, archetype)).map(|(i, s)| (i, weight(s))).collect();
    for k in 0..n {
        let Some(i) = pick(&pool, h(1, k as i64)) else { break };
        out.push(i);
        pool.retain(|p| p.0 != i);
    }
    if h(2, 0) < jackpot {
        let jp: Vec<(usize, f64)> = file.specialty.iter().enumerate().filter(|(_, s)| s.jackpot && s.fits(mats, can, archetype)).map(|(i, s)| (i, weight(s))).collect();
        if let Some(i) = pick(&jp, h(2, 1)) {
            out.push(i);
        }
    }
    out
}

/// Put `element` into material `m` at `share` (by mass), and into the forms
/// it melts or boils into, scaling the rest down.
fn enrich_material(mats: &mut MaterialTable, m: MatId, element: &str, share: f64) {
    let targets: Vec<usize> = (0..mats.list.len())
        .filter(|&i| {
            // Follow what each material was made from back to `m`.
            let mut j = i;
            for _ in 0..4 {
                if j == m as usize {
                    return true;
                }
                match mats.list[j].source {
                    Some(s) => j = s as usize,
                    None => return false,
                }
            }
            false
        })
        .collect();
    for i in targets {
        let c = &mut mats.list[i].composition;
        let old = c.get(element).copied().unwrap_or(0.0);
        if old >= share {
            continue;
        }
        let scale = (1.0 - share) / (1.0 - old).max(1e-12);
        for (e, f) in c.iter_mut() {
            if e != element {
                *f *= scale;
            }
        }
        c.insert(element.to_string(), share);
    }
}

/// Apply the rolled specialties to a body's own material table and geology
/// rules; returns what came out, and the lode pods (placed by `place`, which
/// is given (pod index, attempt) and returns a centre or None).
pub fn apply(file: &File, picked: &[usize], mats: &mut MaterialTable, can: &[bool], seed: u64, mut place: impl FnMut(&LodeSpec, u64) -> Option<(f64, f64)>) -> (Vec<Rolled>, Vec<Pod>) {
    let mut rolled = Vec::new();
    let mut pods = Vec::new();
    for (k, &i) in picked.iter().enumerate() {
        let s = &file.specialty[i];
        let ks = hash(seed, k as i64, i as i64, 0x5BED);
        let mut effects = Vec::new();
        for (name, mult) in &s.boost {
            let Ok(m) = mats.id(name) else { continue };
            let shown = can.get(m as usize).copied().unwrap_or(false);
            let g = &mut mats.geology;
            // First how much of what could form does (its scarcity), then,
            // once all of it does, how often it is picked over other ores.
            let k = g.keep(m) * mult;
            let weight = if k > 1.0 { k } else { 1.0 };
            if let Some(v) = g.scarcity.get_mut(m as usize) {
                *v = k.min(1.0);
            }
            for h in g.hydrothermal.iter_mut().filter(|h| h.0 == m) {
                h.4 *= weight;
            }
            for c in g.cumulate.iter_mut().chain(g.skarn.iter_mut()).filter(|c| c.0 == m) {
                c.1 *= weight;
            }
            for a in g.accessory.iter_mut().filter(|a| a.material == m) {
                a.chance = (a.chance * mult).min(0.5);
            }
            if shown {
                effects.push(if *mult >= 1.0 { format!("{name} ×{mult}") } else { format!("{name} cut to ×{mult}") });
            }
        }
        for (j, e) in s.enrich.iter().enumerate() {
            let share = range(e.share, unit(hash(ks, j as i64, 0, 1)));
            for n in &e.materials {
                if let Ok(m) = mats.id(n) {
                    enrich_material(mats, m, &e.element, share);
                }
            }
            let shown = if share >= 0.001 { format!("{:.2}%", share * 100.0) } else { format!("{:.0} ppm", share * 1e6) };
            // (Named only where this body has them.)
            let here: Vec<&str> = e.materials.iter().filter(|n| mats.id(n).is_ok_and(|m| can.get(m as usize).copied().unwrap_or(false))).map(|n| n.as_str()).collect();
            effects.push(format!("{} {} in {}", e.element, shown, if here.is_empty() { e.materials.join(", ") } else { here.join(", ") }));
        }
        if let Some(l) = &s.lode {
            let Ok(material) = mats.id(&l.material) else { continue };
            let host: Vec<MatId> = l.host.iter().filter_map(|n| mats.id(n).ok()).collect();
            let n = l.count[0] + ((l.count[1] - l.count[0] + 1) as f64 * unit(hash(ks, 0, 1, 2))) as u32;
            let mut placed = 0;
            for p in 0..n {
                let ps = hash(ks, p as i64, 2, 3);
                if let Some((cx, cy)) = place(l, ps) {
                    pods.push(Pod { cx, cy, radius: range(l.radius_m, unit(hash(ps, 0, 0, 4))), material, host: host.clone(), seed: ps });
                    placed += 1;
                }
            }
            effects.push(format!("{placed} pods of {}", l.material));
        }
        rolled.push(Rolled { name: s.name.clone(), riff: s.riff.clone(), jackpot: s.jackpot, effects });
    }
    (rolled, pods)
}

impl Pod {
    /// Whether the pod covers (fx, fy), material `m` there now.
    pub fn hit(&self, mats: &MaterialTable, fx: f64, fy: f64, m: MatId) -> bool {
        let (dx, dy) = (fx - self.cx, fy - self.cy);
        let d2 = dx * dx + dy * dy;
        if d2 > self.radius * self.radius * 1.69 {
            return false;
        }
        if self.host.is_empty() {
            if !matches!(mats.get(m).state, State::Solid | State::Powder) {
                return false;
            }
        } else if !self.host.contains(&m) {
            return false;
        }
        let r = self.radius * (0.7 + 0.6 * crate::noise::fbm(self.seed, fx / (self.radius * 0.5), fy / (self.radius * 0.5), 3));
        d2 <= r * r
    }
}
