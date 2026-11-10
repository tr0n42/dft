//! Body specs: one data file per body, in data/bodies/.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Shape {
    /// Surface height variation as a fraction of radius.
    pub roughness: f64,
    /// Size of the surface lumps, in metres.
    pub wavelength_m: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Layer {
    pub name: String,
    /// Outer edge as a fraction of the radius.
    pub outer: f64,
    pub material: String,
    pub temperature_k: f64,
    #[serde(default)]
    pub boundary_noise: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Caves {
    pub material: String,
    pub scale_m: f64,
    pub threshold: f64,
    pub r_min: f64,
    pub r_max: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Blob {
    pub material: String,
    pub count: u32,
    pub radius_min_m: f64,
    pub radius_max_m: f64,
    pub r_min: f64,
    pub r_max: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Vein {
    pub material: String,
    pub host: Vec<String>,
    pub scale_m: f64,
    pub width: f64,
    pub r_min: f64,
    pub r_max: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Sprinkle {
    pub material: String,
    pub host: Vec<String>,
    pub chance: f64,
    pub r_min: f64,
    pub r_max: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Pipes {
    pub material: String,
    pub count: u32,
    pub width_m: f64,
    pub r_min: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Craters {
    pub count: u32,
    pub radius_min_m: f64,
    pub radius_max_m: f64,
    /// Shattered rock under each bowl, as a fraction of the crater's radius.
    #[serde(default)]
    pub breccia: f64,
    #[serde(default)]
    pub breccia_material: Option<String>,
    /// A sheet of melted glass lining the floor, as a fraction of the radius.
    #[serde(default)]
    pub melt: f64,
    #[serde(default)]
    pub melt_material: Option<String>,
    /// Raised rim height as a share of the crater's radius (with an ejecta
    /// blanket of shattered rock around it).
    #[serde(default)]
    pub rim: f64,
}

/// A rubble pile: packed boulders with gaps (G3 geology).
#[derive(Debug, Clone, Deserialize)]
pub struct Rubble {
    pub boulder_m: f64,
    pub gap_m: f64,
    pub gap_material: String,
    /// Fraction of boulder spaces left empty.
    #[serde(default)]
    pub voids: f64,
    #[serde(default)]
    pub r_min: f64,
    #[serde(default = "one")]
    pub r_max: f64,
    pub rock: Vec<RubbleRock>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RubbleRock {
    pub material: String,
    #[serde(default = "one")]
    pub weight: f64,
    /// The largest boulder size this rock comes in: "big", "medium" or
    /// "small". Rare rocks kept to smaller sizes make a body's makeup steadier
    /// from seed to seed (one giant metal boulder can't swing it).
    #[serde(default)]
    pub largest: Option<String>,
}

/// A kind of cave that forms only in certain rock (G3 geology).
#[derive(Debug, Clone, Deserialize)]
pub struct CaveKind {
    /// "karst" (dissolved carbonate) or "tube" (drained lava channel).
    pub kind: String,
    pub host: Vec<String>,
    pub scale_m: f64,
    pub threshold: f64,
    #[serde(default = "far")]
    pub max_depth_m: f64,
}

fn far() -> f64 {
    1.0e12
}

/// Polar ice caps.
#[derive(Debug, Clone, Deserialize)]
pub struct IceCaps {
    pub material: String,
    /// Thickness at the pole itself.
    pub thickness_m: f64,
    /// Where the caps begin, as the sine of latitude (0 equator, 1 pole).
    pub from_latitude: f64,
}

/// A lens of one material inside a strata band (G3 geology).
#[derive(Debug, Clone, Deserialize)]
pub struct Lens {
    /// The band rock it forms in.
    pub host: String,
    pub material: String,
    /// "base", "middle" or "top" of the band.
    pub position: String,
    pub length_m: [f64; 2],
    /// Thickness at the lens centre, as a fraction of the band.
    pub thickness: f64,
    /// Average distance between lenses along a band.
    pub spacing_m: f64,
    #[serde(default)]
    pub nugget: Option<String>,
    #[serde(default)]
    pub nugget_chance: f64,
}

/// One kind of rock in a strata stack.
#[derive(Debug, Clone, Deserialize)]
pub struct StrataBand {
    pub material: String,
    /// Thickness range of one band, in metres.
    pub thickness_m: [f64; 2],
    /// How often this rock is picked, relative to the others.
    #[serde(default = "one")]
    pub weight: f64,
}

fn one() -> f64 {
    1.0
}

/// Bands of rock that follow the surface (G1 geology).
#[derive(Debug, Clone, Deserialize)]
pub struct Strata {
    /// Total depth of the stack below the surface, in metres.
    pub depth_m: f64,
    /// How far folds lift and sink the stack, in metres.
    #[serde(default)]
    pub fold_amplitude_m: f64,
    #[serde(default = "thousand")]
    pub fold_wavelength_m: f64,
    /// How much each band's thickness varies along the surface (0 to 1; above
    /// about 0.6 bands pinch out into lenses).
    #[serde(default)]
    pub pinch: f64,
    #[serde(default = "thousand")]
    pub pinch_wavelength_m: f64,
    pub band: Vec<StrataBand>,
    #[serde(default)]
    pub lens: Vec<Lens>,
    /// An older era under these layers (G5): its own stack, folded hard,
    /// then worn flat and buried. The worn surface is an unconformity.
    #[serde(default)]
    pub old: Option<Box<OldStrata>>,
}

/// The older stack under an unconformity.
#[derive(Debug, Clone, Deserialize)]
pub struct OldStrata {
    /// How deep the worn surface lies under the young layers (varies along it).
    pub cover_m: [f64; 2],
    #[serde(default = "thousand")]
    pub cover_wavelength_m: f64,
    /// How much of the old stack's top was worn away before burial (cuts down
    /// into its folds).
    #[serde(default)]
    pub erode_m: f64,
    /// A gravel layer laid on the worn surface, at the base of the young stack.
    #[serde(default)]
    pub conglomerate: Option<String>,
    #[serde(default)]
    pub conglomerate_m: [f64; 2],
    #[serde(flatten)]
    pub stack: Strata,
}

fn fifty() -> f64 {
    50.0
}

fn thousand() -> f64 {
    1000.0
}

/// A surface feature family (G5): each seed draws whether it happens, how
/// many, and their sizes.
#[derive(Debug, Clone, Deserialize)]
pub struct Relief {
    /// volcano, rift, basin, range, plateau or lowland.
    pub kind: String,
    /// Chance this family appears at all on a given seed.
    #[serde(default = "one")]
    pub chance: f64,
    #[serde(default = "single")]
    pub count: [u32; 2],
    /// Half-width (radius) of each feature along the surface, in metres. For a
    /// lowland, the width of the slope between the halves.
    pub width_m: [f64; 2],
    /// Height (volcano, range, plateau) or depth (rift, basin, lowland).
    pub height_m: [f64; 2],
    /// Volcano: caldera width as a share of the base. Basin: rim height (m).
    #[serde(default)]
    pub extra: f64,
    /// Volcano: a steep cone rather than a broad shield.
    #[serde(default)]
    pub steep: bool,
    /// What it's built or filled with (volcano flows are picked per band).
    #[serde(default)]
    pub material: Vec<Pick>,
    #[serde(default)]
    pub band_m: [f64; 2],
    /// Rift sediment, basin lava or lowland lava thickness.
    #[serde(default)]
    pub fill_m: f64,
    /// Volcanoes: chance each one is still active (molten chamber and
    /// conduit) rather than extinct (frozen to `chamber`).
    #[serde(default = "half")]
    pub active_chance: f64,
    /// What an extinct volcano's chamber has frozen to (default: gabbro under
    /// shields, granite under steep cones).
    #[serde(default)]
    pub chamber: Option<String>,
    /// Arc volcanoes: how far from the trench, toward where the slab sinks.
    #[serde(default)]
    pub offset_m: [f64; 2],
    /// Volcanoes: what rises through them (default magma; brine or liquid
    /// ammonia for cryovolcanoes) and what their dikes freeze to (default
    /// dolerite for magma, else the cone's first material).
    #[serde(default)]
    pub melt: Option<String>,
    #[serde(default)]
    pub dike: Option<String>,
    /// Basins and lowlands filled with a liquid: a solid lid this thick on
    /// the fill (and the rim made of it too), so the pool lies sealed until
    /// broken into. The lid is `crust` (default: what the liquid freezes to).
    #[serde(default)]
    pub crust_m: f64,
    #[serde(default)]
    pub crust: Option<String>,
    /// Basins and lowlands: the rock under the fill is replaced by `floor`
    /// this deep (`floor_m`), e.g. ocean-floor basalt under a sea while the
    /// land around keeps its own layers.
    #[serde(default)]
    pub floor: Option<String>,
    #[serde(default)]
    pub floor_m: f64,
    /// Lowlands: instead of a fill, a sea this far below the body's mean
    /// radius. Once the surface is final, every hollow below that level fills
    /// with the lowland's material, flat on top.
    #[serde(default)]
    pub sea_level_m: f64,
}

fn half() -> f64 {
    0.5
}

fn single() -> [u32; 2] {
    [1, 1]
}

/// A family of plutons: bodies of magma that cooled underground, with the
/// dikes, ore veins and baked rock around them (G2 geology).
#[derive(Debug, Clone, Deserialize)]
pub struct Intrusion {
    pub material: String,
    /// "felsic" (granite-like) or "mafic" (gabbro-like): decides the ore.
    pub kind: String,
    pub count: u32,
    pub radius_m: [f64; 2],
    /// Depth of the pluton's top below the surface, in metres.
    pub top_depth_m: [f64; 2],
    #[serde(default)]
    pub dikes: [u32; 2],
    #[serde(default)]
    pub dike_material: Option<String>,
    #[serde(default = "dike_width")]
    pub dike_width_m: [f64; 2],
    #[serde(default)]
    pub veins: [u32; 2],
    #[serde(default = "vein_width")]
    pub vein_width_m: [f64; 2],
    /// How far ore veins reach from the pluton, in metres.
    #[serde(default = "halo")]
    pub halo_m: f64,
    /// Width of the baked rim around the pluton.
    #[serde(default)]
    pub aureole_m: f64,
    /// Width of the skarn zone where the pluton meets carbonate rock.
    #[serde(default)]
    pub skarn_m: f64,
}

fn dike_width() -> [f64; 2] {
    [2.0, 5.0]
}

fn vein_width() -> [f64; 2] {
    [1.0, 3.0]
}

fn halo() -> f64 {
    300.0
}

/// Straight breaks from the surface down; rock on one side is shifted.
#[derive(Debug, Clone, Deserialize)]
pub struct Faults {
    pub count: u32,
    /// Range of how far one side moves, in metres (up or down at random).
    pub throw_m: [f64; 2],
    /// How far down the fault runs, in metres.
    pub length_m: f64,
    /// Lean from straight down: 0 is vertical, 1 is 45 degrees.
    #[serde(default)]
    pub dip: f64,
    /// Crushed rock along the break, if any.
    #[serde(default)]
    pub gouge: Option<String>,
    #[serde(default)]
    pub gouge_width_m: f64,
    /// Share of the throw left standing as a cliff at the surface (0 to 1).
    #[serde(default)]
    pub scarp: f64,
}

/// A weighted material choice.
#[derive(Debug, Clone, Deserialize)]
pub struct Pick {
    pub material: String,
    #[serde(default = "one")]
    pub weight: f64,
}

/// "Marble cake" streaks: old crust stirred into the mantle by convection,
/// stretched into loops around each convection cell.
#[derive(Debug, Clone, Deserialize)]
pub struct Streaks {
    pub host: Vec<String>,
    /// The shell they fill, as fractions of the radius.
    pub r_min: f64,
    pub r_max: f64,
    /// Convection cells around the body.
    pub cells: u32,
    /// Loops per cell, from its rim to its middle.
    pub bands: f64,
    /// Share of each loop's thickness that is streak (0 to 1).
    pub width: f64,
    /// Share of loops that carry a streak.
    pub fraction: f64,
    /// How much the loops wander (0 = perfect ovals).
    #[serde(default = "warp")]
    pub warp: f64,
    pub rock: Vec<Pick>,
}

fn warp() -> f64 {
    0.18
}

/// Hot columns rising from deep down, with a mushroom head.
#[derive(Debug, Clone, Deserialize)]
pub struct Plume {
    pub count: u32,
    /// Where the stem starts and where the head sits, as fractions of the radius.
    pub base: f64,
    pub top: f64,
    pub stem_m: [f64; 2],
    pub head_m: [f64; 2],
    pub material: String,
    #[serde(default)]
    pub melt: Option<String>,
    /// Share of the head that is molten.
    #[serde(default)]
    pub melt_share: f64,
    /// How much hotter than the surrounding mantle.
    #[serde(default)]
    pub heat_k: f64,
}

/// Cold sheets of old crust sinking from the surface.
#[derive(Debug, Clone, Deserialize)]
pub struct Slab {
    pub count: u32,
    pub thickness_m: f64,
    /// How deep the slab reaches, as a fraction of the radius.
    pub bottom: f64,
    pub material: String,
    /// Chance that a slab stalls and lies flat at `stall_at`.
    #[serde(default)]
    pub stall_chance: f64,
    #[serde(default)]
    pub stall_at: f64,
    /// How much colder than the surrounding mantle (positive number).
    #[serde(default)]
    pub chill_k: f64,
    #[serde(default)]
    pub diamond: Option<String>,
    #[serde(default)]
    pub diamond_chance: f64,
}

/// Dense piles resting on the core, with thin melt pockets at their edges.
#[derive(Debug, Clone, Deserialize)]
pub struct Piles {
    pub count: u32,
    /// The core's surface, as a fraction of the radius.
    pub at: f64,
    /// Width of each pile, as a fraction of the way around the body.
    pub width: [f64; 2],
    pub height_m: [f64; 2],
    pub material: String,
    #[serde(default)]
    pub melt: Option<String>,
    #[serde(default)]
    pub melt_m: f64,
}

/// Giant iron crystals radiating from the centre, with precious alloy along
/// their boundaries, richest toward the middle.
#[derive(Debug, Clone, Deserialize)]
pub struct InnerCore {
    pub host: String,
    pub grain_m: f64,
    pub alt: String,
    pub precious: String,
    /// Chance a boundary cell is precious alloy, at the centre and at the edge.
    pub precious_centre: f64,
    pub precious_edge: f64,
    #[serde(default = "one")]
    pub boundary_m: f64,
}

/// Convection columns in the liquid outer core, lined up with the spin axis
/// (vertical in our pole-to-pole slice), and a light layer at its top.
#[derive(Debug, Clone, Deserialize)]
pub struct OuterCore {
    pub host: String,
    pub material: String,
    pub column_m: f64,
    pub share: f64,
    #[serde(default)]
    pub top_material: Option<String>,
    #[serde(default)]
    pub top_m: f64,
}

/// Structure in the deep interior (mantle and core).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Deep {
    #[serde(default)]
    pub streaks: Vec<Streaks>,
    #[serde(default)]
    pub plume: Vec<Plume>,
    #[serde(default)]
    pub slab: Vec<Slab>,
    #[serde(default)]
    pub piles: Option<Piles>,
    #[serde(default)]
    pub inner_core: Option<InnerCore>,
    #[serde(default)]
    pub outer_core: Option<OuterCore>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BodySpec {
    pub name: String,
    pub archetype: String,
    /// How the meta tree steers this body's specialties (see specialty::Bias).
    #[serde(default)]
    pub bias: crate::specialty::Bias,
    pub seed: u64,
    pub radius_m: f64,
    pub heart: bool,
    pub regolith_depth_m: f64,
    /// Loose ground only lies where the surface is gentler than this slope
    /// (rise over run); steeper ground is bare rock. None: everywhere.
    #[serde(default)]
    pub regolith_max_slope: Option<f64>,
    pub atmosphere_m: f64,
    pub atmosphere_colour: [u8; 3],
    pub shape: Shape,
    pub layer: Vec<Layer>,
    pub caves: Option<Caves>,
    #[serde(default)]
    pub blob: Vec<Blob>,
    #[serde(default)]
    pub vein: Vec<Vein>,
    #[serde(default)]
    pub sprinkle: Vec<Sprinkle>,
    pub pipes: Option<Pipes>,
    pub craters: Option<Craters>,
    /// Gravity drones and loose rock feel at the surface. Defaults to the
    /// physics table's value (a gameplay number, much stronger than the real
    /// pull of a small body).
    #[serde(default)]
    pub gravity_m_s2: Option<f64>,
    /// Temperature of the sky an open surface radiates into. Defaults to the
    /// physics table's deep-space value; a body near its star sees a warmer sky.
    #[serde(default)]
    pub space_k: Option<f64>,
    /// Share of the air that is oxygen (0, no air to burn with, on most
    /// bodies; about 0.21 on an Earth-like world).
    #[serde(default)]
    pub oxygen: f64,
    /// The cross-section is a slice through both poles, which sit at the top
    /// and bottom. Surface rock at a pole is this much colder than at the
    /// equator (falling off as the square of the sine of latitude).
    #[serde(default)]
    pub polar_cooling_k: f64,
    /// How deep the polar chill reaches before the body's own heat takes over.
    #[serde(default = "fifty")]
    pub polar_depth_m: f64,
    #[serde(default)]
    pub strata: Option<Strata>,
    #[serde(default)]
    pub faults: Option<Faults>,
    #[serde(default)]
    pub intrusion: Vec<Intrusion>,
    #[serde(default)]
    pub rubble: Option<Rubble>,
    #[serde(default)]
    pub cave: Vec<CaveKind>,
    #[serde(default)]
    pub ice_caps: Option<IceCaps>,
    /// What the loose surface blanket is made of (default: regolith).
    #[serde(default)]
    pub regolith_material: Option<String>,
    #[serde(default)]
    pub deep: Option<Deep>,
    #[serde(default)]
    pub relief: Vec<Relief>,
    /// Sunlight: a day side that warms exposed ground, turning with the body.
    #[serde(default)]
    pub sun: Option<Sun>,
    /// Metal bodies: the crystal structure of slowly cooled iron.
    #[serde(default)]
    pub widmanstatten: Option<Widmanstatten>,
    /// The glamour pass over the interior: 0 (or none) is the realistic model
    /// as locked in; up to 1 adds ever bolder eye candy, from plausible
    /// geology toward science fiction.
    #[serde(default)]
    pub glamour: Option<f64>,
    /// Life (data/life.toml): how much flora the body grows where its
    /// chemistry allows, 0 to 2 (1 normal). None for now means no flora
    /// (bodies opt in until life is part of the game).
    #[serde(default)]
    pub life: Option<f64>,
    /// Pin the body's colour scheme instead of drawing it from the seed:
    /// "analogous", "complementary", "split-complementary", "triadic" or
    /// "square", and the base hue in degrees (0 red, 120 green, 240 blue).
    #[serde(default)]
    pub palette: Option<PaletteSpec>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PaletteSpec {
    #[serde(default)]
    pub scheme: Option<String>,
    #[serde(default)]
    pub hue: Option<f64>,
}

impl BodySpec {
    pub fn parse(text: &str) -> Result<Self, String> {
        let s: BodySpec = toml::from_str(text).map_err(|e| e.to_string())?;
        if s.layer.is_empty() {
            return Err("a body needs at least one layer".into());
        }
        let mut last = 0.0;
        for l in &s.layer {
            if l.outer <= last {
                return Err(format!("layer '{}' must sit outside the layer before it", l.name));
            }
            last = l.outer;
        }
        if (last - 1.0).abs() > 1e-9 {
            return Err("the outermost layer must reach 1.0".into());
        }
        Ok(s)
    }

    pub fn load(path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        Self::parse(&text)
    }
}

/// The Widmanstätten structure of meteoritic iron (see metal.rs).
#[derive(Debug, Clone, Deserialize)]
pub struct Widmanstatten {
    /// Materials restyled (the metal core).
    pub host: Vec<String>,
    /// Size of the parent crystals.
    pub grain_m: f64,
    /// Distance between plates of the main family.
    pub spacing_m: f64,
    /// Thickness of the nickel-rich edge on each plate.
    pub taenite_m: f64,
    pub kamacite: String,
    pub taenite: String,
    pub plessite: String,
    pub r_min: f64,
    pub r_max: f64,
}

/// Sunlight on a turning body.
#[derive(Debug, Clone, Deserialize)]
pub struct Sun {
    /// Temperature bare ground comes to with the Sun straight overhead.
    pub day_k: f64,
    /// Seconds of game time for one day (one turn of the body).
    pub day_s: f64,
    /// Where the Sun starts, in degrees from straight up (+y), turning
    /// counterclockwise.
    #[serde(default)]
    pub start_deg: f64,
}

/// A body spec resized to `radius_m`. Layers are shares of the radius, so
/// they follow by themselves; every length in metres (feature sizes,
/// depths, thicknesses, spacings) and every feature count is adjusted so the
/// body still looks like its kind at the new size:
///
/// - smaller: lengths shrink in proportion, counts stay (a miniature);
/// - bigger: lengths grow only with the square root of the size, and counts
///   grow with it (up to three times), so a big world has more features,
///   each a little bigger, rather than a few blown up to continent size.
///   Bumps given as a share of the radius (the surface's roughness, the
///   wobble of layer boundaries) grow the same way, not with the radius.
pub fn resize_text(text: &str, radius_m: f64) -> Result<String, String> {
    let mut v: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
    let old = v.get("radius_m").and_then(|r| r.as_float().or(r.as_integer().map(|i| i as f64))).ok_or("the body has no radius_m")?;
    if radius_m <= 0.0 || (radius_m - old).abs() < 1e-6 {
        return Ok(text.to_string());
    }
    let k = radius_m / old;
    let len = if k < 1.0 { k } else { k.sqrt() };
    let cnt = if k < 1.0 { 1.0 } else { k.min(3.0) };
    let share = len / k;
    fn walk(v: &mut toml::Value, key: &str, len: f64, cnt: f64, share: f64, basin: bool) {
        let scale_num = |x: &mut toml::Value, f: f64, keep_int: bool| match x {
            toml::Value::Float(n) => *n *= f,
            toml::Value::Integer(n) => {
                if keep_int {
                    *n = ((*n as f64 * f).round() as i64).max(if *n > 0 { 1 } else { 0 });
                } else {
                    *x = toml::Value::Float(*n as f64 * f);
                }
            }
            _ => {}
        };
        match v {
            toml::Value::Table(t) => {
                let is_basin = t.get("kind").and_then(|k| k.as_str()) == Some("basin");
                for (k, x) in t.iter_mut() {
                    walk(x, k, len, cnt, share, is_basin);
                }
            }
            toml::Value::Array(a) => {
                let numeric = a.iter().all(|x| x.is_float() || x.is_integer());
                if numeric && (key.ends_with("_m") || key == "count" || (basin && key == "extra")) {
                    for x in a.iter_mut() {
                        if key == "count" {
                            scale_num(x, cnt, true);
                        } else {
                            scale_num(x, len, false);
                        }
                    }
                } else {
                    for x in a.iter_mut() {
                        walk(x, key, len, cnt, share, basin);
                    }
                }
            }
            x @ (toml::Value::Float(_) | toml::Value::Integer(_)) => {
                if key == "count" {
                    scale_num(x, cnt, true);
                } else if key.ends_with("_m") || (basin && key == "extra") {
                    scale_num(x, len, false);
                } else if key == "roughness" || key == "boundary_noise" {
                    scale_num(x, share, false);
                }
            }
            _ => {}
        }
    }
    if let toml::Value::Table(t) = &mut v {
        for (key, x) in t.iter_mut() {
            if key != "radius_m" {
                walk(x, key, len, cnt, share, false);
            }
        }
        t.insert("radius_m".into(), toml::Value::Float(radius_m));
    }
    toml::to_string(&v).map_err(|e| e.to_string())
}
