//! Molten rock drawn alive (Benjamin, 26 Nov: "it's the post-processing and
//! variation in the lava's flows that I like from the mockup"; "flows of
//! higher incandescence due to temperature"). Tiles mark their molten
//! texels (tiles.rs, LAVA_ALPHA); over each tile on screen with any, an
//! overlay draws them with shaders/lava.wgsl every frame, brighter where
//! the simulation says the rock is hotter (its heat grid, sim.rs), and the
//! molten rock lights what is round it.
use crate::tiles::{Tile, Tiles};
use bevy::asset::{uuid_handle, RenderAssetUsages};
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat};
use bevy::shader::{Shader, ShaderRef};
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dPlugin};
use std::collections::{HashMap, HashSet};

pub const LAVA_SHADER: Handle<Shader> = uuid_handle!("6b0b6f4e-3c1d-4a43-9d3e-1f0a7c5d2e02");

/// The heat grid's texture side (texels; sim.rs's HEAT_N).
const HEAT_TEX: u32 = 256;
/// The molten rock's light: colour and strength.
const LIGHT: [f32; 3] = [255.0, 120.0, 40.0];
const LIGHT_I: f32 = 0.35;
/// The lava pattern repeats every this many metres (shaders/lava.wgsl's PERIOD).
const PERIOD_M: f64 = 65536.0;

pub struct LavaPlugin;

impl Plugin for LavaPlugin {
    fn build(&self, app: &mut App) {
        app.world_mut().resource_mut::<Assets<Shader>>().insert(&LAVA_SHADER, Shader::from_wgsl(include_str!("shaders/lava.wgsl"), "shaders/lava.wgsl")).ok();
        app.add_plugins(Material2dPlugin::<LavaMaterial>::default()).add_systems(Startup, setup).add_systems(Update, (take_heat, overlays).chain().after(crate::tiles::place));
    }
}

#[derive(ShaderType, Clone, Copy, Debug, Default)]
pub struct LavaParams {
    origin: Vec4,
    heat: Vec4,
    size: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct LavaMaterial {
    #[uniform(0)]
    p: LavaParams,
    #[texture(1)]
    #[sampler(2)]
    tile: Handle<Image>,
    #[texture(3)]
    #[sampler(4)]
    heat: Handle<Image>,
}

impl Material2d for LavaMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(LAVA_SHADER)
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

#[derive(Component)]
struct LavaOverlay;

/// The heat grid as last taken: its texture, where it starts (m) and its
/// side (m); and each tile's overlay.
#[derive(Resource)]
struct Lava {
    heat: Handle<Image>,
    at: (f64, f64),
    side: f64,
    has_heat: bool,
    /// When it was taken (real s): a grid gone stale (zoomed out, where the
    /// simulation stops sending it) is no longer used.
    heat_at: f64,
    quad: Handle<Mesh>,
    overlays: HashMap<crate::tiles::Key, (Entity, Handle<LavaMaterial>)>,
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut meshes: ResMut<Assets<Mesh>>) {
    let mut img = Image::new(Extent3d { width: HEAT_TEX, height: HEAT_TEX, depth_or_array_layers: 1 }, TextureDimension::D2, vec![0u8; (HEAT_TEX * HEAT_TEX) as usize], TextureFormat::R8Unorm, RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD);
    img.sampler = ImageSampler::nearest();
    let heat = images.add(img);
    let quad = meshes.add(Rectangle::new(1.0, 1.0));
    commands.insert_resource(Lava { heat, at: (0.0, 0.0), side: 1.0, has_heat: false, heat_at: f64::NEG_INFINITY, quad, overlays: HashMap::new() });
}

/// The simulation's newest heat grid, into the texture.
fn take_heat(run: Option<Res<crate::Run>>, mut lava: ResMut<Lava>, mut images: ResMut<Assets<Image>>, time: Res<Time<Real>>) {
    let Some(run) = run else { return };
    let Some(h) = run.0.out.lock().heat.take() else { return };
    let n = h.n.min(HEAT_TEX as usize);
    let mut data = vec![0u8; (HEAT_TEX * HEAT_TEX) as usize];
    for j in 0..n {
        // (Rows run top down: the grid's bottom row is the texture's last.)
        let row = HEAT_TEX as usize - 1 - j;
        data[row * HEAT_TEX as usize..row * HEAT_TEX as usize + n].copy_from_slice(&h.data[j * h.n..j * h.n + n]);
    }
    if let Some(mut img) = images.get_mut(&lava.heat) {
        img.data = Some(data);
    }
    lava.at = (h.x0, h.y0);
    lava.side = h.g * HEAT_TEX as f64;
    lava.has_heat = true;
    lava.heat_at = time.elapsed_secs_f64();
}

/// An overlay over every tile on screen with molten rock, laid exactly
/// over it; the molten rock's light.
#[allow(clippy::type_complexity)]
fn overlays(
    mut commands: Commands,
    tiles: Res<Tiles>,
    mut lava: ResMut<Lava>,
    mut materials: ResMut<Assets<LavaMaterial>>,
    q: Query<(&Tile, &Transform, &Sprite, &Visibility), Without<LavaOverlay>>,
    mut shown: Query<(&mut Transform, &mut Visibility), (With<LavaOverlay>, Without<Tile>)>,
    time: Res<Time<Real>>,
) {
    let t = (time.elapsed_secs_f64() % 3600.0) as f32;
    let mut seen: HashSet<crate::tiles::Key> = HashSet::new();
    for (tile, tf, sprite, vis) in q.iter() {
        if *vis != Visibility::Visible {
            continue;
        }
        let k = tile.0;
        let Some((img, l)) = tiles.lava(&k) else { continue };
        seen.insert(k);
        let s = k.span();
        let (x0, y0) = (k.ix as f64 * s, k.iy as f64 * s);
        let p = LavaParams {
            // The tile's corner in metres, wrapped every 65,536 m (the
            // pattern repeats at that, so it has no seam).
            origin: Vec4::new(x0.rem_euclid(PERIOD_M) as f32, y0.rem_euclid(PERIOD_M) as f32, 0.0, t),
            heat: Vec4::new((x0 - lava.at.0) as f32, (y0 - lava.at.1) as f32, lava.side as f32, if lava.has_heat && time.elapsed_secs_f64() - lava.heat_at < 1.0 { 1.0 } else { 0.0 }),
            size: Vec4::new(s as f32, 0.0, 0.0, 0.0),
        };
        let size = sprite.custom_size.unwrap_or(Vec2::ONE);
        let place = Transform::from_translation(tf.translation + Vec3::Z * 0.4).with_scale(size.extend(1.0));
        let existing = lava.overlays.get(&k).cloned();
        match existing {
            Some((e, h)) => {
                if let Some(mut m) = materials.get_mut(&h) {
                    m.p = p;
                    if m.tile != img {
                        m.tile = img.clone();
                    }
                }
                if let Ok((mut otf, mut ov)) = shown.get_mut(e) {
                    *otf = place;
                    if *ov != Visibility::Visible {
                        *ov = Visibility::Visible;
                    }
                }
            }
            None => {
                let h = materials.add(LavaMaterial { p, tile: img.clone(), heat: lava.heat.clone() });
                let e = commands.spawn((Mesh2d(lava.quad.clone()), MeshMaterial2d(h.clone()), place, Visibility::Visible, LavaOverlay)).id();
                lava.overlays.insert(k, (e, h));
            }
        }
        // The light off it: from where it is molten, spilling a few metres
        // round, the same in metres at every zoom (fx::glow).
        let m = crate::tiles::LAVA_MASK;
        let q = s / m as f64;
        for (o, &n) in l.mask.iter().enumerate() {
            if n > 0 {
                crate::fx::glow(x0 + (o % m) as f64 * q, y0 + (o / m) as f64 * q, q, n as f32 / 64.0, LIGHT, LIGHT_I);
            }
        }
    }
    // Overlays of tiles not on screen now: hidden, and forgotten once there
    // are many.
    for (k, (e, _)) in lava.overlays.iter() {
        if !seen.contains(k) {
            if let Ok((_, mut ov)) = shown.get_mut(*e) {
                if *ov != Visibility::Hidden {
                    *ov = Visibility::Hidden;
                }
            }
        }
    }
    if lava.overlays.len() > 600 {
        let gone: Vec<_> = lava.overlays.keys().filter(|k| !seen.contains(*k)).copied().collect();
        for k in gone {
            if let Some((e, h)) = lava.overlays.remove(&k) {
                commands.entity(e).despawn();
                materials.remove(&h);
            }
        }
    }
}
