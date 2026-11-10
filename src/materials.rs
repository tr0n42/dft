//! The material table, loaded from data/materials.toml (and the element table
//! beside it).
//!
//! Phase changes are generic: a material only says at what temperature it
//! melts and boils. Unless it names what it turns into, a molten form ("molten
//! basalt") and a vapour ("basalt vapour") are generated for it at load time,
//! with the same mass per cell, and they remember what they came from: molten
//! basalt freezes back into basalt. Liquid methane boiling into methane vapour
//! needs no rule of its own, any more than water does.

use crate::elements::ElementTable;
use serde::Deserialize;
use std::collections::BTreeMap;

pub type MatId = u16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Solid,
    Powder,
    Liquid,
    Gas,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Material {
    pub name: String,
    pub state: State,
    /// kg per cubic metre, which is also the mass of one 1 m cell.
    pub density: f64,
    pub hardness: u8,
    pub colour: [u8; 3],
    /// Processing bonus applied when this material is refined.
    pub multiplier: f64,
    /// Element mass fractions, summing to 1 (empty for vacuum and the Heart).
    pub composition: BTreeMap<String, f64>,
    /// Phase changes, each to a material of the same density (a cell keeps its
    /// mass whatever it turns into). At or above `melts_at_k` it becomes
    /// `melts_to`; at or below `freezes_at_k`, `freezes_to`; and likewise for
    /// boiling and condensing.
    #[serde(default)]
    pub melts_at_k: Option<f64>,
    #[serde(default)]
    pub melts_to: Option<String>,
    #[serde(default)]
    pub freezes_at_k: Option<f64>,
    #[serde(default)]
    pub freezes_to: Option<String>,
    #[serde(default)]
    pub boils_at_k: Option<f64>,
    #[serde(default)]
    pub boils_to: Option<String>,
    #[serde(default)]
    pub condenses_at_k: Option<f64>,
    #[serde(default)]
    pub condenses_to: Option<String>,
    /// Latent heat of melting and of boiling, as kelvin of heating held back at
    /// the transition (latent heat / specific heat). Defaults by kind.
    #[serde(default)]
    pub latent_melt_k: Option<f64>,
    #[serde(default)]
    pub latent_boil_k: Option<f64>,
    /// Physical density for sinking and floating, if different from the mass
    /// per cell (a vapour holds its liquid's mass, but rises).
    #[serde(default)]
    pub buoyancy: Option<f64>,
    /// How readily heat passes into and out of this material (0 to 1).
    #[serde(default = "half")]
    pub conduct: f64,
    /// Chance per sandbox tick that a loose cell (powder, liquid or gas) moves:
    /// 1 runs freely, low values are thick and slow, like lava.
    #[serde(default = "one")]
    pub flow: f64,
    /// Generated at load time as a phase of another material (its source).
    #[serde(skip)]
    pub source: Option<MatId>,
}

impl Material {
    /// Density for sinking and floating.
    pub fn buoyancy(&self) -> f64 {
        self.buoyancy.unwrap_or(self.density)
    }
}

fn half() -> f64 {
    0.5
}
fn one() -> f64 {
    1.0
}

/// A phase change resolved to ids: at `at_k`, after `latent_k` of heating (or
/// cooling) held back at that temperature, the cell becomes `to`.
#[derive(Debug, Clone, Copy)]
pub struct Change {
    pub at_k: f64,
    pub to: MatId,
    pub latent_k: f64,
}

/// Phase changes of one material, resolved to ids.
#[derive(Debug, Clone, Copy, Default)]
pub struct Phases {
    /// Heating: melting, then boiling.
    pub up: [Option<Change>; 2],
    /// Cooling: freezing, then condensing.
    pub down: [Option<Change>; 2],
}

/// A reaction between two touching cells: `a` and `b` become `a_to` and `b_to`
/// with this chance per sandbox tick, releasing `heat_k` into both (negative
/// absorbs). Mass is conserved: the two inputs weigh what the two outputs do.
#[derive(Debug, Clone, Deserialize)]
pub struct ReactionRow {
    pub name: String,
    pub a: String,
    pub b: String,
    pub a_to: String,
    pub b_to: String,
    pub chance: f64,
    #[serde(default)]
    pub heat_k: f64,
}

/// A row of data/burning.toml.
#[derive(Debug, Clone, Deserialize)]
pub struct BurnRow {
    pub material: String,
    pub ignite_k: f64,
    pub flame_k: f64,
    pub rate: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OxidizerRow {
    pub material: String,
}

/// How a material burns (see data/burning.toml).
#[derive(Debug, Clone, Copy)]
pub struct Burn {
    /// Catches at this temperature (kelvin), burns at flame_k, and burns
    /// away with this chance per second at full exposure to air.
    pub ignite_k: f64,
    pub flame_k: f64,
    pub rate: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct Reaction {
    pub a: MatId,
    pub b: MatId,
    pub a_to: MatId,
    pub b_to: MatId,
    pub chance: f64,
    pub heat_k: f64,
}

/// Constants for heat and motion in the sandbox.
#[derive(Debug, Clone, Deserialize)]
pub struct Physics {
    /// Sandbox ticks per second of game time.
    pub ticks_per_s: f64,
    /// What open space pulls a hot cell's temperature toward, and how fast (per tick).
    pub space_k: f64,
    pub radiate: f64,
    /// Share of a temperature difference that crosses between two touching cells
    /// per tick, before each material's `conduct` scales it.
    pub conduct: f64,
    /// A neighbour this much hotter or colder wakes up and starts tracking heat.
    pub wake_gradient_k: f64,
    /// A cell whose temperature moved less than this in a tick, and that did
    /// nothing else, goes back to sleep.
    pub settle_k: f64,
    /// Liquids from the generator start at least this far above their freezing
    /// point; and depth raises melting and boiling points to at least this far
    /// above the body's own temperature there (the weight above holds it solid).
    pub melt_margin_k: f64,
    /// Pull toward the centre on loose rigid bodies, m/s².
    pub gravity_m_s2: f64,
    /// Solid rock cut off from everything, up to this many cells, breaks free...
    pub island_max_cells: usize,
    /// ...if it has at least this many (smaller specks cling where they are).
    pub island_min_cells: usize,
    /// A body still for this long sets down into the grid.
    pub settle_s: f64,
    /// A body moving faster than this wrecks drones working where it passes.
    pub crush_speed_m_s: f64,
    /// Bigger chunks cut free, up to this many cells, move as one rigid slab
    /// (no tumbling), found by a slower search every `slab_check_s` seconds.
    #[serde(default = "slab_max_cells")]
    pub slab_max_cells: usize,
    #[serde(default = "slab_check_s")]
    pub slab_check_s: f64,
    /// Chunks leave with a small kick (m/s), from the cutting: on a body with
    /// little gravity that can be enough to drift away for good.
    #[serde(default = "chunk_kick_m_s")]
    pub chunk_kick_m_s: f64,
    /// Share of the gap to the Sun's warmth that a sunlit open face closes
    /// per tick (sunlight heats bare ground much faster than space cools it).
    #[serde(default = "sunlight")]
    pub sunlight: f64,
    /// A drone working beside a liquid this hot (lava, molten metal) is wrecked.
    pub lava_kill_k: f64,
    /// Chance that a drone caught by lava is wrecked (the rest get away).
    pub lava_kill_chance: f64,
}

#[derive(Debug, Deserialize)]
struct File {
    material: Vec<Material>,
    #[serde(default)]
    reaction: Vec<ReactionRow>,
    #[serde(default)]
    burn: Vec<BurnRow>,
    #[serde(default)]
    oxidizer: Vec<OxidizerRow>,
    physics: Physics,
    #[serde(default)]
    geology: crate::geology::RulesFile,
    #[serde(default)]
    glamour: Option<crate::glamour::GlamourFile>,
    #[serde(default)]
    life: Option<crate::flora::LifeFile>,
}

#[derive(Debug, Clone)]
pub struct MaterialTable {
    pub elements: ElementTable,
    pub list: Vec<Material>,
    pub phases: Vec<Phases>,
    pub reactions: Vec<Reaction>,
    /// Per material: how it burns, if it does; and whether it feeds a fire
    /// it touches (an oxidizer).
    pub burns: Vec<Option<Burn>>,
    pub oxidizer: Vec<bool>,
    pub physics: Physics,
    pub geology: crate::geology::Rules,
    /// The glamour pass's rules (data/glamour.toml), if loaded.
    pub glamour: Option<crate::glamour::Rules>,
    /// The rules that breed each body's flora (data/life.toml), if loaded.
    pub life: Option<crate::flora::LifeFile>,
    /// The planet specialties (data/specialties.toml), if loaded.
    pub specialties: Option<std::sync::Arc<crate::specialty::File>>,
    /// The table as loaded, before a body's specialties changed it: a body
    /// built again from this table starts from here, so nothing applies twice.
    pub pristine: Option<std::sync::Arc<MaterialTable>>,
}

pub const VACUUM: MatId = 0;

impl MaterialTable {
    pub fn parse(text: &str, elements: ElementTable) -> Result<Self, String> {
        let f: File = toml::from_str(text).map_err(|e| e.to_string())?;
        if f.material.first().map(|m| m.name.as_str()) != Some("vacuum") {
            return Err("the first material must be vacuum".into());
        }
        for m in &f.material {
            let sum: f64 = m.composition.values().sum();
            if !m.composition.is_empty() && (sum - 1.0).abs() > 1e-9 {
                return Err(format!("{}: composition sums to {sum}, not 1", m.name));
            }
            for el in m.composition.keys() {
                if elements.get(el).is_none() {
                    return Err(format!("{}: unknown element {el}", m.name));
                }
            }
        }
        let mut t = MaterialTable { elements, list: f.material, phases: Vec::new(), reactions: Vec::new(), burns: Vec::new(), oxidizer: Vec::new(), physics: f.physics, geology: Default::default(), glamour: None, life: f.life.clone(), specialties: None, pristine: None };
        t.generate_phases()?;
        if t.list.len() > MatId::MAX as usize {
            return Err(format!("{} materials with their phases; at most {}", t.list.len(), MatId::MAX));
        }
        // Resolve every phase change to ids, with its latent heat.
        let mut phases = Vec::new();
        for m in &t.list {
            let mut ph = Phases::default();
            let metal = t.elements.metal_fraction(&m.composition) > 0.5;
            let latent_melt = m.latent_melt_k.unwrap_or(if metal { 500.0 } else { 400.0 });
            let latent_boil = m.latent_boil_k.unwrap_or(2000.0);
            let pairs = [
                (&m.melts_at_k, &m.melts_to, true, 0, latent_melt),
                (&m.boils_at_k, &m.boils_to, true, 1, latent_boil),
                (&m.freezes_at_k, &m.freezes_to, false, 0, latent_melt),
                (&m.condenses_at_k, &m.condenses_to, false, 1, latent_boil),
            ];
            for (at, to, up, slot, latent_k) in pairs {
                match (at, to) {
                    (Some(k), Some(name)) => {
                        let id = t.id(name)?;
                        if (t.get(id).density - m.density).abs() > 1e-9 {
                            return Err(format!("{} turns into {name}, which must have the same density", m.name));
                        }
                        let c = Some(Change { at_k: *k, to: id, latent_k });
                        if up { ph.up[slot] = c } else { ph.down[slot] = c }
                    }
                    (None, None) => {}
                    _ => return Err(format!("{}: a phase change needs both a temperature and a material", m.name)),
                }
            }
            phases.push(ph);
        }
        t.phases = phases;
        // Reactions (overrides): both directions are looked up, and mass must balance.
        for r in &f.reaction {
            let (a, b, a_to, b_to) = (t.id(&r.a)?, t.id(&r.b)?, t.id(&r.a_to)?, t.id(&r.b_to)?);
            let before = t.get(a).density + t.get(b).density;
            let after = t.get(a_to).density + t.get(b_to).density;
            if (before - after).abs() > 1e-9 {
                return Err(format!("reaction '{}' turns {before} kg into {after} kg; mass must balance", r.name));
            }
            t.reactions.push(Reaction { a, b, a_to, b_to, chance: r.chance, heat_k: r.heat_k });
        }
        // Burning.
        t.burns = vec![None; t.list.len()];
        t.oxidizer = vec![false; t.list.len()];
        for b in &f.burn {
            let id = t.id(&b.material)?;
            if b.flame_k <= b.ignite_k {
                return Err(format!("{} burns cooler than it catches", b.material));
            }
            t.burns[id as usize] = Some(Burn { ignite_k: b.ignite_k, flame_k: b.flame_k, rate: b.rate });
        }
        for o in &f.oxidizer {
            let id = t.id(&o.material)?;
            t.oxidizer[id as usize] = true;
        }
        t.geology = crate::geology::Rules::resolve(&f.geology, &t)?;
        if let Some(gf) = &f.glamour {
            t.glamour = Some(crate::glamour::Rules::resolve(gf, &t)?);
        }
        Ok(t)
    }

    /// Generate the molten and vapour forms every material needs but doesn't
    /// name, appended after the written materials (so their ids never move).
    fn generate_phases(&mut self) -> Result<(), String> {
        let mut i = 0;
        while i < self.list.len() {
            let m = self.list[i].clone();
            let metal = self.elements.metal_fraction(&m.composition) > 0.5;
            let base = ["molten-", "molten ", "liquid "].iter().find_map(|p| m.name.strip_prefix(p)).unwrap_or(&m.name).to_string();
            // A solid that melts but doesn't say into what: its molten form.
            if matches!(m.state, State::Solid | State::Powder) && m.melts_at_k.is_some() && m.melts_to.is_none() {
                let name = format!("molten {base}");
                if self.list.iter().all(|x| x.name != name) {
                    let melt = m.melts_at_k.unwrap();
                    self.list.push(Material {
                        name: name.clone(),
                        state: State::Liquid,
                        density: m.density,
                        hardness: 0,
                        colour: glow(m.colour),
                        multiplier: m.multiplier,
                        composition: m.composition.clone(),
                        melts_at_k: None,
                        melts_to: None,
                        freezes_at_k: Some(melt - 20.0),
                        freezes_to: Some(m.name.clone()),
                        boils_at_k: m.boils_at_k,
                        boils_to: None,
                        condenses_at_k: None,
                        condenses_to: None,
                        latent_melt_k: m.latent_melt_k,
                        latent_boil_k: m.latent_boil_k,
                        buoyancy: Some(m.buoyancy() * 0.92),
                        conduct: if metal { 0.5 } else { 0.05 },
                        flow: if metal { 0.7 } else { 0.25 },
                        source: Some(i as MatId),
                    });
                }
                self.list[i].melts_to = Some(name);
                // The solid itself doesn't boil: its melt does.
                self.list[i].boils_at_k = None;
            }
            // A liquid that boils but doesn't say into what: its vapour.
            if m.state == State::Liquid && m.boils_at_k.is_some() && m.boils_to.is_none() {
                let name = format!("{base} vapour");
                if self.list.iter().all(|x| x.name != name) {
                    let boil = m.boils_at_k.unwrap();
                    self.list.push(Material {
                        name: name.clone(),
                        state: State::Gas,
                        density: m.density,
                        hardness: 0,
                        colour: pale(m.colour),
                        multiplier: m.multiplier,
                        composition: m.composition.clone(),
                        melts_at_k: None,
                        melts_to: None,
                        freezes_at_k: None,
                        freezes_to: None,
                        boils_at_k: None,
                        boils_to: None,
                        condenses_at_k: Some(boil - 10.0),
                        condenses_to: Some(m.name.clone()),
                        latent_melt_k: m.latent_melt_k,
                        latent_boil_k: m.latent_boil_k,
                        buoyancy: Some(1.0),
                        conduct: 0.2,
                        flow: 1.0,
                        source: Some(i as MatId),
                    });
                }
                self.list[i].boils_to = Some(name);
            }
            i += 1;
        }
        Ok(())
    }

    /// Load data/materials.toml, with data/elements.toml from the same folder.
    pub fn load(path: &str) -> Result<Self, String> {
        let mut text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        let dir = std::path::Path::new(path).parent().unwrap_or(std::path::Path::new("."));
        // The generated catalogue (tools/catalogue.py) adds its materials after
        // the hand-written ones.
        for extra in ["materials-catalogue.toml", "geology.toml", "glamour.toml", "burning.toml", "life.toml"] {
            let cat = dir.join(extra);
            if cat.exists() {
                text.push('\n');
                text.push_str(&std::fs::read_to_string(&cat).map_err(|e| format!("{}: {e}", cat.display()))?);
            }
        }
        let elements = ElementTable::load(&dir.join("elements.toml").to_string_lossy())?;
        let mut t = Self::parse(&text, elements)?;
        if let Some(f) = crate::specialty::File::load(&dir.join("specialties.toml"))? {
            f.check(&t)?;
            t.specialties = Some(std::sync::Arc::new(f));
        }
        Ok(t)
    }

    pub fn id(&self, name: &str) -> Result<MatId, String> {
        self.list
            .iter()
            .position(|m| m.name == name)
            .map(|i| i as MatId)
            .ok_or_else(|| format!("unknown material '{name}'"))
    }

    /// Add a copy of material `of` under a new name and colour, with the same
    /// physics (phase changes, burning): one body's flora gets its own colours
    /// this way. Returns the new id.
    pub fn add_copy(&mut self, of: MatId, name: String, colour: [u8; 3]) -> MatId {
        let mut m = self.list[of as usize].clone();
        m.name = name;
        m.colour = colour;
        self.list.push(m);
        self.phases.push(self.phases[of as usize]);
        self.burns.push(self.burns[of as usize]);
        self.oxidizer.push(self.oxidizer[of as usize]);
        (self.list.len() - 1) as MatId
    }

    pub fn get(&self, id: MatId) -> &Material {
        &self.list[id as usize]
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }
}

/// A molten material's colour: its solid colour heated to a glow.
fn glow(c: [u8; 3]) -> [u8; 3] {
    [(c[0] as u16 / 3 + 170).min(255) as u8, (c[1] as u16 / 3 + 70).min(255) as u8, (c[2] as u16 / 4 + 10) as u8]
}

/// A vapour's colour: pale and washed out.
fn pale(c: [u8; 3]) -> [u8; 3] {
    [(c[0] as u16 / 3 + 160) as u8, (c[1] as u16 / 3 + 160) as u8, (c[2] as u16 / 3 + 165) as u8]
}

fn slab_max_cells() -> usize {
    20_000
}
fn slab_check_s() -> f64 {
    10.0
}
fn chunk_kick_m_s() -> f64 {
    0.4
}
fn sunlight() -> f64 {
    0.1
}
