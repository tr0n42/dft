//! Light and small effects, as the swarm mockup does them.
//!
//! **One lighting engine** (the mockup's `emit` / `lightPass`): anything
//! lit or hot says so each frame (`emit`: where, how far its light reaches,
//! its colour, how strong); every light is added into a light map on the
//! 1 m grid, falling off with distance, then laid over the scene in stepped
//! bands (no smooth gradients): strongly on solid blocks, which it lights,
//! and faintly in open space, as a dithered haze. Coarser cells when zoomed
//! far out.
//!
//! **Effects** (`spark`, `smoke`, `dust`): small loose blocks with a life of
//! their own, falling toward the body (sparks), drifting up and spreading
//! (smoke, dust). Drawn in whole blocks like everything else.
//!
//! Anything may add lights and effects from anywhere during a frame (they go
//! into a queue); this module draws them last, over the ground and machines.

use crate::view::View;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;
use std::sync::Mutex;

pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        // (Lights added during a frame are drawn by the next one at the latest.)
        app.add_systems(PostUpdate, (draw, flash_and_shake));
    }
}

/// Light on solid blocks and in open space (the mockup's LIGHT_SOLID and
/// LIGHT_AIR), and the bands it is stepped in.
const LIGHT_SOLID: f32 = 0.62;
const LIGHT_AIR: f32 = 0.16;
const BANDS: f32 = 6.0;
/// Light-map cells drawn at most (coarser cells when zoomed out).
const MAX_CELLS: f64 = 70_000.0;
/// Effects alive at once, at most.
const MAX_FX: usize = 3000;

/// A light this frame: where (m), how far it reaches (m), and its colour
/// times strength (0-1 a channel at full).
#[derive(Clone, Copy)]
struct Emit {
    x: f64,
    y: f64,
    r: f64,
    c: [f32; 3],
}

#[derive(Clone, Copy)]
enum Kind {
    Spark,
    Smoke,
    Dust,
}

#[derive(Clone, Copy)]
struct Bit {
    kind: Kind,
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    age: f32,
    life: f32,
    size: f32,
    c: [f32; 3],
}

struct Queue {
    lights: Vec<Emit>,
    new: Vec<Bit>,
    rng: u64,
    /// A white flash over the whole screen, and the screen shaking (pixels),
    /// each dying away.
    flash: f32,
    shake: f32,
}

static QUEUE: Mutex<Queue> = Mutex::new(Queue { lights: Vec::new(), new: Vec::new(), rng: 0x5EED, flash: 0.0, shake: 0.0 });

/// A white flash over the whole screen (0 to 1), dying away.
pub fn flash(f: f32) {
    let mut q = QUEUE.lock().unwrap();
    q.flash = q.flash.max(f);
}

/// The screen shakes (up to `s` pixels), dying away.
pub fn shake(s: f32) {
    let mut q = QUEUE.lock().unwrap();
    q.shake = q.shake.max(s);
}

fn rand(q: &mut Queue) -> f64 {
    q.rng = q.rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (q.rng >> 11) as f64 / (1u64 << 53) as f64
}

/// A light at (x, y) this frame (world metres): reach `r` m, colour `c`
/// (sRGB 0-255), strength `i` (the mockup's emit).
pub fn emit(x: f64, y: f64, r: f64, c: [f32; 3], i: f32) {
    if i <= 0.01 || r <= 0.0 {
        return;
    }
    let mut q = QUEUE.lock().unwrap();
    if q.lights.len() < 20_000 {
        q.lights.push(Emit { x, y, r, c: [c[0] / 255.0 * i, c[1] / 255.0 * i, c[2] / 255.0 * i] });
    }
}

/// Sparks: `n` hot flecks thrown out from (x, y) at up to `speed` m/s
/// (the mockup's spark), `up` the way up there.
pub fn spark(x: f64, y: f64, n: usize, c: [f32; 3], speed: f64) {
    let mut q = QUEUE.lock().unwrap();
    for _ in 0..n {
        let a = rand(&mut q) * std::f64::consts::TAU;
        let v = speed * (0.3 + rand(&mut q));
        let life = 0.4 + rand(&mut q) as f32 * 0.7;
        let (ux, uy) = up_of(x, y);
        q.new.push(Bit { kind: Kind::Spark, x, y, vx: a.cos() * v + ux * 4.0, vy: a.sin() * v + uy * 4.0, age: 0.0, life, size: 1.0, c });
    }
}

/// A puff of smoke rising from (x, y) (the mockup's smokeC).
pub fn smoke(x: f64, y: f64, c: [f32; 3]) {
    let mut q = QUEUE.lock().unwrap();
    let (ux, uy) = up_of(x, y);
    let (tx, ty) = (-uy, ux);
    let side = (rand(&mut q) - 0.5) * 3.0;
    let rise = 2.0 + rand(&mut q) * 3.0;
    let life = 2.5 + rand(&mut q) as f32 * 2.0;
    let off = rand(&mut q) - 0.5;
    q.new.push(Bit { kind: Kind::Smoke, x: x + tx * off, y: y + ty * off, vx: tx * side + ux * rise, vy: ty * side + uy * rise, age: 0.0, life, size: 1.0, c });
}

/// Dust thrown up from (x, y): a pillar of it from a plume or a drill hole.
pub fn dust(x: f64, y: f64, c: [f32; 3], rise: f64, spread: f64, life: f32) {
    let mut q = QUEUE.lock().unwrap();
    let (ux, uy) = up_of(x, y);
    let (tx, ty) = (-uy, ux);
    let side = (rand(&mut q) - 0.5) * spread;
    let up = rise * (0.6 + 0.8 * rand(&mut q));
    let life = life * (0.7 + 0.6 * rand(&mut q) as f32);
    q.new.push(Bit { kind: Kind::Dust, x, y, vx: tx * side + ux * up, vy: ty * side + uy * up, age: 0.0, life, size: 1.0, c });
}

/// Away from the body's centre at (x, y).
fn up_of(x: f64, y: f64) -> (f64, f64) {
    let r = (x * x + y * y).sqrt().max(1.0);
    (x / r, y / r)
}

#[derive(Component)]
struct Flash;

/// The flash over the screen and the camera's shake, dying away (the
/// proving ground's: flash -3 a second, shake -18 pixels a second).
fn flash_and_shake(mut commands: Commands, time: Res<Time<Real>>, view: Res<View>, mut cams: Query<&mut Transform, (With<Camera2d>, Without<Flash>)>, mut flashes: Query<(&mut Sprite, &mut Transform, &mut Visibility), With<Flash>>) {
    let dt = time.delta_secs().min(0.05);
    let (f, s, j) = {
        let mut q = QUEUE.lock().unwrap();
        q.flash = (q.flash - dt * 3.0).max(0.0);
        q.shake = (q.shake - dt * 18.0).max(0.0);
        let (a, b) = (rand(&mut q) as f32 - 0.5, rand(&mut q) as f32 - 0.5);
        (q.flash, q.shake, (a, b))
    };
    for mut tf in cams.iter_mut() {
        tf.translation.x = j.0 * s;
        tf.translation.y = j.1 * s;
    }
    match flashes.iter_mut().next() {
        Some((mut sp, mut tf, mut vis)) => {
            sp.color = Color::srgba(1.0, 0.97, 0.9, f * 0.55);
            sp.custom_size = Some(Vec2::new(view.w as f32 * 1.2, view.h as f32 * 1.2));
            tf.translation = Vec3::new(j.0 * s, j.1 * s, 30.0);
            let v = if f > 0.001 { Visibility::Visible } else { Visibility::Hidden };
            if *vis != v {
                *vis = v;
            }
        }
        None => {
            commands.spawn((Sprite { color: Color::srgba(1.0, 1.0, 1.0, 0.0), custom_size: Some(Vec2::ONE), ..default() }, Transform::from_xyz(0.0, 0.0, 30.0), Visibility::Hidden, Flash));
        }
    }
}

#[derive(Resource)]
struct Meshes {
    bits: Handle<Mesh>,
    light: Handle<Mesh>,
}

fn empty() -> Mesh {
    let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD);
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3]);
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; 3]);
    m
}

fn lin(c: [f32; 3], a: f32) -> [f32; 4] {
    let k = Color::srgba(c[0].clamp(0.0, 1.0), c[1].clamp(0.0, 1.0), c[2].clamp(0.0, 1.0), a.clamp(0.0, 1.0)).to_linear();
    [k.red, k.green, k.blue, k.alpha]
}

#[allow(clippy::too_many_arguments)]
fn draw(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    handles: Option<Res<Meshes>>,
    view: Res<View>,
    time: Res<Time<Real>>,
    body: Res<crate::Body>,
    run: Option<Res<crate::Run>>,
    mut bits: Local<Vec<Bit>>,
    mut plumes: Local<Vec<(f64, f64, f32, f32)>>,
    mut perf: ResMut<crate::log::Perf>,
) {
    let _timed = crate::log::Timed::new(&mut perf, crate::log::P_LIGHTS);
    let Some(h) = handles else {
        let bits_h = meshes.add(empty());
        let light_h = meshes.add(empty());
        // Effects over the machines; the light over everything.
        commands.spawn((Mesh2d(bits_h.clone()), MeshMaterial2d(materials.add(ColorMaterial::default())), Transform::from_xyz(0.0, 0.0, 16.0), NoFrustumCulling));
        commands.spawn((Mesh2d(light_h.clone()), MeshMaterial2d(materials.add(ColorMaterial::default())), Transform::from_xyz(0.0, 0.0, 17.0), NoFrustumCulling));
        commands.insert_resource(Meshes { bits: bits_h, light: light_h });
        return;
    };
    let dt = time.delta_secs().min(0.05);
    let (lights, new) = {
        let mut q = QUEUE.lock().unwrap();
        (std::mem::take(&mut q.lights), std::mem::take(&mut q.new))
    };
    let (l, b, r, t) = view.bounds();
    // A block of `m` metres at world (x, y) (its lower-left corner), on whole
    // pixels.
    let quad = |pos: &mut Vec<[f32; 3]>, col: &mut Vec<[f32; 4]>, x: f64, y: f64, m: f64, c: [f32; 4]| {
        let a = view.to_screen(x, y);
        let z = view.to_screen(x + m, y + m);
        let (x0, y0, x1, y1) = (a.x.round(), a.y.round(), z.x.round().max(a.x.round() + 1.0), z.y.round().max(a.y.round() + 1.0));
        pos.extend_from_slice(&[[x0, y0, 0.0], [x1, y0, 0.0], [x1, y1, 0.0], [x0, y0, 0.0], [x1, y1, 0.0], [x0, y1, 0.0]]);
        col.extend_from_slice(&[c; 6]);
    };

    // ---- Plumes: a column of dust over the mouth while the broken rock
    // blasts out, glowing at the vent (the proving ground's plume) ----
    if let Some(run) = &run {
        for (x, y, pw) in std::mem::take(&mut run.0.out.lock().plumes) {
            let dur = 0.6 + 0.35 * (pw * 3.0).max(1.0).log10();
            plumes.push((x, y, pw, dur + 0.6));
        }
    }
    plumes.retain_mut(|p| {
        p.3 -= dt;
        if p.3 <= 0.0 {
            return false;
        }
        let mut q = QUEUE.lock().unwrap();
        if rand(&mut q) < 0.9 {
            drop(q);
            dust(p.0, p.1 + 2.0, [110.0, 100.0, 88.0], (14.0 + 14.0) * 0.75 * (p.2 as f64).sqrt(), 6.0, 6.0);
        } else {
            drop(q);
        }
        emit(p.0, p.1, 6.0, [255.0, 170.0, 80.0], 0.8);
        true
    });
    let new = {
        let mut q = QUEUE.lock().unwrap();
        let mut n = new;
        n.append(&mut q.new);
        n
    };

    // ---- Effects ----
    for nb in new {
        if bits.len() < MAX_FX {
            bits.push(nb);
        }
    }
    let mut pos = Vec::new();
    let mut col = Vec::new();
    bits.retain_mut(|p| {
        p.age += dt;
        if p.age >= p.life {
            return false;
        }
        let (ux, uy) = up_of(p.x, p.y);
        let d = dt as f64;
        match p.kind {
            Kind::Spark => {
                p.vx -= ux * 30.0 * d;
                p.vy -= uy * 30.0 * d;
            }
            Kind::Smoke | Kind::Dust => {
                // Rising slows; it spreads as it goes.
                p.vx *= 1.0 - 0.35 * d;
                p.vy *= 1.0 - 0.35 * d;
                p.size = (p.size + dt * 1.2).min(5.0);
            }
        }
        p.x += p.vx * d;
        p.y += p.vy * d;
        true
    });
    for p in bits.iter() {
        if p.x < l - 10.0 || p.x > r + 10.0 || p.y < b - 10.0 || p.y > t + 10.0 {
            continue;
        }
        let f = 1.0 - p.age / p.life;
        match p.kind {
            Kind::Spark => quad(&mut pos, &mut col, p.x.floor(), p.y.floor(), 1.0, lin([p.c[0] / 255.0, p.c[1] / 255.0, p.c[2] / 255.0], f)),
            Kind::Smoke | Kind::Dust => {
                // Blocks of its own size, on the grid of that size.
                let s = p.size.round().max(1.0) as f64;
                let a = if matches!(p.kind, Kind::Smoke) { 0.5 } else { 0.4 } * f;
                quad(&mut pos, &mut col, (p.x / s).floor() * s, (p.y / s).floor() * s, s, lin([p.c[0] / 255.0, p.c[1] / 255.0, p.c[2] / 255.0], a));
            }
        }
    }
    if pos.is_empty() {
        pos.extend_from_slice(&[[0.0; 3]; 3]);
        col.extend_from_slice(&[[0.0; 4]; 3]);
    }
    if let Some(m) = meshes.get_mut(&h.bits) {
        let m = m.into_inner();
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    }

    // ---- Melt left by a blast glows, flickering, until it cools ----
    let mut lights = lights;
    if let Some(run) = &run {
        let m = run.0.mirror.read();
        let tt = time.elapsed_secs_f64();
        let mut n = 0;
        m.looks_in(l.floor() as i64 - 4, r.ceil() as i64 + 4, b.floor() as i64 - 4, t.ceil() as i64 + 4, |c, k| {
            if k == dft_sim::world::Look::MELT && n < 4000 {
                n += 1;
                let fl = 0.8 + 0.2 * ((tt * 6.0 + c.0 as f64 * 0.7 + c.1 as f64 * 1.3).sin() as f32);
                let i = 0.45 * fl;
                lights.push(Emit { x: c.0 as f64 + 0.5, y: c.1 as f64 + 0.5, r: 4.0, c: [255.0 / 255.0 * i, 150.0 / 255.0 * i, 60.0 / 255.0 * i] });
            }
        });
    }

    // ---- The light pass ----
    let mut pos = Vec::new();
    let mut col = Vec::new();
    let lights: Vec<Emit> = lights.into_iter().filter(|e| e.x + e.r >= l && e.x - e.r <= r && e.y + e.r >= b && e.y - e.r <= t).collect();
    if !lights.is_empty() {
        // The visible cells, coarser when zoomed far out.
        let (vx0, vx1, vy0, vy1) = (l.floor() - 1.0, r.ceil() + 1.0, b.floor() - 1.0, t.ceil() + 1.0);
        let g = ((vx1 - vx0) * (vy1 - vy0) / MAX_CELLS).sqrt().ceil().max(1.0);
        let (gx0, gy0) = ((vx0 / g).floor() as i64, (vy0 / g).floor() as i64);
        let gw = ((vx1 / g).ceil() as i64 - gx0 + 1) as usize;
        let gh = ((vy1 / g).ceil() as i64 - gy0 + 1) as usize;
        let mut buf = vec![[0.0f32; 3]; gw * gh];
        let mut lit: Vec<usize> = Vec::new();
        for e in &lights {
            let i0 = (((e.x - e.r) / g).floor() as i64 - gx0).max(0);
            let i1 = (((e.x + e.r) / g).floor() as i64 - gx0).min(gw as i64 - 1);
            let j0 = (((e.y - e.r) / g).floor() as i64 - gy0).max(0);
            let j1 = (((e.y + e.r) / g).floor() as i64 - gy0).min(gh as i64 - 1);
            for j in j0..=j1 {
                for i in i0..=i1 {
                    let dx = ((i + gx0) as f64 + 0.5) * g - e.x;
                    let dy = ((j + gy0) as f64 + 0.5) * g - e.y;
                    let d = (dx * dx + dy * dy).sqrt() / e.r;
                    if d >= 1.0 {
                        continue;
                    }
                    let f = ((1.0 - d) * (1.0 - d)) as f32;
                    let o = j as usize * gw + i as usize;
                    if buf[o] == [0.0; 3] {
                        lit.push(o);
                    }
                    buf[o][0] += e.c[0] * f;
                    buf[o][1] += e.c[1] * f;
                    buf[o][2] += e.c[2] * f;
                }
            }
        }
        let q = |v: f32| (v * BANDS).floor().min(BANDS * 1.5) / BANDS;
        let p = &body.0;
        let mirror = run.as_ref().map(|r| r.0.mirror.read());
        let solid_at = |x: i64, y: i64| -> bool {
            let m = mirror.as_ref().and_then(|m| m.get(x, y)).unwrap_or_else(|| p.material(x, y));
            m != dft_sim::materials::VACUUM
        };
        lit.sort_unstable();
        for o in lit {
            let c = buf[o];
            let m = c[0].max(c[1]).max(c[2]);
            if m < 1.0 / BANDS {
                continue;
            }
            let (i, j) = ((o % gw) as i64, (o / gw) as i64);
            let (wx, wy) = (((i + gx0) as f64) * g, ((j + gy0) as f64) * g);
            let half = (g / 2.0).floor();
            let solid = solid_at((wx + half) as i64, (wy + half) as i64);
            // Thin light in open space is dithered: every other block.
            if !solid && m < 3.0 / BANDS && (i + j) & 1 == 1 {
                continue;
            }
            let k = if solid { LIGHT_SOLID } else { LIGHT_AIR };
            // Added over what is there: its own colour, as strong as it is.
            let cc = [q(c[0]) * k, q(c[1]) * k, q(c[2]) * k];
            let a = cc[0].max(cc[1]).max(cc[2]).min(1.0);
            if a <= 0.0 {
                continue;
            }
            quad(&mut pos, &mut col, wx, wy, g, lin([cc[0] / a, cc[1] / a, cc[2] / a], a));
        }
    }
    if pos.is_empty() {
        pos.extend_from_slice(&[[0.0; 3]; 3]);
        col.extend_from_slice(&[[0.0; 4]; 3]);
    }
    if let Some(m) = meshes.get_mut(&h.light) {
        let m = m.into_inner();
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    }
}
