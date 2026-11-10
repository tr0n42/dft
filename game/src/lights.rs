//! The swarm's lights: one colour, chosen by the player, that every machine
//! the swarm builds wears on its telltales (storage drones now; cutters and
//! haulers as they get their own looks). F8 (a cheat key for now) opens a
//! colour wheel with a luminosity slider.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

pub struct LightsPlugin;

impl Plugin for LightsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Lights>().add_systems(EguiPrimaryContextPass, wheel);
    }
}

#[derive(Resource)]
pub struct Lights {
    /// Hue and saturation (0 to 1) from the wheel, and luminosity (0 to 1).
    pub hue: f32,
    pub sat: f32,
    pub lum: f32,
    open: bool,
    /// Bumped on every change, for anything that bakes the colour in.
    pub version: u64,
}

impl Default for Lights {
    fn default() -> Self {
        // Cold phosphor green.
        Lights { hue: 0.37, sat: 0.9, lum: 0.8, open: false, version: 1 }
    }
}

/// Seconds for one slow pulse of the lights.
pub const PULSE_S: f64 = 4.0;
/// Pulses round the body at once: the lights brighten and dim in waves
/// that sweep round it in step, every machine in time with its neighbours.
const WAVES: f64 = 3.0;

impl Lights {
    /// The lights' colour (sRGB, opaque), at full strength.
    pub fn color(&self) -> Color {
        let c = egui::ecolor::Hsva::new(self.hue, self.sat, self.lum, 1.0).to_srgba_unmultiplied();
        Color::srgb_u8(c[0], c[1], c[2])
    }

    /// As sRGB 0-1 floats.
    pub fn rgb(&self) -> [f32; 3] {
        let c = self.color().to_srgba();
        [c.red, c.green, c.blue]
    }

    /// How strong the lights are at time `t` (real seconds) on a machine at
    /// angle `a` round the body: a slow pulse sweeping round in waves, never
    /// quite dark.
    pub fn pulse(t: f64, a: f64) -> f32 {
        let p = (std::f64::consts::TAU * t / PULSE_S - a * WAVES).sin();
        (0.62 + 0.38 * p) as f32
    }

    /// The lights' colour at time `t` on a machine at angle `a`.
    pub fn at(&self, t: f64, a: f64) -> Color {
        let k = Self::pulse(t, a);
        let c = self.rgb();
        Color::srgb(c[0] * k, c[1] * k, c[2] * k)
    }
}

fn wheel(mut contexts: EguiContexts, mut lights: ResMut<Lights>, keys: Res<ButtonInput<KeyCode>>) -> Result {
    if keys.just_pressed(KeyCode::F8) {
        lights.open = !lights.open;
    }
    if !lights.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let mut open = lights.open;
    let (mut hue, mut sat, mut lum) = (lights.hue, lights.sat, lights.lum);
    egui::Window::new("Swarm lights").id(egui::Id::new("lights-window")).open(&mut open).resizable(false).collapsible(false).default_pos([24.0, 420.0]).show(ctx, |ui| {
        // The wheel: hue round it, saturation out from the middle.
        let size = 180.0;
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click_and_drag());
        let c = rect.center();
        let rad = size * 0.5 - 2.0;
        let painter = ui.painter_at(rect);
        // (One mesh with a colour at each corner: smooth, no seams.)
        let rings = 24;
        let slices = 96;
        let mut mesh = egui::Mesh::default();
        let col = |h: f32, s: f32| egui::Color32::from(egui::ecolor::Hsva::new(h, s, lum.max(0.05), 1.0));
        for i in 0..=rings {
            for j in 0..=slices {
                let (rr, aa) = (rad * i as f32 / rings as f32, std::f32::consts::TAU * j as f32 / slices as f32);
                mesh.colored_vertex(c + egui::vec2(aa.cos(), aa.sin()) * rr, col(j as f32 / slices as f32, i as f32 / rings as f32));
            }
        }
        let row = (slices + 1) as u32;
        for i in 0..rings as u32 {
            for j in 0..slices as u32 {
                let (a, b2, c2, d) = (i * row + j, i * row + j + 1, (i + 1) * row + j + 1, (i + 1) * row + j);
                mesh.add_triangle(a, b2, c2);
                mesh.add_triangle(a, c2, d);
            }
        }
        painter.add(egui::Shape::mesh(mesh));
        if let Some(p) = resp.interact_pointer_pos() {
            let d = p - c;
            hue = (d.y.atan2(d.x) / std::f32::consts::TAU).rem_euclid(1.0);
            sat = (d.length() / rad).clamp(0.0, 1.0);
        }
        // Where the colour sits.
        let a = hue * std::f32::consts::TAU;
        let m = c + egui::vec2(a.cos(), a.sin()) * sat * rad;
        painter.circle_stroke(m, 6.0, egui::Stroke::new(2.0, egui::Color32::WHITE));
        painter.circle_stroke(m, 7.5, egui::Stroke::new(1.0, egui::Color32::BLACK));
        ui.add(egui::Slider::new(&mut lum, 0.05..=1.0).text("luminosity"));
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(egui::vec2(36.0, 18.0), egui::Sense::hover());
            ui.painter().rect_filled(r, 3.0, egui::Color32::from(egui::ecolor::Hsva::new(hue, sat, lum, 1.0)));
            if ui.button("phosphor green").clicked() {
                (hue, sat, lum) = (0.37, 0.9, 0.8);
            }
        });
        ui.label(egui::RichText::new("F8 to close").weak());
    });
    if (hue, sat, lum) != (lights.hue, lights.sat, lights.lum) {
        (lights.hue, lights.sat, lights.lum) = (hue, sat, lum);
        lights.version += 1;
    }
    lights.open = open;
    Ok(())
}
