//! Cargo you can see: a hold drawn full of the materials actually brought
//! up, in their own colours and in proportion to how much of each came. The
//! storage drones use it now; haulers will next.

/// What's been brought: each material's colour and how much of it (kg).
pub type Mix = Vec<([u8; 3], f64)>;

fn unit(x: u64, y: u64, salt: u64) -> f64 {
    dft_sim::noise::unit(dft_sim::noise::hash(x, y as i64, salt as i64, 0xCA_60))
}

/// The colour of each cell of a hold `w` x `h` cells (row by row), packed
/// with `mix`: each 2 x 2 clump of cells is one material, picked in
/// proportion to its share (so a trace shows as the odd clump), every cell
/// with the ground's per-cell grain. `seed` makes each hold its own.
pub fn ore_cells(mix: &Mix, w: usize, h: usize, seed: u64) -> Vec<[u8; 3]> {
    let total: f64 = mix.iter().map(|m| m.1.max(0.0)).sum();
    let mut out = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let base = if total <= 0.0 {
                [70, 66, 60]
            } else {
                // The clump's material.
                let u = unit(seed ^ (x / 2) as u64, (y / 2) as u64, 1) * total;
                let mut acc = 0.0;
                let mut pick = mix[0].0;
                for (c, kg) in mix {
                    acc += kg.max(0.0);
                    if u < acc {
                        pick = *c;
                        break;
                    }
                }
                pick
            };
            // Compacted: a little darker, and grainy.
            let g = 0.62 + 0.3 * unit(seed ^ x as u64, y as u64, 2);
            out.push([(base[0] as f64 * g) as u8, (base[1] as f64 * g) as u8, (base[2] as f64 * g) as u8]);
        }
    }
    out
}
