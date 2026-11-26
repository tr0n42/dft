// Molten rock drawn alive (the swarm foundry mockup's lavaC): two waves
// running across it, crossing into bright veins, over a jitter of 2 m
// blocks that shift twice a second and a per-block fleck, stepped into
// five hard levels from dark crust to white-hot, scaled from the rock's
// own colour. Heat from the simulation (fresh flows hot, old ones cooling)
// pushes it brighter or darker.
struct LavaParams {
    // x, y: the tile's lower-left corner in blocks (offset and wrapped at
    // 65,536); z: its
    // side in blocks; w: the time (s). A block is a metre close up, about
    // four screen pixels zoomed out (the mockup's blocks at any zoom).
    origin: vec4<f32>,
    // x, y: the tile's corner from the heat grid's (m); z: the grid's side
    // (m); w: 1 when there is a heat grid.
    heat: vec4<f32>,
    // x: the tile's side (m).
    size: vec4<f32>,
};

@group(2) @binding(0) var<uniform> p: LavaParams;
@group(2) @binding(1) var tile_tex: texture_2d<f32>;
@group(2) @binding(2) var tile_smp: sampler;
@group(2) @binding(3) var heat_tex: texture_2d<f32>;
@group(2) @binding(4) var heat_smp: sampler;

struct FragmentIn {
    @builtin(position) position: vec4<f32>,
    @location(2) uv: vec2<f32>,
};

fn hash3(x: i32, y: i32, s: u32) -> f32 {
    var h: u32 = ((bitcast<u32>(x) * 374761393u) ^ (bitcast<u32>(y) * 668265263u)) ^ (s * 2246822519u);
    h = (h ^ (h >> 13u)) * 1274126177u;
    h = h ^ (h >> 16u);
    return f32(h) / 4294967296.0;
}

@fragment
fn fragment(in: FragmentIn) -> @location(0) vec4<f32> {
    let c = textureSample(tile_tex, tile_smp, in.uv);
    // Where in the world (m from the tile's corner, y up), on the picture's
    // texels.
    let span = p.size.x;
    let ts = span / 128.0;
    let up = vec2<f32>(in.uv.x, 1.0 - in.uv.y);
    let tc = floor(up * 128.0);
    let local = (tc + 0.5) * ts;
    let hp = p.heat.xy + local;
    let huv = vec2<f32>(hp.x / p.heat.z, 1.0 - hp.y / p.heat.z);
    let hv = textureSampleLevel(heat_tex, heat_smp, clamp(huv, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0).r;
    if (abs(c.a * 255.0 - 254.0) > 0.5) {
        discard;
    }
    var heat = 0.5;
    let inside = huv.x >= 0.0 && huv.x <= 1.0 && huv.y >= 0.0 && huv.y <= 1.0;
    if (p.heat.w > 0.5 && inside && hv > 0.002) {
        heat = (hv * 255.0 - 1.0) / 254.0;
    }
    // The block this texel is in (blocks are whole texels or more).
    let cell = floor(p.origin.xy + floor(up * p.origin.z));
    let w = cell + 0.5;
    let t = p.origin.w;
    var f = 0.5 + 0.22 * sin(0.9 * t + 0.21 * w.x + 0.13 * w.y) + 0.22 * sin(0.7 * t - 0.17 * w.x + 0.29 * w.y);
    f = f + 0.2 * (hash3(i32(floor(cell.x * 0.5)), i32(floor(cell.y * 0.5)) + i32(floor(2.0 * t)), 7u) - 0.5);
    f = f + 0.1 * (hash3(i32(cell.x), i32(cell.y), 11u) - 0.5);
    // Hotter rock is brighter, and only hot rock reaches white heat: rock
    // near its freezing point glows dull red at most.
    f = min(f + (heat - 0.5) * 0.4, 0.55 + heat * 0.6);
    let base = c.rgb;
    var col: vec3<f32>;
    if (f > 0.82) {
        col = mix(base, vec3<f32>(1.0, 0.75, 0.3), 0.75);
    } else if (f > 0.66) {
        col = mix(base, vec3<f32>(1.0, 0.45, 0.08), 0.35) * 1.15;
    } else if (f > 0.45) {
        col = base * 0.66;
    } else if (f > 0.30) {
        col = base * 0.32;
    } else {
        col = base * 0.14;
    }
    return vec4<f32>(col, 1.0);
}
