//! Finding an element: click one in the stockpile's table and the view pans
//! (zoom unchanged) to the nearest ground made with it, from where the view
//! is now, and outlines the body of that material there.
//!
//! The search runs off the main thread, over the screen's copy of the dug
//! ground and the generator (never waiting on the simulation): rings of
//! growing size around the view's middle, each searched row by row on all
//! cores, stopping at the first ring that holds a match (the nearest in it
//! wins). The material's body is then flood-filled from that cell, within a
//! window round it, and its outline drawn until another find or Escape.

use crate::{Body, Run};
use bevy::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use dft_sim::materials::MatId;

pub struct FinderPlugin;

impl Plugin for FinderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Finder>().add_systems(Update, (finder, draw_outline).chain());
    }
}

/// Searches for a material, not an element, are numbered from here.
pub const MAT_BASE: usize = 1_000_000;

/// Half the side of the window the material's body is traced within (m).
const TRACE_HALF: i64 = 512;
/// Seconds the view takes to glide to what was found.
const GLIDE_S: f64 = 0.6;

/// What was found.
#[derive(Clone)]
pub struct Found {
    pub element: usize,
    pub mat: MatId,
    pub cell: (i64, i64),
    pub dist: f64,
    /// Cells in the traced body, and whether it runs on past the window.
    pub cells: usize,
    pub clipped: bool,
    /// Separate bodies of it in view of the cell found, and where the others
    /// are (a cell of each, at most a few hundred) to mark too.
    pub bodies: usize,
    others: Vec<(i64, i64)>,
    /// The outline: (x0, y0, x1, y1) in world metres.
    segments: Vec<[f64; 4]>,
}

/// A search to make.
struct Job {
    id: u64,
    p: Arc<dft_sim::planet::Planet>,
    shared: Option<Arc<crate::sim::Shared>>,
    from: (f64, f64),
    half: (f64, f64),
    element: usize,
    with: Vec<bool>,
}

/// The searcher: one thread of its own, apart from the game and the
/// simulation, taking searches one at a time (a newer one cancels the one
/// under way). It reads the screen's copy of the ground a row at a time, so
/// it never holds it long enough to make the simulation or the tile
/// builders wait, and it yields between rows: it only uses time nothing
/// else wants.
struct Worker {
    jobs: mpsc::Sender<Job>,
    results: Mutex<mpsc::Receiver<(u64, Option<Found>)>>,
    latest: Arc<AtomicU64>,
    next: u64,
}

impl Worker {
    fn start() -> Worker {
        let (jt, jr) = mpsc::channel::<Job>();
        let (rt, rr) = mpsc::channel();
        let latest = Arc::new(AtomicU64::new(0));
        let l2 = latest.clone();
        std::thread::Builder::new()
            .name("finder".into())
            .spawn(move || {
                while let Ok(mut job) = jr.recv() {
                    // Only the newest search matters.
                    while let Ok(j) = jr.try_recv() {
                        job = j;
                    }
                    let r = search(&job.p, job.shared.as_deref(), job.from, job.half, job.element, &job.with, &|| l2.load(Ordering::Relaxed) != job.id);
                    if l2.load(Ordering::Relaxed) == job.id && rt.send((job.id, r)).is_err() {
                        break;
                    }
                }
            })
            .expect("start the finder thread");
        Worker { jobs: jt, results: Mutex::new(rr), latest, next: 1 }
    }
}

#[derive(Resource, Default)]
pub struct Finder {
    /// An element (its place in the element table) to look for.
    pub want: Option<usize>,
    /// Or a material to look for (its body outlined the same way).
    pub want_mat: Option<MatId>,
    /// The background searcher (made on the first search) and the search
    /// it is on (the element, and its job number).
    worker: Option<Worker>,
    task: Option<(usize, u64)>,
    pub found: Option<Found>,
    /// For the panel: what is going on.
    pub status: String,
    /// The screen the stockpile window covers (pixels), when open.
    pub covered: Option<bevy_egui::egui::Rect>,
    /// The glide: from, to, started (real seconds).
    glide: Option<((f64, f64), (f64, f64), f64)>,
}

fn finder(mut f: ResMut<Finder>, run: Option<Res<Run>>, body: Res<Body>, mut view: ResMut<crate::view::View>, mut cam: ResMut<crate::camera::Camera>, keys: Res<ButtonInput<KeyCode>>, time: Res<Time<Real>>) {
    let now = time.elapsed_secs_f64();
    if keys.just_pressed(KeyCode::Escape) {
        f.found = None;
        f.status.clear();
    }
    // A new search (one at a time: a new click replaces the old one).
    // (A material is asked for as MAT_BASE + its id.)
    let wanted = f.want.take().or_else(|| f.want_mat.take().map(|m| MAT_BASE + m as usize));
    if let Some(e) = wanted {
        let p = body.0.clone();
        let shared = run.as_ref().map(|r| r.0.clone());
        let from = (view.x, view.y);
        let half = (view.w * 0.5 * view.mpp, view.h * 0.5 * view.mpp);
        let (with, name): (Vec<bool>, String) = if e >= MAT_BASE {
            ((0..p.mats.len()).map(|m| m == e - MAT_BASE).collect(), p.mats.get((e - MAT_BASE) as MatId).name.clone())
        } else {
            // The materials made with it.
            let sym = p.mats.elements.list[e].symbol.clone();
            ((0..p.mats.len()).map(|m| p.mats.get(m as MatId).composition.get(&sym).is_some_and(|&x| x > 0.0)).collect(), p.mats.elements.list[e].name.clone())
        };
        f.status = if with.iter().any(|&w| w) { format!("looking for {name}…") } else { format!("nothing on this body has {name} in it") };
        if with.iter().any(|&w| w) {
            if f.worker.is_none() {
                f.worker = Some(Worker::start());
            }
            let w = f.worker.as_mut().unwrap();
            let id = w.next;
            w.next += 1;
            w.latest.store(id, Ordering::Relaxed);
            let _ = w.jobs.send(Job { id, p, shared, from, half, element: e, with });
            f.task = Some((e, id));
        } else {
            f.task = None;
        }
    }
    let done = match (&f.task, &f.worker) {
        (Some((_, id)), Some(w)) => w.results.lock().unwrap().try_iter().filter(|r| r.0 == *id).last().map(|r| r.1),
        _ => None,
    };
    if let (Some((e, _)), Some(result)) = (f.task, done) {
        {
            f.task = None;
            let name = if e >= MAT_BASE { body.0.mats.get((e - MAT_BASE) as MatId).name.clone() } else { body.0.mats.elements.list[e].name.clone() };
            match result {
                Some(found) => {
                    let m = body.0.mats.get(found.mat);
                    let tail = format!(
                        "{:.0} m away; largest body in view {} m²{}{}",
                        found.dist,
                        found.cells,
                        if found.clipped { " (runs on past the view)" } else { "" },
                        if found.bodies > 1 { format!(", of {} bodies", found.bodies) } else { String::new() }
                    );
                    f.status = if e >= MAT_BASE {
                        format!("{name}: nearest {tail}")
                    } else {
                        let pct = m.composition.get(&body.0.mats.elements.list[e].symbol).copied().unwrap_or(0.0) * 100.0;
                        format!("{name}: nearest is {} ({pct:.0}% {name}), {tail}", m.name)
                    };
                    // The cell found in the middle of the open part of the
                    // screen (beside the stockpile window, not under it).
                    let mut to = (found.cell.0 as f64 + 0.5, found.cell.1 as f64 + 0.5);
                    if let Some(r) = f.covered {
                        let (w, h) = (view.w as f32, view.h as f32);
                        let aim = if r.min.x > w - r.max.x { (r.min.x * 0.5, h * 0.5) } else { ((r.max.x + w) * 0.5, h * 0.5) };
                        to.0 -= (aim.0 as f64 - view.w * 0.5) * view.mpp;
                        to.1 += (aim.1 as f64 - view.h * 0.5) * view.mpp;
                    }
                    f.glide = Some(((view.x, view.y), to, now));
                    cam.manual(now);
                    f.found = Some(found);
                }
                None => {
                    f.status = format!("no {name} anywhere that can be seen");
                    f.found = None;
                }
            }
        }
    }
    // Glide there (eased), zoom untouched; any drag by the player stops it.
    if let Some((a, b, t0)) = f.glide {
        let k = ((now - t0) / GLIDE_S).clamp(0.0, 1.0);
        let k = k * k * (3.0 - 2.0 * k);
        view.x = a.0 + (b.0 - a.0) * k;
        view.y = a.1 + (b.1 - a.1) * k;
        cam.manual(now);
        if k >= 1.0 {
            f.glide = None;
        }
    }
}

/// The material at a cell as the screen knows it.
fn mat_at(p: &dft_sim::planet::Planet, mirror: Option<&crate::sim::Mirror>, x: i64, y: i64) -> MatId {
    mirror.and_then(|m| m.get(x, y)).unwrap_or_else(|| p.material(x, y))
}

fn search(p: &dft_sim::planet::Planet, shared: Option<&crate::sim::Shared>, from: (f64, f64), half: (f64, f64), element: usize, with: &[bool], cancelled: &dyn Fn() -> bool) -> Option<Found> {
    // A row at a time from the screen's copy of the ground (held only that
    // long), then a moment for everything else.
    let row = |y: i64, x0: i64, x1: i64| -> Vec<MatId> {
        let guard = shared.map(|s| s.mirror.read());
        let mirror = guard.as_deref();
        let v = (x0..x1).map(|x| mat_at(p, mirror, x, y)).collect();
        drop(guard);
        std::thread::yield_now();
        v
    };
    let (cx, cy) = (from.0.floor() as i64, from.1.floor() as i64);
    let reach = (p.max_radius + 2.0) as i64;
    let far = (cx.abs().max(cy.abs()) + reach) * 2;
    let is_m = |m: MatId| (m as usize) < with.len() && with[m as usize];
    // Square rings [r0, r1) of growing size; within the first holding a
    // match, the nearest by straight distance (the next ring is checked too,
    // since a ring's corners are further than the next ring's sides).
    let mut r0 = 0i64;
    let mut r1 = 32i64;
    let mut best: Option<(i64, (i64, i64))> = None;
    while r0 <= far {
        let mut hit: Option<(i64, (i64, i64))> = None;
        for y in cy - r1 + 1..cy + r1 {
            if cancelled() {
                return None;
            }
            let dy = y - cy;
            // Off the body: nothing.
            if y.abs() > reach {
                continue;
            }
            // The ring's part of this row: all of it, or its two ends.
            let spans: Vec<(i64, i64)> = if dy.abs() >= r0 { vec![(cx - r1 + 1, cx + r1)] } else { vec![(cx - r1 + 1, cx - r0 + 1), (cx + r0, cx + r1)] };
            for (x0, x1) in spans {
                let (x0, x1) = (x0.max(-reach), x1.min(reach + 1));
                if x1 <= x0 {
                    continue;
                }
                for (i, m) in row(y, x0, x1).into_iter().enumerate() {
                    let x = x0 + i as i64;
                    let d = (x - cx) * (x - cx) + dy * dy;
                    if is_m(m) && hit.is_none_or(|(bd, bc)| d < bd || (d == bd && (x, y) < bc)) {
                        hit = Some((d, (x, y)));
                    }
                }
            }
        }
        if let Some(h) = hit {
            if best.is_none_or(|b| h.0 < b.0) {
                best = Some(h);
            }
        }
        // Done once the best so far is nearer than anything further out.
        if let Some((d, _)) = best {
            if (r1 * r1) as i64 > d {
                break;
            }
        }
        r0 = r1;
        r1 *= 2;
    }
    let (d2, cell) = best?;
    let mat = row(cell.1, cell.0, cell.0 + 1)[0];
    // Every body of that material within a screen's view of it: the
    // largest is the one outlined (ore often lies in scattered single cells
    // round a bigger lode).
    let (hw, hh) = ((half.0.ceil() as i64).clamp(16, TRACE_HALF), (half.1.ceil() as i64).clamp(16, TRACE_HALF));
    let (wx0, wy0) = (cell.0 - hw, cell.1 - hh);
    let (nw, nh) = ((2 * hw) as usize, (2 * hh) as usize);
    let idx = |x: i64, y: i64| ((y - wy0) as usize) * nw + (x - wx0) as usize;
    let inside = |x: i64, y: i64| x >= wx0 && y >= wy0 && x < wx0 + nw as i64 && y < wy0 + nh as i64;
    // The window's materials, read once, a row at a time.
    let mut here: Vec<bool> = Vec::with_capacity(nw * nh);
    for j in 0..nh as i64 {
        if cancelled() {
            return None;
        }
        here.extend(row(wy0 + j, wx0, wx0 + nw as i64).into_iter().map(|m| m == mat));
    }
    // Label the bodies (0: none).
    let mut label = vec![0u32; nw * nh];
    let mut sizes: Vec<(usize, bool)> = vec![(0, false)];
    let mut stack = Vec::new();
    for start in 0..nw * nh {
        if !here[start] || label[start] != 0 {
            continue;
        }
        let id = sizes.len() as u32;
        sizes.push((0, false));
        label[start] = id;
        stack.push(start);
        while let Some(k) = stack.pop() {
            sizes[id as usize].0 += 1;
            let (x, y) = (wx0 + (k % nw) as i64, wy0 + (k / nw) as i64);
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if !inside(nx, ny) {
                    sizes[id as usize].1 = true;
                    continue;
                }
                let q = idx(nx, ny);
                if here[q] && label[q] == 0 {
                    label[q] = id;
                    stack.push(q);
                }
            }
        }
    }
    // The largest (ties: the one holding the cell found).
    let own = label[idx(cell.0, cell.1)];
    let mut pick = own;
    for (id, sz) in sizes.iter().enumerate().skip(1) {
        if sz.0 > sizes[pick as usize].0 {
            pick = id as u32;
        }
    }
    let (cells, clipped) = sizes[pick as usize];
    let is_in = |x: i64, y: i64| inside(x, y) && label[idx(x, y)] == pick;
    let (n_x, n_y) = (nw as i64, nh as i64);
    // Its outline: every side of a cell in it facing a cell not in it, joined
    // into runs along rows and columns (not along the window's edge).
    let mut segments = Vec::new();
    for y in wy0..wy0 + n_y {
        for (side, ny) in [(1i64, y + 1), (0i64, y - 1)] {
            let mut run: Option<i64> = None;
            for x in wx0..=wx0 + n_x {
                let edge = x < wx0 + n_x && is_in(x, y) && !is_in(x, ny) && inside(x, ny);
                match (edge, run) {
                    (true, None) => run = Some(x),
                    (false, Some(s)) => {
                        let yy = (y + side) as f64;
                        segments.push([s as f64, yy, x as f64, yy]);
                        run = None;
                    }
                    _ => {}
                }
            }
        }
    }
    for x in wx0..wx0 + n_x {
        for (side, nx) in [(1i64, x + 1), (0i64, x - 1)] {
            let mut run: Option<i64> = None;
            for y in wy0..=wy0 + n_y {
                let edge = y < wy0 + n_y && is_in(x, y) && !is_in(nx, y) && inside(nx, y);
                match (edge, run) {
                    (true, None) => run = Some(y),
                    (false, Some(s)) => {
                        let xx = (x + side) as f64;
                        segments.push([xx, s as f64, xx, y as f64]);
                        run = None;
                    }
                    _ => {}
                }
            }
        }
    }
    let bodies = sizes.len() - 1;
    // A cell of each other body (the first met), for small marks.
    let mut marked = vec![false; sizes.len()];
    marked[pick as usize] = true;
    let mut others = Vec::new();
    for (k, &l) in label.iter().enumerate() {
        if l != 0 && !marked[l as usize] && others.len() < 400 {
            marked[l as usize] = true;
            others.push((wx0 + (k % nw) as i64, wy0 + (k / nw) as i64));
        }
    }
    Some(Found { element, mat, cell, dist: (d2 as f64).sqrt(), cells, clipped, bodies, others, segments })
}

/// The outline, pulsing gently, over everything.
fn draw_outline(f: Res<Finder>, view: Res<crate::view::View>, mut gizmos: Gizmos, time: Res<Time<Real>>) {
    let Some(found) = &f.found else { return };
    let (l, b, r, t) = view.bounds();
    let pulse = 0.65 + 0.35 * (time.elapsed_secs() * 3.0).sin();
    let c = Color::srgba(1.0, 0.92, 0.25, pulse);
    for s in &found.segments {
        // (Only what is on screen.)
        if s[0].max(s[2]) < l || s[0].min(s[2]) > r || s[1].max(s[3]) < b || s[1].min(s[3]) > t {
            continue;
        }
        gizmos.line_2d(view.to_screen(s[0], s[1]), view.to_screen(s[2], s[3]), c);
    }
    // Small marks on the other bodies of it in view.
    let dim = Color::srgba(1.0, 0.92, 0.25, 0.55 * pulse);
    for o in &found.others {
        let (x, y) = (o.0 as f64 + 0.5, o.1 as f64 + 0.5);
        if x < l || x > r || y < b || y > t {
            continue;
        }
        gizmos.circle_2d(view.to_screen(x, y), (4.0f32).max((1.0 / view.mpp) as f32), dim);
    }
    // A ring round the cell found.
    let p = view.to_screen(found.cell.0 as f64 + 0.5, found.cell.1 as f64 + 0.5);
    gizmos.circle_2d(p, (6.0f32).max((2.0 / view.mpp) as f32), c);
}
