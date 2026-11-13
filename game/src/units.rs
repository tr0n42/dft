//! The specialists on screen: sappers, tunnelers, heat-sink bots and
//! foresters (abductors use the forester's frame until animals exist), drawn
//! the way the swarm mockup designed them, in one-metre blocks, upright to
//! the body (a tunneler along its shaft), every block joined edge to edge
//! with the rest. Lights are the swarm's colour.
//!
//! Called from the drones' draw with the haulers' mesh: each block is two
//! triangles turned with its unit (see `rot_square` in drones.rs).
use crate::view::View;
use bevy::prelude::*;
use dft_sim::production::Unit;
use dft_sim::swarm::UnitView;

/// A sapper, top row first: A hull, n charge, L light.
const SAPPER: [&str; 3] = [" AAA ", "AnnLA", "A A A"];

/// A heat-sink bot, top row first: r radiator fins, A armour, D dark, z
/// lens, L light, p strut.
const HEATSINK: [&str; 11] = [
    " r r r r r r r ",
    " r r r r r r r ",
    " AAAAAAAAAAAAA ",
    "AAAAAAAAAAAAAAA",
    "AADDDDDDDDDDDAA",
    "AADzDDDLDDDzDAA",
    "AADDDDDDDDDDDAA",
    "AAAAAAAAAAAAAAA",
    " AAAAAAAAAAAAA ",
    "  p  p   p  p  ",
    "  p  p   p  p  ",
];

/// The tunneler, nose to the right (+u), 44 long and 14 high, as the
/// mockup builds it: A armour, p plate edge, j joint, b bone, s sinew, L
/// light, d/D the turning drill cone.
fn tunneler_rows() -> Vec<Vec<u8>> {
    let mut rows = Vec::new();
    for y in 0..14i32 {
        let mut row = Vec::new();
        for u in 0..44i32 {
            let v = y - 6;
            let av = v.abs();
            let mut ch = b' ';
            if u >= 31 {
                let half = 6.5 * (44 - u) as f32 / 13.0;
                if av as f32 <= half {
                    ch = if (u + v).rem_euclid(3) == 0 { b'D' } else { b'd' };
                }
            } else {
                let half = if u < 4 { 2 + u } else { 5 };
                if av <= half {
                    ch = if u % 6 == 5 {
                        b'j'
                    } else if av == half {
                        b'p'
                    } else if u % 6 == 2 && av < 2 {
                        if u > 26 {
                            b'L'
                        } else {
                            b'b'
                        }
                    } else {
                        b'A'
                    };
                } else if v > 0 && av <= half + 2 && u % 3 == 1 && u < 30 {
                    ch = if av <= half + 1 { b's' } else { b'b' };
                } else if v < 0 && av == half + 1 && u % 6 == 2 && u < 30 {
                    ch = b'b';
                }
            }
            row.push(ch);
        }
        rows.push(row);
    }
    rows
}

fn lin(r: f32, g: f32, b: f32) -> [f32; 4] {
    let k = Color::srgb(r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0)).to_linear();
    [k.red, k.green, k.blue, 1.0]
}

fn lina(r: f32, g: f32, b: f32, a: f32) -> [f32; 4] {
    let mut c = lin(r, g, b);
    c[3] = a;
    c
}

/// A number in [0, 1) from a few integers (for each block's own shade).
fn shade(a: u32, b: i32, c: i32) -> f32 {
    dft_sim::noise::unit(dft_sim::noise::hash(a as u64, b as i64, c as i64, 0x0B17)) as f32
}

fn armour(id: u32, u: i32, w: i32) -> [f32; 4] {
    let k = 0.27 + 0.07 * shade(id, u, w);
    lin(k, k * 1.06, k * 1.04)
}

fn sinew(id: u32, u: i32, w: i32) -> [f32; 4] {
    let k = 0.11 + 0.05 * shade(id, u, w);
    lin(k * 0.9, k * 0.97, k * 1.14)
}

fn bone(id: u32, u: i32, w: i32) -> [f32; 4] {
    let k = 0.37 + 0.07 * shade(id, u, w);
    lin(k * 0.95, k, k * 1.05)
}

/// One unit's blocks: in its own frame (u along, w up, in metres; whole
/// metres keep the blocks on its grid), turned by `angle` about its middle
/// at `p` on screen.
struct Pen<'a> {
    pos: &'a mut Vec<[f32; 3]>,
    col: &'a mut Vec<[f32; 4]>,
    p: Vec2,
    half: f32,
    angle: f32,
}

impl Pen<'_> {
    fn block(&mut self, u: f32, w: f32, c: [f32; 4]) {
        crate::drones::rot_square(self.pos, self.p.x, self.p.y, self.half, self.angle, u, w, 1.0);
        self.col.extend_from_slice(&[c; 6]);
    }

    /// A run of blocks from one cell to another, one axis at a time, so
    /// it is always joined edge to edge (never a diagonal). `colour` gets
    /// the step's index and the cell.
    fn line(&mut self, a: (i32, i32), b: (i32, i32), mut colour: impl FnMut(usize, i32, i32) -> [f32; 4]) -> usize {
        let (mut x, mut y) = a;
        let mut k = 0;
        self.block(x as f32, y as f32, colour(k, x, y));
        for _ in 0..400 {
            if (x, y) == b {
                break;
            }
            let (ex, ey) = ((b.0 - x).abs(), (b.1 - y).abs());
            if ex >= ey {
                x += (b.0 - x).signum();
            } else {
                y += (b.1 - y).signum();
            }
            k += 1;
            self.block(x as f32, y as f32, colour(k, x, y));
        }
        k + 1
    }

    /// A shape of rows (top row first), its lower-left corner at (u0, w0).
    fn rows(&mut self, rows: &[&str], u0: i32, w0: i32, mut colour: impl FnMut(u8, i32, i32) -> Option<[f32; 4]>) {
        let h = rows.len() as i32;
        for (r, line) in rows.iter().enumerate() {
            for (c, ch) in line.bytes().enumerate() {
                if ch == b' ' {
                    continue;
                }
                let (u, w) = (u0 + c as i32, w0 + h - 1 - r as i32);
                if let Some(k) = colour(ch, u, w) {
                    self.block(u as f32, w as f32, k);
                }
            }
        }
    }
}

fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// A quadratic curve from a to c bending toward b, in n steps (the mockup's bez).
fn bez(a: (f32, f32), b: (f32, f32), c: (f32, f32), n: usize) -> Vec<(f32, f32)> {
    (0..=n)
        .map(|i| {
            let u = i as f32 / n as f32;
            let (p, q, z) = ((1.0 - u) * (1.0 - u), 2.0 * (1.0 - u) * u, u * u);
            (p * a.0 + q * b.0 + z * c.0, p * a.1 + q * b.1 + z * c.1)
        })
        .collect()
}

/// A walker's legs (see rig.rs): the crab's four, two a side, stepping in
/// diagonal pairs; the spider's three long curved legs, one at a time, as
/// the mockup's tripod stands, facing its way. Both scale with the body's
/// height (50 m over the tallest plant).
fn rig_of(un: &UnitView, face: f32) -> crate::rig::RigSpec {
    use crate::rig::{LegSpec, RigSpec};
    if un.kind == Unit::Forester {
        let h = un.lift.max(50.0);
        let k = h / 50.0;
        let hips = [-15.0f32, -6.0, 6.0, 15.0];
        let rest = [-34.0f32, -13.0, 13.0, 34.0];
        let groups = [0u8, 1, 0, 1];
        RigSpec {
            legs: (0..4).map(|i| LegSpec { hip: (hips[i], 3.0), rest: rest[i] * k, bones: vec![0.48, 0.52], group: groups[i], bend: 0.8 }).collect(),
            height: h,
            step_s: 0.35 * k.sqrt(),
            lift_m: 6.0 * k.sqrt(),
            trigger_m: 7.0 * k,
            reach: 1.15,
            sag_m: 3.0,
        }
    } else {
        let h = un.lift.max(62.0);
        let k = h / 62.0;
        let hips = [6.0f32, 9.0, 12.0];
        let rest = [-26.0f32, 4.0, 30.0];
        RigSpec {
            legs: (0..3).map(|i| LegSpec { hip: (hips[i] * face, 3.0), rest: rest[i] * k * face, bones: vec![0.3, 0.3, 0.24, 0.16], group: i as u8, bend: 0.7 }).collect(),
            height: h,
            step_s: 0.4 * k.sqrt(),
            lift_m: 7.0 * k.sqrt(),
            trigger_m: 8.0 * k,
            reach: 1.35,
            sag_m: 4.0,
        }
    }
}

/// Draw every specialist in view into the haulers' mesh.
#[allow(clippy::too_many_arguments)]
pub fn draw(units: &[UnitView], now: f64, real_t: f64, v: &View, bounds: [f64; 4], cell_px: f32, lc: [f32; 3], solid: &dyn Fn(i64, i64) -> bool, pos: &mut Vec<[f32; 3]>, col: &mut Vec<[f32; 4]>) {
    let [l, b, r, t] = bounds;
    let tt = real_t as f32;
    let tunneler = tunneler_rows();
    for un in units {
        let (x, y) = un.at(now);
        let reach = 60.0 + un.a.abs().max(0.0) as f64 + un.lift as f64;
        if x < l - reach || x > r + reach || y < b - reach || y > t + reach {
            continue;
        }
        let p = v.to_screen(x, y);
        let a = y.atan2(x);
        let up_angle = (a - std::f64::consts::FRAC_PI_2) as f32;
        let lamp = crate::lights::Lights::pulse(real_t, a);
        let light = |k: f32| lin(lc[0] * k, lc[1] * k, lc[2] * k);
        // A point in the world in this unit's own frame (metres along, up).
        let local = |q: (f64, f64)| -> (f32, f32) {
            let (dx, dy) = (q.0 - x, q.1 - y);
            let (s, c) = (up_angle as f64).sin_cos();
            ((dx * c + dy * s) as f32, (-dx * s + dy * c) as f32)
        };
        let half = cell_px * 0.5;
        match un.kind {
            Unit::Sapper => {
                let mut pen = Pen { pos: &mut *pos, col: &mut *col, p, half, angle: up_angle };
                let armed = un.state == 3;
                // Its light blinks, faster and faster once armed; the charge
                // glows in its back.
                let blink = if armed { (tt * (8.0 + un.a * 14.0)).sin() > 0.0 } else { (tt * 3.0 + un.id as f32).sin() > 0.5 };
                let charge = if armed { 0.6 + 0.4 * (tt * 20.0).sin() } else { 0.3 };
                pen.rows(&SAPPER, -2, -1, |ch, u, w| {
                    Some(match ch {
                        b'A' => armour(un.id, u, w),
                        b'L' => {
                            if blink {
                                light(lamp)
                            } else {
                                light(0.3)
                            }
                        }
                        _ => lin(0.35 + 0.65 * charge, 0.27 + 0.6 * charge, 0.12 + 0.35 * charge),
                    })
                });
                match un.state {
                    // Dropping on its thruster.
                    0 => {
                        pen.block(0.0, -2.0, lin(1.0, 0.86, 0.6));
                        pen.block(0.0, -3.0, lina(1.0, 0.55, 0.24, 0.8));
                    }
                    // Drilling: the bit turning, a spark.
                    2 => {
                        let spin = ((tt * 20.0) as i32 % 2) as f32;
                        pen.block(-1.0 + spin, -2.0, lin(0.89, 0.88, 0.84));
                        pen.block(0.0 - spin, -2.0, lin(0.59, 0.59, 0.61));
                        pen.block(0.0, -3.0, lina(1.0, 0.86, 0.6, 0.9));
                        // Hot flecks at the bit, and dust out of the hole.
                        let (sa, ca) = (up_angle as f64).sin_cos();
                        let (bx, by) = (x + 2.0 * sa, y - 2.0 * ca);
                        if dft_sim::noise::unit(dft_sim::noise::hash(un.id as u64, (real_t * 40.0) as i64, 0, 0xD1)) < 0.5 {
                            crate::fx::spark(bx, by, 1, [255.0, 210.0, 140.0], 10.0);
                        }
                        crate::fx::emit(bx, by, 4.0, [255.0, 190.0, 110.0], 0.6);
                        if dft_sim::noise::unit(dft_sim::noise::hash(un.id as u64, (real_t * 8.0) as i64, 1, 0xD2)) < 0.13 {
                            crate::fx::dust(un.mark.0, un.mark.1 + 1.0, [110.0, 100.0, 88.0], 12.0, 4.0, 4.0);
                        }
                    }
                    _ => {}
                }
                if blink {
                    crate::fx::emit(x, y, if armed { 6.0 } else { 3.0 }, [lc[0] * 255.0, lc[1] * 255.0, lc[2] * 255.0], if armed { 1.0 } else { 0.4 });
                }
            }
            Unit::Tunneler => {
                // Along its shaft, nose (the drill) at the head.
                let heading = (un.b as f32).atan2(un.a as f32);
                let mut pen = Pen { pos: &mut *pos, col: &mut *col, p, half, angle: heading };
                let turn = (tt * 6.0) as i32;
                for (row, line) in tunneler.iter().enumerate() {
                    for (c, &ch) in line.iter().enumerate() {
                        if ch == b' ' {
                            continue;
                        }
                        let (u, w) = (c as i32 - 44, 6 - row as i32);
                        let k = match ch {
                            b'A' => armour(un.id, u, w),
                            b'p' => {
                                let k = 0.2 + 0.05 * shade(un.id, u, w);
                                lin(k, k * 1.05, k * 1.04)
                            }
                            b'j' | b's' => sinew(un.id, u, w),
                            b'b' => bone(un.id, u, w),
                            b'L' => light(lamp),
                            _ => {
                                // The cone turns: its bands run along it.
                                let band = (c as i32 + row as i32 + turn).rem_euclid(3) == 0;
                                if un.state == 1 && band {
                                    lin(0.82, 0.8, 0.74)
                                } else {
                                    lin(0.45, 0.45, 0.47)
                                }
                            }
                        };
                        pen.block(u as f32, w as f32, k);
                    }
                }
                // Tentacles back from its flanks, gripping the shaft walls.
                if un.state == 1 {
                    for (j, side) in [(0, 1i32), (1, -1), (2, 1), (3, -1)] {
                        let u0 = -30 + j * 7;
                        let reach = 2 + ((tt * 0.8 + j as f32 * 1.7).sin() * 1.5).round() as i32;
                        pen.line((u0, side * 6), (u0 - 3, side * (7 + reach)), |k, u, w| if k % 3 == 2 { bone(un.id, u, w) } else { sinew(un.id, u, w) });
                    }
                }
            }
            Unit::HeatSink => {
                let mut pen = Pen { pos: &mut *pos, col: &mut *col, p, half, angle: up_angle };
                let working = un.state == 1;
                pen.rows(&HEATSINK, -7, -5, |ch, u, w| {
                    Some(match ch {
                        b'r' => {
                            // Fins glow with the heat coming up.
                            let k = if working { 0.35 + 0.25 * (tt * 2.0 + u as f32).sin().max(0.0) } else { 0.2 };
                            lin(k * 1.4, k * 0.7, k * 0.4)
                        }
                        b'A' => armour(un.id, u, w),
                        b'D' => lin(0.12, 0.13, 0.125),
                        b'z' => light(lamp * 0.8),
                        b'L' => light(lamp),
                        _ => sinew(un.id, u, w),
                    })
                });
                // Steam off the fins while it works.
                if working {
                    for k in 0..6 {
                        let ph = (tt * 0.7 + k as f32 * 0.37).fract();
                        let u = -6.0 + (k * 3 % 13) as f32 + (ph * 6.0 + k as f32).sin() * 1.5;
                        let w = 6.0 + ph * 14.0;
                        let s = 0.75 - 0.3 * ph;
                        pen.block(u.round(), w.round(), lina(s, s, s, 0.55 * (1.0 - ph)));
                    }
                }
                // Cooling tentacles down into the pool, toward where they last
                // froze it; red-hot at the tips.
                if un.state != 0 {
                    let tip = local(un.mark);
                    let depth = (un.a as f32).max(2.0);
                    for (j, u0) in [-5i32, -2, 2, 5].into_iter().enumerate() {
                        let sway = ((tt * 0.6 + j as f32 * 1.3).sin() * 2.0).round() as i32;
                        let (tu, tw) = if working && j == 1 { (tip.0.round() as i32, tip.1.round() as i32) } else { (u0 + u0.signum() * 2 + sway, -6 - depth.round() as i32 + (j as i32 % 2) * 2) };
                        let n = ((tu - u0).abs() + (tw + 6).abs()).max(1) as f32;
                        pen.line((u0, -6), (tu, tw), |k, u, w| {
                            let f = k as f32 / n;
                            if working && f > 0.7 {
                                let h = (f - 0.7) / 0.3;
                                lin(0.3 + 0.7 * h, 0.12 + 0.3 * h, 0.05)
                            } else if k % 4 == 3 {
                                bone(un.id, u, w)
                            } else {
                                sinew(un.id, u, w)
                            }
                        });
                    }
                }
            }
            Unit::Forester | Unit::Abductor => {
                let mut pen = Pen { pos: &mut *pos, col: &mut *col, p, half, angle: up_angle };
                draw_tripod(&mut pen, un, x, y, now, real_t, lc, &local, solid);
            }
            _ => {}
        }
    }
}

// ---- The tripods (forester and abductor), ported one for one from the
// swarm mockup (mockups/swarm-foundry.html, sceneTripod). Its frame: x along
// (facing +x), y up, the body's base at `by` over the ground; here x is
// mirrored by `face` and the body rides 50 m over the tallest plant (the
// simulation's lift). Colours are the mockup's, in sRGB 0-255. ----

type Rgb = [f32; 3];

fn mhash(x: i32, y: i32, s: i32) -> f32 {
    // The mockup's hash.
    let mut h = (x.wrapping_mul(374761393)).wrapping_add(y.wrapping_mul(668265263)).wrapping_add(s.wrapping_mul(-2048144777));
    h = (h ^ ((h as u32) >> 13) as i32).wrapping_mul(1274126177);
    ((h ^ ((h as u32) >> 16) as i32) as u32) as f32 / 4294967296.0
}

const SEED: i32 = 1001;
/// The crab: its claws' shoulders either side (m from the middle), and its
/// eye stalks either side of the middle.
const CRAB_SHOULDER: f32 = 18.0;
const CRAB_STALKS: [f32; 2] = [-3.0, 3.0];
/// Seconds a tripod takes to turn about.
const TURN_S: f32 = 0.5;

fn m_armour(x: i32, y: i32) -> Rgb {
    let k = (0.27 + 0.07 * mhash(x, y, SEED)) * 255.0;
    [k, k * 1.06, k * 1.04]
}
fn m_light(x: i32, y: i32) -> Rgb {
    let k = (0.40 + 0.06 * mhash(x, y, SEED)) * 255.0;
    [k, k * 1.04, k * 1.03]
}
fn m_strut(x: i32, y: i32) -> Rgb {
    let k = (0.34 + 0.05 * mhash(x, y, SEED)) * 255.0;
    [k, k * 1.05, k * 1.05]
}
fn m_darkg(x: i32, y: i32) -> Rgb {
    let k = (0.10 + 0.04 * mhash(x, y, SEED)) * 255.0;
    [k, k * 1.05, k * 1.03]
}
fn m_sinew(x: i32, y: i32) -> Rgb {
    let k = (0.11 + 0.05 * mhash(x, y, SEED)) * 255.0;
    [k * 0.9, k * 0.97, k * 1.14]
}
fn m_bone(x: i32, y: i32) -> Rgb {
    let k = (0.37 + 0.07 * mhash(x, y, SEED)) * 255.0;
    [k * 0.95, k, k * 1.05]
}
fn mx(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}
fn sc(a: Rgb, k: f32) -> Rgb {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn rgb(c: Rgb) -> [f32; 4] {
    lin(c[0] / 255.0, c[1] / 255.0, c[2] / 255.0)
}
const DARK: Rgb = [31.0, 33.0, 32.0];
const WHITE: Rgb = [226.0, 224.0, 214.0];
const WOOD: Rgb = [176.0, 136.0, 90.0];
const MEAT: Rgb = [150.0, 40.0, 48.0];

/// The forester's shell, row widths bottom up.
const SHELL: [i32; 9] = [13, 16, 17, 17, 17, 16, 14, 11, 7];
fn in_shell(x: i32, y: i32) -> bool {
    if !(0..SHELL.len() as i32).contains(&y) {
        return false;
    }
    let w = SHELL[y as usize];
    if x >= 0 {
        x <= w + if y == 4 { 3 } else if y == 3 || y == 5 { 1 } else { 0 } - if y < 2 { 1 } else { 0 }
    } else {
        -x <= w - if y > 5 { 2 } else { 0 } - if y < 2 { 2 } else { 0 }
    }
}
fn shell_top(x: i32) -> i32 {
    (0..SHELL.len() as i32).rev().find(|&y| in_shell(x, y)).unwrap_or(0)
}

/// One arm's (claw's, palp's) state, as the mockup runs them.
#[derive(Clone, Copy)]
struct Arm {
    state: u8, // 0 idle, 1 reach, 2 cut, 3 lift
    t: f32,
    /// Where it is going (world), where it is (frame), where its lift began.
    target: (f64, f64),
    tip: (f32, f32),
    from: (f32, f32),
    load: bool,
}

/// An eye stalk on its spring.
#[derive(Clone, Copy)]
struct Stalk {
    a: f32,
    va: f32,
    mode: u8, // 0 search, 1 focus, 2 glance
    t: f32,
    goal: f32,
    hold: f32,
    ph: f32,
}

/// Everything a tripod's drawing remembers between frames.
struct Tripod {
    arms: [Arm; 2],
    stalks: [Stalk; 2],
    rotor: f32,
    last_cut: f64,
    pending: Option<(f64, f64)>,
    /// How far round it has turned (see `turn` in draw_tripod).
    turn: f32,
    rng: u64,
    last_t: f64,
}

static TRIPODS: std::sync::Mutex<Option<std::collections::HashMap<u32, Tripod>>> = std::sync::Mutex::new(None);

impl Tripod {
    fn rand(&mut self) -> f32 {
        self.rng = self.rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.rng >> 33) as f32) / (1u64 << 31) as f32
    }
}

/// Draw a forester or an abductor, the mockup's way. `x, y` is where it
/// stands on the ground; `local` turns a world point into its frame.
#[allow(clippy::too_many_arguments)]
fn draw_tripod(pen: &mut Pen, un: &UnitView, x: f64, y: f64, now: f64, real_t: f64, lc: [f32; 3], local: &dyn Fn((f64, f64)) -> (f32, f32), solid: &dyn Fn(i64, i64) -> bool) {
    let forester = un.kind == Unit::Forester;
    let t = real_t as f32;
    let lamp: Rgb = [lc[0] * 255.0, lc[1] * 255.0, lc[2] * 255.0];
    let face_ = if forester { 1.0f64 } else if un.a >= 0.0 { -1.0 } else { 1.0 };
    // A point of the mockup's frame (the middle of block bx, by) in the
    // world, for lights and effects.
    let (sa, ca) = (y.atan2(x) - std::f64::consts::FRAC_PI_2).sin_cos();
    let wpt = move |bx: f32, by: f32| -> (f64, f64) {
        let (u, w) = (face_ * (bx as f64 + 0.5), by as f64 + 0.5);
        (x + u * ca - w * sa, y + u * sa + w * ca)
    };
    let emit = |bx: f32, by: f32, r: f64, c: Rgb, i: f32| {
        let (ex, ey) = wpt(bx, by);
        crate::fx::emit(ex, ey, r, c, i);
    };
    // Its own frame's +u is clockwise round the body; facing +1 is
    // counter-clockwise. The crab is the same both ways round.
    let face = if forester { 1.0f32 } else if un.a >= 0.0 { -1.0 } else { 1.0 };
    // How far round it has turned: +1 facing +u, -1 facing -u, between
    // while it turns about (the frame squeezes through edge-on).
    let turn = std::cell::Cell::new(face);
    // A block at mockup frame (x, y): mirrored to face its way.
    let mut blk = |pen: &mut Pen, bx: f32, by: f32, c: Rgb| {
        let u = (turn.get() * (bx + 0.5) - 0.5).round();
        pen.block(u, by.round(), rgb(c));
    };
    // A world point in the mockup's frame (x toward where it faces).
    let frame = |q: (f64, f64)| -> (f32, f32) {
        let (u, w) = local(q);
        (u * face, w)
    };
    // A path of blocks joined edge to edge (drawPath): `colour` by point.
    fn path(pen: &mut Pen, face: f32, pts: &[(f32, f32)], colour: &dyn Fn(usize) -> Rgb) {
        let put = |pen: &mut Pen, px: i32, py: i32, c: Rgb| {
            let u = (face * (px as f32 + 0.5) - 0.5).round();
            pen.block(u, py as f32, rgb(c));
        };
        let (mut px, mut py) = (pts[0].0.round() as i32, pts[0].1.round() as i32);
        put(pen, px, py, colour(0));
        for (i, p) in pts.iter().enumerate().skip(1) {
            let (bx, by) = (p.0.round() as i32, p.1.round() as i32);
            for _ in 0..60 {
                if px == bx && py == by {
                    break;
                }
                if (bx - px).abs() >= (by - py).abs() {
                    px += (bx - px).signum();
                } else {
                    py += (by - py).signum();
                }
                put(pen, px, py, colour(i));
            }
        }
    }

    // ---- Dropped bag on the ground, and the couriers that come for it ----
    if un.state == 9 {
        // Seconds until the couriers lift it (negative: lifted that long ago).
        let left = (un.t0 - now) as f32;
        let base = if forester { 16.0 } else { 10.0 };
        // Down at 40 m/s from 150 m up, latched for 0.6 s, then up, faster
        // and faster, carrying it.
        let (rise, courier) = if left > 0.0 {
            (0.0, if left < 150.0 / 40.0 { Some(6.0 + left * 40.0) } else { None })
        } else {
            let tt = -left;
            if tt < 0.6 {
                (0.0, Some(6.0))
            } else {
                let u = tt - 0.6;
                (10.0 * u + 9.0 * u * u, Some(6.0))
            }
        };
        let top = base + rise;
        draw_store(pen, &mut blk, forester, 0.0, top, 1.0, t, 0.0);
        if let Some(h) = courier {
            let items = if forester { 16.0 } else { 14.0 };
            let n = 2 + (items / 7.0f32).ceil() as i32;
            for k in 0..n {
                let o = (k as f32 - (n - 1) as f32 / 2.0) * 4.0;
                let (cx, cy) = (o.round(), (top + h).round());
                blk(pen, cx - 1.0, cy, m_light(1, 1));
                blk(pen, cx, cy, m_light(2, 1));
                let pulse = 0.62 + 0.38 * (t * 1.6 + cx * 0.3).sin();
                blk(pen, cx, cy - 1.0, mx([20.0, 30.0, 24.0], lamp, pulse));
                emit(cx, cy - 1.0, 4.0, lamp, 0.4);
                // Latched on: a line of light down to the bag.
                if left <= 0.0 {
                    let mut q = cy - 2.0;
                    while q > top {
                        if (q as i32) % 2 == 0 {
                            blk(pen, cx, q, mx([20.0, 30.0, 24.0], lamp, 0.5));
                        }
                        q -= 1.0;
                    }
                }
            }
        }
        return;
    }
    // ---- Falling, folded into its package ----
    if un.state == 0 {
        draw_package(pen, &mut blk, forester, 0.0, 0.0, t, lamp);
        return;
    }

    // Its legs, by the procedural rig (rig.rs), on the ground as drawn.
    let r0 = (x * x + y * y).sqrt().max(1.0);
    let ground = |s: f64| -> Option<f32> {
        // Down the radius at `s` m round the body: the top of the first
        // solid cell, as height over where it stands.
        let a = s / r0;
        let (ux, uy) = (a.cos(), a.sin());
        let mut w = 40.0;
        while w > -90.0 {
            let rr = r0 + w;
            if solid((ux * rr).floor() as i64, (uy * rr).floor() as i64) {
                return Some(w as f32 + 0.5);
            }
            w -= 1.0;
        }
        None
    };
    let spec = rig_of(un, face);
    let pose = crate::rig::step(un.id as u64 | ((un.kind.index() as u64) << 40), &spec, x, y, real_t, un.state == 4, &ground);
    let by = pose.body;
    let legs = pose.legs;
    let lean_u = pose.lean;
    let landed = pose.landed;
    // (The spider's frame faces its way.)
    let lean = lean_u * face;
    // A foot coming down under load: dust.
    for (i, &l) in landed.iter().enumerate() {
        if l {
            let foot = *legs[i].last().unwrap();
            let (fx, fy) = wpt(foot.0 * face, foot.1 + 0.5);
            for _ in 0..3 {
                crate::fx::smoke(fx, fy, [110.0, 100.0, 88.0]);
            }
        }
    }
    let mut all = TRIPODS.lock().unwrap();
    let map = all.get_or_insert_with(Default::default);
    if map.len() > 64 {
        map.retain(|_, s| real_t - s.last_t < 30.0);
    }
    let idle_arm = Arm { state: 0, t: 1.0, target: (0.0, 0.0), tip: (0.0, 0.0), from: (0.0, 0.0), load: false };
    let st = map.entry(un.id).or_insert_with(|| Tripod {
        arms: [idle_arm, idle_arm],
        stalks: [Stalk { a: -0.3, va: 0.0, mode: 0, t: 0.0, goal: 0.0, hold: 0.5, ph: 0.0 }, Stalk { a: 0.3, va: 0.0, mode: 0, t: 0.0, goal: 0.0, hold: 0.5, ph: 2.3 }],
        rotor: 0.0,
        last_cut: un.cut_at,
        pending: None,
        turn: face,
        rng: un.id as u64 * 0x9E37 + 1,
        last_t: real_t,
    });
    let dt = (real_t - st.last_t).clamp(0.0, 0.05) as f32;
    st.last_t = real_t;
    // Turning about takes half a second.
    st.turn = if face > st.turn { (st.turn + dt / TURN_S * 2.0).min(face) } else { (st.turn - dt / TURN_S * 2.0).max(face) };
    turn.set(st.turn);
    let b = lean;
    // The store hangs here (the mockup's store()).
    // (The crab's bag hangs under the middle of it.)
    let store = if forester { (b, by - 2.0) } else { (b - 9.0, by + 2.0) };

    // ---- The arms: each cut the simulation reports, a claw (palp) reaches,
    // works and lifts it into the store ----
    // Each section the simulation's claws shear: a claw goes for it.
    if un.cut_at > st.last_cut {
        st.pending = Some(un.claw);
        st.last_cut = un.cut_at;
    }
    let (reach_s, cut_s, lift_s) = if forester { (0.5, 0.5, 0.8) } else { (0.6, 0.0, 0.9) };
    for k in 0..2 {
        let mut a = st.arms[k];
        a.t += dt;
        let side = if k == 0 { 1.0f32 } else { -1.0 };
        let sh = if forester { (b + side * CRAB_SHOULDER, by + 3.0) } else { (b + 12.0 - k as f32 * 3.0, by + 1.0) };
        match a.state {
            0 => {
                if a.t > 0.6 {
                    // The crab's claw on the side the plant is.
                    let mine = st.pending.is_some_and(|m| !forester || (frame(m).0 - b) * side >= 0.0);
                    if mine {
                        a.target = st.pending.take().unwrap();
                        a.state = 1;
                        a.t = 0.0;
                    }
                }
            }
            1 => {
                let g = frame(a.target);
                let goal = (g.0, g.1 - 1.0);
                let e = ease(a.t / reach_s);
                a.tip = (sh.0 + (goal.0 - sh.0) * e, sh.1 + (goal.1 - sh.1) * e);
                if a.t >= reach_s {
                    a.state = if forester { 2 } else { 3 };
                    a.t = 0.0;
                    a.from = a.tip;
                    a.load = !forester;
                }
            }
            2 => {
                if mhash((real_t * 30.0) as i32, k as i32, un.id as i32) < 0.5 {
                    let (sx, sy) = wpt(a.tip.0, a.tip.1);
                    crate::fx::spark(sx, sy, 1, if mhash(k as i32, (real_t * 7.0) as i32, 3) < 0.5 { WOOD } else { [140.0, 190.0, 90.0] }, 7.0);
                }
                if a.t > cut_s {
                    a.load = true;
                    a.state = 3;
                    a.t = 0.0;
                    a.from = a.tip;
                }
            }
            _ => {
                let to = if forester { (store.0, store.1 + 4.0) } else { store };
                let e = ease(a.t / lift_s);
                a.tip = (a.from.0 + (to.0 - a.from.0) * e, a.from.1 + (to.1 - a.from.1) * e);
                if a.t >= lift_s {
                    a.load = false;
                    a.state = 0;
                    a.t = 0.0;
                }
            }
        }
        st.arms[k] = a;
    }
    // The laser's sweep: from where it began toward where it ends, over 0.8 s
    // (game time), as the mockup sweeps it.
    let since = (now - un.fired_at) as f32;
    let firing = un.c == 2.0 && (0.0..0.9).contains(&since);
    let sweep = (since / 0.8).clamp(0.0, 1.0) as f64;
    let laser_world = (un.sweep_from.0 + (un.mark.0 - un.sweep_from.0) * sweep, un.sweep_from.1 + (un.mark.1 - un.sweep_from.1) * sweep);
    let laser_mark = frame(laser_world);

    // ---- The eye stalks (forester): searching, darting, focusing ----
    if forester {
        let busy = st.arms.iter().find(|a| a.state == 1 || a.state == 2).map(|a| a.tip);
        let poi = if firing { Some(laser_mark) } else { busy };
        for i in 0..2 {
            let ex = [CRAB_STALKS[0], CRAB_STALKS[1]][i];
            let base = (b + ex, by + 9.0);
            let lim = |a: f32| a.clamp(-0.95, 0.95);
            let mut e = st.stalks[i];
            e.t += dt;
            let r1 = st.rand();
            let r2 = st.rand();
            match e.mode {
                0 => {
                    if e.t > e.hold {
                        e.goal = lim((r1 * 2.0 - 1.0) * 0.9);
                        e.hold = 0.5 + r2 * 1.6;
                        e.t = 0.0;
                    }
                    if poi.is_some() && st.rand() < dt * 2.0 {
                        e.mode = 1;
                        e.t = 0.0;
                    } else if st.rand() < dt * 0.08 {
                        e.mode = 2;
                        e.t = 0.0;
                        e.goal = if i == 1 { -0.9 } else { 0.9 };
                    }
                }
                1 => {
                    if let Some(p) = poi {
                        e.goal = lim((p.0 - base.0).atan2(p.1 - base.1) * 0.6);
                    }
                    if poi.is_none() && e.t > 0.5 {
                        e.mode = 0;
                        e.t = 0.0;
                        e.hold = 0.3;
                    }
                }
                _ => {
                    if e.t > 0.7 {
                        e.mode = 0;
                        e.t = 0.0;
                        e.hold = 0.4;
                    }
                }
            }
            e.va += ((e.goal - e.a) * 160.0 - e.va * 24.0) * dt;
            e.a = lim(e.a + e.va * dt);
            st.stalks[i] = e;
        }
    }

    if forester {
        draw_crab(pen, un, st, by, &legs, b, t, real_t, lamp, firing, laser_mark, laser_world, &wpt);
        return;
    }

    // ---- Drawing, in the mockup's order ----
    // Legs: each a chain of bones from the rig, drawn as a curve through
    // its joints (each bone bowed up a little), ringed in bone.
    for (i, joints) in legs.iter().enumerate() {
        // Into the mockup's frame (x toward where it faces).
        let pts: Vec<(f32, f32)> = joints.iter().map(|p| (p.0 * face, p.1)).collect();
        let col = |k: usize| if k % 3 == 0 { sc(m_bone(k as i32, i as i32), 1.1) } else { mx(m_light(k as i32, i as i32), m_sinew(k as i32, i as i32), 0.35) };
        let mut curve: Vec<(f32, f32)> = Vec::new();
        for w in pts.windows(2) {
            let (p0, p1) = (w[0], w[1]);
            let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
            let l = (dx * dx + dy * dy).sqrt().max(1e-3);
            // Bowed away from the ground: the side of the bone facing up.
            let (nx, ny) = (-dy / l, dx / l);
            let (nx, ny) = if ny < 0.0 { (-nx, -ny) } else { (nx, ny) };
            let mid = ((p0.0 + p1.0) / 2.0 + nx * l * 0.12, (p0.1 + p1.1) / 2.0 + ny * l * 0.12);
            let seg = bez(p0, mid, p1, (l as usize).max(4));
            if curve.is_empty() {
                curve.extend(seg);
            } else {
                curve.extend(seg.into_iter().skip(1));
            }
        }
        path(pen, turn.get(), &curve, &col);
        for (k, j) in pts.iter().enumerate().skip(1).take(pts.len().saturating_sub(2)) {
            blk(pen, j.0.round(), j.1.round(), sc(m_bone(4 + k as i32, i as i32), 1.2));
        }
        let f = *pts.last().unwrap();
        blk(pen, f.0.round() - 1.0, f.1.round(), m_bone(1, i as i32));
        blk(pen, f.0.round() + 1.0, f.1.round(), m_bone(2, i as i32));
    }
    // Palps or claws.
    for k in 0..2 {
        let a = st.arms[k];
        let kf = k as f32;
        let sh = if forester { (b + 15.0 - kf * 4.0, by + 2.0) } else { (b + 12.0 - kf * 3.0, by + 1.0) };
        let tip = if a.state == 0 { (sh.0 + 4.0 + (t * 1.3 + kf).sin() * 1.5, sh.1 - 5.0 - kf * 2.0) } else { a.tip };
        let (dx, dy) = (tip.0 - sh.0, tip.1 - sh.1);
        if !forester {
            // A whip: thick at the root, thin to the tip, lagging behind its
            // own motion so it lashes out and curls back; idle, it hangs and
            // twitches.
            let len = (dx * dx + dy * dy).sqrt().max(1.0);
            let (nx, ny) = (-dy / len, dx / len);
            let lash = match a.state {
                1 => (a.t / 0.6).min(1.0) * std::f32::consts::PI,
                3 => (a.t / 0.9).min(1.0) * std::f32::consts::PI,
                _ => 0.0,
            };
            let lash = match a.state {
                1 => lash.sin() * 7.0,
                3 => -4.0 * lash.sin(),
                _ => 3.0 * (t * 1.7 + kf * 2.0).sin(),
            };
            let pts: Vec<(f32, f32)> = (0..=30)
                .map(|q| {
                    let u = q as f32 / 30.0;
                    let w = (u * std::f32::consts::PI).sin() * lash + (t * 6.0 + u * 9.0 + kf).sin() * 0.6 * u;
                    (sh.0 + dx * u + nx * w, sh.1 + dy * u + ny * w)
                })
                .collect();
            let col = |q: usize| if q % 4 == 0 { sc(m_bone(q as i32, k as i32), 0.9) } else { mx(m_sinew(q as i32, k as i32), [70.0, 40.0, 46.0], 0.35) };
            path(pen, turn.get(), &pts, &col);
            let thick: Vec<(f32, f32)> = pts.iter().take(10).map(|p| (p.0 + 1.0, p.1)).collect();
            path(pen, turn.get(), &thick, &col);
            let (tx, ty) = (tip.0.round(), tip.1.round());
            let shut = a.state == 3;
            blk(pen, tx, ty, sc(m_bone(3, k as i32), 1.2));
            if a.state == 1 && a.t > 0.5 {
                emit(tx, ty, 3.0, lamp, 0.4);
            }
            blk(pen, tx + if shut { 0.0 } else { 1.0 }, ty - 1.0, WHITE);
            blk(pen, tx - 1.0, ty + if shut { 0.0 } else { 1.0 }, WHITE);
            if a.load {
                blk(pen, tx, ty - 1.0, MEAT);
                blk(pen, tx - 1.0, ty - 1.0, MEAT);
                blk(pen, tx, ty - 2.0, MEAT);
            }
            continue;
        }
        for (j, (u0, u1, w)) in [(0.0f32, 0.45f32, 2u8), (0.4, 0.78, 2), (0.74, 1.0, 1)].into_iter().enumerate() {
            let p0 = (sh.0 + dx * u0, sh.1 + dy * u0);
            let p1 = (sh.0 + dx * u1, sh.1 + dy * u1);
            let c = match j {
                0 => sc(m_armour(j as i32, 0), 1.2),
                1 => sc(m_armour(j as i32, 1), 1.1),
                _ => m_light(j as i32, 2),
            };
            path(pen, turn.get(), &[p0, p1], &|_| c);
            if w > 1 {
                path(pen, turn.get(), &[(p0.0 + 1.0, p0.1), (p1.0 + 1.0, p1.1)], &|_| c);
            }
            blk(pen, p0.0.round(), p0.1.round(), DARK);
        }
        let (tx, ty) = (tip.0.round(), tip.1.round());
        // A crab's claw: a fixed jaw with a saw edge and a moving one. Held
        // open while it reaches, closed at rest; one clean shear per cut.
        let o = match a.state {
            2 => {
                if a.t < 0.15 {
                    2.0
                } else if a.t < 0.3 {
                    0.0
                } else {
                    1.0
                }
            }
            1 => 2.0,
            _ => 0.0,
        };
        for q in -1..=3 {
            blk(pen, tx + q as f32, ty + 1.0, sc(m_armour(q, 0), 1.15));
            if q % 2 != 0 {
                blk(pen, tx + q as f32, ty, WHITE);
            }
        }
        for q in 0..=3 {
            blk(pen, tx + q as f32, ty - 1.0 - o, m_light(q, 1));
        }
        if a.load {
            blk(pen, tx, ty - 1.0, WOOD);
            blk(pen, tx - 1.0, ty - 1.0, WOOD);
            blk(pen, tx, ty - 2.0, WOOD);
        }
    }
    let (ox, oy) = (b.round(), by.round());
    let fill = un.b.clamp(0.0, 1.0);
    if !forester {
        // The spider: the abdomen (its store) behind, a narrow waist, and the
        // head in front: armoured, ridged, a cluster of eyes, two fangs.
        for dx in -1..=1 {
            blk(pen, ox + 1.0 + dx as f32, oy + 4.0, m_sinew(dx, 4));
        }
        draw_store(pen, &mut blk, false, ox - 9.0, oy + 10.0, fill, t, 0.0);
        emit(ox - 9.0, oy + 10.0 - (6.0 + fill * 2.0), 6.0, [160.0, 60.0, 60.0], 0.12);
        for yy in 0..8 {
            for xx in 3..=15 {
                let q = ((xx as f32 - 9.0) / 6.5).powi(2) + ((yy as f32 - 3.5) / 4.2).powi(2);
                if q > 1.0 {
                    continue;
                }
                let c = if q > 0.72 {
                    m_strut(xx, yy)
                } else if yy == 5 && xx % 2 != 0 {
                    m_bone(xx, yy)
                } else if yy == 2 {
                    mx([20.0, 30.0, 24.0], lamp, 0.5 + 0.5 * (t * 1.5 - xx as f32 * 0.4).sin())
                } else {
                    sc(m_armour(xx, yy), 1.08)
                };
                blk(pen, ox + xx as f32, oy + yy as f32, c);
            }
        }
        // Its eyes: a slow, uneasy glow while it roams; flaring and pulsing
        // fast when it has something in its sights.
        let hunting = firing || st.arms.iter().any(|a| a.state == 1);
        let eyes = [(13, 5), (14, 4), (12, 6), (14, 6), (13, 7), (12, 4)];
        let lead = ((t * if hunting { 12.0 } else { 1.5 }).floor() as usize) % eyes.len();
        for (i, &(ex, ey)) in eyes.iter().enumerate() {
            let p = if hunting { 0.55 + 0.45 * (t * 14.0 + i as f32 * 1.3).sin() } else { 0.35 + 0.25 * (t * 1.2 + i as f32 * 0.9).sin() };
            let c = if i == lead { mx(lamp, [255.0; 3], if hunting { 0.5 } else { 0.2 }) } else { mx([14.0, 20.0, 18.0], lamp, p) };
            blk(pen, ox + ex as f32, oy + ey as f32, c);
        }
        emit(ox + 13.0, oy + 5.0, if hunting { 9.0 } else { 5.0 }, lamp, if hunting { 1.1 + 0.4 * (t * 14.0).sin() } else { 0.35 });
        let open = if (t * 2.0).sin() > 0.5 { 1.0 } else { 0.0 };
        for sd in 0..2 {
            for q in 0..4 {
                let dxo = if q == 3 { (if sd == 1 { -1.0 } else { 1.0 }) * open } else { 0.0 };
                blk(pen, ox + 13.0 + sd as f32 * 2.0 + dxo, oy - q as f32, if q == 3 { WHITE } else { m_bone(q, sd) });
            }
        }
    } else {
        // The crab: a wide, flat shell, plated, a rotor of blades in its
        // intake, eyes on stalks; the clippings bag slung behind.
        for q in -3..=3 {
            blk(pen, ox - 17.0 + q as f32, oy + 3.0, m_bone(q, 0));
        }
        let swing = ((t * 1.3).sin() * 1.5 - lean * 2.0).round();
        draw_store(pen, &mut blk, true, ox - 17.0, oy + 2.0, fill, t, swing);
        draw_shell(pen, &mut blk, ox + 2.0, oy, t, true, lamp);
        let feeding = st.arms.iter().any(|a| a.state == 3);
        for q in 0..4 {
            let bob = if feeding && (t * 8.0 + q as f32).sin() > 0.0 { 1.0 } else { 0.0 };
            blk(pen, ox + 16.0 + q as f32, oy - 1.0 - (q % 2) as f32 - bob, m_bone(q, 3));
        }
        if feeding {
            st.rotor += 0.25;
        }
        for yy in 0..3 {
            let q = ((st.rotor + yy as f32 * 2.0).floor() as i32).rem_euclid(3);
            blk(pen, ox + 20.0, oy + 1.0 + yy as f32, if q == 0 { WHITE } else { DARK });
        }
        // Eye stalks: flexible, segmented, each swinging on its own, its tip
        // an eye in the swarm's light colour, lit toward where it looks.
        for (i, e) in st.stalks.iter().enumerate() {
            let ex = [9i32, 12][i];
            let base = (ox + 2.0 + ex as f32, oy + shell_top(ex) as f32 + 1.0);
            let dir = (e.a.sin(), e.a.cos());
            let tip = (base.0 + dir.0 * 7.0, base.1 + dir.1 * 7.0);
            let ctrl = (base.0 + dir.0 * 1.5, base.1 + 4.0);
            path(pen, turn.get(), &bez(base, ctrl, tip, 10), &|q| if q % 3 == 2 { m_bone(q as i32, ex) } else { m_strut(q as i32, ex) });
            let (tx, ty) = (tip.0.round(), tip.1.round());
            let (lx, ly) = ((tip.0 + dir.0).round(), (tip.1 + dir.1).round());
            let focus = e.mode == 1;
            let p = if focus { 0.8 + 0.2 * (t * 9.0).sin() } else { 0.35 + 0.3 * (t * 1.4 + e.ph).sin() };
            blk(pen, tx, ty, mx([14.0, 20.0, 18.0], lamp, p * 0.7));
            blk(pen, lx, ly, if focus { mx(lamp, [255.0; 3], 0.45) } else { mx([14.0, 20.0, 18.0], lamp, p) });
            emit(lx, ly, if focus { 6.0 } else { 3.0 }, lamp, if focus { 0.9 } else { 0.35 });
        }
    }
    // The laser.
    if firing {
        let em = if forester { (ox + 16.0, oy) } else { (ox + 11.0, oy) };
        let flick = mhash((t * 30.0) as i32, un.id as i32, 7) < 0.6;
        let c = if forester { if flick { [255.0, 220.0, 150.0] } else { [255.0, 150.0, 60.0] } } else if flick { [255.0, 120.0, 100.0] } else { [255.0, 230.0, 220.0] };
        let (x0, y0, x1, y1) = (em.0.floor() as i32, em.1.floor() as i32, laser_mark.0.floor() as i32, laser_mark.1.floor() as i32);
        // blockLine (Bresenham).
        let (mut ax, mut ay) = (x0, y0);
        let (ddx, ddy) = ((x1 - ax).abs(), -(y1 - ay).abs());
        let (sx, sy) = (if ax < x1 { 1 } else { -1 }, if ay < y1 { 1 } else { -1 });
        let mut err = ddx + ddy;
        for _ in 0..400 {
            blk(pen, ax as f32, ay as f32, c);
            if ax == x1 && ay == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= ddy {
                err += ddy;
                ax += sx;
            }
            if e2 <= ddx {
                err += ddx;
                ay += sy;
            }
        }
        blk(pen, laser_mark.0.round(), laser_mark.1.round(), [255.0, 250.0, 240.0]);
        crate::fx::emit(laser_world.0, laser_world.1, 10.0, if forester { [255.0, 160.0, 60.0] } else { [255.0, 80.0, 60.0] }, 1.5);
        if forester && mhash((real_t * 20.0) as i32, un.id as i32, 5) < 0.3 {
            crate::fx::smoke(laser_world.0, laser_world.1, [70.0, 66.0, 62.0]);
        }
    }
}

/// The abdomen (abductor) or the clippings bag (forester): drawStore.
#[allow(clippy::too_many_arguments)]
fn draw_store(pen: &mut Pen, blk: &mut dyn FnMut(&mut Pen, f32, f32, Rgb), forester: bool, x0: f32, top: f32, fill: f32, t: f32, swing: f32) {
    let items = (fill * if forester { 16.0 } else { 14.0 }).round() as i32;
    if !forester {
        // A leathery sac in bone hoops: dark, faintly see-through where it is
        // stretched, a slow swell and ease as if breathing.
        let rx = 7.0 + fill * 3.0 + 0.3 * (t * 1.1).sin();
        let ry = 6.0 + fill * 2.0;
        let (cx, cy) = (rx.ceil() as i32, ry.ceil() as i32);
        for y in -cy..=cy {
            for x in -cx..=cx {
                let q = (x as f32 / (rx + 0.5)).powi(2) + (y as f32 / (ry + 0.5)).powi(2);
                if q > 1.0 {
                    continue;
                }
                let hoop = x.abs() % 4 == 1 && q < 0.9;
                let rim = q > 0.78;
                let c = if rim {
                    mx(m_armour(x, y), [40.0, 26.0, 28.0], 0.4)
                } else if hoop {
                    sc(m_bone(x, y), 0.75)
                } else {
                    mx([46.0, 32.0, 34.0], [68.0, 44.0, 46.0], mhash(x, y, 8))
                };
                blk(pen, x0 + x as f32, top - ry.round() + y as f32, c);
            }
        }
        for i in 0..items {
            let x = x0 - 5.0 + (i % 4) as f32 * 3.0;
            let y = top - ry.round() - 2.0 + (i / 4) as f32 * 2.0;
            blk(pen, x, y, mx([46.0, 32.0, 34.0], MEAT, 0.35));
        }
    } else {
        let bw = (6.0 + fill * 3.0).round() as i32;
        let bh = (10.0 + fill * 6.0).round() as i32;
        for y in 0..bh {
            let half = (bw as f32 * (0.55 + 0.45 * (((y + 1) as f32 / bh as f32).min(1.0) * std::f32::consts::PI * 0.85).sin())).round() as i32;
            let sx = (swing * y as f32 / bh as f32).round();
            for x in -half..=half {
                let edge = x.abs() == half || y == bh - 1;
                let mesh = (x + y).rem_euclid(3) == 0 || (x - y).rem_euclid(3) == 0;
                let full_here = (y as f32) > bh as f32 * (1.0 - fill) - 1.0;
                let c = if edge {
                    m_strut(x, y)
                } else if mesh {
                    mx([40.0, 46.0, 40.0], m_strut(x, y), 0.4)
                } else if full_here {
                    mx([60.0, 100.0, 44.0], [120.0, 96.0, 60.0], mhash(x, y + items, 5))
                } else {
                    [16.0, 20.0, 18.0]
                };
                blk(pen, x0 + x as f32 + sx, top - y as f32, c);
            }
        }
    }
}

/// The forester's shell: a broad carapace, higher at the front, with a
/// pointed rostrum, spines along its rim, grooves across its plates, the
/// light showing through in the grooves.
fn draw_shell(pen: &mut Pen, blk: &mut dyn FnMut(&mut Pen, f32, f32, Rgb), x0: f32, y0: f32, t: f32, lit: bool, lamp: Rgb) {
    for y in 0..SHELL.len() as i32 {
        for x in -18..=21 {
            if !in_shell(x, y) {
                continue;
            }
            let edge = !in_shell(x - 1, y) || !in_shell(x + 1, y) || !in_shell(x, y - 1) || !in_shell(x, y + 1);
            let groove = y > 0 && y < 7 && (((x - 2).abs() as f32) - (6.0 + ((y - 4) * (y - 4)) as f32 * 0.7)).abs() < 0.6;
            let c = if edge {
                m_strut(x, y)
            } else if groove {
                if lit && (2..=4).contains(&y) {
                    mx([20.0, 30.0, 24.0], lamp, 0.45 + 0.4 * (t * 1.6 + y as f32).sin())
                } else {
                    m_darkg(x, y)
                }
            } else if y == 6 && x.abs() < 5 {
                sc(m_bone(x, y), 0.9)
            } else {
                sc(mx(m_light(x, y), [150.0, 118.0, 96.0], 0.12), 0.9 + 0.08 * (x as f32 * 0.5).sin())
            };
            blk(pen, x0 + x as f32, y0 + y as f32, c);
        }
    }
    let mut x = -15;
    while x <= 15 {
        blk(pen, x0 + x as f32, y0 + shell_top(x) as f32 + 1.0, sc(m_bone(x, 9), 1.15));
        x += 3;
    }
    for x in [19, 20] {
        blk(pen, x0 + x as f32 + 1.0, y0 + 4.0, sc(m_bone(x, 4), 1.2));
    }
}

/// The folded package it falls in: the unit's own shape, packed tight.
fn draw_package(pen: &mut Pen, blk: &mut dyn FnMut(&mut Pen, f32, f32, Rgb), forester: bool, x0: f32, y0: f32, t: f32, lamp: Rgb) {
    if !forester {
        // A curled spider: the abdomen, the head tucked under, the legs
        // folded over its back.
        for y in 0..9 {
            for x in -8..=8 {
                let q = (x as f32 / 8.5).powi(2) + ((y as f32 - 4.0) / 4.8).powi(2);
                if q <= 1.0 {
                    let c = if q > 0.75 {
                        mx(m_armour(x, y), [40.0, 26.0, 28.0], 0.4)
                    } else if x.abs() % 4 == 1 {
                        sc(m_bone(x, y), 0.75)
                    } else {
                        mx([46.0, 32.0, 34.0], [68.0, 44.0, 46.0], mhash(x, y, 8))
                    };
                    blk(pen, x0 - 2.0 + x as f32, y0 + y as f32, c);
                }
            }
        }
        for y in 0..5 {
            for x in 5..=10 {
                if (x as f32 - 7.5).powi(2) / 9.0 + (y as f32 - 2.0).powi(2) / 6.0 <= 1.0 {
                    blk(pen, x0 + x as f32, y0 + y as f32, if y == 4 { m_strut(x, y) } else { sc(m_armour(x, y), 1.1) });
                }
            }
        }
        for (ex, ey) in [(9, 3), (10, 2), (8, 2)] {
            blk(pen, x0 + ex as f32, y0 + ey as f32, mx([14.0, 20.0, 18.0], lamp, 0.25 + 0.15 * (t * 1.5 + ex as f32).sin()));
        }
        for k in 0..10 {
            let (a, r) = (k as f32 * 0.9, 2.6 - k as f32 * 0.2);
            blk(pen, (x0 + 8.0 + a.cos() * r).round(), (y0 - 1.0 + a.sin() * r * 0.7).round(), if k % 3 == 0 { m_bone(k, 7) } else { mx(m_sinew(k, 7), [70.0, 40.0, 46.0], 0.35) });
        }
        for (a, r) in [(0, 9.0f32), (1, 10.0), (2, 11.0)] {
            for k in 0..12 {
                let ang = std::f32::consts::PI * (0.15 + 0.7 * k as f32 / 11.0);
                let x = (x0 - 2.0 + ang.cos() * r * 0.9).round();
                let y = (y0 + 3.0 + ang.sin() * (r - 3.0)).round();
                blk(pen, x, y, if k % 3 == 0 { m_bone(k, a) } else { mx(m_light(k, a), m_sinew(k, a), 0.35) });
            }
        }
    } else {
        // The crab drawn in: its own shell on top, the legs folded flat
        // beneath it, the claws tucked under the rostrum, the eye stalks laid
        // back along the shell, their tips dark.
        for x in -14..=14 {
            blk(pen, x0 + x as f32, y0 + 1.0, if x % 3 == 0 { m_bone(x, 1) } else { mx(m_light(x, 1), m_sinew(x, 1), 0.35) });
            if x % 2 == 0 && x.abs() < 12 {
                blk(pen, x0 + x as f32, y0, mx(m_light(x, 0), m_sinew(x, 0), 0.35));
            }
        }
        for sd in 0..2 {
            for k in 0..6 {
                blk(pen, x0 + 14.0 + k as f32, y0 + 1.0 + sd as f32, if k > 3 { WHITE } else { sc(m_armour(k, sd), 1.15) });
            }
        }
        draw_shell(pen, blk, x0 - 2.0, y0 + 2.0, t, false, lamp);
        for ex in [9, 12] {
            for k in 0..6 {
                blk(pen, x0 - 2.0 + (ex - k) as f32, y0 + 2.0 + shell_top(ex) as f32 + 1.0, if k == 0 { [30.0, 44.0, 40.0] } else { m_strut(k, ex) });
            }
        }
    }
}


/// The forester: a crab that walks sideways, the same both ways round, in
/// the swarm mockup's colours and parts. A wide, flat, plated shell, the
/// light showing in its grooves; four legs, two a side; a big saw-edged
/// claw each side, each taking what grows on its side; the woven clippings
/// bag slung under the middle; two eye stalks in the middle of the shell,
/// whose eyes fire the laser; a rotor of blades in the intake under it.
#[allow(clippy::too_many_arguments)]
fn draw_crab(pen: &mut Pen, un: &UnitView, st: &mut Tripod, by: f32, legs: &[Vec<(f32, f32)>], b: f32, t: f32, real_t: f64, lamp: Rgb, firing: bool, laser_mark: (f32, f32), laser_world: (f64, f64), wpt: &dyn Fn(f32, f32) -> (f64, f64)) {
    let blk = |pen: &mut Pen, u: f32, w: f32, c: Rgb| pen.block(u.round(), w.round(), rgb(c));
    fn path(pen: &mut Pen, pts: &[(f32, f32)], colour: &dyn Fn(usize) -> Rgb) {
        let (mut px, mut py) = (pts[0].0.round() as i32, pts[0].1.round() as i32);
        pen.block(px as f32, py as f32, rgb(colour(0)));
        for (i, p) in pts.iter().enumerate().skip(1) {
            let (bx, by) = (p.0.round() as i32, p.1.round() as i32);
            for _ in 0..80 {
                if px == bx && py == by {
                    break;
                }
                if (bx - px).abs() >= (by - py).abs() {
                    px += (bx - px).signum();
                } else {
                    py += (by - py).signum();
                }
                pen.block(px as f32, py as f32, rgb(colour(i)));
            }
        }
    }
    let (ox, oy) = (b.round(), by.round());
    let fill = un.b.clamp(0.0, 1.0);
    // Legs from the rig: thigh and shin, two blocks thick, bone at every
    // third block, a bone knuckle at the knee, a two-toed foot.
    for (i, joints) in legs.iter().enumerate() {
        let col = |k: usize| if k % 3 == 0 { sc(m_bone(k as i32, i as i32), 1.1) } else { mx(m_light(k as i32, i as i32), m_sinew(k as i32, i as i32), 0.35) };
        let out = if joints.last().unwrap().0 >= joints[0].0 { 1.0 } else { -1.0 };
        for w in joints.windows(2) {
            let n = ((w[1].0 - w[0].0).abs() + (w[1].1 - w[0].1).abs()) as usize;
            let seg: Vec<(f32, f32)> = (0..=n.max(1)).map(|q| {
                let t = q as f32 / n.max(1) as f32;
                (w[0].0 + (w[1].0 - w[0].0) * t, w[0].1 + (w[1].1 - w[0].1) * t)
            }).collect();
            path(pen, &seg, &col);
            let thick: Vec<(f32, f32)> = seg.iter().map(|p| (p.0 + out, p.1)).collect();
            path(pen, &thick, &col);
        }
        for j in joints.iter().skip(1).take(joints.len().saturating_sub(2)) {
            blk(pen, j.0, j.1, sc(m_bone(4, i as i32), 1.2));
        }
        let f = *joints.last().unwrap();
        blk(pen, f.0.round() - 1.0, f.1.round(), m_bone(1, i as i32));
        blk(pen, f.0.round() + 1.0, f.1.round(), m_bone(2, i as i32));
    }
    // The bag, slung under the middle on a bone strap, swaying.
    for q in -4..=4 {
        blk(pen, ox + q as f32, oy + 1.0, m_bone(q, 0));
    }
    let swing = ((t * 1.3).sin() * 1.5).round();
    {
        let mut put = |pen: &mut Pen, bx: f32, bw: f32, c: Rgb| pen.block(bx.round(), bw.round(), rgb(c));
        draw_store(pen, &mut put, true, ox, oy, fill, t, swing);
    }
    // The shell: wide and flat, the same both ways, plated; grooves across
    // it lit from within; spines along the rim.
    let in_shell = |x: i32, y: i32| (0..SHELL.len() as i32).contains(&y) && x.abs() <= SHELL[y as usize] - if y > 5 { 2 } else { 0 } - if y < 2 { 2 } else { 0 };
    for y in 0..SHELL.len() as i32 {
        for x in -18..=18 {
            if !in_shell(x, y) {
                continue;
            }
            let edge = !in_shell(x - 1, y) || !in_shell(x + 1, y) || !in_shell(x, y - 1) || !in_shell(x, y + 1);
            let groove = y > 0 && y < 7 && ((x.abs() as f32) - (6.0 + ((y - 4) * (y - 4)) as f32 * 0.7)).abs() < 0.6;
            let c = if edge {
                m_strut(x, y)
            } else if groove {
                if (2..=4).contains(&y) { mx([20.0, 30.0, 24.0], lamp, 0.45 + 0.4 * (t * 1.6 + y as f32).sin()) } else { m_darkg(x, y) }
            } else if y == 6 && x.abs() < 5 {
                sc(m_bone(x, y), 0.9)
            } else {
                sc(mx(m_light(x, y), [150.0, 118.0, 96.0], 0.12), 0.9 + 0.08 * (x as f32 * 0.5).sin())
            };
            blk(pen, ox + x as f32, oy + y as f32, c);
        }
    }
    let top = |x: i32| (0..SHELL.len() as i32).rev().find(|&y| in_shell(x, y)).unwrap_or(0);
    let mut x = -15;
    while x <= 15 {
        blk(pen, ox + x as f32, oy + top(x) as f32 + 1.0, sc(m_bone(x, 9), 1.15));
        x += 3;
    }
    // The intake under the middle: a rotor of blades turning while it feeds,
    // mouthparts working either side.
    let feeding = st.arms.iter().any(|a| a.state == 3);
    if feeding {
        st.rotor += 0.25;
    }
    for q in -1..=1 {
        let k = ((st.rotor + q as f32 * 2.0).floor() as i32).rem_euclid(3);
        blk(pen, ox + q as f32, oy - 1.0, if k == 0 { WHITE } else { DARK });
    }
    for sd in [-1.0f32, 1.0] {
        for q in 0..2 {
            let bob = if feeding && (t * 8.0 + q as f32 + sd).sin() > 0.0 { 1.0 } else { 0.0 };
            blk(pen, ox + sd * (2.0 + q as f32), oy - 1.0 - bob, m_bone(q, 3));
        }
    }
    // The claws, one a side: three segments out from the shoulder, and a big
    // claw, a fixed jaw with a saw edge and a moving one, open as it reaches,
    // shut on what it lifts; one clean shear per cut.
    for k in 0..2 {
        let a = st.arms[k];
        let side = if k == 0 { 1.0f32 } else { -1.0 };
        let sh = (b + side * CRAB_SHOULDER, by + 3.0);
        let tip = if a.state == 0 { (sh.0 + side * (6.0 + (t * 1.3 + k as f32).sin() * 1.5), sh.1 - 6.0) } else { a.tip };
        let (dx, dy) = (tip.0 - sh.0, tip.1 - sh.1);
        for (j, (u0, u1, w)) in [(0.0f32, 0.45f32, 3u8), (0.4, 0.78, 2), (0.74, 1.0, 2)].into_iter().enumerate() {
            let p0 = (sh.0 + dx * u0, sh.1 + dy * u0);
            let p1 = (sh.0 + dx * u1, sh.1 + dy * u1);
            let c = match j {
                0 => sc(m_armour(j as i32, 0), 1.2),
                1 => sc(m_armour(j as i32, 1), 1.1),
                _ => m_light(j as i32, 2),
            };
            for o in 0..w {
                path(pen, &[(p0.0, p0.1 + o as f32), (p1.0, p1.1 + o as f32)], &|_| c);
            }
            blk(pen, p0.0, p0.1, DARK);
        }
        let (tx, ty) = (tip.0.round(), tip.1.round());
        let o = match a.state {
            2 => {
                if a.t < 0.15 {
                    3.0
                } else if a.t < 0.3 {
                    0.0
                } else {
                    1.0
                }
            }
            1 => 3.0,
            _ => 0.0,
        };
        // The fixed jaw, two blocks deep, its saw edge on top.
        for q in -1..=6 {
            blk(pen, tx + side * q as f32, ty + 1.0, sc(m_armour(q, 0), 1.15));
            blk(pen, tx + side * q as f32, ty + 2.0, sc(m_armour(q, 1), 1.05));
            if q % 2 != 0 {
                blk(pen, tx + side * q as f32, ty, WHITE);
            }
        }
        // The moving jaw, hinged at the root.
        for q in 0..=5 {
            blk(pen, tx + side * q as f32, ty - 1.0 - o * (q as f32 / 5.0), m_light(q, 1));
        }
        if a.load {
            for (du, dw) in [(0.0, -1.0), (side, -1.0), (0.0, -2.0), (side * 2.0, -1.0)] {
                blk(pen, tx + du, ty + dw, WOOD);
            }
        }
    }
    // Eye stalks in the middle of the shell: flexible, segmented, each
    // swinging on its own, its tip an eye in the swarm's light colour.
    let mut eyes = [(0.0f32, 0.0f32); 2];
    for (i, e) in st.stalks.iter().enumerate() {
        let ex = CRAB_STALKS[i];
        let base = (ox + ex, oy + top(ex as i32) as f32 + 1.0);
        let dir = (e.a.sin(), e.a.cos());
        let tip = (base.0 + dir.0 * 8.0, base.1 + dir.1 * 8.0);
        let ctrl = (base.0 + dir.0 * 1.5, base.1 + 4.0);
        path(pen, &bez(base, ctrl, tip, 10), &|q| if q % 3 == 2 { m_bone(q as i32, i as i32) } else { m_strut(q as i32, i as i32) });
        let (lx, ly) = ((tip.0 + dir.0).round(), (tip.1 + dir.1).round());
        eyes[i] = (lx, ly);
        let focus = e.mode == 1;
        let p = if focus { 0.8 + 0.2 * (t * 9.0).sin() } else { 0.35 + 0.3 * (t * 1.4 + e.ph).sin() };
        blk(pen, tip.0, tip.1, mx([14.0, 20.0, 18.0], lamp, p * 0.7));
        blk(pen, lx, ly, if focus { mx(lamp, [255.0; 3], 0.45) } else { mx([14.0, 20.0, 18.0], lamp, p) });
        let (wx, wy) = wpt(lx, ly);
        crate::fx::emit(wx, wy, if focus { 6.0 } else { 3.0 }, lamp, if focus { 0.9 } else { 0.35 });
    }
    // The laser, from the eye on the side it fires to, sweeping the ground.
    if firing {
        let em = if laser_mark.0 >= b { if eyes[1].0 > eyes[0].0 { eyes[1] } else { eyes[0] } } else if eyes[1].0 < eyes[0].0 { eyes[1] } else { eyes[0] };
        let flick = mhash((t * 30.0) as i32, un.id as i32, 7) < 0.6;
        let c = if flick { [255.0, 220.0, 150.0] } else { [255.0, 150.0, 60.0] };
        let (x0, y0, x1, y1) = (em.0 as i32, em.1 as i32, laser_mark.0.floor() as i32, laser_mark.1.floor() as i32);
        let (mut ax, mut ay) = (x0, y0);
        let (ddx, ddy) = ((x1 - ax).abs(), -(y1 - ay).abs());
        let (sx, sy) = (if ax < x1 { 1 } else { -1 }, if ay < y1 { 1 } else { -1 });
        let mut err = ddx + ddy;
        for _ in 0..600 {
            blk(pen, ax as f32, ay as f32, c);
            if ax == x1 && ay == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= ddy {
                err += ddy;
                ax += sx;
            }
            if e2 <= ddx {
                err += ddx;
                ay += sy;
            }
        }
        blk(pen, laser_mark.0, laser_mark.1, [255.0, 250.0, 240.0]);
        crate::fx::emit(laser_world.0, laser_world.1, 10.0, [255.0, 160.0, 60.0], 1.5);
        if mhash((real_t * 20.0) as i32, un.id as i32, 5) < 0.3 {
            crate::fx::smoke(laser_world.0, laser_world.1, [70.0, 66.0, 62.0]);
        }
    }
}

// ---- Placeholder animals (the Dev planet's herds, dft_sim fauna.rs) ----

/// Draw every animal in view: a block body sized by its weight, a head, two
/// legs that trot while it moves, and a white tail raised while it flees.
/// Placeholder art until Fauna L1 gives them real bodies.
#[allow(clippy::too_many_arguments)]
pub fn draw_animals(animals: &[dft_sim::swarm::AnimalView], now: f64, real_t: f64, v: &View, bounds: [f64; 4], cell_px: f32, pos: &mut Vec<[f32; 3]>, col: &mut Vec<[f32; 4]>) {
    let [l, b, r, t] = bounds;
    let tt = real_t as f32;
    for an in animals {
        let (x, y) = an.at(now);
        if x < l - 10.0 || x > r + 10.0 || y < b - 10.0 || y > t + 10.0 {
            continue;
        }
        let p = v.to_screen(x, y);
        let up_angle = (y.atan2(x) - std::f64::consts::FRAC_PI_2) as f32;
        let mut pen = Pen { pos: &mut *pos, col: &mut *col, p, half: cell_px * 0.5, angle: up_angle };
        let f = if an.facing >= 0.0 { 1i32 } else { -1 };
        // Size by weight: 40 kg is 2 blocks long, 400 kg 5 long and 2 high.
        let long = (2.0 + an.kg / 120.0).round().clamp(2.0, 5.0) as i32;
        let high = if an.kg > 220.0 { 2 } else { 1 };
        let hide = |u: i32, w: i32| {
            let k = 0.36 + 0.08 * shade(an.id, u, w);
            lin(k * 1.25, k * 0.95, k * 0.62)
        };
        let moving = an.from != an.to;
        let trot = if moving { ((tt * if an.fleeing { 9.0 } else { 3.0 } + an.id as f32).sin() > 0.0) as i32 } else { 0 };
        // Body over its legs; u runs from its tail (u0) to its nose.
        let u0 = -long / 2;
        for i in 0..long {
            for w in 1..=high {
                let u = (u0 + i) * f;
                pen.block(u as f32, w as f32, hide(u, w));
            }
        }
        // Head, a block up at the front, with a dark eye block for big ones.
        let nose = (u0 + long) * f;
        pen.block(nose as f32, (high + 1) as f32, hide(nose, high + 1));
        if high > 1 {
            pen.block(nose as f32, high as f32, hide(nose, high));
        }
        // Legs: back and front, one block, stepping in turn as it moves.
        let back = (u0 + trot) * f;
        let front = (u0 + long - 1 - trot) * f;
        let dark = lin(0.2, 0.15, 0.1);
        pen.block(back as f32, 0.0, dark);
        pen.block(front as f32, 0.0, dark);
        // Panic: the white tail goes up.
        if an.fleeing {
            let tail = (u0 - 1) * f;
            pen.block(tail as f32, (high + 1) as f32, lin(0.95, 0.95, 0.92));
        }
    }
}
