//! The sky behind everything: a field of stars and a faint haze, made once
//! and tiled across the window. Space is infinitely far away, so panning
//! barely moves it; circling the body turns it slowly, which reads as motion
//! without pulling the eye off the ground.
use crate::view::View;
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use dft_sim::noise::{hash, unit};

/// The star image's size (it repeats).
const SIZE: u32 = 1024;
/// Pixels the sky slides per radian of the camera's way round the body.
const TURN_PX: f64 = 700.0;

pub struct StarsPlugin;

impl Plugin for StarsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, follow);
    }
}

#[derive(Component)]
struct Sky;

fn make(seed: u64) -> Vec<u8> {
    let n = SIZE as usize;
    let mut px = vec![0u8; n * n * 4];
    // A faint haze: soft noise on a lattice that wraps at the image's edge,
    // two octaves, so the repeat never shows a seam.
    let wrap_noise = |seed: u64, period: i64, x: f64, y: f64| -> f64 {
        let (xi, yi) = (x.floor() as i64, y.floor() as i64);
        let (tx, ty) = (x - xi as f64, y - yi as f64);
        let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
        let at = |i: i64, j: i64| unit(hash(seed, i.rem_euclid(period), j.rem_euclid(period), 9));
        let top = at(xi, yi) + (at(xi + 1, yi) - at(xi, yi)) * sx;
        let bot = at(xi, yi + 1) + (at(xi + 1, yi + 1) - at(xi, yi + 1)) * sx;
        top + (bot - top) * sy
    };
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f64 / n as f64, y as f64 / n as f64);
            let h1 = wrap_noise(seed ^ 0x51, 5, fx * 5.0, fy * 5.0);
            let h2 = wrap_noise(seed ^ 0x52, 13, fx * 13.0, fy * 13.0);
            let haze = ((h1 * 0.7 + h2 * 0.3) - 0.55).max(0.0) * 2.2;
            let o = (y * n + x) * 4;
            px[o] = (6.0 + 10.0 * haze) as u8;
            px[o + 1] = (7.0 + 6.0 * haze) as u8;
            px[o + 2] = (12.0 + 18.0 * haze) as u8;
            px[o + 3] = 255;
        }
    }
    // Stars: mostly dim specks, a few brighter, the odd one tinted.
    for k in 0..2600u64 {
        let h = hash(seed ^ 0x57A2, k as i64, 0, 3);
        let (x, y) = ((unit(h) * n as f64) as usize, (unit(hash(h, 1, 0, 0)) * n as f64) as usize);
        let b = unit(hash(h, 2, 0, 0)).powi(5);
        let lum = 70.0 + 185.0 * b;
        let tint = unit(hash(h, 3, 0, 0));
        let (r, g, bl) = if tint < 0.1 { (1.0, 0.82, 0.7) } else if tint < 0.2 { (0.75, 0.85, 1.0) } else { (0.95, 0.95, 1.0) };
        let mut put = |x: usize, y: usize, f: f64| {
            let o = ((y % n) * n + (x % n)) * 4;
            px[o] = px[o].max((lum * r * f) as u8);
            px[o + 1] = px[o + 1].max((lum * g * f) as u8);
            px[o + 2] = px[o + 2].max((lum * bl * f) as u8);
        };
        put(x, y, 1.0);
        // The brightest get a soft cross.
        if b > 0.35 {
            for (dx, dy) in [(1, 0), (n - 1, 0), (0, 1), (0, n - 1)] {
                put(x + dx, y + dy, 0.35);
            }
        }
    }
    px
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, body: Res<crate::Body>) {
    let mut img = Image::new(Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 }, TextureDimension::D2, make(body.0.spec.seed), TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD);
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor { address_mode_u: ImageAddressMode::Repeat, address_mode_v: ImageAddressMode::Repeat, ..ImageSamplerDescriptor::nearest() });
    let h = images.add(img);
    commands.spawn((Sprite { image: h, custom_size: Some(Vec2::splat(SIZE as f32)), rect: Some(Rect::new(0.0, 0.0, SIZE as f32, SIZE as f32)), ..default() }, Transform::from_xyz(0.0, 0.0, -500.0), Sky));
}

fn follow(view: Res<View>, mut q: Query<&mut Sprite, With<Sky>>) {
    let Ok(mut sprite) = q.single_mut() else { return };
    let (w, h) = (view.w as f32, view.h as f32);
    let turn = (view.y.atan2(view.x) * TURN_PX) as f32;
    let ox = turn.rem_euclid(SIZE as f32);
    sprite.custom_size = Some(Vec2::new(w, h));
    sprite.rect = Some(Rect::new(ox, 0.0, ox + w, h));
}
