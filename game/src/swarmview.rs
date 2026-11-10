//! The mother swarm as seen: a living mass of drones high over the ground.
//!
//! Its shape: along its arc the band's middle line wanders and its thickness
//! swells and thins with slow drifting waves, so it is never a smooth ribbon,
//! while its thickness on average (its volume) stays what the stockpile it
//! holds calls for. Its ends are rounded and fat, not pinched: the swarm puts
//! its mass into the edges it is growing along.
//!
//! Its depth: three layers of drones. The far layer is small, dim and slow;
//! the near layer large, brighter and quicker; between them (in the middle
//! layer's depth) sit the storage drones and anything that has come up into
//! the swarm, so they look wrapped inside it. Currents of drones zip through
//! it in swirling streams.
//!
//! `Shape` is shared: the storage drones and the drawing of cutters and
//! haulers use it to know where the swarm is (to put what's inside behind).

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use bevy::camera::visibility::NoFrustumCulling;
use std::f64::consts::{PI, TAU};

pub struct SwarmViewPlugin;

impl Plugin for SwarmViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SwarmNow>().add_systems(Startup, setup).add_systems(PreUpdate, ease).add_systems(Update, draw);
    }
}

/// The stockpile is held in the mother swarm at this many kg per square
/// metre of it: the more it holds, the thicker it is.
pub const STORE_KG_PER_M2: f64 = 50.0;

/// Depths (z) of the swarm's layers, and of what is inside it.
pub const Z_FAR: f32 = 9.0;
pub const Z_INSIDE: f32 = 9.3;
pub const Z_MID: f32 = 9.45;
pub const Z_NEAR: f32 = 9.8;

/// Where the swarm is, at one moment.
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    pub centre: f64,
    pub half: f64,
    /// Its middle's distance from the body's centre.
    pub alt: f64,
    /// Half its thickness on average (m).
    pub h: f64,
    /// Seconds (real), for its slow changes.
    pub t: f64,
    pub whole: bool,
}

/// Smooth wandering in [-1, 1]: a few sines at unrelated frequencies.
fn wave(x: f64, p: f64) -> f64 {
    ((x + p).sin() + 0.6 * (2.13 * x + 1.7 * p + 0.4).sin() + 0.35 * (4.71 * x - 0.9 * p + 2.1).sin()) / 1.95
}

impl Shape {
    pub fn new(mother: (f64, f64, f64), held: f64, max_radius: f64, t: f64) -> Shape {
        let (centre, half, alt) = mother;
        let length = (2.0 * half * alt).max(1.0);
        let room = ((alt - max_radius) * 0.6).max(20.0);
        let thick = 14.0 + (held / (length * STORE_KG_PER_M2)).min(room);
        Shape { centre, half, alt, h: thick * 0.5, t, whole: half >= PI - 1e-6 }
    }

    pub fn exists(&self) -> bool {
        self.half > 0.0
    }

    /// Angle `a` brought within the swarm's arc (it may be given in any turn).
    fn local(&self, a: f64) -> f64 {
        let mut d = a - self.centre;
        while d > PI {
            d -= TAU;
        }
        while d < -PI {
            d += TAU;
        }
        d
    }

    /// At angle `a`: the band's middle (distance from the body's centre) and
    /// its half thickness there (0 off the arc).
    pub fn at(&self, a: f64) -> (f64, f64) {
        let d = self.local(a);
        if !self.whole && d.abs() > self.half {
            return (self.alt, 0.0);
        }
        // Arc distance from its middle, in units of its own scale.
        let s = d * self.alt;
        let scale = (self.h * 6.0).max(60.0);
        let x = s / scale;
        let tt = self.t;
        // Waves along it, drifting slowly (wrapping cleanly round a whole ring).
        let (wx, wy) = if self.whole { let k = (TAU * self.alt / scale / 6.0).round().max(1.0); ((d * k).cos(), (d * k).sin()) } else { (x, 0.0) };
        let mid = self.alt + 0.45 * self.h * wave(wx * 1.3 + wy * 0.7, tt * 0.05);
        let mut half = self.h * (1.0 + 0.55 * wave(wx * 0.9 - wy * 0.5 + 3.0, -tt * 0.04));
        if !self.whole {
            // The ends: rounded, and fat (the swarm piles into its growing edges).
            let to_end = (self.half - d.abs()) * self.alt;
            let cap = self.h * 1.6;
            if to_end < cap {
                let q = to_end / cap;
                half *= (1.0 - (1.0 - q) * (1.0 - q)).max(0.0).sqrt();
            }
            half *= 1.0 + 0.6 * (-to_end / (self.h * 5.0)).exp();
        }
        (mid, half.max(0.0))
    }

    /// Whether world point (x, y) is inside the swarm (with a margin, m).
    pub fn inside(&self, x: f64, y: f64, margin: f64) -> bool {
        if !self.exists() {
            return false;
        }
        let r = (x * x + y * y).sqrt();
        if (r - self.alt).abs() > self.h * 3.0 + margin {
            return false;
        }
        let (mid, half) = self.at(y.atan2(x));
        half > 0.0 && (r - mid).abs() <= half + margin
    }

    /// The arc's angle ranges in view, as drawn (on both sides of the join
    /// once it wraps all the way round).
    pub fn ranges(&self, v: &crate::view::View) -> Vec<(f64, f64)> {
        let (vx, vy) = (v.x, v.y);
        let span = (v.w.max(v.h) * v.mpp).max(1.0);
        let dist = (vx * vx + vy * vy).sqrt();
        let mut out = Vec::new();
        if dist < span || dist - span < 1.0 {
            out.push((self.centre - self.half, self.centre + self.half));
        } else {
            let va = vy.atan2(vx);
            let w = (span / (dist - span).max(1.0)).min(PI);
            for o in [-TAU, 0.0, TAU] {
                let c = va + o;
                let (a0, a1) = ((c - w).max(self.centre - self.half), (c + w).min(self.centre + self.half));
                if a1 > a0 {
                    out.push((a0, a1));
                }
            }
        }
        out
    }
}

#[derive(Resource)]
struct Layers {
    far: Handle<Mesh>,
    mid: Handle<Mesh>,
    near: Handle<Mesh>,
    currents: Handle<Mesh>,
}

fn empty_mesh() -> Mesh {
    let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD);
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3]);
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; 3]);
    m
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
    let mut make = |z: f32| {
        let h = meshes.add(empty_mesh());
        commands.spawn((Mesh2d(h.clone()), MeshMaterial2d(materials.add(ColorMaterial::default())), Transform::from_xyz(0.0, 0.0, z), NoFrustumCulling));
        h
    };
    let far = make(Z_FAR);
    let mid = make(Z_MID);
    let near = make(Z_NEAR);
    let currents = make(Z_NEAR + 0.05);
    commands.insert_resource(Layers { far, mid, near, currents });
}

fn take(mesh: &mut Mesh) -> (Vec<[f32; 3]>, Vec<[f32; 4]>) {
    let p = match mesh.remove_attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(mut v)) => {
            v.clear();
            v
        }
        _ => Vec::new(),
    };
    let c = match mesh.remove_attribute(Mesh::ATTRIBUTE_COLOR) {
        Some(VertexAttributeValues::Float32x4(mut v)) => {
            v.clear();
            v
        }
        _ => Vec::new(),
    };
    (p, c)
}

fn put(mesh: &mut Mesh, mut p: Vec<[f32; 3]>, mut c: Vec<[f32; 4]>) {
    if p.is_empty() {
        p.extend_from_slice(&[[0.0; 3]; 3]);
        c.extend_from_slice(&[[0.0; 4]; 3]);
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, p);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, c);
}

fn square(pos: &mut Vec<[f32; 3]>, col: &mut Vec<[f32; 4]>, x: f32, y: f32, h: f32, c: [f32; 4]) {
    pos.extend_from_slice(&[[x - h, y - h, 0.0], [x + h, y - h, 0.0], [x + h, y + h, 0.0], [x - h, y - h, 0.0], [x + h, y + h, 0.0], [x - h, y + h, 0.0]]);
    col.extend_from_slice(&[c; 6]);
}

fn lin(r: f32, g: f32, b: f32, a: f32) -> [f32; 4] {
    let c = Color::srgba(r, g, b, a).to_linear();
    [c.red, c.green, c.blue, c.alpha]
}

/// The swarm as drawn this frame (for every drawing system). It follows
/// the simulation's swarm slowly: grown longer, thicker or higher, it takes
/// its time getting there rather than jumping.
#[derive(Resource)]
pub struct SwarmNow(pub Shape);

impl Default for SwarmNow {
    fn default() -> Self {
        SwarmNow(Shape { centre: 0.0, half: 0.0, alt: 0.0, h: 0.0, t: 0.0, whole: false })
    }
}

/// Seconds (time constants) for the drawn swarm to catch up: its length,
/// its height and its thickness.
const EASE_LENGTH_S: f64 = 10.0;
const EASE_ALT_S: f64 = 12.0;
const EASE_THICK_S: f64 = 25.0;

fn ease(mut now: ResMut<SwarmNow>, run: Option<Res<crate::Run>>, body: Res<crate::Body>, time: Res<Time<Real>>, mut last: Local<f64>) {
    let Some(run) = run else { return };
    let real = time.elapsed_secs_f64();
    let dt = (real - *last).clamp(0.0, 0.5);
    *last = real;
    let (mother, held) = {
        let out = run.0.out.lock();
        (out.mother, out.stats.matter[0])
    };
    let target = Shape::new(mother, held, body.0.max_radius, real);
    let cur = &mut now.0;
    // A new body, or the first sight of the swarm: there at once.
    if !cur.exists() || !target.exists() || (cur.centre - target.centre).abs() > 1e-9 {
        *cur = target;
        return;
    }
    let k = |tau: f64| 1.0 - (-dt / tau).exp();
    cur.half += (target.half - cur.half) * k(EASE_LENGTH_S);
    cur.alt += (target.alt - cur.alt) * k(EASE_ALT_S);
    cur.h += (target.h - cur.h) * k(EASE_THICK_S);
    cur.t = real;
    cur.whole = cur.half >= PI - 1e-3;
    if cur.whole {
        cur.half = PI;
    }
}

fn draw(layers: Res<Layers>, mut meshes: ResMut<Assets<Mesh>>, view: Res<crate::view::View>, swarm: Res<SwarmNow>, lights: Res<crate::lights::Lights>, time: Res<Time<Real>>) {
    let real = time.elapsed_secs_f64();
    let sh = swarm.0;
    let v = *view;
    let (l, b, r, t) = v.bounds();
    let (l, b, r, t) = (l - 4.0, b - 4.0, r + 4.0, t + 4.0);
    let mut out: [(Vec<[f32; 3]>, Vec<[f32; 4]>); 4] = std::array::from_fn(|_| (Vec::new(), Vec::new()));
    for (i, hdl) in [&layers.far, &layers.mid, &layers.near, &layers.currents].into_iter().enumerate() {
        if let Some(mut m) = meshes.get_mut(hdl) {
            out[i] = take(&mut m);
        }
    }
    // The swarm takes on a hint of its lights' colour (the currents more).
    let lc = lights.rgb();
    let hue = |base: [f32; 3], k: f32| [base[0] * (1.0 - k) + lc[0] * 0.55 * k, base[1] * (1.0 - k) + lc[1] * 0.55 * k, base[2] * (1.0 - k) + lc[2] * 0.55 * k];
    let speck_c = hue([0.36, 0.42, 0.39], 0.28);
    let current_c = hue([0.55, 0.66, 0.6], 0.45);
    if sh.exists() {
        let speck = ((1.6 / v.mpp) as f32).max(1.5);
        // Specks per step across the band, by how thick it is.
        let across = ((sh.h * 2.0 / 14.0).sqrt() * 3.0).round().clamp(3.0, 12.0) as i64;
        // The three layers: (how many of the specks, size, brightness,
        // opacity, drift speed, salt).
        let layer = [(0.30, 0.65f32, 0.55f32, 0.55f32, 0.15, 0x3A_01u64), (0.38, 1.0, 0.8, 0.8, 0.3, 0x3A_02), (0.32, 1.35, 1.05, 0.9, 0.5, 0x3A_03)];
        for (a0, a1) in sh.ranges(&v) {
            let step = (4.0 * v.mpp / sh.alt).max((a1 - a0) / 6000.0);
            let n = ((a1 - a0) / step) as i64 + 1;
            let k0 = (a0 / step).floor() as i64;
            for i in 0..=n {
                let k = k0 + i;
                let a = k as f64 * step;
                if a < a0 || a > a1 {
                    continue;
                }
                let (mid, half) = sh.at(a);
                if half <= 0.0 {
                    continue;
                }
                for j in 0..across {
                    let h = dft_sim::noise::hash(k as u64, j, 0, 0x30_7E);
                    let u0 = dft_sim::noise::unit(h);
                    let u1 = crate::rnd::unit(h, 0x77);
                    let u2 = crate::rnd::unit(h, 0x1234);
                    // Its layer.
                    let li = if u0 < layer[0].0 { 0 } else if u0 < layer[0].0 + layer[1].0 { 1 } else { 2 };
                    let (_, size, bright, alpha, speed, _) = layer[li];
                    // Each speck drifts slowly back and forth.
                    let drift = ((real * (speed + u2 * speed) + u1 * 6.28).sin()) * 0.5;
                    // Spread through the thickness, thicker in the middle.
                    let w = (u1 - 0.5) * 2.0;
                    let w = w * (0.55 + 0.45 * w.abs());
                    let rr = mid + (w + drift * 0.25) * half;
                    let aa = a + drift * step;
                    let (x, y) = (aa.cos() * rr, aa.sin() * rr);
                    if x < l || x > r || y < b || y > t {
                        continue;
                    }
                    let s = v.to_screen(x, y);
                    let shade = (0.55 + 0.45 * u2 as f32) * bright;
                    let c = lin(speck_c[0] * shade, speck_c[1] * shade, speck_c[2] * shade, alpha);
                    let (p, cc) = &mut out[li];
                    square(p, cc, s.x, s.y, speck * 0.5 * size, c);
                }
            }
            // Currents: drones zipping through in streams, each living a few
            // seconds (fading in and out), swirling across the band as they
            // go. Seeded per stretch of arc, so the same ones are seen from
            // anywhere.
            let seg = 24.0f64;
            let seg_a = seg / sh.alt;
            let s0 = (a0 / seg_a).floor() as i64 - 8;
            let s1 = (a1 / seg_a).ceil() as i64 + 8;
            let life = 7.0;
            let dot = ((1.1 / v.mpp) as f32).max(1.2);
            for sk in s0..=s1 {
                for j in 0..3u64 {
                    let h = dft_sim::noise::hash(sk as u64, j as i64, 5, 0xC0_44);
                    let (u1, u2, u3) = (dft_sim::noise::unit(h), crate::rnd::unit(h, 0x9E), crate::rnd::unit(h, 0x51));
                    let off = u1 * life;
                    let cycle = ((real + off) / life).floor();
                    let tau = (real + off) - cycle * life;
                    // A new course each life.
                    let hc = dft_sim::noise::hash(h, cycle as i64, 0, 0xC0_45);
                    let (c1, c2) = (dft_sim::noise::unit(hc), crate::rnd::unit(hc, 0x33));
                    let dir = if c1 < 0.5 { 1.0 } else { -1.0 };
                    let speed = 12.0 + 30.0 * u2;
                    let fade = (tau / life * PI).sin() as f32;
                    // A short trail behind it.
                    for back in 0..4 {
                        let tb = tau - back as f64 * 0.08;
                        if tb < 0.0 {
                            break;
                        }
                        let along = sk as f64 * seg + dir * speed * tb;
                        let a = along / sh.alt;
                        let (mid, half) = sh.at(a);
                        if half <= 0.0 {
                            break;
                        }
                        // Swirling: across the band and back, round in loops.
                        let swirl = (tb * (1.2 + 1.5 * u3) + c2 * 6.28).sin() * 0.8 + 0.15 * (tb * 4.0 + u1 * 6.28).sin();
                        let rr = mid + swirl * half;
                        let loop_r = 3.0 + 4.0 * u3;
                        let (lx, ly) = ((tb * 2.5 + c2 * 6.28).cos() * loop_r, (tb * 2.5 + c2 * 6.28).sin() * loop_r);
                        let (x, y) = (a.cos() * rr + lx * 0.4, a.sin() * rr + ly * 0.4);
                        if x < l || x > r || y < b || y > t {
                            continue;
                        }
                        let p = v.to_screen(x, y);
                        let k = 1.0 - back as f32 * 0.24;
                        let c = lin(current_c[0], current_c[1], current_c[2], 0.85 * fade * k);
                        let (pp, cc) = &mut out[3];
                        square(pp, cc, p.x, p.y, dot * 0.5 * (0.6 + 0.4 * k), c);
                    }
                }
            }
        }
    }
    for (i, hdl) in [&layers.far, &layers.mid, &layers.near, &layers.currents].into_iter().enumerate() {
        if let Some(mut m) = meshes.get_mut(hdl) {
            let (p, c) = std::mem::take(&mut out[i]);
            put(&mut m, p, c);
        }
    }
}
