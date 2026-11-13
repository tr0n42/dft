//! Drones and haulers.
//!
//! The screen keeps its own copy of every drone's flight leg; the simulation
//! sends only the legs that changed, so handing over costs next to nothing
//! however big the swarm. Each frame every drone is moved along its leg to
//! the moment being drawn (on all cores), and drawn as a small square, one
//! mesh per kind of drone (waiting, flying out, flying home).
//!
//! Drones too small to see apart are approximated: when a drone is under a
//! few pixels across, the screen is cut into cells that size and each cell
//! with any drone of a kind in it is drawn once. What it costs to draw then
//! depends on the screen, not the swarm: half a million drones over a whole
//! planet cost the same as the few thousand pixels they cover.
use crate::view::View;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use bevy::tasks::{ComputeTaskPool, ParallelSlice};
use dft_sim::swarm::Leg;
use std::sync::atomic::{AtomicU8, Ordering::Relaxed};

pub struct DronesPlugin;

impl Plugin for DronesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Fleet>().add_systems(Startup, setup).add_systems(Update, draw);
    }
}

/// Every drone slot's current leg, as last sent by the simulation.
#[derive(Resource, Default)]
pub struct Fleet {
    pub legs: Vec<Leg>,
}

impl Fleet {
    /// Forget everything (a new body).
    pub fn clear(&mut self) {
        self.legs.clear();
    }
}

/// The meshes: one per kind of drone, and one for the hubs.
#[derive(Resource)]
struct Meshes {
    /// Each kind out in the open, then each inside the mother swarm (drawn
    /// behind its near layers, so it looks wrapped in it).
    kinds: [Handle<Mesh>; 6],
    hubs: Handle<Mesh>,
    hubs_back: Handle<Mesh>,
    /// Cutters close enough to see their shapes (in the open, and inside
    /// the mother swarm), cell by cell in colour.
    detail: [Handle<Mesh>; 2],
    chunks: Handle<Mesh>,
    booms: Handle<Mesh>,
}

/// What the drones' drawing keeps between frames: which materials are
/// plants (not ground for feet), and the steady clock it draws by.
#[derive(Default)]
struct DrawMem {
    flora: Vec<bool>,
    clock: Option<f64>,
}

/// The smallest a drone is drawn, in pixels across.
const MIN_PX: f32 = 3.0;

fn empty_mesh(colours: bool) -> Mesh {
    let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD);
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3]);
    if colours {
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; 3]);
    }
    m
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
    // Waiting drones underneath, those flying home on top.
    // (Zoomed out, all one plain machine grey whatever they're doing: icons
    // for a map view come later.)
    let colours = [Color::srgba_u8(118, 124, 122, 220), Color::srgb_u8(132, 138, 136), Color::srgb_u8(128, 134, 132)];
    let kinds: [Handle<Mesh>; 6] = std::array::from_fn(|i| {
        let h = meshes.add(empty_mesh(false));
        let (c, z) = if i < 3 { (colours[i], 10.0 + i as f32) } else { (colours[i - 3].with_alpha(0.55), crate::swarmview::Z_INSIDE + 0.02 + 0.01 * (i - 3) as f32) };
        commands.spawn((Mesh2d(h.clone()), MeshMaterial2d(materials.add(ColorMaterial::from(c))), Transform::from_xyz(0.0, 0.0, z), NoFrustumCulling));
        h
    });
    // Rock in the air, under the drones.
    let chunks = meshes.add(empty_mesh(true));
    commands.spawn((Mesh2d(chunks.clone()), MeshMaterial2d(materials.add(ColorMaterial::default())), Transform::from_xyz(0.0, 0.0, 9.0), NoFrustumCulling));
    let hubs = meshes.add(empty_mesh(true));
    commands.spawn((Mesh2d(hubs.clone()), MeshMaterial2d(materials.add(ColorMaterial::default())), Transform::from_xyz(0.0, 0.0, 14.0), NoFrustumCulling));
    // Haulers inside the mother swarm: behind its near layers.
    let hubs_back = meshes.add(empty_mesh(true));
    commands.spawn((Mesh2d(hubs_back.clone()), MeshMaterial2d(materials.add(ColorMaterial::from(Color::srgba(1.0, 1.0, 1.0, 0.6)))), Transform::from_xyz(0.0, 0.0, crate::swarmview::Z_INSIDE + 0.06), NoFrustumCulling));
    // Explosions on top.
    let booms = meshes.add(empty_mesh(true));
    commands.spawn((Mesh2d(booms.clone()), MeshMaterial2d(materials.add(ColorMaterial::default())), Transform::from_xyz(0.0, 0.0, 15.0), NoFrustumCulling));
    let detail: [Handle<Mesh>; 2] = std::array::from_fn(|i| {
        let h = meshes.add(empty_mesh(true));
        let (c, z) = if i == 0 { (Color::WHITE, 10.5) } else { (Color::srgba(1.0, 1.0, 1.0, 0.6), crate::swarmview::Z_INSIDE + 0.05) };
        commands.spawn((Mesh2d(h.clone()), MeshMaterial2d(materials.add(ColorMaterial::from(c))), Transform::from_xyz(0.0, 0.0, z), NoFrustumCulling));
        h
    });
    commands.insert_resource(Meshes { kinds, hubs, hubs_back, detail, chunks, booms });
}

/// A hauler, cell by cell (top row first): A armour, H hold, L light,
/// D dark, V thrust vent, space nothing. Twelve by nine metres.
const HAULER: [&str; 9] = [
    "    AAAA    ",
    " AAAAAAAAAA ",
    "AALHHHHHHLAA",
    "AADHHHHHHDAA",
    "AADHHHHHHDAA",
    "AADHHHHHHDAA",
    "AAAHHHHHHAAA",
    " AAAAAAAAAA ",
    "  VV    VV  ",
];
const HAULER_W: usize = 12;
/// A hauler's tentacles, squid-like, hanging from its belly between the
/// thrust vents: which column each grows from, and whether it is one of the
/// two long feeding arms (longer, swaying more, a light at the tip) or one
/// of the short ones.
const TENTACLES: [(usize, bool); 6] = [(1, false), (4, false), (5, true), (6, true), (7, false), (10, false)];
/// Tentacle lengths (whole metres, below the root block): short and long.
const TENTACLE_SHORT: (u64, u64) = (4, 6);
const TENTACLE_LONG: (u64, u64) = (8, 10);
/// How long a hauler takes to settle into hovering over its flock, and to
/// gather itself up again as it lifts off (game seconds).
const SETTLE_S: f64 = 6.0;
/// How high over the ground a hauler's middle is always kept: half its
/// height, its longest tentacle and the light at its tip, and a margin, so
/// it floats and its tentacles never touch the ground.
const HOVER_CLEAR_M: f64 = 4.5 + 1.0 + TENTACLE_LONG.1 as f64 + 2.0;
/// Haulers are kept at least this far apart (middle to middle).
const HAULER_GAP_M: f64 = 14.0;

/// How settled a hauler is over its flock (0 to 1): settling once it has
/// arrived to be loaded, gathering itself up as it lifts off.
fn settle(hl: &dft_sim::swarm::HaulerView, now: f64) -> f32 {
    (match hl.state {
        dft_sim::swarm::HaulerState::Working => ((now - hl.t1) / SETTLE_S).clamp(0.0, 1.0),
        dft_sim::swarm::HaulerState::Lifting => 1.0 - ((now - hl.t0) / SETTLE_S).clamp(0.0, 1.0),
        _ => 0.0,
    }) as f32
}
const HOLD_X0: usize = 3;
const HOLD_Y0: usize = 2;
const HOLD_W: usize = 6;
const HOLD_H: usize = 5;
/// The hold's cells in the order they fill: bottom row first, left to right.
const HOLD_CELLS: [(usize, usize); 30] = {
    let mut out = [(0usize, 0usize); 30];
    let mut k = 0;
    let mut row = HOLD_Y0 + HOLD_H;
    while row > HOLD_Y0 {
        row -= 1;
        let mut c = HOLD_X0;
        while c < HOLD_X0 + HOLD_W {
            out[k] = (c, row);
            k += 1;
            c += 1;
        }
    }
    out
};

/// Cutters' shapes, three cells by three (top row first): A hull, L light,
/// C cargo (dark when empty), space nothing. Each cutter is one of these,
/// in its own shade.
const CUTTERS: [[&str; 3]; 10] = [
    [" A ", "ALA", "A A"],
    ["AAA", "ALA", " C "],
    ["A A", "ALA", " C "],
    ["AA ", "ALC", " AA"],
    [" A ", "ALA", " C "],
    ["A A", " L ", "ACA"],
    ["AAA", "L A", "ACA"],
    [" AA", "ALC", "AA "],
    ["ALA", " C ", "A A"],
    ["A A", "ACA", " L "],
];

/// One cutter at (x, y) on screen, `h` half its size in pixels: its shape
/// and shade (by `id`); its light in the swarm's colour (`lc`, at strength
/// `pulse`), dim while it waits; its cargo cell, flying home, in the colour
/// of something brought up lately.
#[allow(clippy::too_many_arguments)]
fn cutter(pos: &mut Vec<[f32; 3]>, col: &mut Vec<[f32; 4]>, x: f32, y: f32, h: f32, id: u64, state: u8, lc: [f32; 3], pulse: f32, recent: &[([u8; 3], f64)]) {
    let hsh = dft_sim::noise::hash(id, 0, 0, 0xC0_77);
    let shape = &CUTTERS[(hsh % CUTTERS.len() as u64) as usize];
    let shade = 0.38 + 0.16 * crate::rnd::unit(hsh, 0x51) as f32;
    // A few run a little warm or cold, most plain grey.
    let tint = match (hsh >> 20) % 7 {
        0 => [1.12, 1.0, 0.9],
        1 => [0.9, 1.0, 1.12],
        _ => [1.0, 1.04, 1.02],
    };
    let cargo = if state == 2 && !recent.is_empty() {
        let total: f64 = recent.iter().map(|e| e.1).sum();
        let mut u = crate::rnd::unit(hsh, 0xCA) * total;
        let mut pick = recent[0].0;
        for (c, kg) in recent {
            if u < *kg {
                pick = *c;
                break;
            }
            u -= kg;
        }
        [pick[0] as f32 / 255.0, pick[1] as f32 / 255.0, pick[2] as f32 / 255.0]
    } else {
        [0.04, 0.045, 0.045]
    };
    let cell = 2.0 * h / 3.0;
    for (row, line) in shape.iter().enumerate() {
        for (c, ch) in line.bytes().enumerate() {
            let k: [f32; 3] = match ch {
                b'A' => [shade * tint[0], shade * tint[1], shade * tint[2]],
                b'L' => {
                    let p = if state == 0 { 0.35 } else { pulse };
                    [lc[0] * p, lc[1] * p, lc[2] * p]
                }
                b'C' => cargo,
                _ => continue,
            };
            let lin = Color::srgb(k[0], k[1], k[2]).to_linear();
            let (cx, cy) = (x - h + (c as f32 + 0.5) * cell, y + h - (row as f32 + 0.5) * cell);
            let hh = cell * 0.5;
            pos.extend_from_slice(&[[cx - hh, cy - hh, 0.0], [cx + hh, cy - hh, 0.0], [cx + hh, cy + hh, 0.0], [cx - hh, cy - hh, 0.0], [cx + hh, cy + hh, 0.0], [cx - hh, cy + hh, 0.0]]);
            col.extend_from_slice(&[[lin.red, lin.green, lin.blue, 1.0]; 6]);
        }
    }
}

/// Linear colour from sRGB bytes.
fn rgb(r: u8, g: u8, b: u8) -> [f32; 4] {
    let c = Color::srgb_u8(r, g, b).to_linear();
    [c.red, c.green, c.blue, 1.0]
}

/// Two triangles for a square of half-size `h` at (x, y).
fn square(out: &mut Vec<[f32; 3]>, x: f32, y: f32, h: f32) {
    out.extend_from_slice(&[[x - h, y - h, 0.0], [x + h, y - h, 0.0], [x + h, y + h, 0.0], [x - h, y - h, 0.0], [x + h, y + h, 0.0], [x - h, y + h, 0.0]]);
}

/// Two triangles for a piece of a cell turned with its body: the cell's
/// middle at (cx, cy) on screen, `h` half a cell in pixels, the piece's
/// lower-left corner at (u0, v0) and its size `f` (cell units, from the
/// cell's middle).
#[allow(clippy::too_many_arguments)]
pub(crate) fn rot_square(out: &mut Vec<[f32; 3]>, cx: f32, cy: f32, h: f32, angle: f32, u0: f32, v0: f32, f: f32) {
    let (sn, cs) = angle.sin_cos();
    let s = 2.0 * h;
    let pt = |u: f32, v: f32| -> [f32; 3] {
        let (x, y) = (u * s, v * s);
        [cx + x * cs - y * sn, cy + x * sn + y * cs, 0.0]
    };
    let (a, b, c, d) = (pt(u0, v0), pt(u0 + f, v0), pt(u0 + f, v0 + f), pt(u0, v0 + f));
    out.extend_from_slice(&[a, b, c, a, c, d]);
}

/// The flocks' flow fields, found by where their haulers were, so a drone
/// flying out from or home to its hauler on screen can be drawn following
/// the way round through open space instead of straight through rock.
#[derive(Default)]
pub struct PathIndex {
    version: u64,
    fields: Vec<std::sync::Arc<dft_sim::flow::FlowField>>,
    /// Fields by 16 m square of their hub.
    grid: std::collections::HashMap<(i64, i64), Vec<usize>>,
}

impl PathIndex {
    fn set(&mut self, version: u64, fields: Vec<std::sync::Arc<dft_sim::flow::FlowField>>) {
        self.version = version;
        self.grid.clear();
        for (i, f) in fields.iter().enumerate() {
            let k = ((f.hub.0 / 16.0).floor() as i64, (f.hub.1 / 16.0).floor() as i64);
            self.grid.entry(k).or_default().push(i);
        }
        self.fields = fields;
    }

    /// The field swept from within 8 m of `p`, if any.
    fn near(&self, p: (f64, f64)) -> Option<&dft_sim::flow::FlowField> {
        let (kx, ky) = ((p.0 / 16.0).floor() as i64, (p.1 / 16.0).floor() as i64);
        let mut best: Option<(f64, usize)> = None;
        for dy in -1..=1 {
            for dx in -1..=1 {
                for &i in self.grid.get(&(kx + dx, ky + dy)).into_iter().flatten() {
                    let h = self.fields[i].hub;
                    let d = (h.0 - p.0).powi(2) + (h.1 - p.1).powi(2);
                    if d <= 64.0 && best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, i));
                    }
                }
            }
        }
        best.map(|(_, i)| &*self.fields[i])
    }

    /// Where a drone is at game time t: along its flock's field when it is
    /// flying out from its hauler or home to it, else straight.
    fn at(&self, leg: &Leg, t: f64) -> (f64, f64) {
        if leg.state == 1 || leg.state == 2 {
            let (hub_end, other) = if leg.state == 1 { (leg.from, leg.to) } else { (leg.to, leg.from) };
            if let Some(f) = self.near(hub_end) {
                let total = f.dist_m(other.0, other.1).or_else(|| f.reach_m((other.0.floor() as i64, other.1.floor() as i64)));
                if let Some(total) = total {
                    let s = (t - leg.t0).max(0.0) * leg.speed;
                    if s >= total {
                        return leg.to;
                    }
                    let from_hub = if leg.state == 1 { s } else { total - s };
                    if let Some(p) = f.along(other, from_hub) {
                        return p;
                    }
                }
            }
        }
        leg.at(t)
    }
}

/// What rock in the air needs to look like the ground: the texture kind of
/// each material, and each cell's colour where it was cut from.
#[derive(Default)]
pub struct AirLook {
    kinds: Vec<dft_sim::texture::Kind>,
    colour: std::collections::HashMap<((i64, i64), dft_sim::materials::MatId), [u8; 3]>,
}

/// Take a mesh's position list to refill (keeping its memory).
fn take_positions(mesh: &mut Mesh) -> Vec<[f32; 3]> {
    match mesh.remove_attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(mut v)) => {
            v.clear();
            v
        }
        _ => Vec::new(),
    }
}

/// The screen cut into cells a drone across, for drones too small to see
/// apart: each cell holds a bit per kind of drone in it.
#[derive(Default)]
struct Grid {
    cells: Vec<AtomicU8>,
}

#[allow(clippy::too_many_arguments)]
fn draw(
    meshes_r: Res<Meshes>,
    mut meshes: ResMut<Assets<Mesh>>,
    view: Res<View>,
    run: Option<Res<crate::Run>>,
    mut fleet: ResMut<Fleet>,
    mut perf: ResMut<crate::log::Perf>,
    mut grid: Local<Grid>,
    body: Res<crate::Body>,
    mut shown: Local<Vec<(dft_sim::swarm::Boom, f64)>>,
    time: Res<Time<Real>>,
    swarm: Res<crate::swarmview::SwarmNow>,
    lights: Res<crate::lights::Lights>,
    mut air: Local<AirLook>,
    mut paths: Local<PathIndex>,
    mut mem: Local<DrawMem>,
) {
    let mem = &mut *mem;
    let flora = &mut mem.flora;
    let t0 = std::time::Instant::now();
    let Some(run) = run else { return };
    // Take what changed, and the moment to draw.
    let shape = swarm.0;
    let (now, haulers, units, chunks, new_booms, recent, heights) = {
        let mut out = run.0.out.lock();
        if let Some(v) = out.leg_full.take() {
            fleet.legs = v;
        }
        let changes = std::mem::take(&mut out.leg_changes);
        for (i, leg) in changes {
            let i = i as usize;
            if i >= fleet.legs.len() {
                fleet.legs.resize(i + 1, Leg { from: (0.0, 0.0), to: (0.0, 0.0), t0: 0.0, speed: 0.0, state: Leg::GONE });
            }
            fleet.legs[i] = leg;
        }
        let control = *run.0.control.lock();
        // Move the drones on from when the simulation published to now, at
        // the speed the game is actually running (the speed asked for, or
        // slower if the simulation can't keep up; as fast as it is going at
        // the top speed), so a slow simulation step doesn't freeze them.
        // Never past a quarter second, and never back in time.
        let speed = crate::sim::SPEEDS[control.speed.min(crate::sim::SPEEDS.len() - 1)];
        let actual = out.stats.speed.max(0.0);
        let rate = if actual > 0.0 { speed.min(actual * 1.05) } else if speed.is_finite() { speed } else { 0.0 };
        let ahead = match out.at {
            Some(at) if !control.paused && out.stats.launched_at.is_none() => at.elapsed().as_secs_f64().min(0.25) * rate,
            _ => 0.0,
        };
        let target = out.stats.time + ahead;
        // The moment drawn runs on a steady clock: every frame it moves on by
        // the frame's time at the game's speed, and eases toward the
        // simulation's own time rather than jumping to it or waiting for it,
        // so drones and ships move smoothly at the screen's frame rate. (Back
        // to the simulation's time at once after a jump: a new body or a load.)
        let dt = time.delta_secs_f64().min(0.1);
        let paused = control.paused || out.stats.launched_at.is_some();
        let t = match mem.clock {
            Some(c) => {
                let mut c = c + if paused { 0.0 } else { dt * rate };
                let err = target - c;
                if err.abs() > 2.0 * rate.max(1.0) + 1.0 {
                    c = target;
                } else {
                    c += err * (dt * 3.0).min(1.0);
                }
                c
            }
            None => target,
        };
        mem.clock = Some(t);
        if out.fields_version != paths.version {
            paths.set(out.fields_version, out.fields.clone());
        }
        (t, out.haulers.clone(), (out.units.clone(), out.animals.clone()), out.chunks.clone(), std::mem::take(&mut out.booms), out.stats.recent_mix.clone(), out.heights.as_ref().map(|h| h.1.clone()))
    };
    let lc = lights.rgb();
    let v = *view;
    let real_half = (0.5 / v.mpp) as f32;
    let tiny = real_half * 2.0 < MIN_PX;
    let half = real_half.max(MIN_PX / 2.0);
    // A drone this far outside the window can't show.
    let (l, b, r, t) = v.bounds();
    let m = 2.0 * half as f64 * v.mpp;
    let (l, b, r, t) = (l - m, b - m, r + m, t + m);
    let pool = ComputeTaskPool::get();
    let chunk = 16384;
    let mut lists: [Vec<[f32; 3]>; 6] = std::array::from_fn(|i| meshes.get_mut(&meshes_r.kinds[i]).map(|mut mm| take_positions(&mut *mm)).unwrap_or_default());
    let shape = &shape;
    if tiny {
        // Approximate: mark the cells, then draw each marked cell once.
        let (gw, gh) = ((v.w as f32 / MIN_PX).ceil() as usize + 2, (v.h as f32 / MIN_PX).ceil() as usize + 2);
        if grid.cells.len() != gw * gh {
            grid.cells = (0..gw * gh).map(|_| AtomicU8::new(0)).collect();
        } else {
            for c in &grid.cells {
                c.store(0, Relaxed);
            }
        }
        let cells = &grid.cells;
        fleet.legs.par_chunk_map(pool, chunk, |_, legs| {
            for leg in legs {
                if leg.state > 2 {
                    continue;
                }
                let (x, y) = leg.at(now);
                if x < l || x > r || y < b || y > t {
                    continue;
                }
                let p = v.to_screen(x, y);
                let gx = ((p.x + v.w as f32 / 2.0) / MIN_PX).floor() as isize + 1;
                let gy = ((p.y + v.h as f32 / 2.0) / MIN_PX).floor() as isize + 1;
                if gx < 0 || gy < 0 || gx as usize >= gw || gy as usize >= gh {
                    continue;
                }
                // (Read first: most cells are already marked, and writing
                // the same few cells from every core at once is slow.)
                let cell = &cells[gy as usize * gw + gx as usize];
                let inside = shape.inside(x, y, 2.0) as u8;
                let bit = 1u8 << (leg.state + 3 * inside);
                if cell.load(Relaxed) & bit == 0 {
                    cell.fetch_or(bit, Relaxed);
                }
            }
        });
        for gy in 0..gh {
            for gx in 0..gw {
                let bits = cells[gy * gw + gx].load(Relaxed);
                if bits == 0 {
                    continue;
                }
                let x = (gx as f32 - 1.0 + 0.5) * MIN_PX - v.w as f32 / 2.0;
                let y = (gy as f32 - 1.0 + 0.5) * MIN_PX - v.h as f32 / 2.0;
                for (k, list) in lists.iter_mut().enumerate() {
                    if bits & (1u8 << k) != 0 {
                        square(list, x, y, half);
                    }
                }
            }
        }
    } else {
        // Close enough to see each drone: draw each where it is, moving as
        // part of a swirling cloud (see `swirl`).
        let real_t = time.elapsed_secs_f64();
        let paths = &*paths;
        // Big enough on screen to show each one's shape (cells 2 px or more)?
        let detailed = real_half * 2.0 >= 6.0;
        let recent = &recent;
        let parts = fleet.legs.par_chunk_map(pool, chunk, |ci, legs| {
            let mut out: [Vec<[f32; 3]>; 6] = Default::default();
            let mut det: [(Vec<[f32; 3]>, Vec<[f32; 4]>); 2] = Default::default();
            for (j, leg) in legs.iter().enumerate() {
                if leg.state > 2 {
                    continue;
                }
                // (Far outside the window either end: it can't show.)
                let off = |p: (f64, f64)| p.0 < l - 400.0 || p.0 > r + 400.0 || p.1 < b - 400.0 || p.1 > t + 400.0;
                if off(leg.from) && off(leg.to) {
                    continue;
                }
                let (x, y) = paths.at(leg, now);
                let (ox, oy) = swirl(leg, (ci * chunk + j) as u64, now, real_t, x, y);
                let (x, y) = (x + ox, y + oy);
                if x < l || x > r || y < b || y > t {
                    continue;
                }
                let p = v.to_screen(x, y);
                let inside = shape.inside(x, y, 2.0) as usize;
                if detailed {
                    let (dp, dc) = &mut det[inside];
                    cutter(dp, dc, p.x, p.y, real_half, (ci * chunk + j) as u64, leg.state, lc, crate::lights::Lights::pulse(real_t, y.atan2(x)), recent);
                } else {
                    square(&mut out[leg.state as usize + 3 * inside], p.x, p.y, half);
                }
            }
            (out, det)
        });
        let mut dets: [(Vec<[f32; 3]>, Vec<[f32; 4]>); 2] = std::array::from_fn(|i| (meshes.get_mut(&meshes_r.detail[i]).map(|mut mm| take_positions(&mut *mm)).unwrap_or_default(), Vec::new()));
        for (part, det) in parts {
            for k in 0..6 {
                lists[k].extend_from_slice(&part[k]);
            }
            for i in 0..2 {
                dets[i].0.extend_from_slice(&det[i].0);
                dets[i].1.extend_from_slice(&det[i].1);
            }
        }
        for (i, (mut p, mut c)) in dets.into_iter().enumerate() {
            if p.is_empty() {
                p.extend_from_slice(&[[0.0; 3]; 3]);
                c.extend_from_slice(&[[0.0; 4]; 3]);
            }
            if let Some(mut mm) = meshes.get_mut(&meshes_r.detail[i]) {
                mm.insert_attribute(Mesh::ATTRIBUTE_POSITION, p);
                mm.insert_attribute(Mesh::ATTRIBUTE_COLOR, c);
            }
        }
    }
    if tiny || real_half * 2.0 < 6.0 {
        // (No shapes drawn this frame.)
        for i in 0..2 {
            if let Some(mut mm) = meshes.get_mut(&meshes_r.detail[i]) {
                mm.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3]);
                mm.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; 3]);
            }
        }
    }
    let mut quads = 0;
    for (k, mut list) in lists.into_iter().enumerate() {
        quads += list.len() / 6;
        if list.is_empty() {
            // (An empty mesh makes the renderer complain every frame.)
            list.extend_from_slice(&[[0.0; 3]; 3]);
        }
        if let Some(mut mm) = meshes.get_mut(&meshes_r.kinds[k]) {
            mm.insert_attribute(Mesh::ATTRIBUTE_POSITION, list);
        }
    }
    // Rock in the air: each cell drawn as the tiles draw ground (the colour
    // and texture of the place it was cut from, bevelled where open), placed
    // smoothly and turned with its body, so it looks the same falling as
    // standing and settled.
    if let Some(mut mm) = meshes.get_mut(&meshes_r.chunks) {
        use dft_sim::texture::{self as tx, Sides};
        let p = &body.0;
        if air.kinds.is_empty() {
            air.kinds = tx::kinds(&p.mats);
        }
        if air.colour.len() > 200_000 {
            air.colour.clear();
        }
        let mut pos = take_positions(&mut mm);
        let mut col = Vec::new();
        let ppm = (1.0 / v.mpp) as f32;
        // Texels a cell across: as many as fit (as on the tiles), at most 8,
        // fewer when many cells are on screen.
        let on_screen: Vec<&dft_sim::bodies::AirCell> = chunks.iter().filter(|a| a.x > l - 2.0 && a.x < r + 2.0 && a.y > b - 2.0 && a.y < t + 2.0).collect();
        let mut n = 1usize;
        while n < 8 && (n * 2) as f32 <= ppm && on_screen.len() * (n * 2) * (n * 2) <= 150_000 {
            n *= 2;
        }
        let homes: std::collections::HashMap<(i64, i64), dft_sim::materials::MatId> = if n >= 4 { on_screen.iter().map(|a| (a.home, a.m)).collect() } else { Default::default() };
        let h = (0.5 * ppm).max(0.5);
        for a in on_screen {
            let base = *air.colour.entry((a.home, a.m)).or_insert_with(|| {
                let (fx, fy) = (a.home.0 as f64 + 0.5, a.home.1 as f64 + 0.5);
                let rr = (fx * fx + fy * fy).sqrt().max(1e-9);
                dft_sim::render::colour_at(p, a.m, a.home.0, a.home.1, p.surface_radius(fx / rr, fy / rr))
            });
            let c = v.to_screen(a.x, a.y);
            if n == 1 {
                let k = rgb(base[0], base[1], base[2]);
                rot_square(&mut pos, c.x, c.y, h, a.angle, -0.5, -0.5, 1.0);
                col.extend_from_slice(&[k; 6]);
                continue;
            }
            let kind = air.kinds.get(a.m as usize).copied().unwrap_or(tx::Kind::Rock);
            let (hx, hy) = a.home;
            let sides = if n >= 4 {
                let ns = [(hx, hy + 1), (hx, hy - 1), (hx - 1, hy), (hx + 1, hy)];
                Sides { open: ns.map(|q| !homes.contains_key(&q)), other: ns.map(|q| homes.get(&q) != Some(&a.m)) }
            } else {
                Sides::default()
            };
            let f = 1.0 / n as f32;
            for j in 0..n {
                for i in 0..n {
                    let (u, w) = ((i as f64 + 0.5) / n as f64, (j as f64 + 0.5) / n as f64);
                    let tc = tx::texel(kind, base, hx, hy, u, w, n as f64, sides);
                    rot_square(&mut pos, c.x, c.y, h, a.angle, -0.5 + i as f32 * f, -0.5 + j as f32 * f, f);
                    col.extend_from_slice(&[rgb(tc[0], tc[1], tc[2]); 6]);
                }
            }
        }
        if pos.is_empty() {
            pos.extend_from_slice(&[[0.0; 3]; 3]);
            col.extend_from_slice(&[[0.0; 4]; 3]);
        }
        mm.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        mm.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    }
    // The haulers: block-built carriers (one-metre cells, as the ground),
    // upright to the body, their hold filling from the bottom with what
    // their flock has been cutting, in its own colours (see cargo.rs).
    let (mut pos_b, mut col_b) = (meshes.get_mut(&meshes_r.hubs_back).map(|mut mm| take_positions(&mut *mm)).unwrap_or_default(), Vec::new());
    if let Some(mut mm) = meshes.get_mut(&meshes_r.hubs) {
        let mut pos = take_positions(&mut *mm);
        let mut col = Vec::new();
        let real_t = time.elapsed_secs_f64();
        // A cell's size on screen (never under a pixel: a hauler stays
        // readable zoomed out).
        let cell_px = ((1.0 / v.mpp) as f32).max(0.25);
        let reach = HAULER_W as f64 * cell_px as f64 * v.mpp;
        let hold_n = HOLD_CELLS.len();
        // Where each is drawn: always floating high enough over the ground
        // that its tentacles never touch it, bobbing gently there.
        let ground = |a: f64| -> Option<f64> {
            let h = heights.as_ref()?;
            let n = h.len();
            if n == 0 {
                return None;
            }
            let tt = (a.rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU) * n as f64;
            let i = tt.floor() as usize % n;
            let f = tt - tt.floor();
            Some(h[i] as f64 * (1.0 - f) + h[(i + 1) % n] as f64 * f)
        };
        let near = reach + 60.0;
        let mut placed: Vec<(usize, f64, f64, f32)> = Vec::new();
        for (k, hl) in haulers.iter().enumerate() {
            let (x, y) = hl.at(now);
            if x < l - near || x > r + near || y < b - near || y > t + near {
                continue;
            }
            let ext = settle(hl, now);
            let (mut x, mut y) = (x, y);
            let rr = (x * x + y * y).sqrt().max(1.0);
            if let Some(g) = ground(y.atan2(x)) {
                // (Never below the hover height, so it comes down to it and
                // levels off smoothly, and never touches down.)
                let bob = 0.6 * (real_t * 0.8 + hl.id as f64 * 1.37).sin();
                let need = g + HOVER_CLEAR_M + bob;
                if rr < need {
                    x *= need / rr;
                    y *= need / rr;
                }
            }
            placed.push((k, x, y, ext));
        }
        // Only haulers hovering over their flock are kept apart, and only as
        // far as they have settled, so the push fades in on arrival and out
        // on lifting. In flight they pass freely in front of and behind each
        // other. (A stopgap: hover spots will be shared out so that no two
        // take the same one.)
        for _ in 0..6 {
            for i in 0..placed.len() {
                for j in i + 1..placed.len() {
                    let w = placed[i].3.min(placed[j].3) as f64;
                    if w <= 0.0 {
                        continue;
                    }
                    let (dx, dy) = (placed[j].1 - placed[i].1, placed[j].2 - placed[i].2);
                    let d = (dx * dx + dy * dy).sqrt();
                    if d >= HAULER_GAP_M {
                        continue;
                    }
                    // Apart along the line between them (along the ground,
                    // if they are on top of each other).
                    let (ux, uy) = if d > 1e-6 {
                        (dx / d, dy / d)
                    } else {
                        let a = placed[i].2.atan2(placed[i].1);
                        (-a.sin(), a.cos())
                    };
                    let push = (HAULER_GAP_M - d) * 0.5 * w;
                    placed[i].1 -= ux * push;
                    placed[i].2 -= uy * push;
                    placed[j].1 += ux * push;
                    placed[j].2 += uy * push;
                }
            }
        }
        for &(k, x, y, ext) in &placed {
            let hl = &haulers[k];
            if x < l - reach || x > r + reach || y < b - reach || y > t + reach {
                continue;
            }
            let p = v.to_screen(x, y);
            // (Inside the mother swarm, behind its near layers.)
            let back = shape.inside(x, y, 4.0);
            let (pos, col) = if back { (&mut pos_b, &mut col_b) } else { (&mut pos, &mut col) };
            let a = y.atan2(x);
            let angle = (a - std::f64::consts::FRAC_PI_2) as f32;
            let lamp = crate::lights::Lights::pulse(real_t, a);
            // The hold: cells filled from the bottom row up, in the colours
            // of what it carries.
            let mix: crate::cargo::Mix = hl.mix.iter().filter(|e| e.1 > 0.0).map(|e| (body.0.mats.get(e.0).colour, e.1 as f64)).collect();
            let ore = crate::cargo::ore_cells(&mix, HOLD_W, HOLD_H, hl.id as u64 * 7919);
            let full = (hl.fill.clamp(0.0, 1.0) * hold_n as f32).round() as usize;
            for (row, line) in HAULER.iter().enumerate() {
                for (colm, ch) in line.bytes().enumerate() {
                    let g = dft_sim::noise::unit(dft_sim::noise::hash(hl.id as u64, colm as i64, row as i64, 0x4A01)) as f32;
                    let c: [f32; 4] = match ch {
                        b'A' => {
                            let k = 0.27 + 0.07 * g;
                            [k, k * 1.06, k * 1.04, 1.0]
                        }
                        b'D' => [0.12, 0.13, 0.125, 1.0],
                        b'V' => [0.03, 0.035, 0.035, 1.0],
                        b'L' => [lc[0] * lamp, lc[1] * lamp, lc[2] * lamp, 1.0],
                        b'H' => {
                            // Which hold cell this is, counting from the bottom row.
                            let k = HOLD_CELLS.iter().position(|&(hx, hy)| hx == colm && hy == row).unwrap_or(0);
                            if k < full {
                                let (hx, hy) = HOLD_CELLS[k];
                                let o = ore[(hy - HOLD_Y0) * HOLD_W + (hx - HOLD_X0)];
                                [o[0] as f32 / 255.0, o[1] as f32 / 255.0, o[2] as f32 / 255.0, 1.0]
                            } else {
                                [0.07, 0.08, 0.075, 1.0]
                            }
                        }
                        _ => continue,
                    };
                    // sRGB to linear for the mesh.
                    let lin = Color::srgba(c[0], c[1], c[2], c[3]).to_linear();
                    let (u, w) = (colm as f32 - HAULER_W as f32 / 2.0, (HAULER.len() - 1 - row) as f32 - HAULER.len() as f32 / 2.0);
                    rot_square(pos, p.x, p.y, cell_px * 0.5, angle, u, w, 1.0);
                    col.extend_from_slice(&[[lin.red, lin.green, lin.blue, lin.alpha]; 6]);
                }
            }
            // Its tentacles, squid-like: six arms of dark sinew, banded with
            // bone, hanging from its belly in whole one-metre blocks, every
            // block joined edge to edge with the one above (a sideways step
            // gets its own blocks), curling and swaying at the tips. In flight
            // they beat like a swimming squid's, flaring out and pulling in
            // together; settled over its flock they drift, half spread. The
            // two long middle arms carry a light at the tip.
            {
                let tt = real_t as f32;
                let swim = 1.0 - ext;
                let lin = |r: f32, g: f32, b: f32| -> [f32; 4] {
                    let k = Color::srgb(r, g, b).to_linear();
                    [k.red, k.green, k.blue, 1.0]
                };
                let hid = dft_sim::noise::hash(hl.id as u64, 0, 0, 0x7E47);
                let beat = 0.5 + 0.5 * (tt * 1.7 + crate::rnd::unit(hid, 1) as f32 * 6.283).sin();
                let flare = swim * (0.3 + 1.2 * beat) + ext * 0.7;
                // The root row, in cell units from the hauler's middle (the
                // vent row's lower edge).
                let w0 = -(HAULER.len() as f32) / 2.0;
                for (ti, &(colm, long)) in TENTACLES.iter().enumerate() {
                    let rr = |s: u64| crate::rnd::unit(hid, 16 + ti as u64 * 8 + s) as f32;
                    let (lo, hi) = if long { TENTACLE_LONG } else { TENTACLE_SHORT };
                    let len = (lo as f32 + ((hi - lo + 1) as f32 * rr(0)).floor()).min(hi as f32) as i32;
                    let ph = rr(1) * 6.283;
                    let curl = if rr(2) < 0.5 { -1.0 } else { 1.0 } * (0.6 + rr(3)) * if long { 1.5 } else { 1.0 };
                    // Outer arms spread most, the middle ones hardly at all.
                    let spread = (colm as f32 - 5.5) / 4.5;
                    let u0 = colm as f32 - HAULER_W as f32 / 2.0;
                    let put = |pos: &mut Vec<[f32; 3]>, col: &mut Vec<[f32; 4]>, dx: i32, k: i32| {
                        let h = dft_sim::noise::unit(dft_sim::noise::hash(hl.id as u64, ti as i64 * 64 + k as i64, dx as i64, 0x51E7)) as f32;
                        let c = if k > 0 && k % 4 == 0 {
                            let v = 0.37 + 0.07 * h;
                            lin(v * 0.95, v, v * 1.05)
                        } else {
                            let v = 0.11 + 0.05 * h;
                            lin(v * 0.9, v * 0.97, (v * 1.14).min(1.0))
                        };
                        rot_square(pos, p.x, p.y, cell_px * 0.5, angle, u0 + dx as f32, w0 - k as f32, 1.0);
                        col.extend_from_slice(&[c; 6]);
                    };
                    // The root block, under the belly.
                    put(&mut *pos, &mut *col, 0, 0);
                    let mut px = 0i32;
                    for k in 1..=len {
                        let f = k as f32 / len as f32;
                        let sway = (tt * 0.7 + ph + k as f32 * 0.3).sin() * curl * f * f * 3.0 * (0.4 + 0.6 * ext);
                        let out = spread * flare * f * len as f32 * 0.4;
                        let nx = (out + sway).round() as i32;
                        // A sideways step: blocks along the row above first.
                        for sx in px.min(nx)..=px.max(nx) {
                            if sx != px {
                                put(&mut *pos, &mut *col, sx, k - 1);
                            }
                        }
                        put(&mut *pos, &mut *col, nx, k);
                        px = nx;
                    }
                    if long {
                        rot_square(pos, p.x, p.y, cell_px * 0.5, angle, u0 + px as f32, w0 - (len + 1) as f32, 1.0);
                        col.extend_from_slice(&[lin(lc[0] * lamp, lc[1] * lamp, lc[2] * lamp); 6]);
                    }
                }
            }
            quads += 1;
        }
        // The specialists (sappers, tunnelers, heat-sink bots, foresters).
        // Solid ground for feet: the screen's copy of the world (plants
        // aren't ground).
        if flora.len() != body.0.mats.len() {
            *flora = (0..body.0.mats.len()).map(|m| body.0.mats.get(m as dft_sim::materials::MatId).name.starts_with("flora ")).collect();
        }
        let mirror = run.0.mirror.read();
        let solid = |x: i64, y: i64| -> bool {
            let m = mirror.get(x, y).unwrap_or_else(|| body.0.material(x, y));
            m != dft_sim::materials::VACUUM && !flora.get(m as usize).copied().unwrap_or(false)
        };
        crate::units::draw(&units.0, now, real_t, &v, [l, b, r, t], cell_px, lc, &solid, &mut pos, &mut col);
        crate::units::draw_animals(&units.1, now, real_t, &v, [l, b, r, t], cell_px, &mut pos, &mut col);
        drop(mirror);
        if pos.is_empty() {
            pos.extend_from_slice(&[[0.0; 3]; 3]);
            col.extend_from_slice(&[[0.0; 4]; 3]);
        }
        mm.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        mm.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    }
    if let Some(mut mm) = meshes.get_mut(&meshes_r.hubs_back) {
        if pos_b.is_empty() {
            pos_b.extend_from_slice(&[[0.0; 3]; 3]);
            col_b.extend_from_slice(&[[0.0; 4]; 3]);
        }
        mm.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos_b);
        mm.insert_attribute(Mesh::ATTRIBUTE_COLOR, col_b);
    }
    let real = time.elapsed_secs_f64();
    // Explosions, built of blocks like everything else: a white-hot core
    // that collapses in a moment; a burst of fire-blocks flung out, cooling
    // from white through yellow, orange and red to drifting smoke; a ring of
    // dust racing out along the shock; embers arcing away and falling back,
    // trailing; and a scatter of the swarm's own lights from the wreckage.
    // The bigger the blast, the more of all of it, and the longer it lasts.
    for b in new_booms {
        let (vl, vb, vr, vt) = v.bounds();
        // Only a blast the player can see flashes and shakes the screen;
        // anywhere else the physics just runs.
        let near = b.x > vl && b.x < vr && b.y > vb && b.y < vt;
        if b.buried && near {
            // A charge going off underground: a white flash over everything,
            // the screen shaking (the proving ground's flash and shake).
            let kt = (b.size_m as f64 / 2.2 / 14.0).powi(3);
            crate::fx::flash((0.35 + (kt + 1.0).log10() * 0.22).min(1.0) as f32);
            crate::fx::shake((3.0 + 5.0 * (kt * 10.0 + 1.0).log10()).min(22.0) as f32);
        }
        if shown.len() < 600 {
            shown.push((b, real));
        }
    }
    shown.retain(|(bm, at)| real - at < if bm.buried { BURIED_LIFE_S } else { boom_life(bm.size_m) });
    if let Some(mut mm) = meshes.get_mut(&meshes_r.booms) {
        let mut pos = take_positions(&mut mm);
        let mut col = Vec::new();
        let mut budget = 120_000usize;
        let px = (1.0 / v.mpp) as f32;
        let block = |pos: &mut Vec<[f32; 3]>, col: &mut Vec<[f32; 4]>, x: f64, y: f64, m: f32, c: [f32; 4]| {
            if c[3] <= 0.01 {
                return;
            }
            let s = v.to_screen(x, y);
            // Snapped to whole pixels: hard-edged blocks.
            let hh = (m * px * 0.5).max(1.0).round();
            let (sx, sy) = (s.x.round(), s.y.round());
            pos.extend_from_slice(&[[sx - hh, sy - hh, 0.0], [sx + hh, sy - hh, 0.0], [sx + hh, sy + hh, 0.0], [sx - hh, sy - hh, 0.0], [sx + hh, sy + hh, 0.0], [sx - hh, sy + hh, 0.0]]);
            let k = Color::srgba(c[0], c[1], c[2], c[3]).to_linear();
            col.extend_from_slice(&[[k.red, k.green, k.blue, k.alpha]; 6]);
        };
        // Fire's colour as it cools (0 white-hot .. 1 smoke).
        let heat = |q: f32| -> [f32; 3] {
            let stops: [(f32, [f32; 3]); 6] = [(0.0, [1.0, 1.0, 0.92]), (0.12, [1.0, 0.92, 0.5]), (0.28, [1.0, 0.62, 0.18]), (0.45, [0.85, 0.27, 0.08]), (0.62, [0.32, 0.12, 0.08]), (1.0, [0.16, 0.16, 0.16])];
            let q = q.clamp(0.0, 1.0);
            for w in stops.windows(2) {
                if q <= w[1].0 {
                    let f = (q - w[0].0) / (w[1].0 - w[0].0);
                    return [w[0].1[0] + (w[1].1[0] - w[0].1[0]) * f, w[0].1[1] + (w[1].1[1] - w[0].1[1]) * f, w[0].1[2] + (w[1].1[2] - w[0].1[2]) * f];
                }
            }
            stops[5].1
        };
        for (bm, at) in shown.iter() {
            let size = bm.size_m.max(2.0) as f64;
            let life = boom_life(bm.size_m);
            let age = real - at;
            let reach = size * 3.0 + 60.0;
            if bm.x < l - reach || bm.x > r + reach || bm.y < b - reach || bm.y > t + reach || budget == 0 {
                continue;
            }
            // Its light: a flash, fading as it burns out.
            let fade = (1.0 - age / life).clamp(0.0, 1.0) as f32;
            crate::fx::emit(bm.x, bm.y, size * 1.4 + 8.0, [255.0, 200.0, 140.0], 2.2 * fade * fade);
            let seed = dft_sim::noise::hash((bm.x * 8.0) as i64 as u64, (bm.y * 8.0) as i64, (*at * 10.0) as i64, 0xB0_0B);
            let u = |k: u64| crate::rnd::unit(seed, k);
            // Up (away from the body), and along the ground.
            let rr = (bm.x * bm.x + bm.y * bm.y).sqrt().max(1.0);
            let (ux, uy) = (bm.x / rr, bm.y / rr);
            let g = 9.0;
            if bm.buried {
                // Underground: a shock ring of blocks running out at 260 m/s
                // to twice the blast's reach, fading (the proving ground's
                // rings); and the broken rock's heat, glowing and fading.
                let max = bm.size_m as f64;
                let rr_ = age * 260.0;
                if rr_ <= max {
                    let a = (1.0 - rr_ / max) as f32 * 0.8;
                    let n = ((rr_ * 6.0) as usize).max(24);
                    let mut seen = std::collections::HashSet::new();
                    for k in 0..n {
                        let ang = k as f64 / n as f64 * std::f64::consts::TAU;
                        let c = ((bm.x + ang.cos() * rr_).round(), (bm.y + ang.sin() * rr_).round());
                        if seen.insert((c.0 as i64, c.1 as i64)) {
                            block(&mut pos, &mut col, c.0, c.1, 1.0, [1.0, 0.94, 0.82, a]);
                        }
                    }
                }
                let heat = (1.0 - age / BURIED_LIFE_S).clamp(0.0, 1.0) as f32;
                crate::fx::emit(bm.x, bm.y, max * 0.55, [255.0, 150.0, 50.0], 1.3 * heat * heat);
                continue;
            }
            let start = pos.len();
            // The core: a ragged white-hot knot, gone in a blink.
            let core_t = 0.12 + 0.02 * size.sqrt();
            if age < core_t {
                let q = (age / core_t) as f32;
                let rad = size * 0.32 * (0.6 + 0.8 * q as f64);
                let n = ((rad * rad * 1.5) as usize).clamp(8, 900);
                for i in 0..n {
                    let a = u(i as u64 * 3) * std::f64::consts::TAU;
                    let d = u(i as u64 * 3 + 1).sqrt() * rad;
                    let c = heat(q * 0.3 + 0.1 * u(i as u64 * 3 + 2) as f32);
                    block(&mut pos, &mut col, bm.x + a.cos() * d, bm.y + a.sin() * d, 1.0, [c[0], c[1], c[2], 1.0 - q * 0.5]);
                }
            }
            // The fire: blocks flung out, slowed by their own smoke, cooling.
            let fire = ((40.0 + size * 10.0) as usize).min(1400);
            for i in 0..fire {
                let k = 1000 + i as u64 * 5;
                let a = u(k) * std::f64::consts::TAU;
                let sp = size * (0.6 + 1.6 * u(k + 1)) * if u(k + 4) < 0.15 { 2.0 } else { 1.0 };
                let own = life * (0.35 + 0.65 * u(k + 2));
                if age > own {
                    continue;
                }
                let q = (age / own) as f32;
                // Distance with drag: quick out, then hanging.
                let dist = sp * (1.0 - (-age * 3.0).exp()) / 3.0 * 2.2;
                let rise = (age * age * 1.2) * (0.5 + u(k + 3)) * q as f64 * 3.0;
                let (x, y) = (bm.x + a.cos() * dist + ux * rise, bm.y + a.sin() * dist + uy * rise);
                let c = heat(q * 1.05);
                let m = (1.0 + 1.6 * u(k + 3) as f32) * (1.0 + 1.2 * q) * (size as f32 / 20.0).clamp(0.6, 2.5);
                let alpha = if q < 0.6 { 1.0 } else { 1.0 - (q - 0.6) / 0.4 };
                block(&mut pos, &mut col, x, y, m, [c[0], c[1], c[2], alpha * 0.95]);
            }
            // The shock: a ring of dust racing out and thinning.
            let ring_t = 0.9 + 0.04 * size;
            if age < ring_t {
                let q = (age / ring_t) as f32;
                let rad = size * 2.6 * (age / ring_t).powf(0.45);
                let n = ((rad * 1.2) as usize).clamp(16, 800);
                for i in 0..n {
                    let a = (i as f64 + u(5000 + i as u64)) / n as f64 * std::f64::consts::TAU;
                    let d = rad * (0.93 + 0.1 * u(6000 + i as u64));
                    let c = [0.62, 0.58, 0.52];
                    block(&mut pos, &mut col, bm.x + a.cos() * d, bm.y + a.sin() * d, 1.0, [c[0], c[1], c[2], (1.0 - q) * 0.55]);
                }
            }
            // Embers: arcing out and falling back, with short trails.
            let embers = ((14.0 + size * 2.5) as usize).min(400);
            for i in 0..embers {
                let k = 9000 + i as u64 * 5;
                let a = u(k) * std::f64::consts::TAU;
                let sp = size * (1.2 + 2.2 * u(k + 1)) + 10.0;
                let own = life * (0.5 + 0.5 * u(k + 2));
                for tr in 0..4 {
                    let tt = age - tr as f64 * 0.05;
                    if tt < 0.0 || tt > own {
                        continue;
                    }
                    let (vx, vy) = (a.cos() * sp, a.sin() * sp);
                    // Falling back toward the body.
                    let (x, y) = (bm.x + vx * tt - ux * 0.5 * g * tt * tt, bm.y + vy * tt - uy * 0.5 * g * tt * tt);
                    let q = (tt / own) as f32;
                    let c = heat(0.08 + q * 0.4);
                    block(&mut pos, &mut col, x, y, 0.7, [c[0], c[1], c[2], (1.0 - q) * (1.0 - tr as f32 * 0.22)]);
                }
            }
            // The swarm's lights from the wreckage, blinking out as they fall.
            let lights_n = ((size * 0.8) as usize).clamp(4, 160);
            for i in 0..lights_n {
                let k = 20000 + i as u64 * 4;
                let a = u(k) * std::f64::consts::TAU;
                let sp = size * (0.5 + 1.2 * u(k + 1)) + 4.0;
                let own = life * (0.6 + 0.4 * u(k + 2));
                if age > own {
                    continue;
                }
                let (x, y) = (bm.x + a.cos() * sp * age - ux * 0.5 * g * age * age, bm.y + a.sin() * sp * age - uy * 0.5 * g * age * age);
                let blink = if ((age * (3.0 + 4.0 * u(k + 3))) as i64) % 2 == 0 { 1.0 } else { 0.35 };
                let q = (age / own) as f32;
                block(&mut pos, &mut col, x, y, 0.8, [lc[0], lc[1], lc[2], blink * (1.0 - q)]);
            }
            budget = budget.saturating_sub((pos.len() - start) / 6);
        }
        if pos.is_empty() {
            pos.extend_from_slice(&[[0.0; 3]; 3]);
            col.extend_from_slice(&[[0.0; 4]; 3]);
        }
        mm.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        mm.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    }
    perf.quads = quads;
    perf.add(crate::log::P_DRONES, t0);
}


/// How long an explosion this big takes to burn out (seconds).
/// How long a buried blast's heat is seen glowing (s).
const BURIED_LIFE_S: f64 = 4.0;

fn boom_life(size_m: f32) -> f64 {
    (1.4 + 0.06 * size_m as f64).min(6.0)
}

/// How far a drone strays from its simulated path, for the look of a
/// swarm up close (drawing only; the simulation never sees it). Three
/// motions, each strongest where it fits:
/// - waiting at its flock, it circles the hauler in a loose cloud;
/// - flying, it weaves side to side, most in mid-flight, none at either end;
/// - cutting, it hovers and darts about the cell.
/// A slow flow field, the same for drones near each other, is added to all
/// of it, so the cloud boils as one rather than each drone jittering alone.
fn swirl(leg: &Leg, id: u64, t_game: f64, t: f64, x: f64, y: f64) -> (f64, f64) {
    let h = dft_sim::noise::hash(id, 0, 0, 0x5717);
    let u1 = dft_sim::noise::unit(h);
    let u2 = crate::rnd::unit(h, 0x9E37);
    let u3 = crate::rnd::unit(h, 0x51ED);
    // The shared flow: a slowly turning field over space.
    let fa = (x * 0.045 + t * 0.35).sin() + (y * 0.05 - t * 0.27).cos() + (x * 0.017 - y * 0.021 + t * 0.11).sin();
    let flow = (fa.cos() * 1.2, fa.sin() * 1.2);
    let (dx, dy) = (leg.to.0 - leg.from.0, leg.to.1 - leg.from.1);
    let dist = (dx * dx + dy * dy).sqrt();
    let k = if dist < 1e-6 { 1.0 } else { ((t_game - leg.t0).max(0.0) * leg.speed / dist).min(1.0) };
    let phase = u1 * std::f64::consts::TAU;
    let (ox, oy) = if leg.state == 0 {
        // A loose cloud about the hauler: each drone on its own slow orbit,
        // some close in, some further out.
        let r = 3.0 + 9.0 * u2 * u2;
        let w = (0.25 + 0.5 * u3) * if u1 < 0.5 { 1.0 } else { -1.0 };
        let a = phase + t * w;
        let wob = 1.0 + 0.25 * (t * (0.7 + u2) + phase).sin();
        (a.cos() * r * wob, a.sin() * r * wob * 0.8)
    } else if k < 1.0 {
        // In flight: weave across the line of flight, fading in and out.
        let env = (k * std::f64::consts::PI).sin();
        let amp = (dist * 0.08).min(4.0) * (0.4 + 0.6 * u2) * env;
        let s = (t * (1.5 + 2.0 * u3) + phase).sin() * amp;
        (-dy / dist * s, dx / dist * s)
    } else if leg.state == 1 {
        // Cutting: hovering and darting about the cell.
        let a = phase + t * (2.0 + 3.0 * u3);
        let r = 0.5 + 0.4 * (t * 5.0 + phase * 3.0).sin();
        (a.cos() * r, a.sin() * r)
    } else {
        (0.0, 0.0)
    };
    // Less of the shared flow while cutting, so drones stay at their cell.
    let f = if leg.state == 1 && k >= 1.0 { 0.2 } else { 1.0 };
    (ox + flow.0 * f, oy + flow.1 * f)
}
