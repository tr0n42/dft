//! Flora viewer: a live window for browsing the plants each world grows.
//!
//! Pick a body type and seed (and glamour); the world is built and one of
//! its plants is grown and shown, moving with its idle motion. Scroll the
//! mouse wheel (or press the arrow keys) to step through plants from the same
//! world, "Regrow" grows another individual of the same species, and
//! "Attack" plays the species' defence when it has one. A world where
//! nothing can live says so, and why.
//!
//! Options (mostly for testing): --body <name> --seed <n> --glamour <g>
//! --plants <n> (step forward n plants) --attack (start an attack)
//! --shot <file.png> (save a picture after a moment and quit).
use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use dft_sim::flora::{Defence, Motion, Niche, Plan};
use dft_sim::materials::{MatId, MaterialTable};
use dft_sim::noise::{hash, unit};
use dft_sim::planet::Planet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

fn data_dir() -> PathBuf {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("data");
    if here.exists() {
        return here;
    }
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("data"))).unwrap_or_else(|| PathBuf::from("data"))
}

/// The body types to offer: everything in data/bodies but tests and demos.
fn body_types() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(data_dir().join("bodies"))
        .map(|d| d.filter_map(|e| e.ok()).filter_map(|e| e.path().file_stem().map(|s| s.to_string_lossy().into_owned())).filter(|n| !n.contains("demo") && !n.contains("test") && !n.starts_with("tutorial")).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// One grown plant on screen.
struct Specimen {
    species: usize,
    seed: u64,
    /// (x, y, colour, part) with y up from the ground (down for hanging plants).
    cells: Vec<(i64, i64, [u8; 3], u8)>,
}

struct World {
    planet: Planet,
}

#[derive(Resource)]
struct Viewer {
    bodies: Vec<String>,
    body: String,
    seed: u64,
    glamour: f32,
    mats: Option<MaterialTable>,
    world: Option<Arc<World>>,
    building: Option<Arc<Mutex<Option<Result<World, String>>>>>,
    error: Option<String>,
    /// Plants seen so far on this world, and which one is shown.
    history: Vec<Specimen>,
    at: usize,
    /// When the attack started (seconds), if one is playing.
    attack: Option<f64>,
    scroll: f32,
    // Testing.
    shot: Option<PathBuf>,
    plants: usize,
    species: Option<usize>,
    auto_attack: bool,
    frames: u32,
}

const ATTACK_S: f64 = 1.8;

impl Viewer {
    fn rebuild(&mut self) {
        let Some(mats) = self.mats.clone() else { return };
        let (body, seed, glamour) = (self.body.clone(), self.seed, self.glamour as f64);
        let slot: Arc<Mutex<Option<Result<World, String>>>> = Arc::new(Mutex::new(None));
        let tx = slot.clone();
        std::thread::spawn(move || {
            let r = (|| -> Result<World, String> {
                let text = std::fs::read_to_string(data_dir().join("bodies").join(format!("{body}.toml"))).map_err(|e| format!("{body}: {e}"))?;
                let mut spec = dft_sim::spec::BodySpec::parse(&text)?;
                spec.seed = seed;
                spec.glamour = Some(glamour);
                spec.life = Some(1.0);
                Ok(World { planet: Planet::new(spec, mats)? })
            })();
            *tx.lock().unwrap() = Some(r);
        });
        self.building = Some(slot);
        self.world = None;
        self.history.clear();
        self.at = 0;
        self.attack = None;
        self.error = None;
    }

    /// Grow species `si` with this seed into a specimen.
    fn grow(w: &World, si: usize, seed: u64) -> Specimen {
        let f = w.planet.flora.as_ref().unwrap();
        let cells = f.specimen(si, seed, 1.0).into_iter().map(|(x, y, m): (i64, i64, MatId)| (x, y, w.planet.palette.colours[m as usize], f.part_of(si, m))).collect();
        Specimen { species: si, seed, cells }
    }

    /// Step to the next plant, growing a new one past the end.
    fn next(&mut self) {
        let Some(w) = self.world.clone() else { return };
        if self.at + 1 < self.history.len() {
            self.at += 1;
        } else {
            let n = self.history.len() as i64;
            let f = w.planet.flora.as_ref().unwrap();
            let pick = hash(self.seed ^ 0xF1, n, 0, 0x9C);
            let Some(si) = f.pick(pick) else { return };
            self.history.push(Self::grow(&w, si, hash(pick, 1, 0, 0)));
            self.at = self.history.len() - 1;
        }
        self.attack = None;
    }

    fn prev(&mut self) {
        if self.at > 0 {
            self.at -= 1;
            self.attack = None;
        }
    }

    /// Another individual of the shown species.
    fn regrow(&mut self) {
        let Some(w) = self.world.clone() else { return };
        let Some(cur) = self.history.get(self.at) else { return };
        let s = Self::grow(&w, cur.species, hash(cur.seed, 77, 0, 0));
        self.history.insert(self.at + 1, s);
        self.at += 1;
        self.attack = None;
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let bodies = body_types();
    let mut v = Viewer {
        body: if bodies.iter().any(|b| b == "living-world") { "living-world".into() } else { bodies.first().cloned().unwrap_or_default() },
        bodies,
        seed: 24001,
        glamour: 0.4,
        mats: None,
        world: None,
        building: None,
        error: None,
        history: Vec::new(),
        at: 0,
        attack: None,
        scroll: 0.0,
        shot: None,
        plants: 0,
        species: None,
        auto_attack: false,
        frames: 0,
    };
    let mut i = 0;
    while i < a.len() {
        let val = a.get(i + 1).cloned().unwrap_or_default();
        match a[i].as_str() {
            "--body" => v.body = val,
            "--seed" => v.seed = val.parse().unwrap_or(v.seed),
            "--glamour" => v.glamour = val.parse().unwrap_or(v.glamour),
            "--plants" => v.plants = val.parse().unwrap_or(0),
            "--species" => v.species = val.parse().ok(),
            "--shot" => v.shot = Some(PathBuf::from(val)),
            "--attack" => {
                v.auto_attack = true;
                i -= 1;
            }
            _ => {}
        }
        i += 2;
    }
    match MaterialTable::load(&data_dir().join("materials.toml").to_string_lossy()) {
        Ok(m) => v.mats = Some(m),
        Err(e) => v.error = Some(e),
    }
    v.rebuild();
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "The Dark Forest Theory · Flora viewer".into(), resolution: WindowResolution::new(1400, 900), ..default() }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .insert_resource(ClearColor(Color::srgb_u8(8, 9, 12)))
        .insert_resource(v)
        .add_systems(Startup, |mut c: Commands| {
            c.spawn(Camera2d);
        })
        .add_systems(Update, (poll, test_shot))
        .add_systems(EguiPrimaryContextPass, ui)
        .run();
}

fn poll(mut v: ResMut<Viewer>) {
    let Some(slot) = &v.building else { return };
    let Some(r) = slot.lock().unwrap().take() else { return };
    v.building = None;
    match r {
        Ok(w) => {
            v.world = Some(Arc::new(w));
            v.next();
            for _ in 0..v.plants {
                v.next();
            }
            v.plants = 0;
            if let (Some(si), Some(w)) = (v.species.take(), v.world.clone()) {
                if si < w.planet.flora.as_ref().unwrap().species.len() {
                    let s = Viewer::grow(&w, si, 5);
                    v.history.push(s);
                    v.at = v.history.len() - 1;
                }
            }
        }
        Err(e) => v.error = Some(e),
    }
}

fn test_shot(mut commands: Commands, mut v: ResMut<Viewer>, time: Res<Time>, mut exit: MessageWriter<AppExit>, mut shot_at: Local<Option<f64>>) {
    let Some(path) = v.shot.clone() else { return };
    if v.building.is_some() || time.elapsed_secs_f64() < 2.0 {
        return;
    }
    let now = time.elapsed_secs_f64();
    if v.auto_attack && v.attack.is_none() && shot_at.is_none() {
        v.attack = Some(now);
    }
    match *shot_at {
        None => {
            // Mid-attack if one is playing.
            let at = if v.auto_attack { now + ATTACK_S * 0.45 } else { now + 0.5 };
            *shot_at = Some(at);
        }
        Some(t) if now >= t && v.frames == 0 => {
            commands.spawn(bevy::render::view::screenshot::Screenshot::primary_window()).observe(bevy::render::view::screenshot::save_to_disk(path));
            v.frames = 1;
        }
        Some(t) if now >= t + 1.5 => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}

fn c32(c: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(c[0], c[1], c[2])
}

fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn ui(mut contexts: EguiContexts, mut v: ResMut<Viewer>, time: Res<Time>) -> Result {
    let ctx = contexts.ctx_mut()?;
    ctx.request_repaint();
    let t = time.elapsed_secs_f64();

    // ---- Controls ----
    let mut rebuild = false;
    let (mut go_next, mut go_prev, mut regrow, mut attack) = (false, false, false, false);
    egui::Window::new("Flora viewer").anchor(egui::Align2::LEFT_TOP, [10.0, 10.0]).resizable(false).collapsible(false).default_width(300.0).show(ctx, |ui| {
        ui.label(egui::RichText::new("World").strong());
        let before = v.body.clone();
        let bodies = v.bodies.clone();
        egui::ComboBox::from_label("type").selected_text(v.body.clone()).show_ui(ui, |ui| {
            for b in &bodies {
                ui.selectable_value(&mut v.body, b.clone(), b);
            }
        });
        if v.body != before {
            rebuild = true;
        }
        ui.horizontal(|ui| {
            ui.label("seed");
            let was = v.seed;
            let r = ui.add(egui::DragValue::new(&mut v.seed).speed(1.0));
            if r.changed() && !r.dragged() && !r.has_focus() && v.seed != was {
                rebuild = true;
            }
            if (r.drag_stopped() || r.lost_focus()) && v.world.as_ref().is_some_and(|w| w.planet.spec.seed != v.seed) {
                rebuild = true;
            }
            if ui.button("-").clicked() {
                v.seed = v.seed.saturating_sub(1);
                rebuild = true;
            }
            if ui.button("+").clicked() {
                v.seed += 1;
                rebuild = true;
            }
            if ui.button("random").clicked() {
                v.seed = hash(v.seed, (t * 1000.0) as i64, 0, 0xD1CE) % 1_000_000;
                rebuild = true;
            }
        });
        let r = ui.add(egui::Slider::new(&mut v.glamour, 0.0..=1.0).text("glamour").step_by(0.05));
        if r.drag_stopped() || (r.changed() && !r.dragged()) {
            rebuild = true;
        }
        ui.separator();
        ui.label(egui::RichText::new("Plant").strong());
        ui.horizontal(|ui| {
            go_prev = ui.button("< previous").clicked();
            go_next = ui.button("next >").clicked();
            regrow = ui.button("Regrow").on_hover_text("Another of the same species").clicked();
        });
        let can_attack = v.world.as_ref().zip(v.history.get(v.at)).is_some_and(|(w, s)| !w.planet.flora.as_ref().unwrap().species[s.species].defence.is_empty());
        attack = ui.add_enabled(can_attack, egui::Button::new("Attack")).on_disabled_hover_text("This plant has no defence").clicked();
        ui.label(egui::RichText::new("Wheel or arrow keys: browse plants. Space: attack.").small().weak());
        ui.separator();
        // About the plant (or why there is none).
        if let Some(e) = &v.error {
            ui.colored_label(egui::Color32::LIGHT_RED, e);
        } else if v.building.is_some() {
            ui.label("Building the world...");
        } else if let Some(w) = &v.world {
            let f = w.planet.flora.as_ref().unwrap();
            if f.species.is_empty() {
                ui.colored_label(egui::Color32::from_rgb(240, 190, 120), "Nothing can grow on this world.");
                for line in f.why_lifeless() {
                    ui.label(egui::RichText::new(line).small());
                }
            } else if let Some(s) = v.history.get(v.at) {
                let sp = &f.species[s.species];
                ui.label(egui::RichText::new(&sp.name).heading());
                ui.label(format!("plant {} of {} seen · {} species on this world", v.at + 1, v.history.len(), f.species.len()));
                ui.label(format!("{} · {}{}", sp.niche.name(), sp.plan.name(), if sp.cover { " (ground cover)" } else { "" }));
                ui.label(format!("up to {:.0} m · lives at {:.0} to {:.0} K", sp.height, sp.min_k, sp.max_k));
                ui.label(format!("idle: {}", match sp.motion {
                    Motion::Rigid => "still",
                    Motion::Sway => "sways",
                    Motion::Ripple => "ripples",
                    Motion::Drift => "drifts",
                }));
                let d = if sp.defence.is_empty() { "none".to_string() } else { sp.defence.iter().map(|d| d.name()).collect::<Vec<_>>().join(", ") };
                ui.label(format!("defence: {d}"));
                if s.cells.is_empty() {
                    ui.colored_label(egui::Color32::from_rgb(240, 190, 120), "This one couldn't be grown here.");
                }
            }
        }
    });
    let (wheel, keys) = ctx.input(|i| (i.smooth_scroll_delta.y, (i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::Space))));
    let typing = ctx.egui_wants_keyboard_input();
    if !typing {
        go_next |= keys.0;
        go_prev |= keys.1;
        attack |= keys.2;
    }
    if !ctx.is_pointer_over_egui() {
        v.scroll += wheel;
        if v.scroll <= -40.0 {
            v.scroll = 0.0;
            go_next = true;
        } else if v.scroll >= 40.0 {
            v.scroll = 0.0;
            go_prev = true;
        }
    }
    if rebuild {
        v.rebuild();
    }
    if go_next {
        v.next();
    }
    if go_prev {
        v.prev();
    }
    if regrow {
        v.regrow();
    }
    if attack {
        let ok = v.world.as_ref().zip(v.history.get(v.at)).is_some_and(|(w, s)| !w.planet.flora.as_ref().unwrap().species[s.species].defence.is_empty());
        if ok {
            v.attack = Some(t);
        }
    }
    if v.attack.is_some_and(|a| t - a > ATTACK_S) {
        v.attack = None;
    }

    // ---- The stage ----
    let screen = ctx.content_rect();
    let stage = egui::Rect::from_min_max(egui::pos2(screen.min.x + 330.0, screen.min.y + 10.0), egui::pos2(screen.max.x - 10.0, screen.max.y - 10.0));
    let painter = ctx.layer_painter(egui::LayerId::background());
    let Some(w) = v.world.clone() else {
        painter.text(stage.center(), egui::Align2::CENTER_CENTER, if v.building.is_some() { "Growing a world..." } else { "" }, egui::FontId::proportional(22.0), egui::Color32::GRAY);
        return Ok(());
    };
    let f = w.planet.flora.as_ref().unwrap();
    if f.species.is_empty() {
        painter.text(stage.center(), egui::Align2::CENTER_CENTER, "Nothing can grow on this world", egui::FontId::proportional(26.0), egui::Color32::from_rgb(240, 190, 120));
        return Ok(());
    }
    let Some(s) = v.history.get(v.at) else { return Ok(()) };
    let sp = &f.species[s.species];
    draw(&painter, stage, sp, s, t, v.attack.map(|a| ((t - a) / ATTACK_S).clamp(0.0, 1.0)));
    Ok(())
}

/// Draw a plant in `stage`, moving with its idle motion, and its attack at
/// phase `atk` (0 to 1) if one is playing.
fn draw(painter: &egui::Painter, stage: egui::Rect, sp: &dft_sim::flora::Species, s: &Specimen, t: f64, atk: Option<f64>) {
    let roof = sp.niche == Niche::CaveRoof;
    let wet = matches!(sp.niche, Niche::Shallows | Niche::Deeps);
    let sky = matches!(sp.niche, Niche::Soil | Niche::Bare | Niche::Frozen);
    // Backdrop.
    let bg = if sky { [16, 20, 34] } else if wet { [12, 30, 52] } else { [12, 11, 14] };
    painter.rect_filled(stage, 6.0, c32(bg));
    if s.cells.is_empty() {
        painter.text(stage.center(), egui::Align2::CENTER_CENTER, "This plant couldn't be grown", egui::FontId::proportional(24.0), egui::Color32::from_rgb(240, 190, 120));
        return;
    }
    // Fit the plant (with room to sway) to the stage.
    let (mut x0, mut x1, mut y0, mut y1) = (i64::MAX, i64::MIN, i64::MAX, i64::MIN);
    for &(x, y, _, _) in &s.cells {
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    let (wd, ht) = ((x1 - x0 + 1) as f64, (y1 - y0 + 1) as f64);
    let tall = ht.max(1.0);
    let scale = ((stage.width() as f64 * 0.8) / (wd * 1.5 + 6.0)).min((stage.height() as f64 * 0.8) / (ht + 6.0)).clamp(1.0, 40.0);
    let cx = stage.center().x as f64 - ((x0 + x1) as f64 / 2.0) * scale;
    // Ground line: at the bottom for floor plants, the top for hanging ones.
    let ground_y = if roof { stage.min.y as f64 + stage.height() as f64 * 0.1 + (y1 as f64 + 1.0) * scale } else { stage.max.y as f64 - stage.height() as f64 * 0.1 + (y0.min(0) as f64) * scale };
    let to_screen = |x: f64, y: f64| egui::pos2((cx + x * scale) as f32, (ground_y - (y + 1.0) * scale) as f32);
    // The ground itself.
    let gc = c32([46, 40, 36]);
    if roof {
        let gy = to_screen(0.0, -1.0).y;
        painter.rect_filled(egui::Rect::from_min_max(egui::pos2(stage.min.x, stage.min.y), egui::pos2(stage.max.x, gy)), 0.0, gc);
    } else {
        let gy = to_screen(0.0, -1.0).y;
        // Only below the plant's base: roots show through the soil.
        painter.rect_filled(egui::Rect::from_min_max(egui::pos2(stage.min.x, gy), egui::pos2(stage.max.x, stage.max.y)), 0.0, gc);
    }

    // A scale bar: 1, 5, 10 or 50 m, whichever reads best.
    let bar_m = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0].into_iter().find(|m| m * scale >= 60.0).unwrap_or(100.0);
    let b0 = egui::pos2(stage.min.x + 20.0, stage.max.y - 20.0);
    painter.line_segment([b0, b0 + egui::vec2((bar_m * scale) as f32, 0.0)], egui::Stroke::new(2.0, egui::Color32::from_gray(200)));
    painter.text(b0 + egui::vec2(0.0, -6.0), egui::Align2::LEFT_BOTTOM, format!("{bar_m:.0} m"), egui::FontId::proportional(13.0), egui::Color32::from_gray(200));

    // ---- Motion ----
    let kind = sp.defence.first().copied();
    let p = atk.unwrap_or(0.0);
    let pulse = |a: f64, b: f64| if p < a { smooth(p / a) } else if p < b { 1.0 } else { 1.0 - smooth((p - b) / (1.0 - b)) };
    let attacking = atk.is_some();
    // The organ centroid (where traps close and pods burst).
    let (mut ox, mut oy, mut on) = (0.0, 0.0, 0.0);
    for &(x, y, _, part) in &s.cells {
        if part == 2 {
            ox += x as f64;
            oy += y as f64;
            on += 1.0;
        }
    }
    let organ_c = if on > 0.0 { (ox / on, oy / on) } else { (0.0, if roof { -tall * 0.8 } else { tall * 0.8 }) };
    let rmax = s.cells.iter().map(|c| ((c.0 * c.0 + c.1 * c.1) as f64).sqrt()).fold(1.0, f64::max);
    let phase = unit(hash(s.seed, 0, 0, 0)) * 6.28;
    let sway = sp.sway.max(0.05);
    let cell_px = (scale + 1.0).ceil() as f32;
    for &(x, y, col, part) in &s.cells {
        let (fx, fy) = (x as f64, y as f64);
        let h = (fy.abs() / tall).clamp(0.0, 1.0);
        let d = (fx * fx + fy * fy).sqrt() / rmax;
        let root = part == 3;
        let (mut dx, mut dy) = (0.0, 0.0);
        if !root {
            match sp.motion {
                Motion::Rigid => {}
                Motion::Sway => {
                    dx += sway * 0.05 * tall * (t * 0.9 + phase).sin() * h.powf(1.6) + sway * 0.03 * tall * (t * 2.3 + phase * 1.7).sin() * h * h;
                    if part == 1 {
                        dx += 0.25 * (t * 5.0 + fx * 0.7 + fy * 0.4).sin();
                    }
                }
                Motion::Ripple => dx += 0.45 * (t * 3.2 + fx * 0.6).sin() * (0.3 + h),
                Motion::Drift => {
                    dx += sway * 0.07 * rmax * (t * 1.1 - d * 4.0 + phase).sin() * d;
                    dy += sway * 0.02 * rmax * (t * 0.8 - d * 3.0 + phase).cos() * d;
                }
            }
        }
        let mut c = col;
        let mut show = true;
        // ---- Attack ----
        if let (true, Some(k)) = (attacking, kind) {
            match k {
                Defence::Whip => {
                    if !root {
                        let lash = (std::f64::consts::PI * p).sin();
                        dx += 0.45 * rmax * lash * (14.0 * p - d * 5.0).sin() * d;
                    }
                }
                Defence::Snap => {
                    if part == 2 {
                        let close = pulse(0.25, 0.7);
                        let ax = organ_c.0 + (fx - organ_c.0) * (1.0 - 0.85 * close);
                        dx += ax - fx;
                        if close > 0.9 {
                            c = [(c[0] as u16 * 3 / 4 + 60).min(255) as u8, c[1] / 2, c[2] / 2];
                        }
                    }
                }
                Defence::Pods => {
                    if part == 2 {
                        if p < 0.35 {
                            let k = 1.0 + 0.6 * smooth(p / 0.35);
                            dx += (fx - organ_c.0) * (k - 1.0);
                            dy += (fy - organ_c.1) * (k - 1.0);
                        } else if p < 0.9 {
                            show = false;
                        }
                    }
                }
                Defence::Thorns => {
                    if part == 1 || part == 0 {
                        let jab = pulse(0.15, 0.5);
                        dx += (fx - 0.0).signum() * 0.5 * jab + 0.15 * (p * 60.0 + fy).sin();
                        if part == 1 {
                            c = [((c[0] as f64) + (220.0 - c[0] as f64) * jab * 0.6) as u8, (c[1] as f64 * (1.0 - 0.4 * jab)) as u8, (c[2] as f64 * (1.0 - 0.4 * jab)) as u8];
                        }
                    }
                }
                Defence::Gas | Defence::Caustic | Defence::Resin => {
                    if !root {
                        dx += 0.12 * (p * 70.0 + fy).sin();
                    }
                }
                Defence::Flash => {
                    let f = pulse(0.2, 0.35);
                    c = [(c[0] as f64 + (255.0 - c[0] as f64) * f) as u8, (c[1] as f64 + (255.0 - c[1] as f64) * f) as u8, (c[2] as f64 + (240.0 - c[2] as f64) * f) as u8];
                }
            }
        }
        // Lanterns and crystal glints breathe.
        if part == 2 && (sp.tip == dft_sim::flora::Organ::Lantern || sp.plan == Plan::Lattice) {
            let b = 0.8 + 0.2 * (t * 2.0 + fx * 0.3 + fy * 0.2).sin();
            c = [(c[0] as f64 * b).min(255.0) as u8, (c[1] as f64 * b).min(255.0) as u8, (c[2] as f64 * b).min(255.0) as u8];
        }
        if !show {
            continue;
        }
        let q = to_screen(fx + dx, fy + dy);
        painter.rect_filled(egui::Rect::from_min_size(q, egui::vec2(cell_px, cell_px)), 0.0, c32(c));
    }

    // ---- Effects and the unlucky drone ----
    let Some(p) = atk else { return };
    let Some(k) = kind else { return };
    let top = to_screen(organ_c.0, organ_c.1);
    let pcount = 60;
    match k {
        Defence::Pods if p >= 0.35 => {
            let tt = (p - 0.35) * ATTACK_S;
            for i in 0..pcount {
                let a = unit(hash(s.seed, i, 0, 1)) * 6.283;
                let sp_ = (6.0 + 14.0 * unit(hash(s.seed, i, 0, 2))) * scale.min(8.0);
                let pos = egui::pos2(top.x + (a.cos() * sp_ * tt) as f32, top.y + (-a.sin() * sp_ * tt + 30.0 * scale.min(8.0) * tt * tt) as f32);
                let fade = (1.0 - (p - 0.35) / 0.65) as f32;
                painter.circle_filled(pos, (cell_px * 0.5).max(2.0), egui::Color32::from_rgba_unmultiplied(230, 180, 90, (255.0 * fade) as u8));
            }
        }
        Defence::Gas => {
            for i in 0..pcount {
                let start = unit(hash(s.seed, i, 0, 3)) * 0.4;
                if p < start {
                    continue;
                }
                let tt = (p - start) * ATTACK_S;
                let a = unit(hash(s.seed, i, 0, 4)) * 6.283;
                let pos = egui::pos2(top.x + (a.cos() * 10.0 * tt * scale.min(6.0)) as f32, top.y - (14.0 * tt * scale.min(6.0)) as f32 + (a.sin() * 6.0) as f32);
                let r = (4.0 + 18.0 * tt) as f32 * (scale.min(6.0) as f32 / 4.0).max(0.6);
                painter.circle_filled(pos, r, egui::Color32::from_rgba_unmultiplied(170, 210, 70, (90.0 * (1.0 - p)) as u8));
            }
        }
        Defence::Caustic | Defence::Resin => {
            let (col, up) = if k == Defence::Resin { ([255, 150, 40], 1.0) } else { ([140, 230, 90], -1.0) };
            for i in 0..pcount / 2 {
                let c = &s.cells[(hash(s.seed, i, 0, 5) as usize) % s.cells.len()];
                let start = unit(hash(s.seed, i, 0, 6)) * 0.5;
                if p < start {
                    continue;
                }
                let tt = (p - start) * ATTACK_S;
                let q = to_screen(c.0 as f64, c.1 as f64);
                let pos = egui::pos2(q.x, q.y + (-up * 25.0 * tt * scale.min(6.0)) as f32);
                painter.circle_filled(pos, (cell_px * 0.4).max(2.0), egui::Color32::from_rgba_unmultiplied(col[0], col[1], col[2], (220.0 * (1.0 - p)) as u8));
            }
        }
        Defence::Flash => {
            let r = (p * rmax * scale * 1.6) as f32;
            painter.circle_stroke(top, r, egui::Stroke::new(3.0, egui::Color32::from_rgba_unmultiplied(255, 255, 230, (200.0 * (1.0 - p)) as u8)));
        }
        _ => {}
    }
    // The drone: flies in, then is caught, knocked down or blown away.
    let start = egui::pos2(stage.max.x - 30.0, stage.min.y + stage.height() * 0.2);
    let hit = 0.35;
    let size = (cell_px * 1.5).clamp(5.0, 14.0);
    let drone = if p < hit {
        let u = smooth(p / hit) as f32;
        Some((start + (top - start) * u, egui::Color32::from_rgb(200, 205, 215)))
    } else if k == Defence::Snap {
        if p < 0.5 { Some((top, egui::Color32::from_rgb(200, 120, 110))) } else { None }
    } else {
        let u = ((p - hit) / (1.0 - hit)) as f32;
        let fall = egui::vec2(120.0 * u, 60.0 * u + 500.0 * u * u);
        Some((top + fall, egui::Color32::from_rgb(130, 120, 115)))
    };
    if let Some((pos, col)) = drone {
        let pts = vec![pos + egui::vec2(0.0, -size), pos + egui::vec2(size, 0.0), pos + egui::vec2(0.0, size), pos + egui::vec2(-size, 0.0)];
        painter.add(egui::Shape::convex_polygon(pts, col, egui::Stroke::NONE));
    }
}
