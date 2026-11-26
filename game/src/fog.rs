//! Fog of war, drawn as one overlay over the ground (under the drones).
//!
//! Ground is charted where drones and landing points have been: a coarse
//! grid of 12 m cells, kept at several sizes so any zoom finds a grid of a
//! few pixels a cell. Each charted cell remembers when it was last seen.
//!
//!   uncharted   dark; toward charted ground, fading into a sonar chart: a
//!               regular field of faint dots, brighter along rings of equal
//!               depth, with a slow pulse sweeping outward
//!   charted     dimmed: a memory of the ground as last seen
//!   live        full colour, wherever drones are now
//!
//! The edge between charted and uncharted is a faint grey line, breathing. Open sky is
//! never fogged, so the body's outline always shows. F2 turns the fog off and on.
use crate::view::View;
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::tasks::{ComputeTaskPool, ParallelSlice, ParallelSliceMut};
use std::collections::{HashMap, HashSet};

/// A charted cell's side, metres (the finest grid).
pub const CELL: f64 = 12.0;
/// Grids kept, each twice as coarse as the last.
const GRIDS: usize = 26;
/// How far around a drone it sees, in cells (a round reach).
const SENSE: i64 = 3;
/// How far around a landing point, in cells.
const HUB_SENSE: i64 = 5;
/// Far sectors charted a frame (in turn), and the stretch of surface each
/// one charts (about a sector's width, see dft_sim::swarm::SECTOR_M).
const FAR_PER_FRAME: usize = 16;
const FAR_STRETCH_M: f64 = 1100.0;
/// Seconds a cell stays live after a drone was last seen in it, fading out.
const LIVE_S: f64 = 1.5;
/// The overlay is drawn at a pixel of this many screen pixels.
const PX: f64 = 2.0;
/// The grid chosen for drawing has cells at least this many screen pixels.
const MIN_CELL_PX: f64 = 6.0;
/// The share of the swarm looked at each frame (all of it every so many frames).
const SWEEP: usize = 6;

pub struct FogPlugin;

impl Plugin for FogPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, (toggle, chart, draw).chain());
    }
}

/// Cells charted, a bit each, in 64 x 64 blocks (a planet's worth of
/// charted ground in a few tens of megabytes; a map of cells took gigabytes
/// on an Earth-size body).
#[derive(Default, Clone)]
struct Bits {
    blocks: HashMap<(i64, i64), Box<[u64; 64]>>,
    count: usize,
}

impl Bits {
    fn at(c: (i64, i64)) -> ((i64, i64), usize, u64) {
        ((c.0.div_euclid(64), c.1.div_euclid(64)), c.1.rem_euclid(64) as usize, 1u64 << c.0.rem_euclid(64))
    }
    fn contains(&self, c: (i64, i64)) -> bool {
        let (b, row, bit) = Self::at(c);
        self.blocks.get(&b).is_some_and(|w| w[row] & bit != 0)
    }
    /// True if it wasn't charted before.
    fn insert(&mut self, c: (i64, i64)) -> bool {
        let (b, row, bit) = Self::at(c);
        let w = self.blocks.entry(b).or_insert_with(|| Box::new([0u64; 64]));
        if w[row] & bit != 0 {
            return false;
        }
        w[row] |= bit;
        self.count += 1;
        true
    }
    fn cells(&self) -> Vec<(i64, i64)> {
        let mut v = Vec::with_capacity(self.count);
        for (&(bx, by), w) in &self.blocks {
            for (row, bits) in w.iter().enumerate() {
                for i in 0..64 {
                    if bits & (1u64 << i) != 0 {
                        v.push((bx * 64 + i as i64, by * 64 + row as i64));
                    }
                }
            }
        }
        v
    }
}

#[derive(Resource)]
pub struct Fog {
    pub on: bool,
    /// Per grid: the cells charted.
    seen: Vec<Bits>,
    /// Per grid: when cells were last seen (real seconds), only those seen
    /// within LIVE_S (for the glow where drones are now).
    live: Vec<HashMap<(i64, i64), f32>>,
    /// When the live maps were last cleared of the faded.
    pruned: f32,
    /// Where the round-robin over the swarm has got to.
    cursor: usize,
    /// Far sectors' marks charted so far this round (a share a frame).
    far_cursor: usize,
    /// The body the charts are of (a new body starts over).
    body: usize,
    /// The ground's radius by direction, for telling ground from sky.
    surface: Vec<f32>,
    /// The untouched outline, and which of the swarm's height sweeps the
    /// outline was last brought down to.
    base: Vec<f32>,
    heights_seen: u64,
    image: Handle<Image>,
    sprite: Entity,
    /// Charted cells from a save, for the body with this id (applied once
    /// that body is up).
    pub restore: Option<(usize, Vec<(i64, i64)>)>,
}

impl Fog {
    /// Charted cells on the finest grid (for the log and panel).
    pub fn charted(&self) -> usize {
        self.seen.first().map_or(0, |m| m.count)
    }

    /// Charted cells on the finest grid (for saving).
    pub fn charted_cells(&self) -> Vec<(i64, i64)> {
        let mut v: Vec<(i64, i64)> = self.seen.first().map(|m| m.cells()).unwrap_or_default();
        v.sort_unstable();
        v
    }

    /// Whether the world point (x, y) has been charted (fog off: everywhere).
    pub fn charted_at(&self, x: f64, y: f64) -> bool {
        !self.on || self.seen.first().is_some_and(|m| m.contains(((x / CELL).floor() as i64, (y / CELL).floor() as i64)))
    }

    fn mark(&mut self, c: (i64, i64), now: f32) {
        for k in 0..GRIDS {
            let key = (c.0 >> k, c.1 >> k);
            let charted_new = self.seen[k].insert(key);
            // (Only what is seen now glows: a chart restored from a save,
            // marked long ago, doesn't.)
            if now > self.pruned - LIVE_S as f32 {
                let e = self.live[k].entry(key).or_insert(f32::MIN);
                if *e >= now {
                    // Already marked this frame here, so on every coarser grid too.
                    break;
                }
                *e = now;
            } else if !charted_new {
                break;
            }
        }
    }
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, o: Res<crate::Options>) {
    let mut img = Image::new(Extent3d { width: 1, height: 1, depth_or_array_layers: 1 }, TextureDimension::D2, vec![0; 4], TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD);
    img.sampler = ImageSampler::nearest();
    let image = images.add(img);
    let sprite = commands.spawn((Sprite { image: image.clone(), custom_size: Some(Vec2::ONE), ..default() }, Transform::from_xyz(0.0, 0.0, 8.0))).id();
    commands.insert_resource(Fog { on: !o.no_fog, seen: vec![Bits::default(); GRIDS], live: vec![HashMap::new(); GRIDS], pruned: 0.0, cursor: 0, far_cursor: 0, body: 0, surface: Vec::new(), base: Vec::new(), heights_seen: 0, image, sprite, restore: None });
}

fn toggle(keys: Res<ButtonInput<KeyCode>>, mut fog: ResMut<Fog>) {
    if keys.just_pressed(KeyCode::F2) {
        fog.on = !fog.on;
    }
}

/// Chart what the drones and landing points can see (a share of the swarm
/// each frame).
fn chart(mut fog: ResMut<Fog>, fleet: Res<crate::drones::Fleet>, run: Res<crate::Run>, body: Res<crate::Body>, time: Res<Time<Real>>, view: Res<View>) {
    let id = std::sync::Arc::as_ptr(&body.0) as usize;
    if fog.body != id {
        // A new body: forget the charts, and learn its outline.
        fog.body = id;
        fog.seen = vec![Bits::default(); GRIDS];
        fog.live = vec![HashMap::new(); GRIDS];
        fog.cursor = 0;
        let n = 8192;
        let p = &body.0;
        fog.base = (0..=n).map(|k| {
            let a = std::f64::consts::TAU * (k % n) as f64 / n as f64;
            p.surface_radius(a.cos(), a.sin()) as f32
        }).collect();
        fog.surface = fog.base.clone();
        fog.heights_seen = 0;
    }
    // The ground eaten down from above is open sky now: the outline follows
    // the ground's height as the swarm last measured it.
    let heights = run.0.out.lock().heights.clone();
    if let Some((v, h)) = heights {
        if v != fog.heights_seen && !h.is_empty() {
            fog.heights_seen = v;
            let n = fog.base.len() - 1;
            let nh = h.len();
            let surface: Vec<f32> = (0..=n)
                .map(|k| {
                    let t = (k % n) as f64 / n as f64 * nh as f64;
                    let i = t.floor() as usize % nh;
                    let f = (t - t.floor()) as f32;
                    let now = h[i] * (1.0 - f) + h[(i + 1) % nh] * f;
                    fog.base[k].min(now)
                })
                .collect();
            fog.surface = surface;
        }
    }
    let now = time.elapsed_secs_f64() as f32;
    // (Cells seen longer ago than the glow lasts drop out of the live maps.)
    if now - fog.pruned >= 1.0 {
        fog.pruned = now;
        let cut = now - LIVE_S as f32;
        for m in fog.live.iter_mut() {
            m.retain(|_, t| *t >= cut);
        }
    }
    if fog.restore.as_ref().is_some_and(|(b, _)| *b == id) {
        // Charted long ago: remembered, not live.
        let (_, cells) = fog.restore.take().unwrap();
        for c in cells {
            fog.mark(c, -1.0e6);
        }
    }
    // Tunnelers see behind the fog (Benjamin, 25 Nov): round each one's head
    // as it digs, twice a landing point's reach.
    let diggers: Vec<(f64, f64)> = run.0.out.lock().units.iter().filter(|u| u.kind == dft_sim::production::Unit::Tunneler && u.state != 0).map(|u| u.to).collect();
    // Sappers see as far round them as a drone does, down their holes too,
    // so their glow shows (Benjamin, 26 Nov: the sapper's green light).
    let sappers: Vec<(f64, f64)> = run.0.out.lock().units.iter().filter(|u| u.kind == dft_sim::production::Unit::Sapper).map(|u| u.to).collect();
    let (t, hubs) = {
        let out = run.0.out.lock();
        // Flocks every frame; far sectors' marks (a swarm worked as a
        // count, tens of thousands on a big body) a few hundred a frame.
        // (Flocks with cutters: a flock that's gone charts nothing.)
        let v: Vec<(f64, f64)> = out.hubs.iter().filter(|h| h.2 != 3 && h.3 > 0).map(|h| (h.0, h.1)).collect();
        let far: Vec<(f64, f64)> = out.hubs.iter().filter(|h| h.2 == 3).map(|h| (h.0, h.1)).collect();
        let mut stretch = Vec::new();
        if !far.is_empty() {
            let start = fog.far_cursor % far.len();
            let end = (start + FAR_PER_FRAME).min(far.len());
            stretch.extend_from_slice(&far[start..end]);
            fog.far_cursor = if end >= far.len() { 0 } else { end };
        }
        (out.stats.time, (v, stretch))
    };
    let (hubs, stretch) = hubs;
    // A far sector (its swarm worked as a count) charts the whole stretch
    // of surface it eats: a band along it, a few cells deep, under its
    // mark (which sits 20 m over its ground, in its middle).
    for (x, y) in stretch {
        let r = (x * x + y * y).sqrt().max(1.0);
        let a = y.atan2(x);
        let half = 0.5 * FAR_STRETCH_M / r;
        let steps = (FAR_STRETCH_M / CELL).ceil() as i64;
        for i in 0..=steps {
            let b = a - half + 2.0 * half * i as f64 / steps as f64;
            for j in 0..4 {
                let rr = r - 20.0 - CELL * j as f64;
                fog.mark((((b.cos() * rr) / CELL).floor() as i64, ((b.sin() * rr) / CELL).floor() as i64), now);
            }
        }
    }
    let cell = |x: f64, y: f64| ((x / CELL).floor() as i64, (y / CELL).floor() as i64);
    // This frame's share of the swarm, on all cores: the cells drones are in.
    let n = fleet.legs.len();
    let per = n / SWEEP + 1;
    let start = fog.cursor.min(n);
    let end = (start + per).min(n);
    fog.cursor = if end >= n { 0 } else { end };
    // Where each is: near the screen, zoomed in, its whole way as drawn
    // (round through open space, never straight through rock); elsewhere a
    // cheaper place that is never deep in rock either (see
    // `Leg::charted_at`). Benjamin, 26 Nov: drones charted straight lines
    // through the planet, clearing great swathes of fog.
    let paths = &fleet.paths;
    let near = if view.mpp < 2.0 {
        let (l, b, r, t) = view.bounds();
        let m = 0.5 * (r - l).max(t - b);
        Some((l - m, b - m, r + m, t + m))
    } else {
        None
    };
    // (Small chunks: a frame's share is a few thousand drones, and placing
    // one along its way costs about a microsecond; all cores share it.)
    let parts = (&fleet.legs[start..end]).par_chunk_map(ComputeTaskPool::get(), 512, |_, legs| {
        let mut set = HashSet::new();
        let inside = |p: (f64, f64)| near.is_some_and(|(l, b, r, t)| p.0 >= l && p.0 <= r && p.1 >= b && p.1 <= t);
        for leg in legs {
            if leg.state > 2 {
                continue;
            }
            let (x, y) = if inside(leg.from) || inside(leg.to) { paths.at(leg, t) } else { paths.charted(leg, t) };
            set.insert(cell(x, y));
        }
        set
    });
    let mut cells: HashSet<(i64, i64)> = HashSet::new();
    for p in parts {
        cells.extend(p);
    }
    // A round reach, so the charted area's edge follows where drones went.
    let disc = |r: i64| -> Vec<(i64, i64)> {
        let lim = (r as f64 + 0.3).powi(2);
        let mut v = Vec::new();
        for dy in -r..=r {
            for dx in -r..=r {
                if ((dx * dx + dy * dy) as f64) <= lim {
                    v.push((dx, dy));
                }
            }
        }
        v
    };
    let (near, hub_near) = (disc(SENSE), disc(HUB_SENSE));
    cells.extend(sappers.iter().map(|&(x, y)| cell(x, y)));
    // Round a blast while it glows, as far as it reaches.
    for &(x, y, reach, _) in &fleet.blasts {
        let c = cell(x, y);
        for &(dx, dy) in &disc((reach / CELL).ceil() as i64) {
            fog.mark((c.0 + dx, c.1 + dy), now);
        }
    }
    for c in cells {
        for &(dx, dy) in &near {
            fog.mark((c.0 + dx, c.1 + dy), now);
        }
    }
    for (x, y) in hubs {
        let c = cell(x, y);
        for &(dx, dy) in &hub_near {
            fog.mark((c.0 + dx, c.1 + dy), now);
        }
    }
    if !diggers.is_empty() {
        let sight = disc(2 * HUB_SENSE);
        for (x, y) in diggers {
            let c = cell(x, y);
            for &(dx, dy) in &sight {
                fog.mark((c.0 + dx, c.1 + dy), now);
            }
        }
    }
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// The smallest power of two at least v.
fn pow2_at_least(v: f64) -> f64 {
    2f64.powi(v.max(1e-9).log2().ceil() as i32)
}

/// Draw the overlay for this frame.
fn draw(fog: Res<Fog>, view: Res<View>, time: Res<Time<Real>>, mut images: ResMut<Assets<Image>>, mut q: Query<(&mut Sprite, &mut Visibility)>, body: Res<crate::Body>, mut perf: ResMut<crate::log::Perf>) {
    let t0 = std::time::Instant::now();
    let Ok((mut sprite, mut vis)) = q.get_mut(fog.sprite) else { return };
    if !fog.on || fog.surface.is_empty() {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Visible;
    let v = *view;
    let (ow, oh) = ((v.w / PX).ceil().max(1.0) as usize, (v.h / PX).ceil().max(1.0) as usize);
    let now = time.elapsed_secs_f64() as f32;
    // The grid to draw from: cells a few pixels across at least.
    let mut k = 0;
    while k + 1 < GRIDS && CELL * 2f64.powi(k as i32) / v.mpp < MIN_CELL_PX {
        k += 1;
    }
    let cs = CELL * 2f64.powi(k as i32);
    // That grid over the view (and a ring round it), as plain arrays:
    // charted (0 or 1) and live (fading from 1).
    let (l, b, r, t) = v.bounds();
    let (gx0, gy0) = ((l / cs).floor() as i64 - 1, (b / cs).floor() as i64 - 1);
    let (gw, gh) = (((r / cs).floor() as i64 + 2 - gx0) as usize, ((t / cs).floor() as i64 + 2 - gy0) as usize);
    let mut charted = vec![0f32; gw * gh];
    let mut live = vec![0f32; gw * gh];
    let (seen, recent) = (&fog.seen[k], &fog.live[k]);
    for j in 0..gh {
        for i in 0..gw {
            let c = (gx0 + i as i64, gy0 + j as i64);
            if seen.contains(c) {
                charted[j * gw + i] = 1.0;
                if let Some(&s) = recent.get(&c) {
                    live[j * gw + i] = (1.0 - (now - s) / LIVE_S as f32).clamp(0.0, 1.0);
                }
            }
        }
    }
    let sample = |g: &[f32], x: f64, y: f64| -> f32 {
        let (fx, fy) = (x / cs - 0.5 - gx0 as f64, y / cs - 0.5 - gy0 as f64);
        let (i, j) = (fx.floor(), fy.floor());
        let (tx, ty) = ((fx - i) as f32, (fy - j) as f32);
        let (i, j) = (i as isize, j as isize);
        let at = |i: isize, j: isize| if i < 0 || j < 0 || i as usize >= gw || j as usize >= gh { 0.0 } else { g[j as usize * gw + i as usize] };
        // Smoothstep weights: soft, round-looking edges.
        let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
        let top = at(i, j) + (at(i + 1, j) - at(i, j)) * sx;
        let bot = at(i, j + 1) + (at(i + 1, j + 1) - at(i, j + 1)) * sx;
        top + (bot - top) * sy
    };
    // Nearness to charted ground: the same, on a grid 8 times coarser, so
    // the dark lifts gradually over a few hundred pixels around what's known.
    let kp = (k + 3).min(GRIDS - 1);
    let cs2 = CELL * 2f64.powi(kp as i32);
    let (nx0, ny0) = ((l / cs2).floor() as i64 - 1, (b / cs2).floor() as i64 - 1);
    let (nw, nh) = (((r / cs2).floor() as i64 + 2 - nx0) as usize, ((t / cs2).floor() as i64 + 2 - ny0) as usize);
    let mut near = vec![0f32; nw * nh];
    for j in 0..nh {
        for i in 0..nw {
            if fog.seen[kp].contains((nx0 + i as i64, ny0 + j as i64)) {
                near[j * nw + i] = 1.0;
            }
        }
    }
    let nearness = |x: f64, y: f64| -> f32 {
        let (fx, fy) = (x / cs2 - 0.5 - nx0 as f64, y / cs2 - 0.5 - ny0 as f64);
        let (i, j) = (fx.floor(), fy.floor());
        let (tx, ty) = ((fx - i) as f32, (fy - j) as f32);
        let (i, j) = (i as isize, j as isize);
        let at = |i: isize, j: isize| if i < 0 || j < 0 || i as usize >= nw || j as usize >= nh { 0.0 } else { near[j as usize * nw + i as usize] };
        let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
        let top = at(i, j) + (at(i + 1, j) - at(i, j)) * sx;
        let bot = at(i, j + 1) + (at(i + 1, j + 1) - at(i, j + 1)) * sx;
        top + (bot - top) * sy
    };
    let surface = &fog.surface;
    let planet = &*body.0;
    // Closer in than this, a point is surely ground (no need to look up the
    // outline or craters); further out than the body's reach, surely sky.
    let surf_min = surface.iter().fold(f64::INFINITY, |m, &v| m.min(v as f64));
    let crater_floor = planet.crater_floor();
    let inner = surf_min.min(crater_floor) - 1.0;
    let inner2 = inner.max(0.0).powi(2);
    let outer2 = planet.max_radius.powi(2);
    // Only the craters this view touches are checked.
    let craters = planet.craters_in(l, r, b, t);
    let in_crater = |x: f64, y: f64| craters.iter().any(|&(cx, cy, cr)| (x - cx) * (x - cx) + (y - cy) * (y - cy) < cr * cr);
    let ns = surface.len() - 1;
    let ground = |x: f64, y: f64| -> f64 {
        let a = y.atan2(x).rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU * ns as f64;
        let i = a.floor() as usize % ns;
        let f = a - a.floor();
        surface[i] as f64 * (1.0 - f) + surface[i + 1] as f64 * f
    };
    // The sonar chart: dots on a world-anchored grid (spacing doubling as
    // the view zooms out, so they stay put while panning), rings of equal
    // depth, and a pulse sweeping outward from the centre.
    let dot_sp = pow2_at_least(v.mpp * 10.0);
    let dot_r = (0.85 * PX * v.mpp / dot_sp).max(0.12);
    let ring_sp = pow2_at_least(v.mpp * 70.0);
    let radius = body.0.radius.max(1.0);
    // Far from anything known: plain dark. Near it: the sonar chart's shade.
    let dark = [3.0, 3.0, 4.0];
    let bg = [8.0, 9.0, 11.0];
    let memory = [10.0, 13.0, 24.0];
    let dot = [74.0, 90.0, 88.0];
    let glow = [150.0, 150.0, 148.0];
    let mut px = vec![0u8; ow * oh * 4];
    px.par_chunk_map_mut(ComputeTaskPool::get(), ow * 4 * 8, |chunk_i, rows| {
        let row0 = chunk_i * 8;
        for (ri, row) in rows.chunks_mut(ow * 4).enumerate() {
            let pr = row0 + ri;
            let y = v.y + (v.h / 2.0 - (pr as f64 + 0.5) * PX) * v.mpp;
            for pc in 0..ow {
                let x = v.x + ((pc as f64 + 0.5) * PX - v.w / 2.0) * v.mpp;
                let r2 = x * x + y * y;
                if r2 > outer2 {
                    continue;
                }
                // Open sky (and crater bowls) is never fogged.
                let near_surface = r2 > inner2;
                if near_surface && ((r2.sqrt() > ground(x, y)) || (!craters.is_empty() && in_crater(x, y))) {
                    continue;
                }
                let rr = r2.sqrt();
                // Four samples a little apart, averaged: rounds off the
                // grid's corners so the edge follows the charted shape.
                let o = 0.35 * cs;
                let c = 0.25 * (sample(&charted, x - o, y - o) + sample(&charted, x + o, y - o) + sample(&charted, x - o, y + o) + sample(&charted, x + o, y + o));
                let lv = sample(&live, x, y);
                let u = 1.0 - c;
                // How near charted ground: 0 far off (all dark), 1 beside it.
                let prox = nearness(x, y).max(c);
                let mut alpha = u * (1.0 - 0.015 * prox) + c * (1.0 - lv) * 0.5;
                let mut rgb = mix(mix(dark, bg, prox), memory, c);
                if u > 0.01 && prox > 0.01 {
                    let (dx, dy) = (x / dot_sp, y / dot_sp);
                    let (fx, fy) = (dx - dx.floor() - 0.5, dy - dy.floor() - 0.5);
                    if fx * fx + fy * fy < dot_r * dot_r {
                        let depth = (if near_surface { ground(x, y) } else { surf_min }) - rr;
                        let ring = ((depth / ring_sp).fract() - 0.5).abs();
                        let on_ring = if ring > 0.5 - 1.6 * v.mpp / ring_sp { 1.0 } else { 0.0 };
                        let pulse = ((rr / radius * 3.0 - now as f64 * 0.12).rem_euclid(1.0) as f32 - 0.85).max(0.0) / 0.15;
                        let bright = 0.26 + 0.45 * on_ring + 0.35 * pulse;
                        rgb = mix(rgb, dot, (u * prox * bright * 0.8).min(1.0));
                    }
                }
                // The charted edge: a faint, colourless line, breathing gently.
                if c > 0.02 && c < 0.98 {
                    let e = (c - 0.5) / 0.12;
                    let edge = (-e * e).exp() * (0.8 + 0.2 * ((now as f64 * 1.6 + (x + y) / (cs * 2.0)).sin() as f32));
                    rgb = mix(rgb, glow, edge * 0.45);
                    alpha = alpha.max(edge * 0.35);
                }
                if alpha <= 0.0 {
                    continue;
                }
                let o = pc * 4;
                row[o] = rgb[0] as u8;
                row[o + 1] = rgb[1] as u8;
                row[o + 2] = rgb[2] as u8;
                row[o + 3] = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
            }
        }
    });
    if let Some(mut img) = images.get_mut(&fog.image) {
        let size = Extent3d { width: ow as u32, height: oh as u32, depth_or_array_layers: 1 };
        if img.texture_descriptor.size != size {
            img.resize(size);
        }
        img.data = Some(px);
    }
    sprite.custom_size = Some(Vec2::new((ow as f64 * PX) as f32, (oh as f64 * PX) as f32));
    perf.add(crate::log::P_FOG, t0);
}
