//! The view: where the camera looks, in the body's own metres (f64), and
//! how many metres one screen pixel covers. Bevy's camera itself never
//! moves: everything is drawn relative to this, so positions stay exact at
//! any distance from the body's centre.
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

#[derive(Resource, Clone, Copy, Debug)]
pub struct View {
    /// The point at the middle of the window, metres from the body's centre (+y up).
    pub x: f64,
    pub y: f64,
    /// Metres per screen pixel.
    pub mpp: f64,
    /// The window's size in pixels.
    pub w: f64,
    pub h: f64,
}

impl View {
    /// Screen pixel offset from the window's middle (+y up) of world point (x, y).
    pub fn to_screen(&self, x: f64, y: f64) -> Vec2 {
        Vec2::new(((x - self.x) / self.mpp) as f32, ((y - self.y) / self.mpp) as f32)
    }

    /// The world rectangle in view: (left, bottom, right, top).
    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        let (hw, hh) = (self.w * 0.5 * self.mpp, self.h * 0.5 * self.mpp);
        (self.x - hw, self.y - hh, self.x + hw, self.y + hh)
    }

    /// Fit the whole body (and its atmosphere) in the window.
    pub fn fit(&mut self, p: &dft_sim::planet::Planet) {
        let span = 2.0 * (p.max_radius + p.spec.atmosphere_m) * 1.06;
        self.x = 0.0;
        self.y = 0.0;
        self.mpp = span / self.w.min(self.h).max(1.0);
    }
}

pub struct ViewPlugin;

/// The player's camera controls run in this set (the director runs after).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Controls;

impl Plugin for ViewPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(View { x: 0.0, y: 0.0, mpp: 1.0, w: 1.0, h: 1.0 }).add_systems(Startup, start_view).add_systems(PreUpdate, (track_window, controls.run_if(not(bevy_egui::input::egui_wants_any_pointer_input))).chain().in_set(Controls));
    }
}

fn start_view(mut view: ResMut<View>, body: Res<crate::Body>, o: Res<crate::Options>, windows: Query<&Window, With<PrimaryWindow>>) {
    if let Ok(w) = windows.single() {
        view.w = w.width() as f64;
        view.h = w.height() as f64;
    }
    match o.view {
        Some((x, y, mpp)) => {
            view.x = x;
            view.y = y;
            view.mpp = mpp;
        }
        None => view.fit(&body.0),
    }
}

fn track_window(mut view: ResMut<View>, windows: Query<&Window, With<PrimaryWindow>>) {
    if let Ok(w) = windows.single() {
        view.w = w.width() as f64;
        view.h = w.height() as f64;
    }
}

/// Either mouse button (or the arrow keys / WASD) drags the view; the wheel
/// (or Page Up / Page Down) zooms toward the cursor. Any of them takes the
/// camera from the director, if it's on (C toggles it). Middle click or Home
/// hands it back (Home fits the whole body when there's no director).
#[allow(clippy::too_many_arguments)]
fn controls(
    mut cam: ResMut<crate::camera::Camera>,
    time: Res<Time<Real>>,
    mut view: ResMut<View>,
    body: Res<crate::Body>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let now = time.elapsed_secs_f64();
    // C: the action cam on or off.
    if keys.just_pressed(KeyCode::KeyC) && cam.can_direct {
        cam.director = !cam.director;
        cam.resume();
    }
    // Pan with the keyboard (arrows or WASD), a screen's width in about 1.5 s.
    let dt = time.delta_secs_f64().min(0.1);
    let (mut kx, mut ky) = (0.0, 0.0);
    if keys.any_pressed([KeyCode::ArrowLeft, KeyCode::KeyA]) {
        kx -= 1.0;
    }
    if keys.any_pressed([KeyCode::ArrowRight, KeyCode::KeyD]) {
        kx += 1.0;
    }
    if keys.any_pressed([KeyCode::ArrowUp, KeyCode::KeyW]) {
        ky += 1.0;
    }
    if keys.any_pressed([KeyCode::ArrowDown, KeyCode::KeyS]) {
        ky -= 1.0;
    }
    if kx != 0.0 || ky != 0.0 {
        cam.manual(now);
        let speed = view.w.max(view.h) * view.mpp / 1.5;
        view.x += kx * speed * dt;
        view.y += ky * speed * dt;
    }
    if keys.just_pressed(KeyCode::Home) || buttons.just_pressed(MouseButton::Middle) {
        if cam.director {
            cam.resume();
        } else {
            view.fit(&body.0);
        }
    }
    if (buttons.pressed(MouseButton::Right) || buttons.pressed(MouseButton::Left)) && motion.delta != Vec2::ZERO {
        cam.manual(now);
        view.x -= motion.delta.x as f64 * view.mpp;
        view.y += motion.delta.y as f64 * view.mpp;
    }
    let mut notches = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y as f64,
        MouseScrollUnit::Pixel => scroll.delta.y as f64 / 40.0,
    };
    if keys.just_pressed(KeyCode::PageUp) {
        notches += 2.0;
    }
    if keys.just_pressed(KeyCode::PageDown) {
        notches -= 2.0;
    }
    if notches != 0.0 {
        cam.manual(now);
        let f = 0.8f64.powf(notches);
        // Bounds: a block 64 pixels across at most; as far out as the zoom
        // limit allows (twice the whole body at most).
        let max_mpp = (4.0 * (body.0.max_radius + body.0.spec.atmosphere_m) / view.w.min(view.h).max(1.0)).min(cam.max_mpp.max(view.mpp.min(1.0)));
        let new = (view.mpp * f).clamp(1.0 / 64.0, max_mpp.max(1.0 / 64.0));
        // Keep the point under the cursor still.
        if let Some(c) = windows.single().ok().and_then(|w| w.cursor_position()) {
            let (cx, cy) = (c.x as f64 - view.w / 2.0, view.h / 2.0 - c.y as f64);
            view.x += cx * (view.mpp - new);
            view.y += cy * (view.mpp - new);
        }
        view.mpp = new;
    }
}
