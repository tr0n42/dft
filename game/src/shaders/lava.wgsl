// Molten rock drawn alive, the swarm foundry mockup's way (stepped into
// five hard levels from dark crust to white-hot, scaled from the rock's own
// colour), as a cooling crust: plates of it with bright cracks between,
// bent by slow currents and drifting, a boil moving through it, and broad
// flows brighter and duller.
// Heat from the simulation (fresh flows hot, old ones cooling) pushes it
// brighter or darker.
//
// Everything is laid out in metres of the world, sampled at the texel (a
// metre close up), so the pattern is the same at every zoom: zooming out
// only drops what is finer than a texel or two (Benjamin, 26 Nov: it looked
// like a checkerboard, and different at each zoom).
struct LavaParams {
    // x, y: the tile's lower-left corner (m, wrapped every PERIOD m); w: the
    // time (s).
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

// The pattern repeats every this many metres (lava.rs's PERIOD_M).
const PERIOD: f32 = 65536.0;
const TAU: f32 = 6.2831853;

fn hash3(x: i32, y: i32, s: u32) -> f32 {
    var h: u32 = ((bitcast<u32>(x) * 374761393u) ^ (bitcast<u32>(y) * 668265263u)) ^ (s * 2246822519u);
    h = (h ^ (h >> 13u)) * 1274126177u;
    h = h ^ (h >> 16u);
    return f32(h) / 4294967296.0;
}

fn wrap(a: i32, n: i32) -> i32 {
    return ((a % n) + n) % n;
}

// Smooth noise (0..1) on a lattice of `s` m (a power of two), repeating
// every PERIOD m.
fn vnoise(q0: vec2<f32>, s: f32, seed: u32) -> f32 {
    let q = q0 / s;
    let i = floor(q);
    let f = q - i;
    let u = f * f * (3.0 - 2.0 * f);
    let n = i32(PERIOD / s);
    let x0 = wrap(i32(i.x), n);
    let y0 = wrap(i32(i.y), n);
    let x1 = wrap(x0 + 1, n);
    let y1 = wrap(y0 + 1, n);
    let a = hash3(x0, y0, seed);
    let b = hash3(x1, y0, seed);
    let c = hash3(x0, y1, seed);
    let d = hash3(x1, y1, seed);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// A wave number (rad/m) moved to the nearest that repeats every PERIOD m.
fn kq(k: f32) -> f32 {
    return round(k * PERIOD / TAU) * TAU / PERIOD;
}

// How much of a feature `l` m across shows when `e` m is the finest that
// can show (a texel or a screen pixel): all of it from eight up, none under
// four (rather than shimmer or a weave).
fn lod(l: f32, e: f32) -> f32 {
    return clamp(l / e * 0.25 - 1.0, 0.0, 1.0);
}

// Noise on a lattice turned (by 27 degrees, and finer by the square root of
// five) so its grain doesn't run along the screen; still repeating every
// PERIOD m.
fn tnoise(q: vec2<f32>, s: f32, seed: u32) -> f32 {
    return vnoise(vec2<f32>(2.0 * q.x - q.y, q.x + 2.0 * q.y), s, seed);
}

// Cracks much bigger than the screen are toned down (a third left at
// several screens across): a crack 900 m wide doesn't wash a close view
// white-hot.
fn big(l: f32, e: f32) -> f32 {
    return clamp(1.5 - l / (e * 600.0), 0.35, 1.0);
}

// A crack network: 1 along the lines where the noise crosses its middle,
// falling off fast either side.
fn cracks(n: f32) -> f32 {
    let r = 1.0 - abs(2.0 * n - 1.0);
    return r * r * r * r;
}

@fragment
fn fragment(in: FragmentIn) -> @location(0) vec4<f32> {
    let c = textureSample(tile_tex, tile_smp, in.uv);
    let span = p.size.x;
    // A texel's side (m), and the blocks the pattern is drawn in: texels,
    // but never finer than a metre (the world's cells).
    let ts = span / 128.0;
    let blk = max(ts, 1.0);
    let up = vec2<f32>(in.uv.x, 1.0 - in.uv.y);
    // A screen pixel's side (m), and the finest that can show.
    let pix = max(fwidth(up.x), fwidth(up.y)) * span;
    let e = max(blk, pix);
    let local = (floor(up * span / blk) + 0.5) * blk;
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
    let t = p.origin.w;
    // Where in the world (m).
    let at = p.origin.xy + local;
    // Slow currents bend the whole flow.
    let bend = vec2<f32>(vnoise(at + vec2<f32>(0.0, t * 0.8), 64.0, 1u), vnoise(at + vec2<f32>(t * 0.7, 0.0), 64.0, 2u)) - 0.5;
    let w = at + bend * 36.0;
    // The crust: plates with bright cracks between, at every size from 7 m
    // to 900 m (each crack network's mean taken off, so the whole is as
    // bright at any zoom; at any zoom the ones a few pixels and more across
    // show), drifting slowly with the flow.
    let drift = w + vec2<f32>(t * 0.35, t * 0.12);
    var f = 0.52;
    f = f + 0.20 * lod(7.0, e) * big(7.0, e) * (cracks(tnoise(drift + vec2<f32>(5.0, 9.0), 16.0, 4u)) - 0.2);
    f = f + 0.40 * lod(14.0, e) * big(14.0, e) * (cracks(tnoise(drift, 32.0, 3u)) - 0.2);
    f = f + 0.34 * lod(57.0, e) * big(57.0, e) * (cracks(tnoise(drift + vec2<f32>(41.0, 17.0), 128.0, 9u)) - 0.2);
    f = f + 0.30 * lod(229.0, e) * big(229.0, e) * (cracks(tnoise(w + vec2<f32>(300.0, 120.0), 512.0, 10u)) - 0.2);
    f = f + 0.30 * lod(916.0, e) * big(916.0, e) * (cracks(tnoise(at + vec2<f32>(900.0, 2100.0), 2048.0, 12u)) - 0.2);
    // A boil moving through it (in place of the mockup's blocks flickering
    // twice a second).
    f = f + 0.10 * lod(4.0, e) * (vnoise(w - vec2<f32>(t * 2.0, t * 1.4), 4.0, 7u) - 0.5);
    // Each metre a little its own (only close up, where metres show).
    let cell = floor(at);
    f = f + 0.06 * clamp(2.0 - e, 0.0, 1.0) * (hash3(i32(cell.x), i32(cell.y), 11u) - 0.5);
    // Broad flows, brighter and duller.
    f = f + 0.18 * lod(128.0, e) * (vnoise(w, 128.0, 5u) - 0.5);
    f = f + 0.16 * lod(1024.0, e) * (vnoise(at, 1024.0, 6u) - 0.5);
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
