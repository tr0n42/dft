//! The element table, loaded from data/elements.toml: the ground truth that
//! procedural chemistry reads (sizes, structures, electronegativity, melting
//! and boiling points, oxides).

use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Structure {
    Fcc,
    Bcc,
    Hcp,
    Diamond,
    Molecular,
    Noble,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Class {
    Metal,
    Metalloid,
    Nonmetal,
    Noble,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Element {
    pub symbol: String,
    pub name: String,
    /// Atomic mass, g/mol.
    pub mass: f64,
    /// Metallic radius for metals, covalent radius otherwise (pm).
    pub radius_pm: f64,
    pub structure: Structure,
    /// Pauling electronegativity (0 for noble gases).
    pub en: f64,
    pub valence: i32,
    pub melt_k: f64,
    pub boil_k: f64,
    /// Usual oxide ("" if none is stable), and its formation enthalpy per mole
    /// of oxygen atoms (kJ/mol, negative = stable).
    pub oxide: String,
    pub oxide_kj: f64,
    /// Game value of 1 kg of the pure element, in units of 1 kg of common rock.
    pub value: f64,
    pub class: Class,
}

#[derive(Debug, Deserialize)]
struct File {
    element: Vec<Element>,
}

#[derive(Debug, Clone, Default)]
pub struct ElementTable {
    pub list: Vec<Element>,
    by_symbol: BTreeMap<String, usize>,
}

impl ElementTable {
    pub fn parse(text: &str) -> Result<Self, String> {
        let f: File = toml::from_str(text).map_err(|e| format!("elements: {e}"))?;
        let mut by_symbol = BTreeMap::new();
        for (i, e) in f.element.iter().enumerate() {
            if by_symbol.insert(e.symbol.clone(), i).is_some() {
                return Err(format!("element {} is listed twice", e.symbol));
            }
        }
        Ok(ElementTable { list: f.element, by_symbol })
    }

    pub fn load(path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        Self::parse(&text)
    }

    /// An element's place in the table.
    pub fn index(&self, symbol: &str) -> Option<usize> {
        self.by_symbol.get(symbol).copied()
    }

    pub fn get(&self, symbol: &str) -> Option<&Element> {
        self.by_symbol.get(symbol).map(|&i| &self.list[i])
    }

    /// Share of a composition (element mass fractions) that is metal.
    pub fn metal_fraction(&self, composition: &BTreeMap<String, f64>) -> f64 {
        composition.iter().filter(|(s, _)| self.get(s).is_some_and(|e| e.class == Class::Metal)).map(|(_, f)| f).sum()
    }

    /// Value of 1 kg of a composition (mass-weighted element values).
    pub fn value_per_kg(&self, composition: &BTreeMap<String, f64>) -> f64 {
        composition.iter().map(|(s, f)| f * self.get(s).map_or(1.0, |e| e.value)).sum()
    }
}
