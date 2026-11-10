//! Per-thing random numbers for drawing: several independent values from
//! one hash. (Taking `noise::unit(h ^ small)` for a second value doesn't
//! work: unit reads the high bits, which a small xor leaves alone, so the
//! "second" value is nearly the first and things line up in stripes.)

/// A value in [0, 1) from hash `h` and `salt`, independent for each salt.
pub fn unit(h: u64, salt: u64) -> f64 {
    let mut z = h ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}
