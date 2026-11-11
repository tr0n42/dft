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

/// A forester's shell, side on, facing right (+u), top row first: p rim,
/// A carapace, g groove (lit from within), r rostrum, m mouthparts. The
/// eye stalks, claws, legs and bag are added round it.
const CRAB: [&str; 6] = [
    "     pppppppppp     ",
    "   pAAAgAAAAgAAAp   ",
    " pAAAAAgAAAAgAAAAArr",
    "pAAAAAAgAAAAgAAAAAAr",
    " pAAAAAAAAAAAAAAAAmm",
    "   ppppppppppppppp  ",
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

/// Draw every specialist in view into the haulers' mesh.
#[allow(clippy::too_many_arguments)]
pub fn draw(units: &[UnitView], now: f64, real_t: f64, v: &View, bounds: [f64; 4], cell_px: f32, lc: [f32; 3], pos: &mut Vec<[f32; 3]>, col: &mut Vec<[f32; 4]>) {
    let [l, b, r, t] = bounds;
    let tt = real_t as f32;
    let tunneler = tunneler_rows();
    for un in units {
        let (x, y) = un.at(now);
        let reach = 60.0 + un.a.abs().max(0.0) as f64;
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
                    }
                    _ => {}
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
                let face = if un.a >= 0.0 { 1.0f32 } else { -1.0 };
                let mut pen = Pen { pos: &mut *pos, col: &mut *col, p, half, angle: up_angle };
                // Facing left, the frame is mirrored: u -> -u.
                let fu = |u: i32| if face > 0.0 { u } else { -u };
                let walking = un.state == 2 || (un.state == 3 && un.c == 0.0);
                if un.state == 0 {
                    // Folded under its own shell, falling.
                    let rows = ["  pppppp  ", " pAAAAAAp ", "pAAgAAgAAp", " pAAAAAAp ", "  bjbbjb  "];
                    pen.rows(&rows, -5, 0, |ch, u, w| Some(match ch {
                        b'A' => armour(un.id, u, w),
                        b'g' => light(lamp * 0.3),
                        b'b' => bone(un.id, u, w),
                        b'j' => sinew(un.id, u, w),
                        _ => {
                            let k = 0.2 + 0.05 * shade(un.id, u, w);
                            lin(k, k * 1.05, k * 1.04)
                        }
                    }));
                    continue;
                }
                // How far it has stood up (unfolding), and the body's sway on
                // its legs: sagging while a leg swings, jolting as it lands.
                let stand = if un.state == 1 { 0.5 } else { 1.0 };
                let gait = tt * if walking { 2.4 } else { 0.4 };
                let sag = if walking { (gait * 3.0).sin().abs() * 0.8 } else { 0.0 };
                let body_w = (7.0 * stand - sag).round() as i32;
                // The legs: three, hip to a knee riding high over the body,
                // knee to a foot on the ground; feet planted while the body
                // goes on, then lifted in a low arc.
                for j in 0..3 {
                    let hip = (fu(-6 + j * 6), body_w + 2);
                    let ph = (gait / std::f32::consts::TAU + j as f32 / 3.0).fract();
                    let swing = if walking && ph > 0.66 { (ph - 0.66) / 0.34 } else { 0.0 };
                    let home = -11.0 + j as f32 * 11.0;
                    let foot_u = if walking { home + 4.0 * ((ph / 0.66).min(1.0) * -2.0 + 1.0) + 8.0 * swing } else { home };
                    let foot = (fu(foot_u.round() as i32), (swing * (1.0 - swing) * 8.0).round() as i32);
                    let knee = (fu(((home * 0.7) + 2.0 * (j as f32 - 1.0)).round() as i32), body_w + (9.0 * stand) as i32 + 2);
                    pen.line(hip, knee, |k, u, w| if k % 4 == 3 { bone(un.id, u, w) } else { sinew(un.id, u, w) });
                    pen.line(knee, foot, |k, u, w| if k == 0 { lin(0.12, 0.13, 0.125) } else if k % 5 == 4 { bone(un.id, u, w) } else { armour(un.id, u, w) });
                }
                // The shell.
                let mouth = un.c == 1.0 && ((tt * 6.0) as i32 % 2 == 0);
                for (r, line) in CRAB.iter().enumerate() {
                    for (c, ch) in line.bytes().enumerate() {
                        if ch == b' ' {
                            continue;
                        }
                        let (u, w) = (fu(c as i32 - 10), body_w + 5 - r as i32);
                        let k = match ch {
                            b'A' => armour(un.id, u, w),
                            b'g' => light(lamp * (0.4 + 0.3 * ((tt * 1.5 + c as f32 * 0.4).sin() * 0.5 + 0.5))),
                            b'r' => bone(un.id, u, w),
                            b'm' => {
                                if mouth {
                                    sinew(un.id, u, w)
                                } else {
                                    lin(0.12, 0.13, 0.125)
                                }
                            }
                            _ => {
                                let k = 0.2 + 0.05 * shade(un.id, u, w);
                                lin(k, k * 1.05, k * 1.04)
                            }
                        };
                        pen.block(u as f32, w as f32, k);
                    }
                }
                // Eye stalks, tips in the swarm's light, flicking about.
                for (j, u0) in [3i32, 6].into_iter().enumerate() {
                    let lean = ((tt * 0.9 + j as f32 * 2.1).sin() * 1.3).round() as i32;
                    let base = (fu(u0), body_w + 6);
                    let top = (fu(u0 + lean), body_w + 9);
                    pen.line(base, top, |k, u, w| if k == 3 { light(lamp) } else { sinew(un.id, u, w) });
                }
                // Claws, held out in front: open as they reach, one clean
                // shear per cut.
                let shear = un.c == 1.0 && ((tt * 3.0).fract() < 0.5);
                for (j, w0) in [(0i32, body_w + 1), (1, body_w - 1)] {
                    let reach = 11 + j * 2;
                    pen.line((fu(9), w0), (fu(reach), w0 + 2 - j * 3), |k, u, w| if k % 3 == 2 { bone(un.id, u, w) } else { armour(un.id, u, w) });
                    let tip_w = w0 + 2 - j * 3;
                    let gap = if shear { 0 } else { 1 };
                    pen.block(fu(reach + 1) as f32, (tip_w + gap) as f32, lin(0.56, 0.56, 0.58));
                    pen.block(fu(reach + 1) as f32, (tip_w - gap) as f32, lin(0.56, 0.56, 0.58));
                }
                // The clippings bag slung behind, filling (woven, in the
                // colours of what it holds).
                let fill = (un.b.clamp(0.0, 1.0) * 12.0).round() as i32;
                for k in 0..16 {
                    let (bu, bw) = (-13 - k % 4, body_w - 2 + k / 4);
                    let c = if k < fill { lin(0.32 + 0.1 * shade(un.id, k, 1), 0.42, 0.18) } else if (bu + bw) % 2 == 0 { sinew(un.id, bu, bw) } else { bone(un.id, bu, bw) };
                    pen.block(fu(bu) as f32, bw as f32, c);
                }
                // The laser, from the eye to its mark, for a moment.
                if un.c == 2.0 {
                    let to = local(un.mark);
                    let from = (fu(6), body_w + 9);
                    pen.line(from, (to.0.round() as i32, to.1.round() as i32), |_, _, _| light(1.0));
                }
            }
            _ => {}
        }
    }
}
