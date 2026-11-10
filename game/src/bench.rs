//! The scripted camera, for benchmarks: a list of shots, each held for some
//! seconds of real time, after which the game closes. Each shot is logged as
//! a "camera" event, so a log can be read shot by shot.
//!
//! Shots (`--camera grow:20,far:8,sweep:12,mid:10,near:10,pan:15,far:5`):
//!   grow   the whole body, while the swarm grows (a warm-up)
//!   far    the whole body
//!   mid    the busiest landing point, cells 1.5 m to the pixel (loose
//!          matter moves on screen: the most simulated detail)
//!   near   the busiest landing point, close enough to see single drones
//!   sweep  zoom from the whole body down to near and back out (every level
//!          of tiles gets built)
//!   pan    at mid zoom, sweep along the surface half a screen a second
//!          (waking what waited out of sight, building fresh tiles; not in
//!          the standard plans: circling the body is disorienting to watch)
//!   save   save the game (to saves/bench.dft), whole body in view
//!   load   load that save back (once it is written) and carry on
use crate::view::View;
use bevy::prelude::*;

pub const DEFAULT_PLAN: &str = "grow:20,far:8,sweep:12,mid:10,near:10,far:5";
const MID_MPP: f64 = 1.5;
const NEAR_MPP: f64 = 0.15;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shot {
    Grow,
    Far,
    Mid,
    Near,
    Sweep,
    Pan,
    Save,
    Load,
    /// Keep the view as it is and save a picture of the window (logs folder).
    Snap,
}

impl Shot {
    fn name(self) -> &'static str {
        match self {
            Shot::Grow => "grow",
            Shot::Far => "far",
            Shot::Mid => "mid",
            Shot::Near => "near",
            Shot::Sweep => "sweep",
            Shot::Pan => "pan",
            Shot::Save => "save",
            Shot::Load => "load",
            Shot::Snap => "snap",
        }
    }
}

/// Read a plan: "shot:seconds,shot:seconds,...".
pub fn parse(plan: &str) -> Result<Vec<(Shot, f64)>, String> {
    plan.split(',')
        .filter(|s| !s.trim().is_empty())
        .map(|step| {
            let (n, s) = step.split_once(':').ok_or(format!("camera step {step}: needs shot:seconds"))?;
            let shot = match n.trim() {
                "grow" => Shot::Grow,
                "far" => Shot::Far,
                "mid" => Shot::Mid,
                "near" => Shot::Near,
                "sweep" => Shot::Sweep,
                "pan" => Shot::Pan,
                "save" => Shot::Save,
                "load" => Shot::Load,
                "snap" => Shot::Snap,
                other => return Err(format!("unknown camera shot {other}")),
            };
            let secs: f64 = s.trim().parse().map_err(|_| format!("camera step {step}: seconds"))?;
            Ok((shot, secs))
        })
        .collect()
}

#[derive(Resource)]
pub struct Script {
    steps: Vec<(Shot, f64)>,
    /// The step under way, when it began (real seconds), and where it aims.
    at: usize,
    began: f64,
    target: (f64, f64),
    started: bool,
    /// This shot's save or load has been asked for.
    acted: bool,
}

impl Script {
    pub fn new(steps: Vec<(Shot, f64)>) -> Self {
        Script { steps, at: 0, began: 0.0, target: (0.0, 0.0), started: false, acted: false }
    }
}

pub struct BenchPlugin;

impl Plugin for BenchPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, direct.run_if(resource_exists::<Script>));
    }
}

/// Where the swarm is busiest: the hub most drones call home.
fn busiest(run: &crate::Run, body: &dft_sim::planet::Planet) -> (f64, f64) {
    let out = run.0.out.lock();
    out.hubs.iter().max_by_key(|h| h.3).map(|h| (h.0, h.1)).unwrap_or((0.0, body.max_radius))
}

fn fit_mpp(view: &View, body: &dft_sim::planet::Planet) -> f64 {
    let mut v = *view;
    v.fit(body);
    v.mpp
}

fn direct(
    mut script: ResMut<Script>,
    time: Res<Time<Real>>,
    mut view: ResMut<View>,
    run: Res<crate::Run>,
    body: Res<crate::Body>,
    mut perf: ResMut<crate::log::Perf>,
    mut exit: MessageWriter<AppExit>,
    mut saves: ResMut<crate::saves::Saves>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs_f64();
    if !script.started {
        script.started = true;
        script.began = now;
        script.at = usize::MAX;
    }
    // Move on to the next shot when this one's time is up.
    let due = script.at == usize::MAX || now - script.began >= script.steps[script.at].1;
    if due {
        let next = if script.at == usize::MAX { 0 } else { script.at + 1 };
        if next >= script.steps.len() {
            perf.events.push(("camera".into(), ",\"shot\":\"end\"".into()));
            exit.write(AppExit::Success);
            return;
        }
        script.at = next;
        script.began = now;
        script.acted = false;
        script.target = busiest(&run, &body.0);
        let (shot, secs) = script.steps[next];
        perf.events.push((
            "camera".into(),
            format!(",\"shot\":\"{}\",\"secs\":{},\"x\":{:.0},\"y\":{:.0}", shot.name(), secs, script.target.0, script.target.1),
        ));
    }
    let (shot, secs) = script.steps[script.at];
    let t = (now - script.began) / secs.max(1e-6);
    let (tx, ty) = script.target;
    let far = fit_mpp(&view, &body.0);
    match shot {
        Shot::Grow | Shot::Far => view.fit(&body.0),
        Shot::Snap => {
            if !script.acted {
                script.acted = true;
                let n = script.at;
                let path = crate::log::log_dir().join(format!("snap-{n}.png"));
                commands.spawn(bevy::render::view::screenshot::Screenshot::primary_window()).observe(bevy::render::view::screenshot::save_to_disk(path));
            }
        }
        Shot::Save => {
            view.fit(&body.0);
            if !script.acted {
                script.acted = true;
                saves.wanted = Some(crate::saves::Slot::Named("bench".into()));
            }
        }
        Shot::Load => {
            view.fit(&body.0);
            // Wait for the save to be written first.
            if !script.acted && !saves.writing() && saves.wanted.is_none() {
                script.acted = true;
                saves.load(crate::saves::save_dir().join("bench.dft"));
            }
        }
        Shot::Mid => {
            view.x = tx;
            view.y = ty;
            view.mpp = MID_MPP;
        }
        Shot::Near => {
            view.x = tx;
            view.y = ty;
            view.mpp = NEAR_MPP;
        }
        Shot::Sweep => {
            // In over the first half, out over the second; the centre slides
            // from the body's middle to the target as the zoom deepens.
            let u = if t < 0.5 { t * 2.0 } else { (1.0 - t) * 2.0 };
            let u = u.clamp(0.0, 1.0);
            view.mpp = (far.ln() + (NEAR_MPP.ln() - far.ln()) * u).exp();
            let k = 1.0 - view.mpp / far;
            view.x = tx * k;
            view.y = ty * k;
        }
        Shot::Pan => {
            // Along the surface through the target, half a screen a second.
            let r = (tx * tx + ty * ty).sqrt().max(1.0);
            let a0 = ty.atan2(tx);
            let speed = 0.5 * view.w * MID_MPP / r;
            let a = a0 + speed * (now - script.began);
            view.mpp = MID_MPP;
            view.x = r * a.cos();
            view.y = r * a.sin();
        }
    }
}
