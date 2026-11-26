//! The specialists on screen: sappers, tunnelers, heat-sink bots, foresters
//! (a crab) and abductors (a spider), drawn
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

/// A tripod's legs (see rig.rs), as the mockup stands them, in its frame
/// (x toward where it faces): the crab's hips spread along its wide shell,
/// the spider's close under its head. The body rides at the simulation's
/// lift (50 m over the tallest plant), or the mockup's height if that is
/// higher; taller, the legs are simply longer.
fn gait_of(un: &UnitView) -> crate::rig::GaitSpec {
    if un.kind == Unit::Forester {
        crate::rig::GaitSpec { hips: [-10.0, 2.0, 14.0], spread: [-32.0, 3.0, 34.0], height: un.lift.max(50.0) }
    } else {
        crate::rig::GaitSpec { hips: [6.0, 9.0, 12.0], spread: [-26.0, 4.0, 30.0], height: un.lift.max(62.0) }
    }
}

/// The sapper's sickly green: the unstable charge glowing through its hull.
const SAP_GREEN: [f32; 3] = [0.6, 1.0, 0.26];

/// Of the arming countdown: heating up until this share, melting down until
/// the next, then the core collapsing into the blast.
const SAP_HEAT_TO: f32 = 0.42;
const SAP_MELT_TO: f32 = 0.88;

/// Hull colour as it heats (0 cold armour .. 1 yellow-white).
fn sap_heat(base: [f32; 3], h: f32) -> [f32; 3] {
    let stops: [(f32, [f32; 3]); 4] = [(0.0, base), (0.35, [0.5, 0.1, 0.05]), (0.7, [1.0, 0.42, 0.08]), (1.0, [1.0, 0.88, 0.55])];
    let h = h.clamp(0.0, 1.0);
    for w in stops.windows(2) {
        if h <= w[1].0 {
            let f = (h - w[0].0) / (w[1].0 - w[0].0);
            return [w[0].1[0] + (w[1].1[0] - w[0].1[0]) * f, w[0].1[1] + (w[1].1[1] - w[0].1[1]) * f, w[0].1[2] + (w[1].1[2] - w[0].1[2]) * f];
        }
    }
    stops[3].1
}

/// A sapper (Benjamin, 26 Nov: "lateral thrusters... more animated like a
/// maniacal suicide bot... a sickly green glow from the unstable material in
/// it, lighting and sparks and radiation... when it begins its explosion
/// sequence, have it heat up, melt down, then detonate").
///
/// A squat armoured canister with a window onto its green core, a thruster
/// pod either side and a single darting eye. It never holds still: it
/// twitches, its eye flicks about, its side thrusters kick it along in
/// bursts. Its core crackles with radiation (green specks round it, green
/// sparks leaking, green light). Armed, it heats from the core out (dull red
/// to yellow-white), shaking harder; then melts down, its hull sagging and
/// dripping away to bare the core blazing green-white; then the core
/// collapses to a point and the blast takes it.
#[allow(clippy::too_many_arguments)]
fn sapper(un: &UnitView, now: f64, real_t: f64, x: f64, y: f64, p: Vec2, half: f32, up_angle: f32, cell_px: f32, lamp: f32, lc: [f32; 3], pos: &mut Vec<[f32; 3]>, col: &mut Vec<[f32; 4]>) {
    let tt = real_t as f32;
    let id = un.id;
    // Random numbers that change a few times a second (its twitching).
    let beat = |rate: f32, k: i32| shade(id, (tt * rate) as i32, k);
    let armed = un.state == 3;
    // How far through the countdown (0 armed .. 1 the blast).
    let span = (un.t1 - un.t0).max(1e-6);
    let frac = ((now - un.t0) / span).clamp(0.0, 1.0) as f32;
    let k = if armed { ((un.a + frac) / un.c.max(1.0)).clamp(0.0, 1.0) } else { 0.0 };
    let heat = (k / SAP_HEAT_TO).min(1.0);
    let melt = ((k - SAP_HEAT_TO) / (SAP_MELT_TO - SAP_HEAT_TO)).clamp(0.0, 1.0);
    let end = ((k - SAP_MELT_TO) / (1.0 - SAP_MELT_TO)).clamp(0.0, 1.0);
    // From its own frame (m along, up) to the world.
    let (sa, ca) = (up_angle as f64).sin_cos();
    let world = |u: f64, w: f64| (x + u * ca - w * sa, y + u * sa + w * ca);
    // Which way it is going, in its frame.
    let (mu, mw) = {
        let (dx, dy) = (un.to.0 - un.from.0, un.to.1 - un.from.1);
        ((dx * ca + dy * sa) as f32, (-dx * sa + dy * ca) as f32)
    };
    // Its twitching: a jerk a metre aside now and then (shaking harder the
    // nearer the blast), and a jittery tilt.
    let (shake_rate, shake_p, shake_m) = match un.state {
        3 => (14.0 + 30.0 * k, 0.4 + 0.6 * k, if melt > 0.0 { 2.0 } else { 1.0 }),
        2 => (16.0, 0.55, 1.0),
        _ => (7.0, 0.22, 1.0),
    };
    let (ju, jw) = if end > 0.0 || beat(shake_rate, 1) >= shake_p {
        (0.0, 0.0)
    } else {
        let m = shake_m;
        ((beat(shake_rate, 2) * (2.0 * m + 1.0)).floor() - m, (beat(shake_rate, 3) * 3.0).floor() - 1.0)
    };
    let tilt = if un.state == 1 || un.state == 0 { (beat(5.0, 4) - 0.5) * 0.3 } else { 0.0 };
    let angle = up_angle + tilt;
    let (st, ct) = angle.sin_cos();
    let at = Vec2::new(p.x + (ju * ct - jw * st) * cell_px, p.y + (ju * st + jw * ct) * cell_px);
    let mut pen = Pen { pos, col, p: at, half, angle };

    // ---- The core: sickly green, flickering; brighter and whiter armed ----
    let flick = 0.75 + 0.25 * beat(22.0 + 40.0 * k, 5);
    let core_k = (0.55 + 0.45 * heat) * flick;
    let core_white = (0.5 * melt + 0.5 * end).min(1.0);
    let core = |b: f32| -> [f32; 4] {
        let c = [SAP_GREEN[0] * b, SAP_GREEN[1] * b, SAP_GREEN[2] * b];
        lin(c[0] + (1.0 - c[0]) * core_white, c[1] + (1.0 - c[1]) * core_white, c[2] + (1.0 - c[2]) * core_white)
    };

    if end > 0.0 {
        // The end: what is left of it falls in on the core, which shrinks to
        // a white-green point inside a ring of light closing on it.
        let r = (1.0 - end) * 7.0;
        let n = ((r * 7.0) as usize).max(8);
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU + tt * 3.0;
            pen.block((a.cos() * r).round(), (a.sin() * r).round(), lina(0.85, 1.0, 0.7, 0.4 + 0.6 * end));
        }
        let size = if end < 0.6 { 1 } else { 0 };
        for du in -size..=size {
            for dw in -size..=size {
                pen.block(du as f32, dw as f32, lin(0.95, 1.0, 0.85));
            }
        }
        crate::fx::emit(x, y, 10.0 + 30.0 * end as f64, [190.0, 255.0, 150.0], 1.0 + 1.5 * end);
        return;
    }

    // ---- The hull (top row first, its lower-left corner at (-3, -2)) ----
    // A armour, g core window, G the core's heart, T thruster pod, p dark
    // plate, L the eye's slot, c clamp.
    const ROWS: [&str; 6] = ["  AAA  ", " AAAAA ", "TAgGgAT", "TAgggAT", " ApLpA ", "  c c  "];
    // The eye darts about its slot (and stares, fixed and blazing, armed).
    let eye_u = if armed { 0 } else { (beat(4.0 + 3.0 * shade(id, 0, 9), 6) * 3.0) as i32 - 1 };
    let eye_on = armed || beat(9.0, 7) > 0.18;
    let h = ROWS.len() as i32;
    for (r, line) in ROWS.iter().enumerate() {
        for (c, ch) in line.bytes().enumerate() {
            if ch == b' ' {
                continue;
            }
            let (u, w) = (c as i32 - 3, h - 1 - r as i32 - 2);
            let glow_part = matches!(ch, b'g' | b'G');
            // Heat runs out from the core (at about (0, 0.5)).
            let d = ((u * u) as f32 + ((w as f32 - 0.5) * (w as f32 - 0.5))).sqrt();
            let hb = (heat * 1.5 - d * 0.18).clamp(0.0, 1.0);
            // Melting: each block of hull lets go at its own moment, sags
            // and drips away toward the ground, glowing.
            let mut dw = 0.0f32;
            let mut alpha = 1.0f32;
            if melt > 0.0 && !glow_part {
                let lets_go = 0.75 * shade(id, u, w + 40) + 0.08 * (w as f32 - 0.5).max(0.0);
                let m = ((melt - lets_go) / 0.35).max(0.0);
                if m > 0.0 {
                    dw = -(m * m * 7.0).floor();
                    alpha = (1.0 - m * 0.7).max(0.0);
                    if alpha <= 0.05 {
                        continue;
                    }
                }
            }
            let colour = match ch {
                b'g' => core(core_k * (0.62 + 0.25 * shade(id, u, w))),
                b'G' => core(core_k),
                b'L' => {
                    if u == eye_u && eye_on {
                        if armed { lin(1.0, 0.25 + 0.6 * (1.0 - heat), 0.15) } else { lin(lc[0] * lamp, lc[1] * lamp, lc[2] * lamp) }
                    } else {
                        lin(0.06, 0.07, 0.07)
                    }
                }
                _ => {
                    let base = match ch {
                        b'T' => [0.2, 0.21, 0.22],
                        b'p' => [0.13, 0.14, 0.15],
                        b'c' => [0.36, 0.37, 0.38],
                        _ => {
                            let a = armour(id, u, w);
                            // (armour() is linear; back to roughly sRGB.)
                            [a[0].powf(1.0 / 2.2), a[1].powf(1.0 / 2.2), a[2].powf(1.0 / 2.2)]
                        }
                    };
                    // A sickly cast where the core shines through the seams.
                    let seam = if d < 2.3 { 0.12 * flick } else { 0.0 };
                    let mut c = [base[0] + seam * 0.4, base[1] + seam, base[2] + seam * 0.2];
                    if dw < 0.0 {
                        // Molten and running.
                        c = sap_heat(c, 0.75 + 0.25 * shade(id, u, (tt * 10.0) as i32));
                    } else {
                        c = sap_heat(c, hb);
                    }
                    lina(c[0], c[1], c[2], alpha)
                }
            };
            pen.block(u as f32, w as f32 + dw, colour);
            // Drips throw sparks as they go.
            if dw < -1.0 && shade(id, u * 31 + w, (tt * 20.0) as i32) < 0.06 {
                let (wx, wy) = world((u as f32 + ju) as f64, (w as f32 + dw + jw) as f64);
                crate::fx::spark(wx, wy, 1, [255.0, 170.0, 70.0], 6.0);
            }
        }
    }

    // ---- Thrusters ----
    // A plume out of a pod or the base: white-hot at the nozzle, then
    // orange, ragged at the tip.
    let plume = |pen: &mut Pen, from: (i32, i32), dir: (i32, i32), len: i32, k: i32| {
        for i in 0..len {
            let (u, w) = (from.0 + dir.0 * i, from.1 + dir.1 * i);
            let q = i as f32 / len.max(1) as f32;
            let c = if i == 0 { lin(1.0, 0.96, 0.82) } else if q < 0.5 { lina(1.0, 0.7, 0.3, 0.9) } else { lina(0.95, 0.42, 0.15, 0.75 - 0.4 * q) };
            pen.block(u as f32, w as f32, c);
        }
        if shade(id, k, (tt * 30.0) as i32) < 0.25 {
            let (wx, wy) = world((from.0 + dir.0 * len) as f64, from.1 as f64 + 0.5);
            crate::fx::spark(wx, wy, 1, [255.0, 200.0, 120.0], 9.0);
        }
        let (wx, wy) = world(from.0 as f64 + 0.5, from.1 as f64 + 0.5);
        crate::fx::emit(wx, wy, 5.0, [255.0, 180.0, 100.0], 0.5);
    };
    // Which side pods fire now: in bursts, against the way it is going,
    // with twitchy kicks either way; armed, sputtering in panic; molten,
    // dead.
    let burst = |rate: f32, k: i32, p: f32| beat(rate, k) < p;
    let (mut left, mut right) = (false, false);
    match un.state {
        0 => {
            // Dropping: the main thruster braking under it, side pods
            // kicking it this way and that.
            let n = 2 + (beat(20.0, 10) * 3.0) as i32;
            plume(&mut pen, (0, -3), (0, -1), n, 11);
            if burst(6.0, 12, 0.4) {
                if beat(6.0, 13) < 0.5 { left = true } else { right = true }
            }
        }
        1 => {
            // Crawling: kicked along by the pod behind it, in bursts.
            if mu.abs() > 0.15 && burst(8.0, 14, 0.7) {
                if mu > 0.0 { left = true } else { right = true }
            } else if mw.abs() > 0.15 && burst(10.0, 15, 0.5) {
                if beat(10.0, 16) < 0.5 { left = true } else { right = true }
            }
            if burst(5.0, 17, 0.12) {
                left = !left;
            }
        }
        2 => {
            // Drilling: both pods bracing it, in turns.
            if beat(12.0, 18) < 0.5 { left = burst(12.0, 19, 0.7) } else { right = burst(12.0, 19, 0.7) }
        }
        _ => {
            if melt <= 0.0 {
                left = burst(18.0 + 30.0 * heat, 20, 0.15 + 0.4 * heat);
                right = burst(18.0 + 30.0 * heat, 21, 0.15 + 0.4 * heat);
            }
        }
    }
    let len = |k: i32| 1 + (beat(24.0, k) * 3.0) as i32;
    if left {
        plume(&mut pen, (-4, 0), (-1, 0), len(22), 23);
    }
    if right {
        plume(&mut pen, (4, 0), (1, 0), len(24), 25);
    }
    // Drilling: the bit turning under it, hot flecks, dust out of the hole.
    if un.state == 2 {
        let spin = ((tt * 20.0) as i32 % 2) as f32;
        pen.block(-1.0 + spin, -3.0, lin(0.89, 0.88, 0.84));
        pen.block(0.0 - spin, -3.0, lin(0.59, 0.59, 0.61));
        pen.block(0.0, -4.0, lina(1.0, 0.86, 0.6, 0.9));
        let (bx, by) = world(0.0, -4.0);
        if beat(40.0, 26) < 0.5 {
            crate::fx::spark(bx, by, 1, [255.0, 210.0, 140.0], 10.0);
        }
        crate::fx::emit(bx, by, 4.0, [255.0, 190.0, 110.0], 0.6);
        if beat(8.0, 27) < 0.13 {
            crate::fx::dust(un.mark.0, un.mark.1 + 1.0, [110.0, 100.0, 88.0], 12.0, 4.0, 4.0);
        }
    }

    // ---- Radiation ----
    // Green specks crackling round it (a Geiger counter you can see), more
    // and further out the nearer the blast; green sparks leaking from the
    // core; and the core's green light.
    let specks = 3 + (12.0 * k) as i32;
    for i in 0..specks {
        if beat(14.0, 30 + i) < 0.45 {
            let a = beat(14.0, 60 + i) * std::f32::consts::TAU;
            let d = 3.0 + beat(14.0, 90 + i) * (3.0 + 6.0 * k);
            let b = 0.5 + 0.5 * beat(14.0, 120 + i);
            pen.block((a.cos() * d).round(), (0.5 + a.sin() * d).round(), lina(SAP_GREEN[0] * b + 0.2 * core_white, SAP_GREEN[1] * b, SAP_GREEN[2] * b + 0.2 * core_white, 0.35 + 0.5 * b));
        }
    }
    if beat(10.0 + 30.0 * k, 150) < 0.18 + 0.5 * k {
        let (cx, cy) = world((beat(30.0, 151) * 3.0 - 1.5) as f64, 0.5);
        crate::fx::spark(cx, cy, 1 + (k * 3.0) as usize, [150.0, 255.0, 90.0], 4.0 + 10.0 * k as f64);
    }
    let (cx, cy) = world(0.5, 1.0);
    crate::fx::emit(cx, cy, 9.0 + 14.0 * k as f64, [150.0, 255.0, 90.0], (0.45 + 0.7 * k) * flick);
    // Armed, the heat's own light, orange, as it glows.
    if heat > 0.0 {
        crate::fx::emit(cx, cy, 6.0 + 8.0 * heat as f64, [255.0, 140.0, 50.0], 0.4 + 0.6 * heat);
    }
    // Molten, smoke boiling off it.
    if melt > 0.0 && melt < 0.8 && beat(12.0, 160) < 0.12 {
        let (sx, sy) = world(0.5, 3.0);
        crate::fx::smoke(sx, sy, [110.0, 120.0, 96.0]);
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
            Unit::Sapper => sapper(un, now, real_t, x, y, p, half, up_angle, cell_px, lamp, lc, &mut *pos, &mut *col),
            Unit::Tunneler => {
                // Along its shaft, nose (the drill) at the head. States (the
                // opener): 0 flying down, 1 boring, 2 down a passage, 3
                // backing up, 4 done (arming), 5 sinking back down to go off.
                let heading = (un.b as f32).atan2(un.a as f32);
                let boring = un.state == 1 && un.sweep_from.0 > 0.0;
                let arming = un.state == 4;
                // Its crew: cutters carrying its spoil off from the shaft's
                // mouth (on screen only, as many as look right, up to 60).
                if un.lift > 0.0 && (boring || un.c > 0.02) {
                    let up = { let r = (un.mark.0 * un.mark.0 + un.mark.1 * un.mark.1).sqrt().max(1.0); (un.mark.0 / r, un.mark.1 / r) };
                    let over = (un.mark.0 + up.0 * 24.0, un.mark.1 + up.1 * 24.0);
                    draw_crew(pos, col, v, cell_px, un.id, (un.lift as usize).min(60), un.mark, over, 10.0, real_t, lc);
                }
                let mut pen = Pen { pos: &mut *pos, col: &mut *col, p, half, angle: heading };
                let turn = if boring { (tt * 6.0) as i32 } else { 0 };
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
                            b'L' => {
                                if arming {
                                    // Its end: the lights blink fast.
                                    if (tt * 10.0).sin() > 0.0 { lin(1.0, 0.35, 0.25) } else { light(0.2) }
                                } else {
                                    light(lamp)
                                }
                            }
                            _ => {
                                // The cone turns, grimy, its bands running
                                // along it; red-hot toward the tip while it
                                // bores (its nuclear drill heats the rock).
                                let band = (c as i32 + row as i32 + turn).rem_euclid(3) == 0;
                                let grime = shade(un.id, c as i32 + turn, row as i32);
                                let hot = if boring { ((c as f32 - 31.0) / 13.0).clamp(0.0, 1.0) } else { 0.0 };
                                let base = if boring && band { 0.72 + 0.12 * grime } else { 0.36 + 0.14 * grime };
                                lin(base + 0.5 * hot * hot, base * (1.0 - 0.35 * hot), base * (1.0 - 0.7 * hot))
                            }
                        };
                        pen.block(u as f32, w as f32, k);
                    }
                }
                // Its nose glows and spits while it bores.
                if boring {
                    let (s, c) = heading.sin_cos();
                    let nose = (x + c as f64 * 1.0, y + s as f64 * 1.0);
                    crate::fx::emit(nose.0, nose.1, 10.0, [255.0, 150.0, 70.0], 0.9);
                    if dft_sim::noise::unit(dft_sim::noise::hash(un.id as u64, (real_t * 30.0) as i64, 0, 0x7B1)) < 0.6 {
                        crate::fx::spark(nose.0, nose.1, 1, [255.0, 200.0, 120.0], 12.0);
                    }
                }
                if arming {
                    crate::fx::emit(x, y, 14.0, [255.0, 90.0, 60.0], if (tt * 10.0).sin() > 0.0 { 1.2 } else { 0.2 });
                }
                // Tentacles back from its flanks, gripping the shaft walls.
                if un.state == 1 || un.state == 4 {
                    for (j, side) in [(0, 1i32), (1, -1), (2, 1), (3, -1)] {
                        let u0 = -30 + j * 7;
                        let reach = 2 + ((tt * 0.8 + j as f32 * 1.7).sin() * 1.5).round() as i32;
                        pen.line((u0, side * 6), (u0 - 3, side * (7 + reach)), |k, u, w| if k % 3 == 2 { bone(un.id, u, w) } else { sinew(un.id, u, w) });
                    }
                }
            }
            Unit::HeatSink => {
                let working = un.state == 1;
                // On a deep job its crew digs out the passage's middle under
                // it (claw: the dug face), carrying it up past the bot.
                if working && un.lift > 0.0 {
                    let up = { let r = (x * x + y * y).sqrt().max(1.0); (x / r, y / r) };
                    let over = (x + up.0 * 14.0, y + up.1 * 14.0);
                    draw_crew(pos, col, v, cell_px, un.id, (un.lift as usize).min(40), un.claw, over, 5.0, real_t, lc);
                }
                let mut pen = Pen { pos: &mut *pos, col: &mut *col, p, half, angle: up_angle };
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

/// A unit's own crew of cutters (couriers of its spoil, drawn only: the
/// simulation counts their work): `n` small grey drones flying a ribbon
/// between the work face `from` and a point `to` over it and back, spread
/// `spread` metres either side, each its own pace. Each drone is the game's
/// small block cutter: a body and a light in the swarm's colour.
#[allow(clippy::too_many_arguments)]
fn draw_crew(pos: &mut Vec<[f32; 3]>, col: &mut Vec<[f32; 4]>, v: &View, cell_px: f32, id: u32, n: usize, from: (f64, f64), to: (f64, f64), spread: f64, real_t: f64, lc: [f32; 3]) {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let side = (-dy / len, dx / len);
    let half = cell_px * 0.5;
    let up = (from.1.atan2(from.0) - std::f64::consts::FRAC_PI_2) as f32;
    for i in 0..n {
        let r = |k: u64| dft_sim::noise::unit(dft_sim::noise::hash(id as u64, i as i64, k as i64, 0xC4E)) as f64;
        // Up full, down empty: a trip in 6-10 s, each its own phase.
        let ph = (real_t / (6.0 + 4.0 * r(1)) + r(2)).fract();
        let along = if ph < 0.5 { ph * 2.0 } else { 2.0 - ph * 2.0 };
        let off = (r(3) - 0.5) * 2.0 * spread + (real_t * (1.0 + r(4)) + 6.0 * r(5)).sin() * 1.5;
        let wx = from.0 + dx * along + side.0 * off;
        let wy = from.1 + dy * along + side.1 * off;
        let p = v.to_screen(wx, wy);
        let shade = 0.3 + 0.12 * r(6) as f32;
        let body = lin(shade, shade * 1.03, shade * 1.05);
        // A two-by-two body, a light on top.
        for (u, w) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)] {
            crate::drones::rot_square(pos, p.x, p.y, half, up, u, w, 1.0);
            col.extend_from_slice(&[body; 6]);
        }
        let lamp = if ph < 0.5 { 1.0 } else { 0.55 };
        crate::drones::rot_square(pos, p.x, p.y, half, up, 1.0, 1.0, 1.0);
        col.extend_from_slice(&[lin(lc[0] * lamp, lc[1] * lamp, lc[2] * lamp); 6]);
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
    /// When it next vents its store (real time).
    next_vent: f64,
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

/// A path of blocks joined edge to edge (the mockup's drawPath), in the
/// mockup's frame mirrored by `face` (1: the unit's own frame as is);
/// `colour` by point.
fn tri_path(pen: &mut Pen, face: f32, pts: &[(f32, f32)], colour: &dyn Fn(usize) -> Rgb) {
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

/// A leg (the mockup's drawLeg), in the unit's own frame (u along, w up):
/// hip to a high knee to the foot, two curves, ringed in bone; the knee
/// swaying a little of its own; `thick` blocks thick (the second strand
/// toward `fwd`); a bone knuckle at the knee, a two-toed foot.
#[allow(clippy::too_many_arguments)]
fn draw_leg(pen: &mut Pen, hip: (f32, f32), foot: (f32, f32), i: usize, t: f32, knee_up: f32, thick: u8, fwd: f32) {
    let out = if foot.0 < hip.0 { -1.0 } else { 1.0 };
    let fi = i as f32;
    let ii = i as i32;
    let knee = ((hip.0 + foot.0) / 2.0 + out * 6.0 + (t * 0.8 + fi).sin() * 1.5, hip.1.max(foot.1) + knee_up + (t * 1.1 + fi * 2.0).sin() * 1.2);
    let a = bez(hip, (hip.0 + out * 2.0, knee.1 + 1.0), knee, 14);
    let b = bez(knee, (knee.0 + out * 3.0, (knee.1 + foot.1) / 2.0), foot, 28);
    let col = |k: usize| if k % 3 == 0 { sc(m_bone(k as i32, ii), 1.1) } else { mx(m_light(k as i32, ii), m_sinew(k as i32, ii), 0.35) };
    tri_path(pen, 1.0, &a, &col);
    tri_path(pen, 1.0, &b, &col);
    if thick > 1 {
        let a2: Vec<(f32, f32)> = a.iter().map(|p| (p.0 + fwd, p.1)).collect();
        let b2: Vec<(f32, f32)> = b.iter().take(10).map(|p| (p.0 + fwd, p.1)).collect();
        tri_path(pen, 1.0, &a2, &col);
        tri_path(pen, 1.0, &b2, &col);
    }
    pen.block(knee.0.round(), knee.1.round(), rgb(sc(m_bone(4, ii), 1.2)));
    let (fx, fy) = (foot.0.round(), foot.1.round());
    pen.block(fx - 1.0, fy, rgb(m_bone(1, ii)));
    pen.block(fx + 1.0, fy, rgb(m_bone(2, ii)));
}

/// Draw a forester or an abductor, the mockup's way. `x, y` is where it
/// stands on the ground; `local` turns a world point into its frame.
#[allow(clippy::too_many_arguments)]
fn draw_tripod(pen: &mut Pen, un: &UnitView, x: f64, y: f64, now: f64, real_t: f64, lc: [f32; 3], local: &dyn Fn((f64, f64)) -> (f32, f32), solid: &dyn Fn(i64, i64) -> bool) {
    let forester = un.kind == Unit::Forester;
    let t = real_t as f32;
    let lamp: Rgb = [lc[0] * 255.0, lc[1] * 255.0, lc[2] * 255.0];
    // Which way it faces: its own frame's +u is clockwise round the body;
    // facing +1 (the simulation's) is counter-clockwise.
    let face = if un.a >= 0.0 { -1.0f32 } else { 1.0 };
    let (sa, ca) = (y.atan2(x) - std::f64::consts::FRAC_PI_2).sin_cos();
    // A point of its own frame (u along, w up) in the world.
    let wpt_u = move |u: f64, w: f64| -> (f64, f64) { (x + u * ca - w * sa, y + u * sa + w * ca) };
    // A point of the mockup's frame (the middle of block bx, by) in the
    // world, for lights and effects.
    let wpt = move |bx: f32, by: f32| -> (f64, f64) { wpt_u(face as f64 * (bx as f64 + 0.5), by as f64 + 0.5) };
    let emit = |bx: f32, by: f32, r: f64, c: Rgb, i: f32| {
        let (ex, ey) = wpt(bx, by);
        crate::fx::emit(ex, ey, r, c, i);
    };
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
    // ---- Flying folded, a bit like a UFO: a wobble and a glow under it ----
    if un.state == 0 && un.c >= 3.0 {
        let bob = ((t * 2.3 + un.id as f32).sin() * 1.5).round();
        draw_package(pen, &mut blk, forester, 0.0, bob, t, lamp);
        for (k, u) in [-6.0f32, -2.0, 2.0, 6.0].iter().enumerate() {
            let on = ((t * 6.0) as i32 + k as i32) % 4 == 0;
            blk(pen, *u, -2.0 + bob, if on { lamp } else { sc(lamp, 0.35) });
        }
        emit(0.0, -6.0, 12.0, lamp, 0.7);
        return;
    }
    // ---- Falling, folded into its package ----
    if un.state == 0 {
        draw_package(pen, &mut blk, forester, 0.0, 0.0, t, lamp);
        return;
    }

    // ---- Its legs and gait (rig.rs), its feet on the ground as drawn ----
    let r0 = (x * x + y * y).sqrt().max(1.0);
    let a0 = y.atan2(x);
    let ground = |u: f32| -> Option<f32> {
        // Down the radius `u` m along: the top of the first solid cell, as
        // height over where it stands.
        let a = a0 - u as f64 / r0;
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
    let spec = gait_of(un);
    // How fast the simulation has it going (m/s of game time): striding or
    // creeping at its work.
    let speed = if un.t1 > un.t0 { ((un.to.0 - un.from.0).hypot(un.to.1 - un.from.1) / (un.t1 - un.t0)) as f32 } else { 0.0 };
    let pose = crate::rig::step(un.id as u64 | ((un.kind.index() as u64) << 40), &spec, x, y, face, now, real_t, un.state, speed, &ground);
    // Landed and not yet unfolding: the package on the ground.
    if pose.unfold < 0.05 {
        draw_package(pen, &mut blk, forester, 0.0, 0.0, t, lamp);
        return;
    }
    let by = pose.body;
    // The body's lean and surge, in the mockup's frame.
    let b = pose.shift * face;
    let lean = pose.lean * face;
    // A foot coming down under load: dust.
    for (i, &l) in pose.landed.iter().enumerate() {
        if l {
            let f = pose.feet[i];
            let (fx, fy) = wpt_u(f.0 as f64, f.1 as f64 + 1.0);
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
        next_vent: real_t + 14.0 + 8.0 * mhash(un.id as i32, 1, 9) as f64,
        rng: un.id as u64 * 0x9E37 + 1,
        last_t: real_t,
    });
    let dt = (real_t - st.last_t).clamp(0.0, 0.05) as f32;
    st.last_t = real_t;
    // Turning about takes half a second.
    st.turn = if face > st.turn { (st.turn + dt / TURN_S * 2.0).min(face) } else { (st.turn - dt / TURN_S * 2.0).max(face) };
    turn.set(st.turn);
    // The store hangs here (the mockup's store()): the crab's bag slung
    // behind it, the spider's abdomen.
    let store = if forester { (b - 17.0, by + 2.0) } else { (b - 9.0, by + 2.0) };
    let fill = un.b.clamp(0.0, 1.0);

    // ---- The arms: each cut the simulation reports, a claw (palp) reaches,
    // works and lifts it into the store ----
    if un.cut_at > st.last_cut {
        st.pending = Some(un.claw);
        st.last_cut = un.cut_at;
    }
    let (reach_s, cut_s, lift_s) = if forester { (0.5, 0.5, 0.8) } else { (0.6, 0.0, 0.9) };
    for k in 0..2 {
        let mut a = st.arms[k];
        a.t += dt;
        let kf = k as f32;
        let sh = if forester { (b + 15.0 - kf * 4.0, by + 2.0) } else { (b + 12.0 - kf * 3.0, by + 1.0) };
        match a.state {
            0 => {
                if a.t > 0.6 {
                    if let Some(m) = st.pending.take() {
                        a.target = m;
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

    // ---- The eye stalks (forester): searching, swinging slowly from side
    // to side; when there is something to watch (the laser's mark, a claw
    // at work) they snap round and hold on it; now and then one glances back
    // at the bag or across at the other. Each turns on a spring: quick to
    // focus, lazy in the search ----
    if forester {
        let busy = st.arms.iter().find(|a| a.state == 1 || a.state == 2).map(|a| a.tip);
        let poi = if firing { Some(laser_mark) } else { busy };
        for i in 0..2 {
            let ex = [9.0f32, 12.0][i];
            let base = (b + 2.0 + ex, by + 8.0);
            let lim = |a: f32| a.clamp(-0.95, 0.95);
            let mut e = st.stalks[i];
            e.t += dt;
            let r1 = st.rand();
            let r2 = st.rand();
            match e.mode {
                0 => {
                    // Darting: hold still on one far point, then flick to
                    // the next.
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
            // A stiff, well-damped spring: quick flicks that stop dead.
            e.va += ((e.goal - e.a) * 160.0 - e.va * 24.0) * dt;
            e.a = lim(e.a + e.va * dt);
            st.stalks[i] = e;
        }
    }

    // ---- Venting: now and then a pressure-release plume from the store
    // (dry plant matter, or blood) for the ground and the cutters ----
    if real_t > st.next_vent {
        st.next_vent = real_t + 16.0 + 12.0 * st.rand() as f64;
        if fill > 0.3 {
            let (vx, vy) = wpt(store.0, store.1);
            let c0: Rgb = if forester { [200.0, 190.0, 120.0] } else { [200.0, 60.0, 60.0] };
            crate::fx::emit(vx, vy, 12.0, c0, 1.5);
            crate::fx::spark(vx, vy, 6, c0, 4.8);
            for _ in 0..8 {
                let (q1, q2) = (st.rand(), st.rand());
                let c = if forester { mx(mx([150.0, 130.0, 80.0], [110.0, 120.0, 60.0], q1), [90.0, 70.0, 40.0], q2 * 0.5) } else { mx([118.0, 10.0, 18.0], [150.0, 20.0, 30.0], q1) };
                crate::fx::spark(vx, vy, 5, c, 14.0);
            }
            if forester {
                for _ in 0..4 {
                    crate::fx::dust(vx, vy, [150.0, 136.0, 96.0], 10.0, 8.0, 2.5);
                }
            }
        }
    }

    // ---- Drawing, in the mockup's order ----
    // Legs: hip to a high knee to the foot.
    for i in 0..3 {
        // (Mirrored as its blocks are.)
        let hip = (turn.get() * (b + spec.hips[i] + 0.5) - 0.5, by + 3.0);
        draw_leg(pen, hip, pose.feet[i], i, t, if forester { 7.0 } else { 13.0 }, if forester { 2 } else { 1 }, face);
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
                1 => ((a.t / 0.6).min(1.0) * std::f32::consts::PI).sin() * 7.0,
                3 => -4.0 * ((a.t / 0.9).min(1.0) * std::f32::consts::PI).sin(),
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
            tri_path(pen, turn.get(), &pts, &col);
            let thick: Vec<(f32, f32)> = pts.iter().take(10).map(|p| (p.0 + 1.0, p.1)).collect();
            tri_path(pen, turn.get(), &thick, &col);
            // The end: a barbed hook that closes round what it catches.
            let (tx, ty) = (tip.0.round(), tip.1.round());
            let shut = a.state == 3;
            blk(pen, tx, ty, sc(m_bone(3, k as i32), 1.2));
            blk(pen, tx + if shut { 0.0 } else { 1.0 }, ty - 1.0, WHITE);
            blk(pen, tx - 1.0, ty + if shut { 0.0 } else { 1.0 }, WHITE);
            if a.state == 1 && a.t > 0.5 {
                emit(tx, ty, 3.0, lamp, 0.4);
            }
            if a.load {
                blk(pen, tx, ty - 1.0, MEAT);
                blk(pen, tx - 1.0, ty - 1.0, MEAT);
                blk(pen, tx, ty - 2.0, MEAT);
            }
            continue;
        }
        // The claw's arm: three segments out from the shoulder, jointed.
        for (j, (u0, u1, w)) in [(0.0f32, 0.45f32, 2u8), (0.4, 0.78, 2), (0.74, 1.0, 1)].into_iter().enumerate() {
            let p0 = (sh.0 + dx * u0, sh.1 + dy * u0);
            let p1 = (sh.0 + dx * u1, sh.1 + dy * u1);
            let c = match j {
                0 => sc(m_armour(j as i32, 0), 1.2),
                1 => sc(m_armour(j as i32, 1), 1.1),
                _ => m_light(j as i32, 2),
            };
            tri_path(pen, turn.get(), &[p0, p1], &|_| c);
            if w > 1 {
                tri_path(pen, turn.get(), &[(p0.0 + 1.0, p0.1), (p1.0 + 1.0, p1.1)], &|_| c);
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
        // fast when it has something in its sights, the brightest one
        // flicking from eye to eye.
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
        // Mouthparts under the rostrum, still unless it is feeding.
        let feeding = st.arms.iter().any(|a| a.state == 3);
        for q in 0..4 {
            let bob = if feeding && (t * 8.0 + q as f32).sin() > 0.0 { 1.0 } else { 0.0 };
            blk(pen, ox + 16.0 + q as f32, oy - 1.0 - (q % 2) as f32 - bob, m_bone(q, 3));
        }
        // (The mockup's quarter turn a frame, at 60 frames a second.)
        if feeding {
            st.rotor = (st.rotor + dt * 15.0).rem_euclid(3000.0);
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
            tri_path(pen, turn.get(), &bez(base, ctrl, tip, 10), &|q| if q % 3 == 2 { m_bone(q as i32, ex) } else { m_strut(q as i32, ex) });
            let (tx, ty) = (tip.0.round(), tip.1.round());
            let (lx, ly) = ((tip.0 + dir.0).round(), (tip.1 + dir.1).round());
            let focus = e.mode == 1;
            let p = if focus { 0.8 + 0.2 * (t * 9.0).sin() } else { 0.35 + 0.3 * (t * 1.4 + e.ph).sin() };
            blk(pen, tx, ty, mx([14.0, 20.0, 18.0], lamp, p * 0.7));
            blk(pen, lx, ly, if focus { mx(lamp, [255.0; 3], 0.45) } else { mx([14.0, 20.0, 18.0], lamp, p) });
            emit(lx, ly, if focus { 6.0 } else { 3.0 }, lamp, if focus { 0.9 } else { 0.35 });
        }
    }
    // The laser: from under the rostrum (the spider's head), along the
    // ground through what it hits.
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
        // Spitters (a ranged attack): a sickly green spine along the back,
        // and a gob now and then.
        if an.ranged {
            for i in 0..long {
                let u = (u0 + i) * f;
                if i % 2 == 0 {
                    pen.block(u as f32, (high + 1) as f32, lin(0.45, 0.75, 0.2));
                }
            }
            if ((tt * 0.25 + an.id as f32 * 0.37).fract()) < 0.04 {
                crate::fx::spark(x, y + 2.0 + high as f64, 2, [140.0, 220.0, 60.0], 14.0);
            }
        }
    }
}
