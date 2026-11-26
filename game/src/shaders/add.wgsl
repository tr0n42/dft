// Light added over what is drawn below it (the mockup's "lighter"
// compositing): each vertex carries the light to add, in linear colour.
struct AddMaterial {
    strength: vec4<f32>,
};

@group(2) @binding(0) var<uniform> material: AddMaterial;

struct FragmentIn {
    @builtin(position) position: vec4<f32>,
    @location(4) color: vec4<f32>,
};

@fragment
fn fragment(in: FragmentIn) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color.rgb * material.strength.x, 1.0);
}
