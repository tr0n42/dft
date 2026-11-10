//! The camera director: an "action cam" that finds where things are
//! happening and moves smoothly from one to the next, until the player takes
//! the camera (right-drag or the wheel). After 20 s without input, or at once
//! on a middle click or Home, the director takes it back.
//!
//! Every second it scores patches of ground (PATCH metres square) by what is
//! going on there: drones cutting, drones hauling, ground changing (digging,
//! cave-ins, flows), and what the swarm reports happening: a hauler lifting
//! off full, a flock set down or merged, an explosion, a breach into a cave,
//! lava or a sea, rich ore, drones lost. Places shown lately count
//! for less, so the camera roams. A shot is held at least MIN_HOLD seconds
//! (drifting with the action), and replaced when somewhere clearly busier
//! turns up, or when it has gone on long enough. Moves ease in and out; a
//! long move pulls out in an arc so the eye can follow where it goes.
//!
//! The zoom limit: at first the player can only zoom out so far; it opens as
//! more of the body is charted, and fully once a third of it is eaten.
use crate::view::View;
use bevy::prelude::*;
use std::collections::{HashMap, VecDeque};

/// The side of a scored patch of ground, metres.
const PATCH: f64 = 256.0;
/// Shots last at least this long, and are cut after this long (seconds).
const MIN_HOLD: f64 = 8.0;
const MAX_HOLD: f64 = 25.0;
/// A new place must score this much better to cut away early.
const CUT_FACTOR: f32 = 1.4;
/// Seconds without input before the director takes the camera back.
const IDLE_S: f64 = 20.0;
/// How long recently shown places count against being shown again.
const NOVELTY_S: f64 = 90.0;
/// The view span the zoom limit allows at first (metres).
const START_SPAN: f64 = 1500.0;
/// How long a happening keeps drawing the director (seconds).
const HAPPENED_S: f64 = 15.0;

/// What each kind of happening is worth to the director, at first.
fn worth(h: dft_sim::swarm::Happening) -> f32 {
    use dft_sim::swarm::Happening::*;
    match h {
        HaulerLift => 80.0,
        NewFlock => 300.0,
        Merge => 150.0,
        Explosion => 300.0,
        Breach => 250.0,
        RichOre => 25.0,
        Losses => 40.0,
    }
}

fn name(h: dft_sim::swarm::Happening) -> &'static str {
    use dft_sim::swarm::Happening::*;
    match h {
        HaulerLift => "hauler lifting off",
        NewFlock => "new flock",
        Merge => "flocks merging",
        Explosion => "explosion",
        Breach => "breach",
        RichOre => "rich ore",
        Losses => "drones lost",
    }
}

/// The closest and furthest the director frames action (metres per pixel).
const ACTION_MPP: (f64, f64) = (0.15, 1.2);

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Activity>().add_systems(Startup, setup).add_systems(PreUpdate, (limit, direct).chain().after(crate::view::Controls));
    }
}

/// Recent ground changes by patch (decaying), fed by the tiles as the
/// simulation reports them.
#[derive(Resource, Default)]
pub struct Activity {
    changes: HashMap<(i64, i64), f32>,
}

impl Activity {
    /// A 64-cell block changed.
    pub fn block(&mut self, bx: i64, by: i64) {
        let k = (((bx * 64) as f64 / PATCH).floor() as i64, ((by * 64) as f64 / PATCH).floor() as i64);
        *self.changes.entry(k).or_insert(0.0) += 1.0;
    }
}

#[derive(Clone, Copy, Debug)]
struct Shot {
    /// Where it looks (drifts with the action) and how close.
    x: f64,
    y: f64,
    mpp: f64,
    began: f64,
    score: f32,
    /// The patch it was chosen for.
    patch: (i64, i64),
    whole: bool,
}

#[derive(Clone, Copy, Debug)]
struct Move {
    from: (f64, f64, f64),
    began: f64,
    secs: f64,
    /// The furthest out (log mpp) the arc reaches, over the straight path.
    lift: f64,
}

#[derive(Resource)]
pub struct Camera {
    /// The director drives (C toggles it; off by default, and never with a
    /// fixed --view or a benchmark script).
    pub director: bool,
    /// Whether C may turn the director on (not in benchmarks or screenshots).
    pub can_direct: bool,
    /// The player's last input (real seconds); None: the director drives.
    pub manual_at: Option<f64>,
    /// The zoom limit on, and the furthest out it allows now (m per pixel).
    pub limit_on: bool,
    pub max_mpp: f64,
    shot: Option<Shot>,
    moving: Option<Move>,
    shown: VecDeque<(f64, f64, f64)>,
    last_eval: f64,
    hubs_seen: usize,
    /// Happenings lately: what, where, when.
    happened: Vec<(dft_sim::swarm::Happening, f64, f64, f64)>,
    pub status: String,
    /// What drew the director to the current shot.
    why: &'static str,
}

impl Camera {
    /// The player moved the camera.
    pub fn manual(&mut self, now: f64) {
        self.manual_at = Some(now);
        self.moving = None;
    }

    /// Hand the camera back to the director now.
    pub fn resume(&mut self) {
        self.manual_at = None;
        self.shot = None;
    }
}

fn setup(mut commands: Commands, o: Res<crate::Options>) {
    commands.insert_resource(Camera {
        director: o.view.is_none() && o.camera.is_none() && o.screenshot.is_none() && !o.no_director,
        can_direct: o.camera.is_none() && o.screenshot.is_none(),
        manual_at: None,
        limit_on: !o.free_zoom && o.camera.is_none(),
        max_mpp: f64::INFINITY,
        shot: None,
        moving: None,
        shown: VecDeque::new(),
        last_eval: -10.0,
        hubs_seen: 0,
        happened: Vec::new(),
        status: String::new(),
        why: "",
    });
}

/// How far out the player may zoom: a span that grows with what has been
/// charted, up to the whole body.
fn limit(mut cam: ResMut<Camera>, view: Res<View>, body: Res<crate::Body>, fog: Res<crate::fog::Fog>, run: Res<crate::Run>) {
    let whole = {
        let mut v = *view;
        v.fit(&body.0);
        v.mpp * 2.0
    };
    if !cam.limit_on {
        cam.max_mpp = whole;
        return;
    }
    let (eaten, launched) = {
        let out = run.0.out.lock();
        (out.stats.consumed, out.stats.launched_at.is_some())
    };
    let charted_m2 = fog.charted() as f64 * crate::fog::CELL * crate::fog::CELL;
    let span = START_SPAN.max(4.0 * charted_m2.sqrt());
    cam.max_mpp = if launched || eaten > 0.33 { whole } else { (span / view.w.min(view.h).max(1.0)).min(whole) };
}

fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[allow(clippy::too_many_arguments)]
fn direct(
    mut cam: ResMut<Camera>,
    mut view: ResMut<View>,
    mut activity: ResMut<Activity>,
    fleet: Res<crate::drones::Fleet>,
    run: Res<crate::Run>,
    body: Res<crate::Body>,
    time: Res<Time<Real>>,
    mut perf: ResMut<crate::log::Perf>,
) {
    let now = time.elapsed_secs_f64();
    let dt = time.delta_secs_f64();
    // Recent changes fade (half-life about 5 s).
    let fade = 0.5f32.powf(dt as f32 / 5.0);
    activity.changes.retain(|_, v| {
        *v *= fade;
        *v > 0.05
    });
    let (t_game, new, launched) = {
        let mut out = run.0.out.lock();
        (out.stats.time, std::mem::take(&mut out.happenings), out.stats.launched_at.is_some())
    };
    // What the swarm reports happening is worth a look for a while.
    for (what, x, y) in new {
        if cam.happened.len() < 3000 {
            cam.happened.push((what, x, y, now));
        }
    }
    cam.happened.retain(|h| now - h.3 < HAPPENED_S);
    let _ = cam.hubs_seen;
    if !cam.director {
        cam.status = "fixed".into();
        return;
    }
    if let Some(t) = cam.manual_at {
        if now - t < IDLE_S {
            cam.status = format!("manual (director back in {:.0} s, or middle click)", IDLE_S - (now - t));
            return;
        }
        cam.resume();
    }
    // Choose shots once a second.
    if now - cam.last_eval >= 1.0 {
        cam.last_eval = now;
        // What's going on where: drones (a sample, if the swarm is huge) by
        // patch, cutting worth more than hauling.
        let mut score: HashMap<(i64, i64), (f32, f64, f64, f32)> = HashMap::new();
        let legs = &fleet.legs;
        let stride = (legs.len() / 40_000).max(1);
        let w = stride as f32;
        for leg in legs.iter().step_by(stride) {
            if leg.state > 2 || leg.state == 0 {
                continue;
            }
            let (x, y) = leg.at(t_game);
            let k = ((x / PATCH).floor() as i64, (y / PATCH).floor() as i64);
            let e = score.entry(k).or_insert((0.0, 0.0, 0.0, 0.0));
            let v = if leg.state == 1 { 1.0 } else { 0.5 } * w;
            e.0 += v;
            e.1 += x * v as f64;
            e.2 += y * v as f64;
            e.3 += v;
        }
        for (&k, &c) in &activity.changes {
            let e = score.entry(k).or_insert((0.0, 0.0, 0.0, 0.0));
            e.0 += 3.0 * c;
        }
        // Happenings count fully only where drones are at work now (what
        // happened in an empty patch has nothing left to see).
        let busy: HashMap<(i64, i64), f32> = score.iter().map(|(k, e)| (*k, e.3)).collect();
        // Per patch, the happening that counts most there (for the status line).
        let mut why: HashMap<(i64, i64), (f32, dft_sim::swarm::Happening)> = HashMap::new();
        for &(what, x, y, t) in &cam.happened {
            let k = ((x / PATCH).floor() as i64, (y / PATCH).floor() as i64);
            let e = score.entry(k).or_insert((0.0, 0.0, 0.0, 0.0));
            let here = busy.get(&k).copied().unwrap_or(0.0);
            let v = worth(what) * (1.0 - ((now - t) / HAPPENED_S) as f32) * if here >= 3.0 { 1.0 } else { 0.15 };
            e.0 += v;
            if e.3 == 0.0 {
                e.1 = x;
                e.2 = y;
                e.3 = 1.0;
            }
            let w = why.entry(k).or_insert((0.0, what));
            if v > w.0 {
                *w = (v, what);
            }
        }
        // Places shown lately count for less.
        let novelty = |x: f64, y: f64, shown: &VecDeque<(f64, f64, f64)>| -> f32 {
            let mut keep = 1.0f32;
            for &(sx, sy, t) in shown {
                let d2 = (x - sx).powi(2) + (y - sy).powi(2);
                let near = (-d2 / (2.0 * (2.0 * PATCH).powi(2))).exp() as f32;
                let recent = (1.0 - (now - t) / NOVELTY_S).max(0.0) as f32;
                keep = keep.min(1.0 - 0.75 * near * recent);
            }
            keep
        };
        let centre = |k: (i64, i64), e: &(f32, f64, f64, f32)| if e.3 > 0.0 { (e.1 / e.3 as f64, e.2 / e.3 as f64) } else { ((k.0 as f64 + 0.5) * PATCH, (k.1 as f64 + 0.5) * PATCH) };
        let best = score
            .iter()
            .map(|(k, e)| {
                let (x, y) = centre(*k, e);
                (*k, e.0 * novelty(x, y, &cam.shown), x, y)
            })
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)));
        // How the current shot is doing (its own patch, without the novelty
        // penalty it gave itself).
        if let Some(s) = cam.shot {
            if !s.whole {
                let cur = score.get(&s.patch).map_or(0.0, |e| e.0);
                cam.shot.as_mut().unwrap().score = cur;
                // Drift with the action.
                if let Some(e) = score.get(&s.patch) {
                    let (x, y) = centre(s.patch, e);
                    let sh = cam.shot.as_mut().unwrap();
                    sh.x += (x - sh.x) * 0.25;
                    sh.y += (y - sh.y) * 0.25;
                }
            }
        }
        let held = cam.shot.map_or(f64::INFINITY, |s| now - s.began);
        let want_whole = launched;
        let next = if want_whole {
            if cam.shot.is_some_and(|s| s.whole) { None } else { Some(((i64::MIN, 0), 1.0f32, 0.0, 0.0, true)) }
        } else if let Some((k, sc, x, y)) = best {
            let cur = cam.shot.map_or(0.0, |s| s.score);
            let cut = cam.shot.is_none() || (held >= MIN_HOLD && (sc > cur * CUT_FACTOR || held >= MAX_HOLD || cur <= 0.0));
            (cut && sc > 0.0 && cam.shot.is_none_or(|s| s.patch != k || held >= MAX_HOLD)).then_some((k, sc, x, y, false))
        } else {
            None
        };
        if let Some((k, sc, x, y, whole)) = next {
            let (x, y, mpp) = if whole {
                let mut v = *view;
                v.fit(&body.0);
                (v.x, v.y, v.mpp)
            } else {
                // Frame the patch's action: its spread of drones, roughly.
                let spread = PATCH * 0.9;
                (x, y, (spread / view.w.min(view.h).max(1.0)).clamp(ACTION_MPP.0, ACTION_MPP.1))
            };
            let mpp = mpp.min(cam.max_mpp);
            // The move: longer for longer distances, with an arc out.
            let dist = ((x - view.x).powi(2) + (y - view.y).powi(2)).sqrt();
            let span_px = view.w.min(view.h).max(1.0);
            let secs = (1.6 + 0.6 * (1.0 + dist / (span_px * view.mpp.max(mpp))).log2()).clamp(1.6, 5.0);
            let peak = (dist / (0.7 * span_px)).max(view.mpp).max(mpp).min(cam.max_mpp.max(view.mpp));
            // A long way off: cut straight there rather than sweep across
            // empty ground and sky to get there.
            let far = dist > 4.0 * span_px * view.mpp.max(mpp);
            if far {
                view.x = x;
                view.y = y;
                view.mpp = mpp;
                cam.moving = None;
            } else {
                cam.moving = Some(Move { from: (view.x, view.y, view.mpp), began: now, secs, lift: peak.ln() });
            }
            cam.shot = Some(Shot { x, y, mpp, began: now, score: sc, patch: k, whole });
            cam.why = why.get(&k).map(|w| name(w.1)).unwrap_or("activity");
            if !whole {
                cam.shown.push_back((x, y, now));
                while cam.shown.len() > 12 {
                    cam.shown.pop_front();
                }
            }
            perf.events.push(("shot".into(), format!(",\"x\":{:.0},\"y\":{:.0},\"mpp\":{:.2},\"score\":{:.0},\"whole\":{whole},\"why\":\"{}\"", x, y, mpp, sc, cam.why)));
        }
    }
    // Fly the camera.
    let Some(s) = cam.shot else {
        cam.status = "director: looking".into();
        return;
    };
    if let Some(m) = cam.moving {
        let t = ((now - m.began) / m.secs).clamp(0.0, 1.0);
        let e = smooth(t);
        let (fx, fy, fm) = m.from;
        view.x = fx + (s.x - fx) * e;
        view.y = fy + (s.y - fy) * e;
        // Zoom in log space, lifted toward the arc's top midway.
        let straight = fm.ln() + (s.mpp.ln() - fm.ln()) * e;
        let lift = (m.lift - straight).max(0.0) * (std::f64::consts::PI * e).sin();
        view.mpp = (straight + lift).exp();
        if t >= 1.0 {
            cam.moving = None;
        }
    } else {
        // Hold, easing toward the drifting target.
        let k = 1.0 - (-dt / 1.5).exp();
        view.x += (s.x - view.x) * k;
        view.y += (s.y - view.y) * k;
        view.mpp = (view.mpp.ln() + (s.mpp.ln() - view.mpp.ln()) * k).exp();
    }
    cam.status = if s.whole { "director: the whole body".into() } else { format!("director: {} · shot {:.0} s, score {:.0}", cam.why, now - s.began, s.score) };
}
