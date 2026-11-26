//! The Watch panel (Benjamin, 26 Nov): a button for each specialist. One
//! click makes one at once (free, sent to the best work there is for it
//! now) and the camera glides to it, zooms in and follows it. Dragging or
//! panning the view lets go; the wheel still zooms, about the unit. If the
//! unit can't be made (no magma for a heat sink, no plants for a
//! forester...), the panel says why.
use crate::Run;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use dft_sim::production::Unit;

pub struct WatchPlugin;

impl Plugin for WatchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Watch>().add_systems(EguiPrimaryContextPass, panel).add_systems(Update, (press, answer));
    }
}

#[derive(Resource, Default)]
struct Watch {
    /// Asked for and not answered yet.
    asked: Option<Unit>,
    /// The last answer that wasn't a unit to follow, and when (real s).
    note: Option<(String, f64)>,
}

/// The buttons, in the order the units go into the ground.
const KINDS: [(Unit, &str); 5] = [(Unit::Sapper, "Sapper"), (Unit::Tunneler, "Tunneler"), (Unit::HeatSink, "Heat sink"), (Unit::Forester, "Forester"), (Unit::Abductor, "Abductor")];

/// How long a note stays up (s).
const NOTE_S: f64 = 8.0;

fn panel(mut contexts: EguiContexts, run: Option<Res<Run>>, mut watch: ResMut<Watch>, mut cam: ResMut<crate::camera::Camera>, time: Res<Time<Real>>) -> Result {
    let Some(run) = run else { return Ok(()) };
    let ctx = contexts.ctx_mut()?;
    let now = time.elapsed_secs_f64();
    egui::Window::new("Watch a unit")
        .id(egui::Id::new("watch-window"))
        .collapsible(true)
        .resizable(false)
        .anchor(egui::Align2::LEFT_BOTTOM, [10.0, -10.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (u, label) in KINDS {
                    let busy = watch.asked.is_some();
                    let following = cam.follow.is_some_and(|f| f.kind == u);
                    let b = egui::Button::new(label).selected(following);
                    if ui.add_enabled(!busy, b).on_hover_text(format!("Make a {} now and follow it", u.name())).clicked() {
                        run.0.control.lock().summon = Some(u);
                        watch.asked = Some(u);
                        watch.note = None;
                    }
                }
            });
            if let Some(f) = cam.follow {
                ui.horizontal(|ui| {
                    let gone = f.lost_at.is_some();
                    ui.label(if gone { format!("The {} is gone.", f.kind.name()) } else { format!("Following the {}. Drag or pan to let go.", f.kind.name()) });
                    if ui.small_button("let go").clicked() {
                        cam.follow = None;
                        cam.manual(now);
                    }
                });
            } else if let Some(u) = watch.asked {
                ui.label(format!("Making a {}...", u.name()));
            }
            if let Some((n, t)) = &watch.note {
                if now - t < NOTE_S {
                    ui.colored_label(egui::Color32::from_rgb(240, 170, 90), n);
                }
            }
        });
    Ok(())
}

/// With --summon: press that unit's button once, 30 game seconds in.
fn press(o: Res<crate::Options>, run: Option<Res<Run>>, mut watch: ResMut<Watch>, mut done: Local<bool>) {
    let (Some(name), Some(run)) = (&o.summon, run) else { return };
    if *done || run.0.out.lock().stats.time < 30.0 {
        return;
    }
    *done = true;
    let Some(&(u, _)) = KINDS.iter().find(|(u, _)| !name.is_empty() && u.name().replace(['-', ' '], "").starts_with(&name.replace(['-', ' '], ""))) else {
        watch.note = Some((format!("--summon: no unit called {name}"), 0.0));
        return;
    };
    run.0.control.lock().summon = Some(u);
    watch.asked = Some(u);
}

/// The simulation's answer: follow the new unit, or say why there is none.
fn answer(run: Option<Res<Run>>, mut watch: ResMut<Watch>, mut cam: ResMut<crate::camera::Camera>, view: Res<crate::view::View>, time: Res<Time<Real>>) {
    let Some(run) = run else { return };
    let Some((u, r)) = run.0.out.lock().summoned.take() else { return };
    watch.asked = None;
    let now = time.elapsed_secs_f64();
    match r {
        Ok(id) => {
            cam.follow = Some(crate::camera::Follow::new(id, u, now, &view));
            watch.note = None;
        }
        Err(why) => {
            let mut why = why;
            if let Some(c) = why.get_mut(0..1) {
                c.make_ascii_uppercase();
            }
            watch.note = Some((format!("{why}."), now));
        }
    }
}
