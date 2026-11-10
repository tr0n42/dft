//! The planet generator for the browser. A tiny C-style API (no bindings
//! generator needed): the page writes text into memory it got from `alloc`,
//! calls `set_tables` and `set_body`, then asks for strips of pixels.
//!
//! Every pixel comes from the same `Planet::material` the game uses, so what the
//! viewer shows is exactly what the sim would generate for that spec and seed.

use dft_sim::elements::ElementTable;
use dft_sim::materials::{MatId, MaterialTable, State};
use dft_sim::planet::Planet;
use dft_sim::render;
use dft_sim::spec::BodySpec;
use std::cell::RefCell;

struct Viewer {
    sim: Option<dft_sim::swarm::Sim>,
    points: Vec<f32>,
    mats: Option<MaterialTable>,
    planet: Option<Planet>,
    rgba: Vec<u8>,
    ids: Vec<MatId>,
    field: Vec<f32>,
    text: String,
    /// How each material's blocks are drawn up close.
    kinds: Vec<dft_sim::texture::Kind>,
    /// The backdrop's ghost colours by block, kept between frames.
    ghosts: std::collections::HashMap<(i64, i64), [u8; 3]>,
    /// Textures and the backdrop on (1) or plain colours (0).
    looks: bool,
    /// Generated cells already asked for while running (the generator is
    /// deterministic, so they never go stale); bounded.
    made: std::collections::HashMap<(i64, i64), MatId>,
}

thread_local! {
    static V: RefCell<Viewer> = RefCell::new(Viewer {
        sim: None, points: Vec::new(), mats: None, planet: None, rgba: Vec::new(), ids: Vec::new(), field: Vec::new(), text: String::new(), kinds: Vec::new(), ghosts: Default::default(), looks: true, made: Default::default(),
    });
}

fn with<R>(f: impl FnOnce(&mut Viewer) -> R) -> R {
    V.with(|v| f(&mut v.borrow_mut()))
}

/// # Safety
/// The caller passes a pointer and length it got from `alloc`.
unsafe fn read(ptr: *const u8, len: usize) -> String {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf8_lossy(bytes).into_owned()
}

#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// # Safety
/// `ptr` and `len` must come from one `alloc` call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, len: usize) {
    drop(unsafe { Vec::from_raw_parts(ptr, 0, len.max(1)) });
}

/// The last message (an error, or JSON from `info`), as pointer and length.
#[unsafe(no_mangle)]
pub extern "C" fn text_ptr() -> *const u8 {
    with(|v| v.text.as_ptr())
}
#[unsafe(no_mangle)]
pub extern "C" fn text_len() -> usize {
    with(|v| v.text.len())
}

/// Load materials.toml and elements.toml. Returns 1 on success; on failure the
/// error is in the text buffer.
///
/// # Safety
/// Both pointer and length pairs must come from `alloc`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn set_tables(m_ptr: *const u8, m_len: usize, e_ptr: *const u8, e_len: usize) -> i32 {
    let (mt, et) = unsafe { (read(m_ptr, m_len), read(e_ptr, e_len)) };
    let r = ElementTable::parse(&et).and_then(|e| MaterialTable::parse(&mt, e));
    with(|v| match r {
        Ok(m) => {
            v.mats = Some(m);
            v.planet = None;
            1
        }
        Err(e) => {
            v.text = e;
            0
        }
    })
}

/// Build a body from a spec. A seed of -1 keeps the spec's own seed.
///
/// # Safety
/// The pointer and length must come from `alloc`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn set_body(s_ptr: *const u8, s_len: usize, seed: f64) -> i32 {
    let st = unsafe { read(s_ptr, s_len) };
    with(|v| {
        let Some(mats) = v.mats.clone() else {
            v.text = "load the material tables first".into();
            return 0;
        };
        let r = BodySpec::parse(&st).and_then(|mut spec| {
            if seed >= 0.0 {
                spec.seed = seed as u64;
            }
            Planet::new(spec, mats)
        });
        match r {
            Ok(p) => {
                v.kinds = dft_sim::texture::kinds(&p.mats);
                v.ghosts.clear();
                v.planet = Some(p);
                1
            }
            Err(e) => {
                v.text = e;
                0
            }
        }
    })
}

/// Resize a body spec to `radius_m` (see `spec::resize_text`); the result
/// goes into the text buffer. Returns 0 (and the error) if it can't.
///
/// # Safety
/// The pointer and length must come from `alloc`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn resize_spec(s_ptr: *const u8, s_len: usize, radius_m: f64) -> i32 {
    let st = unsafe { read(s_ptr, s_len) };
    with(|v| match dft_sim::spec::resize_text(&st, radius_m) {
        Ok(t) => {
            v.text = t;
            1
        }
        Err(e) => {
            v.text = e;
            0
        }
    })
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Describe the current body and the material table as JSON in the text buffer.
#[unsafe(no_mangle)]
pub extern "C" fn info() -> i32 {
    with(|v| {
        let Some(p) = &v.planet else { return 0 };
        let mut s = String::new();
        s.push_str(&format!(
            "{{\"name\":\"{}\",\"archetype\":\"{}\",\"seed\":{},\"radius\":{},\"max_radius\":{},\"atmosphere_m\":{},\"heart\":{},\"real_g\":{},\"game_g\":{},\"centre_pa\":{},\"scheme\":\"{}\",\"hue\":{:.0},\"layers\":[",
            esc(&p.spec.name),
            esc(&p.spec.archetype),
            p.spec.seed,
            p.radius,
            p.max_radius,
            p.spec.atmosphere_m,
            p.spec.heart,
            p.real_surface_gravity(),
            p.gravity,
            p.pressure_pa(0, 0),
            if p.spec.glamour.unwrap_or(0.0) > 0.0 { p.palette.scheme.name() } else { "natural" },
            p.palette.base_hue
        ));
        for (i, l) in p.spec.layer.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!(
                "{{\"name\":\"{}\",\"outer\":{},\"material\":\"{}\",\"temperature_k\":{}}}",
                esc(&l.name),
                l.outer,
                esc(&l.material),
                l.temperature_k
            ));
        }
        s.push_str("],\"materials\":[");
        for (i, m) in p.mats.list.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let state = match m.state {
                State::Solid => "solid",
                State::Powder => "powder",
                State::Liquid => "liquid",
                State::Gas => "gas",
            };
            let comp: Vec<String> =
                m.composition.iter().map(|(k, f)| format!("\"{}\":{}", esc(k), f)).collect();
            let value = p.mats.elements.value_per_kg(&m.composition) * m.multiplier;
            s.push_str(&format!(
                "{{\"name\":\"{}\",\"state\":\"{}\",\"density\":{},\"hardness\":{},\"colour\":[{},{},{}],\"value\":{},\"melts_at_k\":{},\"boils_at_k\":{},\"composition\":{{{}}}}}",
                esc(&m.name),
                state,
                m.density,
                m.hardness,
                p.palette.colours[i][0],
                p.palette.colours[i][1],
                p.palette.colours[i][2],
                if value.is_finite() { value } else { 0.0 },
                m.melts_at_k.map_or("null".into(), |t| t.to_string()),
                m.boils_at_k.map_or("null".into(), |t| t.to_string()),
                comp.join(",")
            ));
        }
        s.push_str("]}");
        v.text = s;
        1
    })
}

/// Render a strip `w` x `h` pixels. Pixel (col, row) shows world cell
/// floor(x_left + (col + 0.5) * mpp), floor(y_top - (row + 0.5) * mpp), with mpp
/// metres per pixel. Fills the RGBA and material id buffers, and the field
/// buffer with `field` 1 (starting temperature, K) or 2 (pressure, Pa); returns
/// how many pixels were written.
#[unsafe(no_mangle)]
pub extern "C" fn render(x_left: f64, y_top: f64, mpp: f64, w: u32, h: u32, field: i32) -> u32 {
    with(|v| {
        let Viewer { planet, rgba, ids, field: fbuf, kinds, ghosts, looks, .. } = v;
        let Some(p) = planet.as_ref() else { return 0 };
        let mut look = |wx: i64, wy: i64| p.material(wx, wy);
        let temp = |wx: i64, wy: i64, m: MatId| p.start_k(wx, wy, m);
        draw(p, &mut look, None, &temp, kinds, ghosts, *looks, rgba, ids, fbuf, x_left, y_top, mpp, w, h, field)
    })
}

/// Turn textures and the backdrop on or off (plain colours, as before).
#[unsafe(no_mangle)]
pub extern "C" fn set_looks(on: i32) {
    with(|v| v.looks = on != 0)
}

/// Draw a strip: every pixel's cell from `look`; where that is open space
/// that `pristine` says was ground, the backdrop. Zoomed in (4 or more
/// pixels per metre), each block is textured and bevelled.
#[allow(clippy::too_many_arguments)]
fn draw(
    p: &Planet,
    look: &mut dyn FnMut(i64, i64) -> MatId,
    pristine: Option<&dyn Fn(i64, i64) -> MatId>,
    temp: &dyn Fn(i64, i64, MatId) -> f64,
    kinds: &[dft_sim::texture::Kind],
    ghosts: &mut std::collections::HashMap<(i64, i64), [u8; 3]>,
    looks: bool,
    rgba: &mut Vec<u8>,
    ids: &mut Vec<MatId>,
    fbuf: &mut Vec<f32>,
    x_left: f64,
    y_top: f64,
    mpp: f64,
    w: u32,
    h: u32,
    field: i32,
) -> u32 {
    use dft_sim::materials::VACUUM;
    use dft_sim::texture::{self as tx, Sides};
    let n = (w * h) as usize;
    rgba.resize(n * 4, 0);
    ids.resize(n, 0);
    fbuf.resize(if field > 0 { n } else { 0 }, 0.0);
    let ppc = 1.0 / mpp;
    let detail = looks && ppc >= 4.0;
    // Cells seen this frame (the textures look at each block's neighbours).
    // (Zoomed out, each pixel is its own cell and a plain lookup is cheaper.)
    let mut seen: std::collections::HashMap<(i64, i64), MatId> = Default::default();
    let mut get = |x: i64, y: i64| if detail { *seen.entry((x, y)).or_insert_with(|| look(x, y)) } else { look(x, y) };
    // Whether an open cell is underground (eaten ground or a buried void),
    // the last answer kept for the run of pixels in the same cell.
    let mut last_open: Option<((i64, i64), bool, bool)> = None;
    if ghosts.len() > 300_000 {
        ghosts.clear();
    }
    // The backdrop sits further back: it slides past more slowly and looks
    // a little smaller (parallax about the middle of the view).
    let (cam_x, cam_y) = (x_left + w as f64 * mpp / 2.0, y_top - h as f64 * mpp / 2.0);
    // The ground's radius by direction, worked out once per frame instead of
    // once per pixel (drawing only; the sim never uses it). Fine enough that
    // between samples the ground moves less than a pixel.
    let circ = 2.0 * std::f64::consts::PI * p.max_radius;
    let n_dirs = ((circ / mpp.max(0.25)) as usize).clamp(1024, 1 << 20);
    let mut surf_tab: Vec<f32> = Vec::new();
    let surface_at = |fx: f64, fy: f64, tab: &mut Vec<f32>| -> f64 {
        if tab.is_empty() {
            // Filled on first use: views with no ground in them skip it.
            tab.resize(n_dirs + 1, 0.0);
            for (k, v) in tab.iter_mut().enumerate() {
                let a = std::f64::consts::TAU * (k % n_dirs) as f64 / n_dirs as f64;
                *v = p.surface_radius(a.sin(), a.cos()) as f32;
            }
        }
        let a = fx.atan2(fy).rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU * n_dirs as f64;
        let k = a.floor() as usize % n_dirs;
        let t = a - a.floor();
        tab[k] as f64 * (1.0 - t) + tab[k + 1] as f64 * t
    };
    const DEPTH: f64 = 1.15;
    let mut i = 0;
    for row in 0..h {
        let wyf = y_top - (row as f64 + 0.5) * mpp;
        let wy = wyf.floor() as i64;
        let vv = wyf - wy as f64;
        for col in 0..w {
            let wxf = x_left + (col as f64 + 0.5) * mpp;
            let wx = wxf.floor() as i64;
            let uu = wxf - wx as f64;
            let m = get(wx, wy);
            let mut c;
            if m == VACUUM {
                let (eaten, underground) = match last_open {
                    Some((k, e, u)) if k == (wx, wy) => (e, u),
                    _ => {
                        let (fx, fy) = (wx as f64 + 0.5, wy as f64 + 0.5);
                        let r = (fx * fx + fy * fy).sqrt().max(1e-9);
                        let below = r <= p.max_radius && r < surface_at(fx, fy, &mut surf_tab) && !p.in_crater(fx, fy);
                        // Only ground below the surface can have been eaten.
                        // (With the backdrop on, open ground underground is
                        // drawn the same eaten or not, so the generator is
                        // only asked when it matters.)
                        let eaten = below && !looks && pristine.is_some_and(|pr| pr(wx, wy) != VACUUM);
                        last_open = Some(((wx, wy), eaten, below));
                        (eaten, below)
                    }
                };
                if looks && underground && ppc < 0.5 {
                    // Far out, the ghost alone (one cached lookup), no detail.
                    let g = (wx.div_euclid(tx::GHOST_M) * tx::GHOST_M, wy.div_euclid(tx::GHOST_M) * tx::GHOST_M);
                    let gh = *ghosts.entry(g).or_insert_with(|| tx::ghost(p, g.0 + 1, g.1 + 1));
                    c = if gh == tx::VOID && pristine.is_some_and(|pr| pr(wx, wy) != VACUUM) { tx::ghost(p, wx, wy) } else { gh };
                } else if looks && underground {
                    let (bx, by) = (cam_x + (wxf - cam_x) * DEPTH, cam_y + (wyf - cam_y) * DEPTH);
                    // The four ghost blocks around this point, blended.
                    let gm = tx::GHOST_M as f64;
                    let (gi, gj) = ((bx / gm - 0.5).floor() as i64, (by / gm - 0.5).floor() as i64);
                    let mut four = [[0u8; 3]; 4];
                    for (k, (di, dj)) in [(0, 0), (1, 0), (0, 1), (1, 1)].into_iter().enumerate() {
                        let g = ((gi + di) * tx::GHOST_M, (gj + dj) * tx::GHOST_M);
                        four[k] = *ghosts.entry(g).or_insert_with(|| tx::ghost(p, g.0 + 1, g.1 + 1));
                    }
                    // Where the shifted view looks past the old surface, show
                    // this cell's own ghost instead of a hole.
                    if four.contains(&tx::VOID) && pristine.is_some_and(|pr| pr(wx, wy) != VACUUM) {
                        let own = (wx.div_euclid(tx::GHOST_M) * tx::GHOST_M, wy.div_euclid(tx::GHOST_M) * tx::GHOST_M);
                        let mut fill = *ghosts.entry(own).or_insert_with(|| tx::ghost(p, own.0 + 1, own.1 + 1));
                        if fill == tx::VOID {
                            fill = tx::ghost(p, wx, wy);
                        }
                        for q in four.iter_mut().filter(|q| **q == tx::VOID) {
                            *q = fill;
                        }
                    }
                    c = tx::backdrop(four, bx, by);
                    // Ground standing over the gap casts a shadow into it.
                    if detail {
                        let e = (2.5 / ppc).max(0.15);
                        let mut f = 1.0;
                        if vv > 1.0 - e && get(wx, wy + 1) != VACUUM {
                            f *= 0.55;
                        }
                        if uu < e && get(wx - 1, wy) != VACUUM {
                            f *= 0.75;
                        }
                        if uu > 1.0 - e && get(wx + 1, wy) != VACUUM {
                            f *= 0.75;
                        }
                        c = [(c[0] as f64 * f) as u8, (c[1] as f64 * f) as u8, (c[2] as f64 * f) as u8];
                    }
                } else if eaten {
                    c = render::space(p, wx, wy);
                } else {
                    c = render::colour(p, m, wx, wy);
                }
            } else {
                let (fx, fy) = (wx as f64 + 0.5, wy as f64 + 0.5);
                c = render::colour_at(p, m, wx, wy, surface_at(fx, fy, &mut surf_tab));
                if looks && ppc >= 1.0 {
                    let sides = if detail {
                        let ns = [get(wx, wy + 1), get(wx, wy - 1), get(wx - 1, wy), get(wx + 1, wy)];
                        Sides { open: ns.map(|q| q == VACUUM), other: ns.map(|q| q != m) }
                    } else {
                        Sides::default()
                    };
                    c = tx::texel(kinds.get(m as usize).copied().unwrap_or(tx::Kind::Rock), c, wx, wy, uu, vv, ppc, sides);
                }
            }
            rgba[i * 4..i * 4 + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
            ids[i] = m;
            if field > 0 {
                fbuf[i] = match field {
                    1 => temp(wx, wy, m) as f32,
                    2 => p.pressure_pa(wx, wy) as f32,
                    _ => 0.0,
                };
            }
            i += 1;
        }
    }
    n as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn rgba_ptr() -> *const u8 {
    with(|v| v.rgba.as_ptr())
}
#[unsafe(no_mangle)]
pub extern "C" fn ids_ptr() -> *const MatId {
    with(|v| v.ids.as_ptr())
}
#[unsafe(no_mangle)]
pub extern "C" fn field_ptr() -> *const f32 {
    with(|v| v.field.as_ptr())
}

/// The material id at one cell, for the hover readout.
#[unsafe(no_mangle)]
pub extern "C" fn cell(x: f64, y: f64) -> i32 {
    with(|v| v.planet.as_ref().map_or(-1, |p| p.material(x as i64, y as i64) as i32))
}

/// The pressure at one cell, in pascals.
#[unsafe(no_mangle)]
pub extern "C" fn cell_pa(x: f64, y: f64) -> f64 {
    with(|v| v.planet.as_ref().map_or(0.0, |p| p.pressure_pa(x as i64, y as i64)))
}

/// The starting temperature of one cell, in kelvin.
#[unsafe(no_mangle)]
pub extern "C" fn cell_k(x: f64, y: f64) -> f64 {
    with(|v| {
        v.planet.as_ref().map_or(0.0, |p| {
            let (x, y) = (x as i64, y as i64);
            p.start_k(x, y, p.material(x, y))
        })
    })
}

// ---------------------------------------------------------------------------
// Run mode: the full simulation (swarm, sandbox, falling rock) in the browser.
// ---------------------------------------------------------------------------

/// Start a run on the current body with an economy (economy.toml text). The
/// body must have been set with `set_body` first.
///
/// # Safety
/// The pointer and length must come from `alloc`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn run_new(e_ptr: *const u8, e_len: usize) -> i32 {
    let et = unsafe { read(e_ptr, e_len) };
    with(|v| {
        let Some(p) = v.planet.take() else {
            v.text = "set a body first".into();
            return 0;
        };
        match dft_sim::economy::Economy::parse(&et) {
            Ok(eco) => {
                v.made.clear();
                v.ghosts.clear();
                v.sim = Some(dft_sim::swarm::Sim::new(dft_sim::world::World::new(p), eco));
                1
            }
            Err(e) => {
                v.text = e;
                0
            }
        }
    })
}

/// Advance the run by up to `n` logic ticks; returns how many ran (fewer if
/// the swarm launched).
#[unsafe(no_mangle)]
pub extern "C" fn run_ticks(n: u32) -> u32 {
    with(|v| {
        let Some(s) = v.sim.as_mut() else { return 0 };
        let mut done = 0;
        while done < n && s.launched_at.is_none() {
            s.step();
            done += 1;
        }
        done
    })
}

/// Live numbers for the readout, as JSON in the text buffer.
#[unsafe(no_mangle)]
pub extern "C" fn run_info() -> i32 {
    with(|v| {
        let Some(s) = v.sim.as_ref() else { return 0 };
        let individual = s.drone_count();
        let txt = format!(
            "{{\"t\":{},\"drones\":{},\"packet\":{},\"consumed\":{},\"energy\":{},\"mass\":{},\"ready_at\":{},\"launched_at\":{},\"seed_mass\":{},\"crushed\":{},\"burned\":{},\"awake\":{},\"loose\":{},\"deaths\":{},\"purchases\":{}}}",
            s.time(),
            individual,
            s.packet,
            s.consumed_share(),
            s.energy,
            s.mass,
            s.ready_at.map_or("null".into(), |t| t.to_string()),
            s.launched_at.map_or("null".into(), |t| t.to_string()),
            s.seed_mass,
            s.crushed,
            s.burned,
            s.sandbox.awake(),
            s.bodies.count(),
            s.deaths,
            s.purchases.len()
        );
        v.text = txt;
        1
    })
}

/// Render the running world, like `render` (field 1 shows live temperature).
#[unsafe(no_mangle)]
pub extern "C" fn run_render(x_left: f64, y_top: f64, mpp: f64, w: u32, h: u32, field: i32) -> u32 {
    with(|v| {
        let Viewer { sim, rgba, ids, field: fbuf, kinds, ghosts, looks, made, .. } = v;
        let Some(s) = sim.as_ref() else { return 0 };
        let p = &s.world.planet;
        if kinds.len() != p.mats.list.len() {
            *kinds = dft_sim::texture::kinds(&p.mats);
        }
        if made.len() > 1_500_000 {
            made.clear();
        }
        // Cells the store holds come from it; cells still as generated come
        // from the cache (the generator is the slow part of a frame).
        let mut look = |wx: i64, wy: i64| match s.world.material_stored(wx, wy) {
            Some(m) => m,
            None => *made.entry((wx, wy)).or_insert_with(|| p.material(wx, wy)),
        };
        let pristine = |wx: i64, wy: i64| p.material(wx, wy);
        let temp = |wx: i64, wy: i64, m: MatId| s.sandbox.temp(&s.world, (wx, wy), m);
        draw(p, &mut look, Some(&pristine), &temp, kinds, ghosts, *looks, rgba, ids, fbuf, x_left, y_top, mpp, w, h, field)
    })
}

/// Drone positions as (x, y, state) triples; returns how many drones.
#[unsafe(no_mangle)]
pub extern "C" fn run_drones() -> u32 {
    with(|v| {
        let Some(s) = v.sim.as_ref() else { return 0 };
        s.drone_points(&mut v.points);
        (v.points.len() / 3) as u32
    })
}

/// Hubs as (x, y, kind) triples; returns how many.
#[unsafe(no_mangle)]
pub extern "C" fn run_hubs() -> u32 {
    with(|v| {
        let Some(s) = v.sim.as_ref() else { return 0 };
        s.hub_points(&mut v.points);
        (v.points.len() / 3) as u32
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn points_ptr() -> *const f32 {
    with(|v| v.points.as_ptr())
}

/// The run's state fingerprint, as two 32-bit halves (high, then low).
#[unsafe(no_mangle)]
pub extern "C" fn run_hash_hi() -> u32 {
    with(|v| v.sim.as_ref().map_or(0, |s| (s.state_hash() >> 32) as u32))
}
#[unsafe(no_mangle)]
pub extern "C" fn run_hash_lo() -> u32 {
    with(|v| v.sim.as_ref().map_or(0, |s| s.state_hash() as u32))
}
