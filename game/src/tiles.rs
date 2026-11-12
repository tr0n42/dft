//! Tiles: the body cut into squares of 128 x 128 texels, at levels of detail.
//! A tile at level L covers 64·2^L metres a side, one texel per 2^L cells
//! (sampled at the middle of its block). Tiles are built on worker threads,
//! nearest the middle of the view first, and kept in a cache; coarser
//! stand-ins are drawn underneath while finer ones arrive, so the view never
//! shows holes.
//!
//! Where the swarm has changed the ground, a tile reads the running world
//! (briefly holding it; the simulation waits); everywhere else it asks the
//! generator, which needs no lock at all. When the simulation reports that
//! blocks changed, the tiles over them are rebuilt, and the old picture stays
//! up until the new one is ready.
use crate::sim::Shared;
use crate::view::View;
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
use dft_sim::materials::{MatId, VACUUM};
use dft_sim::planet::Planet;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

/// Tile-building totals for the log (worker threads add to them): builds,
/// nanoseconds building, builds that read the running world, and nanoseconds
/// those spent waiting for it and holding it.
pub static BUILDS: AtomicU64 = AtomicU64::new(0);
pub static BUILD_NS: AtomicU64 = AtomicU64::new(0);
pub static LIVE_READS: AtomicU64 = AtomicU64::new(0);
pub static WAIT_NS: AtomicU64 = AtomicU64::new(0);
pub static HOLD_NS: AtomicU64 = AtomicU64::new(0);

/// Texels across a tile. Bigger tiles mean fewer sprites to draw (each tile
/// is its own draw, and a few hundred of them cost the GPU side more than
/// anything else on screen); smaller ones, less to rebuild per change.
pub const TEX: i64 = 128;
/// How many tiles may be building at once.
const IN_FLIGHT: usize = 48;
/// How many tiles to keep before forgetting the least recently seen.
const KEEP: usize = 2500;
/// The deepest level kept track of.
const LEVELS: usize = 42;
/// How soon a changed tile may be rebuilt again, in seconds (a tile under
/// busy digging would otherwise be rebuilt every frame). Stand-ins are hidden
/// under finer tiles most of the time and wait longer.
const REBUILD_S: f64 = 0.1;
const REBUILD_UNDER_S: f64 = 1.0;
/// Tiles just off screen (built ahead so panning shows no pop) are rebuilt
/// less often while the ground under them keeps changing.
const REBUILD_AHEAD_S: f64 = 0.5;
/// The atmosphere follows the ground down once it has been eaten more than
/// this far below where it stood (less, and the old surface is kept: the
/// published heights are too coarse to follow small dips).
const AIR_SINK_M: f64 = 8.0;
/// How far past the screen tiles are built ahead: this many screen widths
/// (and heights) of extra ground around the view, half on each side.
const AHEAD_SCREENS: f64 = 1.0;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Key {
    /// Texel size is 2^level cells: negative levels are zoomed in, several
    /// texels to a cell (where blocks show their textures).
    pub level: i8,
    pub ix: i64,
    pub iy: i64,
    /// A stand-in: drawn under finer tiles while they arrive, so it keeps
    /// only ground safely below the surface and never pokes out past it.
    pub under: bool,
}

/// The simulation reports changes in blocks of 2^6 = 64 cells. `touched`
/// and `areas` are kept in those units, by block level (a block level b
/// region is 64 << b cells across); a tile at level l spans TEX << l cells,
/// which is block level l + TILE_TO_BLOCK.
const BLOCK_SHIFT: i64 = 6;
const TILE_TO_BLOCK: i64 = TEX.trailing_zeros() as i64 - BLOCK_SHIFT;

/// Plants are drawn on tiles finer than this level (texels under 16 m).
const FLORA_LEVEL: i8 = 4;

/// Plants sway on tiles this fine or finer (two texels a cell and up): they
/// are left out of the tile's picture and drawn by sway.rs over it.
pub const SWAY_LEVEL: i8 = -1;

/// A plant cell drawn swaying: where it is, its colour, how far it is up
/// its plant (cells), and how it moves.
#[derive(Clone, Copy, Debug)]
pub struct SwayCell {
    pub x: i64,
    pub y: i64,
    pub c: [u8; 3],
    pub h: f32,
    /// 1 sway, 2 ripple, 3 drift (as flora::Motion).
    pub motion: u8,
    pub amp: f32,
    pub leaf: bool,
}

/// Per material: how its cells sway (None: they don't), and whether it is
/// plant tissue at all.
struct SwayTable {
    kind: Vec<Option<(u8, f32, bool, bool)>>,
    tissue: Vec<bool>,
}

fn sway_table(p: &Planet) -> Option<SwayTable> {
    use dft_sim::flora::{Motion, Niche};
    let f = p.flora.as_ref()?;
    let n = p.mats.len();
    let mut t = SwayTable { kind: vec![None; n], tissue: vec![false; n] };
    for sp in &f.species {
        let code = match sp.motion {
            Motion::Rigid => 0,
            Motion::Sway => 1,
            Motion::Ripple => 2,
            Motion::Drift => 3,
        };
        for (k, m) in [sp.stem_m, sp.foliage_m, sp.organ_m, sp.root_m].into_iter().enumerate() {
            if (m as usize) >= n || m == VACUUM {
                continue;
            }
            t.tissue[m as usize] = true;
            // Roots are in the ground: they don't move.
            if code != 0 && k != 3 {
                t.kind[m as usize] = Some((code, sp.sway as f32, sp.niche == Niche::CaveRoof, k == 1));
            }
        }
    }
    Some(t)
}

/// The finest level: 16 texels to a cell.
const FINEST: i8 = -4;

/// Cells a tile at `level` covers across.
fn span_cells(level: i8) -> i64 {
    if level >= 0 { TEX << level } else { TEX >> -level }
}

impl Key {
    pub fn span(&self) -> f64 {
        span_cells(self.level) as f64
    }
    /// The cell rectangle it covers: (x0, x1, y0, y1), ends exclusive.
    fn rect(&self) -> [i64; 4] {
        let s = span_cells(self.level);
        [self.ix * s, self.ix * s + s, self.iy * s, self.iy * s + s]
    }
    /// Whether any direction round the body it covers is marked in `moved`
    /// (one flag per equal slice of the full turn).
    fn faces_moved(&self, moved: &[bool]) -> bool {
        let n = moved.len();
        if n == 0 {
            return false;
        }
        let [x0, x1, y0, y1] = self.rect();
        // Holding the body's centre: every direction.
        if x0 <= 0 && x1 >= 0 && y0 <= 0 && y1 >= 0 {
            return moved.iter().any(|&m| m);
        }
        let tau = std::f64::consts::TAU;
        let mut a: Vec<f64> = [(x0, y0), (x1, y0), (x0, y1), (x1, y1)].iter().map(|&(x, y)| (y as f64).atan2(x as f64).rem_euclid(tau)).collect();
        a.sort_by(f64::total_cmp);
        // The smallest arc holding all four corners (it may wrap past 0).
        let mut gap = (a[0] + tau - a[3], 3usize);
        for i in 0..3 {
            if a[i + 1] - a[i] > gap.0 {
                gap = (a[i + 1] - a[i], i);
            }
        }
        let (from, to) = if gap.1 == 3 { (a[0], a[3]) } else { (a[gap.1 + 1], a[gap.1] + tau) };
        let (i0, i1) = ((from / tau * n as f64).floor() as i64 - 1, (to / tau * n as f64).ceil() as i64 + 1);
        (i0..=i1).any(|i| moved[i.rem_euclid(n as i64) as usize])
    }
    /// The 64-cell block it lies in (tiles no bigger than a block).
    fn block(&self) -> (i64, i64) {
        let sh = (BLOCK_SHIFT - span_cells(self.level).trailing_zeros() as i64).max(0);
        (self.ix >> sh, self.iy >> sh)
    }
}

/// What a tile shows now.
enum Shown {
    /// Nothing yet: its first build hasn't finished.
    Pending,
    /// Nothing to draw (all open space).
    Empty,
    Sprite(Entity, Handle<Image>),
}

struct Slot {
    shown: Shown,
    task: Option<Task<Option<(Vec<u8>, bool, Vec<SwayCell>)>>>,
    /// Its plants drawn swaying (left out of its picture).
    plants: Arc<Vec<SwayCell>>,
    /// It shows some of the backdrop behind eaten ground (redrawn as the
    /// backdrop collapses).
    ghost: bool,
    /// The ground under it changed since its last build started.
    stale: bool,
    /// When its last build started (seconds since the game began), when
    /// the build of the picture it shows now started, and when the ground
    /// under it last changed.
    built_at: f64,
    image_from: f64,
    changed_at: f64,
    /// The last frame the view wanted it.
    seen: u64,
    /// The view stopped wanting it for a while and came back: the ground may
    /// have changed under it unseen, so if it did, its old picture isn't
    /// shown (a stand-in is) until the rebuild lands. A tile the view kept
    /// wanting just keeps its picture until then, so it never blinks.
    away: bool,
}

#[derive(Resource, Default)]
pub struct Tiles {
    slots: HashMap<Key, Slot>,
    frame: u64,
    building: usize,
    /// Tiles the current view still needs.
    missing: usize,
    /// Tiles the view wants this frame (on screen, stand-ins and the ring
    /// built ahead): the budget never drops below a margin over this, so a
    /// big window doesn't forget tiles it is about to draw again.
    wanted_n: usize,
    level: i8,
    /// The backdrop's current collapse radius (see `collapse_radius`).
    collapse: f64,
    /// The ground's height round the body as last published, and its sweep.
    heights: Option<(u64, Arc<Vec<f32>>)>,
    /// The heights the tiles were last brought up to date with.
    heights_seen: Option<Arc<Vec<f32>>>,
    /// When each 64-cell block last changed (seconds): zoomed-in tiles, many
    /// to a block, check it rather than being marked one by one.
    block_changed: HashMap<(i64, i64), f64>,
    /// A game loaded from a save: the ground may have changed anywhere, so
    /// every tile reads the running world.
    pub all_touched: bool,
    /// Every tile (by level) the swarm has ever changed anything under; the
    /// rest are exactly as generated.
    touched: Vec<HashSet<(i64, i64)>>,
    /// Whole areas emptied at once, noted at the first level where a few
    /// tiles cover them: every finer tile inside counts as touched.
    areas: Vec<HashSet<(i64, i64)>>,
}

impl Tiles {
    /// Everything the view needs is drawn and up to date.
    pub fn settled(&self) -> bool {
        self.building == 0 && self.missing == 0
    }

    /// For the dev panel: tiles kept, tiles building, the level drawn.
    pub fn counts(&self) -> (usize, usize, i8) {
        (self.slots.len(), self.building, self.level)
    }

    /// Forget every tile (a new body).
    pub fn clear(&mut self, commands: &mut Commands) {
        for (_, s) in self.slots.drain() {
            if let Shown::Sprite(e, _) = s.shown {
                commands.entity(e).despawn();
            }
        }
        *self = Tiles::default();
    }

    fn is_touched(&self, k: Key) -> bool {
        if self.all_touched {
            return true;
        }
        if self.touched.is_empty() {
            return false;
        }
        // (In block units: a tile no bigger than a block, by its block.)
        let b = k.level as i64 + TILE_TO_BLOCK;
        let (l, ix, iy) = if b <= 0 { (0usize, k.block().0, k.block().1) } else { (b as usize, k.ix, k.iy) };
        if l >= LEVELS {
            return false;
        }
        self.touched[l].contains(&(ix, iy)) || (l..LEVELS).any(|a| self.areas[a].contains(&(ix >> (a - l), iy >> (a - l))))
    }

    /// Take what the simulation says changed: note it, and mark the tiles over it.
    fn changed(&mut self, blocks: HashSet<(i64, i64)>, rects: Vec<[i64; 4]>, fine: HashSet<(i64, i64)>, now: f64) {
        if self.touched.is_empty() {
            self.touched = vec![HashSet::new(); LEVELS];
            self.areas = vec![HashSet::new(); LEVELS];
        }
        // Zoomed-in tiles: only those over 8 x 8-cell squares whose cells
        // actually changed in the screen's copy are redrawn (marking every
        // tile in a changed 64-cell block redrew dozens for each cell cut).
        for (sx, sy) in fine {
            let (cx, cy) = (sx * 8, sy * 8);
            for l in FINEST..0 {
                let span = span_cells(l);
                let at = (cx.div_euclid(span), cy.div_euclid(span));
                for under in [false, true] {
                    if let Some(s) = self.slots.get_mut(&Key { level: l, ix: at.0, iy: at.1, under }) {
                        s.stale = true;
                        s.changed_at = now;
                    }
                }
            }
        }
        for (bx, by) in blocks {
            self.block_changed.insert((bx, by), now);
            for l in 0..LEVELS {
                let at = (bx >> l, by >> l);
                self.touched[l].insert(at);
                // The tile at this block level (zoomed-in ones are marked
                // from the finer squares above).
                let tl = l as i64 - TILE_TO_BLOCK;
                if tl < 0 {
                    continue;
                }
                for under in [false, true] {
                    if let Some(s) = self.slots.get_mut(&Key { level: tl as i8, ix: at.0, iy: at.1, under }) {
                        s.stale = true;
                        s.changed_at = now;
                    }
                }
            }
        }
        for r in rects {
            for (k, s) in self.slots.iter_mut() {
                let [x0, x1, y0, y1] = k.rect();
                if r[0] < x1 && x0 < r[1] && r[2] < y1 && y0 < r[3] {
                    s.stale = true;
                    s.changed_at = now;
                }
            }
            // The first level where at most 2 x 2 tiles cover it.
            let tiles_at = |l: usize| (((r[1] - 1) >> (6 + l)) - (r[0] >> (6 + l)) + 1, ((r[3] - 1) >> (6 + l)) - (r[2] >> (6 + l)) + 1);
            let mut lr = 0;
            while lr + 1 < LEVELS && { let (a, b) = tiles_at(lr); a > 2 || b > 2 } {
                lr += 1;
            }
            for l in lr..LEVELS {
                for iy in (r[2] >> (6 + l))..=((r[3] - 1) >> (6 + l)) {
                    for ix in (r[0] >> (6 + l))..=((r[1] - 1) >> (6 + l)) {
                        if l == lr {
                            self.areas[l].insert((ix, iy));
                        }
                        self.touched[l].insert((ix, iy));
                    }
                }
            }
        }
    }
}

#[derive(Component)]
pub struct Tile(pub Key);

pub struct TilesPlugin;

impl Plugin for TilesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tiles>().init_resource::<crate::sway::Shown>().add_systems(Update, (take_changes, request, collect, place, crate::sway::draw).chain());
    }
}

/// The finest level worth drawing at this zoom: a texel at least 3/4 of a
/// pixel (sharp, so detail doesn't arrive in visible jumps while zooming).
fn level_for(mpp: f64) -> i8 {
    let mut l = FINEST;
    while 2f64.powi(l as i32) < mpp * 0.75 && l < 40 {
        l += 1;
    }
    l
}

/// The coarsest level: one tile covers the whole body.
fn top_level(p: &Planet) -> i8 {
    let d = 2.0 * (p.max_radius + p.spec.atmosphere_m);
    let mut l = 0i8;
    while ((TEX << l) as f64) < d && l < 40 {
        l += 1;
    }
    l
}

fn keys_in_view(view: &View, level: i8, margin: i64, under: bool) -> Vec<Key> {
    let s = span_cells(level) as f64;
    let (l, b, r, t) = view.bounds();
    let (x0, x1) = ((l / s).floor() as i64 - margin, (r / s).floor() as i64 + margin);
    let (y0, y1) = ((b / s).floor() as i64 - margin, (t / s).floor() as i64 + margin);
    let mut out = Vec::with_capacity(((x1 - x0 + 1) * (y1 - y0 + 1)) as usize);
    for iy in y0..=y1 {
        for ix in x0..=x1 {
            out.push(Key { level, ix, iy, under });
        }
    }
    // Nearest the middle of the view first.
    let (cx, cy) = (view.x / s - 0.5, view.y / s - 0.5);
    out.sort_by(|a, b| {
        let da = (a.ix as f64 - cx).powi(2) + (a.iy as f64 - cy).powi(2);
        let db = (b.ix as f64 - cx).powi(2) + (b.iy as f64 - cy).powi(2);
        da.partial_cmp(&db).unwrap()
    });
    out
}

/// Build a tile's pixels (RGBA, top row first), or None if it's all empty
/// space. `live`: the running world, read where the swarm has been.
fn build(p: &Planet, k: Key, live: Option<Arc<Shared>>, collapse: f64, heights: Option<Arc<Vec<f32>>>) -> Option<(Vec<u8>, bool, Vec<SwayCell>)> {
    let t0 = std::time::Instant::now();
    let r = build_inner(p, k, live, collapse, heights.as_deref().map(|v| v.as_slice()));
    BUILDS.fetch_add(1, Relaxed);
    BUILD_NS.fetch_add(t0.elapsed().as_nanos() as u64, Relaxed);
    r
}

/// How a cell with a look is coloured, as the sapper proving ground colours
/// what a blast leaves: cracks dark; pieces held together a little dark;
/// rubble a little darker, block by block; rock baked short of melting
/// changed (limestone burned to quicklime, sandstone glassy green, the rest
/// reddened and darkened); melt glowing; melt cooled to glass, near black
/// (green from sandstone).
fn look_tint(p: &Planet, m: MatId, c: [u8; 3], look: u8, cx: i64, cy: i64) -> [u8; 3] {
    use dft_sim::world::Look;
    let f = |c: [u8; 3], k: f32| [(c[0] as f32 * k) as u8, (c[1] as f32 * k) as u8, (c[2] as f32 * k) as u8];
    let mix = |a: [u8; 3], b: [f32; 3], t: f32| [(a[0] as f32 + (b[0] - a[0] as f32) * t) as u8, (a[1] as f32 + (b[1] - a[1] as f32) * t) as u8, (a[2] as f32 + (b[2] - a[2] as f32) * t) as u8];
    let comp = &p.mats.get(m).composition;
    let share = |e: &str| comp.get(e).copied().unwrap_or(0.0);
    let n = dft_sim::noise::unit(dft_sim::noise::hash(0x10C, cx, cy, 3)) as f32;
    match look {
        Look::CRACKED => f(c, 0.45),
        Look::HELD => f(c, 0.82),
        Look::RUBBLE => f(c, 0.75 + 0.25 * n),
        Look::BAKED => {
            let b = if share("Ca") > 0.15 {
                mix(c, [228.0, 224.0, 214.0], 0.85)
            } else if share("Si") > 0.38 {
                mix(c, [150.0, 176.0, 150.0], 0.6)
            } else {
                [(c[0] as f32 * 0.85 + 12.0).min(255.0) as u8, (c[1] as f32 * 0.68) as u8, (c[2] as f32 * 0.66) as u8]
            };
            f(b, 0.9 + 0.2 * n)
        }
        Look::MELT => {
            let k = 0.85 + 0.15 * n;
            [(255.0 * k) as u8, (150.0 * k) as u8, (55.0 * k) as u8]
        }
        Look::GLASS => {
            let g = if share("Si") > 0.38 && share("Ca") < 0.1 && share("Fe") < 0.05 { [96.0, 140.0, 116.0] } else { [30.0, 28.0, 36.0] };
            f(mix(c, g, 0.85), 0.85 + 0.3 * n)
        }
        _ => c,
    }
}

/// The backdrop's collapse: as the body is eaten, the ghost of what was
/// there crumbles away from the outside in, in blocky debris, so a body
/// eaten whole leaves only the stars. `collapse` is the radius it has
/// shrunk to (metres); the band it crumbles over is a share of the body.
const CRUMBLE: f64 = 0.12;

/// Draw the ghost backdrop behind eaten ground (off for now: it doesn't
/// match the ground's sharpness; eaten ground shows the starfield). See
/// "Backdrop behind eaten ground" in ROADMAP.md.
const GHOST: bool = false;

/// The ghost's radius when `eaten` (0 to 1) of the body is gone, in steps
/// (tiles are redrawn only when it moves a step).
pub fn collapse_radius(p: &Planet, eaten: f64) -> f64 {
    let full = p.max_radius * (1.0 + CRUMBLE);
    let r = full * (1.0 - eaten.clamp(0.0, 1.0)).sqrt();
    (r / full * 400.0).ceil() / 400.0 * full
}

fn build_inner(p: &Planet, k: Key, live: Option<Arc<Shared>>, collapse: f64, heights: Option<&[f32]>) -> Option<(Vec<u8>, bool, Vec<SwayCell>)> {
    use dft_sim::texture::{self as tx, Sides};
    let span = span_cells(k.level);
    // Texel size in cells, and texels per cell.
    let s = 2f64.powi(k.level as i32);
    let ppc = 1.0 / s;
    let (x0, y0) = (k.ix * span, k.iy * span);
    // Skip tiles wholly beyond the atmosphere.
    let reach = p.max_radius + p.spec.atmosphere_m + 2.0;
    let near_x = 0f64.clamp(x0 as f64, (x0 + span) as f64);
    let near_y = 0f64.clamp(y0 as f64, (y0 + span) as f64);
    if near_x * near_x + near_y * near_y > reach * reach {
        return None;
    }
    let kinds = tx::kinds(&p.mats);
    // The cells this tile needs: one per texel when zoomed out (sampled at the
    // middle of its block); when zoomed in, every cell it covers and a ring
    // around (the bevels look at neighbours).
    let fine = k.level < 0;
    let (gx0, gy0, gn) = if fine { (x0 - 1, y0 - 1, span + 2) } else { (0, 0, TEX) };
    let cell_of = |i: i64, j: i64| -> (i64, i64) {
        if fine { (gx0 + i, gy0 + j) } else { (x0 + i * (1i64 << k.level) + (1i64 << k.level) / 2, y0 + j * (1i64 << k.level) + (1i64 << k.level) / 2) }
    };
    // What the swarm has changed, from the screen's own copy of the dug
    // ground (never the running world: drawing doesn't wait on it).
    let mut looks: Vec<u8> = Vec::new();
    let stored: Option<Vec<Option<MatId>>> = live.map(|live| {
        let tw = std::time::Instant::now();
        let mirror = live.mirror.read();
        let th = std::time::Instant::now();
        let mut v = Vec::with_capacity((gn * gn) as usize);
        for j in 0..gn {
            for i in 0..gn {
                let (x, y) = cell_of(i, j);
                v.push(mirror.get(x, y));
            }
        }
        // A blast's marks on the cells (see dft_sim::world::Look).
        let mut any = false;
        let (ax, ay) = cell_of(0, 0);
        let (bx, by) = cell_of(gn - 1, gn - 1);
        mirror.looks_in(ax, bx, ay, by, |_, _| any = true);
        if any {
            looks = vec![0; (gn * gn) as usize];
            for j in 0..gn {
                for i in 0..gn {
                    let (x, y) = cell_of(i, j);
                    looks[(j * gn + i) as usize] = mirror.look(x, y);
                }
            }
        }
        drop(mirror);
        LIVE_READS.fetch_add(1, Relaxed);
        WAIT_NS.fetch_add((th - tw).as_nanos() as u64, Relaxed);
        HOLD_NS.fetch_add(th.elapsed().as_nanos() as u64, Relaxed);
        v
    });
    // Far out (texels of 16 m and more) plants are smaller than a texel:
    // leave them out rather than grow every forest on the body to draw them.
    let coarse = k.level >= FLORA_LEVEL;
    let mut cells = Vec::with_capacity((gn * gn) as usize);
    for j in 0..gn {
        for i in 0..gn {
            let (x, y) = cell_of(i, j);
            cells.push(stored.as_ref().and_then(|v| v[(j * gn + i) as usize]).unwrap_or_else(|| if coarse { p.base_material(x, y) } else { p.material(x, y) }));
        }
    }
    // A cell by world position (fine tiles only: the grid holds them all).
    let at = |cx: i64, cy: i64| -> MatId { cells[((cy - gy0) * gn + (cx - gx0)) as usize] };
    // Plants that sway are drawn apart, over the tile.
    let swaying = if fine && k.level <= SWAY_LEVEL && !k.under { sway_table(p) } else { None };
    let mut plants = Vec::new();
    if let Some(tab) = &swaying {
        let inside = |x: i64, y: i64| x >= gx0 && x < gx0 + gn && y >= gy0 && y < gy0 + gn;
        for cy in y0..y0 + span {
            for cx in x0..x0 + span {
                let m = at(cx, cy);
                let Some((motion, amp, hang, leaf)) = tab.kind[m as usize] else { continue };
                // How far up its plant: plant cells between it and its ground.
                let (fx, fy) = (cx as f64 + 0.5, cy as f64 + 0.5);
                let r = (fx * fx + fy * fy).sqrt().max(1e-9);
                let st = |u: f64| if u > 0.38 { 1 } else if u < -0.38 { -1 } else { 0 };
                let (mut bx, mut by) = (-st(fx / r), -st(fy / r));
                if hang {
                    (bx, by) = (-bx, -by);
                }
                let (mut qx, mut qy, mut h) = (cx, cy, 0);
                while h < 64 {
                    qx += bx;
                    qy += by;
                    let q = if inside(qx, qy) { at(qx, qy) } else { p.material(qx, qy) };
                    if !tab.tissue.get(q as usize).copied().unwrap_or(false) {
                        break;
                    }
                    h += 1;
                }
                let ground = p.surface_radius(fx / r, fy / r);
                let c0 = dft_sim::render::colour_at(p, m, cx, cy, ground);
                let c = tx::texel(kinds.get(m as usize).copied().unwrap_or(tx::Kind::Rock), c0, cx, cy, 0.5, 0.5, ppc, Sides::default());
                plants.push(SwayCell { x: cx, y: cy, c, h: h as f32, motion, amp, leaf });
            }
        }
    }
    let sways = |m: MatId| swaying.as_ref().is_some_and(|t| t.kind[m as usize].is_some());
    let mut ghosts: HashMap<(i64, i64), [u8; 3]> = HashMap::new();
    let mut ghost = |g: (i64, i64)| *ghosts.entry(g).or_insert_with(|| tx::ghost(p, g.0 + 1, g.1 + 1));
    let gm = tx::GHOST_M;
    let atm = p.spec.atmosphere_m;
    let band = CRUMBLE * p.max_radius;
    let debris = (p.max_radius / 300.0).max(2.0);
    let mut has_ghost = false;
    let mut px = vec![0u8; (TEX * TEX * 4) as usize];
    for row in 0..TEX {
        let j = TEX - 1 - row;
        for i in 0..TEX {
            // The texel's middle, in cells, and the cell it falls in.
            let (x, y) = if fine { (x0 as f64 + (i as f64 + 0.5) * s, y0 as f64 + (j as f64 + 0.5) * s) } else {
                let c = cell_of(i, j);
                (c.0 as f64 + 0.5, c.1 as f64 + 0.5)
            };
            let (cx, cy) = (x.floor() as i64, y.floor() as i64);
            let (u, v) = (x - cx as f64, y - cy as f64);
            let m = if fine { at(cx, cy) } else { cells[(j * TEX + i) as usize] };
            // (A swaying plant is drawn over the tile: here, what's behind it.)
            let m = if sways(m) { VACUUM } else { m };
            let o = ((row * TEX + i) * 4) as usize;
            let (fx, fy) = (cx as f64 + 0.5, cy as f64 + 0.5);
            let r = (fx * fx + fy * fy).sqrt().max(1e-9);
            let ground = p.surface_radius(fx / r, fy / r);
            // A stand-in leaves the outermost layer clear: only texels wholly
            // below the ground (with a texel's margin for rough relief) and
            // outside craters are drawn, and only solid ground.
            if k.under && (r + 2.0 * s.max(1.0) > ground || p.in_crater(fx, fy) || m == VACUUM) {
                continue;
            }
            let rgba: [u8; 4] = if m == VACUUM {
                // The ground as it stands now: where the swarm has eaten it
                // well down, the top of what's left (so the atmosphere sinks
                // with it instead of hanging as a halo over nothing).
                let now = match heights {
                    Some(h) if !h.is_empty() => {
                        let nh = h.len();
                        let t = (fy.atan2(fx).rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU) * nh as f64;
                        let i = t.floor() as usize % nh;
                        let f = t - t.floor();
                        let top = h[i] as f64 * (1.0 - f) + h[(i + 1) % nh] as f64 * f;
                        if top < ground - AIR_SINK_M { top } else { ground }
                    }
                    _ => ground,
                };
                // (Redrawn when the ground's height is next published.)
                if r < ground + atm {
                    has_ghost = true;
                }
                let underground = r < now && !p.in_crater(fx, fy);
                // The ghost crumbles at the collapse's edge: each block of
                // debris holds on until the edge passes its own depth.
                let hold = if underground {
                    let n = dft_sim::noise::unit(dft_sim::noise::hash(0xC011, (x / debris).floor() as i64, (y / debris).floor() as i64, 3));
                    let global = (collapse - r - band * n) / band;
                    // It also crumbles away where the ground above it has
                    // been eaten down: only what still lies under ground
                    // left standing (tunnels, caves) keeps its ghost.
                    let local = match heights {
                        Some(h) if !h.is_empty() => {
                            let nh = h.len();
                            let t = (fy.atan2(fx).rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU) * nh as f64;
                            let i = t.floor() as usize % nh;
                            let f = t - t.floor();
                            let top = h[i] as f64 * (1.0 - f) + h[(i + 1) % nh] as f64 * f;
                            let lb = (4.0 * debris).max(8.0);
                            (top - r - lb * n) / lb
                        }
                        _ => f64::INFINITY,
                    };
                    global.min(local)
                } else {
                    -1.0
                };
                if underground && !GHOST {
                    // Eaten ground shows the stars behind it.
                    continue;
                }
                if underground && hold <= 0.0 {
                    has_ghost = true;
                    continue;
                }
                if underground {
                    has_ghost = true;
                    // Eaten ground (or a buried void): the dim ghost of the
                    // rock that was there, scored with the swarm's drill marks.
                    let c = if ppc < 0.5 {
                        let g = ghost((cx.div_euclid(gm) * gm, cy.div_euclid(gm) * gm));
                        if g == tx::VOID && p.material(cx, cy) != VACUUM { tx::ghost(p, cx, cy) } else { g }
                    } else {
                        let (gi, gj) = ((x / gm as f64 - 0.5).floor() as i64, (y / gm as f64 - 0.5).floor() as i64);
                        let mut four = [[0u8; 3]; 4];
                        for (q, (di, dj)) in [(0, 0), (1, 0), (0, 1), (1, 1)].into_iter().enumerate() {
                            four[q] = ghost(((gi + di) * gm, (gj + dj) * gm));
                        }
                        // Along the old surface, a ghost block can fall in
                        // open space: use this cell's own ghost there.
                        if four.contains(&tx::VOID) && p.material(cx, cy) != VACUUM {
                            let fill = tx::ghost(p, cx, cy);
                            for q in four.iter_mut().filter(|q| **q == tx::VOID) {
                                *q = fill;
                            }
                        }
                        let mut c = tx::backdrop(four, x, y);
                        // Ground standing over the gap casts a shadow into it.
                        if fine && ppc >= 4.0 {
                            let e = (2.5 / ppc).max(0.15);
                            let mut f = 1.0;
                            if v > 1.0 - e && at(cx, cy + 1) != VACUUM {
                                f *= 0.55;
                            }
                            if u < e && at(cx - 1, cy) != VACUUM {
                                f *= 0.75;
                            }
                            if u > 1.0 - e && at(cx + 1, cy) != VACUUM {
                                f *= 0.75;
                            }
                            c = [(c[0] as f64 * f) as u8, (c[1] as f64 * f) as u8, (c[2] as f64 * f) as u8];
                        }
                        c
                    };
                    // Darker toward the crumbling edge.
                    let f = 0.45 + 0.55 * hold.min(1.0);
                    let c = [(c[0] as f64 * f) as u8, (c[1] as f64 * f) as u8, (c[2] as f64 * f) as u8];
                    [c[0], c[1], c[2], 255]
                } else {
                    // Above the ground: the atmosphere, see-through and fading
                    // with height (the stars show behind); beyond it, nothing.
                    let h = (r - now).max(0.0);
                    if atm <= 0.0 || h >= atm {
                        continue;
                    }
                    let a = 0.55 * (1.0 - h / atm);
                    let c = p.palette.atmosphere;
                    [c[0], c[1], c[2], (a * 255.0) as u8]
                }
            } else {
                let mut c = dft_sim::render::colour_at(p, m, cx, cy, ground);
                if !looks.is_empty() {
                    let li = if fine { ((cy - gy0) * gn + (cx - gx0)) as usize } else { (j * TEX + i) as usize };
                    let look = looks.get(li).copied().unwrap_or(0);
                    if look != 0 {
                        c = look_tint(p, m, c, look, cx, cy);
                    }
                }
                if ppc >= 1.0 {
                    let sides = if fine && ppc >= 4.0 {
                        let ns = [at(cx, cy + 1), at(cx, cy - 1), at(cx - 1, cy), at(cx + 1, cy)];
                        Sides { open: ns.map(|q| q == VACUUM), other: ns.map(|q| q != m) }
                    } else {
                        Sides::default()
                    };
                    c = tx::texel(kinds.get(m as usize).copied().unwrap_or(tx::Kind::Rock), c, cx, cy, u, v, ppc, sides);
                }
                [c[0], c[1], c[2], 255]
            };
            px[o..o + 4].copy_from_slice(&rgba);
        }
    }
    Some((px, has_ghost, plants))
}

/// Take the simulation's news of changed ground.
fn take_changes(mut tiles: ResMut<Tiles>, run: Option<Res<crate::Run>>, time: Res<Time>, mut activity: ResMut<crate::camera::Activity>, mut perf: ResMut<crate::log::Perf>) {
    let t0 = std::time::Instant::now();
    let _timer = Timer(&mut perf, crate::log::P_CHANGES, t0);
    let Some(run) = run else { return };
    let (blocks, rects, fine) = {
        let mut out = run.0.out.lock();
        (std::mem::take(&mut out.blocks), std::mem::take(&mut out.rects), std::mem::take(&mut out.fine))
    };
    for &(bx, by) in &blocks {
        activity.block(bx, by);
    }
    if !blocks.is_empty() || !rects.is_empty() || !fine.is_empty() {
        tiles.changed(blocks, rects, fine, time.elapsed_secs_f64());
    }
}

/// Ask for the tiles the view needs: the level the zoom wants, plus coarser
/// stand-ins underneath while they arrive. Changed tiles are rebuilt.
/// Adds the time from its start to a part of the frame when it goes out of scope.
struct Timer<'a, 'w>(&'a mut ResMut<'w, crate::log::Perf>, usize, std::time::Instant);

impl Drop for Timer<'_, '_> {
    fn drop(&mut self) {
        let (part, t0) = (self.1, self.2);
        self.0.add(part, t0);
    }
}

fn request(mut tiles: ResMut<Tiles>, view: Res<View>, body: Res<crate::Body>, run: Option<Res<crate::Run>>, time: Res<Time>, mut perf: ResMut<crate::log::Perf>) {
    let _timer = Timer(&mut perf, crate::log::P_REQUEST, std::time::Instant::now());
    tiles.frame += 1;
    // The backdrop collapses as the body is eaten: when it moves a step,
    // every tile showing some of it is redrawn.
    if let Some(run) = &run {
        let (eaten, heights) = {
            let out = run.0.out.lock();
            (out.stats.consumed, out.heights.clone())
        };
        let c = collapse_radius(&body.0, eaten);
        let new_heights = heights.as_ref().map(|h| h.0) != tiles.heights.as_ref().map(|h| h.0);
        if new_heights {
            tiles.heights = heights;
        }
        // Which directions round the body the ground's height changed in.
        let mut moved: Option<Vec<bool>> = None;
        if new_heights {
            let old = tiles.heights_seen.take();
            if let Some((_, h)) = &tiles.heights {
                moved = Some(match old {
                    Some(o) if o.len() == h.len() => o.iter().zip(h.iter()).map(|(a, b)| (a - b).abs() > 4.0).collect(),
                    _ => vec![true; h.len()],
                });
                tiles.heights_seen = Some(h.clone());
            }
        }
        if (GHOST && c != tiles.collapse) || moved.as_ref().is_some_and(|m| m.iter().any(|&x| x)) {
            let all = GHOST && c != tiles.collapse;
            tiles.collapse = c;
            for (k, s) in tiles.slots.iter_mut() {
                if s.ghost && (all || moved.as_ref().is_some_and(|m| k.faces_moved(m))) {
                    s.stale = true;
                }
            }
        }
        tiles.collapse = c;
    }
    let level = level_for(view.mpp);
    let top = top_level(&body.0).max(level);
    tiles.level = level;
    // In order: the tiles on screen, then the coarse stand-ins, then the
    // ring of tiles built ahead around the view (only with building room to
    // spare, so they never hold up what's on screen).
    // First, a coarse picture of the whole view two levels up (a sixteenth
    // as many tiles): whatever finer tiles are still on their way while
    // zooming and panning, a piece of one of these covers the gap, so
    // nothing is ever left blank.
    let mut wanted: Vec<(Key, bool)> = Vec::new();
    let stand = (level + 2).min(top);
    if stand > level {
        wanted.extend(keys_in_view(&view, stand, 1, false).into_iter().map(|k| (k, false)));
    }
    wanted.extend(keys_in_view(&view, level, 1, false).into_iter().map(|k| (k, false)));
    for l in [top, (level + 3).min(top)] {
        if l > level {
            wanted.extend(keys_in_view(&view, l, 0, true).into_iter().map(|k| (k, false)));
        }
    }
    {
        let (l, b, r, t) = view.bounds();
        let s = span_cells(level) as f64;
        let ahead = ((r - l).max(t - b) / s * AHEAD_SCREENS * 0.5).ceil() as i64;
        if ahead > 1 {
            let near: HashSet<Key> = wanted.iter().map(|w| w.0).collect();
            wanted.extend(keys_in_view(&view, level, ahead, false).into_iter().filter(|k| !near.contains(k)).map(|k| (k, true)));
        }
    }
    let frame = tiles.frame;
    let now = time.elapsed_secs_f64();
    let mut missing = 0;
    // What to build: tiles never built come first (a gap on screen), then
    // rebuilds of tiles already showing a picture (the ground changed).
    let mut fresh_builds: Vec<(Key, bool)> = Vec::new();
    let mut rebuilds: Vec<(Key, bool)> = Vec::new();
    let mut listed: HashSet<Key> = HashSet::new();
    tiles.wanted_n = wanted.len();
    for (k, ahead) in wanted {
        if !listed.insert(k) {
            continue;
        }
        let (fresh, due) = match tiles.slots.get_mut(&k) {
            Some(s) => {
                if s.seen + 1 < frame {
                    s.away = true;
                }
                s.seen = frame;
                let wait = if k.under { REBUILD_UNDER_S } else if ahead { REBUILD_AHEAD_S } else { REBUILD_S };
                let due = s.task.is_none() && s.stale && now - s.built_at >= wait;
                if !k.under && !ahead && (matches!(s.shown, Shown::Pending) || s.task.is_some() || s.stale) {
                    missing += 1;
                }
                (false, due)
            }
            None => {
                if !k.under && !ahead {
                    missing += 1;
                }
                (true, true)
            }
        };
        if due {
            let first = fresh || tiles.slots.get(&k).is_some_and(|s| matches!(s.shown, Shown::Pending));
            if first {
                fresh_builds.push((k, ahead));
            } else {
                rebuilds.push((k, ahead));
            }
        }
    }
    // On screen first (new, then rebuilds), then the ring ahead.
    let (fa, fs): (Vec<_>, Vec<_>) = fresh_builds.into_iter().partition(|x| x.1);
    let (ra, rs): (Vec<_>, Vec<_>) = rebuilds.into_iter().partition(|x| x.1);
    for (k, ahead) in fs.into_iter().chain(rs).chain(fa).chain(ra) {
        if tiles.building >= if ahead { IN_FLIGHT / 2 } else { IN_FLIGHT } {
            if ahead {
                continue;
            }
            break;
        }
        let touched = tiles.is_touched(k);
        let fresh = !tiles.slots.contains_key(&k);
        let p = body.0.clone();
        let live = if touched { run.as_ref().map(|r| r.0.clone()) } else { None };
        let collapse = tiles.collapse;
        let heights = tiles.heights.as_ref().map(|h| h.1.clone());
        let task = AsyncComputeTaskPool::get().spawn(async move { build(&p, k, live, collapse, heights) });
        tiles.building += 1;
        if fresh {
            tiles.slots.insert(k, Slot { shown: Shown::Pending, task: Some(task), plants: Arc::new(Vec::new()), stale: false, built_at: now, image_from: now, changed_at: 0.0, seen: frame, ghost: false, away: false });
        } else {
            let s = tiles.slots.get_mut(&k).unwrap();
            s.task = Some(task);
            s.stale = false;
            s.built_at = now;
        }
    }
    tiles.missing = missing;
}

fn image(px: Vec<u8>) -> Image {
    let mut img = Image::new(
        Extent3d { width: TEX as u32, height: TEX as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        px,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = ImageSampler::nearest();
    img
}

/// Turn finished tiles into sprites (or update the picture in place);
/// forget old ones beyond the budget.
fn collect(mut commands: Commands, mut tiles: ResMut<Tiles>, mut images: ResMut<Assets<Image>>, mut perf: ResMut<crate::log::Perf>) {
    let t0 = std::time::Instant::now();
    let mut done = Vec::new();
    for (k, s) in tiles.slots.iter_mut() {
        if let Some(task) = &mut s.task {
            if let Some(result) = block_on(poll_once(task)) {
                s.task = None;
                done.push((*k, result));
            }
        }
    }
    for (k, result) in done {
        tiles.building -= 1;
        let Some(s) = tiles.slots.get_mut(&k) else { continue };
        let result = result.map(|(px, ghost, plants)| {
            s.ghost = ghost;
            s.plants = Arc::new(plants);
            px
        });
        if result.is_none() {
            s.plants = Arc::new(Vec::new());
        }
        s.image_from = s.built_at;
        s.away = false;
        let old = std::mem::replace(&mut s.shown, Shown::Pending);
        if matches!(old, Shown::Pending) {
            perf.built += 1;
        } else {
            perf.rebuilt += 1;
        }
        s.shown = match (result, old) {
            (None, Shown::Sprite(e, _)) => {
                commands.entity(e).despawn();
                Shown::Empty
            }
            (None, _) => Shown::Empty,
            (Some(px), Shown::Sprite(e, h)) => {
                // (The same picture again: no upload to the GPU.)
                let same = images.get(&h).is_some_and(|img| img.data.as_deref() == Some(px.as_slice()));
                if !same {
                    if let Some(mut img) = images.get_mut(&h) {
                        img.data = Some(px);
                    }
                }
                Shown::Sprite(e, h)
            }
            (Some(px), _) => {
                let h = images.add(image(px));
                let e = commands.spawn((Sprite { image: h.clone(), custom_size: Some(Vec2::ONE), ..default() }, Transform::default(), Visibility::Hidden, Tile(k))).id();
                Shown::Sprite(e, h)
            }
        };
    }
    // Forget the least recently wanted tiles once over budget (never one
    // the view wanted this frame or the last).
    let keep = KEEP.max(tiles.wanted_n * 3 / 2);
    if tiles.slots.len() > keep {
        let recent = tiles.frame.saturating_sub(1);
        let mut ages: Vec<(u64, Key)> = tiles.slots.iter().filter(|(_, s)| s.task.is_none() && s.seen < recent).map(|(k, s)| (s.seen, *k)).collect();
        ages.sort();
        let drop = (tiles.slots.len() - keep * 9 / 10).min(ages.len());
        for (_, k) in ages.into_iter().take(drop) {
            if let Some(Slot { shown: Shown::Sprite(e, _), .. }) = tiles.slots.remove(&k) {
                commands.entity(e).despawn();
            }
        }
    }
    perf.add(crate::log::P_COLLECT, t0);
}

/// A piece of a coarser tile standing in for a finer one still on its way.
#[derive(Component)]
struct Patch;

/// Put every tile where the view says, in screen pixels; finer tiles on top.
/// Tiles finer than the zoom needs are hidden (kept for zooming back in).
/// Where a tile at the zoom's level hasn't arrived yet, the view shows,
/// in order of preference: its four finer tiles (zooming out), the matching
/// piece of a coarser tile (zooming in), or a coarse stand-in of the deep
/// ground. Each place is covered once, so nothing see-through (atmosphere,
/// backdrop) is drawn twice, and nothing blinks while tiles arrive.
#[allow(clippy::type_complexity)]
fn place(
    mut commands: Commands,
    tiles: Res<Tiles>,
    view: Res<View>,
    mut q: Query<(&Tile, &mut Transform, &mut Sprite, &mut Visibility), Without<Patch>>,
    mut patches: Query<(&mut Transform, &mut Sprite, &mut Visibility), (With<Patch>, Without<Tile>)>,
    mut pool: Local<Vec<Entity>>,
    mut perf: ResMut<crate::log::Perf>,
    time: Res<Time>,
    mut swaying: ResMut<crate::sway::Shown>,
) {
    swaying.0.clear();
    let now = time.elapsed_secs_f64();
    let t0 = std::time::Instant::now();
    let mut shown = 0;
    let (l, b, r, t) = view.bounds();
    let level = tiles.level;
    // Arrived, and not long out of date (a tile not looked at for a while
    // may show ground eaten since: better to wait for its rebuild).
    let ready = |k: &Key| tiles.slots.get(k).is_some_and(|s| !matches!(s.shown, Shown::Pending) && !(s.changed_at > s.image_from && now - s.image_from > 3.0));
    // A tile at the zoom's level the view has kept wanting keeps showing its
    // picture until its rebuild lands, however busy the ground under it
    // (swapping it for a stand-in meanwhile is what made tiles blink).
    let ready_here = |k: &Key| tiles.slots.get(k).is_some_and(|s| !matches!(s.shown, Shown::Pending) && !(s.away && s.changed_at > s.image_from));
    let waiting: Vec<Key> = keys_in_view(&view, level, 0, false).into_iter().filter(|k| !ready_here(k)).collect();
    let waiting_set: HashSet<Key> = waiting.iter().copied().collect();
    // Waiting tiles covered by their four finer tiles (all arrived).
    let mut by_children: HashSet<(i64, i64)> = HashSet::new();
    // Waiting tiles covered by a piece of a coarser one: (key, ancestor, image).
    let mut by_piece: Vec<(Key, Key, Handle<Image>)> = Vec::new();
    // Waiting tiles shown with their own out-of-date picture (nothing better).
    let mut old_own: HashSet<Key> = HashSet::new();
    let mut holes = 0usize;
    for w in &waiting {
        if level > FINEST {
            let kids: Vec<Key> = (0..4).map(|i| Key { level: level - 1, ix: w.ix * 2 + (i & 1), iy: w.iy * 2 + (i >> 1), under: false }).collect();
            if kids.iter().all(&ready) {
                by_children.insert((w.ix, w.iy));
                continue;
            }
        }
        let mut found = false;
        for d in 1..=5i8 {
            let a = Key { level: level + d, ix: w.ix >> d, iy: w.iy >> d, under: false };
            // Only an ancestor that is up to date (an old one may show ground
            // since eaten, or the backdrop before it crumbled).
            if !ready(&a) || (a.level < 0 && tiles.block_changed.get(&a.block()).is_some_and(|&t| t > tiles.slots[&a].image_from && now - tiles.slots[&a].image_from > 3.0)) {
                continue;
            }
            match tiles.slots.get(&a).map(|s| &s.shown) {
                Some(Shown::Sprite(_, h)) => {
                    by_piece.push((*w, a, h.clone()));
                    found = true;
                    break;
                }
                // All open space there: nothing to draw.
                Some(Shown::Empty) => {
                    found = true;
                    break;
                }
                _ => {}
            }
        }
        if !found {
            // Better an old picture than a hole: its own (it was away and the
            // ground changed), else any coarser one that has arrived.
            if tiles.slots.get(w).is_some_and(|s| matches!(s.shown, Shown::Sprite(..))) {
                old_own.insert(*w);
                continue;
            }
            for d in 1..=6i8 {
                let a = Key { level: level + d, ix: w.ix >> d, iy: w.iy >> d, under: false };
                match tiles.slots.get(&a).map(|s| &s.shown) {
                    Some(Shown::Sprite(_, h)) => {
                        by_piece.push((*w, a, h.clone()));
                        found = true;
                        break;
                    }
                    Some(Shown::Empty) => {
                        found = true;
                        break;
                    }
                    _ => {}
                }
            }
        }
        if !found {
            // Last, a piece of a coarse stand-in (deep ground only). Only the
            // piece over this tile: a whole stand-in would show through the
            // open ground of tiles already drawn around it.
            for d in 1..=(LEVELS as i8 - 1 - level.max(0)).min(40) {
                let a = Key { level: level + d, ix: w.ix >> d, iy: w.iy >> d, under: true };
                // Up to date only: an old one shows ground since eaten.
                if !ready(&a) {
                    continue;
                }
                if let Some(Shown::Sprite(_, h)) = tiles.slots.get(&a).map(|s| &s.shown) {
                    by_piece.push((*w, a, h.clone()));
                    found = true;
                    break;
                }
            }
            if !found {
                holes += 1;
            }
        }
    }
    let snap = |x0: f64, y0: f64, s: f64| {
        // Edges snapped to whole pixels: neighbours meet exactly, with no
        // gap and no overlap (an overlap shows in the see-through atmosphere).
        let a = view.to_screen(x0, y0);
        let z = view.to_screen(x0 + s, y0 + s);
        let (ax, ay, zx, zy) = (a.x.round(), a.y.round(), z.x.round(), z.y.round());
        (Vec2::new((ax + zx) / 2.0, (ay + zy) / 2.0), Vec2::new((zx - ax).max(1.0), (zy - ay).max(1.0)))
    };
    for (tile, mut tf, mut sprite, mut vis) in q.iter_mut() {
        let k = tile.0;
        let s = k.span();
        let (x0, y0) = (k.ix as f64 * s, k.iy as f64 * s);
        let in_view = x0 < r && x0 + s > l && y0 < t && y0 + s > b;
        // (A tile waiting for its rebuild is hidden: its stand-in covers it.)
        let on = in_view
            && ((k.level == level && !k.under && (!waiting_set.contains(&k) || old_own.contains(&k)))
                || (!k.under && k.level == level - 1 && by_children.contains(&(k.ix >> 1, k.iy >> 1))));
        let v = if on { Visibility::Visible } else { Visibility::Hidden };
        if *vis != v {
            *vis = v;
        }
        if !on {
            continue;
        }
        shown += 1;
        if let Some(sl) = tiles.slots.get(&k) {
            if !sl.plants.is_empty() {
                swaying.0.push(sl.plants.clone());
            }
        }
        let (c, size) = snap(x0, y0, s);
        tf.translation = Vec3::new(c.x, c.y, -(k.level as f32));
        sprite.custom_size = Some(size);
    }
    // The pieces of coarser tiles, each covering one waiting tile.
    while pool.len() < by_piece.len() {
        let e = commands.spawn((Sprite { custom_size: Some(Vec2::ONE), ..default() }, Transform::default(), Visibility::Hidden, Patch)).id();
        pool.push(e);
    }
    for (i, e) in pool.iter().enumerate() {
        let Ok((mut tf, mut sprite, mut vis)) = patches.get_mut(*e) else { continue };
        let Some((w, a, h)) = by_piece.get(i) else {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
            continue;
        };
        let s = w.span();
        let (x0, y0) = (w.ix as f64 * s, w.iy as f64 * s);
        let (c, size) = snap(x0, y0, s);
        // The piece of the ancestor's picture over this tile (its rows run
        // top down).
        let d = a.level - w.level;
        let n = TEX as f32 / (1u64 << d.min(60)) as f32;
        let (ox, oy) = ((w.ix - (a.ix << d)) as f32 * n, (w.iy - (a.iy << d)) as f32 * n);
        sprite.image = h.clone();
        sprite.rect = Some(Rect::new(ox, TEX as f32 - oy - n, ox + n, TEX as f32 - oy));
        sprite.custom_size = Some(size);
        tf.translation = Vec3::new(c.x, c.y, -(w.level as f32));
        if *vis != Visibility::Visible {
            *vis = Visibility::Visible;
        }
        shown += 1;
    }
    perf.tiles_shown = shown;
    perf.tile_holes = holes;
    perf.add(crate::log::P_PLACE, t0);
}
