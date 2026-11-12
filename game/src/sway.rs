//! Plants swaying: near the camera (tiles of two texels a cell and finer),
//! plant cells that move are left out of the tiles and drawn here instead,
//! each block shifted by how far up its plant it is. The cells in the world
//! never move: this is drawing only, as the flora viewer shows it.
//! - Sway (trees, shrubs, stalks): the plant leans back and forth, more the
//!   higher up, in gusts that roll along the ground; leaves flutter.
//! - Ripple (grass and reeds): a quick ripple running along the ground.
//! - Drift (kelp and sea plants): a slow wave travelling up the plant.

use crate::tiles::SwayCell;
use crate::view::View;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::PrimitiveTopology;
use std::sync::Arc;

/// The plant cells of the tiles on screen (filled by tiles::place).
#[derive(Resource, Default)]
pub struct Shown(pub Vec<Arc<Vec<SwayCell>>>);

/// Just over the ground, under the drones.
const Z: f32 = 5.0;
/// The furthest a plant leans, in metres.
const MAX_LEAN_M: f64 = 3.0;

fn empty() -> Mesh {
    let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD);
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3]);
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; 3]);
    m
}

/// Where a cell is drawn at time `t`: its offset in metres, (across, up).
fn offset(c: &SwayCell, along: f64, t: f64) -> (f64, f64) {
    let h = c.h as f64;
    let amp = c.amp as f64;
    match c.motion {
        1 => {
            // A gust rolling along the ground, and a slower lean.
            let gust = (t * 0.9 - along / 30.0).sin() + 0.4 * (t * 2.3 - along / 11.0).sin();
            let mut dx = (amp * 0.05 * h.powf(1.3)).min(MAX_LEAN_M) * gust * 0.7;
            if c.leaf {
                dx += 0.2 * (t * 4.0 + along * 0.12 + h * 0.1).sin();
            }
            (dx, 0.0)
        }
        2 => (0.35 * (t * 3.2 - along * 0.6).sin() * (0.3 + (h / 3.0).min(1.0)), 0.0),
        3 => {
            let w = t * 1.1 - h * 0.25 - along / 20.0;
            ((amp * 0.07 * h).min(MAX_LEAN_M) * w.sin(), (amp * 0.02 * h).min(1.0) * (t * 0.8 - h * 0.2).cos())
        }
        _ => (0.0, 0.0),
    }
}

pub fn draw(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>, shown: Res<Shown>, view: Res<View>, time: Res<Time>, mut handle: Local<Option<Handle<Mesh>>>) {
    let h = handle.get_or_insert_with(|| {
        let h = meshes.add(empty());
        commands.spawn((Mesh2d(h.clone()), MeshMaterial2d(materials.add(ColorMaterial::default())), Transform::from_xyz(0.0, 0.0, Z), NoFrustumCulling));
        h
    });
    let Some(mut mesh) = meshes.get_mut(h.id()) else { return };
    let t = time.elapsed_secs_f64();
    let (l, b, r, top) = view.bounds();
    let margin = MAX_LEAN_M + 1.0;
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut col: Vec<[f32; 4]> = Vec::new();
    for list in &shown.0 {
        for c in list.iter() {
            let (x, y) = (c.x as f64, c.y as f64);
            if x < l - margin || x > r + margin || y < b - margin || y > top + margin {
                continue;
            }
            let (fx, fy) = (x + 0.5, y + 0.5);
            let rr = (fx * fx + fy * fy).sqrt().max(1e-9);
            let (ux, uy) = (fx / rr, fy / rr);
            let (tx, ty) = (-uy, ux);
            // Distance round the body, in metres: gusts roll along it.
            let along = fy.atan2(fx) * rr;
            let (dx, dy) = offset(c, along, t);
            let (ox, oy) = (tx * dx + ux * dy, ty * dx + uy * dy);
            // Whole pixels, so the blocks stay crisp and meet edge to edge.
            let a = view.to_screen(x + ox, y + oy);
            let z = view.to_screen(x + 1.0 + ox, y + 1.0 + oy);
            let (x0, y0, x1, y1) = (a.x.round(), a.y.round(), z.x.round().max(a.x.round() + 1.0), z.y.round().max(a.y.round() + 1.0));
            let k = Color::srgb_u8(c.c[0], c.c[1], c.c[2]).to_linear();
            let k = [k.red, k.green, k.blue, 1.0];
            pos.extend_from_slice(&[[x0, y0, 0.0], [x1, y0, 0.0], [x1, y1, 0.0], [x0, y0, 0.0], [x1, y1, 0.0], [x0, y1, 0.0]]);
            col.extend_from_slice(&[k; 6]);
        }
    }
    if pos.is_empty() {
        pos.extend_from_slice(&[[0.0; 3]; 3]);
        col.extend_from_slice(&[[0.0; 4]; 3]);
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
}
