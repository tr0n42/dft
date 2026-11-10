//! Deterministic hashing and value noise. Only add, multiply and divide are used
//! (no trig, no fused multiply-add), so results match on every x86-64 machine.

#[inline]
pub fn hash(seed: u64, a: i64, b: i64, c: u64) -> u64 {
    let mut z = seed
        ^ (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ c.wrapping_mul(0x1656_67B1_9E37_79F9);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Uniform in [0, 1).
#[inline]
pub fn unit(h: u64) -> f64 {
    (h >> 11) as f64 / (1u64 << 53) as f64
}

#[inline]
fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

/// Value noise in [0, 1), one lattice point per unit.
pub fn value(seed: u64, x: f64, y: f64) -> f64 {
    let (xf, yf) = (x.floor(), y.floor());
    let (xi, yi) = (xf as i64, yf as i64);
    let (tx, ty) = (smooth(x - xf), smooth(y - yf));
    let v = |dx: i64, dy: i64| unit(hash(seed, xi + dx, yi + dy, 0x51));
    let a = v(0, 0) + (v(1, 0) - v(0, 0)) * tx;
    let b = v(0, 1) + (v(1, 1) - v(0, 1)) * tx;
    a + (b - a) * ty
}

/// Fractal noise in roughly [0, 1): several octaves, each half the size.
pub fn fbm(seed: u64, x: f64, y: f64, octaves: u32) -> f64 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for o in 0..octaves {
        sum += amp * value(seed.wrapping_add(o as u64 * 7919), x * freq, y * freq);
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

/// Ridged noise: 1 on thin winding lines, falling to 0 away from them.
pub fn ridge(seed: u64, x: f64, y: f64) -> f64 {
    let n = fbm(seed, x, y, 3);
    1.0 - (2.0 * n - 1.0).abs()
}

/// x raised to an integer power by repeated squaring, in a fixed order of
/// operations. The standard `powi` is left to each platform's runtime, which
/// may round differently (Windows and Linux disagree in the last bit), and
/// the simulation must give identical results everywhere.
pub fn powi(x: f64, n: i32) -> f64 {
    let (mut base, mut e, mut acc) = (x, n.unsigned_abs(), 1.0f64);
    while e > 0 {
        if e & 1 == 1 {
            acc *= base;
        }
        base *= base;
        e >>= 1;
    }
    if n < 0 { 1.0 / acc } else { acc }
}

/// cos and sin of `a` radians from their power series after reducing the
/// angle to [-pi, pi]: plain arithmetic, the same on every machine (library
/// trig functions are not guaranteed to be).
pub fn cos_sin(a: f64) -> (f64, f64) {
    let tau = 2.0 * std::f64::consts::PI;
    let mut a = a - (a / tau).floor() * tau;
    if a > std::f64::consts::PI {
        a -= tau;
    }
    let (mut c, mut s, mut tc, mut ts) = (0.0, 0.0, 1.0, a);
    for k in 0..14 {
        c += tc;
        s += ts;
        let k2 = (2 * k + 1) as f64;
        tc *= -a * a / (k2 * (k2 + 1.0));
        ts *= -a * a / ((k2 + 1.0) * (k2 + 2.0));
    }
    (c, s)
}
