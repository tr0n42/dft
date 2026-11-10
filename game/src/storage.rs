//! Storage drones: the big, slow machines in the mother swarm that hold the
//! stockpile.
//!
//! The stockpile is held compressed: KG_PER_HOLD_CELL of it to each cubic
//! metre of hold, about fifty times as dense as rock, so a planet's worth
//! fits in a sky of them rather than a second planet.
//!
//! None is built until there is something to hold. When the stockpile runs
//! past what the storage drones already hold, the swarm builds one to take
//! the overflow: a base one for a little, a bigger one for more (up to
//! MAX_CAP), each designed for what it carries. So the swarm fills with all
//! kinds of them: long low barges, squat stacks of bays, armoured blocks with
//! towers and pods, from a few dozen metres to a couple of hundred.
//!
//! Built like everything else in the world, of one-metre blocks with the
//! ground's grain, and utterly plain: armour, struts, holds of compressed
//! ore (in the colours of what was brought up when it was built, see
//! cargo.rs), vents, clamps, masts, and telltales in the swarm's light
//! colour (lights.rs). The swarm builds each block by block: a sleeve of
//! drones wheels round the space it will fill while carriers stream in from
//! the sleeve, each setting a block, which glows as it goes on.
//!
//! Each flies on its own and is heavy (the bigger, the slower to start and
//! stop): it picks a cruising speed now and then and takes its time getting
//! there, slows and turns short of the swarm's ends, and leans as it does.
//! It sits inside the swarm, behind its nearer layers.

use crate::Run;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

pub struct StoragePlugin;

impl Plugin for StoragePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, place);
    }
}

/// kg of stockpile in each cubic metre (cell) of hold: compressed, about
/// fifty times the density of rock.
pub const KG_PER_HOLD_CELL: f64 = 150_000.0;
/// The least a storage drone is built to hold, and the most.
pub const BASE_CAP: f64 = 20_000_000.0;
pub const MAX_CAP: f64 = 64.0 * BASE_CAP;
/// At most this many (past that, the last ones stand for more).
const MAX_SHOWN: usize = 240;
/// The gap between storage drones' lanes (m).
const LANE_GAP_M: f64 = 8.0;
/// Each courier carries this much from storage (kg).
const COURIER_KG: f64 = 20_000.0;
/// At most this many being built at once.
const MAX_BUILDING: usize = 3;
/// A base one's cruising speed (m/s, each picks its own up to this) and how
/// fast it can change speed (m/s²); bigger ones are slower at both.
const CRUISE: f64 = 4.0;
const ACCEL: f64 = 0.06;

#[derive(Clone, Copy, PartialEq)]
enum B {
    Air,
    Armour,
    Plate,
    Strut,
    Ore,
    Vent,
    Lamp,
    Dark,
}

/// A design: its size in cells and what each cell is (top row first).
struct Design {
    w: usize,
    h: usize,
    g: Vec<B>,
}

impl Design {
    fn at(&self, x: usize, y: usize) -> B {
        self.g[y * self.w + x]
    }
    fn fill(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, b: B) {
        for y in y0.max(0)..=y1.min(self.h as i64 - 1) {
            for x in x0.max(0)..=x1.min(self.w as i64 - 1) {
                self.g[y as usize * self.w + x as usize] = b;
            }
        }
    }
    fn set(&mut self, x: i64, y: i64, b: B) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.g[y as usize * self.w + x as usize] = b;
        }
    }
}

/// Design a storage drone to hold `cap` kg: its holds sized for that much
/// compressed ore, its hull, armour and fittings chosen from `seed`.
fn design(cap: f64, seed: u64) -> Design {
    let r = |salt: u64| crate::rnd::unit(seed, salt);
    let ri = |salt: u64, lo: i64, hi: i64| lo + (r(salt) * (hi - lo + 1) as f64).floor() as i64;
    let hold_cells = (cap / KG_PER_HOLD_CELL).max(60.0);
    // Its holds: long and low, or squarer, by temperament.
    let aspect = 1.8 + 2.4 * r(1);
    let hw = (hold_cells * aspect).sqrt().round().max(8.0) as i64;
    let hh = (hold_cells / hw as f64).round().max(4.0) as i64;
    // Armour thickens with size; fittings round it.
    let size = (hw * hh) as f64;
    let t = 2 + (size.sqrt() / 25.0) as i64;
    let top = if r(2) < 0.7 { ri(3, 2, 3 + (hh / 3).min(8)) } else { 0 };
    let bottom = 2;
    let pods = r(4) < 0.55;
    let side = if pods { ri(5, 3, 4 + (hh / 6).min(5)) } else { 1 };
    let w = (hw + 2 * t + 2 * side) as usize;
    let h = (hh + 2 * t + top + bottom) as usize;
    let mut d = Design { w, h, g: vec![B::Air; w * h] };
    let (bx0, by0) = (side, top);
    let (bx1, by1) = (w as i64 - 1 - side, h as i64 - 1 - bottom);
    // The box, its corners cut.
    d.fill(bx0, by0, bx1, by1, B::Armour);
    let cut = ri(6, 0, 1 + t);
    for k in 0..cut {
        for (cx, cy, sx, sy) in [(bx0, by0, 1, 1), (bx1, by0, -1, 1), (bx0, by1, 1, -1), (bx1, by1, -1, -1)] {
            for j in 0..(cut - k) {
                d.set(cx + sx * j, cy + sy * k, B::Air);
            }
        }
    }
    // Seams in the plating.
    let seams = ri(7, 0, 2);
    for s in 0..seams {
        let y = by0 + (t / 2).max(1) + s * (by1 - by0 - t);
        d.fill(bx0 + cut, y, bx1 - cut, y, B::Plate);
    }
    // The holds: compressed ore behind a cage of struts, in bays.
    let (ox0, oy0) = (bx0 + t, by0 + t);
    let (ox1, oy1) = (ox0 + hw - 1, oy0 + hh - 1);
    d.fill(ox0, oy0, ox1, oy1, B::Ore);
    let bay = ri(8, 4, 9);
    let mut x = ox0;
    while x <= ox1 {
        d.fill(x, oy0, x, oy1, B::Strut);
        x += bay;
    }
    d.fill(ox1, oy0, ox1, oy1, B::Strut);
    d.fill(ox0, oy0, ox1, oy0, B::Strut);
    d.fill(ox0, oy1, ox1, oy1, B::Strut);
    if hh > 14 && r(9) < 0.6 {
        let mut y = oy0 + bay + 2;
        while y < oy1 - 2 {
            d.fill(ox0, y, ox1, y, B::Strut);
            y += bay + 2;
        }
    }
    // Towers on top: blocks of armour, one with a mast, dark slots in them.
    if top > 0 {
        let towers = ri(10, 1, 3);
        for k in 0..towers {
            let tw = ri(20 + k as u64, 3, 4 + w as i64 / 6);
            let th = ri(30 + k as u64, 1, top);
            let tx = bx0 + cut + ((w as i64 - 2 * side - 2 * cut - tw).max(1) as f64 * r(40 + k as u64)) as i64;
            d.fill(tx, by0 - th, tx + tw - 1, by0 - 1, B::Armour);
            if tw > 4 {
                d.fill(tx + 1, by0 - th, tx + tw - 2, by0 - th, B::Dark);
            }
            if k == 0 {
                d.fill(tx + tw / 2, 0, tx + tw / 2, by0 - th - 1, B::Armour);
            }
        }
    }
    // Side pods (thrusters), or clamps at the corners.
    if pods {
        let ph = ri(11, 4, 4 + hh / 2);
        let py = by0 + (by1 - by0 - ph) / 2;
        d.fill(0, py, side - 1, py + ph, B::Armour);
        d.fill(w as i64 - side, py, w as i64 - 1, py + ph, B::Armour);
        d.fill(0, py + ph, side - 1, py + ph, B::Vent);
        d.fill(w as i64 - side, py + ph, w as i64 - 1, py + ph, B::Vent);
    } else {
        for (cx, cy) in [(bx0 - 1, by0 + cut), (bx1 + 1, by0 + cut), (bx0 - 1, by1 - cut - 2), (bx1 + 1, by1 - cut - 2)] {
            d.fill(cx, cy, cx, cy + 2, B::Armour);
        }
    }
    // Vents along its belly.
    let gap = ri(12, 3, 6);
    let mut x = bx0 + cut + 2;
    while x < bx1 - cut - 1 {
        d.fill(x, by1 + 1, x + 1, by1 + 1, B::Vent);
        x += gap + 2;
    }
    // Telltales: down the front, and along the top on big ones.
    let mut y = oy0 + 1;
    while y < oy1 {
        d.set(bx0 + 1, y, B::Lamp);
        d.set(bx0 + 1, y + 2, B::Dark);
        y += 4;
    }
    if w > 60 {
        let mut x = bx0 + cut + 4;
        while x < bx1 - cut - 4 {
            if d.at(x as usize, (by0 + 1) as usize) == B::Armour {
                d.set(x, by0 + 1, B::Lamp);
            }
            x += 9;
        }
    }
    d
}

/// A cheap per-cell noise in [0, 1).
fn grain(x: usize, y: usize, salt: u64) -> f64 {
    crate::rnd::unit(((x as u64) << 20) ^ y as u64, salt)
}

/// A design's picture (RGBA, top row first) with its holds packed with
/// `ore` (one colour per cell of the design, row by row), and its
/// telltales' mask (white where they are).
fn paint(d: &Design, ore: &[[u8; 3]]) -> (Vec<u8>, Vec<u8>) {
    let (w, h) = (d.w, d.h);
    let mut px = vec![0u8; w * h * 4];
    let mut lamps = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let b = d.at(x, y);
            let n = grain(x, y, 1);
            // Blocks at the bottom of a part sit in their own shadow, as the
            // ground's bevels do.
            let below = y + 1 < h && d.at(x, y + 1) == b;
            let shade = if below { 1.0 } else { 0.78 };
            let i = (y * w + x) * 4;
            let c: [f64; 3] = match b {
                B::Air => continue,
                B::Armour => {
                    let v = 34.0 + 10.0 * n;
                    [v, v + 3.0, v + 2.0]
                }
                B::Plate => {
                    let v = 27.0 + 6.0 * n;
                    [v, v + 2.0, v + 2.0]
                }
                B::Strut => {
                    let v = 22.0 + 6.0 * n;
                    [v, v + 2.0, v + 1.0]
                }
                B::Ore => {
                    let c = ore.get(y * w + x).copied().unwrap_or([60, 58, 54]);
                    [c[0] as f64, c[1] as f64, c[2] as f64]
                }
                B::Vent => [10.0, 12.0, 11.0],
                B::Dark => [14.0, 18.0, 15.0],
                B::Lamp => {
                    lamps[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
                    continue;
                }
            };
            px[i] = (c[0] * shade).clamp(0.0, 255.0) as u8;
            px[i + 1] = (c[1] * shade).clamp(0.0, 255.0) as u8;
            px[i + 2] = (c[2] * shade).clamp(0.0, 255.0) as u8;
            px[i + 3] = 255;
        }
    }
    (px, lamps)
}

fn picture(w: usize, h: usize, px: Vec<u8>) -> Image {
    let mut img = Image::new(Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 }, TextureDimension::D2, px, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD);
    img.sampler = bevy::image::ImageSampler::nearest();
    img
}

/// One drone: what it holds, its shape, its course, and its build.
struct Flight {
    cap: f64,
    w: usize,
    h: usize,
    /// How much bigger than a base one (by length): it is slower to turn.
    scale: f64,
    /// Angle round the body (radians), its speed along the arc (m/s,
    /// signed), the speed it is working toward, and when it next picks one.
    a: f64,
    v: f64,
    target: f64,
    next: f64,
    bank: f64,
    /// Its lane: the drones fly in lanes stacked one over another, from low
    /// in the swarm up and, when the swarm's lanes are full, on up above
    /// it; in a lane they keep clear of each other (passing only drones in
    /// other lanes). Its height over the swarm's middle line (m), easing
    /// toward its lane's.
    lane: usize,
    ro: f64,
    phase: f64,
    image: Handle<Image>,
    lamp_image: Handle<Image>,
    hull: Entity,
    lamps: Entity,
    seed: u64,
    /// When the swarm began building it (real seconds), and how long its
    /// build takes (a big one longer).
    born: f64,
    build_s: f64,
    /// While it is being built: its finished picture, the order its blocks
    /// go on in, and how many are on.
    full: Option<Vec<u8>>,
    order: Vec<u32>,
    built: usize,
    /// Its finished picture, its hold's cells from the top down (the order
    /// they empty in), and how full it is drawn.
    pic: Vec<u8>,
    ore_cells: Vec<u32>,
    shown_fill: f64,
}

/// An empty storage drone falling out of the swarm.
struct Fall {
    x: f64,
    y: f64,
    /// Speed toward the ground and along it (m/s), its turn and spin (rad, rad/s).
    vr: f64,
    vt: f64,
    turn: f64,
    spin: f64,
    w: usize,
    h: usize,
    flip: bool,
    hull: Entity,
    image: Handle<Image>,
    lamp_image: Handle<Image>,
}

/// A courier: a small drone carrying stock from a storage drone to where
/// it is wanted in the swarm.
struct Courier {
    from: (f64, f64),
    to: (f64, f64),
    t0: f64,
    dur: f64,
    cargo: [f32; 3],
    seed: u64,
}

impl Flight {
    /// The seconds of its build: the sleeve gathers, the blocks go on, the
    /// sleeve thins away.
    fn gather_s(&self) -> f64 {
        1.0
    }
    fn formed_s(&self) -> f64 {
        self.gather_s() + self.build_s
    }
    fn gone_s(&self) -> f64 {
        self.formed_s() + 4.0
    }
}

#[derive(Resource)]
struct Storage {
    falling: Vec<Fall>,
    couriers: Vec<Courier>,
    /// The stockpile last seen (to see it drawn on).
    prev_held: f64,
    prev_spent: f64,
    courier_debt: f64,
    flights: Vec<Flight>,
    made: u64,
    /// The sleeves of drones new ones are built in: the near side (over
    /// the drone, and the carriers) and the far side (behind it).
    cloud: Handle<Mesh>,
    cloud_back: Handle<Mesh>,
    /// The mother swarm it last saw (centre), to notice a new body.
    centre: f64,
    last: f64,
}

#[derive(Component)]
struct StorageDrone;

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>) {
    let mut m = Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD);
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3]);
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; 3]);
    let cloud = meshes.add(m.clone());
    commands.spawn((Mesh2d(cloud.clone()), MeshMaterial2d(materials.add(ColorMaterial::default())), Transform::from_xyz(0.0, 0.0, crate::swarmview::Z_INSIDE + 0.01), bevy::camera::visibility::NoFrustumCulling));
    let cloud_back = meshes.add(m);
    commands.spawn((Mesh2d(cloud_back.clone()), MeshMaterial2d(materials.add(ColorMaterial::default())), Transform::from_xyz(0.0, 0.0, crate::swarmview::Z_INSIDE - 0.01), bevy::camera::visibility::NoFrustumCulling));
    commands.insert_resource(Storage { falling: Vec::new(), couriers: Vec::new(), prev_held: 0.0, prev_spent: 0.0, courier_debt: 0.0, flights: Vec::new(), made: 0, cloud, cloud_back, centre: f64::NAN, last: 0.0 });
}

fn quad(pos: &mut Vec<[f32; 3]>, col: &mut Vec<[f32; 4]>, p: Vec2, hh: f32, c: Color) {
    let c = c.to_linear();
    pos.extend_from_slice(&[[p.x - hh, p.y - hh, 0.0], [p.x + hh, p.y - hh, 0.0], [p.x + hh, p.y + hh, 0.0], [p.x - hh, p.y - hh, 0.0], [p.x + hh, p.y + hh, 0.0], [p.x - hh, p.y + hh, 0.0]]);
    col.extend_from_slice(&[[c.red, c.green, c.blue, c.alpha]; 6]);
}

#[allow(clippy::too_many_arguments)]
fn place(
    mut commands: Commands,
    mut st: ResMut<Storage>,
    mut images: ResMut<Assets<Image>>,
    run: Option<Res<Run>>,
    view: Res<crate::view::View>,
    mut meshes: ResMut<Assets<Mesh>>,
    lights: Res<crate::lights::Lights>,
    swarm: Res<crate::swarmview::SwarmNow>,
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut q: Query<(&mut Transform, &mut Sprite, &mut Visibility), With<StorageDrone>>,
) {
    let Some(run) = run else { return };
    let real = time.elapsed_secs_f64();
    let sh = swarm.0;
    let (held, mix, heights, spent) = {
        let out = run.0.out.lock();
        (out.stats.matter[0], out.stats.recent_mix.clone(), out.heights.as_ref().map(|h| h.1.clone()), out.stats.stock_spent)
    };
    // The top of the ground in direction `a` (as last swept).
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
    let dt = (real - st.last).clamp(0.0, 0.25);
    st.last = real;
    let forget = |commands: &mut Commands, images: &mut Assets<Image>, f: Flight| {
        commands.entity(f.hull).despawn();
        commands.entity(f.lamps).despawn();
        images.remove(&f.image);
        images.remove(&f.lamp_image);
    };
    // A new body (or the mother swarm gone): start over.
    if (!st.centre.is_nan() && (st.centre - sh.centre).abs() > 1e-9) || !sh.exists() {
        for f in std::mem::take(&mut st.flights) {
            forget(&mut commands, &mut images, f);
        }
    }
    st.centre = sh.centre;
    let mix: crate::cargo::Mix = mix;
    if sh.exists() {
        // (Those there when the game starts were built long ago.)
        let first = st.flights.is_empty();
        loop {
            let holding: f64 = st.flights.iter().map(|f| f.cap).sum();
            let overflow = held - holding;
            let building = st.flights.iter().filter(|f| f.full.is_some()).count();
            if overflow < BASE_CAP * 0.5 || st.flights.len() >= MAX_SHOWN || (!first && building >= MAX_BUILDING) {
                break;
            }
            // Built for the overflow (at least a base one's worth).
            let cap = overflow.clamp(BASE_CAP, MAX_CAP);
            let k = st.made;
            st.made += 1;
            let seed = dft_sim::noise::hash(k, 7, 0, 0x5707);
            let d = design(cap, seed);
            let ore = crate::cargo::ore_cells(&mix, d.w, d.h, seed);
            let (full, lamp_px) = paint(&d, &ore);
            // Its hold, top row first (it empties from the top down).
            let ore_cells: Vec<u32> = (0..d.w * d.h).filter(|&i| d.at(i % d.w, i / d.w) == B::Ore).map(|i| i as u32).collect();
            let pic = full.clone();
            let order: Vec<u32> = {
                let mut o: Vec<(f64, u32)> = (0..d.w * d.h)
                    .filter(|&i| full[i * 4 + 3] > 0)
                    .map(|i| {
                        let (x, y) = ((i % d.w) as f64 + 0.5 - d.w as f64 / 2.0, (i / d.w) as f64 + 0.5 - d.h as f64 / 2.0);
                        let dd = ((x / (d.w as f64 / 2.0)).powi(2) + (y / (d.h as f64 / 2.0)).powi(2)).sqrt();
                        (dd + 0.4 * crate::rnd::unit(seed, i as u64 + 99), i as u32)
                    })
                    .collect();
                o.sort_by(|a, b| a.0.total_cmp(&b.0));
                o.into_iter().map(|e| e.1).collect()
            };
            let scale = ((d.w * d.h) as f64 / 576.0).sqrt().max(1.0);
            let image = images.add(picture(d.w, d.h, if first { full.clone() } else { vec![0u8; d.w * d.h * 4] }));
            let lamp_image = images.add(picture(d.w, d.h, lamp_px));
            let hull = commands.spawn((Sprite { image: image.clone(), custom_size: Some(Vec2::ONE), color: Color::srgb(0.82, 0.84, 0.83), ..default() }, Transform::from_xyz(0.0, 0.0, crate::swarmview::Z_INSIDE), Visibility::Hidden, StorageDrone)).id();
            let lamps = commands.spawn((Sprite { image: lamp_image.clone(), custom_size: Some(Vec2::ONE), color: lights.color().with_alpha(0.0), ..default() }, Transform::from_xyz(0.0, 0.0, crate::swarmview::Z_INSIDE + 0.001), Visibility::Hidden, StorageDrone)).id();
            let (u1, u2) = (crate::rnd::unit(seed, 1), crate::rnd::unit(seed, 2));
            // Its lane: the lowest with room along it; there, the middle of
            // the widest gap.
            let arc = if sh.whole { std::f64::consts::TAU } else { 2.0 * sh.half } * sh.alt.max(1.0);
            let lanes = st.flights.iter().map(|f| f.lane + 1).max().unwrap_or(0);
            let mut lane = lanes;
            for k in 0..lanes {
                let used: f64 = st.flights.iter().filter(|f| f.lane == k).map(|f| f.w as f64 + 20.0).sum();
                if (used + d.w as f64 + 20.0) / arc <= 0.45 {
                    lane = k;
                    break;
                }
            }
            let mut there: Vec<f64> = st.flights.iter().filter(|f| f.lane == lane).map(|f| f.a).collect();
            there.sort_by(f64::total_cmp);
            let a = if there.is_empty() {
                sh.centre - sh.half + 2.0 * sh.half * u1
            } else {
                // The widest gap between neighbours (round the end, on a ring).
                let (lo, hi) = (sh.centre - sh.half, sh.centre + sh.half);
                let mut best = (0.0, sh.centre);
                let mut edges = vec![lo];
                edges.extend(there.iter().copied());
                edges.push(hi);
                for w2 in edges.windows(2) {
                    if w2[1] - w2[0] > best.0 {
                        best = (w2[1] - w2[0], (w2[0] + w2[1]) / 2.0);
                    }
                }
                best.1
            };
            let n = order.len();
            st.flights.push(Flight {
                cap,
                w: d.w,
                h: d.h,
                scale,
                a,
                v: 0.0,
                target: 0.0,
                next: real,
                bank: 0.0,
                lane,
                ro: f64::NAN,
                phase: u2 * 6.28,
                image,
                lamp_image,
                hull,
                lamps,
                seed,
                born: if first { real - 1000.0 } else { real },
                build_s: (6.0 * scale.powf(0.6)).min(30.0),
                built: if first { n } else { 0 },
                full: if first { None } else { Some(full) },
                order,
                pic,
                ore_cells,
                shown_fill: 1.0,
            });
        }
        // Stock taken out (for building): couriers carry it from the
        // newest storage drone to where it is wanted in the swarm.
        let taken = if st.prev_spent > 0.0 { spent - st.prev_spent } else { 0.0 };
        // (One courier for every COURIER_KG taken.)
        st.courier_debt = (st.courier_debt + taken.max(0.0)).min(COURIER_KG * 48.0);
        if st.courier_debt >= COURIER_KG && !st.flights.is_empty() && !first {
            let n = ((st.courier_debt / COURIER_KG) as usize).min(24);
            st.courier_debt -= n as f64 * COURIER_KG;
            let (src_a, src_ro) = st.flights.last().map(|f| (f.a, f.ro)).unwrap();
            let (mid, _) = sh.at(src_a);
            let rr = mid + if src_ro.is_nan() { 0.0 } else { src_ro };
            let from = (src_a.cos() * rr, src_a.sin() * rr);
            for i in 0..n {
                let k = st.made.wrapping_mul(977) ^ ((real * 1000.0) as u64) ^ i as u64;
                let (u1, u2, u3) = (crate::rnd::unit(k, 1), crate::rnd::unit(k, 2), crate::rnd::unit(k, 3));
                let a = src_a + (u1 - 0.5) * 2.0 * 300.0 / sh.alt.max(1.0);
                let (m2, h2) = sh.at(a);
                let r2 = m2 + (u2 - 0.5) * h2;
                let to = (a.cos() * r2, a.sin() * r2);
                let dist = ((to.0 - from.0).powi(2) + (to.1 - from.1).powi(2)).sqrt();
                let cargo = mix.get((u3 * mix.len() as f64) as usize).map_or([0.4, 0.38, 0.35], |m| [m.0[0] as f32 / 255.0, m.0[1] as f32 / 255.0, m.0[2] as f32 / 255.0]);
                if st.couriers.len() < 600 {
                    st.couriers.push(Courier { from, to, t0: real + i as f64 * 0.08, dur: (dist / 28.0).max(1.5), cargo, seed: k });
                }
            }
        }
        // The newest one drawn down: its hold empties from the top. Empty,
        // it drops out of the swarm (and will hit the ground hard).
        loop {
            let holding: f64 = st.flights.iter().map(|f| f.cap).sum();
            let Some(f) = st.flights.last_mut() else { break };
            let left = held - (holding - f.cap);
            if left > 0.0 || f.full.is_some() {
                let fill = (left / f.cap).clamp(0.0, 1.0);
                if (fill - f.shown_fill).abs() > 0.01 && f.full.is_none() {
                    f.shown_fill = fill;
                    let gone = ((1.0 - fill) * f.ore_cells.len() as f64).round() as usize;
                    let mut px = f.pic.clone();
                    for &i in f.ore_cells.iter().take(gone) {
                        let i = i as usize * 4;
                        px[i..i + 4].copy_from_slice(&[12, 14, 13, 255]);
                    }
                    if let Some(mut img) = images.get_mut(&f.image) {
                        img.data = Some(px);
                    }
                }
                break;
            }
            let f = st.flights.pop().unwrap();
            let (mid, _) = sh.at(f.a);
            let rr = mid + if f.ro.is_nan() { 0.0 } else { f.ro };
            commands.entity(f.lamps).despawn();
            let turn = f.a - std::f64::consts::FRAC_PI_2;
            let spin = (crate::rnd::unit(f.seed, 77) - 0.5) * 0.3 / f.scale;
            st.falling.push(Fall { x: f.a.cos() * rr, y: f.a.sin() * rr, vr: 0.0, vt: f.v, turn, spin, w: f.w, h: f.h, flip: f.v < 0.0, hull: f.hull, image: f.image, lamp_image: f.lamp_image });
        }
    }
    // F9 (a cheat, for now): drop the newest storage drone out of the sky.
    if keys.just_pressed(KeyCode::F9) && sh.exists() {
        if let Some(i) = st.flights.iter().rposition(|f| f.full.is_none()) {
            let f = st.flights.remove(i);
            let (mid, _) = sh.at(f.a);
            let rr = mid + if f.ro.is_nan() { 0.0 } else { f.ro };
            commands.entity(f.lamps).despawn();
            let spin = (crate::rnd::unit(f.seed, 77) - 0.5) * 0.3 / f.scale;
            st.falling.push(Fall { x: f.a.cos() * rr, y: f.a.sin() * rr, vr: 0.0, vt: f.v, turn: f.a - std::f64::consts::FRAC_PI_2, spin, w: f.w, h: f.h, flip: f.v < 0.0, hull: f.hull, image: f.image, lamp_image: f.lamp_image });
        }
    }
    st.prev_held = held;
    st.prev_spent = spent;
    // The fallen: down they go, slowly turning, until they hit the ground
    // and blow (a crater for their size, their hulls in pieces).
    let mut landed = Vec::new();
    for (i, f) in st.falling.iter_mut().enumerate() {
        let rr = (f.x * f.x + f.y * f.y).sqrt().max(1.0);
        let (ux, uy) = (f.x / rr, f.y / rr);
        f.vr += 9.0 * dt;
        f.x += (-ux * f.vr + -uy * f.vt) * dt;
        f.y += (-uy * f.vr + ux * f.vt) * dt;
        f.turn += f.spin * dt;
        let a = f.y.atan2(f.x);
        if ground(a).is_some_and(|g| rr - (f.h as f64) * 0.4 <= g) || rr < 1.0 {
            landed.push(i);
        }
    }
    for i in landed.into_iter().rev() {
        let f = st.falling.swap_remove(i);
        let size = (f.w as f32 * 1.3).clamp(20.0, 220.0);
        run.0.blasts.lock().push(((f.x, f.y), size, ((f.w * f.h) as u32 / 30).clamp(10, 300)));
        commands.entity(f.hull).despawn();
        images.remove(&f.image);
        images.remove(&f.lamp_image);
    }
    // The lanes' heights: the lowest low in the swarm, each the next over
    // the last by the tallest drone in it and a gap.
    let lanes = st.flights.iter().map(|f| f.lane + 1).max().unwrap_or(0);
    let mut lane_h = vec![16.0f64; lanes];
    for f in st.flights.iter() {
        lane_h[f.lane] = lane_h[f.lane].max(f.h as f64);
    }
    let mut lane_off = vec![0.0f64; lanes];
    let mut y = -sh.h * 0.7;
    for k in 0..lanes {
        lane_off[k] = y + lane_h[k] / 2.0;
        y += lane_h[k] + LANE_GAP_M;
    }
    for f in st.flights.iter_mut() {
        let want = lane_off[f.lane];
        f.ro = if f.ro.is_nan() { want } else { f.ro + (want - f.ro) * (1.0 - (-dt / 6.0).exp()) };
    }
    // Who is where in each lane, this moment.
    let snap: Vec<(usize, f64, f64, f64)> = st.flights.iter().map(|f| (f.lane, f.a, f.v, f.w as f64)).collect();
    let wrap = |d: f64| {
        let mut d = d;
        while d > std::f64::consts::PI {
            d -= std::f64::consts::TAU;
        }
        while d < -std::f64::consts::PI {
            d += std::f64::consts::TAU;
        }
        d
    };
    // On their way, heavily.
    let (lo, hi) = (sh.centre - sh.half, sh.centre + sh.half);
    let alt = sh.alt.max(1.0);
    for (i, f) in st.flights.iter_mut().enumerate() {
        if real - f.born < f.formed_s() {
            f.next = f.next.max(f.born + f.formed_s());
            continue;
        }
        let (cruise, accel) = (CRUISE / f.scale.sqrt(), ACCEL / f.scale);
        if real >= f.next {
            let (u1, u2, u3) = (crate::rnd::unit(f.seed, f.next.to_bits()), crate::rnd::unit(f.seed, f.next.to_bits() ^ 1), crate::rnd::unit(f.seed, f.next.to_bits() ^ 2));
            f.target = cruise * u1 * u1 * if u2 < 0.5 { 1.0 } else { -1.0 };
            f.next = real + (20.0 + 50.0 * u3) * f.scale.sqrt();
        }
        if !sh.whole {
            let to_end = if f.v >= 0.0 { (hi - f.a) * alt } else { (f.a - lo) * alt };
            let stop = f.v * f.v / (2.0 * accel) + 40.0 + f.w as f64;
            if to_end < stop {
                f.target = -f.v.signum() * f.target.abs().max(0.3);
            }
        }
        // Keeping clear in its lane: the nearest ahead (the way it is
        // heading), if it can't stop short of it, it matches its pace or
        // turns back; overlapping (just built in its way), it eases off.
        let heading = if f.target.abs() > 1e-3 { f.target.signum() } else { f.v.signum() };
        let mut push = 0.0;
        for (j, &(lane, a2, v2, w2)) in snap.iter().enumerate() {
            if j == i || lane != f.lane {
                continue;
            }
            let d = wrap(a2 - f.a) * alt;
            let gap = d.abs() - (f.w as f64 + w2) / 2.0;
            if gap < 0.0 {
                push -= d.signum() * (-gap).min(4.0);
            }
            if d.signum() != heading || gap > 200.0 {
                continue;
            }
            let stop = f.v * f.v / (2.0 * accel) + 15.0;
            if gap < stop {
                f.target = if v2.signum() == heading && v2.abs() > 0.05 { heading * v2.abs().min(f.target.abs()) } else { -heading * f.target.abs().max(0.3) };
            }
        }
        f.a += push * dt * 0.5 / alt;
        let dv = (f.target - f.v).clamp(-accel * dt, accel * dt);
        f.v += dv;
        let lean = if dt > 0.0 { (dv / dt / accel) * 0.05 / f.scale.sqrt() } else { 0.0 };
        f.bank += (lean - f.bank) * (dt * 0.6).min(1.0);
        f.a += f.v * dt / alt;
        if !sh.whole {
            f.a = f.a.clamp(lo, hi);
        }
    }
    let lc = lights.rgb();
    // Building: the blocks due by now go on; the newest few glow in the
    // swarm's light colour.
    let spark = [(lc[0] * 255.0) as u8, (lc[1] * 255.0) as u8, (lc[2] * 255.0) as u8];
    for f in st.flights.iter_mut() {
        let Some(full) = &f.full else { continue };
        let q = ((real - f.born - f.gather_s()) / f.build_s).clamp(0.0, 1.0);
        let n = f.order.len();
        let due = ((q * q * (3.0 - 2.0 * q)) * n as f64).round() as usize;
        if due == f.built && due < n {
            continue;
        }
        let glow = (n / 60).clamp(6, 40);
        let mut px = vec![0u8; f.w * f.h * 4];
        for (k, &i) in f.order.iter().take(due).enumerate() {
            let i = i as usize * 4;
            if k + glow >= due && due < n {
                px[i..i + 4].copy_from_slice(&[spark[0], spark[1], spark[2], 255]);
            } else {
                px[i..i + 4].copy_from_slice(&full[i..i + 4]);
            }
        }
        if let Some(mut img) = images.get_mut(&f.image) {
            img.data = Some(px);
        }
        f.built = due;
        if due >= n {
            f.full = None;
        }
    }
    let v = *view;
    let (l, b, r, t) = v.bounds();
    // One texel a metre, as the ground (never smaller than half a pixel).
    // True to scale at every zoom (never under two pixels across).
    let px_per_m = (1.0 / v.mpp) as f32;
    let sized = |w: usize, h: usize| Vec2::new((w as f32 * px_per_m).max(2.0), (h as f32 * px_per_m).max(2.0 * h as f32 / w as f32));
    let knot_c = [0.36 * 0.72 + lc[0] * 0.55 * 0.28, 0.42 * 0.72 + lc[1] * 0.55 * 0.28, 0.39 * 0.72 + lc[2] * 0.55 * 0.28];
    let (mut kp, mut kc): (Vec<[f32; 3]>, Vec<[f32; 4]>) = (Vec::new(), Vec::new());
    let (mut bp, mut bc): (Vec<[f32; 3]>, Vec<[f32; 4]>) = (Vec::new(), Vec::new());
    let hh0 = ((2.0 / v.mpp) as f32).max(1.5) * 0.5;
    for f in st.flights.iter() {
        let lamp = lights.at(real, f.a);
        let age = real - f.born;
        let (mid, half) = sh.at(f.a);
        let _ = half;
        let rr = mid + if f.ro.is_nan() { 0.0 } else { f.ro } + (real * 0.07 + f.phase).sin() * 1.5;
        let (x, y) = (f.a.cos() * rr, f.a.sin() * rr);
        let size = sized(f.w, f.h);
        let margin = (f.w.max(f.h) as f64) * 1.5;
        let on = x > l - margin && x < r + margin && y > b - margin && y < t + margin;
        let s = v.to_screen(x, y);
        let turn = f.a - std::f64::consts::FRAC_PI_2 + f.bank * f.v.signum();
        let rot = Quat::from_rotation_z(turn as f32);
        let flip = f.v < 0.0;
        for (e, is_lamp) in [(f.hull, false), (f.lamps, true)] {
            let Ok((mut tf, mut sprite, mut vis)) = q.get_mut(e) else { continue };
            let want = if on { Visibility::Visible } else { Visibility::Hidden };
            if *vis != want {
                *vis = want;
            }
            if !on {
                continue;
            }
            tf.translation = Vec3::new(s.x.round(), s.y.round(), crate::swarmview::Z_INSIDE + if is_lamp { 0.001 } else { 0.0 });
            tf.rotation = rot;
            sprite.custom_size = Some(size);
            sprite.flip_x = flip;
            let want_c = if is_lamp { lamp.with_alpha(if f.full.is_none() { 1.0 } else { 0.0 }) } else { Color::srgb(0.82, 0.84, 0.83) };
            if sprite.color != want_c {
                sprite.color = want_c;
            }
        }
        if !on || age >= f.gone_s() {
            continue;
        }
        let (ca, sa) = (turn.cos(), turn.sin());
        let to_world = |lx: f64, ly: f64| (x + lx * ca - ly * sa, y + lx * sa + ly * ca);
        let (fw, fh) = (f.w as f64, f.h as f64);
        // The sleeve: drones wheeling round it as round a cylinder lying
        // along it, the near side passing over it, the far side behind; thin
        // enough to see the build through. After, it loosens and thins away.
        let gather = (age / f.gather_s()).clamp(0.0, 1.0);
        let after = ((age - f.formed_s()) / (f.gone_s() - f.formed_s())).clamp(0.0, 1.0);
        let count = ((850.0 * f.scale) * gather * (1.0 - after)).min(5000.0) as u64;
        let swell = 1.0 + 1.8 * after + 0.6 * (1.0 - gather);
        let dir = if flip { -1.0 } else { 1.0 };
        for i in 0..count {
            let (u1, u2, u3) = (crate::rnd::unit(f.seed, 3 * i + 1000), crate::rnd::unit(f.seed, 3 * i + 1001), crate::rnd::unit(f.seed, 3 * i + 1002));
            let lx = dir * ((u1 - 0.5) * fw * 1.15 + (real * (0.3 + 0.4 * u3) + u2 * 6.28).sin() * 1.2);
            let th = u2 * std::f64::consts::TAU + real * (1.6 + 0.9 * u3) / f.scale.sqrt();
            let along = (2.0 * lx.abs() / (fw * 1.15)).min(1.0);
            let rad = fh * 0.62 * (0.88 + 0.22 * u3) * swell * (1.0 - along.powi(6)).max(0.0).powf(0.3);
            let ly = th.sin() * rad;
            let near = th.cos() >= 0.0;
            let (wx, wy) = to_world(lx, ly);
            let p = v.to_screen(wx, wy);
            let shade = (if near { 0.6 + 0.3 * th.cos() } else { 0.45 }) as f32 * (0.85 + 0.3 * u1 as f32);
            let c = Color::srgba(knot_c[0] * shade, knot_c[1] * shade, knot_c[2] * shade, 1.0 - after as f32);
            let hh = hh0 * (0.9 + 0.4 * u3 as f32);
            if near {
                quad(&mut kp, &mut kc, p, hh, c);
            } else {
                quad(&mut bp, &mut bc, p, hh, c);
            }
        }
        // The carriers: drones bringing the next blocks in from the sleeve,
        // each flying to where its block goes on.
        if f.full.is_some() {
            let q = ((age - f.gather_s()) / f.build_s).clamp(0.0, 1.0);
            let n = f.order.len() as f64;
            let due_f = (q * q * (3.0 - 2.0 * q)) * n;
            let lane = (n / 40.0).clamp(12.0, 120.0);
            let carriers = lane as usize;
            let first = due_f.floor() as usize;
            for k in first..(first + carriers).min(f.order.len()) {
                // How far along its flight (1: there, its block goes on).
                let prog = (1.0 - (k as f64 - due_f) / lane).clamp(0.0, 1.0);
                let cell = f.order[k] as usize;
                let (cx, cy) = ((cell % f.w) as f64 + 0.5 - fw / 2.0, fh / 2.0 - ((cell / f.w) as f64 + 0.5));
                let cx = if flip { -cx } else { cx };
                // From a place on the sleeve.
                let th = crate::rnd::unit(f.seed, k as u64 * 7 + 5) * std::f64::consts::TAU;
                let (sx, sy) = ((crate::rnd::unit(f.seed, k as u64 * 7 + 6) - 0.5) * fw * 1.1, th.sin() * fh * 0.62);
                let e = prog * prog * (3.0 - 2.0 * prog);
                let (lx, ly) = (sx + (cx - sx) * e, sy + (cy - sy) * e);
                let (wx, wy) = to_world(lx, ly);
                let p = v.to_screen(wx, wy);
                let c = Color::srgb(0.5 + lc[0] * 0.5, 0.5 + lc[1] * 0.5, 0.5 + lc[2] * 0.5);
                quad(&mut kp, &mut kc, p, hh0 * 0.8, c);
            }
        }
    }
    // The fallen, as they go down.
    for f in st.falling.iter() {
        let Ok((mut tf, mut sprite, mut vis)) = q.get_mut(f.hull) else { continue };
        let on = f.x > l - 200.0 && f.x < r + 200.0 && f.y > b - 200.0 && f.y < t + 200.0;
        let want = if on { Visibility::Visible } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
        let s = v.to_screen(f.x, f.y);
        tf.translation = Vec3::new(s.x.round(), s.y.round(), 9.9);
        tf.rotation = Quat::from_rotation_z(f.turn as f32);
        sprite.custom_size = Some(sized(f.w, f.h));
        sprite.flip_x = f.flip;
    }
    // Couriers on their way: small blocky drones, light and cargo showing.
    st.couriers.retain(|c| real < c.t0 + c.dur);
    let cell = (0.8 * px_per_m).max(1.0);
    for c in st.couriers.iter() {
        let q = ((real - c.t0) / c.dur).clamp(0.0, 1.0);
        if real < c.t0 {
            continue;
        }
        let e = q * q * (3.0 - 2.0 * q);
        // A gentle arc, not a ruler line.
        let bow = (q * std::f64::consts::PI).sin() * 12.0 * (crate::rnd::unit(c.seed, 9) - 0.5);
        let (dx, dy) = (c.to.0 - c.from.0, c.to.1 - c.from.1);
        let d = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (x, y) = (c.from.0 + dx * e - dy / d * bow, c.from.1 + dy * e + dx / d * bow);
        if x < l - 10.0 || x > r + 10.0 || y < b - 10.0 || y > t + 10.0 {
            continue;
        }
        let p = v.to_screen(x, y);
        let hull = Color::srgb(0.42, 0.45, 0.44);
        let lamp = lights.at(real, x.atan2(y));
        for (ox, oy, k) in [(-0.5f32, 0.5f32, hull), (0.5, 0.5, lamp), (-0.5, -0.5, Color::srgb(c.cargo[0], c.cargo[1], c.cargo[2])), (0.5, -0.5, hull)] {
            quad(&mut kp, &mut kc, Vec2::new(p.x + ox * cell, p.y + oy * cell), cell * 0.5, k);
        }
    }
    for (h, mut p, mut c) in [(st.cloud.clone(), kp, kc), (st.cloud_back.clone(), bp, bc)] {
        if let Some(mut m) = meshes.get_mut(&h) {
            if p.is_empty() {
                p.extend_from_slice(&[[0.0; 3]; 3]);
                c.extend_from_slice(&[[0.0; 4]; 3]);
            }
            m.insert_attribute(Mesh::ATTRIBUTE_POSITION, p);
            m.insert_attribute(Mesh::ATTRIBUTE_COLOR, c);
        }
    }
}
