//! The world's glow (Benjamin, 26 Nov: "it's the way particles and lighting
//! work in the mockup that I want"): materials drawn with the game's own
//! shaders. `AddMaterial` adds what it draws over what is below (the
//! mockup's "lighter" compositing), for the light map and sparks: lit rock
//! brightens in its own colours rather than being tinted over.
use bevy::asset::uuid_handle;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor, SpecializedMeshPipelineError};
use bevy::shader::{Shader, ShaderRef};
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dKey, Material2dPipeline, Material2dPlugin};

pub const ADD_SHADER: Handle<Shader> = uuid_handle!("6b0b6f4e-3c1d-4a43-9d3e-1f0a7c5d2e01");

pub struct GlowPlugin;

impl Plugin for GlowPlugin {
    fn build(&self, app: &mut App) {
        app.world_mut().resource_mut::<Assets<Shader>>().insert(&ADD_SHADER, Shader::from_wgsl(include_str!("shaders/add.wgsl"), "shaders/add.wgsl")).ok();
        app.add_plugins(Material2dPlugin::<AddMaterial>::default());
    }
}

/// Drawn added over what is below (vertex colours, linear, times `strength.x`).
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct AddMaterial {
    #[uniform(0)]
    pub strength: Vec4,
}

impl Default for AddMaterial {
    fn default() -> Self {
        AddMaterial { strength: Vec4::new(1.0, 0.0, 0.0, 0.0) }
    }
}

impl Material2d for AddMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(ADD_SHADER)
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        // (Drawn with the see-through things, in order of depth.)
        AlphaMode2d::Blend
    }

    fn specialize(_pipeline: &Material2dPipeline, descriptor: &mut RenderPipelineDescriptor, _layout: &MeshVertexBufferLayoutRef, _key: Material2dKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(f) = descriptor.fragment.as_mut() {
            for t in f.targets.iter_mut().flatten() {
                t.blend = Some(BlendState { color: BlendComponent { src_factor: BlendFactor::One, dst_factor: BlendFactor::One, operation: BlendOperation::Add }, alpha: BlendComponent { src_factor: BlendFactor::Zero, dst_factor: BlendFactor::One, operation: BlendOperation::Add } });
            }
        }
        Ok(())
    }
}
