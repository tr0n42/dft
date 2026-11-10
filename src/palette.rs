//! Colour schemes: each body draws its materials in its own palette.
//!
//! Only how things look changes; what a cell is made of never does. At
//! glamour 0 every material keeps its natural colour (the locked look). As
//! glamour rises, each body picks a colour harmony from the colour wheel
//! (analogous, complementary, split-complementary, triadic or square) around
//! a base hue drawn from its seed, and its materials are steered onto it:
//!
//! - the ground (crust rock, sediments, dust) toward the base hue,
//! - the deep rock (mantle) toward the scheme's second hue,
//! - ices and cool liquids toward the scheme's cool counterpart,
//! - gems, crystals and exotic materials onto the scheme's accent hues,
//!   saturated (from about 0.5 they may take colours no real gem has),
//! - metals only a touch, so they still read as metal,
//! - glowing melts (magma, molten iron) stay as they are until 0.85, so
//!   danger always looks like danger, then they too may change colour.
//!
//! Lightness is kept, so layers and boundaries stay as easy to tell apart as
//! before; each material also keeps a small hue offset of its own so
//! neighbours never merge. The work is done in OKLab, a colour space where
//! equal steps look equal, so hues shift without muddying.
use crate::materials::{MaterialTable, State};
use crate::noise::{hash, unit};
use crate::texture::Kind;

// ---- sRGB <-> OKLab (Björn Ottosson's matrices) ----

fn to_linear(c: u8) -> f64 {
    let c = c as f64 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn from_linear(c: f64) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

/// sRGB to OKLCh: lightness (0..1), chroma (0..~0.37), hue (degrees).
pub fn to_lch(c: [u8; 3]) -> (f64, f64, f64) {
    let (r, g, b) = (to_linear(c[0]), to_linear(c[1]), to_linear(c[2]));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    let ll = 0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s;
    let a = 1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s;
    let bb = 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s;
    (ll, (a * a + bb * bb).sqrt(), bb.atan2(a).to_degrees().rem_euclid(360.0))
}

/// OKLCh to sRGB. Out-of-gamut colours lose chroma until they fit.
pub fn from_lch(l: f64, c: f64, h: f64) -> [u8; 3] {
    let mut c = c;
    for _ in 0..24 {
        let (a, b) = (c * h.to_radians().cos(), c * h.to_radians().sin());
        let l_ = (l + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
        let m_ = (l - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
        let s_ = (l - 0.089_484_177_5 * a - 1.291_485_548 * b).powi(3);
        let r = 4.076_741_662_1 * l_ - 3.307_711_591_3 * m_ + 0.230_969_929_2 * s_;
        let g = -1.268_438_004_6 * l_ + 2.609_757_401_1 * m_ - 0.341_319_396_5 * s_;
        let bl = -0.004_196_086_3 * l_ - 0.703_418_614_7 * m_ + 1.707_614_701 * s_;
        let tol = 1e-4;
        if (-tol..=1.0 + tol).contains(&r) && (-tol..=1.0 + tol).contains(&g) && (-tol..=1.0 + tol).contains(&bl) || c < 1e-4 {
            return [from_linear(r), from_linear(g), from_linear(bl)];
        }
        c *= 0.85;
    }
    let v = from_linear(l * l * l);
    [v, v, v]
}

// ---- Harmonies ----

/// A colour harmony: hue offsets (degrees) from the base hue, in role order
/// [ground, deep, cool, accents...].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scheme {
    Analogous,
    Complementary,
    SplitComplementary,
    Triadic,
    Square,
}

impl Scheme {
    pub const ALL: [Scheme; 5] = [Scheme::Analogous, Scheme::Complementary, Scheme::SplitComplementary, Scheme::Triadic, Scheme::Square];

    pub fn parse(s: &str) -> Option<Scheme> {
        Scheme::ALL.into_iter().find(|k| k.name() == s)
    }

    pub fn name(self) -> &'static str {
        match self {
            Scheme::Analogous => "analogous",
            Scheme::Complementary => "complementary",
            Scheme::SplitComplementary => "split-complementary",
            Scheme::Triadic => "triadic",
            Scheme::Square => "square",
        }
    }

    /// Hue offsets: ground, deep, cool, then the accents.
    pub fn offsets(self) -> &'static [f64] {
        match self {
            // Neighbours on the wheel: calm, one family; accents a little apart.
            Scheme::Analogous => &[0.0, 30.0, -30.0, 60.0, -60.0],
            // Opposites: the ground one way, ice and gems the other.
            Scheme::Complementary => &[0.0, 20.0, 180.0, 180.0, 160.0, 200.0],
            // One side, and the two neighbours of its opposite.
            Scheme::SplitComplementary => &[0.0, 25.0, 150.0, 210.0, 150.0, 180.0],
            // Three evenly spaced.
            Scheme::Triadic => &[0.0, 120.0, 240.0, 120.0, 240.0, 60.0],
            // Four evenly spaced.
            Scheme::Square => &[0.0, 90.0, 180.0, 270.0, 90.0, 180.0],
        }
    }
}

/// How a material takes part in the scheme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Role {
    Ground,
    Deep,
    Cool,
    Metal,
    Gem,
    Glow,
    Fixed,
}

const DEEP: [&str; 15] = ["perovskite", "ringwoodite", "harzburgite", "pyroxenite", "plume rock", "deep pile", "eclogite", "peridotite", "lherzolite", "dunite", "serpentinite", "majorite", "ferropericlase", "kimberlite", "gabbro"];
const EXOTIC: [&str; 4] = ["resonant", "prismatic", "void", "metallic hydrogen"];

pub fn roles(mats: &MaterialTable) -> Vec<Role> {
    let kinds = crate::texture::kinds(mats);
    mats.list
        .iter()
        .zip(kinds)
        .map(|(m, k)| {
            let n = m.name.as_str();
            if n == "vacuum" || n == "heart" || n == "drone-scrap" {
                return Role::Fixed;
            }
            if m.state == State::Gas {
                return Role::Cool;
            }
            if m.state == State::Liquid {
                // Melts glow (everything liquid but the cold seas).
                let hot = m.freezes_at_k.is_some_and(|f| f > 500.0);
                return if hot { Role::Glow } else { Role::Cool };
            }
            if EXOTIC.iter().any(|e| n.contains(e)) || k == Kind::Crystal && n != "obsidian" {
                return Role::Gem;
            }
            match k {
                Kind::Metal => Role::Metal,
                Kind::Ice => Role::Cool,
                _ if DEEP.iter().any(|d| n.contains(d)) => Role::Deep,
                _ => Role::Ground,
            }
        })
        .collect()
}

/// A body's palette: its scheme and base hue, and the colour of every material.
#[derive(Clone, Debug)]
pub struct Palette {
    pub scheme: Scheme,
    pub base_hue: f64,
    pub colours: Vec<[u8; 3]>,
    pub atmosphere: [u8; 3],
}

fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Turn hue `h` toward `to` by share `w` along the short way round.
fn steer(h: f64, to: f64, w: f64) -> f64 {
    let d = (to - h + 540.0).rem_euclid(360.0) - 180.0;
    (h + d * w).rem_euclid(360.0)
}

impl Palette {
    /// The natural colours, unchanged (glamour 0).
    pub fn natural(mats: &MaterialTable, atmosphere: [u8; 3]) -> Palette {
        Palette { scheme: Scheme::Analogous, base_hue: 0.0, colours: mats.list.iter().map(|m| m.colour).collect(), atmosphere }
    }

    /// The palette for a body with this seed and glamour; `scheme` and
    /// `hue` (degrees) pin the harmony and base hue instead of drawing them.
    pub fn build(mats: &MaterialTable, atmosphere: [u8; 3], seed: u64, glamour: f64, scheme: Option<Scheme>, hue: Option<f64>) -> Palette {
        let g = glamour.clamp(0.0, 1.0);
        if g <= 0.0 {
            return Self::natural(mats, atmosphere);
        }
        let hs = hash(seed, 0, 0, 0xC010);
        let scheme = scheme.unwrap_or(Scheme::ALL[(unit(hs) * Scheme::ALL.len() as f64) as usize % Scheme::ALL.len()]);
        let base = hue.unwrap_or(unit(hash(hs, 1, 0, 0)) * 360.0).rem_euclid(360.0);
        let off = scheme.offsets();
        let hue = |k: usize| (base + off[k.min(off.len() - 1)]).rem_euclid(360.0);
        let accents = &off[3..];
        // How far each role is steered at this glamour.
        let ground_w = 0.8 * smooth(g / 0.8);
        let cool_w = 0.65 * smooth(g / 0.8);
        let metal_w = 0.3 * smooth(g / 0.8);
        let gem_w = smooth(g / 0.5);
        let glow_w = smooth((g - 0.85) / 0.15);
        // Grey rock picks up some colour of the scheme as it is steered.
        let tint = (0.03 + 0.06 * g) * smooth(g / 0.8);
        let roles = roles(mats);
        let colours = mats
            .list
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let (l, c, h) = to_lch(m.colour);
                let mh = hash(seed, i as i64, 0, 0xC011);
                // Each material keeps a little hue of its own around its target.
                let jitter = (unit(mh) - 0.5) * 28.0;
                let (l, c, h) = match roles[i] {
                    Role::Fixed => return m.colour,
                    Role::Ground => (l, c + (c.max(tint) - c) * ground_w / 0.8, steer(h, hue(0) + jitter, ground_w)),
                    Role::Deep => (l, c + (c.max(tint * 1.3) - c) * ground_w / 0.8, steer(h, hue(1) + jitter, ground_w)),
                    Role::Cool => (l, c + (c.max(tint) - c) * cool_w / 0.65, steer(h, hue(2) + jitter * 0.5, cool_w)),
                    Role::Metal => (l, c.min(0.06) + tint * 0.3 * metal_w, steer(h, hue(0) + jitter, metal_w)),
                    Role::Gem => {
                        // An accent hue of the scheme; past 0.7 any of them,
                        // natural or not, and richly saturated.
                        let k = (unit(hash(mh, 1, 0, 0)) * accents.len() as f64) as usize % accents.len();
                        let target = (base + accents[k] + jitter).rem_euclid(360.0);
                        let w = if g < 0.7 { gem_w * 0.6 } else { gem_w };
                        let c2 = c.max(0.06 + 0.1 * smooth(g / 0.8));
                        (l.clamp(0.35, 0.9), c + (c2 - c) * gem_w, steer(h, target, w))
                    }
                    Role::Glow => {
                        let target = (base + accents[0]).rem_euclid(360.0);
                        (l, c, steer(h, target, glow_w))
                    }
                };
                from_lch(l, c, h)
            })
            .collect();
        let (al, ac, ah) = to_lch(atmosphere);
        let atmosphere = if atmosphere == [0, 0, 0] { atmosphere } else { from_lch(al, ac.max(0.05 * cool_w), steer(ah, hue(2), cool_w)) };
        Palette { scheme, base_hue: base, colours, atmosphere }
    }
}
