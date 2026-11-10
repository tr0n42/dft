//! The developer panel (F1). A tool for us, not part of the game's look: the
//! frame rate, what the simulation costs, the swarm's numbers, the speed, and
//! pickers to build any body at any size, seed and glamour.
use crate::sim::SPEEDS;
use crate::{Body, Recipe, Run};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};

#[derive(Resource)]
struct Dev {
    open: bool,
    bodies: Vec<String>,
    recipe: Recipe,
    /// Frame times over the last half second (seconds).
    frames: Vec<f64>,
    shown: (f64, f64),
    since: f64,
}

pub struct DevPlugin;

impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default()).add_systems(Startup, setup).add_systems(Update, (toggle, measure)).add_systems(EguiPrimaryContextPass, panel);
    }
}

/// The body types to offer: everything in data/bodies but the test and demo files.
fn body_types() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(crate::data_dir().join("bodies"))
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter_map(|e| e.path().file_stem().map(|s| s.to_string_lossy().into_owned()))
                .filter(|n| !n.contains("demo") && !n.contains("test"))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn setup(mut commands: Commands, o: Res<crate::Options>, body: Res<Body>) {
    let p = &body.0;
    commands.insert_resource(Dev {
        open: o.dev,
        bodies: body_types(),
        recipe: Recipe { body: body.1.name.clone(), seed: Some(p.spec.seed), radius_km: Some(p.radius / 1000.0), glamour: body.1.glamour },
        frames: Vec::new(),
        shown: (0.0, 0.0),
        since: 0.0,
    });
}

fn toggle(keys: Res<ButtonInput<KeyCode>>, mut dev: ResMut<Dev>) {
    if keys.just_pressed(KeyCode::F1) {
        dev.open = !dev.open;
    }
}

fn measure(time: Res<Time>, mut dev: ResMut<Dev>) {
    let dt = time.delta_secs_f64();
    dev.frames.push(dt);
    dev.since += dt;
    if dev.since >= 0.5 {
        let n = dev.frames.len().max(1) as f64;
        let avg = dev.frames.iter().sum::<f64>() / n;
        let worst = dev.frames.iter().cloned().fold(0.0, f64::max);
        dev.shown = (avg, worst);
        dev.frames.clear();
        dev.since = 0.0;
    }
}

fn clock(t: f64) -> String {
    let s = t.max(0.0) as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

fn speed_name(i: usize) -> String {
    let s = SPEEDS[i];
    if s.is_finite() { format!("{s}x") } else { "max".into() }
}

#[allow(clippy::too_many_arguments)]
fn panel(contexts: EguiContexts, dev: ResMut<Dev>, run: Res<Run>, tiles: Res<crate::tiles::Tiles>, view: Res<crate::view::View>, body: Res<Body>, fog: ResMut<crate::fog::Fog>, cam: ResMut<crate::camera::Camera>, saves: ResMut<crate::saves::Saves>, mut perf: ResMut<crate::log::Perf>) -> Result {
    let t0 = std::time::Instant::now();
    let r = panel_inner(contexts, dev, run, tiles, view, body, fog, cam, saves);
    perf.add(crate::log::P_PANEL, t0);
    r
}

#[allow(clippy::too_many_arguments)]
fn panel_inner(mut contexts: EguiContexts, mut dev: ResMut<Dev>, run: Res<Run>, tiles: Res<crate::tiles::Tiles>, view: Res<crate::view::View>, body: Res<Body>, mut fog: ResMut<crate::fog::Fog>, mut cam: ResMut<crate::camera::Camera>, mut saves: ResMut<crate::saves::Saves>) -> Result {
    if !dev.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let stats = run.0.out.lock().stats.clone();
    let mut control = *run.0.control.lock();
    let before = control;
    egui::Window::new("dev").default_pos([12.0, 12.0]).min_width(440.0).resizable(false).show(ctx, |ui| {
        let (avg, worst) = dev.shown;
        ui.monospace(format!("frame   {:5.1} ms  ({:.0} fps)  worst {:.1} ms", avg * 1000.0, 1.0 / avg.max(1e-6), worst * 1000.0));
        ui.separator();
        ui.monospace(format!("time    {}   running at {:.1}x", clock(stats.time), stats.speed));
        ui.monospace(format!("cost    {:.1} ms computing per game second", stats.cost_ms));
        let sp = stats.split;
        ui.monospace(format!("  spent drones {:.0}% · sand {:.0}% · rocks {:.0}% · other {:.0}%", sp[0] * 100.0, sp[1] * 100.0, sp[2] * 100.0, sp[3] * 100.0));
        ui.monospace(format!("drones  {}  + packet {}   hubs {}   lost {}", stats.drones, stats.packet, stats.hubs, stats.deaths));
        ui.monospace(format!("haulers {}  ({} away)   cutters waiting {}   starved {}", stats.haulers, stats.haulers_away, stats.waiting, stats.starved));
        ui.monospace(format!("resting {} out of work, in the mother swarm (not drawn)", stats.resting));
        ui.monospace(format!("flocks  {}   splits {}  merges {}  helpers sent {}   booms {}", stats.flocks, stats.splits, stats.merges, stats.helpers, stats.explosions));
        ui.monospace(format!("mother  {:.0}% of the way round", stats.mother_share * 100.0));
        ui.monospace(format!("heat    {} cutters melted · {} cells burned away · {} hauler moves from lava", stats.melted, stats.burned_cells, stats.site_moves));
        ui.monospace(format!("vents   {} cells blasted out · {} drops in the air", stats.erupted, stats.drops));
        ui.collapsing(format!("director ({} haulers built)", stats.haulers_built), |ui| {
            if stats.director.is_empty() {
                ui.monospace("no calls yet");
            }
            for l in &stats.director {
                ui.monospace(l);
            }
        });
        ui.monospace(format!("eaten   {:.3}%", stats.consumed * 100.0));
        ui.monospace(format!("moving  {} cells awake, {} waiting out of sight, {} loose bodies", stats.awake, stats.dormant, stats.loose));
        if control.free_drones.is_none() {
            ui.monospace(format!("store   {:.0} E energy, {:.0} kg mass", stats.energy, stats.mass));
        }
        if let Some(t) = stats.launched_at {
            ui.monospace(format!("launched at {}", clock(t)));
        }
        ui.horizontal(|ui| {
            if ui.selectable_label(control.paused, "pause").clicked() {
                control.paused = !control.paused;
            }
            for i in 0..SPEEDS.len() {
                if ui.selectable_label(!control.paused && control.speed == i, speed_name(i)).clicked() {
                    control.speed = i;
                    control.paused = false;
                }
            }
        });
        ui.horizontal(|ui| {
            let mut free = control.free_drones.is_some();
            if ui.checkbox(&mut free, "economy off, grow to").changed() {
                control.free_drones = if free { Some(20000) } else { None };
            }
            if let Some(n) = &mut control.free_drones {
                let drag = (*n as f64 * 0.01).max(1.0);
                ui.add(egui::DragValue::new(n).range(10..=1_000_000).speed(drag));
                ui.label("drones");
            }
        });
        ui.checkbox(&mut control.full_detail, "full detail everywhere (loose matter moves out of sight too)");
        ui.horizontal(|ui| {
            ui.checkbox(&mut cam.director, "action cam");
            ui.checkbox(&mut cam.limit_on, "zoom limit");
        });
        ui.monospace(format!("camera  {}", cam.status));
        let charted = fog.charted();
        ui.checkbox(&mut fog.on, format!("fog of war (F2) · {} km² charted", charted as f64 * crate::fog::CELL * crate::fog::CELL / 1e6));
        ui.separator();
        let (kept, building, level) = tiles.counts();
        ui.monospace(format!("view    {:.0}, {:.0} m   {:.3} m/px", view.x, view.y, view.mpp));
        ui.monospace(format!("tiles   level {level}, {kept} kept, {building} building"));
        ui.monospace(format!("body    {} · {:.1} km · seed {}", body.0.spec.name, body.0.radius / 1000.0, body.0.spec.seed));
        ui.separator();
        let dev = &mut *dev;
        egui::Grid::new("recipe").num_columns(2).show(ui, |ui| {
            ui.label("body");
            egui::ComboBox::from_id_salt("body").selected_text(dev.recipe.body.clone()).show_ui(ui, |ui| {
                for b in &dev.bodies {
                    ui.selectable_value(&mut dev.recipe.body, b.clone(), b);
                }
            });
            ui.end_row();
            ui.label("radius km");
            let mut km = dev.recipe.radius_km.unwrap_or(1.0);
            let drag = km * 0.01;
            ui.add(egui::DragValue::new(&mut km).range(0.05..=20000.0).speed(drag).max_decimals(2));
            dev.recipe.radius_km = Some(km);
            ui.end_row();
            ui.label("seed");
            ui.horizontal(|ui| {
                let mut seed = dev.recipe.seed.unwrap_or(1);
                ui.add(egui::DragValue::new(&mut seed));
                if ui.button("random").clicked() {
                    seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64 % 1_000_000_000).unwrap_or(7);
                }
                dev.recipe.seed = Some(seed);
            });
            ui.end_row();
            ui.label("glamour");
            ui.add(egui::Slider::new(&mut dev.recipe.glamour, 0.0..=1.0).step_by(0.05));
            ui.end_row();
        });
        ui.horizontal(|ui| {
            let busy = saves.busy();
            if ui.add_enabled(!busy, egui::Button::new("build")).clicked() {
                saves.build(dev.recipe.clone());
            }
            if busy {
                ui.label("working...");
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            let can = !saves.busy() && !saves.writing();
            if ui.add_enabled(can, egui::Button::new("save (F5)")).clicked() {
                saves.wanted = Some(crate::saves::Slot::Named("quicksave".into()));
            }
            if ui.add_enabled(can, egui::Button::new("save as new")).clicked() {
                let n = format!("save-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0));
                saves.wanted = Some(crate::saves::Slot::Named(n));
            }
            ui.checkbox(&mut saves.autosave, "autosave");
            if ui.small_button("refresh").clicked() {
                saves.refresh();
            }
        });
        if !saves.status.is_empty() {
            ui.small(saves.status.clone());
        }
        if let Some(e) = &saves.error {
            ui.colored_label(egui::Color32::from_rgb(240, 120, 90), e);
        }
        let mut load = None;
        egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
            for e in &saves.list {
                ui.horizontal(|ui| {
                    if ui.add_enabled(!saves.busy(), egui::Button::new("load").small()).clicked() {
                        load = Some(e.path.clone());
                    }
                    ui.small(format!("{} · {} · {:.1} MB", e.name, e.summary, e.bytes as f64 / 1e6));
                });
            }
            if saves.list.is_empty() {
                ui.small("no saves yet");
            }
        });
        if let Some(p) = load {
            saves.load(p);
        }
        if let Some(p) = crate::log::current() {
            ui.small(format!("logging to logs/{}", p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
        }
        ui.separator();
        ui.small("F1 hides this · Space pause · 1-8 speed · +/- step · Home whole body · F5 save · F9 load newest");
    });
    if control.paused != before.paused || control.speed != before.speed || control.free_drones != before.free_drones || control.full_detail != before.full_detail {
        let mut c = run.0.control.lock();
        c.paused = control.paused;
        c.speed = control.speed;
        c.free_drones = control.free_drones;
        c.full_detail = control.full_detail;
    }
    Ok(())
}
