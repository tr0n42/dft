//! The eyedropper: rest the mouse over the ground for two seconds and a card
//! tells you what is there: the material, what it is made of, what it is
//! worth, and how it behaves (how hard it is to cut, when it melts or boils,
//! how hot it is now). Moving the mouse puts the card away.

use crate::{Body, Run};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use dft_sim::materials::{State, VACUUM};

/// Seconds the mouse has to rest before the card shows.
const DWELL_S: f64 = 2.0;
/// Pixels the mouse may drift while resting.
const STILL_PX: f32 = 4.0;
/// How often the card's live readings (temperature) refresh, seconds.
const REFRESH_S: f64 = 0.5;

pub struct EyedropperPlugin;

impl Plugin for EyedropperPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dropper>().add_systems(EguiPrimaryContextPass, eyedropper);
    }
}

/// What the card knows about the cell under the mouse.
#[derive(Clone)]
struct Reading {
    cell: (i64, i64),
    mat: u16,
    /// Kelvin, when the simulation could be asked.
    temp_k: Option<f64>,
    /// Seconds for a cutter to cut it as things stand.
    dig_s: Option<f64>,
    at: f64,
}

#[derive(Resource, Default)]
struct Dropper {
    /// Where the mouse came to rest, and since when (real seconds).
    rest: Option<(Vec2, f64)>,
    reading: Option<Reading>,
}

fn eyedropper(mut contexts: EguiContexts, mut d: ResMut<Dropper>, windows: Query<&Window, With<PrimaryWindow>>, view: Res<crate::view::View>, body: Res<Body>, run: Option<Res<Run>>, fog: Option<Res<crate::fog::Fog>>, time: Res<Time<Real>>) -> Result {
    let now = time.elapsed_secs_f64();
    let ctx = contexts.ctx_mut()?;
    let Some(cursor) = windows.single().ok().and_then(|w| w.cursor_position()) else {
        d.rest = None;
        d.reading = None;
        return Ok(());
    };
    // Over the panel, or a button held (dragging the view): no card.
    if ctx.is_pointer_over_egui() || ctx.input(|i| i.pointer.any_down()) {
        d.rest = None;
        d.reading = None;
        return Ok(());
    }
    match d.rest {
        Some((p, _)) if p.distance(cursor) <= STILL_PX => {}
        _ => {
            d.rest = Some((cursor, now));
            d.reading = None;
        }
    }
    let since = d.rest.map_or(now, |r| r.1);
    if now - since < DWELL_S {
        return Ok(());
    }
    // The world point under the mouse.
    let x = view.x + (cursor.x as f64 - view.w / 2.0) * view.mpp;
    let y = view.y + (view.h / 2.0 - cursor.y as f64) * view.mpp;
    let cell = (x.floor() as i64, y.floor() as i64);
    let p = &body.0;
    if fog.as_ref().is_some_and(|f| !f.charted_at(x, y)) {
        card(ctx, cursor, |ui| {
            ui.label(egui::RichText::new("Uncharted").strong());
            ui.label("No drone has been near enough to see what is here.");
        });
        return Ok(());
    }
    // Read it (again, now and then): from the running world when it is free
    // this instant, else from the screen's copy of the dug ground.
    let stale = d.reading.as_ref().is_none_or(|r| r.cell != cell || now - r.at >= REFRESH_S);
    if stale {
        let mut r = Reading { cell, mat: VACUUM, temp_k: None, dig_s: None, at: now };
        let mut live = false;
        if let Some(run) = &run {
            if let Some(sim) = run.0.try_world() {
                let m = sim.world.material(cell.0, cell.1);
                r.mat = m;
                if m != VACUUM {
                    r.temp_k = Some(sim.sandbox.temp(&sim.world, cell, m));
                    r.dig_s = Some(sim.eco.harvester.dig_s_for(p.mats.get(m).hardness));
                }
                live = true;
            } else {
                r.mat = run.0.mirror.read().get(cell.0, cell.1).unwrap_or_else(|| p.material(cell.0, cell.1));
            }
        } else {
            r.mat = p.material(cell.0, cell.1);
        }
        // (Keep the last temperature while the simulation is busy.)
        if !live {
            if let Some(old) = d.reading.as_ref().filter(|o| o.cell == cell && o.mat == r.mat) {
                r.temp_k = old.temp_k;
                r.dig_s = old.dig_s;
            }
        }
        d.reading = Some(r);
    }
    let r = d.reading.clone().unwrap();
    let m = p.mats.get(r.mat);
    let els = &p.mats.elements;
    // How deep under the ground's surface (as generated) it lies.
    let rr = (x * x + y * y).sqrt().max(1e-9);
    let ground = p.surface_radius(x / rr, y / rr);
    let depth = ground - rr;
    card(ctx, cursor, |ui| {
        if r.mat == VACUUM {
            ui.label(egui::RichText::new("Open space").strong());
            if depth > 0.0 {
                ui.label(format!("{:.0} m below where the surface was", depth));
            } else {
                ui.label(format!("{:.0} m above the ground", -depth));
            }
            return;
        }
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgb(m.colour[0], m.colour[1], m.colour[2]));
            ui.label(egui::RichText::new(title(&m.name)).strong().size(15.0));
            ui.label(egui::RichText::new(state_word(m.state)).weak());
        });
        if depth > 0.5 {
            ui.label(egui::RichText::new(format!("{:.0} m down", depth)).weak());
        }
        ui.separator();
        // What it's made of.
        ui.label(egui::RichText::new("Made of").strong());
        let mut parts: Vec<(&String, &f64)> = m.composition.iter().collect();
        parts.sort_by(|a, b| b.1.total_cmp(a.1).then(a.0.cmp(b.0)));
        if parts.is_empty() {
            ui.label("nothing anyone can name");
        }
        // Traces (under 0.1%) on one line, in ppm.
        let traces: Vec<(&String, &f64)> = parts.iter().filter(|p| *p.1 < 0.001).cloned().collect();
        parts.retain(|p| *p.1 >= 0.001);
        egui::Grid::new("dropper-made-of").num_columns(3).spacing([12.0, 2.0]).show(ui, |ui| {
            for (sym, f) in parts {
                let name = els.get(sym).map_or(sym.as_str(), |e| e.name.as_str());
                ui.label(sym.as_str());
                ui.label(name);
                ui.label(format!("{:.1}%", f * 100.0));
                ui.end_row();
            }
        });
        if !traces.is_empty() {
            let mut line = traces.iter().take(14).map(|(s, f)| format!("{s} {:.1}", **f * 1e6)).collect::<Vec<_>>().join(" · ");
            if traces.len() > 14 {
                line += " …";
            }
            ui.label(egui::RichText::new(format!("traces (ppm): {line}")).weak().small());
        }
        ui.separator();
        // What it's worth.
        ui.label(egui::RichText::new("Worth").strong());
        let per_kg = els.value_per_kg(&m.composition);
        egui::Grid::new("dropper-worth").num_columns(2).spacing([12.0, 2.0]).show(ui, |ui| {
            ui.label("value when delivered");
            ui.label(format!("{:.2}× ordinary rock", m.multiplier));
            ui.end_row();
            ui.label("one cell");
            ui.label(format!("{} kg, worth {} kg of rock", kg(m.density), kg(m.density * m.multiplier)));
            ui.end_row();
            ui.label("its elements, pure");
            ui.label(format!("{:.2} per kg", per_kg));
            ui.end_row();
        });
        ui.separator();
        // How it behaves.
        ui.label(egui::RichText::new("Properties").strong());
        egui::Grid::new("dropper-props").num_columns(2).spacing([12.0, 2.0]).show(ui, |ui| {
            ui.label("density");
            ui.label(format!("{} kg per m³", kg(m.density)));
            ui.end_row();
            if matches!(m.state, State::Solid | State::Powder) {
                ui.label("hardness");
                ui.label(match r.dig_s {
                    Some(s) => format!("{} of 10 ({:.0} s to cut a load)", m.hardness, s),
                    None => format!("{} of 10", m.hardness),
                });
                ui.end_row();
            }
            if let Some(t) = r.temp_k {
                ui.label("temperature now");
                ui.label(format!("{:.0} K ({:.0} °C)", t, t - 273.15));
                ui.end_row();
            }
            for (what, k, to) in [("melts at", m.melts_at_k, &m.melts_to), ("freezes at", m.freezes_at_k, &m.freezes_to), ("boils at", m.boils_at_k, &m.boils_to), ("condenses at", m.condenses_at_k, &m.condenses_to)] {
                if let Some(k) = k {
                    ui.label(what);
                    match to {
                        Some(t) if t != &m.name => ui.label(format!("{:.0} K ({:.0} °C), into {}", k, k - 273.15, t)),
                        _ => ui.label(format!("{:.0} K ({:.0} °C)", k, k - 273.15)),
                    };
                    ui.end_row();
                }
            }
            if m.state == State::Liquid {
                ui.label("flows");
                ui.label(if m.flow >= 0.5 { "freely" } else if m.flow >= 0.2 { "slowly" } else { "barely" });
                ui.end_row();
            }
        });
    });
    Ok(())
}

/// The card, beside the mouse (kept on screen).
fn card(ctx: &egui::Context, cursor: Vec2, add: impl FnOnce(&mut egui::Ui)) {
    let screen = ctx.content_rect();
    // Below right of the mouse; on the other side of it near the right or
    // bottom edge (the card's own corner pinned beside the mouse).
    let right = cursor.x + 330.0 > screen.max.x;
    let low = cursor.y + 380.0 > screen.max.y;
    let pos = egui::pos2(if right { cursor.x - 14.0 } else { cursor.x + 18.0 }, if low { cursor.y - 14.0 } else { cursor.y + 18.0 });
    let pivot = match (right, low) {
        (false, false) => egui::Align2::LEFT_TOP,
        (true, false) => egui::Align2::RIGHT_TOP,
        (false, true) => egui::Align2::LEFT_BOTTOM,
        (true, true) => egui::Align2::RIGHT_BOTTOM,
    };
    egui::Area::new(egui::Id::new("eyedropper")).fixed_pos(pos).pivot(pivot).order(egui::Order::Tooltip).interactable(false).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.set_max_width(300.0);
            add(ui);
        });
    });
}

fn title(name: &str) -> String {
    let mut c = name.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

fn state_word(s: State) -> &'static str {
    match s {
        State::Solid => "solid",
        State::Powder => "loose",
        State::Liquid => "liquid",
        State::Gas => "gas",
    }
}

/// Kilograms, readably.
fn kg(v: f64) -> String {
    if v >= 10_000.0 {
        format!("{:.0},{:03.0}", (v / 1000.0).floor(), v % 1000.0)
    } else {
        format!("{:.0}", v)
    }
}
